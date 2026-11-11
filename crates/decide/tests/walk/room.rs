//! What the engine as a whole leaves a work room for (`decide::Room`): the owner's pause of everything holds back
//! exactly what the work's own pause holds back, a note being answered under it; and where no slot is free, a cell
//! that takes one waits for it before it runs, a run at rest of such a cell has no round, and a cell that takes none,
//! as the engine's own acts, goes on as it would.

use decide::run::Hold;
use decide::{Room, decide_in, hold_in, taking_slot};

use super::*;

/// The owner's pause of everything, a slot free.
const PAUSED: Room = Room {
    paused: true,
    slot: true,
};

/// No slot free, nothing paused.
const FULL: Room = Room {
    paused: false,
    slot: false,
};

/// A research that reads the background, which people write, and a note to it; an implementation that reads the plan;
/// both ran once, and the work rests at the merge.
fn ran() -> World {
    let cells = vec![
        cell("research", &["background", "to research"], &["plan"]),
        cell("implement", &["plan"], &["code"]),
    ];
    let mut w = World::with_gate(cells);
    w.notes(&["to research"]);
    w.person_writes("background");
    w.run("research", &[("plan", Matters)]);
    w.run("implement", &[("code", Matters)]);
    assert_eq!(w.decide(), resting());
    w
}

/// The work's records with its own pause besides, after everything else.
fn paused_itself(w: &World) -> decide::Records {
    let mut records = w.records.clone();
    let last = (records.started.iter().map(|e| e.at))
        .chain(records.rounds.iter().map(|e| e.at))
        .chain(records.returned.iter().map(|e| e.at))
        .chain(records.finished.iter().map(|e| e.at))
        .chain(records.written.iter().map(|e| e.at))
        .chain(records.pauses.iter().map(|e| e.at))
        .max()
        .unwrap_or_default();
    records.pauses.push(decide::Pause {
        paused: true,
        at: Seq(last.0 + 1),
        by: Pauser::Owner,
        why: None,
    });
    records
}

#[test]
fn the_pause_of_everything_holds_back_a_run_as_the_works_own_pause_does() {
    let mut w = ran();
    w.person_writes("background");
    assert_eq!(decide_in(&w.flow, &w.records, PAUSED), Decision::Paused);
    assert_eq!(
        decide_in(&w.flow, &w.records, PAUSED),
        decide::decide(&w.flow, &paused_itself(&w))
    );
    // Lifted, the run goes on as it would have: nothing of the pause of everything is in the work's records.
    assert_eq!(decide_in(&w.flow, &w.records, Room::FREE), w.decide());
    assert!(matches!(w.decide(), Decision::Run { cell, .. } if cell.as_str() == "research"));
}

#[test]
fn the_pause_of_everything_answers_a_note_as_the_works_own_pause_does() {
    let mut w = ran();
    w.person_writes("to research");
    let under = decide_in(&w.flow, &w.records, PAUSED);
    assert!(
        matches!(&under, Decision::Run { cell, .. } if cell.as_str() == "research"),
        "{under:?}"
    );
    assert_eq!(under, decide::decide(&w.flow, &paused_itself(&w)));
}

#[test]
fn the_works_own_pause_holds_after_the_pause_of_everything_is_lifted() {
    let mut w = ran();
    w.person_writes("background");
    w.pause();
    assert_eq!(decide_in(&w.flow, &w.records, PAUSED), Decision::Paused);
    assert_eq!(decide_in(&w.flow, &w.records, Room::FREE), Decision::Paused);
}

#[test]
fn with_no_slot_free_a_cell_that_takes_one_waits_for_it_and_one_that_takes_none_runs() {
    let mut w = World::with_gate(vec![cell("plan", &["request"], &["plan"])]);
    w.person_writes("request");
    assert_eq!(
        decide_in(&w.flow, &w.records, FULL),
        Decision::Wait {
            at: "plan".into(),
            on: vec![Waited::Slot],
        }
    );
    assert!(matches!(
        decide_in(&w.flow, &w.records, Room::FREE),
        Decision::Run { .. }
    ));
    let mut act = cell("plan", &["request"], &["plan"]);
    act.slot = false;
    w.install(vec![act, gate()]);
    assert_eq!(decide_in(&w.flow, &w.records, FULL), w.decide());
    assert!(matches!(w.decide(), Decision::Run { .. }));
}

#[test]
fn a_note_answered_under_the_pause_of_everything_waits_for_a_slot_too() {
    let mut w = ran();
    w.person_writes("to research");
    let neither = Room {
        paused: true,
        slot: false,
    };
    assert_eq!(
        decide_in(&w.flow, &w.records, neither),
        Decision::Wait {
            at: "research".into(),
            on: vec![Waited::Slot],
        }
    );
}

#[test]
fn a_slot_holds_back_every_round_of_a_cell_that_takes_one_and_the_pause_of_everything_what_a_pause_does() {
    let mut w = ran();
    w.person_writes("background");
    let run = w.start("research");
    w.returns("research");
    assert_eq!(hold_in(&w.flow, &w.records, run, Room::FREE), Hold::Nothing);
    assert_eq!(hold_in(&w.flow, &w.records, run, FULL), Hold::Rounds);
    assert_eq!(hold_in(&w.flow, &w.records, run, PAUSED), Hold::Rounds);
    // Of a run for a note, the pause of everything holds back what the work's own pause does: only a round given again
    // after one that failed for which the engine paused the work, and so, with none, nothing; a slot, every round.
    w.finish("research", &[("plan", Matters)]);
    w.person_writes("to research");
    let note = w.start("research");
    w.returns("research");
    assert_eq!(hold_in(&w.flow, &w.records, note, PAUSED), Hold::Nothing);
    assert_eq!(
        hold_in(&w.flow, &w.records, note, PAUSED),
        decide::hold(&w.flow, &paused_itself(&w), note)
    );
    assert_eq!(hold_in(&w.flow, &w.records, note, FULL), Hold::Rounds);
    // The run goes on whatever the room: what it does next is the engine's to hold back.
    assert_eq!(decide_in(&w.flow, &w.records, FULL), go_on("research", note));
}

#[test]
fn a_work_takes_a_slot_while_a_round_of_a_cell_that_takes_one_is_under_way() {
    let mut act = cell("start", &["request"], &["place"]);
    act.slot = false;
    let mut w = World::with_gate(vec![act, cell("plan", &["place"], &["plan"])]);
    w.person_writes("request");
    assert!(!taking_slot(&w.flow, &w.records));
    w.start("start");
    assert!(
        !taking_slot(&w.flow, &w.records),
        "an act of the engine's own takes no slot"
    );
    w.finish("start", &[("place", Matters)]);
    w.start("plan");
    assert!(taking_slot(&w.flow, &w.records));
    w.returns("plan");
    assert!(!taking_slot(&w.flow, &w.records), "a run at rest takes no slot");
}
