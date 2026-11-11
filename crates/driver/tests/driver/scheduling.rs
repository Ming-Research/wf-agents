//! The driver as the scheduler: a work added goes on by itself when it may, unless it is added held; no more rounds of
//! agents go on at once than there are slots, a work whose round ends keeping its slot for its next round, and a slot
//! that comes free going to the works that wait for one in the engine's order; and the owner's pause of everything,
//! kept in the store, holds back what each work's own pause would until it is lifted.

use decide::Holder;

use super::*;

/// A work waits at the writer for a slot.
fn waits_for_a_slot() -> Decision {
    Decision::Wait {
        at: "write".into(),
        on: vec![Waited::Slot],
    }
}

/// The writer's round of the work asks for a tool; gives the call.
fn asks_a_tool(h: &mut H, work: WorkNumber) -> CallId {
    let todos = h.returns(work, Came::Asks(vec![tool("ci")]));
    (todos.iter())
        .find_map(|todo| match &todo.effect {
            Effect::Call { call, .. } if todo.work == work => Some(*call),
            _ => None,
        })
        .expect("the tool is called")
}

#[test]
fn a_work_added_going_on_begins_by_itself_and_one_added_held_once_it_is_let_on() {
    let mut h = H::with_slots(flows(), 2);
    let (_, going) = h.add(going_item("going", &[], "plain"));
    assert_eq!(rounds(&h.d().pending()), [(going, "write".to_string(), 1)]);
    let (_, held) = h.add(new_item("held", &[], "plain"));
    assert_eq!(h.decision(held), Decision::Paused);
    assert_eq!(h.d().held(held), Some(Holder::Pause));
    assert_eq!(rounds(&h.let_on(held)), [(held, "write".to_string(), 1)]);
}

#[test]
fn a_work_added_going_on_still_waits_for_the_items_its_item_depends_on() {
    let mut h = H::with_slots(flows(), 2);
    let (a, _) = h.add(new_item("a", &[], "short"));
    let (_, b) = h.add(going_item("b", &[a], "short"));
    assert_eq!(h.decision(b), waits_for_dependencies("draft"));
    assert_eq!(rounds(&h.d().pending()), []);
}

#[test]
fn no_more_rounds_of_agents_go_on_than_there_are_slots_and_the_others_wait_for_one() {
    let mut h = H::with_slots(flows(), 2);
    let works: Vec<WorkNumber> = (0..3)
        .map(|i| h.add(going_item(&format!("{i}"), &[], "plain")).1)
        .collect();
    assert_eq!(
        rounds(&h.d().pending()),
        [(works[0], "write".to_string(), 1), (works[1], "write".to_string(), 1)]
    );
    assert_eq!(h.decision(works[2]), waits_for_a_slot());
    assert_eq!(h.d().held(works[2]), Some(Holder::Slot));
    assert_eq!(h.d().slots(), driver::Slots { slots: 2, taken: 2 });
    // The first writer's round rests at a call: its slot goes to the third work.
    asks_a_tool(&mut h, works[0]);
    assert_eq!(h.round(works[2]).1, "write");
    assert_eq!(h.d().slots().taken, 2);
}

#[test]
fn a_work_whose_round_ends_keeps_its_slot_for_the_round_it_begins_next() {
    let mut h = H::with_slots(flows(), 1);
    let (_, first) = h.add(going_item("first", &[], "plain"));
    let (_, second) = h.add(going_item("second", &[], "plain"));
    assert_eq!(h.decision(second), waits_for_a_slot());
    // What the writer hands in is sent back, and its next round begins at once, in the slot it held.
    let todos = h.returns(first, hands_in(&[("merged", None)]));
    assert_eq!(rounds(&todos), [(first, "write".to_string(), 2)]);
    assert_eq!(h.decision(second), waits_for_a_slot());
}

#[test]
fn a_slot_that_comes_free_goes_to_a_work_that_has_begun_before_one_that_has_not() {
    let mut h = H::with_slots(flows(), 1);
    let (_, one) = h.add(going_item("one", &[], "plain"));
    let (_, two) = h.add(new_item("two", &[], "plain"));
    let (_, three) = h.add(going_item("three", &[], "plain"));
    let one_call = asks_a_tool(&mut h, one);
    assert_eq!(
        h.round(three).1,
        "write",
        "the slot goes to the only work that waits for one"
    );
    let three_call = asks_a_tool(&mut h, three);
    h.answer(one, one_call);
    assert_eq!(h.round(one).2, 2, "the slot goes to the first work, whose tool is done");
    // Now the second work is let on, and waits; the third's tool is done, and it waits too.
    h.let_on(two);
    assert_eq!(h.decision(two), waits_for_a_slot());
    h.answer(three, three_call);
    assert_eq!(h.d().held(three), Some(Holder::Slot));
    // The slot the first work frees goes to the third, which has begun, before the second, which has not.
    let todos = h.returns(one, hands_in(&[("text", None)]));
    assert_eq!(
        rounds(&todos),
        [(one, "merge".to_string(), 1), (three, "write".to_string(), 2)],
        "the merge, an act of the engine's own, takes no slot"
    );
    assert_eq!(h.decision(two), waits_for_a_slot());
}

#[test]
fn the_pause_of_everything_holds_back_every_work_and_lifted_lets_on_all_but_those_paused_themselves() {
    let mut h = H::with_slots(flows(), 2);
    assert_eq!(h.pause_everything(true), []);
    assert!(h.d().everything_paused().is_some());
    let (_, going) = h.add(going_item("going", &[], "plain"));
    let (_, held) = h.add(new_item("held", &[], "plain"));
    assert_eq!(rounds(&h.d().pending()), []);
    assert_eq!(h.decision(going), Decision::Paused);
    assert_eq!(h.d().held(going), Some(Holder::Everything));
    assert_eq!(h.d().held(held), Some(Holder::Pause));
    // Set again, it changes nothing, and is kept once.
    h.arm(Plan::Never);
    assert_eq!(h.pause_everything(true), []);
    assert_eq!(h.ops(), []);
    let todos = h.pause_everything(false);
    assert_eq!(rounds(&todos), [(going, "write".to_string(), 1)]);
    assert_eq!(h.decision(held), Decision::Paused);
    assert!(h.d().everything_paused().is_none());
}

#[test]
fn a_round_under_way_goes_on_under_the_pause_of_everything_and_what_it_hands_in_is_taken() {
    let mut h = H::with_slots(flows(), 2);
    let (_, work) = h.add(going_item("work", &[], "judged"));
    h.pause_everything(true);
    let todos = h.hands_in(work, &[("text", None)]);
    assert_eq!(rounds(&todos), []);
    assert!(h.d().work(work).expect("the work").work.records.finished.len() == 1);
    assert_eq!(h.d().held(work), Some(Holder::Everything));
    assert_eq!(rounds(&h.pause_everything(false)), [(work, "judge".to_string(), 1)]);
}

#[test]
fn the_pause_of_everything_holds_across_a_stop_and_the_store_keeps_it() {
    let mut h = H::with_slots(flows(), 2);
    h.pause_everything(true);
    let (_, work) = h.add(going_item("work", &[], "plain"));
    assert_eq!(h.reopen(), []);
    assert!(h.d().everything_paused().is_some());
    assert_eq!(h.decision(work), Decision::Paused);
    let todos = h.pause_everything(false);
    assert_eq!(rounds(&todos), [(work, "write".to_string(), 1)]);
    h.reopen();
    assert!(h.d().everything_paused().is_none());
}

#[test]
fn opened_again_the_driver_gives_the_free_slots_in_its_order() {
    let mut h = H::with_slots(flows(), 1);
    h.pause_everything(true);
    let works: Vec<WorkNumber> = (0..3)
        .map(|i| h.add(going_item(&format!("{i}"), &[], "plain")).1)
        .collect();
    h.reopen();
    let todos = h.pause_everything(false);
    assert_eq!(rounds(&todos), [(works[0], "write".to_string(), 1)]);
    assert_eq!(h.decision(works[1]), waits_for_a_slot());
    assert_eq!(h.decision(works[2]), waits_for_a_slot());
}

#[test]
fn a_pause_of_everything_the_store_fails_to_keep_stops_the_driver_and_is_not_kept() {
    let mut h = H::with_slots(flows(), 2);
    h.arm(Plan::Fail {
        op: Some(Op::PauseEverything),
        n: 1,
        on: false,
    });
    let now = h.now();
    assert!(matches!(
        h.try_pause_everything(now, true),
        Outcome::Stopped { kept: false }
    ));
    h.reopen();
    assert!(h.d().everything_paused().is_none());
}
