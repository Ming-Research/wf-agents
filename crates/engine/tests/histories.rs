//! Simulated histories of whole works through `handle` only (principle simulated-histories): a driver holds what the
//! engine gave to be done and not yet done, and at each step does one of those things, or a person or the outside
//! acts, or the engine is stopped and the driver starts anew from `pending`. After every step what the engine must
//! always keep is checked; once people stop, the work must come to rest within a bound its limits give; and the same
//! history with and without stops ends with the same work.
//!
//! What a step is, roughly. In the first 0 to 59 steps people act at a step with a chance of one in four; otherwise,
//! and always once they stop, the driver does one of the things it holds, each with an equal chance:
//! - a round under way ends: in half of them the agent hands in each block its cell may write with a chance of seven
//!   in ten and, in one in ten, a block it may not; a choice with one of its values in four of five, a wrong value or
//!   none otherwise; prose with a value in one in twenty; each mattering or not as a coin says. In three in ten it asks
//!   for one to three calls, a tool in seven of ten and a question for the owner or the architect otherwise; in two in
//!   ten it asks for nothing.
//! - a call has its result: always for a tool; for a person or the owner's call at a limit only while people act.
//!
//! A person's act is, in about equal shares of a tenth: a write of a block people may write (a choice with one of its
//! values); a write that must be refused (of a block people may not write, of a choice with a wrong value, of a block
//! the flow does not have); the outside writing main, mattering or not; letting a random cell go on; pausing or
//! resuming; the owner's word at the merge coming out the other way; a result of a call that has none, was withdrawn
//! or has one already; and, in one act in fifty, cancelling the work, after which nothing more may be given. At any
//! step, with a chance of one in twelve, the engine stops.
//!
//! A failing case prints its seed and the steps it took; `one_history(seed, ..)` replays it.

use std::collections::{BTreeMap, BTreeSet};
use std::panic::{self, AssertUnwindSafe};

use decide::{CallId, Decision, Outcome, Outsider, RunId, TestName, Tests, Who, decide};
use engine::{Ask, Call, Came, Effect, Event, Handed, Work, handle, pending};
use flow::{Compiled, Source, compile};

/// How many histories are run.
const SEEDS: u64 = 300;

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

/// The flow's file, the same for every shape but for its order: the blocks every shape may use.
fn flow_file(order: &[&str]) -> String {
    let order = order.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ");
    format!(
        r#"
        name = "shape"
        order = [{order}]
        [limits]
        runs = 2
        returns = 2
        calls = 3
        [lines.main]
        [tests]
        may_merge = "The work may merge"
        [blocks.request]
        people = true
        [blocks.main]
        outside = true
        shown = true
        [blocks.opinions]
        people = true
        shown = true
        [blocks.plan]
        shown = true
        [blocks.notes]
        shown = true
        [blocks.choice]
        values = ["yes", "no"]
        people = true
        shown = true
        [blocks.merged]
        shown = true
        "#
    )
}

fn agent(inputs: &[&str], outputs: &[&str], when: Option<&str>) -> String {
    let list = |l: &[&str]| l.iter().map(|b| format!("\"{b}\"")).collect::<Vec<_>>().join(", ");
    let when = when.map_or(String::new(), |w| format!("when = {w}\n"));
    format!(
        "title = \"A cell\"\nkind = \"agent\"\nline = \"main\"\n{when}inputs = [{}]\noutputs = [{}]\n",
        list(inputs),
        list(outputs)
    )
}

/// The merge, the engine's own cell, after the cell that last writes the plan.
fn merge() -> String {
    "title = \"Merge\"\nkind = \"engine\"\nrequires = [\"may_merge\"]\ninputs = [\"plan\"]\noutputs = [\"merged\"]\n"
        .to_string()
}

/// One of the shapes of flow, each ending in the merge.
fn shape(n: usize) -> Compiled {
    let choice = Some(r#"{ block = "choice", is = "yes" }"#);
    let cells: Vec<(&str, String)> = match n {
        // A chain.
        0 => vec![
            ("draft", agent(&["request", "main"], &["notes"], None)),
            ("plan", agent(&["notes"], &["plan"], None)),
        ],
        // A writer with a judge that writes opinions for it.
        1 => vec![
            ("write", agent(&["request", "opinions"], &["plan"], None)),
            ("judge", agent(&["plan"], &["opinions"], None)),
        ],
        // A cell with a condition on a choice an earlier cell makes.
        2 => vec![
            ("ask", agent(&["request"], &["choice", "plan"], None)),
            ("extra", agent(&["plan", "main"], &["notes"], choice)),
        ],
        // A cell reading and writing one block.
        _ => vec![
            ("draft", agent(&["request"], &["plan"], None)),
            ("refine", agent(&["plan", "opinions"], &["plan"], None)),
        ],
    };
    let mut order: Vec<&str> = cells.iter().map(|(id, _)| *id).collect();
    order.push("merge");
    let mut files: BTreeMap<String, String> = cells.iter().map(|(id, f)| (id.to_string(), f.clone())).collect();
    files.insert("merge".to_string(), merge());
    let source = Source {
        flow: flow_file(&order),
        cells: files,
    };
    let known: BTreeSet<TestName> = ["may_merge".into()].into();
    compile(&source, &known).expect("the shape compiles")
}

struct Named(BTreeMap<TestName, Outcome>);

impl Tests for Named {
    fn test(&self, name: &TestName) -> Outcome {
        self.0.get(name).cloned().unwrap_or(Outcome::Holds)
    }
}

/// What the histories of all seeds did, so that every class of step is known to occur.
#[derive(Default, Debug)]
struct Seen {
    limits: usize,
    abandons: usize,
    withdrawn: usize,
    refused: usize,
    stops: usize,
}

/// A history: the work, what the driver holds to be done, and what it saw.
struct History {
    flow: Compiled,
    tests: Named,
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
        let flow = shape(rng.below(4));
        let holds = if rng.chance(40) {
            Outcome::Holds
        } else {
            Outcome::Waits { who: Who::Owner }
        };
        Self {
            flow,
            tests: Named([("may_merge".into(), holds)].into()),
            work: Work::default(),
            rng,
            stops: stops.then(|| Rng::new(seed ^ 0x5DEE_CE66_D1CE_F00D)),
            held: Vec::new(),
            rounds: BTreeSet::new(),
            closed: Vec::new(),
            paused: false,
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
        match handle(&self.flow, &self.tests, &mut self.work, event) {
            Ok(effects) => {
                self.note(format!("effects: {effects:?}"));
                for effect in effects {
                    match &effect {
                        Effect::Round { run, number, .. } => {
                            assert!(
                                self.rounds.insert((*run, *number)),
                                "round {number} of {run:?} given twice"
                            );
                        }
                        Effect::Withdraw { call } => {
                            let at = self
                                .held
                                .iter()
                                .position(|e| matches!(e, Effect::Call { call: c, .. } if c == call));
                            let at = at.expect("a withdrawn call was held");
                            self.held.remove(at);
                            self.closed.push(*call);
                            self.seen.withdrawn += 1;
                            continue;
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
        let under_way = (r.started.iter())
            .filter(|s| {
                let began = r.rounds.iter().filter(|x| x.run == s.run).count();
                let ended = r.returned.iter().filter(|x| x.run == s.run).count();
                began > ended
            })
            .count();
        assert!(under_way <= 1, "{under_way} rounds are under way");
        assert!(
            !matches!(decide(&self.flow.flow, r, &self.tests), Decision::Fault(_)),
            "the records are faulty"
        );
        for effect in &self.held {
            match effect {
                Effect::Round { run, .. } => assert!(r.started.iter().any(|s| s.run == *run), "an unknown run"),
                Effect::Call { call, run, .. } => {
                    assert!(
                        r.called.iter().any(|c| c.call == *call && c.run == *run),
                        "an unknown call"
                    );
                }
                Effect::Withdraw { .. } => panic!("a withdrawal is held"),
            }
        }
    }

    /// A round under way ends as its agent returns.
    fn end_round(&mut self, run: RunId, cell: &str) {
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
        let taken = self.take(Event::Returned { run, came });
        assert!(taken, "the return of a round under way was refused");
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
                self.paused = !self.paused;
                if self.paused { Event::Paused } else { Event::Resumed }
            }
            62..=73 => {
                let flipped = match self.tests.0.get(&TestName::from("may_merge")) {
                    Some(Outcome::Holds) => Outcome::Waits { who: Who::Owner },
                    _ => Outcome::Holds,
                };
                self.tests.0.insert("may_merge".into(), flipped);
                Event::Look
            }
            _ => {
                // A result of a call that has none to be given: one closed already, or none at all.
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
        };
        let cancel = event == Event::Cancelled;
        let taken = self.take(event);
        if cancel {
            assert!(taken, "a cancel was refused");
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
        if let Some(stops) = &mut self.stops
            && stops.chance(8)
        {
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
                Effect::Withdraw { .. } => false,
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
            Effect::Round { run, cell, .. } => self.end_round(run, cell.as_str()),
            Effect::Call { call, .. } => self.give_result(call),
            Effect::Withdraw { .. } => unreachable!(),
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
    /// run takes at most four steps a round: the round's end and the results of up to three calls. A run of a cell is a
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
        (before + 1).saturating_mul(4 * rounds)
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
        h.take(Event::Look);
        h.check();
        let people = h.rng.below(PEOPLE);
        for _ in 0..people {
            h.step(true);
            if h.cancelled() {
                break;
            }
            h.check();
        }
        h.note("people stop".into());
        if h.cancelled() {
            // Nothing more is given to a cancelled work.
            let before = h.held.clone();
            h.take(Event::Look);
            assert_eq!(h.held, before, "a cancelled work was given more");
            return;
        }
        let bound = h.bound();
        let mut steps = 0;
        while h.step(false) {
            h.check();
            steps += 1;
            assert!(steps <= bound, "the work has not come to rest within {bound} steps");
        }
        let decision = decide(&h.flow.flow, &h.work.records, &h.tests);
        assert!(
            matches!(decision, Decision::Wait { .. } | Decision::Paused | Decision::Ended(_)),
            "the work rests deciding {decision:?}"
        );
        for effect in pending(&h.work) {
            let Effect::Call { call, .. } = &effect else {
                panic!("at rest a round is pending: {effect:?}");
            };
            let called = h.work.records.called.iter().find(|c| c.call == *call).expect("a call");
            assert!(called.on.is_some(), "at rest a call waits for nobody: {effect:?}");
        }
    }));
    h.seen.abandons = h.work.records.abandoned.len();
    *log = std::mem::take(&mut h.log);
    if let Err(e) = result {
        panic::resume_unwind(e);
    }
    (h.work, h.seen)
}

#[test]
fn whole_works_keep_the_engines_properties_with_and_without_stops() {
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
    let every = seen.limits > 0 && seen.abandons > 0 && seen.withdrawn > 0 && seen.refused > 0 && seen.stops > 0;
    assert!(every, "a class of step never occurred: {seen:?}");
}
