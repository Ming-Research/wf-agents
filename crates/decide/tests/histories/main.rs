//! Random histories (principle simulated-histories): many works, each on a small flow of blocks drawn at random, driven
//! step by step as an engine would drive them, agents returning at random and people acting at random in the first
//! part of each history. After every step the properties the engine must always keep are checked (`properties`, and
//! those of a pause in `pausing` and of the room in `room`), each stating what a rule of the design protects and
//! judged by the histories' own reading of the records (`reading`), not by the walk's; once people stop, the work must
//! come to rest within a bound its limits give, and once it rests, what people wrote must have reached the cells it
//! runs.
//!
//! What the driver reaches, roughly:
//! - the flow: each of the six shapes in a sixth of the histories, cut to its first two cells in a fifth and
//!   lengthened with cells drawn at random in three fifths. A drawn cell is run by each block (main among them, which
//!   only the outside writes) with a chance of one in three and writes each with one in four; it runs only when the
//!   choice is yes in one in five, and only while the named test "flip" holds in one in seven. A cell other than the
//!   merge has the precondition "ready" in one history of four, holding at first in half of those; "flip" fails at
//!   first in one history of four; the merge's precondition holds in two histories of five. Each of these tests is the
//!   owner's word, a choice people write of the same name, and the merge is run by its own. In half the histories of
//!   the first test, drawn apart from the rest, a judge stands before the merge, run only while "flip" holds and
//!   writing a verdict, yes or no; the merge then also waits while the verdict stands at no. In a third of the
//!   histories a cell drawn at random is run by the owner's words to it, a block that interrupts. Each cell may be
//!   entered again for the results of what it asked for one to three times in a row.
//! - people, in the first 0 to 39 steps, at a step with a chance of one in six and at every rest: writing the brief,
//!   which runs the first cell; writing prose into a block cells write; writing the same marked as not mattering;
//!   setting the choice to yes or no; confirming the choice; the outside writing main, mattering or not; turning "flip"
//!   or "ready"; letting a cell go on; pausing or resuming; speaking to the cell the words run; and, once in a hundred
//!   acts, cancelling the work. At a rest they answer a call for them, or let a cell go on at a limit, three times in
//!   five.
//! - agents: each round ends with a return drawn from all the form allows, which the histories' engine judges with
//!   `decide::run::next` and what `decide::told` gives, as the engine's loop does: it does the cell's work in most,
//!   dealing with each of what ran the round with a chance of four in five and writing each output with three in five,
//!   mattering or not, a choice with a value or now and then none or one it has not; it asks for calls, for the owner,
//!   the architect or the engine, in a fifth; and now and then it mixes the two, deals with what did not run it, writes
//!   what the cell may not, or deals with nothing. A return that meets everything is taken at once, its calls made
//!   before its run finishes; one that does not is sent back, the next round told of any version it would have written
//!   over; at the limit of returns the owner is asked. From the third round of a run the agent returns what meets
//!   everything. In half the histories of the first test that have no judge, a round before the third fails in twelve in
//!   a hundred, of a kind that pauses the work, in passing, or of a kind the run takes up itself.
//! - the engine: while the work is paused, a run at rest has its next round only where it started for the owner's words
//!   and its last round did not fail in a way that paused the work with no letting on since (`pausing`).
//! - the room the engine leaves the work (`room`), as before: the owner's pause of everything and the slot come and go
//!   in some histories, and every history asks the walk what it would decide in every other room.
//!
//! Every situation a property is about is counted, and a test fails if none of its histories reaches one of them.
//!
//! A failing case prints its seed, the properties it broke and the steps it took; `one_history(seed, drawn, ..)`
//! replays it, `drawn` being what its test draws from.

// The histories use only part of the helper the rule tests share.
#[allow(dead_code)]
#[path = "../support/mod.rs"]
mod support;

mod pausing;
mod properties;
mod reading;
mod room;

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Mutex, PoisonError};

use decide::run::{self, Failure, Log, Requires, Return, Step, Unmet};
use decide::{
    BlockId, CallId, Called, Cell, CellId, Condition, Decision, End, Finished, PutIn, Returned, Room, Round, RunId,
    Seq, Test, TestKind, TestName, Waited, Who, Write,
};
use properties::{Moment, Seen, Verdict};
use reading::{Reading, Writer};
use support::{GATE, World, cell, gate};

/// What the histories of a test are drawn from: the shape of the flow, or any; the number of steps below which
/// people stop acting; and whether rounds fail in every history, and in a quarter of the rounds before the third, or
/// as `ANY` draws it.
#[derive(Clone, Copy)]
struct Drawn {
    shape: Option<usize>,
    people: usize,
    failing: bool,
}

/// Any flow, people acting in the first 0 to 39 steps.
const ANY: Drawn = Drawn {
    shape: None,
    people: 40,
    failing: false,
};

/// Any flow with no judge, people acting in the first 0 to 99 steps and rounds failing in every history, more often:
/// that a run answering a note while the owner's pause holds has a round fail without pausing the work, and goes on
/// under the pause, takes a row of acts that histories of any flow all but never reach.
const FAILING: Drawn = Drawn {
    shape: None,
    people: 100,
    failing: true,
};

/// The sixth shape only, people acting in the first 0 to 99 steps: that a judge runs while the cell it judges is
/// skipped, writing over what a person wrote for that cell, and the cell then runs again, takes a row of acts that
/// histories of any shape, with people acting for a few steps, all but never reach.
const SKIPPED_JUDGED: Drawn = Drawn {
    shape: Some(5),
    people: 100,
    failing: false,
};

/// The block only people write, a note: a person's words to the cells that read it.
const BRIEF: &str = "brief";

/// The block whose value a condition reads.
const CHOICE: &str = "choice";

/// The block only the outside writes.
const MAIN: &str = "main";

/// The blocks cells may write: prose, opinions for a cell, and the choice.
const WRITTEN: [&str; 6] = ["b0", "b1", "b2", "op0", "op1", CHOICE];

/// The judge's verdict, a choice of yes or no that only the judge and the owner write.
pub const VERDICT: &str = "verdict";

/// The merge's precondition in the histories with a judge: the verdict does not stand at no.
pub const UNREFUSED: &str = "the verdict is no refusal";

/// The block that is the owner's words to a cell: it interrupts.
const SPOKEN: &str = "spoken";

/// From this round of a run on, its agent returns what meets everything the flow requires of it.
const ROUNDS: usize = 3;

/// How often a return of any cell may be sent back before the owner is asked.
const RETURNS: u32 = 2;

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
fn draw_cells(rng: &mut Rng, shape: Option<usize>) -> Vec<Cell> {
    let shape = shape.unwrap_or_else(|| rng.below(6));
    let mut cells = match shape {
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
        c.calls = 1 + rng.below(3) as u32;
    }
    if rng.chance(25) {
        let i = rng.below(cells.len());
        cells[i].requires.push("ready".into());
    }
    if rng.chance(33) {
        let i = rng.below(cells.len());
        cells[i].runs_on.push(SPOKEN.into());
    }
    cells
}

/// What a round's return was, as the histories' engine keeps it to build the run's log: the return as read, with who
/// answers each call it asked for, or the kind of failure.
#[derive(Clone, Debug)]
enum Back {
    Came {
        dealt_with: Vec<BlockId>,
        writes: Vec<Write>,
        calls: Vec<Option<Who>>,
    },
    Failed(Failure),
}

struct History {
    w: World,
    rng: Rng,
    /// Draws what the histories with a judge add, apart from `rng`, so that the histories without one go as they did
    /// before the judge was added.
    judged: Option<Rng>,
    /// Draws which rounds fail, apart from `rng`, in the histories where rounds fail.
    failing: Option<Rng>,
    /// Draws the room the engine leaves the work, apart from `rng`, in the histories where it changes.
    rooms: Option<Rng>,
    /// The room the engine leaves the work: the owner's pause of everything, and whether a slot is free.
    room: Room,
    /// Whether rounds fail more often than `ANY` draws them.
    often: bool,
    /// What each round of each run returned, in order: `None` while under way.
    backs: BTreeMap<RunId, Vec<Option<Back>>>,
    /// The calls the engine made for the owner at a run's limit of returns.
    at_limit: BTreeSet<CallId>,
    /// The runs whose last round failed, pausing the work, and that the work was not let on since.
    unseen: BTreeSet<RunId>,
    /// The runs whose last round failed, pausing the work, and that have neither gone on nor been abandoned since.
    failed: BTreeSet<RunId>,
    /// The runs whose last round failed without pausing the work, and that have neither gone on nor been abandoned
    /// since.
    unpaused: BTreeSet<RunId>,
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
    /// How many of the runs started and finished the properties have looked at.
    looked_at: (usize, usize),
}

impl History {
    fn new(seed: u64, drawn: Drawn) -> Self {
        let shape = drawn.shape;
        let mut rng = Rng::new(seed);
        let mut cells = draw_cells(&mut rng, shape);
        let mut extra = Rng::new(seed ^ 0x0BAD_5EED_CAFE_F00D);
        let judged = (shape.is_none() && !drawn.failing && extra.chance(50)).then_some(extra);
        let mut failing = Rng::new(seed ^ 0x00FA_11ED_0F0F_0F0F);
        let fails = drawn.failing || failing.chance(50);
        let failing = (shape.is_none() && judged.is_none() && fails).then_some(failing);
        let mut rooms = Rng::new(seed ^ 0x0000_5107_5A0F_A11D);
        let mut rooms = (shape.is_none() && judged.is_none() && failing.is_none() && rooms.chance(50)).then_some(rooms);
        if let Some(rooms) = &mut rooms {
            for c in &mut cells {
                c.slot = !rooms.chance(30);
            }
        }
        let mut w = if judged.is_some() {
            let mut judge = when_flipped(cell("judge", &[BRIEF, "b0"], &[VERDICT]));
            judge.limit = 2;
            cells.push(judge);
            let mut merge = gate();
            merge.requires.push(UNREFUSED.into());
            cells.push(merge);
            let unrefused = Test {
                kind: TestKind::NoneStanding,
                reads: vec![VERDICT.into()],
                is: "no".to_string(),
            };
            World::with_tests(cells, [(TestName::from(UNREFUSED), unrefused)].into())
        } else {
            World::with_gate(cells)
        };
        w.notes(&[BRIEF]);
        w.turning(SPOKEN);
        w.person_writes(BRIEF);
        if rng.chance(40) || judged.is_some() {
            w.says(GATE, true);
        }
        if rng.chance(50) {
            w.says("ready", false);
        }
        if rng.chance(25) {
            w.says("flip", false);
        }
        let log = vec![
            format!("flow: {:?}", w.flow.cells()),
            format!("named tests: {:?}", w.flow.tests()),
            format!("written at first: {:?}", w.records.written),
        ];
        Self {
            w,
            rng,
            judged,
            failing,
            rooms,
            room: Room::FREE,
            often: drawn.failing,
            backs: BTreeMap::new(),
            at_limit: BTreeSet::new(),
            unseen: BTreeSet::new(),
            failed: BTreeSet::new(),
            unpaused: BTreeSet::new(),
            log,
            paused: false,
            last_move: None,
            cancelled: false,
            verdict: Verdict::default(),
            moments: Vec::new(),
            looked_at: (0, 0),
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
        if people && let Some(rooms) = &mut self.rooms {
            let (everything, slot) = (rooms.chance(10), rooms.chance(15));
            if everything {
                self.toggle_everything();
            }
            if slot {
                self.toggle_slot();
            }
        }
        let decision = self.decide();
        self.note(format!("decision: {decision:?}"));
        self.moments.push(Moment {
            upto: properties::last_place(&self.w.records),
            room: self.room,
            decision: decision.clone(),
        });
        self.check(&decision);
        let moved = match &decision {
            Decision::Fault(fault) => panic!("the records are said to be faulty: {fault:?}"),
            Decision::Ended(End::Merged) => {
                let merge = self.w.flow.cells().last().expect("a merge").id.clone();
                match &self.last_move {
                    Some(Decision::Run { cell, .. } | Decision::GoOn { cell, .. }) if *cell == merge => {}
                    other => panic!("ended without the merge having run just before: {other:?}"),
                }
                false
            }
            Decision::Ended(End::Cancelled) => {
                assert!(self.cancelled, "the work ended cancelled though no one cancelled it");
                self.w.person_writes(BRIEF);
                assert_eq!(
                    self.decide(),
                    Decision::Ended(End::Cancelled),
                    "a cancelled work went on"
                );
                false
            }
            Decision::Paused => {
                if people {
                    self.lift();
                }
                people
            }
            Decision::Idle => {
                if people {
                    self.person_acts(None, &[]);
                }
                people
            }
            Decision::Busy { cell, run } => {
                self.end_round(cell.clone(), *run);
                true
            }
            Decision::Run { cell, .. } => {
                let run = self.w.start(cell.as_str());
                self.backs.insert(run, vec![None]);
                true
            }
            Decision::GoOn { run, .. } => {
                let run = *run;
                if !self.goes_on_held(run) {
                    if people {
                        self.lift();
                    }
                    return people;
                }
                if self.paused_now() {
                    self.verdict.count(pausing::WENT_ON_PAUSED);
                }
                if self.failed.remove(&run) {
                    self.verdict.count(properties::FAILED_GOES_ON);
                }
                self.unseen.remove(&run);
                if self.unpaused.remove(&run) {
                    self.verdict.count(properties::UNPAUSED_GOES_ON);
                    if decide::paused(&self.w.records) {
                        self.verdict.count(pausing::UNPAUSED_ANSWERS);
                    }
                }
                self.engine(run);
                true
            }
            Decision::Abandon { run, cell, .. } => {
                let run = *run;
                let before = self.consumed_by(cell);
                self.unpaused.remove(&run);
                self.unseen.remove(&run);
                self.failed.remove(&run);
                self.w.abandon();
                if self.consumed_by(cell) != before {
                    self.verdict.fail(
                        properties::CONSUMED,
                        format!("abandoning run {run:?} of {cell} changed what the cell consumed"),
                    );
                } else {
                    self.verdict.count(properties::NOTHING_CONSUMED);
                }
                true
            }
            Decision::Wait { on, .. } if on == &[Waited::Slot] => {
                if people {
                    self.toggle_slot();
                }
                people
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

    /// What the cell consumed of each block that runs it, as the walk shows it.
    fn consumed_by(&self, cell: &CellId) -> Vec<Option<Seq>> {
        let shown = decide::view(&self.w.flow, &self.w.records);
        (shown.cells.into_iter())
            .filter(|c| &c.cell == cell)
            .flat_map(|c| c.runs_on.into_iter().map(|i| i.consumed))
            .collect()
    }

    /// What must hold after every step: no two rounds under way, and the properties, of the decision just taken and
    /// of each run that started or finished since the last step.
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
                matches!(decision, Decision::Busy { .. } | Decision::Abandon { .. }),
                "a round is under way and the walk decides {decision:?}"
            );
        }
        let reading = Reading::new(r);
        let flow = &self.w.flow;
        let v = &mut self.verdict;
        let (started, finished) = self.looked_at;
        for s in &r.started[started..] {
            properties::started(v, flow, &reading, s);
        }
        for f in &r.finished[finished..] {
            properties::finished(v, &reading, f.run);
        }
        self.looked_at = (r.started.len(), r.finished.len());
        properties::consumption(v, flow, &reading);
        let seed = properties::last_place(r).0;
        let paused = decide::paused(r) || self.room.paused;
        properties::decision(v, flow, &reading, decision, self.room, paused, seed);
        pausing::decision(v, flow, &reading, decision, self.room, paused);
        room::decision(v, flow, r);
    }

    /// What the walk decides, in the room the engine leaves the work.
    fn decide(&self) -> Decision {
        decide::decide_in(&self.w.flow, &self.w.records, self.room)
    }

    /// Whether the work is paused, by its own pause or by the owner's pause of everything.
    fn paused_now(&self) -> bool {
        decide::paused(&self.w.records) || self.room.paused
    }

    /// Whether the run at rest has its next round in the room the engine leaves the work, as the engine gives it by
    /// `decide::hold_in`, which `pausing` and `room` judge.
    fn goes_on_held(&mut self, run: RunId) -> bool {
        let reading = Reading::new(&self.w.records);
        let failed = self.unseen.contains(&run);
        pausing::goes_on(&mut self.verdict, &self.w.flow, &reading, run, failed, self.room)
    }

    /// The run's log, as the engine's loop builds it from what each round returned and the records.
    fn log_of(&self, run: RunId) -> Log {
        let r = &self.w.records;
        let ended: Vec<Seq> = r.returned.iter().filter(|e| e.run == run).map(|e| e.at).collect();
        let began: Vec<Seq> = r.rounds.iter().filter(|e| e.run == run).map(|e| e.at).collect();
        let done = |call: CallId| r.done.iter().find(|d| d.call == call).map(|d| d.at);
        let rounds = (self.backs.get(&run).into_iter().flatten().enumerate())
            .map(|(i, back)| run::Round {
                returned: back.as_ref().map(|b| match b {
                    Back::Came {
                        dealt_with,
                        writes,
                        calls,
                    } => Return::Came {
                        dealt_with: dealt_with.clone(),
                        writes: writes.clone(),
                        calls: calls.len(),
                    },
                    Back::Failed(failure) => Return::Failed {
                        failure: *failure,
                        why: "it failed".to_string(),
                        waits: (r.called.iter())
                            .filter(|c| {
                                c.run == run
                                    && ended.get(i).is_some_and(|e| c.at > *e)
                                    && began.get(i + 1).is_none_or(|b| c.at < *b)
                            })
                            .map(|c| run::Call {
                                call: c.call,
                                done: done(c.call).is_some(),
                            })
                            .collect(),
                    },
                }),
            })
            .collect();
        let rounds_ended_before = |at: Seq| ended.iter().filter(|e| **e < at).count();
        let let_go = (r.called.iter())
            .filter(|c| c.run == run && self.at_limit.contains(&c.call))
            .filter_map(|c| done(c.call))
            .max()
            .map_or(0, rounds_ended_before);
        let let_on = (r.pauses.iter())
            .filter(|p| !p.paused)
            .map(|p| p.at)
            .max()
            .map_or(0, rounds_ended_before);
        Log { rounds, let_go, let_on }
    }

    /// Does what the run does next, as the engine's loop does, until a round of it begins or it rests: takes a return
    /// that meets everything, sends back one that does not, tries a failed round again, makes a wait or asks the owner
    /// at the limit of returns.
    fn engine(&mut self, run: RunId) {
        let flow = self.w.flow.clone();
        let started = self
            .w
            .records
            .started
            .iter()
            .find(|s| s.run == run)
            .expect("the run started")
            .clone();
        let cell = flow.cells()[flow.place(&started.cell).expect("a cell of the flow")].clone();
        let choices: BTreeMap<BlockId, Vec<String>> = [CHOICE, VERDICT, GATE, "ready", "flip"]
            .into_iter()
            .map(|b| (b.into(), vec!["yes".to_string(), "no".to_string()]))
            .collect();
        let told = decide::told(&flow, &self.w.records, run).expect("the run started");
        let requires = Requires {
            outputs: &cell.outputs,
            must_write: &[],
            choices: &choices,
            returns: RETURNS,
            told: &told,
        };
        let hold = decide::hold_in(&flow, &self.w.records, run, self.room);
        let step = run::next(&self.log_of(run), requires, hold);
        self.note(format!("run {run:?} of {}: {step:?}", cell.id));
        match step {
            Step::Finish { dealt_with, writes } => {
                let calls = match self.backs[&run].last() {
                    Some(Some(Back::Came { calls, .. })) => calls.clone(),
                    _ => Vec::new(),
                };
                for on in calls {
                    let call = CallId(self.next_id());
                    let at = self.w.tick();
                    self.w.records.called.push(Called { call, run, on, at });
                }
                let at = self.w.tick();
                self.w.records.finished.push(Finished {
                    run,
                    at,
                    writes,
                    dealt_with: Some(dealt_with),
                });
            }
            Step::SendBack { unmet } => {
                self.begin_round(run);
                for u in unmet {
                    if let Unmet::WrittenOver { block, version } = u {
                        self.verdict.count(properties::WRITTEN_OVER_SENT_BACK);
                        let at = self.w.tick();
                        self.w.records.put_in.push(PutIn {
                            run,
                            block,
                            version,
                            at,
                        });
                    }
                }
            }
            Step::Begin | Step::Retry => self.begin_round(run),
            Step::Wait { .. } => {
                let call = CallId(self.next_id());
                let at = self.w.tick();
                self.w.records.called.push(Called {
                    call,
                    run,
                    on: None,
                    at,
                });
            }
            Step::AtLimit(_) => {
                let call = CallId(self.next_id());
                let at = self.w.tick();
                self.at_limit.insert(call);
                self.w.records.called.push(Called {
                    call,
                    run,
                    on: Some(Who::Owner),
                    at,
                });
            }
            Step::Rest { .. } | Step::UnderWay | Step::Held => {}
        }
    }

    fn begin_round(&mut self, run: RunId) {
        let at = self.w.tick();
        self.w.records.rounds.push(Round { run, at });
        self.backs.entry(run).or_default().push(None);
    }

    /// A name no run and no call has yet.
    fn next_id(&self) -> u64 {
        let r = &self.w.records;
        let names = (r.started.iter().map(|s| s.run.0)).chain(r.called.iter().map(|c| c.call.0));
        names.max().unwrap_or(0) + 1000
    }

    /// People lift what holds the work back: the work's own pause, or, where the owner's pause of everything holds
    /// or no slot is free, in half the acts drawn apart from the rest, that instead.
    fn lift(&mut self) {
        let other = (self.room.paused || !self.room.slot) && self.rooms.as_mut().is_some_and(|r| r.chance(50));
        if !other {
            self.toggle_pause();
        } else if !self.room.slot {
            self.toggle_slot();
        } else {
            self.toggle_everything();
        }
    }

    fn toggle_everything(&mut self) {
        self.room.paused = !self.room.paused;
        let what = if self.room.paused {
            "pauses"
        } else {
            "lifts the pause of"
        };
        self.note(format!("the owner {what} everything"));
    }

    fn toggle_slot(&mut self) {
        self.room.slot = !self.room.slot;
        let what = if self.room.slot {
            "comes free"
        } else {
            "is taken by another work"
        };
        self.note(format!("the slot {what}"));
    }

    /// The round under way of the run ends: it fails, in a history where rounds fail, or its agent returns, and the
    /// engine takes up what came back.
    fn end_round(&mut self, cell: CellId, run: RunId) {
        let spec = (self.w.flow.cells().iter())
            .find(|c| c.id == cell)
            .expect("the cell is in the flow")
            .clone();
        let is_merge = self.w.flow.cells().last().is_some_and(|c| c.id == cell);
        let rounds = self.backs.get(&run).map_or(0, Vec::len);
        let at = self.w.tick();
        self.w.records.returned.push(Returned { run, at });
        if !is_merge
            && rounds < ROUNDS
            && let Some(failing) = &mut self.failing
            && failing.chance(if self.often { 25 } else { 12 })
        {
            let kind = failing.below(4);
            let failure = match kind {
                0 | 1 => Failure::Other,
                2 => Failure::Passing,
                _ => Failure::Flagged,
            };
            self.set_back(run, Back::Failed(failure));
            match kind {
                0 | 1 => {
                    self.note(format!("{cell}'s round fails: the engine pauses the work"));
                    self.w.engine_pauses("the agent could not start");
                    self.paused = true;
                    self.failed.insert(run);
                    self.unseen.insert(run);
                    self.verdict.count(properties::FAILED);
                }
                2 => {
                    self.note(format!("{cell}'s round fails in passing: the run waits for the engine"));
                    self.verdict.count(properties::FAILED_IN_PASSING);
                    self.engine(run);
                }
                _ => {
                    self.note(format!("{cell}'s round fails, and the run takes it up itself"));
                    self.unpaused.insert(run);
                    self.verdict.count(properties::FAILED_UNPAUSED);
                }
            }
            return;
        }
        let told = decide::told(&self.w.flow, &self.w.records, run).expect("the run started");
        let back = self.draw_return(&spec, &told, is_merge || rounds >= ROUNDS);
        self.note(format!("{cell} returns {back:?}"));
        self.set_back(run, back);
        self.engine(run);
    }

    fn set_back(&mut self, run: RunId, back: Back) {
        let backs = self.backs.entry(run).or_default();
        if let Some(last) = backs.last_mut() {
            *last = Some(back);
        }
    }

    /// A return drawn from all the form allows; where `meets`, one that does the cell's work, dealing with everything
    /// that ran the round, and writes only what the cell may, each as it may be written.
    fn draw_return(&mut self, spec: &Cell, told: &run::Told, meets: bool) -> Back {
        let choice = |b: &BlockId| [CHOICE, VERDICT].contains(&b.as_str());
        let write = |rng: &mut Rng, block: &BlockId, meets: bool| {
            let value = if choice(block) {
                match rng.below(10) {
                    0 if !meets => None,
                    1 if !meets => Some("maybe".to_string()),
                    n => Some(if n % 2 == 0 { "yes" } else { "no" }.to_string()),
                }
            } else if !meets && rng.chance(5) {
                Some("yes".to_string())
            } else {
                None
            };
            Write {
                block: block.clone(),
                meaning_unchanged: !(meets && choice(block)) && rng.chance(50),
                value,
            }
        };
        let rng = &mut self.rng;
        if meets {
            let mut writes = Vec::new();
            for b in &spec.outputs {
                if rng.chance(60) {
                    writes.push(write(rng, b, true));
                }
            }
            return Back::Came {
                dealt_with: told.ran_by.clone(),
                writes,
                calls: Vec::new(),
            };
        }
        let callers = |rng: &mut Rng| -> Vec<Option<Who>> {
            (0..1 + rng.below(2))
                .map(|_| match rng.below(10) {
                    0 | 1 => Some(Who::Owner),
                    2 => Some(Who::Architect),
                    _ => None,
                })
                .collect()
        };
        match rng.below(20) {
            // Calls, and nothing else.
            0..=3 => Back::Came {
                dealt_with: Vec::new(),
                writes: Vec::new(),
                calls: callers(rng),
            },
            // Deals with nothing, writes nothing, asks nothing.
            4 => Back::Came {
                dealt_with: Vec::new(),
                writes: Vec::new(),
                calls: Vec::new(),
            },
            // Calls beside the cell's work.
            5 => Back::Came {
                dealt_with: told.ran_by.iter().take(1).cloned().collect(),
                writes: spec.outputs.iter().take(1).map(|b| write(rng, b, true)).collect(),
                calls: callers(rng),
            },
            // The cell's work, with what it did not run on or may not write now and then.
            _ => {
                let mut dealt_with: Vec<BlockId> = told.ran_by.iter().filter(|_| rng.chance(80)).cloned().collect();
                if rng.chance(5) {
                    dealt_with.push(MAIN.into());
                }
                let mut writes = Vec::new();
                for b in &spec.outputs {
                    if rng.chance(60) {
                        writes.push(write(rng, b, false));
                    }
                }
                if rng.chance(5) {
                    writes.push(write(rng, &"nowhere".into(), false));
                }
                Back::Came {
                    dealt_with,
                    writes,
                    calls: Vec::new(),
                }
            }
        }
    }

    fn toggle_pause(&mut self) {
        if self.paused {
            self.note("a person lets the work on".into());
            self.w.resume();
            self.unseen.clear();
        } else {
            self.note("a person pauses the work".into());
            self.w.pause();
        }
        self.paused = !self.paused;
    }

    /// The current value of the block: that of its latest write.
    fn value(&self, block: &str) -> Option<String> {
        let r = &self.w.records;
        let by_cells =
            (r.finished.iter()).flat_map(|f| f.writes.iter().map(move |x| (f.at, x.block.clone(), x.value.clone())));
        let by_people = r.written.iter().map(|x| (x.at, x.block.clone(), x.value.clone()));
        let latest: Option<(Seq, BlockId, Option<String>)> = (by_cells.chain(by_people))
            .filter(|(_, b, _)| b.as_str() == block)
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

    /// The block a person writes prose into: in half the acts one that bears on what is going on, a block that runs
    /// the cell the work waits at, a block the open run's cell may write or one that runs a cell that is skipped, and
    /// otherwise any block cells write.
    fn prose_block(&mut self, at: Option<&CellId>) -> &'static str {
        let open = self.open_run().map(|(cell, _)| cell);
        let reading = Reading::new(&self.w.records);
        let mut bearing: Vec<&BlockId> = Vec::new();
        for (i, c) in self.w.flow.cells().iter().enumerate() {
            if Some(&c.id) == at || !reading.applies(&self.w.flow, i) {
                bearing.extend(&c.runs_on);
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
                Waited::Limit { .. } | Waited::Calls { .. } if self.rng.chance(60) => {
                    let at = at.expect("a limit is waited for at a cell").clone();
                    self.note(format!("the owner lets {at} go on"));
                    self.w.let_go(at.as_str());
                    return;
                }
                Waited::Precondition { test, .. }
                    if test.as_str() == UNREFUSED && self.judged.as_mut().is_some_and(|j| j.chance(50)) =>
                {
                    self.note(format!("the owner writes {VERDICT} as yes"));
                    self.person_writes_now();
                    self.w.person_sets(VERDICT, "yes");
                    return;
                }
                _ => {}
            }
        }
        if let Some(judged) = &mut self.judged
            && judged.chance(8)
        {
            let value = if judged.chance(50) { "yes" } else { "no" };
            self.note(format!("the owner writes {VERDICT} as {value}"));
            self.person_writes_now();
            self.w.person_sets(VERDICT, value);
            return;
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
            11..=18 => {
                self.note(format!("a person writes prose into {prose}"));
                self.person_writes_now();
                self.w.person_writes(prose);
            }
            19..=26 => {
                self.note(format!("a person writes prose into {prose}, marked as not mattering"));
                self.person_writes_now();
                self.w.person_writes_not_mattering(prose);
            }
            27..=34 => {
                let value = if self.rng.chance(50) { "yes" } else { "no" };
                self.note(format!("a person sets {CHOICE} to {value}"));
                self.person_writes_now();
                self.w.person_sets(CHOICE, value);
            }
            35..=42 => {
                if let Some(value) = self.value(CHOICE) {
                    self.note(format!("a person confirms {CHOICE} as {value}"));
                    self.person_writes_now();
                    self.w.person_confirms(CHOICE, &value);
                }
            }
            43..=52 => {
                if self.rng.chance(50) {
                    self.note(format!("the outside writes {MAIN}"));
                    self.w.outside_writes(MAIN);
                } else {
                    self.note(format!("the outside writes {MAIN}, approved as not mattering"));
                    self.w.outside_writes_not_mattering(MAIN);
                }
            }
            53..=62 => {
                self.note(format!("the owner speaks: the outside writes {SPOKEN}"));
                self.person_writes_now();
                self.w.outside_writes(SPOKEN);
            }
            63..=70 => {
                let test = if self.rng.chance(50) { "flip" } else { "ready" };
                let holds = self.value(test).as_deref() == Some("no");
                self.note(format!(
                    "the owner's word {test} comes to {}",
                    if holds { "yes" } else { "no" }
                ));
                self.w.says(test, holds);
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
    /// which the later in the order is not run by it.
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
            if (writers.iter()).any(|&i| writers.iter().any(|&j| j > i && !cells[j].runs_on.contains(block))) {
                self.verdict.count(properties::TWO_CELLS_LATER_UNREAD);
            }
        }
    }

    /// How many steps, once people stop, the work may take before it must wait, rest or end (property
    /// a-work-comes-to-rest). Once people stop, a cell runs again only for what cells write, the results of its calls
    /// or what was new when they stopped; its ways round are bounded by its limit, its entries for results by its
    /// limit of calls, and a run's rounds by the limit of returns, at which it waits for the owner. A generous bound
    /// of the same shape as before: each cell at place `i`, of limits `L_i` and `C_i` and `n_i` blocks that run it,
    /// starts at most `S_i = (L_i + C_i + 3) * (3 + n_i + S_0 + ... + S_(i-1))` runs, and a run takes at most
    /// `6 * ROUNDS` steps.
    fn bound(&self) -> u64 {
        let mut before: u64 = 0;
        for c in self.w.flow.cells() {
            let s = (u64::from(c.limit) + u64::from(c.calls) + 3).saturating_mul(3 + c.runs_on.len() as u64 + before);
            before = before.saturating_add(s);
        }
        (before + 1).saturating_mul(6 * ROUNDS as u64)
    }
}

/// One history: people act in its first part, then the work is driven until it rests, and what must hold of the whole
/// history is checked.
fn one_history(seed: u64, drawn: Drawn, log: &mut Vec<String>, verdict: &mut Verdict) {
    let mut h = History::new(seed, drawn);
    let people_until = h.rng.below(drawn.people);
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        for _ in 0..people_until {
            h.step(true);
            if matches!(h.decide(), Decision::Ended(_)) {
                break;
            }
        }
        h.note("people stop".into());
        if h.room != Room::FREE {
            h.note("the pause of everything is lifted, and a slot is free".into());
            h.room = Room::FREE;
        }
        // In a history whose rounds fail, the owner lets the work on, whoever paused it.
        if h.failing.is_some() && h.paused && !h.cancelled {
            h.toggle_pause();
        }
        if !h.cancelled {
            let bound = h.bound();
            let mut steps = 0;
            while h.step(false) {
                steps += 1;
                assert!(steps <= bound, "the work has not come to rest within {bound} steps");
            }
            let rest = h.decide();
            properties::reaches(&mut h.verdict, &h.w.flow, &h.w.records, &rest, &h.moments);
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

/// Held while a test runs its histories: each silences the panic hook while its histories run and gives it back
/// before it fails, and two tests side by side would each take the other's.
static HOOK: Mutex<()> = Mutex::new(());

/// Runs this many histories drawn so, and fails if one breaks a property, or if none reached one of these situations.
fn histories(drawn: Drawn, seeds: u64, reached: &[&str]) {
    let _hook = HOOK.lock().unwrap_or_else(PoisonError::into_inner);
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let mut failures = Vec::new();
    let mut seen = Seen::default();
    let mut broken: BTreeMap<&str, Vec<u64>> = BTreeMap::new();
    for seed in 0..seeds {
        let mut log = Vec::new();
        let mut verdict = Verdict::default();
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| one_history(seed, drawn, &mut log, &mut verdict)));
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
        let seeds_failed: Vec<_> = failures.iter().map(|(s, m, _)| format!("{s}: {m}")).collect();
        panic!(
            "{} of {seeds} histories failed.\n\nproperties broken:\n{}\n\n{}\n\nthe steps of seed {seed} ({message}):\n{}",
            failures.len(),
            by_property.join("\n"),
            seeds_failed.join("\n"),
            log.join("\n")
        );
    }
    // How often the rounds that failed were reached, said whatever comes of the test: written to the standard error
    // itself, which the test harness does not hold back as it holds back what a test prints.
    let failing = [
        properties::FAILED,
        properties::FAILED_GOES_ON,
        properties::FAILED_IN_PASSING,
        properties::FAILED_UNPAUSED,
        properties::UNPAUSED_GOES_ON,
        pausing::UNPAUSED_ANSWERS,
    ];
    let counts: Vec<String> = failing
        .iter()
        .map(|what| format!("{what}: {}", seen.get(what)))
        .collect();
    let _ = writeln!(
        std::io::stderr(),
        "the deciding code's histories reached: {}",
        counts.join("; ")
    );
    // And how often each of what they must reach was, so that a rare one shows how close it is to none.
    let must: Vec<String> = reached
        .iter()
        .map(|what| format!("{what}: {}", seen.get(what)))
        .collect();
    let _ = writeln!(std::io::stderr(), "of what they must reach: {}", must.join("; "));
    let unreached: Vec<_> = reached.iter().filter(|what| seen.get(what) == 0).collect();
    assert!(
        unreached.is_empty(),
        "no history reached these, so the properties about them check nothing: {unreached:?}"
    );
}

#[test]
fn random_works_keep_the_engines_properties_and_come_to_rest() {
    histories(
        ANY,
        800,
        &[properties::REACHED, pausing::REACHED, room::REACHED].concat(),
    );
}

#[test]
fn rounds_failing_often_while_people_pause_and_write_keep_the_engines_properties() {
    histories(FAILING, 600, &[pausing::UNPAUSED_ANSWERS]);
}

#[test]
fn a_judge_running_while_the_cell_it_judges_is_skipped_keeps_the_engines_properties() {
    histories(SKIPPED_JUDGED, 600, &[]);
}
