//! What the room the engine leaves a work holds back, checked after every step in every room: the walk is asked what it
//! would decide in each, whatever room the history is in. Under the owner's pause of everything the walk decides
//! exactly what it decides of the same records with the work's own pause added after them, and holds back the same of
//! each run at rest (decision `stops-from-outside-are-no-cells`: a pause holds back only what would begin a round, and
//! a paused work still answers a note). With no slot free it decides what it decides with one, but that a cell that
//! takes a slot, which it would run, waits for one at that cell; and it holds back every round of a run at rest of
//! such a cell, and nothing else.

use decide::run::Hold;
use decide::{Decision, Flow, Pause, Pauser, Records, Room, Seq, Waited, decide_in, hold_in};

use crate::properties::{Verdict, last_place};

pub const AS_ITS_OWN: &str = "the pause of everything holds back what the work's own pause does";
pub const SLOT_HOLDS: &str = "with no slot free, a cell that takes one begins no round, and nothing else is held";

/// What the histories reached of the room, each of which some history must reach.
pub const REACHED: &[&str] = &[EVERYTHING_HELD, EVERYTHING_ANSWERS, SLOT_WAITED, SLOT_HELD];

const EVERYTHING_HELD: &str = "the pause of everything holds back a run";
const EVERYTHING_ANSWERS: &str = "a run for a note starts under the pause of everything";
const SLOT_WAITED: &str = "a cell waits for a slot";
const SLOT_HELD: &str = "a run at rest has no round for want of a slot";

/// The records with the work's own pause added after everything they hold.
fn paused_itself(records: &Records) -> Records {
    let mut own = records.clone();
    own.pauses.push(Pause {
        paused: true,
        at: Seq(last_place(records).0 + 1),
        by: Pauser::Owner,
        why: None,
    });
    own
}

/// What must hold of the walk of these records in every room.
pub fn decision(v: &mut Verdict, flow: &Flow, records: &Records) {
    let own = paused_itself(records);
    for slot in [true, false] {
        let under = decide_in(flow, records, Room { paused: true, slot });
        let itself = decide_in(flow, &own, Room { paused: false, slot });
        if under != itself {
            v.fail(
                AS_ITS_OWN,
                format!("under the pause of everything the walk decides {under:?}, and paused itself {itself:?}"),
            );
        }
        match &under {
            Decision::Paused => v.count(EVERYTHING_HELD),
            Decision::Run { .. } => v.count(EVERYTHING_ANSWERS),
            _ => {}
        }
        for run in records.started.iter().map(|s| s.run) {
            let under = hold_in(flow, records, run, Room { paused: true, slot });
            let itself = hold_in(flow, &own, run, Room { paused: false, slot });
            if under != itself {
                v.fail(
                    AS_ITS_OWN,
                    format!("of run {run:?} the pause of everything holds back {under:?}, and the work's {itself:?}"),
                );
            }
        }
    }
    for paused in [false, true] {
        let free = decide_in(flow, records, Room { paused, slot: true });
        let full = decide_in(flow, records, Room { paused, slot: false });
        let expected = match &free {
            Decision::Run { cell, .. } if flow.cells()[flow.place(cell).expect("a cell of the flow")].slot => {
                v.count(SLOT_WAITED);
                Decision::Wait {
                    at: cell.clone(),
                    on: vec![Waited::Slot],
                }
            }
            other => other.clone(),
        };
        if full != expected {
            v.fail(
                SLOT_HOLDS,
                format!("with a slot free the walk decides {free:?}, and with none {full:?}"),
            );
        }
        for s in &records.started {
            let free = hold_in(flow, records, s.run, Room { paused, slot: true });
            let full = hold_in(flow, records, s.run, Room { paused, slot: false });
            let takes = flow.place(&s.cell).is_none_or(|i| flow.cells()[i].slot);
            let expected = if takes { Hold::Rounds } else { free };
            if takes
                && free != Hold::Rounds
                && matches!(decide_in(flow, records, Room::FREE), Decision::GoOn { run, .. } if run == s.run)
            {
                v.count(SLOT_HELD);
            }
            if full != expected {
                v.fail(
                    SLOT_HOLDS,
                    format!(
                        "of run {:?} with no slot free the room holds back {full:?}, with one {free:?}",
                        s.run
                    ),
                );
            }
        }
    }
}
