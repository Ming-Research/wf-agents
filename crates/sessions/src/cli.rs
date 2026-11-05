//! The command line an agent's round runs in: how it starts (the context, permissions, tools, model and reasoning
//! effort it is given), one run of it in the work's worktree from the message to the return, and what its stream
//! reports. Every round's message goes in one way: on the standard input as one user message of the stream-json input
//! format, its text and then the images it gives, each after its address, so that the owner's images reach an agent
//! in its message and no agent has to find a file or be let read one. The flags the installation gives every agent's command line, such as the tools it offers, stay in the
//! configuration; the flags that shape a session's context belong here. It is a trait so that the engine's tests can
//! script the agents; [`Claude`] runs `claude -p`.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};

use crate::shape::{Shape, required_of};

/// What the engine asks of one round.
#[derive(Debug, Clone)]
pub struct Request {
    pub work: i64,
    pub cell: String,
    pub line: String,
    pub cli: config::Cli,
    pub model: String,
    pub effort: Option<String>,
    pub worktree: PathBuf,
    /// The configuration directory the CLI runs with, apart from the owner's own; none to leave it as it is.
    pub config_dir: Option<PathBuf>,
    /// The CLI's session to resume; none for the line's first round, which starts `new_session`.
    pub resume: Option<String>,
    /// The session to resume is forked, not resumed: the CLI copies it under `new_session`, which holds all it held and
    /// goes on from there, and leaves it as it was. A matter of the architect forks its base so.
    pub fork: bool,
    /// What the command line appends to its own system prompt: what every session of the round's line is given alike,
    /// which the session started with ([`crate::system`]); none for a session started without one.
    pub system: Option<String>,
    pub new_session: String,
    /// The message's text, given on the CLI's standard input ([`stdin`]).
    pub prompt: String,
    /// The images the message gives after its text, each after its address; none for most rounds.
    pub images: Vec<Image>,
    /// What the return must be, as JSON Schema; the engine checks the return against it.
    pub schema: Value,
    /// The schema the CLI is given, the same for every cell of the line and of the lines forked together with it:
    /// [`envelope`].
    pub envelope: Value,
    /// The commands the round may run besides those the CLI's own arguments allow, as the CLI's tool rules name them:
    /// its line's permissions.
    pub allowed: Vec<String>,
    /// The tools and commands the round may not use, as the CLI's tool rules name them: its line's permissions.
    pub refused: Vec<String>,
    /// Directories outside the worktree the agent may read but not change, such as where the engine saves a tool's
    /// output too long to give inline; absolute paths.
    pub readable: Vec<PathBuf>,
    /// For a keep-warm ping: the whole command line of the session's last round, run again as it was, resuming the
    /// session, with only the message changed, so that everything that shapes the prompt is the same and the prompt
    /// cache is read; none for a round, whose command line is built from the fields above.
    pub replay: Option<Vec<String>>,
    /// A file holding what the round's cell was given, whole, which the command line gives the session again at once
    /// when it compacts the session within the round; none to give nothing again.
    pub regive: Option<PathBuf>,
    /// The shell command the command line's hook runs after each tool use, which hands in the interjections waiting
    /// for the round's line; none to hand in none during the round.
    pub deliver: Option<String>,
    /// A call apart from an agent's round, such as the repair of a return whose JSON the command line could not parse:
    /// it is offered no tools and given no structured output, and what it answers is its text.
    pub bare: bool,
}

/// An image a round's message gives after its text: the owner's, sent with words the message gives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// What names it in the message, written just before it: `image:12`.
    pub address: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

/// The variable that names to `claude` the configuration directory it runs with: its settings, its login and the
/// sessions it keeps.
pub const CONFIG_DIR: &str = "CLAUDE_CONFIG_DIR";

/// The account a configuration directory of a CLI is signed in as: for `claude`, the email of the account its
/// `.claude.json` keeps, and nothing else of that file; none when it is not signed in.
pub fn signed_in_as(kind: config::CliKind, dir: &Path) -> Option<String> {
    match kind {
        config::CliKind::Claude => {
            let text = std::fs::read_to_string(dir.join(".claude.json")).ok()?;
            let v: Value = serde_json::from_str(&text).ok()?;
            v["oauthAccount"]["emailAddress"]
                .as_str()
                .filter(|e| !e.trim().is_empty())
                .map(str::to_string)
        }
    }
}

/// How the owner signs a new configuration directory of a CLI in, once, by hand: the variable that names the directory
/// to it, and what to type once it runs.
pub fn sign_in(kind: config::CliKind) -> (&'static str, &'static str) {
    match kind {
        config::CliKind::Claude => (CONFIG_DIR, "/login"),
    }
}

/// The environment every agent's command line starts with, besides its configuration directory: each tool input
/// streamed whole, not in pieces, so that the service checks its JSON; three attempts at a return, not five, since
/// each attempt writes the whole return again; and no Rust toolchain installed by rustup when a command the agent may
/// run, such as `cargo update` or `rustfmt`, asks for one the machine lacks: the command fails instead of downloading
/// it.
pub const ENVIRONMENT: [(&str, &str); 3] = [
    ("CLAUDE_CODE_ENABLE_FINE_GRAINED_TOOL_STREAMING", "0"),
    ("MAX_STRUCTURED_OUTPUT_RETRIES", "3"),
    ("RUSTUP_AUTO_INSTALL", "0"),
];

/// The tool the command line hands a return in with, under `--json-schema`.
pub const STRUCTURED_OUTPUT: &str = "StructuredOutput";

/// An attempt at a return that the command line refused, because it could not parse it as JSON or because it does not
/// fit the [`envelope`], or that is no JSON: the input the model gave its StructuredOutput tool, whole, rebuilt from
/// the stream's partial messages.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Refused {
    /// The tool use's id.
    pub id: String,
    /// The input as the model wrote it.
    pub raw: String,
    /// Its length as the command line reported it ("first 200 of N bytes", N counting characters); none when it did
    /// not.
    pub length: Option<usize>,
    /// Why the command line refused it, as it said, without the part of the input it quoted.
    pub why: Option<String>,
}

/// Where a return was read from: only the tool's input has been checked against the schema the command line was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The input of the `StructuredOutput` tool, which the command line checked against its schema.
    Tool,
    /// The JSON in the text of the run's result, which no check of the command line saw.
    Text,
}

impl Source {
    /// The word the round's record keeps for it.
    pub fn name(self) -> &'static str {
        match self {
            Source::Tool => "tool",
            Source::Text => "text",
        }
    }
}

/// What a round came to.
#[derive(Debug, Clone, Default)]
pub struct Reply {
    /// The return; none when the run handed in nothing that parses.
    pub output: Option<Value>,
    /// Where the return was read from; none when there is none.
    pub output_from: Option<Source>,
    /// The CLI's session the round ran in.
    pub session: String,
    pub model: Option<String>,
    /// Tokens and cost as the CLI reported them. The cost and the tokens by model are the session's so far, the
    /// rounds it resumed from included; the other tokens are this round's.
    pub usage: Value,
    pub compactions: i64,
    /// Why the run failed, when it did.
    pub error: Option<String>,
    /// The command line the round ran with in the end; empty when it is the one the request gives.
    pub command: Vec<String>,
    /// The text the run answered with, as the command line gave it; none when it gave none.
    pub result: Option<String>,
    /// Each attempt at a return the command line refused or that is no JSON, in the order made; empty when it took the
    /// first.
    pub refused: Vec<Refused>,
    /// Why the run was stopped before it ended, when whoever watched it answered [`Next::Stop`]: such a run hands in
    /// nothing, and its failure is told by this and never by the words of what the CLI reported.
    pub stopped: Option<String>,
}

/// One call to the model, as the stream reports it. The stream writes a call on several lines, and says its output
/// tokens only as they come, so a call may be told more than once: the same `id`, with more output.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Call {
    /// The id of the message the call wrote.
    pub id: String,
    /// The model it ran on, as the stream names it; none when it does not.
    pub model: Option<String>,
    /// Its input tokens: not read from the cache, read from it, and written to it for five minutes and for an hour.
    pub input: i64,
    pub read: i64,
    pub written_5m: i64,
    pub written_1h: i64,
    /// Its output tokens so far.
    pub output: i64,
    /// A subagent's call, which is not in the context of the round's session.
    pub subagent: bool,
}

impl Call {
    /// What the call was given to read: all its input tokens.
    pub fn prompt(&self) -> i64 {
        self.input + self.read + self.written_5m + self.written_1h
    }
}

/// What the watcher of a running round is told, in terms of no command line in particular.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seen {
    /// What the agent is doing, in a line for the owner's page.
    Doing(String),
    /// A call to the model, or more of a call told before.
    Call(Call),
    /// A tool the agent used: its name, what it used it on, and a digest of its input, by which the same use is known
    /// again; the input itself is not kept.
    Tool {
        name: String,
        on: Option<String>,
        digest: u64,
    },
    /// The command line compacted the session.
    Compacted,
}

/// What the watcher of a running round answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    Go,
    /// Stop the round, for this reason.
    Stop(String),
}

/// A watcher for a run that only tells what the agent is doing, and never stops it: a call apart from a round, such as
/// a repair.
pub fn telling(activity: &dyn Fn(&str)) -> impl Fn(Seen) -> Next + '_ {
    move |seen| {
        if let Seen::Doing(what) = seen {
            activity(&what);
        }
        Next::Go
    }
}

/// The command line agents run in, which the engine's threads share.
pub trait Agents: Send + Sync {
    /// Runs a round; `watch` is told what the run does as it happens, and may answer that it be stopped.
    fn run(&self, r: &Request, watch: &dyn Fn(Seen) -> Next) -> Reply;

    /// The CLI's own version, as it reports it.
    fn version(&self, _cli: &config::Cli, _config_dir: Option<&std::path::Path>) -> String {
        "unknown".to_string()
    }
}

/// The command line a round runs with, the program first. The CLI reads only the settings of its own configuration
/// directory, not the project's: it reads no project instructions, such as the worktree's AGENTS.md, by itself, so the
/// project's guide reaches the agent only as the engine gives it, once in a session.
pub fn command_line(r: &Request) -> Vec<String> {
    let mut out = vec![r.cli.command.clone()];
    out.extend(r.cli.args.iter().cloned());
    out.extend(
        [
            "-p",
            // The message comes on the standard input as one user message of the stream-json format, which carries
            // images as well as text ([`stdin`]).
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--verbose",
            // The stream carries each tool input in pieces as it is written, which rebuild an attempt at a return the
            // command line refused.
            "--include-partial-messages",
            "--setting-sources",
            "user",
            "--model",
            r.model.as_str(),
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    // What every session of the line is given alike, at the end of the system prompt, where the prompt cache that
    // sessions share reads it.
    if let Some(s) = &r.system {
        out.extend([crate::system::FLAG.to_string(), s.clone()]);
    }
    // The CLI gives the model the schema with the session's tools, so a schema that changed from one round to the
    // next would leave nothing of the session's prompt cache, and a fork given another schema than its base would read
    // nothing of the base from it: it is the line's envelope, the same for every cell of the line and of the lines
    // forked together with it. A bare call is given no schema and no tools.
    if r.bare {
        out.extend(["--tools".to_string(), String::new()]);
    } else {
        out.push("--json-schema".into());
        out.push(r.envelope.to_string());
    }
    if let Some(e) = &r.effort {
        out.extend(["--effort".to_string(), e.clone()]);
    }
    // A directory outside the worktree the agent is to read is added to the CLI's working directories, which lets
    // its tools read and edit there, and edits there are refused: an Edit rule covers every tool that edits a file,
    // and a path that starts with two slashes is absolute.
    for d in &r.readable {
        out.extend(["--add-dir".to_string(), d.display().to_string()]);
    }
    // The line's permissions, after the CLI's own arguments: the CLI allows what any rule of either allows, unless a
    // rule of either refuses it, and refuses what any rule of either refuses. A flag given twice adds its values to
    // those it was given before, as `claude`'s parser of arguments does with every flag that takes a list (checked
    // with claude 2.1.316).
    if !r.allowed.is_empty() {
        out.push("--allowedTools".into());
        out.extend(r.allowed.iter().cloned());
    }
    let mut refused = r.refused.clone();
    refused.extend(r.readable.iter().map(|d| format!("Edit(/{}/**)", d.display())));
    if !refused.is_empty() {
        out.push("--disallowedTools".into());
        out.extend(refused);
    }
    // The command line's hooks: after a compaction within the round, which leaves the session holding nothing it was
    // given, one gives it again at once what the round's cell was given, whole; after each tool use, one hands in the
    // interjections waiting for the line.
    if let Some(h) = hooks(r) {
        out.push("--settings".into());
        out.push(h.to_string());
    }
    // A fork resumes the base under the fork's own id, which the CLI takes only with its flag to fork (checked with
    // claude 2.1.318: `--session-id` with `--resume` is refused without `--fork-session`).
    match (&r.resume, r.fork) {
        (Some(s), true) => out.extend([
            "--resume".to_string(),
            s.clone(),
            "--fork-session".to_string(),
            "--session-id".to_string(),
            r.new_session.clone(),
        ]),
        (Some(s), false) => out.extend(["--resume".to_string(), s.clone()]),
        (None, _) => out.extend(["--session-id".to_string(), r.new_session.clone()]),
    }
    out
}

/// Whether a command line takes its message as the stream-json input format: every round's since rounds were sent so,
/// and a keep-warm ping's that replays one of them.
fn streams_input(line: &[String]) -> bool {
    line.windows(2)
        .any(|w| w[0] == "--input-format" && w[1] == "stream-json")
}

/// What a command line is given on its standard input: with the stream-json input format, one user message as a line
/// of JSON, the message's text and then each image after a text block of its address; a command line from before rounds
/// were sent so, as a keep-warm ping replays it, the text as it is.
pub fn stdin(line: &[String], prompt: &str, images: &[Image]) -> Vec<u8> {
    if !streams_input(line) {
        return prompt.as_bytes().to_vec();
    }
    let mut content = vec![json!({ "type": "text", "text": prompt })];
    for im in images {
        content.push(json!({ "type": "text", "text": im.address }));
        content.push(json!({
            "type": "image",
            "source": { "type": "base64", "media_type": im.media_type, "data": db::attachments::base64(&im.bytes) },
        }));
    }
    let message = json!({ "type": "user", "message": { "role": "user", "content": content } });
    let mut out = message.to_string().into_bytes();
    out.push(b'\n');
    out
}

/// A word as a shell reads it back as it is: in single quotes.
pub fn quote(word: &str) -> String {
    format!("'{}'", word.replace('\'', "'\\''"))
}

/// The settings that give a round's command line its hooks, whose output the command line adds to the session: at the
/// start of a session it compacted, what a file holds; after each tool use, what a command gives. None when the round
/// has neither.
fn hooks(r: &Request) -> Option<Value> {
    let mut hooks = serde_json::Map::new();
    if let Some(f) = &r.regive {
        let cat = format!("cat {}", quote(&f.display().to_string()));
        hooks.insert(
            "SessionStart".into(),
            json!([{ "matcher": "compact", "hooks": [{ "type": "command", "command": cat }] }]),
        );
    }
    if let Some(c) = &r.deliver {
        hooks.insert(
            "PostToolUse".into(),
            json!([{ "matcher": "*", "hooks": [{ "type": "command", "command": c }] }]),
        );
    }
    (!hooks.is_empty()).then(|| json!({ "hooks": hooks }))
}

/// The prefixes of the names of the variables the CLI reads, which its command line inherits from the engine's own
/// environment besides those the engine sets.
const READ_BY_CLI: [&str; 8] = [
    "ANTHROPIC_",
    "CLAUDE",
    "DISABLE_",
    "ENABLE_",
    "MAX_",
    "BASH_",
    "MCP_",
    "API_",
];

/// The words in a variable's name that mark its value as a secret, which a round's record keeps only by its digest:
/// enough to see that it changed.
const SECRET: [&str; 7] = ["KEY", "TOKEN", "SECRET", "PASSWORD", "PASSPHRASE", "AUTH", "HEADERS"];

/// The variables of an environment the CLI reads, by name, each secret by its digest.
pub fn read_by_cli(vars: impl IntoIterator<Item = (OsString, OsString)>) -> BTreeMap<String, String> {
    vars.into_iter()
        .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)))
        .filter(|(k, _)| READ_BY_CLI.iter().any(|p| k.starts_with(p)))
        .map(|(k, v)| {
            let v = if SECRET.iter().any(|s| k.contains(s)) {
                format!("digest {}", &db::content_hash(&v)[..16])
            } else {
                v
            };
            (k, v)
        })
        .collect()
}

/// What a round's record says of its call: the whole command line (the structured-output schema in it), the
/// environment it runs with as far as the CLI reads it, the one the engine inherited and the variables the engine sets,
/// the session it started or resumed, and the engine's and the CLI's versions.
pub fn call(r: &Request, command: &[String], cli_version: &str, engine: &str) -> Value {
    let session = match (&r.resume, r.fork) {
        (Some(s), true) => json!({ "forked": s, "started": r.new_session }),
        (Some(s), false) => json!({ "resumed": s }),
        (None, _) => json!({ "started": r.new_session }),
    };
    let mut env: serde_json::Map<String, Value> = read_by_cli(std::env::vars_os())
        .into_iter()
        .map(|(k, v)| (k, json!(v)))
        .collect();
    if let Some(d) = &r.config_dir {
        env.insert(CONFIG_DIR.into(), json!(d.display().to_string()));
    }
    for (k, v) in ENVIRONMENT {
        env.insert(k.into(), json!(v));
    }
    json!({
        "command": command,
        "env": env,
        "cwd": r.worktree.display().to_string(),
        "session": session,
        "engine": engine,
        "cli": cli_version,
    })
}

/// The configuration directory a CLI runs agents with, made when it is not there yet, and kept out of git even when the
/// owner made it first by logging in there; with what went wrong making it, if anything.
pub fn home(data: &Path, cli: &config::Cli, name: &str) -> (PathBuf, Option<String>) {
    let dir = cli.home(data, name);
    let ignore = dir.join(".gitignore");
    if !ignore.exists()
        && let Err(x) = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&ignore, "*\n"))
    {
        let why = format!("{}: {x}", dir.display());
        return (dir, Some(why));
    }
    (dir, None)
}

/// Agents run with `claude -p`.
pub struct Claude;

impl Claude {
    /// The command line a request runs: the one it replays, or the round's.
    fn line(r: &Request) -> Vec<String> {
        r.replay.clone().unwrap_or_else(|| command_line(r))
    }

    fn command(r: &Request) -> Command {
        let line = Claude::line(r);
        let mut c = Command::new(&line[0]);
        c.args(&line[1..]);
        if let Some(d) = &r.config_dir {
            c.env(CONFIG_DIR, d);
        }
        c.envs(ENVIRONMENT);
        c.current_dir(&r.worktree);
        c
    }
}

impl Agents for Claude {
    fn run(&self, r: &Request, watch: &dyn Fn(Seen) -> Next) -> Reply {
        let reply = Claude::once(r, watch);
        // A session whose first round was cut off exists, though no round of it ever ended: resume it. So does a fork
        // whose first round was cut off once the CLI had copied the base: it is resumed, not forked again.
        let in_use = reply.error.as_deref().is_some_and(|e| e.contains("already in use"));
        if (r.resume.is_none() || r.fork) && reply.output.is_none() && in_use {
            let again = Request {
                resume: Some(r.new_session.clone()),
                fork: false,
                ..r.clone()
            };
            let mut reply = Claude::once(&again, watch);
            reply.command = command_line(&again);
            return reply;
        }
        reply
    }

    fn version(&self, cli: &config::Cli, config_dir: Option<&std::path::Path>) -> String {
        let mut c = Command::new(&cli.command);
        c.arg("--version");
        if let Some(d) = config_dir {
            c.env(CONFIG_DIR, d);
        }
        match c.stdin(Stdio::null()).output() {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
            Ok(o) => format!("unknown: {} --version exited with {}", cli.command, o.status),
            Err(e) => format!("unknown: {}: {e}", cli.command),
        }
    }
}

/// How long a stopped command line has, from the polite signal, to end by itself and what it started, before it and
/// its process group are killed. It is what the command line's own clean-up may take, and a stopped round holds a slot.
/// The engine's tool `dependency_sources` gives a command it stops for its time limit the same.
pub const STOP_WAIT: Duration = Duration::from_secs(3);

/// Sends a signal to a process group, by the `kill` command, which needs no `unsafe`. Only the engine's own child's
/// group is ever named, while that child is not reaped: a group's id cannot be taken by another while its leader's
/// process entry stands, so the signal cannot reach anyone else's. The engine's tool `dependency_sources` stops the
/// command it runs the same way.
pub fn signal_group(group: u32, signal: &str) {
    let _ = Command::new("kill")
        .args(["-s", signal, "--", &format!("-{group}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// What a run's overseer does, the group being `$1` and the wait `$2`: it waits for its standard input to close, then
/// stops the group as [`stop`] does.
const OVERSEEING: &str = r#"read -r line; kill -s TERM -- "-$1"; sleep "$2"; kill -s KILL -- "-$1""#;

/// What stops a run's process group when the engine goes away while it runs. The group is out of the terminal's reach,
/// so a Ctrl-C or a closed terminal would end the engine and not the run, and no signal handler sees a crash or a
/// `kill -9`. So a small shell waits on a pipe that only the engine holds: however the engine ends, the pipe closes and
/// the shell stops the group. It is in a process group of its own, so that the terminal's signals do not end it first.
/// Dropped, it ends without touching the run.
struct Overseer(std::process::Child);

impl Overseer {
    /// None when no shell can be started: the run goes on, with what a crash leaves behind as before.
    fn of(group: u32) -> Option<Overseer> {
        Command::new("sh")
            .args(["-c", OVERSEEING, "overseer"])
            .args([group.to_string(), STOP_WAIT.as_secs().to_string()])
            .process_group(0)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()
            .map(Overseer)
    }
}

impl Drop for Overseer {
    /// The shell is killed while the pipe is open: closing the pipe is what makes it stop the group.
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Claude {
    /// One run of the CLI, in a process group of its own, so that it can be stopped with everything it started: the
    /// message on its standard input ([`stdin`]), its stream read line by line as it comes, what each line says told to
    /// `watch`, which may answer that the run be stopped.
    fn once(r: &Request, watch: &dyn Fn(Seen) -> Next) -> Reply {
        let mut reply = Reply {
            session: match &r.resume {
                Some(s) if !r.fork => s.clone(),
                _ => r.new_session.clone(),
            },
            ..Reply::default()
        };
        let child = Claude::command(r)
            .process_group(0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(e) => {
                reply.error = Some(format!("{}: {e}", r.cli.command));
                return reply;
            }
        };
        let overseer = Overseer::of(child.id());
        // The message is written beside, as the standard error is read, so that a large one, images and all, and the
        // stream never wait on each other; the standard input is closed once it is written, so the CLI knows it has all
        // of it.
        let message = stdin(&Claude::line(r), &r.prompt, &r.images);
        let writer = child
            .stdin
            .take()
            .map(|mut input| std::thread::spawn(move || input.write_all(&message)));
        // The standard error is read beside, so that neither pipe fills while the other is read.
        let stderr = child.stderr.take().map(|mut e| {
            std::thread::spawn(move || {
                let mut s = String::new();
                let _ = std::io::Read::read_to_string(&mut e, &mut s);
                s
            })
        });
        let mut stream = Stream::default();
        if let Some(out) = child.stdout.take() {
            let mut lines = std::io::BufRead::lines(std::io::BufReader::new(out));
            'reading: for line in lines.by_ref().map_while(Result::ok) {
                for seen in stream.line(&line) {
                    if let Next::Stop(why) = watch(seen) {
                        reply.stopped = Some(why);
                        break 'reading;
                    }
                }
            }
            // Before the child can be reaped, after which its group's id is free for another's.
            drop(overseer);
            if let Some(why) = reply.stopped.clone() {
                stop(&mut child, lines);
                stream.end(&mut reply);
                // Whatever the stream gave before it was stopped is not a return, and the run's own end was not read.
                reply.output = None;
                reply.output_from = None;
                reply.error = Some(why);
                return reply;
            }
        }
        if let Some(Err(e)) = writer.and_then(|t| t.join().ok()) {
            reply.error = Some(format!("giving the message: {e}"));
        }
        let status = child.wait();
        let err = stderr.and_then(|t| t.join().ok()).unwrap_or_default();
        stream.end(&mut reply);
        if reply.output.is_none() && reply.error.is_none() {
            let status = status.map_or_else(|e| e.to_string(), |s| s.to_string());
            reply.error = Some(format!(
                "{} exited with {status} and no result: {}",
                r.cli.command,
                err.trim()
            ));
        }
        reply
    }
}

/// Stops a run and everything it started: asks its process group to end, reads what is left of its stream beside, so
/// that no write of its own keeps it from ending, gives it [`STOP_WAIT`] at most, and kills the group whatever came of
/// that. Only then is the child reaped, so that the group's id is the child's for every signal; the threads that hold
/// its standard input and error are left to end as their pipes close, since a process that left the group could hold
/// them open.
fn stop<R>(child: &mut std::process::Child, lines: std::io::Lines<R>)
where
    R: std::io::BufRead + Send + 'static,
{
    let group = child.id();
    signal_group(group, "TERM");
    let (ended, read_all) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for _ in lines.map_while(Result::ok) {}
        let _ = ended.send(());
    });
    let _ = read_all.recv_timeout(STOP_WAIT);
    signal_group(group, "KILL");
    // The child itself, in case there is no `kill` to run.
    let _ = child.kill();
    let _ = child.wait();
}

/// What the command line's stream gave as it came: its lines kept for the end but for its partial messages, which are
/// passed over once the StructuredOutput inputs are rebuilt from them and the calls read from them.
#[derive(Debug, Default)]
pub struct Stream {
    kept: String,
    partials: Partials,
    calls: Calls,
    /// The tool uses told, by their ids.
    tools: BTreeSet<String>,
}

impl Stream {
    /// Takes one line of the stream; what it says happened, if anything.
    pub fn line(&mut self, line: &str) -> Vec<Seen> {
        if line.starts_with(PARTIAL) {
            // Only the lines that start or end a call or that matter to the input being rebuilt are parsed.
            let wanted = line.contains("message_start") || line.contains("message_delta") || self.partials.wants(line);
            if !wanted {
                return Vec::new();
            }
            return serde_json::from_str::<Value>(line).map_or_else(|_| Vec::new(), |v| self.partial(&v));
        }
        let parsed: Option<Value> = serde_json::from_str(line).ok();
        if let Some(v) = &parsed
            && v["type"] == "stream_event"
        {
            return self.partial(v);
        }
        self.kept.push_str(line);
        self.kept.push('\n');
        let Some(v) = parsed else { return Vec::new() };
        let mut seen: Vec<Seen> = activity_of_value(&v).map(Seen::Doing).into_iter().collect();
        match v["type"].as_str() {
            Some("assistant") => {
                seen.extend(
                    self.calls
                        .message(&v["message"], &v["parent_tool_use_id"])
                        .map(Seen::Call),
                );
                if v["parent_tool_use_id"].is_null() {
                    seen.extend(self.tool_uses(&v["message"]));
                }
            }
            Some("system") if v["subtype"] == "compact_boundary" => seen.push(Seen::Compacted),
            _ => {}
        }
        seen
    }

    /// Takes a partial message: what it rebuilds, and the call it starts or says more of.
    fn partial(&mut self, v: &Value) -> Vec<Seen> {
        self.partials.event(v);
        let parent = &v["parent_tool_use_id"];
        let ev = &v["event"];
        let call = match ev["type"].as_str() {
            Some("message_start") => self.calls.message(&ev["message"], parent),
            Some("message_delta") => self.calls.delta(&ev["usage"], parent),
            _ => None,
        };
        call.map(Seen::Call).into_iter().collect()
    }

    /// The tool uses of an agent's message not told yet.
    fn tool_uses(&mut self, message: &Value) -> Vec<Seen> {
        let mut out = Vec::new();
        for c in message["content"].as_array().into_iter().flatten() {
            let Some(id) = c["id"].as_str().filter(|_| c["type"] == "tool_use") else {
                continue;
            };
            if self.tools.insert(id.to_string()) {
                out.push(Seen::Tool {
                    name: c["name"].as_str().unwrap_or("a tool").to_string(),
                    on: subject(&c["input"]).map(cut_line),
                    digest: digest(&c["input"].to_string()),
                });
            }
        }
        out
    }

    /// What the run came to, from the lines kept: [`read_stream`], with each attempt at a return the command line
    /// refused.
    pub fn end(self, reply: &mut Reply) {
        read_stream(&self.kept, reply);
        reply.refused = self.partials.refused(&refusals(&self.kept));
    }
}

/// What a partial message's line starts with, as the command line writes it.
const PARTIAL: &str = r#"{"type":"stream_event""#;

/// A digest of a tool's input, to know the same input again within a run.
fn digest(text: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(text, &mut h);
    std::hash::Hasher::finish(&h)
}

/// The calls a stream told of, each once by the id of its message, however many lines give it: the partial message that
/// starts it, each block of it as a line of its own, and the partial message that says its output in the end. Each line
/// may say its output tokens so far, so a call is told again only when they grew.
#[derive(Debug, Default)]
struct Calls {
    told: BTreeMap<String, Call>,
    /// The call being written by the agent (the key of none) and by each subagent.
    open: BTreeMap<Option<String>, String>,
}

impl Calls {
    /// Takes a message of the stream, with the tool use of the subagent that wrote it when one did: the call it tells,
    /// when it tells a new one or more of one. A message the command line makes itself, such as the one it puts in when
    /// it resumes a session, called no model.
    fn message(&mut self, m: &Value, parent: &Value) -> Option<Call> {
        if m["model"] == "<synthetic>" {
            return None;
        }
        let u = &m["usage"];
        let n = |k: &str| u[k].as_i64();
        let (input, read, written) = (
            n("input_tokens")?,
            n("cache_read_input_tokens")?,
            n("cache_creation_input_tokens")?,
        );
        if input + read + written == 0 {
            return None;
        }
        let (written_5m, written_1h) = match u.get("cache_creation") {
            Some(c) => (
                c["ephemeral_5m_input_tokens"].as_i64().unwrap_or(0),
                c["ephemeral_1h_input_tokens"].as_i64().unwrap_or(0),
            ),
            None => (written, 0),
        };
        let id = m["id"]
            .as_str()
            .map_or_else(|| format!("no id {}", self.told.len()), str::to_string);
        let key = parent.as_str().map(str::to_string);
        self.open.insert(key, id.clone());
        let output = n("output_tokens").unwrap_or(0);
        if self.told.contains_key(&id) {
            return self.more(&id, output);
        }
        let call = Call {
            id: id.clone(),
            model: m["model"].as_str().map(str::to_string),
            input,
            read,
            written_5m,
            written_1h,
            output,
            subagent: !parent.is_null(),
        };
        self.told.insert(id, call.clone());
        Some(call)
    }

    /// Takes the usage of a partial message that ends the call being written: its output in the end.
    fn delta(&mut self, usage: &Value, parent: &Value) -> Option<Call> {
        let id = self.open.get(&parent.as_str().map(str::to_string))?.clone();
        self.more(&id, usage["output_tokens"].as_i64()?)
    }

    /// A call told before, with its output now, when that grew.
    fn more(&mut self, id: &str, output: i64) -> Option<Call> {
        let call = self.told.get_mut(id)?;
        (output > call.output).then(|| {
            call.output = output;
            call.clone()
        })
    }
}

/// The inputs the model gave the command line's StructuredOutput tool, rebuilt whole from the stream's partial
/// messages (`input_json_delta`), the only tool inputs rebuilt; every other piece is passed over unread.
#[derive(Debug, Default)]
struct Partials {
    /// The StructuredOutput block being written: its index in its message, its tool use's id and its input so far.
    open: Option<(i64, String, String)>,
    /// Each input written, by its tool use's id, in order.
    done: Vec<(String, String)>,
}

impl Partials {
    /// Whether a line that is a partial message may start, add to or end a StructuredOutput input.
    fn wants(&self, line: &str) -> bool {
        line.contains("content_block_start")
            || (self.open.is_some() && (line.contains("input_json_delta") || line.contains("content_block_stop")))
    }

    /// Takes a partial message: the agent's own, not a subagent's.
    fn event(&mut self, v: &Value) {
        if !v["parent_tool_use_id"].is_null() {
            return;
        }
        let ev = &v["event"];
        let index = ev["index"].as_i64();
        match ev["type"].as_str() {
            // A new message numbers its blocks afresh: an input left open was cut off, and is kept as it came.
            Some("message_start") => self.close(None),
            Some("content_block_start") => {
                let b = &ev["content_block"];
                if b["type"] == "tool_use" && b["name"] == STRUCTURED_OUTPUT {
                    self.close(None);
                    if let Some(i) = index {
                        self.open = Some((i, b["id"].as_str().unwrap_or_default().to_string(), String::new()));
                    }
                }
            }
            Some("content_block_delta") if ev["delta"]["type"] == "input_json_delta" => {
                if let Some((i, _, raw)) = &mut self.open
                    && Some(*i) == index
                    && let Some(piece) = ev["delta"]["partial_json"].as_str()
                {
                    raw.push_str(piece);
                }
            }
            Some("content_block_stop") => self.close(index),
            _ => {}
        }
    }

    /// Ends the input being written: at the stop of its block, or whatever block it is at for none.
    fn close(&mut self, index: Option<i64>) {
        if let Some((i, id, raw)) = self.open.take() {
            if index.is_some_and(|x| x != i) {
                self.open = Some((i, id, raw));
                return;
            }
            self.done.push((id, raw));
        }
    }

    /// The inputs that are attempts the command line refused, for whatever reason, and any that is no JSON, such as one
    /// cut off; each with the length it reported and why it refused it. An input it took is not one.
    fn refused(mut self, refusals: &[(String, String)]) -> Vec<Refused> {
        self.close(None);
        self.done
            .into_iter()
            .filter_map(|(id, raw)| {
                let said = refusals.iter().find(|(r, _)| *r == id).map(|(_, why)| why.as_str());
                let refused = said.is_some() || serde_json::from_str::<Value>(&raw).is_err();
                refused.then(|| Refused {
                    length: said.and_then(reported_length),
                    why: said.map(without_quote),
                    id,
                    raw,
                })
            })
            .collect()
    }
}

/// The tool uses the command line refused, each by its id with what it said: the tool results of the agent's own
/// messages that are errors.
fn refusals(stream: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in stream.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v["type"] != "user" || !v["parent_tool_use_id"].is_null() {
            continue;
        }
        for c in v["message"]["content"].as_array().into_iter().flatten() {
            if c["type"] != "tool_result" || c["is_error"] != true {
                continue;
            }
            let text = match &c["content"] {
                Value::String(s) => s.clone(),
                Value::Array(xs) => xs
                    .iter()
                    .filter_map(|x| x["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => String::new(),
            };
            out.push((c["tool_use_id"].as_str().unwrap_or_default().to_string(), text));
        }
    }
    out
}

/// The length the command line reports of an input it could not parse, "first 200 of N bytes": N, which counts
/// characters.
fn reported_length(said: &str) -> Option<usize> {
    let rest = &said[said.find("first ")?..];
    let of = rest.find(" of ")?;
    let digits: String = rest[of + 4..].chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// What the command line said of an input it refused, without the part of the input it quotes after the colon of
/// "You sent (first 200 of N bytes):", which is the agent's own return.
fn without_quote(said: &str) -> String {
    let said = said.trim().trim_start_matches("<tool_use_error>");
    match said.find("bytes):") {
        Some(at) => said[..at + "bytes)".len()].to_string(),
        None => said.trim_end_matches("</tool_use_error>").to_string(),
    }
}

/// What a line of the CLI's stream says the agent is doing: a tool it uses with what it uses it on, the start of
/// what it says, or a compaction; none for anything else.
pub fn activity_of(line: &str) -> Option<String> {
    activity_of_value(&serde_json::from_str(line).ok()?)
}

/// What a tool's input says it is used on.
fn subject(input: &Value) -> Option<&str> {
    ["command", "file_path", "path", "pattern", "description", "url", "query"]
        .iter()
        .find_map(|k| input[*k].as_str())
}

/// A text on one line, its start only when it is long.
fn cut_line(s: &str) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > 160 {
        format!("{}…", one.chars().take(160).collect::<String>())
    } else {
        one
    }
}

/// What a parsed line of the CLI's stream says the agent is doing.
fn activity_of_value(v: &Value) -> Option<String> {
    match v["type"].as_str()? {
        "assistant" => {
            let said: Vec<String> = v["message"]["content"]
                .as_array()?
                .iter()
                .filter_map(|c| match c["type"].as_str()? {
                    "tool_use" => {
                        let name = c["name"].as_str().unwrap_or("a tool");
                        Some(match subject(&c["input"]) {
                            Some(o) => format!("{name}: {}", cut_line(o)),
                            None => name.to_string(),
                        })
                    }
                    "text" => c["text"]
                        .as_str()
                        .filter(|t| !t.trim().is_empty())
                        .map(|t| format!("says: {}", cut_line(t))),
                    _ => None,
                })
                .collect();
            (!said.is_empty()).then(|| said.join("; "))
        }
        "system" if v["subtype"] == "compact_boundary" => Some("compacting the session".to_string()),
        _ => None,
    }
}

/// The schema the CLI is given for every round of a line's agents, those of the graph's agents' cells on `line` and on
/// the lines forked together with it ([`forked_together`]), the same for every cell of them, since the CLI gives the
/// schema among the tools, which come first in the prompt, so that a schema that changed from one round to the next
/// leaves nothing of the session's prompt cache, and a fork whose schema is not its base's reads nothing of the base
/// from it, writing the whole base again (in a trial each matter forked from the architect's base read 31,414 tokens of
/// the 80,301 or 133,030 the base held). It is these lines' own, instead of the graph's, since a field every return of
/// some lines holds (`tell_owner`) is one another line's agents must not hand in, which the schema the CLI gives the
/// model would otherwise offer them. It holds a return's top-level fields: the short fields every return holds first,
/// each with only its type, and the field an agent's forms are under last, so that only closing brackets come after
/// the long answer; that field with the [`Shape`] of every form of every agent's cell of the lines taken together
/// ([`Shape::merged`]), each object with its fields and each list with its items, down to every value. It refuses what
/// no agent's cell of the lines takes, as far as types and fields tell: an object requires the fields every one of its
/// alternatives requires, at the top those every agent's cell of the lines requires, and a
/// field some forms leave out unless the return is one of those forms ([`chained`]: `progress`, which `tools`
/// and `search` leave out, so that a return of the architect's `B4` without it, which a trial's engine sent back, is
/// refused by the command line); a field the cells of some of the lines hold and require and those of the others do not
/// hold (`tell_owner`, which the base and each matter's session hand in and the second eye does not) is required unless
/// the return is a form the others hand in, and refused beside a form only they hand in, so that the second eye's
/// `check` with `tell_owner` is refused by the command line, while `tools` with it or without it is left to the engine's
/// check of the cell's own schema; and it takes no field beyond those it lists when none of its alternatives does, the
/// field of the forms included, so that a field written inside the form it does not belong to is refused, and a field
/// every return holds written inside the field of the forms is too. A union below the field of the forms, whose
/// branches fix one field to a value of their own (a deviation of the kind `surprise` or `workaround`), keeps its
/// branches in the schema ([`crate::shape::Split`]): a value that fixes the field requires what its branch requires and takes no
/// field the branch does not take, so that `why` written in a `surprise` is refused naming it, where the fields of all
/// branches taken together accept it. Each form that has a field no other form of the
/// line has is told apart by it, and then requires what that form requires and takes no field it does not take
/// ([`told_apart`]), so that a return of a cell's own answer without a field the answer requires is refused naming it,
/// as `why` of the engineer's `D2`, which no other form has and which the shape of every form taken together cannot
/// require. A form all of whose fields other forms have too, such as the engineer's `I1`, whose fields `T6`'s form has
/// as well, `T5`, whose `columns` `T4`'s has, or the architect's `U1`, whose `notebook` most of its forms have, is told
/// apart by no field of its own and is checked by the shape of every form taken together alone: which form a return of
/// it is cannot be told from its fields, and a rule read from the fields it lacks would be a guess. The command line
/// refuses such a return saying where and why, and the model hands it in again within the run, one more call that
/// rewrites the return, where the engine would send it back and run the round again: in a trial one round handed in no
/// `tell_owner`, and another wrote `effects` inside `take` instead of beside it. When the command line gives up after
/// its attempts, the engine takes the last as any return (`unparsed` in the engine), moving a field every return holds
/// up from inside the field of the forms. A field its object may leave out may also be null, which the engine takes as
/// the field left out. Without the types the model hands in each field as the JSON text of its value, and with the
/// field of the forms an object with no fields it handed in the whole form as a string holding its JSON.
pub fn envelope(g: &flow::CompiledGraph, line: Option<&str>) -> Value {
    let lines = forked_together(g, line);
    let cells: Vec<&flow::CompiledCell> = g
        .cells
        .iter()
        .filter(|c| c.kind == "agent" && lines.contains(&c.line))
        .collect();
    let schemas: Vec<&Value> = cells.iter().filter_map(|c| c.schema.as_ref()).collect();
    // The fields beside the forms', of every agent's cell of the line, in the order the cells give them.
    let mut props = serde_json::Map::new();
    for (k, v) in schemas
        .iter()
        .filter_map(|s| s.get("properties")?.as_object())
        .flatten()
    {
        if *k != g.within && !props.contains_key(k) {
            props.insert(
                k.clone(),
                json!({ "type": v.get("type").cloned().unwrap_or(Value::Null) }),
            );
        }
    }
    let beside: Vec<String> = props.keys().cloned().collect();
    let desc = format!(
        "The form this round hands in, an object whose fields are written as JSON values, never a string holding the \
         JSON text of one: exactly one of the forms `format:common` lists, the cell's own answer as `format:<cell>` \
         gives it or another form it names; {} sit beside it, not inside it",
        beside.join(" and ")
    );
    let forms = schemas
        .iter()
        .filter_map(|s| s.get("properties")?.get(&g.within))
        .map(Shape::of)
        .reduce(|a, b| a.merged(&b))
        .map(|f| f.schema());
    let mut within = serde_json::Map::new();
    within.insert("type".into(), json!("object"));
    within.insert("description".into(), json!(desc));
    for k in ["properties", "required", "additionalProperties"] {
        if let Some(v) = forms.as_ref().and_then(|f| f.get(k)) {
            within.insert(k.into(), v.clone());
        }
    }
    let mut rules: Vec<Value> = forms
        .as_ref()
        .and_then(|f| f.get("allOf")?.as_array())
        .cloned()
        .unwrap_or_default();
    rules.extend(told_apart(&forms_of(&schemas, &g.within)));
    if !rules.is_empty() {
        within.insert("allOf".into(), Value::Array(rules));
    }
    props.insert(g.within.clone(), Value::Object(within));
    let required: Vec<String> = props
        .keys()
        .filter(|k| **k == g.within || schemas.iter().all(|s| required_of(s).contains(&k.as_str())))
        .cloned()
        .collect();
    let mut out = json!({
        "type": "object",
        "properties": props,
        "required": required,
        "additionalProperties": false,
    });
    // The fields some forms leave out, as the engine checks, grouped by the forms that leave them out: a field every
    // agent's cell of the lines has and requires of its other forms is left out by the forms that say so; a field the
    // cells of some of the lines have and require and those of the others do not have is left out by every form the
    // others take, and refused beside a form only they take.
    let mut left: BTreeMap<Vec<String>, Vec<String>> = BTreeMap::new();
    let mut refused: BTreeMap<Vec<String>, Vec<String>> = BTreeMap::new();
    for k in beside.iter().filter(|k| !required_of(&out).contains(&k.as_str())) {
        let has = |s: &Value| s.get("properties").is_some_and(|p| p.get(k.as_str()).is_some());
        let (having, lacking): (Vec<&Value>, Vec<&Value>) = schemas.iter().copied().partition(|&s| has(s));
        if lacking.is_empty() {
            let forms: BTreeSet<String> = cells
                .iter()
                .flat_map(|c| &c.forms)
                .filter(|f| f.without.contains(k))
                .filter_map(|f| f.field.clone())
                .collect();
            if !forms.is_empty() {
                left.entry(forms.into_iter().collect()).or_default().push(k.clone());
            }
        } else if having.iter().all(|s| required_of(s).contains(&k.as_str())) {
            let (theirs, others) = (taken_under(&lacking, &g.within), taken_under(&having, &g.within));
            if !theirs.is_empty() {
                left.entry(theirs.iter().cloned().collect())
                    .or_default()
                    .push(k.clone());
            }
            let only: Vec<String> = theirs.difference(&others).cloned().collect();
            if !only.is_empty() {
                refused.entry(only).or_default().push(k.clone());
            }
        }
    }
    // A return holds each field of a group unless its form is one of the group's; and beside a form only the cells
    // that lack some fields take, it holds none of those.
    let one_of = |forms: &[String]| -> Vec<Value> {
        forms
            .iter()
            .map(|f| json!({ "type": "object", "required": [f] }))
            .collect()
    };
    let mut rules: Vec<(Value, Value)> = left
        .iter()
        .map(|(forms, fields)| {
            (
                json!({ "properties": { &g.within: { "type": "object", "not": { "anyOf": one_of(forms) } } } }),
                json!({ "required": fields }),
            )
        })
        .collect();
    for (forms, fields) in &refused {
        let kept: serde_json::Map<String, Value> = beside
            .iter()
            .filter(|k| !fields.contains(k))
            .chain([&g.within])
            .map(|k| (k.clone(), json!({})))
            .collect();
        rules.push((
            json!({ "required": [&g.within],
                    "properties": { &g.within: { "type": "object", "anyOf": one_of(forms) } } }),
            json!({ "properties": kept, "additionalProperties": false }),
        ));
    }
    if let (Some(rule), Some(o)) = (chained(&rules), out.as_object_mut()) {
        o.extend(rule);
    }
    out
}

/// The lines whose sessions share with a session of `line` all that comes before their messages in the prompt: the
/// line a session of it forks from in the end (the line its sessions fork from, and the line that one's fork from, and
/// so on), and every line whose sessions fork from that one in the end, `line` among them, in the order the agents'
/// cells of the graph give them; `line` alone for a line no session forks from or to, and for a cell on no line. A fork
/// holds its base's session and reads it from the prompt cache only where its tools, the schema the command line gives
/// among them, and its system prompt are the base's: so these lines are given one schema ([`envelope`]), and a fork
/// carries its base's system prompt (`crate::system`).
pub fn forked_together(g: &flow::CompiledGraph, line: Option<&str>) -> Vec<Option<String>> {
    let agents = || g.cells.iter().filter(|c| c.kind == "agent");
    // The line a line's sessions fork from in the end; itself for one that forks from none.
    let base = |line: &str| -> String {
        let mut at = line.to_string();
        let mut seen = vec![at.clone()];
        while let Some(up) = agents()
            .find(|c| c.line.as_deref() == Some(at.as_str()))
            .and_then(|c| c.fork_of.clone())
        {
            if seen.contains(&up) {
                break;
            }
            seen.push(up.clone());
            at = up;
        }
        at
    };
    let Some(root) = line.map(base) else {
        return vec![None];
    };
    let mut out: Vec<Option<String>> = Vec::new();
    for c in agents() {
        if let Some(l) = c.line.as_deref()
            && !out.contains(&c.line)
            && base(l) == root
        {
            out.push(c.line.clone());
        }
    }
    if out.is_empty() {
        out.push(line.map(str::to_string));
    }
    out
}

/// The fields under the field of the forms that any of these cells' schemas takes: each form's field, and those beside
/// it in the form, as `notebook`.
fn taken_under(schemas: &[&Value], within: &str) -> BTreeSet<String> {
    schemas
        .iter()
        .filter_map(|s| s.get("properties")?.get(within))
        .flat_map(|w| {
            Shape::of(w)
                .field_names()
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Rules at the top of the schema, each an `if` and its `then`, as one: the first's `if` and `then`, with the rest in its
/// `then` and in its `else`, since the API takes no `anyOf` or `allOf` at the top of the schema of the tool a return is
/// handed in with (as tried with 2.1.312: "input_schema does not support oneOf, allOf, or anyOf at the top level"). An
/// object a keyword reads says it is one, which the command line's strict check otherwise warns of. None for no rule.
fn chained(rules: &[(Value, Value)]) -> Option<serde_json::Map<String, Value>> {
    let ((when, then), rest) = rules.split_first()?;
    let rest = chained(rest);
    let mut then = then.clone();
    let mut rule = serde_json::Map::new();
    rule.insert("if".into(), when.clone());
    if let Some(r) = rest {
        if let Some(t) = then.as_object_mut() {
            t.extend(r.clone());
        }
        rule.insert("else".into(), Value::Object(r));
    }
    rule.insert("then".into(), then);
    Some(rule)
}

/// Every form of the agents' cells of a graph, each as its [`Shape`], in the order the cells give them: each of the
/// alternatives of a cell's field of the forms, those with the same fields taken together as one ([`Shape::merged`]),
/// as the `tools` of cells that may run different tools.
fn forms_of(schemas: &[&Value], within: &str) -> Vec<Shape> {
    let mut out: Vec<(Vec<String>, Shape)> = Vec::new();
    for s in schemas.iter().filter_map(|s| s.get("properties")?.get(within)) {
        let alts: Vec<&Value> = match ["anyOf", "oneOf"].iter().find_map(|k| s.get(*k)?.as_array()) {
            Some(xs) => xs.iter().collect(),
            None => vec![s],
        };
        for a in alts {
            let form = Shape::of(a);
            let mut fields: Vec<String> = form.field_names().into_iter().map(str::to_string).collect();
            fields.sort();
            match out.iter_mut().find(|(f, _)| *f == fields) {
                Some((_, x)) => *x = x.merged(&form),
                None => out.push((fields, form)),
            }
        }
    }
    out.into_iter().map(|(_, f)| f).collect()
}

/// For each form with a field no other form has, the rule that a value of the field of the forms holding one of those
/// fields is that form: an `if` that it does, and a `then` with the fields the form requires and, when it takes no
/// other, the fields it takes; in an `allOf` inside the field of the forms, where the API takes it. The command line
/// refuses a return of that form without a field it requires naming the field ("/outcome: must have required property
/// 'why', /outcome: must match \"then\" schema", as tried with 2.1.312), where an `anyOf` of the forms is refused with
/// the faults of every form (some 150 for a return of `D2` without `why` in the engineer's graph). A form all of whose
/// fields another form has too has no rule. An object a keyword reads says it is one, as in [`unless_left_out`].
fn told_apart(forms: &[Shape]) -> Vec<Value> {
    let fields: Vec<Vec<&str>> = forms.iter().map(Shape::field_names).collect();
    let mut rules = Vec::new();
    for (i, form) in forms.iter().enumerate() {
        let own: Vec<&str> = fields[i]
            .iter()
            .copied()
            .filter(|k| fields.iter().enumerate().all(|(j, o)| j == i || !o.contains(k)))
            .collect();
        let has = |k: &&str| json!({ "type": "object", "required": [k] });
        let when = match own.as_slice() {
            [] => continue,
            [one] => has(one),
            _ => json!({ "type": "object", "anyOf": own.iter().map(has).collect::<Vec<_>>() }),
        };
        let mut then = serde_json::Map::new();
        then.insert("type".into(), json!("object"));
        if !form.required.is_empty() {
            then.insert("required".into(), json!(form.required));
        }
        if form.closed {
            let taken: serde_json::Map<String, Value> = fields[i].iter().map(|k| (k.to_string(), json!({}))).collect();
            then.insert("properties".into(), Value::Object(taken));
            then.insert("additionalProperties".into(), json!(false));
        }
        if then.len() > 1 {
            rules.push(json!({ "if": when, "then": then }));
        }
    }
    rules
}

/// Reads what `claude -p --output-format stream-json` printed: the compactions on the way, and the result at the end.
pub fn read_stream(text: &str, reply: &mut Reply) {
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match v["type"].as_str() {
            Some("system") if v["subtype"] == "compact_boundary" => reply.compactions += 1,
            Some("system") if v["subtype"] == "init" => {
                // The session exists from here on, whatever the round comes to.
                if reply.usage.is_null() {
                    reply.usage = json!({ "started": true });
                }
                if let Some(m) = v["model"].as_str() {
                    reply.model = Some(m.to_string());
                }
                if let Some(s) = v["session_id"].as_str() {
                    reply.session = s.to_string();
                }
            }
            Some("assistant") if reply.usage.get(FIRST_CALL).is_none() => {
                if let Some(first) = first_call(&v) {
                    if !reply.usage.is_object() {
                        reply.usage = json!({ "started": true });
                    }
                    reply.usage[FIRST_CALL] = first;
                }
            }
            Some("result") => {
                if let Some(s) = v["session_id"].as_str() {
                    reply.session = s.to_string();
                }
                let first = reply.usage.get(FIRST_CALL).cloned();
                reply.usage = json!({
                    "usage": v["usage"],
                    "session_cost_usd": v["total_cost_usd"],
                    "turns": v["num_turns"],
                    "duration_ms": v["duration_ms"],
                    "models": v["modelUsage"],
                });
                if let Some(f) = first {
                    reply.usage[FIRST_CALL] = f;
                }
                if v["is_error"] == true || v["subtype"] != "success" {
                    // A result that failed on the CLI's side, such as running out of retries, has no text: what went
                    // wrong is in its errors.
                    let errors: Vec<&str> = v["errors"]
                        .as_array()
                        .map(|xs| xs.iter().filter_map(Value::as_str).collect())
                        .unwrap_or_default();
                    let why = v["result"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .or_else(|| (!errors.is_empty()).then(|| errors.join("; ")))
                        .unwrap_or_else(|| "the CLI gave no reason".to_string());
                    // The subtype names what failed on the CLI's side; an error the CLI reports under "success", as it
                    // reports an API it cannot reach, is said by its reason alone.
                    reply.error = Some(match v["subtype"].as_str() {
                        Some("success") => why,
                        k => format!("{}: {why}", k.unwrap_or("error")),
                    });
                }
                reply.result = v["result"].as_str().map(str::to_string);
                let structured = v.get("structured_output").filter(|s| !s.is_null()).cloned();
                (reply.output, reply.output_from) = match structured {
                    Some(s) => (Some(s), Some(Source::Tool)),
                    None => match v["result"].as_str().and_then(json_in) {
                        Some(s) => (Some(s), Some(Source::Text)),
                        None => (None, None),
                    },
                };
            }
            _ => {}
        }
    }
}

/// Where a run's usage record keeps what its first call to the model read from the cache and wrote to it. The result
/// the CLI reports at the end gives the tokens of the whole run and of its last call only; whether a resumed session
/// was read from the cache shows in its first call alone, since every later call reads what the calls before it wrote.
pub const FIRST_CALL: &str = "first_call";

/// What a message of the stream says of the call that wrote it, when it is the agent's own message from the model: its
/// input tokens, those it read from the cache and those it wrote to it. None for a subagent's message and for a message
/// the CLI makes itself, such as the one it puts in when it resumes a session, which called no model.
fn first_call(v: &Value) -> Option<Value> {
    let m = &v["message"];
    if !v["parent_tool_use_id"].is_null() || m["model"] == "<synthetic>" {
        return None;
    }
    let u = &m["usage"];
    let n = |k: &str| u[k].as_i64();
    let (input, read, written) = (
        n("input_tokens")?,
        n("cache_read_input_tokens")?,
        n("cache_creation_input_tokens")?,
    );
    (input + read + written > 0).then(|| {
        json!({
            "input_tokens": input,
            "cache_read_input_tokens": read,
            "cache_creation_input_tokens": written,
        })
    })
}

/// The JSON object a text holds: the whole text, or the first fenced block, or from the first brace to the last.
fn json_in(text: &str) -> Option<Value> {
    let t = text.trim();
    if let Ok(v) = serde_json::from_str::<Value>(t) {
        return v.is_object().then_some(v);
    }
    if let Some(start) = t.find("```") {
        let rest = &t[start + 3..];
        let rest = rest.strip_prefix("json").unwrap_or(rest);
        if let Some(end) = rest.find("```")
            && let Ok(v) = serde_json::from_str::<Value>(rest[..end].trim())
        {
            return Some(v);
        }
    }
    let (a, b) = (t.find('{')?, t.rfind('}')?);
    serde_json::from_str(&t[a..=b]).ok()
}

/// The kinds of run that end without a return, as the flow's rounds section names them, and what tells each apart in
/// what the CLI reports. The first kind whose words the report holds is the kind; a report that holds none of them is
/// a passing failure.
const KINDS: [(&str, &[&str]); 6] = [
    // First, since what the command line reports of it may quote the agent's return, which may hold any words.
    ("unparsed", &["max_structured_output_retries"]),
    // The service's own words when its safety classifier stops a run or refuses its message.
    ("flagged", &["safeguards flagged", "stopped by a safety classifier"]),
    (
        "usage_limit",
        &[
            "usage limit",
            "hit your limit",
            "limit reached",
            "limit will reset",
            "out of extra usage",
        ],
    ),
    (
        "cannot_run",
        &[
            "not logged in",
            "/login",
            "invalid api key",
            "oauth token",
            "token has expired",
            "authentication_error",
            "authentication failed",
            "no such file or directory",
            "command not found",
        ],
    ),
    (
        "prompt_too_long",
        &["prompt is too long", "prompt too long", "context length"],
    ),
    (
        "offline",
        &[
            "enotfound",
            "eai_again",
            "enetunreach",
            "econnrefused",
            "getaddrinfo",
            "network is unreachable",
            "connection error",
            "unable to connect",
            "could not resolve host",
        ],
    ),
];

/// The kind of a run that ended without a return, by what the CLI reported: one of the flow's kinds of failed run.
pub fn failure_kind(report: &str) -> &'static str {
    let t = report.to_lowercase();
    KINDS
        .iter()
        .find(|(_, words)| words.iter().any(|w| t.contains(w)))
        .map_or("transient", |(k, _)| k)
}

/// When a usage limit the CLI reported resets, in milliseconds: from the time it gives, as seconds since the epoch
/// after a bar (`limit reached|1760000000`), or as a time of day after "resets" (`resets 5pm`, `resets 11:30am
/// (Europe/Paris)`), the next time the clock shows it there. `offset` gives a time zone's distance from UTC in seconds,
/// the local one for none. None when the report gives no time it can read.
pub fn resets_at(report: &str, now: i64, offset: &dyn Fn(Option<&str>) -> Option<i64>) -> Option<i64> {
    if let Some((_, rest)) = report.split_once('|') {
        let digits: String = rest.trim().chars().take_while(char::is_ascii_digit).collect();
        if let Ok(n) = digits.parse::<i64>()
            && digits.len() >= 9
        {
            return Some(if digits.len() >= 13 { n } else { n * 1000 });
        }
    }
    let lower = report.to_lowercase();
    let after = &lower[lower.find("resets")? + "resets".len()..];
    let after = after.trim_start().trim_start_matches("at ").trim_start();
    // The time of day: the first run of digits, with minutes after a colon, and am or pm after it.
    let start = after.find(|c: char| c.is_ascii_digit())?;
    let rest = &after[start..];
    let hour_text: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let mut hour: i64 = hour_text.parse().ok()?;
    let rest = &rest[hour_text.len()..];
    let (minute, rest) = match rest.strip_prefix(':') {
        Some(r) => {
            let m: String = r.chars().take_while(char::is_ascii_digit).collect();
            (m.parse::<i64>().ok()?, &r[m.len()..])
        }
        None => (0, rest),
    };
    let rest = rest.trim_start();
    if rest.starts_with("pm") && hour < 12 {
        hour += 12;
    } else if rest.starts_with("am") && hour == 12 {
        hour = 0;
    }
    if hour > 23 || minute > 59 {
        return None;
    }
    // A time zone named in brackets, as the CLI names it, with its original case.
    let zone = report
        .rfind('(')
        .and_then(|i| report[i + 1..].split_once(')').map(|(z, _)| z.trim()))
        .filter(|z| z.contains('/'));
    let off = offset(zone)?;
    let local = now / 1000 + off;
    let day = local - local.rem_euclid(86_400);
    let mut at = day + hour * 3600 + minute * 60;
    if at <= local {
        at += 86_400;
    }
    Some((at - off) * 1000)
}

/// A time zone's distance from UTC in seconds, as `date` gives it there; the local one for none.
pub fn zone_offset(zone: Option<&str>) -> Option<i64> {
    let mut c = Command::new("date");
    c.arg("+%z");
    if let Some(z) = zone {
        c.env("TZ", z);
    }
    let out = c.output().ok()?;
    let t = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let (sign, digits) = match t.split_at_checked(1)? {
        ("-", d) => (-1, d),
        ("+", d) => (1, d),
        _ => return None,
    };
    let h: i64 = digits.get(..2)?.parse().ok()?;
    let m: i64 = digits.get(2..4)?.parse().ok()?;
    Some(sign * (h * 3600 + m * 60))
}

/// A new session id: a random version 4 UUID.
pub fn new_session_id() -> String {
    let mut b = [0u8; 16];
    let read = std::fs::File::open("/dev/urandom").and_then(|mut f| std::io::Read::read_exact(&mut f, &mut b));
    if read.is_err() {
        let t = db::now() as u128 ^ (u128::from(std::process::id()) << 64);
        b = t.to_le_bytes();
    }
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stream_gives_the_result_the_session_and_the_compactions() {
        let text = [
            r#"{"type":"system","subtype":"init","session_id":"s1","model":"claude-opus-5-5"}"#,
            r#"{"type":"assistant","message":{}}"#,
            r#"{"type":"system","subtype":"compact_boundary"}"#,
            r#"{"type":"result","subtype":"success","is_error":false,"result":"done","session_id":"s1","total_cost_usd":0.5,"usage":{"input_tokens":3},"structured_output":{"outcome":{},"progress":"p","side_reports":[]}}"#,
        ]
        .join("\n");
        let mut r = Reply::default();
        read_stream(&text, &mut r);
        assert_eq!(r.session, "s1");
        assert_eq!(r.model.as_deref(), Some("claude-opus-5-5"));
        assert_eq!(r.compactions, 1);
        assert_eq!(r.output.unwrap()["progress"], "p");
        assert_eq!(r.usage["session_cost_usd"], 0.5);
        assert!(r.error.is_none());
    }

    // The usage record keeps what the run's first call to the model read from the cache and wrote to it, which the
    // result does not tell: the message the CLI puts in when it resumes a session and a subagent's message are passed
    // over, and so is every call after the first.
    #[test]
    fn the_usage_record_keeps_the_first_call_of_the_run() {
        let called = |model: &str, parent: Value, read: i64, written: i64| {
            let input = if read + written > 0 { 4 } else { 0 };
            json!({ "type": "assistant", "parent_tool_use_id": parent, "message": { "model": model, "usage": {
                "input_tokens": input, "cache_read_input_tokens": read, "cache_creation_input_tokens": written,
                "output_tokens": 9,
            } } })
            .to_string()
        };
        let text = [
            r#"{"type":"system","subtype":"init","session_id":"s1","model":"m"}"#.to_string(),
            called("<synthetic>", Value::Null, 0, 0),
            called("m", json!("toolu_1"), 0, 900),
            called("m", Value::Null, 32_208, 18_304),
            called("m", Value::Null, 50_512, 700),
            r#"{"type":"result","subtype":"success","result":"{}","session_id":"s1","total_cost_usd":0.5,"usage":{"cache_read_input_tokens":82720,"iterations":[{"cache_read_input_tokens":50512}]}}"#.to_string(),
        ]
        .join("\n");
        let mut r = Reply::default();
        read_stream(&text, &mut r);
        assert_eq!(
            r.usage[FIRST_CALL],
            json!({ "input_tokens": 4, "cache_read_input_tokens": 32_208, "cache_creation_input_tokens": 18_304 })
        );
        assert_eq!(r.usage["usage"]["cache_read_input_tokens"], 82_720);
    }

    // The record keeps of the engine's own environment the variables the CLI reads, and a secret only by its digest.
    #[test]
    fn the_record_keeps_the_inherited_variables_the_cli_reads() {
        let vars = [
            ("PATH", "/bin"),
            ("CLAUDE_CODE_ENTRYPOINT", "cli"),
            ("ANTHROPIC_API_KEY", "sk-secret"),
            ("DISABLE_PROMPT_CACHING", "1"),
            ("HOME", "/h"),
        ]
        .map(|(k, v)| (OsString::from(k), OsString::from(v)));
        let kept = read_by_cli(vars);
        assert_eq!(
            kept.keys().collect::<Vec<_>>(),
            ["ANTHROPIC_API_KEY", "CLAUDE_CODE_ENTRYPOINT", "DISABLE_PROMPT_CACHING"]
        );
        assert_eq!(kept["CLAUDE_CODE_ENTRYPOINT"], "cli");
        let key = &kept["ANTHROPIC_API_KEY"];
        assert!(
            key.starts_with("digest ") && !key.contains("sk-secret") && key.len() == 23,
            "{key}"
        );
    }

    fn compiled() -> flow::Compiled {
        let relations = db::relations(db::SCHEMA).unwrap();
        let tables = db::index_of(db::SCHEMA).unwrap();
        flow::compile(flow::FLOW, &flow::Context { relations, tables }).unwrap()
    }

    /// The lines of a graph's agents' cells, each once, in the order the cells give them.
    fn lines_of(g: &flow::CompiledGraph) -> Vec<Option<&str>> {
        let mut out: Vec<Option<&str>> = Vec::new();
        for c in g.cells.iter().filter(|c| c.kind == "agent") {
            if !out.contains(&c.line.as_deref()) {
                out.push(c.line.as_deref());
            }
        }
        out
    }

    // The CLI is given one schema for every cell of a line: the short fields every return of the line holds first, the
    // field its forms are under last, with the shape of every form of the line. Required are the fields every agent's
    // cell of the line requires, so not `progress`, which the form `tools` leaves out, and the field of the forms takes
    // no field beyond those of the forms, so a field beside it written inside it is refused. Nothing a cell's own
    // schema takes is refused. Only the engineer and the architect tell the owner: the lines that help them have no
    // `tell_owner` to hand in.
    #[test]
    fn every_cell_of_a_line_gives_the_cli_the_same_schema() {
        let f = compiled();
        let g = f.graph("engineer").unwrap();
        let s = envelope(g, Some("engineer"));
        let keys = |s: &Value| -> Vec<String> { s["properties"].as_object().unwrap().keys().cloned().collect() };
        assert_eq!(
            keys(&s),
            ["tell_owner", "progress", "side_reports", "flow_feedback", "outcome"]
        );
        assert_eq!(s["properties"][&g.within]["type"], "object");
        assert!(
            s["properties"][&g.within]["description"]
                .as_str()
                .unwrap()
                .contains("side_reports")
        );
        assert_eq!(
            s["required"],
            json!(["tell_owner", "side_reports", "flow_feedback", "outcome"])
        );
        assert_eq!(s["properties"][&g.within]["additionalProperties"], false);
        // The reader of the history asks for no tools, so nothing it hands in leaves out `progress`.
        let reader = envelope(g, Some("librarian"));
        assert_eq!(
            reader["required"],
            json!(["progress", "side_reports", "flow_feedback", "outcome"])
        );
        for line in ["review", "adversarial", "advisor"] {
            let h = envelope(g, Some(line));
            assert_eq!(
                keys(&h),
                ["progress", "side_reports", "flow_feedback", "outcome"],
                "{line}"
            );
            assert_eq!(
                h["required"],
                json!(["side_reports", "flow_feedback", "outcome"]),
                "{line}"
            );
        }
        let a = f.graph("architect").unwrap();
        // The base and each matter's session tell the owner; the second eye and the search of observations do not. The
        // second eye shares one schema with the base and each matter's session, which fork the same base: it offers
        // `tell_owner` and requires it of no form the second eye hands in too.
        for line in ["architect", "matter", "second_eye"] {
            let arch = envelope(a, Some(line));
            assert_eq!(
                keys(&arch),
                ["tell_owner", "progress", "flow_feedback", "outcome"],
                "{line}"
            );
            assert_eq!(arch["required"], json!(["flow_feedback", "outcome"]), "{line}");
        }
        // The search of observations asks for no tools, so nothing it hands in leaves out `progress`.
        let h = envelope(a, Some("observer"));
        assert_eq!(keys(&h), ["progress", "flow_feedback", "outcome"]);
        assert_eq!(h["required"], json!(["progress", "flow_feedback", "outcome"]));
        // A project that does not ask for flow feedback gives its agents no such field.
        let off = envelope(&g.clone().as_asked(|_| false), Some("engineer"));
        assert!(off["properties"].get("flow_feedback").is_none(), "{off}");
        assert_eq!(off["required"], json!(["tell_owner", "side_reports", "outcome"]));
        // The field of the forms is never an object with no fields: it holds every form of every agent's cell of the
        // line, each value with its type, which a string holding the JSON of one does not have.
        for g in &f.graphs {
            for line in lines_of(g) {
                let s = envelope(g, line);
                let within = &s["properties"][&g.within];
                assert!(
                    within["properties"].as_object().is_some_and(|p| !p.is_empty()),
                    "{within}"
                );
                for c in g
                    .cells
                    .iter()
                    .filter(|c| c.kind == "agent" && c.line.as_deref() == line)
                {
                    let schema = c.schema.as_ref().unwrap();
                    let form = Shape::of(&schema["properties"][&g.within]).schema();
                    assert_eq!(beyond(&form, within, ""), None, "cell {}", c.id);
                    for k in schema["properties"].as_object().unwrap().keys() {
                        assert!(s["properties"].get(k).is_some(), "cell {}: {k}", c.id);
                    }
                    for k in required_of(&s) {
                        assert!(required_of(schema).contains(&k), "cell {}: {k}", c.id);
                    }
                }
            }
        }
    }

    // A fork reads its base's session from the prompt cache only where its tools are the base's, and the command line
    // gives the schema of the return among them: in a trial each matter's session, forked from the architect's base with
    // a schema of its own, read 31,414 tokens of a base that held 80,301 or 133,030, and wrote the rest again. The lines
    // that fork from one another are given one schema, made from the cells of them all, and every other line keeps its
    // own: on the architect's, the base, each matter's session and the second eye share one, and the search of
    // observations has its own.
    #[test]
    fn a_fork_is_given_the_schema_of_the_session_it_forks() {
        let f = compiled();
        let mut forks = 0;
        for g in &f.graphs {
            for c in g.cells.iter().filter(|c| c.kind == "agent") {
                if let Some(base) = &c.fork_of {
                    assert_eq!(
                        envelope(g, c.line.as_deref()).to_string(),
                        envelope(g, Some(base)).to_string(),
                        "graph {}, cell {}",
                        g.name,
                        c.id
                    );
                    forks += 1;
                }
            }
        }
        assert!(forks > 0);
        let a = f.graph("architect").unwrap();
        let line = |l: &str| Some(l.to_string());
        let together = forked_together(a, Some("matter"));
        for l in ["architect", "matter", "second_eye"] {
            assert!(together.contains(&line(l)), "{l}: {together:?}");
            assert_eq!(forked_together(a, Some(l)), together, "{l}");
        }
        assert_eq!(forked_together(a, Some("observer")), [line("observer")]);
        assert_eq!(forked_together(a, None), [None]);
        let g = f.graph("engineer").unwrap();
        assert_eq!(forked_together(g, Some("engineer")), [line("engineer")]);
    }

    // The base and each matter's session tell the owner and the second eye does not, and the three share one schema:
    // it requires `tell_owner` of every form but those the second eye hands in too (the base's `plan` among them, below),
    // and refuses it beside the second eye's own `check`, as the command line did when the second eye had a schema of
    // its own. A form the second eye hands in too, as `tools`, is taken with it and without it, and the engine's check of
    // the cell's own schema tells which the cell needs.
    #[test]
    fn a_field_some_lines_of_one_schema_hold_is_required_or_refused_by_the_form() {
        let f = compiled();
        let g = f.graph("architect").unwrap();
        let s = envelope(g, Some("second_eye"));
        let schema = g.cell("G2").unwrap().schema.clone().unwrap();
        let mut check = json!({
            "progress": "Looking at what the change loses",
            "flow_feedback": "none",
            "outcome": { "check": { "verdict": "sound", "why": "Nothing it holds is lost." } },
        });
        assert!(flow::valid(&schema, &check), "{check}");
        assert_eq!(refused(&s, &check, ""), None);
        check["tell_owner"] = json!([]);
        assert!(!flow::valid(&schema, &check));
        assert_eq!(
            refused(&s, &check, "").as_deref(),
            Some("root: must NOT have additional properties ('tell_owner' is not allowed)")
        );
        let tools = json!({
            "flow_feedback": "none",
            "outcome": { "tools": [{ "tool": "query", "sql": "SELECT id FROM items" }] },
        });
        assert_eq!(refused(&s, &tools, ""), None);
        let mut told = tools.clone();
        told["tell_owner"] = json!([]);
        assert_eq!(refused(&s, &told, ""), None);
    }

    // A round whose effort turns thinking on or off reads nothing of its session from the cache, which the flow's check
    // keeps from happening within a line: a fork's first round whose effort does so against its base's last reads
    // nothing of the base, so the lines forked together run at `low`, which does not think, all or none.
    #[test]
    fn the_lines_forked_together_think_alike() {
        let f = compiled();
        for g in &f.graphs {
            for line in lines_of(g) {
                let lines = forked_together(g, line);
                let low: BTreeSet<bool> = g
                    .cells
                    .iter()
                    .filter(|c| c.kind == "agent" && lines.contains(&c.line))
                    .filter_map(|c| c.effort.as_deref())
                    .map(|e| e == "low")
                    .collect();
                assert!(low.len() <= 1, "graph {}, lines {lines:?}", g.name);
            }
        }
    }

    /// Where a return does not fit the envelope, as the command line checks it and in its words: a value of a type the
    /// schema does not name or other than the one it fixes, a field required and missing, or a field its object takes no
    /// more of, and what the `then` or the `else` of an `if` requires; none when it fits. A value fits `not` when it
    /// does not fit what it holds, `anyOf` when it fits one of them, and `allOf` when it fits each.
    fn refused(s: &Value, x: &Value, at: &str) -> Option<String> {
        let here = if at.is_empty() { "root" } else { at };
        if s.get("const").is_some_and(|c| c != x) {
            return Some(format!("{here}: must be equal to constant"));
        }
        if let Some(c) = s.get("if") {
            let branch = if refused(c, x, at).is_none() { "then" } else { "else" };
            if let Some(why) = s.get(branch).and_then(|b| refused(b, x, at)) {
                return Some(why);
            }
        }
        for each in s.get("allOf").and_then(Value::as_array).into_iter().flatten() {
            if let Some(why) = refused(each, x, at) {
                return Some(why);
            }
        }
        if s.get("not").is_some_and(|n| refused(n, x, at).is_none()) {
            return Some(format!("{here}: must NOT be valid"));
        }
        if let Some(alts) = s.get("anyOf").and_then(Value::as_array)
            && alts.iter().all(|a| refused(a, x, at).is_some())
        {
            return Some(format!("{here}: must match a schema in anyOf"));
        }
        if let Some(t) = s.get("type") {
            let kind = match x {
                Value::Null => "null",
                Value::Bool(_) => "boolean",
                Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
                Value::Number(_) => "number",
                Value::String(_) => "string",
                Value::Array(_) => "array",
                Value::Object(_) => "object",
            };
            let all = types(t);
            if !all.contains(&kind) && !(kind == "integer" && all.contains(&"number")) {
                return Some(format!("{here}: must be {}", all.join(",")));
            }
        }
        if let Some(o) = x.as_object() {
            if let Some(r) = required_of(s).into_iter().find(|r| !o.contains_key(*r)) {
                return Some(format!("{here}: must have required property '{r}'"));
            }
            for (k, v) in o {
                match s.get("properties").and_then(|p| p.get(k)) {
                    Some(p) => {
                        if let Some(why) = refused(p, v, &format!("{at}/{k}")) {
                            return Some(why);
                        }
                    }
                    None if closed(s) => {
                        return Some(format!(
                            "{here}: must NOT have additional properties ('{k}' is not allowed)"
                        ));
                    }
                    None => {}
                }
            }
        }
        if let (Some(it), Some(xs)) = (s.get("items"), x.as_array()) {
            for (i, v) in xs.iter().enumerate() {
                if let Some(why) = refused(it, v, &format!("{at}/{i}")) {
                    return Some(why);
                }
            }
        }
        None
    }

    // In a trial, a round of the architect's cell U1 handed in no `tell_owner`, which every return of the architect's
    // line holds, and the engine sent the return back, which cost a whole round. The command line now refuses it, saying
    // what is missing, as the installed one did when tried (2.1.312), and the agent hands it in again within its run.
    // The search of observations, a line that tells the owner nothing, has it refused.
    #[test]
    fn a_return_without_a_field_every_return_holds_is_refused_by_the_command_line() {
        let f = compiled();
        let g = f.graph("architect").unwrap();
        let schema = g.cell("U1").unwrap().schema.clone().unwrap();
        let s = envelope(g, Some("architect"));
        let mut ret = json!({
            "progress": "Reading where each line stands",
            "flow_feedback": "",
            "outcome": { "understanding": "The parser: half done." },
        });
        assert!(!flow::valid(&schema, &ret));
        assert_eq!(
            refused(&s, &ret, "").as_deref(),
            Some("root: must have required property 'tell_owner'")
        );
        ret["tell_owner"] = json!([]);
        assert!(flow::valid(&schema, &ret), "{ret}");
        assert_eq!(refused(&s, &ret, ""), None);
        // Written inside the field of the forms, it is refused there too.
        ret["outcome"]["tell_owner"] = json!([]);
        assert_eq!(
            refused(&s, &ret, "").as_deref(),
            Some("/outcome: must NOT have additional properties ('tell_owner' is not allowed)")
        );
        let observer = envelope(g, Some("observer"));
        let search = json!({
            "tell_owner": [],
            "progress": "Found none",
            "flow_feedback": "none",
            "outcome": { "candidates": [] },
        });
        assert_eq!(
            refused(&observer, &search, "").as_deref(),
            Some("root: must NOT have additional properties ('tell_owner' is not allowed)")
        );
    }

    // In a trial, a round of the architect's cell that takes a report in wrote `effects` inside `take`, beside `way` and `why`, instead of
    // beside `take`, and the engine sent the return back, which cost a whole round. The command line now refuses it,
    // saying where, as the installed one did when tried (2.1.312), where the agent moved it beside `take`.
    #[test]
    fn a_field_written_inside_a_form_it_does_not_belong_to_is_refused_by_the_command_line() {
        let f = compiled();
        let g = f.graph("architect").unwrap();
        let schema = g.cell("H4").unwrap().schema.clone().unwrap();
        let s = envelope(g, Some("matter"));
        let finding = json!({ "what": "A slow parser.", "grounds": "Ran it.", "sure": "checked" });
        let take = json!({ "way": "nothing", "why": "Nothing in it to take in" });
        let mut inside = take.clone();
        inside["effects"] = json!({});
        let mut ret = json!({
            "tell_owner": [],
            "progress": "Taking in an observation",
            "flow_feedback": "",
            "outcome": { "finding": finding, "take": inside },
        });
        assert!(!flow::valid(&schema, &ret));
        assert_eq!(
            refused(&s, &ret, "").as_deref(),
            Some("/outcome/take: must NOT have additional properties ('effects' is not allowed)")
        );
        ret["outcome"] = json!({ "finding": finding, "take": take, "effects": {} });
        assert!(flow::valid(&schema, &ret), "{ret}");
        assert_eq!(refused(&s, &ret, ""), None);
    }

    // In the sixth trial a return of the architect's cell that rethought a batch handed in no `progress`, which every
    // return holds but one of the forms that leave it out, `tools` and `search`, and the engine sent it back, which cost
    // a whole round: the schema the command line was given had left it out. It now requires it unless the return is one
    // of those forms, and the command line refuses a return without it, naming it, as the installed one did when tried
    // (2.1.312: "root: must have required property 'progress', root: must match \"then\" schema"), while a return
    // asking for tools without it passed. So on the engineer's and the architect's lines: a pause without it is
    // refused, and a return asking for tools is not.
    #[test]
    fn a_return_without_progress_is_refused_unless_its_form_leaves_it_out() {
        let f = compiled();
        let g = f.graph("architect").unwrap();
        let s = envelope(g, Some("matter"));
        let schema = g.cell("H4").unwrap().schema.clone().unwrap();
        let mut ret = json!({
            "tell_owner": [],
            "flow_feedback": "none",
            "outcome": {
                "finding": { "what": "A slow parser.", "grounds": "Ran it.", "sure": "checked" },
                "take": { "way": "nothing", "why": "Nothing to take in." }
            },
        });
        assert_eq!(
            refused(&s, &ret, "").as_deref(),
            Some("root: must have required property 'progress'")
        );
        ret["progress"] = json!("Found what the report is");
        assert!(flow::valid(&schema, &ret), "{ret}");
        assert_eq!(refused(&s, &ret, ""), None);
        for (g, line) in [("engineer", "engineer"), ("architect", "architect")] {
            let g = f.graph(g).unwrap();
            let s = envelope(g, Some(line));
            // The fields every cell of the line requires, beside the forms.
            let schemas: Vec<&Value> = g
                .cells
                .iter()
                .filter(|c| c.kind == "agent" && c.line.as_deref() == Some(line))
                .filter_map(|c| c.schema.as_ref())
                .collect();
            let mut base = json!({});
            for k in required_of(schemas[0])
                .into_iter()
                .filter(|k| *k != g.within && schemas.iter().all(|s| required_of(s).contains(k)))
            {
                base[k] = if k == "flow_feedback" { json!("none") } else { json!([]) };
            }
            let mut pause = base.clone();
            pause[&g.within] = json!({ "pause": {} });
            assert_eq!(
                refused(&s, &pause, "").as_deref(),
                Some("root: must have required property 'progress'"),
                "graph {}",
                g.name
            );
            let mut tools = base.clone();
            tools[&g.within] = json!({ "tools": [{ "tool": "query", "sql": "SELECT id FROM items" }] });
            assert_eq!(refused(&s, &tools, ""), None, "graph {}", g.name);
            let takes = g
                .cells
                .iter()
                .filter(|c| c.kind == "agent" && c.line.as_deref() == Some(line))
                .filter(|c| c.forms.iter().any(|x| x.field.as_deref() == Some("tools")))
                .any(|c| flow::valid(c.schema.as_ref().unwrap(), &tools));
            assert!(takes, "graph {}: no cell takes {tools}", g.name);
        }
    }

    // In the engineer's trial a return of the design handed in its answer without `why`, which the answer required and
    // no other form of the graph had, and the engine sent it back, which cost a round (R18-6, $0.34): the shape of every
    // form taken together could not require it. A form with a field of its own is now told apart by it, and the command
    // line refuses such a return naming the missing field, as the installed one did when tried (2.1.312). So thinking
    // again outside the frame without `changed`, its field of its own, is refused naming it, with it passes the envelope
    // and the cell's schema, and beside a field of another form it is refused.
    #[test]
    fn a_cell_s_own_answer_without_a_field_it_requires_is_refused_by_the_command_line() {
        let f = compiled();
        let g = f.graph("engineer").unwrap();
        let s = envelope(g, Some("designer"));
        let schema = g.cell("D2").unwrap().schema.clone().unwrap();
        let mut ret = samples()["D2"][0]["json"].clone();
        assert!(flow::valid(&schema, &ret), "{ret}");
        assert_eq!(refused(&s, &ret, ""), None);
        let changed = ret["outcome"].as_object_mut().unwrap().shift_remove("changed").unwrap();
        assert!(!flow::valid(&schema, &ret));
        assert_eq!(
            refused(&s, &ret, "").as_deref(),
            Some("/outcome: must have required property 'changed'")
        );
        ret["outcome"]["changed"] = changed;
        ret["outcome"]["pause"] = json!({});
        let mixed = refused(&s, &ret, "").unwrap_or_default();
        assert!(mixed.contains("is not allowed"), "{mixed}");
    }

    /// The sample returns of the engineer's graph, by cell: one for each row of each cell.
    fn samples() -> BTreeMap<String, Vec<Value>> {
        serde_json::from_str(include_str!("../../flow/tests/samples.json")).unwrap()
    }

    /// The engineer's `I1` return that is done, with its deviations as given.
    fn done_with(deviations: Value) -> Value {
        let mut ret = samples()["I1"][2]["json"].clone();
        ret["outcome"]["deviations"] = deviations;
        ret
    }

    // In the engineer's trial a `surprise` row of the deviations carried `why`, which only the other kinds take, eight
    // times (R227-21 among them): the schema the command line was given took the fields of the three kinds together,
    // the engine sent the return back and a whole round was spent each time. A union whose branches fix a field now
    // keeps its branches: a `surprise` with `why` is refused naming `why`, and taken without it; a `workaround` and a
    // `design_changed` without `why` are refused naming it, and taken with it.
    #[test]
    fn a_deviation_with_a_field_of_another_kind_is_refused_by_the_command_line_naming_it() {
        let f = compiled();
        let g = f.graph("engineer").unwrap();
        let s = envelope(g, Some("engineer"));
        let schema = g.cell("I1").unwrap().schema.clone().unwrap();
        let row = |kind: &str, why: Option<&str>| {
            let mut r = json!({ "kind": kind, "what": "A thing" });
            if let Some(w) = why {
                r["why"] = json!(w);
            }
            done_with(json!([r]))
        };
        let with = row("surprise", Some("Because"));
        assert!(!flow::valid(&schema, &with));
        assert_eq!(
            refused(&s, &with, "").as_deref(),
            Some("/outcome/deviations/0: must NOT have additional properties ('why' is not allowed)")
        );
        for (kind, why, taken) in [
            ("surprise", None, true),
            ("workaround", None, false),
            ("workaround", Some("Because"), true),
            ("design_changed", None, false),
            ("design_changed", Some("Because"), true),
        ] {
            let ret = row(kind, why);
            assert_eq!(flow::valid(&schema, &ret), taken, "{kind} {why:?}");
            let refusal = refused(&s, &ret, "");
            assert_eq!(refusal.is_none(), taken, "{kind} {why:?}: {refusal:?}");
            if !taken {
                let refusal = refusal.unwrap();
                assert!(refusal.contains("'why'"), "{refusal}");
            }
        }
        // Every row of a list is checked, not the first alone.
        let mixed = done_with(json!([
            { "kind": "surprise", "what": "A" },
            { "kind": "workaround", "what": "B", "why": "C" },
            { "kind": "surprise", "what": "D", "why": "E" },
        ]));
        assert_eq!(
            refused(&s, &mixed, "").as_deref(),
            Some("/outcome/deviations/2: must NOT have additional properties ('why' is not allowed)")
        );
    }

    // `stuck` belongs to the branch `no` of whether the research settles the approach: the command line refuses it with
    // `yes`, naming it, and takes it with `no`.
    #[test]
    fn a_field_of_another_branch_of_the_research_s_answer_is_refused_by_the_command_line() {
        let f = compiled();
        let g = f.graph("engineer").unwrap();
        let s = envelope(g, Some("engineer"));
        let schema = g.cell("R1").unwrap().schema.clone().unwrap();
        let mut ret = samples()["R1"][0]["json"].clone();
        assert_eq!(ret["outcome"]["can_start"]["verdict"], "yes");
        assert_eq!(refused(&s, &ret, ""), None);
        assert!(flow::valid(&schema, &ret));
        ret["outcome"]["can_start"]["stuck"] = json!("Two approaches to compare");
        assert!(!flow::valid(&schema, &ret));
        assert_eq!(
            refused(&s, &ret, "").as_deref(),
            Some("/outcome/can_start: must NOT have additional properties ('stuck' is not allowed)")
        );
        ret["outcome"]["can_start"] =
            json!({ "verdict": "no", "why": "Unchecked", "stuck": "Two approaches to compare" });
        assert!(flow::valid(&schema, &ret));
        assert_eq!(refused(&s, &ret, ""), None);
    }

    // A null in a field of a form the return is not (`ask: null` beside what was learned, as R241-2 and R262-2 beside a
    // brief; `why: null` in a `surprise`) is refused by the command line, which the agent puts right within its run; a
    // return the command line
    // did not check reaches the engine with it, which takes it as left out, so that the return fits the cell's schema,
    // and gives the path of each such null. A null in a field the return's own form requires, a value of nothing but
    // nulls, and a null in a field the form may leave out (which is left out as ever) are not among them.
    #[test]
    fn a_null_in_a_field_of_another_form_is_refused_by_the_command_line_and_left_out_by_the_engine() {
        let f = compiled();
        let g = f.graph("engineer").unwrap();
        let s = envelope(g, Some("engineer"));
        let schema = g.cell("I1").unwrap().schema.clone().unwrap();
        let mut surprise = done_with(json!([{ "kind": "surprise", "what": "A thing", "why": null }]));
        assert!(refused(&s, &surprise, "").is_some_and(|r| r.contains("'why'")));
        assert!(!flow::valid(&schema, &surprise));
        assert_eq!(
            flow::without_null_options(&schema, &mut surprise),
            ["outcome.deviations[0].why"]
        );
        assert!(flow::valid(&schema, &surprise), "{surprise}");
        // The same two lines of a `brief` of the reader of the history.
        let reader = envelope(g, Some("librarian"));
        let brief_schema = g.cell("R0").unwrap().schema.clone().unwrap();
        let mut brief = samples()["R0"][0]["json"].clone();
        brief["outcome"]["pause"] = Value::Null;
        assert!(refused(&reader, &brief, "").is_some_and(|r| r.contains("pause")));
        assert!(!flow::valid(&brief_schema, &brief));
        assert_eq!(flow::without_null_options(&brief_schema, &mut brief), ["outcome.pause"]);
        assert!(flow::valid(&brief_schema, &brief), "{brief}");
        // What the form requires is not left out.
        let mut required = done_with(json!([{ "kind": "workaround", "what": "A thing", "why": null }]));
        assert!(refused(&s, &required, "").is_some());
        assert_eq!(flow::without_null_options(&schema, &mut required), Vec::<String>::new());
        assert!(!flow::valid(&schema, &required));
        // A value of nothing but nulls is no form's.
        let mut only = samples()["R0"][0]["json"].clone();
        only["outcome"] = json!({ "pause": null });
        assert_eq!(
            flow::without_null_options(&brief_schema, &mut only),
            Vec::<String>::new()
        );
        assert!(!flow::valid(&brief_schema, &only));
        // A field the form may leave out is taken by the command line as null, left out by the engine, and not told.
        let mut optional = samples()["I1"][2]["json"].clone();
        optional["outcome"]["followup"] = Value::Null;
        assert_eq!(refused(&s, &optional, ""), None);
        assert!(!flow::valid(&schema, &optional));
        assert_eq!(flow::without_null_options(&schema, &mut optional), Vec::<String>::new());
        assert!(optional["outcome"].get("followup").is_none());
        assert!(flow::valid(&schema, &optional), "{optional}");
    }

    // Tightening the schema refuses nothing a cell takes: every sample return of the engineer's graph, one for each row
    // of each agent's cell, is taken by the schema of the line of its cell.
    #[test]
    fn every_sample_is_taken_by_the_schema_of_its_line() {
        let f = compiled();
        let g = f.graph("engineer").unwrap();
        let mut taken = 0;
        for (cell, xs) in samples() {
            let c = g.cell(&cell).unwrap();
            if c.kind != "agent" {
                continue;
            }
            let s = envelope(g, c.line.as_deref());
            for x in xs {
                assert_eq!(refused(&s, &x["json"], ""), None, "a sample of {cell}: {}", x["json"]);
                taken += 1;
            }
        }
        assert!(taken >= 20, "{taken}");
    }

    /// A smallest value of a cell's schema: what it fixes, else its first value or alternative, the fields it
    /// requires, and as many items as it must hold.
    fn minimal(s: &Value) -> Value {
        if let Some(c) = s.get("const") {
            return c.clone();
        }
        if let Some(e) = s.get("enum").and_then(Value::as_array) {
            return e[0].clone();
        }
        if let Some(alts) = ["oneOf", "anyOf"].iter().find_map(|k| s.get(*k)?.as_array()) {
            return minimal(&alts[0]);
        }
        match s.get("type").and_then(Value::as_str) {
            Some("string") => json!("x"),
            Some("integer") => json!(1),
            Some("boolean") => json!(true),
            Some("array") => {
                let n = s.get("minItems").and_then(Value::as_u64).unwrap_or(0);
                Value::Array((0..n).map(|_| minimal(&s["items"])).collect())
            }
            _ => {
                let required = required_of(s);
                let mut o = serde_json::Map::new();
                for (k, p) in s.get("properties").and_then(Value::as_object).into_iter().flatten() {
                    if required.contains(&k.as_str()) {
                        o.insert(k.clone(), minimal(p));
                    }
                }
                Value::Object(o)
            }
        }
    }

    /// A union in an agent's cell's schema whose branches all fix one field to a value of their own: the fields and
    /// list items down to it (`None` for an item), the field, and each branch with the value it fixes.
    struct Union<'a> {
        at: Vec<Option<&'a str>>,
        by: &'a str,
        branches: Vec<(&'a Value, &'a Value)>,
    }

    /// The value an alternative fixes a field to.
    fn fixed<'a>(a: &'a Value, k: &str) -> Option<&'a Value> {
        a.get("properties")?.get(k)?.get("const")
    }

    /// Every such union in a schema, each with where it is.
    fn unions<'a>(s: &'a Value, at: &mut Vec<Option<&'a str>>, out: &mut Vec<Union<'a>>) {
        if let Some(alts) = ["oneOf", "anyOf"].iter().find_map(|k| s.get(*k)?.as_array()) {
            let first = alts.first().and_then(|a| a.get("properties")?.as_object());
            let by = first
                .into_iter()
                .flatten()
                .map(|(k, _)| k.as_str())
                .find(|k| alts.iter().all(|a| fixed(a, k).is_some()));
            if let Some(by) = by {
                out.push(Union {
                    at: at.clone(),
                    by,
                    branches: alts.iter().filter_map(|a| Some((fixed(a, by)?, a))).collect(),
                });
            }
            for a in alts {
                unions(a, at, out);
            }
        }
        if let Some(ps) = s.get("properties").and_then(Value::as_object) {
            for (k, p) in ps {
                at.push(Some(k.as_str()));
                unions(p, at, out);
                at.pop();
            }
        }
        if let Some(it) = s.get("items") {
            at.push(None);
            unions(it, at, out);
            at.pop();
        }
    }

    /// The parts of a cell's schema at the fields and items given, through the alternatives of each part on the way.
    fn places<'a>(s: &'a Value, at: &[Option<&str>]) -> Vec<&'a Value> {
        let Some((step, rest)) = at.split_first() else {
            return vec![s];
        };
        let alts: Vec<&Value> = ["oneOf", "anyOf"]
            .iter()
            .filter_map(|k| s.get(*k)?.as_array())
            .flatten()
            .collect();
        let parts = if alts.is_empty() { vec![s] } else { alts };
        let mut out = Vec::new();
        for part in parts {
            let next = match step {
                Some(k) => part.get("properties").and_then(|p| p.get(*k)),
                None => part.get("items"),
            };
            if let Some(n) = next {
                out.extend(places(n, rest));
            }
        }
        out
    }

    /// The part of an envelope at the fields and items given.
    fn down<'a>(s: &'a Value, at: &[Option<&str>]) -> Option<&'a Value> {
        at.iter().try_fold(s, |s, step| match step {
            Some(k) => s.get("properties")?.get(*k),
            None => s.get("items"),
        })
    }

    // Each branch of every union of every agent's cell, of every graph, is told apart on its line: a value of the
    // branch with only the fields the branch requires is taken by the line's schema (so that tightening it refuses
    // nothing a cell takes, whichever cell the branch is of), and the same value with a field of another branch of the
    // union, which no cell of the line has in this branch, is refused. The branches are read from the cells' own
    // schemas, not from the generated one.
    #[test]
    fn each_branch_of_every_union_is_told_apart_on_its_line() {
        let f = compiled();
        let (mut taken, mut refused_with) = (0, 0);
        // The places of unions whose branches are not told apart on the line, for the reason below: listed, so that a
        // union that stops being told apart anywhere else is found by this test.
        let mut skipped: BTreeSet<String> = BTreeSet::new();
        for g in &f.graphs {
            for line in lines_of(g) {
                let env = envelope(g, line);
                // The cells the line's schema is made of: those of the lines forked together with it too.
                let lines = forked_together(g, line);
                let cells: Vec<&flow::CompiledCell> = g
                    .cells
                    .iter()
                    .filter(|c| c.kind == "agent" && lines.contains(&c.line))
                    .collect();
                let found: Vec<(&str, Vec<Union<'_>>)> = cells
                    .iter()
                    .map(|c| {
                        let mut out = Vec::new();
                        unions(c.schema.as_ref().unwrap(), &mut Vec::new(), &mut out);
                        (c.id.as_str(), out)
                    })
                    .collect();
                for (cell, us) in &found {
                    for u in us {
                        let place = || format!("graph {}, cell {cell}, at {:?} by {}", g.name, u.at, u.by);
                        let sub = down(&env, &u.at).unwrap_or_else(|| panic!("{}: no such part", place()));
                        for (value, branch) in &u.branches {
                            let example = minimal(branch);
                            assert_eq!(
                                refused(sub, &example, ""),
                                None,
                                "{}: {value} taken as {example}",
                                place()
                            );
                            taken += 1;
                            // The fields this value has in a branch of a cell of the line at this place.
                            let mut has: BTreeSet<&str> = BTreeSet::new();
                            for (_, vs) in &found {
                                for v in vs.iter().filter(|v| v.at == u.at && v.by == u.by) {
                                    for (w, b) in &v.branches {
                                        if w == value {
                                            has.extend(
                                                b["properties"]
                                                    .as_object()
                                                    .into_iter()
                                                    .flatten()
                                                    .map(|(k, _)| k.as_str()),
                                            );
                                        }
                                    }
                                }
                            }
                            // Taken with a value that is no such union, a union keeps no branches (the design's merge:
                            // the line must take what either cell does): `answer` of the architect's `H8` is a union
                            // by `way`, and of another of its cells a plain object holding `why`. So the branches
                            // are told apart only where every cell of the line that has a value at this place has
                            // this union there.
                            let here = cells
                                .iter()
                                .flat_map(|c| places(c.schema.as_ref().unwrap(), &u.at))
                                .count();
                            let unions_here = found
                                .iter()
                                .flat_map(|(_, vs)| vs)
                                .filter(|v| v.at == u.at && v.by == u.by)
                                .count();
                            if here != unions_here {
                                skipped.insert(place());
                            }
                            let foreign = u
                                .branches
                                .iter()
                                .filter(|(w, _)| w != value && here == unions_here)
                                .flat_map(|(_, b)| b["properties"].as_object().into_iter().flatten())
                                .find(|(k, _)| !has.contains(k.as_str()));
                            if let Some((k, p)) = foreign {
                                let mut other = example.clone();
                                other[k] = minimal(p);
                                assert!(
                                    refused(sub, &other, "").is_some(),
                                    "{}: {value} takes {k} of another branch: {other}",
                                    place()
                                );
                                refused_with += 1;
                            }
                        }
                    }
                }
            }
        }
        assert!(taken > 20 && refused_with > 5, "{taken} {refused_with}");
        // The one union whose branches are not told apart on its line: `answer` of the architect's `H8`, a union by
        // `way` there and a plain object in another cell of the line, so that the merge keeps no branches (it must take
        // what either cell does). A union that stops being told apart anywhere else, or this one that starts to be,
        // changes the list and fails here.
        assert_eq!(
            skipped.into_iter().collect::<Vec<_>>(),
            ["graph architect, cell H8, at [Some(\"outcome\"), Some(\"answer\")] by way"]
        );
    }

    // A line's schema is the same each time it is made, and from the flow as the engine reads it back from what it
    // stored, so that it is the same for every cell and every round of the line, which keeps the session's cache.
    #[test]
    fn a_lines_schema_is_the_same_each_time_it_is_made() {
        let f = compiled();
        let back: flow::Compiled = serde_json::from_str(&f.json()).unwrap();
        for (g, h) in f.graphs.iter().zip(&back.graphs) {
            for line in lines_of(g) {
                let once = envelope(g, line).to_string();
                assert_eq!(once, envelope(g, line).to_string(), "graph {}", g.name);
                assert_eq!(once, envelope(h, line).to_string(), "graph {} as stored", g.name);
            }
        }
    }

    // A return says where it was read from: the tool's input, which the command line checked against its schema, or the
    // JSON in the text of the result, which it did not; none for a run that handed in none.
    #[test]
    fn a_return_says_where_it_was_read_from() {
        let read = |result: &str| {
            let mut r = Reply::default();
            read_stream(result, &mut r);
            (r.output, r.output_from)
        };
        let tool = r#"{"type":"result","subtype":"success","result":"Done.","structured_output":{"progress":"p"}}"#;
        assert_eq!(read(tool), (Some(json!({ "progress": "p" })), Some(Source::Tool)));
        let text = r#"{"type":"result","subtype":"success","result":"{\"progress\":\"p\"}"}"#;
        assert_eq!(read(text), (Some(json!({ "progress": "p" })), Some(Source::Text)));
        let none = r#"{"type":"result","subtype":"success","result":"Nothing to hand in."}"#;
        assert_eq!(read(none), (None, None));
        assert_eq!((Source::Tool.name(), Source::Text.name()), ("tool", "text"));
    }

    // The rules that tell the forms apart refuse nothing a cell takes: for each form of each agent's cell of each graph,
    // the rule of every form whose own fields it holds requires no field the form does not require and, when it takes
    // no other field, takes every field the form takes. The engineer's and the architect's lines have such rules.
    #[test]
    fn the_rules_that_tell_the_forms_apart_refuse_nothing_a_cell_takes() {
        let f = compiled();
        for g in &f.graphs {
            for line in lines_of(g) {
                check_rules_of(g, line);
            }
        }
    }

    /// The test above for the agents' cells of one line.
    fn check_rules_of(g: &flow::CompiledGraph, line: Option<&str>) {
        let s = envelope(g, line);
        let rules = s["properties"][&g.within]["allOf"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if matches!(line, Some("engineer" | "architect")) {
            assert!(!rules.is_empty(), "graph {}", g.name);
        }
        for c in g
            .cells
            .iter()
            .filter(|c| c.kind == "agent" && c.line.as_deref() == line)
        {
            let within = &c.schema.as_ref().unwrap()["properties"][&g.within];
            let alts = within["anyOf"]
                .as_array()
                .cloned()
                .unwrap_or_else(|| vec![within.clone()]);
            for form in alts.iter().map(Shape::of) {
                let fields = form.field_names();
                for r in &rules {
                    let when = &r["if"];
                    let names: Vec<&str> = match when.get("anyOf").and_then(Value::as_array) {
                        Some(xs) => xs.iter().flat_map(required_of).collect(),
                        None => required_of(when),
                    };
                    if !names.iter().any(|n| fields.iter().any(|k| k == n)) {
                        continue;
                    }
                    let then = &r["then"];
                    for k in required_of(then) {
                        assert!(
                            form.required.iter().any(|x| x == k),
                            "graph {}, cell {}: {k} required by {r}",
                            g.name,
                            c.id
                        );
                    }
                    if closed(then) {
                        for k in &fields {
                            assert!(
                                then["properties"].get(*k).is_some(),
                                "graph {}, cell {}: {k} refused by {r}",
                                g.name,
                                c.id
                            );
                        }
                    }
                }
            }
        }
    }

    /// The types a schema's `type` names, in order.
    fn types(t: &Value) -> Vec<&str> {
        match t {
            Value::String(s) => vec![s.as_str()],
            Value::Array(xs) => xs.iter().filter_map(Value::as_str).collect(),
            _ => Vec::new(),
        }
    }

    /// Whether a schema takes no field of an object beyond those it lists.
    fn closed(s: &Value) -> bool {
        s.get("additionalProperties") == Some(&Value::Bool(false))
    }

    /// Whether a schema may be an object: it names that type, or none, as it may be anything.
    fn takes_object(s: &Value) -> bool {
        s.get("type").is_none_or(|t| types(t).contains(&"object"))
    }

    /// The first place of a shape where another does not take every value it takes: a type it lacks, a field or the
    /// items it has none of, a field it requires that the first does not, or a field it refuses that the first takes;
    /// none when it takes them all.
    fn beyond(a: &Value, b: &Value, at: &str) -> Option<String> {
        if let Some(t) = b.get("type") {
            let all = types(t);
            if a.get("type").is_none_or(|x| types(x).iter().any(|y| !all.contains(y))) {
                return Some(format!("{at}: {} beyond {t}", a.get("type").unwrap_or(&Value::Null)));
            }
        }
        if takes_object(a) {
            if let Some(k) = required_of(b).into_iter().find(|k| !required_of(a).contains(k)) {
                return Some(format!("{at}.{k}: required"));
            }
            if closed(b) && !closed(a) {
                return Some(format!("{at}: takes no other field"));
            }
        }
        for (k, v) in a.get("properties").and_then(Value::as_object).into_iter().flatten() {
            let Some(w) = b.get("properties").and_then(|p| p.get(k)) else {
                return Some(format!("{at}.{k}: no such field"));
            };
            if let Some(x) = beyond(v, w, &format!("{at}.{k}")) {
                return Some(x);
            }
        }
        match (a.get("items"), b.get("items")) {
            (Some(x), Some(y)) => beyond(x, y, &format!("{at}[]")),
            (Some(_), None) => Some(format!("{at}[]: no items")),
            _ => None,
        }
    }

    // The input of each attempt at a return is rebuilt whole from the pieces the stream's partial messages carry, and
    // kept only when the command line refused it, with the length it reported and why, without the part of
    // the return it quoted; a partial message is not kept with the stream, and another tool's input is not rebuilt.
    #[test]
    fn an_attempt_the_command_line_refused_is_rebuilt_from_the_stream() {
        let event = |e: Value| json!({ "type": "stream_event", "event": e, "parent_tool_use_id": null }).to_string();
        let start = |i: i64, id: &str, name: &str| {
            event(json!({ "type": "content_block_start", "index": i,
                          "content_block": { "type": "tool_use", "id": id, "name": name, "input": {} } }))
        };
        let piece = |i: i64, p: &str| {
            event(json!({ "type": "content_block_delta", "index": i,
                          "delta": { "type": "input_json_delta", "partial_json": p } }))
        };
        let stop = |i: i64| event(json!({ "type": "content_block_stop", "index": i }));
        let refused = |id: &str, text: &str| {
            json!({ "type": "user", "parent_tool_use_id": null, "message": { "content": [
                { "type": "tool_result", "tool_use_id": id, "is_error": true, "content": text }] } })
            .to_string()
        };
        // A bare quote inside text of characters of several bytes (symbols and emoji), which the model wrote unescaped.
        let broken =
            "{\"outcome\": {\"answer\": {\"why\": \"\u{1f600}\u{2192}\"\u{2714}\u{1f680}\"\"}}, \"progress\": \"p\"}";
        let whole = r#"{"outcome": {"answer": {"why": "w"}}, "progress": "p"}"#;
        let mut lines = vec![
            json!({ "type": "system", "subtype": "init", "session_id": "s1", "model": "m" }).to_string(),
            event(json!({ "type": "message_start", "message": {} })),
            start(0, "read-1", "Read"),
            piece(0, r#"{"file_path": "/w/a"}"#),
            stop(0),
            start(1, "so-1", STRUCTURED_OUTPUT),
        ];
        let cut = broken
            .char_indices()
            .map(|(i, _)| i)
            .filter(|i| i % 7 == 0)
            .collect::<Vec<_>>();
        let mut from = 0;
        for at in cut.into_iter().skip(1).chain([broken.len()]) {
            lines.push(piece(1, &broken[from..at]));
            from = at;
        }
        lines.push(stop(1));
        let said = format!(
            "<tool_use_error>InputValidationError: StructuredOutput was called with input that could not be parsed as \
             JSON.\nYou sent (first 200 of {} bytes): {}</tool_use_error>",
            broken.chars().count(),
            broken
        );
        lines.push(refused("so-1", &said));
        lines.push(event(json!({ "type": "message_start", "message": {} })));
        lines.push(start(0, "so-2", STRUCTURED_OUTPUT));
        lines.push(piece(0, whole));
        lines.push(stop(0));
        lines.push(
            json!({ "type": "result", "subtype": "success", "session_id": "s1",
                    "structured_output": serde_json::from_str::<Value>(whole).unwrap() })
            .to_string(),
        );
        let mut stream = Stream::default();
        for l in &lines {
            stream.line(l);
        }
        assert!(!stream.kept.contains("stream_event"), "{}", stream.kept);
        let mut r = Reply::default();
        stream.end(&mut r);
        assert_eq!(r.refused.len(), 1, "{:?}", r.refused);
        let u = &r.refused[0];
        assert_eq!(u.id, "so-1");
        assert_eq!(u.raw, broken);
        assert_eq!(u.length, Some(broken.chars().count()));
        let why = u.why.clone().unwrap();
        assert!(why.ends_with("bytes)") && !why.contains("\u{2714}\u{1f680}"), "{why}");
        assert_eq!(r.output.unwrap()["progress"], "p");
    }

    // Every attempt the command line refused is kept with why, whatever it refused it for, in a run that handed a
    // return in at last as in one it gave up on; the attempt it took is not.
    #[test]
    fn every_attempt_the_command_line_refused_is_kept() {
        let event = |e: Value| json!({ "type": "stream_event", "event": e, "parent_tool_use_id": null }).to_string();
        let attempt = |id: &str, input: &str| {
            let block = json!({ "type": "tool_use", "id": id, "name": STRUCTURED_OUTPUT, "input": {} });
            [
                event(json!({ "type": "message_start", "message": {} })),
                event(json!({ "type": "content_block_start", "index": 0, "content_block": block })),
                event(json!({ "type": "content_block_delta", "index": 0,
                              "delta": { "type": "input_json_delta", "partial_json": input } })),
                event(json!({ "type": "content_block_stop", "index": 0 })),
            ]
        };
        let refused = |id: &str, text: &str| {
            json!({ "type": "user", "parent_tool_use_id": null, "message": { "content": [
                { "type": "tool_result", "tool_use_id": id, "is_error": true, "content": text }] } })
            .to_string()
        };
        let as_text = r#"{"progress": "p", "outcome": "{\"answer\": {\"why\": \"w\"}}"}"#;
        let without = r#"{"progress": "p", "outcome": {"answer": {"why": "w"}}}"#;
        let taken = r#"{"tell_owner": [], "progress": "p", "outcome": {"answer": {"why": "w"}}}"#;
        let missing = "Output does not match required schema: root: must have required property 'tell_owner'";
        let run = |result: Value| {
            let mut lines = vec![json!({ "type": "system", "subtype": "init", "session_id": "s1" }).to_string()];
            lines.extend(attempt("so-1", as_text));
            lines.push(refused(
                "so-1",
                "Output does not match required schema: /outcome: must be object",
            ));
            lines.extend(attempt("so-2", without));
            lines.push(refused("so-2", missing));
            if result["subtype"] == "success" {
                lines.extend(attempt("so-3", taken));
            }
            lines.push(result.to_string());
            let mut stream = Stream::default();
            for l in &lines {
                stream.line(l);
            }
            let mut r = Reply::default();
            stream.end(&mut r);
            r
        };
        let r = run(json!({ "type": "result", "subtype": "success", "session_id": "s1",
                            "structured_output": serde_json::from_str::<Value>(taken).unwrap() }));
        let kept: Vec<(&str, &str)> = r.refused.iter().map(|u| (u.id.as_str(), u.raw.as_str())).collect();
        assert_eq!(kept, [("so-1", as_text), ("so-2", without)]);
        assert_eq!(r.refused[1].why.as_deref(), Some(missing));
        assert_eq!(r.output.unwrap()["tell_owner"], json!([]));
        let gave_up = json!({ "type": "result", "subtype": "error_max_structured_output_retries", "is_error": true,
                              "session_id": "s1" });
        let r = run(gave_up);
        let kept: Vec<&str> = r.refused.iter().map(|u| u.id.as_str()).collect();
        assert_eq!(kept, ["so-1", "so-2"]);
        assert!(r.output.is_none());
    }

    #[test]
    fn a_failed_result_says_why_from_its_errors() {
        let mut r = Reply::default();
        read_stream(
            r#"{"type":"result","subtype":"error_max_structured_output_retries","is_error":true,"errors":["Failed to provide valid structured output after 5 attempts"]}"#,
            &mut r,
        );
        assert_eq!(
            r.error.as_deref(),
            Some("error_max_structured_output_retries: Failed to provide valid structured output after 5 attempts")
        );
        let mut r = Reply::default();
        read_stream(
            r#"{"type":"result","subtype":"error_during_execution","is_error":true}"#,
            &mut r,
        );
        assert_eq!(
            r.error.as_deref(),
            Some("error_during_execution: the CLI gave no reason")
        );
        // An API it cannot reach the CLI reports as an error under "success": the reason alone, still told offline.
        let mut r = Reply::default();
        read_stream(
            r#"{"type":"result","subtype":"success","is_error":true,"result":"API Error: Can't reach the API server (ENOTFOUND)"}"#,
            &mut r,
        );
        assert_eq!(
            r.error.as_deref(),
            Some("API Error: Can't reach the API server (ENOTFOUND)")
        );
        assert_eq!(failure_kind(r.error.as_deref().unwrap_or_default()), "offline");
    }

    // What the agent does is told from the stream as it comes: a tool with what it is used on, the start of what it
    // says, a compaction; anything else says nothing.
    #[test]
    fn what_the_agent_does_is_read_from_the_stream() {
        let tool = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"git diff --stat"}}]}}"#;
        assert_eq!(activity_of(tool).as_deref(), Some("Bash: git diff --stat"));
        let read = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/w/src/lib.rs"}}]}}"#;
        assert_eq!(activity_of(read).as_deref(), Some("Read: /w/src/lib.rs"));
        let said = format!(
            r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"{}"}}]}}}}"#,
            "a ".repeat(200)
        );
        let s = activity_of(&said).unwrap();
        assert!(
            s.starts_with("says: a a") && s.ends_with('…') && s.chars().count() < 200,
            "{s}"
        );
        assert_eq!(
            activity_of(r#"{"type":"system","subtype":"compact_boundary"}"#).as_deref(),
            Some("compacting the session")
        );
        assert_eq!(activity_of(r#"{"type":"user","message":{"content":[]}}"#), None);
        assert_eq!(activity_of("not json"), None);
    }

    #[test]
    fn a_failed_run_is_told_apart_by_what_the_cli_reports() {
        // What the command line reports of a return it could not parse may quote any words of the return.
        assert_eq!(
            failure_kind(
                "error_max_structured_output_retries: Failed to provide valid structured output after 3 attempts \
                 (first 200 of 900 bytes): please run /login"
            ),
            "unparsed"
        );
        assert_eq!(failure_kind("Claude AI usage limit reached|1760000000"), "usage_limit");
        assert_eq!(failure_kind("You've hit your limit · resets 5pm"), "usage_limit");
        assert_eq!(failure_kind("Invalid API key · Please run /login"), "cannot_run");
        assert_eq!(failure_kind("Prompt is too long"), "prompt_too_long");
        assert_eq!(
            failure_kind("claude: No such file or directory (os error 2)"),
            "cannot_run"
        );
        assert_eq!(
            failure_kind("API Error: Connection error. (getaddrinfo ENOTFOUND api.anthropic.com)"),
            "offline"
        );
        assert_eq!(failure_kind("API Error: 529 Overloaded"), "transient");
        assert_eq!(
            failure_kind(
                "API Error: Opus 5.5's safeguards flagged this message (https://www.anthropic.com/legal/aup)."
            ),
            "flagged"
        );
        assert_eq!(failure_kind("claude exited with signal: 9 and no result"), "transient");
    }

    #[test]
    fn a_usage_limit_resets_when_the_cli_says() {
        let utc = |_: Option<&str>| Some(0);
        // 2026-10-25 20:00 UTC.
        let now = 1_792_958_400_000;
        assert_eq!(
            resets_at("Claude AI usage limit reached|1792962000", now, &utc),
            Some(1_792_962_000_000)
        );
        assert_eq!(
            resets_at("You've hit your limit · resets 9pm", now, &utc),
            Some(now + 3_600_000)
        );
        // A time already past today is tomorrow's.
        assert_eq!(
            resets_at("5-hour limit reached ∙ resets 7:30pm", now, &utc),
            Some(now + 23 * 3_600_000 + 1_800_000)
        );
        let paris = |z: Option<&str>| (z == Some("Europe/Paris")).then_some(3600);
        assert_eq!(
            resets_at("You've hit your limit · resets 11pm (Europe/Paris)", now, &paris),
            Some(now + 2 * 3_600_000)
        );
        assert_eq!(resets_at("usage limit reached", now, &utc), None);
    }

    #[test]
    fn a_round_runs_with_its_whole_command_line_and_its_own_configuration_directory() {
        let cli = config::Cli {
            kind: config::CliKind::Claude,
            command: "claude".into(),
            args: [
                "--permission-mode",
                "acceptEdits",
                "--allowedTools",
                "Bash(git *)",
                "Bash(cargo fmt *)",
            ]
            .iter()
            .map(|a| a.to_string())
            .collect(),
            config_dir: None,
        };
        let permissions = config::Permissions::default();
        let r = Request {
            work: 1,
            cell: "I1".into(),
            line: "engineer".into(),
            cli: cli.clone(),
            model: "m".into(),
            effort: Some("high".into()),
            worktree: PathBuf::from("/w"),
            config_dir: Some(PathBuf::from("/data/agents/claude")),
            resume: Some("s1".into()),
            fork: false,
            system: None,
            new_session: "s2".into(),
            prompt: "p".into(),
            images: Vec::new(),
            schema: json!({ "type": "object", "properties": { "outcome": {}, "progress": { "type": "string" } } }),
            envelope: json!({ "type": "object", "properties": { "outcome": {}, "progress": { "type": "string" } } }),
            allowed: permissions.allowed("engineer", &cli),
            refused: permissions.refused("engineer", &cli),
            readable: vec![PathBuf::from("/data/tool-output")],
            replay: None,
            regive: Some(PathBuf::from("/tmp/given it.md")),
            deliver: Some("wf-agents deliver --work 1".into()),
            bare: false,
        };
        let line = command_line(&r);
        // The stream carries each tool input in pieces, which rebuild an attempt the command line refused.
        assert!(line.iter().any(|a| a == "--include-partial-messages"), "{line:?}");
        // The engine's saved tool output is readable and not editable.
        assert!(
            line.windows(2).any(|w| w == ["--add-dir", "/data/tool-output"]),
            "{line:?}"
        );
        assert!(line.iter().any(|a| a == "Edit(//data/tool-output/**)"), "{line:?}");
        assert_eq!(
            &line[..7],
            [
                "claude",
                "--permission-mode",
                "acceptEdits",
                "--allowedTools",
                "Bash(git *)",
                "Bash(cargo fmt *)",
                "-p"
            ]
        );
        // Heavy work is refused on every line; an engineer may still edit and commit, and update a lock file with the
        // package manager, which the line's own rule allows, after the CLI's own, and no rule refuses. What the CLI's
        // own arguments allow of a heavy command left out for that is refused again.
        let flag = |f: &str| line.iter().rposition(|a| a == f).unwrap();
        let (allowed, refused) = (flag("--allowedTools"), flag("--disallowedTools"));
        assert!(allowed > 7);
        assert_eq!(line[allowed + 1..refused], permissions.allowed("engineer", &cli)[..]);
        assert!(line[allowed + 1..refused].iter().any(|a| a == "Bash(cargo update *)"));
        for a in ["Bash(npm run *)", "Bash(cargo fmt *)", "Bash(npm install *=*)"] {
            assert!(line[refused..].iter().any(|x| x == a), "{a}: {line:?}");
        }
        assert!(
            !line.iter().any(|a| a == "Bash(cargo *)" || a == "Bash(npm install *)"),
            "{line:?}"
        );
        assert!(
            !line.iter().any(|a| a == "Edit" || a == "Bash(git commit *)"),
            "{line:?}"
        );
        // The review and the advisor only read: they may not edit or commit, and their command line allows nothing
        // beyond the CLI's own arguments; the adversarial tester writes tests.
        for l in ["review", "advisor"] {
            let refused = permissions.refused(l, &cli);
            assert!(refused.iter().any(|a| a == "Edit") && refused.iter().any(|a| a == "Bash(git commit *)"));
            let reader = command_line(&Request {
                line: l.into(),
                allowed: permissions.allowed(l, &cli),
                refused,
                ..r.clone()
            });
            assert_eq!(
                reader.iter().filter(|a| *a == "--allowedTools").count(),
                1,
                "{reader:?}"
            );
            assert!(!reader.iter().any(|a| a.starts_with("Bash(cargo update")), "{reader:?}");
        }
        assert!(!permissions.refused("adversarial", &cli).iter().any(|a| a == "Edit"));
        assert_eq!(&line[line.len() - 2..], ["--resume", "s1"]);
        // A compaction within the round has the command line give the session again what the round was given.
        let settings = line.iter().position(|a| a == "--settings").unwrap();
        let hooks: Value = serde_json::from_str(&line[settings + 1]).unwrap();
        let start = &hooks["hooks"]["SessionStart"][0];
        assert_eq!(start["matcher"], "compact");
        assert_eq!(start["hooks"][0]["command"], "cat '/tmp/given it.md'");
        // After each tool use a hook hands in the interjections waiting for the line.
        assert_eq!(
            hooks["hooks"]["PostToolUse"][0]["hooks"][0]["command"],
            "wf-agents deliver --work 1"
        );
        assert_eq!(quote("it's"), "'it'\\''s'");
        // It reads no project instructions or settings by itself.
        assert!(line.windows(2).any(|w| w == ["--setting-sources", "user"]), "{line:?}");
        let c = call(&r, &line, "2.1.0 (Claude Code)", "e1");
        assert_eq!(c["session"]["resumed"], "s1");
        assert_eq!(c["env"]["CLAUDE_CONFIG_DIR"], "/data/agents/claude");
        assert_eq!(c["env"]["CLAUDE_CODE_ENABLE_FINE_GRAINED_TOOL_STREAMING"], "0");
        assert_eq!(c["env"]["MAX_STRUCTURED_OUTPUT_RETRIES"], "3");
        assert_eq!(c["env"]["RUSTUP_AUTO_INSTALL"], "0");
        // The structured-output schema is in the command line, and not recorded again beside it.
        assert!(c.get("schema").is_none());
        assert!(
            line.windows(2)
                .any(|w| w[0] == "--json-schema" && w[1].contains("\"progress\""))
        );
        assert_eq!(c["cli"], "2.1.0 (Claude Code)");
        assert_eq!(c["engine"], "e1");
        let cmd = Claude::command(&r);
        let env: Vec<(String, String)> = cmd
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()).unwrap_or_default(),
                )
            })
            .collect();
        let want = |k: &str, v: &str| env.iter().any(|(a, b)| a == k && b == v);
        assert!(
            want("CLAUDE_CONFIG_DIR", "/data/agents/claude")
                && want("CLAUDE_CODE_ENABLE_FINE_GRAINED_TOOL_STREAMING", "0")
                && want("MAX_STRUCTURED_OUTPUT_RETRIES", "3")
                && want("RUSTUP_AUTO_INSTALL", "0")
                && env.len() == 4,
            "{env:?}"
        );
        // A bare call, as a repair, is offered no tools and given no structured output.
        let bare = command_line(&Request {
            bare: true,
            ..r.clone()
        });
        assert!(
            bare.windows(2).any(|w| w[0] == "--tools" && w[1].is_empty()) && !bare.iter().any(|a| a == "--json-schema"),
            "{bare:?}"
        );
        // A keep-warm ping runs the command line it replays, as it is.
        let replayed: Vec<String> = ["claude", "-p", "--resume", "s1"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let ping = Request {
            replay: Some(replayed),
            ..r.clone()
        };
        let cmd = Claude::command(&ping);
        assert_eq!(cmd.get_program(), "claude");
        assert_eq!(cmd.get_args().collect::<Vec<_>>(), ["-p", "--resume", "s1"]);
    }

    // Every round's message goes on the standard input one way: one user message of the stream-json input format, its
    // text, then each image after a text block of its address, so that the owner's images reach the agent in the
    // message; a round with no images gives its text alone. A keep-warm ping that replays a command line from before
    // rounds were sent so gives its text as it is, and one that replays a newer command line, as stream-json.
    #[test]
    fn a_round_s_message_goes_on_the_standard_input_as_stream_json() {
        let r = Request {
            work: 1,
            cell: "I1".into(),
            line: "engineer".into(),
            cli: config::Cli {
                kind: config::CliKind::Claude,
                command: "claude".into(),
                args: Vec::new(),
                config_dir: None,
            },
            model: "m".into(),
            effort: None,
            worktree: PathBuf::from("/w"),
            config_dir: None,
            resume: None,
            fork: false,
            system: None,
            new_session: "s1".into(),
            prompt: "# R1-2 · cell I1\n\nFrom the owner: look.\n\nImages: image:12, image:13\n".into(),
            images: vec![
                Image {
                    address: "image:12".into(),
                    media_type: "image/png".into(),
                    bytes: b"\x89PNG\r\n\x1a\n one".to_vec(),
                },
                Image {
                    address: "image:13".into(),
                    media_type: "image/jpeg".into(),
                    bytes: b"\xff\xd8\xff two".to_vec(),
                },
            ],
            schema: json!({}),
            envelope: json!({}),
            allowed: Vec::new(),
            refused: Vec::new(),
            readable: Vec::new(),
            replay: None,
            regive: None,
            deliver: None,
            bare: false,
        };
        let line = command_line(&r);
        assert!(
            line.windows(2).any(|w| w == ["--input-format", "stream-json"]),
            "{line:?}"
        );
        assert!(
            line.windows(2).any(|w| w == ["--output-format", "stream-json"]),
            "{line:?}"
        );
        let given = stdin(&line, &r.prompt, &r.images);
        assert_eq!(given.last(), Some(&b'\n'));
        let text = String::from_utf8(given).unwrap();
        assert_eq!(text.lines().count(), 1, "{text}");
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            (v["type"].as_str(), v["message"]["role"].as_str()),
            (Some("user"), Some("user"))
        );
        let content = v["message"]["content"].as_array().unwrap();
        assert_eq!(content.len(), 5, "{v}");
        assert_eq!(content[0], json!({ "type": "text", "text": r.prompt }));
        for (i, im) in r.images.iter().enumerate() {
            assert_eq!(content[1 + 2 * i], json!({ "type": "text", "text": im.address }));
            let block = &content[2 + 2 * i];
            assert_eq!(block["type"], "image");
            assert_eq!(block["source"]["type"], "base64");
            assert_eq!(block["source"]["media_type"], json!(im.media_type));
            let data = block["source"]["data"].as_str().unwrap();
            assert_eq!(db::attachments::from_base64(data).unwrap(), im.bytes);
        }
        // A round with no images gives its text alone, in the same way.
        let plain: Value = serde_json::from_slice(&stdin(&line, "p", &[])).unwrap();
        assert_eq!(plain["message"]["content"], json!([{ "type": "text", "text": "p" }]));
        // A keep-warm ping replays the last round's command line as it ran.
        let before: Vec<String> = ["claude", "-p", "--output-format", "stream-json", "--resume", "s1"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(stdin(&before, "ping", &[]), b"ping");
        let ping: Value = serde_json::from_slice(&stdin(&crate::warm::replayed(&line, "s1"), "ping", &[])).unwrap();
        assert_eq!(ping["message"]["content"][0]["text"], "ping");
    }

    #[test]
    fn a_result_without_structured_output_is_read_from_its_text() {
        let mut r = Reply::default();
        read_stream(
            r#"{"type":"result","subtype":"success","result":"Here:\n```json\n{\"a\":1}\n```"}"#,
            &mut r,
        );
        assert_eq!(r.output.unwrap()["a"], 1);
        let id = new_session_id();
        assert_eq!(id.len(), 36);
        assert_eq!(&id[14..15], "4");
    }

    /// The calls of what a stream's lines tell, each time one is told.
    fn calls_told(lines: &[String]) -> Vec<Call> {
        let mut stream = Stream::default();
        let told = lines.iter().flat_map(|l| stream.line(l)).collect::<Vec<_>>();
        told.into_iter()
            .filter_map(|s| match s {
                Seen::Call(c) => Some(c),
                _ => None,
            })
            .collect()
    }

    /// A line of a message of the model.
    fn message(id: &str, parent: Value, prompt: i64, output: i64) -> String {
        json!({ "type": "assistant", "parent_tool_use_id": parent, "message": { "id": id, "model": "m", "usage": {
            "input_tokens": 2, "cache_read_input_tokens": prompt, "cache_creation_input_tokens": 500,
            "cache_creation": { "ephemeral_5m_input_tokens": 0, "ephemeral_1h_input_tokens": 500 },
            "output_tokens": output,
        }, "content": [] } })
        .to_string()
    }

    // The stream gives one call on many lines: it is told once, and again only for more output; the partial message
    // that starts a call tells of it first, and the one that ends it says its output in the end. A subagent's call is
    // marked, and a message the command line makes itself called no model.
    #[test]
    fn a_call_is_told_once_however_many_lines_give_it() {
        let partial = |event: Value, parent: Value| {
            json!({ "type": "stream_event", "event": event, "parent_tool_use_id": parent }).to_string()
        };
        let start = partial(
            json!({ "type": "message_start", "message": { "id": "a", "model": "m", "usage": {
                "input_tokens": 2, "cache_read_input_tokens": 1000, "cache_creation_input_tokens": 500,
                "output_tokens": 1,
            } } }),
            Value::Null,
        );
        let end = partial(
            json!({ "type": "message_delta", "usage": { "output_tokens": 435 } }),
            Value::Null,
        );
        let synthetic = json!({ "type": "assistant", "parent_tool_use_id": null, "message": { "id": "s",
            "model": "<synthetic>", "usage": { "input_tokens": 1, "cache_read_input_tokens": 1,
            "cache_creation_input_tokens": 1 }, "content": [] } })
        .to_string();
        let told = calls_told(&[
            synthetic,
            start,
            message("a", Value::Null, 1000, 1),
            message("a", Value::Null, 1000, 1),
            message("b", json!("toolu_1"), 90, 5),
            end,
            message("a", Value::Null, 1000, 435),
            message("c", Value::Null, 2000, 7),
        ]);
        let shown: Vec<(&str, i64, i64, bool)> = told
            .iter()
            .map(|c| (c.id.as_str(), c.prompt(), c.output, c.subagent))
            .collect();
        assert_eq!(
            shown,
            [
                ("a", 1502, 1, false),
                ("b", 592, 5, true),
                ("a", 1502, 435, false),
                ("c", 2502, 7, false)
            ]
        );
        assert_eq!((told[1].written_5m, told[1].written_1h), (0, 500));
    }

    // Each tool use is told once with a digest of its input, by which the same use is known again; a subagent's is not
    // the agent's. A compaction is told as one.
    #[test]
    fn a_tool_use_is_told_once_with_a_digest_of_its_input() {
        let using = |id: &str, parent: Value, input: Value| {
            json!({ "type": "assistant", "parent_tool_use_id": parent, "message": { "content": [
                { "type": "tool_use", "id": id, "name": "Bash", "input": input }] } })
            .to_string()
        };
        let lines = [
            using("t1", Value::Null, json!({ "command": "ls" })),
            using("t1", Value::Null, json!({ "command": "ls" })),
            using("t2", Value::Null, json!({ "command": "ls" })),
            using("t3", Value::Null, json!({ "command": "pwd" })),
            using("t4", json!("toolu_1"), json!({ "command": "ls" })),
            r#"{"type":"system","subtype":"compact_boundary"}"#.to_string(),
        ];
        let mut stream = Stream::default();
        let told: Vec<Seen> = lines
            .iter()
            .flat_map(|l| stream.line(l))
            .filter(|s| !matches!(s, Seen::Doing(_)))
            .collect();
        let digest_of = |s: &Seen| match s {
            Seen::Tool { digest, .. } => Some(*digest),
            _ => None,
        };
        assert_eq!(told.len(), 4, "{told:?}");
        assert_eq!(digest_of(&told[0]), digest_of(&told[1]));
        assert_ne!(digest_of(&told[0]), digest_of(&told[2]));
        assert!(matches!(&told[0], Seen::Tool { on: Some(on), .. } if on == "ls"));
        assert_eq!(told[3], Seen::Compacted);
    }

    /// A request that runs a program, as a test's command line.
    fn run_of(dir: &Path, program: &Path) -> Request {
        Request {
            work: 1,
            cell: "I1".into(),
            line: "engineer".into(),
            cli: config::Cli {
                kind: config::CliKind::Claude,
                command: program.display().to_string(),
                args: Vec::new(),
                config_dir: None,
            },
            model: "m".into(),
            effort: None,
            worktree: dir.to_path_buf(),
            config_dir: None,
            resume: None,
            fork: false,
            system: None,
            new_session: "s1".into(),
            prompt: "p".into(),
            images: Vec::new(),
            schema: Value::Null,
            envelope: Value::Null,
            allowed: Vec::new(),
            refused: Vec::new(),
            readable: Vec::new(),
            replay: None,
            regive: None,
            deliver: None,
            bare: true,
        }
    }

    // A stopped run ends with everything it started, what ignores the polite signal too, before the run gives its
    // reply: the program here starts a child that outlives the stream, and neither takes the signal to end.
    #[test]
    fn a_stopped_run_takes_its_whole_process_group_with_it() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("wf-agents-stop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (program, child) = (dir.join("cli"), dir.join("child"));
        let line = message("a", Value::Null, 1000, 1);
        let script = format!(
            "#!/bin/sh\ntrap '' TERM\nsleep 120 &\necho $! > {}\nprintf '%s\\n' '{line}'\nwait\n",
            quote(&child.display().to_string())
        );
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let began = std::time::Instant::now();
        let reply = Claude.run(&run_of(&dir, &program), &|seen| match seen {
            Seen::Call(_) => Next::Stop("past its line".into()),
            _ => Next::Go,
        });
        assert_eq!(reply.stopped.as_deref(), Some("past its line"), "{reply:?}");
        assert_eq!(reply.error.as_deref(), Some("past its line"), "{reply:?}");
        assert!(reply.output.is_none(), "{reply:?}");
        assert!(began.elapsed() < Duration::from_secs(60), "{:?}", began.elapsed());
        let pid = std::fs::read_to_string(&child).unwrap().trim().to_string();
        let alive = || {
            Command::new("kill")
                .args(["-0", &pid])
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
        };
        // What is killed is gone at once, but for its parent's taking it in.
        let gone = (0..100).any(|_| {
            if alive() {
                std::thread::sleep(Duration::from_millis(50));
                return false;
            }
            true
        });
        let _ = std::fs::remove_dir_all(&dir);
        assert!(gone, "process {pid} outlived the stop");
    }

    /// Whether a process stands, the zombie its parent has not reaped apart.
    fn alive(pid: u32) -> bool {
        Command::new("ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .output()
            .is_ok_and(|o| {
                let stat = String::from_utf8_lossy(&o.stdout);
                !stat.trim().is_empty() && !stat.trim().starts_with('Z')
            })
    }

    // When the engine goes away the pipe its overseer waits on closes, and the run's group ends, what ignores the polite
    // signal too; an overseer dropped because the run went on to its end leaves the group alone. Taking the pipe closes
    // it as the end of the engine's process does.
    #[test]
    fn a_run_outlives_no_engine_and_its_overseer_only_acts_when_the_engine_is_gone() {
        let group = || {
            Command::new("sh")
                .args(["-c", "trap '' TERM; sleep 120 & wait"])
                .process_group(0)
                .spawn()
                .unwrap()
        };
        let mut left = group();
        drop(Overseer::of(left.id()).unwrap());
        std::thread::sleep(Duration::from_millis(100));
        assert!(alive(left.id()), "a dropped overseer stopped the group");
        signal_group(left.id(), "KILL");
        let _ = left.wait();

        let mut orphan = group();
        let mut overseer = Overseer::of(orphan.id()).unwrap();
        drop(overseer.0.stdin.take());
        let ended = (0..100).any(|_| {
            if alive(orphan.id()) {
                std::thread::sleep(Duration::from_millis(100));
                return false;
            }
            true
        });
        signal_group(orphan.id(), "KILL");
        let _ = orphan.wait();
        assert!(ended, "the group outlived its overseer's pipe");
    }

    // A run that ends by itself ends as ever under a watcher that lets it go on: the process group changes nothing.
    #[test]
    fn a_run_let_go_on_ends_as_ever() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("wf-agents-go-on-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let program = dir.join("cli");
        let result =
            r#"{"type":"result","subtype":"success","result":"{\"a\":1}","session_id":"s1","total_cost_usd":0.5}"#;
        std::fs::write(&program, format!("#!/bin/sh\nprintf '%s\\n' '{result}'\n")).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let reply = Claude.run(&run_of(&dir, &program), &|_| Next::Go);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(reply.stopped.is_none(), "{reply:?}");
        assert_eq!(reply.output.unwrap()["a"], 1);
    }
}
