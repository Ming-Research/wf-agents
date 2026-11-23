//! The `claude` command line, as a round is run with it: the arguments, the environment, the message on its standard
//! input, which session it begins or goes on in, the hooks it is given, and its stream read into what came back and
//! what it took. Each is a function of what it is given, with no I/O, but for the hooks' shell commands and the files
//! the owner's words wait in ([`Words`]).
//!
//! As runs of version 2.1.323 show:
//! `-p --input-format stream-json` reads the message from the standard input as one user message of the stream-json
//! format, a line of JSON ([`stdin`]); `--output-format stream-json --verbose` prints a line of JSON for each thing
//! that happens, and near the end the result, which only the command line's own `system` lines (a `task_summary`)
//! follow, with the structured return in `structured_output`, the tokens of this call in `usage`, and in
//! `model_usage_since_last_result` what the program spent, subagents and compactions included, since the result before
//! it, or since the program began where there is none: a round's own cost is the sum of those over its results, and
//! `total_cost_usd`, the whole session's cost, is read only by the one-off repair of recorded costs ([`Stream::told`]).
//! Each message of the model comes whole in an `assistant` line;
//! with `--include-partial-messages` it also comes in pieces as the model writes it (`stream_event` lines), the input
//! of each tool among them. The result gives no text of an attempt at a return the command line refused: an attempt
//! that is JSON is whole in its `assistant` line, and one cut off, which is no JSON, only in its pieces. A failed call
//! exits with 1 and, where it got as far, prints a result with `is_error` set and its reason in `result` or `errors`,
//! which at a return given up on ends with the last refusal, quoting the attempt where it could not be parsed.
//! Beginning a session whose id is taken says on the standard error `Session ID <id> is already in use.`, and going on
//! in one there is none of says `No conversation found with session ID: <id>`; both print nothing on the standard
//! output. Version 2.1.327 says the second in the `errors` of a result with `is_error` set instead, which is the one
//! line it prints, and exits with 1.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use decide::{BlockId, RunId, Seq};
use driver::Said;
use flow::Compiled;
use flow::agents::{ProgramWords, fill};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use service::Token;
use uuid::Uuid;
use work::{RanOn, Spent, Work, sessions};

use crate::permissions::Permissions;
use crate::settings::Agents;
use crate::{cost, failure};

/// What the command line says when a session is begun under an id that has one already.
pub const IN_USE: &str = "is already in use";
/// What the command line says when a session that is not there is gone on in.
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

    /// Whether what the command line said means the session is to be begun the other way.
    pub fn refused(self, said: &str) -> bool {
        match self {
            Begin::New => said.contains(IN_USE),
            Begin::Resume => said.contains(NOT_FOUND),
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
/// use of a tool moves that file aside, as the first `<given>.<n>.json` there is none of, and prints it, which the
/// program adds to the session. The hook needs nothing but the shell and these files, so that what runs the agents
/// may be apart from the engine. After the program's own tool for the return the hook gives nothing: the round has
/// made its return, and words given then would be taken as delivered and never read. Which versions of the owner's
/// words each file holds is kept beside them, in a file the program never reads, by the digest of what the file says
/// ([`Words::given`]): whoever reads what the hook gave, the process of the engine that put the words or one after it,
/// reads it there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Words {
    /// The file the words wait in, as the hook prints them.
    pub waiting: PathBuf,
    /// Where the words the hook gave are moved, without the number and the extension.
    pub given: PathBuf,
}

/// Something put for the hook to give and not taken back, as the versions file keeps it: the digest of what the hook is
/// to print ([`work::digest`]), and the versions of the owner's words it says.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Put {
    digest: String,
    versions: Vec<(BlockId, Seq)>,
}

impl Words {
    /// The hook's shell command: it reads what the program says of the tool on its standard input, whole, so that the
    /// program never writes to a pipe nobody reads, and gives nothing after the tool for the return.
    fn command(&self) -> String {
        let (waiting, given) = (quote(&self.waiting), quote(&self.given));
        format!(
            "input=$(cat); case \"$input\" in \
             *'\"tool_name\":\"{STRUCTURED_OUTPUT}\"'* | *'\"tool_name\": \"{STRUCTURED_OUTPUT}\"'*) ;; \
             *) if [ -e {waiting} ]; then n=1; while [ -e {given}.$n.json ]; do n=$((n+1)); done; \
             {{ mv {waiting} {given}.$n.json 2>/dev/null && cat {given}.$n.json; }} || :; fi ;; esac"
        )
    }

    /// Where what was put is kept with its versions, in the order it was put: beside what the hook gave.
    fn versions(&self) -> PathBuf {
        self.given.with_extension("said.json")
    }

    /// Where the hook moved what it gave the `n`th time, the first being one.
    fn given_at(&self, n: usize) -> PathBuf {
        let mut at = self.given.clone().into_os_string();
        at.push(format!(".{n}.json"));
        PathBuf::from(at)
    }

    /// What was put and not taken back, in the order it was put; nothing where nothing was.
    async fn puts(&self) -> std::result::Result<Vec<Put>, String> {
        let at = self.versions();
        match tokio::fs::read(&at).await {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", at.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(format!("{}: {e}", at.display())),
        }
    }

    /// Keeps what was put, whole or not at all.
    async fn keep(&self, puts: &[Put]) -> std::result::Result<(), String> {
        let at = self.versions();
        let named = |path: &Path, e: std::io::Error| format!("{}: {e}", path.display());
        let bytes = serde_json::to_vec(puts).map_err(|e| format!("{}: {e}", at.display()))?;
        let part = at.with_extension("json.part");
        tokio::fs::write(&part, bytes).await.map_err(|e| named(&part, e))?;
        tokio::fs::rename(&part, &at).await.map_err(|e| named(&at, e))
    }

    /// Takes what waits for the hook from where it waits, the hook not having moved it, and gives what it says; none
    /// where nothing waits. A rename either takes it whole or finds that the hook has moved it.
    async fn take(&self) -> std::result::Result<Option<String>, String> {
        let taking = self.waiting.with_extension("json.taking");
        match tokio::fs::rename(&self.waiting, &taking).await {
            Ok(()) => {
                let text =
                    (tokio::fs::read_to_string(&taking).await).map_err(|e| format!("{}: {e}", taking.display()))?;
                (tokio::fs::remove_file(&taking).await).map_err(|e| format!("{}: {e}", taking.display()))?;
                Ok(Some(text))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{}: {e}", self.waiting.display())),
        }
    }

    /// Puts what is said, which says these versions of the owner's words, where the hook reads it, as the hook is to
    /// print it: what the program adds to the session after a use of a tool. What waits there still is taken back first
    /// and given before it, with its versions, so that one file holds everything said. The versions are kept before
    /// the file is put, so that whatever the hook gives has them. One writer puts at a time.
    pub async fn put(&self, said: &str, versions: &[(BlockId, Seq)]) -> std::result::Result<(), String> {
        let named = |path: &Path, e: std::io::Error| format!("{}: {e}", path.display());
        let mut puts = self.puts().await?;
        let mut context = String::new();
        let mut with = Vec::new();
        if let Some(waited) = self.take().await? {
            let value: Value = serde_json::from_str(&waited).map_err(|e| format!("{}: {e}", self.waiting.display()))?;
            if let Some(text) = value["hookSpecificOutput"]["additionalContext"].as_str() {
                context.push_str(text);
                context.push_str("\n\n");
            }
            let digest = work::digest(&waited);
            if let Some(at) = puts.iter().rposition(|put| put.digest == digest) {
                with = puts.remove(at).versions;
            }
        }
        context.push_str(said);
        let output = json!({ "hookSpecificOutput": { "hookEventName": "PostToolUse", "additionalContext": context } });
        let output = output.to_string();
        with.extend_from_slice(versions);
        puts.push(Put {
            digest: work::digest(&output),
            versions: with,
        });
        self.keep(&puts).await?;
        // Written whole beside it and renamed into place, so that the hook never moves a file half written.
        let part = self.waiting.with_extension("json.part");
        tokio::fs::write(&part, output).await.map_err(|e| named(&part, e))?;
        tokio::fs::rename(&part, &self.waiting)
            .await
            .map_err(|e| named(&self.waiting, e))
    }

    /// Takes back what waits for the hook, once the program has ended and its hook runs no more, as a process of the
    /// engine that ended may have left it: gives whether something waited there, which the hook never gave, and whose
    /// versions stay new.
    pub async fn take_back(&self) -> std::result::Result<bool, String> {
        let Some(waited) = self.take().await? else {
            return Ok(false);
        };
        let mut puts = self.puts().await?;
        let digest = work::digest(&waited);
        if let Some(at) = puts.iter().rposition(|put| put.digest == digest) {
            puts.remove(at);
            self.keep(&puts).await?;
        }
        Ok(true)
    }

    /// What the hook gave, in the order it gave it: the versions each file it moved says, as they were put. A file
    /// whose versions were not kept, as one given before they were, is given apart, its words not known to be given.
    pub async fn given(&self) -> std::result::Result<(Vec<(BlockId, Seq)>, Vec<PathBuf>), String> {
        let mut puts = self.puts().await?;
        let (mut versions, mut unknown) = (Vec::new(), Vec::new());
        for n in 1.. {
            let at = self.given_at(n);
            let text = match tokio::fs::read_to_string(&at).await {
                Ok(text) => text,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
                Err(e) => return Err(format!("{}: {e}", at.display())),
            };
            let digest = work::digest(&text);
            match puts.iter().position(|put| put.digest == digest) {
                Some(put) => versions.extend(puts.remove(put).versions),
                None => unknown.push(at),
            }
        }
        Ok((versions, unknown))
    }
}

/// A path as a shell reads it back as it is: in single quotes.
fn quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}

/// The settings the engine gives the program, as JSON. Its hooks: at the start of a session it compacted, and after
/// each use of a tool, the program adds what the hook's command prints to the session. And its own memory off
/// (`autoMemoryEnabled`, which its settings' schema says keeps it from reading or writing its auto-memory directory):
/// a session holds what the engine gives it and what its agent reads, and the program's own memory, which its own
/// system prompt would tell the agent to write, would hold what the engine never sees, against the rule the agents are
/// given that they keep nothing there. And what every command of the program's is given ([`keeping`]).
fn settings(compacted: &Compacted, words: &Words) -> String {
    let hook = |command: String| json!([{ "type": "command", "command": command }]);
    let hooks = json!({
        "SessionStart": [{ "matcher": "compact", "hooks": hook(compacted.command()) }],
        "PostToolUse": [{ "matcher": "*", "hooks": hook(words.command()) }],
    });
    let mut settings = keeping();
    settings["autoMemoryEnabled"] = json!(false);
    settings["hooks"] = hooks;
    settings.to_string()
}

/// The settings every command of the program's the engine runs is given, as `--settings`: that its cleanup keeps a
/// session for [`KEEP_DAYS`](crate::account::KEEP_DAYS) days (`cleanupPeriodDays`, which any settings file and the
/// command line's `--settings` may set, as the program's documentation says), so that a session the engine goes on in,
/// however long its work waited, is never deleted from the program's directory, which is the engine's (decision
/// the-programs-directory-is-the-engines). Given with every command, it holds whatever the directory's own files say.
pub fn keeping() -> Value {
    json!({ "cleanupPeriodDays": crate::account::KEEP_DAYS })
}

/// The arguments of a round's command line, after the program: the engine's own, then the installation's, then the
/// line's permissions, then the session, with the context at which the program compacts it (`compact`). The message
/// goes in, and the stream comes out, as stream-json; the stream carries each message in pieces as it is written,
/// which rebuild an attempt at a return the command line refused. The schema is the form of return of the round's
/// flow, the same for every round of every cell of it, and the system prompt is the same for every round of a line, so
/// that they open every request of a session alike and the prompt cache keeps them. The round runs on the model and at
/// the effort it was given.
#[allow(clippy::too_many_arguments)]
pub fn arguments(
    agents: &Agents,
    ran: &RanOn,
    system: &str,
    schema: &str,
    permissions: &Permissions,
    compacted: &Compacted,
    words: &Words,
    compact: u64,
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
    args.extend(["--settings".to_string(), settings(compacted, words)]);
    args.extend(agents.args.iter().cloned());
    args.extend(permissions.arguments());
    args.extend(["--autocompact".to_string(), compact.to_string()]);
    let flag = match begin {
        Begin::New => "--session-id",
        Begin::Resume => "--resume",
    };
    args.extend([flag.to_string(), session.to_string()]);
    args
}

/// The arguments of a fork of a session: those of the session's round, as [`arguments`] makes them with `--resume`, so
/// that the fork's first request begins as the session's did and is read from the prompt cache (the same system prompt,
/// form of return, tools, model and effort); but for its settings' hooks, which gain one that refuses every use of a
/// tool before it runs ([`refusal`]), and the session, forked after it rather than gone on with in place. The tools stay
/// in the request: one given none, its front changed, read nothing of the session from the cache. Where the session went
/// on past the round asked of, the fork begins at that round's last message, `begins`, and holds nothing of what came
/// after.
pub fn forked(args: Vec<String>, begins: Option<&str>) -> Vec<String> {
    let mut args = refusing(args);
    args.push("--fork-session".to_string());
    if let Some(begins) = begins {
        args.extend(["--resume-session-at".to_string(), begins.to_string()]);
    }
    args
}

/// The arguments of a call that only reads its session, as a keep-warm ping makes: those of the session's round, but
/// for its settings' hooks, which gain one that refuses every use of a tool before it runs ([`refusal`]), so that
/// the call changes nothing in the worktree the work's other lines run in. The tools stay in the request, as for a
/// fork ([`forked`]); the session is gone on with in place.
pub fn refusing(mut args: Vec<String>) -> Vec<String> {
    let at = args.iter().position(|a| a == "--settings").map(|i| i + 1);
    if let Some(value) = at.and_then(|i| args.get_mut(i)) {
        let mut settings: Value = serde_json::from_str(value).unwrap_or_else(|_| json!({}));
        let refuse = json!([{ "type": "command", "command": refusal() }]);
        settings["hooks"]["PreToolUse"] = json!([{ "matcher": "*", "hooks": refuse }]);
        *value = settings.to_string();
    }
    args
}

/// The last message of a round in its session, as the round's stream names it: the id of the last message the program
/// said it wrote to the session, of the session's own agent or given to it; a subagent's, which is not of the session's
/// own thread, is passed over, as [`Stream`] passes it over. None where the stream names none.
pub fn last_message(stream: &[u8]) -> Option<String> {
    (String::from_utf8_lossy(stream).lines())
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| (v["type"] == "assistant" || v["type"] == "user") && v["parent_tool_use_id"].is_null())
        .filter_map(|v| v["uuid"].as_str().map(str::to_string))
        .next_back()
}

/// The hook's shell command that refuses the use of a tool, as the command line takes a hook's exit with 2, saying
/// nothing; but the program's own tool for the return, which it lets through, so that the answer comes in the return's
/// form. It reads what the program says of the tool on its standard input, whole, as the words' hook does.
pub fn refusal() -> String {
    format!(
        "input=$(cat); case \"$input\" in \
         *'\"tool_name\":\"{STRUCTURED_OUTPUT}\"'* | *'\"tool_name\": \"{STRUCTURED_OUTPUT}\"'*) exit 0 ;; \
         *) exit 2 ;; esac"
    )
}

/// The families of models the command line knows, each also its alias for the family's latest model.
const FAMILIES: [&str; 5] = ["haiku", "sonnet", "opus", "fable", "mythos"];

/// Whether the command line takes a model by this name, in the forms its `--model` takes: the alias of a family's
/// latest model (`opus`) or a model's full name (`claude-opus-5-5`, `claude-3-5-sonnet-20241022`), each with `[1m]`
/// after it or not; a family it does not know, or a name in no such form, it does not take.
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

/// The variable that has the command line strip every credential from the environment of the commands, hooks and
/// servers it starts, keeping it for its own calls.
pub const SCRUB: &str = "CLAUDE_CODE_SUBPROCESS_ENV_SCRUB";

/// What a refusal of a permission mode says to write instead: the program starts in default permission mode under
/// [`SCRUB`], so what an agent may do is given as rules.
pub const PERMISSION_ADVICE: &str =
    "allow what agents may do with --allowedTools rules in agents.args, such as Edit(./**)";

/// The variables the carrier sets for every round's command line: a round waits for whatever it starts, since a
/// subagent left in the background has the command line ask for the return before it is back; the service checks each
/// tool's input as JSON before it streams it, since inputs it did not check came cut off; the
/// command line asks the model again three times, not five, for a return that does not fit its form, each try writing
/// the whole return again; and no command an agent runs is given the token of the account the round runs under
/// ([`SCRUB`]). What a project's toolchain reads is the project's, which its declaration names (`[environment]`).
pub const ENVIRONMENT: [(&str, &str); 4] = [
    ("CLAUDE_CODE_DISABLE_BACKGROUND_TASKS", "1"),
    ("CLAUDE_CODE_ENABLE_FINE_GRAINED_TOOL_STREAMING", "0"),
    ("MAX_STRUCTURED_OUTPUT_RETRIES", "3"),
    (SCRUB, "1"),
];

/// The prefixes of the variables the agents' program reads itself, such as the account's token and the program's
/// directory of settings, its endpoint and its model: a project's declaration names none of them, since each would
/// change a round while the records name what the engine chose. Prefixes, not names, since the program's variables are
/// never all listed.
pub const PROGRAM: [&str; 2] = ["CLAUDE_", "ANTHROPIC_"];

/// The environment a command of the program's is given, in place of the engine's own: of the engine's variables,
/// those [`KEPT`], every `LC_*`, and those the settings name; then the carrier's own, [`ENVIRONMENT`]; the variables the
/// project's declaration names, `declared`; the program's directory of settings and sessions, the engine's `profile`;
/// and the token of the account the command runs under, which is given nowhere else.
pub fn environment(
    agents: &Agents,
    profile: &Path,
    engine: impl IntoIterator<Item = (String, String)>,
    declared: &BTreeMap<String, String>,
    token: &Token,
) -> Vec<(String, String)> {
    let kept =
        |name: &str| KEPT.contains(&name) || name.starts_with("LC_") || agents.env.iter().any(|named| named == name);
    let mut env: Vec<(String, String)> = engine.into_iter().filter(|(name, _)| kept(name)).collect();
    env.sort();
    env.extend(ENVIRONMENT.map(|(name, value)| (name.to_string(), value.to_string())));
    env.extend(declared.iter().map(|(name, value)| (name.clone(), value.clone())));
    env.push((
        crate::account::VARIABLE.to_string(),
        profile.to_string_lossy().into_owned(),
    ));
    env.push((crate::account::TOKEN.to_string(), token.secret().to_string()));
    env
}

/// What is wrong with the variables a project's declaration names for every round's command line, each said with where
/// it is written: a name that is no name of a variable; one the engine owns, which it keeps of its environment, sets
/// itself or gives the program; one the installation's settings name to keep (`agents.env`: `named`), which a declared
/// value would silently override; one the agents' program reads itself ([`PROGRAM`]); and a value that holds a NUL, which
/// ends a string of a process's environment, so that no program can be started with it. Any other value is none.
pub fn environment_faults(declared: &BTreeMap<String, String>, named: &[String]) -> Vec<String> {
    let at = format!("{} [environment]", flow::agents::DECLARATION);
    let mut faults = Vec::new();
    for (name, value) in declared {
        if value.contains('\0') {
            faults.push(format!(
                "{at}: {name} holds a NUL in its value, with which no program can be started"
            ));
        }
        let shell = name.starts_with(|c: char| c == '_' || c.is_ascii_alphabetic())
            && name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric());
        if !shell {
            faults.push(format!("{at}: {name:?} is no name of a variable"));
        } else if KEPT.contains(&name.as_str())
            || name.starts_with("LC_")
            || ENVIRONMENT.iter().any(|(own, _)| own == name)
        {
            faults.push(format!(
                "{at}: {name} is the engine's own, which it gives every round itself"
            ));
        } else if named.contains(name) {
            faults.push(format!(
                "{at}: {name} is one the installation's agents.env keeps of the engine's environment, which a declared value would override"
            ));
        } else if PROGRAM.iter().any(|prefix| name.starts_with(prefix)) {
            faults.push(format!(
                "{at}: {name} is read by the agents' program itself, which it would change in a round beside what the records name"
            ));
        }
    }
    faults
}

/// The session a round runs in, as the records say: which of its line's sessions, and whether the round begins it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InSession {
    pub number: u32,
    pub begin: Begin,
}

/// The session the round of this number of the run of a cell on the line runs in: it goes on in it where an earlier
/// round of the line in the same session put its message into it, having reached the model or its program having said
/// it took the message in (`work::sessions::wrote_into`).
pub fn in_session(flow: &Compiled, work: &Work, line: &str, run: RunId, number: usize) -> InSession {
    let on_line = |run: RunId| {
        let cell = work.records.started.iter().find(|s| s.run == run).map(|s| &s.cell);
        let spec = cell.and_then(|cell| flow.cells.iter().find(|spec| spec.id == *cell));
        spec.and_then(|spec| spec.line.as_deref()) == Some(line)
    };
    let (session, begins) = sessions::of_round(work, line, on_line, run, number);
    InSession {
        number: session,
        begin: if begins { Begin::New } else { Begin::Resume },
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
    usage: Usage,
    structured_output: Option<Value>,
}

impl Result {
    /// What the result says went wrong: its text and its errors, each without its quote of an attempt at the return.
    fn said(&self) -> String {
        (self.result.iter().chain(&self.errors))
            .map(|said| without_quote(said))
            .filter(|said| !said.is_empty())
            .collect::<Vec<_>>()
            .join("; ")
    }
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
    /// Why there is no return, as a sentence in the words beside the flows.
    pub why: String,
    /// What the command line itself reported of it, in no word of the engine's: what the kind of the failure is told
    /// by, so that changing the words beside the flows changes no kind.
    pub report: String,
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
    /// How many lines it gave.
    lines: usize,
    /// Its result: the last it gave, of the message being answered.
    result: Option<Value>,
    /// The results of the messages answered before it, where the program was given more than one message: each ended
    /// before the next message's first call began.
    earlier: Vec<Value>,
    attempts: Attempts,
    /// What the command line said of each use of a tool of the agent's own that it refused, by the use's id.
    refusals: Vec<(String, String)>,
    /// What it said of the service's errors, in its own fields.
    said: failure::Said,
    /// Each call to the model, by its message's id, in the order they came: the pieces of one call each give the usage
    /// so far, and the call's own `message_delta` gives the tokens it wrote in the end.
    calls: Vec<Call>,
    /// Whether the service has answered a call of the main loop with a message that is no error.
    answered: bool,
    /// The calls of the session's own agent.
    pub own: Calls,
    /// The uses of tools of the session's own agent, by the use's id, in order: the tool and what it was used on, cut.
    pub tools: Vec<(String, String, String)>,
    /// The program's version, where its first line told it.
    pub program: Option<String>,
    /// The names of the tools the program says, in its first line, it gave the model, in the order it gave them; none
    /// where the line did not say.
    pub program_tools: Option<Vec<String>>,
    /// The names of the agents the program says, in its first line, it gave the model, in the order it gave them;
    /// none where the line did not say.
    pub program_agents: Option<Vec<String>>,
    /// What the latest rate limit event that said anything of the usage of the account the program runs under said.
    usage: Option<Said>,
    /// How many rate limit events said anything of the usage.
    usages: usize,
    /// The session the program says it runs in, from the first line that names it.
    session: Option<String>,
}

/// What a result the program printed says of cost, as the repair of recorded costs reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Told {
    /// What the program spent since the result before it, in millionths of a dollar; none where it says none.
    pub since: Option<u64>,
    /// What the whole session had cost by this result, in millionths of a dollar; none where it says none.
    pub total: Option<u64>,
}

/// A call to the model, as the stream's lines tell it.
#[derive(Debug)]
struct Call {
    id: String,
    model: String,
    tokens: cost::Tokens,
    /// How many results the stream held when the call began: the call is spent in the span the result of this number
    /// ends, counting from none, and a call after the last result belongs to none.
    span: usize,
    /// Whether the call's `message_delta` gave its tokens in the end, which a line before or after it does not change.
    ended: bool,
}

/// What the program says it spent since the result before this one, in millionths of a dollar: what each model's usage
/// cost, summed; none where the result gives none of it.
fn since_last(result: &Value) -> Option<u64> {
    let models = result.get("model_usage_since_last_result")?.as_object()?;
    let usd = (models.values())
        .map(|usage| usage["costUSD"].as_f64())
        .sum::<Option<f64>>()?;
    Some((usd.max(0.0) * 1_000_000.0).round() as u64)
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

/// The names an array of the program's first line holds, in order; none where it is not an array of names.
fn names(list: &Value) -> Option<Vec<String>> {
    (list.as_array()?.iter())
        .map(|name| name.as_str().map(str::to_string))
        .collect()
}

/// How much of what a tool was used on is kept to say what the agent was doing, in characters.
const ON: usize = 160;

impl Stream {
    /// Takes one line of the stream, without its newline, and says whether the line is kept with the round: every
    /// line is but a partial message, which is kept only where it rebuilds an attempt at a return or starts a call of
    /// the session's own agent, so that what is kept rebuilds the attempts and the calls again, as a try its engine did
    /// not see end is read, and holds each token of the model's once.
    pub fn line(&mut self, line: &[u8]) -> bool {
        self.lines += 1;
        let Ok(text) = std::str::from_utf8(line) else {
            return true;
        };
        // The first partial message of each call tells its prompt, whether or not it rebuilds an attempt at a return.
        let starts = text.starts_with(PARTIAL) && text.contains(r#""message_start""#);
        let ends = text.starts_with(PARTIAL) && text.contains(r#""message_delta""#);
        if starts && self.result.is_some() {
            self.next_message();
        }
        if text.starts_with(PARTIAL) && !starts && !ends && !self.attempts.wants(text) {
            return false;
        }
        let Ok(v) = serde_json::from_str::<Value>(text) else {
            return true;
        };
        let own_message = matches!(v["type"].as_str(), Some("assistant" | "user")) && v["parent_tool_use_id"].is_null();
        if own_message && self.result.is_some() {
            self.next_message();
        }
        if self.session.is_none() {
            self.session = v["session_id"].as_str().map(str::to_string);
        }
        self.said.line(&v);
        self.called(&v);
        if let Some(said) = crate::account::said(&v) {
            self.usage = Some(said);
            self.usages += 1;
        }
        match v["type"].as_str() {
            Some("stream_event") => {
                let own = starts && v["parent_tool_use_id"].is_null();
                if own {
                    let usage = &v["event"]["message"]["usage"];
                    let tokens = |key: &str| usage[key].as_u64().unwrap_or(0);
                    let (fresh, read) = (tokens("input_tokens"), tokens("cache_read_input_tokens"));
                    self.own.take(fresh, read, tokens("cache_creation_input_tokens"));
                }
                match v["event"]["type"].as_str() {
                    Some("message_start") => self.started(&v["event"]["message"]),
                    Some("message_delta") => self.ended(&v),
                    _ => {}
                }
                // What a call cost is read from its start and its end, whoever made the call, so that a try the engine
                // did not see end is priced again from what it kept as it was.
                return self.attempts.event(&v) || starts || ends;
            }
            Some("result") => self.result = Some(v),
            Some("user") if v["parent_tool_use_id"].is_null() => self.refusals.extend(refusals(&v)),
            Some("assistant") if v["parent_tool_use_id"].is_null() => self.used(&v),
            Some("system") if v["subtype"] == "init" => {
                self.program = v["claude_code_version"].as_str().map(str::to_string);
                self.program_tools = names(&v["tools"]);
                self.program_agents = names(&v["agents"]);
            }
            _ => {}
        }
        true
    }

    /// A message after the one the result ended begins: that result is kept with the earlier ones, and what is read of
    /// a message's return, its attempts, what the program refused and what it said of the service's errors, begins
    /// again, so that the return is read from the last message alone. A stream of one message never comes here.
    fn next_message(&mut self) {
        if let Some(result) = self.result.take() {
            self.earlier.push(result);
        }
        self.attempts = Attempts::default();
        self.refusals.clear();
        self.said = failure::Said::default();
    }

    /// How many messages the program ended with a result.
    pub fn results(&self) -> usize {
        self.earlier.len() + usize::from(self.result.is_some())
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
        self.lines == 0
    }

    /// Whether the command line refused to begin the session, or to go on in it, the way the round tried, which is
    /// then tried the other way: it said so on its standard error and printed nothing, or printed only a result that
    /// says so ([`Stream::refusal`]).
    pub fn refused(&self, begin: Begin, stderr: &str) -> bool {
        (self.is_empty() && begin.refused(stderr)) || self.refusal() == Some(begin)
    }

    /// The way the command line refused to begin the session or to go on in it, where all the stream holds is a result
    /// that is an error saying so; none where it holds anything else, as where the program reached the model.
    pub fn refusal(&self) -> Option<Begin> {
        let result = self.result.as_ref().filter(|_| self.lines == 1)?;
        let result = Result::deserialize(result).ok().filter(|result| result.is_error)?;
        let said = result.said();
        [Begin::New, Begin::Resume]
            .into_iter()
            .find(|begin| begin.refused(&said))
    }

    /// Whether the stream gave a result.
    pub fn has_result(&self) -> bool {
        self.result.is_some()
    }

    /// What the latest rate limit event that said anything of the usage of the account the program runs under said;
    /// none where none did.
    pub fn usage(&self) -> Option<&Said> {
        self.usage.as_ref()
    }

    /// How many rate limit events said anything of the usage: a count that grows as each comes, by which each is told
    /// once while the stream comes.
    pub fn usages(&self) -> usize {
        self.usages
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
        match self.calls.iter_mut().find(|call| call.id == id) {
            // The end of a call has its final tokens, which nothing after it changes; the model it does not say.
            Some(call) if call.ended => call.model = Self::model_of(&call.model, model),
            Some(call) => (call.model, call.tokens) = (model, tokens),
            None => self.call(id, model, tokens, false),
        }
    }

    /// Takes a call whose message begins: its prompt, which its own end does not repeat apart from its tokens.
    fn started(&mut self, message: &Value) {
        let Some(id) = message["id"].as_str() else {
            return;
        };
        let model = message["model"].as_str().unwrap_or_default().to_string();
        if let Some(call) = self.calls.iter_mut().find(|call| call.id == id) {
            call.model = Self::model_of(&call.model, model);
            return;
        }
        self.call(id, model, cost::Tokens::of(&message["usage"]), false);
    }

    /// The model of a call: the one it has, or, where its end came before any line that names it, the one a later line
    /// names.
    fn model_of(has: &str, named: String) -> String {
        if has.is_empty() { named } else { has.to_string() }
    }

    /// Takes the end of a call, which names it as `api_message_id`: the tokens it wrote, which its message's start and
    /// its whole message give as the few it began with. A call whose start was not seen is taken as the end gives it,
    /// with no model until a line of its own names it, and priced as the dearest while it has none.
    fn ended(&mut self, line: &Value) {
        let (Some(id), usage) = (line["api_message_id"].as_str(), &line["event"]["usage"]) else {
            return;
        };
        let Some(output) = usage["output_tokens"].as_u64() else {
            return;
        };
        match self.calls.iter_mut().find(|call| call.id == id) {
            Some(call) => (call.tokens.output, call.ended) = (output, true),
            None => self.call(id, String::new(), cost::Tokens::of(usage), true),
        }
    }

    fn call(&mut self, id: &str, model: String, tokens: cost::Tokens, ended: bool) {
        let span = self.earlier.len() + usize::from(self.result.is_some());
        self.calls.push(Call {
            id: id.to_string(),
            model,
            tokens,
            span,
            ended,
        });
    }

    /// What the stream's calls came to: their tokens, the context of the last, and what they cost as [`Stream::cost`]
    /// says; none where it made none.
    pub fn took(&self) -> Option<Spent> {
        let last = self.calls.last()?;
        let mut spent = Spent {
            usd_micros: self.cost().0,
            context_tokens: Some(last.tokens.prompt()),
            cache_written_tokens: Some(0),
            ..Spent::default()
        };
        for call in &self.calls {
            spent.uncached_tokens += call.tokens.input;
            spent.cache_written_tokens = spent
                .cache_written_tokens
                .map(|w| w + call.tokens.written_1h + call.tokens.written_5m);
            spent.cached_tokens += call.tokens.read;
            spent.output_tokens += call.tokens.output;
        }
        Some(spent)
    }

    /// What the stream cost, in millionths of a dollar, and whether any of it is priced from its calls and not told by
    /// the program: each result the program printed gives what it spent since the one before it, and where a result
    /// gives none, or the calls after the last result have none yet, those calls cost what their tokens do at their
    /// models' prices. The one place a stream's cost is worked out.
    pub fn cost(&self) -> (u64, bool) {
        // What the calls of the spans that `wanted` names cost at their models' prices, and whether there are any: a
        // result that says nothing of cost and had no call has nothing priced, and is no estimate.
        let priced = |wanted: &dyn Fn(usize) -> bool| {
            let calls = self.calls.iter().filter(|call| wanted(call.span));
            (
                calls
                    .clone()
                    .map(|call| cost::of_call(&call.model, &call.tokens))
                    .sum::<u64>(),
                calls.count() > 0,
            )
        };
        let (mut usd, mut estimated) = (0, false);
        let mut results = 0;
        for (span, result) in self.earlier.iter().chain(&self.result).enumerate() {
            results = span + 1;
            match since_last(result) {
                Some(told) => usd += told,
                None => {
                    let (spent, any) = priced(&|of| of == span);
                    (usd, estimated) = (usd + spent, estimated || any);
                }
            }
        }
        let (spent, any) = priced(&|of| of >= results);
        (usd + spent, estimated || any)
    }

    /// What each result the stream holds says of cost, in the order the program printed them.
    pub fn told(&self) -> Vec<Told> {
        let micros = |usd: f64| (usd.max(0.0) * 1_000_000.0).round() as u64;
        (self.earlier.iter().chain(&self.result))
            .map(|result| Told {
                since: since_last(result),
                total: result["total_cost_usd"].as_f64().map(micros),
            })
            .collect()
    }

    /// The session the program says it runs in, where a line of the stream names it.
    pub fn session(&self) -> Option<&str> {
        self.session.as_deref()
    }

    /// Whether any of what the stream cost is priced from its calls ([`Stream::cost`]).
    pub fn estimated(&self) -> bool {
        self.cost().1
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

    /// What the round took, as the stream says: what its last result says where it printed one that can be read, else
    /// what its calls came to ([`Stream::took`]); none where it made none. For where the words of why there is no
    /// return are of no use.
    pub fn what_it_took(&self) -> Option<Spent> {
        let result = (self.result.clone()).and_then(|result| serde_json::from_value::<Result>(result).ok());
        match result {
            Some(result) => Some(spent_of_result(&result, &self.earlier, self.cost().0)),
            None => self.took(),
        }
    }

    /// Reads the stream: the return the command line took, or, where it gave up on the return or the agent stopped
    /// trying and the result holds none, its last attempt at it, rebuilt; or says why there is no return to take, with
    /// what the round took where the result could be read. What it cost is what [`Stream::cost`] says, whatever the
    /// session cost before: the program counts a result from the result before it. What it says of why there is none
    /// is in these words.
    pub fn read(self, words: &ProgramWords) -> std::result::Result<Read, Box<NoResult>> {
        let (usd_micros, _) = self.cost();
        let took = self.took();
        let fields = self.said;
        let Some(result) = self.result else {
            return Err(Box::new(NoResult {
                why: words.no_stream_result.clone(),
                report: String::new(),
                spent: took,
                said: fields,
            }));
        };
        let result: Result = match serde_json::from_value(result) {
            Ok(result) => result,
            Err(e) => {
                let report = e.to_string();
                return Err(Box::new(NoResult {
                    why: fill(&words.unreadable_result, &[("why", &report)]),
                    report,
                    spent: took,
                    said: fields,
                }));
            }
        };
        let spent = spent_of_result(&result, &self.earlier, usd_micros);
        if !result.is_error {
            // The agent may stop trying after the command line refused its attempts, and end its turn: its last
            // attempt is read as one the command line gave up on is.
            let structured = match result.structured_output {
                Some(structured) => Ok(structured),
                None => last_attempt(
                    self.attempts,
                    &self.refusals,
                    (words.no_return.clone(), String::new()),
                    words,
                ),
            };
            return match structured {
                Ok(structured) => Ok(Read { structured, spent }),
                Err((why, report)) => Err(Box::new(NoResult {
                    why,
                    report,
                    spent: Some(spent),
                    said: fields,
                })),
            };
        }
        // What it says goes back to the agent where the return is sent back: its quote of the attempt is left out.
        let said = result.said();
        let kind = result.subtype.as_deref().filter(|subtype| *subtype != "success");
        let why = match kind {
            Some(subtype) => fill(&words.error_kind, &[("subtype", subtype), ("said", &said)]),
            None => fill(&words.error, &[("said", &said)]),
        };
        // What the command line reported, the words that tell the kind of the failure: its kind and what it said.
        let report = kind.into_iter().chain([said.as_str()]).collect::<Vec<_>>().join(": ");
        // The command line gave up on the return, having refused every attempt at it: the last is read as any return
        // is, by the engine's own reading, which sends it back where it cannot be read.
        let (why, report) = if failure::gave_up(&report, &fields) {
            match last_attempt(self.attempts, &self.refusals, (why, report), words) {
                Ok(structured) => return Ok(Read { structured, spent }),
                Err(unread) => unread,
            }
        } else {
            (why, report)
        };
        Err(Box::new(NoResult {
            why,
            report,
            spent: Some(spent),
            said: fields,
        }))
    }
}

/// The last attempt at a return in the stream, read as any return is, by the engine's own reading, which sends it
/// back where it cannot be read; or, as the reason there is none and what the command line reported, the first of the
/// pair where the stream holds no attempt, and that with what is wrong with the attempt where it is no JSON, with
/// what the command line said of it where the reason does not already hold that.
fn last_attempt(
    attempts: Attempts,
    refusals: &[(String, String)],
    (why, report): (String, String),
    words: &ProgramWords,
) -> std::result::Result<Value, (String, String)> {
    let Some((id, raw)) = attempts.last() else {
        return Err((why, report));
    };
    serde_json::from_str::<Value>(&raw).map_err(|e| {
        let refused = (refusals.iter())
            .find(|(refused, _)| *refused == id)
            .map(|(_, said)| without_quote(said))
            .filter(|said| !why.contains(said.as_str()));
        let told = refused
            .as_ref()
            .map_or_else(String::new, |said| fill(&words.refused, &[("said", said)]));
        let error = e.to_string();
        // The reason before it is a whole sentence, whatever the command line ended what it said with.
        let reason = flow::agents::sentence(&why);
        let why = fill(
            &words.last_attempt,
            &[("reason", &reason), ("error", &error), ("refused", &told)],
        );
        let report = [report]
            .into_iter()
            .chain(refused)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        (why, report.join("; "))
    })
}

/// What a whole stream says the round took, as [`Stream::what_it_took`] does.
pub fn took(stream: &[u8]) -> Option<Spent> {
    let mut read = Stream::default();
    for line in stream.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
        read.line(line);
    }
    read.what_it_took()
}

/// Reads a whole stream, line by line, as [`Stream::read`] does.
pub fn read(stream: &[u8], words: &ProgramWords) -> std::result::Result<Read, Box<NoResult>> {
    let mut read = Stream::default();
    for line in stream.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
        read.line(line);
    }
    read.read(words)
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

/// What a fork's command line is given on its standard input: a first message, whose answer is not read, then the
/// question, each as [`stdin`] says. The first is the same for every fork of a round, so that the session's history
/// and the first message are written to the prompt cache once, by the first fork, and the forks after it read them.
pub fn fork_stdin(first: &str, question: &str) -> Vec<u8> {
    let mut bytes = stdin(first);
    bytes.extend(stdin(question));
    bytes
}

/// What the call whose result this is took, at the cost the stream worked out.
/// What a round took as its last result says, with the tokens of the messages answered before it, which its own result
/// leaves out: the cost counts them, each result giving what it spent since the one before.
fn spent_of_result(result: &Result, earlier: &[Value], usd_micros: u64) -> Spent {
    let mut spent = spent(result, usd_micros);
    for earlier in earlier {
        let usage = &earlier["usage"];
        let tokens = |key: &str| usage[key].as_u64().unwrap_or(0);
        spent.uncached_tokens += tokens("input_tokens") + tokens("cache_creation_input_tokens");
        spent.cache_written_tokens = spent
            .cache_written_tokens
            .map(|w| w + tokens("cache_creation_input_tokens"));
        spent.cached_tokens += tokens("cache_read_input_tokens");
        spent.output_tokens += tokens("output_tokens");
    }
    spent
}

fn spent(result: &Result, usd_micros: u64) -> Spent {
    let usage = &result.usage;
    Spent {
        usd_micros,
        uncached_tokens: usage.input_tokens + usage.cache_creation_input_tokens,
        cache_written_tokens: Some(usage.cache_creation_input_tokens),
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
            highest_level: None,
            slots: 2,
            args: vec!["--strict-mcp-config".to_string()],
            env: Vec::new(),
            setting_sources: "user".to_string(),
            subagents: Default::default(),
            work_budget_usd: None,
            let_on_allowance_usd: 20.0,
        }
    }

    /// The words beside the flows of this repository, which the sentences of a result are in.
    fn program() -> ProgramWords {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
        let beside = flow::agents::read(&root).expect("what is written beside the flows is read");
        beside.cost.program
    }

    fn words() -> Words {
        Words {
            waiting: PathBuf::from("rounds/1/2-3.words.json"),
            given: PathBuf::from("rounds/1/2-3.given"),
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
            "errors": [said], "total_cost_usd": 0.4, "model_usage_since_last_result": { "m": { "costUSD": 0.4 } },
            "usage": { "input_tokens": 10, "output_tokens": 300 } })
        .to_string()
    }

    /// A stream the command line printed for a call that gave up on the return, what named the machine and the account
    /// replaced: twice the model wrote the list of asks as a string, which its schema refuses.
    const GAVE_UP: &str = include_str!("../tests/fixtures/gave-up.jsonl");

    #[test]
    fn the_stream_reads_to_its_return_and_what_it_took() {
        let read = read(STREAM.as_bytes(), &program()).expect("the fixture reads");
        assert_eq!(
            read.spent,
            Spent {
                usd_micros: 2614,
                uncached_tokens: 9094,
                cache_written_tokens: Some(9090),
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
        // Its first line names the tools it gave the model, in order, and no agents.
        let given: Vec<&str> = (stream.program_tools.iter().flatten()).map(String::as_str).collect();
        assert_eq!(given, ["Bash", "Read", STRUCTURED_OUTPUT]);
        assert_eq!(stream.program_agents, None);
        let tools: Vec<&str> = stream.tools.iter().map(|(_, tool, _)| tool.as_str()).collect();
        assert_eq!(tools, ["Bash", STRUCTURED_OUTPUT]);
        // The result says the cache writes apart from the fresh tokens and the reads, the thinking and the turns.
        assert_eq!(stream.printed(), Some((4, 63_348, 9_090, Some(153), 3)));
    }

    // A list of the first line that is not a list of names is no list: it says nothing, not a shorter list.
    #[test]
    fn a_list_of_the_first_line_that_holds_other_than_names_says_none() {
        assert_eq!(
            names(&json!(["Bash", "Read"])),
            Some(vec!["Bash".to_string(), "Read".to_string()])
        );
        assert_eq!(names(&json!([])), Some(Vec::new()));
        assert_eq!(names(&json!(["Bash", 7])), None);
        assert_eq!(names(&json!("Bash")), None);
        assert_eq!(names(&Value::Null), None);
    }

    #[test]
    fn a_partial_message_is_kept_only_where_it_rebuilds_an_attempt_at_a_return_or_starts_an_agents_call() {
        let mut stream = Stream::default();
        let kept: Vec<&str> = (STREAM.lines()).filter(|line| stream.line(line.as_bytes())).collect();
        let partial = kept.iter().filter(|line| line.starts_with(PARTIAL)).count();
        // The pieces of the thinking and of the other tool's input are passed over; the messages' starts, which say each
        // call's prompt, their ends, which say what each call wrote, the return's start, its eleven pieces and its stop
        // are kept, and every other line.
        assert_eq!(partial, 17, "{kept:#?}");
        assert_eq!(kept.len() - partial, 14);
        // What is kept rebuilds the same attempts, and only the attempts, and the same calls of the session's own
        // agent: a try its engine did not see end is read from it.
        let mut again = Stream::default();
        for line in &kept {
            assert!(again.line(line.as_bytes()), "{line}");
        }
        assert_eq!(again.own, stream.own);
        assert_eq!(again.took(), stream.took());
        assert_eq!(again.cost(), stream.cost());
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
        let read = read(GAVE_UP.as_bytes(), &program()).expect("the last attempt is taken");
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
        let read = read(lines.join("\n").as_bytes(), &program()).expect("the last attempt is taken");
        assert_eq!(read.structured, serde_json::from_str::<Value>(last).unwrap());
        assert_eq!(read.spent.usd_micros, 400_000, "what the call cost");
        assert_eq!(read.spent.output_tokens, 300);

        // The first attempt alone, rebuilt whole across the pieces that cut its characters.
        let mut only = vec![init];
        only.extend(attempt("so-1", &pieces, Some(first_refused)));
        only.push(gave_up(first_refused));
        let read = super::read(only.join("\n").as_bytes(), &program()).expect("it is taken");
        assert_eq!(read.structured, serde_json::from_str::<Value>(&first).unwrap());
    }

    #[test]
    fn the_last_attempt_is_read_where_the_agent_stopped_trying_and_the_result_holds_no_return() {
        // Work 21's run 35: the outcome sent as a string, refused each time, then the agent ended its turn in prose.
        let attempted = json!({ "outcome": json!({ "verdict": "pass" }).to_string() }).to_string();
        let refused = "Output does not match required schema: /outcome: must be object";
        let init = json!({ "type": "system", "subtype": "init", "session_id": "s" }).to_string();
        let ended = json!({ "type": "result", "subtype": "success", "is_error": false, "result": "My review.",
            "total_cost_usd": 0.4, "model_usage_since_last_result": { "m": { "costUSD": 0.4 } },
            "usage": { "input_tokens": 10, "output_tokens": 300 } })
        .to_string();
        let mut lines = vec![init];
        for id in ["so-1", "so-2", "so-3"] {
            lines.extend(attempt(id, &[&attempted], Some(refused)));
        }
        lines.push(ended.clone());
        let read = read(lines.join("\n").as_bytes(), &program()).expect("the last attempt is taken");
        assert_eq!(read.structured, serde_json::from_str::<Value>(&attempted).unwrap());
        assert_eq!(read.spent.usd_micros, 400_000, "what the call cost");

        // No attempt in the stream: no return, as before; an attempt that is no JSON is none either, said as where the
        // command line gave up, and is no kind of failure the words tell, so the work still pauses.
        let none = program().no_return;
        let no = super::read(ended.as_bytes(), &program()).expect_err("no return");
        assert_eq!(no.why, none);
        let mut cut = attempt("so-1", &[r#"{"outcome": "#], Some(refused));
        cut.push(ended);
        let no = super::read(cut.join("\n").as_bytes(), &program()).expect_err("no return");
        assert!(
            no.why
                .starts_with(&format!("{none} Its last attempt at the return is no JSON (")),
            "{}",
            no.why
        );
        assert!(
            no.why.ends_with(&format!(" The command line said: {refused}")),
            "{}",
            no.why
        );
        assert_eq!(failure::of(&no.report, &no.said, no.spent.as_ref()), Failure::Other);
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
        let no = read(lines.join("\n").as_bytes(), &program()).expect_err("no JSON is no return");
        assert!(
            no.why.contains("Its last attempt at the return is no JSON"),
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
            failure::of(&no.report, &no.said, no.spent.as_ref()),
            Failure::Unread,
            "it is sent back"
        );
        assert_eq!(no.spent.map(|s| s.usd_micros), Some(400_000));
        // Given up on with no attempt in the stream, the result's own words say why, without the quote.
        let no = read(gave_up(&said).as_bytes(), &program()).expect_err("nothing to take");
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
        let no = read(lines.join("\n").as_bytes(), &program()).expect_err("no JSON is no return");
        assert!(
            no.why.ends_with(
                "The command line said: InputValidationError: StructuredOutput was called with \
                              input that could not be parsed as JSON.\nYou sent (first 200 of 92 bytes)"
            ),
            "{}",
            no.why
        );
    }

    // What the command line reported tells the kind of a failure, and the words the engine says it in tell none: the
    // sentences changed to hold the words of other kinds change no kind, as the files' words may be changed.
    #[test]
    fn the_kind_of_a_failure_is_told_by_what_the_command_line_reported_and_not_by_the_words_that_say_it() {
        let loud = |text: &str| format!("You have hit your usage limit and are not logged in. {text}");
        let ordinary = program();
        let changed = ProgramWords {
            no_stream_result: loud(&ordinary.no_stream_result),
            unreadable_result: loud(&ordinary.unreadable_result),
            no_return: loud(&ordinary.no_return),
            error_kind: loud(&ordinary.error_kind),
            error: loud(&ordinary.error),
            last_attempt: loud(&ordinary.last_attempt),
            ..ordinary.clone()
        };
        let ended =
            r#"{"type":"result","subtype":"success","is_error":false,"result":"My review.","total_cost_usd":0.4}"#;
        let error = r#"{"type":"result","is_error":true,"result":"There's an issue with the selected model."}"#;
        let giving_up = r#"{"type":"result","subtype":"error_max_structured_output_retries","is_error":true,"errors":["Failed to provide valid structured output after 3 attempts"]}"#;
        let streams = [
            (ended, Failure::Other),
            (error, Failure::Other),
            (giving_up, Failure::Unread),
        ];
        for (stream, kind) in streams {
            for words in [&ordinary, &changed] {
                let no = super::read(stream.as_bytes(), words).expect_err("no return");
                assert_eq!(
                    failure::of(&no.report, &no.said, no.spent.as_ref()),
                    kind,
                    "{stream}: {}",
                    no.why
                );
            }
        }
        // The sentence is the words', and is the only place they come in.
        let no = super::read(ended.as_bytes(), &changed).expect_err("no return");
        assert!(no.why.starts_with("You have hit your usage limit"), "{}", no.why);
        assert_eq!(no.report, "");
    }

    #[test]
    fn an_attempt_of_a_subagent_or_of_another_failure_is_not_taken() {
        let mut lines = attempt("so-1", &[r#"{"hands_in": [], "asks": []}"#], None);
        let error = r#"{"type":"result","subtype":"success","is_error":true,"result":"API Error: 529 Overloaded"}"#;
        lines.push(error.to_string());
        assert!(
            read(lines.join("\n").as_bytes(), &program()).is_err(),
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
        let no = stream.read(&program()).expect_err("no result");
        let spent = no.spent.expect("its calls are priced");
        assert_eq!(
            (
                spent.uncached_tokens,
                spent.cache_written_tokens,
                spent.cached_tokens,
                spent.output_tokens
            ),
            (200, Some(0), 2000, 70)
        );
        // Opus 5.5: 200 × 4 + 2000 × 0.2 + 70 × 20 millionths.
        assert_eq!(spent.usd_micros, 800 + 400 + 1400);
        // An error the service gave is no answer.
        let mut refused = Stream::default();
        let error = json!({ "type": "assistant", "parent_tool_use_id": null, "error": "overloaded", "message": {} });
        refused.line(error.to_string().as_bytes());
        assert!(!refused.answered());
    }

    // The tokens a round's calls wrote to the cache are told from those sent fresh, as a result's are: the calls are
    // priced in the cost of a round that crashed before its result.
    #[test]
    fn a_stream_cut_off_before_its_result_says_how_many_tokens_its_calls_wrote_to_the_cache() {
        let call = |id: &str, written: u64| {
            json!({ "type": "assistant", "parent_tool_use_id": Value::Null, "message": { "id": id,
                "model": "claude-opus-5-5", "usage": { "input_tokens": 100, "cache_creation_input_tokens": written,
                "cache_creation": { "ephemeral_1h_input_tokens": written, "ephemeral_5m_input_tokens": 0 },
                "cache_read_input_tokens": 1000, "output_tokens": 10 } } })
            .to_string()
        };
        let mut stream = Stream::default();
        for line in [call("msg_1", 300), call("msg_2", 40)] {
            stream.line(line.as_bytes());
        }
        let no = stream.read(&program()).expect_err("no result");
        let spent = no.spent.expect("its calls are priced");
        assert_eq!(
            (spent.uncached_tokens, spent.cache_written_tokens),
            ((100 + 300) + (100 + 40), Some(300 + 40))
        );
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
        let no = read(stream.as_bytes(), &program()).expect_err("a refusal is not taken");
        assert_eq!(no.said.kind(), Some(Failure::UsageLimit));
        assert_eq!(no.said.resets, Some(1_794_380_400_000));
        assert_eq!(
            failure::of(&no.report, &no.said, no.spent.as_ref()),
            Failure::UsageLimit
        );
        // A stream cut off before its result says what it said all the same.
        let cut = read(refused.to_string().as_bytes(), &program()).expect_err("no result");
        assert_eq!((cut.spent, cut.said.kind()), (None, Some(Failure::UsageLimit)));
    }

    #[test]
    fn a_round_ended_with_the_context_of_its_last_call() {
        let two = r#"{"type":"result","is_error":false,"total_cost_usd":0.1,"structured_output":{},"usage":{"input_tokens":3,"output_tokens":9,"iterations":[{"input_tokens":1,"cache_read_input_tokens":100,"cache_creation_input_tokens":20},{"input_tokens":2,"cache_read_input_tokens":120,"cache_creation_input_tokens":30}]}}"#;
        let read = super::read(two.as_bytes(), &program()).expect("it reads");
        assert_eq!(read.spent.context_tokens, Some(152), "the last call's prompt");
        let none = r#"{"type":"result","is_error":false,"total_cost_usd":0.1,"structured_output":{}}"#;
        assert_eq!(
            super::read(none.as_bytes(), &program()).unwrap().spent.context_tokens,
            None
        );
    }

    #[test]
    fn an_error_and_what_is_no_result_are_not_taken() {
        let error = r#"{"type":"result","is_error":true,"result":"There's an issue with the selected model."}"#;
        let refused = read(error.as_bytes(), &program()).expect_err("an error is not taken");
        assert!(refused.why.contains("issue with the selected model"), "{}", refused.why);
        let nothing = read(b"Error: something", &program()).expect_err("what is no JSON is not taken");
        assert_eq!(nothing.spent, None, "what the round took is not known");
        // Before its first call the round took nothing known; once a call began it took that call's prompt and what it had
        // written, priced (Haiku 5.5: 2 fresh, 27,199 read and 8,950 written for an hour, with 17 tokens out).
        let calls = |lines: usize| STREAM.lines().take(lines).collect::<Vec<_>>().join("\n");
        let cut = read(calls(2).as_bytes(), &program()).expect_err("a stream with no result is not taken");
        assert_eq!(cut.spent, None);
        let begun = read(calls(5).as_bytes(), &program()).expect_err("a stream with no result is not taken");
        assert_eq!(begun.spent.map(|s| s.usd_micros), Some(2071));
        let bare = r#"{"type":"result","is_error":false,"result":"hello","total_cost_usd":0.1}"#;
        assert!(
            read(bare.as_bytes(), &program()).is_err(),
            "a result with no structured return"
        );
    }

    #[test]
    fn a_result_that_is_no_return_still_says_what_its_call_took() {
        let error = r#"{"type":"result","is_error":true,"result":"Out of structured output retries.","total_cost_usd":0.8,"model_usage_since_last_result":{"m":{"costUSD":0.3}},"usage":{"input_tokens":10,"cache_creation_input_tokens":5,"cache_read_input_tokens":100,"output_tokens":20}}"#;
        let spent = Spent {
            usd_micros: 300_000,
            uncached_tokens: 15,
            cache_written_tokens: Some(5),
            cached_tokens: 100,
            output_tokens: 20,
            ..Default::default()
        };
        let refused = read(error.as_bytes(), &program()).expect_err("an error is not taken");
        assert_eq!(refused.spent, Some(spent));
        let bare = r#"{"type":"result","is_error":false,"total_cost_usd":0.8,"model_usage_since_last_result":{"m":{"costUSD":0.8}}}"#;
        let refused = read(bare.as_bytes(), &program()).expect_err("no structured return is not taken");
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
    fn a_forks_input_is_its_first_message_and_then_its_question() {
        let bytes = fork_stdin("# A question follows", "Which words can be read two ways?");
        let lines: Vec<Value> = (bytes.split(|b| *b == b'\n'))
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice(line).expect("JSON"))
            .collect();
        assert_eq!(lines.len(), 2, "two user messages");
        let said = |line: &Value| line["message"]["content"][0]["text"].as_str().map(str::to_string);
        assert_eq!(said(&lines[0]).as_deref(), Some("# A question follows"));
        assert_eq!(said(&lines[1]).as_deref(), Some("Which words can be read two ways?"));
        assert!(
            lines
                .iter()
                .all(|line| line["type"] == "user" && line["message"]["role"] == "user")
        );
    }

    /// The lines of a call given two messages: the program's first line, each message's call and its result. The first
    /// message's return is an attempt at a return, and the second's is `second`'s: a message of the agent's own that
    /// makes none, or an attempt.
    fn two_messages(second: Option<&str>) -> Vec<String> {
        let first = json!({ "type": "result", "subtype": "success", "is_error": false, "structured_output": { "ok": true },
            "total_cost_usd": 0.01, "model_usage_since_last_result": { "m": { "costUSD": 0.01 } },
            "usage": { "input_tokens": 2, "cache_creation_input_tokens": 100, "cache_read_input_tokens": 50, "output_tokens": 5 } })
        .to_string();
        let mut asked = json!({ "type": "result", "subtype": "success", "is_error": false, "total_cost_usd": 0.03,
            "model_usage_since_last_result": { "m": { "costUSD": 0.02 } },
            "usage": { "input_tokens": 4, "cache_creation_input_tokens": 10, "cache_read_input_tokens": 200, "output_tokens": 8 } });
        let mut lines = vec![json!({ "type": "system", "subtype": "init", "session_id": "s" }).to_string()];
        lines.extend(attempt("so-1", &[r#"{"ok": true}"#], None));
        lines.push(first);
        // The program begins the next message with a partial message, as it prints one, its type first.
        lines.push(
            r#"{"type":"stream_event","event":{"type":"message_start","message":{}},"parent_tool_use_id":null}"#
                .to_string(),
        );
        if let Some(piece) = second {
            lines.extend(attempt("so-2", &[piece], None));
            asked["structured_output"] = serde_json::from_str(piece).unwrap();
        }
        lines.push(asked.to_string());
        lines
    }

    #[test]
    fn the_spend_of_a_call_with_two_results_is_the_sum_of_their_tokens_and_their_costs() {
        let lines = two_messages(Some(r#"{"answer": "yes"}"#));
        let mut stream = Stream::default();
        for line in &lines {
            stream.line(line.as_bytes());
        }
        assert_eq!(stream.results(), 2);
        let read = stream.read(&program()).expect("the question's return is read");
        assert_eq!(read.structured, json!({ "answer": "yes" }));
        assert_eq!(
            read.spent,
            Spent {
                usd_micros: 30_000,
                uncached_tokens: (2 + 100) + (4 + 10),
                cache_written_tokens: Some(100 + 10),
                cached_tokens: 50 + 200,
                output_tokens: 5 + 8,
                context_tokens: None,
                compactions: 0,
            }
        );
    }

    #[test]
    fn the_results_of_a_stream_are_counted_by_the_messages_the_program_ended() {
        let mut stream = Stream::default();
        assert_eq!(stream.results(), 0);
        let lines = two_messages(Some(r#"{"answer": "yes"}"#));
        let first_result = lines
            .iter()
            .position(|line| line.contains(r#""type":"result""#))
            .expect("a result");
        for line in &lines[..=first_result] {
            stream.line(line.as_bytes());
        }
        assert_eq!(
            stream.results(),
            1,
            "a fork whose program ended one message has no answer"
        );
        for line in &lines[first_result + 1..] {
            stream.line(line.as_bytes());
        }
        assert_eq!(stream.results(), 2);
    }

    #[test]
    fn a_fork_whose_question_made_no_attempt_is_not_read_as_the_opening_messages_return() {
        let lines = two_messages(None);
        let no = read(lines.join("\n").as_bytes(), &program()).expect_err("no return to the question");
        assert_eq!(no.why, program().no_return);
        // The tokens of both messages are what it took.
        assert_eq!(no.spent.map(|s| s.output_tokens), Some(5 + 8));
    }

    /// The lines of the real one-message stream (`../tests/fixtures/stream.jsonl`), the command line's own output.
    fn real_message() -> Vec<String> {
        STREAM.lines().filter(|l| !l.is_empty()).map(str::to_string).collect()
    }

    /// The lines the command line prints for a second message of the same session, as the real stream's but its first
    /// line: it prints no first line again, and the calls and the tools' uses are its own.
    fn second_message() -> Vec<String> {
        (real_message().into_iter().skip(1))
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

    /// The index of the line of the real stream's result.
    fn result_at(lines: &[String]) -> usize {
        (lines.iter())
            .position(|line| line.contains(r#""type":"result""#))
            .expect("the fixture holds a result")
    }

    // Two messages of the real stream read as two results, and the return is the last message's, the tokens and the cost
    // of both counted. The one-message stream is the oracle: a stream of two messages is that stream twice, as the
    // command line prints one result for each message it is given (research/fork-cache-shared-history.md). A few
    // milliseconds, in memory; a stream of the test's own making could not show that the real lines are read so.
    #[test]
    fn two_messages_of_the_real_stream_read_as_two_results_and_the_return_of_the_second() {
        let single = read(real_message().join("\n").as_bytes(), &program()).expect("one message reads");
        let mut lines = real_message();
        lines.extend(second_message());
        let stream = feed(&lines);
        assert_eq!(stream.results(), 2);
        let got = stream.read(&program()).expect("the question's return is read");
        assert_eq!(got.structured, single.structured, "the return of the last message");
        // The tokens and the cost of both messages: each result says what the program spent since the one before.
        assert_eq!(got.spent.uncached_tokens, 2 * single.spent.uncached_tokens);
        assert_eq!(
            got.spent.cache_written_tokens,
            single.spent.cache_written_tokens.map(|written| 2 * written)
        );
        assert_eq!(got.spent.cached_tokens, 2 * single.spent.cached_tokens);
        assert_eq!(got.spent.output_tokens, 2 * single.spent.output_tokens);
        assert_eq!(got.spent.usd_micros, 2 * single.spent.usd_micros);
        assert_eq!(got.spent.context_tokens, single.spent.context_tokens);
    }

    // A first message that ended in an error with no return, as when its request is stopped, does not stop the question
    // being read: it went on, and its return is the one read. A few milliseconds.
    #[test]
    fn a_first_message_the_command_line_ended_in_error_does_not_stop_the_question_being_read() {
        let mut first = real_message();
        let at = result_at(&first);
        let mut result: Value = serde_json::from_str(&first[at]).expect("JSON");
        result["is_error"] = json!(true);
        result["subtype"] = json!("error_during_execution");
        result.as_object_mut().expect("an object").remove("structured_output");
        first[at] = result.to_string();
        first.truncate(at + 1);
        let single = read(real_message().join("\n").as_bytes(), &program()).expect("one message reads");
        first.extend(second_message());
        let stream = feed(&first);
        assert_eq!(stream.results(), 2);
        let got = stream.read(&program()).expect("the question's return is read");
        assert_eq!(got.structured, single.structured);
        assert_eq!(got.spent.output_tokens, 2 * single.spent.output_tokens);
    }

    // A first message the program answers itself, with no call to the model, as a command it knows (measured on the
    // program 2.1.327 with `/cost` and then a question: a `system` line, a message of its own and a result with no
    // tokens and no cost, then the question's own lines and its result, each message ending in a result). The question
    // is still the second result, and the one read. A few milliseconds.
    #[test]
    fn a_first_message_the_program_answered_itself_does_not_stop_the_question_being_read() {
        let single = read(real_message().join("\n").as_bytes(), &program()).expect("one message reads");
        let own = [
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Nothing to show."}]},"parent_tool_use_id":null}"#,
            r#"{"type":"result","subtype":"success","is_error":false,"result":"Nothing to show.","total_cost_usd":0,"usage":{"input_tokens":0,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":0}}"#,
        ];
        let mut lines = vec![real_message()[0].clone()];
        lines.extend(own.iter().map(|line| line.to_string()));
        lines.extend(second_message());
        let stream = feed(&lines);
        assert_eq!(stream.results(), 2);
        let got = stream.read(&program()).expect("the question's return is read");
        assert_eq!(got.structured, single.structured);
        assert_eq!(
            (
                got.spent.uncached_tokens,
                got.spent.cached_tokens,
                got.spent.output_tokens
            ),
            (
                single.spent.uncached_tokens,
                single.spent.cached_tokens,
                single.spent.output_tokens
            )
        );
        assert_eq!(got.spent.usd_micros, single.spent.usd_micros);
    }

    // A program that ended while the question was answered has one result and no return, and says what its first
    // message's calls cost. A few milliseconds.
    #[test]
    fn a_program_that_ended_while_the_question_was_answered_has_one_result_and_no_return() {
        let mut lines = real_message();
        // The second message's lines up to its first call's message: it ends there.
        lines.extend(second_message().into_iter().take(2));
        let stream = feed(&lines);
        assert_eq!(stream.results(), 1, "only the first message ended");
        let no = stream.read(&program()).expect_err("the question has no result");
        assert_eq!(no.why, program().no_stream_result);
        assert!(no.spent.is_some(), "the first message's calls were paid for");
    }

    // The question is read by its own attempts and not by those of the first message, on the real stream's lines: a
    // question that makes no attempt at a return, its result holding none, has no return. A few milliseconds.
    #[test]
    fn the_question_is_read_by_its_own_attempts_and_not_by_those_of_the_first_message() {
        let mut lines = real_message();
        let second = second_message();
        let at = result_at(&second);
        let mut result: Value = serde_json::from_str(&second[at]).expect("JSON");
        result.as_object_mut().expect("an object").remove("structured_output");
        // A message of the question that is only a plain answer: its start, a call without a use of a tool, its result.
        let start = second
            .iter()
            .find(|l| l.starts_with(PARTIAL) && l.contains(r#""message_start""#))
            .expect("a start")
            .clone();
        lines.push(start);
        lines.push(result.to_string());
        let stream = feed(&lines);
        assert_eq!(stream.results(), 2);
        let no = stream.read(&program()).expect_err("no return to the question");
        assert_eq!(no.why, program().no_return);
    }

    #[test]
    fn a_stream_with_one_result_keeps_answered_and_has_result_true_and_reads_as_before() {
        let mut stream = Stream::default();
        for line in STREAM.lines() {
            stream.line(line.as_bytes());
        }
        assert!(stream.has_result());
        assert!(stream.answered());
        assert_eq!(stream.results(), 1);
        let read = stream.read(&program()).expect("the fixture reads");
        assert_eq!(read.structured["hands_in"][0]["content"], "BANANA");
    }

    /// The real stream's lines with what its result says it spent, and its session's total so far, as given: a
    /// session the program went on in, whose total is what it cost over all its rounds.
    fn with_cost(total: f64, since: f64) -> Vec<String> {
        let mut lines = real_message();
        let at = result_at(&lines);
        let mut result: Value = serde_json::from_str(&lines[at]).expect("JSON");
        result["total_cost_usd"] = json!(total);
        result["model_usage_since_last_result"]["claude-haiku-5-5"]["costUSD"] = json!(since);
        lines[at] = result.to_string();
        lines
    }

    /// The real stream's lines without the field of its result that says what it spent.
    fn without_cost() -> Vec<String> {
        let mut lines = real_message();
        let at = result_at(&lines);
        let mut result: Value = serde_json::from_str(&lines[at]).expect("JSON");
        result
            .as_object_mut()
            .expect("an object")
            .remove("model_usage_since_last_result");
        lines[at] = result.to_string();
        lines
    }

    // Work 46's run 10: the round before was cut off by a restart and printed no result, so the program's total, $72.8774,
    // holds it, and this round's own result says it spent $0.1832. The cost is the $0.1832, whatever the session cost
    // before. A few milliseconds, in memory.
    #[test]
    fn a_round_after_an_interrupted_one_costs_only_itself() {
        let got = read(with_cost(72.8774, 0.1832).join("\n").as_bytes(), &program()).expect("it reads");
        assert_eq!(got.spent.usd_micros, 183_200);
        let stream = feed(&with_cost(72.8774, 0.1832));
        assert_eq!(stream.cost(), (183_200, false), "told by the program, none priced");
    }

    // A result that does not say what it spent is priced from its calls and says so, with the cost of the calls at their
    // models' prices as the oracle: the real stream's two calls come to $0.002614 by the program's own count too.
    #[test]
    fn a_result_without_the_field_is_priced_from_its_calls_and_says_so() {
        let stream = feed(&without_cost());
        assert_eq!(stream.cost(), (2614, true));
        assert!(stream.estimated());
        assert_eq!(stream.took().map(|s| s.usd_micros), Some(2614));
        let told = feed(&real_message());
        assert_eq!(told.cost(), (2614, false));
        assert!(!told.estimated());
    }

    // A stream cut off before its result prices each call by the tokens its `message_delta` says it wrote: the messages the
    // program prints as a call goes hold the few it began with (17 and 5 here, against 74 and 251).
    #[test]
    fn a_stream_cut_off_prices_each_call_by_its_message_delta() {
        let cut: Vec<String> = real_message().into_iter().take(result_at(&real_message())).collect();
        let stream = feed(&cut);
        assert_eq!(stream.cost(), (2614, true));
        let without: Vec<String> = cut
            .iter()
            .filter(|l| !l.contains(r#""message_delta""#))
            .cloned()
            .collect();
        // Haiku 5.5: 2 fresh, 27,199 read and 8,950 written for an hour, then 2, 36,149 and 140, with 17 and 5 tokens out.
        assert_eq!(feed(&without).cost(), (2071 + 392, true));
    }

    // The end of a call is final whichever comes first, its message or its `message_delta`: the program prints the message
    // first today, and the end is moved before it.
    #[test]
    fn a_message_delta_is_final_whatever_comes_after() {
        let mut moved: Vec<String> = real_message().into_iter().take(result_at(&real_message())).collect();
        for id in ["msg_011Cgzr4nakbPgB7hCYJ8N5x", "msg_011Cgzr4rZNvF7n4GhWLpvRV"] {
            let at = |moved: &[String], kind: &str| {
                (moved.iter())
                    .position(|l| l.contains(kind) && l.contains(id))
                    .expect("a line of the call")
            };
            let end = moved.remove(at(&moved, r#""message_delta""#));
            let message = at(&moved, r#"{"type":"assistant""#);
            moved.insert(message, end);
            assert!(at(&moved, r#""message_delta""#) < at(&moved, r#"{"type":"assistant""#));
        }
        assert_eq!(feed(&moved).cost(), (2614, true));
    }

    // The lines of a call a subagent made are kept like the agent's own, so that a try the engine did not see end is
    // priced from them again, and the call is priced with the rest. Opus 5.5: 100 fresh, 1,000 read, 50 out.
    #[test]
    fn the_start_and_the_end_of_every_call_are_kept_and_priced_again_as_they_were() {
        // As the program prints them, the type first.
        let start = r#"{"type":"stream_event","parent_tool_use_id":"toolu_1","event":{"type":"message_start","message":{"id":"msg_sub","model":"claude-opus-5-5","usage":{"input_tokens":100,"cache_creation_input_tokens":0,"cache_read_input_tokens":1000,"output_tokens":1}}}}"#;
        let end = r#"{"type":"stream_event","parent_tool_use_id":"toolu_1","api_message_id":"msg_sub","event":{"type":"message_delta","usage":{"input_tokens":100,"output_tokens":50}}}"#;
        let piece = r#"{"type":"stream_event","parent_tool_use_id":"toolu_1","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"x"}}}"#;
        let mut stream = Stream::default();
        assert!(stream.line(start.as_bytes()), "a subagent's start is kept");
        assert!(!stream.line(piece.as_bytes()), "a piece of its text is not");
        assert!(stream.line(end.as_bytes()), "its end is kept");
        // 100 × 4 + 1,000 × 0.2 + 50 × 20 millionths.
        assert_eq!(stream.cost(), (1600, true));
        let mut again = Stream::default();
        for line in [start, end] {
            again.line(line.as_bytes());
        }
        assert_eq!(again.cost(), stream.cost());
    }

    // A fork whose program ended after its first message's result: that message costs what its result says, here $0.50
    // where its calls would be priced at $0.002614, and only the calls after it are priced. The second message's first
    // call, as the real stream's, comes to $0.002099.
    #[test]
    fn a_fork_cut_off_after_its_first_message_uses_the_exact_first_part_and_prices_the_rest() {
        let mut lines = with_cost(0.5, 0.5);
        let second = second_message();
        let end = second
            .iter()
            .position(|l| l.contains(r#""message_delta""#))
            .expect("a call's end");
        lines.extend(second.into_iter().take(end + 1));
        let stream = feed(&lines);
        assert_eq!(stream.results(), 1);
        assert_eq!(stream.cost(), (500_000 + 2099, true));
        let no = stream.read(&program()).expect_err("no result for the question");
        assert_eq!(no.spent.map(|s| s.usd_micros), Some(500_000 + 2099));
    }

    #[test]
    fn the_command_line_begins_a_session_and_goes_on_in_it() {
        let session = Uuid::parse_str("6a1f3c2e-8d4b-4e7a-9c5f-2b8e1d0a7f43").unwrap();
        let permissions = Permissions::of(true, &[], &[], &Default::default());
        let ran = RanOn {
            level: "1".to_string(),
            model: "claude-haiku-5-5".to_string(),
            effort: None,
            directory: None,
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
                &compacted,
                &words(),
                500_000,
                session,
                begin,
            )
        };
        let hooks = json!({ "autoMemoryEnabled": false, "cleanupPeriodDays": 365_000, "hooks": {
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
        // The line's permissions come after the installation's arguments, which they add to, and the session's own
        // arguments last: the context the program compacts it at, which the installation gives, then its id.
        new.extend(permissions.arguments());
        new.extend(["--autocompact".to_string(), "500000".to_string()]);
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
            directory: None,
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
            &compacted,
            &words(),
            500_000,
            session,
            Begin::New,
        );
        let at = args.iter().position(|a| a == "--model").expect("a model");
        assert_eq!(args[at..at + 4], ["--model", "claude-opus-5-5", "--effort", "xhigh"]);
    }

    /// How the refusing hook a fork is given ends where the command line asks it of a use of the tool of this name.
    fn refused(tool: &str) -> Option<i32> {
        use std::io::Write;
        let mut child = std::process::Command::new("sh")
            .args(["-c", &refusal()])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("sh runs");
        let input = json!({ "tool_name": tool, "tool_input": { "text": "x".repeat(200_000) } }).to_string();
        let _ = child.stdin.take().expect("its input").write_all(input.as_bytes());
        child.wait().expect("it ends").code()
    }

    #[test]
    fn a_rounds_last_message_is_the_last_of_its_own_thread_the_program_wrote_to_its_session() {
        // The real stream's last message of its own is the result of the tool that took the return; a subagent's message
        // after it is not the session's own.
        let taken = Some("ebcbcfba-6b99-4d90-b5d8-667683894910");
        assert_eq!(last_message(STREAM.as_bytes()).as_deref(), taken);
        let sub = json!({ "type": "assistant", "uuid": "sub-1", "parent_tool_use_id": "toolu_9", "message": {} });
        let with_sub = format!("{}\n{sub}\n", STREAM.trim_end());
        assert_eq!(last_message(with_sub.as_bytes()).as_deref(), taken);
        let stream = [
            r#"{"type":"system","subtype":"init","uuid":"s-1"}"#,
            r#"{"type":"assistant","uuid":"a-1"}"#,
            r#"{"type":"stream_event","uuid":"e-1"}"#,
            r#"{"type":"user","uuid":"u-1"}"#,
            r#"{"type":"stream_event","uuid":"e-2"}"#,
            r#"{"type":"result","subtype":"success","uuid":"r-1"}"#,
        ]
        .join("\n");
        assert_eq!(last_message(stream.as_bytes()).as_deref(), Some("u-1"));
        assert_eq!(last_message(br#"{"type":"result","uuid":"r-1"}"#), None);
    }

    #[test]
    fn a_fork_is_given_every_argument_of_its_sessions_round_but_a_hook_that_refuses_each_tool_and_the_fork() {
        let ran = RanOn {
            level: "2".to_string(),
            model: "claude-sonnet-5-5".to_string(),
            effort: Some("high".to_string()),
            directory: None,
            account: None,
        };
        let compacted = Compacted {
            marks: PathBuf::from("compacted"),
            again: PathBuf::from("again.md"),
        };
        let permissions = Permissions::of(false, &[], &["cargo".to_string()], &Default::default());
        let session = Uuid::nil();
        let round = arguments(
            &agents(),
            &ran,
            "SYSTEM",
            "SCHEMA",
            &permissions,
            &compacted,
            &words(),
            500_000,
            session,
            Begin::Resume,
        );
        // A fork reads the session from the cache, so it is given the same compaction point as the round.
        let at = round
            .iter()
            .position(|a| a == "--autocompact")
            .expect("a compaction point");
        assert_eq!(round[at + 1], "500000");
        let fork = forked(round.clone(), None);
        assert_eq!(fork.len(), round.len() + 1);
        assert_eq!(fork.last().map(String::as_str), Some("--fork-session"));
        // Where the session went on past the round, the fork begins at the round's last message.
        let cut = forked(round.clone(), Some("m-9"));
        assert_eq!(cut[round.len()..], ["--fork-session", "--resume-session-at", "m-9"]);
        let at = round.iter().position(|a| a == "--settings").expect("its settings") + 1;
        for (i, (was, is)) in round.iter().zip(&fork).enumerate().filter(|(i, _)| *i != at) {
            assert_eq!(was, is, "argument {i}");
        }
        let was: Value = serde_json::from_str(&round[at]).expect("the round's settings");
        let is: Value = serde_json::from_str(&fork[at]).expect("the fork's settings");
        // The round's own hooks stay, the fork's files theirs; one that refuses each use of a tool is added.
        assert_eq!(is["hooks"]["SessionStart"], was["hooks"]["SessionStart"]);
        assert_eq!(is["hooks"]["PostToolUse"], was["hooks"]["PostToolUse"]);
        assert_eq!(is["autoMemoryEnabled"], false);
        assert_eq!(is["hooks"]["PreToolUse"][0]["matcher"], "*");
        assert_eq!(is["hooks"]["PreToolUse"][0]["hooks"][0]["command"], refusal());
    }

    // A call that only reads its session, as a keep-warm ping is, gains the hook that refuses every tool and nothing
    // else: the same arguments, the session gone on with in place, the round's own hooks kept.
    #[test]
    fn a_call_that_refuses_every_tool_adds_the_hook_and_does_not_fork_the_session() {
        let round: Vec<String> = ["--settings", r#"{"hooks":{"PostToolUse":[1]}}"#, "--resume", "s-1"]
            .iter()
            .map(|a| a.to_string())
            .collect();
        let refusing = refusing(round.clone());
        assert_eq!(refusing.len(), round.len());
        assert_eq!((refusing[0].as_str(), &refusing[2..]), ("--settings", &round[2..]));
        assert!(!refusing.iter().any(|a| a == "--fork-session"));
        let is: Value = serde_json::from_str(&refusing[1]).expect("the settings");
        assert_eq!(is["hooks"]["PostToolUse"], json!([1]));
        assert_eq!(is["hooks"]["PreToolUse"][0]["matcher"], "*");
        assert_eq!(is["hooks"]["PreToolUse"][0]["hooks"][0]["command"], refusal());
    }

    #[test]
    fn the_refusing_hook_refuses_every_tool_but_the_programs_own_for_the_return() {
        assert_eq!(refused("Bash"), Some(2));
        assert_eq!(refused("Read"), Some(2));
        assert_eq!(refused("Task"), Some(2));
        assert_eq!(refused(STRUCTURED_OUTPUT), Some(0));
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
    async fn the_owners_words_are_given_once_after_a_tool_and_moved_aside_as_given() {
        let dir = tempfile::tempdir().unwrap();
        // A data directory whose path a shell would split, or end a quote at.
        let rounds = dir.path().join("it's data").join("rounds");
        std::fs::create_dir_all(&rounds).unwrap();
        let words = Words {
            waiting: rounds.join("2-3.words.json"),
            given: rounds.join("2-3.given"),
        };
        assert_eq!(after(&words, "Bash", dir.path()), "", "nothing waits");

        words.put("Use the name \"Ann\".", &[]).await.expect("put");
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
        let first = rounds.join("2-3.given.1.json");
        assert_eq!(
            serde_json::from_slice::<Value>(&std::fs::read(&first).unwrap()).unwrap(),
            given
        );
        assert_eq!(after(&words, "Read", dir.path()), "", "given once");

        // Said twice before a tool, given once, together; the next delivery is kept beside the first.
        words.put("First.", &[]).await.expect("put");
        words.put("Second.", &[]).await.expect("put");
        let given: Value = serde_json::from_str(&after(&words, "Edit", dir.path())).expect("the hook's JSON");
        assert_eq!(given["hookSpecificOutput"]["additionalContext"], "First.\n\nSecond.");
        assert!(rounds.join("2-3.given.2.json").is_file());
        assert!(first.is_file());
        let mut left: Vec<_> = std::fs::read_dir(&rounds)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        let kept = ["2-3.given.1.json", "2-3.given.2.json", "2-3.said.json"];
        assert_eq!(left, kept, "nothing else is left");
    }

    // The versions of what the hook gave are read back from the files alone, as a process of the engine after the one
    // that put the words reads them: words taken back as more are put keep their versions with the new, what was taken
    // back once the program ended stays new, and a file the hook gave with no versions kept is told apart. Well under a
    // second alone; no other test reads the versions back.
    #[tokio::test]
    async fn words_taken_back_and_put_again_keep_both_versions() {
        let dir = tempfile::tempdir().unwrap();
        let words = Words {
            waiting: dir.path().join("1-1.words.json"),
            given: dir.path().join("1-1.given"),
        };
        let version = |n: u64| (BlockId("build_words".to_string()), Seq(n));
        assert_eq!(words.given().await, Ok((Vec::new(), Vec::new())), "nothing was given");
        words.put("First.", &[version(3)]).await.expect("put");
        words.put("Second.", &[version(5)]).await.expect("put");
        assert_ne!(after(&words, "Bash", dir.path()), "");
        words.put("Third.", &[version(7)]).await.expect("put");
        assert_eq!(words.take_back().await, Ok(true), "the third waited");
        assert!(!words.waiting.exists(), "taken back, for no hook to give");
        assert_eq!(words.take_back().await, Ok(false), "nothing waits");
        let read_again = Words {
            waiting: words.waiting.clone(),
            given: words.given.clone(),
        };
        assert_eq!(
            read_again.given().await,
            Ok((vec![version(3), version(5)], Vec::new())),
            "the first two, given together"
        );

        std::fs::write(dir.path().join("1-1.given.2.json"), "{}").unwrap();
        let (given, unknown) = read_again.given().await.expect("read");
        assert_eq!(given, [version(3), version(5)]);
        assert_eq!(
            unknown,
            [dir.path().join("1-1.given.2.json")],
            "given with no versions kept"
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

    // The command line refuses a session on its standard error with nothing on its standard output, or, as version
    // 2.1.327 does, in a result like the one a real round kept, which is the one line it prints. A stream that holds
    // anything besides that result is no refusal. Well under a second alone; no other test reads a refusal from a
    // stream.
    #[test]
    fn a_session_refused_on_the_standard_error_or_in_a_result_alone_is_tried_the_other_way() {
        let id = "6a1f3c2e-8d4b-4e7a-9c5f-2b8e1d0a7f43";
        let not_found = format!("No conversation found with session ID: {id}");
        let in_use = format!("Error: Session ID {id} is already in use.");
        let stream = |lines: &[String]| {
            let mut stream = Stream::default();
            for line in lines {
                stream.line(line.as_bytes());
            }
            stream
        };
        let failed = |is_error: bool, said: &str| {
            let usage = json!({ "input_tokens": 0, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0,
                "output_tokens": 0 });
            json!({ "type": "result", "subtype": "error_during_execution", "duration_ms": 0, "is_error": is_error,
                "num_turns": 0, "session_id": id, "total_cost_usd": 0, "usage": usage, "errors": [said] })
            .to_string()
        };

        let empty = stream(&[]);
        assert!(empty.refused(Begin::Resume, &not_found));
        assert!(empty.refused(Begin::New, &in_use));
        assert!(!empty.refused(Begin::New, &not_found));
        assert!(!empty.refused(Begin::Resume, "Prompt is too long"));

        let lost = stream(&[failed(true, &not_found)]);
        assert_eq!(lost.refusal(), Some(Begin::Resume));
        assert!(lost.refused(Begin::Resume, ""));
        assert!(!lost.refused(Begin::New, ""));
        let taken = stream(&[failed(true, &in_use)]);
        assert_eq!(taken.refusal(), Some(Begin::New));
        assert!(taken.refused(Begin::New, ""));
        assert!(!taken.refused(Begin::Resume, ""));

        let init = json!({ "type": "system", "subtype": "init", "claude_code_version": "2.1.327" }).to_string();
        for not in [
            stream(&[failed(true, "Prompt is too long")]),
            stream(&[failed(false, &not_found)]),
            stream(&[init.clone(), failed(true, &not_found)]),
            stream(&[failed(true, &not_found), init]),
        ] {
            assert_eq!(not.refusal(), None);
            assert!(!not.refused(Begin::Resume, &not_found));
        }
    }

    #[test]
    fn the_environment_is_built_from_what_is_kept_and_named_and_the_carriers_own() {
        let engine = [
            ("PATH", "/bin"),
            ("HOME", "/home/someone"),
            ("LC_ALL", "C"),
            ("SECRET_TOKEN", "x"),
            ("CLAUDE_CONFIG_DIR", "/elsewhere"),
        ]
        .map(|(n, v)| (n.to_string(), v.to_string()));
        let token = Token::new("sk-ant-oat01-secret".to_string());
        let profile = Path::new("/data/profile");
        let none = BTreeMap::new();
        assert_eq!(
            environment(&agents(), profile, engine, &none, &token),
            [
                ("HOME", "/home/someone"),
                ("LC_ALL", "C"),
                ("PATH", "/bin"),
                ("CLAUDE_CODE_DISABLE_BACKGROUND_TASKS", "1"),
                ("CLAUDE_CODE_ENABLE_FINE_GRAINED_TOOL_STREAMING", "0"),
                ("MAX_STRUCTURED_OUTPUT_RETRIES", "3"),
                ("CLAUDE_CODE_SUBPROCESS_ENV_SCRUB", "1"),
                ("CLAUDE_CONFIG_DIR", "/data/profile"),
                ("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-oat01-secret"),
            ]
            .map(|(n, v)| (n.to_string(), v.to_string())),
            "the engine's own directory, never the engine's environment's"
        );
        let mut configured = agents();
        configured.env = vec!["LANGUAGE".to_string()];
        let engine = [("LANGUAGE", "en"), ("OTHER", "x")].map(|(n, v)| (n.to_string(), v.to_string()));
        let given = environment(&configured, profile, engine, &none, &token);
        assert!(given.contains(&("LANGUAGE".to_string(), "en".to_string())), "named");
        assert!(!given.iter().any(|(n, _)| n == "OTHER"));
        assert_eq!(
            given.iter().filter(|(_, v)| v.contains("sk-ant")).count(),
            1,
            "the token given once"
        );
    }

    // What the project's declaration names comes after the carrier's own, and before the program's directory and the
    // token, which stay last and are given once. Well under a millisecond alone; no other test holds the order of the
    // environment a declaration adds to.
    #[test]
    fn the_variables_a_declaration_names_come_after_the_carriers_own_and_before_the_token() {
        let token = Token::new("sk-ant-oat01-secret".to_string());
        let declared = BTreeMap::from([("RUSTUP_AUTO_INSTALL".to_string(), "0".to_string())]);
        let given = environment(&agents(), Path::new("/data/profile"), [], &declared, &token);
        let names: Vec<&str> = given.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            [
                "CLAUDE_CODE_DISABLE_BACKGROUND_TASKS",
                "CLAUDE_CODE_ENABLE_FINE_GRAINED_TOOL_STREAMING",
                "MAX_STRUCTURED_OUTPUT_RETRIES",
                "CLAUDE_CODE_SUBPROCESS_ENV_SCRUB",
                "RUSTUP_AUTO_INSTALL",
                "CLAUDE_CONFIG_DIR",
                "CLAUDE_CODE_OAUTH_TOKEN",
            ]
        );
        assert!(given.contains(&("RUSTUP_AUTO_INSTALL".to_string(), "0".to_string())));
    }

    // A declaration may name no variable the engine owns, none the installation keeps by name, none the agents' program
    // reads itself, and none that is no name of a variable. Well under a millisecond alone; no other test holds what a
    // declaration may name for the command line, one case for each rule.
    #[test]
    fn a_declaration_names_no_variable_of_the_engines_own() {
        let faults = |name: &str, named: &[&str]| {
            let declared = BTreeMap::from([(name.to_string(), "1".to_string())]);
            let named: Vec<String> = named.iter().map(|n| n.to_string()).collect();
            environment_faults(&declared, &named)
        };
        assert_eq!(faults("RUSTUP_AUTO_INSTALL", &[]), Vec::<String>::new());
        assert_eq!(faults("RUSTUP_AUTO_INSTALL", &["LANGUAGE"]), Vec::<String>::new());
        for own in [
            "PATH",
            "LC_ALL",
            "MAX_STRUCTURED_OUTPUT_RETRIES",
            "CLAUDE_CODE_SUBPROCESS_ENV_SCRUB",
        ] {
            assert_eq!(
                faults(own, &[]),
                [format!(
                    "wf-agents.toml [environment]: {own} is the engine's own, which it gives every round itself"
                )],
                "{own}"
            );
        }
        let kept = faults("LANGUAGE", &["LANGUAGE"]);
        assert_eq!(kept.len(), 1);
        assert!(
            kept[0].contains("LANGUAGE") && kept[0].contains("agents.env"),
            "{kept:?}"
        );
        for program in ["CLAUDE_CONFIG_DIR", "CLAUDE_CODE_OAUTH_TOKEN", "ANTHROPIC_BASE_URL"] {
            let said = faults(program, &[]);
            assert_eq!(said.len(), 1, "{program}");
            assert!(
                said[0].contains(program) && said[0].contains("agents' program"),
                "{said:?}"
            );
        }
        for wrong in ["1BAD", "A-B", ""] {
            let said = faults(wrong, &[]);
            assert_eq!(said.len(), 1, "{wrong:?}");
            assert!(said[0].contains("no name of a variable"), "{said:?}");
        }
    }
}
