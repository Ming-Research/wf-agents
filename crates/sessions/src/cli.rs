//! The command line an agent's round runs in: how it starts (the context, permissions, tools, model and reasoning
//! effort it is given), one run of it in the work's worktree from the message to the return, and what its stream
//! reports. The flags the installation gives every agent's command line, such as the tools it offers, stay in the
//! configuration; the flags that shape a session's context belong here. It is a trait so that the engine's tests can
//! script the agents; [`Claude`] runs `claude -p`.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

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
    pub new_session: String,
    /// The message, given on the CLI's standard input.
    pub prompt: String,
    /// What the return must be, as JSON Schema; the engine checks the return against it.
    pub schema: Value,
    /// The schema the CLI is given, the same for every cell of the graph: [`envelope`].
    pub envelope: Value,
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
/// streamed whole, not in pieces, so that the service checks its JSON; and three attempts at a return, not five, since
/// each attempt writes the whole return again.
pub const ENVIRONMENT: [(&str, &str); 2] = [
    ("CLAUDE_CODE_ENABLE_FINE_GRAINED_TOOL_STREAMING", "0"),
    ("MAX_STRUCTURED_OUTPUT_RETRIES", "3"),
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

/// What a round came to.
#[derive(Debug, Clone, Default)]
pub struct Reply {
    /// The return; none when the run handed in nothing that parses.
    pub output: Option<Value>,
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
}

pub trait Agents {
    /// Runs a round; `activity` is told what the agent is doing while it runs, as the CLI reports it.
    fn run(&self, r: &Request, activity: &dyn Fn(&str)) -> Reply;

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
    // The CLI gives the model the schema with the session's tools, so a schema that changed from one round to the
    // next would leave nothing of the session's prompt cache: it is the graph's envelope, the same for every cell. A
    // bare call is given no schema and no tools.
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
    let mut refused = r.refused.clone();
    refused.extend(r.readable.iter().map(|d| format!("Edit(/{}/**)", d.display())));
    // The line's permissions, after the CLI's own arguments: the CLI refuses what any rule of either refuses.
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
    match &r.resume {
        Some(s) => out.extend(["--resume".to_string(), s.clone()]),
        None => out.extend(["--session-id".to_string(), r.new_session.clone()]),
    }
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
    let session = match &r.resume {
        Some(s) => json!({ "resumed": s }),
        None => json!({ "started": r.new_session }),
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
    fn command(r: &Request) -> Command {
        let line = r.replay.clone().unwrap_or_else(|| command_line(r));
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
    fn run(&self, r: &Request, activity: &dyn Fn(&str)) -> Reply {
        let reply = Claude::once(r, activity);
        // A session whose first round was cut off exists, though no round of it ever ended: resume it.
        let in_use = reply.error.as_deref().is_some_and(|e| e.contains("already in use"));
        if r.resume.is_none() && reply.output.is_none() && in_use {
            let again = Request {
                resume: Some(r.new_session.clone()),
                ..r.clone()
            };
            let mut reply = Claude::once(&again, activity);
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

impl Claude {
    /// One run of the CLI: the message on its standard input, its stream read line by line as it comes, each line
    /// that says what the agent does told to `activity`.
    fn once(r: &Request, activity: &dyn Fn(&str)) -> Reply {
        let mut reply = Reply {
            session: r.resume.clone().unwrap_or_else(|| r.new_session.clone()),
            ..Reply::default()
        };
        let child = Claude::command(r)
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
        // The standard input is closed once the message is written, so the CLI knows it has all of it.
        if let Some(mut stdin) = child.stdin.take()
            && let Err(e) = stdin.write_all(r.prompt.as_bytes())
        {
            reply.error = Some(format!("giving the prompt: {e}"));
        }
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
            for line in std::io::BufRead::lines(std::io::BufReader::new(out)).map_while(Result::ok) {
                if let Some(what) = stream.line(&line) {
                    activity(&what);
                }
            }
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

/// What the command line's stream gave as it came: its lines kept for the end but for its partial messages, which are
/// passed over once the StructuredOutput inputs are rebuilt from them.
#[derive(Debug, Default)]
pub struct Stream {
    kept: String,
    partials: Partials,
}

impl Stream {
    /// Takes one line of the stream; what it says the agent does, if anything.
    pub fn line(&mut self, line: &str) -> Option<String> {
        if self.partials.take(line) {
            return None;
        }
        let parsed: Option<Value> = serde_json::from_str(line).ok();
        if let Some(v) = &parsed
            && v["type"] == "stream_event"
        {
            self.partials.event(v);
            return None;
        }
        self.kept.push_str(line);
        self.kept.push('\n');
        activity_of_value(&parsed?)
    }

    /// What the run came to, from the lines kept: [`read_stream`], with each attempt at a return the command line
    /// refused.
    pub fn end(self, reply: &mut Reply) {
        read_stream(&self.kept, reply);
        reply.refused = self.partials.refused(&refusals(&self.kept));
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
    /// Takes a line that is a partial message as the command line writes it, its type first; whether it was one. Only
    /// the lines that start, add to or end a StructuredOutput input are parsed.
    fn take(&mut self, line: &str) -> bool {
        if !line.starts_with(r#"{"type":"stream_event""#) {
            return false;
        }
        let wanted = line.contains("content_block_start")
            || line.contains("message_start")
            || (self.open.is_some() && (line.contains("input_json_delta") || line.contains("content_block_stop")));
        if wanted && let Ok(v) = serde_json::from_str::<Value>(line) {
            self.event(&v);
        }
        true
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

/// What a parsed line of the CLI's stream says the agent is doing.
fn activity_of_value(v: &Value) -> Option<String> {
    let cut = |s: &str| -> String {
        let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
        if one.chars().count() > 160 {
            format!("{}…", one.chars().take(160).collect::<String>())
        } else {
            one
        }
    };
    match v["type"].as_str()? {
        "assistant" => {
            let said: Vec<String> = v["message"]["content"]
                .as_array()?
                .iter()
                .filter_map(|c| match c["type"].as_str()? {
                    "tool_use" => {
                        let name = c["name"].as_str().unwrap_or("a tool");
                        let on = ["command", "file_path", "path", "pattern", "description", "url", "query"]
                            .iter()
                            .find_map(|k| c["input"][*k].as_str());
                        Some(match on {
                            Some(o) => format!("{name}: {}", cut(o)),
                            None => name.to_string(),
                        })
                    }
                    "text" => c["text"]
                        .as_str()
                        .filter(|t| !t.trim().is_empty())
                        .map(|t| format!("says: {}", cut(t))),
                    _ => None,
                })
                .collect();
            (!said.is_empty()).then(|| said.join("; "))
        }
        "system" if v["subtype"] == "compact_boundary" => Some("compacting the session".to_string()),
        _ => None,
    }
}

/// The schema the CLI is given for every round of a graph's agents, the same for every cell, since a schema that
/// changed from one round to the next leaves nothing of the session's prompt cache. It holds a return's top-level
/// fields: the short fields every return holds first, each with only its type, and the field an agent's forms are under
/// last, so that only closing brackets come after the long answer; that field with the [`shape`] of every form of every
/// agent's cell of the graph taken together ([`merged`]), each object with its fields and each list with its items,
/// down to every value. It refuses what no agent's cell of the graph takes, as far as types and fields tell: an object
/// requires the fields every one of its alternatives requires, at the top those every agent's cell requires, so not a
/// field some form leaves out (`progress`, which `tools` leaves out); and it takes no field beyond those it lists when
/// none of its alternatives does, the field of the forms included, so that a field written inside the form it does not
/// belong to is refused, and a field every return holds written inside the field of the forms is too. The command line
/// refuses such a return saying where and why, and the model hands it in again within the run, one more call that
/// rewrites the return, where the engine would send it back and run the round again: in a trial one round handed in no
/// `tell_owner`, and another wrote `effects` inside `take` instead of beside it. When the command line gives up after
/// its attempts, the engine takes the last as any return (`unparsed` in the engine), moving a field every return holds
/// up from inside the field of the forms. A field its object may leave out may also be null, which the engine takes as
/// the field left out. Without the types the model hands in each field as the JSON text of its value, and with the
/// field of the forms an object with no fields it handed in the whole form as a string holding its JSON.
pub fn envelope(g: &flow::CompiledGraph) -> Value {
    let schemas: Vec<&Value> = g
        .cells
        .iter()
        .filter(|c| c.kind == "agent")
        .filter_map(|c| c.schema.as_ref())
        .collect();
    // The fields beside the forms', of every agent's cell, in the order the cells give them.
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
        .map(shape)
        .reduce(|a, b| merged(&a, &b));
    let mut within = serde_json::Map::new();
    within.insert("type".into(), json!("object"));
    within.insert("description".into(), json!(desc));
    for k in ["properties", "required", "additionalProperties"] {
        if let Some(v) = forms.as_ref().and_then(|f| f.get(k)) {
            within.insert(k.into(), v.clone());
        }
    }
    props.insert(g.within.clone(), Value::Object(within));
    let required: Vec<String> = props
        .keys()
        .filter(|k| **k == g.within || schemas.iter().all(|s| required_of(s).contains(&k.as_str())))
        .cloned()
        .collect();
    json!({
        "type": "object",
        "properties": props,
        "required": required,
        "additionalProperties": false,
    })
}

/// The shape of a part of a cell's schema: the type of each value, each object with all its fields, those it requires
/// and whether it takes no other, and each list with its items, and nothing else; alternatives taken together as one
/// ([`merged`]). A value with no type of its own takes that of its fixed value or of all its values; a field its object
/// may leave out may also be null, which the engine takes as the field left out.
fn shape(s: &Value) -> Value {
    let Some(o) = s.as_object() else { return json!({}) };
    let mut out = serde_json::Map::new();
    if let Some(t) = type_of(o) {
        out.insert("type".into(), t);
    }
    if let Some(ps) = o.get("properties").and_then(Value::as_object) {
        let required = required_of(s);
        let fields: serde_json::Map<String, Value> = ps
            .iter()
            .map(|(k, p)| {
                let mut x = shape(p);
                if !required.contains(&k.as_str()) {
                    or_null(&mut x);
                }
                (k.clone(), x)
            })
            .collect();
        out.insert("properties".into(), Value::Object(fields));
        let listed: Vec<&String> = ps.keys().filter(|k| required.contains(&k.as_str())).collect();
        if !listed.is_empty() {
            out.insert("required".into(), json!(listed));
        }
    }
    if closed(s) {
        out.insert("additionalProperties".into(), json!(false));
    }
    if let Some(it) = o.get("items") {
        out.insert("items".into(), shape(it));
    }
    let alts = ["anyOf", "oneOf"]
        .iter()
        .filter_map(|k| o.get(*k)?.as_array())
        .flatten()
        .map(shape)
        .reduce(|a, b| merged(&a, &b));
    match alts {
        Some(a) if out.is_empty() => a,
        Some(a) => merged(&Value::Object(out), &a),
        None => Value::Object(out),
    }
}

/// A schema's own type, or that of its fixed value or of all its values when they share one.
fn type_of(o: &serde_json::Map<String, Value>) -> Option<Value> {
    if let Some(t) = o.get("type") {
        return Some(t.clone());
    }
    let values = o
        .get("const")
        .into_iter()
        .chain(o.get("enum").and_then(Value::as_array).into_iter().flatten());
    let mut types: Vec<&str> = Vec::new();
    for v in values {
        let t = match v {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        };
        if !types.contains(&t) {
            types.push(t);
        }
    }
    match types.as_slice() {
        [one] => Some(json!(one)),
        _ => None,
    }
}

/// The types a shape's type names, in order.
fn types(t: &Value) -> Vec<&str> {
    match t {
        Value::String(s) => vec![s.as_str()],
        Value::Array(xs) => xs.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    }
}

/// A shape that may also be null; one of no type is left as it is, as it may be anything.
fn or_null(x: &mut Value) {
    let Some(t) = x.get("type") else { return };
    let mut ts: Vec<&str> = types(t);
    if !ts.contains(&"null") {
        ts.push("null");
    }
    let ts = json!(ts);
    x["type"] = ts;
}

/// The fields a schema or a shape requires of an object.
fn required_of(s: &Value) -> Vec<&str> {
    s.get("required")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

/// Whether a schema or a shape takes no field of an object beyond those it lists.
fn closed(s: &Value) -> bool {
    s.get("additionalProperties") == Some(&Value::Bool(false))
}

/// Whether a shape may be an object: it names that type, or none, as it may be anything.
fn takes_object(s: &Value) -> bool {
    s.get("type").is_none_or(|t| types(t).contains(&"object"))
}

/// Two shapes taken together, as alternatives of one value: the types of both, the fields of both, a field both have
/// taken together too, and the items of both lists taken together; of no type when either is, as it may be anything.
/// Of the shapes that may be an object, the fields all of them require are required, and no other field is taken when
/// none of them takes one.
fn merged(a: &Value, b: &Value) -> Value {
    let mut out = serde_json::Map::new();
    if let (Some(x), Some(y)) = (a.get("type"), b.get("type")) {
        let mut ts: Vec<&str> = types(x);
        for t in types(y) {
            if !ts.contains(&t) {
                ts.push(t);
            }
        }
        out.insert(
            "type".into(),
            match ts.as_slice() {
                [one] => json!(one),
                _ => json!(ts),
            },
        );
    }
    let (pa, pb) = (a.get("properties"), b.get("properties"));
    if pa.is_some() || pb.is_some() {
        let mut fields = pa.and_then(Value::as_object).cloned().unwrap_or_default();
        for (k, v) in pb.and_then(Value::as_object).into_iter().flatten() {
            let x = match fields.get(k) {
                Some(f) => merged(f, v),
                None => v.clone(),
            };
            fields.insert(k.clone(), x);
        }
        out.insert("properties".into(), Value::Object(fields));
    }
    let objects: Vec<&Value> = [a, b].into_iter().filter(|s| takes_object(s)).collect();
    if let Some(first) = objects.first() {
        let required: Vec<&str> = required_of(first)
            .into_iter()
            .filter(|k| objects.iter().all(|s| required_of(s).contains(k)))
            .collect();
        if !required.is_empty() {
            out.insert("required".into(), json!(required));
        }
        if objects.iter().all(|s| closed(s)) {
            out.insert("additionalProperties".into(), json!(false));
        }
    }
    match (a.get("items"), b.get("items")) {
        (Some(x), Some(y)) => {
            out.insert("items".into(), merged(x, y));
        }
        (Some(x), None) | (None, Some(x)) => {
            out.insert("items".into(), x.clone());
        }
        (None, None) => {}
    }
    Value::Object(out)
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
                    reply.error = Some(format!("{}: {why}", v["subtype"].as_str().unwrap_or("error")));
                }
                reply.result = v["result"].as_str().map(str::to_string);
                let structured = v.get("structured_output").filter(|s| !s.is_null()).cloned();
                reply.output = structured.or_else(|| v["result"].as_str().and_then(json_in));
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
const KINDS: [(&str, &[&str]); 5] = [
    // First, since what the command line reports of it may quote the agent's return, which may hold any words.
    ("unparsed", &["max_structured_output_retries"]),
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

    // The CLI is given one schema for a whole graph: the short fields every return holds first, the field its forms are
    // under last, with the shape of every form of the graph. Required are the fields every agent's cell requires, so
    // not `progress`, which the form `tools` leaves out, and the field of the forms takes no field beyond those of the
    // forms, so a field beside it written inside it is refused. Nothing a cell's own schema takes is refused.
    #[test]
    fn every_cell_of_a_graph_gives_the_cli_the_same_schema() {
        let f = compiled();
        let g = f.graph("engineer").unwrap();
        let s = envelope(g);
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
        let a = envelope(f.graph("architect").unwrap());
        assert_eq!(keys(&a), ["tell_owner", "progress", "flow_feedback", "outcome"]);
        assert_eq!(a["required"], json!(["tell_owner", "flow_feedback", "outcome"]));
        // A project that does not ask for flow feedback gives its agents no such field.
        let off = envelope(&g.clone().as_asked(|_| false));
        assert!(off["properties"].get("flow_feedback").is_none(), "{off}");
        assert_eq!(off["required"], json!(["tell_owner", "side_reports", "outcome"]));
        // The field of the forms is never an object with no fields: it holds every form of every agent's cell, each
        // value with its type, which a string holding the JSON of one does not have.
        for g in &f.graphs {
            let s = envelope(g);
            let within = &s["properties"][&g.within];
            assert!(
                within["properties"].as_object().is_some_and(|p| !p.is_empty()),
                "{within}"
            );
            for c in g.cells.iter().filter(|c| c.kind == "agent") {
                let schema = c.schema.as_ref().unwrap();
                let form = shape(&schema["properties"][&g.within]);
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

    /// Where a return does not fit the envelope, as the command line checks it and in its words: a value of a type the
    /// schema does not name, a field required and missing, or a field its object takes no more of; none when it fits.
    fn refused(s: &Value, x: &Value, at: &str) -> Option<String> {
        let here = if at.is_empty() { "root" } else { at };
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

    // In a trial, a round of the architect's cell U1 handed in no `tell_owner`, which every return of the graph holds,
    // and the engine sent the return back, which cost a whole round. The command line now refuses it, saying what is
    // missing, as the installed one did when tried (2.1.312), and the agent hands it in again within its run.
    #[test]
    fn a_return_without_a_field_every_return_holds_is_refused_by_the_command_line() {
        let f = compiled();
        let g = f.graph("architect").unwrap();
        let schema = g.cell("U1").unwrap().schema.clone().unwrap();
        let s = envelope(g);
        let notebook = json!([
            { "section": "project_now", "text": "The parser: half done." },
            { "section": "unclear", "text": "Nothing." }
        ]);
        let mut ret = json!({
            "progress": "Reading where each line stands",
            "flow_feedback": "",
            "outcome": { "notebook": notebook },
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
    }

    // In a trial, a round of the architect's cell H5 wrote `effects` inside `take`, beside `way` and `why`, instead of
    // beside `take`, and the engine sent the return back, which cost a whole round. The command line now refuses it,
    // saying where, as the installed one did when tried (2.1.312), where the agent moved it beside `take`.
    #[test]
    fn a_field_written_inside_a_form_it_does_not_belong_to_is_refused_by_the_command_line() {
        let f = compiled();
        let g = f.graph("architect").unwrap();
        let schema = g.cell("H5").unwrap().schema.clone().unwrap();
        let s = envelope(g);
        let take = json!({ "way": "nothing", "why": "Nothing in it to take in" });
        let mut inside = take.clone();
        inside["effects"] = json!({});
        let mut ret = json!({
            "tell_owner": [],
            "progress": "Taking in an observation",
            "flow_feedback": "",
            "outcome": { "take": inside },
        });
        assert!(!flow::valid(&schema, &ret));
        assert_eq!(
            refused(&s, &ret, "").as_deref(),
            Some("/outcome/take: must NOT have additional properties ('effects' is not allowed)")
        );
        ret["outcome"] = json!({ "take": take, "effects": {} });
        assert!(flow::valid(&schema, &ret), "{ret}");
        assert_eq!(refused(&s, &ret, ""), None);
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

    // A shape keeps only the types of the values, each object with its fields, those it requires and whether it takes
    // no other, and each list with its items: no value is fixed, and a field its object may leave out may also be null.
    // Alternatives come together as one: an object requires what all of them require and takes no other field when
    // none of them does, an alternative that is no object leaving that to the others; a value of no type of its own
    // takes that of its values.
    #[test]
    fn a_shape_keeps_only_the_types_and_the_fields_of_the_values() {
        let s = json!({
            "description": "What this round comes to",
            "anyOf": [
                { "type": "object", "required": ["reading"], "additionalProperties": false, "properties": {
                    "reading": { "type": "object", "required": ["direction"], "additionalProperties": false,
                                 "properties": {
                        "direction": { "type": "string", "description": "The direction" },
                        "asks": { "type": "array", "minItems": 1, "items": { "type": "string" } },
                    } },
                } },
                { "type": "object", "required": ["verdict"], "properties": {
                    "verdict": { "oneOf": [
                        { "type": "object", "required": ["verdict"], "additionalProperties": false,
                          "properties": { "verdict": { "const": "holds" } } },
                        { "type": "object", "required": ["verdict", "kind"], "additionalProperties": false,
                          "properties": {
                            "verdict": { "const": "fails" },
                            "kind": { "enum": ["a", "b"] },
                        } },
                    ] },
                } },
                { "type": "object", "required": ["reading"], "additionalProperties": false, "properties": {
                    "reading": { "type": "string" },
                } },
            ],
        });
        assert_eq!(
            shape(&s),
            json!({ "type": "object", "properties": {
                "reading": { "type": ["object", "string"], "properties": {
                    "direction": { "type": "string" },
                    "asks": { "type": ["array", "null"], "items": { "type": "string" } },
                }, "required": ["direction"], "additionalProperties": false },
                "verdict": { "type": "object", "properties": {
                    "verdict": { "type": "string" },
                    "kind": { "type": "string" },
                }, "required": ["verdict"], "additionalProperties": false },
            } })
        );
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
        // A bare quote inside text in the owner's language, which the model wrote unescaped.
        let broken =
            "{\"outcome\": {\"answer\": {\"why\": \"\u{4ed6}\u{8bf4}\"\u{4e0d}\u{884c}\"\"}}, \"progress\": \"p\"}";
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
        assert!(why.ends_with("bytes)") && !why.contains("\u{4e0d}\u{884c}"), "{why}");
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
        let r = Request {
            work: 1,
            cell: "I1".into(),
            line: "engineer".into(),
            cli: config::Cli {
                kind: config::CliKind::Claude,
                command: "claude".into(),
                args: vec!["--permission-mode".into(), "acceptEdits".into()],
                config_dir: None,
            },
            model: "m".into(),
            effort: Some("high".into()),
            worktree: PathBuf::from("/w"),
            config_dir: Some(PathBuf::from("/data/agents/claude")),
            resume: Some("s1".into()),
            new_session: "s2".into(),
            prompt: "p".into(),
            schema: json!({ "type": "object", "properties": { "outcome": {}, "progress": { "type": "string" } } }),
            envelope: json!({ "type": "object", "properties": { "outcome": {}, "progress": { "type": "string" } } }),
            refused: config::Permissions::default().refused("engineer"),
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
        assert_eq!(&line[..4], ["claude", "--permission-mode", "acceptEdits", "-p"]);
        // Heavy work is refused on every line; an engineer may still edit and commit.
        assert!(line.iter().any(|a| a == "Bash(cargo *)") && line.iter().any(|a| a == "Bash(npm run *)"));
        assert!(
            !line.iter().any(|a| a == "Edit" || a == "Bash(git commit *)"),
            "{line:?}"
        );
        // The review and the advisor only read; the adversarial tester writes tests.
        for l in ["review", "advisor"] {
            let refused = config::Permissions::default().refused(l);
            assert!(refused.iter().any(|a| a == "Edit") && refused.iter().any(|a| a == "Bash(git commit *)"));
        }
        assert!(
            !config::Permissions::default()
                .refused("adversarial")
                .iter()
                .any(|a| a == "Edit")
        );
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
                && env.len() == 3,
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
}
