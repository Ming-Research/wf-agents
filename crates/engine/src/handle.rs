//! The loop: one thing that happened is added to the work's records, and the deciding code is asked what follows
//! until nothing more does.

use std::collections::BTreeMap;

use decide::run::{self, Failure, Pausing, Requires, Step, Told, Unmet};
use decide::{
    Abandoned, BlockId, CallId, Called, CellId, Decision, Done, End, Finished, LetGo, Outsider, Pause, Pauser, PutIn,
    Returned, Room, Round, RunId, Seq, Started, Who, Write, Written, decide, decide_in, hold_in, told,
};
use flow::{Compiled, Kind};

use crate::asked;
use crate::event::{Effect, Event, Gate, Refused};
use crate::spoken;
use work::{Ask, Back, Call, Filled, Given, Moment, Ran, Session, Time, Work};

/// Takes one thing that happened to the work, at the time the driver's clock gives, and gives what is to be done for
/// it outside the engine, were nothing outside the work to hold it back ([`Room::FREE`], [`Gate::Open`]). Where the
/// records cannot be those of a work nothing is given: the deciding code says so when asked.
pub fn handle(flow: &Compiled, work: &mut Work, now: Time, event: Event) -> Result<Vec<Effect>, Refused> {
    handle_held(flow, work, now, event, Room::FREE, Gate::Open)
}

/// Takes one thing that happened to the work as [`handle`] does, in the room the engine leaves the work: under the
/// owner's pause of everything, and where no slot is free for a round that takes one, what would begin such a round
/// waits, and everything else is done.
pub fn handle_in(
    flow: &Compiled,
    work: &mut Work,
    now: Time,
    event: Event,
    room: Room,
) -> Result<Vec<Effect>, Refused> {
    handle_held(flow, work, now, event, room, Gate::Open)
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
    handle_held(flow, work, now, event, Room::FREE, gate)
}

/// Takes one thing that happened to the work as [`handle`] does, in the room the engine leaves it ([`handle_in`]) and
/// with the rounds held as the gate of the wait every work shares says ([`handle_with`]): what either holds back
/// waits, and everything else is done.
pub fn handle_held(
    flow: &Compiled,
    work: &mut Work,
    now: Time,
    event: Event,
    room: Room,
    gate: Gate,
) -> Result<Vec<Effect>, Refused> {
    handle_then(flow, work, now, event, room, gate, &|_, _| Vec::new())
}

/// Takes one thing that happened to the work as [`handle_held`] does, and then, before the deciding code is asked what
/// follows, what `then` says follows from it: what someone who writes from outside the work, reading the work as it
/// now is, adds in the same moment, so that the walk decides with it.
pub fn handle_then(
    flow: &Compiled,
    work: &mut Work,
    now: Time,
    event: Event,
    room: Room,
    gate: Gate,
    then: &dyn Fn(&Compiled, &Work) -> Vec<Event>,
) -> Result<Vec<Effect>, Refused> {
    let returned = match &event {
        Event::Returned { run, .. } => Some(*run),
        _ => None,
    };
    let from = work.tick();
    let releasing = release(flow, work).is_some();
    let ended = matches!(decide(&flow.flow, &work.records), Decision::Ended(_));
    let mut effects = Vec::new();
    match event {
        Event::Said { line, words } => spoken::said(flow, work, line, words, &mut effects)?,
        event => take(flow, work, event)?,
    }
    // What a round returned is taken as the round ends: where it meets everything the flow requires, the run finishes
    // here, before the walk, so that no cell before it comes between.
    if let Some(run) = returned
        && open(work, run)
        && let Some(told) = told(&flow.flow, &work.records, run)
    {
        let choices = choices(flow);
        if let Some(requires) = requires(flow, &choices, &told, work, run)
            && let Step::Finish { dealt_with, writes } = run::next(
                &log(work, run, &told),
                requires,
                hold_in(&flow.flow, &work.records, run, room),
            )
        {
            finish(flow, work, run, dealt_with, writes, &mut effects);
        }
    }
    for event in then(flow, work) {
        take(flow, work, event)?;
    }
    let at = At { from, now, room, gate };
    advance(flow, work, at, &mut effects)?;
    // A work that has just ended withdraws the calls its cells were waiting on: no cell is entered for their results.
    // Those of a run abandoned were withdrawn as it was.
    if !ended && matches!(decide(&flow.flow, &work.records), Decision::Ended(_)) {
        let finished = |call: &CallId| {
            (work.records.called.iter())
                .any(|c| c.call == *call && work.records.finished.iter().any(|f| f.run == c.run))
        };
        let calls: Vec<CallId> = unwanted(flow, work).into_iter().filter(finished).collect();
        effects.extend(calls.into_iter().map(|call| Effect::Withdraw { call }));
    }
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
/// its calls that have no result. Of a run that finished: the calls its return asked for that have no result, while
/// the work has not ended. Of a run that was abandoned: the stopping of its round under way, and the withdrawing of its
/// calls that have no result; so too the calls a work that has ended was waiting on. Of a work that has ended with
/// nothing under way: letting go of what it holds outside, until it is said to be let go. A driver that starts anew
/// asks for these again.
pub fn pending(flow: &Compiled, work: &Work) -> Vec<Effect> {
    let mut effects = Vec::new();
    let unwanted = unwanted(flow, work);
    for started in &work.records.started {
        let run = started.run;
        let number = began(work, run);
        let under_way = number > ended(work, run);
        if open(work, run)
            && under_way
            && let Some(given) = work.given.get(&(run, number))
        {
            effects.push(Effect::Round {
                run,
                cell: started.cell.clone(),
                number,
                given: given.clone(),
            });
        }
        if abandoned(work, run) && under_way {
            effects.push(Effect::Stop { run });
        }
        for call in open_calls(work, run) {
            if unwanted.contains(&call) {
                effects.push(Effect::Withdraw { call });
            } else if let Some(what) = work.calls.get(&call) {
                effects.push(Effect::Call {
                    call,
                    run,
                    what: what.clone(),
                });
            }
        }
    }
    effects.extend(release(flow, work).map(|end| Effect::Release { end }));
    effects
}

/// The calls that have no result and are no longer wanted: those of a run that was abandoned, and, once the work has
/// ended, every one.
fn unwanted(flow: &Compiled, work: &Work) -> Vec<CallId> {
    let ended = matches!(decide(&flow.flow, &work.records), Decision::Ended(_));
    (work.records.called.iter())
        .filter(|c| done(work, c.call).is_none() && (ended || abandoned(work, c.run)))
        .map(|c| c.call)
        .collect()
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
            // What stands for the questions and their answers the engine writes itself, as they are taken.
            let engines = [asked::ASKED, flow::ANSWER];
            let theirs = match by {
                Outsider::Person => declared.writers.people,
                Outsider::Outside => (declared.writers.outside.as_deref()).is_some_and(|s| !engines.contains(&s)),
            };
            if !theirs {
                return Err(Refused::NotTheirs { block, by });
            }
            match (&declared.values, &value) {
                (Some(_), None) => return Err(Refused::NoValue { block }),
                (Some(values), Some(v)) if !values.contains(v) => {
                    let value = v.clone();
                    return Err(Refused::NotAValue { block, value });
                }
                (None, Some(_)) => return Err(Refused::NotAChoice { block }),
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
        Event::WroteForRound { run, block, content } => {
            if began(work, run) == ended(work, run) || !open(work, run) {
                return Err(Refused::NoRoundUnderWay(run));
            }
            let wrote = Event::Wrote {
                block: block.clone(),
                by: Outsider::Outside,
                content,
                matters: true,
                value: None,
            };
            take(flow, work, wrote)?;
            let version = work
                .records
                .written
                .last()
                .map(|w| w.at)
                .expect("the version was written");
            let at = work.tick();
            work.records.put_in.push(PutIn {
                run,
                block,
                version,
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
        Event::Cancelled { why } => {
            if work.records.cancelled.is_none() {
                work.records.cancelled = Some(work.tick());
                work.why_cancelled = why;
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
                // A return that does not say what it dealt with dealt with everything that ran its round.
                let dealt_with = match came.dealt_with {
                    Some(named) => named,
                    None => told(&flow.flow, &work.records, run).map_or_else(Vec::new, |t| t.ran_by),
                };
                // What the engine asks with the round is the work's from the round's end, whatever becomes of the
                // return.
                let cell = (work.records.started.iter())
                    .find(|s| s.run == run)
                    .map(|s| s.cell.clone());
                if let Some(cell) = cell {
                    asked::record(work, run, &cell, true, &came.engine);
                }
                let back = Back::Returned {
                    dealt_with,
                    handed: came.writes,
                    asks: came.asks,
                    found: came.found,
                    said: came.said,
                    engine: came.engine,
                };
                work.back.insert((run, number), back);
                asked::tell(flow, work);
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
                let told = told(&flow.flow, &work.records, run).unwrap_or_default();
                if let Some(pausing) = run::pausing(&log(work, run, &told)) {
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
        Event::Answer {
            question,
            version,
            option,
            why,
        } => {
            asked::answer(flow, work, question, version, option, why)?;
            asked::tell(flow, work);
        }
        // Taken before the rest, as it gives what is to be done.
        Event::Said { .. } => {}
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
        Event::Ran { run, attempt, on } => {
            let number = began(work, run);
            if number == ended(work, run) {
                return Err(Refused::NoRoundUnderWay(run));
            }
            let at = work.tick();
            work.ran.push(Ran {
                at,
                run,
                round: number,
                attempt,
                on,
            });
        }
        Event::Filled { run, attempt, fills } => {
            let number = began(work, run);
            if number == ended(work, run) {
                return Err(Refused::NoRoundUnderWay(run));
            }
            let at = work.tick();
            work.filled.push(Filled {
                at,
                run,
                round: number,
                attempt,
                fills,
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

/// When something that happened is taken: the first place given to its entries, the time, the room the engine leaves
/// the work, and whether the agents' rounds may begin.
#[derive(Clone, Copy)]
struct At {
    from: Seq,
    now: Time,
    room: Room,
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

/// Asks the deciding code what follows, and does it, until the work has a round under way, waits or has ended. Each
/// time round abandons a run, starts one, which then begins its round or rests, or goes on with one, which then
/// finishes, makes a wait or rests at a limit: so no more often than twice the runs the work has started, and once
/// for each cell besides. Going round more often, the deciding code decides without end on these records, which is
/// refused with what it decided last, rather than the engine holding the process for ever.
fn advance(flow: &Compiled, work: &mut Work, at: At, effects: &mut Vec<Effect>) -> Result<(), Refused> {
    let bound = 2 * work.records.started.len() + flow.cells.len() + 2;
    let mut passes = 0;
    loop {
        let decision = decide_in(&flow.flow, &work.records, at.room);
        passes += 1;
        if passes > bound {
            return Err(Refused::WithoutEnd(decision));
        }
        let run = match decision {
            // A run of a cell whose rounds are held is not started: its first round could not begin.
            Decision::Run { cell, .. } if at.holds(flow, &cell) => return Ok(()),
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
            | Decision::Wait { .. }
            | Decision::Idle => {
                return Ok(());
            }
        };
        if !step(flow, work, run, at, effects) {
            return Ok(());
        }
    }
}

/// Does what the run does next. Says whether the deciding code is to be asked again: it is when the run finished, had
/// a wait made or came to rest for the owner at a limit, and not when a round of it began or is held.
fn step(flow: &Compiled, work: &mut Work, run: RunId, at: At, effects: &mut Vec<Effect>) -> bool {
    let choices = choices(flow);
    let Some(told) = told(&flow.flow, &work.records, run) else {
        return false;
    };
    let Some(requires) = requires(flow, &choices, &told, work, run) else {
        return false;
    };
    let Some(started) = work.records.started.iter().find(|s| s.run == run) else {
        return false;
    };
    let cell = started.cell.clone();
    // A run entered for the results of its cell's calls is given them in its first round.
    let results = started.because.iter().find_map(|cause| match cause {
        decide::Cause::Results { calls } => Some(calls.clone()),
        _ => None,
    });
    let next = run::next(
        &log(work, run, &told),
        requires,
        hold_in(&flow.flow, &work.records, run, at.room),
    );
    // While the cell's rounds are held, none begins; the run rests until they may.
    let begins = matches!(next, Step::Begin | Step::Retry | Step::SendBack { .. });
    if begins && at.holds(flow, &cell) {
        return false;
    }
    let mut written_over = Vec::new();
    let given = match next {
        Step::Begin => results.map_or(Given::Task, Given::Results),
        Step::Retry => {
            let Some(given) = work.given.get(&(run, ended(work, run))) else {
                return false;
            };
            given.clone()
        }
        Step::SendBack { unmet } => {
            // The round that rewrites a return that would have written over what it never saw is told of the version.
            written_over = (unmet.iter())
                .filter_map(|u| match u {
                    Unmet::WrittenOver { block, version } => Some((block.clone(), *version)),
                    _ => None,
                })
                .collect();
            Given::SentBack(unmet)
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
        Step::Finish { dealt_with, writes } => {
            finish(flow, work, run, dealt_with, writes, effects);
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
    for (block, version) in written_over {
        let at = work.tick();
        work.records.put_in.push(PutIn {
            run,
            block,
            version,
            at,
        });
    }
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

/// Takes the run's return, which met everything the flow requires: the calls it asks for are made, and the run
/// finishes, recording what it dealt with, the blocks it wrote and their content under the place of the finish, and
/// then the questions it asks, asks anew or withdraws, each a version of its own of its cell. A return of the form
/// before a return that asks ended its run made all its calls at once as it was taken, and its run rested at them,
/// unless the owner's word at the limit of calls was asked for first: such a run finishes making none again, or all it
/// asked for where it made none, and its cell is owed those it made. The run keeps, as it finishes, the observations
/// its returns said that it keeps (crate `work`, module `observations`).
fn finish(
    flow: &Compiled,
    work: &mut Work,
    run: RunId,
    dealt_with: Vec<BlockId>,
    writes: Vec<Write>,
    effects: &mut Vec<Effect>,
) {
    let mut questions = Vec::new();
    let mut reply = None;
    let (handed, asks) = match work.back.get(&(run, ended(work, run))) {
        Some(Back::Returned { handed, asks, said, .. }) => {
            questions = said.questions.clone();
            reply = said.reply.clone();
            (handed.clone(), asks.clone())
        }
        Some(Back::HandedIn(handed)) => (handed.clone(), Vec::new()),
        Some(Back::Asked(asks)) => {
            let r = &work.records;
            let ended_at = (r.returned.iter()).filter(|e| e.run == run).map(|e| e.at).max();
            let made = (r.called.iter()).any(|c| {
                c.run == run
                    && ended_at.is_none_or(|e| c.at > e)
                    && matches!(work.calls.get(&c.call), Some(Call::Asked(_)))
            });
            (Vec::new(), if made { Vec::new() } else { asks.clone() })
        }
        Some(Back::Failed { .. }) | None => (Vec::new(), Vec::new()),
    };
    for ask in asks {
        let on = match &ask {
            Ask::Person { who, .. } => Some(who.clone()),
            Ask::Tool { .. } => None,
        };
        call(work, run, on, Call::Asked(ask), effects);
    }
    let at = work.tick();
    for h in handed {
        work.contents.insert((h.block, at), h.content);
    }
    // What the return says in a field that a datum is written through is the run's version of that datum: its answer
    // to the owner an entry of the record of what is said, its questions the questions as they then stand.
    let mut writes = writes;
    let reply_to = fielded(flow, REPLY).filter(|_| reply.is_some());
    let asked_in = fielded(flow, QUESTIONS).filter(|_| !questions.is_empty());
    for block in reply_to.iter().chain(&asked_in) {
        writes.push(Write {
            block: block.clone(),
            meaning_unchanged: false,
            value: None,
        });
    }
    if let (Some(block), Some(reply)) = (reply_to, reply) {
        work.contents.insert((block, at), reply);
    }
    work.records.finished.push(Finished {
        run,
        at,
        writes,
        dealt_with: Some(dealt_with),
    });
    let line = (work.records.started.iter().find(|s| s.run == run))
        .and_then(|s| flow.cells.iter().find(|c| c.id == s.cell))
        .and_then(|c| c.line.as_deref());
    let observed = work::observations::kept(work, run, line);
    work.observed.extend(observed);
    if !questions.is_empty()
        && let Some(cell) = (work.records.started.iter())
            .find(|s| s.run == run)
            .map(|s| s.cell.clone())
    {
        asked::record(work, run, &cell, false, &questions);
        asked::tell(flow, work);
    }
    if let Some(block) = asked_in {
        let standing = asked::standing(flow, work, &block);
        work.contents.insert((block, at), standing);
    }
}

/// The field of a return that holds its answer to the owner.
const REPLY: &str = "reply";

/// The field of a return that holds its questions.
const QUESTIONS: &str = "questions";

/// The block of the flow written through this field of a return, where it has one.
fn fielded(flow: &Compiled, field: &str) -> Option<BlockId> {
    (flow.blocks.iter())
        .find(|(_, b)| b.field.as_deref() == Some(field))
        .map(|(name, _)| name.clone())
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
pub(crate) fn choices(flow: &Compiled) -> BTreeMap<BlockId, Vec<String>> {
    (flow.blocks.iter())
        .filter_map(|(block, declared)| declared.values.clone().map(|values| (block.clone(), values)))
        .collect()
}

/// What the flow requires of what the run's cell returns, with what its round was told; none where the run's cell is
/// not in the flow.
pub(crate) fn requires<'a>(
    flow: &'a Compiled,
    choices: &'a BTreeMap<BlockId, Vec<String>>,
    told: &'a Told,
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
        told,
    })
}

/// The run so far, as the deciding code reads it. A return of the form before a return said what it dealt with reads
/// as one that dealt with nothing where it asked, and with everything that ran its round where it handed in.
pub(crate) fn log(work: &Work, run: RunId, told: &Told) -> run::Log {
    let writes = |handed: &[work::Handed]| {
        (handed.iter())
            .map(|h| Write {
                block: h.block.clone(),
                meaning_unchanged: !h.matters,
                value: h.value.clone(),
            })
            .collect()
    };
    let returned = |i: usize, b: &Back| match b {
        Back::Returned {
            dealt_with,
            handed,
            asks,
            found,
            said,
            ..
        } => run::Return::Came {
            dealt_with: dealt_with.clone(),
            writes: writes(handed),
            calls: asks.len(),
            found: found.clone(),
            waits: said.waits(),
            questions: (said.questions.iter())
                .map(|q| run::Asking {
                    name: q.name().to_string(),
                    withdrawn: matches!(q, work::Question::Withdrawn { .. }),
                })
                .collect(),
        },
        Back::Asked(asks) => run::Return::Came {
            dealt_with: Vec::new(),
            writes: Vec::new(),
            calls: asks.len(),
            found: Vec::new(),
            waits: false,
            questions: Vec::new(),
        },
        Back::HandedIn(handed) => run::Return::Came {
            dealt_with: told.ran_by.clone(),
            writes: writes(handed),
            calls: 0,
            found: Vec::new(),
            waits: false,
            questions: Vec::new(),
        },
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

/// The waits the engine made after the run's round at this place, counted from none, which failed in passing: those
/// made after the round ended and before the next began.
fn waits(work: &Work, run: RunId, round: usize) -> Vec<CallId> {
    let r = &work.records;
    let ended = r.returned.iter().filter(|e| e.run == run).map(|e| e.at).nth(round);
    let next = r.rounds.iter().filter(|e| e.run == run).map(|e| e.at).nth(round + 1);
    let Some(ended) = ended else {
        return Vec::new();
    };
    (r.called.iter())
        .filter(|c| c.run == run && c.at > ended && next.is_none_or(|next| c.at < next))
        .filter(|c| matches!(work.calls.get(&c.call), Some(Call::Wait { .. })))
        .map(|c| c.call)
        .collect()
}

/// Whether the run was abandoned.
fn abandoned(work: &Work, run: RunId) -> bool {
    work.records.abandoned.iter().any(|a| a.run == run)
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
