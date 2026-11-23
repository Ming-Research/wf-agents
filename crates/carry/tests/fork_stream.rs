//! What the command line prints for a fork given two messages, a first one and the question, read as the engine reads it.
//! The oracle is the command line's own output: `fixtures/stream.jsonl` is a real stream of one message, and a stream of
//! two messages is that stream twice, as the command line prints one result for each message it is given (measured in
//! `research/fork-cache-shared-history.md`). The streams are read in memory, line by line: a few milliseconds each.

use carry::cli::{self, Begin, Stream};
use serde_json::{Value, json};

/// The lines of the real one-message stream.
fn one_message() -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stream.jsonl");
    let text = std::fs::read_to_string(path).expect("the fixture is read");
    text.lines().filter(|l| !l.is_empty()).map(str::to_string).collect()
}

/// The lines the command line prints for a second message of the same session: no first line again, and the ids of the
/// calls and of the tools' uses its own.
fn next_message() -> Vec<String> {
    one_message()
        .into_iter()
        .skip(1)
        .map(|line| line.replace("msg_011", "msg_022").replace("toolu_01", "toolu_02"))
        .collect()
}

fn feed(lines: &[String]) -> Stream {
    let mut stream = Stream::default();
    for line in lines {
        stream.line(line.as_bytes());
    }
    stream
}

/// The index of the line of the result.
fn result_at(lines: &[String]) -> usize {
    lines
        .iter()
        .position(|line| line.contains(r#""type":"result""#))
        .expect("the fixture holds a result")
}

#[test]
fn two_messages_of_the_real_stream_read_as_two_results_and_the_return_of_the_second() {
    let single = cli::read(one_message().join("\n").as_bytes(), Begin::New, 0).expect("one message reads");
    let mut lines = one_message();
    lines.extend(next_message());
    let stream = feed(&lines);
    assert_eq!(stream.results(), 2);
    let read = stream.read(Begin::Resume, 0).expect("the question's return is read");
    assert_eq!(read.structured, single.structured, "the return of the last message");
    // The tokens of both messages, and the total of the last result as the cost: the command line's total is its whole
    // call's.
    assert_eq!(read.spent.input_tokens, 2 * single.spent.input_tokens);
    assert_eq!(read.spent.cached_tokens, 2 * single.spent.cached_tokens);
    assert_eq!(read.spent.output_tokens, 2 * single.spent.output_tokens);
    assert_eq!(read.spent.usd_micros, single.spent.usd_micros);
    assert_eq!(read.spent.context_tokens, single.spent.context_tokens);
}

#[test]
fn a_first_message_the_command_line_ended_in_error_does_not_stop_the_question_being_read() {
    // The first request was stopped (a safety check): its result is an error with no return. The question went on.
    let mut first = one_message();
    let at = result_at(&first);
    let mut result: Value = serde_json::from_str(&first[at]).expect("JSON");
    result["is_error"] = json!(true);
    result["subtype"] = json!("error_during_execution");
    result.as_object_mut().expect("an object").remove("structured_output");
    first[at] = result.to_string();
    first.truncate(at + 1);
    let single = cli::read(one_message().join("\n").as_bytes(), Begin::New, 0).expect("one message reads");
    first.extend(next_message());
    let stream = feed(&first);
    assert_eq!(stream.results(), 2);
    let read = stream.read(Begin::Resume, 0).expect("the question's return is read");
    assert_eq!(read.structured, single.structured);
    assert_eq!(read.spent.output_tokens, 2 * single.spent.output_tokens);
}

#[test]
fn a_result_after_a_result_is_a_second_message_ended_whatever_lines_came_between() {
    // The command line prints one result for each message it is given, also for one it ends before the model answers
    // (a slash command, a prompt it refuses): the fork's question is then answered with nothing but its result.
    let mut lines = one_message();
    let mut again: Value = serde_json::from_str(&lines[result_at(&lines)]).expect("JSON");
    again["total_cost_usd"] = json!(0.5);
    again["is_error"] = json!(true);
    lines.push(again.to_string());
    assert_eq!(feed(&lines).results(), 2);
}

#[test]
fn a_program_that_ended_while_the_question_was_answered_has_one_result_and_no_return() {
    let mut lines = one_message();
    // The second message's lines up to its first call's message: it ends there.
    lines.extend(next_message().into_iter().take(2));
    let stream = feed(&lines);
    assert_eq!(stream.results(), 1, "only the first message ended");
    let no = stream.read(Begin::Resume, 0).expect_err("the question has no result");
    assert_eq!(no.why, "the command line's stream holds no result");
    assert!(no.spent.is_some(), "the first message's calls were paid for");
}

#[test]
fn the_question_is_read_by_its_own_attempts_and_not_by_those_of_the_first_message() {
    // The question's message makes no attempt at a return, and its result holds none: the first message's attempt is no
    // return to it.
    let mut lines = one_message();
    let second = next_message();
    let at = result_at(&second);
    let mut result: Value = serde_json::from_str(&second[at]).expect("JSON");
    result.as_object_mut().expect("an object").remove("structured_output");
    // A message of the question that is only a plain answer: its start, a call without a use of a tool, its result.
    let start = second
        .iter()
        .find(|l| l.starts_with(r#"{"type":"stream_event""#) && l.contains(r#""message_start""#))
        .expect("a start")
        .clone();
    lines.push(start);
    lines.push(result.to_string());
    let stream = feed(&lines);
    assert_eq!(stream.results(), 2);
    let no = stream.read(Begin::Resume, 0).expect_err("no return to the question");
    assert_eq!(no.why, "the command line's result holds no structured return");
}

#[test]
fn a_fork_is_given_two_lines_whatever_its_question_holds() {
    // A message is a line of the standard input (JSON Lines): a question of several lines, with quotes and the
    // separators JSON leaves unescaped, is still one.
    let question = "Why \"this\"?\nAnd why not\r\nthat\u{2028}one?\n\n";
    let bytes = cli::fork_stdin("# A question follows", question);
    assert!(bytes.ends_with(b"\n"));
    let lines: Vec<&[u8]> = bytes.split(|b| *b == b'\n').filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 2);
    let second: Value = serde_json::from_slice(lines[1]).expect("JSON");
    assert_eq!(second["message"]["content"][0]["text"], question);
}
