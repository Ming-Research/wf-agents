//! The faults of wf-agents the engine tells the flow's keeper of: a round that missed its session's prompt cache, and a
//! flow anomaly, where the way on an engineer chose did not fit the situation and the work asked the owner, in a flow
//! version from before the engineer's graph lost its hand-over to the owner; a flow anomaly now goes to the project's
//! architect as a gap of the flow, and the keeper is told of it no longer. wf-agents runs other projects too, and such
//! a fault is its own whichever project it came up in, to be fixed in wf-agents: so it goes to the keeper, in a message
//! that names the fault, and into what an architect reads only once it is passed on to the architect of the project
//! wf-agents is built from. A further cache miss with the same cause on the same line of the same project, while its
//! message is unread or a matter it was passed on as waits in an architect's queue, adds to that message instead of
//! making a new one; an architect is handed a fault as it stands when its matter is handled. A miss after what comes
//! before the session's messages changed since its round before (the model, the tools the envelope is given with, the
//! command line's version) is the command line writing the session again by design, no fault: it is kept for what it
//! cost, and the keeper is not told.

use db::Actor;
use db::rusqlite::{Connection, OptionalExtension, Transaction, params};
use flow::{CompiledGraph, CompiledRow};
use serde_json::{Value, json};
use sessions::miss::{Differs, Miss};

use crate::Engine;
use crate::work::{RoundId, Work, query_json};

/// The kind of fault a cache miss is, as its message names it (`messages.body`).
pub const CACHE_MISS: &str = "cache_miss";

/// The kind of fault a flow anomaly is, as its message names it (`messages.body`).
pub const FLOW_ANOMALY: &str = "flow_anomaly";

/// The counter of the flow's rows that a flow anomaly takes.
const FLOW_ANOMALIES: &str = "flow_anomalies";

/// Whether a row counts a flow anomaly: the way on chosen at its cell did not fit the situation.
pub fn counts_anomaly(row: &CompiledRow) -> bool {
    row.counts.as_deref() == Some(FLOW_ANOMALIES)
}

/// Whether a work came to the cell it is at by a row that counts a flow anomaly: the latest row its rounds took.
pub fn came_of_anomaly(conn: &Connection, work: i64, g: &CompiledGraph) -> Result<bool, String> {
    let took: Option<(String, String)> = conn
        .query_row(
            "SELECT cell, transition FROM rounds WHERE work = ?1 AND cell IS NOT NULL AND transition IS NOT NULL
             ORDER BY n DESC LIMIT 1",
            [work],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|x| x.to_string())?;
    Ok(took.is_some_and(|(cell, row)| {
        g.cell(&cell)
            .and_then(|c| c.rows.iter().find(|r| r.name == row))
            .is_some_and(counts_anomaly)
    }))
}

/// The most of a value of the command line a message of a fault quotes.
const QUOTED: usize = 80;

/// The most cache misses an architect is handed with a fault passed on to it.
const EVIDENCE: usize = 200;

/// A fault of wf-agents passed on to an architect, as it stands when its matter is handled: the message the flow's
/// keeper was told, with its kind; the engine's evidence, the cache misses the message holds by then (each with its
/// round, where it came up, what differs in its command line or its environment from the session's round before, and
/// what it wrote again and cost) or the ask a flow anomaly put, with the owner's answer once given; and who passed it
/// on. None for a matter that holds no fault.
pub fn forwarded(conn: &Connection, matter: i64) -> Result<Option<Value>, String> {
    type Found = (i64, Option<String>, String, i64, Option<i64>, Option<String>);
    let found: Option<Found> = conn
        .query_row(
            "SELECT n.id, json_extract(n.body, '$.kind'), n.text, n.at, json_extract(n.body, '$.ask'),
                    json_extract(m.body, '$.by')
             FROM deliveries d JOIN messages n ON n.id = d.message JOIN architect_matters m ON m.id = d.matter
             WHERE d.matter = ?1",
            [matter],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .optional()
        .map_err(|x| x.to_string())?;
    let Some((message, kind, text, at, ask, by)) = found else {
        return Ok(None);
    };
    let evidence = match kind.as_deref() {
        Some(CACHE_MISS) => {
            let mut misses = query_json(
                conn,
                "SELECT 'R' || c.work || '-' || c.round AS round, coalesce(i.project, x.project) AS project,
                        r.cell, c.line, c.differs, c.written, c.held, c.cost_usd, r.started_at AS at
                 FROM cache_misses c JOIN rounds r ON r.work = c.work AND r.n = c.round
                 JOIN works x ON x.id = c.work LEFT JOIN items i ON i.id = x.item
                 WHERE c.message = ?1 ORDER BY r.started_at, c.work, c.round",
                &[&message],
                EVIDENCE,
            )?;
            for m in &mut misses {
                m["at"] = json!(m["at"].as_i64().map(crate::utc));
            }
            json!({ "cache_misses": misses })
        }
        Some(FLOW_ANOMALY) => {
            let asked = query_json(
                conn,
                "SELECT id AS ask, 'R' || work || '-' || round AS round, body, answer FROM owner_asks WHERE id = ?1",
                &[&ask],
                1,
            )?;
            json!({ "ask": asked.into_iter().next() })
        }
        _ => Value::Null,
    };
    let by = match by.as_deref() {
        Some("setting") => "the owner's setting that forwarded each new fault of wf-agents to you as it came",
        _ => "the owner",
    };
    Ok(Some(json!({
        "notice": message,
        "kind": kind,
        "text": text,
        "at": crate::utc(at),
        "evidence": evidence,
        "forwarded_by": by,
    })))
}

/// Tells the flow's keeper that a round missed its session's prompt cache, and keeps the miss: with a new message, or
/// with the message still open of a miss with the same cause on the same line of the same project. A miss the engine
/// expects, after what comes before the session's messages changed, is kept for what it cost with no message.
pub fn cache_miss(e: &mut Engine, w: &Work, round: RoundId, cell: &str, line: &str, m: &Miss) -> Result<(), String> {
    if m.expected() {
        e.db.write(Actor::Engine, |tx| record(tx, round, line, m, None))
            .map_err(|x| x.to_string())?;
        let parts: Vec<&str> = m.before_messages().iter().map(|d| d.part.as_str()).collect();
        e.say(&format!(
            "{round} wrote its session's prompt cache again, as expected after {} changed since {}: kept for its cost, \
             no fault",
            parts.join(", "),
            compared(m)
        ));
        return Ok(());
    }
    let item = e.subject(w)?;
    let (project, about) = match &item {
        Some(i) => (
            i.project.clone(),
            format!("work {} (item {}: {}) in project {}", w.id, i.id, i.title, i.project),
        ),
        None => (
            w.project.clone().unwrap_or_default(),
            format!("work {} ({})", w.id, w.of_architect()),
        ),
    };
    let text = told(&about, round, cell, line, m);
    let kept = Kept {
        round,
        line,
        project: &project,
        text: &text,
        miss: m,
    };
    let added =
        e.db.write(Actor::Engine, |tx| keep(tx, &kept))
            .map_err(|x| x.to_string())?;
    let to = if added {
        "added to the report still open"
    } else {
        "reported"
    };
    e.say(&format!("{round} missed its session's prompt cache: {to}"));
    Ok(())
}

/// A cache miss to keep, with where it came up and what the flow's keeper is told of it.
struct Kept<'a> {
    round: RoundId,
    line: &'a str,
    project: &'a str,
    text: &'a str,
    miss: &'a Miss,
}

/// Keeps a cache miss with the message still open of a miss with the same cause on the same line of the same project,
/// unread by the flow's keeper or passed on to an architect whose matter of it waits, or with a new message that tells
/// the keeper; gives whether it was added to a message.
fn keep(tx: &Transaction<'_>, k: &Kept<'_>) -> db::Result<bool> {
    let differs = k.miss.differs_json().to_string();
    let open: Option<i64> = tx
        .query_row(
            "SELECT n.id FROM messages n JOIN deliveries d ON d.message = n.id AND d.recipient = 'keeper'
             JOIN cache_misses c ON c.message = n.id
             JOIN works x ON x.id = c.work LEFT JOIN items i ON i.id = x.item
             WHERE (d.read_at IS NULL
                    OR EXISTS (SELECT 1 FROM deliveries a JOIN architect_matters m ON m.id = a.matter
                               WHERE a.message = n.id AND m.handled_round IS NULL AND m.withdrawn IS NULL))
               AND json_extract(n.body, '$.kind') = ?1 AND c.line = ?2
               AND coalesce(i.project, x.project) = ?3 AND c.differs = ?4
             ORDER BY n.id DESC LIMIT 1",
            params![CACHE_MISS, k.line, k.project, differs],
            |r| r.get(0),
        )
        .optional()?;
    let message = match open {
        Some(n) => n,
        None => crate::hub::fault(tx, k.round.work, CACHE_MISS, k.text).map_err(db::Error::Abort)?,
    };
    record(tx, k.round, k.line, k.miss, Some(message))?;
    Ok(open.is_some())
}

/// Records a round's cache miss with the message that tells the flow's keeper of it, or with none for a miss the
/// engine expects.
fn record(tx: &Transaction<'_>, round: RoundId, line: &str, m: &Miss, message: Option<i64>) -> db::Result<()> {
    tx.execute(
        "INSERT INTO cache_misses (work, round, line, differs, written, held, cost_usd, message)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            round.work,
            round.n,
            line,
            m.differs_json().to_string(),
            m.written,
            m.held,
            m.cost_usd,
            message
        ],
    )?;
    Ok(())
}

/// What a miss's round was compared with: the session's round before, or the keep-warm ping since it that recorded its
/// call, under which the cache kept the session last.
fn compared(m: &Miss) -> String {
    match m.ping {
        Some(p) => format!("keep-warm ping {p}, after the session's round before, {}", m.before),
        None => format!("the session's round before, {}", m.before),
    }
}

/// What the owner is told of a cache miss: where it came up, what it read of what the session held, what it wrote again
/// and cost, and what differs in the command line and its environment from the session's round before, or from the
/// keep-warm ping since it that recorded its call.
fn told(about: &str, round: RoundId, cell: &str, line: &str, m: &Miss) -> String {
    let cost = match m.cost_usd {
        Some(c) => format!(", which cost about ${c:.2} more than reading them"),
        None => String::new(),
    };
    let differs = if m.differs.is_empty() {
        format!(
            "No part of the command line or its environment differs from {}.",
            compared(m)
        )
    } else {
        let parts: Vec<String> = m.differs.iter().map(differ).collect();
        format!(
            "The command line or its environment differs from {}, in: {}.",
            compared(m),
            parts.join("; ")
        )
    };
    format!(
        "A fault of wf-agents, to be fixed in wf-agents: {round} of {about}, at cell {cell} on the {line} line, missed \
         its session's prompt cache. It resumed the session {} minutes after the session's last call, within the \
         cache's hour, and its first call read {} tokens from the cache of the {} the session held: it wrote the other \
         {} to the cache again{cost}. {differs}",
        m.after_ms / 60_000,
        thousands(m.read),
        thousands(m.held),
        thousands(m.written),
    )
}

/// A part of the command line that differs, with its value before and now, each cut short past [`QUOTED`] characters.
fn differ(d: &Differs) -> String {
    let quote = |v: &Option<String>| match v {
        Some(s) if s.chars().count() > QUOTED => format!("{}…", s.chars().take(QUOTED).collect::<String>()),
        Some(s) => s.clone(),
        None => "none".to_string(),
    };
    format!("{} ({} → {})", d.part, quote(&d.before), quote(&d.after))
}

/// A number with its thousands set apart by commas.
fn thousands(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A work came of a flow anomaly when the latest row its rounds took counts one; by another row, it did not.
    #[test]
    fn a_work_came_of_a_flow_anomaly_by_the_row_it_took() {
        let relations = db::relations(db::SCHEMA).unwrap();
        let tables = db::index_of(db::SCHEMA).unwrap();
        let f = flow::compile(flow::FLOW, &flow::Context { relations, tables }).unwrap();
        let g = f.graph("engineer").unwrap();
        let (cell, anomaly, other) = g
            .cells
            .iter()
            .find_map(|c| {
                let a = c.rows.iter().find(|r| r.counts.as_deref() == Some(FLOW_ANOMALIES))?;
                let o = c.rows.iter().find(|r| r.counts.as_deref() != Some(FLOW_ANOMALIES))?;
                Some((c.id.clone(), a.name.clone(), o.name.clone()))
            })
            .unwrap();
        let mut db = db::Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            for _ in 0..2 {
                tx.execute(
                    "INSERT INTO works (flow_version, graph, at) VALUES (1, 'engineer', 'A3')",
                    [],
                )?;
            }
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, transition, config_commit, started_at)
                 VALUES (1, 1, ?1, '{}', ?2, 'c', 1), (2, 1, ?1, '{}', ?3, 'c', 1)",
                params![cell, anomaly, other],
            )?;
            Ok(())
        })
        .unwrap();
        assert!(came_of_anomaly(db.conn(), 1, g).unwrap());
        assert!(!came_of_anomaly(db.conn(), 2, g).unwrap());
    }

    // The owner is told where the miss came up, what it wrote again and cost, and what differs in the command line, or
    // that nothing does.
    #[test]
    fn a_cache_miss_is_told_with_its_cause() {
        let mut m = Miss {
            before: RoundId::new(7, 4),
            ping: None,
            after_ms: 14 * 60_000 + 5_000,
            read: 5_655,
            written: 412_345,
            held: 418_000,
            cost_usd: Some(1.234),
            differs: Vec::new(),
        };
        let text = told("work 7 (the architect of p)", RoundId::new(7, 5), "B4", "architect", &m);
        assert!(
            text.contains("R7-5 of work 7 (the architect of p), at cell B4 on the architect line")
                && text.contains("14 minutes after")
                && text.contains(
                    "its first call read 5,655 tokens from the cache of the 418,000 the session held: it wrote the \
                     other 412,345 to the cache again, which cost about $1.23"
                )
                && text.ends_with(
                    "No part of the command line or its environment differs from the session's round before, R7-4."
                ),
            "{text}"
        );
        m.differs = vec![
            Differs {
                part: "the CLI's version".into(),
                before: Some("2.1.3".into()),
                after: Some("2.1.4".into()),
            },
            Differs {
                part: "the environment's CLAUDE_CODE_X".into(),
                before: None,
                after: Some("1".into()),
            },
        ];
        let text = told("work 7", RoundId::new(7, 5), "B4", "architect", &m);
        assert!(
            text.ends_with(
                "differs from the session's round before, R7-4, in: the CLI's version (2.1.3 → 2.1.4); the \
                 environment's CLAUDE_CODE_X (none → 1)."
            ),
            "{text}"
        );
        // A fork of the architect's base after the ping that warmed the base is compared with the ping's call.
        m.ping = Some(12);
        let text = told("work 7", RoundId::new(7, 5), "B4", "matter", &m);
        assert!(
            text.contains("differs from keep-warm ping 12, after the session's round before, R7-4, in: the CLI's"),
            "{text}"
        );
        assert_eq!(
            (thousands(999), thousands(1_000), thousands(6_492)),
            ("999".into(), "1,000".into(), "6,492".into())
        );
    }

    // A further miss with the same cause on the same line of the same project adds to the message while the flow's
    // keeper has not read it; once read, or with another cause or on another line, a miss makes a message of its own.
    #[test]
    fn a_further_miss_with_the_same_cause_adds_to_the_unread_message() {
        let mut db = db::Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (flow_version, graph, at, project) VALUES (1, 'architect', 'B4', 'p')",
                [],
            )?;
            for n in 1..=6 {
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, ?1, 'B4', '{}', 'c', ?1)",
                    [n],
                )?;
            }
            Ok(())
        })
        .unwrap();
        let mut m = Miss {
            before: RoundId::new(1, 1),
            ping: None,
            after_ms: 60_000,
            read: 10_000,
            written: 400_000,
            held: 410_000,
            cost_usd: Some(1.5),
            differs: Vec::new(),
        };
        fn kept(db: &mut db::Db, n: i64, line: &str, m: &Miss) -> bool {
            let k = Kept {
                round: RoundId::new(1, n),
                line,
                project: "p",
                text: "missed",
                miss: m,
            };
            db.write(Actor::Engine, |tx| keep(tx, &k)).unwrap()
        }
        assert!(!kept(&mut db, 2, "architect", &m));
        assert!(kept(&mut db, 3, "architect", &m));
        assert!(!kept(&mut db, 4, "observer", &m));
        m.differs = vec![Differs {
            part: "--effort".into(),
            before: Some("high".into()),
            after: Some("medium".into()),
        }];
        assert!(!kept(&mut db, 5, "architect", &m));
        db.write(Actor::Owner, |tx| {
            tx.execute("UPDATE deliveries SET read_at = 9", [])?;
            Ok(())
        })
        .unwrap();
        assert!(!kept(&mut db, 6, "architect", &m));
        let (messages, misses): (i64, i64) = db
            .conn()
            .query_row(
                "SELECT (SELECT count(*) FROM messages WHERE json_extract(body, '$.kind') = 'cache_miss'),
                        (SELECT count(*) FROM cache_misses)",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((messages, misses), (4, 5));
    }

    /// A database with one work of project p's architect and six rounds of it.
    fn with_rounds() -> db::Db {
        let mut db = db::Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (flow_version, graph, at, project) VALUES (1, 'architect', 'B4', 'p')",
                [],
            )?;
            for n in 1..=6 {
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, ?1, 'B4', '{}', 'c', ?1)",
                    [n],
                )?;
            }
            Ok(())
        })
        .unwrap();
        db
    }

    /// Keeps a miss of round n of work 1 on the architect line; gives whether it was added to a message.
    fn missed(db: &mut db::Db, n: i64, m: &Miss) -> bool {
        let k = Kept {
            round: RoundId::new(1, n),
            line: "architect",
            project: "p",
            text: "A fault of wf-agents: a round missed its session's prompt cache.",
            miss: m,
        };
        db.write(Actor::Engine, |tx| keep(tx, &k)).unwrap()
    }

    fn rows<T: db::rusqlite::types::FromSql>(db: &db::Db, q: &str) -> Vec<T> {
        let mut st = db.conn().prepare(q).unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    }

    // A miss the engine expects, after the envelope's schema changed, is kept for its cost with no message; a miss that
    // is a fault after it makes a message of its own, to the flow's keeper.
    #[test]
    fn a_miss_the_engine_expects_is_kept_with_no_message() {
        let mut db = with_rounds();
        let mut m = Miss {
            before: RoundId::new(1, 1),
            ping: None,
            after_ms: 60_000,
            read: 10_000,
            written: 400_000,
            held: 410_000,
            cost_usd: Some(1.5),
            differs: vec![Differs {
                part: "--json-schema".into(),
                before: Some("content a".into()),
                after: Some("content b".into()),
            }],
        };
        assert!(m.expected());
        db.write(Actor::Engine, |tx| {
            record(tx, RoundId::new(1, 2), "architect", &m, None)
        })
        .unwrap();
        m.differs.clear();
        assert!(!m.expected());
        assert!(!missed(&mut db, 3, &m));
        assert_eq!(
            rows::<String>(
                &db,
                "SELECT d.recipient || ' ' || json_extract(m.body, '$.kind') FROM messages m
                 JOIN deliveries d ON d.message = m.id"
            ),
            ["keeper cache_miss"]
        );
        assert_eq!(
            rows::<Option<i64>>(&db, "SELECT message FROM cache_misses ORDER BY round"),
            [None, Some(1)]
        );
    }

    // The owner passes a fault on to the architect of the project wf-agents is built from: a matter of the kind report
    // with the detail fault in its queue, which the message's delivery to it names, and the keeper's delivery is read.
    // The same fault to it again, a fault to another project's architect, or a message that is no fault is refused and
    // queues nothing. A further miss with the same cause adds to the message while its matter waits, and the architect
    // is handed every miss it holds by then.
    #[test]
    fn a_fault_is_passed_on_only_to_the_architect_of_wf_agents_and_once() {
        let mut db = with_rounds();
        let m = Miss {
            before: RoundId::new(1, 1),
            ping: None,
            after_ms: 60_000,
            read: 10_000,
            written: 400_000,
            held: 410_000,
            cost_usd: Some(1.5),
            differs: Vec::new(),
        };
        assert!(!missed(&mut db, 2, &m));
        let message: i64 = rows(&db, "SELECT id FROM messages")[0];
        let e = api::pass_on(&mut db, message, "p", Some("wf")).unwrap_err();
        assert!(e.contains("only to the architect of wf"), "{e}");
        let e = api::pass_on(&mut db, message, "wf", None).unwrap_err();
        assert!(e.contains("no project"), "{e}");
        assert_eq!(
            api::pass_on(&mut db, message, "wf", Some("wf")).unwrap(),
            format!("message {message} goes to the architect of wf as matter 1")
        );
        let e = api::pass_on(&mut db, message, "wf", Some("wf")).unwrap_err();
        assert!(e.contains("went to the architect of wf already"), "{e}");
        assert_eq!(
            rows::<String>(
                &db,
                "SELECT m.project || ' ' || m.kind || ' ' || m.detail || ' ' || d.message || ' ' || json_extract(m.body, '$.by')
                 FROM architect_matters m JOIN deliveries d ON d.matter = m.id ORDER BY m.id"
            ),
            [format!("wf report fault {message} owner")]
        );
        assert_eq!(rows::<String>(&db, "SELECT project FROM architect_queue"), ["wf"]);
        assert_eq!(
            rows::<i64>(
                &db,
                "SELECT count(*) FROM deliveries WHERE recipient = 'keeper' AND read_at IS NULL"
            ),
            [0]
        );
        assert_eq!(
            rows::<String>(
                &db,
                "SELECT DISTINCT actor FROM changes WHERE table_name = 'architect_matters'"
            ),
            ["owner"]
        );
        assert!(missed(&mut db, 3, &m));
        let f = forwarded(db.conn(), 1).unwrap().unwrap();
        assert_eq!(f["kind"], CACHE_MISS);
        assert_eq!(f["evidence"]["cache_misses"].as_array().map(Vec::len), Some(2));
        assert_eq!(f["evidence"]["cache_misses"][1]["round"], "R1-3");
        assert_eq!(f["forwarded_by"], "the owner");
        // A message that is no fault is not passed on.
        db.write(Actor::Engine, |tx| {
            crate::hub::tell(tx, None, "Every work is halted.").map_err(db::Error::Abort)
        })
        .unwrap();
        let other: i64 = rows(&db, "SELECT max(id) FROM messages")[0];
        assert!(api::pass_on(&mut db, other, "wf", Some("wf")).is_err());
        assert_eq!(rows::<i64>(&db, "SELECT count(*) FROM architect_matters"), [1]);
    }
}
