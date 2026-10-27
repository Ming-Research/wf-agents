//! The architect's side of the engine. A wake is a work of the architect's graph on no item: the engine starts one
//! when none runs and the architect's queue has a matter due, one at a time, and a wake holds no slot. What puts a
//! matter in the queue is here and in the actions: an engineer's question, a side report, a design card, the owner's
//! words, answers and approvals, a place to set straight, an advance the engine raises, a next look the architect set
//! and a new policy. Here too are the steps, inputs, values, checks, actions and the engine_op tool of the architect's
//! graph, and its asks of the owner. The architect's line goes on in one CLI session across its wakes until a new
//! policy ends it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use db::rusqlite::{Connection, OptionalExtension, Transaction, params};
use db::{Actor, Matter, queue_matter};
use flow::format::Src;
use flow::{CompiledCell, CompiledGraph};
use serde_json::{Map, Value, json};

use crate::actions::{self, Ctx};
use crate::checks;
use crate::inputs::{self, Gathered, Origin, at_path, last_return};
use crate::message::Held;
use crate::steps::{finish, what_is_asked};
use crate::work::{self, Ended, Item, Round, RoundId, Work, json_col, query_json};
use crate::{ARCHITECT, Advance, ENGINEER, Engine, git, values};

/// An hour, in milliseconds: how long a side report that is not urgent waits before it is due, and the least time
/// between two advances the engine raises.
pub const HOUR: i64 = 3_600_000;

/// Why a work the architect paused is paused: the start of its reason, by which the architect alone lets it go on.
pub const PAUSED_BY_ARCHITECT: &str = "paused by the architect";

/// The engine's inputs the architect's cells read, each computed by [`input`].
pub const INPUTS: [&str; 10] = [
    "since_last_wake",
    "projects_guides",
    "constitutions",
    "policy_signals",
    "running_and_next",
    "slots",
    "matter_material",
    "whole_view",
    "similar_items",
    "changes_touch",
];

/// The kinds of point an engineer's question is queued as a matter of the same kind; a question of any other kind is
/// queued as other_question.
const QUESTION_KINDS: [&str; 7] = [
    "split",
    "duplicate",
    "depends_on_other_item",
    "out_of_scope_defect",
    "root_elsewhere",
    "too_costly",
    "over_plan",
];

/// The most rows an input of the architect's gives of one list.
const ROWS: usize = 200;

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn count(conn: &Connection, q: &str, args: &[&dyn db::rusqlite::ToSql]) -> Result<i64, String> {
    conn.query_row(q, args, |r| r.get(0)).map_err(sql)
}

/// The items of a list, a value that is no list being one item and none being none.
fn items(v: Option<&Value>) -> Vec<Value> {
    match v {
        Some(Value::Array(xs)) => xs.clone(),
        Some(Value::Null) | None => Vec::new(),
        Some(other) => vec![other.clone()],
    }
}

/// Whether a value has nothing in it.
fn empty(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.trim().is_empty(),
        Value::Array(xs) => xs.is_empty(),
        Value::Object(o) => o.is_empty(),
        _ => false,
    }
}

// Where the architect works.

/// The architect's working directory: `worktrees/architect` in the data directory, which its git leaves out.
pub fn workdir(data: &Path) -> PathBuf {
    config::worktrees(data).join(ARCHITECT)
}

/// The architect's working directory, made when it is not there yet.
pub fn made_workdir(data: &Path) -> Result<PathBuf, String> {
    let dir = workdir(data);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir)
}

// Wakes.

/// The wake that runs: the newest wake not stopped, unless it reached its end. A wake starts only while none runs, so
/// an older one has ended or reached its end. A paused wake still runs.
pub fn running(e: &mut Engine) -> Result<Option<i64>, String> {
    let newest: Option<i64> =
        e.db.conn()
            .query_row(
                "SELECT max(id) FROM works WHERE graph = 'architect' AND ended IS NULL",
                [],
                |r| r.get(0),
            )
            .map_err(sql)?;
    let Some(id) = newest else { return Ok(None) };
    let w = e.work(id)?;
    Ok((!e.done(&w)?).then_some(id))
}

/// Whether the architect's queue has a matter due.
pub fn queue_has(e: &Engine) -> Result<bool, String> {
    Ok(count(e.db.conn(), "SELECT count(*) FROM architect_queue", &[])? > 0)
}

/// Starts a wake: a work of the architect's graph on no item, at its start, on the flow version new works start on;
/// paused by the owner when the owner's step started it.
pub fn start(e: &mut Engine, paused: bool) -> Result<i64, String> {
    let v = e.flow_now;
    let start = e
        .flow(v)?
        .graph(ARCHITECT)
        .map(|g| g.start.clone())
        .ok_or("the flow has no architect's graph")?;
    let paused = paused.then_some(db::PAUSED_BY_OWNER);
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, paused, paused_by)
             VALUES (NULL, ?1, ?2, ?3, ?4, CASE WHEN ?4 IS NULL THEN NULL ELSE 'owner' END)",
            params![v, ARCHITECT, start, paused],
        )?;
        Ok(tx.last_insert_rowid())
    })
    .map_err(sql)
}

/// The machines' slots, all together.
fn total_slots(e: &Engine) -> i64 {
    e.config
        .config
        .install
        .machines
        .hosts
        .values()
        .map(|h| i64::from(h.slots))
        .sum()
}

/// Queues what the engine raises itself once it is due: an advance, when fewer items are ready to start than there are
/// slots, none waits in the queue, and the last was handled more than an hour ago or never; the hour keeps an advance
/// that found nothing to start from coming back at once.
pub fn queue_due(e: &mut Engine) -> Result<(), String> {
    let slots = total_slots(e);
    let conn = e.db.conn();
    let ready = count(conn, "SELECT count(*) FROM ready_items", &[])?;
    if ready >= slots {
        return Ok(());
    }
    let waiting = count(
        conn,
        "SELECT count(*) FROM architect_matters WHERE kind = 'advance' AND handled_round IS NULL",
        &[],
    )?;
    if waiting > 0 {
        return Ok(());
    }
    let last: Option<i64> = conn
        .query_row(
            "SELECT max(r.ended_at) FROM architect_matters m JOIN rounds r ON r.work = m.handled_work AND r.n = m.handled_round
             WHERE m.kind = 'advance'",
            [],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if last.is_some_and(|t| db::now() - t <= HOUR) {
        return Ok(());
    }
    let m = Matter {
        kind: "advance".into(),
        what: format!("Fewer items are ready to start ({ready}) than there are slots ({slots})"),
        body: json!({ "slots": slots, "ready": ready }),
        ..Matter::default()
    };
    e.db.write(Actor::Engine, |tx| queue_matter(tx, &m)).map_err(sql)?;
    Ok(())
}

/// Cell A0: the wake goes on in the CLI session the last wake left, unless a new policy ended it: its line's session is
/// a row of this wake's with the same CLI session, which the next round of the line resumes. The first wake, and the
/// first after a new policy took effect, start a new one at their first round; so does a wake that comes back here
/// after its new policy ended its session.
pub fn wake(e: &mut Engine, w: &Work) -> Result<Value, String> {
    let conn = e.db.conn();
    let had = count(
        conn,
        "SELECT count(*) FROM sessions WHERE work = ?1 AND line = 'architect'",
        &[&w.id],
    )?;
    if had > 0 {
        return Ok(json!({}));
    }
    let last: Option<(String, String, Option<i64>)> = conn
        .query_row(
            "SELECT s.cli, s.cli_session, s.ended FROM sessions s JOIN works x ON x.id = s.work
             WHERE x.graph = 'architect' AND s.line = 'architect' AND s.work < ?1 ORDER BY s.id DESC LIMIT 1",
            [w.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(sql)?;
    if let Some((cli, cli_session, None)) = last {
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO sessions (work, line, cli, cli_session) VALUES (?1, 'architect', ?2, ?3)",
                params![w.id, cli, cli_session],
            )?;
            Ok(())
        })
        .map_err(sql)?;
    }
    Ok(json!({}))
}

/// The matter the sixth question last picked in a wake, as its return chose it, with its number in the queue when it
/// has one; none before it picked, and after it found nothing to do.
pub fn picked(conn: &Connection, work: i64, within: &str) -> Result<Option<Value>, String> {
    let chosen: Option<Option<String>> = conn
        .query_row(
            "SELECT json_extract(output, ?2) FROM rounds
             WHERE work = ?1 AND cell = 'A6' AND transition IS NOT NULL AND json_extract(output, ?3) IS NOT NULL
             ORDER BY n DESC LIMIT 1",
            params![work, format!("$.{within}.pick.chosen"), format!("$.{within}.pick")],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?;
    Ok(json_col(chosen.flatten()))
}

/// The number in the queue of the matter the sixth question last picked in a wake, when it picked one from the queue.
pub fn picked_matter(conn: &Connection, work: i64, within: &str) -> Result<Option<i64>, String> {
    Ok(picked(conn, work, within)?.and_then(|c| c["queued"].as_i64()))
}

/// Whether the architect's line goes on in the session the last wake left, or is new: a session started afresh, or
/// one compacted since it last held an earlier wake's rounds, holds nothing from before.
fn session(e: &Engine, w: &Work) -> Result<String, String> {
    let cli_session: Option<String> =
        e.db.conn()
            .query_row(
                "SELECT cli_session FROM sessions WHERE work = ?1 AND line = 'architect' AND ended IS NULL",
                [w.id],
                |r| r.get(0),
            )
            .optional()
            .map_err(sql)?;
    let Some(cli_session) = cli_session else {
        return Ok("new".into());
    };
    let held = Held::of(&work::session_rounds(e.db.conn(), &cli_session)?);
    Ok(if held.rounds.iter().any(|r| r.work != w.id) {
        "continued".into()
    } else {
        "new".into()
    })
}

/// One of the engine's values of the architect's graph; none for a value of another graph.
pub fn value(e: &Engine, w: &Work, g: &CompiledGraph, name: &str) -> Result<Option<String>, String> {
    Ok(Some(match name {
        "session" => session(e, w)?,
        "about" => picked(e.db.conn(), w.id, &g.within)?
            .and_then(|c| c["about"].as_str().map(str::to_string))
            .unwrap_or_default(),
        "tidy_left" => {
            let left = count(
                e.db.conn(),
                "SELECT count(*) FROM architect_matters WHERE kind = 'running_unfit' AND handled_round IS NULL
                 AND held IS NULL",
                &[],
            )?;
            if left > 0 { "some" } else { "none" }.to_string()
        }
        _ => return Ok(None),
    }))
}

// Asks of the owner.

/// The ask of the owner a round put, and its answer once the owner gave it.
pub fn owner_ask_of(e: &Engine, round: RoundId) -> Result<Option<(i64, Option<Value>)>, String> {
    e.db.conn()
        .query_row(
            "SELECT id, answer FROM owner_asks WHERE work = ?1 AND round = ?2 ORDER BY id LIMIT 1",
            [round.work, round.n],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)),
        )
        .optional()
        .map(|o| o.map(|(id, a)| (id, json_col(a))))
        .map_err(sql)
}

/// What a step that is not an agent's round records of its inputs: the value of each, by its sources.
fn recorded(gathered: &[Gathered]) -> Value {
    Value::Array(
        gathered
            .iter()
            .filter(|x| !x.itself())
            .map(|x| json!({ "from": x.input.from, "value": x.value }))
            .collect(),
    )
}

/// What a round asking someone holds of what the cells handed in: each input read from a return, with what it is.
fn asked_of(gathered: &[Gathered]) -> Vec<Value> {
    gathered
        .iter()
        .filter(|x| x.value.is_some() && x.input.from.iter().all(|s| matches!(s, Src::Place(..))))
        .map(|x| json!({ "what": x.input.what, "value": x.value }))
        .collect()
}

/// The options an owner's cell offers, each with what it means, as its return's schema lists them for `ruling.choice`.
fn choices(cell: &CompiledCell) -> Vec<Value> {
    let Some(f) = cell
        .schema
        .as_ref()
        .and_then(|s| s.pointer("/properties/ruling/properties/choice"))
    else {
        return Vec::new();
    };
    let values: Vec<String> = f["enum"]
        .as_array()
        .map(|xs| xs.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let desc = f["description"].as_str().unwrap_or_default();
    let listed = desc.rsplit_once("Values: ").map(|x| x.1).unwrap_or(desc);
    values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let start = listed.find(&format!("{v}: ")).map(|p| p + v.len() + 2);
            let end = values
                .get(i + 1)
                .and_then(|n| listed.find(&format!("; {n}: ")))
                .unwrap_or(listed.len());
            let means = start.filter(|s| *s <= end).map(|s| &listed[s..end]).unwrap_or_default();
            json!([v, means])
        })
        .collect()
}

/// What an answer of the owner's comes back to an owner's cell of the architect's graph as: a ruling with the option
/// chosen and the words, for a cell that rules; a reply with the words otherwise.
fn answered(cell: &CompiledCell, answer: &Value) -> Value {
    let text = answer["text"].as_str().unwrap_or_default();
    let rules = cell
        .schema
        .as_ref()
        .and_then(|s| s.pointer("/properties/ruling"))
        .is_some();
    if rules {
        json!({ "ruling": { "choice": answer["choice"], "text": text } })
    } else {
        json!({ "reply": { "text": text } })
    }
}

/// The cells A14, P2 and P9: what the architect asks, or puts before the owner to read or approve, becomes an ask of
/// the owner, and the wake waits for the answer, holding no slot. An ask from the eighth question holds the matter it
/// handles instead: the wake does not wait, the round ends at once saying so, and the owner's answer comes back to the
/// queue as a matter of its own.
pub fn ask_owner(
    e: &mut Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    open: Option<Round>,
) -> Result<Advance, String> {
    if let Some(r) = open.filter(|r| r.cell.as_deref() == Some(cell.id.as_str())) {
        let Some((id, answer)) = owner_ask_of(e, r.id)? else {
            return Err(format!("{} asks, but put no ask to the owner", r.id));
        };
        let Some(answer) = answer else {
            return Ok(Advance::Waits(format!("ask {id} waits for the owner")));
        };
        let output = answered(cell, &answer);
        let gathered = inputs::gather(e, w, g, cell, None)?;
        let given = values::given(e, w, g, cell, None, &gathered)?;
        let row = g.take(&cell.id, &output, &given)?.clone();
        return finish(e, w, g, cell, r.id, output, &row, Ended::default(), &[], None, &given);
    }
    let gathered = inputs::gather(e, w, g, cell, None)?;
    let given = values::given(e, w, g, cell, None, &gathered)?;
    let eighth = w.pos.callers.last().map(String::as_str) == Some("A8");
    let holds = if eighth {
        picked_matter(e.db.conn(), w.id, &g.within)?
    } else {
        None
    };
    let mut body = json!({ "from": "architect", "work": w.id, "cell": cell.id });
    let trials = gathered
        .iter()
        .flat_map(|x| &x.parts)
        .find(|p| p.origin == Origin::Db("policy_trials".into()))
        .and_then(|p| items(Some(&p.value)).into_iter().next());
    let notice = if let Some(t) = trials {
        body["kind"] = json!("policy_approval");
        body["policy"] = t["policy"].clone();
        body["probes"] = t["probes"].clone();
        body["readings"] = t["readings"].clone();
        body["choices"] = Value::Array(choices(cell));
        "The architect's new policy, read by new architects, waits for your approval".to_string()
    } else if !choices(cell).is_empty() {
        body["kind"] = json!("study");
        body["study"] = gathered.iter().find_map(|x| x.value.clone()).unwrap_or(Value::Null);
        body["choices"] = Value::Array(choices(cell));
        "The architect's study of a new policy is ready for you to read".to_string()
    } else {
        let asked = asked_of(&gathered);
        let about = what_is_asked(&asked);
        body["kind"] = json!("ask");
        body["asked"] = Value::Array(asked);
        if let Some(m) = holds {
            body["holds"] = json!(m);
        }
        format!("The architect asks you{about}")
    };
    let input = json!({ "inputs": recorded(&gathered) });
    let commit = e.config.commit.clone();
    let (round, id) =
        e.db.write(Actor::Engine, |tx| {
            let round = work::start_round(tx, w.id, &cell.id, None, &input, &commit, None).map_err(db::Error::Abort)?;
            tx.execute(
                "INSERT INTO owner_asks (work, round, body) VALUES (?1, ?2, ?3)",
                params![round.work, round.n, body.to_string()],
            )?;
            let id = tx.last_insert_rowid();
            actions::notify_ask(tx, w.id, id, &format!("{notice} (ask {id})")).map_err(db::Error::Abort)?;
            // The matter the ask holds is handled at this round; the owner's answer comes back as a matter of its own.
            if let Some(m) = holds {
                tx.execute(
                    "UPDATE architect_matters SET held = 'owner_ask', held_by = ?2,
                            handled_work = coalesce(handled_work, ?3), handled_round = coalesce(handled_round, ?4)
                     WHERE id = ?1",
                    params![m, id, round.work, round.n],
                )?;
            }
            Ok((round, id))
        })
        .map_err(sql)?;
    if !eighth {
        return Ok(Advance::Waits(format!("ask {id} waits for the owner")));
    }
    let held = match holds {
        Some(m) => format!("matter {m} is held until the owner answers"),
        None => "the matter is held until the owner answers".to_string(),
    };
    let text = format!(
        "The ask is with the owner as ask {id}; {held}, when the answer comes back to your queue as a matter of its \
         own. Hand in this cell's answer for what you can decide now."
    );
    let output = json!({ "reply": { "text": text } });
    let row = g.take(&cell.id, &output, &given)?.clone();
    finish(e, w, g, cell, round, output, &row, Ended::default(), &[], None, &given)
}

// An engineer's question.

/// The matter an engineer's question was queued as, by the round that asked, with the architect's reply once it gave
/// one.
pub fn question(e: &Engine, round: RoundId) -> Result<Option<(i64, Option<Value>)>, String> {
    e.db.conn()
        .query_row(
            "SELECT id, reply FROM architect_matters WHERE work = ?1 AND round = ?2 ORDER BY id LIMIT 1",
            [round.work, round.n],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)),
        )
        .optional()
        .map(|o| o.map(|(id, reply)| (id, json_col(reply))))
        .map_err(sql)
}

/// The kind of matter a question to the architect is: by the kind of the ask's first point, when a cell asked; for an
/// engine tool that failed, tool_failed; for main's environment that changed, environment_changed.
fn question_kind(gathered: &[Gathered]) -> &'static str {
    fn path_of(p: &inputs::Part) -> String {
        match &p.origin {
            Origin::Place { path, .. } => path.clone(),
            _ => String::new(),
        }
    }
    let parts: Vec<&inputs::Part> = gathered.iter().flat_map(|x| &x.parts).collect();
    if let Some(ask) = parts.iter().find(|p| path_of(p) == "ask" && !empty(&p.value)) {
        let kind = ask.value["points"][0]["kind"].as_str().unwrap_or_default();
        return QUESTION_KINDS
            .iter()
            .find(|k| **k == kind)
            .copied()
            .unwrap_or("other_question");
    }
    if parts
        .iter()
        .any(|p| path_of(p).contains("environment_changed") && !empty(&p.value))
    {
        return "environment_changed";
    }
    if parts
        .iter()
        .any(|p| !matches!(path_of(p).as_str(), "" | "ask" | "progress") && !empty(&p.value))
    {
        return "tool_failed";
    }
    "other_question"
}

/// Cell 15: what is asked is queued for the architect as a matter, which wakes it if it sleeps; the work waits,
/// holding no slot, until the architect replies with engine_op, and the reply goes back to the cell that asked
/// unchanged.
pub fn ask_architect(
    e: &mut Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    item: &Item,
    open: Option<Round>,
) -> Result<Advance, String> {
    if let Some(r) = open.filter(|r| r.cell.as_deref() == Some(cell.id.as_str())) {
        let Some((matter, reply)) = question(e, r.id)? else {
            return Err(format!("{} asks the architect, but queued no matter", r.id));
        };
        let Some(reply) = reply else {
            return Ok(Advance::Waits(format!(
                "matter {matter} waits for the architect's reply"
            )));
        };
        let output = json!({ "reply": reply });
        let gathered = inputs::gather(e, w, g, cell, Some(item))?;
        let given = values::given(e, w, g, cell, Some(item), &gathered)?;
        let row = g.take(&cell.id, &output, &given)?.clone();
        return finish(e, w, g, cell, r.id, output, &row, Ended::default(), &[], None, &given);
    }
    let gathered = inputs::gather(e, w, g, cell, Some(item))?;
    let asked = asked_of(&gathered);
    let kind = question_kind(&gathered);
    let about = match what_is_asked(&asked) {
        a if a.is_empty() => format!(" ({})", kind.replace('_', " ")),
        a => a,
    };
    let what = format!("Work {} on item {} ({}) asks{about}", w.id, item.id, item.title);
    let body = json!({ "to": "architect", "work": w.id, "item": item.id, "title": item.title, "asked": asked });
    let input = json!({ "inputs": recorded(&gathered) });
    let commit = e.config.commit.clone();
    let matter =
        e.db.write(Actor::Engine, |tx| {
            let round = work::start_round(tx, w.id, &cell.id, None, &input, &commit, None).map_err(db::Error::Abort)?;
            let m = Matter {
                kind: kind.into(),
                what: what.clone(),
                body: body.clone(),
                item: Some(item.id),
                raised: Some((round.work, round.n)),
                ..Matter::default()
            };
            queue_matter(tx, &m)
        })
        .map_err(sql)?;
    (e.say)(&format!("work {} asks the architect: matter {matter}", w.id));
    Ok(Advance::Waits(format!(
        "matter {matter} waits for the architect's reply"
    )))
}

/// Where a matter came from and the reply given to it: the work and round that raised it, that round's cell, and the
/// reply.
type Raised = (Option<i64>, Option<i64>, Option<String>, Option<String>);

/// Whether a matter is an engineer's question whose work still waits for the reply: raised by a round that queued it
/// at the engineer's question cell, whose round is still open, with no reply given.
fn waits_for_reply(e: &Engine, matter: i64) -> Result<Option<(i64, bool)>, String> {
    let row: Option<Raised> =
        e.db.conn()
            .query_row(
                "SELECT m.work, m.round, r.cell, m.reply FROM architect_matters m
             LEFT JOIN rounds r ON r.work = m.work AND r.n = m.round WHERE m.id = ?1",
                [matter],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()
            .map_err(sql)?;
    let Some((Some(work), Some(n), Some(cell), reply)) = row else {
        return Ok(None);
    };
    let asks = e
        .flows
        .get(&e.flow_now)
        .and_then(|f| f.graph(ENGINEER))
        .and_then(|g| g.cell(&cell))
        .is_some_and(|c| c.step.as_deref() == Some("ask_architect"));
    if !asks {
        return Ok(None);
    }
    let open = count(
        e.db.conn(),
        "SELECT count(*) FROM rounds r JOIN works w ON w.id = r.work
         WHERE r.work = ?1 AND r.n = ?2 AND r.ended_at IS NULL AND w.ended IS NULL",
        &[&work, &n],
    )?;
    Ok(Some((work, open > 0 && reply.is_none())))
}

// The wrap-up.

/// Who waits for a matter, in words.
fn who_waits(e: &Engine, matter: Option<i64>) -> Result<String, String> {
    let Some(m) = matter else {
        return Ok("no one".into());
    };
    if let Some((work, true)) = waits_for_reply(e, m)? {
        return Ok(format!("the engineer of work {work}, stopped at its question"));
    }
    let kind: Option<String> =
        e.db.conn()
            .query_row("SELECT kind FROM architect_matters WHERE id = ?1", [m], |r| r.get(0))
            .optional()
            .map_err(sql)?;
    Ok(match kind.as_deref() {
        Some("owner_words" | "owner_approved") => "the owner".into(),
        _ => "no one".into(),
    })
}

/// Cell A15: every change the wake collected is put to the owner in one ask, numbered from 1, each with its reason, the
/// matter it belongs to and who waits; the matters held for them wait for the answer. With none collected it puts
/// nothing. The wake does not wait for the answer.
pub fn submit_changes(e: &mut Engine, w: &Work, cell: &str, round: RoundId) -> Result<Value, String> {
    let rows = query_json(
        e.db.conn(),
        "SELECT matter, changes FROM wake_changes WHERE work = ?1 ORDER BY round",
        &[&w.id],
        ROWS,
    )?;
    let mut changes = Vec::new();
    for r in &rows {
        let matter = r["matter"].clone();
        let waiting = who_waits(e, matter["queued"].as_i64())?;
        for c in items(r.get("changes")) {
            changes.push(json!({ "n": changes.len() + 1, "matter": matter, "change": c, "waiting": waiting }));
        }
    }
    if changes.is_empty() {
        return Ok(json!({}));
    }
    let n = changes.len();
    let body = json!({ "from": "architect", "work": w.id, "cell": cell, "kind": "changes", "changes": changes });
    let id = e
        .db
        .write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO owner_asks (work, round, body) VALUES (?1, ?2, ?3)",
                params![round.work, round.n, body.to_string()],
            )?;
            let id = tx.last_insert_rowid();
            tx.execute(
                "UPDATE architect_matters SET held_by = ?2 WHERE held = 'approval' AND held_by IS NULL AND handled_work = ?1",
                params![w.id, id],
            )?;
            let text = format!("The architect collected {n} changes to the database for your approval (ask {id})");
            actions::notify_ask(tx, w.id, id, &text).map_err(db::Error::Abort)?;
            Ok(id)
        })
        .map_err(sql)?;
    (e.say)(&format!("work {}: {n} changes put to the owner in ask {id}", w.id));
    Ok(json!({}))
}

/// A time written as the architect writes when to look next, `2026-10-27T14:00Z` in UTC, in milliseconds since the
/// epoch; none for a text that is not one.
pub fn parse_utc(text: &str) -> Option<i64> {
    let t = text.trim().trim_end_matches('Z').trim_end_matches('z');
    let (date, time) = t.split_once('T').unwrap_or((t, "00:00"));
    let mut d = date.split('-').map(|x| x.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let mut h = time.split(':').map(|x| x.parse::<i64>().ok());
    let (hour, minute) = (h.next()??, h.next().flatten().unwrap_or(0));
    if !(1..=12).contains(&m) || !(1..=31).contains(&day) || !(0..24).contains(&hour) || !(0..60).contains(&minute) {
        return None;
    }
    // Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 24 + hour) * 60 + minute) * 60_000)
}

/// The next looks the wrap-up set, as matters to queue when the wake ends: one at a time, due then (at once when the
/// time cannot be read), or once an item's work ends.
pub fn next_looks(gathered: &[Gathered], round: RoundId) -> Vec<Matter> {
    let looks = gathered.iter().find_map(|x| x.value.clone());
    items(looks.as_ref())
        .into_iter()
        .map(|l| {
            let when = &l["when"];
            let (due, due_item) = match when["kind"].as_str() {
                Some("item_done") => (None, when["item"].as_i64()),
                _ => (when["time"].as_str().and_then(parse_utc), None),
            };
            Matter {
                kind: "next_look".into(),
                what: format!(
                    "A look you set: {}",
                    db::one_sentence(l["what"].as_str().unwrap_or_default())
                ),
                body: json!({ "what": l["what"], "when": when }),
                item: due_item,
                raised: Some((round.work, round.n)),
                due,
                due_item,
                ..Matter::default()
            }
        })
        .collect()
}

/// Queues the next looks the wake set, in the transaction that ends it.
pub fn queue_looks(tx: &Transaction<'_>, looks: &[Matter]) -> Result<(), String> {
    for m in looks {
        queue_matter(tx, m).map_err(sql)?;
    }
    Ok(())
}

/// Cell P8: how many new sessions in a row read this version of the policy's text and were judged to understand it;
/// two pass it, one asks for another.
pub fn count_trials(e: &mut Engine, w: &Work) -> Result<Value, String> {
    let rows = query_json(
        e.db.conn(),
        "SELECT readings FROM policy_trials WHERE work = ?1",
        &[&w.id],
        1,
    )?;
    let readings = rows.first().map(|r| items(r.get("readings"))).unwrap_or_default();
    let passed = readings
        .iter()
        .rev()
        .take_while(|r| r["verdict"]["verdict"] == "understood")
        .count();
    let verdict = if passed >= 2 { "two_passed" } else { "one_more" };
    Ok(json!({ "count": { "verdict": verdict } }))
}

// The architect's actions.

/// Carries out an action of the architect's graph; false for an action of another.
pub fn act(ctx: &Ctx, tx: &Transaction<'_>, name: &str, at: Option<&Value>) -> Result<bool, String> {
    let w = ctx.work.id;
    let raised = Some((ctx.round.work, ctx.round.n));
    let hold = |reason: &str| -> Result<(), String> {
        tx.execute(
            "INSERT INTO dispatch_holds (reason, since) VALUES (?1, ?2) ON CONFLICT (reason) DO NOTHING",
            params![reason, db::now()],
        )
        .map_err(sql)?;
        Ok(())
    };
    let release = |reason: &str| -> Result<(), String> {
        tx.execute("DELETE FROM dispatch_holds WHERE reason = ?1", [reason])
            .map_err(sql)?;
        Ok(())
    };
    match name {
        "open_asks" => {
            for x in items(at) {
                let question = x["choice"]["question"].as_str().unwrap_or_default().to_string();
                let body = json!({
                    "from": "architect", "work": w, "cell": ctx.cell.id, "kind": "open",
                    "choice": x["choice"], "affects": x["affects"],
                });
                tx.execute(
                    "INSERT INTO owner_asks (work, round, body) VALUES (?1, ?2, ?3)",
                    params![ctx.round.work, ctx.round.n, body.to_string()],
                )
                .map_err(sql)?;
                let id = tx.last_insert_rowid();
                let text = format!(
                    "The architect asks you, without waiting (ask {id}): {}",
                    db::one_sentence(&question)
                );
                actions::notify_ask(tx, w, id, &text)?;
            }
        }
        "queue_fixes" => {
            for fix in items(at) {
                let what = format!(
                    "To change in what runs, {}: {}",
                    db::one_sentence(fix["where"].as_str().unwrap_or_default()),
                    db::one_sentence(fix["fix"].as_str().unwrap_or_default())
                );
                let m = Matter {
                    kind: "running_unfit".into(),
                    what,
                    body: fix.clone(),
                    raised,
                    ..Matter::default()
                };
                queue_matter(tx, &m).map_err(sql)?;
            }
        }
        "hold_for_tidying" => hold("tidying")?,
        "release_tidy_hold" => release("tidying")?,
        "hold_for_policy" => hold("policy")?,
        "release_policy_hold" => release("policy")?,
        "hold_changes" => {
            // Changes collected for a matter of the queue hold it until the owner approves; a matter an ask of the
            // owner already holds stays held by that ask.
            if items(at).is_empty() {
                return Ok(true);
            }
            if let Some(m) = picked_matter(tx, w, &ctx.within)? {
                tx.execute(
                    "UPDATE architect_matters SET held = 'approval', handled_work = coalesce(handled_work, ?2),
                            handled_round = coalesce(handled_round, ?3)
                     WHERE id = ?1 AND held IS NULL",
                    params![m, ctx.round.work, ctx.round.n],
                )
                .map_err(sql)?;
            }
        }
        "close_matter" => {
            if let Some(m) = picked_matter(tx, w, &ctx.within)? {
                tx.execute(
                    "UPDATE architect_matters SET handled_work = ?2, handled_round = ?3
                     WHERE id = ?1 AND handled_round IS NULL AND held IS NULL",
                    params![m, ctx.round.work, ctx.round.n],
                )
                .map_err(sql)?;
            }
        }
        "new_architect_session" => {
            tx.execute(
                "UPDATE sessions SET ended = ?2 WHERE work = ?1 AND line = 'architect' AND ended IS NULL",
                params![w, db::now()],
            )
            .map_err(sql)?;
        }
        "queue_policy_changed" => {
            let m = Matter {
                kind: "policy_changed".into(),
                what: "A new version of the policy took effect; the old one is no longer used".into(),
                body: json!({}),
                raised,
                ..Matter::default()
            };
            queue_matter(tx, &m).map_err(sql)?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

// The architect's tool engine_op.

/// The work running on an item: not stopped, and not at the end of its graph.
fn running_work(e: &mut Engine, item: i64) -> Result<Option<Work>, String> {
    let id: Option<i64> =
        e.db.conn()
            .query_row(
                "SELECT max(id) FROM works WHERE item = ?1 AND ended IS NULL",
                [item],
                |r| r.get(0),
            )
            .map_err(sql)?;
    let Some(id) = id else { return Ok(None) };
    let w = e.work(id)?;
    Ok((!e.done(&w)?).then_some(w))
}

/// Has the engine do what the architect asks at once, without the owner's approval; what it did or why it did not.
pub fn engine_op(e: &mut Engine, w: &Work, req: &Value) -> Value {
    match op(e, w, req) {
        Ok(()) => json!({ "done": { "verdict": "done" } }),
        Err(why) => json!({ "done": { "verdict": "refused", "why": why } }),
    }
}

fn op(e: &mut Engine, w: &Work, req: &Value) -> Result<(), String> {
    let item = req["item"].as_i64();
    let need_item = || item.ok_or_else(|| "this operation needs the item's number".to_string());
    let say = req["say"].as_str().map(str::trim).filter(|s| !s.is_empty());
    match req["op"].as_str().unwrap_or_default() {
        "pause_work" => {
            let i = need_item()?;
            let work = running_work(e, i)?.ok_or_else(|| format!("item {i} has no work running"))?;
            if work.paused.is_some() {
                return Err(format!("work {} on item {i} is paused already", work.id));
            }
            let why = match say {
                Some(s) => format!("{PAUSED_BY_ARCHITECT}: {s}"),
                None => format!("{PAUSED_BY_ARCHITECT} in its wake, work {}", w.id),
            };
            let text = format!("Work {} on item {i} is paused: {why}", work.id);
            e.db.write(Actor::Architect, |tx| {
                tx.execute(
                    "UPDATE works SET paused = ?2, paused_by = 'engine' WHERE id = ?1",
                    params![work.id, why],
                )?;
                actions::notify(tx, Some(work.id), &text).map_err(db::Error::Abort)
            })
            .map_err(sql)
        }
        "resume_work" => {
            let i = need_item()?;
            let work = running_work(e, i)?.ok_or_else(|| format!("item {i} has no work running"))?;
            let by_architect = work.paused_by.as_deref() == Some("engine")
                && work
                    .paused
                    .as_deref()
                    .is_some_and(|p| p.starts_with(PAUSED_BY_ARCHITECT));
            if !by_architect {
                return Err(format!("work {} on item {i} is not paused by the architect", work.id));
            }
            e.db.write(Actor::Architect, |tx| {
                tx.execute("UPDATE works SET paused = NULL, paused_by = NULL WHERE id = ?1", [work.id])?;
                Ok(())
            })
            .map_err(sql)
        }
        "hold_dispatch" => e
            .db
            .write(Actor::Architect, |tx| {
                tx.execute(
                    "INSERT INTO dispatch_holds (reason, since) VALUES ('architect', ?1) ON CONFLICT (reason) DO NOTHING",
                    [db::now()],
                )?;
                Ok(())
            })
            .map_err(sql),
        "release_dispatch" => {
            let n = e
                .db
                .write(Actor::Architect, |tx| {
                    Ok(tx.execute("DELETE FROM dispatch_holds WHERE reason = 'architect'", [])?)
                })
                .map_err(sql)?;
            if n == 0 {
                return Err("new works were not held by the architect".into());
            }
            Ok(())
        }
        "reply" => {
            let i = need_item()?;
            let reply = req
                .get("reply")
                .filter(|r| r.is_object())
                .ok_or("reply needs the reply")?
                .clone();
            let work = running_work(e, i)?.ok_or_else(|| format!("item {i} has no work running"))?;
            let g = e.graph_of(&work)?;
            let asking = g
                .cell(&work.pos.at)
                .is_some_and(|c| c.step.as_deref() == Some("ask_architect"));
            let open = work::open_round(e.db.conn(), work.id)?.filter(|r| asking && r.cell.as_deref() == Some(work.pos.at.as_str()));
            let Some(r) = open else {
                return Err(format!("work {} on item {i} does not wait for your reply", work.id));
            };
            let Some((matter, None)) = question(e, r.id)? else {
                return Err(format!("work {} on item {i} does not wait for your reply", work.id));
            };
            e.db.write(Actor::Architect, |tx| {
                tx.execute(
                    "UPDATE architect_matters SET reply = ?2 WHERE id = ?1",
                    params![matter, reply.to_string()],
                )?;
                Ok(())
            })
            .map_err(sql)
        }
        "interject" => {
            let i = need_item()?;
            let text = say.ok_or("interject needs what to say")?.to_string();
            let work = running_work(e, i)?.ok_or_else(|| format!("item {i} has no work running"))?;
            e.db.write(Actor::Architect, |tx| {
                tx.execute(
                    "INSERT INTO interjections (work, line, speaker, text) VALUES (?1, 'engineer', 'architect', ?2)",
                    params![work.id, text],
                )?;
                Ok(())
            })
            .map_err(sql)
        }
        "rule_design_card" => {
            let card = req["design_card"]
                .as_i64()
                .ok_or("rule_design_card needs the design card's number")?;
            let verdicts: Vec<db::Verdict> = items(req.get("verdicts"))
                .iter()
                .map(|v| db::Verdict {
                    decision: v["decision"].as_i64().unwrap_or_default(),
                    verdict: v["verdict"].as_str().unwrap_or_default().to_string(),
                    option: v["approach"].as_str().map(str::to_string),
                    text: v["why"].as_str().map(str::to_string),
                })
                .collect();
            e.db.write(Actor::Architect, |tx| {
                db::rule_design_card(tx, card, ARCHITECT, &verdicts).map_err(db::Error::Abort)
            })
            .map(|_| ())
            .map_err(sql)
        }
        other => Err(format!("no such operation: {other}")),
    }
}

// The architect's checks.

/// The articles of each project's constitution the architect is given, in the order of the projects' names and of
/// each constitution: its level-two headings that start with "Article".
fn articles(e: &Engine) -> Vec<String> {
    let mut out = Vec::new();
    for p in e.config.config.projects.values() {
        let Some(path) = &p.constitution else { continue };
        let Ok(text) = std::fs::read_to_string(p.checkout.join(path)) else {
            continue;
        };
        for line in text.lines() {
            if let Some(h) = line.strip_prefix("## ")
                && h.trim_start().to_lowercase().starts_with("article")
            {
                out.push(h.trim().to_string());
            }
        }
    }
    out
}

/// Whether a row names an article: by its number, as "Article 2", or by its title.
fn names_article(principle: &str, article: &str) -> bool {
    let p = principle.to_lowercase();
    let a = article.to_lowercase();
    let (number, title) = a.split_once('.').unwrap_or((a.as_str(), ""));
    let number = number.trim();
    let by_number = p
        .match_indices(number)
        .any(|(i, _)| !p[i + number.len()..].chars().next().is_some_and(|c| c.is_ascii_digit()));
    let title = title.trim();
    by_number || (!title.is_empty() && p.contains(title))
}

/// The changes the owner approved: those of every approval, by the numbers approved.
fn approved_changes(conn: &Connection) -> Result<Vec<(i64, Value)>, String> {
    let rows = query_json(
        conn,
        "SELECT id, body FROM architect_matters WHERE kind = 'owner_approved' ORDER BY id",
        &[],
        10_000,
    )?;
    let mut out = Vec::new();
    for r in rows {
        let approved: Vec<i64> = items(r["body"].get("approved"))
            .iter()
            .filter_map(Value::as_i64)
            .collect();
        for c in items(r["body"].get("changes")) {
            if c["n"].as_i64().is_some_and(|n| approved.contains(&n)) {
                out.push((r["id"].as_i64().unwrap_or_default(), c["change"].clone()));
            }
        }
    }
    Ok(out)
}

/// The changes db_write wrote, each as it was handed in.
pub fn written_changes(conn: &Connection) -> Result<Vec<Value>, String> {
    let rows = query_json(
        conn,
        "SELECT args FROM tool_runs WHERE tool = 'db_write' AND json_extract(result, '$.written.verdict') = 'written'
         ORDER BY id",
        &[],
        10_000,
    )?;
    Ok(rows.iter().flat_map(|r| items(r["args"].get("changes"))).collect())
}

/// The changes the owner approved that are not written yet, oldest first: each change is written once, so one written
/// takes away one approved that is the same, word for word.
pub fn approved_unwritten(conn: &Connection) -> Result<Vec<Value>, String> {
    let mut written = written_changes(conn)?;
    let mut out = Vec::new();
    for (_, c) in approved_changes(conn)? {
        match written.iter().position(|x| *x == c) {
            Some(i) => {
                written.remove(i);
            }
            None => out.push(c),
        }
    }
    Ok(out)
}

/// The texts of the owner's words in a wake's asks and in the owner's matters: what a policy may not quote.
fn owner_words(conn: &Connection, work: i64) -> Result<Vec<String>, String> {
    let rows = query_json(
        conn,
        "SELECT json_extract(answer, '$.text') AS text FROM owner_asks WHERE work = ?1 AND answer IS NOT NULL
         UNION ALL SELECT json_extract(body, '$.text') FROM architect_matters WHERE kind = 'owner_words'
         UNION ALL SELECT json_extract(body, '$.answer') FROM architect_matters WHERE kind = 'owner_words'",
        &[&work],
        10_000,
    )?;
    Ok(rows
        .iter()
        .filter_map(|r| r["text"].as_str().map(str::to_string))
        .collect())
}

/// The sentences of a text, each trimmed.
fn sentences(text: &str) -> Vec<String> {
    text.split(['.', '!', '?', '\n', '\u{3002}'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Whether a text names a date, as `2026-10-27`.
fn names_date(text: &str) -> bool {
    let b = text.as_bytes();
    (0..b.len().saturating_sub(9)).any(|i| {
        let d = |k: usize| b[i + k].is_ascii_digit();
        d(0) && d(1) && d(2) && d(3) && b[i + 4] == b'-' && d(5) && d(6) && b[i + 7] == b'-' && d(8) && d(9)
    })
}

/// The cells of the flow a text names: a cell of the architect's graph by its id, as `A6`, or any cell as `cell 15`.
fn names_cell(e: &Engine, text: &str) -> Option<String> {
    let f = e.flows.get(&e.flow_now)?;
    let words: Vec<&str> = text
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    for g in &f.graphs {
        for c in &g.cells {
            let own = g.name == ARCHITECT && words.contains(&c.id.as_str());
            let named = words
                .windows(2)
                .any(|w| w[0].eq_ignore_ascii_case("cell") && w[1] == c.id);
            if own || named {
                return Some(c.id.clone());
            }
        }
    }
    None
}

/// The point numbers of a policy.
fn point_numbers(policy: &Value) -> Vec<i64> {
    items(policy.get("points"))
        .iter()
        .filter_map(|p| p["no"].as_i64())
        .collect()
}

/// The place beside another in a return: `policy` beside `probes`, at the same depth.
fn beside(at: Option<&str>, from: &str, to: &str) -> String {
    match at.and_then(|a| a.rsplit_once('.')) {
        Some((head, _)) => format!("{head}.{to}"),
        None => at.unwrap_or(from).replace(from, to),
    }
}

/// Runs a check of the architect's graph.
pub fn check(c: &checks::Ctx<'_>, name: &str, at: Option<&str>, v: Option<&Value>) -> Result<(), String> {
    let conn = c.e.db.conn();
    match name {
        "constitution_covered" => {
            let articles = articles(c.e);
            if articles.is_empty() {
                return Ok(());
            }
            let rows = items(v);
            if rows.len() != articles.len() {
                return Err(format!(
                    "there is one row for each article of the constitutions given, {} of them, and there are {}",
                    articles.len(),
                    rows.len()
                ));
            }
            for (i, (row, article)) in rows.iter().zip(&articles).enumerate() {
                let principle = row["principle"].as_str().unwrap_or_default();
                if !names_article(principle, article) {
                    return Err(format!(
                        "row {} is for \"{article}\", in the constitution's order, and names \"{principle}\"",
                        i + 1
                    ));
                }
            }
            Ok(())
        }
        "chosen_first" => {
            let Some(chosen) = v else { return Ok(()) };
            let first = at_path(&c.unwrapped, "pick.ranked")
                .and_then(|r| items(Some(&r)).first().cloned())
                .and_then(|r| r["matter"].as_str().map(str::to_string))
                .unwrap_or_default();
            let matter = chosen["matter"].as_str().unwrap_or_default();
            if matter.trim() != first.trim() {
                return Err(format!(
                    "the matter chosen, \"{matter}\", is not the first of the ranked list, \"{first}\""
                ));
            }
            if let Some(q) = chosen["queued"].as_i64()
                && count(conn, "SELECT count(*) FROM architect_queue WHERE matter = ?1", &[&q])? == 0
            {
                return Err(format!("matter {q} is not in your queue, or is handled or held"));
            }
            Ok(())
        }
        "changes_valid" => crate::arch_changes::validate(c.e, &items(v)),
        "question_answered" => {
            let Some(m) = picked_matter(conn, c.w.id, &c.g.within)? else {
                return Ok(());
            };
            let waiting = waits_for_reply(c.e, m)?;
            let replied: Option<String> = conn
                .query_row("SELECT reply FROM architect_matters WHERE id = ?1", [m], |r| r.get(0))
                .map_err(sql)?;
            let Some((work, _)) = waiting else { return Ok(()) };
            let changes = items(v);
            match (changes.is_empty(), replied.is_some()) {
                (true, false) => Err(format!(
                    "matter {m} is the question of work {work}, and no change waits for approval: reply to it with \
                     engine_op reply before handing in"
                )),
                (false, true) => Err(format!(
                    "changes wait for the owner's approval, and matter {m} was replied to: the reply comes once they \
                     are approved and written"
                )),
                _ => Ok(()),
            }
        }
        "approved_written" => {
            let Some(m) = picked_matter(conn, c.w.id, &c.g.within)? else {
                return Ok(());
            };
            let body: Option<(String, String)> = conn
                .query_row("SELECT kind, body FROM architect_matters WHERE id = ?1", [m], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .optional()
                .map_err(sql)?;
            let Some(("owner_approved", body)) = body.as_ref().map(|(k, b)| (k.as_str(), b)) else {
                return Ok(());
            };
            let body: Value = serde_json::from_str(body).unwrap_or(Value::Null);
            let approved: Vec<i64> = items(body.get("approved")).iter().filter_map(Value::as_i64).collect();
            let mut written = written_changes(conn)?;
            let mut waiting: BTreeSet<i64> = BTreeSet::new();
            for ch in items(body.get("changes")) {
                let n = ch["n"].as_i64().unwrap_or_default();
                if !approved.contains(&n) {
                    continue;
                }
                match written.iter().position(|x| *x == ch["change"]) {
                    Some(i) => {
                        written.remove(i);
                    }
                    None => return Err(format!("approved change {n} is not written: write it with db_write")),
                }
                waiting.extend(ch["matter"]["queued"].as_i64());
            }
            for q in waiting {
                if let Some((work, true)) = waits_for_reply(c.e, q)? {
                    return Err(format!(
                        "the engineer of work {work} waits for the changes of matter {q}: reply to it with engine_op \
                         reply"
                    ));
                }
            }
            Ok(())
        }
        "policy_text_clean" => {
            let Some(policy) = v else { return Ok(()) };
            let text = policy.to_string();
            for p in items(at_path(&c.unwrapped, &beside(at, "policy", "probes")).as_ref()) {
                let ask = p["ask"].as_str().unwrap_or_default();
                if ask.len() >= 20 && text.contains(ask) {
                    return Err(format!("the policy holds the probe {}", p["id"]));
                }
            }
            for said in owner_words(conn, c.w.id)? {
                if let Some(s) = sentences(&said)
                    .into_iter()
                    .find(|s| s.chars().count() >= 30 && text.contains(s.as_str()))
                {
                    return Err(format!("the policy quotes the owner's words: \"{s}\""));
                }
            }
            if let Some(cell) = names_cell(c.e, &text) {
                return Err(format!("the policy names the cell {cell} of the flow"));
            }
            if names_date(&text) {
                return Err("the policy names a date".into());
            }
            Ok(())
        }
        "probes_cover_points" => {
            let probes = items(v);
            let policy = at_path(&c.unwrapped, &beside(at, "probes", "policy")).unwrap_or(Value::Null);
            let numbers = point_numbers(&policy);
            for p in &probes {
                for n in items(p.get("points")).iter().filter_map(Value::as_i64) {
                    if !numbers.contains(&n) {
                        return Err(format!(
                            "the probe {} names point {n}, which the text does not have",
                            p["id"]
                        ));
                    }
                }
            }
            for n in numbers {
                let probed = probes
                    .iter()
                    .any(|p| items(p.get("points")).iter().any(|x| x.as_i64() == Some(n)));
                if !probed {
                    return Err(format!("point {n} has no probe"));
                }
            }
            Ok(())
        }
        "failed_probes_gone" => {
            let gaps = query_json(
                conn,
                "SELECT output FROM rounds WHERE work = ?1 AND cell = 'P6' AND transition = 'gaps'",
                &[&c.w.id],
                ROWS,
            )?;
            let said: Vec<String> = gaps
                .iter()
                .flat_map(|r| items(at_path(&c.g.unwrap(&r["output"]), "verdict.gaps").as_ref()))
                .map(|g| {
                    format!(
                        "{} {}",
                        g["what"].as_str().unwrap_or_default(),
                        g["seen"].as_str().unwrap_or_default()
                    )
                })
                .collect();
            for p in items(v) {
                let id = p["id"].as_str().unwrap_or_default();
                if !id.trim().is_empty() && said.iter().any(|s| s.contains(id)) {
                    return Err(format!("the probe {id} was answered wrongly, and is not used again"));
                }
            }
            Ok(())
        }
        "reading_complete" => {
            let reading = v.cloned().unwrap_or(Value::Null);
            let policy = query_json(conn, "SELECT policy FROM policy_text WHERE work = ?1", &[&c.w.id], 1)?
                .first()
                .map(|r| r["policy"].clone())
                .unwrap_or(Value::Null);
            let probes = query_json(
                conn,
                "SELECT probes FROM policy_probes_asked WHERE work = ?1",
                &[&c.w.id],
                1,
            )?
            .first()
            .map(|r| items(r.get("probes")))
            .unwrap_or_default();
            let restated: Vec<i64> = items(reading.get("restate"))
                .iter()
                .filter_map(|r| r["no"].as_i64())
                .collect();
            if let Some(n) = point_numbers(&policy).into_iter().find(|n| !restated.contains(n)) {
                return Err(format!("point {n} is not restated"));
            }
            let answered: Vec<String> = items(reading.get("answers"))
                .iter()
                .filter_map(|a| a["id"].as_str().map(str::to_string))
                .collect();
            if let Some(p) = probes
                .iter()
                .filter_map(|p| p["id"].as_str())
                .find(|id| !answered.iter().any(|a| a == id))
            {
                return Err(format!("the probe {p} is not answered"));
            }
            Ok(())
        }
        other => Err(format!("no such engine check: {other}")),
    }
}

// The architect's inputs.

/// What the engine computes for an input of the architect's cells; none when there is nothing.
pub fn input(e: &Engine, w: &Work, g: &CompiledGraph, name: &str) -> Result<Option<Value>, String> {
    match name {
        "since_last_wake" => since_last_wake(e, w, g),
        "projects_guides" => Ok(Some(projects_guides(e, w, g)?)),
        "constitutions" => Ok(constitutions(e)),
        "policy_signals" => policy_signals(e),
        "running_and_next" => running_and_next(e).map(Some),
        "slots" => slots(e).map(Some),
        "matter_material" => matter_material(e, w, g),
        "whole_view" => whole_view(e).map(Some),
        "similar_items" => similar_items(e, w, g).map(Some),
        "changes_touch" => changes_touch(e, w, g),
        other => Err(format!("no such engine input: {other}")),
    }
}

/// The engineer's graph of the flow new works start on.
fn engineer_graph(e: &Engine) -> Option<&CompiledGraph> {
    e.flows.get(&e.flow_now).and_then(|f| f.graph(ENGINEER))
}

/// What an engineer's work waits for, in words; none when it waits for nothing.
fn work_waits(e: &Engine, work: i64, at: &str, paused: Option<&str>) -> Result<Option<String>, String> {
    if let Some(p) = paused {
        return Ok(Some(format!("paused: {p}")));
    }
    let ask: Option<i64> =
        e.db.conn()
            .query_row(
                "SELECT max(id) FROM owner_asks WHERE work = ?1 AND answer IS NULL",
                [work],
                |r| r.get(0),
            )
            .map_err(sql)?;
    if let Some(a) = ask {
        return Ok(Some(format!("the owner's answer to ask {a}")));
    }
    let asks_architect = engineer_graph(e)
        .and_then(|g| g.cell(at))
        .is_some_and(|c| c.step.as_deref() == Some("ask_architect"));
    if asks_architect {
        return Ok(Some("the architect's reply to its question".into()));
    }
    Ok(None)
}

/// The items being worked on, at which stage each is, what each waits for, and who waits for whom.
fn whole_view(e: &Engine) -> Result<Value, String> {
    let end = engineer_graph(e).map(|g| g.end.clone()).unwrap_or_default();
    let rows = query_json(
        e.db.conn(),
        "SELECT i.id AS item, i.project, i.title, i.segment, i.held, w.id AS work, w.at, w.paused,
                (SELECT json_group_array(l.depends_on) FROM item_links l WHERE l.item = i.id) AS waits_for_items,
                (SELECT json_group_array(l.item) FROM item_links l WHERE l.depends_on = i.id) AS waited_by
         FROM items i LEFT JOIN works w ON w.id = (SELECT max(x.id) FROM works x WHERE x.item = i.id)
         WHERE i.cancelled IS NULL AND (w.id IS NULL OR w.ended IS NULL)
         ORDER BY i.id",
        &[],
        10_000,
    )?;
    let mut out = Vec::new();
    for mut r in rows {
        let at = r["at"].as_str().unwrap_or_default().to_string();
        if !at.is_empty() && at == end {
            continue;
        }
        if let Some(work) = r["work"].as_i64() {
            let stage = engineer_graph(e)
                .and_then(|g| g.cell(&at))
                .and_then(|c| c.stage.clone());
            r["stage"] = json!(stage);
            r["waits"] = json!(work_waits(e, work, &at, r["paused"].as_str())?);
        }
        out.push(r);
        if out.len() >= ROWS {
            break;
        }
    }
    Ok(Value::Array(out))
}

/// What changed since the architect's last wake; for a new session, the whole view instead.
fn since_last_wake(e: &Engine, w: &Work, _g: &CompiledGraph) -> Result<Option<Value>, String> {
    if session(e, w)? == "new" {
        return Ok(Some(json!({ "whole_view": whole_view(e)? })));
    }
    let conn = e.db.conn();
    let since: i64 = conn
        .query_row(
            "SELECT coalesce(min(r.started_at), 0) FROM rounds r
             WHERE r.work = (SELECT max(x.id) FROM works x WHERE x.graph = 'architect' AND x.id < ?1)",
            [w.id],
            |r| r.get(0),
        )
        .map_err(sql)?;
    let items_changed = query_json(
        conn,
        "SELECT i.id AS item, i.project, i.title, i.segment, i.held, i.cancelled FROM items i
         WHERE i.id IN (SELECT json_extract(c.row_key, '$.id') FROM changes c WHERE c.table_name = 'items' AND c.at > ?1)
         ORDER BY i.id",
        &[&since],
        ROWS,
    )?;
    let works = query_json(
        conn,
        "SELECT w.id AS work, w.item, i.title, w.at, w.paused, w.ended FROM works w JOIN items i ON i.id = w.item
         WHERE w.graph = 'engineer' AND EXISTS (SELECT 1 FROM rounds r WHERE r.work = w.id AND r.started_at > ?1)
         ORDER BY w.id",
        &[&since],
        ROWS,
    )?;
    let merged = query_json(
        conn,
        "SELECT w.item, w.pr, r.ended_at AS at FROM rounds r JOIN works w ON w.id = r.work
         WHERE r.cell = '9' AND r.transition = 'merged' AND r.ended_at > ?1 ORDER BY r.ended_at",
        &[&since],
        ROWS,
    )?;
    let answered = query_json(
        conn,
        "SELECT a.id AS ask, a.work, json_extract(a.answer, '$.text') AS answer FROM owner_asks a
         WHERE json_extract(a.answer, '$.by') = 'owner' AND json_extract(a.answer, '$.at') > ?1 ORDER BY a.id",
        &[&since],
        ROWS,
    )?;
    let said = query_json(
        conn,
        "SELECT id AS matter, kind, what FROM architect_matters
         WHERE kind IN ('owner_words', 'owner_approved') AND at > ?1 ORDER BY id",
        &[&since],
        ROWS,
    )?;
    Ok(Some(json!({
        "since": since,
        "items_changed": items_changed,
        "works": works,
        "merged": merged,
        "owner_answered": answered,
        "owner_said": said,
    })))
}

/// The text of the matter at hand, as the projects' guides cite nodes by it and similar items are looked up by.
fn matter_text(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<String, String> {
    let mut text = String::new();
    if let Some(chosen) = picked(e.db.conn(), w.id, &g.within)? {
        text.push_str(&chosen.to_string());
        if let Some(m) = chosen["queued"].as_i64() {
            let row = query_json(
                e.db.conn(),
                "SELECT what, body FROM architect_matters WHERE id = ?1",
                &[&m],
                1,
            )?;
            if let Some(r) = row.first() {
                text.push_str(&r.to_string());
            }
        }
    }
    if let Some(changes) = last_return(e, w, g, "A8", "changes")? {
        text.push_str(&changes.to_string());
    }
    Ok(text)
}

/// Each project's guide: its AGENTS.md, its design goal, and the index of its design tree, with the nodes the matter
/// at hand names in full.
fn projects_guides(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<Value, String> {
    let cites = matter_text(e, w, g)?;
    let mut out = Map::new();
    for (name, p) in &e.config.config.projects {
        out.insert(
            name.clone(),
            json!({
                "design_goal": p.design_goal,
                "guide": config::project_guide(&p.checkout, &p.design_paths, &cites),
            }),
        );
    }
    Ok(Value::Object(out))
}

/// Each project's constitution, as its configuration names it; none when no project names one.
fn constitutions(e: &Engine) -> Option<Value> {
    let mut out = Map::new();
    for (name, p) in &e.config.config.projects {
        let Some(path) = &p.constitution else { continue };
        if let Ok(text) = std::fs::read_to_string(p.checkout.join(path)) {
            out.insert(name.clone(), Value::String(text));
        }
    }
    (!out.is_empty()).then_some(Value::Object(out))
}

/// The signals the policy in force says to report on; the engine measures none of them in this version, so each says
/// so. None before the first policy takes effect.
fn policy_signals(e: &Engine) -> Result<Option<Value>, String> {
    let rows = query_json(e.db.conn(), "SELECT policy FROM policy_current", &[], 1)?;
    let Some(policy) = rows.first().map(|r| r["policy"].clone()).filter(|p| !p.is_null()) else {
        return Ok(None);
    };
    let signals: Vec<Value> = items(policy.get("signals"))
        .into_iter()
        .map(|s| json!({ "what": s["what"], "how": s["how"], "measured": "the engine measures no signal in this version" }))
        .collect();
    Ok(Some(Value::Array(signals)))
}

/// What the engine checks of a segment: whether it is deferred, and whether the segments before it are done.
fn segment_state(e: &Engine, segment: Option<&str>) -> Result<Value, String> {
    let Some(key) = segment else {
        return Ok(json!({ "segment": null }));
    };
    let end = engineer_graph(e).map(|g| g.end.clone()).unwrap_or_default();
    let deferred: Option<Option<String>> =
        e.db.conn()
            .query_row("SELECT deferred FROM plan_segments WHERE key = ?1", [key], |r| r.get(0))
            .optional()
            .map_err(sql)?;
    let unfinished = count(
        e.db.conn(),
        "SELECT count(*) FROM json_each((SELECT after FROM plan_segments WHERE key = ?1)) a
         JOIN items i ON i.segment = a.value
         WHERE i.cancelled IS NULL AND NOT EXISTS (
           SELECT 1 FROM works x WHERE x.item = i.id AND x.graph = 'engineer' AND x.ended IS NULL AND x.at = ?2)",
        &[&key, &end],
    )?;
    Ok(json!({
        "segment": key,
        "deferred": deferred.flatten(),
        "earlier_segments_done": unfinished == 0,
    }))
}

/// The works running, and the items the engine would start next, as many as there are slots, each with its segment,
/// whether that is deferred, whether the segments before it are done, what it waits for and for how long.
fn running_and_next(e: &Engine) -> Result<Value, String> {
    let end = engineer_graph(e).map(|g| g.end.clone()).unwrap_or_default();
    let now = db::now();
    let works = query_json(
        e.db.conn(),
        "SELECT w.id AS work, w.item, i.title, i.segment, w.at, w.paused,
                (SELECT max(r.ended_at) FROM rounds r WHERE r.work = w.id) AS last_round_ended
         FROM works w JOIN items i ON i.id = w.item WHERE w.graph = 'engineer' AND w.ended IS NULL ORDER BY w.id",
        &[],
        10_000,
    )?;
    let mut running = Vec::new();
    for mut r in works {
        let at = r["at"].as_str().unwrap_or_default().to_string();
        if at == end {
            continue;
        }
        let waits = work_waits(e, r["work"].as_i64().unwrap_or_default(), &at, r["paused"].as_str())?;
        let waited = waits.as_ref().and(r["last_round_ended"].as_i64()).map(|t| now - t);
        r["segment_state"] = segment_state(e, r["segment"].as_str())?;
        r["waits"] = json!(waits);
        r["waited_ms"] = json!(waited);
        running.push(r);
    }
    let slots = total_slots(e);
    let mut next = query_json(
        e.db.conn(),
        "SELECT item, project, title, segment FROM ready_items LIMIT ?1",
        &[&slots],
        ROWS,
    )?;
    for r in &mut next {
        r["segment_state"] = segment_state(e, r["segment"].as_str())?;
    }
    let holds = query_json(e.db.conn(), "SELECT reason, since FROM dispatch_holds", &[], 10)?;
    Ok(json!({ "running": running, "next": next, "dispatch_holds": holds }))
}

/// How many works hold a slot: the engineer's works going on by themselves that wait for nothing.
fn holding(e: &Engine) -> Result<i64, String> {
    let Some(g) = engineer_graph(e) else { return Ok(0) };
    let works = query_json(
        e.db.conn(),
        "SELECT w.id, w.at, (SELECT r.transition FROM rounds r WHERE r.work = w.id ORDER BY r.n DESC LIMIT 1) AS last,
                (SELECT r.cell FROM rounds r WHERE r.work = w.id ORDER BY r.n DESC LIMIT 1) AS last_cell
         FROM works w WHERE w.graph = 'engineer' AND w.ended IS NULL AND w.paused IS NULL",
        &[],
        10_000,
    )?;
    let mut n = 0;
    for w in works {
        let at = w["at"].as_str().unwrap_or_default();
        let Some(cell) = g.cell(at) else { continue };
        if at == g.end || matches!(cell.step.as_deref(), Some("ask_owner" | "ask_architect")) {
            continue;
        }
        let waits = w["last_cell"] == at
            && w["last"]
                .as_str()
                .is_some_and(|t| cell.rows.iter().any(|r| r.name == t && r.waits));
        if !waits {
            n += 1;
        }
    }
    Ok(n)
}

/// The machines' slots: how many there are, how many works hold one, how many are free, and how many items are ready.
fn slots(e: &Engine) -> Result<Value, String> {
    let total = total_slots(e);
    let held = holding(e)?;
    let ready = count(e.db.conn(), "SELECT count(*) FROM ready_items", &[])?;
    let machines: Vec<Value> = e
        .config
        .config
        .install
        .machines
        .hosts
        .iter()
        .map(|(n, h)| json!({ "name": n, "slots": h.slots }))
        .collect();
    Ok(json!({ "machines": machines, "total": total, "holding": held, "free": total - held, "ready": ready }))
}

/// The matter the architect picked, with its material by its kind.
fn matter_material(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let Some(chosen) = picked(conn, w.id, &g.within)? else {
        return Ok(None);
    };
    let mut material = Map::new();
    let matter = match chosen["queued"].as_i64() {
        Some(m) => query_json(
            conn,
            "SELECT id AS matter, kind, what, body, item, work, round, at, answers, reply FROM architect_matters
             WHERE id = ?1",
            &[&m],
            1,
        )?
        .into_iter()
        .next(),
        None => None,
    };
    let kind = matter
        .as_ref()
        .and_then(|m| m["kind"].as_str().map(str::to_string))
        .or_else(|| chosen["about"].as_str().map(str::to_string))
        .unwrap_or_default();
    // The work the matter came from, and its item, when it came from one.
    if let Some(m) = &matter {
        if let Some(item) = m["item"].as_i64() {
            for (name, q) in [
                ("item", "SELECT * FROM item_body WHERE item = ?1"),
                ("acceptance", "SELECT * FROM item_acceptance WHERE item = ?1"),
                ("around", "SELECT * FROM item_context WHERE item = ?1"),
                ("depends_on", "SELECT * FROM item_dependencies WHERE item = ?1"),
                (
                    "waited_by",
                    "SELECT l.item, i.title, l.needs FROM item_links l JOIN items i ON i.id = l.item WHERE l.depends_on = ?1",
                ),
            ] {
                material.insert(name.into(), Value::Array(query_json(conn, q, &[&item], ROWS)?));
            }
        }
        if let Some(work) = m["work"].as_i64() {
            material.insert(
                "log".into(),
                Value::Array(query_json(
                    conn,
                    "SELECT round, cell, progress FROM work_progress WHERE work = ?1 AND progress IS NOT NULL
                     ORDER BY round DESC LIMIT 30",
                    &[&work],
                    30,
                )?),
            );
        }
        if let Some(held) = m["answers"].as_i64() {
            material.insert(
                "answers".into(),
                Value::Array(query_json(
                    conn,
                    "SELECT id AS matter, kind, what, body FROM architect_matters WHERE id = ?1",
                    &[&held],
                    1,
                )?),
            );
        }
    }
    match kind.as_str() {
        "design_card" => {
            let card = matter.as_ref().and_then(|m| m["body"]["design_card"].as_i64());
            if let Some(card) = card {
                let rows = query_json(
                    conn,
                    "SELECT c.id AS design_card, c.work, c.round, c.body, c.status, c.rulings, c.changed, w.worktree,
                            i.project
                     FROM design_cards c JOIN works w ON w.id = c.work LEFT JOIN items i ON i.id = w.item
                     WHERE c.id = ?1",
                    &[&card],
                    1,
                )?;
                if let Some(c) = rows.first() {
                    let design = query_json(
                        conn,
                        "SELECT json_extract(output, '$.outcome') AS design FROM rounds
                         WHERE work = ?1 AND cell = '2a' AND transition IS NOT NULL ORDER BY n DESC LIMIT 1",
                        &[&c["work"].as_i64().unwrap_or_default()],
                        1,
                    )?;
                    material.insert("design_card".into(), c.clone());
                    material.insert(
                        "design".into(),
                        design.first().map(|d| d["design"].clone()).unwrap_or(Value::Null),
                    );
                    let project = c["project"].as_str().and_then(|p| e.config.config.projects.get(p));
                    if let (Some(wt), Some(p)) = (c["worktree"].as_str(), project) {
                        let wt = Path::new(wt);
                        let diff =
                            git::fork_point(wt, &p.main).and_then(|f| git::diff(wt, &f, "HEAD", &p.design_paths));
                        material.insert("design_tree_diff".into(), json!(diff.unwrap_or_else(|x| x)));
                    }
                }
            }
        }
        "running_unfit" | "constitution_violation" | "judgment_mismatch" | "tidy_board" => {
            for (cell, path) in [("A2", "against"), ("A2", "mismatches"), ("A5", "fit")] {
                if let Some(r) = last_return(e, w, g, cell, path)? {
                    material.insert(format!("{cell}.{path}"), at_path(&r, path).unwrap_or(Value::Null));
                }
            }
        }
        "advance" => {
            material.insert("slots".into(), slots(e)?);
            material.insert("running_and_next".into(), running_and_next(e)?);
        }
        "policy_changed" => {
            material.insert(
                "policy".into(),
                Value::Array(query_json(conn, "SELECT policy FROM policy_current", &[], 1)?),
            );
        }
        _ => {}
    }
    Ok(Some(
        json!({ "chosen": chosen, "matter": matter, "material": material }),
    ))
}

/// The significant words of a text: four letters or more, in lower case.
fn words(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 4)
        .map(str::to_lowercase)
        .collect()
}

/// The items whose title or problem is close to those at hand, and the works under way whose branches changed the
/// same paths as the work the matter came from.
fn similar_items(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<Value, String> {
    let conn = e.db.conn();
    let at_hand = words(&matter_text(e, w, g)?);
    let mut close: Vec<(usize, Value)> = query_json(
        conn,
        "SELECT id AS item, title, problem FROM items WHERE cancelled IS NULL",
        &[],
        100_000,
    )?
    .into_iter()
    .filter_map(|r| {
        let mine = words(&format!(
            "{} {}",
            r["title"].as_str().unwrap_or_default(),
            r["problem"].as_str().unwrap_or_default()
        ));
        let shared: Vec<&String> = mine.intersection(&at_hand).collect();
        (shared.len() >= 2).then(|| {
            (
                shared.len(),
                json!({ "item": r["item"], "title": r["title"], "shared_words": shared }),
            )
        })
    })
    .collect();
    close.sort_by_key(|x| std::cmp::Reverse(x.0));
    let close: Vec<Value> = close.into_iter().take(10).map(|x| x.1).collect();
    // The paths each work under way changed on its branch, against the work the matter came from.
    let mut same_paths = Vec::new();
    let from_work = picked_matter(conn, w.id, &g.within)?.and_then(|m| {
        conn.query_row("SELECT work FROM architect_matters WHERE id = ?1", [m], |r| {
            r.get::<_, Option<i64>>(0)
        })
        .ok()
        .flatten()
    });
    let changed_by = |work: &Value| -> Option<BTreeSet<String>> {
        let wt = Path::new(work["worktree"].as_str()?);
        let main = e
            .config
            .config
            .projects
            .get(work["project"].as_str()?)
            .map(|p| p.main.clone())?;
        let fork = git::fork_point(wt, &main).ok()?;
        Some(git::changed(wt, &fork, "HEAD").ok()?.into_iter().collect())
    };
    let running = query_json(
        conn,
        "SELECT w.id AS work, w.item, w.worktree, i.project FROM works w JOIN items i ON i.id = w.item
         WHERE w.graph = 'engineer' AND w.ended IS NULL AND w.worktree IS NOT NULL",
        &[],
        ROWS,
    )?;
    if let Some(mine) = running
        .iter()
        .find(|r| r["work"].as_i64() == from_work)
        .and_then(changed_by)
    {
        for r in running.iter().filter(|r| r["work"].as_i64() != from_work) {
            let theirs = changed_by(r).unwrap_or_default();
            let both: Vec<&String> = mine.intersection(&theirs).collect();
            if !both.is_empty() {
                same_paths.push(json!({ "work": r["work"], "item": r["item"], "paths": both }));
            }
        }
    }
    Ok(json!({ "close_items": close, "same_paths": same_paths }))
}

/// The items and segments the changes collected touch, with where each item hangs, who waits for it and what it waits
/// for; each segment as the plan has it; the gates and budgets touched.
fn changes_touch(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<Option<Value>, String> {
    let Some(changes) = last_return(e, w, g, "A8", "changes")?.and_then(|r| r.get("changes").cloned()) else {
        return Ok(None);
    };
    let mut item_ids: BTreeSet<i64> = BTreeSet::new();
    let mut segments: BTreeSet<String> = BTreeSet::new();
    let mut gates: BTreeSet<String> = BTreeSet::new();
    let mut budgets: BTreeSet<String> = BTreeSet::new();
    let mut touched: BTreeSet<String> = BTreeSet::new();
    let number_or_key = |v: &Value, ids: &mut BTreeSet<i64>, keys: &mut BTreeSet<String>| match v {
        Value::Number(n) => ids.extend(n.as_i64()),
        Value::String(s) => match s.trim().trim_start_matches('#').parse::<i64>() {
            Ok(n) => {
                ids.insert(n);
            }
            Err(_) => {
                keys.insert(s.clone());
            }
        },
        _ => {}
    };
    for c in items(Some(&changes)) {
        for k in ["item", "on", "keep"] {
            ids_of(&c[k], &mut item_ids);
        }
        for a in items(c.get("absorbed")) {
            ids_of(&a, &mut item_ids);
        }
        for k in ["under", "to", "fix_item"] {
            number_or_key(&c[k], &mut item_ids, &mut segments);
        }
        for k in ["segment", "key"] {
            if let Some(s) = c[k].as_str().filter(|_| c["kind"] != "create_item") {
                segments.insert(s.to_string());
            }
        }
        for k in ["after", "order", "segments"] {
            segments.extend(items(c.get(k)).iter().filter_map(|x| x.as_str().map(str::to_string)));
        }
        segments.extend(
            items(c.get("split"))
                .iter()
                .filter_map(|x| x["segment"].as_str().map(str::to_string)),
        );
        touched.extend(
            items(c.get("touches"))
                .iter()
                .filter_map(|x| x.as_str().map(str::to_string)),
        );
        match c["kind"].as_str() {
            Some("add_gate") | Some("edit_gate") => {
                gates.extend(c["name"].as_str().or(c["gate"].as_str()).map(str::to_string));
            }
            Some("add_budget") | Some("edit_budget") => {
                budgets.extend(c["name"].as_str().or(c["budget"].as_str()).map(str::to_string));
            }
            _ => {}
        }
    }
    let conn = e.db.conn();
    let mut item_rows = Vec::new();
    for id in &item_ids {
        item_rows.extend(query_json(
            conn,
            "SELECT i.id AS item, i.title, i.segment, i.cancelled, i.held,
                    (SELECT json_group_array(l.item) FROM item_links l WHERE l.depends_on = i.id) AS waited_by,
                    (SELECT json_group_array(l.depends_on) FROM item_links l WHERE l.item = i.id) AS waits_for
             FROM items i WHERE i.id = ?1",
            &[id],
            1,
        )?);
    }
    let mut segment_rows = Vec::new();
    for key in &segments {
        segment_rows.extend(query_json(conn, "SELECT * FROM plan WHERE segment = ?1", &[key], 1)?);
    }
    let mut gate_rows = query_json(conn, "SELECT * FROM plan_gates ORDER BY name", &[], ROWS)?;
    gate_rows.retain(|r| {
        r["name"].as_str().is_some_and(|n| gates.contains(n))
            || r["what"].as_str().is_some_and(|w| touched.contains(w))
            || items(r.get("segments"))
                .iter()
                .any(|s| s.as_str().is_some_and(|s| segments.contains(s)))
    });
    let mut budget_rows = query_json(conn, "SELECT * FROM plan_budgets ORDER BY name", &[], ROWS)?;
    budget_rows.retain(|r| {
        r["name"].as_str().is_some_and(|n| budgets.contains(n))
            || items(r.get("split"))
                .iter()
                .any(|s| s["segment"].as_str().is_some_and(|s| segments.contains(s)))
    });
    Ok(Some(json!({
        "items": item_rows,
        "segments": segment_rows,
        "gates": gate_rows,
        "budgets": budget_rows,
    })))
}

/// An item's number given as a number or as a text of one, added to a set.
fn ids_of(v: &Value, ids: &mut BTreeSet<i64>) {
    match v {
        Value::Number(n) => ids.extend(n.as_i64()),
        Value::String(s) => ids.extend(s.trim().trim_start_matches('#').parse::<i64>().ok()),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_the_architect_sets_is_read_in_utc() {
        assert_eq!(parse_utc("1970-01-01T00:00Z"), Some(0));
        assert_eq!(parse_utc("2026-10-25T21:00Z"), Some(1_792_962_000_000));
        assert_eq!(parse_utc("2000-02-29T00:00Z"), Some(951_782_400_000));
        assert_eq!(parse_utc("tomorrow"), None);
        assert_eq!(parse_utc("2026-13-01T00:00Z"), None);
    }

    #[test]
    fn a_row_names_an_article_by_its_number_or_its_title() {
        let a = "Article 1. Independent of any project; developed by itself";
        assert!(names_article("Article 1", a));
        assert!(names_article("independent of any project; developed by itself", a));
        assert!(!names_article("Article 10", a));
        assert!(!names_article("Article 2", a));
    }

    #[test]
    fn a_policy_names_no_date() {
        assert!(names_date("as said on 2026-10-27, keep it"));
        assert!(!names_date("version 2026-10 and 27"));
    }
}
