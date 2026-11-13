//! What the engine must always keep, checked after every step of every history: each property states what a rule of
//! the design is there to protect, not the rule itself, and is judged by the histories' own reading of the records
//! (`reading`), never by the walk's. A property found broken is noted and the history goes on, so that one history
//! can show every property it breaks; the history fails at its end.
//!
//! The properties the histories of every crate name, each with what it says (design document `data-and-context`):
//!
//! - `a-run-is-one-mechanism`: a run writes no block before its return is taken, and its return is taken only when it
//!   meets what the flow requires of it; one that does not is sent back, as is one that would write over a version of a
//!   block it writes that its round was never told of: nothing is written over unseen.
//! - `consumed-only-by-what-a-return-dealt-with`: a datum becomes old to a cell only by a return of that cell that was
//!   taken and named it, up to the version its round was told of; a round that failed, a return sent back and a run
//!   stopped or abandoned consume nothing.
//! - `entered-only-for-what-is-new`: a cell is entered only when something new runs it, or the results of the calls it
//!   waits for are all in, or the owner turned to it; the cell entered is the first in the order that something new
//!   runs and that the flow lets run; a version handed in as changing nothing in meaning, or a cell's own write, runs
//!   no cell.
//! - `one-round-of-a-work-at-a-time`: no work has two rounds under way at once, and a round under way is stopped only
//!   for the owner turning to a cell before it.
//! - `no-merge-on-what-is-not-settled`: no work merges while a precondition of the merge is not met, among them that
//!   every judging cell is content, nor while something is new to a cell before it: a merge cannot be taken back.
//! - `a-work-comes-to-rest`: when people do nothing more and nothing outside the work changes, every work, after a
//!   number of rounds its limits bound, waits, saying for what and for whom, rests with nothing new, or has ended.
//! - `records-give-the-same-walk`: walking a work's records as they were at any moment, on the flow then installed and
//!   with the named tests as they then came out, gives the decision the engine took at that moment.
//! - `a-stop-changes-nothing`: for the same returns of the agents and the same acts of people, a history in which the
//!   engine was stopped at any step and started again ends with the same blocks and the same decisions, each effect
//!   made once, as the history without the stop.

use std::collections::BTreeMap;

use decide::{
    Abandoning, BlockId, Cause, Decision, End, Flow, Maker, Pause, Pauser, Records, Room, RunId, Seq, Started,
    TestKind, Waited, decide, decide_in, view,
};

use crate::reading::{Reading, Reason, Writer};
use crate::{Rng, VERDICT};

pub const UNSEEN: &str = "nothing is written over unseen";
pub const CONSUMED: &str = "a datum is consumed only by a return that dealt with it";
pub const ENTERED: &str = "a cell is entered only for what is new to it";
pub const FIRST: &str = "the cell entered is the first that something new runs";
pub const STOPPED: &str = "a round is stopped only for the owner turning to a cell before it";
pub const SPOKE: &str = "who wrote what is new is counted right";
pub const PURE: &str = "the decision is a function of the records";
pub const REACHES: &str = "a person's write reaches the cells it runs";
pub const SETTLED: &str = "the merge rests on what is current and settled";
pub const REST: &str = "a run at rest goes on only while its cell applies";
pub const HELD: &str = "a work is held only by a precondition that does not hold";
pub const MAKERS: &str = "a precondition's wait names only those who write a block keeping it from holding";

/// What the histories reached, each of which some history must reach: a property about a situation no history
/// reaches checks nothing.
pub const REACHED: &[&str] = &[
    PERSON_UNDER_WAY,
    PERSON_AT_REST,
    PERSON_AND_CELL,
    TWO_CELLS_LATER_UNREAD,
    WRITES_CHECKED,
    WRITTEN_OVER_SENT_BACK,
    CONSUMED_CHECKED,
    PARTLY_DEALT_WITH,
    NOTHING_CONSUMED,
    UNCHANGED_RUNS_NONE,
    CAUSES_CHECKED,
    RESULTS_ENTERED,
    TURNED_TO_WHILE_WAITING,
    STOPS,
    LIMITS_CHECKED,
    CALL_LIMITS_CHECKED,
    WAYS_ROUND_BOUNDED,
    DECIDED_AGAIN,
    DECIDED_SHUFFLED,
    MOMENTS_REPLAYED,
    PERSON_VERSIONS_REACHED,
    MERGE_ROUNDS,
    GOING_ON,
    MAKERS_CHECKED,
    STANDING_NO_HOLDS,
    PERSON_NO_HOLDS,
    SKIPPED_NO_PASSES,
    FAILED,
    FAILED_GOES_ON,
    FAILED_IN_PASSING,
    FAILED_UNPAUSED,
    UNPAUSED_GOES_ON,
];

pub const PERSON_UNDER_WAY: &str = "a person writes while a round is under way";
pub const PERSON_AT_REST: &str = "a person writes while a run is at rest";
pub const PERSON_AND_CELL: &str = "a block both a person and a cell write";
pub const TWO_CELLS_LATER_UNREAD: &str = "a block two cells write, the later not run by it";
const WRITES_CHECKED: &str = "a run finishes writing a block";
pub const WRITTEN_OVER_SENT_BACK: &str = "a return that would write over what its round was not told of is sent back";
const CONSUMED_CHECKED: &str = "what a cell consumed of a block that runs it";
const PARTLY_DEALT_WITH: &str = "a return deals with part of what ran its round";
pub const NOTHING_CONSUMED: &str = "a run is stopped or abandoned and its cell consumed nothing by it";
const UNCHANGED_RUNS_NONE: &str = "a version handed in as changing nothing in meaning runs no cell";
const CAUSES_CHECKED: &str = "the reasons of a run as it starts";
const RESULTS_ENTERED: &str = "a cell is entered for the results of its calls";
const TURNED_TO_WHILE_WAITING: &str = "a cell waiting for the results of its calls is entered for the owner's words";
const STOPS: &str = "a round under way is stopped for the owner turning to a cell before it";
const LIMITS_CHECKED: &str = "a work waits at a cell's limit of ways round";
const CALL_LIMITS_CHECKED: &str = "a work waits at a cell's limit of calls";
const WAYS_ROUND_BOUNDED: &str = "a way round starts";
const DECIDED_AGAIN: &str = "a decision taken again on the same records";
const DECIDED_SHUFFLED: &str = "a decision taken on the records in another order";
const MOMENTS_REPLAYED: &str = "a decision taken again on the records as they were";
const PERSON_VERSIONS_REACHED: &str = "a person's version of a block that runs a cell before where the work rests";
const MERGE_ROUNDS: &str = "a round of the merge begins";
const GOING_ON: &str = "a run at rest goes on";
const MAKERS_CHECKED: &str = "a work waits on a precondition";
const STANDING_NO_HOLDS: &str = "the merge waits on a judge's no while the judge is not skipped";
const PERSON_NO_HOLDS: &str = "the merge waits on a person's no";
const SKIPPED_NO_PASSES: &str = "a round of the merge begins over the no of a judge that is skipped";
/// Not required: the histories' merge waits for its word while nothing else is to be done.
const IDLE: &str = "a work rests with nothing new to any cell";
pub const FAILED: &str = "a round fails and the engine pauses its work";
pub const FAILED_GOES_ON: &str = "a run whose round failed goes on once the work is let on";
pub const FAILED_IN_PASSING: &str = "a round fails in passing and its run waits for the engine";
pub const FAILED_UNPAUSED: &str = "a round fails for a cause that does not pause its work";
pub const UNPAUSED_GOES_ON: &str = "a run whose round failed without pausing its work goes on";

/// How often each situation was reached.
#[derive(Default)]
pub struct Seen(pub BTreeMap<&'static str, u64>);

impl Seen {
    pub fn count(&mut self, what: &'static str) {
        *self.0.entry(what).or_default() += 1;
    }

    pub fn add(&mut self, other: &Seen) {
        for (what, n) in &other.0 {
            *self.0.entry(what).or_default() += n;
        }
    }

    pub fn get(&self, what: &str) -> u64 {
        self.0.get(what).copied().unwrap_or(0)
    }
}

/// What the properties found in one history: the first breach of each, and what was reached.
#[derive(Default)]
pub struct Verdict {
    pub broken: Vec<(&'static str, String)>,
    pub seen: Seen,
}

impl Verdict {
    pub fn fail(&mut self, property: &'static str, what: String) {
        if !self.broken.iter().any(|(p, _)| *p == property) {
            self.broken.push((property, what));
        }
    }

    pub fn count(&mut self, what: &'static str) {
        self.seen.count(what);
    }
}

/// A decision the driver took, with what it was taken on: the records up to this place, in the room the engine left
/// the work then.
pub struct Moment {
    pub upto: Seq,
    pub room: Room,
    pub decision: Decision,
}

/// The place of the latest entry of the records.
pub fn last_place(r: &Records) -> Seq {
    (r.started.iter().map(|e| e.at))
        .chain(r.rounds.iter().map(|e| e.at))
        .chain(r.returned.iter().map(|e| e.at))
        .chain(r.called.iter().map(|e| e.at))
        .chain(r.done.iter().map(|e| e.at))
        .chain(r.finished.iter().map(|e| e.at))
        .chain(r.abandoned.iter().map(|e| e.at))
        .chain(r.put_in.iter().map(|e| e.at))
        .chain(r.written.iter().map(|e| e.at))
        .chain(r.let_go.iter().map(|e| e.at))
        .chain(r.pauses.iter().map(|e| e.at))
        .chain(r.cancelled)
        .chain(r.released)
        .max()
        .unwrap_or_default()
}

/// The records as they were when the entry at this place was the latest.
pub fn as_of(r: &Records, upto: Seq) -> Records {
    fn kept<T: Clone>(list: &[T], at: impl Fn(&T) -> Seq, upto: Seq) -> Vec<T> {
        list.iter().filter(|e| at(e) <= upto).cloned().collect()
    }
    Records {
        started: kept(&r.started, |e| e.at, upto),
        rounds: kept(&r.rounds, |e| e.at, upto),
        returned: kept(&r.returned, |e| e.at, upto),
        called: kept(&r.called, |e| e.at, upto),
        done: kept(&r.done, |e| e.at, upto),
        finished: kept(&r.finished, |e| e.at, upto),
        abandoned: kept(&r.abandoned, |e| e.at, upto),
        put_in: kept(&r.put_in, |e| e.at, upto),
        written: kept(&r.written, |e| e.at, upto),
        let_go: kept(&r.let_go, |e| e.at, upto),
        pauses: kept(&r.pauses, |e| e.at, upto),
        cancelled: r.cancelled.filter(|at| *at <= upto),
        released: r.released.filter(|at| *at <= upto),
    }
}

/// A copy of the records with every list, and the writes of every finished run, in another order: the same entries
/// at the same places.
fn shuffled(r: &Records, seed: u64) -> Records {
    fn shuffle<T>(list: &mut [T], rng: &mut Rng) {
        for i in (1..list.len()).rev() {
            list.swap(i, rng.below(i + 1));
        }
    }
    let mut rng = Rng::new(seed);
    let mut r = r.clone();
    shuffle(&mut r.started, &mut rng);
    shuffle(&mut r.rounds, &mut rng);
    shuffle(&mut r.returned, &mut rng);
    shuffle(&mut r.called, &mut rng);
    shuffle(&mut r.done, &mut rng);
    shuffle(&mut r.finished, &mut rng);
    for f in &mut r.finished {
        shuffle(&mut f.writes, &mut rng);
    }
    shuffle(&mut r.abandoned, &mut rng);
    shuffle(&mut r.put_in, &mut rng);
    shuffle(&mut r.written, &mut rng);
    shuffle(&mut r.let_go, &mut rng);
    shuffle(&mut r.pauses, &mut rng);
    r
}

/// A run that has just started (properties entered-only-for-what-is-new and who wrote what is new).
///
/// The run starts from the latest version of each block that runs its cell or that it may write, and of nothing else.
/// Its reasons are exactly what the records give: each block that runs the cell with a version new to it, `was` being
/// the version the cell consumed, then the results it is owed where all are in; or, while one has no result, only the
/// owner turning to it. For each `Changed { block, behind }`, `behind` is true exactly when every new version was written
/// by a cell after this one (decision every-way-round-has-a-limit), and no cell starts a way round once its last
/// finished runs since the owner last let it go on, as many in a row as its limit, were each one by this reading.
pub fn started(v: &mut Verdict, flow: &Flow, reading: &Reading, s: &Started) {
    let i = flow.place(&s.cell).expect("the histories do not change the flow");
    let cell = &flow.cells()[i];
    let mut blocks: Vec<&BlockId> = cell.runs_on.iter().chain(&cell.outputs).collect();
    blocks.sort();
    blocks.dedup();
    let basis_blocks: Vec<&BlockId> = s.basis.keys().collect();
    if blocks != basis_blocks {
        v.fail(
            UNSEEN,
            format!(
                "run {:?} of {} starts from {basis_blocks:?}, not from what runs it or it may write",
                s.run, s.cell
            ),
        );
    }
    for b in blocks {
        let current = reading.current_before(b, s.at).map(|x| x.at);
        if s.basis.get(b).copied().flatten() != current {
            v.fail(
                UNSEEN,
                format!(
                    "run {:?} of {} starts from {b} at {:?}, not its latest version {current:?}",
                    s.run,
                    s.cell,
                    s.basis.get(b)
                ),
            );
        }
    }

    let reasons = reading.entering(flow, cell, s.at);
    let given: Vec<Reason> = (s.because.iter())
        .filter_map(|c| match c {
            Cause::NoRecord => None,
            Cause::Changed { block, was, .. } => Some(Reason::Changed {
                block: block.clone(),
                was: *was,
            }),
            Cause::Results { calls } => Some(Reason::Results { calls: calls.clone() }),
        })
        .collect();
    v.count(CAUSES_CHECKED);
    if given.is_empty() || given != reasons {
        v.fail(
            ENTERED,
            format!(
                "run {:?} of {} starts because of {given:?}; the records give {reasons:?}",
                s.run, s.cell
            ),
        );
    }
    if given.iter().any(|r| matches!(r, Reason::Results { .. })) {
        v.count(RESULTS_ENTERED);
    }
    let owed = reading.owed(&s.cell, s.at);
    if owed.iter().any(|c| !reading.done(*c, s.at)) {
        v.count(TURNED_TO_WHILE_WAITING);
    }

    for c in &s.because {
        let Cause::Changed { block, behind, .. } = c else {
            continue;
        };
        let expected = reading.behind(flow, i, block, s.at);
        if *behind != expected {
            let who: Vec<_> = (reading.new_to(&s.cell, block, s.at).iter())
                .map(|x| (x.at, x.by.clone()))
                .collect();
            v.fail(
                SPOKE,
                format!(
                    "run {:?} of {} starts for {block} with behind {behind}, but its new versions were written by {who:?}",
                    s.run, s.cell
                ),
            );
        }
    }

    if reading.way_round(flow, i, &reasons, s.at) {
        v.count(WAYS_ROUND_BOUNDED);
        let in_a_row = ways_round_in_a_row(flow, reading, i, s.at);
        if in_a_row >= cell.limit as usize {
            v.fail(
                SPOKE,
                format!(
                    "run {:?} of {} is a way round after {in_a_row} in a row, and its limit is {}",
                    s.run, s.cell, cell.limit
                ),
            );
        }
    }
    if reasons.iter().any(|r| matches!(r, Reason::Results { .. })) {
        let in_a_row = asked_in_a_row(reading, &s.cell, s.at);
        if in_a_row >= cell.calls as usize {
            v.fail(
                SPOKE,
                format!(
                    "run {:?} of {} is entered for results after {in_a_row} runs in a row that only asked, and its \
                     limit of calls is {}",
                    s.run, s.cell, cell.calls
                ),
            );
        }
    }
}

/// The cell's runs that finished before this place, started since the owner last let it go on, in the order they
/// finished.
fn finished_since_let_go<'r>(reading: &'r Reading, cell: &decide::CellId, place: Seq) -> Vec<&'r Started> {
    let let_go = (reading.records.let_go.iter())
        .filter(|l| &l.cell == cell && l.at < place)
        .map(|l| l.at)
        .max();
    let mut runs: Vec<(Seq, &Started)> = (reading.finished.values())
        .filter(|f| f.at < place)
        .map(|f| (f.at, reading.started[&f.run]))
        .filter(|(_, s)| &s.cell == cell && let_go.is_none_or(|l| s.at > l))
        .collect();
    runs.sort_by_key(|(at, _)| *at);
    runs.into_iter().map(|(_, s)| s).collect()
}

/// How many of the cell's last finished runs before this place, since the owner last let it go on, were in a row ways
/// round that dealt with something (plan, D2): a run one of whose reasons someone other than a cell after it wrote ends
/// the row, and one that dealt with nothing or was entered for no block is passed over.
fn ways_round_in_a_row(flow: &Flow, reading: &Reading, i: usize, place: Seq) -> usize {
    let cell = &flow.cells()[i];
    let mut count = 0;
    for s in finished_since_let_go(reading, &cell.id, place).into_iter().rev() {
        let forward = s.because.iter().any(|c| match c {
            Cause::NoRecord => true,
            Cause::Changed { behind, .. } => !behind,
            Cause::Results { .. } => false,
        });
        if forward {
            break;
        }
        let changed = s.because.iter().any(|c| matches!(c, Cause::Changed { .. }));
        let dealt = reading.finished[&s.run]
            .dealt_with
            .as_ref()
            .is_none_or(|d| !d.is_empty());
        if changed && dealt {
            count += 1;
        }
    }
    count
}

/// How many of the cell's last finished runs before this place, since the owner last let it go on, each asked for calls
/// and dealt with nothing, in a row (plan, D1).
fn asked_in_a_row(reading: &Reading, cell: &decide::CellId, place: Seq) -> usize {
    (finished_since_let_go(reading, cell, place).into_iter().rev())
        .take_while(|s| {
            let f = reading.finished[&s.run];
            let ended = (reading.records.returned.iter())
                .filter(|e| e.run == s.run)
                .map(|e| e.at)
                .max();
            let asked = (reading.records.called.iter())
                .any(|c| c.run == s.run && c.at < f.at && ended.is_none_or(|e| c.at > e));
            f.dealt_with.as_ref().is_some_and(Vec::is_empty) && asked
        })
        .count()
}

/// A run that has just finished (property a-run-is-one-mechanism, nothing written over unseen): of each block it
/// writes, the version latest just before its finish is the latest its round was told of. A run that dealt with part
/// of what ran it is counted.
pub fn finished(v: &mut Verdict, reading: &Reading, run: RunId) {
    let s = reading.started[&run];
    let f = reading.finished[&run];
    for w in &f.writes {
        v.count(WRITES_CHECKED);
        let before = reading.current_before(&w.block, f.at).map(|x| x.at);
        let told = reading.told(run, &w.block);
        if before != told {
            v.fail(
                UNSEEN,
                format!(
                    "run {run:?} of {} wrote {} over the version at {before:?}, and its round was told of {told:?}",
                    s.cell, w.block
                ),
            );
        }
    }
    let ran: Vec<&BlockId> = (s.because.iter())
        .filter_map(|c| match c {
            Cause::Changed { block, .. } => Some(block),
            _ => None,
        })
        .collect();
    if let Some(dealt) = &f.dealt_with
        && !dealt.is_empty()
        && ran.iter().any(|b| !dealt.contains(b))
    {
        v.count(PARTLY_DEALT_WITH);
    }
    if f.writes.iter().any(|w| w.meaning_unchanged) {
        v.count(UNCHANGED_RUNS_NONE);
    }
}

/// After every step (property consumed-only-by-what-a-return-dealt-with, and entered-only-for-what-is-new): of every
/// cell and every block that runs it, what the walk shows the cell consumed and whether the block is new to it are what
/// the histories' own reading gives, from the returns that were taken alone.
pub fn consumption(v: &mut Verdict, flow: &Flow, reading: &Reading) {
    let now = Seq(u64::MAX);
    let shown = view(flow, reading.records);
    for (cell, seen) in flow.cells().iter().zip(&shown.cells) {
        for input in &seen.runs_on {
            v.count(CONSUMED_CHECKED);
            let consumed = reading.consumed(&cell.id, &input.block, now);
            let new = !reading.new_to(&cell.id, &input.block, now).is_empty();
            if input.consumed != consumed || input.new != new {
                v.fail(
                    CONSUMED,
                    format!(
                        "{} shows {} consumed at {:?} and new {}; the returns taken give {consumed:?} and {new}",
                        cell.id, input.block, input.consumed, input.new
                    ),
                );
            }
        }
    }
}

/// What must hold of each decision as it is taken in the room the engine left the work. `paused` says whether the
/// records have the work paused, or the owner's pause of everything holds.
pub fn decision(v: &mut Verdict, flow: &Flow, reading: &Reading, d: &Decision, room: Room, paused: bool, seed: u64) {
    let records = reading.records;
    // Property 3, the decision is a function of the records (property records-give-the-same-walk; principle
    // decide-purely): the same records walked again in the same room give the same decision, and so do the same
    // entries at the same places listed in another order, as a store may give them back.
    v.count(DECIDED_AGAIN);
    if decide_in(flow, records, room) != *d {
        v.fail(PURE, "walking the same records again gives another decision".into());
    }
    v.count(DECIDED_SHUFFLED);
    let other = decide_in(flow, &shuffled(records, seed), room);
    if other != *d {
        v.fail(PURE, format!("the same records in another order give {other:?}"));
    }

    let merge = flow.cells().len() - 1;
    let now = Seq(u64::MAX);
    match d {
        Decision::Wait { at, on } => {
            let i = flow.place(at).expect("the work waits at a cell of the flow");
            for o in on {
                match o {
                    Waited::Limit { runs } => limit(v, flow, reading, at, *runs),
                    Waited::Calls { runs } => calls_limit(v, flow, reading, at, *runs),
                    Waited::Precondition { test, by } => {
                        held(v, flow, reading, i, test);
                        made_by(v, flow, reading, at, test, by);
                    }
                    Waited::Call { .. } | Waited::Slot => {}
                }
            }
            first_entered(v, flow, reading, i);
        }
        Decision::Run { cell, .. } => {
            let i = flow.place(cell).expect("a cell of the flow");
            first_entered(v, flow, reading, i);
        }
        Decision::GoOn { run, .. } => {
            v.count(GOING_ON);
            let s = reading.started[run];
            let i = flow.place(&s.cell);
            if !i.is_some_and(|i| reading.applies(flow, i)) {
                v.fail(
                    REST,
                    format!("run {run:?} of {} goes on, though its cell is skipped", s.cell),
                );
            }
            if let Some(i) = i {
                first_entered(v, flow, reading, i);
            }
        }
        Decision::Abandon { run, because, .. } => abandoned(v, flow, reading, *run, because),
        Decision::Idle => {
            v.count(IDLE);
            for (i, cell) in flow.cells().iter().enumerate() {
                if reading.applies(flow, i) && !reading.entering(flow, cell, now).is_empty() {
                    v.fail(
                        FIRST,
                        format!("the work rests with nothing to do, and {} has something new", cell.id),
                    );
                }
            }
        }
        _ => {}
    }
    let merge_begins = match d {
        Decision::Run { cell, .. } => flow.place(cell) == Some(merge),
        Decision::GoOn { cell, .. } => !paused && room.slot && flow.place(cell) == Some(merge),
        _ => false,
    };
    if merge_begins {
        settled(v, flow, reading);
    }
}

/// Property entered-only-for-what-is-new, the order: where the walk enters a cell, goes on with its run or waits at it,
/// no cell before it that applies has a run of its own at rest, anything to be entered for, or calls it waits for, by
/// the histories' own reading.
fn first_entered(v: &mut Verdict, flow: &Flow, reading: &Reading, i: usize) {
    let now = Seq(u64::MAX);
    let records = reading.records;
    for (j, cell) in flow.cells()[..i].iter().enumerate() {
        if !reading.applies(flow, j) {
            continue;
        }
        let at_rest = (records.started.iter()).any(|s| {
            s.cell == cell.id
                && !reading.finished.contains_key(&s.run)
                && !records.abandoned.iter().any(|a| a.run == s.run)
        });
        let entering = reading.entering(flow, cell, now);
        let owed_open = reading.owed(&cell.id, now).iter().any(|c| !reading.done(*c, now));
        // A precondition that does not hold holds the work only at a cell that has something to be entered for.
        let unmet = cell.requires.iter().any(|t| !reading.holds(flow, t, j));
        if at_rest || !entering.is_empty() || owed_open {
            v.fail(
                FIRST,
                format!(
                    "the walk passes {} to {}, though it is at rest {at_rest}, has {entering:?} to be entered for, \
                     waits for calls {owed_open} or for a precondition {unmet}",
                    cell.id,
                    flow.cells()[i].id
                ),
            );
        }
    }
}

/// Why a run was abandoned (properties one-round-of-a-work-at-a-time and consumed-only-by-what-a-return-dealt-with): as
/// cancelled only in a cancelled work; as no longer applying only when its cell is skipped; as stopped only while a
/// round of it is under way, for a cell before it that applies, to which something that interrupts is new.
pub fn abandoned(v: &mut Verdict, flow: &Flow, reading: &Reading, run: RunId, because: &Abandoning) {
    let s = reading.started[&run];
    let now = Seq(u64::MAX);
    let ok = match because {
        Abandoning::Cancelled => reading.records.cancelled.is_some(),
        Abandoning::NoLongerApplies => !(flow.place(&s.cell)).is_some_and(|i| reading.applies(flow, i)),
        Abandoning::Stopped { cell, block } => {
            v.count(STOPS);
            let place = |c: &decide::CellId| flow.place(c);
            let before = place(cell).zip(place(&s.cell)).is_some_and(|(j, i)| j < i);
            let new = !reading.new_to(cell, block, now).is_empty();
            let applies = place(cell).is_some_and(|j| reading.applies(flow, j));
            before && new && applies && flow.interrupts(block)
        }
    };
    if !ok {
        let property = match because {
            Abandoning::Stopped { .. } => STOPPED,
            _ => REST,
        };
        v.fail(
            property,
            format!(
                "run {run:?} of {} is abandoned as {because:?}, which the records do not bear out",
                s.cell
            ),
        );
    }
}

/// A further property, of the preconditions (decision conditions-and-preconditions: what a work waits for is the
/// precondition not met): a work waits on a precondition only while, by the histories' own reading, it does not hold.
/// So the merge waits on the judge's verdict only while it stands at no: never for the no of a judge that is skipped.
fn held(v: &mut Verdict, flow: &Flow, reading: &Reading, i: usize, test: &decide::TestName) {
    if reading.holds(flow, test, i) {
        v.fail(
            HELD,
            format!("the work waits at {} on {test}, which holds", flow.cells()[i].id),
        );
    } else if flow.tests()[test].kind == TestKind::NoneStanding {
        let latest = reading.current(&VERDICT.into());
        if latest.is_some_and(|x| x.by == Writer::Person) {
            v.count(PERSON_NO_HOLDS);
        } else {
            v.count(STANDING_NO_HOLDS);
        }
    }
}

/// A further property, of who a precondition waits for (principle decide-purely: a wait says who can give it; decision
/// conditions-and-preconditions): a work waits on a precondition for someone, each named once, and each it names
/// writes, by the flow, a block the test reads that keeps it from holding by the histories' own reading: one without
/// the value for a test that every block has it, any for a test that one has it, and one with the value for the others.
fn made_by(
    v: &mut Verdict,
    flow: &Flow,
    reading: &Reading,
    at: &decide::CellId,
    test: &decide::TestName,
    by: &[Maker],
) {
    v.count(MAKERS_CHECKED);
    let named = &flow.tests()[test];
    let has = |b: &BlockId| {
        reading
            .current(b)
            .is_some_and(|x| x.value.as_deref() == Some(named.is.as_str()))
    };
    let holding: Vec<&BlockId> = (named.reads.iter())
        .filter(|b| match named.kind {
            TestKind::All => !has(b),
            TestKind::Any => true,
            TestKind::None | TestKind::NoneStanding => has(b),
        })
        .collect();
    let writes = |m: &Maker, b: &BlockId| {
        let outsiders = flow.outsiders(b);
        match m {
            Maker::Owner => outsiders.is_some_and(|o| o.people),
            Maker::Outside { source } => outsiders.is_some_and(|o| o.outside.as_ref() == Some(source)),
            Maker::Cell { cell } => flow.writers(b).any(|c| &c.id == cell),
        }
    };
    let once = by.windows(2).all(|w| w[0] < w[1]);
    if by.is_empty() || !once || !by.iter().all(|m| holding.iter().any(|b| writes(m, b))) {
        v.fail(
            MAKERS,
            format!(
                "the work waits at {at} on {test} for {by:?}, though the blocks keeping it from holding are {holding:?}"
            ),
        );
    }
}

/// Property no-merge-on-what-is-not-settled, which is there because a merge cannot be taken back: as a round of the merge
/// begins, any of which may be its last, every precondition of the merge holds by the histories' own reading. For the
/// judges' verdicts that is said from the records ([`Reading::refused`]): no round of the merge begins while some judge
/// that applies now has, as its own last write of its verdict, no, with no person's write of the verdict since, nor
/// while a person's no is the latest write. And nothing is new to a cell before the merge that is not skipped.
fn settled(v: &mut Verdict, flow: &Flow, reading: &Reading) {
    v.count(MERGE_ROUNDS);
    let cells = flow.cells();
    let m = cells.len() - 1;
    let merge = &cells[m];
    for test in &merge.requires {
        if !reading.holds(flow, test, m) {
            v.fail(SETTLED, format!("the merge begins a round while {test} does not hold"));
        }
    }
    // The merge begins over a no that a judge wrote last and that holds nothing, the judge being skipped.
    let verdict = reading.current(&VERDICT.into());
    let skipped_no = verdict.is_some_and(|x| match &x.by {
        Writer::Cell(judge, _) => {
            x.value.as_deref() == Some("no") && flow.place(judge).is_some_and(|j| !reading.applies(flow, j))
        }
        Writer::Person | Writer::Outside => false,
    });
    if skipped_no {
        v.count(SKIPPED_NO_PASSES);
    }
    let now = Seq(u64::MAX);
    for (i, cell) in cells[..m].iter().enumerate() {
        if !reading.applies(flow, i) {
            continue;
        }
        let reasons = reading.entering(flow, cell, now);
        if !reasons.is_empty() {
            v.fail(
                SETTLED,
                format!(
                    "the merge begins a round while {} has something new: {reasons:?}",
                    cell.id
                ),
            );
        }
    }
}

/// Who wrote what is new, the count: a work waits at a cell's limit of ways round only when the cell's last finished
/// runs since the owner last let it go on, as many in a row as the wait says and at least its limit, were each a way
/// round that dealt with something, and the run it would start is one too (plan, D2).
fn limit(v: &mut Verdict, flow: &Flow, reading: &Reading, at: &decide::CellId, runs: u32) {
    v.count(LIMITS_CHECKED);
    let i = flow.place(at).expect("the work waits at a cell of the flow");
    let cell = &flow.cells()[i];
    let now = Seq(u64::MAX);
    let in_a_row = ways_round_in_a_row(flow, reading, i, now);
    if runs < cell.limit || (runs as usize) != in_a_row {
        v.fail(
            SPOKE,
            format!(
                "the work waits at {at}'s limit of {} after {runs} ways round, and the records give {in_a_row} in a \
                 row",
                cell.limit
            ),
        );
    }
    let next = reading.entering(flow, cell, now);
    if !reading.way_round(flow, i, &next, now) {
        v.fail(
            SPOKE,
            format!("the work waits at {at}'s limit, and its next run would be no way round: {next:?}"),
        );
    }
}

/// The count of calls: a work waits at a cell's limit of calls only when its last finished runs since the owner last
/// let it go on, as many in a row as the wait says and at least its limit, each asked for calls and dealt with
/// nothing, and the cell would be entered for their results (plan, D1).
fn calls_limit(v: &mut Verdict, flow: &Flow, reading: &Reading, at: &decide::CellId, runs: u32) {
    v.count(CALL_LIMITS_CHECKED);
    let i = flow.place(at).expect("the work waits at a cell of the flow");
    let cell = &flow.cells()[i];
    let now = Seq(u64::MAX);
    let in_a_row = asked_in_a_row(reading, at, now);
    let results = (reading.entering(flow, cell, now).iter()).any(|r| matches!(r, Reason::Results { .. }));
    if runs < cell.calls || (runs as usize) != in_a_row || !results {
        v.fail(
            SPOKE,
            format!(
                "the work waits at {at}'s limit of {} calls after {runs} runs that asked; the records give {in_a_row} \
                 in a row, and it would be entered for results: {results}",
                cell.calls
            ),
        );
    }
}

/// A person's write reaches the cells it runs (the design of data and context: whoever wrote a version, the cells it
/// runs are entered for it until a return of each deals with it). Once the history has come to rest, no version a
/// person wrote of a block is new to a cell it runs that is not skipped. Exempt are, where the work waits at a cell,
/// that cell and every cell after it; where the work is paused, the cell it would go on with or run once let on and
/// every cell after it; a cancelled work, all of whose cells are, since it stops wherever it is; and in a merged work,
/// the versions written after the merge's last round began, the cells being taken as they were then.
pub fn reaches(v: &mut Verdict, flow: &Flow, records: &Records, rest: &Decision, moments: &[Moment]) {
    let cells = flow.cells();
    let merge = cells.len() - 1;
    let place = |cell: &decide::CellId| flow.place(cell).expect("a cell of the flow");
    let (then, exempt_from) = match rest {
        Decision::Ended(End::Cancelled) => return,
        Decision::Ended(End::Merged) => {
            let began = moments.iter().rev().find(|m| match &m.decision {
                Decision::Run { cell, .. } | Decision::GoOn { cell, .. } => place(cell) == merge,
                _ => false,
            });
            let Some(m) = began else {
                v.fail(REACHES, "the work merged and no round of the merge began".into());
                return;
            };
            (as_of(records, m.upto), merge)
        }
        Decision::Wait { at, .. } => (records.clone(), place(at)),
        Decision::Idle => (records.clone(), cells.len()),
        Decision::Paused | Decision::GoOn { .. } => {
            let mut let_on = records.clone();
            let_on.pauses.push(Pause {
                paused: false,
                at: Seq(last_place(records).0 + 1),
                by: Pauser::Owner,
                why: None,
            });
            match decide(flow, &let_on) {
                Decision::Run { cell, .. } | Decision::GoOn { cell, .. } | Decision::Wait { at: cell, .. } => {
                    (records.clone(), place(&cell))
                }
                Decision::Idle => (records.clone(), cells.len()),
                other => {
                    v.fail(REACHES, format!("a paused work let on decides {other:?}"));
                    return;
                }
            }
        }
        other => {
            v.fail(REACHES, format!("the history ended without coming to rest: {other:?}"));
            return;
        }
    };
    let reading = Reading::new(&then);
    let now = Seq(u64::MAX);
    for (i, cell) in cells[..exempt_from].iter().enumerate() {
        if !reading.applies(flow, i) {
            continue;
        }
        for block in &cell.runs_on {
            let new = reading.new_to(&cell.id, block, now);
            for pv in reading.of(block).iter().filter(|x| x.by == Writer::Person) {
                v.count(PERSON_VERSIONS_REACHED);
                if new.iter().any(|n| n.at == pv.at) {
                    v.fail(
                        REACHES,
                        format!(
                            "the version of {block} a person wrote at {:?} is still new to {} at rest",
                            pv.at, cell.id
                        ),
                    );
                }
            }
        }
    }
}

/// Property 3 on the whole history: walking the records as they were at each moment the driver took a decision gives
/// the decision it took (property records-give-the-same-walk).
pub fn replay(v: &mut Verdict, flow: &Flow, records: &Records, moments: &[Moment]) {
    for m in moments {
        v.count(MOMENTS_REPLAYED);
        let then = decide_in(flow, &as_of(records, m.upto), m.room);
        if then != m.decision {
            v.fail(
                PURE,
                format!(
                    "the records as they were at {:?} give {then:?}, and the decision then was {:?}",
                    m.upto, m.decision
                ),
            );
        }
    }
}
