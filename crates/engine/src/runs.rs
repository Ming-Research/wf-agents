//! The engine's loop as a whole: its runs, each recorded from its start to its stop (`engine_runs`), and the faults it
//! meets. A fault of one work pauses that work with why, so that the others go on; a fault that belongs to no work is
//! said and told to the owner once, and the loop's next turn tries again. Nothing in the loop ends it but the stop file.

use db::Actor;

use crate::work::Work;
use crate::{Engine, VERSION, actions, lock};

impl Engine {
    /// Records that the engine's loop starts: the commit it was built from, the configuration commit it starts with and
    /// the flow version of its build. A run found never completed is marked lost first. Gives the run's id, for
    /// [`Engine::record_stop`].
    pub fn record_start(&mut self) -> Result<i64, String> {
        let commit = self.config.commit.clone();
        let flow = self.flow_now;
        self.db
            .write(Actor::Engine, |tx| {
                db::start_engine_run(tx, db::now(), VERSION, &commit, flow)
            })
            .map_err(|e| e.to_string())
    }

    /// Records that run `run` of the loop stopped, and why: `stop file`, or `error: ` and the error that ended it.
    pub fn record_stop(&mut self, run: i64, reason: &str) -> Result<(), String> {
        self.db
            .write(Actor::Engine, |tx| db::stop_engine_run(tx, run, db::now(), reason))
            .map_err(|e| e.to_string())
    }

    /// Reads a work for the loop. A work that cannot be read is paused with why ([`Engine::fault_work`]), and the loop
    /// goes on without it.
    pub(crate) fn work_or_fault(&mut self, id: i64) -> Option<Work> {
        let read = self.work(id);
        self.or_fault(id, read)
    }

    /// What the loop did for work `id` as `done` says it, or none when it failed: the work is then paused with why
    /// ([`Engine::fault_work`]), and the loop goes on without it.
    pub(crate) fn or_fault<T>(&mut self, id: i64, done: Result<T, String>) -> Option<T> {
        match done {
            Ok(t) => Some(t),
            Err(x) => {
                self.fault_work(id, &format!("the engine's loop failed on it: {x}"));
                None
            }
        }
    }

    /// Pauses a work the loop failed on, or whose step failed in a way its own step did not deal with, with `why`, so
    /// that the other works go on and the owner sees the reason. A work already ended or paused is left as it is, with
    /// the reason it has, and nothing is said of it: the loop meets such a work at every turn.
    pub(crate) fn fault_work(&mut self, id: i64, why: &str) {
        let standing: Result<(Option<String>, Option<String>), _> =
            self.db
                .conn()
                .query_row("SELECT ended, paused FROM works WHERE id = ?1", [id], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                });
        match standing {
            Ok((None, None)) => {}
            Ok(_) => return,
            Err(x) => {
                self.say(&format!("work {id} is not paused: its row is not read: {x}"));
                return;
            }
        }
        self.say(&format!("work {id} paused: {why}"));
        if let Err(x) = self.pause(id, why) {
            self.say(&format!("work {id} is not paused: {x}"));
        }
    }

    /// What a part of the loop that belongs to no work did, `what` it was doing, or none when it failed. The failure is
    /// said, and the owner is told of it once for as long as the engine runs (a notice about no work): the loop's next
    /// turn tries again, and its other parts go on.
    pub fn faulted<T>(&mut self, what: &str, done: Result<T, String>) -> Option<T> {
        match done {
            Ok(t) => Some(t),
            Err(x) => {
                let text = format!("The engine's loop failed while {what}; it tries again at its next turn: {x}");
                self.say(&text);
                let first = lock(&self.shared.faults_told).insert(text.clone());
                if first
                    && let Err(y) = self
                        .db
                        .write(Actor::Engine, |tx| {
                            actions::notify(tx, None, &text).map_err(db::Error::Abort)
                        })
                        .map_err(|e| e.to_string())
                {
                    self.say(&format!("the owner is not told of it: {y}"));
                }
                None
            }
        }
    }
}
