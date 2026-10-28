//! The command line an agent's round runs in: how it starts (the context, permissions, tools, model and reasoning
//! effort it is given), one run of it in the work's worktree from the message to the return, and what its stream
//! reports. The flags the installation gives every agent's command line, such as the tools it offers, stay in the
//! configuration; the flags that shape a session's context belong here. It is a trait so that the engine's tests can
//! script the agents; [`Claude`] runs `claude -p`.

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
            "--setting-sources",
            "user",
            "--model",
            r.model.as_str(),
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    // The CLI gives the model the schema with the session's tools, so a schema that changed from one round to the
    // next would leave nothing of the session's prompt cache: it is the graph's envelope, the same for every cell.
    out.push("--json-schema".into());
    out.push(r.envelope.to_string());
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

/// What a round's record says of its call: the whole command line (the structured-output schema in it), the
/// environment the engine set, the session it started or resumed, and the engine's and the CLI's versions.
pub fn call(r: &Request, command: &[String], cli_version: &str, engine: &str) -> Value {
    let session = match &r.resume {
        Some(s) => json!({ "resumed": s }),
        None => json!({ "started": r.new_session }),
    };
    let env = match &r.config_dir {
        Some(d) => json!({ "CLAUDE_CONFIG_DIR": d.display().to_string() }),
        None => json!({}),
    };
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
            c.env("CLAUDE_CONFIG_DIR", d);
        }
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
            c.env("CLAUDE_CONFIG_DIR", d);
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
        let mut stream = String::new();
        if let Some(out) = child.stdout.take() {
            for line in std::io::BufRead::lines(std::io::BufReader::new(out)).map_while(Result::ok) {
                if let Some(what) = activity_of(&line) {
                    activity(&what);
                }
                stream.push_str(&line);
                stream.push('\n');
            }
        }
        let status = child.wait();
        let err = stderr.and_then(|t| t.join().ok()).unwrap_or_default();
        read_stream(&stream, &mut reply);
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

/// What a line of the CLI's stream says the agent is doing: a tool it uses with what it uses it on, the start of
/// what it says, or a compaction; none for anything else.
pub fn activity_of(line: &str) -> Option<String> {
    let v: Value = serde_json::from_str(line).ok()?;
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

/// The schema the CLI is given for every round of a graph's agents, the same for every cell, since a schema that changed
/// from one round to the next leaves nothing of the session's prompt cache: a return's top-level fields, each with only
/// its type; the field an agent's forms are under is an object that holds none of the fields beside it. Required are
/// that field and those beside it that no form of any cell leaves out. Without the types the model hands in each field
/// as the JSON text of its value; the cell's own answer is a conditional input of the message, and the engine checks the
/// return against the cell's whole schema.
pub fn envelope(g: &flow::CompiledGraph) -> Value {
    let cells: Vec<&flow::CompiledCell> = g.cells.iter().filter(|c| c.kind == "agent").collect();
    let empty = serde_json::Map::new();
    let ps = cells
        .iter()
        .find_map(|c| c.schema.as_ref()?.get("properties")?.as_object())
        .unwrap_or(&empty);
    let beside: Vec<&str> = ps.keys().filter(|k| **k != g.within).map(String::as_str).collect();
    let props: serde_json::Map<String, Value> = ps
        .iter()
        .map(|(k, v)| {
            if *k != g.within {
                return (
                    k.clone(),
                    json!({ "type": v.get("type").cloned().unwrap_or(Value::Null) }),
                );
            }
            let desc = format!(
                "Exactly one of the forms the prompt's schema lists under {k}; {} sit beside it, not inside it",
                beside.join(" and ")
            );
            let not_inside: Vec<Value> = beside.iter().map(|b| json!({ "required": [b] })).collect();
            (
                k.clone(),
                json!({ "type": "object", "description": desc, "not": { "anyOf": not_inside } }),
            )
        })
        .collect();
    let left_out: std::collections::BTreeSet<&str> = cells
        .iter()
        .flat_map(|c| &c.forms)
        .flat_map(|f| &f.without)
        .map(String::as_str)
        .collect();
    let required: Vec<&str> = std::iter::once(g.within.as_str())
        .chain(beside.iter().copied().filter(|b| !left_out.contains(b)))
        .collect();
    json!({
        "type": "object",
        "properties": props,
        "required": required,
        "additionalProperties": false,
    })
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
            Some("result") => {
                if let Some(s) = v["session_id"].as_str() {
                    reply.session = s.to_string();
                }
                reply.usage = json!({
                    "usage": v["usage"],
                    "session_cost_usd": v["total_cost_usd"],
                    "turns": v["num_turns"],
                    "duration_ms": v["duration_ms"],
                    "models": v["modelUsage"],
                });
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
                let structured = v.get("structured_output").filter(|s| !s.is_null()).cloned();
                reply.output = structured.or_else(|| v["result"].as_str().and_then(json_in));
            }
            _ => {}
        }
    }
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
const KINDS: [(&str, &[&str]); 4] = [
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

    // The CLI is given one schema for a whole graph: its form's field holds none of the fields beside it, and a field
    // beside it that some form leaves out, such as progress, is not required.
    #[test]
    fn every_cell_of_a_graph_gives_the_cli_the_same_schema() {
        let relations = db::relations(db::SCHEMA).unwrap();
        let f = flow::compile(
            flow::FLOW,
            &flow::Context {
                relations,
                tables: db::index_of(db::SCHEMA).unwrap(),
            },
        )
        .unwrap();
        let g = f.graph("engineer").unwrap();
        let s = envelope(g);
        assert_eq!(s["properties"][&g.within]["type"], "object");
        assert!(
            s["properties"][&g.within]["description"]
                .as_str()
                .unwrap()
                .contains("side_reports")
        );
        assert_eq!(s["required"], json!(["outcome", "side_reports", "tell_owner"]));
        assert_eq!(
            s["properties"][&g.within]["not"],
            json!({ "anyOf": [{ "required": ["side_reports"] }, { "required": ["tell_owner"] },
                              { "required": ["progress"] }] })
        );
        let a = envelope(f.graph("architect").unwrap());
        assert_eq!(a["required"], json!(["outcome", "tell_owner"]));
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
            cell: "4".into(),
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
        };
        let line = command_line(&r);
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
        // The structured-output schema is in the command line, and not recorded again beside it.
        assert!(c.get("schema").is_none());
        assert!(
            line.windows(2)
                .any(|w| w[0] == "--json-schema" && w[1].contains("\"progress\""))
        );
        assert_eq!(c["cli"], "2.1.0 (Claude Code)");
        assert_eq!(c["engine"], "e1");
        let cmd = Claude::command(&r);
        let env: Vec<_> = cmd.get_envs().collect();
        assert_eq!(
            env,
            [(
                std::ffi::OsStr::new("CLAUDE_CONFIG_DIR"),
                Some(std::ffi::OsStr::new("/data/agents/claude"))
            )]
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
