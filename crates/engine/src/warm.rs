//! When a work waits, the session its next round will go on in, which the session manager keeps warm
//! ([`sessions::warm`]): the engine knows where the work stands and what it waits on, and the flow where it goes next.
//! A work the owner's global pause holds waits on it, and is kept warm while the owner keeps that switch on.

use std::collections::BTreeSet;

use db::rusqlite::params;
use flow::{CompiledCell, CompiledGraph, Position};
use sessions::warm::Waited;

use crate::work::{self, Work};
use crate::{ARCHITECT, Engine, values};

/// The owner's global pause, while it holds, as keeping sessions warm reads it: whether the owner keeps the sessions of
/// waiting works warm meanwhile (the switch beside the pause, on at first), the works the owner started, which run on
/// despite it, and the projects whose architect the owner started, whose wakes then do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlobalPause {
    pub warm: bool,
    pub runs: BTreeSet<i64>,
    pub architects: BTreeSet<String>,
}

/// The agents' cells a work's next round may be at, from where it stands: the cell itself when it is an agent's, else
/// every agent's cell its rows lead to through the engine's, the tools' and the owner's cells; none when a way leads
/// to the end with no agent's round, so that the next round may never come.
pub fn next_cells(g: &CompiledGraph, pos: &Position) -> Option<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: Vec<(String, Vec<String>)> = Vec::new();
    let mut todo = vec![pos.clone()];
    while let Some(p) = todo.pop() {
        let cell = g.cell(&p.at)?;
        if cell.kind == "agent" {
            if !out.contains(&cell.id) {
                out.push(cell.id.clone());
            }
            continue;
        }
        if cell.rows.is_empty() {
            return None;
        }
        let key = (p.at.clone(), p.callers.clone());
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        for r in &cell.rows {
            // A row that waits where it is, or stays, is the wait itself.
            if r.to == "@self" || r.to == cell.id {
                continue;
            }
            if let Ok(next) = g.step(&p, r) {
                todo.push(next);
            }
        }
    }
    (!out.is_empty()).then_some(out)
}

/// What a work waits on, where it stands: its pause, the owner's or the architect's answer, a row that waits for
/// other items or a ruling; none when it does not wait, and so goes on by itself.
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
    match cell.step.as_deref() {
        Some("ask_owner") => {
            let open = work::open_round(e.db.conn(), w.id)?.map(|r| r.id);
            let ask: Option<i64> = open.and_then(|r| {
                e.db.conn()
                    .query_row(
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
            let row = work::rounds(e.db.conn(), w.id)?
                .pop()
                .and_then(|r| r.transition)
                .and_then(|t| cell.rows.iter().find(|x| x.name == t).cloned());
            let said = row.map(|r| r.note.unwrap_or(r.name)).unwrap_or_default();
            let gate = match e.subject(w)? {
                Some(item) => values::gates_waiting_for(e, w, &item).unwrap_or(None),
                None => None,
            };
            Ok(Some(match gate {
                Some(text) if text.starts_with("the owner's") => waited("owner", text),
                Some(text) => waited("items", text),
                None => waited("items", said),
            }))
        }
    }
}

/// Sends a keep-warm ping to each session whose work waits and whose next round will go on in it, when the session
/// manager finds it due at `now`; gives how many it sent. While the owner's global pause holds (`pause`), a work it
/// holds waits on it; none is sent then if the owner switched keeping warm off, and none while a failed run's cause
/// stops every work. A work whose next round starts the new session the owner asked for has nothing to keep warm.
pub fn keep_warm(e: &mut Engine, now: i64, pause: Option<&GlobalPause>) -> Result<usize, String> {
    if e.halted() || pause.is_some_and(|p| !p.warm) {
        return Ok(0);
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
    let mut sent = 0;
    for id in ids {
        let w = e.work(id)?;
        if w.flow_version != e.flow_now || e.done(&w)? {
            continue;
        }
        let wake = w.project.as_deref().filter(|_| w.graph == ARCHITECT);
        if e.sessions.asked_new(w.id, wake) {
            continue;
        }
        let g = e.graph_of(&w)?;
        let waited = match waited_on(e, &w, &g)? {
            Some(x) => x,
            None if pause.is_some_and(|p| !p.runs.contains(&id) && !wake.is_some_and(|x| p.architects.contains(x))) => {
                Waited {
                    cell: w.pos.at.clone(),
                    on: "pause".to_string(),
                    what: "the owner paused every work".to_string(),
                }
            }
            None => continue,
        };
        let Some(next) = next_cells(&g, &w.pos) else {
            continue;
        };
        let cells: Vec<&CompiledCell> = next.iter().filter_map(|c| g.cell(c)).collect();
        let limit = e.config.config.install.agents.context_limit;
        // The model the next round runs on: a session on another one starts anew, and is not kept warm.
        let item = e.subject(&w)?;
        let Some(model) = cells
            .first()
            .and_then(|c| crate::steps::level_of(e, &g, c, item.as_ref()).ok())
            .map(|(_, l, _)| l.model)
        else {
            continue;
        };
        let Some(due) = sessions::warm::due(e.db.conn(), w.id, &cells, limit, &model, now)? else {
            continue;
        };
        (e.say)(&format!(
            "work {id}: keep-warm ping {} of this wait to session {}, as the work waits at cell {} on {}: {}",
            due.pings + 1,
            due.session,
            waited.cell,
            waited.on,
            waited.what
        ));
        sessions::warm::ping(&mut e.db, e.sessions.agents(), &e.config.config, &due, &waited, now)?;
        sent += 1;
    }
    Ok(sent)
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

    // A work at an agent's cell goes on there; one waiting for the owner's answer goes back to the line that asked;
    // one waiting at the merge gate may merge with no agent's round, so its next round may never come.
    #[test]
    fn the_next_round_is_where_every_way_from_the_wait_leads() {
        let g = graph();
        assert_eq!(
            next_cells(&g, &at("implement", &[], None)),
            Some(vec!["implement".to_string()])
        );
        let asked = next_cells(&g, &at("to_owner", &["implement"], Some("implement"))).unwrap();
        assert!(
            asked
                .iter()
                .all(|c| g.cell(c).unwrap().line.as_deref() == Some("engineer")),
            "{asked:?}"
        );
        assert_eq!(next_cells(&g, &at("merge_gate", &[], None)), None);
    }
}
