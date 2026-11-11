//! What a pause holds back, checked after every step: a paused work still answers what a person writes for a cell,
//! and begins nothing else. While the work is paused, a run starts only for a cell one of whose reasons is a change of
//! meaning a person made since the cell last ran; and the pause alone holds back no cell one of whose reasons is such
//! a change. Both are judged by the histories' own reading of the reasons (`reading`); which cell the work would run
//! once let on is what the walk says of the same records with the pause lifted, as `properties::reaches` takes it.

use decide::{Decision, Flow, Pause, Pauser, Seq, decide};

use crate::properties::{Verdict, last_place};
use crate::reading::{Reading, Reason, Writer};

pub const ONLY_FOR_A_PERSON: &str = "a paused work starts a run only for what a person wrote";
pub const ANSWERS: &str = "a paused work answers what a person wrote";

/// What the histories reached of a pause, each of which some history must reach.
pub const REACHED: &[&str] = &[STARTED_PAUSED, HELD_PAUSED, WENT_ON_PAUSED];

const STARTED_PAUSED: &str = "a run starts while the work is paused";
const HELD_PAUSED: &str = "a paused work holds back a cell that has a reason to run";
pub const WENT_ON_PAUSED: &str = "a run started for what a person wrote has its next round while the work is paused";

/// Whether one of these reasons, of a run that starts or started at this place, is a change of the block's meaning
/// that a person made after the version the cell's record knew, up to the version current just before the place.
pub fn for_a_person(reading: &Reading, reasons: &[Reason], place: Seq) -> bool {
    reasons.iter().any(|r| match r {
        Reason::NoRecord => false,
        Reason::Changed { block, was } => reading
            .current_before(block, place)
            .is_some_and(|now| (reading.changes(block, *was, now.at).iter()).any(|v| v.by == Writer::Person)),
    })
}

/// What must hold of each decision as it is taken, `paused` saying whether the records have the work paused.
pub fn decision(v: &mut Verdict, flow: &Flow, reading: &Reading, d: &Decision, paused: bool) {
    if !paused {
        return;
    }
    let now = Seq(u64::MAX);
    let reasons = |cell: &decide::CellId| {
        let c = &flow.cells()[flow.place(cell).expect("a cell of the flow")];
        reading.reasons(c, now)
    };
    match d {
        Decision::Run { cell, .. } => {
            v.count(STARTED_PAUSED);
            let reasons = reasons(cell);
            if !for_a_person(reading, &reasons, now) {
                v.fail(
                    ONLY_FOR_A_PERSON,
                    format!("{cell} starts a run while the work is paused, for {reasons:?}, which no person wrote"),
                );
            }
        }
        Decision::Paused => {
            let records = reading.records;
            let mut let_on = records.clone();
            let_on.pauses.push(Pause {
                paused: false,
                at: Seq(last_place(records).0 + 1),
                by: Pauser::Owner,
                why: None,
            });
            if let Decision::Run { cell, .. } = decide(flow, &let_on) {
                v.count(HELD_PAUSED);
                let reasons = reasons(&cell);
                if for_a_person(reading, &reasons, now) {
                    v.fail(
                        ANSWERS,
                        format!("the pause holds back {cell}, which runs for {reasons:?}, written by a person"),
                    );
                }
            }
        }
        _ => {}
    }
}
