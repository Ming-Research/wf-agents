//! A wait every work shares, one for each service the works reach: the agents' service, and the host the engine's acts
//! reach for the project's repository. A round failed for a cause that is no one work's and holds every round that
//! service serves while it stands: the agents' rounds, at the account's usage limit, for a command line that cannot
//! run or without the network to the service; the acts' rounds, without the network to their host. A cause of one
//! service holds nothing of the other, and nothing that comes of the other's rounds ends it. Each is one fact, worked
//! out from the records of every work, never written on each (principle `state-from-records`), and the reason the page
//! gives for every work it holds.
//!
//! The agents' wait stands from the latest round of an agent, of any work, that failed so, until a round of an agent
//! that began after that one ended comes back, or the service answers it while it is under way: what it is to try is
//! whether the service serves again, which its first answer shows, and the round may go on for long after. Until then
//! the agents' rounds may begin again only so:
//!
//! - At the account's usage limit, none begins until it resets, at the time the command line said; then every work
//!   goes on. Where it said none it could be read by, one round, of whichever work comes to one first, tries half an
//!   hour after the last one ended: every work going on at a time guessed would each meet the limit again.
//! - Where the command line cannot run, only the owner can mend it, by signing in again or installing it, and writes
//!   nothing: one round, of whichever work comes to one first, tries every half minute after the last one ended.
//! - Without the network, the engine looks at the service's host itself every half minute, which costs no round and
//!   writes nothing into a session, and one round tries once a look finds it reached, and never before. A round tried
//!   while the network is down would write its message once more into its session (principle
//!   `no-unseen-spending-loop`).
//!
//! The acts' wait stands from the latest act, of any work, that failed for want of the network, until a look at the
//! acts' host, taken every half minute, reaches it: a look is what an act tries, at no cost, and every act goes on
//! once it answers. An act that comes back after it ends it too.
//!
//! A look at the network is no record: it is what the outside says at a moment, which the driver keeps only until the
//! next. Lost in a stop, it is taken again.

use std::collections::BTreeSet;

use decide::RunId;
use decide::run::Failure;
use flow::{Compiled, Kind};
use store::WorkNumber;
use work::{Back, Time, Work};

/// How long after the last round ended the next one tries, at a usage limit whose reset the command line did not say
/// so that it could be read.
pub const UNKNOWN_RESET: u64 = 30 * 60 * 1000;

/// How long after the last round ended the next one tries, where the command line cannot run; and how often the
/// network is looked at while it cannot be reached.
pub const PROBE: u64 = 30 * 1000;

/// A service the works reach, each with its wait.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Service {
    /// The agents' service: the rounds of the cells an agent carries out.
    Agents,
    /// The host of the project's repository, as the engine's acts reach it: the rounds of the engine's own cells.
    Host,
}

impl Service {
    /// Whether the service serves the rounds of the cell of this kind.
    fn serves(self, kind: Kind) -> bool {
        match self {
            Service::Agents => kind == Kind::Agent,
            Service::Host => kind == Kind::Engine,
        }
    }
}

/// A wait every work shares: why no round of any work that a service serves begins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outage {
    /// The service whose rounds it holds.
    pub service: Service,
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
    /// When the last round of the service that began after it ended; none where none has.
    pub last: Option<Time>,
    /// Whether a round of the service that began after it is under way: it tries whether the cause has passed.
    pub trying: bool,
}

/// When the rounds the outage holds may begin again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Until {
    /// At this time, every work's.
    At(Time),
    /// One round, of whichever work comes to one first, at this time; or, where `look`, once a look at the service's
    /// host, taken from then on every half minute, finds it reached.
    Trial { at: Time, look: bool },
    /// Every round, once a look at the service's host, taken from this time on every half minute, finds it reached.
    Looked { at: Time },
    /// Once the round that tries now has ended, or the service has answered it.
    Tried,
}

impl Outage {
    /// When the rounds it holds may begin again.
    pub fn until(&self) -> Until {
        let from = self.last.unwrap_or(self.since);
        let probe = Time(from.0.saturating_add(PROBE));
        match (self.service, self.failure, self.resets) {
            (Service::Host, ..) => Until::Looked { at: probe },
            (_, Failure::UsageLimit, Some(at)) => Until::At(at),
            _ if self.trying => Until::Tried,
            (_, Failure::UsageLimit, None) => Until::Trial {
                at: Time(from.0.saturating_add(UNKNOWN_RESET)),
                look: false,
            },
            (_, Failure::Offline, _) => Until::Trial { at: probe, look: true },
            _ => Until::Trial { at: probe, look: false },
        }
    }

    /// Whether the rounds it holds are held now, a look at the service's host having found it reached at `reached`,
    /// where one did since the last round ended.
    pub fn holds(&self, now: Time, reached: Option<Time>) -> bool {
        let looked = || {
            let from = self.last.unwrap_or(self.since);
            reached.is_some_and(|at| at >= from)
        };
        match self.until() {
            Until::At(at) => now < at,
            Until::Tried => true,
            Until::Trial { look: false, at } => now < at,
            Until::Trial { look: true, .. } | Until::Looked { .. } => !looked(),
        }
    }
}

/// A round of a cell, as the outage reads it.
struct Taken {
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

/// The wait of the service every work shares, from the records of these works on the flows they run on, and the
/// rounds the service answered while under way, each of a work and run by its number, if one stands.
pub fn outage<'a>(
    service: Service,
    works: impl IntoIterator<Item = (WorkNumber, &'a Compiled, &'a Work)>,
    answered: &BTreeSet<(WorkNumber, RunId, usize)>,
) -> Option<Outage> {
    let mut rounds = Vec::new();
    for (number, flow, work) in works {
        rounds.extend(rounds_of(service, number, flow, work));
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
    let after = |r: &&Taken| r.began > since || (r.work == round.work && r.began_at > ended_at);
    let later: Vec<&Taken> = rounds.iter().filter(after).collect();
    let came_back = later.iter().any(|r| {
        let back = matches!(
            &r.ended,
            Some((_, _, Some(Back::Asked(_) | Back::HandedIn(_) | Back::Returned { .. })))
        );
        back || answered.contains(&(r.work, r.run, r.number))
    });
    if came_back {
        return None;
    }
    Some(Outage {
        service,
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

/// Every round of the work's runs of cells the service serves, with when it began and ended and what came back.
fn rounds_of(service: Service, number: WorkNumber, flow: &Compiled, work: &Work) -> Vec<Taken> {
    let r = &work.records;
    let mut out = Vec::new();
    for started in &r.started {
        let served = (flow.cells.iter()).any(|spec| spec.id == started.cell && service.serves(spec.kind));
        if !served {
            continue;
        }
        let began = r.rounds.iter().filter(|x| x.run == started.run);
        let mut ended = r.returned.iter().filter(|x| x.run == started.run);
        for (i, round) in began.enumerate() {
            let end = ended.next().map(|e| {
                let back = work.back.get(&(started.run, i + 1)).cloned();
                (work.time_of(e.at).unwrap_or_default(), e.at, back)
            });
            out.push(Taken {
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
