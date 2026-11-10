//! Random histories (principle simulated-histories): many works, each on a small flow of blocks drawn at random, driven
//! step by step as an engine would drive them, agents returning at random and people acting at random in the first
//! part of each history. After every step the properties the engine must always keep are checked (`properties`),
//! each stating what a rule of the entries protects and judged by the histories' own reading of the records
//! (`reading`), not by the walk's; once people stop, the work must come to rest within a bound its limits give, and
//! once it rests, what people wrote must have reached the cells that read it.
//!
//! What the driver reaches, roughly:
//! - the flow: each of the six shapes in a sixth of the histories, cut to its first two cells in a fifth and
//!   lengthened with cells drawn at random in three fifths. A drawn cell reads each block (main among them, which only
//!   the outside writes) with a chance of one in three and writes each with one in four; it runs only when the choice
//!   is yes in one in five, and only while the named test "flip" holds in one in seven. A cell other than the merge has
//!   the precondition "ready" in one history of four, holding at first in half of those; "flip" fails at first in one
//!   history of four; the merge's precondition holds in two histories of five. The shapes give a block both a person
//!   and a cell write (every block cells write, since people write them too), a block two cells write of which the
//!   later does not read it, a judge writing opinions for a cell before it, and that cell skipped while the judge runs.
//!   The flow does not change during a history: the walk's tests of a changed flow are in `walk/changed_flow.rs`.
//! - people, in the first 0 to 39 steps, at a step with a chance of one in six and at every rest, in about equal shares
//!   of a tenth: writing the brief; writing prose into a block cells write (an opinions block in a third of those, the
//!   choice without a value in a sixth); writing the same marked as not mattering; setting the choice to yes or no;
//!   confirming the choice; the outside writing main, mattering or not; turning "flip" or "ready"; letting a cell go
//!   on; pausing or resuming; and, once in a hundred acts, cancelling the work, after which nothing more is decided.
//!   Half the prose goes where it bears on what is going on: into an input of the cell the work waits at or of a cell
//!   that is skipped, or into a block the open run's cell may write, so that people write while a round is under way
//!   and while a run is at rest, and write for a cell while it is skipped.
//!   At a rest they answer a call for them, or let a cell go on at its limit, three times in five.
//! - agents: a round ends finishing in half the rounds (each output written with a chance of three in five, mattering
//!   or not as a coin says, the choice as yes or no in four of five and without a value in one), returning with
//!   nothing in a quarter, and resting at a call in a quarter (the owner's in two of ten, the architect's in one, the
//!   engine's otherwise); its third round always finishes. A run found stale as its round ends is abandoned, not
//!   finished, as the engine does.
//!
//! Every situation a property is about is counted, and the test fails if no history reaches one of them.
//!
//! A failing case prints its seed, the properties it broke and the steps it took; `one_history(seed, ..)` replays it.

// The histories use only part of the helper the rule tests share.
#[allow(dead_code)]
#[path = "../support/mod.rs"]
mod support;

mod properties;
mod reading;

use std::collections::BTreeMap;
use std::panic::{self, AssertUnwindSafe};

use decide::{Abandoned, BlockId, Cell, CellId, Condition, Decision, End, Outcome, RunId, Seq, Waited, Who};
use properties::{Moment, Seen, Verdict};
use reading::{Reading, Writer};
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

fn when_flipped(mut c: Cell) -> Cell {
    c.applies = Some(Condition::Test("flip".into()));
    c
}

/// The cells of a flow before its merge: a shape, cut or lengthened with cells drawn at random, each with a limit of
/// one or two.
fn draw_cells(rng: &mut Rng) -> Vec<Cell> {
    let mut cells = match rng.below(6) {
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
        // Two writers of one block, the later not reading it and run for main: the earlier may run, for the brief,
        // while the later's run is open.
        3 => vec![
            cell("c0", &[BRIEF], &["b0"]),
            cell("c1", &[MAIN], &["b0"]),
            cell("c2", &["b0"], &["b1"]),
        ],
        // A cell reading and writing one block.
        4 => vec![
            cell("c0", &[BRIEF], &["b0"]),
            cell("c1", &["b0"], &["b0", "b1"]),
            cell("c2", &["b1"], &[]),
        ],
        // A cell run only when the one before it chose so, and a judge after it writing opinions for it and reading
        // the brief: each write of the brief chooses again, and the judge may run, and write over what a person wrote
        // for the cell, while the cell is skipped.
        _ => vec![
            cell("c0", &[BRIEF], &[CHOICE]),
            when_chosen(cell("c1", &[BRIEF, "op0"], &["b0"])),
            cell("c2", &[BRIEF, "b0"], &["op0"]),
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
            c = when_flipped(c);
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
    /// What the properties found.
    verdict: Verdict,
    /// Each decision the driver took, with what it was taken on.
    moments: Vec<Moment>,
    /// How many of the runs started, finished and abandoned the properties have looked at.
    looked_at: (usize, usize, usize),
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
        if rng.chance(25) {
            w.set("flip", Outcome::Waits { who: Who::Architect });
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
            verdict: Verdict::default(),
            moments: Vec::new(),
            looked_at: (0, 0, 0),
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
        self.moments.push(Moment {
            upto: properties::last_place(&self.w.records),
            named: self.w.named.0.clone(),
            decision: decision.clone(),
        });
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
            // A pause holds back the next round of a run that goes on: that is the run's to say, and the engine holds
            // it until the work is let on.
            Decision::GoOn { .. } if decide::paused(&self.w.records) => {
                if people {
                    self.toggle_pause();
                }
                people
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

    /// What must hold after every step: no two rounds under way, and the properties, of the decision just taken and
    /// of each run that started, finished or was abandoned since the last step.
    fn check(&mut self, decision: &Decision) {
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
                // A cancel holds wherever the work is, a round under way or not: the run is abandoned.
                matches!(
                    decision,
                    Decision::Busy { .. }
                        | Decision::Abandon {
                            because: decide::Abandoning::Cancelled,
                            ..
                        }
                ),
                "a round is under way and the walk decides {decision:?}"
            );
        }
        let reading = Reading::new(r);
        let flow = &self.w.flow;
        let v = &mut self.verdict;
        let (started, finished, abandoned) = self.looked_at;
        for s in &r.started[started..] {
            properties::started(v, flow, &reading, s);
        }
        for f in &r.finished[finished..] {
            properties::ended(v, flow, &reading, f.run, f.at);
        }
        for a in &r.abandoned[abandoned..] {
            properties::ended(v, flow, &reading, a.run, a.at);
        }
        self.looked_at = (r.started.len(), r.finished.len(), r.abandoned.len());
        let seed = properties::last_place(r).0;
        properties::decision(v, flow, &self.w.named, &reading, decision, decide::paused(r), seed);
    }

    /// The round under way of the run ends as its agent returns: it finishes, returns with nothing, or rests at a call.
    /// A run whose last round ends as it has become stale is abandoned, not finished: the engine asks whether this run
    /// is stale.
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
                if let Some(because) = decide::stale(&self.w.flow, &self.w.records, &self.w.named, run) {
                    self.note(format!("{cell} returns, stale ({because:?}): the engine abandons it"));
                    let reading = Reading::new(&self.w.records);
                    properties::abandoned(&mut self.verdict, &self.w.flow, &self.w.named, &reading, run, &because);
                    let at = self.w.tick();
                    self.w.records.abandoned.push(Abandoned { run, at });
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

    /// The cell of the run that has neither finished nor been abandoned, the latest started if several have not, and
    /// whether a round of it is under way.
    fn open_run(&self) -> Option<(CellId, bool)> {
        let r = &self.w.records;
        (r.started.iter().rev())
            .find(|s| !r.finished.iter().any(|f| f.run == s.run) && !r.abandoned.iter().any(|a| a.run == s.run))
            .map(|s| {
                let began = r.rounds.iter().filter(|x| x.run == s.run).count();
                let ended = r.returned.iter().filter(|x| x.run == s.run).count();
                (s.cell.clone(), began > ended)
            })
    }

    /// Counts a person's write by what is going on as it is made.
    fn person_writes_now(&mut self) {
        match self.open_run() {
            Some((_, true)) => self.verdict.count(properties::PERSON_UNDER_WAY),
            Some((_, false)) => self.verdict.count(properties::PERSON_AT_REST),
            None => {}
        }
    }

    /// The block a person writes prose into: in half the acts one that bears on what is going on, an input of the cell
    /// the work waits at, a block the open run's cell may write or an input of a cell that is skipped, and otherwise
    /// any block cells write.
    fn prose_block(&mut self, at: Option<&CellId>) -> &'static str {
        let open = self.open_run().map(|(cell, _)| cell);
        let reading = Reading::new(&self.w.records);
        let mut bearing: Vec<&BlockId> = Vec::new();
        for c in self.w.flow.cells() {
            if Some(&c.id) == at || !reading.applies(c, &self.w.named) {
                bearing.extend(&c.inputs);
            }
            if Some(&c.id) == open.as_ref() {
                bearing.extend(&c.outputs);
            }
        }
        let bearing: Vec<&'static str> = (WRITTEN.into_iter())
            .filter(|b| bearing.iter().any(|x| x.as_str() == *b))
            .collect();
        if !bearing.is_empty() && self.rng.chance(50) {
            return bearing[self.rng.below(bearing.len())];
        }
        WRITTEN[self.rng.below(WRITTEN.len())]
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
        let prose = self.prose_block(at);
        match self.rng.below(100) {
            0 => {
                self.note("a person cancels the work".into());
                self.cancelled = true;
                self.w.cancel();
            }
            1..=10 => {
                self.note(format!("a person writes {BRIEF}"));
                self.person_writes_now();
                self.w.person_writes(BRIEF);
            }
            11..=20 => {
                self.note(format!("a person writes prose into {prose}"));
                self.person_writes_now();
                self.w.person_writes(prose);
            }
            21..=30 => {
                self.note(format!("a person writes prose into {prose}, marked as not mattering"));
                self.person_writes_now();
                self.w.person_writes_not_mattering(prose);
            }
            31..=40 => {
                let value = if self.rng.chance(50) { "yes" } else { "no" };
                self.note(format!("a person sets {CHOICE} to {value}"));
                self.person_writes_now();
                self.w.person_sets(CHOICE, value);
            }
            41..=50 => {
                if let Some(value) = self.choice() {
                    self.note(format!("a person confirms {CHOICE} as {value}"));
                    self.person_writes_now();
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
                let outcome = match self.w.named.0.get(&test.into()) {
                    Some(Outcome::Waits { .. }) => Outcome::Holds,
                    _ => Outcome::Waits { who: Who::Architect },
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

    /// Counts what the whole history reached: a block both a person and a cell wrote, and a block two cells wrote of
    /// which the later in the order does not read it.
    fn count_blocks(&mut self) {
        let reading = Reading::new(&self.w.records);
        let cells = self.w.flow.cells();
        for (block, versions) in &reading.versions {
            let writers: Vec<usize> = (versions.iter())
                .filter_map(|v| match &v.by {
                    Writer::Cell(c, _) => self.w.flow.place(c),
                    _ => None,
                })
                .collect();
            if versions.iter().any(|v| v.by == Writer::Person) && !writers.is_empty() {
                self.verdict.count(properties::PERSON_AND_CELL);
            }
            if (writers.iter()).any(|&i| writers.iter().any(|&j| j > i && !cells[j].inputs.contains(block))) {
                self.verdict.count(properties::TWO_CELLS_LATER_UNREAD);
            }
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

/// One history: people act in its first part, then the work is driven until it rests, and what must hold of the whole
/// history is checked.
fn one_history(seed: u64, log: &mut Vec<String>, verdict: &mut Verdict) {
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
        if !h.cancelled {
            let bound = h.bound();
            let mut steps = 0;
            while h.step(false) {
                steps += 1;
                assert!(steps <= bound, "the work has not come to rest within {bound} steps");
            }
            let rest = h.w.decide();
            properties::reaches(&mut h.verdict, &h.w.flow, &h.w.records, &h.w.named, &rest, &h.moments);
        }
        properties::replay(&mut h.verdict, &h.w.flow, &h.w.records, &h.moments);
        h.count_blocks();
    }));
    *log = std::mem::take(&mut h.log);
    *verdict = std::mem::take(&mut h.verdict);
    if let Err(e) = result {
        panic::resume_unwind(e);
    }
}

#[test]
fn random_works_keep_the_engines_properties_and_come_to_rest() {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let mut failures = Vec::new();
    let mut seen = Seen::default();
    let mut broken: BTreeMap<&str, Vec<u64>> = BTreeMap::new();
    for seed in 0..SEEDS {
        let mut log = Vec::new();
        let mut verdict = Verdict::default();
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| one_history(seed, &mut log, &mut verdict)));
        seen.add(&verdict.seen);
        let mut message = match outcome {
            Err(e) => (e.downcast_ref::<String>().cloned())
                .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_string()))
                .unwrap_or_default(),
            Ok(()) => String::new(),
        };
        for (property, what) in &verdict.broken {
            broken.entry(property).or_default().push(seed);
            if !message.is_empty() {
                message.push_str("; ");
            }
            message.push_str(&format!("{property}: {what}"));
        }
        if !message.is_empty() {
            failures.push((seed, message, log));
        }
    }
    panic::set_hook(hook);
    if let Some((seed, message, log)) = failures.first() {
        let by_property: Vec<_> = (broken.iter())
            .map(|(p, s)| format!("{p}: {} histories, the first {:?}", s.len(), &s[..s.len().min(5)]))
            .collect();
        let seeds: Vec<_> = failures.iter().map(|(s, m, _)| format!("{s}: {m}")).collect();
        panic!(
            "{} of {SEEDS} histories failed.\n\nproperties broken:\n{}\n\n{}\n\nthe steps of seed {seed} ({message}):\n{}",
            failures.len(),
            by_property.join("\n"),
            seeds.join("\n"),
            log.join("\n")
        );
    }
    let unreached: Vec<_> = (properties::REACHED.iter())
        .filter(|what| seen.get(what) == 0)
        .collect();
    assert!(
        unreached.is_empty(),
        "no history reached these, so the properties about them check nothing: {unreached:?}"
    );
}
