//! What is kept of each round of an agent in the data directory, where it is and how it is read back for the page.
//!
//! Each try of a round keeps the message it was told and what the command line printed, in `rounds/<work>` in the
//! data directory: `<run>-<round>.message.md` and `.result.json` for its first try, `.message.2.md` and
//! `.result.2.json` for the next, and so on. Read back, a try is what it was told, and of what it printed only what
//! the page shows: the agent's closing words, the structured return, how many turns it took, how long, and by which
//! models. The rest, ids of sessions and of credentials among it, is never read out of the file.

use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use decide::RunId;
use serde::Deserialize;
use serde::de::IgnoredAny;
use service::{Returned, Told, Try};
use store::WorkNumber;

use crate::agents::ROUNDS;

/// Where what the round of this number of the run of the work was told and returned are kept, without the extension.
pub(crate) fn base(data: &Path, work: WorkNumber, run: RunId, number: usize) -> PathBuf {
    let dir = data.join(ROUNDS).join(work.0.to_string());
    dir.join(format!("{}-{number}", run.0))
}

/// Where the try of this number of a round, the first being one, keeps what it is told and what it returns.
pub(crate) fn at(base: &Path, n: usize) -> (PathBuf, PathBuf) {
    match n {
        1 => (base.with_extension("message.md"), base.with_extension("result.json")),
        n => (
            base.with_extension(format!("message.{n}.md")),
            base.with_extension(format!("result.{n}.json")),
        ),
    }
}

/// What the round kept at `base` was told and what came back of it, each try in order; no try where it kept nothing.
pub(crate) fn told(base: &Path) -> Result<Told, String> {
    let mut tries = Vec::new();
    for n in 1.. {
        let (message, result) = at(base, n);
        let Some(message) = read(&message)? else {
            break;
        };
        tries.push(Try {
            message: String::from_utf8_lossy(&message).into_owned(),
            returned: read(&result)?.map(|printed| returned(&printed)),
        });
    }
    Ok(Told { tries })
}

/// The file's bytes; none where there is no such file.
fn read(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// What the command line printed, as much of it as the page is given.
#[derive(Deserialize)]
struct Printed {
    result: Option<String>,
    structured_output: Option<serde_json::Value>,
    num_turns: Option<u64>,
    duration_ms: Option<u64>,
    #[serde(rename = "modelUsage", default)]
    model_usage: BTreeMap<String, IgnoredAny>,
}

/// What came back, read from what the command line printed; or why it could not be read.
fn returned(printed: &[u8]) -> Result<Returned, String> {
    if printed.is_empty() {
        return Err("the program printed nothing".to_string());
    }
    let printed: Printed = serde_json::from_slice(printed).map_err(|e| format!("it is not a result: {e}"))?;
    let structured = (printed.structured_output)
        .filter(|value| !value.is_null())
        .map(|value| serde_json::to_string_pretty(&value).map_err(|e| e.to_string()))
        .transpose()?;
    Ok(Returned {
        text: printed.result,
        structured,
        turns: printed.num_turns,
        duration_ms: printed.duration_ms,
        models: printed.model_usage.into_keys().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A result the command line printed, with what is not for the page beside what is.
    const RESULT: &str = include_str!("../tests/fixtures/result.json");

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

    /// What came back of [`RESULT`]: its structured return pretty-printed, and none of the ids beside it.
    fn greeted() -> Returned {
        let structured = serde_json::json!({ "hands_in": [{ "block": "greeting", "content": "hello" }], "asks": [] });
        Returned {
            text: Some(r#"{"hands_in":[{"block":"greeting","content":"hello"}],"asks":[]}"#.to_string()),
            structured: Some(serde_json::to_string_pretty(&structured).unwrap()),
            turns: Some(2),
            duration_ms: Some(1781),
            models: vec!["claude-haiku-5-5".to_string()],
        }
    }

    #[test]
    fn a_round_tried_once_gives_its_message_and_what_came_back() {
        let (dir, base) = kept();
        keep(&base, 1, "# Task\n\nGreet the user.\n", Some(RESULT));
        assert!(base.starts_with(dir.path().join("rounds").join("3")));
        assert_eq!(
            told(&base).unwrap(),
            Told {
                tries: vec![Try {
                    message: "# Task\n\nGreet the user.\n".to_string(),
                    returned: Some(Ok(greeted())),
                }],
            }
        );
    }

    #[test]
    fn a_round_tried_again_gives_each_try_in_order() {
        let (_dir, base) = kept();
        keep(&base, 1, "first", Some(""));
        keep(&base, 2, "second", Some(r#"{"result":"Done.","num_turns":1}"#));
        keep(&base, 3, "third", Some(RESULT));
        let tries = told(&base).unwrap().tries;
        let messages: Vec<&str> = tries.iter().map(|t| t.message.as_str()).collect();
        assert_eq!(messages, ["first", "second", "third"]);
        assert_eq!(tries[0].returned, Some(Err("the program printed nothing".to_string())));
        let second = Returned {
            text: Some("Done.".to_string()),
            turns: Some(1),
            ..Returned::default()
        };
        assert_eq!(tries[1].returned, Some(Ok(second)));
        assert_eq!(tries[2].returned, Some(Ok(greeted())));
    }

    #[test]
    fn a_round_under_way_gives_its_message_and_nothing_back() {
        let (_dir, base) = kept();
        keep(&base, 1, "Greet the user.", None);
        assert_eq!(
            told(&base).unwrap().tries,
            [Try {
                message: "Greet the user.".to_string(),
                returned: None,
            }]
        );
    }

    #[test]
    fn a_round_never_run_gives_nothing() {
        let (_dir, base) = kept();
        assert_eq!(told(&base).unwrap(), Told::default());
    }

    #[test]
    fn a_result_that_cannot_be_read_gives_the_message_and_says_so() {
        let (_dir, base) = kept();
        keep(&base, 1, "Greet the user.", Some("{\"result\": "));
        let tries = told(&base).unwrap().tries;
        assert_eq!(tries[0].message, "Greet the user.");
        match &tries[0].returned {
            Some(Err(why)) => assert!(why.starts_with("it is not a result: "), "{why}"),
            other => panic!("{other:?}"),
        }
    }
}
