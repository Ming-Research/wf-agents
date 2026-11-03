//! What installing a flow would do with each work that is neither ended nor done, worked out before it is installed, so
//! that whoever designs, rules on or installs a flow change sees what it costs the works that go on. On an engine made
//! only to read that runs the flow ([`Engine::trying`]), each work is judged by the very decisions the engine makes as it
//! brings a work on an older version up when the work next goes on ([`Engine::plan`], [`Engine::version_cell`]): it
//! moves where it stands, goes back through its graph's cell for a work on an older version to a cell it does again,
//! stays at that cell stuck, or ends. Nothing is written.

use db::rusqlite::OptionalExtension;

use crate::versions::{self, Plan};
use crate::work::{self, RoundId, Work};
use crate::{ENGINEER, Engine};

/// What installing a flow version does with a work when the work next goes on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// The work is on that version already: nothing.
    Stays,
    /// It moves where it stands: the same work at the same cell, in the same sessions.
    Moves,
    /// It comes to cell `through` and goes back from there to cell `to`, which it does again from its start, for the
    /// reason `why`: round `from` and the rounds after it, `redone` in all, are done again.
    GoesBack {
        through: String,
        to: String,
        why: String,
        from: Option<RoundId>,
        redone: usize,
    },
    /// It comes to cell `at` and stays there, paused, for the reason `why`: the owner's Start takes it back to cell
    /// `back_to` all the same, or, with none, looks again.
    Stuck {
        at: String,
        why: String,
        back_to: Option<String>,
    },
    /// It ends, as no cell of the version can run it, for the reason `why`; a new work takes its item over when it is
    /// an engineer's work on an item (`taken_over`).
    Ends { why: String, taken_over: bool },
}

/// A work that is neither ended nor done, with what installing the flow does with it.
#[derive(Debug, Clone)]
pub struct WorkCost {
    pub work: Work,
    /// The title of its item, for a work on one.
    pub title: Option<String>,
    /// What installing the flow does with it, or why that cannot be worked out, which the engine meets too as it
    /// brings the work up.
    pub effect: Result<Effect, String>,
}

/// What installing a flow would do: the version it would be stored as, whether the database has that version already,
/// and each work that is neither ended nor done, by id, those paused included.
#[derive(Debug, Clone)]
pub struct Cost {
    pub version: i64,
    pub stored: bool,
    pub works: Vec<WorkCost>,
}

impl Engine {
    /// What installing the flow this engine runs would do with each work that is neither ended nor done, on an engine
    /// made only to read that runs it ([`Engine::trying`]). A paused work is judged as well: it is brought up as it next
    /// goes on, and nothing it carries changes meanwhile.
    pub fn flow_cost(&mut self) -> Result<Cost, String> {
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
            let w = Work::load(self.db.conn(), id, None)?;
            let Some(effect) = self.effect(id).transpose() else {
                continue;
            };
            let title = match w.item {
                Some(item) => self
                    .db
                    .conn()
                    .query_row("SELECT title FROM items WHERE id = ?1", [item], |r| r.get(0))
                    .optional()
                    .map_err(|e| format!("item {item}: {e}"))?,
                None => None,
            };
            works.push(WorkCost { work: w, title, effect });
        }
        Ok(Cost {
            version: self.flow_now,
            stored,
            works,
        })
    }

    /// What bringing work `id` to the version the engine runs does with it, decided as [`Engine::bring`] decides it;
    /// none for a work that is done, which is not brought up.
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
        let effect = match to {
            Some(to) if !stuck => {
                // The rounds it does again, as the cell it comes to records them ([`crate::steps::bring_up`]).
                let now = Work {
                    flow_version: self.flow_now,
                    ..w.clone()
                };
                let g = self.graph_of(&now)?;
                let rounds = work::rounds(self.db.conn(), id)?;
                let first = versions::redone_from(&g, &rounds, &to);
                Effect::GoesBack {
                    through,
                    to,
                    why,
                    from: rounds.get(first).map(|r| r.id),
                    redone: rounds.len() - first,
                }
            }
            back_to => Effect::Stuck {
                at: through,
                why,
                back_to,
            },
        };
        Ok(Some(effect))
    }
}
