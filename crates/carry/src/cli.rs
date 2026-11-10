//! The `claude` command line, as a round is run with it: the arguments, which session it begins or goes on in, and
//! what it printed, read into what came back, what it took and the requests it made. Each is a function of what it is
//! given, with no I/O.
//!
//! Learnt from the command line (version 2.1.321): `-p --output-format stream-json --verbose` prints one JSON object a
//! line on the standard output, as the round goes. Each request to the model, one for each turn, prints its message
//! as one `assistant` line for each part of it (a thought, a text, a tool's use), every part with the message's id and
//! the request's `usage`, whose tokens sent, read from the cache and written to it are known as the request begins
//! (its `output_tokens` there are not all of them). A message whose `parent_tool_use_id` is set is taken for one of
//! an agent the round started, in a context of its own (not measured: the field names the tool use that started it);
//! one whose model is `<synthetic>` was made by the command line, no request having been made. The last line is the
//! result, as `--output-format json` prints it: the structured return in `structured_output`, the tokens of the whole
//! round in `usage` (its `iterations` holding only the last request's), and in `total_cost_usd` the cost of the whole
//! session so far, every earlier call of it included: a round's own cost is that less what the session's earlier
//! rounds cost. The message is read whole from the standard input. A failed call exits with 1 and, where it got as
//! far, prints a result with `is_error` set and its reason in `result`.
//! Beginning a session whose id is taken says on the standard error `Session ID <id> is already in use.` and prints
//! nothing on the standard output; going on in one there is none of says `No conversation found with session ID:
//! <id>` there, and prints a result that is an error, having made no request. Going on in a session, the command line
//! sends the system prompt the session began with: a round given another `--append-system-prompt` was not told it,
//! and read the cache whole.

use decide::RunId;
use flow::Compiled;
use serde::Deserialize;
use uuid::Uuid;
use work::{Spent, Work};

use crate::cache::Request;
use crate::process;
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
    let mut args: Vec<String> = [
        "-p",
        "--output-format",
        "stream-json",
        "--verbose",
        "--model",
        &agents.model,
    ]
    .iter()
    .map(|a| a.to_string())
    .collect();
    args.extend(["--append-system-prompt".to_string(), protocol.to_string()]);
    args.extend(["--json-schema".to_string(), schema.to_string()]);
    args.extend(["--setting-sources".to_string(), agents.setting_sources.clone()]);
    args.extend(agents.args.iter().cloned());
    let flag = match begin {
        Begin::New => "--session-id",
        Begin::Resume => "--resume",
    };
    args.extend([flag.to_string(), session.to_string()]);
    args
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
        .filter(move |((run, _), _)| of_line(flow, work, *run, line))
        .map(|(_, spent)| spent)
}

/// Whether the run is of a cell of the line.
fn of_line(flow: &Compiled, work: &Work, run: RunId, line: &str) -> bool {
    let cell = work.records.started.iter().find(|s| s.run == run).map(|s| &s.cell);
    let spec = cell.and_then(|cell| flow.cells.iter().find(|spec| spec.id == *cell));
    spec.and_then(|spec| spec.line.as_deref()) == Some(line)
}

/// Each round of the line in this work that ended, in the order they ended: its run, its number in the run and what
/// it took. The runs of a line may take turns, one resting while a cell before it runs.
pub fn line_ended(flow: &Compiled, work: &Work, line: &str) -> Vec<(RunId, usize, Spent)> {
    let mut ended: Vec<(RunId, usize)> = Vec::new();
    for returned in &work.records.returned {
        // A run's rounds end in their order: this is the next of its run.
        let number = ended.iter().filter(|(run, _)| *run == returned.run).count() + 1;
        ended.push((returned.run, number));
    }
    (ended.into_iter())
        .filter(|(run, _)| of_line(flow, work, *run, line))
        .map(|(run, number)| (run, number, work.spent.get(&(run, number)).copied().unwrap_or_default()))
        .collect()
}

/// The model a message is said to be of where the command line made it itself, no request having been made.
pub const SYNTHETIC: &str = "<synthetic>";

/// A line of the stream, as much of it as is read.
#[derive(Debug, Deserialize)]
struct Line {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    message: Option<Message>,
    #[serde(default)]
    parent_tool_use_id: Option<serde_json::Value>,
}

/// The message of an `assistant` line, as much of it as is read.
#[derive(Debug, Deserialize)]
struct Message {
    id: Option<String>,
    model: Option<String>,
    usage: Option<Usage>,
}

/// The lines of the stream that read as what a line is, each with its text: a line cut short, as the last of a
/// program that died can be, or one that is no JSON, is passed over.
fn lines(stdout: &[u8]) -> impl Iterator<Item = (&[u8], Line)> {
    (stdout.split(|b| *b == b'\n')).filter_map(|text| Some((text, serde_json::from_slice::<Line>(text).ok()?)))
}

/// The requests the round made to the model, in the order it made them, one for each message: those of its own
/// context, not of an agent it started, and none the command line made up itself.
pub fn requests(stdout: &[u8]) -> Vec<Request> {
    let mut seen = std::collections::BTreeSet::new();
    let mut requests = Vec::new();
    for (_, line) in lines(stdout) {
        let own = line.parent_tool_use_id.as_ref().is_none_or(|parent| parent.is_null());
        let Some(Message {
            id: Some(id),
            model: Some(model),
            usage: Some(usage),
        }) = line.message
        else {
            continue;
        };
        if line.kind != "assistant" || !own || model == SYNTHETIC || !seen.insert(id) {
            continue;
        }
        requests.push(Request {
            model,
            input: usage.input_tokens,
            read: usage.cache_read_input_tokens,
            written: usage.cache_creation_input_tokens,
        });
    }
    requests
}

/// The round's result, as it was printed: the stream's last line that is one.
pub fn result(stdout: &[u8]) -> Option<&[u8]> {
    (lines(stdout)
        .filter(|(_, line)| line.kind == "result")
        .map(|(text, _)| text))
    .last()
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
    let Some(line) = result(stdout) else {
        let text = String::from_utf8_lossy(stdout);
        return Err(NoResult {
            why: format!(
                "what the command line printed holds no result: {}",
                process::tail(text.trim().as_bytes())
            ),
            spent: None,
        });
    };
    let result: Result = serde_json::from_slice(line).map_err(|e| NoResult {
        why: format!(
            "the command line's result cannot be read ({e}): {}",
            String::from_utf8_lossy(line)
        ),
        spent: None,
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
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use engine::Came;
    use work::Ask;

    use super::*;
    use crate::message::tests::Small;

    /// A result the command line printed for a session's first round, with what named the machine and the account
    /// replaced.
    const RESULT: &str = include_str!("../tests/fixtures/result.json");

    /// The stream the command line printed for a round that went on in its session, its first request in two lines,
    /// with what named the machine, the account and the session replaced.
    const STREAM: &str = include_str!("../tests/fixtures/stream.jsonl");

    /// The result of the fixture, on one line, as the stream prints it.
    fn result_line() -> Vec<u8> {
        let value: serde_json::Value = serde_json::from_str(RESULT).expect("the fixture is JSON");
        serde_json::to_vec(&value).unwrap()
    }

    fn haiku(input: u64, read: u64, written: u64) -> Request {
        Request {
            model: "claude-haiku-5-5".to_string(),
            input,
            read,
            written,
        }
    }

    fn agents() -> Agents {
        Agents {
            command: "claude".to_string(),
            model: "claude-haiku-5-5".to_string(),
            slots: 2,
            args: vec!["--strict-mcp-config".to_string()],
            env: Vec::new(),
            setting_sources: "user".to_string(),
            config_dir: None,
            owner_language: "Chinese".to_string(),
            work_budget_usd: None,
            cache_check: true,
            cache_life_minutes: 55,
            cache_shortfall_percent: 2.0,
            cache_shortfall_tokens: 512,
        }
    }

    #[test]
    fn the_fixture_reads_to_what_it_took() {
        let read = read(&result_line(), Begin::New, 0).expect("the fixture reads");
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
        let read = read(&result_line(), Begin::Resume, 1000).expect("the fixture reads");
        assert_eq!(read.spent.usd_micros, 15);
    }

    #[test]
    fn a_resumed_round_whose_session_cost_less_than_before_costs_the_whole_total() {
        let read = read(&result_line(), Begin::Resume, 5000).expect("the fixture reads");
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
        let error = r#"{"type":"result","is_error":true,"result":"Out of structured output retries.","total_cost_usd":0.8,"usage":{"input_tokens":10,"cache_creation_input_tokens":5,"cache_read_input_tokens":100,"output_tokens":20}}"#;
        let spent = Spent {
            usd_micros: 300_000,
            input_tokens: 15,
            cached_tokens: 100,
            output_tokens: 20,
        };
        let refused = read(error.as_bytes(), Begin::Resume, 500_000).expect_err("an error is not taken");
        assert_eq!(refused.spent, Some(spent));
        let bare = r#"{"type":"result","is_error":false,"total_cost_usd":0.8}"#;
        let refused = read(bare.as_bytes(), Begin::New, 0).expect_err("no structured return is not taken");
        assert_eq!(refused.spent.map(|s| s.usd_micros), Some(800_000));
    }

    #[test]
    fn the_stream_reads_to_one_request_for_each_message_and_its_result() {
        let stdout = STREAM.as_bytes();
        assert_eq!(
            requests(stdout),
            [haiku(2, 4932, 127), haiku(4, 5059, 131)],
            "the first request's two lines are one request"
        );
        let read = read(stdout, Begin::Resume, 1500).expect("the stream holds a result");
        assert_eq!(
            read.spent,
            Spent {
                usd_micros: 183,
                input_tokens: 264,
                cached_tokens: 9991,
                output_tokens: 154,
            }
        );
        assert_eq!(read.structured["answer"], "\"You are terse. Protocol A.\"");
        let line = result(stdout).expect("a result");
        assert!(STREAM.trim_end().ends_with(std::str::from_utf8(line).unwrap()));
    }

    #[test]
    fn a_stream_cut_short_or_with_no_result_holds_the_requests_it_made_and_no_result() {
        let lines: Vec<&str> = STREAM.lines().collect();
        // The program died as it printed its second request's line.
        let second = lines.iter().position(|l| l.contains(r#""id":"msg_02""#)).unwrap();
        let cut = format!("{}\n{}", lines[..second].join("\n"), &lines[second][..80]);
        assert_eq!(requests(cut.as_bytes()), [haiku(2, 4932, 127)]);
        let refused = read(cut.as_bytes(), Begin::Resume, 0).expect_err("no result");
        assert_eq!(refused.spent, None, "what the round took is not known");
        assert!(refused.why.contains("holds no result"), "{}", refused.why);
        // It printed every request, and died before its result.
        let whole = lines[..lines.len() - 1].join("\n");
        assert_eq!(requests(whole.as_bytes()).len(), 2);
        assert_eq!(result(whole.as_bytes()), None);
        assert!(read(whole.as_bytes(), Begin::Resume, 0).is_err());
    }

    #[test]
    fn a_message_the_command_line_made_or_of_an_agent_the_round_started_is_no_request_of_the_round() {
        let line = |id: &str, model: &str, parent: &str| {
            format!(
                r#"{{"type":"assistant","message":{{"id":"{id}","model":"{model}","usage":{{"input_tokens":1,"cache_creation_input_tokens":2,"cache_read_input_tokens":3,"output_tokens":4}}}},"parent_tool_use_id":{parent}}}"#
            )
        };
        let stream = [
            line("a", SYNTHETIC, "null"),
            line("b", "claude-haiku-5-5", r#""toolu_01""#),
            line("c", "claude-haiku-5-5", "null"),
        ]
        .join("\n");
        assert_eq!(requests(stream.as_bytes()), [haiku(1, 3, 2)]);
    }

    #[test]
    fn the_rounds_of_a_line_are_in_the_order_they_ended() {
        let mut small = Small::new();
        small.hands_in(&[("plan", "Greet by name.", None)]);
        small.back(Came::Asks(vec![Ask::Tool {
            name: "check".to_string(),
            input: String::new(),
        }]));
        small.done("The check passed.");
        small.hands_in(&[("code", "A greeting.", None)]);
        let runs: Vec<RunId> = small.work.records.started.iter().map(|s| s.run).collect();
        let rounds = |line| {
            (line_ended(&small.flow, &small.work, line).into_iter())
                .map(|(run, number, _)| (run, number))
                .collect::<Vec<_>>()
        };
        assert_eq!(rounds("engineer"), [(runs[0], 1), (runs[1], 1), (runs[1], 2)]);
        assert_eq!(rounds("reviewer"), [], "the review's round has not ended");
    }

    #[test]
    fn the_command_line_begins_a_session_and_goes_on_in_it() {
        let session = Uuid::parse_str("6a1f3c2e-8d4b-4e7a-9c5f-2b8e1d0a7f43").unwrap();
        let head = |begin| arguments(&agents(), "PROTOCOL", "SCHEMA", session, begin);
        let common = [
            "-p",
            "--output-format",
            "stream-json",
            "--verbose",
            "--model",
            "claude-haiku-5-5",
            "--append-system-prompt",
            "PROTOCOL",
            "--json-schema",
            "SCHEMA",
            "--setting-sources",
            "user",
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
