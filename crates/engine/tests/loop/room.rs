//! The room the engine leaves a work (`decide::Room`): where no slot is free, no round of an agent begins, and what
//! begins no round is done all the same: what a round hands in is taken, the calls it asks for are made, and the
//! engine's own act runs; under the owner's pause of everything no round begins but of a run that answers a note. What
//! holds the work back is said (`engine::held`): its own pause first, then the pause of everything, then a slot.

use decide::{Holder, Room};
use engine::{handle_in, held};

use super::*;

const FULL: Room = Room {
    paused: false,
    slot: false,
};

const PAUSED: Room = Room {
    paused: true,
    slot: true,
};

impl W {
    /// Takes the event in this room, which the loop must take, and gives what is to be done.
    fn on_in(&mut self, event: Event, room: Room) -> Vec<Effect> {
        let now = self.tick();
        handle_in(&self.flow, &mut self.work, now, event, room).expect("the event is taken")
    }
}

#[test]
fn with_no_slot_free_a_request_begins_no_round_and_the_work_waits_for_a_slot() {
    let mut w = W::new();
    assert_eq!(w.on_in(wrote("request", Outsider::Person, true, None), FULL), vec![]);
    assert!(w.work.records.started.is_empty(), "no run starts without a slot");
    assert_eq!(held(&w.flow, &w.work, FULL), Some(Holder::Slot));
    assert_eq!(held(&w.flow, &w.work, Room::FREE), None);
    let (run, cell, ..) = round(&w.on_in(Event::Look, Room::FREE));
    assert_eq!((run, cell.as_str()), (RunId(1), "write"));
}

#[test]
fn with_no_slot_free_what_a_round_hands_in_is_taken_and_the_next_agents_round_waits() {
    let (mut w, write, _) = W::requested();
    assert_eq!(
        w.on_in(
            Event::Returned {
                run: write,
                came: hands_in(&[("text", None)]),
                spent: spent(1)
            },
            FULL
        ),
        vec![]
    );
    assert!(
        w.work.records.finished.iter().any(|f| f.run == write),
        "the writer finished"
    );
    assert_eq!(held(&w.flow, &w.work, FULL), Some(Holder::Slot));
}

#[test]
fn with_no_slot_free_the_calls_a_round_asks_for_are_made_and_the_run_given_their_results_waits() {
    let (mut w, write, _) = W::requested();
    let effects = w.on_in(
        Event::Returned {
            run: write,
            came: Came::asks(vec![tool("ci")]),
            spent: spent(1),
        },
        FULL,
    );
    let calls = calls_of(&effects);
    assert_eq!(calls.len(), 1, "the call is made: {effects:?}");
    assert_eq!(held(&w.flow, &w.work, FULL), None, "the cell waits for the call");
    let now = w.tick();
    let effects = handle_in(
        &w.flow,
        &mut w.work,
        now,
        Event::Done {
            call: calls[0].0,
            result: "passed".to_string(),
        },
        FULL,
    )
    .expect("the result is taken");
    assert_eq!(effects, vec![]);
    assert_eq!(held(&w.flow, &w.work, FULL), Some(Holder::Slot));
    let (run, _, number, given) = round(&w.on_in(Event::Look, Room::FREE));
    assert_ne!(run, write);
    assert_eq!((number, given), (1, Given::Results(vec![calls[0].0])));
}

#[test]
fn with_no_slot_free_the_engines_own_act_runs() {
    let (mut w, write, _) = W::requested();
    let judge = round(&w.returns(write, hands_in(&[("text", None)]))).0;
    w.returns(judge, hands_in(&[("verdict", Some("yes"))]));
    let effects = w.on_in(wrote("go", Outsider::Person, true, Some("yes")), FULL);
    let (_, cell, ..) = round(&effects);
    assert_eq!(cell, "merge", "the merge, an act of the engine's own, takes no slot");
}

#[test]
fn under_the_pause_of_everything_no_round_begins_but_of_a_run_for_a_note() {
    let mut w = W::new();
    assert_eq!(w.on_in(wrote("request", Outsider::Person, true, None), PAUSED), vec![]);
    assert_eq!(held(&w.flow, &w.work, PAUSED), Some(Holder::Everything));
    let (write, ..) = round(&w.on_in(Event::Look, Room::FREE));
    let judge = round(&w.returns(write, hands_in(&[("text", None)]))).0;
    // The judge's round under way goes on, and what it hands in is taken; the writer, run again for the judge's
    // opinions, waits.
    assert_eq!(
        w.on_in(
            Event::Returned {
                run: judge,
                came: hands_in(&[("opinions", None)]),
                spent: spent(2)
            },
            PAUSED
        ),
        vec![]
    );
    assert_eq!(held(&w.flow, &w.work, PAUSED), Some(Holder::Everything));
    // A note a person writes to the writer is answered.
    let (run, cell, ..) = round(&w.on_in(wrote("opinions", Outsider::Person, true, None), PAUSED));
    assert_eq!(cell, "write");
    assert_ne!(run, write);
}

#[test]
fn the_works_own_pause_is_said_before_the_pause_of_everything_and_both_before_a_slot() {
    let mut w = W::new();
    w.on(Event::Paused {
        by: Pauser::Owner,
        why: None,
    });
    w.on(wrote("request", Outsider::Person, true, None));
    let neither = Room {
        paused: true,
        slot: false,
    };
    assert_eq!(held(&w.flow, &w.work, neither), Some(Holder::Pause));
    let (write, ..) = round(&w.on(Event::Resumed));
    assert_eq!(
        w.on_in(
            Event::Returned {
                run: write,
                came: hands_in(&[("verdict", Some("yes"))]),
                spent: spent(1)
            },
            neither
        ),
        vec![]
    );
    assert_eq!(
        held(&w.flow, &w.work, neither),
        Some(Holder::Everything),
        "the round sent back waits"
    );
    assert_eq!(held(&w.flow, &w.work, FULL), Some(Holder::Slot));
}
