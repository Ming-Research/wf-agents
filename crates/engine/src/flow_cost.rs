//! What installing a flow would do with each work that is neither ended nor done, worked out before it is installed, so
//! that whoever designs, rules on or installs a flow change sees what it costs the works that go on. On an engine made
//! only to read that runs the flow ([`Engine::trying`]), each work is judged by the very code the engine runs as it
//! brings a work on an older version up when the work next goes on: the plan ([`Engine::plan`]), the cell a work comes
//! to when it cannot move ([`Engine::version_cell`]), and that cell's step without its writes ([`steps::arrival`]): the
//! row it takes there by what it is given, where that row leads, and whether it pauses the work. So a work moves where
//! it stands, goes from that cell to the cell its row leads to, stays there stuck, or ends: where the version has no
//! graph for it, or where that row leads to its graph's end for a work stopped before its end. Nothing is written, and
//! the whole is read as one moment of the database.

use std::fmt;
use std::path::Path;

use db::rusqlite::OptionalExtension;

use crate::versions::Plan;
use crate::work::{RoundId, Work};
use crate::{ENGINEER, Engine, architect, steps};

/// What installing a flow version does with a work when the work next goes on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// The work is on that version already: nothing.
    Stays,
    /// It moves where it stands: the same work at the same cell, in the same sessions.
    Moves,
    /// It comes to cell `through` for the reason `why`, and goes from there to cell `to`, by the row that cell takes,
    /// which it does again from its start: round `from` and the rounds after it, `redone` in all, are done again.
    GoesBack {
        through: String,
        to: String,
        why: String,
        from: Option<RoundId>,
        redone: usize,
    },
    /// It comes to the version's cell for it for the reason `why` and stays at cell `at`, paused for `pause`, which says
    /// what the owner can do; the owner's Start takes it back to cell `back_to` all the same, when it has one.
    Stuck {
        at: String,
        why: String,
        back_to: Option<String>,
        pause: String,
    },
    /// It ends, for the reason `why`: no cell of the version can run it, or the row the version's cell for it takes
    /// leads to the graph's end for a work stopped before its end; a new work takes its item over when it is an
    /// engineer's work on an item whose graph the version lacks (`taken_over`).
    Ends { why: String, taken_over: bool },
}

/// A work that is neither ended nor done, with what installing the flow does with it.
#[derive(Debug, Clone)]
pub struct WorkCost {
    pub id: i64,
    /// The work as the database holds it; none when it cannot be read.
    pub work: Option<Work>,
    /// The title of its item, for a work on one.
    pub title: Option<String>,
    /// What installing the flow does with it, or why that cannot be worked out: the engine meets that error as it
    /// brings the work up, and stops.
    pub effect: Result<Effect, String>,
}

impl WorkCost {
    /// The group the work is told in, among those of what happens to the works: [`Group`].
    pub fn group(&self) -> Group {
        match &self.effect {
            Err(_) => Group::Unreadable,
            Ok(Effect::GoesBack { .. }) => Group::GoesBack,
            Ok(Effect::Stuck { .. }) => Group::Stuck,
            Ok(Effect::Ends { .. }) => Group::Ends,
            Ok(Effect::Moves) => Group::Moves,
            Ok(Effect::Stays) => Group::Stays,
        }
    }
}

/// What happens to a work, as the works are told: the command line and the owner's page both list them in this order,
/// the ones that cost something first, and the one that cannot be worked out before all, since the engine stops when it
/// brings such a work up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    /// It cannot be worked out.
    Unreadable,
    /// It goes back through a cell to a cell it does again.
    GoesBack,
    /// It stays stuck at a cell, paused.
    Stuck,
    /// It ends.
    Ends,
    /// It moves where it stands.
    Moves,
    /// It is on the version already.
    Stays,
}

/// What installing a flow would do: the version it would be stored as, whether the database has that version already,
/// and each work that is neither ended nor done, by id, those paused included.
#[derive(Debug, Clone)]
pub struct Cost {
    pub version: i64,
    pub stored: bool,
    pub works: Vec<WorkCost>,
}

/// [`Cost`] of a flow given as text, with the version the database was at when it was read.
#[derive(Debug, Clone)]
pub struct Costed {
    pub cost: Cost,
    /// The database's version before the engine that would run the flow migrates it: an older one is judged as that
    /// engine finds it, on a copy migrated in memory.
    pub database_at: i64,
}

/// Why a flow could not be costed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// The flow does not compile against the database definition built into this program, or does not fit the committed
    /// configuration: what the text says is the flow's.
    Flow(String),
    /// The data directory could not be read: its configuration or its database.
    Data(String),
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (Refused::Flow(s) | Refused::Data(s)) = self;
        f.write_str(s)
    }
}

/// What installing the flow `text` on the data directory `data` would do with each work that is neither ended nor done:
/// the flow is compiled and checked against the data directory's committed configuration as the engine does at its
/// start, and each work judged by an engine made only to read, on the database opened only to read ([`Engine::trying`]).
/// Nothing is written, so it runs beside a running engine. The command line and the owner's page both read the cost
/// of a flow through this one function.
pub fn of_text(data: &Path, text: &str) -> Result<Costed, Refused> {
    let committed = config::committed(data).map_err(|e| Refused::Data(e.to_string()))?;
    let compiled = crate::compile_flow_for(text, &committed.config).map_err(Refused::Flow)?;
    let github = crate::github::Gh::new(data.join("logs"));
    let (mut e, database_at) = Engine::trying(
        data,
        committed,
        compiled,
        Box::new(crate::agent::Claude),
        Box::new(github),
    )
    .map_err(Refused::Data)?;
    let cost = e.flow_cost().map_err(Refused::Data)?;
    Ok(Costed { cost, database_at })
}

impl Engine {
    /// What installing the flow this engine runs would do with each work that is neither ended nor done, on an engine
    /// made only to read that runs it ([`Engine::trying`]), read in one transaction, so that a round written meanwhile
    /// does not mix two moments. A paused work is judged as well: it is brought up as it next goes on, and nothing it
    /// carries changes meanwhile.
    pub fn flow_cost(&mut self) -> Result<Cost, String> {
        self.db
            .conn()
            .execute_batch("BEGIN")
            .map_err(|e| format!("reading the database: {e}"))?;
        let cost = self.cost();
        // The read ends however it went; it wrote nothing.
        let _ = self.db.conn().execute_batch("ROLLBACK");
        cost
    }

    /// What installing the flow this engine runs would do with each work ([`Engine::flow_cost`]), within its read.
    fn cost(&mut self) -> Result<Cost, String> {
        let stored: bool = self
            .db
            .conn()
            .query_row(
                "SELECT count(*) > 0 FROM flow_versions WHERE id = ?1",
                [self.flow_now],
                |r| r.get(0),
            )
            .map_err(|e| format!("the flow versions: {e}"))?;
        let ids: Vec<i64> = {
            let mut st = self
                .db
                .conn()
                .prepare("SELECT id FROM works WHERE ended IS NULL ORDER BY id")
                .map_err(|e| e.to_string())?;
            st.query_map([], |r| r.get(0))
                .map_err(|e| e.to_string())?
                .collect::<Result<_, _>>()
                .map_err(|e| e.to_string())?
        };
        let mut works = Vec::new();
        for id in ids {
            let work = Work::load(self.db.conn(), id, None);
            let effect = match &work {
                Ok(_) => self.effect(id).transpose(),
                Err(e) => Some(Err(e.clone())),
            };
            let Some(effect) = effect else { continue };
            let work = work.ok();
            let title = work.as_ref().and_then(|w| w.item).and_then(|item| {
                self.db
                    .conn()
                    .query_row("SELECT title FROM items WHERE id = ?1", [item], |r| r.get(0))
                    .optional()
                    .ok()
                    .flatten()
            });
            works.push(WorkCost {
                id,
                work,
                title,
                effect,
            });
        }
        Ok(Cost {
            version: self.flow_now,
            stored,
            works,
        })
    }

    /// What bringing work `id` to the version the engine runs does with it, decided as [`Engine::bring`] decides it and
    /// as the step of the cell it comes to then goes; none for a work that is done, which is not brought up.
    fn effect(&mut self, id: i64) -> Result<Option<Effect>, String> {
        let w = self.work(id)?;
        if self.done(&w)? {
            return Ok(None);
        }
        if w.flow_version == self.flow_now {
            return Ok(Some(Effect::Stays));
        }
        let (to, why, stuck) = match self.plan(&w)? {
            Plan::Move => return Ok(Some(Effect::Moves)),
            Plan::GoBack { to, why } => (Some(to), why, false),
            Plan::Stuck { why, back_to } => (back_to, why, true),
        };
        let Some(through) = self.version_cell(&w)? else {
            let taken_over = w.graph == ENGINEER && w.item.is_some();
            return Ok(Some(Effect::Ends { why, taken_over }));
        };
        // The work as the engine leaves it at that cell ([`Engine::bring_back`]): on the version it runs, standing there,
        // with the cell it goes back to as where it stopped and no cell waiting for an answer.
        let now = Work {
            flow_version: self.flow_now,
            ..w.clone()
        };
        let g = self.graph_of(&now)?;
        let mut there = Work::load(self.db.conn(), id, Some(&g))?;
        there.flow_version = self.flow_now;
        there.pos.at = through.clone();
        there.pos.stopped = to;
        there.pos.callers = Vec::new();
        let cell = g
            .cell(&through)
            .cloned()
            .ok_or_else(|| format!("work {id}: flow version {} has no cell {through}", self.flow_now))?;
        let a = steps::arrival(self, &there, &g, &cell, w.flow_version, &why, stuck)?;
        let next = g.step(&there.pos, &a.row)?;
        // A row to the graph's end for a work stopped before its end ends the work there at once, as that end's step
        // says why.
        if let Some(end) = g.cancel.as_deref().filter(|c| *c == next.at).and_then(|c| g.cell(c)) {
            let why = match end.step.as_deref() {
                Some(architect::STOP) => architect::stopped_why(self.flow_now, &why),
                _ => why,
            };
            return Ok(Some(Effect::Ends { why, taken_over: false }));
        }
        Ok(Some(match a.pause {
            Some(pause) => Effect::Stuck {
                at: next.at,
                why,
                back_to: a.output["back_to"].as_str().map(str::to_string),
                pause,
            },
            None => Effect::GoesBack {
                through,
                to: next.at,
                why,
                from: a.redone.map(|(r, _)| r),
                redone: a.redone.map_or(0, |(_, n)| n),
            },
        }))
    }
}
