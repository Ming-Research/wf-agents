//! The faults of wf-agents the engine tells the owner of: a round that missed its session's prompt cache, and a flow
//! anomaly, where the way on an engineer chose did not fit the situation and the work asks the owner. wf-agents runs
//! other projects too, and such a fault is its own whichever project it came up in, to be fixed in wf-agents: so it
//! goes to the owner, in a notice that names the fault, and not into what a project's architect reads unless the owner
//! forwards it to that architect, by hand or by the setting that forwards each new one as it comes. A further cache
//! miss with the same cause on the same line of the same project, while its notice is unread or a matter forwarded
//! from it waits in an architect's queue, adds to that notice instead of making a new one; an architect is handed a
//! fault as it stands when its matter is handled.

use db::Actor;
use db::rusqlite::{Connection, OptionalExtension, Transaction, params};
use flow::{CompiledGraph, CompiledRow};
use serde_json::{Value, json};
use sessions::miss::{Differs, Miss};

use crate::work::{RoundId, Work, query_json};
use crate::{Engine, actions};

/// The kind of fault a cache miss is, as `engine_notices.fault` names it.
pub const CACHE_MISS: &str = "cache_miss";

/// The kind of fault a flow anomaly is, as `engine_notices.fault` names it.
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

/// The most of a value of the command line a notice quotes.
const QUOTED: usize = 80;

/// The most cache misses an architect is handed with a fault forwarded to it.
const EVIDENCE: usize = 200;

/// A fault of wf-agents forwarded to an architect, as it stands when its matter is handled: the notice the owner was
/// told, with its kind; the engine's evidence, the cache misses the notice holds by then (each with its round, where
/// it came up, what differs in its command line or its environment from the session's round before, and what it wrote
/// again and cost) or the ask a flow anomaly put, with the owner's answer once given; and who forwarded it. None for a
/// matter that holds no fault.
pub fn forwarded(conn: &Connection, matter: i64) -> Result<Option<Value>, String> {
    type Found = (i64, Option<String>, String, i64, Option<i64>, Option<String>);
    let found: Option<Found> = conn
        .query_row(
            "SELECT n.id, n.fault, n.text, n.at, n.ask, json_extract(m.body, '$.by')
             FROM architect_matters m JOIN engine_notices n ON n.id = m.notice WHERE m.id = ?1",
            [matter],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .optional()
        .map_err(|x| x.to_string())?;
    let Some((notice, kind, text, at, ask, by)) = found else {
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
                 WHERE c.notice = ?1 ORDER BY r.started_at, c.work, c.round",
                &[&notice],
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
        Some("setting") => "the owner's setting, which forwards each new fault of wf-agents to you as it comes",
        _ => "the owner",
    };
    Ok(Some(json!({
        "notice": notice,
        "kind": kind,
        "text": text,
        "at": crate::utc(at),
        "evidence": evidence,
        "forwarded_by": by,
    })))
}

/// Tells the owner that a round missed its session's prompt cache, and keeps the miss: with a new notice, or with the
/// notice still open of a miss with the same cause on the same line of the same project.
pub fn cache_miss(e: &mut Engine, w: &Work, round: RoundId, cell: &str, line: &str, m: &Miss) -> Result<(), String> {
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
    (e.say)(&format!("{round} missed its session's prompt cache: {to}"));
    Ok(())
}

/// A cache miss to keep, with where it came up and what the owner is told of it.
struct Kept<'a> {
    round: RoundId,
    line: &'a str,
    project: &'a str,
    text: &'a str,
    miss: &'a Miss,
}

/// Keeps a cache miss with the notice still open of a miss with the same cause on the same line of the same project,
/// unread or forwarded to an architect whose matter of it waits, or with a new notice that tells the owner; gives
/// whether it was added to a notice.
fn keep(tx: &Transaction<'_>, k: &Kept<'_>) -> db::Result<bool> {
    let differs = k.miss.differs_json().to_string();
    let open: Option<i64> = tx
        .query_row(
            "SELECT n.id FROM engine_notices n JOIN cache_misses c ON c.notice = n.id
             JOIN works x ON x.id = c.work LEFT JOIN items i ON i.id = x.item
             WHERE (n.read_at IS NULL
                    OR EXISTS (SELECT 1 FROM architect_matters m
                               WHERE m.notice = n.id AND m.handled_round IS NULL AND m.withdrawn IS NULL))
               AND n.fault = ?1 AND c.line = ?2
               AND coalesce(i.project, x.project) = ?3 AND c.differs = ?4
             ORDER BY n.id DESC LIMIT 1",
            params![CACHE_MISS, k.line, k.project, differs],
            |r| r.get(0),
        )
        .optional()?;
    let notice = match open {
        Some(n) => n,
        None => actions::notify_fault(tx, k.round.work, CACHE_MISS, k.text).map_err(db::Error::Abort)?,
    };
    let m = k.miss;
    tx.execute(
        "INSERT INTO cache_misses (work, round, line, differs, written, held, cost_usd, notice)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            k.round.work,
            k.round.n,
            k.line,
            differs,
            m.written,
            m.held,
            m.cost_usd,
            notice
        ],
    )?;
    Ok(open.is_some())
}

/// What the owner is told of a cache miss: where it came up, what it read of what the session held, what it wrote again
/// and cost, and what differs in the command line and its environment from the session's round before.
fn told(about: &str, round: RoundId, cell: &str, line: &str, m: &Miss) -> String {
    let cost = match m.cost_usd {
        Some(c) => format!(", which cost about ${c:.2} more than reading them"),
        None => String::new(),
    };
    let differs = if m.differs.is_empty() {
        format!(
            "No part of the command line or its environment differs from the session's round before, {}.",
            m.before
        )
    } else {
        let parts: Vec<String> = m.differs.iter().map(differ).collect();
        format!(
            "The command line or its environment differs from the session's round before, {}, in: {}.",
            m.before,
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
        assert_eq!(
            (thousands(999), thousands(1_000), thousands(6_492)),
            ("999".into(), "1,000".into(), "6,492".into())
        );
    }

    // A further miss with the same cause on the same line of the same project adds to the notice while it is unread;
    // once the owner read it, or with another cause or on another line, a miss makes a notice of its own.
    #[test]
    fn a_further_miss_with_the_same_cause_adds_to_the_unread_notice() {
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
            tx.execute("UPDATE engine_notices SET read_at = 9", [])?;
            Ok(())
        })
        .unwrap();
        assert!(!kept(&mut db, 6, "architect", &m));
        let (notices, misses): (i64, i64) = db
            .conn()
            .query_row(
                "SELECT (SELECT count(*) FROM engine_notices WHERE fault = 'cache_miss'), (SELECT count(*) FROM cache_misses)",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((notices, misses), (4, 5));
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

    /// Keeps a miss of round n of work 1 on the architect line; gives whether it was added to a notice.
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

    // The owner forwards a fault to a project's architect: a matter of the kind report with the detail fault in its
    // queue, naming the notice, which is then read. The same fault to the same architect again is refused and queues
    // nothing; to another architect it goes.
    #[test]
    fn a_fault_forwarded_by_hand_goes_to_each_architect_once() {
        let mut db = with_rounds();
        let m = Miss {
            before: RoundId::new(1, 1),
            after_ms: 60_000,
            read: 10_000,
            written: 400_000,
            held: 410_000,
            cost_usd: Some(1.5),
            differs: Vec::new(),
        };
        assert!(!missed(&mut db, 2, &m));
        let notice: i64 = rows(&db, "SELECT id FROM engine_notices")[0];
        assert_eq!(
            api::forward_fault(&mut db, notice, "wf").unwrap(),
            format!("fault {notice} goes to the architect of wf as matter 1")
        );
        let e = api::forward_fault(&mut db, notice, "wf").unwrap_err();
        assert!(e.contains("went to the architect of wf already"), "{e}");
        api::forward_fault(&mut db, notice, "p").unwrap();
        assert_eq!(
            rows::<String>(
                &db,
                "SELECT project || ' ' || kind || ' ' || detail || ' ' || notice || ' ' || json_extract(body, '$.by')
                 FROM architect_matters ORDER BY id"
            ),
            [
                format!("wf report fault {notice} owner"),
                format!("p report fault {notice} owner")
            ]
        );
        assert_eq!(
            rows::<String>(&db, "SELECT project FROM architect_queue ORDER BY matter"),
            ["wf", "p"]
        );
        assert_eq!(
            rows::<i64>(&db, "SELECT count(*) FROM engine_notices WHERE read_at IS NULL"),
            [0]
        );
        assert_eq!(
            rows::<String>(
                &db,
                "SELECT DISTINCT actor FROM changes WHERE table_name = 'architect_matters'"
            ),
            ["owner"]
        );
        // A notice of no fault is not forwarded.
        db.write(Actor::Engine, |tx| {
            actions::notify(tx, None, "Every work is halted.").map_err(db::Error::Abort)
        })
        .unwrap();
        let other: i64 = rows(&db, "SELECT max(id) FROM engine_notices")[0];
        assert!(api::forward_fault(&mut db, other, "wf").is_err());
    }

    // With the owner's setting naming a project, a new fault goes to its architect at once, read, from the setting; a
    // further miss with the same cause adds to it while its matter waits, and the architect is handed every miss it
    // holds by then. Set back to nobody, a new fault stays with the owner, unread, and goes to no architect.
    #[test]
    fn a_new_fault_goes_to_the_architect_the_owner_s_setting_names_and_to_none_set_to_nobody() {
        let mut db = with_rounds();
        assert_eq!(api::fault_forwarding(db.conn()).unwrap(), None);
        api::set_fault_forwarding(&mut db, Some("wf")).unwrap();
        assert_eq!(api::fault_forwarding(db.conn()).unwrap().as_deref(), Some("wf"));
        let mut m = Miss {
            before: RoundId::new(1, 1),
            after_ms: 60_000,
            read: 10_000,
            written: 400_000,
            held: 410_000,
            cost_usd: Some(1.5),
            differs: Vec::new(),
        };
        assert!(!missed(&mut db, 2, &m));
        assert_eq!(
            rows::<String>(
                &db,
                "SELECT m.project || ' ' || m.detail || ' ' || json_extract(m.body, '$.by') || ' ' || (n.read_at IS NOT NULL)
                 FROM architect_matters m JOIN engine_notices n ON n.id = m.notice"
            ),
            ["wf fault setting 1"]
        );
        assert!(missed(&mut db, 3, &m));
        let f = forwarded(db.conn(), 1).unwrap().unwrap();
        assert_eq!(f["kind"], CACHE_MISS);
        assert_eq!(f["evidence"]["cache_misses"].as_array().map(Vec::len), Some(2));
        assert_eq!(f["evidence"]["cache_misses"][1]["round"], "R1-3");
        assert!(f["forwarded_by"].as_str().unwrap().starts_with("the owner's setting"));
        api::set_fault_forwarding(&mut db, None).unwrap();
        assert_eq!(api::fault_forwarding(db.conn()).unwrap(), None);
        m.differs = vec![Differs {
            part: "--effort".into(),
            before: Some("high".into()),
            after: Some("medium".into()),
        }];
        assert!(!missed(&mut db, 4, &m));
        assert_eq!(rows::<i64>(&db, "SELECT count(*) FROM architect_matters"), [1]);
        assert_eq!(
            rows::<i64>(
                &db,
                "SELECT count(*) FROM engine_notices WHERE read_at IS NULL AND fault IS NOT NULL"
            ),
            [1]
        );
        assert_eq!(
            rows::<String>(
                &db,
                "SELECT actor FROM changes WHERE table_name = 'owner_choices' ORDER BY id"
            ),
            ["owner", "owner"]
        );
    }
}
