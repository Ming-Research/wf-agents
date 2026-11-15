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
//!   number of rounds its limits bound, waits, saying for what and for whom, is stalled, saying what it waits for and
//!   why no cell that writes it runs, or has ended.
//! - `records-give-the-same-walk`: walking a work's records as they were at any moment, on the flow then installed and
//!   with the named tests as they then came out, gives the decision the engine took at that moment.
//! - `a-stop-changes-nothing`: for the same returns of the agents and the same acts of people, a history in which the
//!   engine was stopped at any step and started again ends with the same blocks and the same decisions, each effect
//!   made once, as the history without the stop.

use std::collections::BTreeMap;

use decide::{
    Abandoning, BlockId, Cause, Condition, Decision, End, Flow, Maker, Pause, Pauser, Records, Room, RunId, Seq,
    Started, TestKind, Waited, decide, decide_in, view,
};

use crate::{Rng, VERDICT};
use reading::{Reading, Reason, Writer};

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
pub const STALLED_RIGHT: &str = "a stalled work names what it waits for, who writes it and why none of them runs";

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
    SENT_BACK_BOUNDED,
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
const LIMITS_CHECKED: &str = "a work waits at a cell's limit of the times one sender sent it back";
const SENT_BACK_BOUNDED: &str = "a run for what a cell after it wrote starts";
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
pub const WAITS: &str = "while a cell waits for a result, only a cell that gives it runs after it, and no round stops";
pub const RESULTS: &str = "a result is handed in only for an entry that waits for it";
const WAITED_FOR_A_RESULT: &str = "a cell runs that gives the result a cell before it waits for";
const RESULT_GIVEN: &str = "the result of an entry is handed in";
const ENTRY_PUT_AWAY: &str = "the entry of a cell skipped is put away while a cell that gives results runs";
const ENTRY_TAKEN_UP: &str = "a cell that gives results runs for an entry it put away, its cell applying again";
const WAITING_HOLDS_A_STOP: &str = "the owner turns to a cell a waiting cell holds back while a round is under way";
pub const OUTSIDE: &str = "a work waits for the outside only for what the latest run of a cell said it waits for";
const OUTSIDE_WAITED: &str = "a work waits for main to move, as a check said what it wrote waits for";

/// What the histories with a check before the merge must reach (decision what-a-run-wrote-may-wait-for-the-outside).
pub const CHECKS_REACHED: &[&str] = &[OUTSIDE_WAITED];

/// What the histories that draw a cell asking for tests must reach (decisions a-wait-holds-its-cell,
/// a-wait-of-a-skipped-cell-is-put-away).
pub const WAITS_REACHED: &[&str] = &[
    WAITED_FOR_A_RESULT,
    RESULT_GIVEN,
    ENTRY_PUT_AWAY,
    ENTRY_TAKEN_UP,
    WAITING_HOLDS_A_STOP,
];
/// Not required: the histories' merge waits for its word while nothing else is to be done.
const STALLED: &str = "a work is stalled";
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
        .chain(r.cancels.iter().copied())
        .chain(r.releases.iter().copied())
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
        questions: kept(&r.questions, |e| e.at, upto),
        answers: kept(&r.answers, |e| e.at, upto),
        let_go: kept(&r.let_go, |e| e.at, upto),
        pauses: kept(&r.pauses, |e| e.at, upto),
        cancels: kept(&r.cancels, |at| *at, upto),
        reopens: kept(&r.reopens, |at| *at, upto),
        releases: kept(&r.releases, |at| *at, upto),
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
/// owner turning to it. For each `Changed { block, behind }`, `behind` is true exactly when every new version was
/// written by a cell after this one, and no cell after it sends it back once more where it did so as many times in a
/// row as the cell's limit since its counts last began anew, by this reading (decision the-same-sender-again).
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
                    "run {:?} of {} starts for {block} with behind {behind}, but its new versions were written by \
                     {who:?}",
                    s.run, s.cell
                ),
            );
        }
    }

    // Each cell after it whose version runs it sends it back once more, which its limit allows only so often since its
    // counts last began anew (decision the-same-sender-again).
    if let Some(senders) = reading.sent_by(flow, i, &s.because, s.at) {
        for sender in senders {
            v.count(SENT_BACK_BOUNDED);
            let times = reading.sent_back_since(flow, i, &sender, s.at);
            if times >= cell.again as usize {
                v.fail(
                    SPOKE,
                    format!(
                        "run {:?} of {} starts for what {sender} wrote, which sent it back {times} times since its \
                         counts last began anew, and its limit is {}",
                        s.run, s.cell, cell.again
                    ),
                );
            }
        }
    }
}

/// The first cell before place `upto` that applies and waits for the result of what it asked for, by this reading.
fn waiting(flow: &Flow, reading: &Reading, upto: usize) -> Option<usize> {
    let now = Seq(u64::MAX);
    (0..upto).find(|&j| reading.applies(flow, j) && reading.pending(&flow.cells()[j], now))
}

/// While a cell waits for the result of what it asked for, no later cell runs but one that gives that result; and the
/// owner's turning to a cell such a wait holds back stops no round under way (decision a-wait-holds-its-cell).
pub fn waits(v: &mut Verdict, flow: &Flow, reading: &Reading, decision: &Decision) {
    let cells = flow.cells();
    match decision {
        Decision::Run { cell, .. } => {
            let Some(i) = flow.place(cell) else { return };
            if !reading.taken_up(flow, &cells[i], Seq(u64::MAX)).is_empty() {
                v.count(ENTRY_TAKEN_UP);
            }
            let Some(j) = waiting(flow, reading, i) else { return };
            if (cells[j].asks.iter()).any(|asks| cells[i].runs_on.contains(&asks.record)) {
                v.count(WAITED_FOR_A_RESULT);
            } else {
                v.fail(
                    WAITS,
                    format!("{cell} runs while {} before it waits for a result", cells[j].id),
                );
            }
        }
        Decision::Busy { cell, .. } => {
            // The owner's words to a cell the wait holds back wait for the wait to end.
            let Some(i) = flow.place(cell) else { return };
            let Some(j) = waiting(flow, reading, i) else { return };
            let now = Seq(u64::MAX);
            let held = (j + 1..i).any(|k| {
                (reading.reasons(&cells[k], now).iter())
                    .any(|r| matches!(r, Reason::Changed { block, .. } if flow.interrupts(block)))
            });
            if held {
                v.count(WAITING_HOLDS_A_STOP);
            }
        }
        Decision::Abandon {
            cell,
            because: Abandoning::Stopped { cell: to, .. },
            ..
        } => {
            let (Some(i), Some(k)) = (flow.place(cell), flow.place(to)) else {
                return;
            };
            if let Some(j) = waiting(flow, reading, k)
                && j < i
            {
                let what = format!(
                    "{cell}'s round is stopped for {to}, which {} waiting before it holds back",
                    cells[j].id
                );
                v.fail(WAITS, what);
            }
        }
        _ => {}
    }
}

/// A result the run handed in is for an entry of the asking cell that waited for it as the run started: an entry it
/// wrote before, with no result since; and an entry of a cell that does not apply, waiting so, is passed over (decision
/// a-wait-of-a-skipped-cell-is-put-away).
pub fn results(v: &mut Verdict, flow: &Flow, reading: &Reading, run: RunId) {
    let s = reading.started[&run];
    let f = reading.finished[&run];
    let entry_of = |cell: &decide::Cell, record: &BlockId| {
        (reading.of(record).iter())
            .filter(|x| x.at < s.at && matches!(&x.by, Writer::Cell(c, _) if *c == cell.id))
            .map(|x| x.at)
            .max()
    };
    for (j, cell) in flow.cells().iter().enumerate() {
        for asks in &cell.asks {
            let entry = entry_of(cell, &asks.record);
            let answered = |entry: Seq| reading.of(&asks.result).iter().any(|x| x.at > entry && x.at < s.at);
            let gave = f.writes.iter().any(|w| w.block == asks.result);
            match entry {
                Some(entry) if gave && !answered(entry) => v.count(RESULT_GIVEN),
                _ if gave => v.fail(
                    RESULTS,
                    format!("run {run:?} hands in {} for no entry that waits", asks.result),
                ),
                Some(entry) if !answered(entry) && !reading.applies(flow, j) => {
                    let gives = flow.cells().iter().find(|c| c.id == s.cell);
                    if gives.is_some_and(|g| g.runs_on.contains(&asks.record)) {
                        v.count(ENTRY_PUT_AWAY);
                    }
                }
                _ => {}
            }
        }
    }
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
            // A record whose entry waits again for the cell that gives its result enters that cell as a new one does.
            let taken_up = reading.taken_up(flow, cell, now).contains(&input.block);
            let new = !reading.new_to(&cell.id, &input.block, now).is_empty() || taken_up;
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
    match d {
        Decision::Wait { at, on } => {
            let i = flow.place(at).expect("the work waits at a cell of the flow");
            for o in on {
                match o {
                    Waited::Limit { by, runs } => limit(v, flow, reading, at, by, *runs),
                    Waited::Precondition {
                        on: Condition::Test(test),
                        by,
                    } => {
                        held(v, flow, reading, i, test);
                        made_by(v, flow, reading, at, test, by);
                    }
                    Waited::Precondition { on, .. } => {
                        if reading.key_holds(flow, on, i) {
                            v.fail(HELD, format!("the work waits at {at} on {on:?}, which holds"));
                        }
                    }
                    Waited::Outside { source, run, block } => outside(v, flow, reading, i, (source, *run, block)),
                    Waited::Call { .. } | Waited::Slot | Waited::Answer { .. } | Waited::Result { .. } => {}
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
        Decision::Stalled { at, on, by } => stalled(v, flow, reading, at, on, by),
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
/// no cell before it that applies has a run of its own at rest, anything to be entered for, calls it waits for, or a
/// precondition that does not hold, by the histories' own reading.
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
        let unmet = cell.requires.iter().any(|key| !reading.key_holds(flow, key, j));
        // A cell that waits for a result the cell entered gives holds the work behind it, whatever else it waits for:
        // only the owner turning to it, or its run at rest, comes first (decision a-wait-holds-its-cell).
        let gives = (cell.asks.iter()).any(|asks| flow.cells()[i].runs_on.contains(&asks.record));
        if reading.pending(cell, now) && gives {
            if at_rest || !entering.is_empty() {
                let what = format!(
                    "the walk passes {} to {}, which gives what it waits for",
                    cell.id,
                    flow.cells()[i].id
                );
                v.fail(FIRST, what);
            }
            return;
        }
        if at_rest || !entering.is_empty() || owed_open || unmet {
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
        // A cell that waits for a result holds the work: the cells after it are passed for what ends the wait, which
        // the property of waits judges (decision a-wait-holds-its-cell).
        if reading.pending(cell, now) {
            return;
        }
    }
}

/// Property what-a-run-wrote-may-wait-for-the-outside: a work waits at the cell at this place for a source outside the
/// work only where the run named is the latest finished run of its cell, which writes the block named; that run said
/// what it wrote waits for a block the source writes, of which nothing is new to the cell; and the block keeps a key of
/// the preconditions of the cell where the work waits from holding.
fn outside(v: &mut Verdict, flow: &Flow, reading: &Reading, i: usize, (source, run, block): (&str, RunId, &BlockId)) {
    v.count(OUTSIDE_WAITED);
    let now = Seq(u64::MAX);
    let cells = flow.cells();
    let Some(started) = reading.started.get(&run) else {
        v.fail(
            OUTSIDE,
            format!("the work waits for {source} for run {run:?}, which never started"),
        );
        return;
    };
    let cell = &started.cell;
    let latest = (reading.finished.values())
        .filter(|f| reading.started[&f.run].cell == *cell)
        .max_by_key(|f| f.at);
    let Some(latest) = latest.filter(|f| f.run == run) else {
        v.fail(
            OUTSIDE,
            format!("the work waits for {source} for run {run:?}, the latest finished of {cell} or none"),
        );
        return;
    };
    let waited = latest.waits_for.as_ref();
    let of_source = waited.is_some_and(|b| flow.outsiders(b).is_some_and(|o| o.outside.as_deref() == Some(source)));
    let place = flow.place(cell).expect("a cell of the flow");
    let writes = cells[place].outputs.contains(block);
    let new = waited.is_some_and(|b| !reading.new_to(cell, b, now).is_empty());
    let keeps = (cells[i].requires.iter()).any(|key| {
        !reading.key_holds(flow, key, i)
            && match key {
                Condition::Is { block: b, .. } => b == block,
                Condition::Test(test) => flow.tests()[test].reads.contains(block),
            }
    });
    if !of_source || !writes || new || !keeps {
        v.fail(
            OUTSIDE,
            format!(
                "the work waits at {} for {source} for {block} of run {run:?} of {cell}, which said it waits for \
                 {waited:?} of the source {of_source}, writes {block} {writes}, has something of it new {new}, and \
                 keeps a key from holding {keeps}",
                cells[i].id
            ),
        );
    }
}

/// A further property, of a stalled work (principles decide-purely and no-unseen-spending-loop: a work that cannot go
/// on by itself says why): nothing the flow lets run before where it stands can run, nor anything after its last cell
/// where it stands there; what it waits for there does not hold, and no person or source outside the work could make
/// it hold by writing a block it reads that keeps it from holding; no call waits there; and each cell it names writes a
/// block it waits for, once, in the order of the walk, with the reason the histories' reading gives: held at or after
/// where the work stands, skipped where its condition does not hold, and else with nothing new.
fn stalled(
    v: &mut Verdict,
    flow: &Flow,
    reading: &Reading,
    at: &decide::CellId,
    on: &[Condition],
    by: &[decide::Stalling],
) {
    v.count(STALLED);
    let now = Seq(u64::MAX);
    let cells = flow.cells();
    let i = flow.place(at).expect("the work is stalled at a cell of the flow");
    let cell = &cells[i];
    let unmet: Vec<&Condition> = (cell.requires.iter())
        .filter(|key| !reading.key_holds(flow, key, i))
        .collect();
    if on.iter().any(|key| reading.key_holds(flow, key, i)) || unmet.iter().any(|key| !on.contains(key)) {
        v.fail(
            STALLED_RIGHT,
            format!("the work is stalled at {at} on {on:?}, while the keys that do not hold there are {unmet:?}"),
        );
    }
    let records = reading.records;
    let open_call = (records.called.iter()).any(|c| {
        reading.started[&c.run].cell == *at
            && !reading.done(c.call, now)
            && !records.abandoned.iter().any(|a| a.run == c.run)
    });
    if open_call {
        v.fail(
            STALLED_RIGHT,
            format!("the work is stalled at {at}, where a call waits"),
        );
    }
    // What it waits for: the blocks the keys read, or, where it waits on none, those that run its last cell.
    let read: Vec<BlockId> = if on.is_empty() {
        if i != cells.len() - 1 {
            v.fail(
                STALLED_RIGHT,
                format!("the work is stalled at {at} on nothing, which is not its last cell"),
            );
        }
        for (j, other) in cells.iter().enumerate() {
            if reading.applies(flow, j) && !reading.entering(flow, other, now).is_empty() {
                v.fail(
                    FIRST,
                    format!(
                        "the work is stalled at its last cell, and {} has something new",
                        other.id
                    ),
                );
            }
        }
        cell.runs_on.clone()
    } else {
        first_entered(v, flow, reading, i);
        (on.iter())
            .flat_map(|key| match key {
                Condition::Test(test) => flow.tests()[test].reads.clone(),
                Condition::Is { block, .. } => vec![block.clone()],
            })
            .collect()
    };
    // No person or source outside the work writes a block that keeps a key from holding, as the reading sees it.
    for key in on {
        let keeping = reading.keeping(flow, key, i);
        if let Some(b) = keeping
            .iter()
            .find(|b| flow.outsiders(b).is_some_and(|o| o.people || o.outside.is_some()))
        {
            v.fail(
                STALLED_RIGHT,
                format!("the work is stalled at {at} on {key:?}, which {b}, written from outside the cells, keeps"),
            );
        }
    }
    let places: Vec<usize> = (by.iter())
        .map(|s| flow.place(&s.cell).expect("a cell of the flow"))
        .collect();
    if !places.windows(2).all(|w| w[0] < w[1]) {
        v.fail(
            STALLED_RIGHT,
            format!("the work is stalled at {at}, naming {by:?} out of order or twice"),
        );
    }
    for (s, &j) in by.iter().zip(&places) {
        let writes = (cells[j].outputs.iter()).any(|b| read.contains(b));
        let why = if j >= i {
            decide::Unrun::Held
        } else if !reading.applies(flow, j) {
            let on = (cells[j].applies.iter())
                .filter(|key| !reading.key_holds(flow, key, j))
                .cloned()
                .collect();
            decide::Unrun::Skipped { on }
        } else {
            decide::Unrun::NothingNew
        };
        if !writes || s.why != why {
            v.fail(
                STALLED_RIGHT,
                format!(
                    "the work is stalled at {at}, naming {} as {:?}, which writes what it waits for {writes} and does \
                     not run as {why:?}",
                    s.cell, s.why
                ),
            );
        }
        if why == decide::Unrun::NothingNew && !reading.entering(flow, &cells[j], now).is_empty() {
            v.fail(
                FIRST,
                format!("the work is stalled at {at}, and {} has something new", s.cell),
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
        Abandoning::Cancelled => reading.records.cancelled().is_some(),
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
    let i = flow.place(at).expect("the work waits at a cell of the flow");
    let key = Condition::Test(test.clone());
    let holding = reading.keeping(flow, &key, i);
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

/// Property no-merge-on-what-is-not-settled, which is there because a merge cannot be taken back: as a round of the
/// merge begins, any of which may be its last, every precondition of the merge holds by the histories' own reading. For
/// the judges' verdicts that is said from the records ([`Reading::refused`]): no round of the merge begins while some
/// judge that applies now has, as its own last write of its verdict, no, with no person's write of the verdict since,
/// nor while a person's no is the latest write. And nothing is new to a cell before the merge that is not skipped.
fn settled(v: &mut Verdict, flow: &Flow, reading: &Reading) {
    v.count(MERGE_ROUNDS);
    let cells = flow.cells();
    let m = cells.len() - 1;
    let merge = &cells[m];
    for key in &merge.requires {
        if !reading.key_holds(flow, key, m) {
            v.fail(SETTLED, format!("the merge begins a round while {key:?} does not hold"));
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

/// Who wrote what is new, the count: a work waits at a cell's limit of a sender only when that sender, a cell after
/// it, sent it back as many times since its counts last began anew as the wait says and at least its limit, and the run
/// it would start is for what that sender wrote (decision the-same-sender-again).
fn limit(v: &mut Verdict, flow: &Flow, reading: &Reading, at: &decide::CellId, by: &decide::CellId, runs: u32) {
    v.count(LIMITS_CHECKED);
    let i = flow.place(at).expect("the work waits at a cell of the flow");
    let cell = &flow.cells()[i];
    let now = Seq(u64::MAX);
    let times = reading.sent_back_since(flow, i, by, now);
    if runs < cell.again || (runs as usize) != times {
        v.fail(
            SPOKE,
            format!(
                "the work waits at {at}'s limit of {} after {by} sent it back {runs} times, and the records give \
                 {times}",
                cell.again
            ),
        );
    }
    let next: Vec<Cause> = (reading.entering(flow, cell, now).into_iter())
        .filter_map(|r| match r {
            Reason::Changed { block, was } => Some(Cause::Changed {
                behind: reading.behind(flow, i, &block, now),
                block,
                was,
            }),
            Reason::Results { .. } => None,
        })
        .collect();
    if !reading
        .sent_by(flow, i, &next, now)
        .is_some_and(|senders| senders.contains(by))
    {
        v.fail(
            SPOKE,
            format!("the work waits at {at}'s limit of {by}, and its next run would not be for what {by} wrote"),
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
        Decision::Stalled { at, on, .. } if !on.is_empty() => (records.clone(), place(at)),
        Decision::Stalled { .. } => (records.clone(), cells.len()),
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
                Decision::Stalled { at, on, .. } if !on.is_empty() => (records.clone(), place(&at)),
                Decision::Stalled { .. } => (records.clone(), cells.len()),
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
    let reading = Reading::of_flow(flow, &then);
    let now = Seq(u64::MAX);
    for (i, cell) in cells[..exempt_from].iter().enumerate() {
        // A cell that waits for a result, and every cell after it, is reached once the wait is over (decision
        // a-wait-holds-its-cell).
        if reading.applies(flow, i) && reading.pending(cell, now) {
            break;
        }
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

/// A result is no one's send-back (decision the-same-sender-again): of a cell whose limit is one, a run entered by the
/// result of its entry and by a version a cell after it wrote, then a run entered by the result of its next entry
/// alone, counts no sender for the result; the count the properties keep says so as the walk does. Well under a
/// millisecond alone; the random histories reach this row of acts only now and then, and this holds it every run.
#[test]
fn a_result_sends_no_cell_back_in_the_count_the_properties_keep() {
    use crate::support::{Says::Matters, World, cell};
    use crate::{BRIEF, GIVER, TESTS};
    let mut asker = cell("asker", &[BRIEF, "o", "asker_tested"], &["x", TESTS]);
    asker.asks.push(decide::Asks {
        record: TESTS.into(),
        result: "asker_tested".into(),
    });
    asker.again = 1;
    let cells = vec![
        asker,
        cell("later", &["x"], &["o"]),
        cell(GIVER, &[TESTS], &["asker_tested"]),
    ];
    let mut w = World::with_gate(cells);
    w.notes(&[BRIEF]);
    w.written_by_cells("asker_tested");
    w.written_by_cells(TESTS);
    w.person_writes(BRIEF);
    w.run("asker", &[("x", Matters)]);
    w.run("later", &[("o", Matters)]);
    // Entered for what the later cell wrote, it asks for tests and leaves that for after their result.
    w.start("asker");
    w.finish_dealing("asker", &[], &[(TESTS, Matters)]);
    w.run(GIVER, &[("asker_tested", Matters)]);
    // Entered for the result and for what the later cell wrote, it deals with both, and asks again.
    w.run("asker", &[(TESTS, Matters)]);
    w.run(GIVER, &[("asker_tested", Matters)]);
    // Entered for the result alone.
    w.start("asker");
    let reading = Reading::of_flow(&w.flow, &w.records);
    let mut v = Verdict::default();
    for s in &w.records.started {
        started(&mut v, &w.flow, &reading, s);
    }
    let spoke: Vec<&String> = (v.broken.iter())
        .filter(|(p, _)| *p == SPOKE)
        .map(|(_, what)| what)
        .collect();
    assert!(spoke.is_empty(), "{spoke:?}");
}
