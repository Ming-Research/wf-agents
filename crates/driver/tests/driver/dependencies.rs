//! Other works: an item waits before its first cell until every item it depends on has merged. That those items have
//! merged reaches the work as its dependencies block, which the driver writes as the item is added, in the very call in
//! which the last of them merges, and, on opening, wherever the block and what it stands for have come apart.

use driver::{MERGED, WAITING};
use store::Storage;

use super::*;

/// Takes the work through until its merge's round is under way.
fn to_merge(h: &mut H, work: WorkNumber) {
    for _ in 0..20 {
        let (_, cell, ..) = h.round(work);
        if cell == "merge" {
            return;
        }
        let kept = h.d().work(work).expect("the work is there");
        let came = proper(kept.flow, &cell);
        h.returns(work, came);
    }
    panic!("the work never reached its merge");
}

#[test]
fn an_item_waits_before_its_first_cell_until_the_one_it_depends_on_merges_and_starts_in_that_very_call() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "short"));
    let (_, wb) = h.add(new_item("b", &[a], "judged"));
    assert_eq!(h.dependencies(wb).as_deref(), Some(WAITING));
    assert_eq!(h.let_on(wb), vec![]);
    assert_eq!(h.decision(wb), waits_for_dependencies("write"));
    let todos = h.let_on(wa);
    assert_eq!(rounds(&todos), [(wa, "draft".to_string(), 1)]);
    to_merge(&mut h, wa);
    assert_eq!(h.decision(wb), waits_for_dependencies("write"));
    // What the merged work held is let go as it merges.
    let todos = h.hands_in(wa, &[("merged", None)]);
    assert_eq!(h.decision(wa), Decision::Ended(End::Merged));
    assert_eq!(rounds(&h.released(todos)), [(wb, "write".to_string(), 1)]);
    assert_eq!(h.dependencies(wb).as_deref(), Some(MERGED));
}

#[test]
fn a_waiting_work_not_yet_let_on_has_its_block_written_at_the_merge_and_begins_when_let_on() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "short"));
    let (_, wb) = h.add(new_item("b", &[a], "judged"));
    h.let_on(wa);
    let todos = h.through(wa);
    assert_eq!(todos, vec![], "a paused work was given something to do");
    assert_eq!(h.dependencies(wb).as_deref(), Some(MERGED));
    assert_eq!(h.decision(wb), Decision::Paused);
    let todos = h.let_on(wb);
    assert_eq!(rounds(&todos), [(wb, "write".to_string(), 1)]);
}

#[test]
fn a_chain_of_three_starts_each_as_the_one_before_merges() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "judged"));
    let (b, wb) = h.add(new_item("b", &[a], "short"));
    let (_, wc) = h.add(new_item("c", &[b], "judged"));
    for w in [wc, wb, wa] {
        h.let_on(w);
    }
    assert_eq!(h.decision(wb), waits_for_dependencies("draft"));
    assert_eq!(h.decision(wc), waits_for_dependencies("write"));
    let todos = h.through(wa);
    assert_eq!(rounds(&todos), [(wb, "draft".to_string(), 1)]);
    assert_eq!(h.decision(wc), waits_for_dependencies("write"));
    assert_eq!(h.dependencies(wc).as_deref(), Some(WAITING));
    let todos = h.through(wb);
    assert_eq!(rounds(&todos), [(wc, "write".to_string(), 1)]);
    let todos = h.through(wc);
    assert_eq!(todos, vec![]);
}

#[test]
fn an_item_depending_on_two_starts_only_once_both_have_merged() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "short"));
    let (b, wb) = h.add(new_item("b", &[], "plain"));
    let (_, wc) = h.add(new_item("c", &[a, b], "judged"));
    for w in [wa, wb, wc] {
        h.let_on(w);
    }
    let todos = h.through(wb);
    assert_eq!(todos, vec![]);
    assert_eq!(h.dependencies(wc).as_deref(), Some(WAITING));
    assert_eq!(h.decision(wc), waits_for_dependencies("write"));
    let todos = h.through(wa);
    assert_eq!(rounds(&todos), [(wc, "write".to_string(), 1)]);
    assert_eq!(h.dependencies(wc).as_deref(), Some(MERGED));
}

#[test]
fn a_cancelled_dependency_never_lets_the_item_start() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "short"));
    let (_, wb) = h.add(new_item("b", &[a], "judged"));
    h.let_on(wb);
    h.let_on(wa);
    let (run, ..) = h.round(wa);
    let todos = h.on(wa, Event::Cancelled);
    assert_eq!(
        todos,
        vec![Todo {
            work: wa,
            effect: Effect::Stop { run },
        }]
    );
    let spent = h.spent();
    let came = hands_in(&[("plan", None)]);
    let release = vec![Todo {
        work: wa,
        effect: Effect::Release { end: End::Cancelled },
    }];
    assert_eq!(h.on(wa, Event::Returned { run, came, spent }), release);
    assert_eq!(h.on(wa, Event::Released), vec![]);
    assert_eq!(h.decision(wa), Decision::Ended(End::Cancelled));
    assert_eq!(h.decision(wb), waits_for_dependencies("write"));
    assert_eq!(h.dependencies(wb).as_deref(), Some(WAITING));
    // Nothing is to be done for either, and opening again changes nothing of it.
    assert_eq!(h.d().pending(), vec![]);
    assert_eq!(h.reopen(), vec![]);
    assert_eq!(h.decision(wb), waits_for_dependencies("write"));
}

#[test]
fn an_item_added_after_its_dependency_merged_says_so_from_its_first_entry_and_starts_when_let_on() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "plain"));
    h.let_on(wa);
    h.through(wa);
    let (_, wb) = h.add(new_item("b", &[a], "short"));
    assert_eq!(h.dependencies(wb).as_deref(), Some(MERGED));
    let kept = h.d().work(wb).expect("the work is there");
    let writes: Vec<Option<&str>> = (kept.work.records.written.iter())
        .filter(|w| w.block == block("deps"))
        .map(|w| w.value.as_deref())
        .collect();
    assert_eq!(writes, [Some(MERGED)], "the block was written otherwise");
    let todos = h.let_on(wb);
    assert_eq!(rounds(&todos), [(wb, "draft".to_string(), 1)]);
}

/// Two items, the second depending on the first and let on; the first's merge round under way, its end to be told to
/// a driver whose store fails as the plan says.
fn merging(plan: Plan) -> (H, WorkNumber, WorkNumber, Outcome<Vec<Todo>>) {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "short"));
    let (_, wb) = h.add(new_item("b", &[a], "judged"));
    h.let_on(wb);
    h.let_on(wa);
    to_merge(&mut h, wa);
    let (run, ..) = h.round(wa);
    h.arm(plan);
    let now = h.now();
    let spent = h.spent();
    let came = hands_in(&[("merged", None)]);
    let outcome = h.try_on(now, wa, Event::Returned { run, came, spent });
    (h, wa, wb, outcome)
}

#[test]
fn opened_after_a_failure_between_the_merge_being_kept_and_the_dependents_block_the_block_is_brought_to_agree() {
    let plan = Plan::Fail {
        op: Some(Op::Save),
        n: 2,
        on: false,
    };
    let (mut h, wa, wb, outcome) = merging(plan);
    assert!(h.failed(), "the second save never came");
    assert!(
        matches!(outcome, Outcome::Stopped { kept: true }),
        "the merge was not said to be kept"
    );
    let todos = h.restart();
    assert_eq!(h.decision(wa), Decision::Ended(End::Merged));
    assert_eq!(h.dependencies(wb).as_deref(), Some(MERGED));
    assert_eq!(rounds(&h.released(todos)), [(wb, "write".to_string(), 1)]);
}

#[test]
fn opened_after_a_stop_right_after_the_merge_was_kept_the_block_is_brought_to_agree() {
    let (mut h, wa, wb, outcome) = merging(Plan::DieAfter { n: 1 });
    assert!(h.dead());
    assert!(
        matches!(outcome, Outcome::Stopped { kept: true }),
        "the merge was not said to be kept"
    );
    let todos = h.restart();
    assert_eq!(h.decision(wa), Decision::Ended(End::Merged));
    assert_eq!(h.dependencies(wb).as_deref(), Some(MERGED));
    assert_eq!(rounds(&h.released(todos)), [(wb, "write".to_string(), 1)]);
}

#[test]
fn a_failure_of_the_merge_itself_keeps_nothing_and_it_is_told_again() {
    let plan = Plan::Fail {
        op: Some(Op::Save),
        n: 1,
        on: false,
    };
    let (mut h, wa, wb, outcome) = merging(plan);
    assert!(
        matches!(outcome, Outcome::Stopped { kept: false }),
        "the failed merge was said to be kept"
    );
    let todos = h.restart();
    assert_eq!(rounds(&todos), [(wa, "merge".to_string(), 1)]);
    assert_eq!(h.dependencies(wb).as_deref(), Some(WAITING));
    let todos = h.hands_in(wa, &[("merged", None)]);
    assert_eq!(rounds(&h.released(todos)), [(wb, "write".to_string(), 1)]);
}

/// Every version of the work's dependencies block: its value and its text.
fn versions_of(h: &H, work: WorkNumber) -> Vec<(Option<String>, String)> {
    let kept = h.d().work(work).expect("the work is there");
    let versions = decide::versions(&kept.work.records);
    (versions.get(&block("deps")).into_iter().flatten())
        .map(|v| {
            let text = kept
                .work
                .contents
                .get(&(block("deps"), v.at))
                .cloned()
                .unwrap_or_default();
            (v.value.clone(), text)
        })
        .collect()
}

fn version(value: &str, text: &str) -> (Option<String>, String) {
    (Some(value.to_string()), text.to_string())
}

#[test]
fn the_blocks_text_says_which_items_are_still_waited_for_and_which_were_cancelled() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "plain"));
    let (b, wb) = h.add(new_item("b", &[], "short"));
    let (_, wc) = h.add(new_item("c", &[a, b], "judged"));
    h.let_on(wc);
    h.let_on(wa);
    h.let_on(wb);
    // One of the two merges: the block still says waiting, now for the other alone, and the work does not start.
    let todos = h.through(wa);
    assert_eq!(todos, vec![]);
    assert_eq!(h.decision(wc), waits_for_dependencies("write"));
    // The other is cancelled: the block says waiting still, and which was cancelled; the work waits for good.
    let (run, ..) = h.round(wb);
    let todos = h.on(wb, Event::Cancelled);
    assert_eq!(
        todos,
        vec![Todo {
            work: wb,
            effect: Effect::Stop { run },
        }]
    );
    assert_eq!(h.decision(wc), waits_for_dependencies("write"));
    assert_eq!(
        versions_of(&h, wc),
        [
            version(WAITING, "Waits for the items 1, 2 to merge."),
            version(WAITING, "Waits for the items 2 to merge."),
            version(WAITING, "Waits for the items 2 (cancelled) to merge."),
        ]
    );
    let kept = h.d().work(wc).expect("the work is there");
    assert!(kept.work.records.started.is_empty(), "the work started");
    // Opening again writes nothing more: the block says what it stands for.
    h.reopen();
    assert_eq!(versions_of(&h, wc).len(), 3);
}

#[test]
fn a_cancelled_dependency_is_named_beside_one_still_going() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "plain"));
    let (b, _) = h.add(new_item("b", &[], "short"));
    let (_, wc) = h.add(new_item("c", &[b, a], "short"));
    h.on(wa, Event::Cancelled);
    assert_eq!(
        versions_of(&h, wc),
        [
            version(WAITING, "Waits for the items 1, 2 to merge."),
            version(WAITING, "Waits for the items 1 (cancelled), 2 to merge."),
        ]
    );
    assert_eq!(h.let_on(wc), vec![]);
    assert_eq!(h.decision(wc), waits_for_dependencies("draft"));
}

#[test]
fn a_work_whose_flow_has_no_dependencies_block_waits_for_no_item() {
    let mut h = H::new();
    let (a, _) = h.add(new_item("a", &[], "judged"));
    let (_, wb) = h.add(new_item("b", &[a], "plain"));
    let todos = h.let_on(wb);
    assert_eq!(rounds(&todos), [(wb, "write".to_string(), 1)]);
}

#[test]
fn a_block_left_to_bring_to_agree_on_a_store_that_fails_as_it_is_written_stops_the_opening() {
    let plan = Plan::Fail {
        op: Some(Op::Save),
        n: 2,
        on: false,
    };
    let (mut h, _, wb, outcome) = merging(plan);
    assert!(matches!(outcome, Outcome::Stopped { kept: true }));
    // The first save as the driver opens is the write of the block, and it fails.
    let now = Time(h.clock);
    let failing = Plan::Fail {
        op: Some(Op::Save),
        n: 1,
        on: false,
    };
    let (opened, calls) = open_with_plan(&h.db, failing, now);
    assert!(
        calls.lock().expect("the calls are noted").failed,
        "the opening made no save"
    );
    assert_eq!(opened.err(), Some(Fault::Store(failure())));
    assert_eq!(
        h.reader.load(wb).expect("the work loads").records.written.len(),
        1,
        "a failed write was kept"
    );
    // Opened on a store that works, and the pause of everything it made as it opened lifted at once by the owner, it
    // ends as the same history with no failure: each entry added at the same time, the moments that the pause put
    // apart aside, and the pause and its lifting kept beside.
    h.restart();
    let (straight, _, _, outcome) = merging(Plan::Never);
    assert!(matches!(outcome, Outcome::Done(_)));
    h.check();
    straight.check();
    let by_time_alone = |mut state: State| {
        state.works = (state.works.into_iter())
            .map(|(head, work)| (head, by_time(&work)))
            .collect();
        state.file.remove("moments");
        state.file.remove("everything_paused");
        state
    };
    assert_eq!(
        by_time_alone(h.state()),
        by_time_alone(straight.state()),
        "opened after the failure, the driver holds otherwise"
    );
}

#[test]
fn a_store_that_fails_at_the_second_dependents_write_keeps_the_merge_and_opening_brings_both_to_agree() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "short"));
    let (_, wb) = h.add(new_item("b", &[a], "judged"));
    let (_, wc) = h.add(new_item("c", &[a], "short"));
    h.let_on(wb);
    h.let_on(wc);
    h.let_on(wa);
    to_merge(&mut h, wa);
    let (run, ..) = h.round(wa);
    // The first save keeps the merge, the second the first dependent's block, the third fails.
    h.arm(Plan::Fail {
        op: Some(Op::Save),
        n: 3,
        on: false,
    });
    let now = h.now();
    let spent = h.spent();
    let came = hands_in(&[("merged", None)]);
    let outcome = h.try_on(now, wa, Event::Returned { run, came, spent });
    assert!(h.failed(), "the third save never came");
    assert!(
        matches!(outcome, Outcome::Stopped { kept: true }),
        "the merge was not said to be kept"
    );
    // A dependent is written given no slot, the free slots going out once every dependent is written: the first
    // dependent's write was kept, and no round of either began.
    assert_eq!(h.reader.load(wb).expect("the work loads").records.started.len(), 0);
    assert_eq!(h.reader.load(wc).expect("the work loads").records.started.len(), 0);
    let todos = h.restart();
    assert_eq!(h.dependencies(wb).as_deref(), Some(MERGED));
    assert_eq!(h.dependencies(wc).as_deref(), Some(MERGED));
    // Opened again, the driver writes the second dependent's block, and gives the free slots in its order.
    assert_eq!(
        rounds(&h.released(todos)),
        [(wb, "write".to_string(), 1), (wc, "draft".to_string(), 1)]
    );
}

#[test]
fn a_person_may_not_write_the_dependencies_block_and_the_outside_may_as_things_stand() {
    let mut h = H::new();
    let (a, _) = h.add(new_item("a", &[], "short"));
    let (_, wb) = h.add(new_item("b", &[a], "judged"));
    h.let_on(wb);
    let why = h.refused(|d, now| {
        d.happened(now, wb, wrote("deps", Outsider::Person, Some(MERGED)))
            .map(drop)
    });
    assert_eq!(
        why,
        Failed::Refused(engine::Refused::NotTheirs {
            block: block("deps"),
            by: Outsider::Person,
        })
    );
    // A hole, stated as it is: a write by the outside does not say which source it is from, so the engine's loop
    // takes the outside's write of the block the driver alone is to write, and the work starts though the item it
    // depends on has not merged. This test is to change when the loop refuses that write.
    let now = h.now();
    let written = h.dm().happened(now, wb, wrote("deps", Outsider::Outside, Some(MERGED)));
    let todos = written.expect("the outside's write is taken");
    assert_eq!(rounds(&todos), [(wb, "write".to_string(), 1)]);
    assert_eq!(h.dependencies(wb).as_deref(), Some(MERGED));
}
