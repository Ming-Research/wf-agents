//! What a pause holds back, checked after every step: a paused work still answers the owner, a note a person wrote to
//! a cell or the owner turning to it, and begins nothing else. While the work is paused, a run starts only for a cell
//! one of whose reasons is such a version written since the cell last ran; the pause alone holds back no cell one of
//! whose reasons is one; and the engine gives a run at rest its next round under a pause exactly when the run started
//! for one and its last round did not fail in a way that paused the work. All are judged by the histories' own reading
//! of the reasons (`reading`); which cell the work would run once let on is what the walk says of the same records with
//! the pause lifted, as `properties::reaches` takes it.

use decide::run::Hold;
use decide::{Cell, Decision, Flow, Pause, Pauser, Room, Seq, decide_in, hold_in};

use crate::properties::{Verdict, last_place};
use crate::reading::{Reading, Reason, Writer};

pub const ONLY_FOR_A_NOTE: &str = "a paused work starts a run only for a note a person wrote";
pub const ANSWERS: &str = "a paused work answers a note a person wrote";

/// What the histories reached of a pause, each of which some history must reach.
pub const REACHED: &[&str] = &[STARTED_PAUSED, HELD_PAUSED, WENT_ON_PAUSED];

const STARTED_PAUSED: &str = "a run starts while the work is paused";
const HELD_PAUSED: &str = "a paused work holds back a cell that has a reason to run";
pub const WENT_ON_PAUSED: &str = "a run started for a note has its next round while the work is paused";
/// Reached rarely: a note's run whose round failed without pausing the work while the owner's pause held.
pub const UNPAUSED_ANSWERS: &str =
    "a run for a note whose round failed without pausing the work goes on while the work is paused";

/// Whether one of these reasons, of a run of the cell that starts or started at this place, is the owner's since the
/// cell last ran: a version of a note that runs it that a person wrote, or of a block that interrupts that the outside
/// wrote, changing its meaning, after the version the cell consumed, after the start of its latest run that finished
/// dealing with something, or with none after the start of its first run, and before the place. A cell no run of which started before the place
/// has not run, and has none.
pub fn for_a_note(flow: &Flow, reading: &Reading, cell: &Cell, reasons: &[Reason], place: Seq) -> bool {
    // A run that only asked for calls dealt with nothing: the cell has not run for anything by it.
    let record = (reading.finished.values())
        .filter(|f| f.at < place && reading.started[&f.run].cell == cell.id)
        .filter(|f| f.dealt_with.as_ref().is_none_or(|d| !d.is_empty()))
        .max_by_key(|f| f.at)
        .map(|f| reading.started[&f.run].at);
    let first = (reading.started.values())
        .filter(|s| s.cell == cell.id && s.at < place)
        .map(|s| s.at)
        .min();
    let Some(since) = record.or(first) else {
        return false;
    };
    let owners = |block, was: Option<Seq>| {
        (reading.of(block).iter()).any(|v| {
            let theirs = (flow.is_note(block) && v.by == Writer::Person)
                || (flow.interrupts(block) && matches!(v.by, Writer::Person | Writer::Outside));
            theirs && v.changed_meaning() && v.at > since && v.at < place && Some(v.at) > was
        })
    };
    reasons.iter().any(|r| match r {
        Reason::Changed { block, was } => owners(block, *was),
        Reason::Results { .. } => false,
    })
}

/// What must hold of each decision as it is taken in this room, `paused` saying whether the records have the work
/// paused or the owner's pause of everything holds, which hold back the same.
pub fn decision(v: &mut Verdict, flow: &Flow, reading: &Reading, d: &Decision, room: Room, paused: bool) {
    if !paused {
        return;
    }
    let now = Seq(u64::MAX);
    let cell_of = |cell: &decide::CellId| &flow.cells()[flow.place(cell).expect("a cell of the flow")];
    match d {
        Decision::Run { cell, .. } => {
            v.count(STARTED_PAUSED);
            let reasons = reading.entering(flow, cell_of(cell), now);
            if !for_a_note(flow, reading, cell_of(cell), &reasons, now) {
                v.fail(
                    ONLY_FOR_A_NOTE,
                    format!(
                        "{cell} starts a run while the work is paused, for {reasons:?}, none a note a person wrote"
                    ),
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
            let lifted = Room { paused: false, ..room };
            if let Decision::Run { cell, .. } = decide_in(flow, &let_on, lifted) {
                v.count(HELD_PAUSED);
                let reasons = reading.entering(flow, cell_of(&cell), now);
                if for_a_note(flow, reading, cell_of(&cell), &reasons, now) {
                    v.fail(
                        ANSWERS,
                        format!("the pause holds back {cell}, which runs for {reasons:?}, a note a person wrote"),
                    );
                }
            }
        }
        _ => {}
    }
}

/// Whether a run at rest has its next round in this room, which the engine asks of `decide::hold_in`: never where its
/// cell takes a slot and none is free (`room::SLOT_HOLDS`); otherwise, while the work is paused or the owner's pause
/// of everything holds, where the run started for a note a person wrote, by the histories' own reading, and its last
/// round did not fail in a way that paused the work with no letting on since (`failed`); and otherwise always. Says so
/// where the two differ, and gives what the engine does.
pub fn goes_on(v: &mut Verdict, flow: &Flow, reading: &Reading, run: decide::RunId, failed: bool, room: Room) -> bool {
    let hold = hold_in(flow, reading.records, run, room);
    let goes = match hold {
        Hold::Nothing => true,
        Hold::Retry => !failed,
        Hold::Rounds => false,
    };
    let s = reading.started[&run];
    let cell = &flow.cells()[flow.place(&s.cell).expect("the histories do not change the flow")];
    if cell.slot && !room.slot {
        if goes {
            v.fail(
                crate::room::SLOT_HOLDS,
                format!("run {run:?} of {} has its next round with no slot free", s.cell),
            );
        }
        return goes;
    }
    if !decide::paused(reading.records) && !room.paused {
        if !goes {
            v.fail(
                ANSWERS,
                format!("run {run:?} of {} is held back with nothing holding it", s.cell),
            );
        }
        return goes;
    }
    let reasons: Vec<Reason> = (s.because.iter())
        .filter_map(|c| match c {
            decide::Cause::Changed { block, was, .. } => Some(Reason::Changed {
                block: block.clone(),
                was: *was,
            }),
            _ => None,
        })
        .collect();
    let expected = !failed && for_a_note(flow, reading, cell, &reasons, s.at);
    if goes != expected {
        let property = if goes { ONLY_FOR_A_NOTE } else { ANSWERS };
        v.fail(
            property,
            format!(
                "run {run:?} of {} is held back as {hold:?} under a pause, its last round failed: {failed}",
                s.cell
            ),
        );
    }
    goes
}
