//! The driver of simulated histories of whole works through `handle` only (principle simulated-histories), which the
//! tests of histories share. It holds what the engine gave to be done and not yet done (rounds to end, calls to answer,
//! rounds to stop, calls to withdraw) and at each step does one of those things, or a person or the outside acts; at
//! any step the engine may stop, and the driver then forgets what it held and takes `pending` instead. What an agent
//! returns at the end of a round, and what a person or the outside does, is the test's own: its [`Part`].

use std::collections::BTreeSet;

use decide::run::Failure;
use decide::{CallId, Cause, Decision, Outsider, Pauser, Records, RunId, decide};
use engine::{Ask, Back, Call, Came, Effect, Event, Spent, Time, Work, handle, pending};
use flow::Compiled;

/// A small deterministic generator (xorshift), so that a seed gives the same history every time.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(
            seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)
                .wrapping_add(0x2545_F491_4F6C_DD1D)
                | 1,
        )
    }

    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    pub fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

/// What the histories of all seeds did, so that every class of step is known to occur.
#[derive(Default, Debug)]
pub struct Seen {
    pub limits: usize,
    pub abandons: usize,
    pub withdrawn: usize,
    pub refused: usize,
    pub stops: usize,
    /// Stops of the engine while the driver held a stop or a withdrawal not yet done.
    pub stops_undoing: usize,
    /// Rounds stopped by a cancel.
    pub stopped: usize,
    /// Histories that ended cancelled.
    pub cancels: usize,
    /// Rounds that failed, of a run that went on.
    pub failed: usize,
    /// Rounds given again once the work was let on, the round before them having failed.
    pub retried: usize,
    /// Runs abandoned, while the work was not cancelled, after their last round failed.
    pub failed_abandoned: usize,
    /// Histories whose work came to rest, once people stopped, not paused.
    pub rested_unpaused: usize,
    /// Rounds given while the work was paused: of a run started for a note a person wrote.
    pub paused_rounds: usize,
    /// Hand-ins that lacked a block their cell must write, by a run still open after them: sent back, or resting at
    /// the limit of returns.
    pub lacking: usize,
    /// Finished runs of cells that must write a block each time.
    pub bound_finished: usize,
    /// Releases given of what a work that ended held.
    pub released: usize,
}

/// The first finished run of the records that lacks a write of a block its cell must write, read from the records of
/// the finishes and the cells' definitions (property a-run-is-one-mechanism), said in words; none where there is none.
/// A run is held to what its cell must write where its return asked for no call and dealt with everything that ran its
/// round (plan, D8).
pub fn lacking_a_must_write(flow: &Compiled, records: &Records) -> Option<String> {
    for finished in &records.finished {
        let started = (records.started.iter())
            .find(|s| s.run == finished.run)
            .expect("a finished run started");
        let ended = (records.returned.iter())
            .filter(|r| r.run == finished.run)
            .map(|r| r.at)
            .max();
        let asked = (records.called.iter())
            .any(|c| c.run == finished.run && c.at < finished.at && ended.is_none_or(|e| c.at > e));
        let ran: Vec<&decide::BlockId> = (started.because.iter())
            .filter_map(|c| match c {
                Cause::Changed { block, .. } => Some(block),
                _ => None,
            })
            .chain(
                records
                    .put_in
                    .iter()
                    .filter(|p| p.run == finished.run)
                    .map(|p| &p.block),
            )
            .collect();
        let everything = finished
            .dealt_with
            .as_ref()
            .is_none_or(|dealt| ran.iter().all(|b| dealt.contains(b)));
        if asked || !everything {
            continue;
        }
        let spec = (flow.cells.iter())
            .find(|c| c.id == started.cell)
            .expect("the cell is in the flow");
        for block in &spec.must_write {
            if !finished.writes.iter().any(|w| w.block == *block) {
                return Some(format!(
                    "run {:?} of {} finished without writing {block}, which its cell must write",
                    finished.run, started.cell
                ));
            }
        }
    }
    None
}

/// Whether the run started for a note a person wrote: one of its reasons is a note that a person wrote since its cell
/// last ran (since the start of the run of its record, or with no record of its first run), after the version its
/// cell's record knew, and before the run started; for a cell with no record, a note the run started from. A cell no
/// run of which started before has not run. Read from the records and the flow's notes; it does not ask whether the
/// write changed the note's meaning, nor, with no record, whether the cell reads it, so it is a little wider than the
/// rule.
pub fn for_a_note(flow: &decide::Flow, records: &Records, run: RunId) -> bool {
    let r = records;
    let Some(started) = r.started.iter().find(|s| s.run == run) else {
        return false;
    };
    // Since the cell last ran: since the start of its latest run that finished dealing with something, or with none of
    // its first run. A run that only asked for calls dealt with nothing.
    let of_cell = || (r.started.iter()).filter(|s| s.cell == started.cell && s.at < started.at);
    let record = (r.finished.iter())
        .filter(|f| f.at < started.at && f.dealt_with.as_ref().is_none_or(|d| !d.is_empty()))
        .filter_map(|f| of_cell().find(|s| s.run == f.run).map(|s| (f.at, s.at)))
        .max();
    let Some(since) = record.map(|(_, at)| at).or_else(|| of_cell().map(|s| s.at).min()) else {
        return false;
    };
    let by_person = |block: &decide::BlockId, was: Option<decide::Seq>| {
        flow.is_note(block)
            && (r.written.iter()).any(|w| {
                w.by == Outsider::Person && w.block == *block && w.at > since && Some(w.at) > was && w.at < started.at
            })
    };
    started.because.iter().any(|c| match c {
        Cause::NoRecord => started.basis.keys().any(|block| by_person(block, None)),
        Cause::Changed { block, was, .. } => by_person(block, *was),
        Cause::Results { .. } => false,
    })
}

/// Why the round of the run that has this number may not be given while the work is paused, if it may not: a paused
/// work gives a round only of a run started for a note a person wrote ([`for_a_note`]), and never the round after
/// one whose failure paused the work, which waits for the owner to let the work on.
pub fn held_back(flow: &Compiled, work: &Work, run: RunId, number: usize) -> Option<String> {
    if !for_a_note(&flow.flow, &work.records, run) {
        return Some(format!(
            "round {number} of {run:?}, which started for no note a person wrote"
        ));
    }
    if failure_paused(work, run, number - 1) {
        return Some(format!(
            "round {number} of {run:?}, given again after its failure paused the work"
        ));
    }
    None
}

/// What a test makes of the steps that are its own: whether a round fails, what an agent returns, and what a person or
/// the outside does.
pub trait Part {
    /// Whether the round under way, or one given to be stopped where `stopped` says so, fails as it ends instead of
    /// returning, `people` saying whether people still act.
    fn fails(&mut self, d: &mut Driver, stopped: bool, people: bool) -> bool;

    /// What the round under way of this run of this cell returns as it ends.
    fn returns(&mut self, d: &mut Driver, run: RunId, cell: &str) -> Came;

    /// What a person or the outside does now.
    fn acts(&mut self, d: &mut Driver) -> Event;
}

/// A key that orders effects the same way however they came to be held.
pub fn key(effect: &Effect) -> String {
    format!("{effect:?}")
}

/// A history being driven: the work, and what the driver holds to be done.
pub struct Driver {
    pub flow: Compiled,
    pub work: Work,
    pub rng: Rng,
    /// Draws the stops alone, so that a history with stops makes the same choices as one without.
    pub stops: Option<Rng>,
    /// What the engine gave to be done and is not yet done, in a fixed order.
    pub held: Vec<Effect>,
    /// Every round given, by run and number.
    pub rounds: BTreeSet<(RunId, usize)>,
    /// Calls that were withdrawn or have their result.
    pub closed: Vec<CallId>,
    /// The driver's clock, in milliseconds; it never goes back.
    pub clock: u64,
    /// Whether the work is paused, by whoever, as the records said after the last event.
    pub paused: bool,
    pub log: Vec<String>,
    pub seen: Seen,
}

impl Driver {
    /// The driver of a history of a new work on this flow, drawing from this generator; with stops of the engine
    /// drawn apart, from the seed, where `stops` says so.
    pub fn new(flow: Compiled, rng: Rng, seed: u64, stops: bool) -> Self {
        Self {
            flow,
            work: Work::default(),
            rng,
            stops: stops.then(|| Rng::new(seed ^ 0x5DEE_CE66_D1CE_F00D)),
            held: Vec::new(),
            rounds: BTreeSet::new(),
            closed: Vec::new(),
            clock: 1_800_000_000_000,
            paused: false,
            log: Vec::new(),
            seen: Seen::default(),
        }
    }

    pub fn note(&mut self, what: String) {
        self.log.push(what);
    }

    /// Hands the event to the loop, and takes what is to be done, or checks a refusal left the work as it was.
    pub fn take(&mut self, event: Event) -> bool {
        self.note(format!("event: {event:?}"));
        let before = self.work.clone();
        let failed = matches!(event, Event::Failed { .. });
        // The driver's clock: a second for each thing it hands the engine, so that a history with stops keeps the same
        // times as one without.
        self.clock += 1000;
        let now = Time(self.clock);
        match handle(&self.flow, &mut self.work, now, event) {
            Ok(effects) => {
                self.note(format!("effects: {effects:?}"));
                if failed && decide::pause(&self.work.records).is_some_and(|p| p.by == Pauser::Engine) {
                    self.seen.failed += 1;
                }
                let r = &self.work.records;
                for a in &r.abandoned[before.records.abandoned.len()..] {
                    let ended = r.returned.iter().filter(|e| e.run == a.run).count();
                    let last = self.work.back.get(&(a.run, ended));
                    if r.cancelled.is_none() && matches!(last, Some(Back::Failed { .. })) {
                        self.seen.failed_abandoned += 1;
                    }
                }
                self.paused = decide::paused(&self.work.records);
                for effect in effects {
                    match &effect {
                        Effect::Round { run, number, .. } => {
                            assert!(
                                self.rounds.insert((*run, *number)),
                                "round {number} of {run:?} given twice"
                            );
                            if matches!(self.work.back.get(&(*run, number - 1)), Some(Back::Failed { .. })) {
                                self.seen.retried += 1;
                            }
                            // A paused work begins a round only of a run for a note a person wrote.
                            if self.paused {
                                if let Some(why) = held_back(&self.flow, &self.work, *run, *number) {
                                    panic!("the work is paused and gives {why}");
                                }
                                self.seen.paused_rounds += 1;
                            }
                        }
                        // What is withdrawn or stopped is no longer to be done as it was: it is to be undone.
                        Effect::Withdraw { call } => {
                            let at = self
                                .held
                                .iter()
                                .position(|e| matches!(e, Effect::Call { call: c, .. } if c == call));
                            self.held.remove(at.expect("a withdrawn call was held"));
                            self.seen.withdrawn += 1;
                        }
                        Effect::Stop { run } => {
                            let at = self
                                .held
                                .iter()
                                .position(|e| matches!(e, Effect::Round { run: r, .. } if r == run));
                            self.held.remove(at.expect("a stopped round was held"));
                            self.seen.stopped += 1;
                        }
                        Effect::Call {
                            what: Call::AtLimit(_), ..
                        } => self.seen.limits += 1,
                        Effect::Call { .. } => {}
                        Effect::Release { .. } => {
                            assert!(
                                !self.held.iter().any(|e| matches!(e, Effect::Release { .. })),
                                "the release is given while it is held already"
                            );
                            self.seen.released += 1;
                        }
                    }
                    self.held.push(effect);
                }
                self.held.sort_by_key(key);
                true
            }
            Err(why) => {
                self.note(format!("refused: {why:?}"));
                assert_eq!(self.work, before, "a refused event changed the work");
                self.seen.refused += 1;
                false
            }
        }
    }

    /// What must hold of the engine after every step.
    pub fn check(&self) {
        let r = &self.work.records;
        let mut pending = pending(&self.flow, &self.work);
        pending.sort_by_key(key);
        assert_eq!(pending, self.held, "what is pending is not what the driver holds");
        // A round that ended, as a failed one has, is not under way: none is to be done again.
        for effect in &pending {
            if let Effect::Round { run, number, .. } = effect {
                assert!(
                    !self.work.spent.contains_key(&(*run, *number)),
                    "round {number} of {run:?} is pending though it ended"
                );
            }
        }
        // A round after one that failed is given what the failed one was given.
        for (&(run, number), given) in &self.work.given {
            if number > 1 && matches!(self.work.back.get(&(run, number - 1)), Some(Back::Failed { .. })) {
                assert_eq!(
                    Some(given),
                    self.work.given.get(&(run, number - 1)),
                    "round {number} of {run:?}, after a round that failed, is given otherwise than that round was"
                );
            }
        }
        // No round was under way as the engine paused the work for a failed round: while that pause holds, a round is
        // pending only where it began since, of a run for a note a person wrote.
        if decide::pause(r).is_some_and(|p| p.by == Pauser::Engine) {
            for effect in &pending {
                if let Effect::Round { run, number, .. } = effect
                    && let Some(why) = held_back(&self.flow, &self.work, *run, *number)
                {
                    panic!("{why} is pending while the engine's pause for a failed round holds the work");
                }
            }
        }
        let under_way = (r.started.iter())
            .filter(|s| {
                let began = r.rounds.iter().filter(|x| x.run == s.run).count();
                let ended = r.returned.iter().filter(|x| x.run == s.run).count();
                began > ended
            })
            .count();
        assert!(under_way <= 1, "{under_way} rounds are under way");
        let times: Vec<Time> = self.work.moments.iter().map(|m| m.at).collect();
        assert!(
            times.windows(2).all(|t| t[0] <= t[1]),
            "the times of the moments go back: {times:?}"
        );
        let places: Vec<_> = self.work.moments.iter().map(|m| m.from).collect();
        assert!(
            places.windows(2).all(|p| p[0] < p[1]),
            "the moments do not follow the records: {places:?}"
        );
        assert!(
            !matches!(decide(&self.flow.flow, r), Decision::Fault(_)),
            "the records are faulty"
        );
        if let Some(why) = lacking_a_must_write(&self.flow, r) {
            panic!("{why}");
        }
        for effect in &self.held {
            match effect {
                Effect::Round { run, .. } => assert!(r.started.iter().any(|s| s.run == *run), "an unknown run"),
                Effect::Call { call, run, .. } => {
                    assert!(
                        r.called.iter().any(|c| c.call == *call && c.run == *run),
                        "an unknown call"
                    );
                }
                Effect::Withdraw { call } => {
                    assert!(r.called.iter().any(|c| c.call == *call), "an unknown call withdrawn");
                }
                Effect::Stop { run } => assert!(r.started.iter().any(|s| s.run == *run), "an unknown run stopped"),
                // Nothing of a work is under way as it lets go of what it holds: no round, and no call, which a tool
                // may be working at in the worktree.
                Effect::Release { end } => {
                    assert_eq!(
                        decide(&self.flow.flow, r),
                        Decision::Ended(*end),
                        "what the work holds is let go though it has not ended so"
                    );
                    let nothing = (self.held.iter()).all(|e| matches!(e, Effect::Release { .. }));
                    assert!(
                        nothing,
                        "what the work holds is let go while something of it is to be done"
                    );
                }
            }
        }
        // A work that ended with nothing under way has let go of what it holds, or is to.
        let ended = matches!(decide(&self.flow.flow, r), Decision::Ended(_));
        if ended && r.released.is_none() && (self.held.iter()).all(|e| matches!(e, Effect::Release { .. })) {
            assert_eq!(
                self.held.len(),
                1,
                "a work that ended is not to let go of what it holds: {:?}",
                self.held
            );
        }
    }

    /// The round under way of the run ends, having returned this.
    pub fn returned(&mut self, run: RunId, came: Came) {
        let spent = self.spent(run);
        let taken = self.take(Event::Returned { run, came, spent });
        assert!(taken, "the return of a round under way was refused");
    }

    /// The call has its result.
    pub fn give_result(&mut self, call: CallId) {
        let taken = self.take(Event::Done {
            call,
            result: format!("the result of {}", call.0),
        });
        assert!(taken, "the result of an open call was refused");
        self.closed.push(call);
    }

    /// A person or the outside acts as the part says.
    pub fn acts(&mut self, part: &mut impl Part) {
        let event = part.acts(self);
        let cancel = matches!(event, Event::Cancelled { .. });
        let taken = self.take(event);
        if cancel {
            assert!(taken, "a cancel was refused");
        }
    }

    /// What a round of the run took, as its agent reports it: told apart by the run and the clock.
    pub fn spent(&self, run: RunId) -> Spent {
        Spent {
            usd_micros: run.0,
            input_tokens: self.clock % 997,
            cached_tokens: 0,
            output_tokens: 1,
            ..Default::default()
        }
    }

    /// The driver forgets what it held and takes what is pending instead.
    pub fn stop(&mut self) {
        self.note("the engine stops".into());
        let mut held = pending(&self.flow, &self.work);
        held.sort_by_key(key);
        self.held = held;
        self.seen.stops += 1;
    }

    /// The engine stops, where the generator of stops says so: with a chance of one in twelve, and of one in two when
    /// the driver holds a stop or a withdrawal not yet done.
    pub fn maybe_stop(&mut self) {
        let undoing = (self.held.iter()).any(|e| matches!(e, Effect::Withdraw { .. } | Effect::Stop { .. }));
        if let Some(stops) = &mut self.stops
            && stops.chance(if undoing { 50 } else { 8 })
        {
            if undoing {
                self.seen.stops_undoing += 1;
            }
            self.stop();
        }
    }

    /// What the driver holds that it can do now: everything, but a call for a person only while people act.
    pub fn doable(&self, people: bool) -> Vec<Effect> {
        (self.held.iter())
            .filter(|e| match e {
                Effect::Round { .. } => true,
                Effect::Call { what, .. } => people || matches!(what, Call::Asked(Ask::Tool { .. })),
                Effect::Withdraw { .. } | Effect::Stop { .. } | Effect::Release { .. } => true,
            })
            .cloned()
            .collect()
    }

    /// Does this, which the driver holds: a round fails or ends as the part says, a call has its result, a call
    /// withdrawn is said to have its result, a round stopped fails as the part says or ends with what it returns
    /// dropped. `people` says whether people still act.
    pub fn carry_out(&mut self, effect: Effect, part: &mut impl Part, people: bool) {
        let at = self.held.iter().position(|e| *e == effect).expect("held");
        self.held.remove(at);
        match effect {
            Effect::Round { run, cell, .. } => {
                if part.fails(self, false, people) {
                    let spent = self.spent(run);
                    let why = format!("the agent of {cell} could not start");
                    let taken = self.take(Event::Failed {
                        run,
                        why,
                        failure: Failure::Other,
                        resets: None,
                        spent,
                    });
                    assert!(taken, "the failure of a round under way was refused");
                    return;
                }
                let came = part.returns(self, run, cell.as_str());
                let place = self.flow.flow.place(&cell).expect("the cell is in the flow");
                let must = &self.flow.cells[place].must_write;
                let lacks = came.asks.is_empty()
                    && came.dealt_with.is_none()
                    && must.iter().any(|b| !came.writes.iter().any(|h| h.block == *b));
                self.returned(run, came);
                let r = &self.work.records;
                let open = !r.finished.iter().any(|f| f.run == run) && !r.abandoned.iter().any(|a| a.run == run);
                if lacks && open {
                    self.seen.lacking += 1;
                }
            }
            Effect::Call { call, .. } => self.give_result(call),
            Effect::Withdraw { call } => {
                let taken = self.take(Event::Done {
                    call,
                    result: "withdrawn".to_string(),
                });
                assert!(taken, "the result of a withdrawn call was refused");
                self.closed.push(call);
            }
            Effect::Stop { run } => {
                let spent = self.spent(run);
                let end = if part.fails(self, true, people) {
                    Event::Failed {
                        run,
                        why: "stopped".to_string(),
                        failure: Failure::Other,
                        resets: None,
                        spent,
                    }
                } else {
                    Event::Returned {
                        run,
                        came: Came::asks(vec![]),
                        spent,
                    }
                };
                assert!(self.take(end), "the end of a stopped round was refused");
            }
            Effect::Release { .. } => {
                assert!(
                    self.take(Event::Released),
                    "the release of a work that ended was refused"
                );
            }
        }
    }

    /// One step: the engine may stop first; then, while people act, they do so with a chance of one in four, and
    /// otherwise the driver does one of the things it can, each with an equal chance. Says whether anything was done:
    /// once people stop, nothing is when the driver holds nothing it can do.
    pub fn step(&mut self, part: &mut impl Part, people: bool) -> bool {
        self.maybe_stop();
        if people && self.rng.chance(25) {
            self.acts(part);
            return true;
        }
        let doable = self.doable(people);
        if doable.is_empty() {
            if people {
                self.acts(part);
                return true;
            }
            return false;
        }
        let effect = doable[self.rng.below(doable.len())].clone();
        self.carry_out(effect, part, people);
        true
    }
}

/// Whether the run's round of this number failed and the engine paused the work for it, which was not let on since:
/// the engine paused it after the round began, as for its failure or its going over the budget, and no letting on came
/// after the round's beginning.
pub fn failure_paused(work: &Work, run: RunId, number: usize) -> bool {
    if number == 0 || !matches!(work.back.get(&(run, number)), Some(Back::Failed { .. })) {
        return false;
    }
    let r = &work.records;
    let Some(began) = r.rounds.iter().filter(|e| e.run == run).map(|e| e.at).nth(number - 1) else {
        return false;
    };
    let paused = (r.pauses.iter()).any(|p| p.paused && p.by == Pauser::Engine && p.at > began);
    let let_on = (r.pauses.iter()).any(|p| !p.paused && p.at > began);
    paused && !let_on
}
