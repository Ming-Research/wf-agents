//! The meaning of a version: who says whether a write changed what a block says.

use super::*;

#[test]
fn a_row_of_writes_each_said_not_to_matter_keeps_the_first_meaning() {
    let (mut w, _, plan, code) = planned();
    for _ in 0..3 {
        w.person_writes("background");
        w.run("research", &[("plan", DoesNotMatter)]);
        assert_eq!(w.decide(), resting());
    }
    // A change that matters is a change from the meaning the implementation knows, the first plan.
    w.person_writes("background");
    let p = w.run("research", &[("plan", Matters)]);
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("plan", Some(plan), false)],
            &[("plan", Some(p)), ("code", Some(code))]
        )
    );
}

#[test]
fn the_first_version_of_a_block_has_a_meaning_of_its_own_whatever_its_writer_says() {
    let mut w = World::with_gate(vec![
        cell("research", &["background"], &["plan"]),
        cell("implement", &["plan"], &[]),
    ]);
    w.person_writes("background");
    w.run("research", &[]);
    // No plan has a version, so nothing runs the implementation.
    assert_eq!(w.decide(), resting());
    w.person_writes("background");
    let plan = w.run("research", &[("plan", DoesNotMatter)]);
    assert_eq!(
        w.decide(),
        runs("implement", vec![changed("plan", None, false)], &[("plan", Some(plan))])
    );
}

#[test]
fn a_persons_confirmation_that_leaves_a_choice_as_it_was_runs_nobody_and_a_changed_choice_runs_its_readers() {
    let (mut w, choice) = chosen();
    w.person_confirms("choice", "yes");
    assert_eq!(w.decide(), resting());
    let no = w.person_sets("choice", "no");
    assert_eq!(
        w.decide(),
        runs(
            "use",
            vec![changed("choice", Some(choice), false)],
            &[("choice", Some(no))]
        )
    );
}

#[test]
fn a_cell_writing_a_choice_again_with_the_same_value_changes_no_meaning() {
    let (mut w, _) = chosen();
    w.person_writes("topic");
    w.run("ask", &[("choice", Value("yes"))]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_write_said_not_to_matter_by_a_run_that_did_not_see_what_it_wrote_over_is_not_taken_at_its_word() {
    let (mut w, _, known, code) = planned();
    w.person_writes("background");
    w.start("research");
    // A person rewrites the plan while the research's round is under way: the research never saw it.
    w.person_writes("plan");
    let plan = w.finish("research", &[("plan", DoesNotMatter)]);
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("plan", Some(known), false)],
            &[("plan", Some(plan)), ("code", Some(code))]
        )
    );
}

#[test]
fn of_two_cells_that_write_one_block_the_latest_write_is_current_whoever_wrote_it() {
    let cells = vec![
        cell("first", &["x"], &["plan"]),
        cell("second", &["y"], &["plan"]),
        cell("reader", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    let x1 = w.person_writes("x");
    w.person_writes("y");
    w.run("first", &[("plan", Matters)]);
    let p2 = w.run("second", &[("plan", Matters)]);
    assert_eq!(w.decide(), runs("reader", first(&["plan"]), &[("plan", Some(p2))]));
    w.run("reader", &[]);
    let x2 = w.person_writes("x");
    assert_eq!(
        w.decide(),
        runs(
            "first",
            vec![changed("x", Some(x1), false)],
            &[("x", Some(x2)), ("plan", Some(p2))]
        )
    );
    let p3 = w.run("first", &[("plan", Matters)]);
    assert_eq!(
        w.decide(),
        runs("reader", vec![changed("plan", Some(p2), false)], &[("plan", Some(p3))])
    );
}

#[test]
fn a_choice_rewritten_without_a_value_has_changed_its_meaning_whatever_its_writer_says() {
    let cells = vec![
        cell("ask", &["topic"], &["choice"]),
        when("choice", "yes", cell("extra", &["topic"], &[])),
        cell("use", &["choice"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("topic");
    let yes = w.run("ask", &[("choice", Value("yes"))]);
    w.run("extra", &[]);
    w.run("use", &[]);
    assert_eq!(w.decide(), resting());
    w.person_writes("topic");
    // The ask writes the choice again with no value and says it does not matter: the extra is skipped now, and the
    // reader of the choice runs.
    let none = w.run("ask", &[("choice", DoesNotMatter)]);
    assert_eq!(
        w.decide(),
        runs(
            "use",
            vec![changed("choice", Some(yes), false)],
            &[("choice", Some(none))]
        )
    );
}

#[test]
fn a_run_at_rest_whose_output_a_cell_before_it_rewrote_as_not_mattering_goes_on_and_is_taken_at_its_word() {
    // Both cells may write the plan; the reader comes after both.
    let cells = vec![
        cell("draft", &["brief"], &["plan"]),
        cell("refine", &["notes"], &["plan"]),
        cell("reader", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("brief");
    w.person_writes("notes");
    w.run("draft", &[("plan", Matters)]);
    w.run("refine", &[("plan", Matters)]);
    w.run("reader", &[]);
    w.person_writes("notes");
    let refine = w.start("refine");
    w.returns("refine");
    // The draft, before it, rewrites the plan as not mattering: what the refinement started from keeps its meaning.
    w.person_writes("brief");
    w.run("draft", &[("plan", DoesNotMatter)]);
    assert_eq!(w.decide(), go_on("refine", refine));
    w.go_on("refine");
    // The refinement says its plan does not matter. The version it writes over was written after its run started,
    // but has the meaning of the one it started from: it saw that meaning, and the reader is not run.
    w.finish("refine", &[("plan", DoesNotMatter)]);
    assert_eq!(w.decide(), resting());
}
