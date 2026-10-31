//! A round that missed its session's prompt cache. A round that resumes its session within the cache's lifetime since
//! the session's last call reads from the cache, at its first call, what the session holds; one whose first call reads
//! less than half of it paid for writing the rest again, what it should have read. What the round wrote to the cache
//! is no measure of a miss: it is also what the session gained since, the last answer of the round before and the
//! round's own message, which after a round of tools is large. A miss is a fault of wf-agents, whichever project it
//! came up in, so the engine tells the owner of it, with the parts of the command line and of its environment that
//! differ from the session's round before, where a cause most often shows, or that none does; but a miss after what
//! comes before the session's messages in the prompt changed since that round (the model, the tools the envelope is
//! given with, the command line's own version) is what the command line does by design, and is no fault.

use std::collections::BTreeSet;

use db::rusqlite::{Connection, OptionalExtension};
use serde_json::{Value, json};

use crate::cli::FIRST_CALL;
use crate::round::{Round, RoundId};
use crate::{choose, cost, warm};

/// The flags that name the session, which differ between rounds by design.
const SESSION_FLAGS: [&str; 2] = ["--resume", "--session-id"];

/// The flag whose value is the round's hooks.
const SETTINGS: &str = "--settings";

/// The parts of a call that make what comes before the session's messages in its prompt: the model, whose cache is its
/// own; the tools, among which the command line gives the envelope's schema (`--json-schema`) and which `--tools`
/// names; the system prompt the engine appends to the command line's own (`--append-system-prompt`); and the command
/// line's own version, whose system prompt and tools come first. When one of them differs from the session's round
/// before, the command line writes the whole session to the cache again by design.
const BEFORE_MESSAGES: [&str; 5] = [
    "--model",
    "--json-schema",
    "--tools",
    crate::system::FLAG,
    "the CLI's version",
];

/// The flag whose bare tool names take those tools from the model, beside its rules on how a tool may be used, which
/// leave the tools as they are.
const TAKEN_TOOLS: &str = "--disallowedTools";

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
    /// The tokens the round's first call read from the cache.
    pub read: i64,
    /// The tokens of what the session held that the round's first call did not read from the cache, and so wrote to it
    /// again.
    pub written: i64,
    /// The tokens the session held: the last prompt of its round before, as recorded.
    pub held: i64,
    /// What writing them again cost beyond reading them, in US dollars; none when the round's record does not tell.
    pub cost_usd: Option<f64>,
    /// The parts of the command line and of its environment that differ from the round before; empty when none does.
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

    /// The parts that differ from the session's round before and come before its messages in the prompt, which the
    /// command line writes to the cache again whole when they change: a miss with any is one the engine expects, and
    /// no fault; none when only other parts differ, or none does.
    pub fn before_messages(&self) -> Vec<&Differs> {
        self.differs
            .iter()
            .filter(|d| {
                BEFORE_MESSAGES.contains(&d.part.as_str())
                    || (d.part == TAKEN_TOOLS && tools_taken(&d.before) != tools_taken(&d.after))
            })
            .collect()
    }

    /// Whether the miss is one the engine expects: what comes before the session's messages changed since its round
    /// before.
    pub fn expected(&self) -> bool {
        !self.before_messages().is_empty()
    }
}

/// The tools a value of `--disallowedTools` takes from the model: its words that name a tool alone, with no rule on how
/// it may be used.
fn tools_taken(value: &Option<String>) -> BTreeSet<&str> {
    value
        .as_deref()
        .unwrap_or_default()
        .split(' ')
        .filter(|w| !w.is_empty() && !w.contains(['(', ')']))
        .collect()
}

/// Whether a round that resumed a CLI session missed its prompt cache: it started within the cache's lifetime since the
/// session's last call, and its first call read from the cache less than half of what the session held. `before` are
/// the session's rounds before it, oldest first, `call` is its call as its record keeps it, `usage` its usage record
/// and `model` the model it ran on, whose prices the miss's cost is at. None when the record cannot tell: no round
/// before recorded its calls or its command line, or the round's record does not say what its first call read.
pub fn missed(
    conn: &Connection,
    cli_session: &str,
    before: &[Round],
    round: RoundId,
    call: &Value,
    usage: &Value,
    model: Option<&str>,
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
    let Some(read) = usage[FIRST_CALL]["cache_read_input_tokens"].as_i64() else {
        return Ok(None);
    };
    if read * 2 >= held {
        return Ok(None);
    }
    let written = held - read;
    Ok(Some(Miss {
        before: prev.id,
        after_ms,
        read,
        written,
        held,
        cost_usd: model.and_then(|m| cost::rewritten(m, usage, written)),
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

/// The parts of a call's command line and of its environment that differ from those of the call before, in the order
/// the later call has them, then those only the call before had.
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
/// installation's arguments to it, each flag with its values, the CLI's and the engine's versions, each variable of the
/// environment as the record keeps it, and the working directory. The flag that names the session is left out, and so
/// are the commands of the round's hooks, which name the round; which hooks it sets is kept.
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
    if let Some(env) = call["env"].as_object() {
        out.extend(env.iter().map(|(k, v)| (format!("the environment's {k}"), text(v))));
    }
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
    // round and are left out; a changed reasoning effort, a new CLI and each variable of the environment that changed or
    // came are named with their values.
    #[test]
    fn the_parts_that_differ_are_named_with_their_values() {
        assert!(differs(&call("high", "2.1.3", "1", false), &call("high", "2.1.3", "2", true)).is_empty());
        let mut now = call("low", "2.1.4", "2", true);
        now["env"]["CLAUDE_CONFIG_DIR"] = json!("/e");
        now["env"]["CLAUDE_CODE_X"] = json!("1");
        let named = |part: &str, before: Option<&str>, after: Option<&str>| Differs {
            part: part.into(),
            before: before.map(str::to_string),
            after: after.map(str::to_string),
        };
        assert_eq!(
            differs(&call("high", "2.1.3", "1", true), &now),
            [
                named("--effort", Some("high"), Some("low")),
                named("the CLI's version", Some("2.1.3"), Some("2.1.4")),
                named("the environment's CLAUDE_CONFIG_DIR", Some("/d"), Some("/e")),
                named("the environment's CLAUDE_CODE_X", None, Some("1")),
            ]
        );
    }

    // A miss after what comes before the session's messages changed is one the engine expects: the envelope's schema,
    // the model, the command line's version or a tool taken from the model. A changed effort, a variable of the
    // environment or a rule on how a tool may be used leaves that as it was, and a miss with only those is a fault.
    #[test]
    fn a_miss_after_what_comes_before_the_messages_changed_is_expected() {
        let was = call("high", "2.1.3", "1", true);
        let before_messages = |now: &Value| -> Vec<String> {
            let miss = Miss {
                before: RoundId::new(1, 1),
                after_ms: 0,
                read: 0,
                written: 1,
                held: 1,
                cost_usd: None,
                differs: differs(&was, now),
            };
            assert_eq!(miss.expected(), !miss.before_messages().is_empty());
            miss.before_messages().iter().map(|d| d.part.clone()).collect()
        };
        let with = |at: usize, word: Value| {
            let mut now = call("high", "2.1.3", "2", true);
            now["command"][at] = word;
            now
        };
        assert_eq!(
            before_messages(&with(5, json!({ "content": "def" }))),
            ["--json-schema"]
        );
        assert_eq!(before_messages(&with(3, json!("m2"))), ["--model"]);
        assert_eq!(before_messages(&with(10, json!("WebSearch"))), ["--disallowedTools"]);
        assert_eq!(
            before_messages(&call("high", "2.1.4", "2", true)),
            ["the CLI's version"]
        );
        let mut env = call("low", "2.1.3", "2", true);
        env["env"]["CLAUDE_CONFIG_DIR"] = json!("/e");
        for now in [
            with(9, json!("Bash(git push:*)")),
            env,
            call("high", "2.1.3", "2", true),
        ] {
            assert!(before_messages(&now).is_empty(), "{now}");
        }
    }

    const SONNET: Option<&str> = Some("claude-sonnet-5-5");

    /// What the CLI reports a round used: what its first call read from the cache and wrote to it, and the run's whole,
    /// whose last call read what the first wrote and wrote a little more.
    fn used(read: i64, written: i64) -> Value {
        json!({ "cost_usd": 1.0, "session_cost_usd": 3.0,
            "first_call": { "input_tokens": 10, "cache_read_input_tokens": read, "cache_creation_input_tokens": written },
            "usage": {
                "input_tokens": 20, "cache_read_input_tokens": 2 * read + written,
                "cache_creation_input_tokens": written + 500, "output_tokens": 100,
                "iterations": [{
                    "input_tokens": 10, "cache_read_input_tokens": read + written, "cache_creation_input_tokens": 500,
                }],
            } })
    }

    fn round(n: i64, usage: Value, ended_at: i64) -> Round {
        Round {
            id: RoundId::new(1, n),
            cell: Some("B4".into()),
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
                "INSERT INTO works (flow_version, graph, at, project) VALUES (1, 'architect', 'B4', 'p')",
                [],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, ?1, 'B4', '{}', 'c', ?2)",
                params![n, at],
            )?;
            Ok(())
        })
        .unwrap();
    }

    // A round that resumed its session within the hour and whose first call read less than half of what the session
    // held missed the cache, though it read a small fixed part; one that read the session, or came after the cache let
    // it go, did not. A round that read the session and wrote much more after it, as one does after a round of tools
    // whose results its message gives, did not miss: what it wrote is new.
    #[test]
    fn a_round_whose_first_call_reads_less_than_half_its_session_missed_the_cache() {
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
            SONNET,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            (miss.before, miss.read, miss.written, miss.held, miss.after_ms),
            (RoundId::new(1, 1), 6_492, 394_018, 400_510, 600_000)
        );
        assert!(miss.differs.is_empty());
        // Written for five minutes on Sonnet 5.5, at $2.50 a million where reading them would have cost $0.10.
        assert!(
            miss.cost_usd.is_some_and(|c| (c - 394_018.0 * 2.4 / 1e6).abs() < 1e-9),
            "{:?}",
            miss.cost_usd
        );
        for (read, written) in [(400_500, 2_000), (400_500, 300_000)] {
            let hit = missed(
                db.conn(),
                "s1",
                &before,
                RoundId::new(1, 2),
                &now,
                &used(read, written),
                SONNET,
            )
            .unwrap();
            assert_eq!(hit, None, "{read} read, {written} written");
        }
        // A record that does not say what the first call read cannot tell.
        let mut untold = used(6_492, 410_000);
        untold.as_object_mut().unwrap().remove(FIRST_CALL);
        assert_eq!(
            missed(db.conn(), "s1", &before, RoundId::new(1, 2), &now, &untold, SONNET).unwrap(),
            None
        );
        let mut late = Db::in_memory().unwrap();
        started(&mut late, 2, 1_000_000 + warm::CACHE_MS);
        let expired = missed(
            late.conn(),
            "s1",
            &before,
            RoundId::new(1, 2),
            &now,
            &used(6_492, 410_000),
            SONNET,
        )
        .unwrap();
        assert_eq!(expired, None);
    }
}
