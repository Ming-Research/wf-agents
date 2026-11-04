//! A project's architect's base and the fixed order it takes its matters in. The base is the session that read the
//! project last (cell `U1`, and `N1` when it planned): every matter after it is handled one at a time in a session forked
//! from where it ended, given the decision log since the base and what changed since. The engine takes the matters by
//! fact, never by an agent's pick: what something stopped waits on first (the owner's words and replies, then main's CI
//! red, then a work stopped at its question), then the base when it is due, then what the plan queued, then what waited
//! for the planning. Here too are the facts that call for a new base, the inputs that carry what changed since one, and
//! the second eye's kinds of change.

use std::collections::{BTreeMap, BTreeSet};

use db::rusqlite::{Connection, OptionalExtension, params};
use flow::CompiledCell;
use serde_json::{Map, Value, json};

use crate::work::{RoundId, Work, query_json};
use crate::{ARCHITECT, Engine};

/// The line of the base, which the lines of a matter's session and of the second eye fork from.
pub const BASE_LINE: &str = "architect";

/// The cells of the base: reading the project, and planning in the same session.
pub const READ: &str = "U1";
pub const PLAN: &str = "N1";

/// The rounds that leave a base built, of the rounds table named `r`: reading the project that found the plan holds when
/// only the base was due, and every plan that answered.
const BUILT: &str = "((r.cell = 'U1' AND r.transition = 'holds')
     OR (r.cell = 'N1' AND r.transition IN ('fits', 'some_to_change', 'set_straight')))";

/// The most characters of a text an input of the base gives, cut at a word.
const EXCERPT: usize = 400;

/// The most rows of each list an input of the base gives, the latest.
const ROWS: usize = 60;

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn text(v: &Value) -> &str {
    v.as_str().unwrap_or_default()
}

/// A text cut to at most [`EXCERPT`] characters at a word, its spaces collapsed.
pub fn excerpt(s: &str) -> String {
    flow::cut(&s.split_whitespace().collect::<Vec<_>>().join(" "), EXCERPT)
}

/// A project's base as it stands: the session of the round that built it (its row and the CLI's id for it), that round,
/// when it ended, the model it ran on and the commit of main it read the project at.
#[derive(Debug, Clone)]
pub struct Base {
    pub session: i64,
    pub cli_session: String,
    pub round: RoundId,
    pub at: i64,
    pub model: Option<String>,
    pub main: Option<String>,
}

/// The project's latest base; none before the first was built.
pub fn latest(conn: &Connection, project: &str) -> Result<Option<Base>, String> {
    conn.query_row(
        &format!(
            "SELECT r.session, s.cli_session, r.work, r.n, r.ended_at, r.model, json_extract(r.input, '$.engine.main')
             FROM rounds r JOIN works w ON w.id = r.work JOIN sessions s ON s.id = r.session
             WHERE w.graph = 'architect' AND w.project = ?1 AND r.ended_at IS NOT NULL AND {BUILT}
             ORDER BY r.ended_at DESC, r.work DESC, r.n DESC LIMIT 1"
        ),
        [project],
        |r| {
            Ok(Base {
                session: r.get(0)?,
                cli_session: r.get(1)?,
                round: RoundId::new(r.get(2)?, r.get(3)?),
                at: r.get(4)?,
                model: r.get(5)?,
                main: r.get(6)?,
            })
        },
    )
    .optional()
    .map_err(sql)
}

/// When the base before the project's latest one began: the first round of its session, which a new base reads what
/// was said in the owner's one-to-one since; none when the project has had no base, or only one being built.
pub fn previous_start(conn: &Connection, project: &str) -> Result<Option<i64>, String> {
    let Some(b) = latest(conn, project)? else {
        return Ok(None);
    };
    conn.query_row(
        "SELECT min(r.started_at) FROM rounds r JOIN sessions s ON s.id = r.session WHERE s.cli_session = ?1",
        [&b.cli_session],
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
    Ok(Some(sessions::Fork {
        matter,
        base,
        can_fork,
    }))
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
    /// Slots free with nothing of the project to start, or a plan a flow version before queued: a planning answers it.
    Planning,
    /// What the plan queued: a place to set straight, a check of the policy, a new measure.
    Planned,
    /// What waits for the planning: side reports, what to do next after a report, engineers' answers, faults the
    /// owner's setting forwarded, other architects' messages no work waits on.
    Gathered,
}

impl Class {
    /// The verdict of the look at the queue that takes a matter of the class.
    pub fn verdict(self) -> &'static str {
        match self {
            Class::Owner => "owner",
            Class::MainRed => "main_red",
            Class::Waiting => "waiting",
            Class::Planning => "plan",
            Class::Planned => "planned",
            Class::Gathered => "gathered",
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
        ("slots_free" | "make_plan", _) => Class::Planning,
        ("set_straight" | "check_policy", _) => Class::Planned,
        ("report" | "other_architect", _) => Class::Gathered,
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

/// The matters of a project's queue that wait for the planning, oldest first.
fn gathered_rows(conn: &Connection, project: &str) -> Result<Vec<Value>, String> {
    Ok(queue(conn, project)?
        .into_iter()
        .filter(|(_, c)| *c == Class::Gathered)
        .map(|(r, _)| r)
        .collect())
}

/// What comes next for a project's architect, as the look at the queue (`Q0`) hands it in.
pub fn next(e: &Engine, project: &str) -> Result<Value, String> {
    let conn = e.db.conn();
    let q = queue(conn, project)?;
    let first = |c: Class| q.iter().find(|(_, k)| *k == c).map(|(r, _)| r["matter"].clone());
    for c in [Class::Owner, Class::MainRed, Class::Waiting] {
        if let Some(m) = first(c) {
            return Ok(json!({ "next": { "verdict": c.verdict(), "matter": m } }));
        }
    }
    let due = due(e, project)?;
    if !due.plan.is_empty() {
        return Ok(json!({ "next": { "verdict": "plan", "why": due.plan.join("; ") } }));
    }
    if !due.rebuild.is_empty() {
        return Ok(json!({ "next": { "verdict": "rebuild", "why": due.rebuild.join("; ") } }));
    }
    if let Some(m) = first(Class::Planned) {
        return Ok(json!({ "next": { "verdict": "planned", "matter": m } }));
    }
    let built = latest(conn, project)?.map(|b| b.at);
    if let Some(m) = q
        .iter()
        .find(|(r, c)| *c == Class::Gathered && built.is_none_or(|at| r["at"].as_i64().unwrap_or(0) < at))
        .map(|(r, _)| r["matter"].clone())
    {
        return Ok(json!({ "next": { "verdict": "gathered", "matter": m } }));
    }
    Ok(json!({ "next": { "verdict": "none" } }))
}

/// Whether a matter is due for a project's architect, which starts a work of it (`S0`): one something waits on, one a
/// planning answers or the plan queued, one that waited for the planning and came before the base was built (or with
/// no base yet), or as many waiting for the planning as the installation's amount (`architect.gathered_most`).
pub fn is_due(e: &Engine, project: &str) -> Result<bool, String> {
    let conn = e.db.conn();
    let q = queue(conn, project)?;
    if q.iter().any(|(_, c)| *c != Class::Gathered) {
        return Ok(true);
    }
    let built = latest(conn, project)?.map(|b| b.at);
    let before = q
        .iter()
        .any(|(r, _)| built.is_none_or(|at| r["at"].as_i64().unwrap_or(0) < at));
    let most = usize::try_from(e.config.config.install.architect.gathered_most).unwrap_or(usize::MAX);
    Ok(before || q.len() >= most.max(1))
}

// The facts that call for a new base.

/// Which facts call for a planning and which for a rebuild of the base alone, each in words; and every fact as the look
/// at the queue records it.
#[derive(Debug, Default)]
pub struct Due {
    pub plan: Vec<String>,
    pub rebuild: Vec<String>,
    pub facts: Map<String, Value>,
}

/// The eight facts that call for a new base, since the project's latest base, and the project having none: for a
/// planning, the policy changed, slots free with nothing of the project to start (or a plan a flow version before
/// queued), the matters waiting for the planning reached the installation's amount, a matter handled since said it
/// changes the plan; for a rebuild alone, the decision log and what changed since grew past the installation's amount,
/// a merge since deleted or moved a file or changed the project's guide, a matter handled since said the base is wrong,
/// the architect's model is not the base's. Neither time nor a cold prompt cache is one.
pub fn due(e: &Engine, project: &str) -> Result<Due, String> {
    let conn = e.db.conn();
    let mut d = Due::default();
    let Some(base) = latest(conn, project)? else {
        d.plan.push("the project has no base yet".into());
        d.facts.insert("base".into(), json!("none yet"));
        return Ok(d);
    };
    d.facts.insert("base".into(), json!(base.round.to_string()));
    // The policy took effect after the base was built.
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
        d.plan.push("the policy changed".into());
    }
    let q = queue(conn, project)?;
    let planning: Vec<String> = q
        .iter()
        .filter(|(_, c)| *c == Class::Planning)
        .map(|(r, _)| text(&r["what"]).to_string())
        .collect();
    d.facts.insert("slots_free".into(), json!(planning));
    if let Some(w) = planning.first() {
        d.plan.push(w.clone());
    }
    let gathered = q.iter().filter(|(_, c)| *c == Class::Gathered).count();
    let most = e.config.config.install.architect.gathered_most;
    d.facts
        .insert("waiting_for_planning".into(), json!({ "matters": gathered, "most": most }));
    if u64::try_from(gathered).unwrap_or(u64::MAX) >= most.max(1) {
        d.plan.push(format!(
            "{gathered} matters wait for the planning, the installation's amount being {most}"
        ));
    }
    let said = logged_since(conn, project, base.at)?;
    let plan_changes: Vec<String> = said
        .iter()
        .filter(|l| l["plan"] == "yes")
        .map(|l| format!("matter {}: {}", l["matter"], text(&l["plan_why"])))
        .collect();
    d.facts.insert("changes_the_plan".into(), json!(plan_changes));
    if let Some(p) = plan_changes.first() {
        d.plan.push(format!("a matter handled since changes the plan ({p})"));
    }
    // A rebuild alone.
    let log = decision_log_of(conn, project, base.at)?;
    let since = since_base_of(e, project, &base)?;
    let carried = log.to_string().chars().count() + since.to_string().chars().count();
    let carried_most = e.config.config.install.architect.carried_most;
    d.facts
        .insert("carried".into(), json!({ "characters": carried, "most": carried_most }));
    if u64::try_from(carried).unwrap_or(u64::MAX) >= carried_most {
        d.rebuild.push(format!(
            "the decision log and what changed since the base are {carried} characters, the installation's amount being \
             {carried_most}"
        ));
    }
    let moved = structure_moved(e, project, &base)?;
    d.facts.insert("structure".into(), json!(moved));
    if let Some(m) = moved.first() {
        d.rebuild.push(format!("a merge since the base moved the project's structure ({m})"));
    }
    let wrong: Vec<String> = said
        .iter()
        .filter(|l| l["base"] == "wrong")
        .map(|l| format!("matter {}: {}", l["matter"], text(&l["base_why"])))
        .collect();
    d.facts.insert("base_wrong".into(), json!(wrong));
    if let Some(w) = wrong.first() {
        d.rebuild.push(format!("a matter handled since found the base wrong ({w})"));
    }
    let level = e.config.config.architect_level();
    let model = e.config.config.level(&level).map(|(l, _)| l.model.clone());
    let other = matches!((&model, &base.model), (Some(now), Some(then)) if now != then);
    d.facts
        .insert("model".into(), json!({ "now": model, "base": base.model }));
    if other {
        d.rebuild.push("the architect's model is not the base's".into());
    }
    Ok(d)
}

/// The facts the look at the queue records with what it took, and the planning is given as why it is due.
pub fn base_due(e: &Engine, project: &str) -> Result<Value, String> {
    let d = due(e, project)?;
    Ok(json!({ "plan": d.plan, "rebuild": d.rebuild, "facts": d.facts }))
}

/// Why the planning is due, as the look at the queue found it, with the slots free and the items ready to start.
pub fn planning_why(e: &Engine, project: &str) -> Result<Value, String> {
    let d = due(e, project)?;
    let why = if d.plan.is_empty() {
        d.rebuild.clone()
    } else {
        d.plan.clone()
    };
    let slots: Vec<Value> = queue(e.db.conn(), project)?
        .into_iter()
        .filter(|(_, c)| *c == Class::Planning)
        .map(|(r, _)| r["matter"].clone())
        .collect();
    let mut out = json!({ "why": why });
    if !slots.is_empty() {
        let bodies = query_json(
            e.db.conn(),
            "SELECT id, what, body FROM architect_matters WHERE id IN (SELECT value FROM json_each(?1))",
            &[&Value::Array(slots).to_string()],
            ROWS,
        )?;
        out["slots_free"] = Value::Array(
            bodies
                .into_iter()
                .map(|b| {
                    let body = match &b["body"] {
                        Value::String(s) => serde_json::from_str(s).unwrap_or(Value::Null),
                        v => v.clone(),
                    };
                    json!({ "what": b["what"], "free": body["free"], "ready": body["ready"] })
                })
                .collect(),
        );
    }
    Ok(out)
}

/// What each matter handled since `at` left as its session ended (`logged`), oldest first, with its matter.
fn logged_since(conn: &Connection, project: &str, at: i64) -> Result<Vec<Value>, String> {
    let rows = query_json(
        conn,
        "SELECT m.id, m.kind, m.logged, r.ended_at FROM architect_matters m
         JOIN rounds r ON r.work = m.handled_work AND r.n = m.handled_round
         WHERE m.project = ?1 AND m.logged IS NOT NULL AND r.ended_at > ?2
         ORDER BY r.ended_at, m.id",
        &[&project, &at],
        100_000,
    )?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let mut l = match &r["logged"] {
                Value::String(s) => serde_json::from_str(s).unwrap_or(Value::Null),
                v => v.clone(),
            };
            l["matter"] = r["id"].clone();
            l["kind"] = r["kind"].clone();
            l
        })
        .collect())
}

/// The decision log since `at`: each matter's line, by its number, oldest first, so that a session given it once is given
/// after that only the lines new to it.
fn decision_log_of(conn: &Connection, project: &str, at: i64) -> Result<Value, String> {
    let log: Map<String, Value> = logged_since(conn, project, at)?
        .into_iter()
        .map(|l| {
            (
                format!("#{}", l["matter"]),
                json!({ "kind": l["kind"], "round": l["round"], "line": l["log"] }),
            )
        })
        .collect();
    Ok(Value::Object(log))
}

/// The decision log since the project's base was built; none when nothing was handled since, or there is no base.
pub fn decision_log(e: &Engine, project: &str) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let Some(base) = latest(conn, project)? else {
        return Ok(None);
    };
    let log = decision_log_of(conn, project, base.at)?;
    Ok((log.as_object().is_some_and(|o| !o.is_empty())).then_some(log))
}

/// What changed in the project since its base was built ([`since_base_of`]); with no base, since a day ago.
pub fn since_base(e: &Engine, project: &str) -> Result<Value, String> {
    let conn = e.db.conn();
    match latest(conn, project)? {
        Some(b) => since_base_of(e, project, &b),
        None => {
            let day_ago = db::now() - 24 * crate::architect::HOUR;
            since_at(e, project, day_ago, None)
        }
    }
}

/// What changed in the project since the base `base` was built.
fn since_base_of(e: &Engine, project: &str, base: &Base) -> Result<Value, String> {
    since_at(e, project, base.at, base.main.as_deref())
}

/// What changed in the project since `at`, main having been at `main` then: the items created, changed, cancelled or
/// merged, each with its title, its version now and what of it changed; the works that moved on, each with the cells it
/// went through and where it stands; the merges into main; and what the owner said since, to the architect and in the
/// works' groups, each cut to an excerpt.
fn since_at(e: &Engine, project: &str, at: i64, main: Option<&str>) -> Result<Value, String> {
    let conn = e.db.conn();
    let changes = query_json(
        conn,
        "SELECT c.id, json_extract(c.row_key, '$.id') AS item, c.old_row, c.new_row, c.actor FROM changes c
         JOIN items i ON i.id = json_extract(c.row_key, '$.id')
         WHERE c.table_name = 'items' AND c.at > ?1 AND i.project = ?2 ORDER BY c.id",
        &[&at, &project],
        100_000,
    )?;
    let mut items: BTreeMap<i64, (BTreeSet<String>, i64, bool)> = BTreeMap::new();
    for c in &changes {
        let Some(id) = c["item"].as_i64() else { continue };
        let old: Value = match &c["old_row"] {
            Value::String(s) => serde_json::from_str(s).unwrap_or(Value::Null),
            v => v.clone(),
        };
        let new: Value = match &c["new_row"] {
            Value::String(s) => serde_json::from_str(s).unwrap_or(Value::Null),
            v => v.clone(),
        };
        let entry = items.entry(id).or_insert((BTreeSet::new(), 0, false));
        entry.1 = c["id"].as_i64().unwrap_or(entry.1);
        if old.is_null() {
            entry.2 = true;
        }
        if let (Some(o), Some(n)) = (old.as_object(), new.as_object()) {
            for (k, v) in n {
                if o.get(k) != Some(v) {
                    entry.0.insert(k.clone());
                }
            }
        }
    }
    let mut changed = Map::new();
    for (id, (fields, version, created)) in items.iter().rev().take(ROWS) {
        let (title, cancelled): (String, Option<String>) = conn
            .query_row("SELECT title, cancelled FROM items WHERE id = ?1", [id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .map_err(sql)?;
        let mut x = json!({ "title": title, "version": version });
        if *created {
            x["created"] = json!(true);
        } else {
            x["changed"] = json!(fields.iter().collect::<Vec<_>>());
        }
        if let Some(c) = cancelled {
            x["cancelled"] = json!(c);
        }
        changed.insert(format!("#{id}"), x);
    }
    // The works that moved: the cells their rounds went through since, each work's in order, and where it stands.
    let moved = query_json(
        conn,
        "SELECT w.id AS work, w.item, w.at, w.ended, group_concat(r.cell, ' > ') AS cells
         FROM works w JOIN items i ON i.id = w.item
         JOIN (SELECT work, cell, n FROM rounds WHERE started_at > ?1 AND cell IS NOT NULL ORDER BY work, n) r
           ON r.work = w.id
         WHERE w.graph = 'engineer' AND i.project = ?2
         GROUP BY w.id ORDER BY w.id DESC LIMIT 60",
        &[&at, &project],
        ROWS,
    )?;
    let works: Map<String, Value> = moved
        .into_iter()
        .map(|w| {
            let mut x = json!({ "item": w["item"], "at": w["at"], "through": w["cells"] });
            if let Some(end) = w["ended"].as_str() {
                x["ended"] = json!(end);
            }
            (format!("W{}", w["work"]), x)
        })
        .collect();
    let merges = query_json(
        conn,
        "SELECT w.item, json_extract(r.output, '$.result.merge_commit') AS commit_merged, r.ended_at
         FROM rounds r JOIN works w ON w.id = r.work JOIN items i ON i.id = w.item
         WHERE w.graph = 'engineer' AND i.project = ?2 AND r.ended_at > ?1
           AND json_extract(r.output, '$.result.kind') = 'merged'
         ORDER BY r.ended_at",
        &[&at, &project],
        ROWS,
    )?;
    let merged: Vec<Value> = merges
        .into_iter()
        .map(|m| json!({ "item": m["item"], "commit": m["commit_merged"] }))
        .collect();
    // What the owner said since: to the architect, wherever, and to a work's engineer in its group.
    let words = query_json(
        conn,
        "SELECT id, what, json_extract(body, '$.text') AS said, at FROM architect_matters
         WHERE project = ?1 AND kind = 'owner_words' AND at > ?2 ORDER BY at",
        &[&project, &at],
        ROWS,
    )?;
    // An interjection is said when its row is made, as the log of changes keeps it.
    let to_works = query_json(
        conn,
        "SELECT x.id, x.work, w.item, x.text, c.at FROM interjections x JOIN works w ON w.id = x.work
         JOIN items i ON i.id = w.item
         JOIN changes c ON c.table_name = 'interjections' AND json_extract(c.row_key, '$.id') = x.id
           AND c.old_row IS NULL
         WHERE x.speaker = 'owner' AND i.project = ?1 AND c.at > ?2 ORDER BY c.at",
        &[&project, &at],
        ROWS,
    )?;
    let mut owner = Map::new();
    for w in words {
        let said = w["said"].as_str().unwrap_or_else(|| text(&w["what"]));
        owner.insert(
            format!("matter {}", w["id"]),
            json!({ "at": w["at"].as_i64().map(crate::utc), "said": excerpt(said) }),
        );
    }
    for x in to_works {
        owner.insert(
            format!("to work {} on #{}", x["work"], x["item"]),
            json!({ "at": x["at"].as_i64().map(crate::utc), "said": excerpt(text(&x["text"])) }),
        );
    }
    let mut out = json!({ "since": crate::utc(at), "items": changed, "works": works, "merged": merged, "owner": owner });
    if let Some(m) = main {
        out["main_then"] = json!(m);
    }
    Ok(out)
}

/// The merges since the base that deleted or moved a tracked file, or changed the project's guide (its AGENTS.md or
/// README.md at the root), as the architect's worktree has main now against the commit the base read: each such file
/// with what became of it. None when the base read no commit, or the worktree is not there.
fn structure_moved(e: &Engine, project: &str, base: &Base) -> Result<Vec<String>, String> {
    let Some(then) = base.main.as_deref() else {
        return Ok(Vec::new());
    };
    let dir = crate::architect::worktree(&e.data, ARCHITECT, project);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let Ok(out) = crate::git::git(&dir, &["diff", "--name-status", "-M", then, "HEAD"]) else {
        return Ok(Vec::new());
    };
    Ok(out
        .lines()
        .filter_map(|l| {
            let mut parts = l.split('\t');
            let status = parts.next()?;
            let path = parts.collect::<Vec<_>>().join(" to ");
            let guide = matches!(path.as_str(), "AGENTS.md" | "README.md");
            match status.chars().next()? {
                'D' => Some(format!("{path} deleted")),
                'R' => Some(format!("{path} moved")),
                _ if guide => Some(format!("{path} changed")),
                _ => None,
            }
        })
        .take(ROWS)
        .collect())
}

/// The matters that wait for the planning, oldest first, each with its kind, item, since when, what it is and an
/// excerpt of its text; none when none waits.
pub fn gathered(e: &Engine, project: &str) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let rows = gathered_rows(conn, project)?;
    if rows.is_empty() {
        return Ok(None);
    }
    let mut out = Map::new();
    for r in rows.iter().take(200) {
        let m = r["matter"].as_i64().unwrap_or_default();
        let body: Value = conn
            .query_row("SELECT body FROM architect_matters WHERE id = ?1", [m], |x| {
                x.get::<_, String>(0)
            })
            .map(|b| serde_json::from_str(&b).unwrap_or(Value::Null))
            .map_err(sql)?;
        let said = body["report"]["what"]
            .as_str()
            .or_else(|| body["text"].as_str())
            .or_else(|| body["followup"].as_str())
            .unwrap_or_default();
        let mut x = json!({
            "kind": r["kind"],
            "item": r["item"],
            "since": r["at"].as_i64().map(crate::utc),
            "what": r["what"],
        });
        if let Some(d) = r["detail"].as_str() {
            x["detail"] = json!(d);
        }
        if !said.trim().is_empty() {
            x["excerpt"] = json!(excerpt(said));
        }
        out.insert(format!("#{m}"), x);
    }
    Ok(Some(Value::Object(out)))
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

    // The engine's order takes what waits by fact: the owner first, then main red, then a work stopped at its question;
    // what the plan queued and what waits for the planning after; a side report that causes loss now is taken as a work
    // waiting, and a fault the owner forwarded by hand as the owner's.
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
        assert_eq!(class(&row("failure", Some("tool_failed"), "agent"), &none), Class::Waiting);
        assert_eq!(class(&row("slots_free", None, "none"), &none), Class::Planning);
        assert_eq!(class(&row("set_straight", Some("order"), "none"), &none), Class::Planned);
        assert_eq!(class(&row("check_policy", None, "none"), &none), Class::Planned);
        assert_eq!(class(&row("report", Some("defect"), "none"), &none), Class::Gathered);
        assert_eq!(class(&row("report", Some("answer"), "none"), &none), Class::Gathered);
        assert_eq!(class(&row("other_architect", Some("message"), "none"), &none), Class::Gathered);
        let mut harms = row("report", Some("defect"), "none");
        harms["urgent"] = json!(1);
        assert_eq!(class(&harms, &none), Class::Waiting);
        let fault = row("report", Some(db::FAULT), "none");
        assert_eq!(class(&fault, &json!({ "by": "owner" })), Class::Owner);
        assert_eq!(class(&fault, &json!({ "by": "setting" })), Class::Gathered);
        assert!(Class::Owner < Class::MainRed && Class::MainRed < Class::Waiting);
        assert!(Class::Waiting < Class::Planning && Class::Planned < Class::Gathered);
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
        assert!(checks(json!({ "kind": "edit_acceptance", "item": "4", "acceptance": ["x"] })));
        assert!(checks(json!({ "kind": "create_item", "key": "k", "under": { "item": "4", "dependency": "blocking" } })));
        assert!(checks(json!({ "kind": "move", "item": "4", "to": { "place": "under", "item": "5" } })));
        assert!(!checks(json!({ "kind": "create_item", "key": "k", "title": "t" })));
        assert!(!checks(json!({ "kind": "edit_item", "item": "4", "part": "title", "text": "t" })));
        assert!(!checks(json!({ "kind": "move", "item": "4", "to": { "place": "first" } })));
        assert!(!checks(json!({ "kind": "hold", "item": "4" })));
        assert!(!checks(json!({ "kind": "set_level", "item": "4", "level": "l" })));
    }
}
