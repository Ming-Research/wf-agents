//! The judge of a play (design document `testing`, 14): it reads a work's records against the path its script states
//! (module `script::items`), by the reading of the records the histories and the exploration judge by (crate
//! `reading`) and never by the deciding code the play tests, and names the first step that left the path, with what
//! the records hold there.
//!
//! The records hold, in the order of their places, each step of a path but a few waits: a run of a cell started; a
//! round that failed, with its kind; a return sent back, which the run's next round is given; a call a run rests at,
//! for a tool, a person, the engine's wait before a round is tried again, or the owner's word on the engine's own call
//! at a run's limit; a question that waits for the owner's answer; what a run wrote waiting for a source outside the
//! work; the work paused and let on, by whom; a cell let go at its limit; a cancel, a reopen, and the merge's finished
//! run. A wait on a precondition, or at a cell's limit of a sender, no entry records: it is read where the work came to
//! rest after a thing that happened to it, with nothing of it under way and no call, question or source waited for,
//! at the cell the reading finds it stands at, where a key of that cell's preconditions does not hold that a person or
//! a source outside the work can make hold (one only cells make hold, none of which runs, is a stall), or where the
//! cell would run once more for what a sender wrote that sent it back as often as the flow's `again` lets one since its
//! counts last began anew (decision the-same-sender-again); the wait that lasts over several things that happened is
//! one step. The pause the work was added with, and the letting on a play gives it as it adds it, are the item's start
//! and no step of its path.
//!
//! What no record holds is read from the engine's view of the work now, and so only as a path's end: a stall, which
//! the reading cross-checks (no cell that applies has anything to be entered for, no call is owed); a wait for a slot,
//! for the turn to check and merge, and for what every work shares, which are kept in memory only (decisions
//! slots-in-the-scheduler, one-work-checks-and-merges-at-a-time and the-engine-checks-the-outside-itself); and the
//! pause of everything, which the installation keeps. A round under way, which the records hold, is a path's end too.

use std::collections::BTreeSet;
use std::fmt;

use decide::{BlockId, CellId, Condition, Decision, End, Flow, Pauser, Records, RunId, Seq, TestKind, Waited};
use driver::{Along, Driver};
use flow::{Compiled, Kind};
use reading::{Reading, Reason};
use store::WorkNumber;
use work::{Ask, Back, Call, Given, Work};

use crate::script::items::{Step, Wait};

/// What the engine's view of a work shows now, for what no record of it holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown {
    /// What the work does next, in the room the engine leaves it.
    pub decision: Decision,
    /// Whether a round of it under way says it waits for the turn to check and merge.
    pub turn: bool,
    /// Whether the wait every work shares holds the round it would begin next.
    pub shared: bool,
    /// The pause of everything, by who set it, while it holds.
    pub everything: Option<Pauser>,
}

impl Shown {
    /// What the driver shows of the work now; none where it has no such work.
    pub fn of(driver: &Driver, number: WorkNumber) -> Option<Shown> {
        let kept = driver.work(number)?;
        let decision = driver.view(number)?.decision;
        let turn = (driver.along_of(number).iter()).any(|(_, along)| matches!(along, Along::Turn { .. }));
        let gate = driver.gate_of(number);
        let kind = |cell: &CellId| kept.flow.cells.iter().find(|c| c.id == *cell).map(|c| c.kind);
        let shared = match &decision {
            Decision::Run { cell, .. } | Decision::GoOn { cell, .. } => match kind(cell) {
                Some(Kind::Agent) => gate.holds_agents(),
                Some(Kind::Engine) => gate.holds_acts(),
                None => false,
            },
            _ => false,
        };
        let everything = driver.everything_paused().map(|paused| paused.by);
        Some(Shown {
            decision,
            turn,
            shared,
            everything,
        })
    }
}

/// How a work's records stand against its path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Judged {
    /// The work took its path and stands at its end.
    Took,
    /// The work took its path up to the step of this place, counted from one, which it has not taken, or, its last,
    /// no longer stands at: the records hold nothing past the steps before it, and the engine shows the work at this
    /// now, where it is said. More may yet come.
    Short {
        step: usize,
        said: String,
        shown: Option<String>,
    },
    /// The step of this place in the path, counted from one, left it: the path says this there, or, past its end,
    /// nothing, and the records hold this.
    Left {
        step: usize,
        said: Option<String>,
        records: String,
    },
}

impl fmt::Display for Judged {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Judged::Took => f.write_str("took its path"),
            Judged::Short {
                step,
                said,
                shown: None,
            } => write!(
                f,
                "did not take step {step} of its path, {said}: the records hold nothing more"
            ),
            Judged::Short {
                step,
                said,
                shown: Some(shown),
            } => write!(
                f,
                "does not stand at step {step} of its path, {said}: the engine shows {shown}"
            ),
            Judged::Left {
                step,
                said: Some(said),
                records,
            } => write!(f, "left its path at step {step}, {said}: the records hold {records}"),
            Judged::Left {
                said: None, records, ..
            } => write!(f, "went on past the end of its path: the records hold {records}"),
        }
    }
}

/// A step the records hold, with whether the work still stands at it as they end: a wait that has not ended, a pause
/// not lifted, a cancel that stands; any other step stands once taken.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seen {
    pub step: Step,
    pub stands: bool,
}

/// How the work's records stand against its path: `held` says whether its item is held, never let on as it was added;
/// `shown` is what the engine shows of the work now, for the end the records do not hold.
pub fn judge(flow: &Compiled, work: &Work, held: bool, path: &[Step], shown: &Shown) -> Judged {
    let seen = seen(flow, work, held);
    let (steps, end) = match path.split_last() {
        Some((end, before)) if end.ends_only() => (before, Some(end)),
        _ => (path, None),
    };
    for (k, said) in steps.iter().enumerate() {
        match seen.get(k) {
            Some(s) if said.matches(&s.step) => {}
            Some(s) => {
                return Judged::Left {
                    step: k + 1,
                    said: Some(said.to_string()),
                    records: s.step.to_string(),
                };
            }
            None => {
                return Judged::Short {
                    step: k + 1,
                    said: said.to_string(),
                    shown: None,
                };
            }
        }
    }
    if let Some(more) = seen.get(steps.len()) {
        return Judged::Left {
            step: steps.len() + 1,
            said: end.map(Step::to_string),
            records: more.step.to_string(),
        };
    }
    let now = || {
        let states: Vec<String> = (states(work, shown).iter()).map(Step::to_string).collect();
        match states.is_empty() {
            true => "nothing the work stands at".to_string(),
            false => states.join(" and "),
        }
    };
    match end {
        Some(end) => match stands_at_end(flow, &work.records, end, shown) {
            Ok(()) => Judged::Took,
            Err(Some(contradiction)) => Judged::Left {
                step: path.len(),
                said: Some(end.to_string()),
                records: contradiction,
            },
            Err(None) => Judged::Short {
                step: path.len(),
                said: end.to_string(),
                shown: Some(now()),
            },
        },
        None => match (seen.last(), path.last()) {
            (Some(last), Some(said)) if !last.stands => Judged::Short {
                step: path.len(),
                said: said.to_string(),
                shown: Some(now()),
            },
            _ => Judged::Took,
        },
    }
}

/// Whether the work stands now at an end its records do not hold, as the engine shows it: where it does not, what the
/// records say against the engine's view, where they do.
fn stands_at_end(flow: &Compiled, records: &Records, end: &Step, shown: &Shown) -> Result<(), Option<String>> {
    let holds = match end {
        Step::UnderWay { cell, run, round } => {
            let mut runs: Vec<&decide::Started> = records.started.iter().filter(|s| s.cell == *cell).collect();
            runs.sort_by_key(|s| s.at);
            (runs.into_iter().enumerate()).any(|(n, s)| {
                let began = records.rounds.iter().filter(|r| r.run == s.run).count();
                let ended = records.returned.iter().filter(|r| r.run == s.run).count();
                open(records, s.run)
                    && began > ended
                    && run.is_none_or(|run| run == n + 1)
                    && round.is_none_or(|round| round == began)
            })
        }
        Step::Stalled(cell) => {
            if !matches!(&shown.decision, Decision::Stalled { at, .. } if at == cell) {
                return Err(None);
            }
            // A stall is the walk's own conclusion; the records bear it out where nothing the flow lets run has
            // anything to be entered for and no call waits.
            let reading = Reading::of_flow(&flow.flow, records);
            let now = Seq(u64::MAX);
            let cells = flow.flow.cells();
            let entered = (0..cells.len())
                .find(|&i| reading.applies(&flow.flow, i) && !reading.entering(&flow.flow, &cells[i], now).is_empty());
            if let Some(i) = entered {
                return Err(Some(format!(
                    "something for {} to be entered for, while the engine shows the work stalled",
                    cells[i].id
                )));
            }
            if let Some(call) = records
                .called
                .iter()
                .find(|c| !records.done.iter().any(|d| d.call == c.call))
            {
                return Err(Some(format!(
                    "call {} with no result, while the engine shows the work stalled",
                    call.call.0
                )));
            }
            true
        }
        Step::Waits { cell, on: Wait::Slot } => {
            matches!(&shown.decision, Decision::Wait { at, on } if at == cell && on.contains(&Waited::Slot))
        }
        Step::Waits { cell, on: Wait::Turn } => {
            shown.turn && matches!(&shown.decision, Decision::Busy { cell: c, .. } if c == cell)
        }
        Step::Waits { cell, on: Wait::Shared } => {
            let next = match &shown.decision {
                Decision::Run { cell, .. } | Decision::GoOn { cell, .. } => Some(cell),
                _ => None,
            };
            shown.shared && next == Some(cell)
        }
        Step::Everything(by) => shown.everything == Some(*by),
        _ => false,
    };
    holds.then_some(()).ok_or(None)
}

/// The states the engine shows the work at now, as steps: a round under way, each thing it waits for at the cell it
/// waits at, a stall, its end, the wait every work shares holding its next round, the work's own pause and the pause of
/// everything. What an act waits for is one of these.
pub fn states(work: &Work, shown: &Shown) -> Vec<Step> {
    let r = &work.records;
    let mut states = Vec::new();
    match &shown.decision {
        Decision::Busy { cell, run } => {
            let nth = (r.started.iter()).filter(|s| s.cell == *cell && s.run <= *run).count();
            let round = r.rounds.iter().filter(|x| x.run == *run).count();
            states.push(Step::UnderWay {
                cell: cell.clone(),
                run: Some(nth),
                round: Some(round),
            });
            if shown.turn {
                states.push(Step::Waits {
                    cell: cell.clone(),
                    on: Wait::Turn,
                });
            }
        }
        Decision::Wait { at, on } => {
            for waited in on {
                let on = match waited {
                    Waited::Call { call, on: None } => match work.calls.get(call) {
                        Some(Call::Wait { .. }) => Wait::Retry,
                        _ => Wait::Tool,
                    },
                    Waited::Call { call, on: Some(who) } => match work.calls.get(call) {
                        Some(Call::AtLimit(_)) => Wait::Limit(None),
                        _ => Wait::Person(who.clone()),
                    },
                    Waited::Precondition { on, .. } => Wait::Precondition(Some(key(on))),
                    Waited::Limit { by, .. } => Wait::Limit(Some(by.clone())),
                    Waited::Result { .. } => Wait::Result,
                    Waited::Slot => Wait::Slot,
                    Waited::Answer { question } => Wait::Answer(Some(question.clone())),
                    Waited::Outside { block, .. } => Wait::Outside(Some(block.clone())),
                };
                states.push(Step::Waits { cell: at.clone(), on });
            }
        }
        Decision::Stalled { at, .. } => states.push(Step::Stalled(at.clone())),
        Decision::Ended(end) => states.push(Step::Ends(*end)),
        Decision::Run { cell, .. } | Decision::GoOn { cell, .. } if shown.shared => states.push(Step::Waits {
            cell: cell.clone(),
            on: Wait::Shared,
        }),
        _ => {}
    }
    if let Some(pause) = r.pauses.iter().max_by_key(|p| p.at).filter(|p| p.paused) {
        states.push(Step::Paused(pause.by));
    }
    if let Some(by) = shown.everything {
        states.push(Step::Everything(by));
    }
    states
}

/// The name a step gives a key of a cell's preconditions: a named test's, or the block a key of a value reads.
fn key(condition: &Condition) -> String {
    match condition {
        Condition::Test(test) => test.to_string(),
        Condition::Is { block, .. } => block.to_string(),
    }
}

/// Whether the run has neither finished nor been abandoned.
fn open(records: &Records, run: RunId) -> bool {
    !records.finished.iter().any(|f| f.run == run) && !records.abandoned.iter().any(|a| a.run == run)
}

/// The steps the work's records hold, in order (the module's own words); `held` says whether its item is held, never
/// let on as it was added.
pub fn seen(flow: &Compiled, work: &Work, held: bool) -> Vec<Seen> {
    let r = &work.records;
    let cell_of = |run: RunId| (r.started.iter().find(|s| s.run == run)).map(|s| s.cell.clone());
    let mut entries: Vec<(Seq, Seen)> = Vec::new();
    let mut push = |at: Seq, step: Step, stands: bool| entries.push((at, Seen { step, stands }));
    for s in &r.started {
        push(s.at, Step::Runs(s.cell.clone()), true);
        let mut ended: Vec<Seq> = (r.returned.iter().filter(|e| e.run == s.run)).map(|e| e.at).collect();
        ended.sort();
        for (n, at) in (1..).zip(ended) {
            match work.back.get(&(s.run, n)) {
                Some(Back::Failed { failure, .. }) => push(
                    at,
                    Step::Fails {
                        cell: s.cell.clone(),
                        failure: *failure,
                    },
                    true,
                ),
                Some(_) if matches!(work.given.get(&(s.run, n + 1)), Some(Given::SentBack(_))) => {
                    push(at, Step::SentBack(s.cell.clone()), true);
                }
                _ => {}
            }
        }
    }
    for c in &r.called {
        let on = match work.calls.get(&c.call) {
            Some(Call::Asked(Ask::Tool { .. })) => Wait::Tool,
            Some(Call::Asked(Ask::Person { who, .. })) => Wait::Person(who.clone()),
            Some(Call::AtLimit(_)) => Wait::Limit(None),
            Some(Call::Wait { .. }) => Wait::Retry,
            None => continue,
        };
        let Some(cell) = cell_of(c.run) else { continue };
        let stands = !r.done.iter().any(|d| d.call == c.call);
        push(c.at, Step::Waits { cell, on }, stands);
    }
    // A question that waits is one wait while it is asked anew unanswered, and stands while its latest version waits.
    let now = reading::questions(r, Seq(u64::MAX));
    let mut questions: Vec<&decide::Questioned> = r.questions.iter().collect();
    questions.sort_by_key(|q| q.at);
    for q in questions {
        let before = reading::questions(r, q.at);
        let waited = before
            .get(&q.name)
            .is_some_and(|b| b.waits && !b.withdrawn && !b.answered);
        if !q.waits || q.withdrawn || waited {
            continue;
        }
        let stands = now.get(&q.name).is_some_and(|n| n.waits && !n.withdrawn && !n.answered);
        push(
            q.at,
            Step::Waits {
                cell: q.cell.clone(),
                on: Wait::Answer(Some(q.name.clone())),
            },
            stands,
        );
    }
    for f in &r.finished {
        let (Some(block), Some(cell)) = (&f.waits_for, cell_of(f.run)) else {
            continue;
        };
        let stands = !(r.written.iter()).any(|w| w.block == *block && w.by == decide::Outsider::Outside && w.at > f.at);
        push(
            f.at,
            Step::Waits {
                cell,
                on: Wait::Outside(Some(block.clone())),
            },
            stands,
        );
    }
    // The pause the work was added with, and the letting on that follows it where the item is not held, are its start.
    let mut pauses: Vec<&decide::Pause> = r.pauses.iter().collect();
    pauses.sort_by_key(|p| p.at);
    let latest = pauses.last().map(|p| p.at);
    let start = match pauses.get(1) {
        Some(let_on) if !held && !let_on.paused => 2,
        _ => 1,
    };
    for p in pauses.iter().skip(start) {
        match p.paused {
            true => push(p.at, Step::Paused(p.by), latest == Some(p.at)),
            false => push(p.at, Step::LetOn, true),
        }
    }
    for l in &r.let_go {
        push(l.at, Step::LetGo(l.cell.clone()), true);
    }
    for at in &r.cancels {
        push(*at, Step::Ends(End::Cancelled), r.cancelled() == Some(*at));
    }
    for at in &r.reopens {
        push(*at, Step::Reopened, true);
    }
    let merge = flow.flow.cells().last().map(|cell| cell.id.clone());
    for f in &r.finished {
        let dealt = f.dealt_with.as_ref().is_none_or(|d| !d.is_empty());
        if dealt && cell_of(f.run) == merge {
            push(f.at, Step::Ends(End::Merged), true);
        }
    }
    entries.sort_by_key(|(at, _)| *at);

    // The waits no entry records, read where the work came to rest after each thing that happened to it.
    // The end of the first thing that happened to it, the pause it was added with, is no place it rested: the rest of
    // its add followed at once, its block of dependencies among it.
    let mut froms: Vec<Seq> = work.moments.iter().map(|m| m.from).collect();
    froms.sort();
    let mut ends: Vec<Seq> = froms.into_iter().skip(2).collect();
    ends.push(Seq(u64::MAX));
    let mut seen = Vec::new();
    let mut entries = entries.into_iter().peekable();
    let mut standing: Vec<Step> = Vec::new();
    for end in ends {
        while let Some((_, s)) = entries.next_if(|(at, _)| *at < end) {
            if matches!(s.step, Step::Runs(_)) {
                standing.clear();
            }
            seen.push(s);
        }
        let resting = at_rest(flow, &r.before(end));
        for step in resting.iter().filter(|step| !standing.contains(step)) {
            seen.push(Seen {
                step: step.clone(),
                stands: false,
            });
        }
        standing = resting;
    }
    // Those the work stands at as its records end stand.
    let mut left: BTreeSet<usize> = BTreeSet::new();
    for step in &standing {
        if let Some(k) = (0..seen.len())
            .rev()
            .find(|k| seen[*k].step == *step && !left.contains(k))
        {
            seen[k].stands = true;
            left.insert(k);
        }
    }
    seen
}

/// The waits no entry records that hold the work as these records stand, where it is at rest: nothing of it under way,
/// no call without its result, no question waiting for its answer and no source outside the work waited for; at the
/// cell the reading finds it stands at, each key of that cell's preconditions that does not hold, or else the cell's
/// limit of the first sender that sent it back as often as its limit lets one, where it would run once more for what
/// that sender wrote.
fn at_rest(flow: &Compiled, records: &Records) -> Vec<Step> {
    let reading = Reading::of_flow(&flow.flow, records);
    let now = Seq(u64::MAX);
    let cells = flow.flow.cells();
    let merged = (records.finished.iter()).any(|f| {
        let cell = records.started.iter().find(|s| s.run == f.run).map(|s| &s.cell);
        cell == cells.last().map(|c| &c.id) && f.dealt_with.as_ref().is_none_or(|d| !d.is_empty())
    });
    let under_way = (records.started.iter()).any(|s| {
        let began = records.rounds.iter().filter(|r| r.run == s.run).count();
        began > records.returned.iter().filter(|r| r.run == s.run).count()
    });
    let calling = (records.called.iter()).any(|c| !records.done.iter().any(|d| d.call == c.call));
    let asking = (reading::questions(records, now).values()).any(|q| q.waits && !q.withdrawn && !q.answered);
    let outside = (cells.iter()).any(|cell| {
        let latest = (records.finished.iter())
            .filter(|f| records.started.iter().any(|s| s.run == f.run && s.cell == cell.id))
            .max_by_key(|f| f.at);
        latest.is_some_and(|f| {
            f.waits_for.as_ref().is_some_and(|block| {
                !(records.written.iter()).any(|w| w.block == *block && w.by == decide::Outsider::Outside && w.at > f.at)
            })
        })
    });
    if records.cancelled().is_some() || merged || under_way || calling || asking || outside {
        return Vec::new();
    }
    let Some(i) = reading.standing(&flow.flow) else {
        return Vec::new();
    };
    let cell = &cells[i];
    if reading.pending(cell, now) {
        return Vec::new();
    }
    let unmet: Vec<Step> = (cell.requires.iter())
        .filter(|key| !reading.key_holds(&flow.flow, key, i))
        .map(|condition| Step::Waits {
            cell: cell.id.clone(),
            on: Wait::Precondition(Some(key(condition))),
        })
        .collect();
    // A precondition only cells can make hold, none of which runs, is a stall, which only the engine's view shows.
    if !unmet.is_empty() {
        let made = (cell.requires.iter())
            .filter(|key| !reading.key_holds(&flow.flow, key, i))
            .any(|key| made_from_outside(&flow.flow, &reading, key));
        return if made { unmet } else { Vec::new() };
    }
    if records
        .started
        .iter()
        .any(|s| s.cell == cell.id && open(records, s.run))
    {
        return Vec::new();
    }
    let because: Vec<decide::Cause> = (reading.entering(&flow.flow, cell, now).into_iter())
        .map(|reason| match reason {
            Reason::Changed { block, was } => decide::Cause::Changed {
                behind: reading.behind(&flow.flow, i, &block, now),
                block,
                was,
            },
            Reason::Results { calls } => decide::Cause::Results { calls },
        })
        .collect();
    if because.is_empty() {
        return Vec::new();
    }
    let senders = reading.sent_by(&flow.flow, i, &because, now).unwrap_or_default();
    let limit =
        (senders.into_iter()).find(|sender| reading.sent_back_since(&flow.flow, i, sender, now) >= cell.again as usize);
    limit
        .map(|sender| Step::Waits {
            cell: cell.id.clone(),
            on: Wait::Limit(Some(sender)),
        })
        .into_iter()
        .collect()
}

/// Whether a person or a source outside the work writes a block that keeps this key, which does not hold, from holding:
/// one without the value a test of every block looks for, any a test of one looks for, one with the value a test of
/// none looks for; or the block a key of a value reads.
fn made_from_outside(flow: &Flow, reading: &Reading, key: &Condition) -> bool {
    let keeping: Vec<&BlockId> = match key {
        Condition::Is { block, .. } => vec![block],
        Condition::Test(name) => {
            let test = &flow.tests()[name];
            let has = |block: &BlockId| match (test.kind, test.is.as_deref()) {
                (TestKind::AnyContent, _) => reading.current(block).is_some_and(|v| !v.empty),
                (_, None) => false,
                (_, Some(is)) => match reading.current(block) {
                    Some(v) => v.value.as_deref() == Some(is),
                    None => flow.default_of(block) == Some(is),
                },
            };
            match test.kind {
                TestKind::All => test.reads.iter().filter(|block| !has(block)).collect(),
                TestKind::Any | TestKind::AnyContent => test.reads.iter().collect(),
                TestKind::None | TestKind::NoneStanding => test.reads.iter().filter(|block| has(block)).collect(),
            }
        }
    };
    (keeping.into_iter()).any(|block| flow.outsiders(block).is_some_and(|o| o.people || o.outside.is_some()))
}
