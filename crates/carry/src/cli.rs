//! The `claude` command line, as a round is run with it: the arguments, the environment, the message on its standard
//! input, which session it begins or goes on in, the hooks it is given, and its stream read into what came back and
//! what it took. Each is a function of what it is given, with no I/O, but for the hooks' shell commands and the files
//! the owner's words wait in ([`Words`]).
//!
//! As the old engine ran it (`crates/sessions/src/cli.rs` on `old-main`), and as runs of version 2.1.323 show:
//! `-p --input-format stream-json` reads the message from the standard input as one user message of the stream-json
//! format, a line of JSON ([`stdin`]); `--output-format stream-json --verbose` prints a line of JSON for each thing
//! that happens, and near the end the result, which only the command line's own `system` lines (a `task_summary`)
//! follow, with the structured return in `structured_output`, the tokens of this call in `usage`, and in
//! `total_cost_usd` the cost of the whole session so far, every earlier call of it included: a round's own cost is
//! that less what the session's earlier rounds cost. Each message of the model comes whole in an `assistant` line;
//! with `--include-partial-messages` it also comes in pieces as the model writes it (`stream_event` lines), the input
//! of each tool among them. The result gives no text of an attempt at a return the command line refused: an attempt
//! that is JSON is whole in its `assistant` line, and one cut off, which is no JSON, only in its pieces. A failed call
//! exits with 1 and, where it got as far, prints a result with `is_error` set and its reason in `result` or `errors`,
//! which at a return given up on ends with the last refusal, quoting the attempt where it could not be parsed.
//! Beginning a session whose id is taken says on the standard error `Session ID <id> is already in use.`, and going on
//! in one there is none of says `No conversation found with session ID: <id>`; both print nothing on the standard
//! output.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use decide::RunId;
use flow::Compiled;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;
use work::{RanOn, Spent, Work, sessions};

use crate::permissions::Permissions;
use crate::settings::Agents;
use crate::{cost, failure};

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
    /// The hook's shell command.
    fn command(&self) -> String {
        format!("printf x >> {} && cat {}", quote(&self.marks), quote(&self.again))
    }
}

/// What the owner says to a round while it runs, given to its session right after the next use of one of the
/// program's own tools: the runner puts it in one file ([`Words::put`]), and the hook the command line runs after each
/// use of a tool moves that file aside, as the first `<delivered>.<n>.json` there is none of, and prints it, which the
/// program adds to the session. The hook needs nothing but the shell and these files, so that what runs the agents
/// may be apart from the engine. After the program's own tool for the return the hook gives nothing: the round has
/// made its return, and words given then would be taken as delivered and never read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Words {
    /// The file the words wait in, as the hook prints them.
    pub waiting: PathBuf,
    /// Where the words the hook gave are moved, without the number and the extension.
    pub delivered: PathBuf,
}

impl Words {
    /// The hook's shell command: it reads what the program says of the tool on its standard input, whole, so that the
    /// program never writes to a pipe nobody reads, and gives nothing after the tool for the return.
    fn command(&self) -> String {
        let (waiting, delivered) = (quote(&self.waiting), quote(&self.delivered));
        format!(
            "input=$(cat); case \"$input\" in \
             *'\"tool_name\":\"{STRUCTURED_OUTPUT}\"'* | *'\"tool_name\": \"{STRUCTURED_OUTPUT}\"'*) ;; \
             *) if [ -e {waiting} ]; then n=1; while [ -e {delivered}.$n.json ]; do n=$((n+1)); done; \
             {{ mv {waiting} {delivered}.$n.json 2>/dev/null && cat {delivered}.$n.json; }} || :; fi ;; esac"
        )
    }

    /// Puts what is said where the hook reads it, as the hook is to print it: what the program adds to the session
    /// after a use of a tool. What waits there still is taken back first and given before it, so that one file holds
    /// everything said: a rename either takes it back whole or finds that the hook has moved it. Gives whether it took
    /// back what waited there, the hook not having given it. One writer puts at a time.
    pub async fn put(&self, said: &str) -> std::result::Result<bool, String> {
        let named = |path: &Path, e: std::io::Error| format!("{}: {e}", path.display());
        let taking = self.waiting.with_extension("json.taking");
        let mut context = String::new();
        let took_back = match tokio::fs::rename(&self.waiting, &taking).await {
            Ok(()) => {
                let bytes = tokio::fs::read(&taking).await.map_err(|e| named(&taking, e))?;
                let waited: Value = serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", taking.display()))?;
                if let Some(text) = waited["hookSpecificOutput"]["additionalContext"].as_str() {
                    context.push_str(text);
                    context.push_str("\n\n");
                }
                tokio::fs::remove_file(&taking).await.map_err(|e| named(&taking, e))?;
                true
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(named(&self.waiting, e)),
        };
        context.push_str(said);
        let output = json!({ "hookSpecificOutput": { "hookEventName": "PostToolUse", "additionalContext": context } });
        // Written whole beside it and renamed into place, so that the hook never moves a file half written.
        let part = self.waiting.with_extension("json.part");
        tokio::fs::write(&part, output.to_string())
            .await
            .map_err(|e| named(&part, e))?;
        tokio::fs::rename(&part, &self.waiting)
            .await
            .map_err(|e| named(&self.waiting, e))?;
        Ok(took_back)
    }

    /// Takes back what waits for the hook, once the program has ended and its hook runs no more: gives whether
    /// something waited there, which the hook never gave.
    pub async fn take_back(&self) -> std::result::Result<bool, String> {
        let taking = self.waiting.with_extension("json.taking");
        match tokio::fs::rename(&self.waiting, &taking).await {
            Ok(()) => tokio::fs::remove_file(&taking)
                .await
                .map(|()| true)
                .map_err(|e| format!("{}: {e}", taking.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(format!("{}: {e}", self.waiting.display())),
        }
    }
}

/// A path as a shell reads it back as it is: in single quotes.
fn quote(path: &Path) -> String {
    quoted(&path.display().to_string())
}

/// A text as a shell reads it back as it is: in single quotes.
fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// Which of its tools the program may use: a round, every one its line's permissions let it; a keep-warm ping, none
/// but the tool for its return, every other refused as it is called, with these words, so that the ping changes
/// nothing that a round of another line, or an act, works on beside it in the worktree. Refused by a hook as each is
/// called and not by the permissions, which would change the tools that open every request of the session, and so the
/// prompt the ping is there to keep in the cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tools<'a> {
    Permitted,
    TheReturnAlone { refused: &'a str },
}

impl Tools<'_> {
    /// The command of the hook the program runs before each use of a tool, where it refuses some: it reads what the
    /// program says of the tool, whole, takes the tool's name from its first key `tool_name`, which comes before the
    /// tool's input, so that no input names another, lets the tool for the return be used, and refuses any other, the
    /// words on its error output, which the program gives the model.
    fn command(&self) -> Option<String> {
        let Tools::TheReturnAlone { refused } = self else {
            return None;
        };
        Some(format!(
            "input=$(cat); name=${{input#*'\"tool_name\"'}}; name=${{name#*'\"'}}; name=${{name%%'\"'*}}; \
             [ \"$name\" = {STRUCTURED_OUTPUT} ] || {{ printf '%s\\n' {} >&2; exit 2; }}",
            quoted(refused)
        ))
    }
}

/// The settings the engine gives the program, as JSON. Its hooks: at the start of a session it compacted, and after
/// each use of a tool, the program adds what the hook's command prints to the session; and before each use of a tool,
/// where it may use only some ([`Tools`]), the hook refuses the others. And its own memory off
/// (`autoMemoryEnabled`, which its settings' schema says keeps it from reading or writing its auto-memory directory):
/// a session holds what the engine gives it and what its agent reads, and the program's own memory, which its own
/// system prompt would tell the agent to write, would hold what the engine never sees, against the rule the agents are
/// given that they keep nothing there.
fn settings(compacted: &Compacted, words: &Words, tools: Tools<'_>) -> String {
    let hook = |command: String| json!([{ "type": "command", "command": command }]);
    let mut hooks = json!({
        "SessionStart": [{ "matcher": "compact", "hooks": hook(compacted.command()) }],
        "PostToolUse": [{ "matcher": "*", "hooks": hook(words.command()) }],
    });
    if let Some(command) = tools.command() {
        hooks["PreToolUse"] = json!([{ "matcher": "*", "hooks": hook(command) }]);
    }
    json!({ "autoMemoryEnabled": false, "hooks": hooks }).to_string()
}

/// The arguments of a round's command line, after the program: the engine's own, then the installation's, then the
/// line's permissions, then the session. The message goes in, and the stream comes out, as stream-json; the stream
/// carries each message in pieces as it is written, which rebuild an attempt at a return the command line refused. The
/// schema is the form of return of the round's flow, the same for every round of every cell of it, and the system
/// prompt is the same for every round of a line, so that they open every request of a session alike and the prompt
/// cache keeps them. The round runs on the model and at the effort it was given, using the tools it may ([`Tools`]).
#[allow(clippy::too_many_arguments)]
pub fn arguments(
    agents: &Agents,
    ran: &RanOn,
    system: &str,
    schema: &str,
    permissions: &Permissions,
    (compacted, words, tools): (&Compacted, &Words, Tools<'_>),
    session: Uuid,
    begin: Begin,
) -> Vec<String> {
    let mut args: Vec<String> = [
        "-p",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--model",
        &ran.model,
    ]
    .iter()
    .map(|a| a.to_string())
    .collect();
    if let Some(effort) = &ran.effort {
        args.extend(["--effort".to_string(), effort.clone()]);
    }
    args.extend(["--append-system-prompt".to_string(), system.to_string()]);
    args.extend(["--json-schema".to_string(), schema.to_string()]);
    args.extend(["--setting-sources".to_string(), agents.setting_sources.clone()]);
    args.extend(["--settings".to_string(), settings(compacted, words, tools)]);
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
/// engine on `old-main`, which checked the models of its levels so.
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

/// The variables the carrier sets for every round's command line, each the agents' program's: a round waits for
/// whatever it starts, since a subagent left in the background has the command line ask for the return before it is
/// back; the service checks each tool's input as JSON before it streams it, since inputs it did not check came cut off;
/// and the command line asks the model again three times, not five, for a return that does not fit its form, each try
/// writing the whole return again. What a project's toolchain reads its declaration names (`[environment]`).
pub const ENVIRONMENT: [(&str, &str); 3] = [
    ("CLAUDE_CODE_DISABLE_BACKGROUND_TASKS", "1"),
    ("CLAUDE_CODE_ENABLE_FINE_GRAINED_TOOL_STREAMING", "0"),
    ("MAX_STRUCTURED_OUTPUT_RETRIES", "3"),
];

/// The environment a round's command line is given, in place of the engine's own: of the engine's variables, those
/// [`KEPT`], every `LC_*`, and those the settings name; then the carrier's own, [`ENVIRONMENT`]; the variables the
/// project's declaration names, `declared`; and the directory of settings and sign-in of the account it runs under,
/// where it runs under one of the installation's.
pub fn environment(
    agents: &Agents,
    account: Option<&str>,
    engine: impl IntoIterator<Item = (String, String)>,
    declared: &BTreeMap<String, String>,
) -> Vec<(String, String)> {
    let kept =
        |name: &str| KEPT.contains(&name) || name.starts_with("LC_") || agents.env.iter().any(|named| named == name);
    let mut env: Vec<(String, String)> = engine.into_iter().filter(|(name, _)| kept(name)).collect();
    env.sort();
    env.extend(ENVIRONMENT.map(|(name, value)| (name.to_string(), value.to_string())));
    env.extend(declared.iter().map(|(name, value)| (name.clone(), value.clone())));
    if let Some(dir) = account {
        env.push((crate::account::VARIABLE.to_string(), dir.to_string()));
    }
    env
}

/// What is wrong with the variables a project's declaration names for every round's command line: a name of the
/// engine's own, which it keeps of its environment, sets itself or names the program's directory of settings by; and
/// one that is no name a shell takes. Each said with where it is written.
pub fn environment_faults(declared: &BTreeMap<String, String>) -> Vec<String> {
    let at = format!("{} [environment]", flow::agents::DECLARATION);
    let engines = |name: &str| {
        KEPT.contains(&name)
            || name.starts_with("LC_")
            || ENVIRONMENT.iter().any(|(own, _)| *own == name)
            || name == crate::account::VARIABLE
    };
    let mut faults = Vec::new();
    for name in declared.keys() {
        let shell = name.starts_with(|c: char| c == '_' || c.is_ascii_alphabetic())
            && name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric());
        if !shell {
            faults.push(format!("{at}: {name:?} is no name of a variable"));
        } else if engines(name) {
            faults.push(format!(
                "{at}: {name} is the engine's own, which it gives every round itself"
            ));
        }
    }
    faults
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
/// round of the line in the same session put its message into it, having reached the model or its program having said
/// it took the message in (`work::sessions::wrote_into`); and the session cost before it what those earlier rounds
/// cost, and the keep-warm pings it was sent before it.
pub fn in_session(flow: &Compiled, work: &Work, line: &str, run: RunId, number: usize) -> InSession {
    let on_line = |run: RunId| {
        let cell = work.records.started.iter().find(|s| s.run == run).map(|s| &s.cell);
        let spec = cell.and_then(|cell| flow.cells.iter().find(|spec| spec.id == *cell));
        spec.and_then(|spec| spec.line.as_deref()) == Some(line)
    };
    let (session, begins) = sessions::of_round(work, line, on_line, run, number);
    let at = sessions::start(work, run, number);
    let before = (work.spent.iter())
        .filter(|((r, _), _)| on_line(*r))
        .filter(|((r, n), _)| {
            let b = sessions::start(work, *r, *n);
            b < at && sessions::number_at(work, line, b) == session
        })
        .map(|(_, spent)| spent.usd_micros)
        .sum::<u64>()
        + (work.pinged.iter())
            .filter(|p| p.line == line && p.session == session && p.at < at)
            .map(|p| p.spent.usd_micros)
            .sum::<u64>();
    InSession {
        number: session,
        begin: if begins { Begin::New } else { Begin::Resume },
        before,
    }
}

/// The command line's result, as much of it as is read.
#[derive(Debug, Deserialize)]
struct Result {
    /// What kind of end it is, as the command line names it: `success`, or the error it ended in.
    #[serde(default)]
    subtype: Option<String>,
    #[serde(default)]
    is_error: bool,
    #[serde(default)]
    result: Option<String>,
    /// What went wrong on the command line's side, such as running out of tries at the return, where the result
    /// has no text of it.
    #[serde(default)]
    errors: Vec<String>,
    #[serde(default)]
    total_cost_usd: f64,
    #[serde(default)]
    usage: Usage,
    structured_output: Option<Value>,
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
    pub structured: Value,
    pub spent: Spent,
}

/// Why what the command line printed is no result to take, with what the round took where the result could be read:
/// an error, or a result with no structured return, still says what the call cost, and the call is paid for all the
/// same. Where the stream holds no result to be read, what the round took is what its calls' tokens cost at their
/// model's prices; none where it made none. What the stream said of the service's errors in its own fields comes with
/// it, whatever was read.
#[derive(Debug)]
pub struct NoResult {
    pub why: String,
    pub spent: Option<Spent>,
    pub said: failure::Said,
}

/// The tool the command line hands a return in with, under `--json-schema`.
pub const STRUCTURED_OUTPUT: &str = "StructuredOutput";

/// What a partial message's line starts with, as the command line writes it.
const PARTIAL: &str = r#"{"type":"stream_event""#;

/// The command line's stream as it comes, line by line: its result, and each attempt at a return, rebuilt from the
/// partial messages, with what the command line said of those it refused; and the calls of the session's own agent to
/// the model, each as its first partial message tells its prompt, with the tools the agent used, and the program's
/// version, as its first line tells it.
#[derive(Debug, Default)]
pub struct Stream {
    /// Whether it gave any line.
    any: bool,
    /// Its result: the last it gave.
    result: Option<Value>,
    attempts: Attempts,
    /// What the command line said of each use of a tool of the agent's own that it refused, by the use's id.
    refusals: Vec<(String, String)>,
    /// What it said of the service's errors, in its own fields.
    said: failure::Said,
    /// Each call to the model, by its message's id, in the order they came, with its model and its tokens as its
    /// latest message gave them: the pieces of one call each give the usage so far.
    calls: Vec<(String, String, cost::Tokens)>,
    /// Whether the service has answered a call of the main loop with a message that is no error.
    answered: bool,
    /// The calls of the session's own agent.
    pub own: Calls,
    /// The uses of tools of the session's own agent, by the use's id, in order: the tool and what it was used on, cut.
    pub tools: Vec<(String, String, String)>,
    /// The program's version, where its first line told it.
    pub program: Option<String>,
}

/// The calls of the session's own agent to the model, as their first partial messages tell their prompts: a call made
/// by a subagent is left out, its context not being the session's.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Calls {
    /// How many.
    pub count: u32,
    /// The first's prompt: what it read from the cache, and its whole, in tokens.
    pub first: Option<(u64, u64)>,
    /// The latest's whole prompt.
    pub last: Option<u64>,
    /// The tokens the prompt rose by from each call to the next, each rise counted, a fall taking none back.
    pub added: u64,
    /// The tokens of the prompts sent fresh, read from the cache and written to it, over the calls.
    pub fresh: u64,
    pub read: u64,
    pub written: u64,
}

impl Calls {
    /// Takes a call whose prompt was sent fresh, read from the cache and written to it in these tokens.
    fn take(&mut self, fresh: u64, read: u64, written: u64) {
        let whole = fresh + read + written;
        (self.fresh, self.read, self.written) = (self.fresh + fresh, self.read + read, self.written + written);
        self.count += 1;
        if self.first.is_none() {
            self.first = Some((read, whole));
        }
        if let Some(last) = self.last {
            self.added += whole.saturating_sub(last);
        }
        self.last = Some(whole);
    }
}

/// How much of what a tool was used on is kept to say what the agent was doing, in characters.
const ON: usize = 160;

impl Stream {
    /// Takes one line of the stream, without its newline, and says whether the line is kept with the round: every
    /// line is but a partial message, which is kept only where it rebuilds an attempt at a return or starts a call of
    /// the session's own agent, so that what is kept rebuilds the attempts and the calls again, as a try its engine did
    /// not see end is read, and holds each token of the model's once.
    pub fn line(&mut self, line: &[u8]) -> bool {
        self.any = true;
        let Ok(text) = std::str::from_utf8(line) else {
            return true;
        };
        // The first partial message of each call tells its prompt, whether or not it rebuilds an attempt at a return.
        let starts = text.starts_with(PARTIAL) && text.contains(r#""message_start""#);
        if text.starts_with(PARTIAL) && !starts && !self.attempts.wants(text) {
            return false;
        }
        let Ok(v) = serde_json::from_str::<Value>(text) else {
            return true;
        };
        self.said.line(&v);
        self.called(&v);
        match v["type"].as_str() {
            Some("stream_event") => {
                let own = starts && v["parent_tool_use_id"].is_null();
                if own {
                    let usage = &v["event"]["message"]["usage"];
                    let tokens = |key: &str| usage[key].as_u64().unwrap_or(0);
                    let (fresh, read) = (tokens("input_tokens"), tokens("cache_read_input_tokens"));
                    self.own.take(fresh, read, tokens("cache_creation_input_tokens"));
                }
                return self.attempts.event(&v) || own;
            }
            Some("result") => self.result = Some(v),
            Some("user") if v["parent_tool_use_id"].is_null() => self.refusals.extend(refusals(&v)),
            Some("assistant") if v["parent_tool_use_id"].is_null() => self.used(&v),
            Some("system") if v["subtype"] == "init" => {
                self.program = v["claude_code_version"].as_str().map(str::to_string);
            }
            _ => {}
        }
        true
    }

    /// Takes the uses of tools a whole message of the agent's own holds, each once by its id.
    fn used(&mut self, message: &Value) {
        let content = message["message"]["content"].as_array().into_iter().flatten();
        for used in content.filter(|c| c["type"] == "tool_use") {
            let id = used["id"].as_str().unwrap_or_default().to_string();
            if self.tools.iter().any(|(seen, ..)| *seen == id) {
                continue;
            }
            let name = used["name"].as_str().unwrap_or_default().to_string();
            let on: String = used["input"].to_string().chars().take(ON).collect();
            self.tools.push((id, name, on));
        }
    }

    /// Whether the program said, as its stream began, that it had taken its message in and opened its session: its
    /// first line, which it prints once it has read its input.
    pub fn began(&self) -> bool {
        self.program.is_some()
    }

    /// Whether the stream gave any line.
    pub fn is_empty(&self) -> bool {
        !self.any
    }

    /// Whether the stream gave a result.
    pub fn has_result(&self) -> bool {
        self.result.is_some()
    }

    /// Whether the service has answered a call of the round's main loop with a message that is no error: it serves
    /// the round, whatever comes of it.
    pub fn answered(&self) -> bool {
        self.answered
    }

    /// Takes what a message of the model's says of its call: its tokens, and whether the service answered.
    fn called(&mut self, v: &Value) {
        if v["type"] != "assistant" {
            return;
        }
        let message = &v["message"];
        if v["parent_tool_use_id"].is_null() && v["error"].is_null() && v["api_error"].is_null() {
            self.answered = true;
        }
        let (Some(id), Some(usage)) = (message["id"].as_str(), message.get("usage")) else {
            return;
        };
        let model = message["model"].as_str().unwrap_or_default().to_string();
        let tokens = cost::Tokens::of(usage);
        match self.calls.iter_mut().find(|(call, ..)| call == id) {
            Some(call) => *call = (id.to_string(), model, tokens),
            None => self.calls.push((id.to_string(), model, tokens)),
        }
    }

    /// What the stream's calls cost at their models' prices, and their tokens; none where it made none.
    pub fn priced(&self) -> Option<Spent> {
        let last = self.calls.last()?;
        let mut spent = Spent {
            context_tokens: Some(last.2.prompt()),
            ..Spent::default()
        };
        for (_, model, tokens) in &self.calls {
            spent.usd_micros += cost::of_call(model, tokens);
            spent.input_tokens += tokens.input;
            spent.cached_tokens += tokens.read;
            spent.output_tokens += tokens.output;
        }
        Some(spent)
    }

    /// What the try's prompts came to as the program's result says: the tokens sent fresh, read from the cache and
    /// written to it, those thought in, where it says, and the turns; none where it printed no result.
    pub fn printed(&self) -> Option<(u64, u64, u64, Option<u64>, u64)> {
        let result = self.result.as_ref()?;
        let usage = &result["usage"];
        let tokens = |key: &str| usage[key].as_u64().unwrap_or(0);
        Some((
            tokens("input_tokens"),
            tokens("cache_read_input_tokens"),
            tokens("cache_creation_input_tokens"),
            usage["output_tokens_details"]["thinking_tokens"].as_u64(),
            result["num_turns"].as_u64().unwrap_or(0),
        ))
    }

    /// Reads the stream, for a round that began its session or went on in it with the session having cost `before`:
    /// the return the command line took, or, where it gave up on the return, its last attempt at it, rebuilt; or says
    /// why there is no return to take, with what the round took where the result could be read. A round that began
    /// its session cost the session's total; one that went on in it, that total less what the line's earlier rounds
    /// cost, or the whole total where that is less (measured: the total is the whole session's, even where two lines
    /// take turns in one worktree).
    pub fn read(self, begin: Begin, before: u64) -> std::result::Result<Read, NoResult> {
        let fields = self.said;
        let priced = self.priced();
        let Some(result) = self.result else {
            let why = "the command line's stream holds no result".to_string();
            return Err(NoResult {
                why,
                spent: priced,
                said: fields,
            });
        };
        let result: Result = match serde_json::from_value(result) {
            Ok(result) => result,
            Err(e) => {
                let why = format!("the command line's result cannot be read: {e}");
                return Err(NoResult {
                    why,
                    spent: priced,
                    said: fields,
                });
            }
        };
        let spent = spent(&result, begin, before);
        if !result.is_error {
            return match result.structured_output {
                Some(structured) => Ok(Read { structured, spent }),
                None => Err(NoResult {
                    why: "the command line's result holds no structured return".to_string(),
                    spent: Some(spent),
                    said: fields,
                }),
            };
        }
        // What it says goes back to the agent where the return is sent back: its quote of the attempt is left out.
        let said = (result.result.iter().chain(&result.errors))
            .map(|said| without_quote(said))
            .filter(|said| !said.is_empty())
            .collect::<Vec<_>>()
            .join("; ");
        let why = match result.subtype.as_deref() {
            Some(subtype) if subtype != "success" => {
                format!("the command line's result is an error ({subtype}): {said}")
            }
            _ => format!("the command line's result is an error: {said}"),
        };
        // The command line gave up on the return, having refused every attempt at it: the last is read as any return
        // is, by the engine's own reading, which sends it back where it cannot be read.
        if failure::gave_up(&why, &fields)
            && let Some((id, raw)) = self.attempts.last()
        {
            return match serde_json::from_str::<Value>(&raw) {
                Ok(structured) => Ok(Read { structured, spent }),
                Err(e) => {
                    // What the command line said of it, where its result does not already end with that.
                    let refused = (self.refusals.iter())
                        .find(|(refused, _)| *refused == id)
                        .map(|(_, said)| without_quote(said))
                        .filter(|said| !why.contains(said.as_str()))
                        .map(|said| format!(", and the command line said: {said}"))
                        .unwrap_or_default();
                    Err(NoResult {
                        why: format!("{why}; its last attempt at the return is no JSON ({e}){refused}"),
                        spent: Some(spent),
                        said: fields,
                    })
                }
            };
        }
        Err(NoResult {
            why,
            spent: Some(spent),
            said: fields,
        })
    }
}

/// Reads a whole stream, line by line, as [`Stream::read`] does.
pub fn read(stream: &[u8], begin: Begin, before: u64) -> std::result::Result<Read, NoResult> {
    let mut read = Stream::default();
    for line in stream.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
        read.line(line);
    }
    read.read(begin, before)
}

/// The inputs the model gave the command line's tool for the return, each an attempt at a return, rebuilt whole from
/// the partial messages (`input_json_delta`) of the agent's own messages; the pieces of every other input are passed
/// over unread.
#[derive(Debug, Default)]
struct Attempts {
    /// The input being written: its block's index in its message, its use's id, and the input so far.
    open: Option<(i64, String, String)>,
    /// Each input written, by its use's id, in order.
    done: Vec<(String, String)>,
}

impl Attempts {
    /// Whether a line that is a partial message may start, add to or end an input.
    fn wants(&self, line: &str) -> bool {
        let ends = ["input_json_delta", "content_block_stop", "message_start"];
        line.contains("content_block_start") || (self.open.is_some() && ends.iter().any(|kind| line.contains(kind)))
    }

    /// Takes a partial message, and says whether it started, added to or ended an input.
    fn event(&mut self, v: &Value) -> bool {
        if !v["parent_tool_use_id"].is_null() {
            return false;
        }
        let event = &v["event"];
        let index = event["index"].as_i64();
        match event["type"].as_str() {
            // A new message numbers its blocks afresh: an input left open was cut off, and is kept as it came.
            Some("message_start") => self.close(None),
            Some("content_block_start") => {
                let block = &event["content_block"];
                let Some(index) = index.filter(|_| block["type"] == "tool_use" && block["name"] == STRUCTURED_OUTPUT)
                else {
                    return false;
                };
                self.close(None);
                let id = block["id"].as_str().unwrap_or_default().to_string();
                self.open = Some((index, id, String::new()));
                true
            }
            Some("content_block_delta") if event["delta"]["type"] == "input_json_delta" => {
                match (&mut self.open, event["delta"]["partial_json"].as_str()) {
                    (Some((at, _, raw)), Some(piece)) if Some(*at) == index => {
                        raw.push_str(piece);
                        true
                    }
                    _ => false,
                }
            }
            Some("content_block_stop") => self.close(index),
            _ => false,
        }
    }

    /// Ends the input being written, at the stop of its block, or whatever block it is at for none; and says whether
    /// one was ended.
    fn close(&mut self, index: Option<i64>) -> bool {
        match self.open.take() {
            Some((at, id, raw)) if index.is_none_or(|index| index == at) => {
                self.done.push((id, raw));
                true
            }
            open => {
                self.open = open;
                false
            }
        }
    }

    /// The last input written, cut off or not, with its use's id.
    fn last(mut self) -> Option<(String, String)> {
        self.close(None);
        self.done.pop()
    }
}

/// The uses of tools the command line refused that a message of the agent's own answers, each by its use's id with
/// what the command line said: the results of tools that are errors.
fn refusals(message: &Value) -> Vec<(String, String)> {
    let content = message["message"]["content"].as_array().into_iter().flatten();
    (content.filter(|c| c["type"] == "tool_result" && c["is_error"] == true))
        .map(|c| {
            let said = match &c["content"] {
                Value::String(text) => text.clone(),
                Value::Array(parts) => (parts.iter().filter_map(|part| part["text"].as_str()))
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => String::new(),
            };
            (c["tool_use_id"].as_str().unwrap_or_default().to_string(), said)
        })
        .collect()
}

/// What the command line said, of an input it refused or of a return it gave up on, without its tags and without the
/// part of the input it quotes last, after "You sent (first 200 of N bytes):", which is the agent's own return.
fn without_quote(said: &str) -> String {
    let said = match said.find("bytes):") {
        Some(at) => &said[..at + "bytes)".len()],
        None => said,
    };
    let said = said.replace("<tool_use_error>", "").replace("</tool_use_error>", "");
    said.trim().to_string()
}

/// What a round's command line is given on its standard input: the message as one user message of the stream-json
/// input format, a line of JSON.
pub fn stdin(message: &str) -> Vec<u8> {
    let content = json!([{ "type": "text", "text": message }]);
    let line = json!({ "type": "user", "message": { "role": "user", "content": content } });
    let mut bytes = line.to_string().into_bytes();
    bytes.push(b'\n');
    bytes
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

    use decide::run::Failure;

    use super::*;

    /// A stream the command line printed for a session's first round, what named the machine and the account replaced:
    /// a tool used, then the return written in pieces and taken, the result, and a `task_summary` after it.
    const STREAM: &str = include_str!("../tests/fixtures/stream.jsonl");

    fn agents() -> Agents {
        Agents {
            command: "claude".to_string(),
            levels: Default::default(),
            slots: 2,
            args: vec!["--strict-mcp-config".to_string()],
            env: Vec::new(),
            setting_sources: "user".to_string(),
            accounts: Vec::new(),
            permissions: Default::default(),
            subagents: Default::default(),
            work_budget_usd: None,
        }
    }

    fn words() -> Words {
        Words {
            waiting: PathBuf::from("rounds/1/2-3.words.json"),
            delivered: PathBuf::from("rounds/1/2-3.delivered"),
        }
    }

    /// A partial message of the agent's own.
    fn event(event: Value) -> String {
        json!({ "type": "stream_event", "event": event, "parent_tool_use_id": null }).to_string()
    }

    /// The lines of a message that is an attempt at a return, in the order the command line writes them: its input
    /// written in these pieces as the message's first block, the whole message, the block's end, what the command line
    /// answered where it refused the attempt, as a plain string, and the message's end.
    fn attempt(id: &str, pieces: &[&str], refused: Option<&str>) -> Vec<String> {
        let block = json!({ "type": "tool_use", "id": id, "name": STRUCTURED_OUTPUT, "input": {} });
        let mut lines = vec![
            event(json!({ "type": "message_start", "message": {} })),
            event(json!({ "type": "content_block_start", "index": 0, "content_block": block })),
        ];
        for piece in pieces {
            let delta = json!({ "type": "input_json_delta", "partial_json": piece });
            lines.push(event(
                json!({ "type": "content_block_delta", "index": 0, "delta": delta }),
            ));
        }
        let input = serde_json::from_str::<Value>(&pieces.concat()).unwrap_or(json!({}));
        let used = json!({ "type": "tool_use", "id": id, "name": STRUCTURED_OUTPUT, "input": input });
        let message = json!({ "role": "assistant", "content": [used] });
        lines.push(json!({ "type": "assistant", "message": message, "parent_tool_use_id": null }).to_string());
        lines.push(event(json!({ "type": "content_block_stop", "index": 0 })));
        if let Some(said) = refused {
            let result = json!({ "type": "tool_result", "content": said, "is_error": true, "tool_use_id": id });
            let answer = json!({ "role": "user", "content": [result] });
            lines.push(json!({ "type": "user", "message": answer, "parent_tool_use_id": null }).to_string());
        }
        lines.push(event(json!({ "type": "message_delta", "delta": {}, "usage": {} })));
        lines.push(event(json!({ "type": "message_stop" })));
        lines
    }

    /// The result of a call that gave up on the return, in the command line's words, which end with the last refusal.
    fn gave_up(last: &str) -> String {
        let gave_up = "Failed to provide valid structured output after 3 attempts";
        let said = format!("{gave_up} — last StructuredOutput error: {last}");
        json!({ "type": "result", "subtype": "error_max_structured_output_retries", "is_error": true, "result": null,
            "errors": [said], "total_cost_usd": 0.4, "usage": { "input_tokens": 10, "output_tokens": 300 } })
        .to_string()
    }

    /// A stream the command line printed for a call that gave up on the return, what named the machine and the account
    /// replaced: twice the model wrote the list of asks as a string, which its schema refuses.
    const GAVE_UP: &str = include_str!("../tests/fixtures/gave-up.jsonl");

    #[test]
    fn the_stream_reads_to_its_return_and_what_it_took() {
        let read = read(STREAM.as_bytes(), Begin::New, 0).expect("the fixture reads");
        assert_eq!(
            read.spent,
            Spent {
                usd_micros: 2614,
                input_tokens: 9094,
                cached_tokens: 63348,
                output_tokens: 325,
                context_tokens: Some(36291),
                compactions: 0,
            }
        );
        assert_eq!(read.structured["hands_in"][0]["content"], "BANANA");
    }

    #[test]
    fn the_stream_tells_each_call_of_the_agents_own_its_tools_and_the_programs_version_and_the_result_what_it_took() {
        let mut stream = Stream::default();
        for line in STREAM.lines() {
            stream.line(line.as_bytes());
        }
        // Two calls, each as its first partial message tells its prompt: fresh, read from the cache, written to it.
        let calls = &stream.own;
        assert_eq!(calls.count, 2);
        assert_eq!(calls.first, Some((27_199, 2 + 27_199 + 8_950)));
        assert_eq!(calls.last, Some(2 + 36_149 + 140));
        assert_eq!(calls.added, (2 + 36_149 + 140) - (2 + 27_199 + 8_950));
        assert_eq!(
            (calls.fresh, calls.read, calls.written),
            (4, 27_199 + 36_149, 8_950 + 140)
        );
        assert_eq!(stream.program.as_deref(), Some("2.1.323"));
        let tools: Vec<&str> = stream.tools.iter().map(|(_, tool, _)| tool.as_str()).collect();
        assert_eq!(tools, ["Bash", STRUCTURED_OUTPUT]);
        // The result says the cache writes apart from the fresh tokens and the reads, the thinking and the turns.
        assert_eq!(stream.printed(), Some((4, 63_348, 9_090, Some(153), 3)));
    }

    #[test]
    fn a_partial_message_is_kept_only_where_it_rebuilds_an_attempt_at_a_return_or_starts_an_agents_call() {
        let mut stream = Stream::default();
        let kept: Vec<&str> = (STREAM.lines()).filter(|line| stream.line(line.as_bytes())).collect();
        let partial = kept.iter().filter(|line| line.starts_with(PARTIAL)).count();
        // The pieces of the thinking and of the other tool's input, and the messages' ends, are passed over; the
        // messages' starts, which say each call's prompt, the return's start, its eleven pieces and its stop are kept,
        // and every other line.
        assert_eq!(partial, 15, "{kept:#?}");
        assert_eq!(kept.len() - partial, 14);
        // What is kept rebuilds the same attempts, and only the attempts, and the same calls of the session's own
        // agent: a try its engine did not see end is read from it (decision
        // a-try-its-engine-did-not-see-end-ends-as-its-stream-says).
        let mut again = Stream::default();
        for line in &kept {
            assert!(again.line(line.as_bytes()), "{line}");
        }
        assert_eq!(again.own, stream.own);
        assert_eq!(again.priced(), stream.priced());
        assert_eq!((again.began(), again.has_result()), (true, true));
        assert_eq!(again.attempts.done.len(), 1);
        let raw = again.attempts.last().map(|(_, raw)| raw).expect("an attempt");
        assert_eq!(
            serde_json::from_str::<Value>(&raw).unwrap(),
            json!({ "hands_in": [{ "block": "greeting", "content": "BANANA" }], "asks": [] })
        );
    }

    #[test]
    fn the_last_attempt_of_a_return_the_command_line_gave_up_on_is_read() {
        let read = read(GAVE_UP.as_bytes(), Begin::New, 0).expect("the last attempt is taken");
        assert_eq!(read.structured, json!({ "hands_in": [], "asks": "\"[]\"" }));
        assert_eq!(read.spent.usd_micros, 2108, "what the call cost");
        // An attempt that is JSON is whole in its message too, as the pieces rebuild it.
        let message = (GAVE_UP.lines().rev())
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .find(|line| line["type"] == "assistant")
            .expect("the last attempt's message");
        assert_eq!(message["message"]["content"][0]["input"], read.structured);
    }

    #[test]
    fn a_return_the_command_line_gave_up_on_is_its_last_attempt_rebuilt_from_the_stream() {
        // Lists written as strings, which the command line's schema refuses and the engine reads; the first inside text
        // of characters of several bytes, cut into pieces anywhere.
        let list = json!([{ "block": "plan", "content": "Greet \u{1f600}\u{2192}\"", "matters": true }]);
        let first = json!({ "hands_in": list.to_string(), "asks": [] }).to_string();
        let last = r#"{"hands_in": [{"block": "plan", "content": "Greet by name.", "matters": true}], "asks": "[]"}"#;
        let cut: Vec<usize> = (first.char_indices().map(|(i, _)| i))
            .filter(|i| i % 7 == 0)
            .chain([first.len()])
            .collect();
        let pieces: Vec<&str> = cut.windows(2).map(|w| &first[w[0]..w[1]]).collect();
        let init = json!({ "type": "system", "subtype": "init", "session_id": "s" }).to_string();
        let first_refused = "Output does not match required schema: /hands_in: must be array";
        let last_refused = "Output does not match required schema: /asks: must be array";
        let mut lines = vec![init.clone()];
        lines.extend(attempt("so-1", &pieces, Some(first_refused)));
        lines.extend(attempt("so-2", &[&last[..20], &last[20..]], Some(last_refused)));
        lines.push(gave_up(last_refused));
        let read = read(lines.join("\n").as_bytes(), Begin::New, 0).expect("the last attempt is taken");
        assert_eq!(read.structured, serde_json::from_str::<Value>(last).unwrap());
        assert_eq!(read.spent.usd_micros, 400_000, "what the call cost");
        assert_eq!(read.spent.output_tokens, 300);

        // The first attempt alone, rebuilt whole across the pieces that cut its characters.
        let mut only = vec![init];
        only.extend(attempt("so-1", &pieces, Some(first_refused)));
        only.push(gave_up(first_refused));
        let read = super::read(only.join("\n").as_bytes(), Begin::New, 0).expect("it is taken");
        assert_eq!(read.structured, serde_json::from_str::<Value>(&first).unwrap());
    }

    #[test]
    fn a_last_attempt_that_is_no_json_is_no_return_and_says_why_without_quoting_it() {
        let broken = r#"{"hands_in": [{"block": "plan", "content": "a "quoted" word", "matters": true}], "asks": []}"#;
        let said = format!(
            "InputValidationError: StructuredOutput was called with input that could not be parsed as JSON.\nYou \
             sent (first 200 of {} bytes): {broken}",
            broken.chars().count()
        );
        let mut lines = attempt(
            "so-1",
            &[broken],
            Some(&format!("<tool_use_error>{said}</tool_use_error>")),
        );
        lines.push(gave_up(&said));
        let no = read(lines.join("\n").as_bytes(), Begin::New, 0).expect_err("no JSON is no return");
        assert!(
            no.why.contains("its last attempt at the return is no JSON"),
            "{}",
            no.why
        );
        assert_eq!(
            no.why.matches("could not be parsed as JSON.").count(),
            1,
            "said once: {}",
            no.why
        );
        assert!(
            !no.why.contains("quoted"),
            "the attempt itself is not quoted: {}",
            no.why
        );
        assert_eq!(
            failure::of(&no.why, &no.said, no.spent.as_ref()),
            Failure::Unread,
            "it is sent back"
        );
        assert_eq!(no.spent.map(|s| s.usd_micros), Some(400_000));
        // Given up on with no attempt in the stream, the result's own words say why, without the quote.
        let no = read(gave_up(&said).as_bytes(), Begin::New, 0).expect_err("nothing to take");
        assert!(
            no.why.contains("Failed to provide valid structured output"),
            "{}",
            no.why
        );
        assert!(no.why.ends_with("(first 200 of 92 bytes)"), "{}", no.why);
        // Where the result does not say what was refused, what the command line answered the attempt is added.
        let mut lines = attempt("so-1", &[broken], Some(&said));
        lines.push(
            json!({ "type": "result", "subtype": "error_max_structured_output_retries", "is_error": true,
            "errors": ["Failed to provide valid structured output after 3 attempts"] })
            .to_string(),
        );
        let no = read(lines.join("\n").as_bytes(), Begin::New, 0).expect_err("no JSON is no return");
        assert!(
            no.why.ends_with(
                "and the command line said: InputValidationError: StructuredOutput was called with \
                              input that could not be parsed as JSON.\nYou sent (first 200 of 92 bytes)"
            ),
            "{}",
            no.why
        );
    }

    #[test]
    fn an_attempt_of_a_subagent_or_of_another_failure_is_not_taken() {
        let mut lines = attempt("so-1", &[r#"{"hands_in": [], "asks": []}"#], None);
        let error = r#"{"type":"result","subtype":"success","is_error":true,"result":"API Error: 529 Overloaded"}"#;
        lines.push(error.to_string());
        assert!(
            read(lines.join("\n").as_bytes(), Begin::New, 0).is_err(),
            "a failure of another kind"
        );
        let mut stream = Stream::default();
        let sub = json!({ "type": "stream_event", "parent_tool_use_id": "toolu_9",
            "event": { "type": "content_block_start", "index": 0,
                       "content_block": { "type": "tool_use", "id": "x", "name": STRUCTURED_OUTPUT } } });
        assert!(!stream.line(sub.to_string().as_bytes()), "a subagent's is not kept");
        assert!(stream.attempts.last().is_none());
    }

    // A round that crashed before its result spent what its calls' tokens cost: each call once, at its latest usage,
    // a subagent's among them; and the service answered it, with a message that is no error.
    #[test]
    fn a_stream_cut_off_before_its_result_says_what_its_calls_cost() {
        let call = |id: &str, parent: Value, output: u64| {
            json!({ "type": "assistant", "parent_tool_use_id": parent, "message": { "id": id,
                "model": "claude-opus-5-5", "usage": { "input_tokens": 100, "cache_creation_input_tokens": 0,
                "cache_read_input_tokens": 1000, "output_tokens": output } } })
            .to_string()
        };
        let mut stream = Stream::default();
        assert!(!stream.answered());
        for line in [
            call("msg_1", Value::Null, 10),
            call("msg_1", Value::Null, 50),
            call("msg_2", json!("toolu_1"), 20),
        ] {
            stream.line(line.as_bytes());
        }
        assert!(stream.answered());
        let no = stream.read(Begin::New, 0).expect_err("no result");
        let spent = no.spent.expect("its calls are priced");
        assert_eq!(
            (spent.input_tokens, spent.cached_tokens, spent.output_tokens),
            (200, 2000, 70)
        );
        // Opus 5.5: 200 × 4 + 2000 × 0.2 + 70 × 20 millionths.
        assert_eq!(spent.usd_micros, 800 + 400 + 1400);
        // An error the service gave is no answer.
        let mut refused = Stream::default();
        let error = json!({ "type": "assistant", "parent_tool_use_id": null, "error": "overloaded", "message": {} });
        refused.line(error.to_string().as_bytes());
        assert!(!refused.answered());
    }

    // A weekly limit as version 2.1.323 reports it: the result's words and fields as a real round kept them, and the
    // refusal's message as the command line's schema gives it, resetting at 11pm in Los Angeles. The stream's fields
    // say the kind and the day of the reset, which the words alone give as a time of day.
    #[test]
    fn what_the_stream_says_of_a_refusal_comes_with_the_failure() {
        let refused = json!({ "type": "assistant", "parent_tool_use_id": null, "error": "rate_limit",
            "api_error": "usage_limit_reached", "api_error_status": 429,
            "api_error_params": { "rate_limit_info": { "status": "rejected", "resetsAt": 1_794_380_400_u64,
                "rateLimitType": "seven_day", "isUsingOverage": false } },
            "message": { "content": [{ "type": "text",
                "text": "You've hit your weekly limit · resets 11pm (America/Los_Angeles)" }] } });
        let result = json!({ "type": "result", "subtype": "success", "is_error": true, "api_error_status": 429,
            "api_error": "usage_limit_reached", "terminal_reason": "api_error", "total_cost_usd": 9.62,
            "result": "You've hit your weekly limit · resets 11pm (America/Los_Angeles)" });
        let stream = [refused.to_string(), result.to_string()].join("\n");
        let no = read(stream.as_bytes(), Begin::Resume, 9_620_000).expect_err("a refusal is not taken");
        assert_eq!(no.said.kind(), Some(Failure::UsageLimit));
        assert_eq!(no.said.resets, Some(1_794_380_400_000));
        assert_eq!(failure::of(&no.why, &no.said, no.spent.as_ref()), Failure::UsageLimit);
        // A stream cut off before its result says what it said all the same.
        let cut = read(refused.to_string().as_bytes(), Begin::New, 0).expect_err("no result");
        assert_eq!((cut.spent, cut.said.kind()), (None, Some(Failure::UsageLimit)));
    }

    #[test]
    fn a_round_ended_with_the_context_of_its_last_call() {
        let two = r#"{"type":"result","is_error":false,"total_cost_usd":0.1,"structured_output":{},"usage":{"input_tokens":3,"output_tokens":9,"iterations":[{"input_tokens":1,"cache_read_input_tokens":100,"cache_creation_input_tokens":20},{"input_tokens":2,"cache_read_input_tokens":120,"cache_creation_input_tokens":30}]}}"#;
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
        let read = read(STREAM.as_bytes(), Begin::Resume, 1000).expect("the fixture reads");
        assert_eq!(read.spent.usd_micros, 1614);
    }

    #[test]
    fn a_resumed_round_whose_session_cost_less_than_before_costs_the_whole_total() {
        let read = read(STREAM.as_bytes(), Begin::Resume, 5000).expect("the fixture reads");
        assert_eq!(read.spent.usd_micros, 2614);
    }

    #[test]
    fn an_error_and_what_is_no_result_are_not_taken() {
        let error = r#"{"type":"result","is_error":true,"result":"There's an issue with the selected model."}"#;
        let refused = read(error.as_bytes(), Begin::New, 0).expect_err("an error is not taken");
        assert!(refused.why.contains("issue with the selected model"), "{}", refused.why);
        let nothing = read(b"Error: something", Begin::New, 0).expect_err("what is no JSON is not taken");
        assert_eq!(nothing.spent, None, "what the round took is not known");
        let cut = STREAM.lines().take(5).collect::<Vec<_>>().join("\n");
        let cut = read(cut.as_bytes(), Begin::New, 0).expect_err("a stream with no result is not taken");
        assert_eq!(cut.spent, None);
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
            ..Default::default()
        };
        let refused = read(error.as_bytes(), Begin::Resume, 500_000).expect_err("an error is not taken");
        assert_eq!(refused.spent, Some(spent));
        let bare = r#"{"type":"result","is_error":false,"total_cost_usd":0.8}"#;
        let refused = read(bare.as_bytes(), Begin::New, 0).expect_err("no structured return is not taken");
        assert_eq!(refused.spent.map(|s| s.usd_micros), Some(800_000));
    }

    #[test]
    fn the_message_goes_in_as_one_user_message_of_the_stream() {
        let bytes = stdin("# Plan\n\nSay \"hello\".");
        assert_eq!(bytes.iter().filter(|b| **b == b'\n').count(), 1, "one line");
        let line: Value = serde_json::from_slice(&bytes).expect("JSON");
        assert_eq!(line["type"], "user");
        assert_eq!(line["message"]["role"], "user");
        assert_eq!(
            line["message"]["content"],
            json!([{ "type": "text", "text": "# Plan\n\nSay \"hello\"." }])
        );
    }

    #[test]
    fn the_command_line_begins_a_session_and_goes_on_in_it() {
        let session = Uuid::parse_str("6a1f3c2e-8d4b-4e7a-9c5f-2b8e1d0a7f43").unwrap();
        let permissions = Permissions::of(true, &[], &[], &Default::default());
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
                "SYSTEM",
                "SCHEMA",
                &permissions,
                (&compacted, &words(), Tools::Permitted),
                session,
                begin,
            )
        };
        let hooks = json!({ "autoMemoryEnabled": false, "hooks": {
            "SessionStart": [{ "matcher": "compact", "hooks": [{ "type": "command",
                "command": "printf x >> 'rounds/1/2-3.compacted' && cat 'rounds/1/2-3.again.md'" }] }],
            "PostToolUse": [{ "matcher": "*", "hooks": [{ "type": "command", "command": words().command() }] }],
        } });
        let hooks = hooks.to_string();
        let common = [
            "-p",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--verbose",
            "--include-partial-messages",
            "--model",
            "claude-haiku-5-5",
            "--append-system-prompt",
            "SYSTEM",
            "--json-schema",
            "SCHEMA",
            "--setting-sources",
            "user",
            "--settings",
            &hooks,
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
        assert!(!new.iter().any(|a| a == "--fork-session"), "no session is forked");
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
        let permissions = Permissions::of(false, &[], &[], &Default::default());
        let args = arguments(
            &agents(),
            &ran,
            "SYSTEM",
            "SCHEMA",
            &permissions,
            (&compacted, &words(), Tools::Permitted),
            session,
            Begin::New,
        );
        let at = args.iter().position(|a| a == "--model").expect("a model");
        assert_eq!(args[at..at + 4], ["--model", "claude-opus-5-5", "--effort", "xhigh"]);
    }

    // A keep-warm ping is given the very command line of a round of its session, its tools and permissions included,
    // which open every request of the session, so that it reads the session's prompt from the cache; its settings alone
    // add a hook before each use of a tool. Well under a millisecond alone; no other test holds what a ping's command
    // line is.
    #[test]
    fn a_ping_has_the_command_line_of_a_round_of_its_session_and_a_hook_that_refuses_its_tools() {
        let session = Uuid::parse_str("6a1f3c2e-8d4b-4e7a-9c5f-2b8e1d0a7f43").unwrap();
        let ran = RanOn {
            level: "1".to_string(),
            model: "claude-haiku-5-5".to_string(),
            effort: None,
            account: None,
        };
        let compacted = Compacted {
            marks: PathBuf::from("compacted"),
            again: PathBuf::from("again.md"),
        };
        let permissions = Permissions::of(false, &[], &[], &Default::default());
        let of = |tools| {
            arguments(
                &agents(),
                &ran,
                "SYSTEM",
                "SCHEMA",
                &permissions,
                (&compacted, &words(), tools),
                session,
                Begin::Resume,
            )
        };
        let round = of(Tools::Permitted);
        let ping = of(Tools::TheReturnAlone {
            refused: "Use no tool.",
        });
        let settings_of = |args: &[String]| {
            let at = args.iter().position(|a| a == "--settings").expect("settings");
            serde_json::from_str::<Value>(&args[at + 1]).expect("the settings' JSON")
        };
        let (mut round_settings, ping_settings) = (settings_of(&round), settings_of(&ping));
        let refusing = &ping_settings["hooks"]["PreToolUse"];
        assert_eq!(refusing[0]["matcher"], "*", "{ping_settings}");
        assert!(
            round_settings["hooks"].get("PreToolUse").is_none(),
            "a round's tools are refused"
        );
        round_settings["hooks"]["PreToolUse"] = refusing.clone();
        assert_eq!(round_settings, ping_settings, "the settings differ beyond the hook");
        let at = round.iter().position(|a| a == "--settings").expect("settings");
        let without = |args: &[String]| [&args[..at], &args[at + 2..]].concat();
        assert_eq!(
            without(&round),
            without(&ping),
            "the command lines differ beyond the settings"
        );
    }

    /// Runs the ping's hook as the command line does before a use of the tool of this name, its input this, and gives
    /// how it ended and what it said on its error output.
    fn before(tools: Tools<'_>, tool: &str, input: &str) -> (Option<i32>, String) {
        use std::io::Write;
        let mut hook = std::process::Command::new("sh")
            .args(["-c", &tools.command().expect("a hook")])
            .stdin(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("sh runs");
        let input = json!({
            "session_id": "s",
            "cwd": "/the \"work\"",
            "hook_event_name": "PreToolUse",
            "tool_name": tool,
            "tool_input": { "command": input },
        });
        (hook.stdin.take().expect("its input"))
            .write_all(input.to_string().as_bytes())
            .expect("the hook's input is written");
        let output = hook.wait_with_output().expect("the hook ends");
        (output.status.code(), String::from_utf8(output.stderr).expect("text"))
    }

    // A ping may use the tool for its return and no other: each other is refused as it is called, the words given the
    // model, an input that names the return's tool among them; a round has no such hook. Under a second alone, five
    // runs of the shell the program runs the hook with; no other test holds what a ping may use.
    #[test]
    fn a_pings_hook_lets_the_return_be_handed_in_and_refuses_every_other_tool_with_its_words() {
        let words = "Use no tool: it's a ping.";
        let ping = Tools::TheReturnAlone { refused: words };
        assert_eq!(before(ping, STRUCTURED_OUTPUT, "{}"), (Some(0), String::new()));
        for (tool, input) in [
            ("Bash", "git commit -am 'x'"),
            ("Edit", "a file"),
            ("Bash", r#"echo "tool_name":"StructuredOutput""#),
            ("StructuredOutputs", "{}"),
        ] {
            assert_eq!(
                before(ping, tool, input),
                (Some(2), format!("{words}\n")),
                "{tool} with {input}"
            );
        }
        assert_eq!(
            Tools::Permitted.command(),
            None,
            "a round's tools are refused by a hook"
        );
    }

    /// Runs the hook's command as the command line does after a use of the tool of this name, in the directory, and
    /// gives what it printed. The tool's name comes first, and its input after it is large, as a return can be: the
    /// hook reads it all, and writing it meets no pipe the hook has closed.
    fn after(words: &Words, tool: &str, dir: &Path) -> String {
        use std::io::Write;
        let mut hook = std::process::Command::new("sh")
            .args(["-c", &words.command()])
            .current_dir(dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("sh runs");
        let large = "x".repeat(1 << 20);
        let input =
            format!(r#"{{"tool_name":"{tool}","hook_event_name":"PostToolUse","tool_input":{{"text":"{large}"}}}}"#);
        (hook.stdin.take().expect("its input"))
            .write_all(input.as_bytes())
            .expect("the hook's input is written");
        let output = hook.wait_with_output().expect("the hook ends");
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout).expect("text")
    }

    #[tokio::test]
    async fn the_owners_words_are_given_once_after_a_tool_and_moved_aside_as_delivered() {
        let dir = tempfile::tempdir().unwrap();
        // A data directory whose path a shell would split, or end a quote at.
        let rounds = dir.path().join("it's data").join("rounds");
        std::fs::create_dir_all(&rounds).unwrap();
        let words = Words {
            waiting: rounds.join("2-3.words.json"),
            delivered: rounds.join("2-3.delivered"),
        };
        assert_eq!(after(&words, "Bash", dir.path()), "", "nothing waits");

        words.put("Use the name \"Ann\".").await.expect("put");
        // Not after the tool for the return: the round has made it.
        assert_eq!(after(&words, STRUCTURED_OUTPUT, dir.path()), "");
        assert!(words.waiting.exists());
        let given: Value = serde_json::from_str(&after(&words, "Bash", dir.path())).expect("the hook's JSON");
        assert_eq!(given["hookSpecificOutput"]["hookEventName"], "PostToolUse");
        assert_eq!(
            given["hookSpecificOutput"]["additionalContext"],
            "Use the name \"Ann\"."
        );
        assert!(!words.waiting.exists(), "moved aside");
        let first = rounds.join("2-3.delivered.1.json");
        assert_eq!(
            serde_json::from_slice::<Value>(&std::fs::read(&first).unwrap()).unwrap(),
            given
        );
        assert_eq!(after(&words, "Read", dir.path()), "", "given once");

        // Said twice before a tool, given once, together; the next delivery is kept beside the first.
        words.put("First.").await.expect("put");
        words.put("Second.").await.expect("put");
        let given: Value = serde_json::from_str(&after(&words, "Edit", dir.path())).expect("the hook's JSON");
        assert_eq!(given["hookSpecificOutput"]["additionalContext"], "First.\n\nSecond.");
        assert!(rounds.join("2-3.delivered.2.json").is_file());
        assert!(first.is_file());
        let left: Vec<_> = std::fs::read_dir(&rounds)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(left.len(), 2, "nothing else is left: {left:?}");
    }

    #[tokio::test]
    async fn what_waits_for_the_hook_is_told_apart_from_what_the_hook_gave() {
        let dir = tempfile::tempdir().unwrap();
        let words = Words {
            waiting: dir.path().join("1-1.words.json"),
            delivered: dir.path().join("1-1.delivered"),
        };
        assert_eq!(words.put("First.").await, Ok(false), "nothing waited");
        assert_eq!(
            words.put("Second.").await,
            Ok(true),
            "the first waited, the hook not having given it"
        );
        assert_ne!(after(&words, "Bash", dir.path()), "");
        assert_eq!(words.put("Third.").await, Ok(false), "the hook gave the first two");
        assert_eq!(words.take_back().await, Ok(true), "the third waited");
        assert!(!words.waiting.exists(), "taken back, for no hook to give");
        assert_eq!(words.take_back().await, Ok(false), "nothing waits");
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

    // A round's command line is given of the engine's environment what is kept and what the settings name, then the
    // carrier's own variables, which are the agents' program's: none of a project's toolchain, which its declaration
    // names. Well under a millisecond alone; no other test holds what a round's command line is given.
    #[test]
    fn the_environment_is_built_from_what_is_kept_and_named_and_the_carriers_own() {
        let engine = [
            ("PATH", "/bin"),
            ("HOME", "/home/someone"),
            ("LC_ALL", "C"),
            ("SECRET_TOKEN", "x"),
            ("ANTHROPIC_API_KEY", "k"),
        ]
        .map(|(n, v)| (n.to_string(), v.to_string()));
        let none = BTreeMap::new();
        assert_eq!(
            environment(&agents(), None, engine.clone(), &none),
            [
                ("HOME", "/home/someone"),
                ("LC_ALL", "C"),
                ("PATH", "/bin"),
                ("CLAUDE_CODE_DISABLE_BACKGROUND_TASKS", "1"),
                ("CLAUDE_CODE_ENABLE_FINE_GRAINED_TOOL_STREAMING", "0"),
                ("MAX_STRUCTURED_OUTPUT_RETRIES", "3"),
            ]
            .map(|(n, v)| (n.to_string(), v.to_string()))
        );
        let mut configured = agents();
        configured.env = vec!["ANTHROPIC_API_KEY".to_string()];
        let under = environment(&configured, Some("/data/claude-second"), engine.clone(), &none);
        let named: Vec<&str> = under.iter().map(|(name, _)| name.as_str()).collect();
        assert!(named.contains(&"ANTHROPIC_API_KEY"));
        assert!(!named.contains(&"SECRET_TOKEN"));
        // The account it runs under is the one it is given, the directory as written.
        let dir = under
            .iter()
            .find(|(name, _)| name == "CLAUDE_CONFIG_DIR")
            .map(|(_, dir)| dir.as_str());
        assert_eq!(dir, Some("/data/claude-second"));
        // What the project's declaration names for its toolchain comes after the carrier's own.
        let declared = BTreeMap::from([("RUSTUP_AUTO_INSTALL".to_string(), "0".to_string())]);
        let given = environment(&agents(), None, engine, &declared);
        assert_eq!(
            given.last(),
            Some(&("RUSTUP_AUTO_INSTALL".to_string(), "0".to_string()))
        );
    }

    // A declaration may name no variable the engine gives every round itself, and none that is no name of a variable.
    // Well under a millisecond alone; no other test holds what a declaration may name for the command line.
    #[test]
    fn a_declaration_names_no_variable_of_the_engines_own() {
        let declared = |name: &str| BTreeMap::from([(name.to_string(), "1".to_string())]);
        assert_eq!(
            environment_faults(&declared("RUSTUP_AUTO_INSTALL")),
            Vec::<String>::new()
        );
        for own in ["PATH", "LC_ALL", "MAX_STRUCTURED_OUTPUT_RETRIES", "CLAUDE_CONFIG_DIR"] {
            assert_eq!(
                environment_faults(&declared(own)),
                [format!(
                    "flow/project.toml [environment]: {own} is the engine's own, which it gives every round itself"
                )],
                "{own}"
            );
        }
        for wrong in ["", "1A", "A B", "A=B"] {
            assert_eq!(
                environment_faults(&declared(wrong)),
                [format!(
                    "flow/project.toml [environment]: {wrong:?} is no name of a variable"
                )],
                "{wrong:?}"
            );
        }
    }
}
