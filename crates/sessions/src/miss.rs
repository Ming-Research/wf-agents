//! A round that missed its session's prompt cache. A round that resumes its session within the cache's lifetime since
//! the session's last call reads from the cache what the session holds; one that writes most of it to the cache again
//! instead paid for writing what it should have read. A miss is a fault of wf-agents, whichever project it came up in,
//! so the engine tells the owner of it, with the parts of the command line that differ from the session's round before,
//! where a cause most often shows, or that none does.

use db::rusqlite::{Connection, OptionalExtension};
use serde_json::{Value, json};

use crate::round::{Round, RoundId};
use crate::{choose, cost, warm};

/// The flags that name the session, which differ between rounds by design.
const SESSION_FLAGS: [&str; 2] = ["--resume", "--session-id"];

/// The flag whose value is the round's hooks.
const SETTINGS: &str = "--settings";

/// A part of the command line that differs from the session's round before: its name, and its value there and in this
/// round, none where the round had no such part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Differs {
    pub part: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// A round's miss of its session's prompt cache.
#[derive(Debug, Clone, PartialEq)]
pub struct Miss {
    /// The session's round before, whose command line this round's is compared with.
    pub before: RoundId,
    /// How long after the session's last call the round started, in milliseconds.
    pub after_ms: i64,
    /// The tokens the round wrote to the cache again: those its first call wrote, or the whole round's when its record
    /// does not list its calls.
    pub written: i64,
    /// The tokens the session held: the last prompt of its round before, as recorded.
    pub held: i64,
    /// What writing them again cost beyond reading them, in US dollars; none when the round's record does not tell.
    pub cost_usd: Option<f64>,
    /// The parts of the command line that differ from the round before; empty when none does.
    pub differs: Vec<Differs>,
}

impl Miss {
    /// The parts that differ as `cache_misses.differs` keeps them, the same text for the same cause.
    pub fn differs_json(&self) -> Value {
        Value::Array(
            self.differs
                .iter()
                .map(|d| json!({ "part": d.part, "before": d.before, "after": d.after }))
                .collect(),
        )
    }
}

/// Whether a round that resumed a CLI session missed its prompt cache: it started within the cache's lifetime since the
/// session's last call, and wrote to the cache again at least half of what the session held. `before` are the
/// session's rounds before it, oldest first, `call` is its call as its record keeps it, and `usage` its usage record
/// with its own cost. None when the record cannot tell: no round before recorded its calls or its command line.
pub fn missed(
    conn: &Connection,
    cli_session: &str,
    before: &[Round],
    round: RoundId,
    call: &Value,
    usage: &Value,
) -> Result<Option<Miss>, String> {
    let Some(prev) = before.iter().rev().find(|r| r.input["call"]["command"].is_array()) else {
        return Ok(None);
    };
    let Some(held) = choose::context(before).filter(|h| *h > 0) else {
        return Ok(None);
    };
    let Some(last) = last_call(conn, cli_session, before)? else {
        return Ok(None);
    };
    let started: Option<i64> = conn
        .query_row(
            "SELECT started_at FROM rounds WHERE work = ?1 AND n = ?2",
            [round.work, round.n],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(started) = started else {
        return Ok(None);
    };
    let after_ms = (started - last).max(0);
    if after_ms >= warm::CACHE_MS {
        return Ok(None);
    }
    let Some(written) = written(usage) else {
        return Ok(None);
    };
    if written * 2 < held {
        return Ok(None);
    }
    Ok(Some(Miss {
        before: prev.id,
        after_ms,
        written,
        held,
        cost_usd: cost::rewritten(usage, written),
        differs: differs(&prev.input["call"], call),
    }))
}

/// When a session's last call ended: its latest round that ran the command line, or its latest keep-warm ping that
/// reached the CLI, whichever came last.
fn last_call(conn: &Connection, cli_session: &str, rounds: &[Round]) -> Result<Option<i64>, String> {
    let round = rounds
        .iter()
        .filter(|r| r.usage.is_some())
        .filter_map(|r| r.ended_at)
        .max();
    let ping: Option<i64> = conn
        .query_row(
            "SELECT max(p.ended_at) FROM pings p JOIN sessions s ON s.id = p.session
             WHERE s.cli_session = ?1 AND p.usage IS NOT NULL",
            [cli_session],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(round.max(ping))
}

/// The tokens a round wrote to the cache: its first call's when its record lists its calls, else the whole round's.
fn written(usage: &Value) -> Option<i64> {
    let u = usage.get("usage")?;
    let first = u["iterations"].as_array().and_then(|xs| xs.first());
    match first {
        Some(call) => call["cache_creation_input_tokens"].as_i64(),
        None => u["cache_creation_input_tokens"].as_i64(),
    }
}

/// The parts of a call's command line that differ from those of the call before, in the order the later call has them,
/// then those only the call before had.
pub fn differs(before: &Value, after: &Value) -> Vec<Differs> {
    let (was, now) = (parts(before), parts(after));
    let mut out = Vec::new();
    for (part, value) in &now {
        let old = was.iter().find(|(p, _)| p == part).map(|(_, v)| v);
        if old != Some(value) {
            out.push(Differs {
                part: part.clone(),
                before: old.cloned(),
                after: Some(value.clone()),
            });
        }
    }
    for (part, value) in &was {
        if !now.iter().any(|(p, _)| p == part) {
            out.push(Differs {
                part: part.clone(),
                before: Some(value.clone()),
                after: None,
            });
        }
    }
    out
}

/// The parts of a call that shape its session's prompt, each by its name with its value: the command with the
/// installation's arguments to it, each flag with its values, the CLI's and the engine's versions, the environment and
/// the working directory. The flag that names the session is left out, and so are the commands of the round's hooks,
/// which name the round; which hooks it sets is kept.
fn parts(call: &Value) -> Vec<(String, String)> {
    let words: Vec<String> = call["command"]
        .as_array()
        .map(|xs| xs.iter().map(word).collect())
        .unwrap_or_default();
    let p = words.iter().position(|w| w == "-p").unwrap_or(words.len());
    let mut out: Vec<(String, String)> = vec![("command".to_string(), words[..p].join(" "))];
    let mut at: Option<usize> = None;
    for w in &words[p..] {
        if w.starts_with('-') {
            if SESSION_FLAGS.contains(&w.as_str()) {
                at = None;
                continue;
            }
            at = Some(match out.iter().position(|(name, _)| name == w) {
                Some(i) => i,
                None => {
                    out.push((w.clone(), String::new()));
                    out.len() - 1
                }
            });
            continue;
        }
        let Some(i) = at else { continue };
        let value = if out[i].0 == SETTINGS { hooks_set(w) } else { w.clone() };
        let v = &mut out[i].1;
        if !v.is_empty() {
            v.push(' ');
        }
        v.push_str(&value);
    }
    let text = |v: &Value| v.as_str().map_or_else(|| v.to_string(), str::to_string);
    out.push(("the CLI's version".to_string(), text(&call["cli"])));
    out.push(("the engine's version".to_string(), text(&call["engine"])));
    out.push(("the environment".to_string(), call["env"].to_string()));
    out.push(("the working directory".to_string(), text(&call["cwd"])));
    out
}

/// A word of a recorded command line: a large text kept in `contents` is named by its hash.
fn word(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Object(o) if o.len() == 1 && o.contains_key("content") => {
            format!("content {}", o["content"].as_str().unwrap_or_default())
        }
        other => other.to_string(),
    }
}

/// The hooks a round's settings set, without the commands they run, which name the round.
fn hooks_set(settings: &str) -> String {
    let Ok(mut v) = serde_json::from_str::<Value>(settings) else {
        return settings.to_string();
    };
    blank_commands(&mut v);
    v.to_string()
}

fn blank_commands(v: &mut Value) {
    match v {
        Value::Object(o) => {
            for (k, x) in o.iter_mut() {
                if k == "command" && x.is_string() {
                    *x = Value::String(String::new());
                } else {
                    blank_commands(x);
                }
            }
        }
        Value::Array(xs) => xs.iter_mut().for_each(blank_commands),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use db::rusqlite::params;
    use db::{Actor, Db};

    use super::*;

    fn call(effort: &str, cli: &str, round: &str, resume: bool) -> Value {
        let settings = json!({ "hooks": { "PostToolUse": [{ "matcher": "*", "hooks": [
            { "type": "command", "command": format!("wf-agents deliver --round {round}") }
        ] }] } })
        .to_string();
        let session = if resume { "--resume" } else { "--session-id" };
        json!({
            "command": ["claude", "-p", "--model", "m", "--json-schema", { "content": "abc" }, "--effort", effort,
                        "--disallowedTools", "Bash(rm:*)", "WebFetch", "--settings", settings, session, "s1"],
            "env": { "CLAUDE_CONFIG_DIR": "/d" },
            "cwd": "/w",
            "session": {},
            "engine": "0.1.0",
            "cli": cli,
        })
    }

    // Only what shapes the prompt is compared: the session flag and the commands of the round's hooks differ every
    // round and are left out; a changed reasoning effort and a new CLI are named with their values.
    #[test]
    fn the_parts_that_differ_are_named_with_their_values() {
        assert!(differs(&call("high", "2.1.3", "1", false), &call("high", "2.1.3", "2", true)).is_empty());
        assert_eq!(
            differs(&call("high", "2.1.3", "1", true), &call("low", "2.1.4", "2", true)),
            [
                Differs {
                    part: "--effort".into(),
                    before: Some("high".into()),
                    after: Some("low".into()),
                },
                Differs {
                    part: "the CLI's version".into(),
                    before: Some("2.1.3".into()),
                    after: Some("2.1.4".into()),
                },
            ]
        );
    }

    fn used(read: i64, written: i64) -> Value {
        json!({ "cost_usd": 1.0, "session_cost_usd": 3.0, "usage": {
            "input_tokens": 10, "cache_read_input_tokens": read, "cache_creation_input_tokens": written,
            "output_tokens": 100,
            "iterations": [{ "input_tokens": 10, "cache_read_input_tokens": read, "cache_creation_input_tokens": written }],
        } })
    }

    fn round(n: i64, usage: Value, ended_at: i64) -> Round {
        Round {
            id: RoundId::new(1, n),
            cell: Some("rethink".into()),
            session: Some(1),
            input: json!({ "call": call("high", "2.1.3", &n.to_string(), n > 1) }),
            output: None,
            transition: None,
            rejected: None,
            compactions: 0,
            started_at: ended_at - 1000,
            ended_at: Some(ended_at),
            usage: Some(usage),
        }
    }

    fn started(db: &mut Db, n: i64, at: i64) {
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (flow_version, graph, at, project) VALUES (1, 'architect', 'rethink', 'p')",
                [],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, ?1, 'rethink', '{}', 'c', ?2)",
                params![n, at],
            )?;
            Ok(())
        })
        .unwrap();
    }

    // A round that resumed its session within the hour and wrote again at least half of what it held missed the cache,
    // though it read a small fixed part; one that read the session, or came after the cache let it go, did not.
    #[test]
    fn a_round_that_writes_again_what_its_session_held_missed_the_cache() {
        let mut db = Db::in_memory().unwrap();
        let before = [round(1, used(0, 400_000), 1_000_000)];
        started(&mut db, 2, 1_000_000 + 10 * 60_000);
        let now = call("high", "2.1.3", "2", true);
        let miss = missed(
            db.conn(),
            "s1",
            &before,
            RoundId::new(1, 2),
            &now,
            &used(6_492, 410_000),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            (miss.before, miss.written, miss.held, miss.after_ms),
            (RoundId::new(1, 1), 410_000, 400_010, 600_000)
        );
        assert!(miss.differs.is_empty());
        assert!(miss.cost_usd.is_some_and(|c| c > 0.0 && c < 1.0), "{:?}", miss.cost_usd);
        let read = missed(
            db.conn(),
            "s1",
            &before,
            RoundId::new(1, 2),
            &now,
            &used(400_000, 2_000),
        )
        .unwrap();
        assert_eq!(read, None);
        let mut late = Db::in_memory().unwrap();
        started(&mut late, 2, 1_000_000 + warm::CACHE_MS);
        let expired = missed(
            late.conn(),
            "s1",
            &before,
            RoundId::new(1, 2),
            &now,
            &used(6_492, 410_000),
        )
        .unwrap();
        assert_eq!(expired, None);
    }
}
