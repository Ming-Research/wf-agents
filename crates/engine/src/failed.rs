//! Agent rounds that do not end in a return the cell takes, handled as the flow's rounds section says. A return sent
//! back runs the cell again, and so many in a row pause the work. A run that ends without a return is told apart by
//! what the command line reports, and is waited out for every work, run again, or asked about; it never counts as a
//! return sent back.

use std::time::Duration;

use db::Actor;
use db::rusqlite::params;
use flow::format::{Failure, Handle, Rounds};
use flow::{CompiledCell, CompiledGraph};
use serde_json::json;

use crate::agent::{self, Reply};
use crate::work::{self, Ended, Round, Work};
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

/// How long the engine waits, the first time, before it lets one round find out whether the network is back; each
/// time it is still down, twice as long, up to [`PROBE_MAX`].
const PROBE: i64 = 60_000;
const PROBE_MAX: i64 = 900_000;

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
/// time it runs again it waits twice as long as the time before.
pub fn waiting(retry_wait: Duration, at_cell: &[Round], resumed: Option<i64>, s: &Rounds) -> Option<String> {
    let fails = retried(at_cell, resumed, s);
    let last = fails.first()?;
    let n = (fails.len() as u32).saturating_sub(1).min(10);
    let wait = retry_wait.saturating_mul(1 << n).as_millis() as i64;
    let due = last.ended_at.unwrap_or(last.started_at) + wait;
    (db::now() < due).then(|| {
        format!(
            "round {} ended without a return; the cell runs again at {}",
            last.id,
            utc(due)
        )
    })
}

/// What a round whose run ended without a return was, and what came of it.
pub struct Failed<'a> {
    pub w: &'a Work,
    pub g: &'a CompiledGraph,
    pub cell: &'a CompiledCell,
    pub round: i64,
    /// The cell's rounds before this one, newest first, and when the owner last let the work go on.
    pub at_cell: &'a [Round],
    pub resumed: Option<i64>,
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
            (Handle::Wait, _) => Some(now + (PROBE << e.probes.min(4)).min(PROBE_MAX)),
            _ => None,
        };
        let mut failed = json!({ "kind": kind.name, "what": what });
        if let Some(t) = until {
            failed["until"] = json!(t);
        }
        (e.say)(&format!(
            "round {} ended without a return ({}): {what}",
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
                if kind.name == "offline" {
                    e.probes += 1;
                }
                let first = e.told.as_deref() != Some(kind.name.as_str());
                e.halt = Some(Halt {
                    kind: kind.name.clone(),
                    until,
                    why: what.clone(),
                });
                if first {
                    e.told = Some(kind.name.clone());
                    let text = format!(
                        "No agent round starts for any work until {}: the command line reports {} ({what}). Each work goes on from its cell then.",
                        utc(until),
                        kind.name
                    );
                    let w = self.w.id;
                    e.db.write(Actor::Engine, |tx| {
                        actions::notify(tx, Some(w), &text).map_err(db::Error::Abort)
                    })
                    .map_err(sql)?;
                }
                Ok(Advance::Waits(format!("{} until {}", kind.name, utc(until))))
            }
            Handle::Retry => {
                let tries = retried(self.at_cell, self.resumed, self.section).len() + 1;
                if tries as u32 <= kind.retries.unwrap_or(0) {
                    return Ok(Advance::Moved);
                }
                self.give_up(e, &kind, &what, tries)
            }
            Handle::Ask => self.give_up(e, &kind, &what, 1),
        }
    }

    /// Pauses the work and asks whom the kind's cell asks (the architect or the owner, a card for the owner either
    /// way in this version, while the architect's flow does not run); the cell runs again once the card is answered.
    fn give_up(&self, e: &mut Engine, kind: &Failure, what: &str, tries: usize) -> Result<Advance, String> {
        let w = self.w;
        let to = match kind
            .then
            .as_deref()
            .and_then(|t| self.g.cell(t))
            .and_then(|c| c.step.as_deref())
        {
            Some("ask_architect") => "architect",
            _ => "owner",
        };
        let item = e.item_of(w)?;
        let body = json!({
            "to": to,
            "work": w.id,
            "item": item.id,
            "title": item.title,
            "failed": {
                "cell": self.cell.id,
                "kind": kind.name,
                "what": what,
                "tries": tries,
                "round": self.round,
                "then": kind.then,
            },
        });
        let why = format!(
            "cell {}: the agent's run ended without a return, {} ({}): {what}",
            self.cell.id,
            if tries == 1 {
                "once".to_string()
            } else {
                format!("{tries} times in a row")
            },
            kind.name
        );
        let note = format!(
            "Work {} on item {} ({}) asks the {to}: {why}; card",
            w.id, item.id, item.title
        );
        let round = self.round;
        let card =
            e.db.write(Actor::Engine, |tx| {
                tx.execute(
                    "INSERT INTO cards (round, kind, body, status) VALUES (?1, 'ask', ?2, 'open')",
                    params![round, body.to_string()],
                )?;
                let card = tx.last_insert_rowid();
                actions::notify(tx, Some(w.id), &format!("{note} {card}")).map_err(db::Error::Abort)?;
                tx.execute(
                    "UPDATE works SET paused = ?2 WHERE id = ?1",
                    params![w.id, format!("{why}; asked the {to} in card {card}")],
                )?;
                Ok(card)
            })
            .map_err(sql)?;
        (e.say)(&format!("work {} paused: {why}; card {card}", w.id));
        Ok(Advance::Stopped)
    }
}

/// Lets each work paused for a failed run go on once the card the run raised is answered: its cell runs again.
pub fn resume_answered(e: &mut Engine) -> Result<(), String> {
    let rows = work::query_json(
        e.db.conn(),
        "SELECT w.id AS work, c.id AS card FROM works w
           JOIN rounds r ON r.id = (SELECT max(x.id) FROM rounds x WHERE x.work = w.id)
           JOIN cards c ON c.round = r.id
         WHERE w.paused IS NOT NULL AND w.ended IS NULL AND c.kind = 'ask' AND c.status = 'ruled'
           AND json_extract(r.rejected, '$.failed') IS NOT NULL",
        &[],
        1000,
    )?;
    for r in rows {
        let (work, card) = (
            r["work"].as_i64().unwrap_or_default(),
            r["card"].as_i64().unwrap_or_default(),
        );
        e.db.write(Actor::Engine, |tx| {
            tx.execute("UPDATE works SET paused = NULL WHERE id = ?1", [work])?;
            Ok(())
        })
        .map_err(sql)?;
        (e.say)(&format!("work {work} goes on: card {card} is answered"));
    }
    Ok(())
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
}
