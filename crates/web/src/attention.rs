//! `GET /api/v1/attention`: what needs whoever plans the work, for the command `wf-agents wait`, as one answer to one
//! request.
//!
//! What it gives is everything the owner's page shows as needing someone, read from the same items the page counts
//! from ([`ItemsCache`]), so that the two never disagree: each thing a work waits on the owner for, as the page counts
//! them in red, and each question of a work that has not ended the owner may answer ahead, as the page counts them in
//! orange; a work at fault, stalled, or whose state cannot be worked out, which the page marks halted; and, from the
//! records of every work ([`driver::attention`]), the pause of everything, the wait every work shares, a project's red
//! main, and a pause of the owner's whose items have all ended. Beside them, what holds work back without anyone being
//! asked: a project's turn to check and merge, held by a work whose round or call has been under way longer than
//! [`Times::turn`] while other works wait for it; a round or call that has waited for a runner longer than
//! [`Times::queued`]; slots free while items could be let on, as `wf-agents candidates` counts them; an item that has
//! not ended, is not a deliverable, and that no item that has not ended depends on, so that it stands at a root; and a
//! work the owner paused without saying why. Two things are said once, as they come, and never again: a work that
//! ended, and a work that came to wait for items none of which is going on.
//!
//! Everything else stands until it is handled. Each thing is given at once where it came to stand since the read the
//! mark was given at, and everything that stands is given whole, most pressing first, at the first request and again
//! each time [`Times::again`] has passed since it was last given whole, until it is gone. Each line says since when it
//! has stood: a thing that stands once some time has passed, as a turn held too long, came to stand when that time had
//! passed, and says since when what it is about began.
//!
//! `since` is the mark the last answer gave; with none, what stands is given whole, the works that ended being taken as
//! read. `wait` is how many seconds at most the request is held open while there is nothing: it is answered as soon as
//! there is something, which may be what stands coming due again, or with no line and the mark to give next time once
//! that time has passed; with none, it is answered at once. While it is held, what the engine holds is read again each
//! time something happens, and at each time something comes to stand by time alone.
//!
//! An answer holds at most [`LINES`] lines: where more are due, it says how many it left out, and the next answer, to
//! its mark, gives them at once.
//!
//! A mark that cannot be read, or one later than the engine's clock, is refused with 400; a driver that stopped, and an
//! engine whose loop has ended, answer 503; what cannot be shown, which the engine never makes, 500.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use driver::Driver;
use driver::attention::{self as of, Heed};
use serde::{Deserialize, Serialize};
use service::{Changed, Handle};
use store::WorkNumber;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::time::Instant;
use work::Time;

use crate::interface::{fail, refused};
use crate::items_cache::ItemsCache;
use crate::system::RedTest;
use crate::works::{Broken, Call, Item, ItemChanged, Now, Outage, Outages, Pauser, QuestionPage, Wait, Waited};

/// The most lines an answer holds; how many more there are is said, and they come at the next request.
pub const LINES: usize = 20;

/// The times `wait` goes by, in milliseconds, as the installation's settings give them (`wait` in `install.toml`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Times {
    /// How long after what stands was last given whole it is given whole again.
    pub again: u64,
    /// How long a work's round or call may be under way while the work holds its project's turn to check and merge and
    /// others wait for it, before that is something to handle.
    pub turn: u64,
    /// How long a round or call may wait for a runner before that is something to handle.
    pub queued: u64,
}

/// What needs the planner: what came since the mark, or what stands, given whole.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attention {
    /// Whether the answer gives what stands whole, as it does with no mark and each time [`Times::again`] has passed,
    /// besides what came since the mark.
    pub whole: bool,
    /// At most [`LINES`] of them: what stands, most pressing first, then the works that ended and those that came to
    /// wait for items none of which is going on, oldest first.
    pub lines: Vec<Line>,
    /// How many more there are, which come at the next request with `mark`.
    pub left_out: usize,
    /// The mark to give next time.
    pub mark: String,
    /// How long after it was given whole what stands is given whole again, in milliseconds.
    pub again: u64,
}

/// Something for the planner, and since when it stands, in milliseconds since the start of 1970.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Line {
    /// Everything was paused, at this time, by the owner or by the engine as it started with works going on: no work
    /// begins a round but to answer a person's words to its cell, until the owner lifts it. It is about no work.
    Everything { since: u64, by: Pauser },
    /// No agent's round of any work begins: a round failed for a cause every work shares. It is about no work. Where
    /// the cause is the agents' command line, which cannot run until the owner mends it, what the owner does, in the
    /// words beside the flow of the work whose round met it (`owner.mend`), said here alone.
    Agents {
        since: u64,
        outage: Outage,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mend: Option<String>,
    },
    /// The main branch of the project is red on these tests, which the check before a merge found failing on its head,
    /// since the time its red began; these works wait for it to pass. It is about no work.
    MainRed {
        since: u64,
        project: String,
        tests: Vec<RedTest>,
        works: Vec<Of>,
    },
    /// The work's records cannot be those of a work.
    Fault { of: Of, since: u64, now: Now },
    /// The engine cannot work out what the work does now ([`Now::Broken`]), since the last thing that happened to it.
    Broken { of: Of, since: u64, now: Now },
    /// The work is stalled: no cell will write what it waits for, and nothing pauses it.
    Stalled { of: Of, since: u64, now: Now },
    /// The work waits on the owner, for each of these, as the page counts them: since it came to as its records read
    /// it, else since the wait every work shares began where it waits in that, else since the last thing that happened
    /// to it.
    ForAPerson {
        of: Of,
        since: u64,
        now: Now,
        waits: Vec<Wait>,
    },
    /// The project's turn to check and merge is held by this work, whose round or call has been under way since this
    /// time, as it does now, while these works wait for the turn.
    Turn {
        since: u64,
        project: Option<String>,
        holder: Of,
        now: Now,
        waiting: Vec<Of>,
    },
    /// A round or call of the work has waited for this runner since this time, as what carries it out has said since
    /// the engine started.
    Queued {
        of: Of,
        since: u64,
        runner: String,
        now: Now,
    },
    /// The owner may answer these questions of the work ahead, as the page counts them, since the oldest was asked.
    Ahead { of: Of, since: u64, questions: Vec<Asked> },
    /// The owner's pause of the work waited for these items to merge or be cancelled, and each has: the work is paused
    /// still, and only the owner lets it on.
    PauseOver { of: Of, since: u64, items: Vec<u64> },
    /// The owner paused the work, at this time, and said no reason.
    Unexplained { of: Of, since: u64, now: Now },
    /// Of so many slots, so many are free, while so many items could be let on, the first of them since this time.
    Slots {
        since: u64,
        slots: u64,
        free: u64,
        items: usize,
    },
    /// The item has not ended, is not a deliverable, and no item that has not ended depends on it, since this time: it
    /// stands at a root.
    Root { of: Of, since: u64 },
    /// The work waits for these items, none of which is going on.
    Idle {
        of: Of,
        since: u64,
        now: Now,
        items: Vec<Idle>,
    },
    /// The work ended, merged or cancelled.
    Ended { of: Of, since: u64, now: Now },
}

/// The work a line is of, with its item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Of {
    pub work: u64,
    pub item: u64,
    pub title: String,
}

/// A question the owner may answer ahead: its name, and when its latest version was asked.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asked {
    pub name: String,
    pub since: u64,
}

/// An item waited for that is not going on, and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Idle {
    pub item: u64,
    pub why: Stuck,
}

/// Why an item waited for is not going on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Stuck {
    /// The owner paused its work, or it was added paused and not let on.
    Paused,
    /// Its work waits on the owner.
    ForAPerson,
    /// Its work is stalled.
    Stalled,
    /// Its work waits in turn for items none of which is going on.
    Idle,
    /// Its work's records cannot be those of a work.
    Fault,
}

impl Stuck {
    fn of(why: of::Stuck) -> Stuck {
        match why {
            of::Stuck::Paused => Stuck::Paused,
            of::Stuck::ForAPerson => Stuck::ForAPerson,
            of::Stuck::Stalled => Stuck::Stalled,
            of::Stuck::Idle => Stuck::Idle,
            of::Stuck::Fault => Stuck::Fault,
        }
    }
}

/// A place in what was given: the time of the read an answer was made from, what came to stand at or after which is new
/// to the next answer; when what stands is next given whole; where an answer giving what stands whole was cut, how many
/// of it, from the most pressing, were given, so that the next gives the rest, else nought; and where an answer was cut
/// before a work that ended or came to be idle, that one, by when it did and its number, else none, the next giving
/// those from the time of the read. Written as those numbers joined by dots, the last two only where there is such a
/// work: `1800000000000.1800000300000.0`, `1800000000000.1800000000000.20.1799999000000.7`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    read: Time,
    due: Time,
    from: usize,
    told: Option<(Time, u64)>,
}

/// Why a text is no mark, in words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unreadable(pub String);

impl Mark {
    /// Reads a mark as [`Mark`]'s text writes it.
    pub fn read(text: &str) -> Result<Mark, Unreadable> {
        let unreadable = || {
            Unreadable(format!(
                "{text:?} is not a mark: a mark is three or five numbers joined by dots, such as \
                 1800000000000.1800000300000.0"
            ))
        };
        let parts = (text.trim().split('.'))
            .map(|part| {
                if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(unreadable());
                }
                part.parse::<u64>().map_err(|_| unreadable())
            })
            .collect::<Result<Vec<u64>, Unreadable>>()?;
        let from = |n: u64| usize::try_from(n).map_err(|_| unreadable());
        match parts[..] {
            [read, due, n] => Ok(Mark {
                read: Time(read),
                due: Time(due),
                from: from(n)?,
                told: None,
            }),
            [read, due, n, at, work] => Ok(Mark {
                read: Time(read),
                due: Time(due),
                from: from(n)?,
                told: Some((Time(at), work)),
            }),
            _ => Err(unreadable()),
        }
    }

    /// From where the works that ended or came to be idle are new to the answer to this mark.
    fn told_from(&self) -> (Time, u64) {
        self.told.unwrap_or((self.read, 0))
    }
}

impl std::fmt::Display for Mark {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.read.0, self.due.0, self.from)?;
        if let Some((at, work)) = self.told {
            write!(f, ".{}.{work}", at.0)?;
        }
        Ok(())
    }
}

/// One thing for the planner, as an answer chooses among them: its line; how pressing it is, the least the most; when
/// it came to stand, which says whether it is new to a mark; whether it is told once as it comes, and given whole with
/// nothing else; and the work it is of, which orders things alike.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Thing {
    pub(crate) line: Line,
    pub(crate) rank: u8,
    pub(crate) came: Time,
    pub(crate) once: bool,
    pub(crate) work: u64,
}

impl Line {
    /// Since when it stands.
    pub fn since(&self) -> u64 {
        match self {
            Line::Everything { since, .. }
            | Line::Agents { since, .. }
            | Line::MainRed { since, .. }
            | Line::Fault { since, .. }
            | Line::Broken { since, .. }
            | Line::Stalled { since, .. }
            | Line::ForAPerson { since, .. }
            | Line::Turn { since, .. }
            | Line::Queued { since, .. }
            | Line::Ahead { since, .. }
            | Line::PauseOver { since, .. }
            | Line::Unexplained { since, .. }
            | Line::Slots { since, .. }
            | Line::Root { since, .. }
            | Line::Idle { since, .. }
            | Line::Ended { since, .. } => *since,
        }
    }
}

/// What an answer gives, and the mark to give next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Given {
    pub(crate) whole: bool,
    pub(crate) lines: Vec<Line>,
    pub(crate) left_out: usize,
    pub(crate) mark: Mark,
}

/// What of these things the answer to the mark gives at this time, what stands being given whole again this long after
/// it was last given whole. With no mark, what stands, whole, and nothing that is told once: the works that ended are
/// taken as read. With one, what came to stand at or after its read, and what is told once from where it says; and
/// what stands, whole, where it is due or an answer before was cut while giving it.
pub(crate) fn answer(things: Vec<Thing>, mark: Option<&Mark>, now: Time, again: u64) -> Given {
    let (mut stands, mut once): (Vec<Thing>, Vec<Thing>) = things.into_iter().partition(|thing| !thing.once);
    stands.sort_by_key(|thing| (thing.rank, thing.line.since(), thing.work));
    once.sort_by_key(|thing| (thing.came, thing.work));
    let whole = mark.is_none_or(|mark| mark.from > 0 || now >= mark.due);
    let from = mark.map_or(0, |mark| mark.from);
    let new = |thing: &Thing| mark.is_some_and(|mark| thing.came >= mark.read);
    let due: Vec<(usize, &Thing)> = (stands.iter().enumerate())
        .filter(|(at, thing)| (whole && *at >= from) || new(*thing))
        .collect();
    let told: Vec<&Thing> = match mark {
        None => Vec::new(),
        Some(mark) => (once.iter())
            .filter(|thing| (thing.came, thing.work) >= mark.told_from())
            .collect(),
    };
    let given = due.len().min(LINES);
    let room = LINES - given;
    let told_given = told.len().min(room);
    let (left, told_left) = (due.len() - given, told.len() - told_given);
    let (due_next, from_next) = match (whole, left > 0) {
        // The rest of what stands comes at once, from after the last given.
        (true, true) => (now, due[given - 1].0.max(from) + 1),
        (true, false) => (Time(now.0.saturating_add(again)), 0),
        // What came is more than an answer holds: what stands comes whole at once, which holds it.
        (false, true) => (now, 0),
        (false, false) => (mark.map_or(now, |mark| mark.due), 0),
    };
    let told_next = (told_left > 0).then(|| match told_given {
        0 => mark.map_or((now, 0), Mark::told_from),
        n => (told[n - 1].came, told[n - 1].work + 1),
    });
    let lines = (due.iter().take(given).map(|(_, thing)| thing.line.clone()))
        .chain(told.iter().take(told_given).map(|thing| thing.line.clone()))
        .collect();
    Given {
        whole,
        lines,
        left_out: left + told_left,
        mark: Mark {
            read: now,
            due: due_next,
            from: from_next,
            told: told_next,
        },
    }
}

/// Every thing for the planner, as the items the page counts from and the driver's records say them at this time, and
/// the first time after it at which one comes to stand by time alone, where one will.
pub(crate) struct Gathered {
    pub(crate) things: Vec<Thing>,
    pub(crate) next: Option<Time>,
}

/// Why no answer could be given.
pub(crate) enum Failed {
    /// The mark is later than the engine's clock: no answer of this engine gave it.
    Later,
    /// The driver stopped, by this fault.
    Stopped(driver::Fault),
    /// What the records name the driver does not hold.
    Broken(Broken),
}

/// Gathers every thing for the planner from the items as the page reads them, checked against the driver first, and
/// from the driver's own lines and records, at this time, going by these times.
pub(crate) fn gather(cache: &Arc<ItemsCache>, driver: &Driver, now: Time, times: &Times) -> Result<Gathered, Failed> {
    let heeds = driver.attention().map_err(Failed::Stopped)?;
    cache.check(driver, now).map_err(Failed::Broken)?;
    let items = cache.items();
    let by_work: BTreeMap<u64, &Item> = items.iter().map(|item| (item.work.number, item)).collect();
    let by_item: BTreeMap<u64, &Item> = items.iter().map(|item| (item.number, item)).collect();
    let at_once = driver.at_once();
    let outages = Outages::of(&at_once);
    let mut things = Vec::new();
    // Since when each work has waited on the owner, and when each work that ended did, as the records read them.
    let mut owner_since: BTreeMap<u64, Time> = BTreeMap::new();
    let mut ended: BTreeMap<u64, Time> = BTreeMap::new();
    let thing = |line: Line, rank: u8, came: Time, work: u64| Thing {
        line,
        rank,
        came,
        once: false,
        work,
    };
    for heeded in heeds {
        let since = heeded.since;
        match heeded.heed {
            Heed::Everything { by } => things.push(thing(
                Line::Everything {
                    since: since.0,
                    by: Pauser::of(by),
                },
                0,
                since,
                0,
            )),
            Heed::Agents { outage } => {
                let mend = (outage.down().0 == decide::run::Failure::CannotRun)
                    .then(|| driver.work(outage.work))
                    .flatten()
                    .map(|kept| kept.flow.waits.owner.mend.clone());
                things.push(thing(
                    Line::Agents {
                        since: since.0,
                        outage: Outage::of(&outage, at_once.gate().holds_agents()),
                        mend,
                    },
                    0,
                    since,
                    outage.work.0,
                ));
            }
            Heed::MainRed { project, tests, works } => {
                let works = (works.iter())
                    .map(|work| item_of(&by_work, work.0).map(of_item))
                    .collect::<Result<_, _>>()?;
                things.push(thing(
                    Line::MainRed {
                        since: since.0,
                        project,
                        tests: tests.iter().map(RedTest::of).collect(),
                        works,
                    },
                    0,
                    since,
                    0,
                ));
            }
            Heed::Fault { work } => {
                let item = item_of(&by_work, work.0)?;
                let line = Line::Fault {
                    of: of_item(item),
                    since: since.0,
                    now: item.work.now.clone(),
                };
                things.push(thing(line, 1, since, work.0));
            }
            Heed::Stalled { work } => {
                let item = item_of(&by_work, work.0)?;
                let line = Line::Stalled {
                    of: of_item(item),
                    since: since.0,
                    now: item.work.now.clone(),
                };
                things.push(thing(line, 1, since, work.0));
            }
            Heed::ForAPerson { work } => {
                owner_since.insert(work.0, since);
            }
            Heed::PauseOver { work, items } => {
                let line = Line::PauseOver {
                    of: of_item(item_of(&by_work, work.0)?),
                    since: since.0,
                    items: items.iter().map(|item| item.0).collect(),
                };
                things.push(thing(line, 5, since, work.0));
            }
            Heed::Idle { work, items } => {
                let item = item_of(&by_work, work.0)?;
                let line = Line::Idle {
                    of: of_item(item),
                    since: since.0,
                    now: item.work.now.clone(),
                    items: (items.iter())
                        .map(|idle| Idle {
                            item: idle.item.0,
                            why: Stuck::of(idle.why),
                        })
                        .collect(),
                };
                things.push(Thing {
                    once: true,
                    ..thing(line, 8, since, work.0)
                });
            }
            Heed::Ended { work, .. } => {
                ended.insert(work.0, since);
                let item = item_of(&by_work, work.0)?;
                let line = Line::Ended {
                    of: of_item(item),
                    since: since.0,
                    now: item.work.now.clone(),
                };
                things.push(Thing {
                    once: true,
                    ..thing(line, 8, since, work.0)
                });
            }
        }
    }
    let shared_since = outages.agents.as_ref().map(|outage| Time(outage.since));
    let mut next: Option<Time> = None;
    let mut wake = |at: Time| next = Some(next.map_or(at, |next: Time| next.min(at)));
    // Who waits for each work's turn to check and merge, with since when each has wanted it.
    let mut turns: BTreeMap<u64, Vec<(Of, Time)>> = BTreeMap::new();
    for item in &items {
        let s = &item.work;
        let work = s.number;
        let going = !matches!(s.now, Now::Merged | Now::Cancelled);
        let last = Time(s.last.unwrap_or(s.added));
        if !s.waits.is_empty() {
            let since = (owner_since.get(&work).copied())
                .or_else(|| shared_since.filter(|_| s.waits.iter().any(|wait| wait.key == driver::owner::AGENTS)))
                .unwrap_or(last);
            let line = Line::ForAPerson {
                of: of_item(item),
                since: since.0,
                now: s.now.clone(),
                waits: s.waits.clone(),
            };
            things.push(thing(line, 2, since, work));
        }
        if matches!(s.now, Now::Broken { .. }) {
            let line = Line::Broken {
                of: of_item(item),
                since: last.0,
                now: s.now.clone(),
            };
            things.push(thing(line, 1, last, work));
        }
        if !going {
            continue;
        }
        if s.ahead > 0 {
            let questions = ahead(driver, item)?;
            if let (Some(oldest), Some(newest)) = (
                questions.iter().map(|q| q.since).min(),
                questions.iter().map(|q| q.since).max(),
            ) {
                let line = Line::Ahead {
                    of: of_item(item),
                    since: oldest,
                    questions,
                };
                things.push(thing(line, 4, Time(newest), work));
            }
        }
        if let Some(since) = unexplained(driver, work) {
            let line = Line::Unexplained {
                of: of_item(item),
                since: since.0,
                now: s.now.clone(),
            };
            things.push(thing(line, 5, since, work));
        }
        if let Now::Waits { on, .. } = &s.now {
            for waited in on {
                if let Waited::Turn { holder, since } = waited {
                    turns.entry(*holder).or_default().push((of_item(item), Time(*since)));
                }
            }
        }
        for (_, runner, since) in driver.queued_of(WorkNumber(work)) {
            let came = Time(since.0.saturating_add(times.queued));
            if came > now {
                wake(came);
                continue;
            }
            let line = Line::Queued {
                of: of_item(item),
                since: since.0,
                runner: runner.to_string(),
                now: s.now.clone(),
            };
            things.push(thing(line, 3, came, work));
        }
        if !s.delivers && root(item, &items) {
            let since = root_since(item, &items, &ended);
            things.push(thing(
                Line::Root {
                    of: of_item(item),
                    since: since.0,
                },
                7,
                since,
                work,
            ));
        }
    }
    for (holder, waiting) in turns {
        let held = item_of(&by_work, holder)?;
        let Some(under_way) = under_way(&held.work.now) else {
            continue;
        };
        let wanted = waiting.iter().map(|(_, since)| *since).min().unwrap_or(under_way);
        let came = Time(under_way.0.saturating_add(times.turn)).max(wanted);
        if came > now {
            wake(came);
            continue;
        }
        let line = Line::Turn {
            since: under_way.0,
            project: held.project.clone(),
            holder: of_item(held),
            now: held.work.now.clone(),
            waiting: waiting.into_iter().map(|(of, _)| of).collect(),
        };
        things.push(thing(line, 3, came, holder));
    }
    let slots = driver.slots();
    let free = slots.slots.saturating_sub(slots.taken);
    let candidates = crate::plan::could_be_let_on(driver);
    if free > 0 && !candidates.is_empty() {
        let ready = (candidates.iter())
            .map(|item| ready_since(driver, *item, &by_item, &ended))
            .collect::<Result<Vec<Time>, Failed>>()?;
        if let (Some(first), Some(last)) = (ready.iter().min(), ready.iter().max()) {
            let line = Line::Slots {
                since: first.0,
                slots: slots.slots as u64,
                free: free as u64,
                items: candidates.len(),
            };
            things.push(thing(line, 6, *last, 0));
        }
    }
    Ok(Gathered { things, next })
}

/// The item whose work is of this number.
fn item_of<'a>(by_work: &BTreeMap<u64, &'a Item>, work: u64) -> Result<&'a Item, Failed> {
    (by_work.get(&work).copied()).ok_or_else(|| Failed::Broken(Broken(format!("there is no item of work {work}"))))
}

/// The work of an item, as a line names it.
fn of_item(item: &Item) -> Of {
    Of {
        work: item.work.number,
        item: item.number,
        title: item.title.clone(),
    }
}

/// The questions of the item's work the owner may answer ahead, as the page counts them ([`QuestionPage::ahead`]),
/// each with when its latest version was asked.
fn ahead(driver: &Driver, item: &Item) -> Result<Vec<Asked>, Failed> {
    let work = WorkNumber(item.work.number);
    let kept = (driver.work(work)).ok_or_else(|| Failed::Broken(Broken(format!("there is no work {work}"))))?;
    let questions = QuestionPage::all(&kept).map_err(Failed::Broken)?;
    let standing = decide::questions::questions(&kept.work.records);
    let asked = |name: &str| -> Result<u64, Failed> {
        (standing.get(name))
            .and_then(|q| kept.work.time_of(q.latest.at))
            .map(|at| at.0)
            .ok_or_else(|| Failed::Broken(Broken(format!("{work} has no time its question {name} was asked at"))))
    };
    (QuestionPage::ahead_of(&questions, &item.work.waits).into_iter())
        .map(|q| -> Result<Asked, Failed> {
            Ok(Asked {
                name: q.name.clone(),
                since: asked(&q.name)?,
            })
        })
        .collect()
}

/// When the owner paused the work and said no reason, where that is its pause.
fn unexplained(driver: &Driver, work: u64) -> Option<Time> {
    let kept = driver.work(WorkNumber(work))?;
    let pause = decide::pause(&kept.work.records)?;
    let said = pause.why.as_deref().is_some_and(|why| !why.trim().is_empty());
    (pause.by == decide::Pauser::Owner && !said)
        .then(|| kept.work.time_of(pause.at))
        .flatten()
}

/// Since when what the work does now has been under way, where it is a round, or the calls of tools the engine carries
/// out that it waits for, from the first made.
fn under_way(now: &Now) -> Option<Time> {
    match now {
        Now::Round { since, .. } => Some(Time(*since)),
        Now::Waits { on, .. } => (on.iter())
            .filter_map(|waited| match waited {
                Waited::Call {
                    who: None,
                    what: Call::Tool { .. },
                    made,
                    ..
                } if *made > 0 => Some(Time(*made)),
                _ => None,
            })
            .min(),
        _ => None,
    }
}

/// Whether no item that has not ended depends on the item.
fn root(item: &Item, items: &[Item]) -> bool {
    !(items.iter())
        .any(|other| !matches!(other.work.now, Now::Merged | Now::Cancelled) && other.depends_on.contains(&item.number))
}

/// Since when the item has stood at a root: since it was added, or since the last item that depended on it ended or
/// stopped depending on it.
fn root_since(item: &Item, items: &[Item], ended: &BTreeMap<u64, Time>) -> Time {
    let mut since = Time(item.added);
    for other in items {
        if other.depends_on.contains(&item.number)
            && let Some(at) = ended.get(&other.work.number)
        {
            since = since.max(*at);
        }
        for change in &other.history.changes {
            if change.what == (ItemChanged::NoLongerDependsOn { item: item.number }) {
                since = since.max(Time(change.time));
            }
        }
    }
    since
}

/// Since when the item could be let on: the latest of when its work came to be paused, as it is now, and when each item
/// it depends on, and each its pause waits for, ended.
fn ready_since(
    driver: &Driver,
    item: u64,
    by_item: &BTreeMap<u64, &Item>,
    ended: &BTreeMap<u64, Time>,
) -> Result<Time, Failed> {
    let of = (by_item.get(&item)).ok_or_else(|| Failed::Broken(Broken(format!("there is no item #{item}"))))?;
    let kept = driver.work(WorkNumber(of.work.number));
    let pause = kept.as_ref().and_then(|kept| decide::pause(&kept.work.records));
    let paused = (kept.as_ref()).and_then(|kept| paused_since(&kept.work.records).and_then(|at| kept.work.time_of(at)));
    let waits_for = pause.map(|pause| pause.waits_for.clone()).unwrap_or_default();
    Ok((of.depends_on.iter().chain(waits_for.iter()))
        .filter_map(|on| by_item.get(on))
        .filter_map(|on| ended.get(&on.work.number))
        .copied()
        .chain(paused)
        .fold(Time(of.added), Time::max))
}

/// Where the work came to be paused, as it is now, without a break: its first pause after it was last let on.
fn paused_since(records: &decide::Records) -> Option<decide::Seq> {
    let let_on = (records.pauses.iter()).filter(|p| !p.paused).map(|p| p.at).max();
    (records.pauses.iter())
        .filter(|p| p.paused && let_on.is_none_or(|on| p.at > on))
        .map(|p| p.at)
        .min()
}

/// What the request asks.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wanted {
    pub since: Option<String>,
    /// How many seconds at most the request is held open while there is nothing.
    pub wait: Option<u64>,
}

/// What needs the planner since the mark, held open as the request asks while there is nothing.
pub(crate) async fn attention(
    State(handle): State<Handle>,
    cache: Arc<ItemsCache>,
    times: Times,
    asked: Result<Query<Wanted>, QueryRejection>,
) -> Response {
    let asked = match asked {
        Ok(Query(asked)) => asked,
        Err(rejection) => {
            return fail(
                StatusCode::BAD_REQUEST,
                format!("The request cannot be read: {}.", rejection.body_text()),
            );
        }
    };
    let mut mark = match asked.since.as_deref().map(Mark::read).transpose() {
        Ok(mark) => mark,
        Err(unreadable) => return fail(StatusCode::BAD_REQUEST, format!("{}.", unreadable.0)),
    };
    let until = match asked
        .wait
        .map(|seconds| Instant::now().checked_add(Duration::from_secs(seconds)))
    {
        None => None,
        Some(Some(until)) => Some(until),
        Some(None) => {
            return fail(
                StatusCode::BAD_REQUEST,
                "The time to wait is longer than this machine's clock can count.".to_string(),
            );
        }
    };
    // Listened to before the works are read, so that nothing that happens between is missed.
    let mut changes = handle.changes();
    loop {
        let (cache, since) = (cache.clone(), mark.clone());
        let read = handle.read_at(move |driver, now| -> Result<(Given, Option<Time>, Time), Failed> {
            if let Some(since) = &since
                && since.read > now
            {
                return Err(Failed::Later);
            }
            let gathered = gather(&cache, driver, now, &times)?;
            Ok((
                answer(gathered.things, since.as_ref(), now, times.again),
                gathered.next,
                now,
            ))
        });
        let (given, next, now) = match read.await {
            Ok(Ok(read)) => read,
            Ok(Err(Failed::Later)) => {
                return fail(
                    StatusCode::BAD_REQUEST,
                    "The mark is later than the engine's clock: it is no mark this engine gave.".to_string(),
                );
            }
            Ok(Err(Failed::Stopped(fault))) => {
                return fail(
                    StatusCode::SERVICE_UNAVAILABLE,
                    format!("The engine stopped: {}.", service::Fault::Driver(fault)),
                );
            }
            Ok(Err(Failed::Broken(broken))) => {
                return fail(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("It cannot be shown: {}.", broken.0),
                );
            }
            Err(refusal) => return refused(&refusal),
        };
        let attention = Attention {
            whole: given.whole,
            lines: given.lines,
            left_out: given.left_out,
            mark: given.mark.to_string(),
            again: times.again,
        };
        let Some(until) = until.filter(|_| attention.lines.is_empty()) else {
            return Json(attention).into_response();
        };
        // Nothing yet: read again from this answer's mark as something happens, as what stands comes due again, or as
        // something comes to stand by time alone, until the time asked for.
        let after = |at: Time| Instant::now() + Duration::from_millis(at.0.saturating_sub(now.0));
        let wake = [Some(given.mark.due), next]
            .into_iter()
            .flatten()
            .map(after)
            .fold(until, Instant::min);
        mark = Some(given.mark);
        match woken(&mut changes, wake).await {
            Woken::TimedOut if wake >= until => return Json(attention).into_response(),
            Woken::TimedOut | Woken::Changed => {}
            Woken::Gone => return refused(&service::Refusal::Gone),
        }
    }
}

/// How the wait of a held request ended.
#[derive(Debug, PartialEq, Eq)]
enum Woken {
    /// The time waited for passed.
    TimedOut,
    /// Something that the works are read again for happened, or more than the listener keeps.
    Changed,
    /// The engine's loop ended.
    Gone,
}

/// Waits until `until` for a change the works are read again for. What the rounds under way have come to is no such
/// change: it comes at every call of every round, and nothing a request is answered with depends on it, so each such
/// notice would make the engine's loop read every work for nothing.
async fn woken(changes: &mut broadcast::Receiver<Changed>, until: Instant) -> Woken {
    loop {
        match tokio::time::timeout_at(until, changes.recv()).await {
            Err(_) => return Woken::TimedOut,
            Ok(Ok(changed)) if changed.only_going() => {}
            Ok(Ok(_) | Err(RecvError::Lagged(_))) => return Woken::Changed,
            Ok(Err(RecvError::Closed)) => return Woken::Gone,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(going: bool, works: Vec<WorkNumber>, items: bool) -> Changed {
        Changed {
            works,
            items,
            copies: false,
            accounts: false,
            observations: false,
            going,
        }
    }

    /// A thing that stands, of this rank, of this work, since and come at this time.
    fn stands(rank: u8, work: u64, since: u64) -> Thing {
        Thing {
            line: Line::Root {
                of: Of {
                    work,
                    item: work,
                    title: format!("item {work}"),
                },
                since,
            },
            rank,
            came: Time(since),
            once: false,
            work,
        }
    }

    /// A work that ended at this time.
    fn ended(work: u64, since: u64) -> Thing {
        Thing {
            line: Line::Ended {
                of: Of {
                    work,
                    item: work,
                    title: format!("item {work}"),
                },
                since,
                now: Now::Merged,
            },
            rank: 8,
            came: Time(since),
            once: true,
            work,
        }
    }

    /// The works the lines are of, in their order.
    fn works(given: &Given) -> Vec<u64> {
        (given.lines.iter())
            .map(|line| match line {
                Line::Root { of, .. } | Line::Ended { of, .. } => of.work,
                other => panic!("no such line was made: {other:?}"),
            })
            .collect()
    }

    const AGAIN: u64 = 300_000;

    // What stands is given whole with no mark, the works that ended being taken as read; from that answer's mark, it is
    // given again only once the time set for it has passed, and not a millisecond before; what came to stand since the
    // mark is given at once, and what ended is given once. Fails where what stands is given again before its time, not
    // at it, or with what came; or where a work that ended is given twice. Milliseconds.
    #[test]
    fn what_stands_is_given_again_once_the_time_set_for_it_has_passed_and_not_before() {
        const FIRST: u64 = 1_000;
        let standing = vec![stands(2, 5, 100), stands(7, 3, 50), ended(9, 90)];
        let first = answer(standing.clone(), None, Time(FIRST), AGAIN);
        assert!(first.whole);
        assert_eq!(works(&first), [5, 3], "the more pressing first, and no work that ended");
        assert_eq!(first.mark.due, Time(FIRST + AGAIN));
        let early = answer(standing.clone(), Some(&first.mark), Time(FIRST + AGAIN - 3), AGAIN);
        assert!(!early.whole);
        assert_eq!(works(&early), Vec::<u64>::new(), "given again before its time");
        let mut came = standing.clone();
        came.push(stands(4, 6, FIRST + AGAIN - 2));
        came.push(ended(7, FIRST + AGAIN - 2));
        let new = answer(came.clone(), Some(&early.mark), Time(FIRST + AGAIN - 1), AGAIN);
        assert!(!new.whole);
        assert_eq!(works(&new), [6, 7], "what came, at once, and nothing that stood");
        let due = answer(came.clone(), Some(&new.mark), Time(FIRST + AGAIN), AGAIN);
        assert!(due.whole);
        assert_eq!(
            works(&due),
            [5, 6, 3],
            "what stands, whole, at its time; what ended, not again"
        );
        assert_eq!(due.mark.due, Time(FIRST + 2 * AGAIN));
        let after = answer(came, Some(&due.mark), Time(FIRST + AGAIN + 1), AGAIN);
        assert_eq!(works(&after), Vec::<u64>::new());
    }

    // An answer holds at most `LINES` lines: where what stands is more, the next answer, at once, gives the rest, and
    // then what stands comes due again only after the time set; where works that ended are more than the room left, the
    // next answer gives those left out, each once, though they ended at the same time. Fails where a line is given
    // twice or not at all across the answers. Milliseconds; `LINES` + 5 things are the least that cut both kinds.
    #[test]
    fn what_an_answer_cannot_hold_comes_in_the_next_each_once() {
        let mut things: Vec<Thing> = (1..=(LINES as u64 + 5)).map(|work| stands(5, work, 10)).collect();
        let first = answer(things.clone(), None, Time(1_000), AGAIN);
        assert_eq!(first.lines.len(), LINES);
        assert_eq!(first.left_out, 5);
        let rest = answer(things.clone(), Some(&first.mark), Time(1_001), AGAIN);
        assert!(rest.whole);
        assert_eq!(works(&rest), (LINES as u64 + 1..=LINES as u64 + 5).collect::<Vec<_>>());
        assert_eq!(rest.mark.due, Time(1_001 + AGAIN));
        assert_eq!(
            works(&answer(things.clone(), Some(&rest.mark), Time(1_002), AGAIN)),
            Vec::<u64>::new()
        );
        // Works that ended at one time, more than the room what stands leaves as it is given whole again.
        things.truncate(LINES - 2);
        let marked = answer(things.clone(), None, Time(2_000), AGAIN).mark;
        things.extend((100..105).map(|work| ended(work, 2_500)));
        let at = 2_000 + AGAIN;
        let cut = answer(things.clone(), Some(&marked), Time(at), AGAIN);
        assert!(cut.whole);
        assert_eq!(works(&cut)[LINES - 2..], [100, 101]);
        assert_eq!(cut.left_out, 3);
        let read = Mark::read(&cut.mark.to_string()).expect("the mark reads back");
        assert_eq!(read, cut.mark);
        let next = answer(things.clone(), Some(&read), Time(at + 1), AGAIN);
        assert_eq!(works(&next), [102, 103, 104]);
        assert_eq!(
            works(&answer(things, Some(&next.mark), Time(at + 2), AGAIN)),
            Vec::<u64>::new()
        );
    }

    // A mark is written as three or five numbers joined by dots and read back as it was written; any other text is no
    // mark.
    #[test]
    fn a_mark_is_written_and_read_back_and_other_text_is_refused() {
        for text in ["1.2.0", "1800000000000.1800000300000.20.1799999000000.7"] {
            assert_eq!(Mark::read(text).expect("it reads").to_string(), text);
        }
        for text in [
            "",
            "7",
            "1.2",
            "1.2.3.4",
            "1.2.x",
            "1..2",
            "-1.2.0",
            "1.2.0.4.5.6",
            "1,2,0",
        ] {
            assert!(Mark::read(text).is_err(), "{text:?} was read as a mark");
        }
    }

    // A red main is a line of its own kind, about no work, which names the tests, the head and the works that wait, and
    // is read back as it was said. Milliseconds; the one test of this line's wire shape.
    #[test]
    fn a_red_main_is_a_line_of_its_own_kind_that_is_read_back_as_it_was_said() {
        let line = Line::MainRed {
            since: 1_700_000_000_000,
            project: "Firn-wf".to_string(),
            tests: vec![RedTest {
                unit: "carry".to_string(),
                name: "tools::tests::b".to_string(),
                head: "9f8e7d6c".to_string(),
            }],
            works: vec![Of {
                work: 184,
                item: 90,
                title: "Greet".to_string(),
            }],
        };
        let said = serde_json::to_value(&line).expect("it is said");
        assert_eq!(
            said,
            serde_json::json!({
                "kind": "mainRed",
                "since": 1_700_000_000_000u64,
                "project": "Firn-wf",
                "tests": [{ "unit": "carry", "name": "tools::tests::b", "head": "9f8e7d6c" }],
                "works": [{ "work": 184, "item": 90, "title": "Greet" }],
            })
        );
        assert_eq!(serde_json::from_value::<Line>(said).expect("it is read"), line);
    }

    // A held request is woken by a change that names a work or sets another flag, also where `going` is set with it,
    // and by none that says only what the rounds under way have come to, which comes at every call. Fails where such a
    // notice wakes the wait, which reads every work in the engine's loop for nothing. A paused clock, nothing run.
    #[tokio::test(start_paused = true)]
    async fn a_notice_of_only_what_the_rounds_under_way_have_come_to_wakes_no_held_request() {
        let wait = |seconds| Instant::now() + Duration::from_secs(seconds);

        let (send, mut changes) = broadcast::channel(16);
        for _ in 0..3 {
            send.send(said(true, Vec::new(), false)).unwrap();
        }
        assert_eq!(woken(&mut changes, wait(5)).await, Woken::TimedOut);

        // What comes after the notices of figures still wakes it, whatever it names.
        for (after, what) in [
            (said(false, vec![WorkNumber(1)], false), "a work"),
            (said(true, vec![WorkNumber(1)], false), "a work, with figures"),
            (said(false, Vec::new(), true), "an item"),
            (said(true, Vec::new(), true), "an item, with figures"),
        ] {
            send.send(said(true, Vec::new(), false)).unwrap();
            send.send(after).unwrap();
            assert_eq!(woken(&mut changes, wait(600)).await, Woken::Changed, "{what}");
        }

        drop(send);
        assert_eq!(woken(&mut changes, wait(600)).await, Woken::Gone);
    }
}
