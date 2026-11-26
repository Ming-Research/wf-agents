//! What needs whoever plans the work, as the records of every work say it: the owner's assistant and an architect.
//!
//! A line is a work that stands where only someone outside its flow can move it, with since when it stands so: it
//! ended, merged or cancelled; it waits on the owner, for what the one definition of that names ([`crate::owner`]), the
//! engine's pause among it, and a work that ended and could not let go of what it held, which the engine paused saying
//! why; it is stalled, no cell able to write what it waits for; it waits for items none of which is going on; its
//! owner's pause waited for items, each of which has merged or been cancelled; or its records cannot be those of a
//! work. A work going on from one cell to the next is nothing for the planner, and neither is one the owner paused,
//! which shows only as the reason why a work waiting for it is idle, unless what its pause waited for is over. One
//! line is about no work: the wait every work shares, while no agent's round begins; a work whose round it holds is
//! going on. So is the pause of everything, the owner's or the engine's as it started with works going on, its own
//! line about no work, under which every work is read as if it did not hold: a work held by it alone goes on, and is
//! no reason for another to be idle. A wait for a slot is going on. Another line is about no work either: the main
//! branch of a project is red on a test ([`crate::red_main`]), since the red began; a red that comes back after main
//! moved on, with no test having passed on it between, is the same red, since the same time.
//!
//! Everything here is read from the records (principle state-from-records). How a work stands is what the deciding
//! code decides on its records; since when it stands so is the first of the moments after which it has stood so
//! without a break, each moment's standing decided again on the records as they were before the next moment. What of
//! the lines is given when, and what else needs the planner, is the interface's to say (crate `web`, module
//! `attention`): it gives what came to stand since it last gave, by these times, and what stands again from time to
//! time.

use std::collections::BTreeMap;

use decide::{CellId, Decision, End, Maker, Pauser, RecordFault, Records, Seq, Waited, decide};

use crate::owner::{self, Shared};
use crate::red_main::RedTest;
use store::{ItemNumber, WorkNumber};
use work::{Moment, Time};

use crate::{DEPENDENCIES, Driver, Fault, Kept, Outage};

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
    /// The main branch of the project is red on these tests, which the check before a merge found failing on its head;
    /// these works wait for it to pass. It is about no work.
    MainRed {
        project: String,
        tests: Vec<RedTest>,
        works: Vec<WorkNumber>,
    },
    /// The work's records cannot be those of a work: nothing is decided on them.
    Fault { work: WorkNumber },
    /// The work waits on the owner, for what the one definition of that names ([`crate::owner`]).
    ForAPerson { work: WorkNumber },
    /// The work is stalled: no cell will write what it waits for, and nothing pauses it.
    Stalled { work: WorkNumber },
    /// The work waits for these items, none of which is going on.
    Idle { work: WorkNumber, items: Vec<Idle> },
    /// The owner's pause of the work waited for these items to merge or be cancelled, and each has: the pause holds
    /// the work still, and only the owner lets it on.
    PauseOver { work: WorkNumber, items: Vec<ItemNumber> },
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
    /// Its work waits on the owner.
    ForAPerson,
    /// Its work is stalled.
    Stalled,
    /// Its work waits in turn for items none of which is going on.
    Idle,
    /// Its work's records cannot be those of a work.
    Fault,
}

impl Driver {
    /// Every line for the planner that stands now, the works that ended among them, each since when it stands so: the
    /// most pressing first, then the longest standing, then by the work's number. None where the driver stopped, by
    /// the fault that stopped it.
    pub fn attention(&self) -> Result<Vec<Line>, Fault> {
        if let Some(fault) = self.fault() {
            return Err(fault.clone());
        }
        let works: BTreeMap<WorkNumber, Kept<'_>> = self.works().map(|kept| (kept.head.number, kept)).collect();
        let standing = (works.iter())
            .map(|(number, kept)| (*number, Standing::of(kept, &kept.work.records)))
            .collect();
        let mut reader = Reader {
            of_item: works
                .values()
                .map(|kept| (kept.item.number, kept.head.number))
                .collect(),
            works,
            standing,
            idle: BTreeMap::new(),
        };
        let numbers: Vec<WorkNumber> = reader.works.keys().copied().collect();
        // In the order of the items, so that an item a work waits for is read before the work.
        let mut by_item: Vec<WorkNumber> = numbers.clone();
        by_item.sort_by_key(|number| reader.works[number].item.number);
        for number in by_item {
            let idle = reader.idle_of(number);
            reader.idle.insert(number, idle);
        }
        let everything = (self.everything_paused()).map(|paused| {
            (
                0,
                Line {
                    heed: Heed::Everything { by: paused.by },
                    since: paused.since,
                },
            )
        });
        let reds = (self.red_main().into_iter()).map(|red| {
            (
                0,
                Line {
                    heed: Heed::MainRed {
                        project: red.project,
                        tests: red.tests,
                        works: red.waiting,
                    },
                    since: red.since,
                },
            )
        });
        let mut lines: Vec<(u8, Line)> = (everything.into_iter())
            .chain(reds)
            .chain(reader.shared(self.outage()))
            .collect();
        for number in &numbers {
            if let Some(line) = reader.line(*number) {
                lines.push(line);
            }
        }
        lines.sort_by_key(|(rank, line)| (*rank, line.since, work_of(&line.heed)));
        Ok(lines.into_iter().map(|(_, line)| line).collect())
    }
}

/// How a work stands, as far as the planner is concerned, and what tells one such standing from the next of the same
/// kind.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Standing {
    Fault(RecordFault),
    Ended(End),
    /// It waits on the owner, for each of these, by their keys.
    Owner {
        keys: Vec<String>,
    },
    /// It waits at the cell for the items its item depends on, and for nothing else.
    Items {
        at: CellId,
    },
    /// The owner paused it, by the pause at this place.
    PausedByOwner {
        at: Seq,
    },
    /// It is stalled, and nothing pauses it: no cell will write what it waits for, and it rests until someone writes
    /// what runs a cell.
    Still,
    /// It goes on: a round is under way or about to begin, or it waits for what the engine or a cell will give.
    Going,
}

impl Standing {
    /// How the work stands on these records, which are its own or as they were at a moment. What holds every work at
    /// once is its own line, and is not read here.
    fn of(kept: &Kept<'_>, records: &Records) -> Standing {
        // A work no flow decides now ended on a version that is not installed, and is read so at every moment the
        // installed flow could be asked of.
        let decision = match kept.on.end() {
            Some(end) => Decision::Ended(end),
            None => decide(&kept.flow.flow, records),
        };
        let waits = owner::waits(kept, records, &decision, None, Shared::default());
        if !waits.is_empty() {
            return Standing::Owner {
                keys: waits.into_iter().map(|wait| wait.key).collect(),
            };
        }
        let paused = || match decide::pause(records) {
            Some(pause) => Standing::PausedByOwner { at: pause.at },
            None => Standing::Going,
        };
        match decision {
            Decision::Fault(fault) => Standing::Fault(fault),
            Decision::Ended(end) => Standing::Ended(end),
            Decision::Paused { .. } => paused(),
            // A run at rest holds its next round while the work is paused, as the pages show it.
            Decision::GoOn { .. } if decide::paused(records) => paused(),
            Decision::Wait { at, on } if !on.is_empty() && on.iter().all(on_items) => Standing::Items { at },
            // A stalled work that its own pause holds is the pauser's to let on.
            Decision::Stalled { .. } if decide::paused(records) => paused(),
            Decision::Stalled { .. } => Standing::Still,
            Decision::Wait { .. }
            | Decision::Busy { .. }
            | Decision::Run { .. }
            | Decision::GoOn { .. }
            | Decision::Hurry { .. }
            | Decision::Abandon { .. } => Standing::Going,
        }
    }
}

/// Whether what the work waits for is the items its item depends on, which only they can give by merging.
pub fn on_items(waited: &Waited) -> bool {
    match waited {
        Waited::Precondition { by, .. } => {
            !by.is_empty()
                && (by.iter()).all(|maker| matches!(maker, Maker::Outside { source } if source == DEPENDENCIES))
        }
        Waited::Call { .. }
        | Waited::Limit { .. }
        | Waited::Calls { .. }
        | Waited::Slot
        | Waited::Turn
        | Waited::Answer { .. }
        | Waited::Outside { .. } => false,
    }
}

/// The work the line is of; for the wait every work shares, the work whose round began it; none for the pause of
/// everything and a red main.
fn work_of(heed: &Heed) -> Option<WorkNumber> {
    match heed {
        Heed::Everything { .. } | Heed::MainRed { .. } => None,
        Heed::Agents { outage } => Some(outage.work),
        Heed::Fault { work }
        | Heed::ForAPerson { work }
        | Heed::Stalled { work }
        | Heed::Idle { work, .. }
        | Heed::PauseOver { work, .. }
        | Heed::Ended { work, .. } => Some(*work),
    }
}

/// The works as one reading reads them.
struct Reader<'a> {
    works: BTreeMap<WorkNumber, Kept<'a>>,
    of_item: BTreeMap<ItemNumber, WorkNumber>,
    standing: BTreeMap<WorkNumber, Standing>,
    /// For each work, the items it waits for where none of them is going on.
    idle: BTreeMap<WorkNumber, Option<Vec<Idle>>>,
}

impl Reader<'_> {
    /// The line of the wait every work shares, the most pressing, where one stands.
    fn shared(&self, outage: Option<Outage>) -> Option<(u8, Line)> {
        let outage = outage?;
        let since = outage.since;
        Some((
            0,
            Line {
                heed: Heed::Agents { outage },
                since,
            },
        ))
    }

    /// The work's line with how pressing it is, where it has one: it stands for the planner.
    fn line(&self, number: WorkNumber) -> Option<(u8, Line)> {
        let (rank, heed) = match &self.standing[&number] {
            Standing::Fault(_) => (1, Heed::Fault { work: number }),
            Standing::Still => (1, Heed::Stalled { work: number }),
            Standing::Owner { .. } => (2, Heed::ForAPerson { work: number }),
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
            Standing::PausedByOwner { .. } => {
                let items = self.pause_over(number)?;
                (4, Heed::PauseOver { work: number, items })
            }
            Standing::Going => return None,
        };
        Some((
            rank,
            Line {
                heed,
                since: self.since(number),
            },
        ))
    }

    /// Since when the work has stood as it does; for an idle work, since the latest of that, of the times the items
    /// it waits for stopped going on, and of the times those it waited for merged or were cancelled: it was not idle
    /// before the last of them ended.
    fn since(&self, number: WorkNumber) -> Time {
        let own = self.stood_from(number);
        let Some(Some(items)) = self.idle.get(&number) else {
            // A pause that waited for items was not over before the last of them ended.
            return (self.ended_paused_for(number).map(|on| self.since(on))).fold(own, Time::max);
        };
        let ended = self.ended_waited_for(number).map(|on| self.since(on));
        (items.iter())
            .map(|idle| self.since(self.of_item[&idle.item]))
            .chain(ended)
            .fold(own, Time::max)
    }

    /// The items, by their numbers, the work's pause waits for, where the owner paused it.
    fn pause_items(&self, number: WorkNumber) -> Vec<u64> {
        match self.standing.get(&number) {
            Some(Standing::PausedByOwner { .. }) => (decide::pause(&self.works[&number].work.records))
                .map_or_else(Vec::new, |pause| pause.waits_for.clone()),
            _ => Vec::new(),
        }
    }

    /// The works of the items the work's pause waits for that merged or were cancelled.
    fn ended_paused_for(&self, number: WorkNumber) -> impl Iterator<Item = WorkNumber> + '_ {
        (self.pause_items(number).into_iter())
            .filter_map(|item| self.of_item.get(&ItemNumber(item)).copied())
            .filter(|on| matches!(self.standing[on], Standing::Ended(_)))
    }

    /// The items the owner's pause of the work waited for, where it waited for some and each has merged or been
    /// cancelled; an item with no work goes on, as it does for an item that waits for it.
    fn pause_over(&self, number: WorkNumber) -> Option<Vec<ItemNumber>> {
        let items = self.pause_items(number);
        let open = (items.iter())
            .filter_map(|item| self.of_item.get(&ItemNumber(*item)))
            .any(|on| !matches!(self.standing[on], Standing::Ended(_)));
        (!items.is_empty() && !open).then(|| items.into_iter().map(ItemNumber).collect())
    }

    /// The works of the items the work's item depends on that merged or were cancelled, which it waits for no more.
    fn ended_waited_for(&self, number: WorkNumber) -> impl Iterator<Item = WorkNumber> + '_ {
        (self.works[&number].item.depends_on.iter())
            .filter_map(|item| self.of_item.get(item).copied())
            .filter(|on| matches!(self.standing[on], Standing::Ended(_)))
    }

    /// When the work came to stand as it does: for a work that ended, the time of the entry its records end at, its
    /// merge's finish or its cancel; for any other, or one whose records place no end, the first moment from which it
    /// has stood so without a break. A work that ended is not decided again moment by moment, which for every work that
    /// ever ended would be a decision at each read.
    fn stood_from(&self, number: WorkNumber) -> Time {
        let kept = &self.works[&number];
        let ended = match &self.standing[&number] {
            Standing::Ended(end) => owner::ended_at(&kept.work.records, *end).and_then(|at| kept.work.time_of(at)),
            _ => None,
        };
        ended.unwrap_or_else(|| self.began(number).map_or(Time::default(), |moment| moment.at))
    }

    /// The moment from which the work has stood as it does without a break.
    fn began(&self, number: WorkNumber) -> Option<&Moment> {
        let kept = &self.works[&number];
        let now = &self.standing[&number];
        let moments = &kept.work.moments;
        for moment in moments.iter().rev() {
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
                // A cancelled item is waited for no more than a merged one.
                Standing::Ended(_) => continue,
                Standing::Fault(_) => Stuck::Fault,
                Standing::Owner { .. } => Stuck::ForAPerson,
                Standing::Still => Stuck::Stalled,
                Standing::PausedByOwner { .. } => Stuck::Paused,
                Standing::Items { .. } => {
                    self.idle.get(on)?.as_ref()?;
                    Stuck::Idle
                }
                Standing::Going => return None,
            };
            items.push(Idle { item: *item, why });
        }
        // Waiting for items that have all merged or were cancelled, the work is about to be told so.
        (!items.is_empty()).then_some(items)
    }
}
