//! Simulated histories of whole works through `handle` only (principle simulated-histories). A driver holds what the
//! engine gave to be done and not yet done (rounds to end, calls to answer, rounds to stop, calls to withdraw) and at
//! each step does one of those things, or a person or the outside acts; at any step the engine may stop, and the
//! driver then forgets what it held and takes `pending` instead.
//!
//! What is checked after every step:
//! - `pending` equals, as a set, what the driver holds (entries engine-holds-no-state, a-stop-changes-nothing);
//! - at most one round is under way, and the deciding code never says the records are faulty;
//! - every effect held names a run and a call the records hold, and no round is given twice;
//! - the times of the moments never go back, the driver's clock never going back, and each moment's entries begin
//!   after the last's;
//! - an event the loop refuses leaves the work equal to what it was;
//! - no finished run of a cell lacks a write of a block the cell must write, read from the records of the finishes and
//!   the cells' definitions (entry a-run-is-one-mechanism);
//! - a round that ended, failed or not, is never given to be done again: no round `pending` gives has ended; and while
//!   the engine's pause for a failed round holds the work, `pending` gives no round at all;
//! - a round after one that failed is given what the failed one was given, read from the records.
//!
//! And once people stop, the owner lets the work on if it is paused, for whatever reason, and no round fails any more:
//! the work then rests within
//! the bound stated on `History::bound` (entry a-work-comes-to-rest), at rest it is not paused and the walk waits or has
//! ended, and `pending` holds only calls for a person; a cancelled work ends with nothing pending, and a look at it
//! gives nothing. The same history ends with the same work run twice, and with its stops as without them, failed rounds
//! among them.
//!
//! What a step is, roughly. In the first 0 to 59 steps people act at a step with a chance of one in four; otherwise,
//! and always once they stop, the driver does one of the things it holds, each with an equal chance:
//! - a round under way fails in one in ten while people act, the engine then pausing the work; otherwise it ends: in
//!   half of them the agent hands in each block its cell may write with a chance of seven in ten, those its cell must
//!   write each time too, so that it leaves one out at times, and, in one in ten, a block it may not; a choice with one
//!   of its values in four of five, a wrong value or none otherwise; prose with a value in one in twenty; each mattering
//!   or not as a coin says. In three in ten it asks for one to three calls, a tool in seven of ten and a question for
//!   the owner or the architect otherwise; in two in ten it asks for nothing.
//! - a call has its result: always for a tool; for a person or the owner's call at a limit only while people act.
//! - a round given to be stopped ends, what it returns being dropped, or, while people act, fails in half of them; a
//!   call given to be withdrawn is said to have its result, that it was withdrawn.
//!
//! A person's act is, in about equal shares of a tenth: a write of a block people may write (a choice with one of its
//! values); a write that must be refused (of a block people may not write, of a choice with a wrong value, of a block
//! the flow does not have); the outside writing main, mattering or not; letting a random cell go on; pausing or
//! resuming, a resume letting on a work the engine paused for a failed round as well as one the owner paused; the
//! owner's word at the merge, a choice the merge's named test reads, written the other way; a result of a call that
//! has one already or of no call, or the return of a run that has no round under way; and, in one act in fifty,
//! cancelling the work. The owner's word is yes from the start in two histories of five. The engine stops at
//! any step with a chance of one in twelve, and with one in two at a step when the driver holds a stop or a withdrawal
//! not yet done.
//!
//! The histories must reach a failed round given again once the work is let on, and a run whose round failed abandoned
//! because what it started from changed before the work was let on, and works that came to rest not paused: each is
//! counted, a test of no history reaching one fails, and what was reached is written to the standard error.
//!
//! A failing case prints its seed and the steps it took; `one_history(seed, ..)` replays it.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::panic::{self, AssertUnwindSafe};

use decide::{CallId, Decision, Outsider, Pauser, RunId, Who, decide};
use engine::{Ask, Back, Call, Came, Effect, Event, Handed, Spent, Time, Work, handle, pending};
use flow::{Compiled, Has, Source, compile};

/// How many histories are run.
const SEEDS: u64 = 200;

/// How many steps, at most, people act in.
const PEOPLE: usize = 60;

/// A small deterministic generator (xorshift), so that a seed gives the same history every time.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(
            seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)
                .wrapping_add(0x2545_F491_4F6C_DD1D)
                | 1,
        )
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

/// The flow's file for a shape: its order, and the blocks its cells use, each declared with who else may write it.
fn flow_file(order: &[&str], blocks: &BTreeSet<&str>) -> String {
    let order = order.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ");
    let declared: String = (blocks.iter())
        .map(|b| {
            let who = match *b {
                "request" | "opinions" => "people = true\n",
                "choice" => "values = [\"yes\", \"no\"]\npeople = true\n",
                "main" => "outside = \"main\"\n",
                _ => "",
            };
            format!("[blocks.{b}]\n{who}shown = true\n")
        })
        .collect();
    format!(
        "name = \"shape\"\norder = [{order}]\n[limits]\nruns = 2\nreturns = 2\ncalls = 3\n[lines.main]\n\
         [tests.may_merge]\nsays = \"The owner let the work merge\"\nkind = \"all\"\nreads = [\"go\"]\nis = \"yes\"\n\
         [blocks.go]\nvalues = [\"yes\", \"no\"]\npeople = true\n{declared}"
    )
}

/// An agent's cell that reads the blocks `inputs`, may write `outputs` and must write `must` each time.
fn agent(inputs: &[&str], outputs: &[&str], must: &[&str], when: Option<&str>) -> (String, Vec<&'static str>) {
    let list = |l: &[&str]| l.iter().map(|b| format!("\"{b}\"")).collect::<Vec<_>>().join(", ");
    let when = when.map_or(String::new(), |w| format!("when = {w}\n"));
    let file = format!(
        "title = \"A cell\"\nkind = \"agent\"\nline = \"main\"\ntask = \"Do the cell's part.\"\n{when}\
         inputs = [{}]\noutputs = [{}]\nmust_write = [{}]\n",
        list(inputs),
        list(outputs),
        list(must)
    );
    let used = inputs.iter().chain(outputs).map(|b| block_name(b)).collect();
    (file, used)
}

/// The block's name as a constant, so that the blocks a shape uses can be gathered.
fn block_name(b: &str) -> &'static str {
    ["request", "main", "opinions", "plan", "notes", "choice", "merged"]
        .into_iter()
        .find(|n| *n == b)
        .expect("a block of the histories")
}

/// The merge, the engine's own cell, after the cell that last writes the plan; where `bound`, it must write what it
/// merged each time.
fn merge(bound: bool) -> String {
    let must = if bound { "\"merged\"" } else { "" };
    format!(
        "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\nrequires = [\"may_merge\"]\n\
         inputs = [\"plan\"]\noutputs = [\"merged\"]\nmust_write = [{must}]\n"
    )
}

/// One of the shapes of flow, each ending in the merge. Each cell that may be bound to write a block each time is,
/// where `binds` draws it so, about one time in two.
fn shape(n: usize, binds: &mut Rng) -> Compiled {
    let choice = Some(r#"{ block = "choice", is = "yes" }"#);
    let mut bound =
        |blocks: &'static [&'static str]| -> &'static [&'static str] { if binds.chance(50) { blocks } else { &[] } };
    let cells: Vec<(&str, (String, Vec<&'static str>))> = match n {
        // A chain, whose second cell may be bound to write the plan each time.
        0 => vec![
            ("draft", agent(&["request", "main"], &["notes"], &[], None)),
            ("plan", agent(&["notes"], &["plan"], bound(&["plan"]), None)),
        ],
        // A writer with a judge that writes opinions for it.
        1 => vec![
            ("write", agent(&["request", "opinions"], &["plan"], &[], None)),
            ("judge", agent(&["plan"], &["opinions"], &[], None)),
        ],
        // A cell with a condition on a choice an earlier cell makes, which that cell may be bound to make each time.
        2 => vec![
            (
                "ask",
                agent(&["request"], &["choice", "plan"], bound(&["choice"]), None),
            ),
            ("extra", agent(&["plan", "main"], &["notes"], &[], choice)),
        ],
        // A cell reading and writing one block, which it may be bound to write each time.
        _ => vec![
            ("draft", agent(&["request"], &["plan"], &[], None)),
            (
                "refine",
                agent(&["plan", "opinions"], &["plan"], bound(&["plan"]), None),
            ),
        ],
    };
    let mut order: Vec<&str> = cells.iter().map(|(id, _)| *id).collect();
    order.push("merge");
    let mut blocks: BTreeSet<&str> = cells.iter().flat_map(|(_, (_, used))| used.iter().copied()).collect();
    blocks.extend(["plan", "merged"]);
    let mut files: BTreeMap<String, String> = cells.iter().map(|(id, (f, _))| (id.to_string(), f.clone())).collect();
    files.insert("merge".to_string(), merge(binds.chance(50)));
    let source = Source {
        flow: flow_file(&order, &blocks),
        cells: files,
    };
    let has = Has {
        sources: ["main".to_string()].into(),
        acts: ["merge".to_string()].into(),
    };
    compile(&source, &has).expect("the shape compiles")
}

/// What the histories of all seeds did, so that every class of step is known to occur.
#[derive(Default, Debug)]
struct Seen {
    limits: usize,
    abandons: usize,
    withdrawn: usize,
    refused: usize,
    stops: usize,
    /// Stops of the engine while the driver held a stop or a withdrawal not yet done.
    stops_undoing: usize,
    /// Rounds stopped by a cancel.
    stopped: usize,
    /// Histories that ended cancelled.
    cancels: usize,
    /// Rounds that failed, of a run that went on.
    failed: usize,
    /// Rounds given again once the work was let on, the round before them having failed.
    retried: usize,
    /// Runs abandoned, while the work was not cancelled, after their last round failed.
    failed_abandoned: usize,
    /// Histories whose work came to rest, once people stopped, not paused.
    rested_unpaused: usize,
    /// Hand-ins that lacked a block their cell must write, by a run still open after them: sent back, or resting at
    /// the limit of returns.
    lacking: usize,
    /// Finished runs of cells that must write a block each time.
    bound_finished: usize,
}

/// A history: the work, what the driver holds to be done, and what it saw.
struct History {
    flow: Compiled,
    /// Whether the owner's word at the merge is yes from the start.
    yes_at_first: bool,
    /// The owner's word at the merge, as last written: yes, or no where it was never written.
    go: bool,
    work: Work,
    rng: Rng,
    /// Draws the stops alone, so that a history with stops makes the same choices as one without.
    stops: Option<Rng>,
    /// What the engine gave to be done and is not yet done, in a fixed order.
    held: Vec<Effect>,
    /// Every round given, by run and number.
    rounds: BTreeSet<(RunId, usize)>,
    /// Calls that were withdrawn or have their result.
    closed: Vec<CallId>,
    paused: bool,
    /// The driver's clock, in milliseconds; it never goes back.
    clock: u64,
    log: Vec<String>,
    seen: Seen,
}

/// A key that orders effects the same way however they came to be held.
fn key(effect: &Effect) -> String {
    format!("{effect:?}")
}

impl History {
    fn new(seed: u64, stops: bool) -> Self {
        let mut rng = Rng::new(seed);
        // Which cells are bound to write a block each time is drawn apart, so that the other draws stay as they were.
        let flow = shape(rng.below(4), &mut Rng::new(seed ^ 0xB10C_5EED_0FC3_1150));
        let yes_at_first = rng.chance(40);
        Self {
            flow,
            yes_at_first,
            go: false,
            work: Work::default(),
            rng,
            stops: stops.then(|| Rng::new(seed ^ 0x5DEE_CE66_D1CE_F00D)),
            held: Vec::new(),
            rounds: BTreeSet::new(),
            closed: Vec::new(),
            paused: false,
            clock: 1_800_000_000_000,
            log: Vec::new(),
            seen: Seen::default(),
        }
    }

    fn note(&mut self, what: String) {
        self.log.push(what);
    }

    /// Hands the event to the loop, and takes what is to be done, or checks a refusal left the work as it was.
    fn take(&mut self, event: Event) -> bool {
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

    /// What must hold after every step.
    fn check(&self) {
        let r = &self.work.records;
        let mut pending = pending(&self.work);
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
        if decide::pause(r).is_some_and(|p| p.by == Pauser::Engine) {
            assert!(
                !pending.iter().any(|e| matches!(e, Effect::Round { .. })),
                "a round is pending while the engine's pause for a failed round holds the work"
            );
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
        for finished in &r.finished {
            let started = r
                .started
                .iter()
                .find(|s| s.run == finished.run)
                .expect("a finished run started");
            let spec = (self.flow.cells.iter())
                .find(|c| c.id == started.cell)
                .expect("the cell is in the flow");
            for block in &spec.must_write {
                assert!(
                    finished.writes.iter().any(|w| w.block == *block),
                    "run {:?} of {} finished without writing {block}, which its cell must write",
                    finished.run,
                    started.cell
                );
            }
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
            }
        }
    }

    /// A round under way fails, or ends as its agent returns.
    fn end_round(&mut self, run: RunId, cell: &str, people: bool) {
        // A round fails only while people act, as the other faults of the first part do, so that once they stop a
        // work comes to rest by what its flow and its limits do.
        if people && self.rng.chance(10) {
            let spent = self.spent(run);
            let why = format!("the agent of {cell} could not start");
            let taken = self.take(Event::Failed { run, why, spent });
            assert!(taken, "the failure of a round under way was refused");
            return;
        }
        let place = self.flow.flow.place(&cell.into()).expect("the cell is in the flow");
        let outputs = self.flow.flow.cells()[place].outputs.clone();
        let came = match self.rng.below(10) {
            0..=4 => {
                let mut blocks = Vec::new();
                for b in &outputs {
                    if self.rng.chance(70) {
                        blocks.push(b.clone());
                    }
                }
                if self.rng.chance(10) {
                    blocks.push(["request", "merged", "opinions", "choice"][self.rng.below(4)].into());
                }
                let handed = (blocks.into_iter())
                    .map(|block| {
                        let value = if self.flow.blocks.get(&block).is_some_and(|b| b.values.is_some()) {
                            match self.rng.below(10) {
                                0..=7 => Some(if self.rng.chance(50) { "yes" } else { "no" }.to_string()),
                                8 => Some("maybe".to_string()),
                                _ => None,
                            }
                        } else if self.rng.chance(5) {
                            Some("yes".to_string())
                        } else {
                            None
                        };
                        Handed {
                            content: format!("{block} by {cell}"),
                            block,
                            matters: self.rng.chance(50),
                            value,
                        }
                    })
                    .collect();
                Came::HandsIn(handed)
            }
            5..=7 => {
                let n = 1 + self.rng.below(3);
                let asks = (0..n)
                    .map(|i| {
                        if self.rng.chance(70) {
                            Ask::Tool {
                                name: format!("tool {i}"),
                                input: "x".to_string(),
                            }
                        } else {
                            let who = if self.rng.chance(50) {
                                Who::Owner
                            } else {
                                Who::Architect
                            };
                            Ask::Person {
                                who,
                                question: "which?".to_string(),
                            }
                        }
                    })
                    .collect();
                Came::Asks(asks)
            }
            _ => Came::Asks(vec![]),
        };
        let must = &self.flow.cells[place].must_write;
        let lacks = match &came {
            Came::HandsIn(handed) => must.iter().any(|b| !handed.iter().any(|h| h.block == *b)),
            Came::Asks(_) => false,
        };
        let spent = self.spent(run);
        let taken = self.take(Event::Returned { run, came, spent });
        assert!(taken, "the return of a round under way was refused");
        let r = &self.work.records;
        let open = !r.finished.iter().any(|f| f.run == run) && !r.abandoned.iter().any(|a| a.run == run);
        if lacks && open {
            self.seen.lacking += 1;
        }
    }

    /// The call has its result.
    fn give_result(&mut self, call: CallId) {
        let taken = self.take(Event::Done {
            call,
            result: format!("the result of {}", call.0),
        });
        assert!(taken, "the result of an open call was refused");
        self.closed.push(call);
    }

    /// A person or the outside acts.
    fn person_acts(&mut self) {
        let event = match self.rng.below(100) {
            0..=1 => Event::Cancelled,
            2..=13 => {
                let block = ["request", "opinions", "choice"][self.rng.below(3)];
                let value = (block == "choice").then(|| if self.rng.chance(50) { "yes" } else { "no" }.to_string());
                let matters = self.rng.chance(50);
                wrote(block, Outsider::Person, matters, value)
            }
            14..=25 => match self.rng.below(3) {
                0 => wrote("plan", Outsider::Person, true, None),
                1 => wrote("choice", Outsider::Person, true, Some("maybe".to_string())),
                _ => wrote("nowhere", Outsider::Person, true, None),
            },
            26..=37 => wrote("main", Outsider::Outside, self.rng.chance(50), None),
            38..=49 => {
                let cells = self.flow.flow.cells();
                Event::LetGo {
                    cell: cells[self.rng.below(cells.len())].id.clone(),
                }
            }
            50..=61 => {
                if self.paused {
                    Event::Resumed
                } else {
                    Event::Paused {
                        by: Pauser::Owner,
                        why: None,
                    }
                }
            }
            62..=73 => self.owner_says(!self.go),
            74..=86 => {
                // A result of a call that has none to be given: one that has one already, or no call at all.
                let call = if !self.closed.is_empty() && self.rng.chance(70) {
                    self.closed[self.rng.below(self.closed.len())]
                } else {
                    CallId(10_000)
                };
                Event::Done {
                    call,
                    result: "late".to_string(),
                }
            }
            _ => Event::Returned {
                run: RunId(10_000),
                came: Came::Asks(vec![]),
                spent: Spent::default(),
            },
        };
        let cancel = event == Event::Cancelled;
        let taken = self.take(event);
        if cancel {
            assert!(taken, "a cancel was refused");
        }
    }

    /// The owner's word at the merge, written as the choice the merge's named test reads.
    fn owner_says(&mut self, go: bool) -> Event {
        self.go = go;
        wrote(
            "go",
            Outsider::Person,
            true,
            Some(if go { "yes" } else { "no" }.to_string()),
        )
    }

    /// What a round of the run took, as its agent reports it: told apart by the run and the clock.
    fn spent(&self, run: RunId) -> Spent {
        Spent {
            usd_micros: run.0,
            input_tokens: self.clock % 997,
            cached_tokens: 0,
            output_tokens: 1,
        }
    }

    /// The driver forgets what it held and takes what is pending instead.
    fn stop(&mut self) {
        self.note("the engine stops".into());
        let mut held = pending(&self.work);
        held.sort_by_key(key);
        self.held = held;
        self.seen.stops += 1;
    }

    /// One step. Says whether anything was done: once people stop, nothing is when the driver holds nothing it can do.
    fn step(&mut self, people: bool) -> bool {
        let undoing = (self.held.iter()).any(|e| matches!(e, Effect::Withdraw { .. } | Effect::Stop { .. }));
        if let Some(stops) = &mut self.stops
            && stops.chance(if undoing { 50 } else { 8 })
        {
            if undoing {
                self.seen.stops_undoing += 1;
            }
            self.stop();
        }
        if people && self.rng.chance(25) {
            self.person_acts();
            return true;
        }
        let doable: Vec<Effect> = (self.held.iter())
            .filter(|e| match e {
                Effect::Round { .. } => true,
                Effect::Call { what, .. } => people || matches!(what, Call::Asked(Ask::Tool { .. })),
                Effect::Withdraw { .. } | Effect::Stop { .. } => true,
            })
            .cloned()
            .collect();
        if doable.is_empty() {
            if people {
                self.person_acts();
                return true;
            }
            return false;
        }
        let effect = doable[self.rng.below(doable.len())].clone();
        let at = self.held.iter().position(|e| *e == effect).expect("held");
        self.held.remove(at);
        match effect {
            Effect::Round { run, cell, .. } => self.end_round(run, cell.as_str(), people),
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
                let end = if people && self.rng.chance(50) {
                    Event::Failed {
                        run,
                        why: "stopped".to_string(),
                        spent,
                    }
                } else {
                    Event::Returned {
                        run,
                        came: Came::Asks(vec![]),
                        spent,
                    }
                };
                assert!(self.take(end), "the end of a stopped round was refused");
            }
        }
        true
    }

    /// Whether the work is cancelled.
    fn cancelled(&self) -> bool {
        self.work.records.cancelled.is_some()
    }

    /// How many steps, once people stop, the work may take before it rests (entry a-work-comes-to-rest).
    ///
    /// Once people stop, the driver ends rounds and gives tools their results, and nothing else. A run's rounds are
    /// at most `returns + 1` that are sent back or hand in what finishes it, and `calls + 1` that ask for calls, each of
    /// which but the last makes at least one: past either limit the run waits for the owner, who no longer answers. A
    /// run takes at most four steps a round, the round's end and the results of up to three calls, and four to be
    /// undone, its round stopped and up to three calls withdrawn. A run of a cell is a
    /// way round unless one of its reasons is having no record or a change that people, the outside or a cell before
    /// it made; nobody lets a cell go on any more, so between two that are not a cell runs at most its limit `L` of
    /// ways round. Those that are not number at most one first run, one for each input (the last write of it by people
    /// or the outside), and one for each run of a cell before it, which may also abandon it once. So the cell at place
    /// `i` starts at most `S_i = (L + 2) * (3 + n_i + S_0 + ... + S_(i-1))` runs, `n_i` its inputs.
    fn bound(&self) -> u64 {
        let rounds = u64::from(self.flow.limits.returns + self.flow.limits.calls + 2);
        let mut before: u64 = 0;
        for c in self.flow.flow.cells() {
            let s = (u64::from(c.limit) + 2).saturating_mul(3 + c.inputs.len() as u64 + before);
            before = before.saturating_add(s);
        }
        (before + 1).saturating_mul(4 * rounds + 4)
    }
}

fn wrote(block: &str, by: Outsider, matters: bool, value: Option<String>) -> Event {
    Event::Wrote {
        block: block.into(),
        by,
        content: format!("{block} by {by:?}"),
        matters,
        value,
    }
}

/// One history, with or without stops: the work it ends with, what it saw and its steps.
fn one_history(seed: u64, stops: bool, log: &mut Vec<String>) -> (Work, Seen) {
    let mut h = History::new(seed, stops);
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let first = if h.yes_at_first {
            h.owner_says(true)
        } else {
            Event::Look
        };
        h.take(first);
        h.check();
        let people = h.rng.below(PEOPLE);
        for _ in 0..people {
            h.step(true);
            h.check();
        }
        h.note("people stop".into());
        // The owner lets the work on, whoever paused it.
        if h.paused {
            h.take(Event::Resumed);
            h.check();
        }
        let bound = h.bound();
        let mut steps = 0;
        while h.step(false) {
            h.check();
            steps += 1;
            assert!(steps <= bound, "the work has not come to rest within {bound} steps");
        }
        let decision = decide(&h.flow.flow, &h.work.records);
        let paused = decide::paused(&h.work.records);
        if !paused {
            h.seen.rested_unpaused += 1;
        }
        // Let on as people stopped, and failing no round since, the work rests not paused: it waits or has ended.
        assert!(!paused, "the work rests paused");
        assert!(
            matches!(decision, Decision::Wait { .. } | Decision::Ended(_)),
            "the work rests deciding {decision:?}"
        );
        for effect in pending(&h.work) {
            let Effect::Call { call, .. } = &effect else {
                panic!("at rest something other than a call is pending: {effect:?}");
            };
            let called = h.work.records.called.iter().find(|c| c.call == *call).expect("a call");
            assert!(called.on.is_some(), "at rest a call waits for nobody: {effect:?}");
        }
        // A work cancelled after it merged has merged.
        if h.cancelled() && decision != Decision::Ended(decide::End::Merged) {
            assert_eq!(
                decision,
                Decision::Ended(decide::End::Cancelled),
                "a cancelled work goes on"
            );
            assert_eq!(pending(&h.work), vec![], "a cancelled work has something pending");
            h.take(Event::Look);
            h.check();
            assert_eq!(h.held, vec![], "a cancelled work was given more");
            h.seen.cancels += 1;
        }
    }));
    h.seen.abandons = h.work.records.abandoned.len();
    let bound = |cell: &decide::CellId| (h.flow.cells.iter()).any(|c| c.id == *cell && !c.must_write.is_empty());
    h.seen.bound_finished = (h.work.records.finished.iter())
        .filter(|f| (h.work.records.started.iter()).any(|s| s.run == f.run && bound(&s.cell)))
        .count();
    *log = std::mem::take(&mut h.log);
    if let Err(e) = result {
        panic::resume_unwind(e);
    }
    (h.work, h.seen)
}

#[test]
fn whole_works_keep_the_engines_properties_with_and_without_stops() {
    // The hook is the whole process's: this test must stay alone in its binary, or it hides the panics of the others.
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let mut failures = Vec::new();
    let mut seen = Seen::default();
    for seed in 0..SEEDS {
        let mut log = Vec::new();
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
            let (stopped, s) = one_history(seed, true, &mut log);
            let mut quiet = Vec::new();
            let (straight, _) = one_history(seed, false, &mut quiet);
            assert_eq!(stopped, straight, "the history with stops ended otherwise than without");
            let (again, _) = one_history(seed, true, &mut quiet);
            assert_eq!(again, stopped, "the same history ended otherwise the second time");
            s
        }));
        match outcome {
            Ok(s) => {
                seen.limits += s.limits;
                seen.abandons += s.abandons;
                seen.withdrawn += s.withdrawn;
                seen.refused += s.refused;
                seen.stops += s.stops;
                seen.stops_undoing += s.stops_undoing;
                seen.stopped += s.stopped;
                seen.cancels += s.cancels;
                seen.failed += s.failed;
                seen.retried += s.retried;
                seen.failed_abandoned += s.failed_abandoned;
                seen.rested_unpaused += s.rested_unpaused;
                seen.lacking += s.lacking;
                seen.bound_finished += s.bound_finished;
            }
            Err(e) => {
                let message = (e.downcast_ref::<String>().cloned())
                    .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_string()))
                    .unwrap_or_default();
                failures.push((seed, message, log));
            }
        }
    }
    panic::set_hook(hook);
    if let Some((seed, message, log)) = failures.first() {
        let seeds: Vec<_> = failures.iter().map(|(s, m, _)| format!("{s}: {m}")).collect();
        panic!(
            "{} of {SEEDS} histories failed:\n{}\n\nthe steps of seed {seed} ({message}):\n{}",
            failures.len(),
            seeds.join("\n"),
            log.join("\n")
        );
    }
    let every = seen.limits > 0
        && seen.abandons > 0
        && seen.withdrawn > 0
        && seen.refused > 0
        && seen.stops > 0
        && seen.stops_undoing > 0
        && seen.stopped > 0
        && seen.cancels > 0
        && seen.lacking > 0
        && seen.bound_finished > 0
        && seen.failed > 0
        && seen.retried > 0
        && seen.failed_abandoned > 0
        && seen.rested_unpaused > 0;
    // What the histories reached, said whatever comes of the test: written to the standard error itself, which the
    // test harness does not hold back as it holds back what a test prints.
    let _ = writeln!(std::io::stderr(), "the engine's histories reached: {seen:?}");
    assert!(every, "a class of step never occurred: {seen:?}");
}
