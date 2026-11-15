//! When a cell runs: a block that runs it is new to it, and only then.

use super::*;

#[test]
fn a_cell_whose_record_knows_the_current_meaning_of_every_input_is_passed_and_a_block_no_cell_reads_changes_nothing() {
    let (mut w, ..) = planned();
    w.person_writes("elsewhere");
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_changed_input_runs_only_the_cells_that_read_it() {
    let cells = vec![
        cell("a", &["x"], &["p"]),
        cell("b", &["y"], &[]),
        cell("c", &["p"], &[]),
    ];
    let mut w = World::with_gate(cells);
    let x1 = w.person_writes("x");
    w.person_writes("y");
    let p1 = w.run("a", &[("p", Matters)]);
    w.run("b", &[]);
    w.run("c", &[]);
    let x2 = w.person_writes("x");
    assert_eq!(
        w.decide(),
        runs(
            "a",
            vec![changed("x", Some(x1), false)],
            &[("x", Some(x2)), ("p", Some(p1))]
        )
    );
    let p2 = w.run("a", &[("p", Matters)]);
    assert_eq!(
        w.decide(),
        runs("c", vec![changed("p", Some(p1), false)], &[("p", Some(p2))])
    );
    w.run("c", &[]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_write_its_writer_says_does_not_matter_runs_nobody() {
    let (mut w, ..) = planned();
    w.person_writes("background");
    w.run("research", &[("plan", DoesNotMatter)]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_cell_does_not_run_for_its_own_write_of_a_block_that_runs_it() {
    let mut w = World::with_gate(vec![
        cell("design", &["plan"], &["plan"]),
        cell("implement", &["plan"], &[]),
    ]);
    w.person_writes("plan");
    let plan = w.run("design", &[("plan", Matters)]);
    assert_eq!(w.decide(), runs("implement", first(&["plan"]), &[("plan", Some(plan))]));
    w.run("implement", &[]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_later_cells_write_of_a_block_that_runs_an_earlier_cell_runs_the_earlier_cell_as_behind() {
    let mut w = World::with_gate(vec![
        cell("design", &["plan"], &["plan"]),
        cell("revise", &["plan"], &["plan"]),
    ]);
    let p0 = w.person_writes("plan");
    let p1 = w.run("design", &[("plan", Matters)]);
    assert_eq!(w.decide(), runs("revise", first(&["plan"]), &[("plan", Some(p1))]));
    let p2 = w.run("revise", &[("plan", Matters)]);
    // The design consumed the person's plan; its own write is not new to it, and the revision is behind it.
    assert_eq!(
        w.decide(),
        runs("design", vec![changed("plan", Some(p0), true)], &[("plan", Some(p2))])
    );
}

#[test]
fn what_a_cell_consumed_is_the_version_its_round_was_told_of_and_its_own_writes_are_not_new_to_it() {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        cell("design", &["plan"], &["plan"]),
        cell("implement", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    let p1 = w.run("research", &[("plan", Matters)]);
    assert_eq!(w.decide(), runs("design", first(&["plan"]), &[("plan", Some(p1))]));
    let p2 = w.run("design", &[("plan", Matters)]);
    w.run("implement", &[]);
    let p3 = w.person_writes("plan");
    // The design consumed the research's plan, and wrote its own, which is not new to it.
    assert_eq!(
        w.decide(),
        runs("design", vec![changed("plan", Some(p1), false)], &[("plan", Some(p3))])
    );
    let p4 = w.run("design", &[("plan", Matters)]);
    // The implementation consumed the design's plan.
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("plan", Some(p2), false)],
            &[("plan", Some(p4))]
        )
    );
}

#[test]
fn a_return_that_deals_with_only_part_of_what_ran_its_round_leaves_the_rest_new() {
    let mut w = World::with_gate(vec![cell("answer", &["question", "plan"], &["reply"])]);
    let question = w.person_writes("question");
    let plan = w.person_writes("plan");
    w.start("answer");
    w.finish_dealing("answer", &["question"], &[("reply", Matters)]);
    let reply = w.records.finished.last().expect("a finish").at;
    assert_eq!(
        w.decide(),
        runs(
            "answer",
            vec![changed("plan", None, false)],
            &[
                ("question", Some(question)),
                ("plan", Some(plan)),
                ("reply", Some(reply))
            ]
        )
    );
}

#[test]
fn a_version_put_in_to_a_round_is_consumed_where_its_return_deals_with_its_block() {
    let mut w = World::with_gate(vec![cell("answer", &["question"], &["reply"])]);
    w.person_writes("question");
    let run = w.start("answer");
    let q2 = w.person_writes("question");
    let at = w.tick();
    w.records.put_in.push(PutIn {
        run,
        block: "question".into(),
        version: q2,
        at,
    });
    w.finish("answer", &[("reply", Matters)]);
    assert_eq!(w.decide(), resting());
    // A version that came after and never reached the round stays new.
    w.person_writes("question");
    let Decision::Run { because, .. } = w.decide() else {
        panic!("the answer does not run: {:?}", w.decide())
    };
    assert_eq!(because, vec![changed("question", Some(q2), false)]);
}

#[test]
fn a_persons_prose_always_changes_the_meaning_even_where_its_entry_says_it_does_not() {
    let (mut w, _, plan, code) = planned();
    let p2 = w.person_writes_not_mattering("plan");
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("plan", Some(plan), false)],
            &[("plan", Some(p2)), ("code", Some(code))]
        )
    );
}

#[test]
fn a_write_from_outside_the_work_always_changes_the_meaning() {
    let mut w = World::with_gate(vec![cell("check", &["main"], &[])]);
    let m1 = w.outside_writes("main");
    w.run("check", &[]);
    assert_eq!(w.decide(), resting());
    let m2 = w.outside_writes("main");
    assert_eq!(
        w.decide(),
        runs("check", vec![changed("main", Some(m1), false)], &[("main", Some(m2))])
    );
}
