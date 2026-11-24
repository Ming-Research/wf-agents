//! What the engine as a whole leaves a work room for, beside the work's own records.
//!
//! Three things hold back every work at once, and none is a cell or a block: the owner's pause of everything, the
//! slots, which bound how many rounds of
//! agents go on at once, and the turn of a project, which lets one work of a project at a time check and merge. The
//! scheduler works them out, from the records it keeps of the installation, the settings and the records of every work,
//! and the walk reads them as given ([`Room`]), so that what a work does next is still a function of what it is given
//! alone (principle `decide-purely`).
//!
//! The pause of everything holds back what the work's own pause holds back, and nothing else: a run that answers a
//! note, a person's words to its cell, still has its rounds. A slot is wanted by every round of a cell that takes one,
//! a round that answers a note among them, and by nothing that begins no round: what a round handed in is taken, and
//! the calls it asked for are made, whether a slot is free or not. The turn is wanted by every round of a cell that
//! takes it, and likewise by nothing else.
//!
//! The turn is a function of the records of the works of a project: a work holds it from its first round of a cell that
//! takes it for as long as its walk stays at such cells ([`holds_turn`]), and the work that wants it longest is the one
//! to have it next ([`wants_turn`]). Nothing is kept for it beyond those records and the flow.

use crate::flow::{CellId, Flow};
use crate::records::{Records, Seq};
use crate::walk::{Decision, decide, ended, read_runs};

/// What the engine leaves room for, as the scheduler gives it to the walk of one work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Room {
    /// Whether the owner's pause of everything holds.
    pub paused: bool,
    /// Whether a slot is free for the work, were it to begin a round of a cell that takes one.
    pub slot: bool,
    /// Whether it is the work's turn to check and merge, were it to begin a round of a cell that takes the turn: no
    /// other work of its project holds the turn, and none that wants it has wanted it longer.
    pub turn: bool,
}

impl Room {
    /// Room for everything: nothing is paused, a slot is free, and it is the work's turn.
    pub const FREE: Room = Room {
        paused: false,
        slot: true,
        turn: true,
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
    /// It is not the work's turn: another work of its project holds the turn, or has wanted it longer.
    Turn,
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

/// Whether the cell of this id takes the turn of its work's project. A cell the flow no longer has takes none: nothing
/// says that it does.
fn takes_turn(flow: &Flow, cell: &CellId) -> bool {
    flow.place(cell).is_some_and(|i| flow.cells()[i].turn)
}

/// Whether the work holds the turn of its project: it has not ended, and its latest round is of a cell that takes the
/// turn, and either that round is under way, or it has ended and the work's walk stands at a cell that takes the turn,
/// to begin a round of it (a run to start, or a run at rest to go on), with no pause of its own having held it since
/// the round ended. So a check, the check again where main moved, a round that failed and is tried again, and the merge
/// that follows a check that passed are one hold; the work lets go as its walk stands anywhere else (it was sent back,
/// waits for main or for an answer, has ended), and as its own pause, the owner's or the engine's after rounds that
/// failed, holds it once its round under way has ended, and the hold stays ended until the work's next round of such a
/// cell begins. A pause set and lifted while the round was under way does not end it; the owner's pause of everything
/// is no pause of the work's own, and does not.
///
/// A function of the work's flow and records alone, so that the turn is the same however and whenever it is read, and
/// after a restart.
pub fn holds_turn(flow: &Flow, records: &Records) -> bool {
    let Some(latest) = records.rounds.iter().max_by_key(|round| round.at) else {
        return false;
    };
    let Some(started) = records.started.iter().find(|s| s.run == latest.run) else {
        return false;
    };
    if !takes_turn(flow, &started.cell) || records.cancelled().is_some() {
        return false;
    }
    let last = &flow.cells()[flow.cells().len() - 1].id;
    if ended(last, records).is_some() {
        return false;
    }
    // The latest round of the run is the run's latest round of all, whichever it is.
    let began = records.rounds.iter().filter(|round| round.run == latest.run).count();
    let mut returned: Vec<Seq> = (records.returned.iter())
        .filter(|returned| returned.run == latest.run)
        .map(|returned| returned.at)
        .collect();
    if returned.len() < began {
        return true;
    }
    returned.sort();
    let Some(end) = returned.get(began - 1).copied() else {
        return false;
    };
    // Its own pause: one that held it as the round ended, or was set after.
    let before = (records.pauses.iter())
        .filter(|pause| pause.at < end)
        .max_by_key(|pause| pause.at)
        .is_some_and(|pause| pause.paused);
    let after = records.pauses.iter().any(|pause| pause.paused && pause.at > end);
    if before || after {
        return false;
    }
    match decide(flow, records) {
        Decision::Run { cell, .. } | Decision::GoOn { cell, .. } => takes_turn(flow, &cell),
        _ => false,
    }
}

/// Whether the work wants the turn of its project and does not hold it: it has not ended, and its walk stands at a cell
/// that takes the turn, to begin a round of it, or, its own pause holding it back, would. The one that has wanted it
/// longest has it next.
pub fn wants_turn(flow: &Flow, records: &Records) -> bool {
    if holds_turn(flow, records) {
        return false;
    }
    match decide(flow, records) {
        Decision::Run { cell, .. } | Decision::GoOn { cell, .. } | Decision::Paused { cell } => takes_turn(flow, &cell),
        _ => false,
    }
}
