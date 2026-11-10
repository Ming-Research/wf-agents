//! Runs at rest that are stale.

use super::*;

#[test]
fn a_run_at_rest_is_abandoned_when_an_input_changes_in_meaning_and_its_cell_runs_again_for_the_change() {
    let (mut w, _, c1) = reviewed();
    w.person_writes("plan");
    w.run("implement", &[("code", Matters)]);
    let review = w.start("review");
    w.returns("review");
    w.person_writes("plan");
    let c3 = w.run("implement", &[("code", Matters)]);
    assert_eq!(w.decide(), abandon("review", review, outdated("code")));
    w.abandon();
    assert_eq!(
        w.decide(),
        runs(
            "review",
            vec![changed("code", Some(c1), false)],
            &[("code", Some(c3)), ("opinions", None)]
        )
    );
}

#[test]
fn a_run_at_rest_is_left_when_a_person_writes_an_opinion_it_may_write() {
    let (mut w, ..) = reviewed();
    w.person_writes("plan");
    w.run("implement", &[("code", Matters)]);
    let review = w.start("review");
    w.returns("review");
    w.person_writes("opinions");
    // The opinions are no input of the review, so its run is not stale. The implementation, before the review, runs
    // first for the person's opinion.
    assert_eq!(
        reasons(&w.decide()),
        ("implement".into(), vec![("opinions".into(), false)])
    );
    // It changes nothing the review reads, and the review's run goes on from where it rested.
    w.run("implement", &[]);
    assert_eq!(w.decide(), go_on("review", review));
}

#[test]
fn a_run_at_rest_is_left_when_a_person_writes_a_block_it_may_write_and_does_not_read() {
    let (mut w, ..) = planned();
    w.person_writes("background");
    let research = w.start("research");
    w.returns("research");
    w.person_writes("plan");
    // The plan is no input of the research: a cell runs again only for what it reads.
    assert_eq!(w.decide(), go_on("research", research));
}

#[test]
fn a_run_at_rest_goes_on_when_what_it_started_from_was_rewritten_in_a_way_said_not_to_matter() {
    let (mut w, ..) = planned();
    w.person_writes("background");
    w.run("research", &[("plan", Matters)]);
    let implement = w.start("implement");
    w.returns("implement");
    w.person_writes("background");
    w.run("research", &[("plan", DoesNotMatter)]);
    assert_eq!(w.decide(), go_on("implement", implement));
}

#[test]
fn a_stale_run_at_rest_is_abandoned_before_the_walk_runs_a_cell_before_it() {
    let mut w = World::with_gate(vec![cell("a", &["topic"], &[]), cell("b", &["topic"], &[])]);
    w.person_writes("topic");
    w.run("a", &[]);
    w.run("b", &[]);
    let t2 = w.person_writes("topic");
    w.run("a", &[]);
    let b = w.start("b");
    w.returns("b");
    let t3 = w.person_writes("topic");
    assert_eq!(w.decide(), abandon("b", b, outdated("topic")));
    w.abandon();
    assert_eq!(
        w.decide(),
        runs("a", vec![changed("topic", Some(t2), false)], &[("topic", Some(t3))])
    );
}
