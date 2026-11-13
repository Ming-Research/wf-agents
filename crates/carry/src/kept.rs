//! What is kept of each round of an agent in the data directory, where it is and how it is read back for the page.
//!
//! Each try of a round keeps the message it was told and what the command line printed, in `rounds/<work>` in the
//! data directory: `<run>-<round>.message.md` and `.stream.jsonl` for its first try, `.message.2.md` and
//! `.stream.2.jsonl` for the next, and so on; the result is the stream's last line but for the command line's own
//! `system` lines, such as a `task_summary`, once it printed one. A try kept before the command line was run with a
//! stream kept its result alone, in `.result.json` (`.result.2.json` for the next try, and so on), which is read for
//! it. Other files there are not read here. What a try ran on is in the work's records, not here; a try kept before
//! tries were recorded kept it here, in `.ran.json` (`.ran.2.json` for the next try, and so on), which is read for it.
//! Read back, a try is what it was told, and of what it printed only what the page shows: the agent's closing words
//! where they are not the structured return itself, the structured return, how many turns it took, how long, and by
//! which models; or, where the program said the try failed, why. The rest, ids of sessions and of credentials among
//! it, is never read out of the file. A result that is empty is no result: the program printed none.
//!
//! What is read is bounded, so that a round kept large neither holds the page up nor fills it: a file's length is
//! looked at before it is read, a message is read up to [`BOUNDS`]' `message` and said to be cut, a result longer than
//! its bound is not read and said to be so, and once a round's tries come to its bound, the later ones are counted and
//! left out.

use std::collections::BTreeMap;
use std::io::{ErrorKind, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use decide::RunId;
use serde::Deserialize;
use serde::de::IgnoredAny;
use service::{Back, Returned, Told, Try};
use store::WorkNumber;
use work::RanOn;

use crate::agents::ROUNDS;

/// How much is read of what a round kept, in bytes.
pub(crate) struct Bounds {
    /// Of one message; a longer one is cut.
    pub message: u64,
    /// Of one result; a longer one is not read.
    pub result: u64,
    /// Of the tries of one round in all; the tries after those that come to it are left out.
    pub round: u64,
}

pub(crate) const BOUNDS: Bounds = Bounds {
    message: 1 << 20,
    result: 8 << 20,
    round: 4 << 20,
};

/// Where what the round of this number of the run of the work was told and returned are kept, without the extension.
pub(crate) fn base(data: &Path, work: WorkNumber, run: RunId, number: usize) -> PathBuf {
    let dir = data.join(ROUNDS).join(work.0.to_string());
    dir.join(format!("{}-{number}", run.0))
}

/// Where the try of this number of a round, the first being one, keeps the command line's stream.
pub(crate) fn stream_at(base: &Path, n: usize) -> PathBuf {
    match n {
        1 => base.with_extension("stream.jsonl"),
        n => base.with_extension(format!("stream.{n}.jsonl")),
    }
}

/// Where the try of this number of a round, the first being one, keeps what it is told, and where a try kept before
/// the stream was kept its result.
pub(crate) fn at(base: &Path, n: usize) -> (PathBuf, PathBuf) {
    match n {
        1 => (base.with_extension("message.md"), base.with_extension("result.json")),
        n => (
            base.with_extension(format!("message.{n}.md")),
            base.with_extension(format!("result.{n}.json")),
        ),
    }
}

/// Where the try of this number of a round, the first being one, kept what it ran on, before tries were recorded.
fn ran_at(base: &Path, n: usize) -> PathBuf {
    match n {
        1 => base.with_extension("ran.json"),
        n => base.with_extension(format!("ran.{n}.json")),
    }
}

/// What a try ran on, as a try kept before tries were recorded kept it, with no account.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RanOnFile {
    level: String,
    model: String,
    effort: Option<String>,
}

/// The longest file of what a try ran on that is read, in bytes: one longer was never written.
const RAN_BOUND: u64 = 4096;

/// What the try whose file this is ran on; none where it kept no such file, or one that is not what a try ran on,
/// which the page then shows as it shows a try that kept nothing of it. Where the file cannot be read at all, why.
fn ran(path: &Path) -> Result<Option<RanOn>, String> {
    match length(path).map_err(|e| e.to_string())? {
        None => Ok(None),
        Some(long) if long > RAN_BOUND => Ok(None),
        Some(_) => {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            let file = serde_json::from_slice::<RanOnFile>(&bytes).ok();
            Ok(file.map(|file| RanOn {
                level: file.level,
                model: file.model,
                effort: file.effort,
                account: None,
            }))
        }
    }
}

/// What the round of this number of the run of the work, kept in the data directory, was told and what came back of
/// it, each try in order; no try where it kept nothing. A file that cannot be read is named from the data directory.
pub(crate) fn told(data: &Path, work: WorkNumber, run: RunId, number: usize) -> Result<Told, String> {
    told_within(data, &base(data, work, run, number), &BOUNDS)
}

fn told_within(data: &Path, base: &Path, bounds: &Bounds) -> Result<Told, String> {
    let named = |path: &Path| path.strip_prefix(data).unwrap_or(path).display().to_string();
    let mut told = Told::default();
    let mut read_so_far = 0;
    for n in 1.. {
        let (message, result) = at(base, n);
        let Some(whole) = length(&message).map_err(|e| format!("{}: {e}", named(&message)))? else {
            break;
        };
        if read_so_far >= bounds.round {
            told.left_out += 1;
            continue;
        }
        let shown = read_up_to(&message, bounds.message).map_err(|e| format!("{}: {e}", named(&message)))?;
        let message = text_of(&shown, whole > bounds.message);
        let stream = stream_at(base, n);
        let streamed = length(&stream).map_err(|e| format!("{}: {e}", named(&stream)))?;
        let back = match streamed {
            Some(long) => {
                result_of_stream(&stream, long, bounds.result).map_err(|e| format!("{}: {e}", named(&stream)))?
            }
            None => match length(&result).map_err(|e| format!("{}: {e}", named(&result)))? {
                None | Some(0) => Back::Nothing,
                Some(long) if long > bounds.result => Back::Unreadable(format!(
                    "it is {long} bytes long, more than the {} that are read of a result",
                    bounds.result
                )),
                Some(_) => {
                    let printed = std::fs::read(&result).map_err(|e| format!("{}: {e}", named(&result)))?;
                    back(&printed)
                }
            },
        };
        read_so_far += message.len() as u64 + size(&back);
        let on = ran_at(base, n);
        let ran = ran(&on).map_err(|e| format!("{}: {e}", named(&on)))?;
        told.tries.push(Try {
            message,
            cut: (whole > bounds.message).then_some(whole),
            back,
            ran,
        });
    }
    Ok(told)
}

/// What came back, read from a stream this long: its result, the last line of it but for the command line's own
/// `system` lines, which may follow the result. No more than `bound` bytes are read, from its end. A stream whose last
/// line but for those is no result, as one under way or stopped, gave nothing back yet.
fn result_of_stream(path: &Path, long: u64, bound: u64) -> std::io::Result<Back> {
    let mut file = std::fs::File::open(path)?;
    // One byte more than the bound, so that the newline before a line of the bound's length is read.
    let from = long.saturating_sub(bound + 1);
    file.seek(SeekFrom::Start(from))?;
    let mut end = Vec::new();
    file.take(bound + 1).read_to_end(&mut end)?;
    let mut lines = end.trim_ascii_end().rsplit(|b| *b == b'\n').peekable();
    while let Some(line) = lines.next() {
        // The first line read may have begun before what was read: none is whole that has none before it.
        if lines.peek().is_none() && from > 0 {
            return Ok(Back::Unreadable(format!(
                "its last lines are longer than the {bound} bytes that are read of a result"
            )));
        }
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(line) else {
            return Ok(Back::Nothing);
        };
        match value["type"].as_str() {
            Some("result") => return Ok(back(line)),
            Some("system") => continue,
            _ => return Ok(Back::Nothing),
        }
    }
    Ok(Back::Nothing)
}

/// The file's length; none where there is no such file.
fn length(path: &Path) -> std::io::Result<Option<u64>> {
    match std::fs::metadata(path) {
        Ok(meta) => Ok(Some(meta.len())),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// The file's first bytes, up to this many.
fn read_up_to(path: &Path, most: u64) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?.take(most).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// The bytes as text; where they were cut, without the part of a character the cut left at their end.
fn text_of(bytes: &[u8], cut: bool) -> String {
    let end = match std::str::from_utf8(bytes) {
        Err(e) if cut && e.error_len().is_none() => e.valid_up_to(),
        _ => bytes.len(),
    };
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// How much of a round's bound what came back takes: what was read of it.
fn size(back: &Back) -> u64 {
    let len = |text: &Option<String>| text.as_ref().map_or(0, |t| t.len() as u64);
    match back {
        Back::Nothing | Back::Unreadable(_) => 0,
        Back::Returned(r) => len(&r.text) + len(&r.structured),
        Back::Failed(why) => why.len() as u64,
    }
}

/// What the command line printed, as much of it as the page is given.
#[derive(Deserialize)]
struct Printed {
    #[serde(default)]
    is_error: bool,
    subtype: Option<String>,
    result: Option<String>,
    structured_output: Option<serde_json::Value>,
    num_turns: Option<u64>,
    duration_ms: Option<u64>,
    #[serde(rename = "modelUsage", default)]
    model_usage: BTreeMap<String, IgnoredAny>,
}

/// What came back, read from what the command line printed: what it returned, or that it failed and why; or why it
/// could not be read.
fn back(printed: &[u8]) -> Back {
    let printed: Printed = match serde_json::from_slice(printed) {
        Ok(printed) => printed,
        Err(e) => return Back::Unreadable(format!("it is not a result: {e}")),
    };
    let subtype = printed.subtype.filter(|s| s != "success");
    if printed.is_error || subtype.as_deref().is_some_and(|s| s.starts_with("error")) {
        let result = printed.result.filter(|r| !r.trim().is_empty());
        let why: Vec<String> = subtype.into_iter().chain(result).collect();
        return Back::Failed(match why.is_empty() {
            true => "the program said it failed, and gave no reason".to_string(),
            false => why.join(": "),
        });
    }
    let structured = printed.structured_output.filter(|value| !value.is_null());
    // The closing words of a round that returned in its form are the structured return itself, written compactly.
    let same = |text: &String| serde_json::from_str::<serde_json::Value>(text).ok() == structured;
    let text = printed.result.filter(|text| structured.is_none() || !same(text));
    let structured = match structured.map(|value| serde_json::to_string_pretty(&value)).transpose() {
        Ok(structured) => structured,
        Err(e) => return Back::Unreadable(e.to_string()),
    };
    Back::Returned(Returned {
        text,
        structured,
        turns: printed.num_turns,
        duration_ms: printed.duration_ms,
        models: printed.model_usage.into_keys().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A result the command line printed, with what is not for the page beside what is: its closing words are its
    /// structured return.
    const RESULT: &str = include_str!("../tests/fixtures/result.json");

    /// A stream the command line printed, with what named the machine and the account replaced: a tool used, then
    /// the return, the result, and a `task_summary` after it.
    const STREAM: &str = include_str!("../tests/fixtures/stream.jsonl");

    /// A data directory, and where the round kept in it is.
    fn kept() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let base = base(dir.path(), WorkNumber(3), RunId(7), 2);
        std::fs::create_dir_all(base.parent().unwrap()).unwrap();
        (dir, base)
    }

    fn keep(base: &Path, n: usize, message: &str, result: Option<&str>) {
        let (message_at, result_at) = at(base, n);
        std::fs::write(message_at, message).unwrap();
        if let Some(result) = result {
            std::fs::write(result_at, result).unwrap();
        }
    }

    fn told_at(dir: &tempfile::TempDir) -> Told {
        told(dir.path(), WorkNumber(3), RunId(7), 2).unwrap()
    }

    /// What came back of [`RESULT`]: its structured return pretty-printed, its closing words, which are the same,
    /// left out, and none of the ids beside them.
    fn greeted() -> Back {
        let structured = serde_json::json!({ "hands_in": [{ "block": "greeting", "content": "hello" }], "asks": [] });
        Back::Returned(Returned {
            text: None,
            structured: Some(serde_json::to_string_pretty(&structured).unwrap()),
            turns: Some(2),
            duration_ms: Some(1781),
            models: vec!["claude-haiku-5-5".to_string()],
        })
    }

    fn tried(message: &str, back: Back) -> Try {
        Try {
            message: message.to_string(),
            cut: None,
            back,
            ran: None,
        }
    }

    #[test]
    fn a_round_tried_once_gives_its_message_and_what_came_back() {
        let (dir, base) = kept();
        keep(&base, 1, "# Task\n\nGreet the user.\n", Some(RESULT));
        assert!(base.starts_with(dir.path().join("rounds").join("3")));
        assert_eq!(
            told_at(&dir),
            Told {
                tries: vec![tried("# Task\n\nGreet the user.\n", greeted())],
                left_out: 0,
            }
        );
    }

    #[test]
    fn a_try_kept_as_a_stream_gives_what_its_result_says_though_system_lines_follow_it() {
        let (dir, base) = kept();
        // The command line's `task_summary` follows the result.
        assert!(STREAM.lines().last().is_some_and(|line| line.contains("task_summary")));
        std::fs::write(at(&base, 1).0, "Greet the user.").unwrap();
        std::fs::write(stream_at(&base, 1), STREAM).unwrap();
        // A stream under way, or stopped: before its result, a `task_summary` after a tool's use is its last line.
        std::fs::write(at(&base, 2).0, "Again.").unwrap();
        let under_way: String = STREAM.lines().take(18).map(|line| format!("{line}\n")).collect();
        assert!(
            under_way
                .lines()
                .last()
                .is_some_and(|line| line.contains("task_summary"))
        );
        std::fs::write(stream_at(&base, 2), under_way).unwrap();
        // A try kept before the stream was kept is read from its result.
        keep(&base, 3, "Once more.", Some(RESULT));
        let structured = serde_json::json!({ "hands_in": [{ "block": "greeting", "content": "BANANA" }], "asks": [] });
        let streamed = Back::Returned(Returned {
            text: None,
            structured: Some(serde_json::to_string_pretty(&structured).unwrap()),
            turns: Some(3),
            duration_ms: Some(2658),
            models: vec!["claude-haiku-5-5".to_string()],
        });
        assert_eq!(
            told_at(&dir).tries,
            [
                tried("Greet the user.", streamed),
                tried("Again.", Back::Nothing),
                tried("Once more.", greeted())
            ]
        );
    }

    #[test]
    fn no_more_of_a_stream_is_read_than_the_bound_of_a_result() {
        let (dir, base) = kept();
        let bounds = Bounds {
            message: 100,
            result: 40,
            round: 1000,
        };
        std::fs::write(at(&base, 1).0, "First.").unwrap();
        let long = format!(
            "{}\n{{\"type\":\"result\",\"result\":\"{}\"}}\n",
            "x".repeat(100),
            "y".repeat(40)
        );
        std::fs::write(stream_at(&base, 1), long).unwrap();
        std::fs::write(at(&base, 2).0, "Second.").unwrap();
        let short = format!("{}\n{{\"type\":\"result\",\"result\":\"ok\"}}\n", "x".repeat(100));
        std::fs::write(stream_at(&base, 2), short).unwrap();
        let told = told_within(dir.path(), &base, &bounds).unwrap();
        let unread = "its last lines are longer than the 40 bytes that are read of a result".to_string();
        let ok = Back::Returned(Returned {
            text: Some("ok".to_string()),
            ..Returned::default()
        });
        assert_eq!(
            told.tries,
            [tried("First.", Back::Unreadable(unread)), tried("Second.", ok)]
        );
    }

    #[test]
    fn a_round_tried_again_gives_each_try_in_order() {
        let (dir, base) = kept();
        keep(&base, 1, "first", Some(""));
        keep(&base, 2, "second", Some(r#"{"result":"Done.","num_turns":1}"#));
        keep(&base, 3, "third", Some(RESULT));
        let second = Back::Returned(Returned {
            text: Some("Done.".to_string()),
            turns: Some(1),
            ..Returned::default()
        });
        assert_eq!(
            told_at(&dir).tries,
            [
                tried("first", Back::Nothing),
                tried("second", second),
                tried("third", greeted())
            ]
        );
    }

    #[test]
    fn a_try_kept_before_tries_were_recorded_gives_what_its_file_says_it_ran_on() {
        let (dir, base) = kept();
        keep(&base, 1, "first", Some(""));
        keep(&base, 2, "second", Some(""));
        keep(&base, 3, "third", Some(""));
        std::fs::write(
            ran_at(&base, 1),
            r#"{"level":"2","model":"claude-sonnet-5-5","effort":null}"#,
        )
        .unwrap();
        std::fs::write(ran_at(&base, 2), r#"{"level": "3", "mo"#).unwrap();
        let ran: Vec<Option<RanOn>> = told_at(&dir).tries.into_iter().map(|t| t.ran).collect();
        let first = RanOn {
            level: "2".to_string(),
            model: "claude-sonnet-5-5".to_string(),
            effort: None,
            account: None,
        };
        // The second's file is torn, and the third, recorded, kept none.
        assert_eq!(ran, [Some(first), None, None]);
    }

    #[test]
    fn closing_words_other_than_the_structured_return_are_kept() {
        let (dir, base) = kept();
        let printed = r#"{"result":"I greeted the user.","structured_output":{"asks":[]}}"#;
        keep(&base, 1, "Greet the user.", Some(printed));
        let back = Back::Returned(Returned {
            text: Some("I greeted the user.".to_string()),
            structured: Some("{\n  \"asks\": []\n}".to_string()),
            ..Returned::default()
        });
        assert_eq!(told_at(&dir).tries, [tried("Greet the user.", back)]);
    }

    #[test]
    fn a_try_the_program_said_failed_gives_why_and_not_closing_words() {
        let (dir, base) = kept();
        let failed = r#"{"type":"result","subtype":"success","is_error":true,"result":"API Error: 529 Overloaded"}"#;
        keep(&base, 1, "first", Some(failed));
        keep(
            &base,
            2,
            "second",
            Some(r#"{"subtype":"error_max_turns","is_error":true,"num_turns":9}"#),
        );
        assert_eq!(
            told_at(&dir).tries,
            [
                tried("first", Back::Failed("API Error: 529 Overloaded".to_string())),
                tried("second", Back::Failed("error_max_turns".to_string())),
            ]
        );
    }

    #[test]
    fn a_round_under_way_gives_its_message_and_nothing_back() {
        let (dir, base) = kept();
        keep(&base, 1, "Greet the user.", None);
        assert_eq!(told_at(&dir).tries, [tried("Greet the user.", Back::Nothing)]);
    }

    #[test]
    fn a_round_never_run_gives_nothing() {
        let (dir, _base) = kept();
        assert_eq!(told_at(&dir), Told::default());
    }

    #[test]
    fn a_result_that_cannot_be_read_gives_the_message_and_says_so() {
        let (dir, base) = kept();
        keep(&base, 1, "Greet the user.", Some("{\"result\": "));
        let tries = told_at(&dir).tries;
        assert_eq!(tries[0].message, "Greet the user.");
        match &tries[0].back {
            Back::Unreadable(why) => assert!(why.starts_with("it is not a result: "), "{why}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_file_that_cannot_be_read_is_named_from_the_data_directory() {
        let (dir, base) = kept();
        // A directory where the message is: it is there, and cannot be read as a file.
        std::fs::create_dir(at(&base, 1).0).unwrap();
        let why = told(dir.path(), WorkNumber(3), RunId(7), 2).unwrap_err();
        assert!(why.starts_with("rounds/3/7-2.message.md: "), "{why}");
        assert!(!why.contains(&dir.path().display().to_string()), "{why}");
    }

    #[test]
    fn what_is_read_of_a_round_is_bounded_and_each_cut_said() {
        let (dir, base) = kept();
        let bounds = Bounds {
            message: 9,
            result: 16,
            round: 20,
        };
        // Cut inside the two bytes of an é, which is left out whole.
        keep(&base, 1, "Greet thé user.", Some(r#"{"result":"Done, and more."}"#));
        keep(&base, 2, "Again.", Some(r#"{"result":"ok"}"#));
        keep(&base, 3, "And more.", Some(r#"{"result":"ok"}"#));
        keep(&base, 4, "Once more.", None);
        let told = told_within(dir.path(), &base, &bounds).unwrap();
        let ok = || {
            Back::Returned(Returned {
                text: Some("ok".to_string()),
                ..Returned::default()
            })
        };
        let unread = "it is 28 bytes long, more than the 16 that are read of a result".to_string();
        assert_eq!(
            told.tries,
            [
                Try {
                    message: "Greet th".to_string(),
                    cut: Some(16),
                    back: Back::Unreadable(unread),
                    ran: None,
                },
                tried("Again.", ok()),
                tried("And more.", ok()),
            ]
        );
        // The first three come to 27 bytes, past the round's 20: the fourth is left out.
        assert_eq!(told.left_out, 1);
    }
}
