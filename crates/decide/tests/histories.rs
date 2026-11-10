//! Random histories (principle simulated-histories): many works, each on a small flow of blocks drawn at random, driven
//! step by step as an engine would drive them, agents returning at random and people acting at random in the first
//! part of each history. After every step the properties the engine must always keep are checked; once people stop,
//! the work must come to rest within a bound its limits give.
//!
//! What the driver reaches, roughly:
//! - the flow: each of the five shapes in a fifth of the histories, cut to its first two cells in a fifth and
//!   lengthened with cells drawn at random in three fifths. A drawn cell reads each block (main among them, which only
//!   the outside writes) with a chance of one in three and writes each with one in four; it runs only when the choice
//!   is yes in one in five, and only while the named test "flip" holds in one in seven. A cell other than the merge has
//!   the precondition "ready" in one history of four, holding at first in half of those; the merge's precondition holds
//!   in two histories of five.
//! - people, in the first 0 to 39 steps, at a step with a chance of one in six and at every rest, in about equal shares
//!   of a tenth: writing the brief; writing prose into a block cells write (an opinions block in a third of those, the
//!   choice without a value in a sixth); writing the same marked as not mattering; setting the choice to yes or no;
//!   confirming the choice; the outside writing main, mattering or not; flipping "flip" or "ready"; letting a cell go
//!   on; pausing or resuming; and, once in a hundred acts, cancelling the work, after which nothing more is decided.
//!   At a rest they answer a call for them, or let a cell go on at its limit, three times in five.
//! - agents: a round ends finishing in half the rounds (each output written with a chance of three in five, mattering
//!   or not as a coin says, the choice as yes or no in four of five and without a value in one), returning with
//!   nothing in a quarter, and resting at a call in a quarter (the owner's in two of ten, the architect's in one, the
//!   engine's otherwise); its third round always finishes.
//!
//! A failing case prints its seed and the steps it took; `one_history(seed, ..)` replays it.

// The histories use only part of the helper the rule tests share.
#[allow(dead_code)]
mod support;

use std::panic::{self, AssertUnwindSafe};

use decide::{BlockId, Cell, CellId, Condition, Decision, End, Outcome, RunId, Seq, Waited, Who};
use support::Says::{DoesNotMatter, Matters, Value};
use support::{Says, World, cell};

/// How many histories are run.
const SEEDS: u64 = 800;

/// The block only people write.
const BRIEF: &str = "brief";

/// The block whose value a condition reads.
const CHOICE: &str = "choice";

/// The block only the outside writes.
const MAIN: &str = "main";

/// The blocks cells may write: prose, opinions for a cell, and the choice.
const WRITTEN: [&str; 6] = ["b0", "b1", "b2", "op0", "op1", CHOICE];

/// The most rounds a run has before its agent finishes: within a run, how often a return is sent back is bounded by the
/// engine, outside the deciding code.
const ROUNDS: usize = 3;

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

fn when_chosen(mut c: Cell) -> Cell {
    c.applies = Some(Condition::Is {
        block: CHOICE.into(),
        value: "yes".into(),
    });
    c
}

/// The cells of a flow before its merge: a shape, cut or lengthened with cells drawn at random, each with a limit of
/// one or two.
fn draw_cells(rng: &mut Rng) -> Vec<Cell> {
    let mut cells = match rng.below(5) {
        // A chain, each cell reading what the one before it writes.
        0 => vec![
            cell("c0", &[BRIEF], &["b0"]),
            cell("c1", &["b0"], &["b1"]),
            cell("c2", &["b1"], &["b2"]),
        ],
        // A judge writing opinions for the cell before it, which reads them.
        1 => vec![
            cell("c0", &[BRIEF, "op0"], &["b0"]),
            cell("c1", &["b0"], &["op0"]),
            cell("c2", &["b0"], &["b1"]),
        ],
        // A cell run only when the one before it chose so.
        2 => vec![
            cell("c0", &[BRIEF], &["b0", CHOICE]),
            when_chosen(cell("c1", &["b0"], &["b1"])),
            cell("c2", &["b0", "b1"], &["b2"]),
        ],
        // Two writers of one block.
        3 => vec![
            cell("c0", &[BRIEF], &["b0"]),
            cell("c1", &[BRIEF], &["b0"]),
            cell("c2", &["b0"], &["b1"]),
        ],
        // A cell reading and writing one block.
        _ => vec![
            cell("c0", &[BRIEF], &["b0"]),
            cell("c1", &["b0"], &["b0", "b1"]),
            cell("c2", &["b1"], &[]),
        ],
    };
    let n = 2 + rng.below(5);
    cells.truncate(n);
    while cells.len() < n {
        let i = cells.len();
        let inputs: Vec<&str> = [BRIEF, MAIN]
            .into_iter()
            .chain(WRITTEN)
            .filter(|_| rng.chance(33))
            .collect();
        let outputs: Vec<&str> = WRITTEN.into_iter().filter(|_| rng.chance(25)).collect();
        let mut c = cell(&format!("c{i}"), &inputs, &outputs);
        if rng.chance(20) && !outputs.contains(&CHOICE) {
            c = when_chosen(c);
        } else if rng.chance(15) {
            c.applies = Some(Condition::Test("flip".into()));
        }
        cells.push(c);
    }
    for c in &mut cells {
        c.limit = 1 + rng.below(2) as u32;
    }
    if rng.chance(25) {
        let i = rng.below(cells.len());
        cells[i].requires.push("ready".into());
    }
    cells
}

struct History {
    w: World,
    rng: Rng,
    log: Vec<String>,
    /// Whether the work is paused, as people last left it.
    paused: bool,
    /// The latest decision that was neither `Busy` nor an end.
    last_move: Option<Decision>,
    /// Whether a person cancelled the work.
    cancelled: bool,
}

impl History {
    fn new(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let cells = draw_cells(&mut rng);
        let mut w = World::with_gate(cells);
        if rng.chance(40) {
            w.set("the merge may go", Outcome::Holds);
        }
        if rng.chance(50) {
            w.set("ready", Outcome::Waits { who: Who::Architect });
        }
        let log = vec![
            format!("flow: {:?}", w.flow.cells()),
            format!("named tests: {:?}", w.named.0),
        ];
        Self {
            w,
            rng,
            log,
            paused: false,
            last_move: None,
            cancelled: false,
        }
    }

    fn note(&mut self, what: String) {
        self.log.push(what);
    }

    /// One step: people may act first, then the engine applies the decision. Says whether the work moved; it has
    /// not when it rests waiting for people who no longer act, is paused, or has ended.
    fn step(&mut self, people: bool) -> bool {
        if people && self.rng.chance(16) {
            self.person_acts(None, &[]);
        }
        let decision = self.w.decide();
        self.note(format!("decision: {decision:?}"));
        self.check(&decision);
        let moved = match &decision {
            Decision::Fault(fault) => panic!("the records are said to be faulty: {fault:?}"),
            Decision::Ended(End::Merged) => {
                let merge = self.w.flow.cells().last().expect("a merge").id.clone();
                match &self.last_move {
                    Some(Decision::Run { cell, .. }) if *cell == merge => {}
                    other => panic!("ended without the merge having run just before: {other:?}"),
                }
                false
            }
            Decision::Ended(End::Cancelled) => {
                assert!(self.cancelled, "the work ended cancelled though no one cancelled it");
                // Nothing that happens after decides anything any more.
                self.w.person_writes(BRIEF);
                assert_eq!(
                    self.w.decide(),
                    Decision::Ended(End::Cancelled),
                    "a cancelled work went on"
                );
                false
            }
            Decision::Paused => {
                if people {
                    self.toggle_pause();
                }
                people
            }
            Decision::Busy { cell, run } => {
                self.end_round(cell.clone(), *run);
                true
            }
            Decision::Run { cell, .. } => {
                self.w.start(cell.as_str());
                true
            }
            Decision::GoOn { cell, .. } => {
                self.w.go_on(cell.as_str());
                true
            }
            Decision::Abandon { .. } => {
                self.w.abandon();
                true
            }
            Decision::Wait { at, on } => {
                assert!(!on.is_empty(), "a wait names nothing waited for");
                let engine = on.iter().find_map(|o| match o {
                    Waited::Call { call, on: None } => Some(*call),
                    _ => None,
                });
                if let Some(call) = engine {
                    self.note(format!("the engine's call {call:?} is done"));
                    self.w.done(call);
                    true
                } else if people {
                    self.person_acts(Some(at), on);
                    true
                } else {
                    false
                }
            }
        };
        if !matches!(decision, Decision::Busy { .. } | Decision::Ended(_)) {
            self.last_move = Some(decision);
        }
        moved
    }

    /// What must hold after every step.
    fn check(&self, decision: &Decision) {
        let r = &self.w.records;
        let under_way = (r.started.iter())
            .filter(|s| {
                let began = r.rounds.iter().filter(|x| x.run == s.run).count();
                let ended = r.returned.iter().filter(|x| x.run == s.run).count();
                began > ended
            })
            .count();
        assert!(under_way <= 1, "{under_way} runs have a round under way");
        if under_way == 1 {
            assert!(
                // A cancel holds wherever the work is, a round under way or not.
                matches!(decision, Decision::Busy { .. } | Decision::Ended(End::Cancelled)),
                "a round is under way and the walk decides {decision:?}"
            );
        }
    }

    /// The round under way of the run ends as its agent returns: it finishes, returns with nothing, or rests at a call.
    /// A run whose last round ends as it has become stale is abandoned, not finished: the engine asks the walk.
    fn end_round(&mut self, cell: CellId, run: RunId) {
        assert_eq!(
            self.w.open(cell.as_str()),
            run,
            "the run under way is the cell's open run"
        );
        let spec = (self.w.flow.cells().iter())
            .find(|c| c.id == cell)
            .expect("the cell is in the flow")
            .clone();
        let is_merge = self.w.flow.cells().last().is_some_and(|c| c.id == cell);
        let rounds = self.w.records.rounds.iter().filter(|r| r.run == run).count();
        let how = if is_merge || rounds >= ROUNDS {
            0
        } else {
            self.rng.below(4)
        };
        match how {
            0 | 1 => {
                let mut writes: Vec<(&str, Says)> = Vec::new();
                for b in &spec.outputs {
                    if self.rng.chance(60) {
                        let block = WRITTEN.into_iter().find(|w| *w == b.as_str()).expect("a known block");
                        let says = if block == CHOICE && self.rng.chance(80) {
                            Value(if self.rng.chance(50) { "yes" } else { "no" })
                        } else if self.rng.chance(50) {
                            Matters
                        } else {
                            DoesNotMatter
                        };
                        writes.push((block, says));
                    }
                }
                self.w.returns(cell.as_str());
                if let Decision::Abandon { run: stale, .. } = self.w.decide()
                    && stale == run
                {
                    self.note(format!("{cell} returns, stale: the engine abandons it"));
                    self.w.abandon();
                } else {
                    self.note(format!(
                        "{cell} finishes, writing {:?}",
                        writes.iter().map(|(b, _)| b).collect::<Vec<_>>()
                    ));
                    self.w.finish(cell.as_str(), &writes);
                }
            }
            2 => {
                self.note(format!("{cell} returns with nothing"));
                self.w.returns(cell.as_str());
            }
            _ => {
                let on = match self.rng.below(10) {
                    0 | 1 => Some(Who::Owner),
                    2 => Some(Who::Architect),
                    _ => None,
                };
                self.note(format!("{cell} calls on {on:?}"));
                self.w.calls(cell.as_str(), on);
            }
        }
    }

    fn toggle_pause(&mut self) {
        if self.paused {
            self.note("a person lets the work on".into());
            self.w.resume();
        } else {
            self.note("a person pauses the work".into());
            self.w.pause();
        }
        self.paused = !self.paused;
    }

    /// The choice's current value: that of its latest write.
    fn choice(&self) -> Option<String> {
        let r = &self.w.records;
        let by_cells =
            (r.finished.iter()).flat_map(|f| f.writes.iter().map(move |x| (f.at, x.block.clone(), x.value.clone())));
        let by_people = r.written.iter().map(|x| (x.at, x.block.clone(), x.value.clone()));
        let latest: Option<(Seq, BlockId, Option<String>)> = (by_cells.chain(by_people))
            .filter(|(_, b, _)| b.as_str() == CHOICE)
            .max_by_key(|(at, _, _)| *at);
        latest.and_then(|(_, _, v)| v)
    }

    /// A person acts: on what the work waits for, where there is something, or at random.
    fn person_acts(&mut self, at: Option<&CellId>, waited: &[Waited]) {
        for o in waited {
            match o {
                Waited::Call { call, on: Some(_) } if self.rng.chance(60) => {
                    self.note(format!("a person answers the call {call:?}"));
                    self.w.done(*call);
                    return;
                }
                Waited::Limit { .. } if self.rng.chance(60) => {
                    let at = at.expect("a limit is waited for at a cell").clone();
                    self.note(format!("the owner lets {at} go on"));
                    self.w.let_go(at.as_str());
                    return;
                }
                _ => {}
            }
        }
        let prose = WRITTEN[self.rng.below(WRITTEN.len())];
        match self.rng.below(100) {
            0 => {
                self.note("a person cancels the work".into());
                self.cancelled = true;
                self.w.cancel();
            }
            1..=10 => {
                self.note(format!("a person writes {BRIEF}"));
                self.w.person_writes(BRIEF);
            }
            11..=20 => {
                self.note(format!("a person writes prose into {prose}"));
                self.w.person_writes(prose);
            }
            21..=30 => {
                self.note(format!("a person writes prose into {prose}, marked as not mattering"));
                self.w.person_writes_not_mattering(prose);
            }
            31..=40 => {
                let value = if self.rng.chance(50) { "yes" } else { "no" };
                self.note(format!("a person sets {CHOICE} to {value}"));
                self.w.person_sets(CHOICE, value);
            }
            41..=50 => {
                if let Some(value) = self.choice() {
                    self.note(format!("a person confirms {CHOICE} as {value}"));
                    self.w.person_confirms(CHOICE, &value);
                }
            }
            51..=60 => {
                if self.rng.chance(50) {
                    self.note(format!("the outside writes {MAIN}"));
                    self.w.outside_writes(MAIN);
                } else {
                    self.note(format!("the outside writes {MAIN}, approved as not mattering"));
                    self.w.outside_writes_not_mattering(MAIN);
                }
            }
            61..=70 => {
                let test = if self.rng.chance(50) { "flip" } else { "ready" };
                let outcome = if self.rng.chance(50) {
                    Outcome::Holds
                } else {
                    Outcome::Waits { who: Who::Architect }
                };
                self.note(format!("the test {test} comes to {outcome:?}"));
                self.w.set(test, outcome);
            }
            71..=80 => {
                let cells = self.w.flow.cells();
                let c = cells[self.rng.below(cells.len())].id.clone();
                self.note(format!("the owner lets {c} go on"));
                self.w.let_go(c.as_str());
            }
            _ => self.toggle_pause(),
        }
    }

    /// How many steps, once people stop, the work may take before it must wait or end (entry a-work-comes-to-rest).
    ///
    /// Once people stop, a run of a cell is a way round unless one of its reasons is the cell having no record, a
    /// block whose current version a person wrote before they stopped, or one a cell before it wrote. Nobody lets a
    /// cell go on any more, so between two runs that are not ways round a cell runs at most its limit of ways round.
    /// The runs that are not ways round number at most one for the first run, one for each input (the last write of
    /// it by a person), and one for each run of a cell before it. A run is abandoned only when a block it started
    /// from changed or its cell became skipped, which once people stop only a run of a cell before it can do (a cell
    /// after it does not run while it is at rest), so its abandoned runs are at most as many as those. With the one
    /// run open when people stopped, the cell at place `i`, of limit `L_i` and `n_i` inputs, starts at most
    /// `S_i = (L_i + 2) * (3 + n_i + S_0 + ... + S_(i-1))` runs. A run takes at most `4 * ROUNDS` steps: each of its
    /// rounds ends, waits for an engine's call and goes on, and it starts or is abandoned once.
    fn bound(&self) -> u64 {
        let mut before: u64 = 0;
        for c in self.w.flow.cells() {
            let s = (u64::from(c.limit) + 2).saturating_mul(3 + c.inputs.len() as u64 + before);
            before = before.saturating_add(s);
        }
        (before + 1).saturating_mul(4 * ROUNDS as u64)
    }
}

/// One history: people act in its first part, then the work is driven until it rests.
fn one_history(seed: u64, log: &mut Vec<String>) {
    let mut h = History::new(seed);
    let people_until = h.rng.below(40);
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        for _ in 0..people_until {
            h.step(true);
            if matches!(h.w.decide(), Decision::Ended(_)) {
                break;
            }
        }
        h.note("people stop".into());
        if h.cancelled {
            return;
        }
        let bound = h.bound();
        let mut steps = 0;
        while h.step(false) {
            steps += 1;
            assert!(steps <= bound, "the work has not come to rest within {bound} steps");
        }
    }));
    *log = std::mem::take(&mut h.log);
    if let Err(e) = result {
        panic::resume_unwind(e);
    }
}

#[test]
fn random_works_keep_the_engines_properties_and_come_to_rest() {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let mut failures = Vec::new();
    for seed in 0..SEEDS {
        let mut log = Vec::new();
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| one_history(seed, &mut log)));
        if let Err(e) = outcome {
            let message = (e.downcast_ref::<String>().cloned())
                .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_string()))
                .unwrap_or_default();
            failures.push((seed, message, log));
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
}
