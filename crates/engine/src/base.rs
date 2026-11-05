//! A project's architect's base and the fixed order it takes its matters in. The base is the session that read the
//! project and planned (cells `B1` and `B2`), which only saves tokens: every matter after it is handled one at a time in
//! a session forked from where it ended, given what came since. The engine takes the matters by fact, never by an
//! agent's pick: a new base when one is due, then what something stopped waits on (the owner's words and replies, then
//! main's CI red, then the matter a work stopped at its question waits on), then the rest of the queue. Here too are the
//! three facts that call for a new base, the input that carries what came since one, and the second eye's kinds of change.

use db::rusqlite::{Connection, OptionalExtension};
use flow::CompiledCell;
use serde_json::{Map, Value, json};

use crate::work::{RoundId, Work, query_json};
use crate::{ARCHITECT, Engine};

/// The most characters of what the architect said that what came since the base carries, cut at a word.
const SAID_MOST: usize = 1_000;

/// What joins a question of the architect's to the owner's answer in what came since.
const OWNER_ANSWERED: &str = "\nThe owner answered: ";

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn text(v: &Value) -> &str {
    v.as_str().unwrap_or_default()
}

/// A project's base as it stands: the session of the round that built it (its row and the CLI's id for it), that round,
/// and when it ended.
#[derive(Debug, Clone)]
pub struct Base {
    pub session: i64,
    pub cli_session: String,
    pub round: RoundId,
    pub at: i64,
}

/// The project's latest base; none before the first was built.
pub fn latest(conn: &Connection, project: &str) -> Result<Option<Base>, String> {
    conn.query_row(
        &format!(
            "SELECT r.session, s.cli_session, r.work, r.n, r.ended_at
             FROM rounds r JOIN works w ON w.id = r.work JOIN sessions s ON s.id = r.session
             WHERE w.graph = 'architect' AND w.project = ?1 AND r.ended_at IS NOT NULL AND {}
             ORDER BY r.ended_at DESC, r.work DESC, r.n DESC LIMIT 1",
            db::BASE_BUILT
        ),
        [project],
        |r| {
            Ok(Base {
                session: r.get(0)?,
                cli_session: r.get(1)?,
                round: RoundId::new(r.get(2)?, r.get(3)?),
                at: r.get(4)?,
            })
        },
    )
    .optional()
    .map_err(sql)
}

/// When the session of the project's latest planning began: its first round, from which a new base reads what was said
/// in the owner's one-to-one; none when the project has not planned yet.
pub fn previous_start(conn: &Connection, project: &str) -> Result<Option<i64>, String> {
    let session: Option<String> = conn
        .query_row(
            "SELECT s.cli_session FROM rounds r JOIN works w ON w.id = r.work JOIN sessions s ON s.id = r.session
             WHERE w.graph = 'architect' AND w.project = ?1 AND r.cell IN ('B2', 'N1') AND r.ended_at IS NOT NULL
             ORDER BY r.ended_at DESC LIMIT 1",
            [project],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?;
    let Some(session) = session else {
        return Ok(None);
    };
    conn.query_row(
        "SELECT min(r.started_at) FROM rounds r JOIN sessions s ON s.id = r.session WHERE s.cli_session = ?1",
        [&session],
        |r| r.get(0),
    )
    .map_err(sql)
}

/// What a round on a line that forks finds its session by ([`sessions::Fork`]): the matter the architect's work has in
/// hand and the project's latest base; none for a cell whose line forks from nothing, or of another graph.
pub fn fork_of(e: &Engine, w: &Work, cell: &CompiledCell, can_fork: bool) -> Result<Option<sessions::Fork>, String> {
    if w.graph != ARCHITECT || cell.fork_of.is_none() {
        return Ok(None);
    }
    let Some(project) = w.project.as_deref() else {
        return Ok(None);
    };
    let Some(matter) = crate::architect::picked(e.db.conn(), w.id)? else {
        return Ok(None);
    };
    let base = latest(e.db.conn(), project)?.map(|b| (b.session, b.cli_session));
    Ok(Some(sessions::Fork { matter, base, can_fork }))
}

// The fixed order.

/// Where a matter of the queue comes in the engine's fixed order, by what waits on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Class {
    /// The owner's words, a matter back with the owner's reply, a fault the owner forwarded by hand: the owner waits.
    Owner,
    /// Main's CI red: the merge queue is stopped.
    MainRed,
    /// A work stopped at its question waits, or a report says it causes loss now.
    Waiting,
    /// A step of the plan, taken in the plan's order, and a check of the policy, which the steps may rest on.
    Planned,
    /// Nothing stopped waits on it: a report, slots free with nothing to start, another architect's message.
    Queued,
}

impl Class {
    /// The verdict of the look at the queue that takes a matter of the class.
    pub fn verdict(self) -> &'static str {
        match self {
            Class::Owner => "owner",
            Class::MainRed => "main_red",
            Class::Waiting => "waiting",
            Class::Planned | Class::Queued => "queued",
        }
    }
}

/// The class of a matter of the queue, a row of `architect_queue` with its body (`body`), by fact.
pub fn class(row: &Value, body: &Value) -> Class {
    let kind = text(&row["kind"]);
    let detail = row["detail"].as_str();
    let flag = |k: &str| row[k] == json!(1) || row[k] == json!(true);
    if flag("answered") || kind == "owner_words" {
        return Class::Owner;
    }
    match (kind, detail) {
        ("report", Some(db::FAULT)) if body["by"] == "owner" => Class::Owner,
        ("main_red", _) => Class::MainRed,
        _ if row["waits"] == "agent" || flag("urgent") => Class::Waiting,
        ("set_straight", _) if body["measure"].is_null() => Class::Planned,
        ("check_policy", _) => Class::Planned,
        ("slots_free" | "set_straight" | "report" | "other_architect", _) => Class::Queued,
        // A work's question, an open question of a design whose merge waits for the answer, a failure, and any kind
        // the flow added since: something waits on it.
        _ => Class::Waiting,
    }
}

/// A project's queue as the engine takes it: each matter not handled, not withdrawn and not parked by a question the
/// owner has not answered, with its class, oldest first; flow feedback and what was said in the works' groups, which
/// no longer go to the architect, left out.
pub fn queue(conn: &Connection, project: &str) -> Result<Vec<(Value, Class)>, String> {
    let rows = query_json(
        conn,
        "SELECT q.*, m.body AS body FROM architect_queue q JOIN architect_matters m ON m.id = q.matter
         WHERE q.project = ?1 AND q.kind <> 'group_talk'
           AND NOT (q.kind IN ('report', 'other_architect') AND q.detail IS 'flow_feedback')
         ORDER BY q.at, q.matter",
        &[&project],
        100_000,
    )?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let body = match &r["body"] {
                Value::String(s) => serde_json::from_str(s).unwrap_or(Value::Null),
                v => v.clone(),
            };
            let c = class(&r, &body);
            (r, c)
        })
        .collect())
}

/// What comes next for a project's architect, as the look at the queue (`N0`) hands it in.
pub fn next(e: &Engine, project: &str) -> Result<Value, String> {
    let conn = e.db.conn();
    let due = due(e, project)?;
    if let Some(why) = due.why {
        return Ok(json!({ "next": { "verdict": "plan", "why": why } }));
    }
    let q = queue(conn, project)?;
    for c in [
        Class::Owner,
        Class::MainRed,
        Class::Waiting,
        Class::Planned,
        Class::Queued,
    ] {
        if let Some((r, _)) = q.iter().find(|(_, k)| *k == c) {
            return Ok(json!({ "next": { "verdict": c.verdict(), "matter": r["matter"] } }));
        }
    }
    Ok(json!({ "next": { "verdict": "none" } }))
}

/// Whether a matter is due for a project's architect, which starts a work of it (`S0`).
pub fn is_due(e: &Engine, project: &str) -> Result<bool, String> {
    Ok(!queue(e.db.conn(), project)?.is_empty())
}

// The facts that call for a new base.

/// Why a new base is due, in words, when one is; and every fact as the look at the queue records it.
#[derive(Debug, Default)]
pub struct Due {
    pub why: Option<String>,
    pub facts: Map<String, Value>,
}

/// The three facts that call for a new base: the project has none, the policy took effect after it was built, or what
/// came since it passed the installation's amount. Neither time nor a cold prompt cache is one.
pub fn due(e: &Engine, project: &str) -> Result<Due, String> {
    let conn = e.db.conn();
    let mut d = Due::default();
    let Some(base) = latest(conn, project)? else {
        d.why = Some("the project has no base yet".into());
        d.facts.insert("base".into(), json!("none yet"));
        return Ok(d);
    };
    d.facts.insert("base".into(), json!(base.round.to_string()));
    let approved: Option<i64> = conn
        .query_row(
            "SELECT max(r.ended_at) FROM policy_current p JOIN rounds r ON r.work = p.work AND r.n = p.round
             WHERE p.project = ?1",
            [project],
            |r| r.get(0),
        )
        .map_err(sql)?;
    let changed = approved.is_some_and(|at| at > base.at);
    d.facts.insert("policy_changed".into(), json!(changed));
    if changed {
        d.why = Some("the policy changed".into());
    }
    let carried = appended(conn, project, base.at)?.map_or(0, |t| t.chars().count());
    let most = e.config.config.install.architect.carried_most;
    d.facts
        .insert("came_since".into(), json!({ "characters": carried, "most": most }));
    if d.why.is_none() && u64::try_from(carried).unwrap_or(u64::MAX) >= most {
        d.why = Some(format!(
            "what came since the base is {carried} characters, the installation's amount being {most}"
        ));
    }
    Ok(d)
}

/// The facts the look at the queue records with what it took.
pub fn base_due(e: &Engine, project: &str) -> Result<Value, String> {
    let d = due(e, project)?;
    Ok(json!({ "why": d.why, "facts": d.facts }))
}

// What came since.

/// What came after `since`, as a matter reads it, one entry after another; none when nothing came.
fn appended(conn: &Connection, project: &str, since: i64) -> Result<Option<String>, String> {
    let entries = db::appended_since(conn, project, since).map_err(sql)?;
    if entries.is_empty() {
        return Ok(None);
    }
    let lines: Vec<String> = entries
        .iter()
        .map(|x| {
            // What the owner said stays whole; what the architect said is on the owner's page and in `owner_messages`.
            let said = match x.text.split_once(OWNER_ANSWERED) {
                _ if x.who.starts_with("owner") => x.text.clone(),
                Some((asked, answer)) => format!("{}{OWNER_ANSWERED}{answer}", flow::cut(asked, SAID_MOST)),
                None => flow::cut(&x.text, SAID_MOST),
            };
            let mut l = format!("{} {}: {}", crate::utc(x.at), x.who, said);
            if let Some(p) = &x.plan {
                l.push_str(&format!("\nIt adds to the plan: {p}"));
            }
            if let Some(u) = &x.understanding {
                l.push_str(&format!("\nIt adds to the understanding: {u}"));
            }
            l
        })
        .collect();
    Ok(Some(lines.join("\n")))
}

/// What came since the project's base was built, which a matter forked from it reads; none with no base.
pub fn appended_input(e: &Engine, project: &str) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let Some(base) = latest(conn, project)? else {
        return Ok(None);
    };
    Ok(appended(conn, project, base.at)?.map(Value::String))
}

/// What came since the plan before was written, which the round that writes the next plan takes in; none before the
/// first.
pub fn since_plan(e: &Engine, project: &str) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let Some(at) = db::plan_written_at(conn, project).map_err(sql)? else {
        return Ok(None);
    };
    Ok(appended(conn, project, at)?.map(Value::String))
}

// The second eye.

/// Whether a change of the items is of a kind the second eye checks before it is made: a cancel, a merge, a split (an
/// item created under another, taking acceptance conditions of it, or one moved under another), a change that rewrites
/// or removes an item's acceptance, and any change to an item a work runs on.
pub fn second_eye_checks(conn: &Connection, change: &Value) -> Result<bool, String> {
    let kind = text(&change["kind"]);
    if matches!(kind, "cancel" | "merge" | "edit_acceptance") {
        return Ok(true);
    }
    if kind == "create_item" && (!change["under"].is_null() || change.get("takes").is_some_and(|t| !t.is_null())) {
        return Ok(true);
    }
    if kind == "move" && change["to"]["place"] == "under" {
        return Ok(true);
    }
    for k in ["item", "keep"] {
        if let Some(id) = number(&change[k])
            && crate::architect::works_on(conn, id)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// An item's number as a change gives it, a number or a text of one.
fn number(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().trim_start_matches('#').parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(kind: &str, detail: Option<&str>, waits: &str) -> Value {
        json!({ "kind": kind, "detail": detail, "waits": waits, "answered": 0, "urgent": 0 })
    }

    // The engine's order takes what waits by fact: the owner first, then main red, then a work stopped at its question,
    // and the rest of the queue after; a side report that causes loss now is taken as a work waiting, and a fault the
    // owner forwarded by hand as the owner's.
    #[test]
    fn a_matter_s_class_is_what_waits_on_it() {
        let none = Value::Null;
        assert_eq!(class(&row("owner_words", Some("said"), "owner"), &none), Class::Owner);
        let mut back = row("engineer_question", None, "none");
        back["answered"] = json!(1);
        assert_eq!(class(&back, &none), Class::Owner);
        assert_eq!(class(&row("main_red", None, "none"), &none), Class::MainRed);
        assert_eq!(class(&row("engineer_question", None, "agent"), &none), Class::Waiting);
        assert_eq!(class(&row("open_question", None, "none"), &none), Class::Waiting);
        assert_eq!(
            class(&row("failure", Some("tool_failed"), "agent"), &none),
            Class::Waiting
        );
        for (kind, detail) in [
            ("slots_free", None),
            ("report", Some("defect")),
            ("report", Some("answer")),
            ("other_architect", Some("message")),
        ] {
            assert_eq!(class(&row(kind, detail, "none"), &none), Class::Queued, "{kind}");
        }
        assert_eq!(class(&row("set_straight", None, "none"), &none), Class::Planned);
        assert_eq!(class(&row("check_policy", None, "none"), &none), Class::Planned);
        let measure = json!({ "measure": { "measure": "m" } });
        assert_eq!(class(&row("set_straight", None, "none"), &measure), Class::Queued);
        let mut harms = row("report", Some("defect"), "none");
        harms["urgent"] = json!(1);
        assert_eq!(class(&harms, &none), Class::Waiting);
        let fault = row("report", Some(db::FAULT), "none");
        assert_eq!(class(&fault, &json!({ "by": "owner" })), Class::Owner);
        assert_eq!(class(&fault, &json!({ "by": "setting" })), Class::Queued);
        assert!(Class::Owner < Class::MainRed && Class::MainRed < Class::Waiting && Class::Waiting < Class::Queued);
    }

    // The second eye checks a cancel, a merge, a split and a rewrite of acceptance; a new item on its own, a text, an
    // order, a hold or a level on an item no work runs on take effect once the engine's checks pass.
    #[test]
    fn the_second_eye_checks_only_the_changes_that_can_lose_something() {
        let db = db::Db::in_memory().unwrap();
        let conn = db.conn();
        let checks = |c: Value| second_eye_checks(conn, &c).unwrap();
        assert!(checks(json!({ "kind": "cancel", "item": "4" })));
        assert!(checks(json!({ "kind": "merge", "keep": "4", "absorbed": ["5"] })));
        assert!(checks(
            json!({ "kind": "edit_acceptance", "item": "4", "acceptance": ["x"] })
        ));
        assert!(checks(
            json!({ "kind": "create_item", "key": "k", "under": { "item": "4", "dependency": "blocking" } })
        ));
        assert!(checks(
            json!({ "kind": "move", "item": "4", "to": { "place": "under", "item": "5" } })
        ));
        assert!(!checks(json!({ "kind": "create_item", "key": "k", "title": "t" })));
        assert!(!checks(
            json!({ "kind": "edit_item", "item": "4", "part": "title", "text": "t" })
        ));
        assert!(!checks(
            json!({ "kind": "move", "item": "4", "to": { "place": "first" } })
        ));
        assert!(!checks(json!({ "kind": "hold", "item": "4" })));
        assert!(!checks(json!({ "kind": "set_level", "item": "4", "level": "l" })));
    }
}
