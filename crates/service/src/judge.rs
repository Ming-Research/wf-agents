//! The judge of a play (design document `testing`, 14): it reads a work's records against the path its script states
//! (module `script::items`), by the reading of the records the histories and the exploration judge by (crate
//! `reading`) and never by the deciding code the play tests, and names the first step that left the path, with what
//! the records hold there.
//!
//! The records hold, in the order of their places, each step of a path but a few waits: a run of a cell started; a
//! round that failed, with its kind; a return sent back, which the run's next round is given; a call a run rests at,
//! for a tool, the engine's wait before a round is tried again, or the owner's word on the engine's own call at a run's
//! limit; a question that waits for the owner's answer; the work paused and let on, by whom; a cell let go at its
//! limit; a cancel, a reopen, and the merge's finished run. A wait on a precondition, for the outside or at a cell's
//! limit of a sender has no entry: it is read where the work came to rest after a thing that happened to it, with
//! nothing of it under way and no call or question waited for, at the cell the reading finds it stands at. There a key
//! of the cell's preconditions that does not hold is a wait for the outside where only cells write the blocks keeping
//! it and the latest run of each said that what it wrote waits for a source outside the work (decision
//! what-a-run-wrote-may-wait-for-the-outside), a precondition where a person or a source outside the work writes one of
//! them, and else a stall, which the engine's view shows; with every key holding, the cell waits at its limit where it
//! would run once more for what a sender wrote that sent it back as often as the flow's `again` lets one since its
//! counts last began anew (decision the-same-sender-again). A wait that lasts over several things that happened is one
//! step. The work's adding (the pause it was added with, the write of its block of dependencies and of what people
//! wrote for it) and the letting on a play gives it then are the item's start: no step of its path, and no rest.
//!
//! What no record holds is read from the engine's view of the work now, and so only as a path's end: a stall, which
//! the reading bears out where it finds the work stalled at the same cell; a wait for a slot, for the turn to check and
//! merge, and for what every work shares, which are kept in memory only (decisions slots-in-the-scheduler,
//! one-work-checks-and-merges-at-a-time and the-engine-checks-the-outside-itself); and the pause of everything, which
//! the installation keeps. A round under way, which the records hold, is a path's end too.

use std::collections::BTreeSet;
use std::fmt;

use decide::{BlockId, CellId, Condition, Decision, End, Flow, Pauser, Records, RunId, Seq, Waited};
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

/// How a work began as its item was added, which is the item's start and no step of its path: the driver adds it
/// paused, writes its block of dependencies where its flow has one and what people write for it, each a thing that
/// happened to it, and looks at it, which begins nothing under the pause; a play then lets it on, unless it is held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Added {
    /// How many blocks people wrote for it as it was added.
    pub writes: usize,
    /// Whether it is held, never let on as it was added.
    pub held: bool,
}

impl Added {
    /// How many things that happened to a work on this flow its adding was: its pause, the write of its block of
    /// dependencies where the flow has one, and each of its writes.
    fn moments(&self, flow: &Compiled) -> usize {
        let dependencies = (flow.blocks.values()).any(|b| b.writers.outside.as_deref() == Some(driver::DEPENDENCIES));
        1 + usize::from(dependencies) + self.writes
    }
}

/// A step the records hold, with whether the work still stands at it as they end: a wait that has not ended, a pause
/// not lifted, a cancel that stands; any other step stands once taken.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seen {
    pub step: Step,
    pub stands: bool,
}

/// How the work's records stand against its path, the work having been added as `added` says; `shown` is what the
/// engine shows of the work now, for the end the records do not hold.
pub fn judge(flow: &Compiled, work: &Work, added: Added, path: &[Step], shown: &Shown) -> Judged {
    let seen = seen(flow, work, added);
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
            // A stall is the walk's own conclusion, which the records bear out where the reading finds the work at
            // rest at that cell and nothing there that a person, the outside or a cell that runs could give.
            let shown = |what: String| {
                Err(Some(format!(
                    "{what}, while the engine shows the work stalled at {cell}"
                )))
            };
            return match at_rest(flow, records) {
                Rest::Stalled(at) if at == *cell => Ok(()),
                Rest::Stalled(at) => shown(format!("a stall at {at}")),
                Rest::Waits(steps) => shown((steps.iter().map(Step::to_string)).collect::<Vec<_>>().join(" and ")),
                Rest::Nothing(Some(at)) => shown(format!("{at} standing to go on")),
                Rest::Nothing(None) => shown("nothing that holds the work".to_string()),
            };
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
/// everything. An act waits for one of these but for the turn, the wait every work shares and the pause of everything,
/// which a path names only as its end (module `script::items`).
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
                    // The owner's word on the engine's own call at a run's limit: no carrier makes a call of a person
                    // else, a script asking the owner by its questions.
                    Waited::Call { call, on: Some(_) } => match work.calls.get(call) {
                        Some(Call::AtLimit(_)) => Wait::Limit(None),
                        _ => continue,
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

/// The steps the work's records hold, in order (the module's own words), the work having been added as `added` says.
pub fn seen(flow: &Compiled, work: &Work, added: Added) -> Vec<Seen> {
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
                        failure: Some(*failure),
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
            Some(Call::AtLimit(_)) => Wait::Limit(None),
            Some(Call::Wait { .. }) => Wait::Retry,
            // No carrier makes a call of a person, a script asking the owner by its questions: a play's records hold
            // none.
            Some(Call::Asked(Ask::Person { .. })) | None => continue,
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
        let waited = (before.get(&q.name)).is_some_and(|b| b.waits && !b.withdrawn && !b.answered);
        if !q.waits || q.withdrawn || waited {
            continue;
        }
        let stands = (now.get(&q.name)).is_some_and(|n| n.waits && !n.withdrawn && !n.answered);
        let on = Wait::Answer(Some(q.name.clone()));
        push(
            q.at,
            Step::Waits {
                cell: q.cell.clone(),
                on,
            },
            stands,
        );
    }
    // The pause the work was added with, and the letting on that follows it where the item is not held, are its start.
    let mut pauses: Vec<&decide::Pause> = r.pauses.iter().collect();
    pauses.sort_by_key(|p| p.at);
    let latest = pauses.last().map(|p| p.at);
    let start = match pauses.get(1) {
        Some(let_on) if !added.held && !let_on.paused => 2,
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

    // The waits no entry records, read where the work came to rest after each thing that happened to it from the end of
    // its adding on: within the adding, no cell had what it was added with yet.
    let mut froms: Vec<Seq> = work.moments.iter().map(|m| m.from).collect();
    froms.sort();
    let mut ends: Vec<Seq> = froms.into_iter().skip(added.moments(flow)).collect();
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
        let resting = match at_rest(flow, &r.before(end)) {
            Rest::Waits(steps) => steps,
            Rest::Stalled(_) | Rest::Nothing(_) => Vec::new(),
        };
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

/// How the work rests as these records stand, by the reading.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Rest {
    /// Nothing no entry records holds it: something of it is under way, a call or a question waits, it has ended, or
    /// it would go on, from the cell where it stands where it stands at one.
    Nothing(Option<CellId>),
    /// It waits at the cell it stands at for these, which no entry records.
    Waits(Vec<Step>),
    /// It can go on by itself no further at this cell, which only the engine's view says.
    Stalled(CellId),
}

/// How the work rests as these records stand (decisions conditions-and-preconditions,
/// what-a-run-wrote-may-wait-for-the-outside, the-same-sender-again): where nothing of it is under way and no call or
/// question waits, at the cell the reading finds it stands at, each key of that cell's preconditions that does not
/// hold is a wait for the outside where only cells write the blocks keeping it and the latest run of each said what it
/// wrote waits for a source outside the work, of which nothing new has come, and a precondition otherwise; where only
/// cells write what keeps each, none of which waits so, the work is stalled there. With no key unmet, the cell's limit
/// of the first sender that sent it back as often as its limit lets one, where it would run once more for what that
/// sender wrote. With no cell to stand at, it is stalled at its last cell, which nothing new runs.
fn at_rest(flow: &Compiled, records: &Records) -> Rest {
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
    let standing = reading.standing(&flow.flow);
    let at = standing.map(|i| cells[i].id.clone());
    if records.cancelled().is_some() || merged || under_way || calling || asking {
        return Rest::Nothing(at);
    }
    let Some(i) = standing else {
        return Rest::Stalled(cells[cells.len() - 1].id.clone());
    };
    let cell = &cells[i];
    if reading.pending(cell, now) {
        return Rest::Nothing(at);
    }
    let unmet: Vec<&Condition> = (cell.requires.iter())
        .filter(|key| !reading.key_holds(&flow.flow, key, i))
        .collect();
    if !unmet.is_empty() {
        let mut waits: Vec<Step> = Vec::new();
        let mut stalled = true;
        for condition in unmet {
            let keeping = reading.keeping(&flow.flow, condition, i);
            let outsiders =
                (keeping.iter()).any(|b| (flow.flow.outsiders(b)).is_some_and(|o| o.people || o.outside.is_some()));
            let outside = if outsiders {
                None
            } else {
                outside(&flow.flow, &reading, &keeping)
            };
            match outside {
                Some(blocks) => {
                    stalled = false;
                    for block in blocks {
                        let wait = Step::Waits {
                            cell: cell.id.clone(),
                            on: Wait::Outside(Some(block)),
                        };
                        if !waits.contains(&wait) {
                            waits.push(wait);
                        }
                    }
                }
                None => {
                    stalled &= !outsiders;
                    waits.push(Step::Waits {
                        cell: cell.id.clone(),
                        on: Wait::Precondition(Some(key(condition))),
                    });
                }
            }
        }
        return match stalled {
            true => Rest::Stalled(cell.id.clone()),
            false => Rest::Waits(waits),
        };
    }
    if records
        .started
        .iter()
        .any(|s| s.cell == cell.id && open(records, s.run))
    {
        return Rest::Nothing(at);
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
    let senders = (!because.is_empty())
        .then(|| reading.sent_by(&flow.flow, i, &because, now))
        .flatten()
        .unwrap_or_default();
    let limit =
        (senders.into_iter()).find(|sender| reading.sent_back_since(&flow.flow, i, sender, now) >= cell.again as usize);
    match limit {
        Some(sender) => Rest::Waits(vec![Step::Waits {
            cell: cell.id.clone(),
            on: Wait::Limit(Some(sender)),
        }]),
        None => Rest::Nothing(at),
    }
}

/// Of the blocks keeping a precondition from holding, which only cells write, the block each such cell's latest
/// finished run wrote among them, where that run said what it wrote waits for a new version of a block a source
/// outside the work writes and none has come new to the cell (decision what-a-run-wrote-may-wait-for-the-outside):
/// the blocks the work waits for that source by. None where any of those cells' latest runs waits for nothing so, or
/// no cell writes them.
fn outside(flow: &Flow, reading: &Reading, keeping: &[&BlockId]) -> Option<Vec<BlockId>> {
    let now = Seq(u64::MAX);
    let mut makers: Vec<&CellId> = Vec::new();
    for block in keeping {
        for writer in flow.writers(block) {
            if !makers.contains(&&writer.id) {
                makers.push(&writer.id);
            }
        }
    }
    if makers.is_empty() {
        return None;
    }
    (makers.into_iter())
        .map(|cell| {
            let latest = (reading.finished.values())
                .filter(|f| reading.started[&f.run].cell == *cell)
                .max_by_key(|f| f.at)?;
            let waits = latest.waits_for.as_ref()?;
            flow.outsiders(waits)?.outside.as_ref()?;
            if !reading.new_to(cell, waits, now).is_empty() {
                return None;
            }
            (latest.writes.iter())
                .find(|w| keeping.contains(&&w.block))
                .map(|w| w.block.clone())
        })
        .collect()
}
