//! Simulated histories of whole works on the engineer's flow as the repository defines it (`flow/engineer`), through
//! `handle` only, by the driver the histories share (`support`). The engine's own properties are checked by the
//! histories of the small flows; these check what the engineer's flow must keep. Every name comes from the compiled
//! definition: the merge is its last cell, the check the cell the act `check` carries out, a verdict a block that a
//! `none_standing` test reads, its refusal the value that test looks for, a judge a cell that writes a verdict.
//!
//! 1. No round of the merge begins while a judge that is not skipped has, as its own last write of its verdict, the
//!    refusal, with no person's write of the verdict since; nor while a person's write of a verdict is its latest and
//!    is the refusal (entry no-merge-on-what-is-not-settled).
//! 2. No round of the merge begins unless the check's record wrote each choice it writes that a precondition of kind
//!    `all` reads with the value that precondition looks for, and started from the current meaning of every block the
//!    check reads (entries no-merge-on-what-is-not-settled and merge-only-on-what-is-current).
//! 3. From the state a random history reached, a work everyone is content with merges within `content_bound` steps
//!    (entry a-work-comes-to-rest).
//! 4. In that continuation, the runs of each cell beyond its first that changed no block's meaning, runs for nothing,
//!    are counted, printed with `--nocapture`, and held to the bound `content_bound` rests on.
//!
//! The first two are judged as each round of the merge begins, by the histories' own reading of the records as they
//! were then (`reading`), never by the deciding code's tests.
//!
//! What a history is. With a chance of four in five it begins with the outside making the first cell's precondition
//! hold (the items the work depends on merged); otherwise with a look. Then, for 0 to 199 steps, the driver's random
//! steps, people acting at a step with a chance of one in four and the engine stopping at any step:
//! - an agent's round ends with one to three calls in a tenth of the rounds (a tool in seven of ten, a question for the
//!   owner or the architect otherwise), asking for nothing in one in twenty-five, handing in a block its cell may not
//!   write beside the rest in one in twenty-five, and otherwise handing in what its cell may write: each choice in nine
//!   of ten, its first value (yes) in a little over half of those; each other block in four of five, mattering in seven
//!   of ten; but a block a cell before it reads, an opinion for that cell, in about a third, mattering. In one history
//!   of five the agents are sloppy: half their returns ask for nothing and are sent back, so that runs reach their
//!   limit of returns;
//! - an act of the engine's own hands in every block its cell may write: each choice with the value a precondition of
//!   kind `all` looks for in three of five (the check passing), another otherwise; an opinion for a cell before it only
//!   when a choice did not have that value (the check failing says why);
//! - a person who acts while the check's round is under way pauses the work, and while the pause then holds
//!   the merge back after the check passed, writes an input of a judge that people write in half the acts and lets
//!   the work on in three of ten, so that a judge looks again after the check passed. Otherwise a person writes a
//!   choice people write (a verdict), yes or no, in about a fifth of the acts; prose people write (the background, an
//!   opinion for a cell) in three of twenty; the outside writes one of its blocks, a choice with the value a
//!   precondition looks for in four of five, prose mattering in half, in three of twenty; the owner lets the cell at
//!   its limit go on, or any cell where none is, in three of twenty; pauses or resumes in a tenth; cancels in one in a
//!   hundred; looks otherwise.
//!
//! In half the histories the random steps end early: in a quarter at the first step after which the check's record
//! failed, in a quarter at the first after which it failed or a judge's own refusal stands. The content continuation
//! is then tried from where the work must go back.
//!
//! Unless the work was cancelled or merged, the content continuation follows (`Content`): every round of an agent or
//! act hands in what its cell must write to go forward and says nothing more. Each verdict is written as a value other
//! than its refusal; each choice a precondition of kind `all` reads, as the value it looks for (the check says yes);
//! every other choice as it last was, or no where it was never written. No opinion for a cell before it is written,
//! and every other block it may write is written, mattering only where it was never written before. Tools answer, the
//! owner answers every question and lets every run go on at its limits; where nothing is to be done and the work waits,
//! the owner resumes a paused work, lets the cell it waits at go on at its limit, and makes a precondition it waits on
//! hold where a person or the outside can: writes a verdict whose refusal stands as a value that is not, and the
//! outside writes what the first cell's precondition looks for. Where the precondition reads a choice that only cells
//! write (the check's), the owner writes a few words in an input of the cell that writes it which only people write
//! (an opinion for the check), so that the cell runs again; in the continuation the check then says yes.
//!
//! Every situation the properties are about is counted over all histories (`required`), and the test fails if one was
//! never reached. A failing case prints its seed and its steps; `one_history(&flow, seed, ..)` replays it.

// The histories use only part of the driver the tests of histories share.
#[allow(dead_code)]
#[path = "../support/mod.rs"]
mod support;

mod reading;

use std::collections::{BTreeMap, BTreeSet};
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use decide::{BlockId, CellId, Decision, End, Outsider, Records, RunId, Seq, TestKind, TestName, Waited, Who, decide};
use engine::{Ask, Came, Effect, Event, Handed};
use flow::{Compiled, Has, Kind, compile, read};
use reading::{Reading, Writer};
use support::{Driver, Part, Rng};

/// How many histories are run.
const SEEDS: u64 = 400;

/// How many random steps, at most, a history takes before the content continuation.
const RANDOM: usize = 200;

/// The most steps the content continuation takes for one run, besides what the driver held as it began: the owner
/// letting the run's cell go on at its limit, the owner letting the run go on at a limit of its own, the up to three
/// calls its last round asked for, and its round, which hands in what finishes it.
const PER_RUN: u64 = 6;

/// The value a choice is written with in the content continuation where it was never written.
const NO: &str = "no";

const MERGE_OVER_A_NO: &str = "1: no merge over a judge's standing no";
const MERGE_ON_AN_OLD_CHECK: &str = "2: no merge on a check that is not of the current code";
const CONTENT_MERGES: &str = "3: a work everyone is content with merges within the bound";
const RUNS_FOR_NOTHING: &str = "4: runs for nothing stay within the bound";
const DRIVER: &str = "the driver";

/// The engineer's flow as the repository defines it, compiled against what the engine has.
fn engineer() -> Compiled {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/engineer");
    let source = read(&dir).expect("the engineer's flow is read");
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
    let has = Has {
        sources: names(&["dependencies", "main", "session", "tree"]),
        acts: names(&["start", "check", "merge"]),
    };
    compile(&source, &has).expect("the engineer's flow compiles")
}

/// The cells and blocks the properties are about, found in the compiled definition.
#[derive(Clone)]
struct Roles {
    /// The place of the merge: the last cell.
    last: usize,
    /// The place of the check: the cell the act `check` carries out.
    check: usize,
    /// Each verdict, a block a `none_standing` test reads, with its refusal, the value the test looks for.
    refusals: BTreeMap<BlockId, String>,
    /// Each judge, a cell that writes a verdict, by its place, with the verdict.
    judges: Vec<(usize, BlockId)>,
    /// Each choice a precondition of kind `all` reads, with the value it looks for.
    passing: BTreeMap<BlockId, String>,
}

impl Roles {
    fn new(flow: &Compiled) -> Self {
        let cells = flow.flow.cells();
        let check = (flow.cells.iter())
            .position(|c| c.act.as_deref() == Some("check"))
            .expect("the flow has a check");
        let tests = flow.flow.tests();
        let mut refusals = BTreeMap::new();
        for t in tests.values().filter(|t| t.kind == TestKind::NoneStanding) {
            for b in &t.reads {
                refusals.insert(b.clone(), t.is.clone());
            }
        }
        let judges = (refusals.keys())
            .flat_map(|b| {
                (flow.blocks[b].writers.cells.iter()).map(move |c| (flow.flow.place(c).expect("a cell"), b.clone()))
            })
            .collect();
        let mut passing = BTreeMap::new();
        for name in cells.iter().flat_map(|c| &c.requires) {
            let t = &tests[name];
            if t.kind == TestKind::All {
                for b in &t.reads {
                    passing.insert(b.clone(), t.is.clone());
                }
            }
        }
        Self {
            last: cells.len() - 1,
            check,
            refusals,
            judges,
            passing,
        }
    }

    /// The choices the check writes that a precondition of kind `all` reads, with the value it looks for.
    fn check_passes<'a>(&'a self, flow: &'a Compiled) -> impl Iterator<Item = (&'a BlockId, &'a String)> {
        let outputs = &flow.flow.cells()[self.check].outputs;
        self.passing.iter().filter(move |(b, _)| outputs.contains(b))
    }
}

/// Every situation some history must reach, so that the properties about it check something.
fn required(flow: &Compiled, roles: &Roles) -> Vec<String> {
    let cells = flow.flow.cells();
    let mut reached: Vec<String> = cells.iter().map(|c| ran(&c.id)).collect();
    reached.extend(cells.iter().filter(|c| c.applies.is_some()).map(|c| skipped(&c.id)));
    reached.extend((roles.judges.iter()).map(|(j, b)| refused(&cells[*j].id, b, &roles.refusals[b])));
    reached.extend(roles.refusals.keys().map(person_wrote));
    reached.extend(roles.check_passes(flow).map(|(b, pass)| check_failed(b, pass)));
    reached.extend(
        [
            CELL_LIMIT,
            RUN_LIMIT,
            NO_BY_A_JUDGE,
            NO_BY_A_PERSON,
            MERGE_BEGAN,
            CONTENT_MERGED,
            CONTENT_ON_A_FAILED_CHECK,
            CONTENT_ON_A_REFUSAL,
            OWNER_ASKED_A_CELL,
            ENGINE_STOPPED,
        ]
        .map(String::from),
    );
    reached
}

fn ran(cell: &CellId) -> String {
    format!("{cell} ran")
}

fn skipped(cell: &CellId) -> String {
    format!("{cell} ran and was then skipped")
}

fn refused(cell: &CellId, block: &BlockId, refusal: &str) -> String {
    format!("{cell} wrote {block} = {refusal}")
}

fn person_wrote(block: &BlockId) -> String {
    format!("a person wrote {block}")
}

fn check_failed(block: &BlockId, pass: &str) -> String {
    format!("the check wrote {block} other than {pass}")
}

const CELL_LIMIT: &str = "the work waited at a cell's limit";
const RUN_LIMIT: &str = "a run reached a limit of its own";
const NO_BY_A_JUDGE: &str = "a judge's refusal stood while the check's record passed, before any run of the merge";
const NO_BY_A_PERSON: &str = "a person's refusal stood while the check's record passed, before any run of the merge";
const MERGE_BEGAN: &str = "a round of the merge began";
const CONTENT_MERGED: &str = "the content continuation merged";
const CONTENT_ON_A_FAILED_CHECK: &str = "the content continuation began as the check's record failed";
const CONTENT_ON_A_REFUSAL: &str = "the content continuation began as a judge's own refusal stood";
const OWNER_ASKED_A_CELL: &str = "the owner had a cell write again a choice a precondition waits on";
const ENGINE_STOPPED: &str = "the engine stopped";

/// The place of the cell.
fn place(flow: &Compiled, cell: &str) -> usize {
    flow.flow.place(&cell.into()).expect("the cell is in the flow")
}

/// Whether the block is an opinion for a cell before the one at this place: a cell before it reads it.
fn opinion(flow: &Compiled, block: &BlockId, i: usize) -> bool {
    (flow.blocks[block].readers.cells.iter()).any(|c| flow.flow.place(c).is_some_and(|j| j < i))
}

/// Whether the block was ever written.
fn ever_written(records: &Records, block: &BlockId) -> bool {
    (records.finished.iter()).any(|f| f.writes.iter().any(|w| &w.block == block))
        || records.written.iter().any(|w| &w.block == block)
}

fn wrote(block: &BlockId, by: Outsider, matters: bool, value: Option<String>) -> Event {
    Event::Wrote {
        block: block.clone(),
        by,
        content: format!("{block} by {by:?}"),
        matters,
        value,
    }
}

fn handed(block: &BlockId, cell: &str, matters: bool, value: Option<String>) -> Handed {
    Handed {
        block: block.clone(),
        content: format!("{block} by {cell}"),
        matters,
        value,
    }
}

/// A write by whoever may write the block outside a cell, a person first, that makes the named test hold, or moves it
/// towards holding; none where nobody outside a cell can.
fn make_hold(d: &Driver, name: &TestName) -> Option<Event> {
    let test = &d.flow.flow.tests()[name];
    let reading = Reading::new(&d.flow.flow, &d.work.records, None);
    for b in &test.reads {
        let block = &d.flow.blocks[b];
        let by = if block.writers.people {
            Outsider::Person
        } else if block.writers.outside.is_some() {
            Outsider::Outside
        } else {
            continue;
        };
        let values = block.values.as_ref()?;
        let value = match test.kind {
            TestKind::All | TestKind::Any => (reading.value(b) != Some(test.is.as_str())).then(|| test.is.clone()),
            TestKind::None | TestKind::NoneStanding => reading
                .current(b)
                .filter(|v| v.value.as_deref() == Some(test.is.as_str()))
                .filter(|v| test.kind != TestKind::NoneStanding || reading.stands(v))
                .and_then(|_| values.iter().find(|v| **v != test.is).cloned()),
        };
        if let Some(value) = value {
            return Some(wrote(b, by, true, Some(value)));
        }
    }
    None
}

/// The random steps of a history.
struct Random {
    roles: Roles,
    /// Whether the agents' returns are sent back half the time, so that a run reaches its limit of returns.
    sloppy: bool,
}

/// Whether the check's record passed, and no run of the merge has started.
fn passed_before_merge(d: &Driver, roles: &Roles) -> bool {
    let flow = &d.flow.flow;
    let records = &d.work.records;
    let last = &flow.cells()[roles.last].id;
    let reading = Reading::new(flow, records, None);
    let passed = reading.record(&flow.cells()[roles.check].id).is_some_and(|f| {
        (roles.check_passes(&d.flow))
            .all(|(b, pass)| f.writes.iter().any(|w| &w.block == b && w.value.as_ref() == Some(pass)))
    });
    passed && !records.started.iter().any(|s| &s.cell == last)
}

impl Random {
    /// What an agent hands in that its cell may write.
    fn valid(&self, d: &mut Driver, i: usize, cell: &str) -> Vec<Handed> {
        let outputs = d.flow.flow.cells()[i].outputs.clone();
        let mut handed_in = Vec::new();
        for b in &outputs {
            if let Some(values) = d.flow.blocks[b].values.clone() {
                if d.rng.chance(90) {
                    let value = if values.len() == 1 || d.rng.chance(55) {
                        values[0].clone()
                    } else {
                        values[1 + d.rng.below(values.len() - 1)].clone()
                    };
                    handed_in.push(handed(b, cell, true, Some(value)));
                }
            } else if opinion(&d.flow, b, i) {
                if d.rng.chance(35) {
                    handed_in.push(handed(b, cell, true, None));
                }
            } else if d.rng.chance(80) {
                let matters = d.rng.chance(70);
                handed_in.push(handed(b, cell, matters, None));
            }
        }
        handed_in
    }

    /// What an act of the engine's own hands in.
    fn act(&self, d: &mut Driver, i: usize, cell: &str) -> Came {
        let outputs = d.flow.flow.cells()[i].outputs.clone();
        let mut failed = false;
        let mut handed_in = Vec::new();
        for b in &outputs {
            let Some(values) = d.flow.blocks[b].values.clone() else {
                continue;
            };
            let value = match self.roles.passing.get(b) {
                Some(pass) if d.rng.chance(60) => pass.clone(),
                Some(pass) => {
                    failed = true;
                    values.iter().find(|v| *v != pass).unwrap_or(pass).clone()
                }
                None => values[d.rng.below(values.len())].clone(),
            };
            handed_in.push(handed(b, cell, true, Some(value)));
        }
        for b in &outputs {
            if d.flow.blocks[b].values.is_none() && (failed || !opinion(&d.flow, b, i)) {
                handed_in.push(handed(b, cell, true, None));
            }
        }
        Came::HandsIn(handed_in)
    }
}

impl Part for Random {
    fn returns(&mut self, d: &mut Driver, _: RunId, cell: &str) -> Came {
        let i = place(&d.flow, cell);
        if d.flow.cells[i].kind == Kind::Engine {
            return self.act(d, i, cell);
        }
        if self.sloppy && d.rng.chance(50) {
            return Came::Asks(vec![]);
        }
        match d.rng.below(100) {
            0..=9 => {
                let n = 1 + d.rng.below(3);
                let asks = (0..n)
                    .map(|k| {
                        if d.rng.chance(70) {
                            Ask::Tool {
                                name: format!("tool {k}"),
                                input: "x".to_string(),
                            }
                        } else {
                            let who = if d.rng.chance(50) { Who::Owner } else { Who::Architect };
                            Ask::Person {
                                who,
                                question: "which?".to_string(),
                            }
                        }
                    })
                    .collect();
                Came::Asks(asks)
            }
            10..=13 => Came::Asks(vec![]),
            14..=17 => {
                let mut handed_in = self.valid(d, i, cell);
                let outputs = &d.flow.flow.cells()[i].outputs;
                let others: Vec<BlockId> = (d.flow.blocks.keys())
                    .filter(|b| !outputs.contains(b))
                    .cloned()
                    .collect();
                let block = others[d.rng.below(others.len())].clone();
                let value = d.flow.blocks[&block].values.as_ref().map(|v| v[0].clone());
                handed_in.push(handed(&block, cell, true, value));
                Came::HandsIn(handed_in)
            }
            _ => Came::HandsIn(self.valid(d, i, cell)),
        }
    }

    fn acts(&mut self, d: &mut Driver) -> Event {
        let blocks = &d.flow.blocks;
        let pick = |rng: &mut Rng, of: Vec<&BlockId>| (!of.is_empty()).then(|| of[rng.below(of.len())].clone());
        let people_choices: Vec<&BlockId> = (blocks.iter())
            .filter(|(_, b)| b.writers.people && b.values.is_some())
            .map(|(id, _)| id)
            .collect();
        let people_prose: Vec<&BlockId> = (blocks.iter())
            .filter(|(_, b)| b.writers.people && b.values.is_none())
            .map(|(id, _)| id)
            .collect();
        let outside: Vec<&BlockId> = (blocks.iter())
            .filter(|(_, b)| b.writers.outside.is_some())
            .map(|(id, _)| id)
            .collect();
        // Around the check: a person pauses the work while the check's round is under way, and while the pause holds
        // the merge back after the check passed, writes for a judge or lets the work on, so that a judge looks again
        // after the check passed.
        let paused = decide::paused(&d.work.records);
        let check = &d.flow.flow.cells()[self.roles.check].id;
        let checking = (d.held.iter()).any(|e| matches!(e, Effect::Round { cell, .. } if cell == check));
        if checking && !paused {
            return Event::Paused;
        }
        if paused && passed_before_merge(d, &self.roles) {
            let for_judges: Vec<&BlockId> = (self.roles.judges.iter())
                .flat_map(|(j, _)| &d.flow.flow.cells()[*j].inputs)
                .filter(|b| blocks[*b].writers.people)
                .collect();
            match d.rng.below(10) {
                0..=4 => {
                    if let Some(b) = pick(&mut d.rng, for_judges) {
                        return wrote(&b, Outsider::Person, true, None);
                    }
                }
                5..=7 => return Event::Resumed,
                _ => {}
            }
        }
        let event = match d.rng.below(100) {
            0 => Some(Event::Cancelled),
            1..=21 => pick(&mut d.rng, people_choices).map(|b| {
                let values = blocks[&b].values.clone().expect("a choice");
                let value = values[d.rng.below(values.len())].clone();
                wrote(&b, Outsider::Person, true, Some(value))
            }),
            22..=36 => pick(&mut d.rng, people_prose).map(|b| wrote(&b, Outsider::Person, true, None)),
            37..=51 => pick(&mut d.rng, outside).map(|b| match blocks[&b].values.clone() {
                Some(values) => {
                    let value = match self.roles.passing.get(&b) {
                        Some(pass) if d.rng.chance(80) => pass.clone(),
                        _ => values[d.rng.below(values.len())].clone(),
                    };
                    wrote(&b, Outsider::Outside, true, Some(value))
                }
                None => {
                    let matters = d.rng.chance(50);
                    wrote(&b, Outsider::Outside, matters, None)
                }
            }),
            52..=66 => {
                let cells = d.flow.flow.cells();
                let cell = match decide(&d.flow.flow, &d.work.records) {
                    Decision::Wait { at, on } if on.iter().any(|w| matches!(w, Waited::Limit { .. })) => at,
                    _ => cells[d.rng.below(cells.len())].id.clone(),
                };
                Some(Event::LetGo { cell })
            }
            67..=76 => Some(if paused { Event::Resumed } else { Event::Paused }),
            _ => None,
        };
        event.unwrap_or(Event::Look)
    }
}

/// The content continuation's rounds: each hands in what its cell must write to go forward, and says nothing more.
struct Content {
    /// The value each choice is written with.
    choices: BTreeMap<BlockId, String>,
}

impl Content {
    /// The values of the choices for the continuation from the work as it is: each verdict a value other than its
    /// refusal, each choice a precondition of kind `all` reads the value it looks for, every other choice its value
    /// now, or no where it was never written.
    fn new(d: &Driver, roles: &Roles) -> Self {
        let reading = Reading::new(&d.flow.flow, &d.work.records, None);
        let choices = (d.flow.blocks.iter())
            .filter_map(|(b, block)| {
                let values = block.values.as_ref()?;
                let value = if let Some(refusal) = roles.refusals.get(b) {
                    values
                        .iter()
                        .find(|v| *v != refusal)
                        .expect("a verdict has a value that is no refusal")
                        .clone()
                } else if let Some(pass) = roles.passing.get(b) {
                    pass.clone()
                } else {
                    reading.value(b).unwrap_or(NO).to_string()
                };
                Some((b.clone(), value))
            })
            .collect();
        Self { choices }
    }
}

impl Part for Content {
    fn returns(&mut self, d: &mut Driver, _: RunId, cell: &str) -> Came {
        let i = place(&d.flow, cell);
        let handed_in = (d.flow.flow.cells()[i].outputs.iter())
            .filter_map(|b| {
                if d.flow.blocks[b].values.is_some() {
                    Some(handed(b, cell, true, Some(self.choices[b].clone())))
                } else if opinion(&d.flow, b, i) {
                    None
                } else {
                    Some(handed(b, cell, !ever_written(&d.work.records, b), None))
                }
            })
            .collect();
        Came::HandsIn(handed_in)
    }

    fn acts(&mut self, _: &mut Driver) -> Event {
        unreachable!("in the content continuation nobody acts at random")
    }
}

/// How many steps the content continuation may take before the work merges.
///
/// In the continuation every round hands in what finishes its run, and nothing is written while a round is under
/// way, so a run begun in it has one round and is never abandoned. Each block changes its meaning at most once in it:
/// prose is written mattering only where it was never written, and a run that finishes saw what it writes over, its
/// run being stale otherwise; every choice is written with one value, the one the owner or the outside also writes
/// where a precondition waits on it, and the conditions, which read choices, come out the same throughout. So after
/// one run for whatever had changed when the continuation began, a cell runs again only for a change of one of its
/// inputs, once each, and the run that was open as the continuation began may be abandoned besides: at most
/// `2 + n` runs of a cell with `n` inputs, each taking at most `PER_RUN` steps. Besides those, the driver does once
/// what it held as the continuation began, the owner resumes the work once, and the owner or the outside writes each
/// block a named test reads at most once. A block a named test reads that only cells write is not written by the
/// owner but asked for: the owner writes once in an opinion for each cell that writes it, which runs that cell once
/// more, after which the cell has written the block with its one value. That write and that run are counted apart:
/// one step, and `PER_RUN` steps, for each writer of each such block (the run is also one of the `2 + n` of its cell,
/// the opinion being one of its inputs; counted twice, the bound only grows). The limits of runs do not enter: the
/// owner lets every cell go on.
fn content_bound(d: &Driver) -> u64 {
    let flow = &d.flow.flow;
    let runs: u64 = flow.cells().iter().map(|c| 2 + c.inputs.len() as u64).sum();
    let tested: BTreeSet<&BlockId> = flow.tests().values().flat_map(|t| &t.reads).collect();
    let asked: u64 = (tested.iter())
        .map(|b| &d.flow.blocks[*b].writers)
        .filter(|w| !w.people && w.outside.is_none())
        .map(|w| w.cells.len() as u64)
        .sum();
    d.held.len() as u64 + PER_RUN * runs + tested.len() as u64 + 1 + asked * (1 + PER_RUN)
}

/// The owner's few words to a cell, where the named test reads a block only cells write: in an input of a cell that
/// writes it which only people write, so that the cell runs again and writes the block anew; none where no such cell
/// or input is.
fn ask_a_cell(d: &Driver, name: &TestName) -> Option<Event> {
    let test = &d.flow.flow.tests()[name];
    let only_cells = |b: &BlockId| {
        let w = &d.flow.blocks[b].writers;
        !w.people && w.outside.is_none()
    };
    let only_people = |b: &BlockId| {
        let block = &d.flow.blocks[b];
        let w = &block.writers;
        w.people && w.cells.is_empty() && w.outside.is_none() && block.values.is_none()
    };
    (test.reads.iter().filter(|b| only_cells(b)))
        .flat_map(|b| &d.flow.blocks[b].writers.cells)
        .filter_map(|c| d.flow.flow.place(c))
        .find_map(|j| d.flow.flow.cells()[j].inputs.iter().find(|b| only_people(b)))
        .map(|b| wrote(b, Outsider::Person, true, None))
}

/// What one history found: the first breach of each property, and what it reached.
#[derive(Default)]
struct Verdict {
    broken: BTreeMap<&'static str, String>,
    reached: BTreeSet<String>,
    /// Of each cell, the runs beyond its first that finished in the content continuation, and those of them that
    /// changed no block's meaning.
    runs: BTreeMap<CellId, (u64, u64)>,
    /// The steps the content continuation took, and its bound.
    steps: Option<(u64, u64)>,
}

/// A history of a work on the engineer's flow.
struct History {
    d: Driver,
    roles: Roles,
    verdict: Verdict,
    /// How many rounds of the merge were judged.
    merges: usize,
    /// Whether the work has ended.
    ended: bool,
}

impl History {
    fn breach(&mut self, property: &'static str, what: String) {
        self.d.note(format!("BROKEN {property}: {what}"));
        self.verdict.broken.entry(property).or_insert(what);
    }

    fn reach(&mut self, what: String) {
        self.verdict.reached.insert(what);
    }

    /// After every step: judges each round of the merge that began since, and counts what the work reached.
    fn observe(&mut self) {
        let mut merges: Vec<Seq> = {
            let records = &self.d.work.records;
            let last = &self.d.flow.flow.cells()[self.roles.last].id;
            (records.rounds.iter())
                .filter(|r| records.started.iter().any(|s| s.run == r.run && &s.cell == last))
                .map(|r| r.at)
                .collect()
        };
        merges.sort();
        for at in merges.split_off(self.merges.min(merges.len())) {
            self.merges += 1;
            self.judge_merge(at);
        }
        let flow = &self.d.flow.flow;
        let records = &self.d.work.records;
        let reading = Reading::new(flow, records, None);
        let mut reached = Vec::new();
        let ran_cells: BTreeSet<usize> = (reading.finished().iter())
            .filter_map(|f| flow.place(&reading.started(f.run).cell))
            .collect();
        for &i in &ran_cells {
            let cell = &flow.cells()[i];
            reached.push(ran(&cell.id));
            if cell.applies.is_some() && !reading.applies(i) {
                reached.push(skipped(&cell.id));
            }
        }
        // Whether a verdict stands at its refusal, written by its judge or by a person, after the check passed and
        // before any run of the merge.
        let passed = passed_before_merge(&self.d, &self.roles);
        for (b, refusal) in &self.roles.refusals {
            if let Some(v) = reading.current(b)
                && passed
                && v.value.as_ref() == Some(refusal)
                && reading.stands(v)
            {
                let by = if v.by == Writer::Person {
                    NO_BY_A_PERSON
                } else {
                    NO_BY_A_JUDGE
                };
                reached.push(by.to_string());
            }
        }
        match decide(flow, records) {
            Decision::Wait { on, .. } if on.iter().any(|w| matches!(w, Waited::Limit { .. })) => {
                reached.push(CELL_LIMIT.to_string());
            }
            Decision::Ended(_) => self.ended = true,
            _ => {}
        }
        for r in reached {
            self.reach(r);
        }
    }

    /// Properties 1 and 2, as the round of the merge at this place began.
    fn judge_merge(&mut self, at: Seq) {
        self.reach(MERGE_BEGAN.to_string());
        let flow = &self.d.flow.flow;
        let r = Reading::new(flow, &self.d.work.records, Some(at));
        let mut broken = Vec::new();
        for (j, block) in &self.roles.judges {
            if !r.applies(*j) {
                continue;
            }
            let judge = &flow.cells()[*j].id;
            let refusal = &self.roles.refusals[block];
            let own = r.of(block).iter().rev().find(|v| r.cell_of(v) == Some(judge));
            if let Some(v) = own
                && v.value.as_ref() == Some(refusal)
                && !r.of(block).iter().any(|x| x.by == Writer::Person && x.at > v.at)
            {
                broken.push((
                    MERGE_OVER_A_NO,
                    format!(
                        "the merge's round at {at:?} began while {judge}, not skipped, had as its own last write of \
                         {block} (at {:?}) {refusal}, and no person wrote {block} since",
                        v.at
                    ),
                ));
            }
        }
        for (block, refusal) in &self.roles.refusals {
            if let Some(v) = r.current(block)
                && v.by == Writer::Person
                && v.value.as_ref() == Some(refusal)
            {
                broken.push((
                    MERGE_OVER_A_NO,
                    format!(
                        "the merge's round at {at:?} began while a person's write of {block} (at {:?}), its latest, \
                         was {refusal}",
                        v.at
                    ),
                ));
            }
        }
        let check = &flow.cells()[self.roles.check];
        match r.record(&check.id) {
            None => broken.push((
                MERGE_ON_AN_OLD_CHECK,
                format!(
                    "the merge's round at {at:?} began with no record of the check {}",
                    check.id
                ),
            )),
            Some(f) => {
                for (b, pass) in self.roles.check_passes(&self.d.flow) {
                    let value = f.writes.iter().find(|w| &w.block == b).and_then(|w| w.value.clone());
                    if value.as_ref() != Some(pass) {
                        broken.push((
                            MERGE_ON_AN_OLD_CHECK,
                            format!(
                                "the merge's round at {at:?} began while the check's record (at {:?}) wrote {b} as \
                                 {value:?}, not {pass}",
                                f.at
                            ),
                        ));
                    }
                }
                let basis = &r.started(f.run).basis;
                for b in &check.inputs {
                    let from = basis.get(b).copied().flatten();
                    let (was, now) = (r.meaning_of(b, from), r.meaning_now(b));
                    if was != now {
                        broken.push((
                            MERGE_ON_AN_OLD_CHECK,
                            format!(
                                "the merge's round at {at:?} began while the check's record (at {:?}) started from \
                                 {b} at {from:?}, of meaning {was:?}, and its meaning is {now:?}",
                                f.at
                            ),
                        ));
                    }
                }
            }
        }
        for (property, what) in broken {
            self.breach(property, what);
        }
    }

    /// What is still to be counted once the history is over: what judges, people and the check wrote, and the limits
    /// and stops the driver saw.
    fn reached_at_the_end(&mut self) {
        let flow = &self.d.flow;
        let records = &self.d.work.records;
        let cells = flow.flow.cells();
        let check = &cells[self.roles.check].id;
        let mut reached = Vec::new();
        for f in &records.finished {
            let cell = &records.started.iter().find(|s| s.run == f.run).expect("a run").cell;
            for w in &f.writes {
                if let Some(refusal) = self.roles.refusals.get(&w.block)
                    && w.value.as_ref() == Some(refusal)
                {
                    reached.push(refused(cell, &w.block, refusal));
                }
                if cell == check
                    && let Some(pass) = self.roles.passing.get(&w.block)
                    && w.value.as_ref() != Some(pass)
                {
                    reached.push(check_failed(&w.block, pass));
                }
            }
        }
        for w in &records.written {
            if w.by == Outsider::Person && self.roles.refusals.contains_key(&w.block) {
                reached.push(person_wrote(&w.block));
            }
        }
        if self.d.seen.limits > 0 {
            reached.push(RUN_LIMIT.to_string());
        }
        if self.d.seen.stops > 0 {
            reached.push(ENGINE_STOPPED.to_string());
        }
        for r in reached {
            self.reach(r);
        }
    }

    /// What the owner or the outside does in the content continuation where nothing is to be done: resumes the work,
    /// lets the cell it waits at go on at its limit, or makes a precondition it waits on hold, or asks the cell that
    /// writes what it reads to write it again, which it says; none where none of these moves it on.
    fn resolve(&self, decision: &Decision) -> Option<(Event, bool)> {
        if decide::paused(&self.d.work.records) {
            return Some((Event::Resumed, false));
        }
        let Decision::Wait { at, on } = decision else {
            return None;
        };
        on.iter().find_map(|w| match w {
            Waited::Limit { .. } => Some((Event::LetGo { cell: at.clone() }, false)),
            Waited::Precondition { test, .. } => {
                (make_hold(&self.d, test).map(|e| (e, false))).or_else(|| ask_a_cell(&self.d, test).map(|e| (e, true)))
            }
            Waited::Call { .. } => None,
        })
    }

    /// Whether the work is at a turn the content continuation is tried from: the check's record failed, or, where
    /// `refusals` says so, a judge's own refusal stands; which, where it is.
    fn turning(&self, refusals: bool) -> Option<&'static str> {
        let flow = &self.d.flow.flow;
        let reading = Reading::new(flow, &self.d.work.records, None);
        let check = &flow.cells()[self.roles.check].id;
        let failed = reading.record(check).is_some_and(|f| {
            (self.roles.check_passes(&self.d.flow))
                .any(|(b, pass)| f.writes.iter().any(|w| &w.block == b && w.value.as_ref() != Some(pass)))
        });
        if failed {
            return Some(CONTENT_ON_A_FAILED_CHECK);
        }
        let refused = refusals
            && (self.roles.refusals.iter()).any(|(b, refusal)| {
                (reading.current(b)).is_some_and(|v| {
                    matches!(v.by, Writer::Run(_)) && v.value.as_ref() == Some(refusal) && reading.stands(v)
                })
            });
        refused.then_some(CONTENT_ON_A_REFUSAL)
    }

    /// Property 3, and the counts of property 4: the content continuation from the work as it is.
    fn content(&mut self) {
        self.d.note("the content continuation begins".into());
        let mut part = Content::new(&self.d, &self.roles);
        let from = self.d.work.tick();
        let bound = content_bound(&self.d);
        let mut steps = 0;
        loop {
            let decision = decide(&self.d.flow.flow, &self.d.work.records);
            if decision == Decision::Ended(End::Merged) {
                self.reach(CONTENT_MERGED.to_string());
                break;
            }
            if steps == bound {
                let what = format!("the work has not merged within {bound} steps, deciding {decision:?}");
                self.breach(CONTENT_MERGES, what);
                break;
            }
            self.d.maybe_stop();
            let doable = self.d.doable(true);
            if doable.is_empty() {
                let Some((event, asked)) = self.resolve(&decision) else {
                    let what = format!(
                        "after {steps} steps the work rests where nobody content moves it, deciding {decision:?}"
                    );
                    self.breach(CONTENT_MERGES, what);
                    break;
                };
                if asked {
                    self.reach(OWNER_ASKED_A_CELL.to_string());
                }
                assert!(self.d.take(event), "the continuation's act was refused");
            } else {
                let effect = doable[self.d.rng.below(doable.len())].clone();
                self.d.carry_out(effect, &mut part);
            }
            steps += 1;
            self.observe();
        }
        self.verdict.steps = Some((steps, bound));
        self.count_runs(from);
    }

    /// Of each cell, the runs beyond its first that finished from this place on, and those of them that changed no
    /// block's meaning, held to the bound of runs `content_bound` rests on.
    fn count_runs(&mut self, from: Seq) {
        let flow = &self.d.flow.flow;
        let reading = Reading::new(flow, &self.d.work.records, None);
        let mut first = BTreeSet::new();
        for f in reading.finished() {
            let cell = reading.started(f.run).cell.clone();
            if first.insert(cell.clone()) || f.at < from {
                continue;
            }
            let changed =
                (f.writes.iter()).any(|w| (reading.of(&w.block).iter()).any(|v| v.at == f.at && v.changed_meaning()));
            let runs = self.verdict.runs.entry(cell).or_default();
            runs.0 += 1;
            if !changed {
                runs.1 += 1;
            }
        }
        let over: Vec<String> = (self.verdict.runs.iter())
            .filter(|(cell, (_, nothing))| {
                let inputs = flow.cells()[flow.place(cell).expect("a cell")].inputs.len() as u64;
                *nothing > 2 + inputs
            })
            .map(|(cell, (_, nothing))| format!("{cell}: {nothing}"))
            .collect();
        if !over.is_empty() {
            self.breach(
                RUNS_FOR_NOTHING,
                format!("more runs for nothing than 2 + inputs: {over:?}"),
            );
        }
    }
}

/// One history of a work on the flow, from the seed: what it found, and its steps.
fn one_history(flow: &Compiled, seed: u64, log: &mut Vec<String>) -> Verdict {
    let mut rng = Rng::new(seed);
    let random = rng.below(RANDOM);
    let starts = rng.chance(80);
    let sloppy = rng.chance(20);
    // Whether the random steps end at a turn: none in half the histories, a failed check in a quarter, a failed check
    // or a judge's refusal in a quarter.
    let early = rng.below(4);
    let roles = Roles::new(flow);
    let mut part = Random {
        roles: roles.clone(),
        sloppy,
    };
    let mut h = History {
        d: Driver::new(flow.clone(), rng, seed, true),
        roles,
        verdict: Verdict::default(),
        merges: 0,
        ended: false,
    };
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let first = (h.d.flow.flow.cells()[0].requires.first())
            .filter(|_| starts)
            .and_then(|test| make_hold(&h.d, test))
            .unwrap_or(Event::Look);
        h.d.take(first);
        h.observe();
        for _ in 0..random {
            if h.ended {
                break;
            }
            h.d.step(&mut part, true);
            h.observe();
            if early >= 2
                && let Some(turn) = h.turning(early == 3)
            {
                h.reach(turn.to_string());
                break;
            }
        }
        if h.d.work.records.cancelled.is_none() {
            h.content();
        }
        h.reached_at_the_end();
    }));
    if let Err(e) = result {
        let message = (e.downcast_ref::<String>().cloned())
            .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_string()))
            .unwrap_or_default();
        h.breach(DRIVER, message);
    }
    *log = std::mem::take(&mut h.d.log);
    h.verdict
}

/// A history that broke a property: its seed, the breach, and its steps.
type Case = (u64, String, Vec<String>);

/// Held while a test runs its histories: each silences the panic hook while its histories run and gives it back
/// before it fails, and two tests side by side would each take the other's.
static HOOK: Mutex<()> = Mutex::new(());

/// Runs the histories, prints what they reached and counted, and fails if one breaks a property, or if none reached a situation the properties are about.
fn histories() {
    let _hook = HOOK.lock().unwrap_or_else(PoisonError::into_inner);
    let flow = engineer();
    let roles = Roles::new(&flow);
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let mut reached: BTreeMap<String, u64> = required(&flow, &roles).into_iter().map(|r| (r, 0)).collect();
    let mut runs: BTreeMap<CellId, (u64, u64)> = BTreeMap::new();
    let mut broken: BTreeMap<&str, Vec<Case>> = BTreeMap::new();
    let mut continued = 0;
    let mut most = (0, 1);
    for seed in 0..SEEDS {
        let mut log = Vec::new();
        let verdict = one_history(&flow, seed, &mut log);
        for r in verdict.reached {
            *reached.entry(r).or_default() += 1;
        }
        for (cell, (all, nothing)) in verdict.runs {
            let total = runs.entry(cell).or_default();
            total.0 += all;
            total.1 += nothing;
        }
        if let Some((steps, bound)) = verdict.steps {
            continued += 1;
            if steps * most.1 > most.0 * bound {
                most = (steps, bound);
            }
        }
        for (property, what) in verdict.broken {
            broken.entry(property).or_default().push((seed, what, log.clone()));
        }
    }
    panic::set_hook(hook);
    println!("{SEEDS} histories, {continued} with a content continuation; the most of its bound one took: {most:?}");
    println!("reached, in how many histories:");
    for (what, n) in &reached {
        println!("  {n:5}  {what}");
    }
    println!("runs beyond a cell's first in the content continuation, and of them runs for nothing:");
    for (cell, (all, nothing)) in &runs {
        println!("  {cell}: {all} runs, {nothing} for nothing");
    }
    let mut failures = Vec::new();
    for (property, cases) in &broken {
        let seeds: Vec<u64> = cases.iter().map(|(s, _, _)| *s).collect();
        println!("{property}: broken in {} histories: {seeds:?}", cases.len());
        // The shortest failing history, to read first.
        let (seed, what, log) = cases.iter().min_by_key(|(_, _, log)| log.len()).expect("a case");
        failures.push(format!(
            "{property}: broken in {} histories, the first {:?}; the shortest, seed {seed}: {what}\n\nits steps:\n{}",
            cases.len(),
            &seeds[..seeds.len().min(10)],
            log.join("\n")
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    let unreached: Vec<&String> = reached.iter().filter(|(_, n)| **n == 0).map(|(r, _)| r).collect();
    assert!(
        unreached.is_empty(),
        "no history reached these, so the properties about them check nothing: {unreached:?}"
    );
}

#[test]
fn random_works_on_the_engineers_flow_keep_what_the_flow_must() {
    histories();
}
