//! Opinions: what someone has to say to a cell, among what runs it.

use super::*;

#[test]
fn a_judging_cells_opinion_runs_the_cell_it_is_for_and_a_judge_with_none_runs_nobody() {
    let (mut w, plan, c1) = judged();
    let o1 = w.run("review", &[("opinions", Matters)]);
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("opinions", None, true)],
            &[("plan", Some(plan)), ("opinions", Some(o1)), ("code", Some(c1))]
        )
    );
    let c2 = w.run("implement", &[("code", Matters)]);
    assert_eq!(
        w.decide(),
        runs(
            "review",
            vec![changed("code", Some(c1), false)],
            &[("code", Some(c2)), ("opinions", Some(o1))]
        )
    );
    // The review has nothing more to say: it writes no opinion, and the implementation's inputs are as they were.
    w.run("review", &[]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_persons_opinion_runs_the_cell_it_is_for_and_is_no_way_round() {
    let (mut w, plan, code) = reviewed();
    let opinion = w.person_writes("opinions");
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("opinions", None, false)],
            &[("plan", Some(plan)), ("opinions", Some(opinion)), ("code", Some(code))]
        )
    );
}

#[test]
fn an_opinion_for_a_cell_that_has_no_record_is_among_what_its_first_run_starts_from() {
    let mut w = World::with_gate(vec![
        cell("design", &["brief", "opinions"], &["plan"]),
        cell("review", &["plan"], &["opinions"]),
    ]);
    let brief = w.person_writes("brief");
    let opinion = w.person_writes("opinions");
    assert_eq!(
        w.decide(),
        runs(
            "design",
            first(&["brief", "opinions"]),
            &[("brief", Some(brief)), ("opinions", Some(opinion)), ("plan", None)]
        )
    );
}

#[test]
fn a_judged_cell_that_writes_nothing_new_leaves_its_judge_passed() {
    let (mut w, ..) = judged();
    w.run("review", &[("opinions", Matters)]);
    // The implementation answers the opinion without changing the code: the review has nothing new to judge, and the
    // work rests where the merge's precondition holds it.
    w.run("implement", &[]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn an_answer_to_an_opinion_that_changes_nothing_else_brings_the_judge_to_look_again() {
    let cells = vec![
        cell("implement", &["brief", "opinions"], &["code", "answers"]),
        cell("review", &["code", "answers"], &["opinions"]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("brief");
    let code = w.run("implement", &[("code", Matters)]);
    let opinion = w.run("review", &[("opinions", Matters)]);
    // The implementation disagrees and changes no code: it writes its answer, which the review reads.
    let answer = w.run("implement", &[("answers", Matters)]);
    assert_eq!(
        w.decide(),
        runs(
            "review",
            vec![changed("answers", None, false)],
            &[
                ("code", Some(code)),
                ("answers", Some(answer)),
                ("opinions", Some(opinion))
            ]
        )
    );
}
