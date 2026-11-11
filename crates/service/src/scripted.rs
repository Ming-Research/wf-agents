//! The test carriers: agents, engine acts and tools that do what a [`Script`] says, each round and each tool taking
//! the time the script gives it, so that the pages show works under way.
//!
//! What they return depends on nothing but the round or the call, so that what was under way when the process stopped
//! is answered again as it would have been. They answer each round and each call once: a round they hold is ended by
//! whichever of its end and its stop comes first, and a call by whichever of its result and its withdrawing. A round
//! the script makes fail they say failed, keeping nothing of it: the round the engine gives again once the work is let
//! on is a new round, which they carry out as the script says of it.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use decide::{CallId, RunId};
use store::WorkNumber;

use crate::carriers::{CallReply, Carriers, Network, Round, RoundReply, Rounds, ToolCall, Tools, WITHDRAWN};
use crate::script::{Ends, Planned, Script};

/// Agents, engine acts and tools that follow a script.
#[derive(Clone)]
pub struct Scripted {
    inner: Arc<Inner>,
}

struct Inner {
    script: Script,
    held: Mutex<Held>,
}

/// What the carriers were given in this process: each round and each call, with its reply while it is not yet
/// answered.
#[derive(Default)]
struct Held {
    rounds: BTreeMap<(WorkNumber, RunId, usize), Option<RoundReply>>,
    calls: BTreeMap<(WorkNumber, CallId), Option<CallReply>>,
}

impl Scripted {
    pub fn new(script: Script) -> Self {
        Scripted {
            inner: Arc::new(Inner {
                script,
                held: Mutex::new(Held::default()),
            }),
        }
    }

    /// The carriers of every kind, all following the script.
    pub fn carriers(&self) -> Carriers {
        Carriers {
            agents: Arc::new(self.clone()),
            acts: Arc::new(self.clone()),
            tools: Arc::new(self.clone()),
            network: Arc::new(self.clone()),
        }
    }

    /// What the round returns: the script's entry for it, as the cell's nth run in its work.
    fn planned(&self, round: &Round) -> Planned {
        // Runs are named in the order they started.
        let started = round.kept.records.started.iter();
        let nth = (started.clone())
            .filter(|s| s.cell == round.cell.id && s.run <= round.run)
            .count();
        let basis = (started.clone().find(|s| s.run == round.run)).map(|s| s.basis.clone());
        let basis = basis.unwrap_or_default();
        self.inner
            .script
            .round(&round.flow, &round.cell, nth, round.number, &basis)
    }
}

impl Inner {
    fn held(&self) -> MutexGuard<'_, Held> {
        // What is held stays whole whatever panicked while it was held: each change is one insert or take.
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Rounds for Scripted {
    fn give(&self, round: Round, reply: RoundReply) {
        let planned = self.planned(&round);
        let key = (round.work, round.run, round.number);
        {
            let mut held = self.inner.held();
            if held.rounds.contains_key(&key) {
                return;
            }
            held.rounds.insert(key, Some(reply));
        }
        let inner = self.inner.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(planned.ms)).await;
            let reply = inner.held().rounds.get_mut(&key).and_then(Option::take);
            if let Some(reply) = reply {
                end(reply, planned);
            }
        });
    }

    fn stop(&self, round: Round, reply: RoundReply) {
        let planned = self.planned(&round);
        let key = (round.work, round.run, round.number);
        let reply = {
            let mut held = self.inner.held();
            match held.rounds.get_mut(&key) {
                Some(slot) => slot.take(),
                // Given before the process started again: its end is this.
                None => {
                    held.rounds.insert(key, None);
                    Some(reply)
                }
            }
        };
        if let Some(reply) = reply {
            end(reply, planned);
        }
    }
}

/// Ends the round as the script plans it: what comes back, or its failure.
fn end(reply: RoundReply, planned: Planned) {
    match planned.ends {
        Ends::Came(came) => reply.ended(came, planned.spent),
        Ends::Fails { why, failure, resets } => reply.failed_as(failure, resets, why, planned.spent),
    }
}

/// The network a script runs on is always reached.
impl Network for Scripted {
    fn reached(&self) -> bool {
        true
    }
}

impl Tools for Scripted {
    fn call(&self, call: ToolCall, reply: CallReply) {
        let (answer, ms) = self.inner.script.tool(&call.name, &call.input);
        let key = (call.work, call.call);
        {
            let mut held = self.inner.held();
            if held.calls.contains_key(&key) {
                return;
            }
            held.calls.insert(key, Some(reply));
        }
        let inner = self.inner.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(ms)).await;
            let reply = inner.held().calls.get_mut(&key).and_then(Option::take);
            if let Some(reply) = reply {
                reply.done(answer);
            }
        });
    }

    fn withdraw(&self, call: ToolCall, reply: CallReply) {
        let key = (call.work, call.call);
        let reply = {
            let mut held = self.inner.held();
            match held.calls.get_mut(&key) {
                Some(slot) => slot.take(),
                None => {
                    held.calls.insert(key, None);
                    Some(reply)
                }
            }
        };
        if let Some(reply) = reply {
            reply.done(WITHDRAWN.to_string());
        }
    }
}
