//! The walk, rule by rule: each test builds a small work and says what the walk decides.

mod support;

use decide::{Cause, Condition, Decision, End, Refuser, Stale, Waited, Who};
use support::Says::{DoesNotMatter, Matters};
use support::{World, cell};

#[test]
fn a_work_walks_its_cells_in_order_and_ends_when_the_last_has_a_record() {
    let mut w = World::new(vec![cell("a", &[], &[]), cell("b", &[], &[])]);
    assert_eq!(w.because(), vec![Cause::NoRecord]);
    w.run("a", &[], &[]);
    assert_eq!(w.runs_next().as_deref(), Some("b"));
    w.run("b", &[], &[]);
    assert_eq!(w.decide(), Decision::Ended(End::Passed));
}

#[test]
fn a_run_is_busy_only_while_a_round_is_under_way_and_goes_on_when_the_walk_reaches_it() {
    let mut w = World::new(vec![cell("a", &[], &[]), cell("b", &[], &[])]);
    let run = w.start("a");
    let busy = Decision::Busy { cell: "a".into(), run };
    assert_eq!(w.decide(), busy);
    w.returns("a");
    let go_on = Decision::GoOn { cell: "a".into(), run };
    assert_eq!(w.decide(), go_on);
    w.go_on("a");
    assert_eq!(w.decide(), busy);
}

#[test]
fn a_refusal_runs_the_writer_again_given_it_and_then_the_one_who_refused() {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        cell("implement", &["plan"], &["code"]),
        cell("review", &["code"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    w.run("research", &[], &[("plan", Matters)]);
    w.run("implement", &[], &[("code", Matters)]);
    w.run("review", &[], &[]);
    assert!(w.rests());
    let refusal = w.refuse("review", "code");
    let by = Refuser::Run(w.record("review"));
    let refused = Cause::Refused {
        refusal,
        product: "code".into(),
        by,
    };
    assert_eq!(w.runs_next().as_deref(), Some("implement"));
    assert_eq!(w.because(), vec![refused]);
    // Even a write said not to matter ends the refusal and brings the one who refused to judge again.
    w.run("implement", &[], &[("code", DoesNotMatter)]);
    let answered = Cause::Answered {
        refusal,
        product: "code".into(),
    };
    assert_eq!(w.runs_next().as_deref(), Some("review"));
    assert_eq!(w.because(), vec![answered]);
    w.run("review", &[], &[]);
    assert!(w.rests());
}

/// A work with the cell that takes comments before its implementation, the implementation resting between two rounds
/// when a first comment arrives.
fn commented() -> World {
    let mut comments = cell("comments", &["comments"], &["notes"]);
    comments.applies = Some(Condition::Test("the item has a comment".into()));
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        comments,
        cell("implement", &["plan", "notes"], &["code"]),
    ];
    let mut w = World::with_gate(cells);
    w.set("the item has a comment", decide::Outcome::Waits { who: Who::Owner });
    w.person_writes("background");
    w.run("research", &[], &[("plan", Matters)]);
    w.start("implement");
    w.returns("implement");
    w.set("the item has a comment", decide::Outcome::Holds);
    w.person_writes("comments");
    w
}

#[test]
fn a_cell_before_a_run_at_rest_runs_first_and_the_run_then_goes_on_if_nothing_it_started_from_changed() {
    let mut w = commented();
    assert_eq!(w.runs_next().as_deref(), Some("comments"));
    // The comment changes nothing: the cell writes no note, and the implementation goes on where it was.
    w.run("comments", &[], &[]);
    w.go_on("implement");
}

#[test]
fn a_run_at_rest_is_abandoned_when_a_cell_before_it_changes_what_it_started_from() {
    let mut w = commented();
    w.run("comments", &[], &[("notes", Matters)]);
    let outdated = Stale::Outdated {
        product: "notes".into(),
    };
    assert_eq!(w.abandon(), outdated);
    assert_eq!(w.runs_next().as_deref(), Some("implement"));
}

#[test]
fn a_refusal_made_while_its_writer_is_at_rest_abandons_that_run_and_the_next_is_given_it() {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        cell("implement", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    w.run("research", &[], &[("plan", Matters)]);
    w.run("implement", &[], &[]);
    w.person_writes("background");
    w.start("research");
    let refusal = w.owner_refuses("plan");
    // The round under way is not interrupted; when it ends the run is found not to have been given the refusal.
    assert!(matches!(w.decide(), Decision::Busy { .. }));
    w.returns("research");
    assert_eq!(w.abandon(), Stale::Refused { refusal });
    let Decision::Run { cell, given, .. } = w.decide() else {
        panic!("research does not run again: {:?}", w.decide())
    };
    assert_eq!((cell.as_str(), given), ("research", vec![refusal]));
    w.run("research", &[], &[("plan", Matters)]);
    assert_eq!(w.runs_next().as_deref(), Some("implement"));
}

#[test]
fn a_cell_run_again_and_again_only_for_what_came_after_it_waits_for_the_owner_at_its_limit() {
    let cells = vec![cell("implement", &["plan"], &["code"]), cell("review", &["code"], &[])];
    let mut w = World::with_gate(cells);
    w.person_writes("plan");
    w.run("implement", &[], &[("code", Matters)]);
    w.run("review", &[], &[]);
    for _ in 0..3 {
        w.refuse("review", "code");
        w.run("implement", &[], &[("code", Matters)]);
        w.run("review", &[], &[]);
    }
    w.refuse("review", "code");
    assert_eq!(w.waits(), ("implement".to_string(), vec![Waited::Limit { runs: 3 }]));
    w.let_go("implement");
    assert_eq!(w.runs_next().as_deref(), Some("implement"));
}
