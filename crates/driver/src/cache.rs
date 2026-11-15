//! A round that missed its session's prompt cache, as the work's records tell it. A round that goes on in its session
//! within the cache's lifetime since the session's last call reads from the cache, at its first call, what the session
//! held; one whose first call read less than half of it paid for writing the rest again, which the cache should have
//! held. What a round wrote to the cache is no measure of a miss: it is also what the session gained since, the answer
//! of the round before and the round's own message, which after a round of tools is large.
//!
//! A miss after what comes before the session's messages in the prompt changed since the call before (the model, the
//! effort, the program's version, the system prompt, the schema, the tools) is what the agents' program does by
//! design, and is expected; any other is a fault of wf-agents, whichever project it came up in. A round after the
//! cache's lifetime writes its session again whole by design too, and is no miss: keep-warm pings keep it from
//! happening while a line waits ([`crate::warm`]). Everything is worked out from the records alone (principle
//! `state-from-records`).

use decide::RunId;
use flow::Compiled;
use work::Work;

use crate::history::last_prompt;

/// A round's miss of its session's prompt cache.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Miss {
    pub run: RunId,
    /// Which round of the run, the first being one.
    pub round: usize,
    /// How long after its session's last call it began, in milliseconds.
    pub after_ms: u64,
    /// The tokens its first call read from the cache.
    pub read: u64,
    /// The tokens its session held: the last prompt before it.
    pub held: u64,
    /// The parts of what comes before the session's messages that differ from the call before, by name; none where
    /// none does, and the miss is a fault.
    pub differs: Vec<String>,
}

impl Miss {
    /// The tokens of what the session held that the round wrote to the cache again.
    pub fn written(&self) -> u64 {
        self.held.saturating_sub(self.read)
    }

    /// Whether the miss is one the agents' program makes by design: what comes before the messages changed.
    pub fn expected(&self) -> bool {
        !self.differs.is_empty()
    }
}

/// Each round of the work that missed its session's prompt cache, which keeps a session `life_ms` after its last call,
/// in the order the rounds began.
pub fn misses(flow: &Compiled, work: &Work, life_ms: u64) -> Vec<Miss> {
    let r = &work.records;
    let mut out = Vec::new();
    let mut counted: Vec<RunId> = Vec::new();
    for round in &r.rounds {
        counted.push(round.run);
        let number = counted.iter().filter(|x| **x == round.run).count();
        let Some(line) = line_of_run(flow, work, round.run) else {
            continue;
        };
        let Some(measures) = work::measured::of_round(&work.measured, round.run, number) else {
            continue;
        };
        let (Some(read), Some(held)) = (measures.first_read, last_prompt(flow, work, line, round.run, number)) else {
            continue;
        };
        if held == 0 || read.saturating_mul(2) >= held {
            continue;
        }
        let Some((last, before)) = last_call(flow, work, line, round.run, number) else {
            continue;
        };
        let begun = work::sessions::start(work, round.run, number);
        let (Some(last), Some(began)) = (work.time_of(last), work.time_of(begun)) else {
            continue;
        };
        let after_ms = began.0.saturating_sub(last.0);
        if after_ms >= life_ms {
            continue;
        }
        let differs = match before {
            Some(before) => {
                let names = before.keys().chain(measures.before.keys());
                let mut differs: Vec<String> = names
                    .filter(|name| before.get(*name) != measures.before.get(*name))
                    .cloned()
                    .collect();
                differs.sort();
                differs.dedup();
                differs
            }
            None => Vec::new(),
        };
        out.push(Miss {
            run: round.run,
            round: number,
            after_ms,
            read,
            held,
            differs,
        });
    }
    out
}

/// The place of the session's last call before the round of this number of the run, on the line: the end of its
/// latest round before it, or of its latest ping since that did not fail, which ran that round's command line; and
/// what came before the messages in that round's prompt, where it said. None where the round begins its session.
#[allow(clippy::type_complexity)]
fn last_call(
    flow: &Compiled,
    work: &Work,
    line: &str,
    run: RunId,
    number: usize,
) -> Option<(decide::Seq, Option<std::collections::BTreeMap<String, String>>)> {
    let on_line = |r: RunId| line_of_run(flow, work, r) == Some(line);
    let (session, begins) = work::sessions::of_round(work, line, on_line, run, number);
    if begins {
        return None;
    }
    let at = work::sessions::start(work, run, number);
    let r = &work.records;
    let mut latest: Option<(decide::Seq, RunId, usize)> = None;
    let mut counted: Vec<RunId> = Vec::new();
    for round in &r.rounds {
        counted.push(round.run);
        let k = counted.iter().filter(|x| **x == round.run).count();
        let began = work::sessions::start(work, round.run, k);
        let ended = (r.returned.iter()).filter(|e| e.run == round.run).nth(k - 1);
        if let Some(ended) = ended
            && on_line(round.run)
            && began < at
            && ended.at < at
            && work::sessions::wrote_into(work, round.run, k)
            && work::sessions::number_at(work, line, began) == session
            && latest.is_none_or(|(l, ..)| l < ended.at)
        {
            latest = Some((ended.at, round.run, k));
        }
    }
    let (ended, r, k) = latest?;
    let pinged = (work.pinged.iter())
        .filter(|p| p.line == line && p.session == session && p.failed.is_none() && p.at > ended && p.at < at)
        .map(|p| p.at)
        .max();
    let before = work::measured::of_round(&work.measured, r, k).map(|m| m.before.clone());
    Some((pinged.unwrap_or(ended), before))
}

/// The line of the run's cell.
fn line_of_run<'a>(flow: &'a Compiled, work: &Work, run: RunId) -> Option<&'a str> {
    let started = work.records.started.iter().find(|s| s.run == run)?;
    (flow.cells.iter())
        .find(|c| c.id == started.cell)
        .and_then(|c| c.line.as_deref())
}
