//! The test carriers: agents, engine acts and tools that do what a [`Script`] says, each round and each tool taking
//! the time the script gives it, and saying as it goes where it stands where the script says so (the turn an act waits
//! for, the CI run it runs as), so that the pages show works under way; and the check of the agents' outside, which
//! finds what the script says. The questions a round asks are read in the words beside the flows, as a real return's
//! are, and what a round read is written where it differs from what its block says, as the real carriers write it.
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

use decide::{BlockId, CallId, RunId};
use driver::{Along, Found};
use flow::agents::Agents;
use flow::{CellSpec, Compiled, Kind};
use store::WorkNumber;

use crate::carriers::{CallReply, Carriers, Network, Outside, Round, RoundReply, Rounds, ToolCall, Tools, WITHDRAWN};
use crate::script::{CHECK, Ends, MAIN, Planned, Read, Script, ToolAsk, nth};
use work::{Call, Handed, Work};

/// Agents, engine acts and tools that follow a script.
#[derive(Clone)]
pub struct Scripted {
    inner: Arc<Inner>,
}

struct Inner {
    script: Script,
    /// What is written beside the flows, in whose words the questions a round asks are read.
    words: Agents,
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
    /// The carriers that follow the script, reading the questions its rounds ask in these words beside the flows.
    pub fn new(script: Script, words: Agents) -> Self {
        Scripted {
            inner: Arc::new(Inner {
                script,
                words,
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
    /// runs the tests hands in besides the result of each entry it gives ([`tested`]). What the engine asks as it ends
    /// is named as the words beside the flows name the engine's question, and what is wrong with the questions it asks
    /// is found with it, and what it read is kept only where it differs from what its block says; an act's
    /// round reads what its entry says, or, where it says nothing, what the act reads by itself ([`taken_in`]).
    fn planned(&self, round: &Round) -> Planned {
        let nth = nth(&round.kept, round.run);
        let started = round.kept.records.started.iter();
        let basis = (started.clone().find(|s| s.run == round.run)).map(|s| s.basis.clone());
        let basis = basis.unwrap_or_default();
        let mut planned = (self.inner.script).round(&round.flow, &round.cell, round.work.0, nth, round.number, &basis);
        let new = |block: &BlockId, content: &str| says_now(&round.kept, block) != Some(content);
        let entry = (self.inner.script).entry(
            &round.flow.name,
            round.cell.id.as_str(),
            round.work.0,
            nth,
            round.number,
        );
        if let Ends::Came(came) = &mut planned.ends {
            came.writes.extend(tested(round));
            // What the engine asks as the round ends is its own question, named as the words beside the flows name it.
            let protected = &self.inner.words.asks.protected.name;
            let its_own = entry.and_then(|entry| entry.engine.as_ref()).into_iter().flatten();
            came.engine = its_own.map(|question| question.question(protected)).collect();
            came.found
                .extend(engine::asked::unmet(&came.said.questions, &self.inner.words));
            came.read.retain(|read| new(&read.block, &read.content));
        }
        let reads = (planned.reads.take()).unwrap_or_else(|| {
            let taken = taken_in(&round.flow, &round.kept, &round.cell, nth, round.number);
            taken.into_iter().collect()
        });
        // A version for the round is put in where the block says it already, and none written; one from outside is
        // not written again.
        planned.reads = Some(
            reads
                .into_iter()
                .filter(|r| r.for_round || new(&r.block, &r.content))
                .collect(),
        );
        planned
    }

    /// The ask of the round that made the call: the call's run's last round, whose return asked for the run's calls in
    /// the order it asked for them; none where the script says nothing of that round or of the call.
    fn asked(&self, call: &ToolCall) -> Option<&ToolAsk> {
        let records = &call.kept.records;
        let started = records.started.iter().find(|s| s.run == call.run)?;
        let number = records.rounds.iter().filter(|r| r.run == call.run).count();
        let made = (records.called.iter())
            .filter(|c| c.run == call.run && matches!(call.kept.calls.get(&c.call), Some(Call::Asked(_))));
        let i = made.map(|c| c.call).position(|made| made == call.call)?;
        let nth = nth(&call.kept, call.run);
        let entry = (self.inner.script).entry(&call.flow.name, started.cell.as_str(), call.work.0, nth, number)?;
        entry.asks.as_ref()?.get(i)
    }
}

/// What the latest version of the block says, where it has one.
fn says_now<'a>(kept: &'a Work, block: &BlockId) -> Option<&'a str> {
    let latest = decide::versions(&kept.records).get(block)?.last()?.at;
    kept.contents.get(&(block.clone(), latest)).map(String::as_str)
}

/// What the act that checks the work reads by itself for this round of the nth run of its cell, the work as it
/// stands: main as the work's block of main holds it now, which the round is told it takes in, as the real check tells
/// its round the latest main it takes in (decision the-check-tells-its-round-the-main-it-takes-in), so that a main
/// written since its run began runs it no more; the version is put into the round, and none is written. Or, where that
/// block holds no version, one made up for the round. Nothing of another act's round, or of an agent's. A build of the
/// page's states that plays an explored path again (crate `wf-agents`) reads the check's round by it too.
pub fn taken_in(flow: &Compiled, kept: &Work, cell: &CellSpec, nth: usize, number: usize) -> Option<Read> {
    if cell.act.as_deref() != Some(CHECK) {
        return None;
    }
    let (block, _) = (flow.blocks.iter()).find(|(_, b)| b.writers.outside.as_deref() == Some(MAIN))?;
    let content = match says_now(kept, block) {
        Some(latest) => latest.to_string(),
        None => format!("The main as {} took it in in its run {nth}, round {number}.", cell.id),
    };
    Some(Read {
        block: block.clone(),
        content,
        for_round: true,
    })
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
        let teller = reply.teller();
        {
            let mut held = self.inner.held();
            if held.rounds.contains_key(&key) {
                return;
            }
            held.rounds.insert(key, Some(reply));
        }
        // What an act read is written as its round begins, each as the script says: for the round, or from outside;
        // what a round of a line that only reads read, from outside as it ends, before what comes back.
        let reads = planned.reads.clone().unwrap_or_default();
        let (ending, beginning): (Vec<Read>, Vec<Read>) =
            reads.into_iter().partition(|_| round.cell.kind == Kind::Agent);
        let write = move |reads: Vec<Read>| {
            for Read {
                block,
                content,
                for_round,
            } in reads
            {
                match for_round {
                    true => teller.outside_wrote_for_round(block, content),
                    false => teller.outside_wrote(block, content),
                }
            }
        };
        write(beginning);
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
                write(ending);
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

/// The outside of the agents' rounds a script runs on is what the script says the engine's check of it finds, and else
/// well: what the script makes a round fail of passes once the engine checks it, unless the script says the check finds
/// it down.
impl Outside for Scripted {
    fn check(&self, _: Option<&str>) -> Pin<Box<dyn Future<Output = Found> + Send + '_>> {
        let script = &self.inner.script;
        let found = match &script.outside.finds {
            None => Found::Well,
            Some(down) => Found::Down {
                failure: down.failure,
                why: down.why.clone(),
                resets: down.resets.map(|ms| script.after_start(ms)),
            },
        };
        Box::pin(async move { found })
    }
}

impl Tools for Scripted {
    fn call(&self, call: ToolCall, reply: CallReply) {
        let (answer, ms) = (self.inner.script).tool(&call.name, &call.input, self.asked(&call));
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
