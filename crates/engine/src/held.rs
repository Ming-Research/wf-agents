//! What holds a work back, as the engine leaves it between two things that happen to it.
//!
//! The walk says, as data, what holds back a run it would start: the work's pause or the owner's pause of everything
//! (`Paused`), or a slot (a wait for one). Of a run at rest that goes on, it is what the run does next that says whether
//! anything holds it: a pause holds back only a round, and only some rounds of a run that answers a note, and a slot
//! only the rounds of a cell that takes one. That is worked out here, with the run's own log, by the same functions
//! the engine's loop uses, so that what is shown of a work and what the loop did with it cannot differ.

use decide::run::{self, Hold, Step};
use decide::{Decision, Holder, Room, decide_in, hold_in, paused};
use flow::Compiled;
use work::Work;

use crate::handle::{choices, log, requires};

/// What holds back the round the work would begin next, in the room the engine leaves it; none where nothing does, or
/// where the work would begin no round. Of the holders, the work's own pause is given first, for only the owner lifts
/// it and it outlasts the others; then the owner's pause of everything; then a slot.
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
        Decision::Paused => Some(pause()),
        Decision::Wait { on, .. } if on == [decide::Waited::Slot] => Some(Holder::Slot),
        Decision::GoOn { run, .. } => {
            let choices = choices(flow);
            let requires = requires(flow, &choices, work, run)?;
            let log = log(work, run);
            let held = |hold: Hold| run::next(&log, requires, hold) == Step::Held;
            // The pauses alone, as if a slot were free; then everything the room holds back.
            let paused_only = Room { slot: true, ..room };
            if held(hold_in(&flow.flow, records, run, paused_only)) {
                return Some(pause());
            }
            held(hold_in(&flow.flow, records, run, room)).then_some(Holder::Slot)
        }
        _ => None,
    }
}
