//! Agent rounds that do not end in a return the cell takes, handled as the flow's rounds section says, which gives
//! every agent's cell a shared row for each. A return sent back runs the cell again, and so many in a row pause the
//! work. A run that ends without a return is told apart by what the command line reports, and is waited out for every
//! work, run again until it passes, or pauses the work; it never counts as a return sent back, and never asks for a
//! written answer.

use std::time::Duration;

use db::Actor;
use db::rusqlite::params;
use flow::format::{Failure, GUARDED, Handle, Rounds};
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
/// the login has expired), before it looks again: at the network itself, or by letting one round find out.
pub(crate) const PROBE: i64 = 30_000;

/// Every how many looks in a row at the network, each finding it down, a round tries anyway: five minutes of looks.
pub(crate) const LOOKS: u32 = 10;

/// How long a look at the network waits for the service's host to take a connection.
const LOOK_WAIT: Duration = Duration::from_secs(5);

/// Whether the agents' service can be reached from here: its host, the one `ANTHROPIC_BASE_URL` names or Anthropic's
/// API, resolves and takes a connection within a few seconds. Looking costs no round and writes nothing in a session.
pub fn network_reachable() -> bool {
    use std::net::{TcpStream, ToSocketAddrs};
    let host = std::env::var("ANTHROPIC_BASE_URL")
        .ok()
        .and_then(|u| {
            let rest = u.split_once("://").map_or(u.as_str(), |(_, r)| r).to_string();
            rest.split('/').next().map(str::to_string)
        })
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "api.anthropic.com".to_string());
    let at = if host.contains(':') {
        host
    } else {
        format!("{host}:443")
    };
    at.to_socket_addrs()
        .ok()
        .and_then(|mut a| a.next())
        .is_some_and(|a| TcpStream::connect_timeout(&a, LOOK_WAIT).is_ok())
}

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

/// Whether a cell's next round is the one the owner let go on after the run guard stopped the cell's last: the owner's
/// Start comes first, so the guard only records that round. It holds for one round: rounds that never ran to an end
/// are passed over, from the newest, and the first that did must be the one stopped, stopped before the owner's Start.
pub fn waived(at_cell: &[Round], resumed: Option<i64>) -> bool {
    match (at_cell.iter().find(|r| !cut_short(r, GUARDED)), resumed) {
        (Some(r), Some(t)) => failed_as(r) == Some(GUARDED) && t > r.ended_at.unwrap_or(r.started_at),
        _ => false,
    }
}

/// The kind of failed run a round ended as, if any.
fn failed_as(r: &Round) -> Option<&str> {
    r.rejected.as_ref()?["failed"]["kind"].as_str()
}

/// Whether a round did not run to an end of its own: the engine cut it off, or it failed as another kind than `kind`,
/// the one that is looked for.
fn cut_short(r: &Round, kind: &str) -> bool {
    let cut = |x: &serde_json::Value| x.get("interrupted").is_some() || x.get("error").is_some();
    r.rejected.as_ref().is_some_and(cut) || failed_as(r).is_some_and(|k| k != kind)
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

/// A work, as the owner is told of it.
fn about(e: &Engine, w: &Work) -> Result<String, String> {
    Ok(match e.subject(w)? {
        Some(item) => format!("Work {} on item {} ({})", w.id, item.id, item.title),
        None => format!("Work {} ({})", w.id, w.of_architect()),
    })
}

/// Pauses a work and tells the owner why; once the owner lets it go on, its cell runs again.
pub fn pause(e: &mut Engine, w: &Work, why: &str) -> Result<Advance, String> {
    let text = format!("{} is paused: {why}", about(e, w)?);
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE works SET paused = ?2, paused_by = 'engine' WHERE id = ?1",
            params![w.id, why],
        )?;
        actions::notify(tx, Some(w.id), &text).map_err(db::Error::Abort)
    })
    .map_err(sql)?;
    e.say(&format!("work {} paused: {why}", w.id));
    Ok(Advance::Stopped)
}

/// The cell's round before `round` in its work, when its run ended as the same kind of failed run too: none when the
/// round before at the cell did anything else. A run that failed in passing or was cut off between the two is that same
/// round run again, and is passed over.
fn refused_before(e: &Engine, round: RoundId, cell: &str, kind: &str) -> Result<Option<RoundId>, String> {
    let at_cell = work::rounds_at(e.db.conn(), round.work, cell)?;
    let before = at_cell.iter().filter(|r| r.id.n < round.n).find(|r| !cut_short(r, kind));
    Ok(before.filter(|r| failed_as(r) == Some(kind)).map(|r| r.id))
}

/// What a round whose run ended without a return was, and what came of it.
pub struct Failed<'a> {
    pub w: &'a Work,
    pub cell: &'a CompiledCell,
    pub round: RoundId,
    pub section: &'a Rounds,
    /// Why the run guard stopped it, as the round after it is told; none for a run it did not stop.
    pub guard: Option<serde_json::Value>,
}

impl Failed<'_> {
    /// Ends the round, recording which kind of failed run it was, and does what the flow says for that kind.
    pub fn handle(&self, e: &mut Engine, ended: Ended, reply: &Reply) -> Result<Advance, String> {
        let what = reply
            .error
            .clone()
            .unwrap_or_else(|| "the round handed in nothing".to_string());
        let now = db::now();
        // A run stopped is told by that, never by the words of a report, which it does not have.
        let name = match &reply.stopped {
            Some(_) => GUARDED,
            None => agent::failure_kind(&what),
        };
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
        if let Some(g) = &self.guard {
            failed["guard"] = g.clone();
        }
        e.say(&format!(
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
                let first = e.halt_every_work(Halt {
                    kind: kind.name.clone(),
                    until,
                    why: what.clone(),
                });
                if first {
                    let text = if kind.name == "usage_limit" {
                        format!(
                            "No agent round starts for any work until {}: the command line reports {} ({what}). Each work goes on from its cell then.",
                            utc(until),
                            kind.name
                        )
                    } else if kind.name == "offline" {
                        format!(
                            "No agent round starts for any work while the command line reports {} ({what}): {} Each work goes on from its cell once the network is back.",
                            kind.name, kind.desc
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
            // The same session would give the same context again: the line's session ends, whatever comes next, and the
            // first time the cell runs again in a new one; when the cell's round before ended the same way, passing
            // failures aside, the work pauses.
            Handle::Fresh => {
                let before = refused_before(e, round, &self.cell.id, &kind.name)?;
                e.db.write(Actor::Engine, |tx| {
                    tx.execute(
                        "UPDATE sessions SET ended = ?3 WHERE ended IS NULL
                           AND id = (SELECT session FROM rounds WHERE work = ?1 AND n = ?2)",
                        params![round.work, round.n, db::now()],
                    )?;
                    Ok(())
                })
                .map_err(sql)?;
                let guarded = kind.name == GUARDED;
                let doer = if guarded {
                    "the run guard stopped"
                } else {
                    "the service refused"
                };
                if let Some(first) = before {
                    let why = format!(
                        "cell {}: {doer} the agent's run in {first} and again in {round}, in a new session ({}): {what}",
                        self.cell.id, kind.name
                    );
                    return pause(e, self.w, &why);
                }
                // The guard's stop is the engine's own act on a run: the owner hears of it the first time too.
                if guarded {
                    let text = format!(
                        "{}: {doer} the agent's run in {round} at cell {} ({what}). Its session ended, and the cell runs \
                         again in a new one.",
                        about(e, self.w)?,
                        self.cell.id
                    );
                    let w = self.w.id;
                    e.db.write(Actor::Engine, |tx| {
                        actions::notify(tx, Some(w), &text).map_err(db::Error::Abort)
                    })
                    .map_err(sql)?;
                }
                e.say(&format!(
                    "{round}: {doer} the run ({}): its line's session ends, and {} runs again in a new one",
                    kind.name, self.cell.id
                ));
                Ok(Advance::Moved)
            }
            // A return to repair is repaired within its round (`unparsed`); one that comes here was not, and waits for
            // the owner as a pause does.
            Handle::Pause | Handle::Repair => {
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

    // The owner's Start waives the guard for the round after the one stopped, and for that round alone, however many
    // runs of it never came to an end; a Start from before the stop waives nothing.
    #[test]
    fn the_owners_start_waives_the_guard_for_one_round() {
        let round = |n: i64, ended: i64, rejected: Option<serde_json::Value>| Round {
            id: RoundId::new(1, n),
            cell: Some("R1".into()),
            session: None,
            input: json!({}),
            output: None,
            transition: rejected.is_none().then(|| "done".to_string()),
            rejected,
            compactions: 0,
            started_at: ended - 1,
            ended_at: Some(ended),
            usage: None,
        };
        let failed = |kind: &str| Some(json!({ "failed": { "kind": kind, "what": "x" } }));
        let stopped = round(1, 10, failed(GUARDED));
        assert!(waived(std::slice::from_ref(&stopped), Some(11)));
        assert!(!waived(std::slice::from_ref(&stopped), Some(9)));
        assert!(!waived(std::slice::from_ref(&stopped), None));
        assert!(!waived(&[], Some(11)));
        // A retry of the waived round is still that round.
        let retried = [round(2, 12, failed("transient")), stopped.clone()];
        assert!(waived(&retried, Some(11)));
        let cut = [
            round(2, 12, Some(json!({ "interrupted": "the engine stopped" }))),
            stopped.clone(),
        ];
        assert!(waived(&cut, Some(11)));
        // Once it came to an end, in a return taken or sent back, rounds are judged again.
        assert!(!waived(&[round(2, 12, None), stopped.clone()], Some(11)));
        let sent_back = [round(2, 12, Some(json!({ "format": "x" }))), stopped];
        assert!(!waived(&sent_back, Some(11)));
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
