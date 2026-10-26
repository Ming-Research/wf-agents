//! One round of an agent: one run of its CLI in the work's worktree, from the input the engine gives to the return
//! the agent hands in. It is a trait so that the engine's tests can script the agents; [`Claude`] runs `claude -p`.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::Value;

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
    /// The CLI's session to resume; none for the line's first round, which starts `new_session`.
    pub resume: Option<String>,
    pub new_session: String,
    pub prompt: String,
    /// What the return must be, as JSON Schema.
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
    /// Tokens and cost as the CLI reported them.
    pub usage: Value,
    pub compactions: i64,
    /// Why the run failed, when it did.
    pub error: Option<String>,
}

pub trait Agents {
    fn run(&self, r: &Request) -> Reply;
}

/// Agents run with `claude -p`.
pub struct Claude;

impl Claude {
    fn command(r: &Request) -> Command {
        let mut c = Command::new(&r.cli.command);
        c.args(&r.cli.args);
        c.args([
            "-p",
            "--output-format",
            "stream-json",
            "--verbose",
            "--model",
            r.model.as_str(),
        ]);
        c.arg("--json-schema").arg(r.schema.to_string());
        if let Some(e) = &r.effort {
            c.args(["--effort", e.as_str()]);
        }
        match &r.resume {
            Some(s) => c.args(["--resume", s.as_str()]),
            None => c.args(["--session-id", r.new_session.as_str()]),
        };
        c.current_dir(&r.worktree);
        c
    }
}

impl Agents for Claude {
    fn run(&self, r: &Request) -> Reply {
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
                    reply.usage = serde_json::json!({ "started": true });
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
                reply.usage = serde_json::json!({
                    "usage": v["usage"],
                    "cost_usd": v["total_cost_usd"],
                    "turns": v["num_turns"],
                    "duration_ms": v["duration_ms"],
                    "models": v["modelUsage"],
                });
                if v["is_error"] == true || v["subtype"] != "success" {
                    reply.error = Some(format!(
                        "{}: {}",
                        v["subtype"].as_str().unwrap_or("error"),
                        v["result"].as_str().unwrap_or_default()
                    ));
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
        assert_eq!(r.usage["cost_usd"], 0.5);
        assert!(r.error.is_none());
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
