//! Simulated histories of whole works on the engineer's flow as the repository defines it (`flow/engineer`), through
//! `handle` only, by the driver the histories share (`support`). The engine's own properties are checked by the
//! histories of the small flows; these check what the engineer's flow must keep. Every name comes from the compiled
//! definition: the merge is its last cell, the check the cell the act `check` carries out, a verdict a block that a
//! `none_standing` test reads, its refusal the value that test looks for, a judge a cell that writes a verdict.
//!
//! 1. No round of the merge begins while a judge that is not skipped has, as its own last write of its verdict, the
//!    refusal, with no person's write of the verdict since; nor while a person's write of a verdict is its latest and
//!    is the refusal (property no-merge-on-what-is-not-settled).
//! 2. No round of the merge begins unless the check's record wrote each choice it writes that a precondition of kind
//!    `all` reads with the value that precondition looks for (property no-merge-on-what-is-not-settled), and nothing is
//!    new to a cell before the merge that is not skipped: of each block that runs it, every version that changes what
//!    the block says and that it did not write itself was dealt with by a return of it (property
//!    merge-only-on-what-is-current).
//! 3. From the state a random history reached, a work everyone is content with merges within `content_bound` steps
//!    (property a-work-comes-to-rest).
//! 4. In that continuation, the runs of each cell beyond its first that changed no block's meaning, runs for nothing,
//!    are counted, written to the standard error, and held to the bound `content_bound` rests on.
//! 5. No finished run lacks a write of a block its cell must write (`must_write`), read from the records of the
//!    finishes and the cells' definitions after every step (property a-run-is-one-mechanism).
//! 6. The work's head that merges is a version the review dealt with: as the merge's run finishes on a branch the work
//!    committed to, the branch is at the head the check last passed, and each judge that the head runs and that
//!    applies, a cell that writes a block the check's preconditions read, was told of a version that says that head
//!    by a run of it that dealt with it.
//! 7. A work never stalls at the check's gate: after every step, the records alone (`Reading::rest`), not the deciding
//!    code, do not read it as stalled on a key that reads a choice the check's record wrote as it failed, which only
//!    the owner speaking to a cell would move; and the deciding code says the work is stalled where, and only where,
//!    the records read it so, so that a stall it calls a wait is found.
//!
//! The first two are judged as each round of the merge begins, and the sixth as the merge finishes, by the histories'
//! own reading of the records as they were then (`reading`), never by the deciding code's tests.
//!
//! The work's branch is drawn beside the records, as its carriers read it: its work head is a commit named by a
//! number, which an agent's round on a line that may change the work moves in two of five, and a person in one act in
//! twenty, and once more in one of three steered refusals, after the check passed. As the carriers do, an agent's round
//! on a line that may change the work hands in the head with its return where it moved; a round on a line that only
//! reads, and the start, write it from outside before they end; the check, as it judges, writes it from outside for
//! its round where no version says it, and fails in passing, judging nothing, where a judge of the head dealt last with
//! another; and the merge, as it begins, writes it from outside where no version says it and fails in passing. No head
//! is read before the work commits, its branch holding no change of its own.
//!
//! What a history is. It begins with the owner's start, the block that runs the first cell, which the outside writes
//! once; then, with a chance of four in five, with the outside making the first cell's precondition hold (the items
//! the work depends on merged); otherwise with a look. Then, for 0 to 199 steps, the driver's random
//! steps, people acting at a step with a chance of one in four and the engine stopping at any step:
//! - an agent's round ends with one to three calls in a tenth of the rounds (a tool in seven of ten, a question for the
//!   owner or the architect otherwise), asking for nothing in one in twenty-five, handing in a block its cell may not
//!   write beside the rest in one in twenty-five, and otherwise handing in what its cell may write: each choice in nine
//!   of ten, its first value (yes) in a little over half of those; each other block in four of five, mattering in seven
//!   of ten; but a block a cell before it reads, an opinion for that cell, in about a third, mattering; and beside a
//!   value that stands with nothing but some blocks (`with_nothing_but`), those alone, and no question. A return that
//!   hands in names as dealt with each thing its round was told ran it, by its name, as an agent writes it. Beside
//!   such a hand-in, the agent of a cell an answer runs writes in the record of the questions a question of its own in
//!   five of a hundred, which waits in one of two, the return then leaving out each block its cell must write in one
//!   of two, as the decision a-return-hands-in-what-its-cell-must-every-time lets it; asks one of its own that stands
//!   anew in two of a hundred, and withdraws one in two of a hundred. In one history of
//!   five the agents are sloppy: half their returns ask for nothing and are sent back, so that runs reach their limit
//!   of returns;
//! - an act of the engine's own hands in every block its cell may write: each choice with the value a precondition of
//!   kind `all` looks for in three of five (the check passing), another otherwise; an opinion for a cell before it only
//!   when a choice did not have that value (the check failing says why);
//! - a person who acts while the check's round is under way pauses the work, and while the pause then holds the merge
//!   back after the check passed, writes an input of a judge that people write in two of five acts, writes a verdict as
//!   its refusal in one of five and lets the work on in three of ten, so that a judge looks again, or a person refuses,
//!   after the check passed. Where a question stands with no answer, the owner answers one, by either option, in one
//!   act of two. Otherwise a person writes a choice people write (a verdict), yes or no, in about a fifth of the acts;
//!   prose people write (the background, an opinion for a cell) in three of twenty; the outside writes one of its
//!   blocks, a choice with the value a precondition looks for in four of five, prose mattering in half but its first
//!   version always, in three of twenty; the owner lets the cell at its limit go on, or any cell where none is, in
//!   three of twenty; pauses or resumes in a tenth; answers a question that stands, its latest version, by either
//!   option, in six of a hundred; cancels in one in a hundred; looks otherwise.
//!
//! Seven histories of ten are steered, drawn by a generator of their own so that the other draws keep their order, each
//! taking every random step it may; in them a judge's verdict, where a return hands one in, is its refusal in one of
//! ten only, so that they reach the check past the five judges before it, a choice a judge's own condition waits for
//! (whether the change needs adversarial testing) is that value in seven of ten, and the owner only lets the work go
//! on: writes the background first, so that the plan's cells and the implementation run before the check, answers each
//! question at once, lets a cell go on at its limit, resumes a paused work, writes what the outside writes, writes a
//! choice people write (a verdict as other than its refusal in nine of ten), and answers a question that stands anew;
//! it does not cancel, pause, write other prose a person writes, commit, speak to a line or remark on a question,
//! which in the histories not steered end or rework a work before the check. In three of ten, the work is
//! paused while the
//! check's round is under way; once
//! the check passed, a person writes, while the pause holds the merge back, an input only people write that runs a cell
//! before a judge, the background, after which the first judge to hand in refuses, or writes a verdict as its refusal,
//! a coin of its own deciding; then the work is let on. In one of ten the check
//! fails the first time it runs and the random steps end there; in two of ten, they end only once the cell told of
//! the failure has answered it with a commit and nothing that matters, the commit running the review and the check
//! again through the work's head; in one of ten, the first cell that may ask asks a question that does not wait, and
//! the random steps end once the round of the cell told of the failure that answered it with nothing committed has
//! ended, taken or sent back, so that the check is left red while a question the owner answered stands, as the
//! property 7 is about. Of the histories not steered, in a third the random steps end early: in a sixth at
//! the first step after which the check's record failed, in a sixth at the first after which it failed or a judge's
//! own refusal stands. The content continuation is then tried from where the work must go back.
//!
//! Unless the work was cancelled or merged, the content continuation follows (`Content`): every round of an agent or
//! act hands in what its cell must write to go forward and says nothing more, an agent's round on a line that may
//! change the work committing besides where it fixes what it was told, as a return to what an act wrote must. Each
//! verdict is written as a value other than its refusal; each choice a precondition of kind `all` reads, as the value
//! it looks for (the check says yes); every other choice as it last was, or as the last of its declared values (no)
//! where it was never written. No opinion for a cell before it is written, unless the cell must write it each time, and
//! every other block it may write is written, mattering only where it was never written before, or where the run runs
//! because an opinion addressed to its cell changed (it fixes what it was told), or where the cell is an act of the
//! engine's, whose writes always matter; so every hand-in has each block its cell must write, and none is sent back.
//! Tools answer, the owner answers every question and lets every run go on at its limits; where nothing is to be done
//! and the work waits, the owner answers a question that stands with no answer by the option it recommends, resumes a
//! paused work, lets the cell it waits at go on at its limit, and makes a
//! precondition it waits on hold where a person or the outside can: writes a verdict whose refusal stands as a value
//! that is not, and the outside writes what the first cell's precondition looks for. Where the precondition reads a
//! choice that only cells write (the check's), the work is stalled: the owner speaks to the line of the cell that
//! writes it, where the owner's words run that cell, or, where they do not (the check is spoken to by no one), of the
//! nearest cell before it that they run and that applies (the implementation, of the engineer's line), so that the cell
//! runs again, or the work goes on from the cell spoken to until it does; in the continuation the check then says yes.
//! Among the random steps, the owner now and then speaks to a line.
//!
//! Every situation the properties are about is counted over all histories (`required`), and the test fails if one was
//! reached in fewer than `FLOOR` histories: the histories run in batches of `SEEDS` (up to `BATCHES`), and the first
//! batch after which each was reached in `FLOOR` is the last. A failing case prints its seed and its steps;
//! `one_history(&flow, seed, ..)` replays it.

// The histories use only part of the driver the tests of histories share.
#[allow(dead_code)]
#[path = "../support/mod.rs"]
mod support;

mod groups;
mod merging;
mod reading;
mod single;

// The reading of the records that judges where a work stands (`Reading::rest`) is the histories' own of crate
// `reading`, which this crate's tests do not depend on; its one file is taken in as it is.
#[allow(dead_code)]
#[path = "../../../reading/src/lib.rs"]
mod whole;

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::rc::Rc;
use std::sync::{Mutex, PoisonError};

use decide::questions::Standing;
use decide::run::Failure;
use decide::{
    BlockId, Cause, CellId, Condition, Decision, End, Flow, Outsider, Pauser, Records, RunId, Seq, TestKind, Waited,
    Who, decide,
};
use engine::spoken::{self, To};
use engine::{Ask, Came, Effect, Event, Handed, Question, Way};
use flow::{Compiled, Has, Kind, Source, compile, read};
use reading::{Reading, Writer};
use support::{Driver, Part, Rng, lacking_a_must_write};

/// How many histories are run.
const SEEDS: u64 = 500;

/// In how many histories each situation the properties are about has to be reached. A situation that one history of
/// hundreds reaches pins nothing: a rule broken only there is found by that one history, and a change of the draws
/// that loses it loses the check without a sign.
const FLOOR: u64 = 10;

/// How many batches of [`SEEDS`] histories are run at most: the first batch in which each situation the properties are
/// about was reached in [`FLOOR`] histories is the last, and the test fails if the last batch leaves one short of it.
/// The seeds are the same each time.
const BATCHES: u64 = 10;

/// How many random steps, at most, a history takes before the content continuation.
const RANDOM: usize = 200;

/// The most steps the content continuation takes for one run, besides what the driver held as it began: the owner
/// letting the run's cell go on at its limit, the owner letting the run go on at a limit of its own, the up to three
/// calls its last round asked for, and its round, which hands in what finishes it.
const PER_RUN: u64 = 6;

const MERGE_OVER_A_NO: &str = "1: no merge over a judge's standing no";
const MERGE_ON_AN_OLD_CHECK: &str = "2: no merge on a failed check or on a record that is not current";
const CONTENT_MERGES: &str = "3: a work everyone is content with merges within the bound";
const RUNS_FOR_NOTHING: &str = "4: runs for nothing stay within the bound";
const MUST_WRITE_WRITTEN: &str = "5: no finished run lacks a block its cell must write";
const MERGE_UNSEEN: &str = "6: the work's head the merge merges is a version the review dealt with";
const CHECK_NEVER_STALLS: &str =
    "7: a work never stalls at the check's gate, and the decision says it is stalled where the records read it so";
const DRIVER: &str = "the driver";

/// The source of the block of the work's head, which the carriers read of the work's branch.
const HEAD: &str = "head";

/// The work's branch beside the records, shared by the parts of a history and the history itself: its work head, a
/// commit named by a number, none before the work commits; the work head the check last passed, where its record
/// passes; how often the check failed in passing on a head a judge had not dealt with, and how often the merge on a
/// head moved since the check passed.
#[derive(Clone, Copy, Debug, Default)]
struct Branch {
    head: u64,
    passed: Option<u64>,
    gated: u64,
    moved: u64,
}

impl Branch {
    /// What a version of the work's head says of the branch, where the work has committed.
    fn says(self) -> Option<String> {
        (self.head > 0).then(|| format!("h{}", self.head))
    }
}

/// The branch, shared.
type Shared = Rc<Cell<Branch>>;

/// The number the owner answers the question's latest version by: its number among the versions of its cell's
/// questions, which each cell of the engineer's flow that is asked in has.
fn latest_number(flow: &Compiled, records: &Records, q: &Standing<'_>) -> u64 {
    let number = engine::asked::number_of(flow, records, &q.latest.cell, q.latest.at);
    number.expect("a version of its cell's questions")
}

/// What the latest version of the block says, where it has one.
fn says_now<'a>(d: &'a Driver, block: &BlockId) -> Option<&'a str> {
    let at = decide::versions(&d.work.records).get(block)?.last()?.at;
    d.work.contents.get(&(block.clone(), at)).map(String::as_str)
}

/// The head a read of the branch finds, as a new version of its block: where the work has committed, and the latest
/// version says another head.
fn read_head(d: &Driver, roles: &Roles, branch: Branch) -> Option<(BlockId, String)> {
    let block = roles.head.clone()?;
    let now = branch.says()?;
    (says_now(d, &block) != Some(now.as_str())).then_some((block, now))
}

/// What the carriers read of the work's head as the round of the cell at this place ends: an agent's round on a line
/// that may change the work hands it in with its return; one on a line that only reads, and the start, write it from
/// outside first. The check reads it as it judges ([`gate`]).
fn round_end(d: &mut Driver, roles: &Roles, branch: Branch, i: usize, came: &mut Came) {
    let Some((block, content)) = read_head(d, roles, branch) else {
        return;
    };
    let spec = &d.flow.cells[i];
    let agent = spec.kind == Kind::Agent;
    let (writes, start) = (agent && !spec.reads_only, spec.act.as_deref() == Some("start"));
    if writes {
        came.read = vec![Handed {
            block,
            content,
            matters: true,
            value: None,
        }];
    } else if agent || start {
        let event = Event::Wrote {
            block,
            by: Outsider::Outside,
            content,
            matters: true,
            value: None,
        };
        assert!(d.take(event), "the outside's read of the head was refused");
    }
}

/// The check's read of the work's head and its gate, as the check takes them as it judges: the head is told to its
/// round as the outside's word, written as a new version where none says it, so that the round consumes the head it
/// judged; and where a judge of the head dealt last with another, or with none, the round fails in passing, judging
/// nothing. And the merge's read, as it begins: a head moved since its latest version, by a commit no round made, is
/// written from outside, and the merge fails in passing, merging nothing.
fn gate(d: &mut Driver, roles: &Roles, branch: &Shared, run: RunId, i: usize) -> Option<(Failure, String)> {
    if i == roles.last {
        let (block, content) = read_head(d, roles, branch.get())?;
        let wrote = Event::Wrote {
            block,
            by: Outsider::Outside,
            content: content.clone(),
            matters: true,
            value: None,
        };
        assert!(d.take(wrote), "the merge's read of the head was refused");
        let mut b = branch.get();
        b.moved += 1;
        branch.set(b);
        let why = format!("The work's head moved to {content} since the check passed.");
        return Some((Failure::Passing, why));
    }
    if i != roles.check {
        return None;
    }
    let block = roles.head.clone()?;
    let now = branch.get().says()?;
    let wrote = Event::WroteForRound {
        run,
        block: block.clone(),
        content: now.clone(),
    };
    assert!(d.take(wrote), "the check's read of the head was refused");
    let check = &d.flow.flow.cells()[i].id;
    let judges = decide::judges(&d.flow.flow, &d.work.records, check, &block);
    let said = |at: Seq| d.work.contents.get(&(block.clone(), at)).map(String::as_str);
    if judges
        .iter()
        .all(|judge| judge.consumed.and_then(said) == Some(now.as_str()))
    {
        return None;
    }
    let mut b = branch.get();
    b.gated += 1;
    branch.set(b);
    let why = format!("The work's head is {now}, which the review has not seen.");
    Some((Failure::Passing, why))
}

/// The engineer's flow as the repository defines it, compiled against what the engine has.
fn engineer() -> Compiled {
    compiled(&engineer_source())
}

/// The files of the engineer's flow as the repository defines it.
fn engineer_source() -> Source {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/engineer");
    read(&dir).expect("the engineer's flow is read")
}

/// The flow of these files, the engineer's or one made from it, compiled against what the engine has.
fn compiled(source: &Source) -> Compiled {
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
    let has = Has {
        sources: names(&[
            "answer",
            "asked",
            "words",
            "dependencies",
            "handed_in",
            "main",
            "start",
            "head",
            "tree",
        ]),
        acts: names(&["start", "check", "merge"]),
        // The tools the flow names for its agent cells, the fields of a return its data are written through, and the
        // place the engine fills with what changed of the tree.
        tools: names(&["query", "run_tests"]),
        ..Default::default()
    };
    compile(source, &has).expect("the engineer's flow compiles")
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
    /// Each choice a judge's own condition waits for, with the value it waits for: a steered history draws it more
    /// often, so that the judge runs in the histories that are steered to what follows a judge.
    gates: BTreeMap<BlockId, String>,
    /// Each block a test of content reads.
    contents: BTreeSet<BlockId>,
    /// The block of the work's head, where the flow has one.
    head: Option<BlockId>,
    /// Each judge of the head, by its place: a cell the head runs that writes a block the check's preconditions read.
    head_judges: Vec<usize>,
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
            let Some(is) = &t.is else { continue };
            for b in &t.reads {
                refusals.insert(b.clone(), is.clone());
            }
        }
        let judges: Vec<(usize, BlockId)> = (refusals.keys())
            .flat_map(|b| {
                (flow.blocks[b].writers.cells.iter()).map(move |c| (flow.flow.place(c).expect("a cell"), b.clone()))
            })
            .collect();
        let gates = (judges.iter())
            .flat_map(|(j, _)| &cells[*j].applies)
            .filter_map(|key| match key {
                Condition::Is { block, value } if flow.blocks[block].values.is_some() => {
                    Some((block.clone(), value.clone()))
                }
                _ => None,
            })
            .collect();
        // What a precondition waits for of a block: its value, as a key on the block or as a test of kind `all`.
        let mut passing = BTreeMap::new();
        for key in cells.iter().flat_map(|c| &c.requires) {
            match key {
                Condition::Is { block, value } => {
                    passing.insert(block.clone(), value.clone());
                }
                Condition::Test(name) if tests[name].kind == TestKind::All => {
                    let Some(is) = &tests[name].is else { continue };
                    for b in &tests[name].reads {
                        passing.insert(b.clone(), is.clone());
                    }
                }
                Condition::Test(_) => {}
            }
        }
        let contents = (tests.values())
            .filter(|t| t.kind == TestKind::AnyContent)
            .flat_map(|t| t.reads.iter().cloned())
            .collect();
        // The judges of the head, found by the graph as the check finds them: what its preconditions read, and who
        // writes that and is run by the head.
        let head = (flow.blocks.iter())
            .find(|(_, b)| b.writers.outside.as_deref() == Some(HEAD))
            .map(|(id, _)| id.clone());
        let read: Vec<&BlockId> = (cells[check].requires.iter())
            .flat_map(|key| key_reads(flow, key))
            .collect();
        let head_judges = (cells.iter().enumerate())
            .filter(|(_, c)| head.as_ref().is_some_and(|h| c.runs_on.contains(h)))
            .filter(|(_, c)| read.iter().any(|b| c.outputs.contains(b)))
            .map(|(j, _)| j)
            .collect();
        Self {
            last: cells.len() - 1,
            check,
            refusals,
            judges,
            passing,
            gates,
            contents,
            head,
            head_judges,
        }
    }

    /// The choices the check writes that a precondition waits on the value of, with the value it waits for.
    fn check_passes<'a>(&'a self, flow: &'a Compiled) -> impl Iterator<Item = (&'a BlockId, &'a String)> {
        let outputs = &flow.flow.cells()[self.check].outputs;
        self.passing.iter().filter(move |(b, _)| outputs.contains(b))
    }
}

/// Every situation some history must reach, so that the properties about it check something.
fn required(flow: &Compiled, roles: &Roles) -> Vec<String> {
    let cells = flow.flow.cells();
    let mut reached: Vec<String> = cells.iter().map(|c| ran(&c.id)).collect();
    reached.extend(cells.iter().filter(|c| can_turn(flow, c)).map(|c| skipped(&c.id)));
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
            ENGINE_STOPPED,
            LACKING_SENT_BACK,
            LACKING_TAKEN_WITH_A_QUESTION,
            MERGED_A_REVIEWED_HEAD,
            CHECK_GATED,
            MERGE_ON_A_MOVED_HEAD,
            ASKED_IN_THE_RECORD,
            RED_AFTER_AN_ANSWER,
        ]
        .map(String::from),
    );
    reached
}

/// The blocks a key of a condition or of preconditions reads: its block, or those of its named test.
fn key_reads<'a>(flow: &'a Compiled, key: &'a Condition) -> Vec<&'a BlockId> {
    match key {
        Condition::Is { block, .. } => vec![block],
        Condition::Test(name) => flow.flow.tests()[name].reads.iter().collect(),
    }
}

/// Whether the condition of the cell can come to fail after the cell ran, so that a history can skip it then: a block
/// a key of it reads can be written again with another value by a person or the outside, by the cell itself, or by a
/// cell that may run while the key holds. A writer whose own condition holds the block at another value than this
/// key's runs only while this key fails, and so cannot turn it: the research that hands the work to the deep research
/// is skipped from then on, and nothing writes the block again. One that holds it at the same value can: the research
/// runs again while it says no, and its yes skips the cell that runs on no.
fn can_turn(flow: &Compiled, cell: &decide::Cell) -> bool {
    cell.applies.iter().any(|key| {
        let held = match key {
            Condition::Is { value, .. } => Some(value),
            Condition::Test(_) => None,
        };
        key_reads(flow, key).into_iter().any(|block| {
            let writers = &flow.blocks[block].writers;
            let kept_out = |writer: &CellId| {
                let place = flow.flow.place(writer).expect("a writer is a cell of the flow");
                (flow.flow.cells()[place].applies.iter())
                    .any(|k| matches!(k, Condition::Is { block: b, value } if b == block && Some(value) != held))
            };
            writers.people
                || writers.outside.is_some()
                || (writers.cells.iter()).any(|writer| *writer == cell.id || !kept_out(writer))
        })
    })
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
// Not required: the engineer's flow leaves no work stalled at the check for the owner's word to a cell, since a return
// that deals with a failed check leaves what runs the check again, or is sent back; property 7 holds it to that.
const OWNER_ASKED_A_CELL: &str = "the owner had a cell write again a choice a precondition waits on";
const ENGINE_STOPPED: &str = "the engine stopped";
const LACKING_SENT_BACK: &str = "a hand-in that lacked a block its cell must write was sent back";
const LACKING_TAKEN_WITH_A_QUESTION: &str =
    "a hand-in that lacked a block its cell must write was taken, with a question its cell waits for";
const MERGED_A_REVIEWED_HEAD: &str = "the merge merged a head the work committed and the review dealt with";
const CHECK_GATED: &str = "the check failed in passing on a head a judge of it had not dealt with";
const MERGE_ON_A_MOVED_HEAD: &str = "the merge failed in passing on a head moved since the check passed";
const ASKED_IN_THE_RECORD: &str = "a cell asked the owner a question in the record of the questions";
const RED_AFTER_AN_ANSWER: &str =
    "a cell told of what the check found began while a question the owner had answered stood";

/// The source of the block the driver writes as the owner starts a work, which runs the first cell.
const START: &str = "start";

/// The owner starts the work: the outside writes the block of [`START`], as the driver does when the owner first lets
/// the work on.
fn started(flow: &Compiled) -> Event {
    let (block, _) = (flow.blocks.iter())
        .find(|(_, b)| b.writers.outside.as_deref() == Some(START))
        .expect("the flow has a block the owner's start writes");
    wrote(block, Outsider::Outside, true, None)
}

/// The place of the cell.
fn place(flow: &Compiled, cell: &str) -> usize {
    flow.flow.place(&cell.into()).expect("the cell is in the flow")
}

/// Whether the block is an opinion for a cell before the one at this place: a cell before it reads it.
fn opinion(flow: &Compiled, block: &BlockId, i: usize) -> bool {
    (flow.blocks[block].readers.cells.iter()).any(|c| flow.flow.place(c).is_some_and(|j| j < i))
}

/// Whether the block is an opinion addressed to the cell at this place: one of its inputs that only people write, as
/// prose, that stands for the owner's words to it, or that a cell after it writes.
fn addressed(flow: &Compiled, block: &BlockId, i: usize) -> bool {
    let b = &flow.blocks[block];
    let only_people =
        b.writers.people && b.writers.cells.is_empty() && b.writers.outside.is_none() && b.values.is_none();
    let words = b.writers.outside.as_deref() == Some(flow::WORDS);
    let from_after = (b.writers.cells.iter()).any(|c| flow.flow.place(c).is_some_and(|j| j > i));
    flow.flow.cells()[i].runs_on.contains(block) && (only_people || words || from_after)
}

/// Whether a question the owner answered stands in these records.
fn answered_stood(records: &Records) -> bool {
    (decide::questions::questions(records).values()).any(|q| q.stands() && q.answer.is_some())
}

/// Whether the block says something: a version of it changed its meaning, which a first version handed in as changing
/// nothing did not (design document `data-and-context`, 9), by the histories' own reading.
fn says_something(flow: &Flow, records: &Records, block: &BlockId) -> bool {
    (Reading::new(flow, records, None).current(block)).is_some_and(|v| v.meaning.is_some())
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

/// What the round of the run of the cell at this place returns that hands in these blocks. An agent's return names as
/// dealt with each thing its round was told ran it, as its agent is told them (crate `context`), and is read as the
/// agents' rounds read a return (`CellSpec::dealt_with`), a group by its own name; an act of the engine's
/// names none, and deals with everything that ran its round.
fn hands_in(d: &Driver, run: RunId, i: usize, writes: Vec<Handed>) -> Came {
    let spec = &d.flow.cells[i];
    if spec.kind == Kind::Engine {
        return Came::hands_in(writes);
    }
    let told = decide::told(&d.flow.flow, &d.work.records, run).expect("the run started");
    let names: Vec<String> = spec.ran(&told.ran_by).map(|t| t.name.clone()).collect();
    let (dealt_with, none) = spec.dealt_with(&names);
    assert!(none.is_empty(), "{} watches none of {none:?}", spec.id);
    Came::works(dealt_with, writes)
}

/// A write by whoever may write the block outside a cell, a person first, that makes the key hold, or moves its named
/// test towards holding; none where nobody outside a cell can.
fn make_hold(d: &Driver, key: &Condition) -> Option<Event> {
    let reading = Reading::new(&d.flow.flow, &d.work.records, None);
    let name = match key {
        Condition::Test(name) => name,
        // A key on a block's value: the person or the outside that writes the block writes that value.
        Condition::Is { block: b, value } => {
            let writers = &d.flow.blocks[b].writers;
            let by = if writers.people {
                Outsider::Person
            } else if writers.outside.is_some() {
                Outsider::Outside
            } else {
                return None;
            };
            return (reading.value_now(b) != Some(value.as_str())).then(|| wrote(b, by, true, Some(value.clone())));
        }
    };
    let test = &d.flow.flow.tests()[name];
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
        // A test of content looks for no value: no choice moves it.
        let is = test.is.as_ref()?;
        let value = match test.kind {
            TestKind::All | TestKind::Any => (reading.value(b) != Some(is.as_str())).then(|| is.clone()),
            TestKind::None | TestKind::NoneStanding => reading
                .current(b)
                .filter(|v| v.value.as_deref() == Some(is.as_str()))
                .filter(|v| test.kind != TestKind::NoneStanding || reading.stands(v))
                .and_then(|_| values.iter().find(|v| *v != is).cloned()),
            TestKind::AnyContent => None,
        };
        if let Some(value) = value {
            return Some(wrote(b, by, true, Some(value)));
        }
    }
    None
}

/// What a history is steered through, beside its random steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Steer {
    Not,
    /// Once the check passed, held back by a pause, a judge looks again and refuses, or a person refuses.
    Refuse,
    /// The check fails the first time it runs, and the random steps end there.
    FailCheck,
    /// The check fails the first time it runs, the cell told of it answers with a commit and nothing that matters, and
    /// the random steps end there.
    FailCheckAnswered,
    /// The first cell that may ask, as it hands in, asks a question that does not wait; the check fails the first time
    /// it runs, the cell told of it answers with nothing committed and nothing that matters, and the random steps end
    /// once that round has ended.
    FailCheckLeft,
}

/// The random steps of a history.
struct Random {
    roles: Roles,
    /// Whether the agents' returns are sent back half the time, so that a run reaches its limit of returns.
    sloppy: bool,
    steer: Steer,
    /// The steering's own generator, so that the other draws keep their order.
    steering: Rng,
    /// How far the steering through a refusal went: 0 before the check passed under a pause, 1 once a person wrote
    /// for it, 2 once the work was let on.
    stage: u8,
    /// Whether the steering paused the work, and is to let it on.
    paused_it: bool,
    /// Whether the next judge to hand in refuses.
    refuse_next: bool,
    /// Whether the steered check failure was handed in.
    check_failed: bool,
    /// Whether the owner of a steered history wrote the background, which runs the plan's cells and so the implementation
    /// before the check, as its first act.
    briefed: bool,
    /// The run of the cell told of the failure that answered it as the steering has it: with a commit and nothing that
    /// matters, or with nothing committed.
    answered: Option<RunId>,
    /// The work's branch.
    branch: Shared,
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
    /// The block the owner of a steered history writes first: the first block only people write, and not a choice, that a
    /// cell which applies runs on, which is the background.
    fn brief(&self, d: &Driver) -> Option<BlockId> {
        let reading = Reading::new(&d.flow.flow, &d.work.records, None);
        (d.flow.flow.cells().iter().enumerate())
            .filter(|(i, _)| reading.applies(*i))
            .flat_map(|(_, c)| c.runs_on.iter())
            .find(|b| {
                let block = &d.flow.blocks[*b];
                block.writers.people && block.writers.cells.is_empty() && block.values.is_none()
            })
            .cloned()
    }

    /// A hand-in of everything the cell writes as it stands: each choice as it is, or its first value where it never
    /// was, a verdict as its refusal where `refuse` says so; each other block, no opinion for a cell before it among
    /// them, as a change that does not matter.
    fn as_it_stands(&self, d: &Driver, i: usize, cell: &str, refuse: bool) -> Vec<Handed> {
        let reading = Reading::new(&d.flow.flow, &d.work.records, None);
        let mut handed_in = Vec::new();
        for b in &d.flow.flow.cells()[i].outputs {
            if let Some(values) = &d.flow.blocks[b].values {
                let value = match self.roles.refusals.get(b) {
                    Some(refusal) if refuse => refusal.clone(),
                    _ => (reading.value(b).map(str::to_string)).unwrap_or_else(|| values[0].clone()),
                };
                handed_in.push(handed(b, cell, true, Some(value)));
            } else if !opinion(&d.flow, b, i) || d.flow.cells[i].must_write.contains(b) {
                handed_in.push(handed(b, cell, false, None));
            }
        }
        handed_in
    }

    /// What the steering through a refusal does now, where it does anything: pauses the work while the check's round
    /// is under way; once the check passed under the pause, commits on the branch as a person, in one of three, or
    /// writes an input that runs the cells before a judge (which then refuses) or a verdict as its refusal, a coin of its
    /// own deciding; then
    /// lets the work on. Where the check failed under its
    /// pause, it lets the work on, to try at the next check.
    fn steered(&mut self, d: &Driver) -> Option<Event> {
        // The owner of a steered history briefs first, as the owner of a work does, so that the plan's cells and the
        // implementation run, and ask, before the check does: a quiet owner that never wrote it would have each verdict
        // written by a person and the check run with no cell that may ask having handed in.
        if self.steer != Steer::Not && !self.briefed {
            self.briefed = true;
            if let Some(background) = self.brief(d) {
                return Some(wrote(&background, Outsider::Person, true, None));
            }
        }
        if self.steer != Steer::Refuse || self.stage >= 2 {
            return None;
        }
        let check = &d.flow.flow.cells()[self.roles.check].id;
        let checking = (d.held.iter()).any(|e| matches!(e, Effect::Round { cell, .. } if cell == check));
        if self.stage == 1 && d.paused {
            self.stage = 2;
            self.paused_it = false;
            return Some(Event::Resumed);
        }
        if checking && !d.paused {
            self.paused_it = true;
            return Some(Event::Paused {
                by: Pauser::Owner,
                why: None,
            });
        }
        if !(d.paused && self.paused_it && !checking) {
            return None;
        }
        if !passed_before_merge(d, &self.roles) {
            self.paused_it = false;
            return Some(Event::Resumed);
        }
        self.stage = 1;
        // A person commits on the branch, which the merge finds moved since the check passed.
        if self.steering.chance(33) {
            let mut b = self.branch.get();
            b.head += 1;
            self.branch.set(b);
            return Some(Event::Look);
        }
        let reading = Reading::new(&d.flow.flow, &d.work.records, None);
        let only_people = |b: &&BlockId| {
            let block = &d.flow.blocks[*b];
            block.writers.people && block.writers.cells.is_empty() && block.values.is_none()
        };
        // The input a judge looks at again is one that runs the cells before it: a block only people write that some cell
        // which applies runs on, the background, from which the plan and then its judges are reached again.
        let for_judges: Vec<&BlockId> = (d.flow.flow.cells().iter().enumerate())
            .filter(|(i, _)| reading.applies(*i))
            .flat_map(|(_, c)| c.runs_on.iter().filter(only_people))
            .collect();
        if self.steering.chance(50) && !for_judges.is_empty() {
            self.refuse_next = true;
            let b = for_judges[self.steering.below(for_judges.len())];
            return Some(wrote(b, Outsider::Person, true, None));
        }
        let verdicts: Vec<(&BlockId, &String)> = self.roles.refusals.iter().collect();
        let (b, refusal) = verdicts[self.steering.below(verdicts.len())];
        Some(wrote(b, Outsider::Person, true, Some(refusal.clone())))
    }

    /// What an agent hands in that its cell may write.
    fn valid(&self, d: &mut Driver, i: usize, cell: &str) -> Vec<Handed> {
        let outputs = d.flow.flow.cells()[i].outputs.clone();
        let mut handed_in = Vec::new();
        for b in &outputs {
            if let Some(values) = d.flow.blocks[b].values.clone() {
                if d.rng.chance(90) {
                    // A steered history's judges pass in nine of ten, so that it reaches the check past the judges
                    // before it, the plan's and the change's.
                    let refusal = self.roles.refusals.get(b).filter(|_| self.steer != Steer::Not);
                    // A steered history's choice that a judge's condition waits for is that value in seven of ten.
                    let gate = (self.roles.gates.get(b))
                        .filter(|_| self.steer != Steer::Not)
                        .filter(|_| d.rng.chance(70));
                    let value = match (refusal, gate) {
                        (Some(refusal), _) if d.rng.chance(90) => {
                            values.iter().find(|v| *v != refusal).unwrap_or(refusal).clone()
                        }
                        (Some(refusal), _) => refusal.clone(),
                        (None, Some(gate)) => gate.clone(),
                        (None, None) if values.len() == 1 || d.rng.chance(55) => values[0].clone(),
                        (None, None) => values[1 + d.rng.below(values.len() - 1)].clone(),
                    };
                    handed_in.push(handed(b, cell, true, Some(value)));
                }
            } else if opinion(&d.flow, b, i) {
                if d.rng.chance(35) {
                    handed_in.push(handed(b, cell, true, None));
                }
            } else if d.rng.chance(80) {
                let matters = d.rng.chance(70);
                let mut h = handed(b, cell, matters, None);
                // A block a test of content reads is at times handed in holding nothing.
                if self.roles.contents.contains(b) && d.rng.chance(30) {
                    h.content = String::new();
                }
                handed_in.push(h);
            }
        }
        alone(&d.flow, &mut handed_in);
        handed_in
    }

    /// What an act of the engine's own hands in.
    fn act(&mut self, d: &mut Driver, i: usize, cell: &str) -> Came {
        // The check fails the first time where the history is steered so.
        let fail = i == self.roles.check
            && !self.check_failed
            && matches!(
                self.steer,
                Steer::FailCheck | Steer::FailCheckAnswered | Steer::FailCheckLeft
            );
        if fail {
            self.check_failed = true;
        }
        let outputs = d.flow.flow.cells()[i].outputs.clone();
        let mut failed = false;
        let mut handed_in = Vec::new();
        for b in &outputs {
            let Some(values) = d.flow.blocks[b].values.clone() else {
                continue;
            };
            let value = match self.roles.passing.get(b) {
                Some(pass) if !fail && d.rng.chance(60) => pass.clone(),
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
        if i == self.roles.check {
            passes_at(&self.branch, !failed);
        }
        Came::hands_in(handed_in)
    }
}

/// Of what a return hands in, each value that stands with nothing but some blocks (`with_nothing_but`), by its choice,
/// with those blocks.
fn standing_alone(flow: &Compiled, handed_in: &[Handed]) -> Vec<(BlockId, Vec<BlockId>)> {
    (handed_in.iter())
        .filter_map(|h| {
            let only = flow.blocks.get(&h.block)?.with_nothing_but.get(h.value.as_ref()?)?;
            Some((h.block.clone(), only.clone()))
        })
        .collect()
}

/// Keeps of what a return hands in, beside a value that stands with nothing but some blocks, those blocks alone, as an
/// agent that does as its task says hands it in.
fn alone(flow: &Compiled, handed_in: &mut Vec<Handed>) {
    let alone = standing_alone(flow, handed_in);
    handed_in.retain(|h| (alone.iter()).all(|(choice, only)| h.block == *choice || only.contains(&h.block)));
}

/// Whether a return of the cell at this place that hands these in may ask a question beside them: each value among
/// them that stands with nothing but some blocks has the cell's questions among them.
fn may_ask(flow: &Compiled, i: usize, handed_in: &[Handed]) -> bool {
    let own = engine::asked::questions_of(flow, &flow.cells[i].id);
    (standing_alone(flow, handed_in).iter()).all(|(_, only)| own.is_some_and(|q| only.contains(q)))
}

/// A commit on the work's branch, which moves its head.
fn commit(branch: &Shared) {
    let mut b = branch.get();
    b.head += 1;
    branch.set(b);
}

/// Whether the round under way of the run was run by what an act of the engine's after its cell wrote: what its return
/// answers by what runs the act again, a commit among it.
fn answers_an_act(d: &Driver, run: RunId, i: usize) -> bool {
    let Some(told) = decide::told(&d.flow.flow, &d.work.records, run) else {
        return false;
    };
    let acts = (d.flow.cells.iter().zip(d.flow.flow.cells())).skip(i + 1);
    let written = |b: &BlockId| (acts.clone()).any(|(spec, act)| spec.kind == Kind::Engine && act.outputs.contains(b));
    told.ran_by.iter().any(written)
}

/// The dialogues in which the round under way of the run owes the next word as the maker, woken by a point of the other
/// side (design document `data-and-context`, 41).
fn points(d: &Driver, run: RunId) -> Vec<BlockId> {
    decide::told(&d.flow.flow, &d.work.records, run).map_or_else(Vec::new, |told| told.points)
}

/// The next word a round of the cell at this place owes, handed in beside what it drew in `chance` of a hundred: of a
/// maker woken by a point, its answer in the dialogue; of a judge that fails the work, its points. Where it is left out
/// the engine sends the return back (design document `data-and-context`, 41, 42).
fn next_word(d: &mut Driver, run: RunId, i: usize, cell: &str, handed_in: &mut Vec<Handed>, chance: u64) {
    let mut owed = points(d, run);
    for (verdict, of) in &d.flow.flow.cells()[i].verdicts {
        let fails = (handed_in.iter()).any(|h| &h.block == verdict && h.value.as_deref() == Some(of.fails.as_str()));
        if fails {
            owed.push(of.dialogue.clone());
        }
    }
    for b in owed {
        if !handed_in.iter().any(|h| h.block == b) && d.rng.chance(chance) {
            handed_in.push(handed(&b, cell, true, None));
        }
    }
}

/// The check, whose round ends now, passed the work's head as the branch holds it, or failed it.
fn passes_at(branch: &Shared, passed: bool) {
    let mut b = branch.get();
    b.passed = passed.then_some(b.head);
    branch.set(b);
}

impl Part for Random {
    /// The engineer's histories fail no round: what a failed round does is the engine's, checked by the histories of
    /// the small flows.
    fn fails(&mut self, _: &mut Driver, _: bool, _: bool) -> bool {
        false
    }

    fn failure(&mut self, d: &mut Driver, run: RunId, cell: &str) -> Option<(Failure, String)> {
        gate(d, &self.roles, &self.branch, run, place(&d.flow, cell))
    }

    /// What is drawn for the round, the branch moved where an agent's round on a line that may change the work commits,
    /// in two of five, and read as the round ends.
    fn returns(&mut self, d: &mut Driver, run: RunId, cell: &str) -> Came {
        let i = place(&d.flow, cell);
        let mut came = self.drawn(d, run, cell);
        let spec = &d.flow.cells[i];
        // The run the steering has answer the failed check with nothing committed commits nothing.
        let left = self.steer == Steer::FailCheckLeft && self.answered == Some(run);
        if spec.kind == Kind::Agent && !spec.reads_only && !left && d.rng.chance(40) {
            commit(&self.branch);
        }
        round_end(d, &self.roles, self.branch.get(), i, &mut came);
        came
    }

    fn acts(&mut self, d: &mut Driver) -> Event {
        self.person(d)
    }
}

impl Random {
    /// What the round under way of this run of this cell returns, as drawn.
    fn drawn(&mut self, d: &mut Driver, run: RunId, cell: &str) -> Came {
        let i = place(&d.flow, cell);
        if d.flow.cells[i].kind == Kind::Engine {
            return self.act(d, i, cell);
        }
        if self.refuse_next && self.roles.judges.iter().any(|(j, _)| *j == i) {
            self.refuse_next = false;
            let mut handed_in = self.as_it_stands(d, i, cell, true);
            // A judge that refuses brings its points.
            next_word(d, run, i, cell, &mut handed_in, 100);
            return hands_in(d, run, i, handed_in);
        }
        let answering = matches!(self.steer, Steer::FailCheckAnswered | Steer::FailCheckLeft);
        if answering && self.check_failed && self.answered.is_none() {
            let check = &d.flow.flow.cells()[self.roles.check].outputs;
            let told = d.flow.flow.cells()[i].runs_on.iter().any(|b| check.contains(b));
            if told {
                self.answered = Some(run);
                if self.steer == Steer::FailCheckAnswered {
                    commit(&self.branch);
                }
                let handed_in = self.as_it_stands(d, i, cell, false);
                return hands_in(d, run, i, handed_in);
            }
        }
        if self.sloppy && d.rng.chance(50) {
            return Came::asks(vec![]);
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
                Came::asks(asks)
            }
            10..=13 => Came::asks(vec![]),
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
                hands_in(d, run, i, handed_in)
            }
            _ => {
                let mut handed_in = self.valid(d, i, cell);
                // Nine agents in ten owe nothing they leave out.
                next_word(d, run, i, cell, &mut handed_in, 90);
                let first = !d.work.records.questions.iter().any(|q| !q.engine);
                let force = self.steer == Steer::FailCheckLeft && first;
                // A value that stands with nothing but some blocks is handed in with no question beside it.
                let questions = match may_ask(&d.flow, i, &handed_in) {
                    true => self.questions(d, i, cell, force),
                    false => Vec::new(),
                };
                // A return whose question waits hands in as much of what its cell must write as the draw gives (decision
                // a-return-hands-in-what-its-cell-must-every-time): each such block is left out in one of two.
                if questions
                    .iter()
                    .any(|q| matches!(q, Question::Asked { waits: true, .. }))
                {
                    let must = d.flow.cells[i].must_write.clone();
                    handed_in.retain(|h| !must.contains(&h.block) || d.rng.chance(50));
                }
                let mut came = hands_in(d, run, i, handed_in);
                came.said.questions = questions;
                came
            }
        }
    }

    /// What the agent of the cell at this place writes in the record of the questions beside its hand-in, where an
    /// answer runs its cell: a question of its own in five of a hundred, which waits in one of two, and always one
    /// that does not wait where `force` says so; one of its own that stands asked anew, as it was, in two of a
    /// hundred; one withdrawn in two of a hundred; nothing otherwise.
    fn questions(&self, d: &mut Driver, i: usize, cell: &str, force: bool) -> Vec<Question> {
        let spec = &d.flow.cells[i];
        if !(spec.watches.iter()).any(|t| t.of_source(&d.flow, flow::ANSWER).is_some()) {
            return Vec::new();
        }
        let own: Vec<String> = (decide::questions::questions(&d.work.records).into_iter())
            .filter(|(_, q)| q.stands() && q.latest.cell.as_str() == cell && !q.latest.engine)
            .map(|(name, _)| name.to_string())
            .collect();
        let fresh = format!("{cell}_{}", d.work.records.questions.len());
        let drawn = if force { None } else { Some(d.rng.below(100)) };
        let (name, waits) = match drawn {
            None => (fresh, false),
            Some(0..=4) => (fresh, d.rng.chance(50)),
            Some(5..=6) if !own.is_empty() => (own[d.rng.below(own.len())].clone(), false),
            Some(7..=8) if !own.is_empty() => {
                let name = own[d.rng.below(own.len())].clone();
                let why = "Settled.".to_string();
                return vec![Question::Withdrawn { name, why }];
            }
            _ => return Vec::new(),
        };
        let way = |does: &str, follows: &str| Way {
            does: does.to_string(),
            follows: follows.to_string(),
        };
        let ask = format!("Which way for {name}?");
        vec![Question::Asked {
            name,
            waits,
            tree: None,
            recommends: Some(1),
            ask,
            options: vec![
                way("the short way", "less to read"),
                way("the long way", "more to read"),
            ],
            about: None,
            reasons: Default::default(),
        }]
    }

    /// What a person or the outside does now, a person's commit on the branch among it.
    fn person(&mut self, d: &mut Driver) -> Event {
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
        // The owner's start is written once, as the work begins; what the engine writes itself, of the questions, the
        // answers and the owner's words, is written by no one outside; and the work's head is written as the carriers
        // read the branch, which a person moves by committing.
        let engines = [START, "asked", flow::ANSWER, flow::WORDS, HEAD];
        let outside: Vec<&BlockId> = (blocks.iter())
            .filter(|(_, b)| b.writers.outside.as_deref().is_some_and(|s| !engines.contains(&s)))
            .map(|(id, _)| id)
            .collect();
        // The lines the owner speaks to.
        let mut lines: Vec<String> = (d.flow.cells.iter())
            .filter(|c| (c.watches.iter()).any(|t| t.of_source(&d.flow, flow::WORDS).is_some()))
            .filter_map(|c| c.line.clone())
            .collect();
        lines.dedup();
        // Around the check: a person pauses the work while the check's round is under way, and while the pause holds
        // the merge back after the check passed, writes for a judge or lets the work on, so that a judge looks again
        // after the check passed.
        let paused = decide::paused(&d.work.records);
        let check = &d.flow.flow.cells()[self.roles.check].id;
        let checking = (d.held.iter()).any(|e| matches!(e, Effect::Round { cell, .. } if cell == check));
        if checking && !paused {
            return Event::Paused {
                by: Pauser::Owner,
                why: None,
            };
        }
        if paused && passed_before_merge(d, &self.roles) {
            let for_judges: Vec<&BlockId> = (self.roles.judges.iter())
                .flat_map(|(j, _)| &d.flow.flow.cells()[*j].runs_on)
                .filter(|b| blocks[*b].writers.people)
                .collect();
            let verdicts: Vec<&BlockId> = self.roles.refusals.keys().collect();
            match d.rng.below(10) {
                0..=3 => {
                    if let Some(b) = pick(&mut d.rng, for_judges) {
                        return wrote(&b, Outsider::Person, true, None);
                    }
                }
                4..=5 => {
                    if let Some(b) = pick(&mut d.rng, verdicts) {
                        let refusal = self.roles.refusals[&b].clone();
                        return wrote(&b, Outsider::Person, true, Some(refusal));
                    }
                }
                6..=8 => return Event::Resumed,
                _ => {}
            }
        }
        // The owner answers a question that stands with no answer in one act of two, by either option, so that what
        // waits for every question to be answered, the check among it, is not held back long.
        let unanswered: Vec<(String, u64)> = (decide::questions::questions(&d.work.records).into_iter())
            .filter(|(_, q)| q.unanswered())
            .map(|(name, q)| (name.to_string(), latest_number(&d.flow, &d.work.records, &q)))
            .collect();
        // A steered history's owner answers each as soon as it can, so that the check is not held back by one.
        let quiet = self.steer != Steer::Not;
        if !unanswered.is_empty() && (quiet || d.rng.chance(50)) {
            let (question, version) = unanswered[d.rng.below(unanswered.len())].clone();
            let option = 1 + d.rng.below(2) as u64;
            return Event::Answer {
                question,
                version,
                option,
            };
        }
        // A steered history's owner leaves the work as it is, but for what lets it go on: no cancelling, no pausing
        // (the steering pauses it where it must), no prose a person writes, no commit, no word to a line and no remark
        // on a question, and a verdict it writes is not a refusal in nine of ten. The steering has to reach the check
        // in the random steps, which a cancelled or reworked work seldom does, and which a work held for a person's
        // verdict never does.
        let roll = match d.rng.below(100) {
            0 | 22..=36 | 77..=89 if quiet => 100,
            67..=76 if quiet && !paused => 100,
            n => n,
        };
        let event = match roll {
            0 => Some(Event::Cancelled { why: None }),
            1..=21 => pick(&mut d.rng, people_choices).map(|b| {
                let values = blocks[&b].values.clone().expect("a choice");
                let value = match self.roles.refusals.get(&b) {
                    Some(refusal) if quiet && d.rng.chance(90) => {
                        values.iter().find(|v| *v != refusal).unwrap_or(refusal).clone()
                    }
                    _ => values[d.rng.below(values.len())].clone(),
                };
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
                // What the outside writes is approved as not mattering only as a change of what the block said: a
                // version so written of a block that says nothing yet would say nothing, and run no cell (design
                // document `data-and-context`, 9).
                None => {
                    let matters = d.rng.chance(50) || !says_something(&d.flow.flow, &d.work.records, &b);
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
            77..=81 => (!lines.is_empty()).then(|| says(&lines[d.rng.below(lines.len())])),
            82..=84 => {
                let standing: Vec<(String, CellId)> = (decide::questions::questions(&d.work.records).into_iter())
                    .filter(|(_, q)| q.stands())
                    .map(|(name, q)| (name.to_string(), q.latest.cell.clone()))
                    .collect();
                (!standing.is_empty()).then(|| {
                    let (question, cell) = &standing[d.rng.below(standing.len())];
                    remarks(question, cell)
                })
            }
            67..=76 => Some(if paused {
                Event::Resumed
            } else {
                Event::Paused {
                    by: Pauser::Owner,
                    why: None,
                }
            }),
            // A person commits on the work's branch, outside any round: nothing is written until a carrier reads it.
            85..=89 => {
                let mut b = self.branch.get();
                b.head += 1;
                self.branch.set(b);
                None
            }
            // The owner answers the latest version of a question that stands, answered or not, by either option.
            90..=95 => {
                let standing: Vec<(String, u64)> = (decide::questions::questions(&d.work.records).into_iter())
                    .filter(|(_, q)| q.stands())
                    .map(|(name, q)| (name.to_string(), latest_number(&d.flow, &d.work.records, &q)))
                    .collect();
                (!standing.is_empty()).then(|| {
                    let (question, version) = standing[d.rng.below(standing.len())].clone();
                    let option = 1 + d.rng.below(2) as u64;
                    Event::Answer {
                        question,
                        version,
                        option,
                    }
                })
            }
            _ => None,
        };
        event.unwrap_or(Event::Look)
    }
}

/// The content continuation's rounds: each hands in what its cell must write to go forward, and says nothing more.
struct Content {
    /// The value each choice is written with.
    choices: BTreeMap<BlockId, String>,
    roles: Roles,
    /// The work's branch, which a round of the continuation commits to only as it fixes what it was told, and which the
    /// carriers read as before.
    branch: Shared,
    /// Whether a person commits on the branch, once, as the check's round is under way: the check then reads a head its
    /// review has not dealt with, and fails in passing.
    commit_at_check: bool,
    /// Whether a person commits on the branch, once, as the merge's round is under way: the merge then reads a head
    /// moved since the check passed, and fails in passing.
    commit_at_merge: bool,
}

impl Content {
    /// The values of the choices for the continuation from the work as it is: each verdict a value other than its
    /// refusal, each choice a precondition of kind `all` reads the value it looks for, every other choice its value
    /// now, or the last of its declared values where it was never written.
    fn new(d: &Driver, roles: &Roles, branch: Shared) -> Self {
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
                    (reading.value(b).map(str::to_string))
                        .unwrap_or_else(|| values.last().expect("a choice has values").clone())
                };
                Some((b.clone(), value))
            })
            .collect();
        Self {
            choices,
            roles: roles.clone(),
            branch,
            commit_at_check: false,
            commit_at_merge: false,
        }
    }
}

impl Part for Content {
    fn fails(&mut self, _: &mut Driver, _: bool, _: bool) -> bool {
        false
    }

    fn failure(&mut self, d: &mut Driver, run: RunId, cell: &str) -> Option<(Failure, String)> {
        let i = place(&d.flow, cell);
        let commits = (i == self.roles.check && std::mem::take(&mut self.commit_at_check))
            || (i == self.roles.last && std::mem::take(&mut self.commit_at_merge));
        if commits {
            commit(&self.branch);
        }
        gate(d, &self.roles, &self.branch, run, i)
    }

    /// What its cell must write to go forward, the branch read as the round ends; the check passes. An agent's round on
    /// a line that may change the work commits its fix where it fixes what it was told: an opinion addressed to its
    /// cell, the owner's words among them, or what an act after its cell wrote.
    fn returns(&mut self, d: &mut Driver, run: RunId, cell: &str) -> Came {
        let i = place(&d.flow, cell);
        let mut came = self.forward(d, run, cell);
        if i == self.roles.check {
            passes_at(&self.branch, true);
        }
        let spec = &d.flow.cells[i];
        if spec.kind == Kind::Agent && !spec.reads_only && (answering(d, run, i) || answers_an_act(d, run, i)) {
            commit(&self.branch);
        }
        round_end(d, &self.roles, self.branch.get(), i, &mut came);
        came
    }

    fn acts(&mut self, _: &mut Driver) -> Event {
        unreachable!("in the content continuation nobody acts at random")
    }
}

impl Content {
    /// What the round of the run of the cell hands in to go forward.
    fn forward(&mut self, d: &mut Driver, run: RunId, cell: &str) -> Came {
        let i = place(&d.flow, cell);
        let must = &d.flow.cells[i].must_write;
        // An act of the engine's says every write matters, as the real acts do; a run for an opinion addressed to its
        // cell fixes what it was told, and says its writes matter.
        let act = d.flow.cells[i].kind == Kind::Engine;
        let answering = answering(d, run, i);
        let mut handed_in: Vec<Handed> = (d.flow.flow.cells()[i].outputs.iter())
            .filter_map(|b| {
                if d.flow.blocks[b].values.is_some() {
                    Some(handed(b, cell, true, Some(self.choices[b].clone())))
                } else if opinion(&d.flow, b, i) && must.contains(b) {
                    // An opinion its cell must write each time is handed in as it stands.
                    Some(handed(b, cell, false, None))
                } else if opinion(&d.flow, b, i) {
                    None
                } else {
                    let matters = act || answering || !says_something(&d.flow.flow, &d.work.records, b);
                    Some(handed(b, cell, matters, None))
                }
            })
            .collect();
        // A maker woken by a point answers it in the dialogue (design document `data-and-context`, 41).
        for b in points(d, run) {
            if !handed_in.iter().any(|h| h.block == b) {
                handed_in.push(handed(&b, cell, true, None));
            }
        }
        hands_in(d, run, i, handed_in)
    }
}

/// Whether the run of the cell at this place runs for an opinion addressed to its cell, and so fixes what it was told.
fn answering(d: &Driver, run: RunId, i: usize) -> bool {
    let started = (d.work.records.started.iter())
        .find(|s| s.run == run)
        .expect("the run started");
    (started.because.iter()).any(|c| matches!(c, Cause::Changed { block, .. } if addressed(&d.flow, block, i)))
}

/// How many cells write the block: those whose runs hand it in, or, of the work's head, which a source outside the work
/// writes, the agent cells on lines that may change the work, whose rounds commit it.
fn writers(d: &Driver, b: &BlockId) -> u64 {
    let block = &d.flow.blocks[b];
    if block.writers.outside.as_deref() == Some(HEAD) {
        let commits = |spec: &&flow::CellSpec| spec.kind == Kind::Agent && !spec.reads_only;
        return d.flow.cells.iter().filter(commits).count() as u64;
    }
    block.writers.cells.len() as u64
}

/// How many steps the content continuation may take before the work merges.
///
/// In the continuation every round hands in what finishes its run, and nothing is written while a round is under way,
/// so a run begun in it has one round and is never abandoned. A run that finishes saw what it writes over, its run
/// being stale otherwise. Every choice is written with one value, the one the owner or the outside also writes where a
/// precondition waits on it, so it changes its meaning at most once, and the conditions, which read choices, come out
/// the same throughout. Prose changes its meaning at most `2 + 2 w` times, `w` its writing cells: its first write; each
/// writer's runs for an opinion addressed to it, which fix and so matter, at most two (one for the opinions that had
/// changed when the continuation began, one for the owner's word where the work waits on what only that cell writes:
/// nothing else writes an opinion in the continuation, and a must-write opinion is handed in as it stands); and one
/// write by the owner or the outside. Acts' writes matter, but every act's prose here is an opinion or read by no cell,
/// and the acts run as often as any cell. So after one run for whatever had changed when the continuation began, a cell
/// runs again only for a change of one of its inputs, and the run open as the continuation began may be abandoned
/// besides: at most `2 + sum of (2 + 2 w)` over its inputs, each taking at most `PER_RUN` steps. The work's head, which
/// a run commits where it fixes what it was told, counts as its writers the cells whose rounds may commit. Besides
/// those, the driver does once what it held as the continuation began, the owner resumes the work once, and the owner
/// or the outside writes each block a named test reads at most once. A block a named test reads that only cells write
/// is not written by the owner but asked for: the owner writes once in an opinion for each cell that writes it, which
/// runs that cell once more. That write and that run are counted apart: one step, and `PER_RUN` steps, for each writer
/// of each such block (the run is also among those of its cell; counted twice, the bound only grows). The owner answers
/// each question that stands with no answer as the continuation begins, which no round of it asks: one step, and
/// `PER_RUN` steps for the run of its cell an answer it waited for begins. The limits of runs do not enter: the owner
/// lets every cell go on.
fn content_bound(d: &Driver) -> u64 {
    let flow = &d.flow.flow;
    let changes = |b: &BlockId| 2 + 2 * writers(d, b);
    let runs: u64 = (flow.cells().iter())
        .map(|c| 2 + c.runs_on.iter().map(changes).sum::<u64>())
        .sum();
    let tested: BTreeSet<&BlockId> = flow.tests().values().flat_map(|t| &t.reads).collect();
    let asked: u64 = (tested.iter())
        .map(|b| &d.flow.blocks[*b].writers)
        .filter(|w| !w.people && w.outside.is_none())
        .map(|w| w.cells.len() as u64)
        .sum();
    let questions = (decide::questions::questions(&d.work.records).values())
        .filter(|q| q.unanswered())
        .count() as u64;
    d.held.len() as u64 + PER_RUN * runs + tested.len() as u64 + 1 + (asked + questions) * (1 + PER_RUN)
}

/// The owner's few words to a cell, where the key reads a block only cells write: to the line of a cell that writes it
/// that the owner's words run and that applies, so that the cell runs again and writes the block anew; where that cell
/// is none, to the line of the nearest cell before it that is one and writes a block the writer runs on, or commits the
/// work's head the writer runs on, whose run, answering the owner, writes it anew, so that the writer runs again. A
/// cell spoken to that writes nothing the writer runs on would run again and leave the work where it was. Where no cell
/// the owner's words run has run yet, so that no line has reached one, the owner writes what the item asks for, the
/// first block only people write that runs a cell, which begins the lines again; none where no such block is either, or
/// where such a cell ran and the engine still gives the words to none that would move the work.
fn ask_a_cell(d: &Driver, key: &Condition) -> Option<Event> {
    let only_cells = |b: &BlockId| {
        let w = &d.flow.blocks[b].writers;
        !w.people && w.outside.is_none()
    };
    let cells = d.flow.flow.cells();
    // The line of the cell at `j`, where the owner's words to it reach that cell, as the engine chooses.
    let spoken_to = |j: usize| {
        let spec = &d.flow.cells[j];
        let line = spec.line.clone()?;
        let reached = spoken::receiver(&d.flow, &d.work, &To::Line(line.clone()));
        (reached.as_ref() == Ok(&spec.id)).then_some(line)
    };
    // Whether the cell at `j` runs the writer at `w` again by what it writes: it is the writer, or writes a block the
    // writer runs on.
    let head = (d.flow.blocks.iter()).find(|(_, b)| b.writers.outside.as_deref() == Some(HEAD));
    let commits_to = |j: usize, w: usize| {
        let spec = &d.flow.cells[j];
        spec.kind == Kind::Agent && !spec.reads_only && head.is_some_and(|(h, _)| cells[w].runs_on.contains(h))
    };
    let leads_to = |j: usize, w: usize| {
        j == w || cells[j].outputs.iter().any(|b| cells[w].runs_on.contains(b)) || commits_to(j, w)
    };
    (key_reads(&d.flow, key).into_iter().filter(|b| only_cells(b)))
        .flat_map(|b| &d.flow.blocks[b].writers.cells)
        .filter_map(|c| d.flow.flow.place(c))
        .find_map(|w| (0..=w).rev().filter(|&j| leads_to(j, w)).find_map(spoken_to))
        .map(|line| says(&line))
        .or_else(|| {
            let spoken_to =
                |c: &CellId| (d.flow.cells.iter()).any(|s| s.id == *c && spoken::words_of(&d.flow, &s.id).is_some());
            if d.work.records.started.iter().any(|s| spoken_to(&s.cell)) {
                return None;
            }
            let only_people = |b: &&BlockId| {
                let block = &d.flow.blocks[*b];
                let w = &block.writers;
                w.people && w.cells.is_empty() && w.outside.is_none() && block.values.is_none()
            };
            (cells.iter().flat_map(|c| &c.runs_on))
                .find(only_people)
                .map(|b| wrote(b, Outsider::Person, true, None))
        })
}

/// The owner's few words to the line.
fn says(line: &str) -> Event {
    Event::Said {
        to: To::Line(line.to_string()),
        words: format!("{line}, do it again"),
    }
}

/// The owner's few words about the question, to the cell whose question it is.
fn remarks(question: &str, cell: &CellId) -> Event {
    Event::Said {
        to: To::Cell(cell.clone()),
        words: format!("About {question}: say more"),
    }
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
    /// Whether the merge's finish was judged.
    merged: bool,
    /// The work's branch, as its parts move it.
    branch: Shared,
    /// Whether, in the content continuation, a person commits as the check's round is under way, and as the merge's
    /// is: the two ways a work's head moves past what judged it, which the random steps take in about one history in
    /// five hundred each. So the check meets a head the review has not dealt with where the head moved after the
    /// review.
    commits: (bool, bool),
}

impl History {
    fn breach(&mut self, property: &'static str, what: String) {
        self.d.note(format!("BROKEN {property}: {what}"));
        self.verdict.broken.entry(property).or_insert(what);
    }

    fn reach(&mut self, what: String) {
        self.verdict.reached.insert(what);
    }

    /// After every step: checks what the driver checks of the engine, judges each round of the merge that began since,
    /// and counts what the work reached.
    fn observe(&mut self) {
        if let Some(why) = lacking_a_must_write(&self.d.flow, &self.d.work.records) {
            self.breach(MUST_WRITE_WRITTEN, why);
        }
        // The engine's own properties, on the real flow: the driver's checks.
        self.d.check();
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
        let merged = {
            let records = &self.d.work.records;
            let last = &self.d.flow.flow.cells()[self.roles.last].id;
            (records.finished.iter())
                .find(|f| records.started.iter().any(|s| s.run == f.run && &s.cell == last))
                .map(|f| f.at)
        };
        if let Some(at) = merged.filter(|_| !self.merged) {
            self.merged = true;
            self.judge_merged(at);
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
            if !cell.applies.is_empty() && !reading.applies(i) {
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
        let mut stalled = None;
        let decision = decide(flow, records);
        match &decision {
            Decision::Wait { on, .. } if on.iter().any(|w| matches!(w, Waited::Limit { .. })) => {
                reached.push(CELL_LIMIT.to_string());
            }
            Decision::Ended(_) => self.ended = true,
            _ => {}
        }
        // Stalled on a key that reads what the check wrote as it failed: only the owner speaking to a cell would move
        // it. Where the work stands is read from the records alone, never from what the deciding code calls it. A
        // work stalled before the check ran, which stands at the first gate that does not hold, the merge's where the
        // check's own hold, is not at the check's.
        let rest = whole::Reading::new(flow, records).rest(flow);
        // And the decision says the work is stalled where, and only where, the records read it so: a stall it calls a
        // wait is one the owner is never shown.
        let reads_stalled = matches!(rest, whole::Rest::Stalled { .. });
        if reads_stalled != matches!(decision, Decision::Stalled { .. }) {
            stalled = Some(format!("the records read {rest:?}, and the decision is {decision:?}"));
        }
        if let whole::Rest::Stalled { at, on } = rest {
            let check = &flow.cells()[self.roles.check].id;
            let failed: Vec<&BlockId> = (self.roles.check_passes(&self.d.flow))
                .filter(|(b, pass)| {
                    (reading.record(check)).is_some_and(|f| {
                        f.writes
                            .iter()
                            .any(|w| &w.block == *b && w.value.as_ref() != Some(*pass))
                    })
                })
                .map(|(b, _)| b)
                .collect();
            if on
                .iter()
                .any(|key| key_reads(&self.d.flow, key).iter().any(|b| failed.contains(b)))
            {
                stalled = Some(format!(
                    "the records read the work stalled at {at} on {on:?}, which only the check writes"
                ));
            }
        }
        for r in reached {
            self.reach(r);
        }
        if let Some(what) = stalled {
            self.breach(CHECK_NEVER_STALLS, what);
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
            }
        }
        for (i, cell) in flow.cells()[..self.roles.last].iter().enumerate() {
            if !r.applies(i) {
                continue;
            }
            for b in &cell.runs_on {
                let new: Vec<Seq> = r.new_to(&cell.id, b).iter().map(|v| v.at).collect();
                if !new.is_empty() {
                    broken.push((
                        MERGE_ON_AN_OLD_CHECK,
                        format!(
                            "the merge's round at {at:?} began while {b} at {new:?} was new to {}",
                            cell.id
                        ),
                    ));
                }
            }
        }
        for (property, what) in broken {
            self.breach(property, what);
        }
    }

    /// Property 6, as the run of the merge finishes at this place, merging: the branch is at the head the check last
    /// passed, and each judge of the head that applies dealt with a version that says it.
    fn judge_merged(&mut self, at: Seq) {
        let flow = &self.d.flow.flow;
        let r = Reading::new(flow, &self.d.work.records, Some(at));
        let branch = self.branch.get();
        let mut broken = Vec::new();
        let mut seen = false;
        if let (Some(head), Some(now)) = (&self.roles.head, branch.says()) {
            seen = true;
            if branch.passed != Some(branch.head) {
                broken.push(format!(
                    "the merge at {at:?} merged the branch at {now}, and the check last passed {:?}",
                    branch.passed
                ));
            }
            for &j in self.roles.head_judges.iter().filter(|&&j| r.applies(j)) {
                let judge = &flow.cells()[j].id;
                let said = |v: &Seq| self.d.work.contents.get(&(head.clone(), *v)).map(String::as_str);
                if !r.dealt_with(judge, head).iter().any(|v| said(v) == Some(now.as_str())) {
                    broken.push(format!(
                        "the merge at {at:?} merged {now}, of which {judge} dealt with no version"
                    ));
                }
            }
        }
        if seen && broken.is_empty() {
            self.reach(MERGED_A_REVIEWED_HEAD.to_string());
        }
        for what in broken {
            self.breach(MERGE_UNSEEN, what);
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
        if records.questions.iter().any(|q| !q.engine) {
            reached.push(ASKED_IN_THE_RECORD.to_string());
        }
        // A run of a cell before the check, begun for what the check wrote while a question the owner had answered
        // stood: the case an answered question once let through, which property 7 holds.
        let found = &cells[self.roles.check].outputs;
        let before_the_check = |c: &CellId| flow.flow.place(c).is_some_and(|j| j < self.roles.check);
        let after_an_answer = (records.started.iter())
            .filter(|s| before_the_check(&s.cell))
            .filter(|s| (s.because.iter()).any(|c| matches!(c, Cause::Changed { block, .. } if found.contains(block))))
            .any(|s| answered_stood(&records.before(s.at)));
        if after_an_answer {
            reached.push(RED_AFTER_AN_ANSWER.to_string());
        }
        if self.d.seen.limits > 0 {
            reached.push(RUN_LIMIT.to_string());
        }
        if self.d.seen.lacking > 0 {
            reached.push(LACKING_SENT_BACK.to_string());
        }
        // A run that finished lacking a block its cell must write, which asked no call and asked a question its cell
        // waits for: the engine took it, as the decision allows.
        let waiting_ask = |run: RunId| {
            (records.questions.iter()).any(|q| q.run == run && !q.engine && q.waits && !q.withdrawn)
                && !records.called.iter().any(|c| c.run == run)
        };
        let lacking_taken = records.finished.iter().any(|f| {
            let cell = &records.started.iter().find(|s| s.run == f.run).expect("a run").cell;
            let spec = flow
                .cells
                .iter()
                .find(|c| &c.id == cell)
                .expect("the cell is in the flow");
            waiting_ask(f.run) && spec.must_write.iter().any(|b| !f.writes.iter().any(|w| &w.block == b))
        });
        if lacking_taken {
            reached.push(LACKING_TAKEN_WITH_A_QUESTION.to_string());
        }
        if self.d.seen.stops > 0 {
            reached.push(ENGINE_STOPPED.to_string());
        }
        if self.branch.get().gated > 0 {
            reached.push(CHECK_GATED.to_string());
        }
        if self.branch.get().moved > 0 {
            reached.push(MERGE_ON_A_MOVED_HEAD.to_string());
        }
        for r in reached {
            self.reach(r);
        }
    }

    /// What the owner or the outside does in the content continuation where nothing is to be done: answers a question
    /// that stands with no answer; of a stalled work, asks the cell that writes what it waits for to write it again,
    /// which it says; resumes the work; lets the cell it
    /// waits at go on at its limit, or makes a precondition it waits on hold, or asks the cell that writes what it
    /// reads to write it again, which it says; none where none of these moves it on.
    fn resolve(&self, decision: &Decision) -> Option<(Event, bool)> {
        // The owner answers a question that stands with no answer, its latest version, by the option it recommends.
        let unanswered = (decide::questions::questions(&self.d.work.records).into_iter())
            .find(|(_, q)| q.unanswered())
            .map(|(name, q)| (name.to_string(), latest_number(&self.d.flow, &self.d.work.records, &q)));
        if let Some((question, version)) = unanswered {
            let answer = Event::Answer {
                question,
                version,
                option: 1,
            };
            return Some((answer, false));
        }
        if let Decision::Stalled { on, .. } = decision
            && let Some(asked) = on.iter().find_map(|key| ask_a_cell(&self.d, key))
        {
            return Some((asked, true));
        }
        if decide::paused(&self.d.work.records) {
            return Some((Event::Resumed, false));
        }
        let Decision::Wait { at, on } = decision else {
            return None;
        };
        on.iter().find_map(|w| match w {
            Waited::Limit { .. } | Waited::Calls { .. } => Some((Event::LetGo { cell: at.clone() }, false)),
            Waited::Precondition { on, .. } => {
                (make_hold(&self.d, on).map(|e| (e, false))).or_else(|| ask_a_cell(&self.d, on).map(|e| (e, true)))
            }
            Waited::Call { .. } | Waited::Slot | Waited::Answer { .. } | Waited::Outside { .. } => None,
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
        let mut part = Content::new(&self.d, &self.roles, self.branch.clone());
        (part.commit_at_check, part.commit_at_merge) = self.commits;
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
                self.d.carry_out(effect, &mut part, true);
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
        // The bound of a cell's runs `content_bound` rests on.
        let most = |cell: &CellId| {
            let inputs = &flow.cells()[flow.place(cell).expect("a cell")].runs_on;
            2 + (inputs.iter()).map(|b| 2 + 2 * writers(&self.d, b)).sum::<u64>()
        };
        let over: Vec<String> = (self.verdict.runs.iter())
            .filter(|(cell, (_, nothing))| *nothing > most(cell))
            .map(|(cell, (_, nothing))| format!("{cell}: {nothing} of at most {}", most(cell)))
            .collect();
        if !over.is_empty() {
            self.breach(
                RUNS_FOR_NOTHING,
                format!("more runs for nothing than a cell may run: {over:?}"),
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
    // Whether the random steps end at a turn: none in two histories of three, a failed check in a sixth, a failed check
    // or a judge's refusal in a sixth.
    let early = rng.below(6);
    let mut steering = Rng::new(seed ^ 0x57EE_4ED0_C4EC_0001);
    let steer = match steering.below(10) {
        0..=2 => Steer::Refuse,
        3 => Steer::FailCheck,
        4..=5 => Steer::FailCheckAnswered,
        6 => Steer::FailCheckLeft,
        _ => Steer::Not,
    };
    let random = if steer == Steer::Not { random } else { RANDOM };
    let roles = Roles::new(flow);
    let branch: Shared = Rc::default();
    let mut part = Random {
        roles: roles.clone(),
        sloppy,
        steer,
        steering,
        stage: 0,
        paused_it: false,
        refuse_next: false,
        check_failed: false,
        briefed: false,
        answered: None,
        branch: branch.clone(),
    };
    let mut h = History {
        d: Driver::new(flow.clone(), rng, seed, true),
        roles,
        verdict: Verdict::default(),
        merges: 0,
        ended: false,
        merged: false,
        branch,
        commits: {
            let mut coins = Rng::new(seed ^ 0xC0_4417_5EED_0002);
            (coins.chance(10), coins.chance(10))
        },
    };
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        h.d.take(started(&h.d.flow));
        let first = (h.d.flow.flow.cells()[0].requires.first())
            .filter(|_| starts)
            .and_then(|key| make_hold(&h.d, key))
            .unwrap_or(Event::Look);
        h.d.take(first);
        h.observe();
        for _ in 0..random {
            if h.ended {
                break;
            }
            match part.steered(&h.d) {
                Some(event) => {
                    h.d.note(format!("steered ({:?})", part.steer));
                    h.d.take(event);
                }
                None => {
                    h.d.step(&mut part, true);
                }
            }
            h.observe();
            let turn = match part.steer {
                Steer::Not if early >= 4 => h.turning(early == 5),
                Steer::FailCheck if part.check_failed => h.turning(false),
                Steer::FailCheckAnswered => {
                    let answered =
                        (part.answered).is_some_and(|run| h.d.work.records.finished.iter().any(|f| f.run == run));
                    if answered { h.turning(false) } else { None }
                }
                // Once the round that answered with nothing committed has ended, taken or sent back.
                Steer::FailCheckLeft => {
                    let ended =
                        (part.answered).is_some_and(|run| h.d.work.records.returned.iter().any(|r| r.run == run));
                    if ended { h.turning(false) } else { None }
                }
                _ => None,
            };
            if let Some(turn) = turn {
                h.reach(turn.to_string());
                break;
            }
        }
        if h.d.work.records.cancelled().is_none() {
            h.content();
        }
        h.reached_at_the_end();
    }));
    if let Err(e) = result {
        h.breach(DRIVER, message(e.as_ref()));
    }
    *log = std::mem::take(&mut h.d.log);
    h.verdict
}

/// The message of a panic.
fn message(e: &(dyn std::any::Any + Send)) -> String {
    (e.downcast_ref::<String>().cloned())
        .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_string()))
        .unwrap_or_default()
}

/// A history that broke a property: its seed, the breach, and its steps.
type Case = (u64, String, Vec<String>);

/// Held while a test runs its histories: each silences the panic hook while its histories run and gives it back
/// before it fails, and two tests side by side would each take the other's.
static HOOK: Mutex<()> = Mutex::new(());

/// Runs `f` on each seed from `from` to below `to` on as many threads as the machine has cores, and gives the results
/// in seed order.
fn in_parallel<T: Send>(from: u64, to: u64, f: impl Fn(u64) -> T + Sync) -> Vec<T> {
    let next = std::sync::atomic::AtomicU64::new(from);
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let mut done: Vec<(u64, T)> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let seed = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if seed >= to {
                            return mine;
                        }
                        mine.push((seed, f(seed)));
                    }
                })
            })
            .collect();
        workers.into_iter().flat_map(|w| w.join().expect("a worker")).collect()
    });
    done.sort_by_key(|(seed, _)| *seed);
    done.into_iter().map(|(_, t)| t).collect()
}

/// Runs the histories, prints what they reached and counted, and fails if one breaks a property, or if none reached a
/// situation the properties are about. A panic anywhere in a history, raised while the hook is silenced, is kept as a
/// breach with its message, so that it is said when the test fails.
fn histories() {
    let _hook = HOOK.lock().unwrap_or_else(PoisonError::into_inner);
    let flow = engineer();
    let roles = Roles::new(&flow);
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let must = required(&flow, &roles);
    let mut reached: BTreeMap<String, u64> = must.iter().map(|r| (r.clone(), 0)).collect();
    let mut runs: BTreeMap<CellId, (u64, u64)> = BTreeMap::new();
    let mut broken: BTreeMap<&str, Vec<Case>> = BTreeMap::new();
    let mut continued = 0;
    let mut most = (0, 1);
    let mut ran = 0;
    while ran < SEEDS * BATCHES {
        let batch = ran..ran + SEEDS;
        let results = in_parallel(batch.start, batch.end, |seed| {
            let mut log = Vec::new();
            let verdict = match panic::catch_unwind(AssertUnwindSafe(|| one_history(&flow, seed, &mut log))) {
                Ok(verdict) => verdict,
                Err(e) => {
                    let mut verdict = Verdict::default();
                    verdict.broken.insert(DRIVER, message(e.as_ref()));
                    verdict
                }
            };
            (verdict, log)
        });
        for (seed, (verdict, log)) in batch.clone().zip(results) {
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
        ran = batch.end;
        if broken.values().any(|cases| !cases.is_empty()) || must.iter().all(|r| reached[r] >= FLOOR) {
            break;
        }
    }
    panic::set_hook(hook);
    // What the histories reached and counted, said whatever comes of the test: written to the standard error itself,
    // which the test harness does not hold back as it holds back what a test prints.
    let mut err = std::io::stderr();
    let _ = writeln!(
        err,
        "{ran} histories, {continued} with a content continuation; the most of its bound one took: {most:?}"
    );
    let _ = writeln!(err, "reached, in how many histories:");
    for (what, n) in &reached {
        let _ = writeln!(err, "  {n:5}  {what}");
    }
    let _ = writeln!(
        err,
        "runs beyond a cell's first in the content continuation, and of them runs for nothing:"
    );
    for (cell, (all, nothing)) in &runs {
        let _ = writeln!(err, "  {cell}: {all} runs, {nothing} for nothing");
    }
    let mut failures = Vec::new();
    for (property, cases) in &broken {
        let seeds: Vec<u64> = cases.iter().map(|(s, _, _)| *s).collect();
        let _ = writeln!(err, "{property}: broken in {} histories: {seeds:?}", cases.len());
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
    let unreached: Vec<String> = (must.iter())
        .filter(|r| reached[*r] < FLOOR)
        .map(|r| format!("{r}: {}", reached[r]))
        .collect();
    let all: Vec<String> = reached.iter().map(|(r, n)| format!("{r}: {n}")).collect();
    assert!(
        unreached.is_empty(),
        "fewer than {FLOOR} of {ran} histories reached these, so the properties about them pin nothing: \
         {unreached:?}\nin how many histories each situation was reached: {all:?}"
    );
}

#[test]
fn random_works_on_the_engineers_flow_keep_what_the_flow_must() {
    histories();
}
