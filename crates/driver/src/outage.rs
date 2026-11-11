//! A wait every work shares: a round of an agent failed for a cause that is no one work's, the account's usage limit,
//! a command line that cannot run or a network that cannot be reached, and no agent's round of any work begins while
//! it stands. It is one fact, worked out from the records of every work, never written on each (principle
//! `state-from-records`), and the reason the page gives for every work it holds.
//!
//! The wait stands from the latest round of an agent, of any work, that failed so, until a round of an agent that
//! began after that one ended comes back. Until then the agents' rounds may begin again only so:
//!
//! - At the account's usage limit, none begins until it resets, at the time the command line said, or half an hour
//!   after the failure where it said none it could be read by; then every work goes on.
//! - Where the command line cannot run, only the owner can mend it, by signing in again or installing it, and writes
//!   nothing: one round, of whichever work comes to one first, tries every half minute after the last one ended.
//! - Without the network, the engine looks at the network itself every half minute, which costs no round and writes
//!   nothing into a session, and one round tries once a look finds it reached, or every five minutes in case what is
//!   looked at cannot be reached while the service can. A round tried while the network is down would write its
//!   message once more into its session (principle `no-unseen-spending-loop`).
//!
//! A look at the network is no record: it is what the outside says at a moment, which the driver keeps only until the
//! next. Lost in a stop, it is taken again.

use decide::RunId;
use decide::run::Failure;
use flow::{Compiled, Kind};
use store::WorkNumber;
use work::{Back, Time, Work};

/// How long a usage limit is waited out whose reset the command line did not say in words it could be read by.
pub const UNKNOWN_RESET: u64 = 30 * 60 * 1000;

/// How long after the last round ended the next one tries, where the command line cannot run; and how often the
/// network is looked at while it cannot be reached.
pub const PROBE: u64 = 30 * 1000;

/// How long after the last round ended the next one tries while the network cannot be reached, whatever the looks at
/// it find.
pub const OFFLINE_TRY: u64 = 5 * 60 * 1000;

/// A wait every work shares: why no agent's round of any work begins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outage {
    /// The kind of the cause: the usage limit, a command line that cannot run, or no network.
    pub failure: Failure,
    /// Why the round failed, as whatever carried it out said.
    pub why: String,
    /// The round that failed so, the latest that did, which began the wait: of this work, this run, this number.
    pub work: WorkNumber,
    pub run: RunId,
    pub round: usize,
    /// When it ended.
    pub since: Time,
    /// When the usage limit resets, where the command line said so.
    pub resets: Option<Time>,
    /// When the last round of an agent that began after it ended; none where none has.
    pub last: Option<Time>,
    /// Whether a round of an agent that began after it is under way: it tries whether the cause has passed.
    pub trying: bool,
}

/// When the agents' rounds may begin again, as far as the outage says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Until {
    /// At this time, every work's.
    At(Time),
    /// One round, of whichever work comes to one first, at this time; or, where `look`, once a look at the network,
    /// taken from then on every half minute, finds it reached, and at the latest at `latest`.
    Trial { at: Time, look: bool, latest: Time },
    /// Once the round that tries now has ended.
    Tried,
}

impl Outage {
    /// When the agents' rounds may begin again.
    pub fn until(&self) -> Until {
        let from = self.last.unwrap_or(self.since);
        match self.failure {
            Failure::UsageLimit => Until::At(self.resets.unwrap_or(Time(self.since.0.saturating_add(UNKNOWN_RESET)))),
            _ if self.trying => Until::Tried,
            Failure::Offline => Until::Trial {
                at: Time(from.0.saturating_add(PROBE)),
                look: true,
                latest: Time(from.0.saturating_add(OFFLINE_TRY)),
            },
            _ => {
                let at = Time(from.0.saturating_add(PROBE));
                Until::Trial {
                    at,
                    look: false,
                    latest: at,
                }
            }
        }
    }

    /// Whether the agents' rounds are held now, a look at the network having found it reached at `reached`, where one
    /// did since the last round ended.
    pub fn holds(&self, now: Time, reached: Option<Time>) -> bool {
        match self.until() {
            Until::At(at) => now < at,
            Until::Tried => true,
            Until::Trial { look: false, at, .. } => now < at,
            Until::Trial { look: true, latest, .. } => {
                let from = self.last.unwrap_or(self.since);
                let looked = reached.is_some_and(|at| at >= from);
                now < latest && !looked
            }
        }
    }
}

/// A round of an agent, as the outage reads it.
struct Agents {
    work: WorkNumber,
    run: RunId,
    number: usize,
    began: Time,
    /// The place of the entry of its beginning, in its work.
    began_at: decide::Seq,
    /// When it ended, and what came back; none while it is under way. What came back is none for a round whose run
    /// was abandoned, when it was dropped.
    ended: Option<(Time, decide::Seq, Option<Back>)>,
}

/// The wait every work shares, from the records of these works on the flows they run on, if one stands.
pub fn outage<'a>(works: impl IntoIterator<Item = (WorkNumber, &'a Compiled, &'a Work)>) -> Option<Outage> {
    let mut rounds = Vec::new();
    for (number, flow, work) in works {
        rounds.extend(agents_rounds(number, flow, work));
    }
    // The latest that failed for a cause every work shares; of two that ended at one time, the later in its work, and
    // of two works, the one numbered higher, as the driver takes works in their order.
    let failed = (rounds.iter())
        .filter_map(|r| match &r.ended {
            Some((time, at, Some(Back::Failed { why, failure, resets }))) if failure.shared() => {
                Some((r, *time, *at, why, *failure, *resets))
            }
            _ => None,
        })
        .max_by_key(|(r, time, at, ..)| (*time, r.work, *at))?;
    let (round, since, ended_at, why, failure, resets) = failed;
    // A round began after it when it began later, or later in the same work: a round of another work that began at
    // the same time may have begun before it ended.
    let after = |r: &&Agents| r.began > since || (r.work == round.work && r.began_at > ended_at);
    let later: Vec<&Agents> = rounds.iter().filter(after).collect();
    let came_back = later
        .iter()
        .any(|r| matches!(&r.ended, Some((_, _, Some(Back::Asked(_) | Back::HandedIn(_))))));
    if came_back {
        return None;
    }
    Some(Outage {
        failure,
        why: why.clone(),
        work: round.work,
        run: round.run,
        round: round.number,
        since,
        resets,
        last: later.iter().filter_map(|r| r.ended.as_ref().map(|(t, ..)| *t)).max(),
        trying: later.iter().any(|r| r.ended.is_none()),
    })
}

/// Every round of the work's runs of cells an agent carries out, with when it began and ended and what came back.
fn agents_rounds(number: WorkNumber, flow: &Compiled, work: &Work) -> Vec<Agents> {
    let r = &work.records;
    let mut out = Vec::new();
    for started in &r.started {
        let agent = (flow.cells.iter()).any(|spec| spec.id == started.cell && spec.kind == Kind::Agent);
        if !agent {
            continue;
        }
        let began = r.rounds.iter().filter(|x| x.run == started.run);
        let mut ended = r.returned.iter().filter(|x| x.run == started.run);
        for (i, round) in began.enumerate() {
            let end = ended.next().map(|e| {
                let back = work.back.get(&(started.run, i + 1)).cloned();
                (work.time_of(e.at).unwrap_or_default(), e.at, back)
            });
            out.push(Agents {
                work: number,
                run: started.run,
                number: i + 1,
                began: work.time_of(round.at).unwrap_or_default(),
                began_at: round.at,
                ended: end,
            });
        }
    }
    out
}
