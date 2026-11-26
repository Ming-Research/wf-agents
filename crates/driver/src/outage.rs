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
//! it well. Then one round goes first (decision `one-goes-first-after-a-wait`): the first to begin, the driver giving
//! the turn first to the work whose round met the failure first in the wait. While it is under way the wait holds every
//! other round; once it comes back, or the service answers it, the wait is over and every work goes on from where it
//! was. A round that fails so again holds every round again, until the next check finds it well; one that fails for a
//! cause of its own work lets the next go first. The checks are spaced from half a minute after
//! the latest round that failed so, each next one twice as long after the one before, up to [`LONGEST_CHECK`], and
//! counted over the whole wait: a cause the check does not find, which the rounds meet again, is met no more often
//! than the checks are taken. At a usage limit whose reset was said, by the round that met it or by the latest check,
//! none is taken before the reset. A check speaks only of the account it was taken under: once the owner takes another,
//! the outside is checked at once under the account taken, and what was found under the one before is no check of it.
//! Where the owner has let the engine move, a usage limit of the account in use is also ended by the engine's taking
//! another account that a check under it found well ([`move_on`]): the others are checked one at a time, in the order
//! added, each check kept with the `since` of the wait it was taken in.
//!
//! The acts' wait stands from the latest act, of any work, that failed for want of the network, until an act that began
//! after it comes back, or the host answers it while it is under way: an act says so at the first of its requests that
//! the host answers without an error, as an agent's round does at the service's first message that is no error. A look
//! at the acts' host is taken every half minute: a look is what an act tries, at no cost, and once one reaches the host
//! one act goes first, as one agent's round does once a check finds the outside well. While that act is under way the
//! wait holds every other act; once the host answers it, every act goes on, however long that act runs after.
//!
//! A check of the outside, and a look at the network, is no record: it is what the outside says at a moment, which the
//! driver keeps only while its wait stands. Lost in a stop, it is taken again.
//!
//! What a wait reads of one work's rounds is worked out from the work's records each time the work grows, and the driver
//! keeps it with the work ([`Rounds`]): of the rounds that came back, only the latest to begin under each account, and
//! every round that did not. The wait that stands is then worked out from what every work keeps so, without reading any
//! work's records, and at a cost that does not grow with the rounds that came back.

use std::collections::{BTreeMap, BTreeSet};

use decide::run::Failure;
use decide::{RunId, Seq};
use flow::Outline;
use store::WorkNumber;
use work::{Back, Time, Work};

use crate::accounts::Accounts;

#[cfg(test)]
thread_local! {
    /// How many times a wait every work shares was worked out on this thread ([`outage`]): what a test of what a step
    /// of the driver costs reads.
    pub(crate) static WORKED_OUT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

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
    /// The work whose round met the failure first in the wait: once the outside is found well, the driver gives it the
    /// first turn to go first.
    pub first: WorkNumber,
    /// The work whose round of the service is under way, begun after the latest round that failed so: the one that went
    /// first, while it has not come back. None where no such round is under way.
    pub trying: Option<WorkNumber>,
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

    /// Whether it holds every round it would hold now but one, which goes first: a check of the outside found it well,
    /// or a look at the acts' host reached it, after the latest round that failed so, and that one has begun.
    pub fn goes_first(&self) -> bool {
        !self.holds() && self.trying.is_some()
    }

    /// Whether it holds the rounds of the service now: until a check of the outside finds it well, or a look at the
    /// acts' host reaches it, after the latest round that failed so; and then, while the round that goes first is
    /// under way, every other round ([`Outage::goes_first`]).
    pub fn holds_now(&self) -> bool {
        self.holds() || self.trying.is_some()
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
    limits: impl FnOnce() -> BTreeMap<String, Limit>,
    checked: &BTreeMap<Option<String>, (Time, Checked)>,
    now: Time,
) -> Vec<(String, Candidate)> {
    let Some(in_use) = accounts.in_use() else {
        return Vec::new();
    };
    if outage.service != Service::Agents || !outage.holds() || outage.down().0 != Failure::UsageLimit {
        return Vec::new();
    }
    // The standing limits are worked out from every round of every work, and only for an account no check of this wait
    // speaks of.
    let limits = std::cell::LazyCell::new(limits);
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
    limits: impl FnOnce() -> BTreeMap<String, Limit>,
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
    limits: impl FnOnce() -> BTreeMap<String, Limit>,
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

/// A work's rounds as the waits every work shares read them, of each service: worked out from the work's records each
/// time the work grows, and kept with the work by the driver, so that a wait is worked out from every work without
/// reading any work's records again.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rounds {
    /// Of the cells an agent carries out.
    agents: Served,
    /// Of the engine's own acts.
    host: Served,
}

/// A work's rounds of the cells one service serves.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Served {
    /// Of the rounds that came back, the latest to begin under each account: all that a wait reads of them.
    came: Vec<Latest>,
    /// Each round that did not come back: under way, failed, or dropped as its run was abandoned. In the order of their
    /// runs' starts, and in each run in the order of their numbers.
    open: Vec<Taken>,
    /// When each round began, whether or not it came back: what tells when a wait in which the agents' command line
    /// could not run was over ([`cannot_run_spans`]).
    began: Vec<Time>,
}

/// Of a work's rounds under one account that came back, the latest to begin: when it began, and the place of the entry
/// of its beginning, each the latest of any of them.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Latest {
    /// The account the rounds ran under, as their last tries recorded it; none where they ran under none.
    account: Option<String>,
    began: Time,
    at: Seq,
}

/// A round of a cell that did not come back, as the outage reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Taken {
    work: WorkNumber,
    run: RunId,
    number: usize,
    /// The account its last try ran under; none where it ran under none, or recorded no try.
    account: Option<String>,
    began: Time,
    /// The place of the entry of its beginning, in its work.
    began_at: Seq,
    /// When it ended, and what came back; none while it is under way. What came back is none for a round whose run
    /// was abandoned, when it was dropped, and else is its failure.
    ended: Option<(Time, Seq, Option<Back>)>,
}

impl Rounds {
    /// The rounds of the work of this number, from its records, each served as the version the work is on has its cell,
    /// with the account its last try ran under: none of a work on no version kept, of whose cells that version does not
    /// say which an agent carries out.
    pub fn of(number: WorkNumber, outline: Option<&Outline>, work: &Work) -> Rounds {
        let mut rounds = Rounds::default();
        let Some(outline) = outline else {
            return rounds;
        };
        let r = &work.records;
        // The account of each round, by its run and number, as its last try recorded it: the tries in the order they
        // began.
        let mut accounts: BTreeMap<(RunId, usize), Option<&str>> = BTreeMap::new();
        for ran in &work.ran {
            accounts.insert((ran.run, ran.round), ran.on.account.as_deref());
        }
        // Where each round of each run began, and where each ended, in order: the round of each number ended at the end
        // of that number, where the run has one.
        let mut began: BTreeMap<RunId, Vec<Seq>> = BTreeMap::new();
        for round in &r.rounds {
            began.entry(round.run).or_default().push(round.at);
        }
        let mut ended: BTreeMap<RunId, Vec<Seq>> = BTreeMap::new();
        for end in &r.returned {
            ended.entry(end.run).or_default().push(end.at);
        }
        for started in &r.started {
            let Some(cell) = outline.cell(&started.cell) else {
                continue;
            };
            let served = match cell.agent {
                true => &mut rounds.agents,
                false => &mut rounds.host,
            };
            let ends = ended.get(&started.run).map_or(&[][..], Vec::as_slice);
            for (i, at) in began.get(&started.run).into_iter().flatten().enumerate() {
                let key = (started.run, i + 1);
                let account = accounts.get(&key).copied().flatten();
                let time = work.time_of(*at).unwrap_or_default();
                served.began.push(time);
                let end = (ends.get(i)).map(|end| (work.time_of(*end).unwrap_or_default(), *end, work.back.get(&key)));
                match end {
                    Some((_, _, Some(Back::Asked(_) | Back::HandedIn(_) | Back::Returned { .. }))) => {
                        served.came_back(account, time, *at);
                    }
                    end => served.open.push(Taken {
                        work: number,
                        run: started.run,
                        number: i + 1,
                        account: account.map(str::to_string),
                        began: time,
                        began_at: *at,
                        ended: end.map(|(time, at, back)| (time, at, back.cloned())),
                    }),
                }
            }
        }
        rounds
    }

    /// The rounds of the cells the service serves.
    fn served(&self, service: Service) -> &Served {
        match service {
            Service::Agents => &self.agents,
            Service::Host => &self.host,
        }
    }
}

impl Served {
    /// Takes a round that came back, under this account, that began at this time and place.
    fn came_back(&mut self, account: Option<&str>, began: Time, at: Seq) {
        match (self.came.iter_mut()).find(|latest| latest.account.as_deref() == account) {
            Some(latest) => {
                latest.began = latest.began.max(began);
                latest.at = latest.at.max(at);
            }
            None => self.came.push(Latest {
                account: account.map(str::to_string),
                began,
                at,
            }),
        }
    }

    /// Of the rounds that came back, or that the service answered while they were under way, each account with when one
    /// began and the place of the entry of one's beginning: for those that came back, the latest of each under each
    /// account.
    fn back<'s>(
        &'s self,
        answered: &'s BTreeSet<(WorkNumber, RunId, usize)>,
    ) -> impl Iterator<Item = (Option<&'s str>, Time, Seq)> + 's {
        let came = (self.came.iter()).map(|latest| (latest.account.as_deref(), latest.began, latest.at));
        let answered = (self.open.iter())
            .filter(move |r| answered.contains(&(r.work, r.run, r.number)))
            .map(|r| (r.account.as_deref(), r.began, r.began_at));
        came.chain(answered)
    }
}

/// The wait of the service every work shares, from the rounds of these works ([`Rounds`]), the rounds the service
/// answered while under way, each of a work and run by its number, and the account in use, if one stands. What the
/// outside says of it, its checks and looks, and when the account in use was taken, is the driver's to add.
pub fn outage<'a>(
    service: Service,
    works: impl Iterator<Item = &'a Rounds> + Clone,
    answered: &BTreeSet<(WorkNumber, RunId, usize)>,
    in_use: Option<&str>,
) -> Option<Outage> {
    #[cfg(test)]
    WORKED_OUT.with(|worked_out| worked_out.set(worked_out.get() + 1));
    // A usage limit holds the rounds only while its account is in use.
    let standing: Vec<Met> = (standing(service, works.clone(), answered).into_iter())
        .filter(|m| m.failure != Failure::UsageLimit || m.round.account.as_deref() == in_use)
        .collect();
    // The latest of those not over, which says the cause; of two that ended at one time, the later in its work, and of
    // two works, the one numbered higher, as the driver takes works in their order. Where the latest is over, every
    // earlier one is.
    let key = |m: &&Met| (m.time, m.round.work, m.at);
    let latest = standing.iter().max_by_key(key)?;
    let first = standing.iter().min_by_key(key)?;
    // A round under way that began after the latest: as a round that came back ends the wait, one of another work that
    // began at the same time may have begun before it ended.
    let trying = (works.flat_map(|rounds| rounds.served(service).open.iter()))
        .filter(|r| r.ended.is_none())
        .find(|r| r.began > latest.time || (r.work == latest.round.work && r.began_at > latest.at))
        .map(|r| r.work);
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
        first: first.round.work,
        trying,
        may_move: false,
    })
}

/// The usage limit of each account that stands, by the account's name, whether or not the account is in use, from the
/// rounds of these works ([`Rounds`]) and the rounds the agents' service answered while under way.
pub fn limits<'a>(
    works: impl Iterator<Item = &'a Rounds> + Clone,
    answered: &BTreeSet<(WorkNumber, RunId, usize)>,
) -> BTreeMap<String, Limit> {
    let mut limits: BTreeMap<String, (Limit, Time)> = BTreeMap::new();
    for m in standing(Service::Agents, works, answered) {
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

/// A stretch in which the agents' command line could not run: from when a round of an agent failed so, to when the next
/// round of an agent, of any work, began; none while none has.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CannotRun {
    pub from: Time,
    pub until: Option<Time>,
    /// Why the latest round that failed so in the stretch said.
    pub why: String,
}

/// Each wait in which the agents' command line could not run that is over, from the rounds of these works ([`Rounds`]):
/// it stands from the end of a round of an agent that failed so, a round another work had under way when the first one
/// failed and that failed so too joining it, until the first round of an agent, of any work, that began after the latest
/// of them. The wait that stands now, which no round has begun after, is the driver's ([`crate::Driver::outage`]): what
/// the records of rounds say of it is the same, and it is drawn from the time its first round failed so, whichever kind
/// of cause the failures before it in the wait had. What the failed rounds recorded is kept whole, so that a wait is read
/// the same whenever it is asked (principle `state-from-records`).
pub fn cannot_run_spans<'a>(works: impl Iterator<Item = &'a Rounds> + Clone) -> Vec<CannotRun> {
    let served = works.map(|rounds| &rounds.agents);
    let mut begins: Vec<Time> = (served.clone())
        .flat_map(|served| served.began.iter().copied())
        .collect();
    begins.sort();
    let mut failed: Vec<(Time, &String)> = (served.flat_map(|served| served.open.iter()))
        .filter_map(|r| match &r.ended {
            Some((
                time,
                _,
                Some(Back::Failed {
                    why,
                    failure: Failure::CannotRun,
                    ..
                }),
            )) => Some((*time, why)),
            _ => None,
        })
        .collect();
    failed.sort_by_key(|(time, _)| *time);
    let mut waits: Vec<CannotRun> = Vec::new();
    for (time, why) in failed {
        let until = begins.iter().find(|began| **began > time).copied();
        match waits.last_mut() {
            Some(last) if last.until.is_none_or(|over| time <= over) => {
                last.until = match (last.until, until) {
                    (Some(over), Some(until)) => Some(over.max(until)),
                    _ => None,
                };
                last.why.clone_from(why);
            }
            _ => waits.push(CannotRun {
                from: time,
                until,
                why: why.clone(),
            }),
        }
    }
    waits.retain(|wait| wait.until.is_some());
    waits
}

/// Each round of these works that failed for a cause every work shares and is not over, with when and where in its work
/// it ended. A failure is over where a round that began after it came back, or the service answered it: one that began
/// later, or later in the same work, a round of another work that began at the same time having perhaps begun before it
/// ended; a usage limit only where that round ran under the same account. So it is over where the latest such round of
/// any work, of the account where it is a usage limit, began later, or the latest of its own work began later in it.
fn standing<'r>(
    service: Service,
    works: impl Iterator<Item = &'r Rounds> + Clone,
    answered: &BTreeSet<(WorkNumber, RunId, usize)>,
) -> Vec<Met<'r>> {
    let served = works.map(move |rounds| rounds.served(service));
    // When the latest round that came back, or that the service answered while under way, began under each account, of
    // any work.
    let mut latest: Vec<(Option<&str>, Time)> = Vec::new();
    for (account, began, _) in (served.clone()).flat_map(|served| served.back(answered)) {
        match (latest.iter_mut()).find(|(of, _)| *of == account) {
            Some((_, time)) => *time = (*time).max(began),
            None => latest.push((account, began)),
        }
    }
    let over = |m: &Met| {
        let same = |account: Option<&str>| m.failure != Failure::UsageLimit || account == m.round.account.as_deref();
        (latest.iter()).any(|(account, began)| same(*account) && *began > m.time)
            || (m.served.back(answered)).any(|(account, _, at)| same(account) && at > m.at)
    };
    (served.flat_map(|served| served.open.iter().map(move |round| (served, round))))
        .filter_map(|(served, round)| match &round.ended {
            Some((time, at, Some(Back::Failed { why, failure, resets }))) if failure.shared() => Some(Met {
                served,
                round,
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
    /// The rounds of its work of the service it failed with.
    served: &'a Served,
    round: &'a Taken,
    /// When it ended, and the place of the entry of its end in its work.
    time: Time,
    at: Seq,
    why: &'a String,
    failure: Failure,
    resets: Option<Time>,
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
            first: WorkNumber(1),
            trying: None,
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
        let stands = candidates(&at_limit(None), &accounts, || limits.clone(), &checks, now);
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
            move_on(&outage, &accounts, || limits.clone(), &checks, now),
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
            move_on(&outage, &accounts, || limits.clone(), &checks, now),
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
            move_on(&outage, &accounts, || limits.clone(), &checks, now),
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
            move_on(&outage, &accounts, || limits.clone(), &checks, now),
            Move::Check {
                name: "a".to_string(),
                at: Time(3_000 + spacing(1))
            }
        );
        assert!(may_move(&outage, &accounts, || limits.clone(), &checks, now));
        checks.insert(
            Some("a".to_string()),
            checked(found_down(Failure::CannotRun, None), 3_000, 2),
        );
        assert_eq!(
            move_on(&outage, &accounts, || limits.clone(), &checks, now),
            Move::Stay,
            "none is left"
        );
        assert!(!may_move(&outage, &accounts, || limits.clone(), &checks, now));
    }

    // Microseconds, nothing run: nothing moves where the wait is not at the in-use account's usage limit and holding.
    #[test]
    fn nothing_moves_unless_the_wait_is_the_agents_at_the_usage_limit_of_an_account_in_use_and_holds() {
        let accounts = accounts(&["use", "a"]);
        let (limits, checks, now) = (BTreeMap::new(), BTreeMap::new(), Time(5_000));
        let moves = |outage: &Outage, accounts: &Accounts| move_on(outage, accounts, || limits.clone(), &checks, now);
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
        // The standing limits are worked out from every round of every work: they are not asked for where nothing
        // moves, nor where every account has a check of this wait that speaks of it.
        let never = || -> BTreeMap<String, Limit> { panic!("the standing limits are not asked for") };
        for stay in [&offline, &host, &well, &found_offline] {
            assert_eq!(move_on(stay, &accounts, never, &checks, now), Move::Stay);
            assert!(!may_move(stay, &accounts, never, &checks, now));
        }
        let spoken = BTreeMap::from([(Some("a".to_string()), checked(Found::Well, 3_000, 1))]);
        assert!(matches!(
            move_on(&at_limit(None), &accounts, never, &spoken, now),
            Move::Take { name, .. } if name == "a"
        ));
    }

    /// The accounts a round's try runs under in these tests.
    const ACCOUNTS: [Option<&str>; 3] = [None, Some("p"), Some("q")];

    /// The kinds of cause a round fails for in these tests: those every work shares, and two of a work's own.
    const KINDS: [Failure; 6] = [
        Failure::UsageLimit,
        Failure::CannotRun,
        Failure::Offline,
        Failure::Unavailable,
        Failure::Passing,
        Failure::Other,
    ];

    /// The draws of a test, the same for the same seed.
    struct Draw(u64);

    impl Draw {
        fn new(seed: u64) -> Draw {
            Draw(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
        }

        /// A number below `n`.
        fn below(&mut self, n: u64) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) % n
        }

        fn pick<T: Copy>(&mut self, of: &[T]) -> T {
            of[self.below(of.len() as u64) as usize]
        }
    }

    /// A version with the agent's cell `a` and the engine's act `e`; a run of `x` is of a cell it does not have.
    fn outline() -> Outline {
        let cell = |id: &str, agent: bool| flow::outline::OutlinedCell {
            id: id.into(),
            title: id.to_string(),
            line: agent.then(|| "main".to_string()),
            agent,
            given: Vec::new(),
            watches: Vec::new(),
            reads: Vec::new(),
            words: None,
            questions: None,
        };
        Outline {
            name: "small".to_string(),
            cells: vec![cell("a", true), cell("e", false)],
            lines: Vec::new(),
            groups: Vec::new(),
            card: Vec::new(),
            earlier_ids: Vec::new(),
            own: Vec::new(),
            from: BTreeMap::new(),
        }
    }

    /// What a round that ended with a return came back with.
    fn returned() -> Back {
        Back::Returned {
            dealt_with: Vec::new(),
            handed: Vec::new(),
            asks: Vec::new(),
            found: Vec::new(),
            said: work::Said::default(),
            engine: Vec::new(),
            waits_for: None,
            read: Vec::new(),
            protected: BTreeMap::new(),
        }
    }

    /// A work's records as they grow: each entry at the place after the last, at the time it is given.
    #[derive(Default)]
    struct Growing {
        work: Work,
        next: u64,
    }

    impl Growing {
        /// The next place, for an entry of what happened at this time.
        fn place(&mut self, at: u64) -> Seq {
            self.next += 1;
            let place = Seq(self.next);
            if self.work.moments.last().is_none_or(|moment| moment.at != Time(at)) {
                self.work.moments.push(work::Moment {
                    from: place,
                    at: Time(at),
                });
            }
            place
        }

        /// A run of the cell starts at this time.
        fn start(&mut self, cell: &str, at: u64) -> RunId {
            let run = RunId(self.work.records.started.len() as u64 + 1);
            let at = self.place(at);
            self.work.records.started.push(decide::Started {
                run,
                cell: cell.into(),
                at,
                because: Vec::new(),
                basis: BTreeMap::new(),
            });
            run
        }

        /// The run's next round begins at this time, its tries under these accounts in turn; gives its number.
        fn begin(&mut self, run: RunId, tries: &[Option<&str>], at: u64) -> usize {
            let place = self.place(at);
            self.work.records.rounds.push(decide::Round { run, at: place });
            let round = (self.work.records.rounds.iter()).filter(|r| r.run == run).count();
            for (attempt, account) in tries.iter().enumerate() {
                let at = self.place(at);
                self.work.ran.push(work::Ran {
                    at,
                    run,
                    round,
                    attempt: attempt + 1,
                    on: work::RanOn {
                        level: "1".to_string(),
                        model: "model".to_string(),
                        effort: None,
                        directory: None,
                        account: account.map(str::to_string),
                    },
                });
            }
            round
        }

        /// The round of the run under way ends at this time, with what came back; none where its run was abandoned.
        fn end(&mut self, run: RunId, round: usize, back: Option<Back>, at: u64) {
            let place = self.place(at);
            self.work.records.returned.push(decide::Returned { run, at: place });
            if let Some(back) = back {
                self.work.back.insert((run, round), back);
            }
        }
    }

    /// Works whose rounds began and ended at times of one clock, often at the same time as another's; of the agent's
    /// cell, of the act and of a cell their version does not have; each try under an account drawn, the last try of a
    /// round under another than the one before at times; each round come back, failed for a cause of any kind, dropped
    /// with its run, or under way; and one work in five on no version kept.
    fn drawn(draw: &mut Draw) -> Vec<(WorkNumber, Option<Outline>, Work)> {
        let count = 1 + draw.below(4) as usize;
        let mut works: Vec<(Growing, Option<(RunId, usize)>)> = (0..count).map(|_| Default::default()).collect();
        let mut clock = 1_000;
        for _ in 0..draw.below(60) {
            clock += draw.below(3);
            let (work, under_way) = &mut works[draw.below(count as u64) as usize];
            match under_way.take() {
                Some((run, round)) => {
                    let back = match draw.below(6) {
                        0 => Some(Back::HandedIn(Vec::new())),
                        1 => Some(Back::Asked(Vec::new())),
                        2 => Some(returned()),
                        3 => None,
                        _ => Some(Back::Failed {
                            why: format!("failed at {clock}"),
                            failure: draw.pick(&KINDS),
                            resets: (draw.below(2) == 0).then_some(Time(clock + 100)),
                        }),
                    };
                    work.end(run, round, back, clock);
                }
                None => {
                    let runs: Vec<RunId> = work.work.records.started.iter().map(|s| s.run).collect();
                    let run = match runs.is_empty() || draw.below(3) == 0 {
                        true => work.start(draw.pick(&["a", "a", "e", "x"]), clock),
                        false => draw.pick(&runs),
                    };
                    let tries: Vec<Option<&str>> = (0..=draw.below(2)).map(|_| draw.pick(&ACCOUNTS)).collect();
                    let round = work.begin(run, &tries, clock);
                    *under_way = Some((run, round));
                }
            }
        }
        (works.into_iter().enumerate())
            .map(|(i, (work, _))| {
                let on = (draw.below(5) != 0).then(outline);
                (WorkNumber(i as u64 + 1), on, work.work)
            })
            .collect()
    }

    /// The rounds the service answered while under way, as drawn: one round in four of these works, and one of no work.
    fn answered_of(
        draw: &mut Draw,
        works: &[(WorkNumber, Option<Outline>, Work)],
    ) -> BTreeSet<(WorkNumber, RunId, usize)> {
        let mut answered = BTreeSet::from([(WorkNumber(99), RunId(1), 1)]);
        for (number, _, work) in works {
            for started in &work.records.started {
                let rounds = (work.records.rounds.iter()).filter(|r| r.run == started.run).count();
                for round in 1..=rounds {
                    if draw.below(4) == 0 {
                        answered.insert((*number, started.run, round));
                    }
                }
            }
        }
        answered
    }

    /// A round of a cell the service serves, as the waits are defined on the records alone.
    struct Plain {
        work: WorkNumber,
        run: RunId,
        number: usize,
        account: Option<String>,
        began: Time,
        began_at: Seq,
        ended: Option<(Time, Seq, Option<Back>)>,
    }

    /// A round that failed for a cause every work shares and that nothing came back after, as the waits are defined.
    struct PlainMet<'r> {
        round: &'r Plain,
        time: Time,
        at: Seq,
        why: &'r String,
        failure: Failure,
        resets: Option<Time>,
    }

    /// When the entry at this place was added: the time of the latest moment that begins at or before it.
    fn when(work: &Work, place: Seq) -> Time {
        (work.moments.iter().rev())
            .find(|moment| moment.from <= place)
            .map_or(Time::default(), |moment| moment.at)
    }

    /// Every round of the works on a version kept of the cells the service serves there, in the order of the works, of
    /// their runs' starts and of the rounds' numbers, with the account its last try ran under.
    fn plain(service: Service, works: &[(WorkNumber, Option<Outline>, Work)]) -> Vec<Plain> {
        let mut rounds = Vec::new();
        for (number, on, work) in works {
            let Some(outline) = on else {
                continue;
            };
            for started in &work.records.started {
                let agent = service == Service::Agents;
                if !outline.cell(&started.cell).is_some_and(|cell| cell.agent == agent) {
                    continue;
                }
                let ends: Vec<Seq> = (work.records.returned.iter())
                    .filter(|end| end.run == started.run)
                    .map(|end| end.at)
                    .collect();
                let begins = (work.records.rounds.iter()).filter(|round| round.run == started.run);
                for (i, round) in begins.enumerate() {
                    let account = (work.ran.iter().rev())
                        .find(|ran| ran.run == started.run && ran.round == i + 1)
                        .and_then(|ran| ran.on.account.clone());
                    let back = work.back.get(&(started.run, i + 1)).cloned();
                    rounds.push(Plain {
                        work: *number,
                        run: started.run,
                        number: i + 1,
                        account,
                        began: when(work, round.at),
                        began_at: round.at,
                        ended: ends.get(i).map(|at| (when(work, *at), *at, back)),
                    });
                }
            }
        }
        rounds
    }

    /// Each round that failed for a cause every work shares, with no round that came back, or that the service
    /// answered, beginning after it: later, or later in its own work; under the same account, for a usage limit.
    fn plain_standing<'r>(rounds: &'r [Plain], answered: &BTreeSet<(WorkNumber, RunId, usize)>) -> Vec<PlainMet<'r>> {
        let back = |r: &Plain| {
            let came = matches!(
                &r.ended,
                Some((_, _, Some(Back::Asked(_) | Back::HandedIn(_) | Back::Returned { .. })))
            );
            came || answered.contains(&(r.work, r.run, r.number))
        };
        (rounds.iter())
            .filter_map(|round| match &round.ended {
                Some((time, at, Some(Back::Failed { why, failure, resets }))) if failure.shared() => Some(PlainMet {
                    round,
                    time: *time,
                    at: *at,
                    why,
                    failure: *failure,
                    resets: *resets,
                }),
                _ => None,
            })
            .filter(|m| {
                !(rounds.iter()).filter(|b| back(b)).any(|b| {
                    let same = m.failure != Failure::UsageLimit || b.account == m.round.account;
                    same && (b.began > m.time || (b.work == m.round.work && b.began_at > m.at))
                })
            })
            .collect()
    }

    /// The wait of the service, as it is defined on the records alone.
    fn plain_outage(
        service: Service,
        works: &[(WorkNumber, Option<Outline>, Work)],
        answered: &BTreeSet<(WorkNumber, RunId, usize)>,
        in_use: Option<&str>,
    ) -> Option<Outage> {
        let rounds = plain(service, works);
        let standing: Vec<PlainMet> = (plain_standing(&rounds, answered).into_iter())
            .filter(|m| m.failure != Failure::UsageLimit || m.round.account.as_deref() == in_use)
            .collect();
        let key = |m: &&PlainMet| (m.time, m.round.work, m.at);
        let latest = standing.iter().max_by_key(key)?;
        let first = standing.iter().min_by_key(key)?;
        let trying = (rounds.iter())
            .filter(|r| r.ended.is_none())
            .find(|r| r.began > latest.time || (r.work == latest.round.work && r.began_at > latest.at))
            .map(|r| r.work);
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
            first: first.round.work,
            trying,
            may_move: false,
        })
    }

    /// The usage limits that stand, as they are defined on the records alone: from the first round under the account
    /// that met it, with the reset the latest said, of two at one time the later in the order of the rounds.
    fn plain_limits(
        works: &[(WorkNumber, Option<Outline>, Work)],
        answered: &BTreeSet<(WorkNumber, RunId, usize)>,
    ) -> BTreeMap<String, Limit> {
        let rounds = plain(Service::Agents, works);
        let mut limits: BTreeMap<String, (Limit, Time)> = BTreeMap::new();
        for m in plain_standing(&rounds, answered) {
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

    /// The waits in which the agents' command line could not run that are over, as they are defined on the records
    /// alone.
    fn plain_cannot_run(works: &[(WorkNumber, Option<Outline>, Work)]) -> Vec<CannotRun> {
        let rounds = plain(Service::Agents, works);
        let mut begins: Vec<Time> = rounds.iter().map(|r| r.began).collect();
        begins.sort();
        let mut failed: Vec<(Time, &String)> = (rounds.iter())
            .filter_map(|r| match &r.ended {
                Some((
                    time,
                    _,
                    Some(Back::Failed {
                        why,
                        failure: Failure::CannotRun,
                        ..
                    }),
                )) => Some((*time, why)),
                _ => None,
            })
            .collect();
        failed.sort_by_key(|(time, _)| *time);
        let mut waits: Vec<CannotRun> = Vec::new();
        for (time, why) in failed {
            let until = begins.iter().find(|began| **began > time).copied();
            match waits.last_mut() {
                Some(last) if last.until.is_none_or(|over| time <= over) => {
                    last.until = match (last.until, until) {
                        (Some(over), Some(until)) => Some(over.max(until)),
                        _ => None,
                    };
                    last.why.clone_from(why);
                }
                _ => waits.push(CannotRun {
                    from: time,
                    until,
                    why: why.clone(),
                }),
            }
        }
        waits.retain(|wait| wait.until.is_some());
        waits
    }

    // Milliseconds in all, nothing run: in each of 400 drawn histories of a few works, the wait of each service under
    // each account in use, the usage limits that stand and the waits in which the command line could not run, worked out
    // from what each work keeps of its rounds, are those the works' records define.
    #[test]
    fn the_waits_worked_out_from_what_each_work_keeps_are_those_its_records_define() {
        for seed in 0..400 {
            let mut draw = Draw::new(seed);
            let works = drawn(&mut draw);
            let answered = answered_of(&mut draw, &works);
            let kept: Vec<Rounds> = (works.iter())
                .map(|(number, on, work)| Rounds::of(*number, on.as_ref(), work))
                .collect();
            for service in [Service::Agents, Service::Host] {
                for in_use in ACCOUNTS {
                    assert_eq!(
                        outage(service, kept.iter(), &answered, in_use),
                        plain_outage(service, &works, &answered, in_use),
                        "seed {seed}, {service:?}, {in_use:?} in use"
                    );
                }
            }
            assert_eq!(
                limits(kept.iter(), &answered),
                plain_limits(&works, &answered),
                "seed {seed}"
            );
            assert_eq!(cannot_run_spans(kept.iter()), plain_cannot_run(&works), "seed {seed}");
        }
    }

    // Microseconds, nothing run: of a work's two hundred rounds that came back, under two accounts, it keeps for the
    // waits the latest to begin under each, and each round that did not come back, so that what a wait reads of every
    // work does not grow with the rounds that came back.
    #[test]
    fn a_work_keeps_of_its_rounds_that_came_back_only_the_latest_under_each_account() {
        let mut growing = Growing::default();
        let run = growing.start("a", 1_000);
        for i in 0..200 {
            let round = growing.begin(run, &[ACCOUNTS[1 + i % 2]], 1_000 + 10 * i as u64);
            growing.end(run, round, Some(Back::HandedIn(Vec::new())), 1_005 + 10 * i as u64);
        }
        let round = growing.begin(run, &[Some("p")], 3_000);
        let failed = Back::Failed {
            why: "offline".to_string(),
            failure: Failure::Offline,
            resets: None,
        };
        growing.end(run, round, Some(failed.clone()), 3_001);
        let work = growing.work;
        let rounds = Rounds::of(WorkNumber(1), Some(&outline()), &work);
        let latest = |account: &str, nth: usize| Latest {
            account: Some(account.to_string()),
            began: when(&work, work.records.rounds[nth].at),
            at: work.records.rounds[nth].at,
        };
        assert_eq!(rounds.agents.came, vec![latest("p", 198), latest("q", 199)]);
        let open: Vec<(usize, Option<&str>, Option<&Back>)> = (rounds.agents.open.iter())
            .map(|r| {
                (
                    r.number,
                    r.account.as_deref(),
                    r.ended.as_ref().and_then(|e| e.2.as_ref()),
                )
            })
            .collect();
        assert_eq!(open, vec![(201, Some("p"), Some(&failed))]);
        assert_eq!(rounds.agents.began.len(), 201);
        assert_eq!(rounds.host, Served::default());
        assert_eq!(
            Rounds::of(WorkNumber(1), None, &work),
            Rounds::default(),
            "on no version kept"
        );
    }
}
