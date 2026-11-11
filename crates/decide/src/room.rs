//! What the engine as a whole leaves a work room for, beside the work's own records.
//!
//! Two things hold back every work at once, and neither is a cell or a block (entry `stops-from-outside-are-no-cells`):
//! the owner's pause of everything, and the slots, which bound how many rounds of agents go on at once. The scheduler
//! works out both, from the records it keeps of the installation, the settings and the records of every work, and the
//! walk reads them as given ([`Room`]), so that what a work does next is still a function of what it is given alone
//! (principle `decide-purely`).
//!
//! The pause of everything holds back what the work's own pause holds back, and nothing else: a run that answers a note,
//! a person's words to its cell, still has its rounds. A slot is wanted by every round of a cell that takes one, a
//! round that answers a note among them, and by nothing that begins no round: what a round handed in is taken, and the
//! calls it asked for are made, whether a slot is free or not.

use crate::flow::Flow;
use crate::records::Records;
use crate::walk::read_runs;

/// What the engine leaves room for, as the scheduler gives it to the walk of one work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Room {
    /// Whether the owner's pause of everything holds.
    pub paused: bool,
    /// Whether a slot is free for the work, were it to begin a round of a cell that takes one.
    pub slot: bool,
}

impl Room {
    /// Room for everything: nothing is paused, and a slot is free.
    pub const FREE: Room = Room {
        paused: false,
        slot: true,
    };
}

impl Default for Room {
    fn default() -> Self {
        Room::FREE
    }
}

/// What holds back the round a work would begin next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Holder {
    /// The work's own pause, by the owner or by the engine.
    Pause,
    /// The owner's pause of everything.
    Everything,
    /// No slot is free.
    Slot,
}

/// Whether the work has begun a round of a cell that takes a slot: a round of the engine's own act alone does not make
/// it begun, for that act takes no slot and runs as soon as it may.
pub fn begun(flow: &Flow, records: &Records) -> bool {
    (read_runs(records).values())
        .filter(|run| run.began > 0)
        .any(|run| (flow.place(&run.started.cell)).is_none_or(|i| flow.cells()[i].slot))
}

/// Whether the work takes a slot now: a round of a cell that takes one is under way. A round under way of a cell the
/// flow no longer has takes one, for nothing says that it does not; so does the round of a run that was abandoned,
/// until it has ended.
pub fn taking_slot(flow: &Flow, records: &Records) -> bool {
    (read_runs(records).values())
        .filter(|run| run.began > run.ended)
        .any(|run| (flow.place(&run.started.cell)).is_none_or(|i| flow.cells()[i].slot))
}
