//! What needs whoever plans the work, since they last looked: the owner's assistant now, an architect's cell later.
//!
//! A line is a work that stands where only someone outside its flow can move it, and came to stand so since the mark
//! the planner gives: it ended, merged or cancelled; it rests for a person, at a cell's question, a ruling only a person
//! can give, a limit, or a pause the engine made with its reason; it waits for items none of which is going on; or its
//! records cannot be those of a work. A work going on from one cell to the next is nothing for the planner, and neither
//! is one the owner paused, which shows only as the reason why a work waiting for it is idle. A work that ended and
//! could not let go of what it held, which the engine paused saying why, rests for a person. One line is about no
//! work: the wait every work shares, while no agent's round begins; a work whose round it holds is going on. So is the
//! pause of everything, the owner's or the engine's as it started with works going on, its own line about no work,
//! under which every work is read as if it did not hold: a work held by it alone goes on, and is no reason for another
//! to be idle. A wait for a slot is going on.
//!
//! Everything here is read from the records (principle state-from-records). How a work stands is what the deciding
//! code decides on its records; since when it stands so is the first of the moments after which it has stood so
//! without a break, each moment's standing decided again on the records as they were before the next moment. The mark
//! is a place in the records: for each work that had not ended, the place up to which its records were read. The same
//! mark therefore gives the same answer whatever the engine did between, a restart included, and nothing that came to
//! stand after it is missed.

use std::collections::BTreeMap;
use std::fmt;

use decide::{CellId, Decision, End, Maker, Pauser, RecordFault, Records, Seq, Waited, decide};
use store::{ItemNumber, WorkNumber};
use work::{Moment, Time};

use crate::{DEPENDENCIES, Driver, Fault, Kept, Outage};

/// The most lines an answer holds; how many more there are is counted, and they come again at the next call.
pub const LINES: usize = 20;

/// What needs the planner since a mark, most pressing first, and the mark to give next time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attention {
    /// At most [`LINES`] of them.
    pub lines: Vec<Line>,
    /// How many more there are. The next mark leaves them unread, so that they come again.
    pub left_out: usize,
    pub mark: Mark,
}

/// Something for the planner, and since when it stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub heed: Heed,
    /// When it came to stand so: the time of the first moment after which it has without a break.
    pub since: Time,
}

/// What is for the planner, in the order of how pressing it is. What a work waits for, or why the engine paused it, is
/// its decision, which its view gives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Heed {
    /// Everything is paused, by the owner or by the engine as it started with works going on: no work begins a round
    /// but to answer a person's words to its cell, until the owner lifts it. It is about no work.
    Everything { by: Pauser },
    /// No agent's round of any work begins: a round failed for a cause every work shares. It is about no work.
    Agents { outage: Outage },
    /// The work's records cannot be those of a work: nothing is decided on them.
    Fault { work: WorkNumber },
    /// The work rests for a person: a call a person answers, a precondition a person can make hold, a limit, or a
    /// pause the engine made with its reason.
    ForAPerson { work: WorkNumber },
    /// The work waits for these items, none of which is going on.
    Idle { work: WorkNumber, items: Vec<Idle> },
    /// The work ended.
    Ended { work: WorkNumber, end: End },
}

/// An item a work waits for that is not going on, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Idle {
    pub item: ItemNumber,
    pub why: Stuck,
}

/// Why an item a work waits for is not going on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stuck {
    /// The owner paused its work, or it was added paused and not let on.
    Paused,
    /// Its work rests for a person.
    ForAPerson,
    /// Its work waits in turn for items none of which is going on.
    Idle,
    /// It was cancelled: the work waits for it for good.
    Cancelled,
    /// Its work's records cannot be those of a work.
    Fault,
}

/// A place in the records of every work: the highest work there was, and for each that had not ended, the place up to
/// which its records were read. A work up to the highest that is not named had ended, and its records were read whole;
/// a work after it is unread from its start. Where the pause of everything held and was read, the time it began, which
/// tells that pause from any later one. Written as the highest work, then each work named with its place,
/// then `e.` and the time of the pause read, such as `12,3.41,7.118,e.1800000000000`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    highest: u64,
    read: BTreeMap<WorkNumber, Seq>,
    everything: Option<Time>,
}

/// Why a text is no mark, in words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unreadable(pub String);

/// Why no answer was given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The mark names this work, or a place of its records, that the records do not hold, or leaves it out as ended
    /// where it has not ended: it is no mark of these records.
    NotOfTheseRecords { work: WorkNumber },
    /// The driver is stopped, by this fault, and shows no work.
    Stopped(Fault),
}

impl Mark {
    /// Reads a mark as [`Mark`]'s text writes it.
    pub fn read(text: &str) -> Result<Mark, Unreadable> {
        let unreadable = || {
            Unreadable(format!(
                "{text:?} is not a mark: a mark is a number, then each work with its place, such as 12,3.41,7.118"
            ))
        };
        let number = |part: &str| -> Result<u64, Unreadable> {
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return Err(unreadable());
            }
            part.parse().map_err(|_| unreadable())
        };
        let mut parts = text.trim().split(',');
        let highest = number(parts.next().unwrap_or_default())?;
        let mut read = BTreeMap::new();
        let mut everything = None;
        for part in parts {
            let (work, place) = part.split_once('.').ok_or_else(unreadable)?;
            if work == "e" {
                if everything.replace(Time(number(place)?)).is_some() {
                    return Err(unreadable());
                }
                continue;
            }
            let (work, place) = (WorkNumber(number(work)?), Seq(number(place)?));
            if work.0 > highest || read.insert(work, place).is_some() {
                return Err(unreadable());
            }
        }
        Ok(Mark {
            highest,
            read,
            everything,
        })
    }
}

impl fmt::Display for Mark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.highest)?;
        for (work, place) in &self.read {
            write!(f, ",{}.{}", work.0, place.0)?;
        }
        if let Some(began) = self.everything {
            write!(f, ",e.{}", began.0)?;
        }
        Ok(())
    }
}

impl Driver {
    /// What needs the planner since the mark, and the mark to give next time. With no mark, what stands now and is
    /// for the planner, but the works that ended: they are taken as read.
    pub fn attention(&self, since: Option<&Mark>) -> Result<Attention, Refused> {
        if let Some(fault) = self.fault() {
            return Err(Refused::Stopped(fault.clone()));
        }
        let works: BTreeMap<WorkNumber, Kept<'_>> = self.works().map(|kept| (kept.head.number, kept)).collect();
        let standing = (works.iter())
            .map(|(number, kept)| (*number, Standing::of(kept, &kept.work.records)))
            .collect();
        let mut reader = Reader {
            unread: BTreeMap::new(),
            of_item: works
                .values()
                .map(|kept| (kept.item.number, kept.head.number))
                .collect(),
            works,
            standing,
            idle: BTreeMap::new(),
        };
        reader.unread = reader.unread_since(since)?;
        let numbers: Vec<WorkNumber> = reader.works.keys().copied().collect();
        // In the order of the items, so that an item a work waits for is read before the work.
        let mut by_item: Vec<WorkNumber> = numbers.clone();
        by_item.sort_by_key(|number| reader.works[number].item.number);
        for number in by_item {
            let idle = reader.idle_of(number);
            reader.idle.insert(number, idle);
        }
        // The pause of everything is read by the time it began: a pause is given once, and a later one again.
        let everything = (self.everything_paused())
            .filter(|paused| since.is_none_or(|mark| mark.everything != Some(paused.since)))
            .map(|paused| {
                (
                    0,
                    Line {
                        heed: Heed::Everything { by: paused.by },
                        since: paused.since,
                    },
                )
            });
        let mut lines: Vec<(u8, Line)> = everything.into_iter().chain(reader.shared(self.outage())).collect();
        for number in &numbers {
            if let Some(line) = reader.line(*number) {
                lines.push(line);
            }
        }
        lines.sort_by_key(|(rank, line)| (*rank, line.since, work_of(&line.heed)));
        let left: Vec<WorkNumber> = (lines.iter().skip(LINES))
            .filter_map(|(_, line)| work_of(&line.heed))
            .collect();
        let everything_left = (lines.iter().skip(LINES)).any(|(_, line)| matches!(line.heed, Heed::Everything { .. }));
        let lines: Vec<Line> = lines.into_iter().take(LINES).map(|(_, line)| line).collect();
        // A work whose line is left out is written unread from its start, so that whatever it stands in comes again;
        // a work that ended is left out of the mark once it is read; every other work is read up to its last entry.
        let mut read = BTreeMap::new();
        for (number, kept) in &reader.works {
            if left.contains(number) {
                read.insert(*number, Seq(0));
            } else if !matches!(reader.standing[number], Standing::Ended(_)) {
                read.insert(*number, kept.work.tick());
            }
        }
        let mark = Mark {
            highest: numbers.last().map_or(0, |number| number.0),
            read,
            everything: (self.everything_paused())
                .map(|paused| paused.since)
                .filter(|_| !everything_left),
        };
        Ok(Attention {
            left_out: left.len() + usize::from(everything_left),
            lines,
            mark,
        })
    }
}

/// Whether what the work rests for is the engine's pause, with its reason, rather than what its decision says it waits
/// for: as when it ended and could not let go of what it held, or waits for what the engine will give.
pub fn rests_for_the_engines_pause(kept: &Kept<'_>) -> bool {
    matches!(Standing::of(kept, &kept.work.records), Standing::PausedByEngine { .. })
}

/// How a work stands, as far as the planner is concerned, and what tells one such standing from the next of the same
/// kind.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Standing {
    Fault(RecordFault),
    Ended(End),
    /// It waits at the cell for each of these, each for a person.
    Asked {
        at: CellId,
        on: Vec<Waited>,
    },
    /// The engine paused it, by the pause at this place.
    PausedByEngine {
        at: Seq,
    },
    /// It waits at the cell for the items its item depends on, and for nothing else.
    Items {
        at: CellId,
    },
    /// The owner paused it, by the pause at this place.
    PausedByOwner {
        at: Seq,
    },
    /// It goes on: a round is under way or about to begin, or it waits for what the engine or a cell will give.
    Going,
}

impl Standing {
    /// How the work stands on these records, which are its own or as they were at a moment.
    fn of(kept: &Kept<'_>, records: &Records) -> Standing {
        let paused = || match decide::pause(records) {
            Some(pause) if pause.by == Pauser::Engine => Standing::PausedByEngine { at: pause.at },
            Some(pause) => Standing::PausedByOwner { at: pause.at },
            None => Standing::Going,
        };
        match decide(&kept.flow.flow, records) {
            Decision::Fault(fault) => Standing::Fault(fault),
            // What the work held outside the engine could not be let go, and the engine paused it with why: only the
            // owner, letting it on, has it tried again.
            Decision::Ended(end) => match decide::pause(records) {
                Some(pause) if pause.by == Pauser::Engine && ended_at(records, end).is_some_and(|at| pause.at > at) => {
                    Standing::PausedByEngine { at: pause.at }
                }
                _ => Standing::Ended(end),
            },
            Decision::Paused => paused(),
            // A run at rest holds its next round while the work is paused, as the pages show it.
            Decision::GoOn { .. } if decide::paused(records) => paused(),
            Decision::Wait { at, on } => {
                let person: Vec<Waited> = on.iter().filter(|waited| for_a_person(waited)).cloned().collect();
                if !person.is_empty() {
                    Standing::Asked { at, on: person }
                } else if let Some(pause) = decide::pause(records).filter(|pause| pause.by == Pauser::Engine) {
                    // What it waits for may come, but the engine's pause holds it once it does, until the owner lets it
                    // on: it rests for a person from the pause, not from when the wait ends.
                    Standing::PausedByEngine { at: pause.at }
                } else if on.iter().all(on_items) {
                    Standing::Items { at }
                } else {
                    Standing::Going
                }
            }
            Decision::Busy { .. } | Decision::Run { .. } | Decision::GoOn { .. } | Decision::Abandon { .. } => {
                Standing::Going
            }
        }
    }
}

/// Whether only a person can give what the work waits for.
fn for_a_person(waited: &Waited) -> bool {
    match waited {
        Waited::Call { on, .. } => on.is_some(),
        Waited::Precondition { by, .. } => by.contains(&Maker::Owner),
        Waited::Limit { .. } => true,
        // The engine gives it as soon as a round of another work ends.
        Waited::Slot => false,
    }
}

/// Whether what the work waits for is the items its item depends on, which only they can give by merging.
fn on_items(waited: &Waited) -> bool {
    match waited {
        Waited::Precondition { by, .. } => {
            !by.is_empty()
                && (by.iter()).all(|maker| matches!(maker, Maker::Outside { source } if source == DEPENDENCIES))
        }
        Waited::Call { .. } | Waited::Limit { .. } | Waited::Slot => false,
    }
}

/// The place at which the work ended: its merge's finish, or its cancel.
fn ended_at(records: &Records, end: End) -> Option<Seq> {
    match end {
        End::Merged => records.finished.iter().map(|finished| finished.at).max(),
        End::Cancelled => records.cancelled,
    }
}

/// The work the line is of; for the wait every work shares, the work whose round began it, whose place in the next
/// mark says whether it is given again; none for the pause of everything.
fn work_of(heed: &Heed) -> Option<WorkNumber> {
    match heed {
        Heed::Everything { .. } => None,
        Heed::Agents { outage } => Some(outage.work),
        Heed::Fault { work } | Heed::ForAPerson { work } | Heed::Idle { work, .. } | Heed::Ended { work, .. } => {
            Some(*work)
        }
    }
}

/// The works as one answer reads them.
struct Reader<'a> {
    works: BTreeMap<WorkNumber, Kept<'a>>,
    of_item: BTreeMap<ItemNumber, WorkNumber>,
    standing: BTreeMap<WorkNumber, Standing>,
    /// From which place each work is unread; none where it is read whole.
    unread: BTreeMap<WorkNumber, Option<Seq>>,
    /// For each work, the items it waits for where none of them is going on.
    idle: BTreeMap<WorkNumber, Option<Vec<Idle>>>,
}

impl Reader<'_> {
    /// From which place each work is unread since the mark; with no mark, every work from its start but those that
    /// ended.
    fn unread_since(&self, since: Option<&Mark>) -> Result<BTreeMap<WorkNumber, Option<Seq>>, Refused> {
        let ended = |number: &WorkNumber| matches!(self.standing[number], Standing::Ended(_));
        let Some(mark) = since else {
            return Ok((self.works.keys())
                .map(|number| (*number, (!ended(number)).then_some(Seq(0))))
                .collect());
        };
        let not_ours = |work| Err(Refused::NotOfTheseRecords { work });
        let highest = self.works.keys().last().map_or(0, |number| number.0);
        if mark.highest > highest {
            return not_ours(WorkNumber(mark.highest));
        }
        for (number, place) in &mark.read {
            match self.works.get(number) {
                Some(kept) if *place <= kept.work.tick() => {}
                _ => return not_ours(*number),
            }
        }
        let mut unread = BTreeMap::new();
        for number in self.works.keys() {
            let from = match mark.read.get(number) {
                Some(place) => Some(*place),
                None if number.0 > mark.highest => Some(Seq(0)),
                // Left out of the mark as ended: a work that ended stays ended.
                None if ended(number) => None,
                None => return not_ours(*number),
            };
            unread.insert(*number, from);
        }
        Ok(unread)
    }

    /// The line of the wait every work shares, the most pressing, where one stands and came since the mark: the round
    /// that began it ended at or after its work's place.
    fn shared(&self, outage: Option<Outage>) -> Option<(u8, Line)> {
        let outage = outage?;
        let kept = self.works.get(&outage.work)?;
        let records = &kept.work.records;
        let ended = (records.returned.iter())
            .filter(|returned| returned.run == outage.run)
            .nth(outage.round.checked_sub(1)?)?
            .at;
        let from = self.unread.get(&outage.work).copied().flatten()?;
        (ended >= from).then(|| {
            let since = outage.since;
            (
                0,
                Line {
                    heed: Heed::Agents { outage },
                    since,
                },
            )
        })
    }

    /// The work's line with how pressing it is, where it has one: it stands for the planner, and came to since the
    /// mark.
    fn line(&self, number: WorkNumber) -> Option<(u8, Line)> {
        let (rank, heed) = match &self.standing[&number] {
            Standing::Fault(_) => (1, Heed::Fault { work: number }),
            Standing::Asked { .. } | Standing::PausedByEngine { .. } => (2, Heed::ForAPerson { work: number }),
            Standing::Items { .. } => {
                let items = self.idle[&number].clone()?;
                (3, Heed::Idle { work: number, items })
            }
            Standing::Ended(end) => (
                4,
                Heed::Ended {
                    work: number,
                    end: *end,
                },
            ),
            Standing::PausedByOwner { .. } | Standing::Going => return None,
        };
        if !self.came_since_the_mark(number) {
            return None;
        }
        Some((
            rank,
            Line {
                heed,
                since: self.since(number),
            },
        ))
    }

    /// Whether the work came to stand as it does since the mark: an idle work, also where an item it waits for
    /// stopped going on since, or one it waited for merged since and left it waiting for items that stood still before.
    fn came_since_the_mark(&self, number: WorkNumber) -> bool {
        let unread =
            |number: &WorkNumber| (self.unread[number]).is_some_and(|from| self.began(*number, from).is_some());
        if unread(&number) {
            return true;
        }
        let Some(Some(items)) = self.idle.get(&number) else {
            return false;
        };
        let stopped = (items.iter()).any(|idle| {
            let on = self.of_item[&idle.item];
            match idle.why {
                Stuck::Idle => self.came_since_the_mark(on),
                _ => unread(&on),
            }
        });
        stopped || self.merged_waited_for(number).any(|on| unread(&on))
    }

    /// Since when the work has stood as it does; for an idle work, since the latest of that, of the times the items
    /// it waits for stopped going on, and of the times those it waited for merged: it was not idle before the last of
    /// them merged.
    fn since(&self, number: WorkNumber) -> Time {
        let own = self.began(number, Seq(0)).map_or(Time::default(), |moment| moment.at);
        let Some(Some(items)) = self.idle.get(&number) else {
            return own;
        };
        let merged = self.merged_waited_for(number).map(|on| self.since(on));
        (items.iter())
            .map(|idle| self.since(self.of_item[&idle.item]))
            .chain(merged)
            .fold(own, Time::max)
    }

    /// The works of the items the work's item depends on that merged.
    fn merged_waited_for(&self, number: WorkNumber) -> impl Iterator<Item = WorkNumber> + '_ {
        (self.works[&number].item.depends_on.iter())
            .filter_map(|item| self.of_item.get(item).copied())
            .filter(|on| self.standing[on] == Standing::Ended(End::Merged))
    }

    /// The moment from which the work has stood as it does without a break, where it is at or after this place.
    fn began(&self, number: WorkNumber, from: Seq) -> Option<&Moment> {
        let kept = &self.works[&number];
        let now = &self.standing[&number];
        let moments = &kept.work.moments;
        for moment in moments.iter().rev() {
            if moment.from < from {
                return None;
            }
            if Standing::of(kept, &kept.work.records.before(moment.from)) != *now {
                return Some(moment);
            }
        }
        moments.first()
    }

    /// The items the work waits for, where it waits for items and none of them is going on. An item waited for is read
    /// before the work, its item having a lower number.
    fn idle_of(&self, number: WorkNumber) -> Option<Vec<Idle>> {
        if !matches!(self.standing[&number], Standing::Items { .. }) {
            return None;
        }
        let mut items = Vec::new();
        for item in &self.works[&number].item.depends_on {
            let on = self.of_item.get(item)?;
            let why = match &self.standing[on] {
                Standing::Ended(End::Merged) => continue,
                Standing::Ended(End::Cancelled) => Stuck::Cancelled,
                Standing::Fault(_) => Stuck::Fault,
                Standing::Asked { .. } | Standing::PausedByEngine { .. } => Stuck::ForAPerson,
                Standing::PausedByOwner { .. } => Stuck::Paused,
                Standing::Items { .. } => {
                    self.idle.get(on)?.as_ref()?;
                    Stuck::Idle
                }
                Standing::Going => return None,
            };
            items.push(Idle { item: *item, why });
        }
        // Waiting for items that have all merged, the work is about to be told so.
        (!items.is_empty()).then_some(items)
    }
}
