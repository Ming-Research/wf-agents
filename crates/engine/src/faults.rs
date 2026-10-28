//! The faults of wf-agents the engine tells the owner of: a round that missed its session's prompt cache. wf-agents
//! runs other projects too, and such a fault is its own whichever project it came up in, to be fixed in wf-agents: so
//! it goes to the owner, in a notice that names the fault, and not into what a project's architect reads. A further
//! miss with the same cause on the same line of the same project, while its notice is unread, adds to that notice
//! instead of making a new one.

use db::Actor;
use db::rusqlite::{OptionalExtension, Transaction, params};
use sessions::miss::{Differs, Miss};

use crate::work::{RoundId, Work};
use crate::{Engine, actions};

/// The kind of fault a cache miss is, as `engine_notices.fault` names it.
pub const CACHE_MISS: &str = "cache_miss";

/// The most of a value of the command line a notice quotes.
const QUOTED: usize = 80;

/// Tells the owner that a round missed its session's prompt cache, and keeps the miss: with a new notice, or with the
/// notice still unread of a miss with the same cause on the same line of the same project.
pub fn cache_miss(e: &mut Engine, w: &Work, round: RoundId, cell: &str, line: &str, m: &Miss) -> Result<(), String> {
    let item = e.subject(w)?;
    let (project, about) = match &item {
        Some(i) => (i.project.clone(), format!("work {} (item {}: {})", w.id, i.id, i.title)),
        None => (
            w.project.clone().unwrap_or_default(),
            format!("work {} (a wake of the architect)", w.id),
        ),
    };
    let text = told(&project, &about, round, cell, line, m);
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
        "added to the report still unread"
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

/// Keeps a cache miss with the notice still unread of a miss with the same cause on the same line of the same project,
/// or with a new notice that tells the owner; gives whether it was added to a notice.
fn keep(tx: &Transaction<'_>, k: &Kept<'_>) -> db::Result<bool> {
    let differs = k.miss.differs_json().to_string();
    let open: Option<i64> = tx
        .query_row(
            "SELECT n.id FROM engine_notices n JOIN cache_misses c ON c.notice = n.id
             JOIN works x ON x.id = c.work LEFT JOIN items i ON i.id = x.item
             WHERE n.read_at IS NULL AND n.fault = ?1 AND c.line = ?2
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

/// What the owner is told of a cache miss: where it came up, what it wrote again and cost, and what differs in the
/// command line from the session's round before.
fn told(project: &str, about: &str, round: RoundId, cell: &str, line: &str, m: &Miss) -> String {
    let cost = match m.cost_usd {
        Some(c) => format!(", which cost about ${c:.2} more than reading them"),
        None => String::new(),
    };
    let differs = if m.differs.is_empty() {
        format!(
            "No part of the command line differs from the session's round before, {}.",
            m.before
        )
    } else {
        let parts: Vec<String> = m.differs.iter().map(differ).collect();
        format!(
            "The command line differs from the session's round before, {}, in: {}.",
            m.before,
            parts.join("; ")
        )
    };
    format!(
        "A fault of wf-agents, to be fixed in wf-agents: {round} of {about} in project {project}, at cell {cell} on \
         the {line} line, missed its session's prompt cache. It resumed the session {} minutes after the session's \
         last call, within the cache's hour, and wrote {} tokens to the cache again of the {} the session held{cost}. \
         {differs}",
        m.after_ms / 60_000,
        thousands(m.written),
        thousands(m.held),
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
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The owner is told where the miss came up, what it wrote again and cost, and what differs in the command line, or
    // that nothing does.
    #[test]
    fn a_cache_miss_is_told_with_its_cause() {
        let mut m = Miss {
            before: RoundId::new(7, 4),
            after_ms: 14 * 60_000 + 5_000,
            written: 412_345,
            held: 418_000,
            cost_usd: Some(1.234),
            differs: Vec::new(),
        };
        let text = told(
            "p",
            "work 7 (a wake of the architect)",
            RoundId::new(7, 5),
            "C1",
            "architect",
            &m,
        );
        assert!(
            text.contains("R7-5 of work 7 (a wake of the architect) in project p, at cell C1 on the architect line")
                && text.contains("14 minutes after")
                && text.contains(
                    "wrote 412,345 tokens to the cache again of the 418,000 the session held, which cost about $1.23"
                )
                && text.ends_with("No part of the command line differs from the session's round before, R7-4."),
            "{text}"
        );
        m.differs = vec![Differs {
            part: "the CLI's version".into(),
            before: Some("2.1.3".into()),
            after: Some("2.1.4".into()),
        }];
        let text = told("p", "work 7", RoundId::new(7, 5), "C1", "architect", &m);
        assert!(
            text.ends_with("differs from the session's round before, R7-4, in: the CLI's version (2.1.3 → 2.1.4)."),
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
                "INSERT INTO works (flow_version, graph, at, project) VALUES (1, 'architect', 'C1', 'p')",
                [],
            )?;
            for n in 1..=6 {
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, ?1, 'C1', '{}', 'c', ?1)",
                    [n],
                )?;
            }
            Ok(())
        })
        .unwrap();
        let mut m = Miss {
            before: RoundId::new(1, 1),
            after_ms: 60_000,
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
}
