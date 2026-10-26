//! One round of an agent: one run of its CLI in the work's worktree, from the input the engine gives to the return
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
    /// The CLI's session to resume; none for the line's first round, which starts `new_session`.
    pub resume: Option<String>,
    pub new_session: String,
    pub prompt: String,
    /// What the return must be, as JSON Schema; the prompt gives it too.
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
        // The CLI gives the model the schema with the session's tools, so a schema that changed from one round to
        // the next would leave nothing of the session's prompt cache. The CLI is given only the fields every return
        // of the graph has, with their types, which is the same for every cell; the cell's own schema is in the
        // prompt, and the engine checks the return against it. Without the types the model hands in each field as
        // the JSON text of its value.
        c.arg("--json-schema").arg(envelope(&r.schema).to_string());
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
        let reply = Claude::once(r);
        // A session whose first round was cut off exists, though no round of it ever ended: resume it.
        let in_use = reply.error.as_deref().is_some_and(|e| e.contains("already in use"));
        if r.resume.is_none() && reply.output.is_none() && in_use {
            return Claude::once(&Request {
                resume: Some(r.new_session.clone()),
                ..r.clone()
            });
        }
        reply
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
        let f = flow::compile(flow::FLOW, &flow::Context { relations }).unwrap();
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
