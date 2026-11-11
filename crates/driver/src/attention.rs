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

use decide::{End, Seq};
use store::{ItemNumber, WorkNumber};
use work::Time;

use crate::{Driver, Fault};

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
        let _ = since;
        Err(Refused::NotOfTheseRecords { work: WorkNumber(0) })
    }
}
