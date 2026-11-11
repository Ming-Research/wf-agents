//! The loop: one thing that happened is added to the work's records, and the deciding code is asked what follows
//! until nothing more does.

use std::collections::BTreeMap;

use decide::run::{self, Failure, Pausing, Requires, Step, Unmet};
use decide::{
    Abandoned, BlockId, CallId, Called, CellId, Decision, Done, End, Finished, LetGo, Outsider, Pause, Pauser,
    Returned, Round, RunId, Seq, Started, Who, Write, Written, decide, hold, stale,
};
use flow::{Compiled, Kind};

use crate::event::{Came, Effect, Event, Gate, Refused};
use work::{Ask, Back, Call, Given, Moment, Ran, Session, Time, Work};

/// Takes one thing that happened to the work, at the time the driver's clock gives, and gives what is to be done for
/// it outside the engine, the agents' rounds being free to begin. Where the records cannot be those of a work nothing
/// is given: the deciding code says so when asked.
pub fn handle(flow: &Compiled, work: &mut Work, now: Time, event: Event) -> Result<Vec<Effect>, Refused> {
    handle_with(flow, work, now, event, Gate::Open)
}

/// Takes one thing that happened to the work as [`handle`] does, the rounds of every work being free to begin or held
/// as the gate says: no run of a cell whose rounds it holds starts, no round of one begins, and everything else goes
/// on.
pub fn handle_with(
    flow: &Compiled,
    work: &mut Work,
    now: Time,
    event: Event,
    gate: Gate,
) -> Result<Vec<Effect>, Refused> {
    handle_then(flow, work, now, event, gate, &|_, _| Vec::new())
}

/// Takes one thing that happened to the work as [`handle_with`] does, and then, before the deciding code is asked what
/// follows, what `then` says follows from it: what someone who writes from outside the work, reading the work as it
/// now is, adds in the same moment, so that the walk decides with it.
pub fn handle_then(
    flow: &Compiled,
    work: &mut Work,
    now: Time,
    event: Event,
    gate: Gate,
    then: &dyn Fn(&Compiled, &Work) -> Vec<Event>,
) -> Result<Vec<Effect>, Refused> {
    let returned = match &event {
        Event::Returned { run, .. } => Some(*run),
        _ => None,
    };
    let from = work.tick();
    let releasing = release(flow, work).is_some();
    take(flow, work, event)?;
    // What a round handed in is taken as the round ends: where it meets everything the flow requires and the run is
    // not stale, the run finishes here, before the walk, so that no cell before it comes between.
    if let Some(run) = returned
        && open(work, run)
        && stale(&flow.flow, &work.records, run).is_none()
    {
        let choices = choices(flow);
        if let Some(requires) = requires(flow, &choices, work, run)
            && let Step::Finish { writes } = run::next(&log(work, run), requires, hold(&flow.flow, &work.records, run))
        {
            finish(work, run, writes);
        }
    }
    for event in then(flow, work) {
        take(flow, work, event)?;
    }
    let mut effects = Vec::new();
    let at = At { from, now, gate };
    advance(flow, work, at, &mut effects);
    // The work's end lets go of what it holds outside once, as the last of it comes to rest.
    if !releasing && let Some(end) = release(flow, work) {
        effects.push(Effect::Release { end });
    }
    // Everything added for this thing that happened was added now.
    if work.tick() > from {
        work.moments.push(Moment { from, at: now });
    }
    Ok(effects)
}

/// What was to be done for the work and is not yet known as done. Of a run that goes on: its round under way, and
/// its calls that have no result. Of a run that was abandoned: the stopping of its round under way, and the
/// withdrawing of its calls that have no result. Of a work that has ended with nothing under way: letting go of what
/// it holds outside, until it is said to be let go. A driver that starts anew asks for these again.
pub fn pending(flow: &Compiled, work: &Work) -> Vec<Effect> {
    let mut effects = Vec::new();
    for started in &work.records.started {
        let run = started.run;
        let number = began(work, run);
        let under_way = number > ended(work, run);
        if open(work, run) {
            if under_way && let Some(given) = work.given.get(&(run, number)) {
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
        } else {
            if under_way {
                effects.push(Effect::Stop { run });
            }
            effects.extend(open_calls(work, run).into_iter().map(|call| Effect::Withdraw { call }));
        }
    }
    effects.extend(release(flow, work).map(|end| Effect::Release { end }));
    effects
}

/// How the work ended, where it has and what it holds outside is to be let go: nothing of it is under way, no round
/// and no call, since a round or a tool may be working in its worktree; it is not let go already; and no failure to
/// let go waits for the owner, as the engine's pause of the work after its end that the owner has not let on from.
fn release(flow: &Compiled, work: &Work) -> Option<End> {
    let r = &work.records;
    if r.released.is_some() {
        return None;
    }
    let Decision::Ended(end) = decide(&flow.flow, r) else {
        return None;
    };
    // Where the work ended: its cancel, or its merge's finish, the last of all.
    let end_at = match end {
        End::Cancelled => r.cancelled,
        End::Merged => r.finished.last().map(|f| f.at),
    };
    let failed =
        (r.pauses.iter()).rposition(|p| p.paused && p.by == Pauser::Engine && end_at.is_some_and(|e| p.at > e));
    if let Some(failed) = failed
        && !r.pauses[failed + 1..].iter().any(|p| !p.paused)
    {
        return None;
    }
    let quiet = (r.started.iter()).all(|s| began(work, s.run) == ended(work, s.run))
        && r.called.iter().all(|c| done(work, c.call).is_some());
    quiet.then_some(end)
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
                Outsider::Outside => declared.writers.outside.is_some(),
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
        Event::Paused { by, why } => {
            // The engine's pause of a work that has ended, as one a carrier asks for as a round of a cancelled work
            // ends, would hold back nothing and say what is not so: it adds nothing.
            if by == Pauser::Engine && matches!(decide(&flow.flow, &work.records), Decision::Ended(_)) {
                return Ok(());
            }
            let at = work.tick();
            work.records.pauses.push(Pause {
                paused: true,
                at,
                by,
                why,
            });
        }
        Event::Resumed => {
            let at = work.tick();
            work.records.pauses.push(Pause {
                paused: false,
                at,
                by: Pauser::Owner,
                why: None,
            });
        }
        Event::Cancelled => {
            if work.records.cancelled.is_none() {
                work.records.cancelled = Some(work.tick());
            }
        }
        Event::Returned { run, came, spent } => {
            if began(work, run) == ended(work, run) {
                return Err(Refused::NoRoundUnderWay(run));
            }
            // The round of a run that was abandoned has ended too, and what came back from it is dropped; what it
            // took is kept.
            let kept = open(work, run);
            let at = work.tick();
            work.records.returned.push(Returned { run, at });
            let number = ended(work, run);
            work.spent.insert((run, number), spent);
            if kept {
                let back = match came {
                    Came::HandsIn(handed) => Back::HandedIn(handed),
                    Came::Asks(asks) => Back::Asked(asks),
                };
                work.back.insert((run, number), back);
            }
        }
        Event::Failed {
            run,
            why,
            failure,
            resets,
            spent,
        } => {
            if began(work, run) == ended(work, run) {
                return Err(Refused::NoRoundUnderWay(run));
            }
            // The round has ended, failed, and what it took is kept. Of a run that goes on, the failure is kept as
            // what came of the round, and where its kind calls for it the work is paused with its reason in the same
            // moment, so that the round is not given again before the owner has seen it, and nothing begins but what
            // answers a note. Of a run that was abandoned it is dropped, as what came back is.
            let kept = open(work, run);
            let at = work.tick();
            work.records.returned.push(Returned { run, at });
            let number = ended(work, run);
            work.spent.insert((run, number), spent);
            if kept {
                work.back.insert(
                    (run, number),
                    Back::Failed {
                        why: why.clone(),
                        failure,
                        resets,
                    },
                );
                if let Some(pausing) = run::pausing(&log(work, run)) {
                    let at = work.tick();
                    work.records.pauses.push(Pause {
                        paused: true,
                        at,
                        by: Pauser::Engine,
                        why: Some(pausing_words(&pausing, &why)),
                    });
                }
            }
        }
        Event::Done { call, result } => {
            if !work.records.called.iter().any(|c| c.call == call) {
                return Err(Refused::NoSuchCall(call));
            }
            if done(work, call).is_some() {
                return Err(Refused::AlreadyDone(call));
            }
            // The call of a run that was abandoned has its result too: that it was withdrawn, or what came back
            // before it could be.
            let at = work.tick();
            work.records.done.push(Done { call, at });
            work.results.insert(call, result);
        }
        Event::Anew { line, why } => {
            if !flow
                .cells
                .iter()
                .any(|cell| cell.line.as_deref() == Some(line.as_str()))
            {
                return Err(Refused::NoSuchLine(line));
            }
            let at = work.tick();
            work.sessions.push(Session { at, line, why });
        }
        Event::Ran { run, on } => {
            let number = began(work, run);
            if number == ended(work, run) {
                return Err(Refused::NoRoundUnderWay(run));
            }
            let at = work.tick();
            work.ran.push(Ran {
                at,
                run,
                round: number,
                on,
            });
        }
        Event::NotReleased { why } => {
            // The engine's pause of the ended work, with why: the owner sees it where the engine's other reasons are,
            // and letting the work on tries again.
            if release(flow, work).is_none() {
                return Err(Refused::NotToRelease);
            }
            let at = work.tick();
            work.records.pauses.push(Pause {
                paused: true,
                at,
                by: Pauser::Engine,
                why: Some(why),
            });
        }
        Event::Released => {
            // Said twice, as by a carrier that let go again after the engine started again, it was let go once.
            if work.records.released.is_some() {
                return Ok(());
            }
            if release(flow, work).is_none() {
                return Err(Refused::NotToRelease);
            }
            work.records.released = Some(work.tick());
        }
    }
    Ok(())
}

/// When something that happened is taken: the first place given to its entries, the time, and whether the agents'
/// rounds may begin.
#[derive(Clone, Copy)]
struct At {
    from: Seq,
    now: Time,
    gate: Gate,
}

impl At {
    /// When the entry at this place was added: now, for one added for what is being taken.
    fn time_of(self, work: &Work, place: Seq) -> Time {
        if place >= self.from {
            return self.now;
        }
        work.time_of(place).unwrap_or(self.now)
    }

    /// Whether the gate holds the rounds of the cell: it holds every round, or an agent's and the cell is one an agent
    /// carries out.
    fn holds(self, flow: &Compiled, cell: &CellId) -> bool {
        match self.gate {
            Gate::Open => false,
            Gate::Every => true,
            Gate::Agents => (flow.cells.iter()).any(|spec| spec.id == *cell && spec.kind == Kind::Agent),
        }
    }
}

/// Asks the deciding code what follows, and does it, until the work has a round under way, waits or has ended.
fn advance(flow: &Compiled, work: &mut Work, at: At, effects: &mut Vec<Effect>) {
    loop {
        let decision = decide(&flow.flow, &work.records);
        let run = match decision {
            // A run of a cell whose rounds are held is not started: its first round could not begin.
            Decision::Run { cell, .. } if at.holds(flow, &cell) => return,
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
                // What the run had under way is stopped, and what it rested at is withdrawn.
                if began(work, run) > ended(work, run) {
                    effects.push(Effect::Stop { run });
                }
                effects.extend(open_calls(work, run).into_iter().map(|call| Effect::Withdraw { call }));
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
        if !step(flow, work, run, at, effects) {
            return;
        }
    }
}

/// Does what the run does next. Says whether the deciding code is to be asked again: it is when the run finished,
/// had its calls made or came to rest for the owner at a limit, and not when a round of it began or is held.
fn step(flow: &Compiled, work: &mut Work, run: RunId, at: At, effects: &mut Vec<Effect>) -> bool {
    let choices = choices(flow);
    let Some(requires) = requires(flow, &choices, work, run) else {
        return false;
    };
    let Some(started) = work.records.started.iter().find(|s| s.run == run) else {
        return false;
    };
    let cell = started.cell.clone();
    let next = run::next(&log(work, run), requires, hold(&flow.flow, &work.records, run));
    // While the cell's rounds are held, none begins; the run rests until they may.
    let begins = matches!(next, Step::Begin | Step::Resume | Step::Retry | Step::SendBack { .. });
    if begins && at.holds(flow, &cell) {
        return false;
    }
    let given = match next {
        Step::Begin => Given::Task,
        Step::Resume => Given::Results(made(work, run, ended(work, run) - 1)),
        Step::Retry => {
            let Some(given) = work.given.get(&(run, ended(work, run))) else {
                return false;
            };
            given.clone()
        }
        Step::SendBack { unmet } => Given::SentBack(unmet),
        Step::Make => {
            let Some(Back::Asked(asks)) = last_back(work, run) else {
                return false;
            };
            for ask in asks.clone() {
                let on = match &ask {
                    Ask::Person { who, .. } => Some(who.clone()),
                    Ask::Tool { .. } => None,
                };
                call(work, run, on, Call::Asked(ask), effects);
            }
            return true;
        }
        Step::Wait { failed, ms } => {
            // The wait is counted from the end of the round that failed.
            let ended_at = (work.records.returned.iter())
                .filter(|r| r.run == run)
                .map(|r| r.at)
                .max();
            let Some(ended_at) = ended_at else {
                return false;
            };
            let until = Time(at.time_of(work, ended_at).0.saturating_add(ms));
            call(work, run, None, Call::Wait { until, failed }, effects);
            return true;
        }
        Step::Finish { writes } => {
            finish(work, run, writes);
            return true;
        }
        Step::AtLimit(limit) => {
            call(work, run, Some(Who::Owner), Call::AtLimit(limit), effects);
            return true;
        }
        Step::Rest { .. } | Step::UnderWay | Step::Held => return false,
    };
    let at = work.tick();
    work.records.rounds.push(Round { run, at });
    let number = began(work, run);
    work.given.insert((run, number), given.clone());
    effects.push(Effect::Round {
        run,
        cell,
        number,
        given,
    });
    false
}

/// Records the run as finished: the blocks it wrote, and their content under the place of the finish.
fn finish(work: &mut Work, run: RunId, writes: Vec<Write>) {
    let at = work.tick();
    if let Some(Back::HandedIn(handed)) = work.back.get(&(run, ended(work, run))) {
        for h in handed {
            work.contents.insert((h.block.clone(), at), h.content.clone());
        }
    }
    work.records.finished.push(Finished { run, at, writes });
}

/// Records a call of the run, and gives it to be made.
fn call(work: &mut Work, run: RunId, on: Option<Who>, what: Call, effects: &mut Vec<Effect>) {
    let call = CallId(work.name());
    let at = work.tick();
    work.records.called.push(Called { call, run, on, at });
    work.calls.insert(call, what.clone());
    effects.push(Effect::Call { call, run, what });
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
        must_write: &spec.must_write,
        choices,
        returns: spec.returns,
        calls: spec.calls,
    })
}

/// The run so far, as the deciding code reads it.
fn log(work: &Work, run: RunId) -> run::Log {
    let returned = |i: usize, b: &Back| match b {
        Back::Asked(asks) => run::Return::Asked {
            asked: asks.len(),
            made: (made(work, run, i).into_iter())
                .map(|call| run::Call {
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
        Back::Failed { why, failure, .. } => run::Return::Failed {
            failure: *failure,
            why: why.clone(),
            waits: (waits(work, run, i).into_iter())
                .map(|call| run::Call {
                    call,
                    done: done(work, call).is_some(),
                })
                .collect(),
        },
    };
    let rounds = (0..began(work, run))
        .map(|i| run::Round {
            returned: work.back.get(&(run, i + 1)).map(|b| returned(i, b)),
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
    // How many rounds had ended when the owner last let the work on after a pause.
    let let_on = (work.records.pauses.iter())
        .filter(|p| !p.paused)
        .map(|p| p.at)
        .max()
        .map_or(0, |at| {
            (work.records.returned.iter())
                .filter(|r| r.run == run && r.at < at)
                .count()
        });
    run::Log { rounds, let_go, let_on }
}

/// The calls made for what the run's round at this place, counted from none, asked for: those made after the round
/// ended and before the next began.
fn made(work: &Work, run: RunId, round: usize) -> Vec<CallId> {
    after(work, run, round, |call| matches!(call, Call::Asked(_)))
}

/// The waits the engine made after the run's round at this place, counted from none, which failed in passing.
fn waits(work: &Work, run: RunId, round: usize) -> Vec<CallId> {
    after(work, run, round, |call| matches!(call, Call::Wait { .. }))
}

/// The calls of the run that are of this sort, made after its round at this place, counted from none, ended and
/// before its next began.
fn after(work: &Work, run: RunId, round: usize, of: impl Fn(&Call) -> bool) -> Vec<CallId> {
    let r = &work.records;
    let ended = r.returned.iter().filter(|e| e.run == run).map(|e| e.at).nth(round);
    let next = r.rounds.iter().filter(|e| e.run == run).map(|e| e.at).nth(round + 1);
    let Some(ended) = ended else {
        return Vec::new();
    };
    (r.called.iter())
        .filter(|c| c.run == run && c.at > ended && next.is_none_or(|next| c.at < next))
        .filter(|c| work.calls.get(&c.call).is_some_and(&of))
        .map(|c| c.call)
        .collect()
}

/// What came back from the round of the run that ended last, if anything did.
fn last_back(work: &Work, run: RunId) -> Option<&Back> {
    work.back.get(&(run, ended(work, run)))
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

/// Why the engine pauses the work as a round fails, in words, after the reason whatever carried the round out gave.
fn pausing_words(pausing: &Pausing, why: &str) -> String {
    match pausing {
        Pausing::Failed(Failure::PromptTooLong) => {
            format!("the prompt is too long for the model, and running the round again cannot help: {why}")
        }
        Pausing::Failed(_) => why.to_string(),
        Pausing::Passing { failed } => {
            format!("the round failed in passing {failed} times in a row, and running it again has not helped: {why}")
        }
        Pausing::FlaggedAgain => {
            format!("the service's safety check flagged the round again, in the new session its line was given: {why}")
        }
    }
}
