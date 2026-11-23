//! What a stream of the agents' program costs, as `carry::cli::Stream` works it out. The oracle of each test is the
//! program's own account of what it spent, which the real streams kept in `fixtures/` carry in their results
//! (`model_usage_since_last_result`, and the `usage` of each call's `message_delta`), or the shape of a result the
//! program printed in a real round, as each test says; none is the engine's own earlier output.

use carry::cli::Stream;
use serde_json::{Value, json};

/// The real stream of a session's first round, with its two calls and its result.
const STREAM: &str = include_str!("fixtures/stream.jsonl");

/// The real stream of a round the command line gave up on: two calls and a result.
const GAVE_UP: &str = include_str!("fixtures/gave-up.jsonl");

fn lines(text: &str) -> Vec<String> {
    text.lines().filter(|l| !l.is_empty()).map(str::to_string).collect()
}

fn feed(lines: &[String]) -> Stream {
    let mut stream = Stream::default();
    for line in lines {
        stream.line(line.as_bytes());
    }
    stream
}

fn is_result(line: &str) -> bool {
    serde_json::from_str::<Value>(line).is_ok_and(|v| v["type"] == "result")
}

/// The lines with the stream's one result changed as `change` says.
fn with_result(lines: &[String], change: impl FnOnce(&mut Value)) -> Vec<String> {
    let mut lines = lines.to_vec();
    let at = lines
        .iter()
        .position(|l| is_result(l))
        .expect("the stream holds a result");
    let mut result: Value = serde_json::from_str(&lines[at]).expect("JSON");
    change(&mut result);
    lines[at] = result.to_string();
    lines
}

/// What the program's own result says it spent since the result before it, in millionths of a dollar, summed over its
/// models as the program gives them.
fn told(lines: &[String]) -> u64 {
    let result: Value = serde_json::from_str(lines.iter().find(|l| is_result(l)).expect("a result")).expect("JSON");
    let usd: f64 = (result["model_usage_since_last_result"].as_object().expect("the field"))
        .values()
        .map(|m| m["costUSD"].as_f64().expect("a cost"))
        .sum();
    (usd * 1_000_000.0).round() as u64
}

/// A second message of the same session, as the real stream's but its first line.
fn second(lines: &[String]) -> Vec<String> {
    (lines.iter().skip(1))
        .map(|line| line.replace("msg_011", "msg_022").replace("toolu_01", "toolu_02"))
        .collect()
}

fn without_the_field(result: &mut Value) {
    result
        .as_object_mut()
        .expect("an object")
        .remove("model_usage_since_last_result");
}

// The two real streams: what the program says it spent in their results is what they cost, whatever their calls would
// be priced at: the gave-up stream's last call is one the program's own count leaves out (its `iterations` is empty and
// its $0.0004 is not in the result's cost), so a cost that priced the calls and added the result would differ from the
// program's. A few milliseconds, in memory; a stream of the test's own making could not show that the real lines are
// read so, and the engineer's tests read the first fixture alone.
#[test]
fn a_real_stream_costs_what_its_result_says_it_spent_and_no_call_is_added_to_it() {
    for text in [STREAM, GAVE_UP] {
        let all = lines(text);
        let said = told(&all);
        let stream = feed(&all);
        assert_eq!(stream.cost(), (said, false), "the program's own count, none priced");
        assert!(!stream.estimated());
        let spent = match stream.read() {
            Ok(read) => read.spent,
            Err(no) => no.spent.expect("a result says what it cost"),
        };
        assert_eq!(spent.usd_micros, said);
    }
}

// A result names the models the program spent on, each with its cost: the round's cost is their sum, not the first's
// or the largest. The program's own account is the oracle: its `total_cost_usd` is the sum of the `costUSD` of its
// models, as it is in each real result (one model in the fixtures, several where a subagent runs another). A few
// milliseconds, in memory; no other test has a result of more than one model.
#[test]
fn a_result_of_several_models_costs_the_sum_of_theirs() {
    let lines = with_result(&lines(STREAM), |result| {
        result["model_usage_since_last_result"] = json!({
            "claude-opus-5-5": { "costUSD": 1.25 },
            "claude-haiku-5-5": { "costUSD": 0.5 },
        });
        result["total_cost_usd"] = json!(99.0);
    });
    assert_eq!(feed(&lines).cost(), (1_750_000, false));
    assert_eq!(feed(&lines).read().expect("it reads").spent.usd_micros, 1_750_000);
}

// A result that names a model with no cost does not say what the program spent: the calls are priced and the stream
// says so, instead of taking the models that give a cost and leaving the other out. The fixture's calls come to
// $0.002614 at their models' prices, as its own result says. A few milliseconds, in memory.
#[test]
fn a_result_that_names_a_model_with_no_cost_is_priced_from_its_calls_and_says_so() {
    let lines = with_result(&lines(STREAM), |result| {
        result["model_usage_since_last_result"] = json!({
            "claude-haiku-5-5": { "costUSD": 0.5 },
            "claude-opus-5-5": { "inputTokens": 5 },
        });
    });
    assert_eq!(feed(&lines).cost(), (2614, true));
}

// A round of a first message that does not say what it spent and a second that does: the first is priced from its own
// calls alone and the second is told, so that the cost is the first's calls and the second's result, and one reading
// that prices every call when any result says nothing, or none when any result does, is wrong. The other way round is
// the same. A few milliseconds, in memory; the engineer's two-message test has the result of each told.
#[test]
fn of_two_messages_the_one_whose_result_says_nothing_is_priced_and_the_other_is_told() {
    let first = lines(STREAM);
    let silent = with_result(&first, without_the_field);
    let spoken = |usd: f64| {
        with_result(&second(&first), move |result| {
            result["model_usage_since_last_result"] = json!({ "m": { "costUSD": usd } })
        })
    };
    let mut both = silent.clone();
    both.extend(spoken(0.3));
    let stream = feed(&both);
    assert_eq!(stream.results(), 2);
    assert_eq!(stream.cost(), (2614 + 300_000, true));

    let loud = with_result(&first, |result| {
        result["model_usage_since_last_result"] = json!({ "m": { "costUSD": 0.5 } })
    });
    let mut both = loud;
    both.extend(with_result(&second(&first), without_the_field));
    let stream = feed(&both);
    assert_eq!(stream.results(), 2);
    assert_eq!(stream.cost(), (500_000 + 2614, true));
}

// The calls of a result that says the program spent nothing since the one before it cost nothing, and are no estimate:
// a round whose calls to the service all failed, as the program says in a result with an empty
// `model_usage_since_last_result` while its `total_cost_usd` holds the session's whole cost. This is the shape of two of
// the real results kept (an `api_retry` series ended by a synthetic message of no tokens). A few milliseconds, in memory.
#[test]
fn a_result_that_names_no_model_spent_nothing_whatever_the_session_cost_before() {
    let lines: Vec<String> = [
        r#"{"type":"system","subtype":"init","session_id":"s-1","claude_code_version":"9.9.9"}"#,
        r#"{"type":"system","subtype":"api_retry","attempt":10,"max_retries":10}"#,
        r#"{"type":"assistant","parent_tool_use_id":null,"error":"unknown","message":{"id":"synthetic-1","model":"<synthetic>","usage":{"input_tokens":0,"output_tokens":0,"cache_creation_input_tokens":0,"cache_read_input_tokens":0},"content":[{"type":"text","text":"API Error"}]}}"#,
        r#"{"type":"result","subtype":"success","is_error":true,"num_turns":1,"total_cost_usd":9.4939593,"model_usage_since_last_result":{},"usage":{"input_tokens":0,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":0},"result":"API Error: unavailable"}"#,
    ]
    .map(str::to_string)
    .to_vec();
    let stream = feed(&lines);
    assert_eq!(stream.cost(), (0, false));
    let no = stream.read().expect_err("an error result is no return");
    assert_eq!(no.spent.map(|s| s.usd_micros), Some(0));
}

// A result that says nothing of what was spent, printed for a round that made no call, is no estimate: nothing was priced
// from calls, and the flag is what the owner's page marks a round as estimated by. The real kept streams hold 34 such
// results, each `error_during_execution` with `total_cost_usd` 0 and no call (the one here is a session the program no
// longer had, its stream one line); before this work such a try recorded `priced` false. A few milliseconds, in
// memory; no other test reads a stream of no call.
#[test]
fn a_result_of_a_round_that_made_no_call_and_says_nothing_of_cost_is_no_estimate() {
    let lines = vec![
        r#"{"type":"result","subtype":"error_during_execution","duration_ms":0,"is_error":true,"num_turns":0,"session_id":"s-1","total_cost_usd":0,"usage":{"input_tokens":0,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":0,"iterations":[]},"modelUsage":{},"errors":["No conversation found with session ID: s-1"]}"#
            .to_string(),
    ];
    let stream = feed(&lines);
    assert_eq!(stream.cost(), (0, false));
    assert!(!stream.estimated());
}

// A call's end that comes before its message, or whose start was not kept, still takes the model the message names: the
// price of 100 tokens out, 1,000 read and 2 sent is $0.00006 at Haiku 5.5's prices and $0.00527 at the dearest, at which
// a call of no model is priced. A few milliseconds, in memory; the engineer's tests give each call its start.
#[test]
fn a_calls_end_before_its_message_or_its_start_is_priced_at_the_model_its_message_names() {
    let end = r#"{"type":"stream_event","parent_tool_use_id":null,"api_message_id":"msg_x","event":{"type":"message_delta","usage":{"input_tokens":2,"cache_creation_input_tokens":0,"cache_read_input_tokens":1000,"output_tokens":100}}}"#;
    let message = r#"{"type":"assistant","parent_tool_use_id":null,"message":{"id":"msg_x","model":"claude-haiku-5-5","usage":{"input_tokens":2,"cache_creation_input_tokens":0,"cache_read_input_tokens":1000,"output_tokens":5}}}"#;
    let start = r#"{"type":"stream_event","parent_tool_use_id":null,"event":{"type":"message_start","message":{"id":"msg_x","model":"claude-haiku-5-5","usage":{"input_tokens":2,"cache_creation_input_tokens":0,"cache_read_input_tokens":1000,"output_tokens":5}}}}"#;
    // 2 × 0.1 + 1,000 × 0.01 + 100 × 0.5 millionths.
    for order in [[end, message], [end, start], [message, end], [start, end]] {
        let stream = feed(&order.map(str::to_string));
        assert_eq!(stream.cost(), (60, true), "{order:?}");
    }
}
