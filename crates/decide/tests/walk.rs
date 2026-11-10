//! The walk, rule by rule: each test builds a small work and says what the walk decides.

mod support;

use decide::{Cause, Condition, Decision, End, Stale, Waited};
use support::Says::{DoesNotMatter, Matters, Value};
use support::{World, cell};

/// What a cause says besides the version it was at: the block, and whether a cell after this one wrote the change.
fn changes(because: &[Cause]) -> Vec<(String, bool)> {
    (because.iter())
        .map(|c| match c {
            Cause::Changed { block, behind, .. } => (block.0.clone(), *behind),
            Cause::NoRecord => panic!("the cell has no record"),
        })
        .collect()
}

fn changed(block: &str) -> Vec<(String, bool)> {
    vec![(block.to_string(), false)]
}

fn behind(block: &str) -> Vec<(String, bool)> {
    vec![(block.to_string(), true)]
}

#[test]
fn a_work_walks_its_cells_in_order_and_ends_when_the_last_has_a_record() {
    let mut w = World::new(vec![cell("a", &[], &[]), cell("b", &[], &[])]);
    assert_eq!(w.runs(), ("a".to_string(), vec![Cause::NoRecord]));
    w.run("a", &[]);
    assert_eq!(w.runs(), ("b".to_string(), vec![Cause::NoRecord]));
    w.run("b", &[]);
    assert_eq!(w.decide(), Decision::Ended(End::Passed));
}

#[test]
fn a_cell_whose_inputs_are_at_the_versions_it_last_ran_on_is_passed() {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        cell("implement", &["plan"], &["code"]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    w.run("research", &[("plan", Matters)]);
    w.run("implement", &[("code", Matters)]);
    assert!(w.rests());
    // A block no cell reads changes nothing.
    w.person_writes("elsewhere");
    assert!(w.rests());
    // A changed input runs the cell that reads it, and a write said not to matter runs nobody after it.
    w.person_writes("background");
    let (next, because) = w.runs();
    assert_eq!((next.as_str(), changes(&because)), ("research", changed("background")));
    w.run("research", &[("plan", DoesNotMatter)]);
    assert!(w.rests());
}

/// The implementation and its review: the review writes its opinions, which the implementation reads.
fn reviewed() -> World {
    let cells = vec![
        cell("implement", &["plan", "opinions"], &["code"]),
        cell("review", &["code"], &["opinions"]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("plan");
    w.run("implement", &[("code", Matters)]);
    w
}

#[test]
fn an_opinion_is_an_input_of_the_cell_it_is_for_and_a_review_that_has_none_writes_nothing() {
    let mut w = reviewed();
    w.run("review", &[("opinions", Matters)]);
    let (next, because) = w.runs();
    assert_eq!((next.as_str(), changes(&because)), ("implement", behind("opinions")));
    w.run("implement", &[("code", Matters)]);
    let (next, because) = w.runs();
    assert_eq!((next.as_str(), changes(&because)), ("review", changed("code")));
    // The review has nothing more to say: it writes no opinion, and the implementation's inputs are as they were.
    w.run("review", &[]);
    assert!(w.rests());
}

#[test]
fn a_cell_run_again_and_again_only_for_what_a_cell_after_it_wrote_waits_for_the_owner_at_its_limit() {
    let mut w = reviewed();
    for _ in 0..3 {
        w.run("review", &[("opinions", Matters)]);
        w.run("implement", &[("code", Matters)]);
    }
    w.run("review", &[("opinions", Matters)]);
    let limit = Decision::Wait {
        at: "implement".into(),
        on: vec![Waited::Limit { runs: 3 }],
    };
    assert_eq!(w.decide(), limit);
    w.let_go("implement");
    let (next, because) = w.runs();
    assert_eq!((next.as_str(), changes(&because)), ("implement", behind("opinions")));
}

#[test]
fn a_person_writing_an_opinion_is_no_way_round_and_starts_the_count_anew() {
    let mut w = reviewed();
    for _ in 0..3 {
        w.run("review", &[("opinions", Matters)]);
        w.run("implement", &[("code", Matters)]);
    }
    w.run("review", &[("opinions", Matters)]);
    w.person_writes("opinions");
    let (next, because) = w.runs();
    assert_eq!((next.as_str(), changes(&because)), ("implement", changed("opinions")));
}

/// A review that says whether to test more, the testing that runs only then, and the implementation both judge.
fn tested() -> World {
    let mut testing = cell("testing", &["code"], &["tests", "opinions"]);
    testing.applies = Some(Condition::Is {
        block: "more".into(),
        value: "yes".into(),
    });
    let cells = vec![
        cell("implement", &["opinions"], &["code"]),
        cell("review", &["code"], &["more"]),
        testing,
        cell("check", &["code", "tests"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.run("implement", &[("code", Matters)]);
    w.run("review", &[("more", Value("yes"))]);
    w.run("testing", &[("tests", Matters), ("opinions", Matters)]);
    w
}

#[test]
fn a_cell_that_is_skipped_this_time_keeps_what_it_wrote_before() {
    let mut w = tested();
    w.run("implement", &[("code", Matters)]);
    // The review finds it enough: the testing is skipped though the code it reads has changed.
    w.run("review", &[("more", Value("no"))]);
    let (next, because) = w.runs();
    assert_eq!((next.as_str(), because), ("check", vec![Cause::NoRecord]));
    // The tests written before are still there for the check, which starts from them.
    let tests = w.records.finished[2].at;
    let Decision::Run { basis, .. } = w.decide() else {
        unreachable!()
    };
    assert_eq!(basis.get(&"tests".into()), Some(&Some(tests)));
}

#[test]
fn a_run_at_rest_lets_a_cell_before_it_run_and_is_abandoned_when_what_it_started_from_changes() {
    let mut w = reviewed();
    w.start("review");
    w.returns("review");
    // A person's opinion reaches the implementation while the review rests between two rounds.
    w.person_writes("opinions");
    let (next, because) = w.runs();
    assert_eq!((next.as_str(), changes(&because)), ("implement", changed("opinions")));
    w.run("implement", &[("code", Matters)]);
    let outdated = Stale::Outdated { block: "code".into() };
    assert_eq!(w.abandon(), outdated);
    assert_eq!(w.runs(), ("review".to_string(), vec![Cause::NoRecord]));
}

#[test]
fn a_write_said_not_to_matter_by_a_run_that_did_not_see_what_it_writes_over_has_a_meaning_of_its_own() {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        cell("implement", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    w.run("research", &[("plan", Matters)]);
    w.run("implement", &[]);
    w.person_writes("background");
    w.start("research");
    // A person rewrites the plan while the research's round is under way: the research never saw it.
    w.person_writes("plan");
    w.finish("research", &[("plan", DoesNotMatter)]);
    let (next, because) = w.runs();
    assert_eq!((next.as_str(), changes(&because)), ("implement", changed("plan")));
}
