//! While a work goes on or waits, the sessions of its lines the flow can come back to, which the session manager keeps
//! warm ([`sessions::warm`]): the engine knows where the work stands and what it waits on, and the flow which lines
//! each way from there leads back to. A work the owner's global pause holds waits on it, and is kept warm while the
//! owner keeps that switch on.

use std::collections::{BTreeMap, BTreeSet};

use db::rusqlite::params;
use flow::{CompiledCell, CompiledGraph, Position};
use sessions::warm::Waited;

use crate::work::{self, Work};
use crate::{ARCHITECT, Engine, values};

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

/// Sends a keep-warm ping to each session of a work's line that the flow can come back to from where the work stands
/// ([`coming_back`]), whether the work goes on or waits, when the session manager finds it due at `now`; gives how many
/// it sent. While the owner's global pause holds (`pause`), a work it holds waits on it; none is sent then if the owner
/// switched keeping warm off, and none while a failed run's cause stops every work. A work whose next round starts the
/// new session the owner asked for has nothing to keep warm, nor has a work on an older flow version that a new work
/// will take over.
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
        // A work on an older flow version goes on in its sessions only when it can move to the version the engine runs.
        if e.done(&w)? || (w.flow_version != e.flow_now && e.cannot_move(&w)?.is_some()) {
            continue;
        }
        let architect = w.project.as_deref().filter(|_| w.graph == ARCHITECT);
        if e.sessions.asked_new(w.id, architect) {
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
            let home = e.cli_dir(&level.cli);
            let on = sessions::On {
                model: &level.model,
                home: home.as_deref(),
            };
            let Some(due) = sessions::warm::due(e.db.conn(), w.id, &cells, limit, on, now)? else {
                continue;
            };
            (e.say)(&format!(
                "work {id}: keep-warm ping {} of this wait to session {} of the {line} line, as the work stands at \
                 cell {} on {}: {}",
                due.pings + 1,
                due.session,
                waited.cell,
                waited.on,
                waited.what
            ));
            sessions::warm::ping(&mut e.db, e.sessions.agents(), &e.config.config, &due, &waited, now)?;
            sent += 1;
        }
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
        // Asking the owner, the work comes back to the line that asked.
        let asked = coming_back(&g, &at("A3", &["I1"], Some("I1")), &none);
        assert!(asked["engineer"].contains(&"I1".to_string()), "{asked:?}");
        assert!(coming_back(&g, &at("E1", &[], None), &none).is_empty());
    }
}
