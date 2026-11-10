//! The loop: one thing that happened is added to the work's records, and the deciding code is asked what follows
//! until nothing more does.

use std::collections::BTreeMap;

use decide::run::{self, Requires, Step, Unmet};
use decide::{
    Abandoned, BlockId, CallId, Called, Decision, Done, Finished, LetGo, Outsider, Pause, Returned, Round, RunId,
    Started, Tests, Who, Write, Written, decide,
};
use flow::Compiled;

use crate::event::{Came, Effect, Event, Refused};
use crate::work::{Ask, Back, Call, Given, Work};

/// Takes one thing that happened to the work, and gives what is to be done for it outside the engine.
pub fn handle(flow: &Compiled, tests: &dyn Tests, work: &mut Work, event: Event) -> Result<Vec<Effect>, Refused> {
    let mut effects = Vec::new();
    take(flow, work, event)?;
    advance(flow, tests, work, &mut effects);
    Ok(effects)
}

/// What was to be done for the work and is not yet known as done: the round under way, and the calls that have no
/// result. A driver that starts anew asks for these again.
pub fn pending(work: &Work) -> Vec<Effect> {
    let mut effects = Vec::new();
    for started in &work.records.started {
        let run = started.run;
        if !open(work, run) {
            continue;
        }
        let number = began(work, run);
        if number > ended(work, run)
            && let Some(given) = work.given.get(&run).and_then(|g| g.last())
        {
            effects.push(Effect::Round {
                run,
                cell: started.cell.clone(),
                number,
                given: given.clone(),
            });
        }
        for call in open_calls(work, run) {
            if let Some(what) = work.calls.get(&call) {
                effects.push(Effect::Call {
                    call,
                    run,
                    what: what.clone(),
                });
            }
        }
    }
    effects
}

/// Adds what happened to the records, or refuses what cannot have happened to this work.
fn take(flow: &Compiled, work: &mut Work, event: Event) -> Result<(), Refused> {
    match event {
        Event::Look => {}
        Event::Wrote {
            block,
            by,
            content,
            matters,
            value,
        } => {
            let Some(declared) = flow.blocks.get(&block) else {
                return Err(Refused::NoSuchBlock(block));
            };
            let theirs = match by {
                Outsider::Person => declared.writers.people,
                Outsider::Outside => declared.writers.outside,
            };
            if !theirs {
                return Err(Refused::NotTheirs { block, by });
            }
            match (&declared.values, &value) {
                (Some(_), None) => return Err(Refused::Unmet(Unmet::NoValue { block })),
                (Some(values), Some(v)) if !values.contains(v) => {
                    let value = v.clone();
                    return Err(Refused::Unmet(Unmet::NotAValue { block, value }));
                }
                (None, Some(_)) => return Err(Refused::Unmet(Unmet::NotAChoice { block })),
                _ => {}
            }
            let at = work.tick();
            work.contents.insert((block.clone(), at), content);
            work.records.written.push(Written {
                block,
                by,
                matters,
                value,
                at,
            });
        }
        Event::LetGo { cell } => {
            if flow.flow.place(&cell).is_none() {
                return Err(Refused::NoSuchCell(cell));
            }
            let at = work.tick();
            work.records.let_go.push(LetGo { cell, at });
        }
        Event::Paused | Event::Resumed => {
            let at = work.tick();
            let paused = event == Event::Paused;
            work.records.pauses.push(Pause { paused, at });
        }
        Event::Cancelled => {
            if work.records.cancelled.is_none() {
                work.records.cancelled = Some(work.tick());
            }
        }
        Event::Returned { run, came } => {
            if !open(work, run) || began(work, run) == ended(work, run) {
                return Err(Refused::NoRoundUnderWay(run));
            }
            let back = match came {
                Came::HandsIn(handed) => Back::HandedIn(handed),
                Came::Asks(asks) => Back::Asked {
                    asks,
                    calls: Vec::new(),
                },
            };
            let at = work.tick();
            work.records.returned.push(Returned { run, at });
            work.back.entry(run).or_default().push(back);
        }
        Event::Done { call, result } => {
            let Some(called) = work.records.called.iter().find(|c| c.call == call) else {
                return Err(Refused::NoSuchCall(call));
            };
            if work.records.done.iter().any(|d| d.call == call) {
                return Err(Refused::AlreadyDone(call));
            }
            if !open(work, called.run) {
                return Err(Refused::Withdrawn(call));
            }
            let at = work.tick();
            work.records.done.push(Done { call, at });
            work.results.insert(call, result);
        }
    }
    Ok(())
}

/// Asks the deciding code what follows, and does it, until the work has a round under way, waits or has ended.
fn advance(flow: &Compiled, tests: &dyn Tests, work: &mut Work, effects: &mut Vec<Effect>) {
    loop {
        let run = match decide(&flow.flow, &work.records, tests) {
            Decision::Run { cell, because, basis } => {
                let run = RunId(work.name());
                let at = work.tick();
                work.records.started.push(Started {
                    run,
                    cell,
                    at,
                    because,
                    basis,
                });
                run
            }
            Decision::GoOn { run, .. } => run,
            Decision::Abandon { run, .. } => {
                for call in open_calls(work, run) {
                    effects.push(Effect::Withdraw { call });
                }
                let at = work.tick();
                work.records.abandoned.push(Abandoned { run, at });
                continue;
            }
            Decision::Fault(_)
            | Decision::Ended(_)
            | Decision::Busy { .. }
            | Decision::Paused
            | Decision::Wait { .. } => {
                return;
            }
        };
        if !step(flow, work, run, effects) {
            return;
        }
    }
}

/// Does what the run does next. Says whether the deciding code is to be asked again: it is not once a round of the
/// run is under way.
fn step(flow: &Compiled, work: &mut Work, run: RunId, effects: &mut Vec<Effect>) -> bool {
    let choices = choices(flow);
    let Some(requires) = requires(flow, &choices, work, run) else {
        return false;
    };
    let Some(started) = work.records.started.iter().find(|s| s.run == run) else {
        return false;
    };
    let cell = started.cell.clone();
    let given = match run::next(&log(work, run), requires) {
        Step::Begin => Given::Task {
            because: started.because.clone(),
            basis: started.basis.clone(),
        },
        Step::Resume => match work.back.get(&run).and_then(|b| b.last()) {
            Some(Back::Asked { calls, .. }) => Given::Results(calls.clone()),
            _ => return false,
        },
        Step::SendBack { unmet } => Given::SentBack(unmet),
        Step::Make => {
            let Some(Back::Asked { asks, .. }) = work.back.get(&run).and_then(|b| b.last()) else {
                return false;
            };
            let made: Vec<CallId> = (asks.clone().into_iter())
                .map(|ask| {
                    let on = match &ask {
                        Ask::Person { who, .. } => Some(who.clone()),
                        Ask::Tool { .. } => None,
                    };
                    call(work, run, on, Call::Asked(ask), effects)
                })
                .collect();
            if let Some(Back::Asked { calls, .. }) = work.back.get_mut(&run).and_then(|b| b.last_mut()) {
                *calls = made;
            }
            return true;
        }
        Step::Finish { writes } => {
            let at = work.tick();
            if let Some(Back::HandedIn(handed)) = work.back.get(&run).and_then(|b| b.last()) {
                for h in handed {
                    work.contents.insert((h.block.clone(), at), h.content.clone());
                }
            }
            work.records.finished.push(Finished { run, at, writes });
            return true;
        }
        Step::AtLimit(limit) => {
            call(work, run, Some(Who::Owner), Call::AtLimit(limit), effects);
            return true;
        }
        Step::Rest { .. } | Step::UnderWay => return false,
    };
    let at = work.tick();
    work.records.rounds.push(Round { run, at });
    work.given.entry(run).or_default().push(given.clone());
    effects.push(Effect::Round {
        run,
        cell,
        number: began(work, run),
        given,
    });
    false
}

/// Records a call of the run, and gives it to be made.
fn call(work: &mut Work, run: RunId, on: Option<Who>, what: Call, effects: &mut Vec<Effect>) -> CallId {
    let call = CallId(work.name());
    let at = work.tick();
    work.records.called.push(Called { call, run, on, at });
    work.calls.insert(call, what.clone());
    effects.push(Effect::Call { call, run, what });
    call
}

/// The values of each block of the flow that is a choice.
fn choices(flow: &Compiled) -> BTreeMap<BlockId, Vec<String>> {
    (flow.blocks.iter())
        .filter_map(|(block, declared)| declared.values.clone().map(|values| (block.clone(), values)))
        .collect()
}

/// What the flow requires of what the run's cell hands in; none where the run's cell is not in the flow.
fn requires<'a>(
    flow: &'a Compiled,
    choices: &'a BTreeMap<BlockId, Vec<String>>,
    work: &Work,
    run: RunId,
) -> Option<Requires<'a>> {
    let started = work.records.started.iter().find(|s| s.run == run)?;
    let place = flow.flow.place(&started.cell)?;
    let spec = flow.cells.get(place)?;
    Some(Requires {
        outputs: &flow.flow.cells()[place].outputs,
        choices,
        returns: spec.returns,
        calls: spec.calls,
    })
}

/// The run so far, as the deciding code reads it.
fn log(work: &Work, run: RunId) -> run::Log {
    let back = work.back.get(&run).map(Vec::as_slice).unwrap_or_default();
    let returned = |b: &Back| match b {
        Back::Asked { asks, calls } => run::Return::Asked {
            asked: asks.len(),
            made: (calls.iter())
                .map(|&call| run::Call {
                    call,
                    done: done(work, call).is_some(),
                })
                .collect(),
        },
        Back::HandedIn(handed) => run::Return::HandedIn(
            (handed.iter())
                .map(|h| Write {
                    block: h.block.clone(),
                    matters: h.matters,
                    value: h.value.clone(),
                })
                .collect(),
        ),
    };
    let rounds = (0..began(work, run))
        .map(|i| run::Round {
            returned: back.get(i).map(returned),
        })
        .collect();
    // How many rounds had ended when the owner last let the run go on at a limit.
    let let_go = (work.records.called.iter())
        .filter(|c| c.run == run && matches!(work.calls.get(&c.call), Some(Call::AtLimit(_))))
        .filter_map(|c| done(work, c.call))
        .max()
        .map_or(0, |at| {
            (work.records.returned.iter())
                .filter(|r| r.run == run && r.at < at)
                .count()
        });
    run::Log { rounds, let_go }
}

/// Whether the run has neither finished nor been abandoned.
fn open(work: &Work, run: RunId) -> bool {
    let r = &work.records;
    r.started.iter().any(|s| s.run == run)
        && !r.finished.iter().any(|f| f.run == run)
        && !r.abandoned.iter().any(|a| a.run == run)
}

/// How many rounds of the run began.
fn began(work: &Work, run: RunId) -> usize {
    work.records.rounds.iter().filter(|r| r.run == run).count()
}

/// How many rounds of the run ended.
fn ended(work: &Work, run: RunId) -> usize {
    work.records.returned.iter().filter(|r| r.run == run).count()
}

/// When the call had its result, if it has one.
fn done(work: &Work, call: CallId) -> Option<decide::Seq> {
    work.records.done.iter().find(|d| d.call == call).map(|d| d.at)
}

/// The calls of the run that have no result.
fn open_calls(work: &Work, run: RunId) -> Vec<CallId> {
    (work.records.called.iter())
        .filter(|c| c.run == run && done(work, c.call).is_none())
        .map(|c| c.call)
        .collect()
}
