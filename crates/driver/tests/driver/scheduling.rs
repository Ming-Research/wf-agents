//! The driver as the scheduler: a work is added paused and goes on once the owner lets it on; no more rounds of
//! agents go on at once than there are slots, a work whose round ends keeping its slot for its next round, and a slot
//! that comes free going to the works that wait for one in the engine's order; the slots the owner sets take effect at
//! once, those added given away as they are and none begun while more are under way than the slots allow; and the
//! owner's pause of everything, kept in the store, holds back what each work's own pause would until it is lifted.

use decide::Holder;
use store::Storage;

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
    let todos = h.returns(work, Came::asks(vec![tool("ci")]));
    (todos.iter())
        .find_map(|todo| match &todo.effect {
            Effect::Call { call, .. } if todo.work == work => Some(*call),
            _ => None,
        })
        .expect("the tool is called")
}

#[test]
fn a_work_is_added_paused_and_begins_once_the_owner_lets_it_on() {
    let mut h = H::with_slots(flows(), 2);
    let (_, going) = h.add(new_item("going", &[], "plain"));
    assert_eq!(rounds(&h.d().pending()), []);
    // Nothing runs its first cell until the owner starts it, as the owner's letting it on does: no round is held back.
    assert_eq!(h.d().held(going), None);
    assert_eq!(rounds(&h.let_on(going)), [(going, "write".to_string(), 1)]);
    assert_eq!(rounds(&h.d().pending()), [(going, "write".to_string(), 1)]);
    let (_, held) = h.add(new_item("held", &[], "plain"));
    // Not started by the owner, it has nothing new to run its first cell.
    assert!(
        matches!(h.decision(held), Decision::Stalled { .. }),
        "{:?}",
        h.decision(held)
    );
    assert_eq!(h.d().held(held), None);
    assert_eq!(rounds(&h.let_on(held)), [(held, "write".to_string(), 1)]);
}

#[test]
fn a_work_let_on_still_waits_for_the_items_its_item_depends_on() {
    let mut h = H::with_slots(flows(), 2);
    let (a, _) = h.add(new_item("a", &[], "short"));
    let (_, b) = h.add_let_on(new_item("b", &[a], "short"));
    assert_eq!(h.decision(b), waits_for_dependencies("draft"));
    assert_eq!(rounds(&h.d().pending()), []);
}

#[test]
fn no_more_rounds_of_agents_go_on_than_there_are_slots_and_the_others_wait_for_one() {
    let mut h = H::with_slots(flows(), 2);
    let works: Vec<WorkNumber> = (0..3)
        .map(|i| h.add_let_on(new_item(&format!("{i}"), &[], "plain")).1)
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
    let (_, first) = h.add_let_on(new_item("first", &[], "plain"));
    let (_, second) = h.add_let_on(new_item("second", &[], "plain"));
    // The first work's round rests at a call, and the second takes the slot; the first's tool is done, and it waits.
    let call = asks_a_tool(&mut h, first);
    assert_eq!(h.round(second).1, "write");
    h.answer(first, call);
    assert_eq!(h.d().held(first), Some(Holder::Slot));
    // What the second hands in is sent back, and its next round begins at once in the slot it held, though a work
    // that began before it, and comes before it by number, waits for one.
    let todos = h.returns(second, hands_in(&[("merged", None)]));
    assert_eq!(rounds(&todos), [(second, "write".to_string(), 2)]);
    assert_eq!(h.d().held(first), Some(Holder::Slot));
}

#[test]
fn a_slot_that_comes_free_goes_to_a_work_that_has_begun_before_one_that_has_not() {
    let mut h = H::with_slots(flows(), 1);
    let (_, one) = h.add_let_on(new_item("one", &[], "plain"));
    let (_, two) = h.add(new_item("two", &[], "plain"));
    let (_, three) = h.add_let_on(new_item("three", &[], "plain"));
    let one_call = asks_a_tool(&mut h, one);
    assert_eq!(
        h.round(three).1,
        "write",
        "the slot goes to the only work that waits for one"
    );
    let three_call = asks_a_tool(&mut h, three);
    h.answer(one, one_call);
    assert_eq!(h.round(one).2, 1, "the slot goes to the first work, whose tool is done");
    // Now the second work is let on, and waits; the third's tool is done, and it waits too.
    h.let_on(two);
    assert_eq!(h.decision(two), waits_for_a_slot());
    h.answer(three, three_call);
    assert_eq!(h.d().held(three), Some(Holder::Slot));
    // The slot the first work frees goes to the third, which has begun, before the second, which has not.
    let todos = h.returns(one, hands_in(&[("text", None)]));
    assert_eq!(
        rounds(&todos),
        [(one, "merge".to_string(), 1), (three, "write".to_string(), 1)],
        "the merge, an act of the engine's own, takes no slot"
    );
    assert_eq!(h.decision(two), waits_for_a_slot());
}

#[test]
fn slots_added_go_at_once_to_the_works_that_wait_for_one_in_the_engines_order() {
    let mut h = H::with_slots(flows(), 1);
    let (_, one) = h.add_let_on(new_item("one", &[], "plain"));
    let (_, two) = h.add(new_item("two", &[], "plain"));
    let (_, three) = h.add_let_on(new_item("three", &[], "plain"));
    let one_call = asks_a_tool(&mut h, one);
    assert_eq!(h.round(three).1, "write");
    h.answer(one, one_call);
    let three_call = asks_a_tool(&mut h, three);
    assert_eq!(h.round(one).1, "write");
    // The second work, let on, has not begun; the third, whose tool is done, has; both wait for the one slot.
    h.let_on(two);
    h.answer(three, three_call);
    assert_eq!(h.d().held(two), Some(Holder::Slot));
    assert_eq!(h.d().held(three), Some(Holder::Slot));
    // A second slot goes at once to the third, which has begun, before the second; a third slot to the second.
    assert_eq!(rounds(&h.set_slots(2)), [(three, "write".to_string(), 1)]);
    assert_eq!(h.decision(two), waits_for_a_slot());
    assert_eq!(rounds(&h.set_slots(3)), [(two, "write".to_string(), 1)]);
    assert_eq!(h.d().slots(), driver::Slots { slots: 3, taken: 3 });
    // Set as they stand, they change nothing, and the store is not asked.
    h.arm(Plan::Never);
    assert_eq!(h.set_slots(3), []);
    assert_eq!(h.ops(), []);
}

// The slots are kept as the driver is opened with them and each time the owner sets them, and again as they stand no
// more than the engine's own store is asked: what the rounds taken are read against, and what no step reads. A driver
// opened with the slots the store last kept keeps none more; one opened with others, as the owner changed them while it
// was stopped, keeps those. Fails where the slots are kept more than once as they stand, not at all where they are set,
// or not at an opening.
#[test]
fn the_slots_are_kept_as_the_driver_opens_with_them_and_each_time_they_are_set_and_not_again_as_they_stand() {
    let mut h = H::with_slots(flows(), 2);
    let kept = |h: &H| -> Vec<(usize, u64)> {
        (h.d().slots_history().iter())
            .map(|record| (record.slots, record.at.0))
            .collect()
    };
    let begun = kept(&h);
    assert_eq!(
        begun.iter().map(|k| k.0).collect::<Vec<_>>(),
        [2],
        "the slots it opened with"
    );
    // Set as they stand, nothing is kept again; set to others, they are kept at the time they were set.
    h.set_slots(2);
    assert_eq!(kept(&h), begun);
    h.set_slots(5);
    let set = kept(&h);
    assert_eq!(set.iter().map(|k| k.0).collect::<Vec<_>>(), [2, 5]);
    assert!(set[1].1 > set[0].1, "kept at the time they were set: {set:?}");
    assert_eq!(
        h.reader.slots_set().expect("the slots are read").len(),
        2,
        "the store holds what the driver says"
    );
    // Opened again with the slots last kept, it keeps none more; with others, it keeps them.
    h.restart();
    assert_eq!(kept(&h), set);
    h.slots = 3;
    h.restart();
    assert_eq!(
        kept(&h).iter().map(|k| k.0).collect::<Vec<_>>(),
        [2, 5, 3],
        "the slots the owner changed while the engine was stopped"
    );
}

// The invariant the time a work waited for a slot is read by: no work waits for a slot while one is free (module
// `driver`). A work is let on while the one slot is taken, and from then until the first work's round ends the walk,
// asked in a room with every slot free, says it would begin a round (`Decision::Run`), which the driver has not begun;
// once the slot is free the round begins at once. Fails where a free slot is left to a work that waits for one.
#[test]
fn no_work_waits_for_a_slot_while_one_is_free_which_is_what_a_wait_for_a_slot_is_read_by() {
    let mut h = H::with_slots(flows(), 1);
    let (_, one) = h.add_let_on(new_item("one", &[], "plain"));
    let (_, two) = h.add_let_on(new_item("two", &[], "plain"));
    // The one slot is taken by the first work's round: the second would run its writer, and does not.
    assert_eq!(h.d().slots(), driver::Slots { slots: 1, taken: 1 });
    assert_eq!(h.d().held(two), Some(Holder::Slot));
    let kept = h.d().work(two).expect("the work is there");
    let would = decide::decide_in(&kept.flow.flow, &kept.work.records, decide::Room::FREE);
    assert!(matches!(would, Decision::Run { .. }), "{would:?}");
    // The first work hands in, its next cell an act that takes no slot, and the slot goes to the second at once.
    let todos = h.returns(one, hands_in(&[("text", None)]));
    assert!(
        rounds(&todos).contains(&(two, "write".to_string(), 1)),
        "the freed slot goes to the work that waited for it: {todos:?}"
    );
    assert_eq!(h.d().held(two), None);
}

// What a work's time went to, read as it stands: the time its writer's tool took is the tools', and from when the tool
// was answered, with the one slot taken by the other work's round, to when its next round began, the work waited for a
// slot. A slot the other work's round took is no wait of the first work's before it has a round, nor of any other kind
// than the slot's. Fails where the wait for a slot is read as anything else, or the tool's time is.
#[test]
fn the_time_a_work_waited_for_a_slot_is_what_its_time_went_to_as_a_wait_for_one() {
    let mut h = H::with_slots(flows(), 1);
    let (_, one) = h.add_let_on(new_item("one", &[], "plain"));
    let call = asks_a_tool(&mut h, one);
    let (_, two) = h.add_let_on(new_item("two", &[], "plain"));
    assert_eq!(h.round(two).1, "write", "the slot the tool left goes to the other work");
    h.answer(one, call);
    let answered = Time(h.clock);
    assert_eq!(h.d().held(one), Some(Holder::Slot));
    h.returns(two, hands_in(&[("text", None)]));
    let began = Time(h.clock);
    assert_eq!(
        h.round(one).1,
        "write",
        "the slot the other work's round freed goes to the first"
    );
    let d = h.d();
    let kept = d.work(one).expect("the work is there");
    let at_once = d.at_once();
    let stretches = driver::time::stretches(
        &kept,
        d.everything_spans(),
        &d.stop_spans(),
        &d.cannot_runs(&at_once),
        &d.downs(&at_once),
        Vec::new(),
        Time(h.clock),
    );
    let of = |went: work::Went| -> Vec<(Time, Time)> {
        (stretches.iter())
            .filter(|s| s.went == went)
            .map(|s| (s.from, s.until))
            .collect()
    };
    assert_eq!(of(work::Went::Slot), [(answered, began)]);
}

#[test]
fn with_fewer_slots_no_round_under_way_is_stopped_and_none_begins_until_fewer_are_under_way() {
    let mut h = H::with_slots(flows(), 3);
    let works: Vec<WorkNumber> = (0..4)
        .map(|i| h.add_let_on(new_item(&format!("{i}"), &[], "plain")).1)
        .collect();
    let &[a, b, c, d] = works.as_slice() else {
        panic!("four works were added: {works:?}")
    };
    assert_eq!(h.decision(d), waits_for_a_slot());
    // Three rounds go on under one slot: none is stopped, and the driver says three are taken of one.
    assert_eq!(h.set_slots(1), []);
    assert_eq!(h.d().slots(), driver::Slots { slots: 1, taken: 3 });
    assert_eq!(rounds(&h.d().pending()).len(), 3);
    // One rests at a call and two go on: nothing begins.
    let call = asks_a_tool(&mut h, a);
    assert_eq!(h.d().slots().taken, 2);
    assert_eq!(h.decision(d), waits_for_a_slot());
    // What one of those two hands in is sent back, and its next round waits while the other goes on, as does the first
    // work's once its tool is done: a work whose round ends keeps its slot only where the others leave one free.
    assert_eq!(rounds(&h.returns(b, hands_in(&[("merged", None)]))), []);
    assert_eq!(h.d().held(b), Some(Holder::Slot));
    assert_eq!(rounds(&h.answer(a, call)), []);
    assert_eq!(h.d().held(a), Some(Holder::Slot));
    // The last round under way rests at a call: the one slot goes to the first work, in the engine's order.
    asks_a_tool(&mut h, c);
    assert_eq!(h.round(a).1, "write");
    assert_eq!(h.d().slots(), driver::Slots { slots: 1, taken: 1 });
    assert_eq!(h.d().held(b), Some(Holder::Slot));
    assert_eq!(h.decision(d), waits_for_a_slot());
}

/// A flow whose first cell is the engine's own act, which takes no slot: it makes the place the writer, an agent,
/// writes in, once the items the work's item depends on have merged; then the merge.
fn started() -> flow::Compiled {
    let flow = r#"
        name = "started"
        order = ["start", "write", "merge"]
        [limits]
        runs = 2
        returns = 2
        calls = 3
        [lines.main]
        [blocks.request]
        people = true
        [blocks.place]
        [blocks.text]
        [blocks.merged]
        shown = true
        [blocks.deps]
        values = ["waiting", "merged"]
        outside = "dependencies"
        writes = { merged = "Every item this one depends on has merged.", waiting = "Waits for the items {items} to merge.", between = ", " }
        [blocks.start]
        outside = "start"
        writes = { text = "The owner started the work." }
        [tests.ready]
        says = "Every item this one depends on has merged"
        kind = "all"
        reads = ["deps"]
        is = "merged"
    "#;
    let start = r#"
        title = "Start"
        kind = "engine"
        act = "start"
        watches = ["start"]
        requires = { test = "ready" }
        outputs = ["place"]
    "#;
    let write = r#"
        title = "Write"
        kind = "agent"
        line = "main"
        task = "Write the text the request asks for."
        effort = "high"
        watches = ["place", "request"]
        outputs = ["text"]

        [kinds]
        place = "last_and_latest"
        request = "last_and_latest"
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        act = "merge"
        watches = ["text"]
        outputs = ["merged"]
    "#;
    compiled(flow, &[("start", start), ("write", write), ("merge", merge)])
}

/// The work's act that makes its place hands it in.
fn started_place(h: &mut H, work: WorkNumber) {
    assert_eq!(h.round(work).1, "start");
    h.hands_in(work, &[("place", None)]);
}

#[test]
fn what_the_driver_writes_as_the_owner_starts_a_work_is_the_flow_s_words() {
    let mut h = H::with_slots(vec![started()], 1);
    let (_, work) = h.add_let_on(new_item("one", &[], "started"));
    let kept = h.d().work(work).expect("the work is there").work;
    let versions = decide::versions(&kept.records);
    let start = decide::BlockId::from("start");
    let at = (versions.get(&start).and_then(|v| v.last()))
        .expect("the start is written")
        .at;
    assert_eq!(
        kept.contents.get(&(start, at)).map(String::as_str),
        Some("The owner started the work.")
    );
}

#[test]
fn a_work_that_only_ran_the_engines_own_act_has_not_begun_and_waits_behind_one_whose_agent_did() {
    let mut h = H::with_slots(vec![started()], 1);
    let (_, one) = h.add(new_item("one", &[], "started"));
    let (_, two) = h.add_let_on(new_item("two", &[], "started"));
    started_place(&mut h, two);
    let (_, three) = h.add_let_on(new_item("three", &[], "started"));
    started_place(&mut h, three);
    assert_eq!(h.d().held(three), Some(Holder::Slot));
    let call = asks_a_tool(&mut h, two);
    assert_eq!(h.round(three).1, "write");
    // The first work is let on midway: its act runs at once, and it waits for a slot, with the second.
    h.let_on(one);
    started_place(&mut h, one);
    h.answer(two, call);
    assert_eq!(h.d().held(one), Some(Holder::Slot));
    assert_eq!(h.d().held(two), Some(Holder::Slot));
    // The slot the third frees goes to the second, whose agent began, before the first, which ran only the act.
    let todos = h.hands_in(three, &[("text", None)]);
    assert_eq!(
        rounds(&todos),
        [(three, "merge".to_string(), 1), (two, "write".to_string(), 1)]
    );
    assert_eq!(h.d().held(one), Some(Holder::Slot));
}

#[test]
fn a_work_let_on_begins_on_what_people_wrote_for_it_as_it_was_added() {
    let mut h = H::with_slots(flows(), 2);
    let mut item = new_item("work", &[], "short");
    item.writes = vec![first_write("request", None)];
    let (_, work) = h.add_let_on(item);
    let kept = h.d().work(work).expect("the work is there").work;
    assert_eq!(
        kept.records.abandoned,
        [],
        "a run was begun before the first writes were in"
    );
    assert_eq!(kept.records.started.len(), 1);
    let request = kept
        .records
        .written
        .iter()
        .find(|w| w.block.as_str() == "request")
        .expect("the request");
    assert_eq!(kept.records.started[0].basis[&block("request")], Some(request.at));
    assert_eq!(rounds(&h.d().pending()), [(work, "draft".to_string(), 1)]);
}

#[test]
fn the_pause_of_everything_holds_back_every_work_and_lifted_lets_on_all_but_those_paused_themselves() {
    let mut h = H::with_slots(flows(), 2);
    assert_eq!(h.pause_everything(true), []);
    assert!(h.d().everything_paused().is_some());
    let (_, going) = h.add_let_on(new_item("going", &[], "plain"));
    let (_, held) = h.add(new_item("held", &[], "plain"));
    assert_eq!(rounds(&h.d().pending()), []);
    assert!(matches!(h.decision(going), Decision::Paused { .. }));
    assert_eq!(h.d().held(going), Some(Holder::Everything));
    assert_eq!(h.d().held(held), None, "never started, it would begin no round");
    // Set again, it changes nothing, and is kept once.
    h.arm(Plan::Never);
    assert_eq!(h.pause_everything(true), []);
    assert_eq!(h.ops(), []);
    let todos = h.pause_everything(false);
    assert_eq!(rounds(&todos), [(going, "write".to_string(), 1)]);
    assert!(
        matches!(h.decision(held), Decision::Stalled { .. }),
        "{:?}",
        h.decision(held)
    );
    assert!(h.d().everything_paused().is_none());
}

#[test]
fn a_round_under_way_goes_on_under_the_pause_of_everything_and_what_it_hands_in_is_taken() {
    let mut h = H::with_slots(flows(), 2);
    let (_, work) = h.add_let_on(new_item("work", &[], "judged"));
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
    let paused = h.d().everything_paused();
    let (_, work) = h.add_let_on(new_item("work", &[], "plain"));
    assert_eq!(h.reopen(), []);
    assert_eq!(h.d().everything_paused(), paused, "the owner's pause is kept as it was");
    assert!(matches!(h.decision(work), Decision::Paused { .. }));
    let todos = h.pause_everything(false);
    assert_eq!(rounds(&todos), [(work, "write".to_string(), 1)]);
    // Opened again with no work going on, the driver pauses nothing.
    h.on(
        work,
        Event::Paused {
            by: Pauser::Owner,
            why: None,
            waits_for: Vec::new(),
        },
    );
    h.reopen();
    assert_eq!(h.d().everything_paused(), None);
}

#[test]
fn opened_again_the_driver_gives_the_free_slots_in_its_order() {
    let mut h = H::with_slots(flows(), 1);
    h.pause_everything(true);
    let works: Vec<WorkNumber> = (0..3)
        .map(|i| h.add_let_on(new_item(&format!("{i}"), &[], "plain")).1)
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
