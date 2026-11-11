//! What needs whoever plans the work, since they last looked: the owner's assistant now, an architect's cell later.
//!
//! A line is a work that stands where only someone outside its flow can move it, and came to stand so since the mark
//! the planner gives: it ended, merged or cancelled; it rests for a person, at a cell's question, a ruling only a person
//! can give, a limit, or a pause the engine made with its reason; it waits for items none of which is going on; or its
//! records cannot be those of a work. A work going on from one cell to the next is nothing for the planner, and neither
//! is one the owner paused, which shows only as the reason why a work waiting for it is idle.
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

use crate::{DEPENDENCIES, Driver, Fault, Kept};

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
/// a work after it is unread from its start. Written as the highest work, then each work named with its place, such
/// as `12,3.41,7.118`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    highest: u64,
    read: BTreeMap<WorkNumber, Seq>,
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
        for part in parts {
            let (work, place) = part.split_once('.').ok_or_else(unreadable)?;
            let (work, place) = (WorkNumber(number(work)?), Seq(number(place)?));
            if work.0 > highest || read.insert(work, place).is_some() {
                return Err(unreadable());
            }
        }
        Ok(Mark { highest, read })
    }
}

impl fmt::Display for Mark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.highest)?;
        for (work, place) in &self.read {
            write!(f, ",{}.{}", work.0, place.0)?;
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
        let mut lines: Vec<(u8, Line)> = Vec::new();
        for number in &numbers {
            if let Some(line) = reader.line(*number) {
                lines.push(line);
            }
        }
        lines.sort_by_key(|(rank, line)| (*rank, line.since, work_of(&line.heed)));
        let left: Vec<WorkNumber> = lines.iter().skip(LINES).map(|(_, line)| work_of(&line.heed)).collect();
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
        };
        Ok(Attention {
            lines,
            left_out: left.len(),
            mark,
        })
    }
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
            Decision::Ended(end) => Standing::Ended(end),
            Decision::Paused => paused(),
            // A run at rest holds its next round while the work is paused, as the pages show it.
            Decision::GoOn { .. } if decide::paused(records) => paused(),
            Decision::Wait { at, on } => {
                let person: Vec<Waited> = on.iter().filter(|waited| for_a_person(waited)).cloned().collect();
                if !person.is_empty() {
                    Standing::Asked { at, on: person }
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
    }
}

/// Whether what the work waits for is the items its item depends on, which only they can give by merging.
fn on_items(waited: &Waited) -> bool {
    match waited {
        Waited::Precondition { by, .. } => {
            !by.is_empty()
                && (by.iter()).all(|maker| matches!(maker, Maker::Outside { source } if source == DEPENDENCIES))
        }
        Waited::Call { .. } | Waited::Limit { .. } => false,
    }
}

fn work_of(heed: &Heed) -> WorkNumber {
    match heed {
        Heed::Fault { work } | Heed::ForAPerson { work } | Heed::Idle { work, .. } | Heed::Ended { work, .. } => *work,
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

    /// The work's line with how pressing it is, where it has one: it stands for the planner, and came to since the
    /// mark.
    fn line(&self, number: WorkNumber) -> Option<(u8, Line)> {
        let (rank, heed) = match &self.standing[&number] {
            Standing::Fault(_) => (0, Heed::Fault { work: number }),
            Standing::Asked { .. } | Standing::PausedByEngine { .. } => (1, Heed::ForAPerson { work: number }),
            Standing::Items { .. } => {
                let items = self.idle[&number].clone()?;
                (2, Heed::Idle { work: number, items })
            }
            Standing::Ended(end) => (
                3,
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
    /// stopped going on since.
    fn came_since_the_mark(&self, number: WorkNumber) -> bool {
        let unread =
            |number: &WorkNumber| (self.unread[number]).is_some_and(|from| self.began(*number, from).is_some());
        if unread(&number) {
            return true;
        }
        let Some(Some(items)) = self.idle.get(&number) else {
            return false;
        };
        (items.iter()).any(|idle| {
            let on = self.of_item[&idle.item];
            match idle.why {
                Stuck::Idle => self.came_since_the_mark(on),
                _ => unread(&on),
            }
        })
    }

    /// Since when the work has stood as it does; for an idle work, since the latest of that and of the times the items
    /// it waits for stopped going on.
    fn since(&self, number: WorkNumber) -> Time {
        let own = self.began(number, Seq(0)).map_or(Time::default(), |moment| moment.at);
        let Some(Some(items)) = self.idle.get(&number) else {
            return own;
        };
        (items.iter())
            .map(|idle| self.since(self.of_item[&idle.item]))
            .fold(own, Time::max)
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
