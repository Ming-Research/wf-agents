//! What the engine must always keep, checked after every step of every history: each property states what a rule of
//! the entries is there to protect, not the rule itself, and is judged by the histories' own reading of the records
//! (`reading`), never by the walk's. A property found broken is noted and the history goes on, so that one history
//! can show every property it breaks; the history fails at its end.

use std::collections::{BTreeMap, BTreeSet};

use decide::{
    Abandoning, BlockId, Cause, Decision, End, Flow, Outcome, Pause, Records, RunId, Seq, Started, TestName, Tests,
    Waited, decide,
};

use crate::Rng;
use crate::reading::{Reading, Reason, Writer};
use crate::support::Named;

pub const UNSEEN: &str = "nothing is written over unseen";
pub const SPOKE: &str = "who spoke is counted right";
pub const PURE: &str = "the decision is a function of the records";
pub const REACHES: &str = "a person's write reaches its readers";
pub const REASONS: &str = "a run starts for exactly what changed";
pub const SETTLED: &str = "the merge rests on what is current and settled";
pub const REST: &str = "a run at rest goes on only while nothing it started from has changed";

/// What the histories reached, each of which some history must reach: a property about a situation no history
/// reaches checks nothing.
pub const REACHED: &[&str] = &[
    PERSON_UNDER_WAY,
    PERSON_AT_REST,
    PERSON_AND_CELL,
    TWO_CELLS_LATER_UNREAD,
    WRITES_CHECKED,
    UNREAD_BY_PERSON,
    UNREAD_BY_CELL,
    UNSEEN_SAME_MEANING,
    CAUSES_CHECKED,
    CAUSES_MIXED,
    LIMITS_CHECKED,
    WAYS_ROUND_BOUNDED,
    DECIDED_AGAIN,
    DECIDED_SHUFFLED,
    MOMENTS_REPLAYED,
    PERSON_VERSIONS_REACHED,
    REASONS_CHECKED,
    OWN_WRITE_KNOWN,
    MERGE_ROUNDS,
    ABANDONS,
    GOING_ON,
];

pub const PERSON_UNDER_WAY: &str = "a person writes while a round is under way";
pub const PERSON_AT_REST: &str = "a person writes while a run is at rest";
pub const PERSON_AND_CELL: &str = "a block both a person and a cell write";
pub const TWO_CELLS_LATER_UNREAD: &str = "a block two cells write, the later not reading it";
const WRITES_CHECKED: &str = "a run finishes writing a block";
const UNREAD_BY_PERSON: &str = "a person changes a block a run may write and does not read, while the run is open";
const UNREAD_BY_CELL: &str = "another cell changes a block a run may write and does not read, while the run is open";
const UNSEEN_SAME_MEANING: &str = "a run finishes over a version it did not start from, which changed nothing";
const CAUSES_CHECKED: &str = "a reason of a run as it starts";
const CAUSES_MIXED: &str = "a reason changed both by a cell after the run's and by someone else";
const LIMITS_CHECKED: &str = "a work waits at a cell's limit";
const WAYS_ROUND_BOUNDED: &str = "a way round starts";
const DECIDED_AGAIN: &str = "a decision taken again on the same records";
const DECIDED_SHUFFLED: &str = "a decision taken on the records in another order";
const MOMENTS_REPLAYED: &str = "a decision taken again on the records as they were";
const PERSON_VERSIONS_REACHED: &str = "a person's version of a block a cell before where the work rests reads";
const REASONS_CHECKED: &str = "the reasons of a run as it starts";
const OWN_WRITE_KNOWN: &str = "a run starts of a cell whose record wrote one of its inputs";
const MERGE_ROUNDS: &str = "a round of the merge begins";
const ABANDONS: &str = "a run is abandoned";
const GOING_ON: &str = "a run at rest goes on";

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

/// A decision the driver took, with what it was taken on: the records up to this place and the named tests as they
/// then came out.
pub struct Moment {
    pub upto: Seq,
    pub named: BTreeMap<TestName, Outcome>,
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
        .chain(r.written.iter().map(|e| e.at))
        .chain(r.let_go.iter().map(|e| e.at))
        .chain(r.pauses.iter().map(|e| e.at))
        .chain(r.cancelled)
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
        written: kept(&r.written, |e| e.at, upto),
        let_go: kept(&r.let_go, |e| e.at, upto),
        pauses: kept(&r.pauses, |e| e.at, upto),
        cancelled: r.cancelled.filter(|at| *at <= upto),
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
    shuffle(&mut r.written, &mut rng);
    shuffle(&mut r.let_go, &mut rng);
    shuffle(&mut r.pauses, &mut rng);
    r
}

/// A run that has just started.
///
/// Property 1, nothing is written over unseen, begins here: a run starts from the current version of each block its
/// cell reads or may write, and of nothing else (the vocabulary of the entries; entry a-run-is-one-mechanism).
///
/// Property 2, who spoke is counted right (entry every-way-round-has-a-limit): for each reason `Changed { block, was,
/// behind }`, `behind` is true exactly when every change of the block's meaning after `was`, the version the cell's
/// record knew, up to the version the run starts from was written by a cell after this one in the order; a change a
/// person, the outside or a cell before it made makes it false. The entry gives a way round as a run "each of its
/// reasons is a block whose meaning a cell after it last changed", and then "a run with a reason that a person, the
/// outside or a cell before it wrote starts the count anew", for "the owner is not asked to let go on what the owner
/// sent back": a person's change the cell has never seen is a reason the person wrote, though a cell after it wrote
/// over it since, and the second sentence and the Why decide. And no cell starts a way round once its last finished
/// runs since the owner last let it go on, as many in a row as its limit, were each one by this reading.
///
/// A further property, entry state-is-blocks: the run's reasons are exactly what the entries give, no record, or each
/// input whose meaning differs from the version the cell's record knows, `was` being that version. A cell does not
/// run for its own write: its record knows the version it wrote itself.
pub fn started(v: &mut Verdict, flow: &Flow, reading: &Reading, s: &Started) {
    let i = flow.place(&s.cell).expect("the histories do not change the flow");
    let cell = &flow.cells()[i];
    let blocks: BTreeSet<&BlockId> = cell.inputs.iter().chain(&cell.outputs).collect();
    let basis_blocks: BTreeSet<&BlockId> = s.basis.keys().collect();
    if blocks != basis_blocks {
        v.fail(
            UNSEEN,
            format!(
                "run {:?} of {} starts from {basis_blocks:?}, not from what it reads or may write",
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
                    "run {:?} of {} starts from {b} at {:?}, not its current version {current:?}",
                    s.run,
                    s.cell,
                    s.basis.get(b)
                ),
            );
        }
    }

    let reasons = reading.reasons(cell, s.at);
    let given: Vec<Reason> = (s.because.iter())
        .map(|c| match c {
            Cause::NoRecord => Reason::NoRecord,
            Cause::Changed { block, was, .. } => Reason::Changed {
                block: block.clone(),
                was: *was,
            },
        })
        .collect();
    v.count(REASONS_CHECKED);
    if given != reasons {
        v.fail(
            REASONS,
            format!(
                "run {:?} of {} starts because of {given:?}; the records give {reasons:?}",
                s.run, s.cell
            ),
        );
    }
    if let Some(record) = reading.record(&s.cell, s.at)
        && let Some(f) = reading.finished.get(&record)
        && f.writes.iter().any(|w| cell.inputs.contains(&w.block))
    {
        v.count(OWN_WRITE_KNOWN);
    }

    for c in &s.because {
        let Cause::Changed { block, was, behind } = c else {
            continue;
        };
        let Some(now) = s.basis.get(block).copied().flatten() else {
            v.fail(
                SPOKE,
                format!(
                    "run {:?} of {} is said to start for {block}, which has no version",
                    s.run, s.cell
                ),
            );
            continue;
        };
        v.count(CAUSES_CHECKED);
        let changes = reading.changes(block, *was, now);
        let after = |w: &Writer| matches!(w, Writer::Cell(c, _) if flow.place(c).is_some_and(|j| j > i));
        if changes.iter().any(|x| after(&x.by)) && changes.iter().any(|x| !after(&x.by)) {
            v.count(CAUSES_MIXED);
        }
        let expected = reading.behind(flow, i, block, *was, now);
        if *behind != expected {
            let who: Vec<_> = changes.iter().map(|x| (x.at, x.by.clone())).collect();
            v.fail(
                SPOKE,
                format!(
                    "run {:?} of {} starts for {block} with behind {behind}, but the changes of its meaning since \
                     {was:?} were written by {who:?}",
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
}

/// How many of the cell's last finished runs before this place, started since the owner last let it go on, were in a
/// row each a way round, by the histories' reading of the reasons each had as it started.
fn ways_round_in_a_row(flow: &Flow, reading: &Reading, i: usize, place: Seq) -> usize {
    let cell = &flow.cells()[i];
    let let_go = (reading.records.let_go.iter())
        .filter(|l| l.cell == cell.id && l.at < place)
        .map(|l| l.at)
        .max();
    let mut runs: Vec<(Seq, &Started)> = (reading.finished.values())
        .filter(|f| f.at < place)
        .map(|f| (f.at, reading.started[&f.run]))
        .filter(|(_, s)| s.cell == cell.id)
        .collect();
    runs.sort_by_key(|(at, _)| *at);
    (runs.iter().rev())
        .take_while(|(_, s)| let_go.is_none_or(|l| s.at > l))
        .take_while(|(_, s)| reading.way_round(flow, i, &reading.reasons(cell, s.at), s.at))
        .count()
}

/// A run that has just ended, finishing or abandoned.
///
/// Property 1, nothing is written over unseen (entry a-run-is-one-mechanism: "a run finished on what had changed
/// under it would write over what it never saw"): when a run finishes and writes a block, the version of the block
/// current just before has the meaning of the version the run started from. No change of what a block says, by a
/// person, the outside or another cell, is replaced as the current version by a run that did not start from it. This
/// holds as much for a block the cell may write and does not read as for one it reads: a run starts from both, and
/// one that did not read what it overwrites still overwrites it. It is narrower than "the version current just before
/// is the one the run started from": a write that changes nothing of what the block says (said not to matter by a
/// writer who saw the meaning, approved from outside as not mattering, a person confirming a choice's value) leaves
/// a run at rest as it was (entries one-round-of-a-work-at-a-time and meaning-told-by-the-writer), so the run may
/// finish over it, and nothing the block says is lost. A run that writes a block it has no version in its basis for,
/// as one started before the flow gave its cell that output would, is held to have started from no version. The
/// histories do not change the flow, so that does not arise here.
pub fn ended(v: &mut Verdict, flow: &Flow, reading: &Reading, run: RunId, end: Seq) {
    let s = reading.started[&run];
    let cell = &flow.cells()[flow.place(&s.cell).expect("the histories do not change the flow")];
    for block in cell.outputs.iter().filter(|&b| !cell.inputs.contains(b)) {
        for x in reading.of(block) {
            if x.at > s.at && x.at < end && x.changed_meaning() {
                match &x.by {
                    Writer::Person => v.count(UNREAD_BY_PERSON),
                    Writer::Cell(_, by) if *by != run => v.count(UNREAD_BY_CELL),
                    _ => {}
                }
            }
        }
    }
    let Some(f) = reading.finished.get(&run) else {
        return;
    };
    for w in &f.writes {
        v.count(WRITES_CHECKED);
        let before = reading.current_before(&w.block, f.at);
        let from = s.basis.get(&w.block).copied().flatten();
        let over = before.map(|x| x.meaning);
        if reading.meaning(&w.block, from) != over {
            v.fail(
                UNSEEN,
                format!(
                    "run {run:?} of {} finished writing {} over the version at {:?} ({:?}), whose meaning {over:?} \
                     it never saw: it started from {from:?}",
                    s.cell,
                    w.block,
                    before.map(|x| x.at),
                    before.map(|x| &x.by)
                ),
            );
        } else if before.map(|x| x.at) != from {
            v.count(UNSEEN_SAME_MEANING);
        }
    }
}

/// Why a run was abandoned, the walk deciding it or the engine finding the run stale as its round ended (a further
/// property, entry one-round-of-a-work-at-a-time, whose Why is that a run at rest is not paid for twice: a remark that
/// changes nothing for it leaves it as it was). A run is abandoned as outdated only when a block it started from has
/// another meaning now, as no longer applying only when its cell is skipped, and as cancelled only in a cancelled
/// work.
pub fn abandoned(v: &mut Verdict, flow: &Flow, tests: &dyn Tests, reading: &Reading, run: RunId, because: &Abandoning) {
    v.count(ABANDONS);
    let s = reading.started[&run];
    let ok = match because {
        Abandoning::Cancelled => reading.records.cancelled.is_some(),
        Abandoning::NoLongerApplies => !(flow.place(&s.cell)).is_some_and(|i| reading.applies(&flow.cells()[i], tests)),
        Abandoning::Outdated { block } => s
            .basis
            .get(block)
            .is_some_and(|from| reading.meaning(block, *from) != reading.current(block).map(|x| x.meaning)),
    };
    if !ok {
        v.fail(
            REST,
            format!(
                "run {run:?} of {} is abandoned as {because:?}, but nothing it started from changed",
                s.cell
            ),
        );
    }
}

/// What must hold of each decision as it is taken. `paused` says whether the records have the work paused.
pub fn decision(v: &mut Verdict, flow: &Flow, tests: &Named, reading: &Reading, d: &Decision, paused: bool, seed: u64) {
    let records = reading.records;
    // Property 3, the decision is a function of the records (entry records-give-the-same-walk; principle
    // decide-purely): the same records walked again give the same decision, and so do the same entries at the same
    // places listed in another order, as a store may give them back.
    v.count(DECIDED_AGAIN);
    if decide(flow, records, tests) != *d {
        v.fail(PURE, "walking the same records again gives another decision".into());
    }
    v.count(DECIDED_SHUFFLED);
    let other = decide(flow, &shuffled(records, seed), tests);
    if other != *d {
        v.fail(PURE, format!("the same records in another order give {other:?}"));
    }

    let merge = flow.cells().len() - 1;
    match d {
        Decision::Wait { at, on } => {
            for o in on {
                if let Waited::Limit { runs } = o {
                    limit(v, flow, reading, at, *runs);
                }
            }
        }
        Decision::GoOn { run, .. } => {
            v.count(GOING_ON);
            let s = reading.started[run];
            let changed: Vec<&BlockId> = (s.basis.iter())
                .filter(|(b, from)| reading.meaning(b, **from) != reading.current(b).map(|x| x.meaning))
                .map(|(b, _)| b)
                .collect();
            let applies = (flow.place(&s.cell)).is_some_and(|i| reading.applies(&flow.cells()[i], tests));
            if !changed.is_empty() || !applies {
                v.fail(
                    REST,
                    format!(
                        "run {run:?} of {} goes on, though {changed:?} changed in meaning since it started or its \
                         cell is skipped ({})",
                        s.cell, !applies
                    ),
                );
            }
        }
        Decision::Abandon { run, because, .. } => abandoned(v, flow, tests, reading, *run, because),
        _ => {}
    }
    let merge_begins = match d {
        Decision::Run { cell, .. } => flow.place(cell) == Some(merge),
        Decision::GoOn { cell, .. } => !paused && flow.place(cell) == Some(merge),
        _ => false,
    };
    if merge_begins {
        settled(v, flow, tests, reading);
    }
}

/// A further property, entries merge-only-on-what-is-current and no-merge-on-what-is-not-settled, whose Why is that a
/// merge cannot be taken back: as a round of the merge begins, any of which may be its last, every precondition of
/// the merge holds, and the record of every cell before it that is not skipped knows the current meaning of each of
/// its inputs.
fn settled(v: &mut Verdict, flow: &Flow, tests: &dyn Tests, reading: &Reading) {
    v.count(MERGE_ROUNDS);
    let cells = flow.cells();
    let merge = &cells[cells.len() - 1];
    for test in &merge.requires {
        if tests.test(test) != Outcome::Holds {
            v.fail(SETTLED, format!("the merge begins a round while {test} does not hold"));
        }
    }
    let now = Seq(u64::MAX);
    for cell in &cells[..cells.len() - 1] {
        if !reading.applies(cell, tests) {
            continue;
        }
        let reasons = reading.reasons(cell, now);
        if !reasons.is_empty() {
            v.fail(
                SETTLED,
                format!("the merge begins a round while {} is not current: {reasons:?}", cell.id),
            );
        }
    }
}

/// Property 2, the count: a work waits at a cell's limit only when the cell's last finished runs since the owner last
/// let it go on, as many in a row as the wait says and at least its limit, were each a way round, and the run it
/// would start is one too. No run with a reason a person, the outside or a cell before it wrote is counted.
fn limit(v: &mut Verdict, flow: &Flow, reading: &Reading, at: &decide::CellId, runs: u32) {
    v.count(LIMITS_CHECKED);
    let i = flow.place(at).expect("the work waits at a cell of the flow");
    let cell = &flow.cells()[i];
    let now = Seq(u64::MAX);
    let in_a_row = ways_round_in_a_row(flow, reading, i, now);
    if runs < cell.limit || (runs as usize) > in_a_row {
        v.fail(
            SPOKE,
            format!(
                "the work waits at {at}'s limit of {} after {runs} ways round, and the records give {in_a_row} in a \
                 row",
                cell.limit
            ),
        );
    }
    let next = reading.reasons(cell, now);
    if !reading.way_round(flow, i, &next, now) {
        v.fail(
            SPOKE,
            format!("the work waits at {at}'s limit, and its next run would be no way round: {next:?}"),
        );
    }
}

/// Property 4, a person's write reaches its readers (entries state-is-blocks and opinions-are-inputs: whoever wrote a
/// change, the cell reading it runs for it; a remark the owner writes for a cell is given to it). Once the history
/// has come to rest, for every version a person wrote of a block and every cell that reads the block and is not
/// skipped, the cell's record knows that version or a later one, or one with the same meaning (a person's
/// confirmation of a choice's value changes nothing). Exempt are, where the work waits at a cell, for a person or at
/// a limit, that cell and every cell after it; where the work is paused, the cell it would go on with or run once let
/// on and every cell after it; a cancelled work, all of whose cells are, since it stops wherever it is; and in a
/// merged work, the versions written after the merge's last round began, the cells being taken as they were then.
pub fn reaches(v: &mut Verdict, flow: &Flow, records: &Records, tests: &Named, rest: &Decision, moments: &[Moment]) {
    let cells = flow.cells();
    let merge = cells.len() - 1;
    let place = |cell: &decide::CellId| flow.place(cell).expect("a cell of the flow");
    let (then, named, exempt_from) = match rest {
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
            (as_of(records, m.upto), Named(m.named.clone()), merge)
        }
        Decision::Wait { at, .. } => (records.clone(), Named(tests.0.clone()), place(at)),
        Decision::Paused | Decision::GoOn { .. } => {
            let mut let_on = records.clone();
            let_on.pauses.push(Pause {
                paused: false,
                at: Seq(last_place(records).0 + 1),
            });
            match decide(flow, &let_on, tests) {
                Decision::Run { cell, .. } | Decision::GoOn { cell, .. } | Decision::Wait { at: cell, .. } => {
                    (records.clone(), Named(tests.0.clone()), place(&cell))
                }
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
    for cell in &cells[..exempt_from] {
        if !reading.applies(cell, &named) {
            continue;
        }
        for block in &cell.inputs {
            for pv in reading.of(block).iter().filter(|x| x.by == Writer::Person) {
                v.count(PERSON_VERSIONS_REACHED);
                let Some(record) = reading.record(&cell.id, now) else {
                    v.fail(
                        REACHES,
                        format!("{} reads {block}, which a person wrote, and has no record", cell.id),
                    );
                    continue;
                };
                let known = reading.known(record, block);
                let reached = known.is_some_and(|k| k >= pv.at) || reading.meaning(block, known) == Some(pv.meaning);
                if !reached {
                    v.fail(
                        REACHES,
                        format!(
                            "{}'s record knows {block} at {known:?}, before the version a person wrote at {:?}",
                            cell.id, pv.at
                        ),
                    );
                }
            }
        }
    }
}

/// Property 3 on the whole history: walking the records as they were at each moment the driver took a decision, with
/// the named tests as they then came out, gives the decision it took (entry records-give-the-same-walk).
pub fn replay(v: &mut Verdict, flow: &Flow, records: &Records, moments: &[Moment]) {
    for m in moments {
        v.count(MOMENTS_REPLAYED);
        let then = decide(flow, &as_of(records, m.upto), &Named(m.named.clone()));
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
