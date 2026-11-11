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

use std::path::{Path, PathBuf};

use decide::RunId;
use flow::Compiled;
use serde::Deserialize;
use uuid::Uuid;
use work::{RanOn, Spent, Work, sessions};

use crate::permissions::Permissions;
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

/// What a round's command line does when the program compacts the session within the round, which leaves the session
/// holding only a summary of what it was told: it marks the compaction in one file, by adding a byte to it, so that
/// the compactions are counted, and gives the session again, at once, what another file holds, which is the round's
/// whole message as a session that begins is told it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Compacted {
    pub marks: PathBuf,
    pub again: PathBuf,
}

impl Compacted {
    /// The settings that give the program the hook, as JSON: at the start of a session it compacted, the program adds
    /// what the hook's command prints to the session.
    fn settings(&self) -> String {
        let quote = |path: &Path| format!("'{}'", path.display().to_string().replace('\'', "'\\''"));
        let command = format!("printf x >> {} && cat {}", quote(&self.marks), quote(&self.again));
        let hook = serde_json::json!({ "type": "command", "command": command });
        let hooks = serde_json::json!({ "SessionStart": [{ "matcher": "compact", "hooks": [hook] }] });
        serde_json::json!({ "hooks": hooks }).to_string()
    }
}

/// The arguments of a round's command line, after the program: the engine's own, then the installation's, then the
/// line's permissions, then the session. The schema is the same for every round, and the system prompt for every round
/// of a line, so that they open every request of a session alike and the prompt cache keeps them. The round runs on the
/// model and at the effort it was given.
#[allow(clippy::too_many_arguments)]
pub fn arguments(
    agents: &Agents,
    ran: &RanOn,
    protocol: &str,
    schema: &str,
    permissions: &Permissions,
    compacted: &Compacted,
    session: Uuid,
    begin: Begin,
) -> Vec<String> {
    let mut args: Vec<String> = ["-p", "--output-format", "json", "--model", &ran.model]
        .iter()
        .map(|a| a.to_string())
        .collect();
    if let Some(effort) = &ran.effort {
        args.extend(["--effort".to_string(), effort.clone()]);
    }
    args.extend(["--append-system-prompt".to_string(), protocol.to_string()]);
    args.extend(["--json-schema".to_string(), schema.to_string()]);
    args.extend(["--setting-sources".to_string(), agents.setting_sources.clone()]);
    args.extend(["--settings".to_string(), compacted.settings()]);
    args.extend(agents.args.iter().cloned());
    args.extend(permissions.arguments());
    let flag = match begin {
        Begin::New => "--session-id",
        Begin::Resume => "--resume",
    };
    args.extend([flag.to_string(), session.to_string()]);
    args
}

/// The families of models the command line knows, each also its alias for the family's latest model.
const FAMILIES: [&str; 5] = ["haiku", "sonnet", "opus", "fable", "mythos"];

/// Whether the command line takes a model by this name, in the forms its `--model` takes: the alias of a family's
/// latest model (`opus`) or a model's full name (`claude-opus-5-5`, `claude-3-5-sonnet-20241022`), each with `[1m]`
/// after it or not; a family it does not know, or a name in no such form, it does not take. Brought over from the
/// engine on `main`, which checked the models of its levels so.
pub fn knows_model(model: &str) -> bool {
    let name = model.strip_suffix("[1m]").unwrap_or(model);
    if FAMILIES.contains(&name) {
        return true;
    }
    let Some(rest) = name.strip_prefix("claude-") else {
        return false;
    };
    let parts: Vec<&str> = rest.split('-').collect();
    let numbers = (parts.iter())
        .filter(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        .count();
    let families = parts.iter().filter(|p| FAMILIES.contains(p)).count();
    families == 1 && numbers > 0 && numbers + 1 == parts.len()
}

/// The variables of the engine's environment a round's command line is always given: what a program needs to find
/// its tools, its home and its language. Nothing else of the engine's environment reaches it, unless the settings
/// name it.
pub const KEPT: [&str; 7] = ["PATH", "HOME", "USER", "LANG", "TMPDIR", "TERM", "SHELL"];

/// The environment a round's command line is given, in place of the engine's own: of the engine's variables, those
/// [`KEPT`], every `LC_*`, and those the settings name; then the carrier's own: a round waits for whatever it starts,
/// the command line asks the model again a few times for a return that does not fit its form, and its directory of
/// settings is the installation's where the settings name one.
pub fn environment(agents: &Agents, engine: impl IntoIterator<Item = (String, String)>) -> Vec<(String, String)> {
    let kept =
        |name: &str| KEPT.contains(&name) || name.starts_with("LC_") || agents.env.iter().any(|named| named == name);
    let mut env: Vec<(String, String)> = engine.into_iter().filter(|(name, _)| kept(name)).collect();
    env.sort();
    env.push(("CLAUDE_CODE_DISABLE_BACKGROUND_TASKS".to_string(), "1".to_string()));
    env.push(("MAX_STRUCTURED_OUTPUT_RETRIES".to_string(), "3".to_string()));
    if let Some(dir) = &agents.config_dir {
        env.push(("CLAUDE_CONFIG_DIR".to_string(), dir.to_string_lossy().into_owned()));
    }
    env
}

/// The session a round runs in, as the records say: which of its line's sessions, whether the round begins it, and
/// what the session cost before the round, in millionths of a dollar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InSession {
    pub number: u32,
    pub begin: Begin,
    pub before: u64,
}

/// The session the round of this number of the run of a cell on the line runs in: it goes on in it where an earlier
/// round of the line in the same session ended having spent tokens, which a round that never reached the model has
/// not; and the session cost before it what those earlier rounds cost.
pub fn in_session(flow: &Compiled, work: &Work, line: &str, run: RunId, number: usize) -> InSession {
    let on_line = |run: RunId| {
        let cell = work.records.started.iter().find(|s| s.run == run).map(|s| &s.cell);
        let spec = cell.and_then(|cell| flow.cells.iter().find(|spec| spec.id == *cell));
        spec.and_then(|spec| spec.line.as_deref()) == Some(line)
    };
    let (session, begins) = sessions::of_round(work, line, on_line, run, number);
    let at = sessions::began_at(work, run, number).unwrap_or_else(|| work.tick());
    let before = (work.spent.iter())
        .filter(|((r, _), _)| on_line(*r))
        .filter(|((r, n), _)| {
            sessions::began_at(work, *r, *n).is_some_and(|b| b < at && sessions::number_at(work, line, b) == session)
        })
        .map(|(_, spent)| spent.usd_micros)
        .sum();
    InSession {
        number: session,
        begin: if begins { Begin::New } else { Begin::Resume },
        before,
    }
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
    /// Each call the round made to the model, in order.
    #[serde(default)]
    iterations: Vec<Iteration>,
}

/// A call to the model, as much of it as is read: what its prompt was made of.
#[derive(Debug, Default, Deserialize)]
struct Iteration {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
}

/// What a round's command line printed, read: the structured return, and what the round took.
#[derive(Clone, Debug, PartialEq)]
pub struct Read {
    pub structured: serde_json::Value,
    pub spent: Spent,
}

/// Why what the command line printed is no result to take, with what the round took where the result could be read:
/// an error, or a result with no structured return, still says what the call cost, and the call is paid for all the
/// same. Where nothing could be read, what the round took is not known.
#[derive(Debug)]
pub struct NoResult {
    pub why: String,
    pub spent: Option<Spent>,
}

/// Reads what the command line printed on its standard output, for a round that began its session or went on in it
/// with the session having cost `before`; or says why it is no result to take, with what the round took where the
/// result could be read. A round that began its session cost the session's total; one that went on in it, that total
/// less what the line's earlier rounds cost, or the whole total where that is less (measured: the total is the whole
/// session's, even where two lines take turns in one worktree).
pub fn read(stdout: &[u8], begin: Begin, before: u64) -> std::result::Result<Read, NoResult> {
    let result: Result = serde_json::from_slice(stdout).map_err(|e| {
        let text = String::from_utf8_lossy(stdout);
        NoResult {
            why: format!("what the command line printed is not its result ({e}): {}", text.trim()),
            spent: None,
        }
    })?;
    let spent = spent(&result, begin, before);
    if result.is_error {
        let why = result.result.unwrap_or_default();
        let why = format!("the command line's result is an error: {why}");
        return Err(NoResult {
            why,
            spent: Some(spent),
        });
    }
    let Some(structured) = result.structured_output else {
        let why = "the command line's result holds no structured return".to_string();
        return Err(NoResult {
            why,
            spent: Some(spent),
        });
    };
    Ok(Read { structured, spent })
}

/// What the call whose result this is took.
fn spent(result: &Result, begin: Begin, before: u64) -> Spent {
    let total = (result.total_cost_usd.max(0.0) * 1_000_000.0).round() as u64;
    // The cost errs high: where the session's total is less than its earlier rounds cost, it cannot be the same
    // session's, and is taken whole.
    let usd_micros = match begin {
        Begin::Resume if total >= before => total - before,
        _ => total,
    };
    let usage = &result.usage;
    Spent {
        usd_micros,
        input_tokens: usage.input_tokens + usage.cache_creation_input_tokens,
        cached_tokens: usage.cache_read_input_tokens,
        output_tokens: usage.output_tokens,
        // The context the round ended with is the prompt of its last call, read from the cache or not.
        context_tokens: (usage.iterations.last())
            .map(|call| call.input_tokens + call.cache_creation_input_tokens + call.cache_read_input_tokens),
        compactions: 0,
    }
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
            levels: Default::default(),
            slots: 2,
            args: vec!["--strict-mcp-config".to_string()],
            env: Vec::new(),
            setting_sources: "user".to_string(),
            config_dir: None,
            permissions: Default::default(),
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
                context_tokens: Some(4841),
                compactions: 0,
            }
        );
        assert_eq!(read.structured["hands_in"][0]["block"], "greeting");
    }

    #[test]
    fn a_round_ended_with_the_context_of_its_last_call() {
        let read = read(RESULT.as_bytes(), Begin::New, 0).expect("the fixture reads");
        assert_eq!(read.spent.context_tokens, Some(4841));
        let two = r#"{"type":"result","is_error":false,"total_cost_usd":0.1,"structured_output":{},
            "usage":{"input_tokens":3,"output_tokens":9,"iterations":[
                {"input_tokens":1,"cache_read_input_tokens":100,"cache_creation_input_tokens":20},
                {"input_tokens":2,"cache_read_input_tokens":120,"cache_creation_input_tokens":30}]}}"#;
        let read = super::read(two.as_bytes(), Begin::New, 0).expect("it reads");
        assert_eq!(read.spent.context_tokens, Some(152), "the last call's prompt");
        let none = r#"{"type":"result","is_error":false,"total_cost_usd":0.1,"structured_output":{}}"#;
        assert_eq!(
            super::read(none.as_bytes(), Begin::New, 0)
                .unwrap()
                .spent
                .context_tokens,
            None
        );
    }

    #[test]
    fn a_resumed_round_costs_what_the_session_cost_more() {
        let read = read(RESULT.as_bytes(), Begin::Resume, 1000).expect("the fixture reads");
        assert_eq!(read.spent.usd_micros, 15);
    }

    #[test]
    fn a_resumed_round_whose_session_cost_less_than_before_costs_the_whole_total() {
        let read = read(RESULT.as_bytes(), Begin::Resume, 5000).expect("the fixture reads");
        assert_eq!(read.spent.usd_micros, 1015);
    }

    #[test]
    fn an_error_and_what_is_no_result_are_not_taken() {
        let error = r#"{"type":"result","is_error":true,"result":"There's an issue with the selected model."}"#;
        let refused = read(error.as_bytes(), Begin::New, 0).expect_err("an error is not taken");
        assert!(refused.why.contains("issue with the selected model"), "{}", refused.why);
        let nothing = read(b"Error: something", Begin::New, 0).expect_err("what is no JSON is not taken");
        assert_eq!(nothing.spent, None, "what the round took is not known");
        let bare = r#"{"type":"result","is_error":false,"result":"hello","total_cost_usd":0.1}"#;
        assert!(
            read(bare.as_bytes(), Begin::New, 0).is_err(),
            "a result with no structured return"
        );
    }

    #[test]
    fn a_result_that_is_no_return_still_says_what_its_call_took() {
        let error = r#"{"type":"result","is_error":true,"result":"Out of structured output retries.","total_cost_usd":0.8,
            "usage":{"input_tokens":10,"cache_creation_input_tokens":5,"cache_read_input_tokens":100,"output_tokens":20}}"#;
        let spent = Spent {
            usd_micros: 300_000,
            input_tokens: 15,
            cached_tokens: 100,
            output_tokens: 20,
            ..Default::default()
        };
        let refused = read(error.as_bytes(), Begin::Resume, 500_000).expect_err("an error is not taken");
        assert_eq!(refused.spent, Some(spent));
        let bare = r#"{"type":"result","is_error":false,"total_cost_usd":0.8}"#;
        let refused = read(bare.as_bytes(), Begin::New, 0).expect_err("no structured return is not taken");
        assert_eq!(refused.spent.map(|s| s.usd_micros), Some(800_000));
    }

    #[test]
    fn the_command_line_begins_a_session_and_goes_on_in_it() {
        let session = Uuid::parse_str("6a1f3c2e-8d4b-4e7a-9c5f-2b8e1d0a7f43").unwrap();
        let permissions = Permissions::of(true, &[], &Default::default());
        let ran = RanOn {
            level: "1".to_string(),
            model: "claude-haiku-5-5".to_string(),
            effort: None,
            account: None,
        };
        let compacted = Compacted {
            marks: PathBuf::from("rounds/1/2-3.compacted"),
            again: PathBuf::from("rounds/1/2-3.again.md"),
        };
        let head = |begin| {
            arguments(
                &agents(),
                &ran,
                "PROTOCOL",
                "SCHEMA",
                &permissions,
                &compacted,
                session,
                begin,
            )
        };
        let hook = r#"{"hooks":{"SessionStart":[{"hooks":[{"command":"printf x >> 'rounds/1/2-3.compacted' && cat 'rounds/1/2-3.again.md'","type":"command"}],"matcher":"compact"}]}}"#;
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
            "--setting-sources",
            "user",
            "--settings",
            hook,
            "--strict-mcp-config",
        ];
        let mut new: Vec<String> = common.iter().map(|a| a.to_string()).collect();
        // The line's permissions come after the installation's arguments, which they add to.
        new.extend(permissions.arguments());
        let mut resumed = new.clone();
        new.extend(["--session-id".to_string(), session.to_string()]);
        resumed.extend(["--resume".to_string(), session.to_string()]);
        assert_eq!(head(Begin::New), new);
        assert_eq!(head(Begin::Resume), resumed);
    }

    #[test]
    fn a_round_runs_on_the_model_it_was_given_and_at_its_effort_where_it_has_one() {
        let session = Uuid::parse_str("6a1f3c2e-8d4b-4e7a-9c5f-2b8e1d0a7f43").unwrap();
        let ran = RanOn {
            level: "3".to_string(),
            model: "claude-opus-5-5".to_string(),
            effort: Some("xhigh".to_string()),
            account: None,
        };
        let compacted = Compacted {
            marks: PathBuf::from("compacted"),
            again: PathBuf::from("again.md"),
        };
        let permissions = Permissions::of(false, &[], &Default::default());
        let args = arguments(
            &agents(),
            &ran,
            "PROTOCOL",
            "SCHEMA",
            &permissions,
            &compacted,
            session,
            Begin::New,
        );
        assert_eq!(
            args[..7],
            [
                "-p",
                "--output-format",
                "json",
                "--model",
                "claude-opus-5-5",
                "--effort",
                "xhigh"
            ]
        );
    }

    #[test]
    fn the_models_the_command_line_takes_are_known_and_others_are_not() {
        for model in [
            "opus",
            "fable",
            "claude-opus-5-5",
            "claude-haiku-5-5[1m]",
            "claude-3-5-sonnet-20241022",
        ] {
            assert!(knows_model(model), "{model}");
        }
        for model in [
            "gpt-5",
            "claude-opus",
            "claude-poem-5",
            "Opus",
            "claude-opus-sonnet-5",
            "",
        ] {
            assert!(!knows_model(model), "{model}");
        }
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
    fn the_environment_is_built_from_what_is_kept_and_named() {
        let engine = [
            ("PATH", "/bin"),
            ("HOME", "/home/someone"),
            ("LC_ALL", "C"),
            ("SECRET_TOKEN", "x"),
            ("ANTHROPIC_API_KEY", "k"),
        ]
        .map(|(n, v)| (n.to_string(), v.to_string()));
        let names = |agents: &Agents| {
            (environment(agents, engine.clone()).into_iter())
                .map(|(name, _)| name)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(&agents()),
            [
                "HOME",
                "LC_ALL",
                "PATH",
                "CLAUDE_CODE_DISABLE_BACKGROUND_TASKS",
                "MAX_STRUCTURED_OUTPUT_RETRIES"
            ]
        );
        let mut configured = agents();
        configured.env = vec!["ANTHROPIC_API_KEY".to_string()];
        configured.config_dir = Some(PathBuf::from("config"));
        let named = names(&configured);
        assert!(named.contains(&"ANTHROPIC_API_KEY".to_string()));
        assert!(named.contains(&"CLAUDE_CONFIG_DIR".to_string()));
        assert!(!named.contains(&"SECRET_TOKEN".to_string()));
    }
}
