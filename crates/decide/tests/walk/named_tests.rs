//! The named tests, worked out by the walk from the versions it reads: each kind on blocks unwritten, written, several
//! and rewritten; a test of what stands, by the writers' conditions; and the two histories of judges the merge waits
//! on.

use decide::{Test, TestKind, TestName};

use super::*;

/// A test of this kind, reading these blocks, for this value.
fn test(kind: TestKind, reads: &[&str], is: &str) -> Test {
    Test {
        kind,
        reads: reads.iter().map(|b| (*b).into()).collect(),
        is: is.to_string(),
    }
}

fn tests(named: &[(&str, Test)]) -> BTreeMap<TestName, Test> {
    named.iter().map(|(n, t)| (TestName::from(*n), t.clone())).collect()
}

/// Whether the test `t`, of this kind over `a` and `b` for yes, holds once people wrote these values in this order:
/// the cell it holds back runs, or the work waits before it for the owner.
fn holds(kind: TestKind, writes: &[(&str, &str)]) -> bool {
    let cells = vec![requiring("t", cell("c", &[], &[])), Cell::new("merge")];
    let mut w = World::with_tests(cells, tests(&[("t", test(kind, &["a", "b"], "yes"))]));
    for (block, value) in writes {
        w.person_sets(block, value);
    }
    match w.decide() {
        Decision::Run { cell, .. } if cell.as_str() == "c" => true,
        d if d == wait("c", vec![precondition("t", Who::Owner)]) => false,
        other => panic!("the walk decides {other:?}"),
    }
}

#[test]
fn a_test_that_every_block_has_the_value_holds_only_when_each_has_it_now() {
    let all = |writes: &[(&str, &str)]| holds(TestKind::All, writes);
    assert!(!all(&[]));
    assert!(!all(&[("a", "yes")]));
    assert!(all(&[("a", "yes"), ("b", "yes")]));
    assert!(!all(&[("a", "yes"), ("b", "no")]));
    assert!(!all(&[("a", "yes"), ("b", "yes"), ("a", "no")]));
    assert!(all(&[("a", "no"), ("b", "yes"), ("a", "yes")]));
}

#[test]
fn a_test_that_one_block_has_the_value_holds_while_any_has_it_now() {
    let any = |writes: &[(&str, &str)]| holds(TestKind::Any, writes);
    assert!(!any(&[]));
    assert!(any(&[("a", "yes")]));
    assert!(any(&[("b", "yes")]));
    assert!(!any(&[("a", "no"), ("b", "no")]));
    assert!(any(&[("a", "no"), ("b", "yes")]));
    assert!(!any(&[("a", "yes"), ("a", "no")]));
}

#[test]
fn a_test_that_no_block_has_the_value_holds_while_none_has_it_now_an_unwritten_one_having_none() {
    let none = |writes: &[(&str, &str)]| holds(TestKind::None, writes);
    assert!(none(&[]));
    assert!(!none(&[("a", "yes")]));
    assert!(none(&[("a", "no"), ("b", "no")]));
    assert!(!none(&[("a", "no"), ("b", "yes")]));
    assert!(none(&[("a", "yes"), ("a", "no")]));
}

#[test]
fn a_test_of_what_stands_holds_as_one_that_no_block_has_the_value_where_people_wrote_every_block() {
    let standing = |writes: &[(&str, &str)]| holds(TestKind::NoneStanding, writes);
    assert!(standing(&[]));
    assert!(!standing(&[("a", "yes")]));
    assert!(standing(&[("a", "no"), ("b", "no")]));
    assert!(!standing(&[("a", "no"), ("b", "yes")]));
    assert!(standing(&[("a", "yes"), ("a", "no")]));
}

/// An asker writing whether a judge runs, from a brief; the judge, run only when asked to, writing a verdict; and the
/// merge, which waits while the verdict stands at no. The judge's condition is on the asker's choice, or a named test
/// of it where `by_test`.
fn judged(by_test: bool) -> World {
    let judge = cell("judge", &["code"], &["verdict"]);
    let judge = if by_test {
        on_test("asked", judge)
    } else {
        when("ask", "yes", judge)
    };
    let cells = vec![
        cell("asker", &["brief"], &["ask", "code"]),
        judge,
        requiring("unrefused", Cell::new("merge")),
    ];
    let named = tests(&[
        ("asked", test(TestKind::Any, &["ask"], "yes")),
        ("unrefused", test(TestKind::NoneStanding, &["verdict"], "no")),
    ]);
    World::with_tests(cells, named)
}

fn refused() -> Decision {
    wait("merge", vec![precondition("unrefused", Who::Owner)])
}

#[test]
fn a_judges_no_holds_the_merge_while_the_judge_is_not_skipped_and_nothing_once_it_is() {
    for by_test in [false, true] {
        let mut w = judged(by_test);
        w.person_writes("brief");
        w.run("asker", &[("ask", Value("yes")), ("code", Matters)]);
        w.run("judge", &[("verdict", Value("no"))]);
        assert_eq!(w.decide(), refused(), "by test: {by_test}");
        // The asker, run again for a new brief, no longer asks for the judge: its no stands no more.
        w.person_writes("brief");
        w.run("asker", &[("ask", Value("no"))]);
        assert_eq!(w.decide(), runs("merge", vec![NO_RECORD], &[]), "by test: {by_test}");
    }
}

#[test]
fn a_judge_that_applies_again_with_nothing_new_to_judge_has_its_old_no_stand_again() {
    let mut w = judged(false);
    w.person_writes("brief");
    w.run("asker", &[("ask", Value("yes")), ("code", Matters)]);
    w.run("judge", &[("verdict", Value("no"))]);
    w.person_writes("brief");
    w.run("asker", &[("ask", Value("no"))]);
    w.person_writes("brief");
    w.run("asker", &[("ask", Value("yes"))]);
    // The judge's record knows the code as it is: it runs not, and its no stands again.
    assert_eq!(w.decide(), refused());
}

#[test]
fn what_a_person_wrote_stands_whatever_the_cell() {
    let mut w = judged(false);
    w.person_writes("brief");
    w.run("asker", &[("ask", Value("no")), ("code", Matters)]);
    // The judge is skipped, and the owner's no stands.
    w.person_sets("verdict", "no");
    assert_eq!(w.decide(), refused());
    w.person_sets("verdict", "yes");
    assert_eq!(w.decide(), runs("merge", vec![NO_RECORD], &[]));
    // Over a judge's no, the owner's yes lets the work go on while the judge runs.
    let mut w = judged(false);
    w.person_writes("brief");
    w.run("asker", &[("ask", Value("yes")), ("code", Matters)]);
    w.run("judge", &[("verdict", Value("no"))]);
    w.person_sets("verdict", "yes");
    assert_eq!(w.decide(), runs("merge", vec![NO_RECORD], &[]));
}

#[test]
fn a_cell_that_names_a_test_the_flow_does_not_have_is_refused() {
    let cells = vec![requiring("nobody", cell("a", &[], &[])), Cell::new("merge")];
    assert_eq!(
        Flow::new(cells, BTreeMap::new()),
        Err(FlowFault::NoSuchTest("a".into(), "nobody".into()))
    );
    let cells = vec![on_test("nobody", cell("a", &[], &[])), Cell::new("merge")];
    assert_eq!(
        Flow::new(cells, BTreeMap::new()),
        Err(FlowFault::NoSuchTest("a".into(), "nobody".into()))
    );
}

/// A writer, a judge that decides whether a second judge runs, and the second; the merge waits while either
/// judge's verdict stands at no. Each judge reads what the writer wrote and its answer, and writes opinions for it.
fn two_judges() -> World {
    let cells = vec![
        cell("write", &["brief", "opinions"], &["code", "answer"]),
        cell("judge", &["code", "answer"], &["verdict", "deeper", "opinions"]),
        when(
            "deeper",
            "yes",
            cell("deep", &["code", "answer"], &["deep_verdict", "opinions"]),
        ),
        requiring("approved", Cell::new("merge")),
    ];
    let named = tests(&[(
        "approved",
        test(TestKind::NoneStanding, &["verdict", "deep_verdict"], "no"),
    )]);
    World::with_tests(cells, named)
}

#[test]
fn the_second_judges_no_holds_nothing_once_the_first_finds_it_no_longer_needed() {
    let mut w = two_judges();
    w.person_writes("brief");
    w.run("write", &[("code", Matters)]);
    w.run("judge", &[("verdict", Value("yes")), ("deeper", Value("yes"))]);
    w.run("deep", &[("deep_verdict", Value("no")), ("opinions", Matters)]);
    // The writer fixes what the second judge found; the first judge looks again and needs the second no more.
    w.run("write", &[("code", Matters), ("answer", Matters)]);
    w.run("judge", &[("verdict", Value("yes")), ("deeper", Value("no"))]);
    // The second judge is skipped, its no stands no more, and the work reaches the merge without the owner.
    assert_eq!(w.decide(), runs("merge", vec![NO_RECORD], &[]));
}

#[test]
fn a_judge_that_still_says_no_with_nothing_new_to_say_holds_the_merge_for_the_owner() {
    let mut w = two_judges();
    w.person_writes("brief");
    w.run("write", &[("code", Matters)]);
    w.run(
        "judge",
        &[("verdict", Value("no")), ("deeper", Value("no")), ("opinions", Matters)],
    );
    // The writer answers without changing what was judged; the judge looks again and still says no, and has nothing
    // new to say: the writer has nothing to run for.
    w.run("write", &[("answer", Matters)]);
    w.run("judge", &[("verdict", Value("no"))]);
    let approved = wait("merge", vec![precondition("approved", Who::Owner)]);
    assert_eq!(w.decide(), approved);
    // The owner writes the judge's verdict as yes, and the work goes on.
    w.person_sets("verdict", "yes");
    assert_eq!(w.decide(), runs("merge", vec![NO_RECORD], &[]));
}
