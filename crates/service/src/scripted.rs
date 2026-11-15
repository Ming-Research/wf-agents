//! The test carriers: agents, engine acts and tools that do what a [`Script`] says, each round and each tool taking
//! the time the script gives it, and saying as it goes where it stands where the script says so (the turn an act waits
//! for, the CI run it runs as), so that the pages show works under way.
//!
//! What they return depends on nothing but the round or the call, so that what was under way when the process stopped
//! is answered again as it would have been. They answer each round and each call once: a round they hold is ended by
//! whichever of its end and its stop comes first, and a call by whichever of its result and its withdrawing. A round
//! the script makes fail they say failed, keeping nothing of it: the round the engine gives again once the work is let
//! on is a new round, which they carry out as the script says of it.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use decide::{CallId, RunId};
use driver::{Along, Found};
use store::WorkNumber;

use crate::carriers::{CallReply, Carriers, Network, Outside, Round, RoundReply, Rounds, ToolCall, Tools, WITHDRAWN};
use crate::script::{Ends, Planned, Script};
use work::Handed;

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

    /// The carriers of every kind, all following the script, with as many slots as it says.
    pub fn carriers(&self) -> Carriers {
        Carriers {
            agents: Arc::new(self.clone()),
            acts: Arc::new(self.clone()),
            tools: Arc::new(self.clone()),
            slots: self.inner.script.slots.unwrap_or(crate::script::SLOTS),
            outside: Arc::new(self.clone()),
            host: Some(Arc::new(self.clone())),
        }
    }

    /// What the round returns: the script's entry for it, as the cell's nth run in its work; a round of the act that
    /// runs the tests hands in besides the result of each entry it gives ([`tested`]).
    fn planned(&self, round: &Round) -> Planned {
        // Runs are named in the order they started.
        let started = round.kept.records.started.iter();
        let nth = (started.clone())
            .filter(|s| s.cell == round.cell.id && s.run <= round.run)
            .count();
        let basis = (started.clone().find(|s| s.run == round.run)).map(|s| s.basis.clone());
        let basis = basis.unwrap_or_default();
        let mut planned = self
            .inner
            .script
            .round(&round.flow, &round.cell, nth, round.number, &basis);
        if let Ends::Came(came) = &mut planned.ends {
            came.writes.extend(tested(round));
        }
        planned
    }
}

impl Inner {
    fn held(&self) -> MutexGuard<'_, Held> {
        // What is held stays whole whatever panicked while it was held: each change is one insert or take.
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// What the act that runs the tests hands in as its round ends: of each entry it gives, its cell's block of the result,
/// saying the tests passed; nothing of a round of any other cell.
fn tested(round: &Round) -> Vec<Handed> {
    if round.cell.act.as_deref() != Some(flow::RUNS_TESTS) {
        return Vec::new();
    }
    (decide::asked_of(&round.flow.flow, &round.kept.records, &round.cell.id).into_iter())
        .map(|entry| Handed {
            content: format!("The tests {} asked for passed.", entry.cell),
            block: entry.result,
            matters: true,
            value: None,
        })
        .collect()
}

impl Rounds for Scripted {
    fn give(&self, round: Round, reply: RoundReply) {
        let planned = self.planned(&round);
        let key = (round.work, round.run, round.number);
        let saying = reply.saying(round.number);
        {
            let mut held = self.inner.held();
            if held.rounds.contains_key(&key) {
                return;
            }
            held.rounds.insert(key, Some(reply));
        }
        let inner = self.inner.clone();
        tokio::spawn(async move {
            // Where it stands as it goes: waiting for the turn for the first half of its time, then the CI run.
            let mut waited = 0;
            if let Some(holder) = planned.turn {
                saying.say(Some(Along::Turn {
                    holder: WorkNumber(holder),
                }));
                waited = planned.ms / 2;
                tokio::time::sleep(Duration::from_millis(waited)).await;
                saying.say(None);
            }
            if let Some(at) = &planned.at {
                saying.say(Some(Along::Runs { at: at.clone() }));
            }
            tokio::time::sleep(Duration::from_millis(planned.ms - waited)).await;
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

/// The outside of the agents' rounds a script runs on is always well: what the script makes a round fail of passes
/// once the engine checks it.
impl Outside for Scripted {
    fn check(&self) -> Pin<Box<dyn Future<Output = Found> + Send + '_>> {
        Box::pin(async { Found::Well })
    }
}

impl Tools for Scripted {
    fn call(&self, call: ToolCall, reply: CallReply) {
        let (answer, ms) = self.inner.script.tool(&call.name, &call.input);
        let at = (self.inner.script.tools.get(&call.name)).and_then(|tool| tool.at.clone());
        let key = (call.work, call.call);
        let saying = reply.saying();
        {
            let mut held = self.inner.held();
            if held.calls.contains_key(&key) {
                return;
            }
            held.calls.insert(key, Some(reply));
        }
        let inner = self.inner.clone();
        tokio::spawn(async move {
            if let Some(at) = at {
                saying.say(Some(Along::Runs { at }));
            }
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
