//! A work that can go on by itself no further: it waits only for what cells write, and none of them will run. The walk
//! says where it stands, what it waits for, and why each cell that writes it does not run.

use decide::{Test, TestKind, TestName};

use super::*;

/// A cell that waits, by the named test of this name, until this block is yes; and the test.
fn waiting_for(test: &str, block: &str, c: Cell) -> (Cell, (TestName, Test)) {
    let named = Test {
        kind: TestKind::All,
        reads: vec![block.into()],
        is: "yes".to_string(),
    };
    (requiring(test, c), (test.into(), named))
}

/// The research, which says whether a deeper look is needed and writes the plan; the implementation of the plan where
/// it is not; the check of the code, which alone writes whether it passed; and the merge, which waits for that.
fn research_to_merge() -> World {
    let (merge, named) = waiting_for("checked", "checked", cell("merge", &["checked"], &[]));
    let cells = vec![
        cell("research", &["background"], &["deep", "plan"]),
        when("deep", "no", cell("implement", &["plan"], &["code"])),
        cell("check", &["code"], &["checked"]),
        merge,
    ];
    let mut w = World::with_tests(cells, BTreeMap::from([named]));
    w.written_by_cells("checked");
    w.person_writes("background");
    w
}

#[test]
fn a_research_that_hands_in_its_choice_alone_stalls_the_work_at_the_merge_for_want_of_the_check() {
    let mut w = research_to_merge();
    w.run("research", &[("deep", Value("no"))]);
    // No plan: the implementation has nothing new, so neither has the check, and only the check makes the merge's
    // precondition hold.
    assert_eq!(
        w.decide(),
        stalled("merge", &["checked"], &[("check", Unrun::NothingNew)])
    );
    // The owner writes what runs the research again, and the work goes on.
    w.person_writes("background");
    assert!(matches!(w.decide(), Decision::Run { cell, .. } if cell.as_str() == "research"));
}

#[test]
fn an_implementation_that_hands_in_nothing_stalls_the_work_at_the_merge() {
    let mut w = research_to_merge();
    w.run("research", &[("deep", Value("no")), ("plan", Matters)]);
    w.run("implement", &[]);
    assert_eq!(
        w.decide(),
        stalled("merge", &["checked"], &[("check", Unrun::NothingNew)])
    );
}

#[test]
fn a_cell_that_writes_what_the_work_waits_for_and_is_skipped_says_which_key_of_its_condition_does_not_hold() {
    let (merge, named) = waiting_for("ok", "ok", cell("merge", &["ok"], &[]));
    let cells = vec![
        cell("ask", &["topic"], &["more"]),
        when("more", "yes", cell("judge", &["topic"], &["ok"])),
        merge,
    ];
    let mut w = World::with_tests(cells, BTreeMap::from([named]));
    w.written_by_cells("ok");
    w.person_writes("topic");
    w.run("ask", &[("more", Value("no"))]);
    let skipped = Unrun::Skipped {
        on: vec![Condition::Is {
            block: "more".into(),
            value: "yes".into(),
        }],
    };
    assert_eq!(w.decide(), stalled("merge", &["ok"], &[("judge", skipped)]));
}

#[test]
fn a_cell_after_where_the_work_stands_that_writes_what_it_waits_for_is_held_there() {
    let (gated, named) = waiting_for("ok", "ok", cell("gated", &["ok"], &["done"]));
    let cells = vec![
        cell("first", &["brief"], &["ok"]),
        gated,
        cell("later", &["done"], &["ok"]),
        cell("merge", &["done"], &[]),
    ];
    let mut w = World::with_tests(cells, BTreeMap::from([named]));
    w.written_by_cells("ok");
    w.person_writes("brief");
    w.run("first", &[("ok", Value("no"))]);
    assert_eq!(
        w.decide(),
        stalled(
            "gated",
            &["ok"],
            &[("first", Unrun::NothingNew), ("later", Unrun::Held)]
        )
    );
}

#[test]
fn a_wait_on_what_only_a_cell_writes_beside_a_call_for_a_person_is_no_stall_until_the_call_has_its_result() {
    let (gated, named) = waiting_for("ok", "ok", cell("gated", &["ok"], &[]));
    let mut w = World::with_tests(vec![cell("first", &["brief"], &["ok"]), gated], BTreeMap::from([named]));
    w.written_by_cells("ok");
    w.person_writes("brief");
    w.run("first", &[("ok", Value("yes"))]);
    // The gated cell starts while its precondition holds, and its run rests at a question for the owner; then the
    // first cell runs again and says no.
    w.start("gated");
    let call = w.calls("gated", Some(Who::Owner));
    w.person_writes("brief");
    w.run("first", &[("ok", Value("no"))]);
    // The owner's answer may come: the work waits for it, and for the first cell beside it.
    assert_eq!(
        w.decide(),
        wait(
            "gated",
            vec![
                Waited::Call {
                    call,
                    on: Some(Who::Owner)
                },
                precondition("ok", vec![by_cell("first")])
            ]
        )
    );
    // Answered, the work waits only for the first cell, which has nothing new.
    w.done(call);
    assert_eq!(w.decide(), stalled("gated", &["ok"], &[("first", Unrun::NothingNew)]));
}

#[test]
fn a_stalled_work_is_stalled_whether_or_not_it_is_paused() {
    let mut w = research_to_merge();
    w.run("research", &[("deep", Value("no"))]);
    let stalled = w.decide();
    assert!(matches!(stalled, Decision::Stalled { .. }), "{stalled:?}");
    w.engine_pauses("it can go on by itself no further");
    assert_eq!(w.decide(), stalled);
}
