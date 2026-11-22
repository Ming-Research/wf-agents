//! A wait every work shares, one for each service the works reach: the agents' service, and the host the engine's acts
//! reach for the project's repository. A round failed for a cause that is outside every work and the same for all of
//! them holds every round that service serves while it stands: the agents' rounds, at the account's usage limit, for a
//! command line that cannot run, without the network to the service or while the service does not serve; the acts'
//! rounds, without the network to their host. A cause of one service holds nothing of the other, and nothing that comes
//! of the other's rounds ends it. Each is one fact, worked out from the records of every work, never written on each
//! (principle `state-from-records`), and the reason the page gives for every work it holds.
//!
//! The agents' wait stands from the first round of an agent, of any work, that failed so after the last that came
//! back, until a round of an agent that began after the latest that failed so comes back, or the service answers it
//! while it is under way. A usage limit is its account's own: it stands from the first round under that account that
//! met it after the last round under it that came back, only a round under it ends it, and it holds the rounds only
//! while its account is in use; taking another ends the wait at once, and taking it back before its reset holds the
//! rounds again. From the first round that failed so, no agent's round of any work begins, and no work's
//! session is touched: the engine checks the outside by itself instead ([`Found`]), with no round, until a check finds
//! it well. Then every work goes on from where it was, the round that met the failure run again; a round that fails so
//! again holds every round again, until the next check finds it well. The checks are spaced from half a minute after
//! the latest round that failed so, each next one twice as long after the one before, up to [`LONGEST_CHECK`], and
//! counted over the whole wait: a cause the check does not find, which the rounds meet again, is met no more often
//! than the checks are taken. At a usage limit whose reset was said, by the round that met it or by the latest check,
//! none is taken before the reset. A check speaks only of the account it was taken under: once the owner takes another,
//! the outside is checked at once under the account taken, and what was found under the one before is no check of it.
//! Where the owner has let the engine move, a usage limit of the account in use is also ended by the engine's taking
//! another account that a check under it found well ([`move_on`]): the others are checked one at a time, in the order
//! added, each check kept with the `since` of the wait it was taken in.
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
use flow::Outline;
use store::WorkNumber;
use work::{Back, Time, Work};

use crate::accounts::Accounts;

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
    /// Whether the service serves the rounds of a cell, an agent's or else an act of the engine's.
    fn serves(self, agent: bool) -> bool {
        match self {
            Service::Agents => agent,
            Service::Host => !agent,
        }
    }
}

/// What a check of the outside of the agents' rounds found: what the rounds need from outside every work, an account
/// in use, the network to their service and a short request the service answers, each in turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found {
    /// Everything is well: an account is in use, the network reaches the service and the service answered.
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
    /// When the owner took the account in use, where that was after the latest round that failed so: what the checks
    /// found before was of another account, and the outside is checked at once under this one. The agents' wait alone
    /// has it.
    pub switched: Option<Time>,
    /// Whether the owner has let the engine move to another account at a usage limit and some other account may yet be
    /// moved to in this wait ([`may_move`]); the agents' wait alone has it.
    pub may_move: bool,
}

/// The usage limit of an account, while it stands: since the first round under the account that met it after the last
/// round under it that came back, and when it resets, where the latest of those rounds said so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limit {
    pub since: Time,
    pub resets: Option<Time>,
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
                // Taken since the latest check, another account is checked at once.
                let spaced = match self.switched {
                    Some(at) if checked.is_none_or(|checked| checked.at < at) => at,
                    _ => Time(from.0.saturating_add(spacing(taken))),
                };
                // The reset the later of the round and the check said: no check is taken before it.
                Until::Checked {
                    at: self.reset().map_or(spaced, |reset| reset.max(spaced)),
                }
            }
        }
    }

    /// When the usage limit resets, as the later of the round that met it and the latest check of the outside said,
    /// where either said so.
    pub fn reset(&self) -> Option<Time> {
        match self.latest() {
            Some(Checked {
                found: Found::Down { resets, .. },
                ..
            }) => *resets,
            Some(_) => None,
            None => self.resets.filter(|_| self.failure == Failure::UsageLimit),
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

/// What the engine does, with the owner's leave, at the usage limit of the account in use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Move {
    /// Takes this account as the one the agents run under: a check found it well. `left_resets` is when the usage limit
    /// of the account left resets, where it was said.
    Take { name: String, left_resets: Option<Time> },
    /// Checks the outside under this account at this time: when the earliest of the accounts that are neither passed over
    /// nor found well is due.
    Check { name: String, at: Time },
    /// Does nothing: the wait is not one that moving to another account ends, or no account is left to move to.
    Stay,
}

/// Where an account other than the one in use stands, in the wait at the in-use account's usage limit, as to being
/// moved to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Candidate {
    /// No check of the outside under it has been taken in this wait, and no limit of its own is known to stand.
    Open,
    /// The latest check under it found the outside well in this wait.
    Well,
    /// The latest check under it found a cause that is not its own, or its usage limit with no reset said: checked
    /// again at this time.
    Spaced { at: Time },
    /// Its usage limit stands until this time, as its records or the latest check under it say: checked again then.
    Waiting { at: Time },
    /// The latest check under it found its token refused: passed over for the rest of the wait.
    Passed,
}

impl Candidate {
    /// Whether the account may still be moved to in this wait, unless its limit stands until a time to come.
    fn left(self) -> bool {
        matches!(self, Candidate::Open | Candidate::Well | Candidate::Spaced { .. })
    }
}

/// The accounts other than the one in use, in the order they were added, each with where it stands as to being moved to
/// in the wait: none where the wait is not the agents' service's, does not hold, or is not at an account's usage limit,
/// or none is in use. `checked` holds, by account, the checks taken in the waits, each with the `since` of the wait it
/// was taken in; the checks under an account that is not the one in use are of the account's being moved to, and of
/// this wait where that is the wait's `since`.
pub fn candidates(
    outage: &Outage,
    accounts: &Accounts,
    limits: &BTreeMap<String, Limit>,
    checked: &BTreeMap<Option<String>, (Time, Checked)>,
    now: Time,
) -> Vec<(String, Candidate)> {
    let Some(in_use) = accounts.in_use() else {
        return Vec::new();
    };
    if outage.service != Service::Agents || !outage.holds() || outage.down().0 != Failure::UsageLimit {
        return Vec::new();
    }
    (accounts.live().into_iter())
        .filter(|live| live.name != in_use)
        .map(|live| {
            let own = checked
                .get(&Some(live.name.clone()))
                .filter(|(of, _)| *of == outage.since)
                .map(|(_, check)| check);
            let state = match own {
                Some(Checked { found: Found::Well, .. }) => Candidate::Well,
                Some(Checked {
                    found: Found::Down { failure, resets, .. },
                    at,
                    taken,
                }) => match (failure, resets) {
                    (Failure::CannotRun, _) => Candidate::Passed,
                    (Failure::UsageLimit, Some(reset)) if reset > at => Candidate::Waiting { at: *reset },
                    _ => Candidate::Spaced {
                        at: Time(at.0.saturating_add(spacing(*taken))),
                    },
                },
                None => match limits.get(&live.name).and_then(|limit| limit.resets) {
                    Some(reset) if reset > now => Candidate::Waiting { at: reset },
                    _ => Candidate::Open,
                },
            };
            (live.name, state)
        })
        .collect()
}

/// Whether the engine may yet move to an account in this wait: some other account is not passed over, nor held by a
/// usage limit that stands until a time to come ([`candidates`]).
pub fn may_move(
    outage: &Outage,
    accounts: &Accounts,
    limits: &BTreeMap<String, Limit>,
    checked: &BTreeMap<Option<String>, (Time, Checked)>,
    now: Time,
) -> bool {
    (candidates(outage, accounts, limits, checked, now).into_iter()).any(|(_, candidate)| candidate.left())
}

/// What the engine does in the wait at the in-use account's usage limit, the owner having let it move to another
/// account: takes the first of the other accounts, in the order they were added, that a check found well; else asks
/// for the check of the account whose check is due first, an account never checked in the wait being due when the
/// wait began, and one that was checked being due when its check says ([`Candidate`]).
pub fn move_on(
    outage: &Outage,
    accounts: &Accounts,
    limits: &BTreeMap<String, Limit>,
    checked: &BTreeMap<Option<String>, (Time, Checked)>,
    now: Time,
) -> Move {
    let candidates = candidates(outage, accounts, limits, checked, now);
    if let Some((name, _)) = candidates.iter().find(|(_, candidate)| *candidate == Candidate::Well) {
        return Move::Take {
            name: name.clone(),
            left_resets: outage.reset(),
        };
    }
    let due = |candidate: &Candidate| match candidate {
        Candidate::Open => Some(outage.met),
        Candidate::Spaced { at } | Candidate::Waiting { at } => Some(*at),
        Candidate::Well | Candidate::Passed => None,
    };
    (candidates.iter())
        .filter_map(|(name, candidate)| due(candidate).map(|at| (at, name)))
        .min_by_key(|(at, _)| *at)
        .map_or(Move::Stay, |(at, name)| Move::Check { name: name.clone(), at })
}

/// A round of a cell, as the outage reads it.
struct Taken {
    work: WorkNumber,
    run: RunId,
    number: usize,
    /// The account its last try ran under; none where it ran under none, or recorded no try.
    account: Option<String>,
    began: Time,
    /// The place of the entry of its beginning, in its work.
    began_at: decide::Seq,
    /// When it ended, and what came back; none while it is under way. What came back is none for a round whose run
    /// was abandoned, when it was dropped.
    ended: Option<(Time, decide::Seq, Option<Back>)>,
}

/// The wait of the service every work shares, from the records of these works on the flows they run on, each round's
/// account read from its last try, the rounds the service answered while under way, each of a work and run by its
/// number, and the account in use, if one stands. What the outside says of it, its checks and looks, and when the
/// account in use was taken, is the driver's to add.
pub fn outage<'a>(
    service: Service,
    works: impl IntoIterator<Item = (WorkNumber, &'a Outline, &'a Work)>,
    answered: &BTreeSet<(WorkNumber, RunId, usize)>,
    in_use: Option<&str>,
) -> Option<Outage> {
    let rounds = rounds(service, works);
    // A usage limit holds the rounds only while its account is in use.
    let standing: Vec<Met> = (standing(&rounds, answered).into_iter())
        .filter(|m| m.failure != Failure::UsageLimit || m.round.account.as_deref() == in_use)
        .collect();
    // The latest of those not over, which says the cause; of two that ended at one time, the later in its work, and of
    // two works, the one numbered higher, as the driver takes works in their order. Where the latest is over, every
    // earlier one is.
    let key = |m: &&Met| (m.time, m.round.work, m.at);
    let latest = standing.iter().max_by_key(key)?;
    let first = standing.iter().min_by_key(key)?;
    Some(Outage {
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
        switched: None,
        may_move: false,
    })
}

/// The usage limit of each account that stands, by the account's name, whether or not the account is in use, from the
/// records of these works and the rounds the agents' service answered while under way.
pub fn limits<'a>(
    works: impl IntoIterator<Item = (WorkNumber, &'a Outline, &'a Work)>,
    answered: &BTreeSet<(WorkNumber, RunId, usize)>,
) -> BTreeMap<String, Limit> {
    let rounds = rounds(Service::Agents, works);
    let mut limits: BTreeMap<String, (Limit, Time)> = BTreeMap::new();
    for m in standing(&rounds, answered) {
        let Some(account) = m.round.account.clone().filter(|_| m.failure == Failure::UsageLimit) else {
            continue;
        };
        let limit = limits.entry(account).or_insert((
            Limit {
                since: m.time,
                resets: m.resets,
            },
            m.time,
        ));
        limit.0.since = limit.0.since.min(m.time);
        if m.time >= limit.1 {
            (limit.0.resets, limit.1) = (m.resets, m.time);
        }
    }
    (limits.into_iter()).map(|(name, (limit, _))| (name, limit)).collect()
}

/// Every round of the works' runs of cells the service serves.
fn rounds<'a>(service: Service, works: impl IntoIterator<Item = (WorkNumber, &'a Outline, &'a Work)>) -> Vec<Taken> {
    let mut rounds = Vec::new();
    for (number, outline, work) in works {
        rounds.extend(rounds_of(service, number, outline, work));
    }
    rounds
}

/// Each round that failed for a cause every work shares and is not over, with when and where in its work it ended. A
/// failure is over where a round that began after it came back, or the service answered it: one that began later, or
/// later in the same work, a round of another work that began at the same time having perhaps begun before it ended;
/// a usage limit only where that round ran under the same account.
fn standing<'r>(rounds: &'r [Taken], answered: &BTreeSet<(WorkNumber, RunId, usize)>) -> Vec<Met<'r>> {
    let back: Vec<&Taken> = (rounds.iter())
        .filter(|r| {
            let came = matches!(
                &r.ended,
                Some((_, _, Some(Back::Asked(_) | Back::HandedIn(_) | Back::Returned { .. })))
            );
            came || answered.contains(&(r.work, r.run, r.number))
        })
        .collect();
    let over = |m: &Met| {
        (back.iter()).any(|r| {
            let same = m.failure != Failure::UsageLimit || r.account == m.round.account;
            same && (r.began > m.time || (r.work == m.round.work && r.began_at > m.at))
        })
    };
    (rounds.iter())
        .filter_map(|r| match &r.ended {
            Some((time, at, Some(Back::Failed { why, failure, resets }))) if failure.shared() => Some(Met {
                round: r,
                time: *time,
                at: *at,
                why,
                failure: *failure,
                resets: *resets,
            }),
            _ => None,
        })
        .filter(|m| !over(m))
        .collect()
}

/// A round that failed for a cause every work shares, as the outage reads it.
struct Met<'a> {
    round: &'a Taken,
    /// When it ended, and the place of the entry of its end in its work.
    time: Time,
    at: decide::Seq,
    why: &'a String,
    failure: Failure,
    resets: Option<Time>,
}

/// Every round of the work's runs of cells the service serves, as the version the work is on has them, with the
/// account its last try ran under, when it began and ended and what came back.
fn rounds_of(service: Service, number: WorkNumber, outline: &Outline, work: &Work) -> Vec<Taken> {
    let r = &work.records;
    // The account of each round, by its run and number, as its last try recorded it: the tries in the order they began.
    let mut accounts: BTreeMap<(RunId, usize), Option<&str>> = BTreeMap::new();
    for ran in &work.ran {
        accounts.insert((ran.run, ran.round), ran.on.account.as_deref());
    }
    let mut out = Vec::new();
    for started in &r.started {
        let served = (outline.cell(&started.cell)).is_some_and(|cell| service.serves(cell.agent));
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
                account: accounts
                    .get(&(started.run, i + 1))
                    .copied()
                    .flatten()
                    .map(str::to_string),
                began: work.time_of(round.at).unwrap_or_default(),
                began_at: round.at,
                ended: end,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use store::{AccountAct, AccountRecord, AccountTaker};

    use super::*;

    const HOUR: u64 = 3_600_000;

    /// The wait at a usage limit, begun at 1,000 and met at 2,000, whose reset the round said.
    fn at_limit(resets: Option<Time>) -> Outage {
        Outage {
            service: Service::Agents,
            failure: Failure::UsageLimit,
            why: "You've hit your limit".to_string(),
            work: WorkNumber(1),
            run: RunId(1),
            round: 1,
            since: Time(1_000),
            met: Time(2_000),
            resets,
            checked: None,
            reached: None,
            switched: None,
            may_move: false,
        }
    }

    /// The accounts added in this order, the first taken.
    fn accounts(names: &[&str]) -> Accounts {
        let mut records: Vec<AccountRecord> = (names.iter())
            .map(|name| AccountRecord {
                name: name.to_string(),
                what: AccountAct::Added,
                at: Time(0),
                taker: AccountTaker::Owner,
                left_resets: None,
            })
            .collect();
        records.push(AccountRecord {
            name: names[0].to_string(),
            what: AccountAct::Taken,
            at: Time(1),
            taker: AccountTaker::Owner,
            left_resets: None,
        });
        Accounts::new(records)
    }

    /// A check under an account, taken at this time as the `taken`th of the wait.
    fn checked(found: Found, at: u64, taken: u32) -> (Time, Checked) {
        (
            Time(1_000),
            Checked {
                taken,
                at: Time(at),
                found,
            },
        )
    }

    fn found_down(failure: Failure, resets: Option<Time>) -> Found {
        Found::Down {
            failure,
            why: "found".to_string(),
            resets,
        }
    }

    // Microseconds, nothing run: where each account stands as to being moved to, from the checks of the wait and the
    // limits the records say.
    #[test]
    fn each_account_but_the_one_in_use_stands_as_its_check_or_its_standing_limit_says() {
        let names = [
            "use", "open", "well", "waits", "no-reset", "offline", "refused", "limit", "over", "old",
        ];
        let accounts = accounts(&names);
        let now = Time(5_000);
        let mut checks = BTreeMap::new();
        checks.insert(Some("well".to_string()), checked(Found::Well, 3_000, 1));
        let reset = Time(3_000 + HOUR);
        checks.insert(
            Some("waits".to_string()),
            checked(found_down(Failure::UsageLimit, Some(reset)), 3_000, 1),
        );
        checks.insert(
            Some("no-reset".to_string()),
            checked(found_down(Failure::UsageLimit, None), 3_000, 1),
        );
        checks.insert(
            Some("offline".to_string()),
            checked(found_down(Failure::Offline, None), 3_000, 3),
        );
        checks.insert(
            Some("refused".to_string()),
            checked(found_down(Failure::CannotRun, None), 3_000, 1),
        );
        // A check of an earlier wait is of no account's standing in this one.
        checks.insert(
            Some("old".to_string()),
            (
                Time(500),
                Checked {
                    taken: 1,
                    at: Time(900),
                    found: Found::Well,
                },
            ),
        );
        let mut limits = BTreeMap::new();
        limits.insert(
            "limit".to_string(),
            Limit {
                since: Time(0),
                resets: Some(Time(now.0 + HOUR)),
            },
        );
        limits.insert(
            "over".to_string(),
            Limit {
                since: Time(0),
                resets: Some(Time(now.0 - 1)),
            },
        );
        let stands = candidates(&at_limit(None), &accounts, &limits, &checks, now);
        assert_eq!(
            stands,
            vec![
                ("open".to_string(), Candidate::Open),
                ("well".to_string(), Candidate::Well),
                ("waits".to_string(), Candidate::Waiting { at: reset }),
                (
                    "no-reset".to_string(),
                    Candidate::Spaced {
                        at: Time(3_000 + spacing(1))
                    }
                ),
                (
                    "offline".to_string(),
                    Candidate::Spaced {
                        at: Time(3_000 + spacing(3))
                    }
                ),
                ("refused".to_string(), Candidate::Passed),
                ("limit".to_string(), Candidate::Waiting { at: Time(now.0 + HOUR) }),
                ("over".to_string(), Candidate::Open),
                ("old".to_string(), Candidate::Open),
            ]
        );
    }

    // Microseconds, nothing run: the first account found well in the order added is taken, with the reset the wait says;
    // else the check that is due first is asked for, an account never checked being due when the wait was met.
    #[test]
    fn the_first_account_found_well_is_taken_else_the_check_due_first_is_asked_for() {
        let accounts = accounts(&["use", "a", "b", "c"]);
        let outage = at_limit(Some(Time(9_000)));
        let now = Time(5_000);
        let limits = BTreeMap::new();
        let mut checks = BTreeMap::new();
        assert_eq!(
            move_on(&outage, &accounts, &limits, &checks, now),
            Move::Check {
                name: "a".to_string(),
                at: Time(2_000)
            },
            "never checked: due when the wait was met, the first in the order added first"
        );
        checks.insert(
            Some("a".to_string()),
            checked(found_down(Failure::Offline, None), 3_000, 1),
        );
        checks.insert(Some("b".to_string()), checked(Found::Well, 3_000, 1));
        checks.insert(Some("c".to_string()), checked(Found::Well, 3_001, 1));
        assert_eq!(
            move_on(&outage, &accounts, &limits, &checks, now),
            Move::Take {
                name: "b".to_string(),
                left_resets: Some(Time(9_000))
            }
        );
        checks.insert(
            Some("b".to_string()),
            checked(found_down(Failure::UsageLimit, Some(Time(7_000))), 3_000, 2),
        );
        checks.insert(
            Some("c".to_string()),
            checked(found_down(Failure::CannotRun, None), 3_001, 1),
        );
        assert_eq!(
            move_on(&outage, &accounts, &limits, &checks, now),
            Move::Check {
                name: "b".to_string(),
                at: Time(7_000)
            },
            "the earlier of b's reset and a's spacing"
        );
        checks.insert(
            Some("b".to_string()),
            checked(found_down(Failure::CannotRun, None), 3_000, 3),
        );
        assert_eq!(
            move_on(&outage, &accounts, &limits, &checks, now),
            Move::Check {
                name: "a".to_string(),
                at: Time(3_000 + spacing(1))
            }
        );
        assert!(may_move(&outage, &accounts, &limits, &checks, now));
        checks.insert(
            Some("a".to_string()),
            checked(found_down(Failure::CannotRun, None), 3_000, 2),
        );
        assert_eq!(
            move_on(&outage, &accounts, &limits, &checks, now),
            Move::Stay,
            "none is left"
        );
        assert!(!may_move(&outage, &accounts, &limits, &checks, now));
    }

    // Microseconds, nothing run: nothing moves where the wait is not at the in-use account's usage limit and holding.
    #[test]
    fn nothing_moves_unless_the_wait_is_the_agents_at_the_usage_limit_of_an_account_in_use_and_holds() {
        let accounts = accounts(&["use", "a"]);
        let (limits, checks, now) = (BTreeMap::new(), BTreeMap::new(), Time(5_000));
        let moves = |outage: &Outage, accounts: &Accounts| move_on(outage, accounts, &limits, &checks, now);
        assert_ne!(moves(&at_limit(None), &accounts), Move::Stay);
        let offline = Outage {
            failure: Failure::Offline,
            ..at_limit(None)
        };
        assert_eq!(moves(&offline, &accounts), Move::Stay);
        let host = Outage {
            service: Service::Host,
            ..at_limit(None)
        };
        assert_eq!(moves(&host, &accounts), Move::Stay);
        // A check that found the outside well ends the hold.
        let well = Outage {
            checked: Some(Checked {
                taken: 1,
                at: Time(3_000),
                found: Found::Well,
            }),
            ..at_limit(None)
        };
        assert!(!well.holds());
        assert_eq!(moves(&well, &accounts), Move::Stay);
        // A check that found it at another cause says the cause is not the limit.
        let found_offline = Outage {
            checked: Some(Checked {
                taken: 1,
                at: Time(3_000),
                found: found_down(Failure::Offline, None),
            }),
            ..at_limit(None)
        };
        assert_eq!(moves(&found_offline, &accounts), Move::Stay);
        assert_eq!(moves(&at_limit(None), &Accounts::default()), Move::Stay, "none in use");
    }
}
