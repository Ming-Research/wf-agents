//! One round of an agent: one run of its CLI in the work's worktree, from the message the engine sends to the return
//! the agent hands in. It is a trait so that the engine's tests can script the agents; [`Claude`] runs `claude -p`.

use std::io::Write;
use std::path::PathBuf;
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
    /// What the return must be, as JSON Schema; the CLI is given only its envelope.
    pub schema: Value,
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
    fn run(&self, r: &Request) -> Reply;

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
    // next would leave nothing of the session's prompt cache. The CLI is given only the fields every return of the
    // graph has, with their types, which is the same for every cell; the cell's own answer is a conditional input of the
    // message, and the engine checks the return against the cell's whole schema. Without the types the model hands
    // in each field as the JSON text of its value.
    out.push("--json-schema".into());
    out.push(envelope(&r.schema).to_string());
    if let Some(e) = &r.effort {
        out.extend(["--effort".to_string(), e.clone()]);
    }
    match &r.resume {
        Some(s) => out.extend(["--resume".to_string(), s.clone()]),
        None => out.extend(["--session-id".to_string(), r.new_session.clone()]),
    }
    out
}

/// What a round's record says of its call: the whole command line (the structured-output schema in it), the
/// environment the engine set, the session it started or resumed, and the engine's and the CLI's versions.
pub fn call(r: &Request, command: &[String], cli_version: &str) -> Value {
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
        "engine": crate::VERSION,
        "cli": cli_version,
    })
}

/// Agents run with `claude -p`.
pub struct Claude;

impl Claude {
    fn command(r: &Request) -> Command {
        let line = command_line(r);
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
    fn run(&self, r: &Request) -> Reply {
        let reply = Claude::once(r);
        // A session whose first round was cut off exists, though no round of it ever ended: resume it.
        let in_use = reply.error.as_deref().is_some_and(|e| e.contains("already in use"));
        if r.resume.is_none() && reply.output.is_none() && in_use {
            let again = Request {
                resume: Some(r.new_session.clone()),
                ..r.clone()
            };
            let mut reply = Claude::once(&again);
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
    fn once(r: &Request) -> Reply {
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
        if let Some(mut stdin) = child.stdin.take()
            && let Err(e) = stdin.write_all(r.prompt.as_bytes())
        {
            reply.error = Some(format!("giving the prompt: {e}"));
        }
        let out = match child.wait_with_output() {
            Ok(o) => o,
            Err(e) => {
                reply.error = Some(format!("{}: {e}", r.cli.command));
                return reply;
            }
        };
        read_stream(&String::from_utf8_lossy(&out.stdout), &mut reply);
        if reply.output.is_none() && reply.error.is_none() {
            let err = String::from_utf8_lossy(&out.stderr);
            reply.error = Some(format!(
                "{} exited with {} and no result: {}",
                r.cli.command,
                out.status,
                err.trim()
            ));
        }
        reply
    }
}

/// A return's top-level fields as a schema gives them, each with only its type: an agent's forms are under one field
/// as an anyOf of objects, so it is an object, which says that the other fields sit beside it.
fn envelope(schema: &Value) -> Value {
    let empty = serde_json::Map::new();
    let ps = schema["properties"].as_object().unwrap_or(&empty);
    let props: serde_json::Map<String, Value> = ps
        .iter()
        .map(|(k, v)| match v.get("type") {
            Some(t) => (k.clone(), json!({ "type": t })),
            None => {
                let beside: Vec<&str> = ps.keys().filter(|x| *x != k).map(String::as_str).collect();
                let desc = format!(
                    "Exactly one of the forms the prompt's schema lists under {k}; {} sit beside it, not inside it",
                    beside.join(" and ")
                );
                (k.clone(), json!({ "type": "object", "description": desc }))
            }
        })
        .collect();
    json!({
        "type": "object",
        "properties": props,
        "required": schema.get("required").cloned().unwrap_or_else(|| json!([])),
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
        let g = f.graph(crate::ENGINEER).unwrap();
        let shapes: Vec<Value> = g
            .cells
            .iter()
            .filter(|c| c.kind == "agent")
            .filter_map(|c| c.schema.as_ref().map(envelope))
            .collect();
        assert!(shapes.len() > 10);
        assert!(shapes.iter().all(|s| *s == shapes[0]));
        assert_eq!(shapes[0]["properties"][&g.within]["type"], "object");
        assert!(
            shapes[0]["properties"][&g.within]["description"]
                .as_str()
                .unwrap()
                .contains("side_reports")
        );
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
        };
        let line = command_line(&r);
        assert_eq!(&line[..4], ["claude", "--permission-mode", "acceptEdits", "-p"]);
        assert_eq!(&line[line.len() - 2..], ["--resume", "s1"]);
        // It reads no project instructions or settings by itself.
        assert!(line.windows(2).any(|w| w == ["--setting-sources", "user"]), "{line:?}");
        let c = call(&r, &line, "2.1.0 (Claude Code)");
        assert_eq!(c["session"]["resumed"], "s1");
        assert_eq!(c["env"]["CLAUDE_CONFIG_DIR"], "/data/agents/claude");
        // The structured-output schema is in the command line, and not recorded again beside it.
        assert!(c.get("schema").is_none());
        assert!(
            line.windows(2)
                .any(|w| w[0] == "--json-schema" && w[1].contains("\"progress\""))
        );
        assert_eq!(c["cli"], "2.1.0 (Claude Code)");
        assert_eq!(c["engine"], crate::VERSION);
        let cmd = Claude::command(&r);
        let env: Vec<_> = cmd.get_envs().collect();
        assert_eq!(
            env,
            [(
                std::ffi::OsStr::new("CLAUDE_CONFIG_DIR"),
                Some(std::ffi::OsStr::new("/data/agents/claude"))
            )]
        );
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
