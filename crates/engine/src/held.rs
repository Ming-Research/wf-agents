//! What holds a work back, as the engine leaves it between two things that happen to it.
//!
//! The walk says, as data, what holds back a run it would start: the work's pause or the owner's pause of everything
//! (`Paused`), the turn of the work's project or a slot (a wait for one). Of a run at rest that goes on, it is what the
//! run does next that says whether anything holds it: a pause holds back only a round, and only some rounds of a run
//! that answers a note, and the turn and a slot only the rounds of a cell that takes them. That is worked out here, with
//! the run's own log, by the same functions the engine's loop uses, so that what is shown of a work and what the loop
//! did with it cannot differ.

use decide::run::{self, Hold, Step};
use decide::{BlockId, Decision, Holder, Room, Write, decide_in, hold_in, paused, told};
use flow::Compiled;
use work::Work;

use crate::handle::{answers, choices, log, nothing_but, requires, stuck_blocks};

/// What holds back the round the work would begin next, in the room the engine leaves it; none where nothing does, or
/// where the work would begin no round. Of the holders, the work's own pause is given first, for only the owner lifts
/// it and it outlasts the others; then the owner's pause of everything; then the turn, which another work gives up;
/// then a slot.
pub fn held(flow: &Compiled, work: &Work, room: Room) -> Option<Holder> {
    let records = &work.records;
    let pause = || {
        if paused(records) {
            Holder::Pause
        } else {
            Holder::Everything
        }
    };
    match decide_in(&flow.flow, records, room) {
        Decision::Paused { .. } => Some(pause()),
        Decision::Wait { on, .. } if on == [decide::Waited::Turn] => Some(Holder::Turn),
        Decision::Wait { on, .. } if on == [decide::Waited::Slot] => Some(Holder::Slot),
        Decision::GoOn { run, .. } => {
            let choices = choices(flow);
            let nothing_but = nothing_but(flow);
            let answers = answers(flow, work, run);
            let told = told(&flow.flow, records, run)?;
            // As the loop asks it: a round the pause holds back is held whether the return before it was sent back for
            // what it left out of the blocks the work needs, or for any other fault.
            let stuck =
                |dealt_with: &[BlockId], writes: &[Write]| stuck_blocks(flow, work, &choices, run, dealt_with, writes);
            let requires = requires(
                flow,
                &choices,
                &nothing_but,
                &answers,
                &told,
                Some(run::Stuck(&stuck)),
                work,
                run,
            )?;
            let log = log(flow, work, run, &told);
            let held = |hold: Hold| run::next(&log, requires, hold) == Step::Held;
            // The pauses alone, as if the turn were this work's and a slot were free; then the turn; then everything the
            // room holds back.
            let paused_only = Room {
                slot: true,
                turn: true,
                ..room
            };
            if held(hold_in(&flow.flow, records, run, paused_only)) {
                return Some(pause());
            }
            let turn_only = Room { slot: true, ..room };
            if held(hold_in(&flow.flow, records, run, turn_only)) {
                return Some(Holder::Turn);
            }
            held(hold_in(&flow.flow, records, run, room)).then_some(Holder::Slot)
        }
        _ => None,
    }
}
