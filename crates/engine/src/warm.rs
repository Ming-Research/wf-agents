//! While a work goes on or waits, the sessions of its lines the flow can come back to, which the session manager keeps
//! warm ([`sessions::warm`]): the engine knows where the work stands and what it waits on, and the flow which lines
//! each way from there leads back to. A work the owner's global pause holds waits on it, and is kept warm while the
//! owner keeps that switch on. While a work of a project's architect runs, the project's base, which its next matters
//! fork, is kept warm too, and before each fork is made the base is warmed when the cache may not keep its end under
//! the fork's command line ([`before_fork`]); each such ping runs the command line the forks run with.

use std::collections::{BTreeMap, BTreeSet};

use db::rusqlite::{OptionalExtension, params};
use flow::{CompiledCell, CompiledGraph, Position};
use serde_json::Value;
use sessions::cli::Request;
use sessions::warm::Waited;

use crate::work::{self, Work};
use crate::{ARCHITECT, Engine, RunsOn, blockers};

/// The owner's global pause, while it holds, as keeping sessions warm reads it: whether the owner keeps the sessions of
/// waiting works warm meanwhile (the switch beside the pause, on at first), the works the owner started, which run on
/// despite it, and the projects whose architect the owner started, whose architect works then do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlobalPause {
    pub warm: bool,
    pub runs: BTreeSet<i64>,
    pub architects: BTreeSet<String>,
}

/// The lines whose sessions a work can come back to from where it stands, each with the agents' cells of the line it
/// can come back at: every agent's cell of the line that some way from here reaches, through the cells of other lines,
/// the engine's, the tools' and the owner's, before any cell of the line where the flow opens it a new session
/// (`opens`), where the session it has going on ends; the cell the work stands at counts when it is an agent's. A line
/// none of whose cells a way reaches, or reaches only past such a cell, is behind the work for good, and is left out.
pub fn coming_back(
    g: &CompiledGraph,
    pos: &Position,
    opens: &dyn Fn(&CompiledCell) -> bool,
) -> BTreeMap<String, Vec<String>> {
    let lines: BTreeSet<&str> = g
        .cells
        .iter()
        .filter(|c| c.kind == "agent")
        .filter_map(|c| c.line.as_deref())
        .collect();
    let mut out = BTreeMap::new();
    for line in lines {
        let mut found: Vec<String> = Vec::new();
        let mut seen: Vec<(String, Vec<String>, Option<String>)> = Vec::new();
        let mut todo = vec![pos.clone()];
        while let Some(p) = todo.pop() {
            let Some(cell) = g.cell(&p.at) else {
                continue;
            };
            // A cell of the line is where its session comes back, unless the flow opens it a new one there.
            if cell.kind == "agent" && cell.line.as_deref() == Some(line) {
                if !opens(cell) && !found.contains(&cell.id) {
                    found.push(cell.id.clone());
                }
                continue;
            }
            let key = (p.at.clone(), p.callers.clone(), p.stopped.clone());
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            for r in &cell.rows {
                // A row that waits where it is, or stays, leads nowhere new.
                if r.to == "@self" || r.to == cell.id {
                    continue;
                }
                if let Ok(next) = g.step(&p, r) {
                    todo.push(next);
                }
            }
        }
        if !found.is_empty() {
            out.insert(line.to_string(), found);
        }
    }
    out
}

/// What a work waits on, where it stands, as the engine judges it there: its pause (`pause`); once what it waited for
/// came, a free slot to go on in, which the loop grants it (`slot`); else the holder of its open blocker, which the flow
/// raises at each cell where a work waits for someone: the owner's answer to its ask (`owner`), the architect's reply
/// to its matter (`architect`), the items it waits for (`items`), only where the flow has the items hold it; with no
/// blocker, the owner's or the architect's answer at a cell that asks one, and else what the row it took waits for
/// (`row`). None when it does not wait, and so goes on by itself.
fn waited_on(e: &mut Engine, w: &Work, g: &CompiledGraph) -> Result<Option<Waited>, String> {
    let waited = |on: &str, what: String| Waited {
        cell: w.pos.at.clone(),
        on: on.to_string(),
        what,
    };
    if let Some(why) = &w.paused {
        return Ok(Some(waited("pause", why.clone())));
    }
    if !e.waiting(w)? {
        return Ok(None);
    }
    let Some(cell) = g.cell(&w.pos.at) else {
        return Ok(None);
    };
    let conn = e.db.conn();
    if db::slot_waiting(conn).map_err(|x| x.to_string())?.contains(&w.id) {
        return Ok(Some(waited(
            "slot",
            format!("a free slot to go on from cell {}, what it waited for come", cell.id),
        )));
    }
    if let Some(b) = blockers::open(conn, w.id)? {
        let what = match (b.holder.as_str(), b.ask, b.matter) {
            ("owner", Some(ask), _) => format!("the owner's answer to ask {ask}"),
            ("architect", _, Some(matter)) => format!("the architect's reply to matter {matter}"),
            ("items", _, _) if !b.items.is_empty() => {
                let items: Vec<String> = b.items.iter().map(|(i, _)| i.to_string()).collect();
                let named = if items.len() == 1 { "item" } else { "items" };
                format!("{named} {}, which this item depends on, to be merged", items.join(", "))
            }
            _ => b.what.clone(),
        };
        return Ok(Some(waited(b.holder.as_str(), what)));
    }
    match cell.step.as_deref() {
        Some("ask_owner") => {
            let open = work::open_round(conn, w.id)?.map(|r| r.id);
            let ask: Option<i64> = open.and_then(|r| {
                conn.query_row(
                    "SELECT id FROM owner_asks WHERE work = ?1 AND round = ?2",
                    params![r.work, r.n],
                    |x| x.get(0),
                )
                .ok()
            });
            let what = ask.map_or_else(
                || "the owner's answer".to_string(),
                |a| format!("the owner's answer to ask {a}"),
            );
            Ok(Some(waited("owner", what)))
        }
        Some("ask_architect") => Ok(Some(waited("architect", "the architect's reply".to_string()))),
        _ => {
            let row = work::rounds(conn, w.id)?
                .pop()
                .and_then(|r| r.transition)
                .and_then(|t| cell.rows.iter().find(|x| x.name == t).cloned());
            let said = row.map(|r| r.note.unwrap_or(r.name)).unwrap_or_default();
            Ok(Some(waited("row", said)))
        }
    }
}

/// A keep-warm ping due to a session of a work's line, with where the work waits and the line's name; for a ping of a
/// project's architect's base, what it runs ([`Prepared`]), which is not the base's last round's command line.
#[derive(Debug, Clone)]
pub struct Ping {
    due: sessions::warm::Due,
    waited: Waited,
    line: String,
    base: Option<Box<Prepared>>,
}

/// A ping of a project's architect's base as the engine prepares it: the request a fork of the base is sent with,
/// resuming the base instead of forking it, with the keep-warm message and no hooks or images, so that all that comes
/// before the messages is what the forks put there and the cache keeps the base's end under it; and its call as a
/// round's record keeps one, with the texts to keep in `contents`.
#[derive(Debug, Clone)]
struct Prepared {
    req: Request,
    call: Value,
    kept: Vec<(String, String)>,
}

/// The ping that warms a project's architect's base for its forks, from the request a fork is sent with (`fork`): it
/// resumes the base (`base_cli`), whose last round was at `cell`, and the CLI it runs reports `cli_version`.
fn prepare(fork: &Request, base_cli: &str, cell: &str, cli_version: &str) -> Prepared {
    let mut req = Request {
        cell: cell.to_string(),
        resume: Some(base_cli.to_string()),
        fork: false,
        new_session: base_cli.to_string(),
        prompt: sessions::warm::MESSAGE.to_string(),
        images: Vec::new(),
        regive: None,
        deliver: None,
        replay: None,
        ..fork.clone()
    };
    let line = sessions::cli::command_line(&req);
    let mut kept = Vec::new();
    let call = crate::steps::kept_call(sessions::cli::call(&req, &line, cli_version, crate::VERSION), &mut kept);
    req.replay = Some(line);
    Prepared { req, call, kept }
}

/// The keep-warm ping due at `now` to the base of a project's architect while a work of the architect runs (`w`, on
/// the graph `g`), whose next matters fork the base, as the session manager finds it due ([`sessions::warm::base_due`]):
/// with the command line its forks run with, as a round of the graph's first cell whose line forks the base would be
/// sent, carrying the base's system prompt. None before the first base, and while a new base is being built, its line
/// having another session going on than the base's, which the line's own pings keep warm.
fn base_due(
    e: &Engine,
    w: &Work,
    g: &CompiledGraph,
    project: &str,
    waited: &Waited,
    now: i64,
) -> Result<Option<Ping>, String> {
    let conn = e.db.conn();
    let Some(base) = crate::base::latest(conn, project)? else {
        return Ok(None);
    };
    let Some(fork) = g.cells.iter().find(|c| c.kind == "agent" && c.fork_of.is_some()) else {
        return Ok(None);
    };
    let line = fork.fork_of.clone().unwrap_or_default();
    if sessions::choose::base(conn, project, &line)?.is_some_and(|(_, cli)| cli != base.cli_session) {
        return Ok(None);
    }
    let (_, level, _) = crate::steps::level_of(e, g, fork, None)?;
    let runs = e.runs_on(&level);
    let Some(due) = sessions::warm::base_due(conn, (base.session, base.cli_session.as_str()), runs.on(), now)? else {
        return Ok(None);
    };
    let request = Request {
        system: sessions::system::of_session(conn, &due.rounds),
        ..crate::steps::cell_request(e, w, g, fork, None)?
    };
    let cell = due.last.cell.clone().unwrap_or_default();
    let prepared = prepare(&request, &base.cli_session, &cell, &e.cli_version(&level.cli));
    Ok(Some(Ping {
        due,
        waited: waited.clone(),
        line,
        base: Some(Box::new(prepared)),
    }))
}

/// Warms the base of a project's architect before the round `fork` (its request, and its call as its record keeps it)
/// makes a fork of it, on what `runs` says its level runs on, when the session manager finds the cache may not keep the
/// base's end under the fork's command line ([`sessions::warm::before_fork`]): one ping that runs the fork's command line,
/// resuming the base, recorded as waiting on the fork, so that the fork and each fork after it read all the base holds
/// from the cache. A ping that fails leaves the fork to write the base again, as it would have.
pub fn before_fork(
    e: &mut Engine,
    w: &Work,
    fork: &Request,
    call: &Value,
    cli_version: &str,
    runs: &RunsOn,
) -> Result<(), String> {
    let Some(base_cli) = fork.resume.as_deref() else {
        return Ok(());
    };
    // The base's latest row, of the architect work that took it up last.
    let row: Option<(i64, String)> =
        e.db.conn()
            .query_row(
                "SELECT id, line FROM sessions WHERE cli_session = ?1 ORDER BY id DESC LIMIT 1",
                [base_cli],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|x| x.to_string())?;
    let Some((row, line)) = row else {
        return Ok(());
    };
    let now = db::now();
    let Some(due) = sessions::warm::before_fork(e.db.conn(), (row, base_cli), call, runs.on(), now)? else {
        return Ok(());
    };
    let cell = due.last.cell.clone().unwrap_or_default();
    let p = Ping {
        waited: Waited {
            cell: w.pos.at.clone(),
            on: "fork".to_string(),
            what: format!("the {} line forks the base next", fork.line),
        },
        line,
        base: Some(Box::new(prepare(fork, base_cli, &cell, cli_version))),
        due,
    };
    send(e, w.id, std::slice::from_ref(&p), now)
}

/// The keep-warm pings due at `now`, by work: one to each session of a work's line that the flow can come back to
/// from where the work stands ([`coming_back`]), whether the work goes on or waits, when the session manager finds it
/// due. A work with a job in flight is due none, since a step of it may run a round in the session (the loop
/// dispatches a work's pings as its job, one job at most for each work). While the owner's global pause holds
/// (`pause`), a work it holds waits on it; none is due then if the owner switched keeping warm off, and none while a
/// failed run's cause stops every work. A work whose next round starts the new session the owner asked for has nothing
/// to keep warm, nor has a work on an older flow version that the version the engine runs cannot take on where it
/// stands, until the loop has brought it up, nor a merged work waiting for its change to take effect. A work of a
/// project's architect is due a ping of the project's base too, whose next matters fork it ([`base_due`]): only while a
/// work of the architect runs, since no fork of the base comes otherwise, and the first fork after a wait warms the base
/// itself ([`before_fork`]).
pub fn due(e: &mut Engine, now: i64, pause: Option<&GlobalPause>) -> Result<Vec<(i64, Vec<Ping>)>, String> {
    let mut out = Vec::new();
    if e.halted() || pause.is_some_and(|p| !p.warm) {
        return Ok(out);
    }
    let ids: Vec<i64> = {
        let mut st =
            e.db.conn()
                .prepare("SELECT id FROM works WHERE ended IS NULL ORDER BY id")
                .map_err(|x| x.to_string())?;
        st.query_map([], |r| r.get(0))
            .map_err(|x| x.to_string())?
            .collect::<Result<_, _>>()
            .map_err(|x| x.to_string())?
    };
    for id in ids {
        if e.has_job(id) {
            continue;
        }
        let w = e.work(id)?;
        // A work on an older flow version goes on in its sessions where it stands only when it moves to the version the
        // engine runs.
        if e.done(&w)? || (w.flow_version != e.flow_now && e.plan(&w)? != crate::versions::Plan::Move) {
            continue;
        }
        // A merged work waiting for its change to take effect may wait for days, and looks in a new session each time.
        if crate::verification::waits_for_effect(&w) {
            continue;
        }
        let architect = w.project.as_deref().filter(|_| w.graph == ARCHITECT);
        if e.sessions().asked_new(w.id, architect) {
            continue;
        }
        let g = e.graph_of(&w)?;
        let waited = match waited_on(e, &w, &g)? {
            Some(x) => x,
            None if pause
                .is_some_and(|p| !p.runs.contains(&id) && !architect.is_some_and(|x| p.architects.contains(x))) =>
            {
                Waited {
                    cell: w.pos.at.clone(),
                    on: "pause".to_string(),
                    what: "the owner paused every work".to_string(),
                }
            }
            // A work that goes on, at another line's round, a tool's or the engine's, keeps the lines it will come
            // back to warm.
            None => Waited {
                cell: w.pos.at.clone(),
                on: "work".to_string(),
                what: "the work goes on".to_string(),
            },
        };
        // The lines the work can come back to, each in the session it has going on: not past a cell where the flow
        // opens the line a new one.
        let lines = {
            let conn = e.db.conn();
            let opens = |c: &CompiledCell| sessions::choose::opens(conn, w.id, c).unwrap_or(true);
            coming_back(&g, &w.pos, &opens)
        };
        let item = e.subject(&w)?;
        let limit = e.config.config.install.agents.context_limit;
        let mut pings = Vec::new();
        for (line, next) in lines {
            let cells: Vec<&CompiledCell> = next.iter().filter_map(|c| g.cell(c)).collect();
            // The model and the login account the line's next round runs on: a session on another one starts anew,
            // and is not kept warm.
            let Some(level) = cells
                .first()
                .and_then(|c| crate::steps::level_of(e, &g, c, item.as_ref()).ok())
                .map(|(_, l, _)| l)
            else {
                continue;
            };
            let runs = e.runs_on(&level);
            let Some(due) = sessions::warm::due(e.db.conn(), w.id, &cells, limit, runs.on(), now)? else {
                continue;
            };
            pings.push(Ping {
                due,
                waited: waited.clone(),
                line,
                base: None,
            });
        }
        // The project's base, which the architect's next matters fork, unless a line of the work keeps its session warm
        // already.
        if let Some(project) = architect
            && let Some(p) = base_due(e, &w, &g, project, &waited, now)?
            && !pings.iter().any(|x| x.due.cli_session == p.due.cli_session)
        {
            pings.push(p);
        }
        if !pings.is_empty() {
            out.push((id, pings));
        }
    }
    Ok(out)
}

/// Sends a work's keep-warm pings found due at `now`, as the job the loop dispatched for them, and records each.
pub fn send(e: &mut Engine, work: i64, pings: &[Ping], now: i64) -> Result<(), String> {
    for p in pings {
        let of = if p.base.is_some() { "the base, " } else { "" };
        e.say(&format!(
            "work {work}: keep-warm ping {} of this wait to {of}session {} of the {} line, as the work stands at cell {} \
             on {}: {}",
            p.due.pings + 1,
            p.due.session,
            p.line,
            p.waited.cell,
            p.waited.on,
            p.waited.what
        ));
        match &p.base {
            Some(b) => sessions::warm::ping_with(
                &mut e.db,
                e.shared.sessions.agents(),
                &b.req,
                &p.due,
                &p.waited,
                Some((&b.call, b.kept.as_slice())),
                now,
            )?,
            None => sessions::warm::ping(
                &mut e.db,
                e.shared.sessions.agents(),
                &e.config.config,
                &p.due,
                &p.waited,
                now,
            )?,
        };
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph() -> CompiledGraph {
        let relations = db::relations(db::SCHEMA).unwrap();
        let tables = db::index_of(db::SCHEMA).unwrap();
        let f = flow::compile(flow::FLOW, &flow::Context { relations, tables }).unwrap();
        f.graph(crate::ENGINEER).unwrap().clone()
    }

    fn at(cell: &str, callers: &[&str], stopped: Option<&str>) -> Position {
        Position {
            at: cell.into(),
            stopped: stopped.map(str::to_string),
            callers: callers.iter().map(|c| c.to_string()).collect(),
            ..Position::default()
        }
    }

    // While the engineer implements, every line of the work can come back: the engineer's at the cell it stands at,
    // the reviews and adversarial testing after it, the advisor it may ask. A line comes back only at a cell where it
    // goes on in its session, not past one where the flow opens it a new one: while the reviews run and fixing has not
    // started, the engineer's implementing session comes back where the work chooses its way on, not at fixing; a line
    // all of whose cells open a new session is behind the work for good, as is every line at the work's end.
    #[test]
    fn a_line_comes_back_where_a_way_reaches_it_in_its_session() {
        let g = graph();
        let none = |_: &CompiledCell| false;
        let implementing = coming_back(&g, &at("I1", &[], None), &none);
        assert_eq!(implementing["engineer"][0], "I1");
        for line in ["review", "adversarial", "advisor"] {
            assert!(implementing.contains_key(line), "{line}: {implementing:?}");
        }
        let fixing_first = |c: &CompiledCell| c.id == "T6";
        let reviewing = coming_back(&g, &at("T4", &[], None), &fixing_first);
        let engineer = &reviewing["engineer"];
        assert!(!engineer.contains(&"T6".to_string()), "{engineer:?}");
        assert!(engineer.contains(&"A4".to_string()), "{engineer:?}");
        assert_eq!(reviewing["review"][0], "T4");
        let review_anew = |c: &CompiledCell| c.line.as_deref() == Some("review");
        let anew = coming_back(&g, &at("T6", &[], None), &review_anew);
        assert!(
            !anew.contains_key("review") && anew.contains_key("engineer"),
            "{anew:?}"
        );
        // Asking the architect, the work comes back to the line that asked, at the cell that takes its answers.
        let asked = coming_back(&g, &at("A2", &["I1"], Some("I1")), &none);
        assert!(asked["engineer"].contains(&"A4".to_string()), "{asked:?}");
        assert!(coming_back(&g, &at("E1", &[], None), &none).is_empty());
    }
}
