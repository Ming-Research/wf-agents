//! Agent rounds that do not end in a return the cell takes, handled as the flow's rounds section says, which gives
//! every agent's cell a shared row for each. A return sent back runs the cell again, and so many in a row pause the
//! work. A run that ends without a return is told apart by what the command line reports, and is waited out for every
//! work, run again until it passes, or pauses the work; it never counts as a return sent back, and never asks for a
//! written answer.

use std::time::Duration;

use db::Actor;
use db::rusqlite::params;
use flow::format::{Failure, Handle, Rounds};
use flow::{CompiledCell, CompiledGraph};
use serde_json::json;

use crate::agent::{self, Reply};
use crate::work::{self, Ended, Round, RoundId, Work};
use crate::{Advance, Engine, actions};

/// Why the engine starts no new round for any work, and until when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Halt {
    /// The kind of failed run, as the flow names it.
    pub kind: String,
    /// Until when, in milliseconds since the epoch.
    pub until: i64,
    /// What the command line reported.
    pub why: String,
}

/// How long the engine waits, each time a cause it does not know the end of is found to last (the network is down,
/// the login has expired), before it lets one round find out whether it has passed.
const PROBE: i64 = 30_000;

/// The longest a run that failed in passing waits before it runs again.
const RETRY_MAX: Duration = Duration::from_secs(300);

/// How long the engine waits for a usage limit whose reset the command line did not say in words it reads.
const UNKNOWN_RESET: i64 = 1_800_000;

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// A time in milliseconds since the epoch, as the owner reads it: the day and minute in UTC.
pub fn utc(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rest = secs.rem_euclid(86_400);
    // The civil date of a day count from 1970-01-01, as Howard Hinnant's days_from_civil inverts it.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02} UTC", rest / 3600, rest % 3600 / 60)
}

/// The rounds section of the flow a work runs on. A flow version stored before the flow said it runs by the section
/// of the flow built into the engine.
pub fn section(e: &mut Engine, g: &CompiledGraph) -> Result<Rounds, String> {
    if let Some(r) = &g.rounds {
        return Ok(r.clone());
    }
    let now = e.flow_now;
    e.flow(now)?
        .graph(crate::ENGINEER)
        .and_then(|x| x.rounds.clone())
        .ok_or_else(|| "the flow has no rounds section".to_string())
}

/// Whether a round was cut off by the engine or ended without a return: neither is a return sent back.
pub fn not_a_return(r: &Round) -> bool {
    r.rejected
        .as_ref()
        .is_some_and(|x| x.get("interrupted").is_some() || x.get("failed").is_some() || x.get("error").is_some())
}

/// The returns of a cell sent back in a row, newest first, from its rounds newest first: rounds cut off and failed
/// runs are passed over, and a round that started before the owner last let the work go on ends the row.
pub fn sent_back(at_cell: &[Round], resumed: Option<i64>) -> Vec<&Round> {
    at_cell
        .iter()
        .filter(|r| !not_a_return(r))
        .take_while(|r| r.rejected.is_some() && resumed.is_none_or(|t| r.started_at > t))
        .collect()
}

fn kind_of<'s>(s: &'s Rounds, name: &str) -> Option<&'s Failure> {
    s.failed.kinds.iter().find(|k| k.name == name)
}

/// The failed runs of a cell in a row whose kind is run again, newest first: since the cell last ran to a return,
/// and since the owner last let the work go on. Rounds the engine cut off are passed over.
fn retried<'r>(at_cell: &'r [Round], resumed: Option<i64>, s: &Rounds) -> Vec<&'r Round> {
    at_cell
        .iter()
        .filter(|r| r.rejected.as_ref().is_none_or(|x| x.get("interrupted").is_none()))
        .take_while(|r| resumed.is_none_or(|t| r.started_at > t))
        .take_while(|r| {
            r.rejected
                .as_ref()
                .and_then(|x| x["failed"]["kind"].as_str())
                .and_then(|k| kind_of(s, k))
                .is_some_and(|k| k.handle == Handle::Retry)
        })
        .collect()
}

/// While the cell's last run, which failed, waits to run again, why; none when the cell's round may run now. Each
/// time it runs again it waits twice as long as the time before, up to [`RETRY_MAX`].
pub fn waiting(retry_wait: Duration, at_cell: &[Round], resumed: Option<i64>, s: &Rounds) -> Option<String> {
    let fails = retried(at_cell, resumed, s);
    let last = fails.first()?;
    let n = (fails.len() as u32).saturating_sub(1).min(10);
    let wait = retry_wait.saturating_mul(1 << n).min(RETRY_MAX).as_millis() as i64;
    let due = last.ended_at.unwrap_or(last.started_at) + wait;
    (db::now() < due).then(|| {
        format!(
            "{} ended without a return; the cell runs again at {}",
            last.id,
            utc(due)
        )
    })
}

/// Pauses a work and tells the owner why; once the owner lets it go on, its cell runs again.
pub fn pause(e: &mut Engine, w: &Work, why: &str) -> Result<Advance, String> {
    let text = match e.subject(w)? {
        Some(item) => format!("Work {} on item {} ({}) is paused: {why}", w.id, item.id, item.title),
        None => format!("Work {} (a wake of the architect) is paused: {why}", w.id),
    };
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE works SET paused = ?2, paused_by = 'engine' WHERE id = ?1",
            params![w.id, why],
        )?;
        actions::notify(tx, Some(w.id), &text).map_err(db::Error::Abort)
    })
    .map_err(sql)?;
    (e.say)(&format!("work {} paused: {why}", w.id));
    Ok(Advance::Stopped)
}

/// What a round whose run ended without a return was, and what came of it.
pub struct Failed<'a> {
    pub w: &'a Work,
    pub cell: &'a CompiledCell,
    pub round: RoundId,
    pub section: &'a Rounds,
}

impl Failed<'_> {
    /// Ends the round, recording which kind of failed run it was, and does what the flow says for that kind.
    pub fn handle(&self, e: &mut Engine, ended: Ended, reply: &Reply) -> Result<Advance, String> {
        let what = reply
            .error
            .clone()
            .unwrap_or_else(|| "the round handed in nothing".to_string());
        let now = db::now();
        let name = agent::failure_kind(&what);
        let kind = kind_of(self.section, name)
            .or_else(|| kind_of(self.section, "transient"))
            .cloned()
            .ok_or("the flow's rounds section names no kind of failed run")?;
        let until = match (kind.handle, kind.name.as_str()) {
            (Handle::Wait, "usage_limit") => {
                Some(agent::resets_at(&what, now, &agent::zone_offset).unwrap_or(now + UNKNOWN_RESET))
            }
            (Handle::Wait, _) => Some(now + PROBE),
            _ => None,
        };
        let mut failed = json!({ "kind": kind.name, "what": what });
        if let Some(t) = until {
            failed["until"] = json!(t);
        }
        (e.say)(&format!(
            "{} ended without a return ({}): {what}",
            self.round, kind.name
        ));
        let ended = Ended {
            rejected: Some(json!({ "failed": failed })),
            transition: None,
            ..ended
        };
        let round = self.round;
        e.db.write(Actor::Engine, |tx| {
            work::finish_round(tx, round, &ended).map_err(db::Error::Abort)
        })
        .map_err(sql)?;
        match kind.handle {
            Handle::Wait => {
                let until = until.unwrap_or(now);
                let first = e.told.as_deref() != Some(kind.name.as_str());
                e.halt = Some(Halt {
                    kind: kind.name.clone(),
                    until,
                    why: what.clone(),
                });
                if first {
                    e.told = Some(kind.name.clone());
                    let text = if kind.name == "usage_limit" {
                        format!(
                            "No agent round starts for any work until {}: the command line reports {} ({what}). Each work goes on from its cell then.",
                            utc(until),
                            kind.name
                        )
                    } else {
                        format!(
                            "No agent round starts for any work while the command line reports {} ({what}): {} One round tries every half minute, and each work goes on from its cell once one runs.",
                            kind.name, kind.desc
                        )
                    };
                    let w = self.w.id;
                    e.db.write(Actor::Engine, |tx| {
                        actions::notify(tx, Some(w), &text).map_err(db::Error::Abort)
                    })
                    .map_err(sql)?;
                }
                Ok(Advance::Waits(format!("{} until {}", kind.name, utc(until))))
            }
            // The cell runs again once its wait is over, until it passes.
            Handle::Retry => Ok(Advance::Moved),
            Handle::Pause => {
                let why = format!(
                    "cell {}: the agent's run ended without a return ({}): {what}",
                    self.cell.id, kind.name
                );
                pause(e, self.w, &why)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_reads_as_its_day_and_minute() {
        assert_eq!(utc(0), "1970-01-01 00:00 UTC");
        assert_eq!(utc(1_792_962_000_000), "2026-10-25 21:00 UTC");
        assert_eq!(utc(951_782_400_000), "2000-02-29 00:00 UTC");
    }

    // Each time a passing failure runs again it waits twice as long, up to five minutes.
    #[test]
    fn a_passing_failure_waits_longer_each_time_up_to_five_minutes() {
        let relations = db::relations(db::SCHEMA).unwrap();
        let tables = db::index_of(db::SCHEMA).unwrap();
        let f = flow::compile(flow::FLOW, &flow::Context { relations, tables }).unwrap();
        let section = f.graph(crate::ENGINEER).unwrap().rounds.clone().unwrap();
        let ended = db::now();
        let rounds = |n: i64| -> Vec<Round> {
            (0..n)
                .map(|i| Round {
                    id: RoundId::new(1, n - i),
                    cell: Some("R1".into()),
                    session: None,
                    input: json!({}),
                    output: None,
                    transition: None,
                    rejected: Some(json!({ "failed": { "kind": "transient", "what": "529" } })),
                    compactions: 0,
                    started_at: ended,
                    ended_at: Some(ended),
                    usage: None,
                })
                .collect()
        };
        let due = |n: i64| waiting(Duration::from_secs(10), &rounds(n), None, &section).unwrap();
        assert!(due(1).ends_with(&utc(ended + 10_000)));
        assert!(due(2).ends_with(&utc(ended + 20_000)));
        assert!(due(9).ends_with(&utc(ended + 300_000)));
    }
}
