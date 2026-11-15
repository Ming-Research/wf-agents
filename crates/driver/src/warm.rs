//! Keeping the sessions of lines that wait warm. The agents' program keeps a session's prompt in its cache a while after
//! its last call (an hour for `claude`); a round after that writes the whole session to the cache again. A write kept
//! an hour costs forty reads on Opus 5.5 and Sonnet 5.5, twenty on Haiku 5.5 and eighty on Fable 5.1, so a session read
//! once by a minimal message a little before the cache lets it go costs a small part of what its next round would pay
//! otherwise.
//!
//! So while a work that has not ended waits, the session of each of its lines that can come back is sent a keep-warm
//! ping `after_ms` after its last call, and again as long as the wait goes on, up to as many pings as cost, each
//! reading the session, no more than writing it again would beyond reading it, on its model ([`Warm::most`]). A line
//! can come back where a cell of it is where the work stands, or is reached from there by the data the cells write: a
//! ping of a line the work has passed for good would be paid for nothing. A ping that failed is tried again a minute
//! after, while the cache keeps the session.
//!
//! A ping is a job of its work: none is due while a round of the work is under way, on any line, and the loop holds
//! back the work's rounds while one of its pings is (crate `service`). What is due is worked out from the work's
//! records and the time alone: a session's last call is the end of the latest of its rounds that called the model, or
//! of its latest ping that did not fail, and each ping is recorded as it ends, so that after a stop a ping is given
//! again only where one is still due. Which pings are under way the driver keeps in memory, so that none is given
//! twice while it runs.

use std::collections::BTreeSet;

use decide::{CellId, Decision, RunId, decide};
use flow::Compiled;
use store::Item;
use work::{Time, Work};

use crate::sessions::{self, Limits, Policy};

/// How long after a ping that failed the next is tried, in milliseconds, while the cache still keeps the session: a
/// failure that passes is most often over in a minute, and a minute is a sixtieth of what the cache keeps.
pub const RETRY_MS: u64 = 60_000;

/// A ping due: the line, and which of its sessions.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Due {
    pub line: String,
    pub session: u32,
}

/// A line's session the work keeps warm: the ping it is due, when, and when the cache lets the session go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kept {
    pub due: Due,
    pub at: Time,
    pub life: Time,
}

/// Each line's session the work keeps warm as of `now`, in the order of the walk: none for a work that has ended, one
/// with a round under way, or with no policy of keeping sessions warm; and none whose cache has let it go by `now`,
/// which a ping would write again whole, for nothing.
pub fn kept(flow: &Compiled, work: &Work, item: &Item, policy: &Policy, limits: &Limits, now: Time) -> Vec<Kept> {
    let decision = decide(&flow.flow, &work.records);
    if matches!(
        decision,
        Decision::Ended(_) | Decision::Busy { .. } | Decision::Fault(_)
    ) || under_way(work)
    {
        return Vec::new();
    }
    let back = coming_back(flow, work, &decision);
    (lines(flow))
        .filter(|line| back.contains(*line))
        .filter_map(|line| next(flow, work, item, line, policy, limits))
        .filter(|kept| now < kept.life)
        .collect()
}

/// The pings due for the work at `now`: those [`kept`] whose time has come.
pub fn due(flow: &Compiled, work: &Work, item: &Item, policy: &Policy, limits: &Limits, now: Time) -> Vec<Due> {
    (kept(flow, work, item, policy, limits, now).into_iter())
        .filter(|kept| kept.at <= now)
        .map(|kept| kept.due)
        .collect()
}

/// Whether a round of the work has begun and not ended.
fn under_way(work: &Work) -> bool {
    work.records.rounds.len() > work.records.returned.len()
}

/// How a line's session stands for keeping it warm: the session, when its next ping is due, whether or not that time
/// has come, and when the cache lets the session go; none where it is not to be kept warm.
fn next(flow: &Compiled, work: &Work, item: &Item, line: &str, policy: &Policy, limits: &Limits) -> Option<Kept> {
    let warm = policy.warm.as_ref()?;
    let on_line = |run: RunId| line_of_run(flow, work, run) == Some(line);
    let session = work::sessions::number_at(work, line, work.tick());
    // The session's latest round that called the model, by the place it ended.
    let r = &work.records;
    let mut counted: Vec<RunId> = Vec::new();
    let mut last_round: Option<(decide::Seq, RunId, usize)> = None;
    for round in &r.rounds {
        counted.push(round.run);
        let k = counted.iter().filter(|x| **x == round.run).count();
        if !on_line(round.run) {
            continue;
        }
        let ended = (r.returned.iter()).filter(|e| e.run == round.run).nth(k - 1);
        let called = work
            .spent
            .get(&(round.run, k))
            .is_some_and(|s| s.cached_tokens + s.input_tokens > 0);
        let began = work::sessions::start(work, round.run, k);
        if let Some(ended) = ended
            && called
            && work::sessions::number_at(work, line, began) == session
            && last_round.is_none_or(|(at, ..)| at < ended.at)
        {
            last_round = Some((ended.at, round.run, k));
        }
    }
    let (last_round, run, k) = last_round?;
    // As many pings as the session's model makes worth it.
    let model = (work.ran.iter().rev()).find(|t| t.run == run && t.round == k)?;
    let most = warm.most.get(&model.on.model).copied().unwrap_or(0);
    let pings: Vec<&work::Pinged> = (work.pinged.iter())
        .filter(|p| p.line == line && p.session == session && p.at > last_round)
        .collect();
    if pings.len() >= most as usize {
        return None;
    }
    // The line is not to begin a new session at its next round, which holds nothing of this one.
    if sessions::due(flow, work, item, line, policy, limits.get(line)).is_some() {
        return None;
    }
    let read = pings.iter().filter(|p| p.failed.is_none()).map(|p| p.at).max();
    let last_call = work.time_of(read.unwrap_or(last_round).max(last_round))?;
    let mut at = Time(last_call.0.saturating_add(warm.after_ms));
    if let Some(failed) = pings.last().filter(|p| p.failed.is_some()) {
        at = at.max(Time(work.time_of(failed.at)?.0.saturating_add(RETRY_MS)));
    }
    let life = Time(last_call.0.saturating_add(warm.life_ms));
    let due = Due {
        line: line.to_string(),
        session,
    };
    (at < life).then_some(Kept { due, at, life })
}

/// The lines the work can come back to from where it stands: the lines of the cells where it stands, those of its open
/// runs and the one it runs or waits at next, or, where it stands at none, the cell it last ran; and of every cell a
/// block those cells write runs, and on, as far as the data reach.
fn coming_back(flow: &Compiled, work: &Work, decision: &Decision) -> BTreeSet<String> {
    let r = &work.records;
    let open = (r.started.iter())
        .filter(|s| !r.finished.iter().any(|f| f.run == s.run) && !r.abandoned.iter().any(|a| a.run == s.run))
        .map(|s| s.cell.clone());
    let mut at: Vec<CellId> = open.collect();
    match decision {
        Decision::Run { cell, .. } | Decision::GoOn { cell, .. } | Decision::Abandon { cell, .. } => {
            at.push(cell.clone())
        }
        Decision::Wait { at: cell, .. } => at.push(cell.clone()),
        _ => {}
    }
    if at.is_empty() {
        at.extend(r.started.last().map(|s| s.cell.clone()));
    }
    let cells = flow.flow.cells();
    let mut reached: Vec<CellId> = Vec::new();
    while let Some(cell) = at.pop() {
        if reached.contains(&cell) {
            continue;
        }
        let Some(place) = flow.flow.place(&cell) else {
            continue;
        };
        for output in &cells[place].outputs {
            at.extend(
                (cells.iter())
                    .filter(|c| c.runs_on.contains(output))
                    .map(|c| c.id.clone()),
            );
        }
        reached.push(cell);
    }
    (flow.cells.iter())
        .filter(|c| reached.contains(&c.id))
        .filter_map(|c| c.line.clone())
        .collect()
}

/// The lines of the flow's cells, each once, in the order of the walk.
fn lines(flow: &Compiled) -> impl Iterator<Item = &str> {
    let mut seen = Vec::new();
    for line in flow.cells.iter().filter_map(|cell| cell.line.as_deref()) {
        if !seen.contains(&line) {
            seen.push(line);
        }
    }
    seen.into_iter()
}

/// The line of the run's cell.
fn line_of_run<'a>(flow: &'a Compiled, work: &Work, run: RunId) -> Option<&'a str> {
    let started = work.records.started.iter().find(|s| s.run == run)?;
    (flow.cells.iter())
        .find(|c| c.id == started.cell)
        .and_then(|c| c.line.as_deref())
}
