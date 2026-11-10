//! Other works: an item waits before its first cell until every item it depends on has merged. That those items have
//! merged reaches the work as its dependencies block, which the driver writes as the item is added, in the very call in
//! which the last of them merges, and, on opening, wherever the block and what it stands for have come apart.

use driver::{MERGED, WAITING};

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
    let todos = h.hands_in(wa, &[("merged", None)]);
    assert_eq!(h.decision(wa), Decision::Ended(End::Merged));
    assert_eq!(rounds(&todos), [(wb, "write".to_string(), 1)]);
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
    assert_eq!(h.on(wa, Event::Returned { run, came, spent }), vec![]);
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
    let todos = h.reopen();
    assert_eq!(h.decision(wa), Decision::Ended(End::Merged));
    assert_eq!(h.dependencies(wb).as_deref(), Some(MERGED));
    assert_eq!(rounds(&todos), [(wb, "write".to_string(), 1)]);
}

#[test]
fn opened_after_a_stop_right_after_the_merge_was_kept_the_block_is_brought_to_agree() {
    let (mut h, wa, wb, outcome) = merging(Plan::DieAfter { n: 1 });
    assert!(h.dead());
    assert!(
        matches!(outcome, Outcome::Stopped { kept: true }),
        "the merge was not said to be kept"
    );
    let todos = h.reopen();
    assert_eq!(h.decision(wa), Decision::Ended(End::Merged));
    assert_eq!(h.dependencies(wb).as_deref(), Some(MERGED));
    assert_eq!(rounds(&todos), [(wb, "write".to_string(), 1)]);
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
    let todos = h.reopen();
    assert_eq!(rounds(&todos), [(wa, "merge".to_string(), 1)]);
    assert_eq!(h.dependencies(wb).as_deref(), Some(WAITING));
    let todos = h.hands_in(wa, &[("merged", None)]);
    assert_eq!(rounds(&todos), [(wb, "write".to_string(), 1)]);
}
