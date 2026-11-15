//! A wait every work shares, one for each service the works reach: the agents' service, and the host the engine's acts
//! reach for the project's repository. A round failed for a cause that is outside every work and the same for all of
//! them holds every round that service serves while it stands: the agents' rounds, at the usage limit of the account
//! in use, for a command line that cannot run, without the network to the service or while the service does not serve;
//! the acts' rounds, without the network to their host. A usage limit is the account's own: a round that met it holds
//! the rounds only while the account it ran under is the one in use, and only a round under that account coming back
//! ends it ([`limit_of`]); every other cause holds the rounds whichever account they run under. A cause of one service holds nothing of the other, and nothing that comes
//! of the other's rounds ends it. Each is one fact, worked out from the records of every work, never written on each
//! (principle `state-from-records`), and the reason the page gives for every work it holds.
//!
//! The agents' wait stands from the first round of an agent, of any work, that failed so after the last that came
//! back, until a round of an agent that began after the latest that failed so comes back, or the service answers it
//! while it is under way. From the first round that failed so, no agent's round of any work begins, and no work's
//! session is touched: the engine checks the outside by itself instead ([`Found`]), with no round, until a check finds
//! it well. Then every work goes on from where it was, the round that met the failure run again; a round that fails so
//! again holds every round again, until the next check finds it well. The checks are spaced from half a minute after
//! the latest round that failed so, each next one twice as long after the one before, up to [`LONGEST_CHECK`], and
//! counted over the whole wait: a cause the check does not find, which the rounds meet again, is met no more often
//! than the checks are taken. At a usage limit whose reset was said, by the round that met it or by the latest check,
//! none is taken before the reset.
//!
//! The acts' wait stands from the latest act, of any work, that failed for want of the network, until a look at the
//! acts' host, taken every half minute, reaches it: a look is what an act tries, at no cost, and every act goes on
//! once it answers. An act that comes back after it ends it too.
//!
//! A check of the outside, and a look at the network, is no record: it is what the outside says at a moment, which the
//! driver keeps only while its wait stands. Lost in a stop, it is taken again.

use std::collections::{BTreeMap, BTreeSet};

use decide::RunId;
use decide::run::Failure;
use flow::{Compiled, Kind};
use store::WorkNumber;
use work::{Back, Time, Work};

/// How long after the latest round that failed for a cause outside every work the outside is first checked; and how
/// often the network to the acts' host is looked at while it cannot be reached.
pub const PROBE: u64 = 30 * 1000;

/// The longest the engine waits from one check of the outside of the agents' rounds to the next.
pub const LONGEST_CHECK: u64 = 5 * 60 * 1000;

/// How long after the latest round that failed, or the latest check, whichever was later, the next check of the
/// outside of the agents' rounds is taken, `taken` checks having been taken in the wait: [`PROBE`] for the first, twice
/// as long for each next one, up to [`LONGEST_CHECK`].
pub fn spacing(taken: u32) -> u64 {
    PROBE.saturating_mul(1 << taken.min(16)).min(LONGEST_CHECK)
}

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

/// What a check of the outside of the agents' rounds found: what the rounds need from outside every work, the sign-in
/// of their command line, the network to their service and a short request the service answers, each in turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found {
    /// Everything is well: the command line is signed in, the network reaches the service and the service answered.
    Well,
    /// What is down, as a failed round's kind of cause, and why, as the check said it; with when the account's usage
    /// limit resets, where the short request met it and said so.
    Down {
        failure: Failure,
        why: String,
        resets: Option<Time>,
    },
}

/// The checks of the outside of the agents' rounds taken while their wait stands: how many, and the latest, when and
/// what it found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    pub taken: u32,
    pub at: Time,
    pub found: Found,
}

/// A wait every work shares: why no round of any work that a service serves begins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outage {
    /// The service whose rounds it holds.
    pub service: Service,
    /// The kind of the cause the latest round that failed so met: the usage limit, a command line that cannot run, no
    /// network, or a service that does not serve.
    pub failure: Failure,
    /// Why that round failed, as whatever carried it out said.
    pub why: String,
    /// That round: of this work, this run, this number.
    pub work: WorkNumber,
    pub run: RunId,
    pub round: usize,
    /// When the first round that failed so in the wait ended: since when what the rounds need is down.
    pub since: Time,
    /// When the latest round that failed so ended.
    pub met: Time,
    /// When the usage limit resets, where the latest round that failed so said so.
    pub resets: Option<Time>,
    /// The checks of the outside taken in the wait, where any was: the agents' wait alone is checked.
    pub checked: Option<Checked>,
    /// When the latest look at the acts' host reached it, where one did since the latest act that failed so: the acts'
    /// wait alone is looked at.
    pub reached: Option<Time>,
}

/// When the rounds the outage holds may begin again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Until {
    /// Once a check of the outside of the agents' rounds, taken at this time, finds it well.
    Checked { at: Time },
    /// Now: the check taken at this time found the outside well, after the latest round that failed so. The wait
    /// stands until a round that began after that round comes back, and holds nothing meanwhile.
    Well { at: Time },
    /// Every act, once a look at the acts' host, taken from this time on every half minute, finds it reached.
    Looked { at: Time },
}

impl Outage {
    /// When the rounds it holds may begin again.
    pub fn until(&self) -> Until {
        let checked = self.latest();
        match (self.service, checked) {
            (Service::Host, _) => Until::Looked {
                at: Time(self.met.0.saturating_add(PROBE)),
            },
            (
                Service::Agents,
                Some(Checked {
                    at, found: Found::Well, ..
                }),
            ) => Until::Well { at: *at },
            (Service::Agents, checked) => {
                let taken = self.checked.as_ref().map_or(0, |checked| checked.taken);
                let from = checked.map_or(self.met, |checked| checked.at);
                let spaced = Time(from.0.saturating_add(spacing(taken)));
                // The reset the later of the round and the check said: no check is taken before it.
                let resets = match checked {
                    Some(Checked {
                        found: Found::Down { resets, .. },
                        ..
                    }) => *resets,
                    Some(_) => None,
                    None => self.resets.filter(|_| self.failure == Failure::UsageLimit),
                };
                Until::Checked {
                    at: resets.map_or(spaced, |reset| reset.max(spaced)),
                }
            }
        }
    }

    /// Whether the rounds it holds are held now: until a check of the outside finds it well, or a look at the acts'
    /// host reaches it, after the latest round that failed so.
    pub fn holds(&self) -> bool {
        match self.until() {
            Until::Well { .. } => false,
            Until::Checked { .. } => true,
            Until::Looked { .. } => self.reached.is_none_or(|at| at < self.met),
        }
    }

    /// What is down, as the latest check of the outside found it after the latest round that failed so, or else as that
    /// round met it: the kind of the cause and why. A check that found it well finds nothing down: what the round met
    /// is said.
    pub fn down(&self) -> (Failure, &str) {
        match self.latest() {
            Some(Checked {
                found: Found::Down { failure, why, .. },
                ..
            }) => (*failure, why),
            _ => (self.failure, &self.why),
        }
    }

    /// The latest check of the outside, where it was taken after the latest round that failed so.
    fn latest(&self) -> Option<&Checked> {
        self.checked.as_ref().filter(|checked| checked.at >= self.met)
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
    /// The account its latest try ran under, as its records say; none for a try that ran under the program's own
    /// directory, or was kept before tries were recorded.
    account: Option<String>,
    /// When it ended, and what came back; none while it is under way. What came back is none for a round whose run
    /// was abandoned, when it was dropped.
    ended: Option<(Time, decide::Seq, Option<Back>)>,
}

/// The wait of the service every work shares, from the records of these works on the flows they run on, and the
/// rounds the service answered while under way, each of a work and run by its number, if one stands, with the agents
/// under this account: a usage limit met under another holds nothing. What the outside says of it, its checks and
/// looks, is the driver's to add.
pub fn outage<'a>(
    service: Service,
    works: impl IntoIterator<Item = (WorkNumber, &'a Compiled, &'a Work)>,
    answered: &BTreeSet<(WorkNumber, RunId, usize)>,
    account: Option<&str>,
) -> Option<Outage> {
    let mut rounds = Vec::new();
    for (number, flow, work) in works {
        rounds.extend(rounds_of(service, number, flow, work));
    }
    standing(&rounds, answered, |failure, under| {
        failure.shared() && (failure != Failure::UsageLimit || under == account)
    })
    .map(|(first, latest)| Outage {
        service,
        failure: latest.failure,
        why: latest.why.clone(),
        work: latest.round.work,
        run: latest.round.run,
        round: latest.round.number,
        since: first.time,
        met: latest.time,
        resets: latest.resets,
        checked: None,
        reached: None,
    })
}

/// A usage limit an account is at, as the records of every work say: when the latest round under it that met the limit
/// ended, and when the limit resets where that round said; none where no round under it met one, or a round under it
/// that began after came back, or the service answered it while under way. Whether the reset has passed is for whoever
/// reads it at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limit {
    pub met: Time,
    pub resets: Option<Time>,
}

/// The usage limit the account is at, as the records of these works say ([`Limit`]).
pub fn limit_of<'a>(
    works: impl IntoIterator<Item = (WorkNumber, &'a Compiled, &'a Work)>,
    answered: &BTreeSet<(WorkNumber, RunId, usize)>,
    account: Option<&str>,
) -> Option<Limit> {
    let mut rounds = Vec::new();
    for (number, flow, work) in works {
        rounds.extend(rounds_of(Service::Agents, number, flow, work));
    }
    let of_it: Vec<Taken> = rounds.into_iter().filter(|r| r.account.as_deref() == account).collect();
    standing(&of_it, answered, |failure, _| failure == Failure::UsageLimit).map(|(_, latest)| Limit {
        met: latest.time,
        resets: latest.resets,
    })
}

/// Of these rounds, the first and the latest of those that failed for a cause `counts` takes, under the account its
/// round ran under, and that no round that began after it came back to end: none where there is none. A usage limit
/// is ended only by a round under the account that met it; any other cause, by a round under any.
fn standing<'r>(
    rounds: &'r [Taken],
    answered: &BTreeSet<(WorkNumber, RunId, usize)>,
    counts: impl Fn(Failure, Option<&str>) -> bool,
) -> Option<(Met<'r>, Met<'r>)> {
    // Each round that failed for a cause every work shares, with when and where in its work it ended.
    let failed: Vec<Met> = (rounds.iter())
        .filter_map(|r| match &r.ended {
            Some((
                time,
                at,
                Some(Back::Failed {
                    why, failure, resets, ..
                }),
            )) if counts(*failure, r.account.as_deref()) => Some(Met {
                round: r,
                time: *time,
                at: *at,
                why,
                failure: *failure,
                resets: *resets,
            }),
            _ => None,
        })
        .collect();
    // Of the rounds that came back, or that the service answered, the latest beginning, of every work and of each, under
    // any account and under each.
    let mut any: Option<Time> = None;
    let mut any_at: BTreeMap<WorkNumber, decide::Seq> = BTreeMap::new();
    let mut own: BTreeMap<Option<&str>, Time> = BTreeMap::new();
    let mut own_at: BTreeMap<(WorkNumber, Option<&str>), decide::Seq> = BTreeMap::new();
    for r in rounds {
        let back = matches!(
            &r.ended,
            Some((_, _, Some(Back::Asked(_) | Back::HandedIn(_) | Back::Returned { .. })))
        );
        if back || answered.contains(&(r.work, r.run, r.number)) {
            let under = r.account.as_deref();
            any = any.max(Some(r.began));
            let began = own.entry(under).or_insert(r.began);
            *began = (*began).max(r.began);
            for at in [
                any_at.entry(r.work).or_insert(r.began_at),
                own_at.entry((r.work, under)).or_insert(r.began_at),
            ] {
                *at = (*at).max(r.began_at);
            }
        }
    }
    // A failure is over where a round that began after it came back: one that began later, or later in the same work,
    // a round of another work that began at the same time having perhaps begun before it ended. A usage limit is over
    // only where that round ran under the account that met it.
    let over = |m: &Met| {
        let under = m.round.account.as_deref();
        let (began, at) = match m.failure {
            Failure::UsageLimit => (own.get(&under).copied(), own_at.get(&(m.round.work, under)).copied()),
            _ => (any, any_at.get(&m.round.work).copied()),
        };
        began.is_some_and(|began| began > m.time) || at.is_some_and(|began| began > m.at)
    };
    let standing = || failed.iter().filter(|m| !over(m));
    // The latest of those not over, which says the cause; of two that ended at one time, the later in its work, and of
    // two works, the one numbered higher, as the driver takes works in their order. Where the latest is over, every
    // earlier one is.
    let key = |m: &&Met| (m.time, m.round.work, m.at);
    let latest = standing().max_by_key(key)?;
    let first = standing().min_by_key(key)?;
    Some((first.clone(), latest.clone()))
}

/// A round that failed for a cause every work shares, as the outage reads it.
#[derive(Clone)]
struct Met<'a> {
    round: &'a Taken,
    /// When it ended, and the place of the entry of its end in its work.
    time: Time,
    at: decide::Seq,
    why: &'a String,
    failure: Failure,
    resets: Option<Time>,
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
            let tried = (work.ran.iter().rev()).find(|t| t.run == started.run && t.round == i + 1);
            out.push(Taken {
                work: number,
                run: started.run,
                number: i + 1,
                began: work.time_of(round.at).unwrap_or_default(),
                began_at: round.at,
                account: tried.and_then(|t| t.on.account.clone()),
                ended: end,
            });
        }
    }
    out
}
