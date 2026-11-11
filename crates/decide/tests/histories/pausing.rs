//! What a pause holds back, checked after every step: a paused work still answers a note, a person's words to a cell,
//! and begins nothing else. While the work is paused, a run starts only for a cell one of whose reasons is a change of
//! meaning of a note that a person made since the cell last ran; the pause alone holds back no cell one of whose
//! reasons is such a change; and the engine gives a run at rest its next round under a pause exactly when the run
//! started for such a change and its last round did not fail. All are judged by the histories' own reading of the
//! reasons (`reading`); which cell the work would run once let on is what the walk says of the same records with the
//! pause lifted, as `properties::reaches` takes it.

use decide::run::Hold;
use decide::{Cell, Decision, Flow, Pause, Pauser, Seq, decide};

use crate::properties::{Verdict, last_place};
use crate::reading::{Reading, Reason, Writer};

pub const ONLY_FOR_A_NOTE: &str = "a paused work starts a run only for a note a person wrote";
pub const ANSWERS: &str = "a paused work answers a note a person wrote";

/// What the histories reached of a pause, each of which some history must reach.
pub const REACHED: &[&str] = &[STARTED_PAUSED, HELD_PAUSED, WENT_ON_PAUSED];

const STARTED_PAUSED: &str = "a run starts while the work is paused";
const HELD_PAUSED: &str = "a paused work holds back a cell that has a reason to run";
pub const WENT_ON_PAUSED: &str = "a run started for a note has its next round while the work is paused";

/// Whether one of these reasons, of a run of the cell that starts or started at this place, is a change of meaning
/// that a person made since the cell last ran: of a note it reads, after the version the cell's record knew where it
/// has a record, after the start of the run of its record, or with no record after the start of its first run, and
/// before the place. A cell no run of which started before the place has not run, and has none.
pub fn for_a_note(flow: &Flow, reading: &Reading, cell: &Cell, reasons: &[Reason], place: Seq) -> bool {
    let record = reading.record(&cell.id, place).map(|run| reading.started[&run].at);
    let first = (reading.started.values())
        .filter(|s| s.cell == cell.id && s.at < place)
        .map(|s| s.at)
        .min();
    let Some(since) = record.or(first) else {
        return false;
    };
    let by_person = |block, was: Option<Seq>| {
        flow.is_note(block)
            && (reading.of(block).iter()).any(|v| {
                v.by == Writer::Person && v.changed_meaning() && v.at > since && v.at < place && Some(v.at) > was
            })
    };
    reasons.iter().any(|r| match r {
        Reason::NoRecord => cell.inputs.iter().any(|block| by_person(block, None)),
        Reason::Changed { block, was } => by_person(block, *was),
    })
}

/// What must hold of each decision as it is taken, `paused` saying whether the records have the work paused.
pub fn decision(v: &mut Verdict, flow: &Flow, reading: &Reading, d: &Decision, paused: bool) {
    if !paused {
        return;
    }
    let now = Seq(u64::MAX);
    let cell_of = |cell: &decide::CellId| &flow.cells()[flow.place(cell).expect("a cell of the flow")];
    match d {
        Decision::Run { cell, .. } => {
            v.count(STARTED_PAUSED);
            let reasons = reading.reasons(cell_of(cell), now);
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
            if let Decision::Run { cell, .. } = decide(flow, &let_on) {
                v.count(HELD_PAUSED);
                let reasons = reading.reasons(cell_of(&cell), now);
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

/// Whether a run at rest has its next round though the work is paused, which the engine asks of `decide::hold`: the
/// run started for a note a person wrote, by the histories' own reading, and its last round did not fail (`failed`).
/// Says so where the two differ, and gives what the engine does.
pub fn goes_on(v: &mut Verdict, flow: &Flow, reading: &Reading, run: decide::RunId, failed: bool) -> bool {
    let hold = decide::hold(flow, reading.records, run);
    let goes = match hold {
        Hold::Nothing => true,
        Hold::Retry => !failed,
        Hold::Rounds => false,
    };
    let s = reading.started[&run];
    let cell = &flow.cells()[flow.place(&s.cell).expect("the histories do not change the flow")];
    let expected = !failed && for_a_note(flow, reading, cell, &reading.reasons(cell, s.at), s.at);
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
