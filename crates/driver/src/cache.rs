//! A round that missed its session's prompt cache, as the work's records tell it. A round that goes on in its session
//! within the cache's lifetime since the session's last call reads from the cache, at its first call, what the session
//! held; one whose first call read less than half of it paid for writing the rest again, which the cache should have
//! held ([`missed`], by which a fork of the session is judged too). What a round wrote to the cache is no measure of a
//! miss: it is also what the session gained since, the answer of the round before and the round's own message, which
//! after a round of tools is large.
//!
//! A round whose session the program compacted before its first call is no miss: the session it goes on in is a
//! summary, and the cache holds nothing of what was cut ([`compacted_before_its_first_call`]), as it holds nothing of a
//! session after the cache's lifetime, which keep-warm pings keep from happening while a line waits
//! ([`crate::warm`]). Of a miss, [`Cause`] says why, from the records alone (principle `state-from-records`). A miss
//! after what comes before the session's messages in the prompt changed since the call before (the model, the account,
//! whose cache is its own, the program's version, the system prompt, the schema, and the tools and agents the program
//! says it gave the model) is what the agents' program does by design, and is expected. Where none changed and both
//! calls hold the program's own report, it is a fault of wf-agents, whichever project it came up in. Where the records
//! do not say, it is of unknown cause: the call before has no measures, or none of the program's report, or the first
//! prompt is shorter than what the session held with no compaction recorded.

use std::collections::BTreeMap;

use decide::{CellId, RunId};
use flow::Compiled;
use work::Work;

use crate::history::last_prompt_by;

/// Parts of what comes before the messages that rows kept earlier hold and that are no cause of a miss: the effort,
/// which caused no miss in the measured changes of it alone, and the digest of the arguments the engine gave the
/// program, which is not what the program gave the model (that is kept as `program_tools`). They are never compared, so
/// that a row kept earlier is read as it was written (research/cache-miss-causes.md).
const NOT_CAUSES: [&str; 2] = ["effort", "tools"];

/// The part whose presence in both calls says each holds the program's own report of what it gave the model.
const REPORT: &str = "program_tools";

/// The parts of that report: compared only where both calls hold it.
const REPORTED: [&str; 2] = ["program_tools", "program_agents"];

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
    /// Why it missed.
    pub cause: Cause,
}

/// Why a round missed its session's prompt cache.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cause {
    /// These parts of what comes before the session's messages differ from the call before, by name: what the agents'
    /// program does by design.
    Differs(Vec<String>),
    /// The records do not say: the call before has no measures, or either call holds none of the program's report of
    /// what it gave the model, or the first prompt is shorter than what the session held and no compaction is
    /// recorded.
    Unknown,
    /// Both calls hold the program's report and nothing before the messages differs: a fault of wf-agents.
    Fault,
}

impl Miss {
    /// The tokens of what the session held that the round wrote to the cache again.
    pub fn written(&self) -> u64 {
        self.held.saturating_sub(self.read)
    }

    /// Whether the miss is one the agents' program makes by design: what comes before the messages changed.
    pub fn expected(&self) -> bool {
        matches!(self.cause, Cause::Differs(_))
    }

    /// Whether the miss is a fault of wf-agents: the records hold what both calls were given and nothing differs.
    pub fn fault(&self) -> bool {
        self.cause == Cause::Fault
    }
}

/// Whether a call that went on in a session, a round's or a fork's, missed the session's prompt cache, where the cache
/// was alive: its first call read under half of the `held` tokens the session held. The one test for a round and for a
/// fork.
pub fn missed(read: u64, held: u64) -> bool {
    held > 0 && read.saturating_mul(2) < held
}

/// Whether the program compacted the session before the round's first call: the round's records say it compacted the
/// session (`compactions`) and its first prompt is shorter than what the session held. Neither alone says it: a
/// compaction later in the round leaves the first prompt at least what the session held, and a first prompt shorter
/// than that with no compaction recorded is of a cause the records do not say ([`Cause::Unknown`]).
pub fn compacted_before_its_first_call(compactions: u32, first_context: Option<u64>, held: u64) -> bool {
    compactions > 0 && cut_down(first_context, held)
}

/// Whether the round's first prompt is shorter than what its session held, which a session gone on in never is unless
/// what it held was cut: a resumed session's first prompt holds the session, the answer before and the new message.
fn cut_down(first_context: Option<u64>, held: u64) -> bool {
    first_context.is_some_and(|first| first < held)
}

/// Each round of the work that missed its session's prompt cache, which keeps a session `life_ms` after its last call,
/// in the order the rounds began.
pub fn misses(flow: &Compiled, work: &Work, life_ms: u64) -> Vec<Miss> {
    let line_of = |cell: &CellId| (flow.cells.iter()).find(|c| c.id == *cell).and_then(|c| c.line.clone());
    misses_by(&line_of, work, life_ms)
}

/// Each round of the work that missed its session's prompt cache ([`misses`]), the line of each run's cell as
/// `line_of` tells it, of the version the work ran on.
pub fn misses_by(line_of: &dyn Fn(&CellId) -> Option<String>, work: &Work, life_ms: u64) -> Vec<Miss> {
    let r = &work.records;
    let mut out = Vec::new();
    let mut counted: Vec<RunId> = Vec::new();
    for round in &r.rounds {
        counted.push(round.run);
        let number = counted.iter().filter(|x| **x == round.run).count();
        let Some(line) = line_of_run(line_of, work, round.run) else {
            continue;
        };
        let line = line.as_str();
        let Some(measures) = work::measured::of_round(&work.measured, round.run, number) else {
            continue;
        };
        let held = last_prompt_by(line_of, work, line, round.run, number);
        let (Some(read), Some(held)) = (measures.first_read, held) else {
            continue;
        };
        if !missed(read, held) {
            continue;
        }
        let compactions = work
            .spent
            .get(&(round.run, number))
            .map_or(0, |spent| spent.compactions);
        if compacted_before_its_first_call(compactions, measures.first_context, held) {
            continue;
        }
        let Some((last, before)) = last_call(line_of, work, line, round.run, number) else {
            continue;
        };
        let begun = work::sessions::first_try_at(work, round.run, number);
        let (Some(last), Some(began)) = (work.time_of(last), work.time_of(begun)) else {
            continue;
        };
        let after_ms = began.0.saturating_sub(last.0);
        if after_ms >= life_ms {
            continue;
        }
        out.push(Miss {
            run: round.run,
            round: number,
            after_ms,
            read,
            held,
            cause: cause(
                before.as_ref(),
                &measures.before,
                cut_down(measures.first_context, held),
            ),
        });
    }
    out
}

/// Why a round missed the cache, given what came before the messages in the call before (none where that call has no
/// measures) and in the round's own, and whether its first prompt was shorter than what the session held with no
/// compaction recorded.
fn cause(before: Option<&BTreeMap<String, String>>, now: &BTreeMap<String, String>, shorter: bool) -> Cause {
    let Some(before) = before else {
        return Cause::Unknown;
    };
    // The program's report is compared only where both calls hold it: a call kept before it was kept says nothing.
    let reported = before.contains_key(REPORT) && now.contains_key(REPORT);
    let mut differs: Vec<String> = before
        .keys()
        .chain(now.keys())
        .filter(|name| !NOT_CAUSES.contains(&name.as_str()))
        .filter(|name| reported || !REPORTED.contains(&name.as_str()))
        .filter(|name| before.get(*name) != now.get(*name))
        .cloned()
        .collect();
    differs.sort();
    differs.dedup();
    match (differs.is_empty(), reported && !shorter) {
        (false, _) => Cause::Differs(differs),
        (true, true) => Cause::Fault,
        (true, false) => Cause::Unknown,
    }
}

/// The place of the session's last call before the round of this number of the run, on the line: the end of its
/// latest round before it, or of its latest ping since that did not fail, which ran that round's command line; and
/// what came before the messages in that round's prompt, where it said. None where the round begins its session.
#[allow(clippy::type_complexity)]
fn last_call(
    line_of: &dyn Fn(&CellId) -> Option<String>,
    work: &Work,
    line: &str,
    run: RunId,
    number: usize,
) -> Option<(decide::Seq, Option<BTreeMap<String, String>>)> {
    let on_line = |r: RunId| line_of_run(line_of, work, r).as_deref() == Some(line);
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

/// The line of the run's cell, as `line_of` tells it.
fn line_of_run(line_of: &dyn Fn(&CellId) -> Option<String>, work: &Work, run: RunId) -> Option<String> {
    let started = work.records.started.iter().find(|s| s.run == run)?;
    line_of(&started.cell)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The one test of a miss, for a round and for a fork: under half of what the session held was read. Half is not a
    // miss, and a session that held nothing has nothing to miss.
    #[test]
    fn a_call_missed_its_sessions_cache_if_it_read_under_half_of_what_the_session_held() {
        assert!(missed(0, 10_000));
        assert!(missed(4_999, 10_000));
        assert!(!missed(5_000, 10_000));
        assert!(!missed(9_000, 10_000));
        assert!(!missed(10_000, 10_000));
        assert!(!missed(0, 0), "nothing was held");
        assert!(!missed(u64::MAX, u64::MAX), "no overflow");
    }

    // The shape of work 35 run 36: the round compacted once and its first prompt (49462) was far under what the
    // session held (461234). Each of the two alone is not it.
    #[test]
    fn a_session_was_compacted_before_the_first_call_where_it_is_recorded_and_the_first_prompt_is_shorter() {
        assert!(compacted_before_its_first_call(1, Some(49_462), 461_234));
        assert!(
            !compacted_before_its_first_call(1, Some(461_300), 461_234),
            "a compaction later in the round leaves the first prompt as long as what was held"
        );
        assert!(
            !compacted_before_its_first_call(1, Some(461_234), 461_234),
            "as long as what was held is not shorter"
        );
        assert!(
            !compacted_before_its_first_call(0, Some(49_462), 461_234),
            "a shorter prompt with no compaction recorded is not known to be one"
        );
        assert!(
            !compacted_before_its_first_call(1, None, 461_234),
            "no first prompt, none shorter"
        );
    }

    fn parts(list: &[(&str, &str)]) -> BTreeMap<String, String> {
        list.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    // Why a round missed: what differs is named; where nothing does, both calls must hold the program's report for
    // the miss to be a fault; a row kept earlier holds the effort and the arguments' digest, which are no cause.
    #[test]
    fn a_miss_is_told_by_what_differs_and_is_a_fault_only_where_both_calls_hold_the_programs_report() {
        let report = [("program_tools", "t1"), ("program_agents", "a1"), ("model", "m")];
        let now = parts(&report);
        assert_eq!(cause(Some(&now), &now, false), Cause::Fault);
        // The shape of work 23 run 15: the program's tool list lost WebFetch.
        let later = parts(&[("program_tools", "t2"), ("program_agents", "a2"), ("model", "m")]);
        assert_eq!(
            cause(Some(&now), &later, false),
            Cause::Differs(vec!["program_agents".to_string(), "program_tools".to_string()])
        );
        // The effort and the old digest of the arguments are never compared.
        let old = parts(&[
            ("effort", "high"),
            ("tools", "x"),
            ("program_tools", "t1"),
            ("program_agents", "a1"),
            ("model", "m"),
        ]);
        let other = parts(&[
            ("effort", "xhigh"),
            ("tools", "y"),
            ("program_tools", "t1"),
            ("program_agents", "a1"),
            ("model", "m"),
        ]);
        assert_eq!(
            cause(Some(&old), &other, false),
            Cause::Fault,
            "a change of effort alone is no cause"
        );
        // A part the engine says differs is named whatever the report holds.
        let account = parts(&[
            ("program_tools", "t1"),
            ("program_agents", "a1"),
            ("model", "m"),
            ("account", "work"),
        ]);
        assert_eq!(
            cause(Some(&now), &account, false),
            Cause::Differs(vec!["account".to_string()])
        );
        // A call kept before the report was kept says nothing of it: the miss is of unknown cause, and the report is
        // not named as the difference.
        let kept_earlier = parts(&[("tools", "x"), ("model", "m")]);
        assert_eq!(cause(Some(&kept_earlier), &now, false), Cause::Unknown);
        assert_eq!(cause(Some(&now), &kept_earlier, false), Cause::Unknown);
        assert_eq!(
            cause(Some(&kept_earlier), &parts(&[("model", "n")]), false),
            Cause::Differs(vec!["model".to_string()])
        );
        // A call before with no measures at all.
        assert_eq!(cause(None, &now, false), Cause::Unknown);
        // A first prompt shorter than what was held, with no compaction recorded.
        assert_eq!(cause(Some(&now), &now, true), Cause::Unknown);
    }
}
