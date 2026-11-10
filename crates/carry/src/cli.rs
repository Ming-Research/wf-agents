//! The `claude` command line, as a round is run with it: the arguments, which session it begins or goes on in, and
//! what it printed, read into what came back and what it took. Each is a function of what it is given, with no I/O.
//!
//! Learnt from the command line (version 2.1.321): `-p --output-format json` prints one JSON object, the result, on
//! the standard output, with the structured return in `structured_output`, the tokens of this call in `usage`, and in
//! `total_cost_usd` the cost of the whole session so far, every earlier call of it included: a round's own cost is
//! that less what the session's earlier rounds cost. The message is read whole from the standard input. A failed call
//! exits with 1 and, where it got as far, prints a result with `is_error` set and its reason in `result`. Beginning a
//! session whose id is taken says on the standard error `Session ID <id> is already in use.`, and going on in one
//! there is none of says `No conversation found with session ID: <id>`; both print nothing on the standard output.

use flow::Compiled;
use serde::Deserialize;
use uuid::Uuid;
use work::{Spent, Work};

use crate::settings::Agents;

/// What the command line says on its standard error when a session is begun under an id that has one already.
pub const IN_USE: &str = "is already in use";
/// What the command line says on its standard error when a session that is not there is gone on in.
pub const NOT_FOUND: &str = "No conversation found with session ID";

/// Whether a round begins its line's session or goes on in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Begin {
    /// The session is begun under its id: `--session-id`.
    New,
    /// The session is gone on in: `--resume`.
    Resume,
}

impl Begin {
    /// The other way, to try once where the command line said this one cannot be.
    pub fn other(self) -> Begin {
        match self {
            Begin::New => Begin::Resume,
            Begin::Resume => Begin::New,
        }
    }

    /// Whether what the command line said on its standard error means the session is to be begun the other way.
    pub fn refused(self, stderr: &str) -> bool {
        match self {
            Begin::New => stderr.contains(IN_USE),
            Begin::Resume => stderr.contains(NOT_FOUND),
        }
    }
}

/// The arguments of a round's command line, after the program: the engine's own, then the installation's, then the
/// session. The system prompt and the schema are the same for every round, so that they open every request alike and
/// the prompt cache keeps them.
pub fn arguments(agents: &Agents, protocol: &str, schema: &str, session: Uuid, begin: Begin) -> Vec<String> {
    let mut args: Vec<String> = ["-p", "--output-format", "json", "--model", &agents.model]
        .iter()
        .map(|a| a.to_string())
        .collect();
    args.extend(["--append-system-prompt".to_string(), protocol.to_string()]);
    args.extend(["--json-schema".to_string(), schema.to_string()]);
    args.extend(agents.args.iter().cloned());
    let flag = match begin {
        Begin::New => "--session-id",
        Begin::Resume => "--resume",
    };
    args.extend([flag.to_string(), session.to_string()]);
    args
}

/// The environment a round's command line is given beside what the engine's own holds: a round waits for whatever
/// it starts, the command line asks the model again a few times for a return that does not fit its form, and its
/// directory of settings is the installation's where the settings name one.
pub fn environment(agents: &Agents) -> Vec<(String, String)> {
    let mut env = vec![
        ("CLAUDE_CODE_DISABLE_BACKGROUND_TASKS".to_string(), "1".to_string()),
        ("MAX_STRUCTURED_OUTPUT_RETRIES".to_string(), "3".to_string()),
    ];
    if let Some(dir) = &agents.config_dir {
        env.push(("CLAUDE_CONFIG_DIR".to_string(), dir.to_string_lossy().into_owned()));
    }
    env
}

/// Whether the round of this number of the run begins the session of the line or goes on in it: it goes on where an
/// earlier round of the line in this work ended having spent tokens, which a round that never reached the model has
/// not.
pub fn begin(flow: &Compiled, work: &Work, line: &str) -> Begin {
    let spent =
        line_rounds(flow, work, line).any(|spent| spent.input_tokens + spent.cached_tokens + spent.output_tokens > 0);
    if spent { Begin::Resume } else { Begin::New }
}

/// What the line's rounds in this work cost before, in millionths of a dollar: what the session cost before the round.
pub fn spent_before(flow: &Compiled, work: &Work, line: &str) -> u64 {
    line_rounds(flow, work, line).map(|spent| spent.usd_micros).sum()
}

/// What each round of the line in this work that ended took.
fn line_rounds<'a>(flow: &'a Compiled, work: &'a Work, line: &'a str) -> impl Iterator<Item = &'a Spent> {
    (work.spent.iter())
        .filter(move |((run, _), _)| {
            let cell = work.records.started.iter().find(|s| s.run == *run).map(|s| &s.cell);
            let spec = cell.and_then(|cell| flow.cells.iter().find(|spec| spec.id == *cell));
            spec.and_then(|spec| spec.line.as_deref()) == Some(line)
        })
        .map(|(_, spent)| spent)
}

/// The command line's result, as much of it as is read.
#[derive(Debug, Deserialize)]
struct Result {
    #[serde(default)]
    is_error: bool,
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    total_cost_usd: f64,
    #[serde(default)]
    usage: Usage,
    structured_output: Option<serde_json::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct Usage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
}

/// What a round's command line printed, read: the structured return, and what the round took.
#[derive(Clone, Debug, PartialEq)]
pub struct Read {
    pub structured: serde_json::Value,
    pub spent: Spent,
}

/// Reads what the command line printed on its standard output, for a round that began its session or went on in it
/// with the session having cost `before`; or says why it is no result to take.
pub fn read(stdout: &[u8], begin: Begin, before: u64) -> std::result::Result<Read, String> {
    let result: Result = serde_json::from_slice(stdout).map_err(|e| {
        let text = String::from_utf8_lossy(stdout);
        format!("what the command line printed is not its result ({e}): {}", text.trim())
    })?;
    if result.is_error {
        let why = result.result.unwrap_or_default();
        return Err(format!("the command line's result is an error: {why}"));
    }
    let Some(structured) = result.structured_output else {
        return Err("the command line's result holds no structured return".to_string());
    };
    let total = (result.total_cost_usd.max(0.0) * 1_000_000.0).round() as u64;
    let usd_micros = match begin {
        Begin::New => total,
        Begin::Resume => total.saturating_sub(before),
    };
    let usage = result.usage;
    let spent = Spent {
        usd_micros,
        input_tokens: usage.input_tokens + usage.cache_creation_input_tokens,
        cached_tokens: usage.cache_read_input_tokens,
        output_tokens: usage.output_tokens,
    };
    Ok(Read { structured, spent })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// A result the command line printed for a session's first round, with what named the machine and the account
    /// replaced.
    const RESULT: &str = include_str!("../tests/fixtures/result.json");

    fn agents() -> Agents {
        Agents {
            command: "claude".to_string(),
            model: "claude-haiku-5-5".to_string(),
            slots: 2,
            args: vec!["--strict-mcp-config".to_string()],
            config_dir: None,
            owner_language: "Chinese".to_string(),
            work_budget_usd: None,
        }
    }

    #[test]
    fn the_fixture_reads_to_what_it_took() {
        let read = read(RESULT.as_bytes(), Begin::New, 0).expect("the fixture reads");
        assert_eq!(
            read.spent,
            Spent {
                usd_micros: 1015,
                input_tokens: 4841,
                cached_tokens: 0,
                output_tokens: 95,
            }
        );
        assert_eq!(read.structured["hands_in"][0]["block"], "greeting");
    }

    #[test]
    fn a_resumed_round_costs_what_the_session_cost_more() {
        let read = read(RESULT.as_bytes(), Begin::Resume, 1000).expect("the fixture reads");
        assert_eq!(read.spent.usd_micros, 15);
    }

    #[test]
    fn an_error_and_what_is_no_result_are_not_taken() {
        let error = r#"{"type":"result","is_error":true,"result":"There's an issue with the selected model."}"#;
        let refused = read(error.as_bytes(), Begin::New, 0).expect_err("an error is not taken");
        assert!(refused.contains("issue with the selected model"), "{refused}");
        assert!(read(b"Error: something", Begin::New, 0).is_err());
        let bare = r#"{"type":"result","is_error":false,"result":"hello","total_cost_usd":0.1}"#;
        assert!(
            read(bare.as_bytes(), Begin::New, 0).is_err(),
            "a result with no structured return"
        );
    }

    #[test]
    fn the_command_line_begins_a_session_and_goes_on_in_it() {
        let session = Uuid::parse_str("6a1f3c2e-8d4b-4e7a-9c5f-2b8e1d0a7f43").unwrap();
        let head = |begin| arguments(&agents(), "PROTOCOL", "SCHEMA", session, begin);
        let common = [
            "-p",
            "--output-format",
            "json",
            "--model",
            "claude-haiku-5-5",
            "--append-system-prompt",
            "PROTOCOL",
            "--json-schema",
            "SCHEMA",
            "--strict-mcp-config",
        ];
        let mut new: Vec<String> = common.iter().map(|a| a.to_string()).collect();
        let mut resumed = new.clone();
        new.extend(["--session-id".to_string(), session.to_string()]);
        resumed.extend(["--resume".to_string(), session.to_string()]);
        assert_eq!(head(Begin::New), new);
        assert_eq!(head(Begin::Resume), resumed);
    }

    #[test]
    fn a_refused_session_is_tried_the_other_way() {
        let in_use = "Error: Session ID 6a1f3c2e-8d4b-4e7a-9c5f-2b8e1d0a7f43 is already in use.";
        let not_found = "No conversation found with session ID: 6a1f3c2e-8d4b-4e7a-9c5f-2b8e1d0a7f43";
        assert!(Begin::New.refused(in_use));
        assert!(!Begin::New.refused(not_found));
        assert!(Begin::Resume.refused(not_found));
        assert_eq!(Begin::New.other(), Begin::Resume);
    }

    #[test]
    fn the_settings_directory_is_given_only_where_the_settings_name_one() {
        let names = |agents: &Agents| {
            environment(agents)
                .into_iter()
                .map(|(name, _)| name)
                .collect::<Vec<_>>()
        };
        assert!(!names(&agents()).contains(&"CLAUDE_CONFIG_DIR".to_string()));
        let mut configured = agents();
        configured.config_dir = Some(PathBuf::from("config"));
        assert!(names(&configured).contains(&"CLAUDE_CONFIG_DIR".to_string()));
    }
}
