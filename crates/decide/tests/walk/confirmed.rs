//! A test of what was confirmed (entry opinions-are-inputs: a confirmation changes no meaning and only meets the
//! precondition that waits for it): the merge waits for the owner's confirmation of what a block says, only while it
//! says something, and again once what it holds changed after the confirmation, whatever its writer says of the
//! change.

use decide::{Outsiders, Test, TestKind, TestName};

use super::*;

/// The confirmation people write, and the block it confirms.
const BY: &str = "confirmed";
const ASKED: &str = "questions";

/// The test of what was confirmed, over the questions, by the owner's yes in [`BY`].
fn confirming() -> Test {
    Test {
        kind: TestKind::Confirmed,
        reads: vec![ASKED.into()],
        is: "yes".to_string(),
        by: Some(BY.into()),
    }
}

/// A plan that may put questions, and a merge that waits until the owner confirmed them.
fn asking() -> World {
    let cells = vec![
        cell("plan", &["brief"], &[ASKED, "code"]),
        requiring("answered", Cell::new("merge")),
    ];
    let mut w = World::with_tests(cells, [(TestName::from("answered"), confirming())].into());
    w.written_by_cells(ASKED);
    w
}

fn waits() -> Decision {
    wait("merge", vec![precondition("answered", vec![Maker::Owner])])
}

fn merges() -> Decision {
    runs("merge", vec![NO_RECORD], &[])
}

#[test]
fn a_work_whose_block_was_never_written_is_not_held_and_the_owner_is_not_asked() {
    let mut w = asking();
    w.person_writes("brief");
    w.run("plan", &[("code", Matters)]);
    assert_eq!(w.decide(), merges());
}

#[test]
fn a_block_that_says_something_holds_the_work_for_the_owner_until_the_owner_confirms_it() {
    let mut w = asking();
    w.person_writes("brief");
    w.run("plan", &[(ASKED, Matters), ("code", Matters)]);
    assert_eq!(w.decide(), waits());
    w.person_sets(BY, "yes");
    assert_eq!(w.decide(), merges());
}

#[test]
fn a_confirmation_given_before_the_block_said_what_it_says_now_does_not_count() {
    let mut w = asking();
    w.person_sets(BY, "yes");
    w.person_writes("brief");
    w.run("plan", &[(ASKED, Matters), ("code", Matters)]);
    assert_eq!(w.decide(), waits());
}

#[test]
fn a_question_added_after_the_confirmation_holds_the_merge_again_though_its_writer_says_it_does_not_matter() {
    let mut w = asking();
    w.person_writes("brief");
    w.run("plan", &[(ASKED, Matters), ("code", Matters)]);
    w.person_sets(BY, "yes");
    assert_eq!(w.decide(), merges());
    // The plan runs again and adds a question, saying the change does not matter: the owner has not seen it.
    w.person_writes("brief");
    w.run("plan", &[(ASKED, DoesNotMatter), ("code", Matters)]);
    assert_eq!(w.decide(), waits());
    // Changed again, saying it matters: asked again, and the same yes written again confirms it.
    w.person_writes("brief");
    w.run("plan", &[(ASKED, Matters), ("code", Matters)]);
    assert_eq!(w.decide(), waits());
    w.person_sets(BY, "yes");
    assert_eq!(w.decide(), merges());
}

#[test]
fn the_same_questions_written_again_keep_the_confirmation_though_their_writer_says_the_change_matters() {
    let mut w = asking();
    w.person_writes("brief");
    w.run("plan", &[(ASKED, Matters), ("code", Matters)]);
    w.person_sets(BY, "yes");
    w.person_writes("brief");
    let at = w.run("plan", &[(ASKED, Matters), ("code", Matters)]);
    // The engine tells that the version holds what the one before it held.
    w.records.unchanged.insert((ASKED.into(), at));
    assert_eq!(w.decide(), merges());
}

#[test]
fn a_block_written_blank_says_nothing_and_holds_nothing() {
    let mut w = asking();
    w.person_writes("brief");
    w.run("plan", &[(ASKED, Matters), ("code", Matters)]);
    assert_eq!(w.decide(), waits());
    w.person_writes("brief");
    let at = w.run("plan", &[(ASKED, Matters), ("code", Matters)]);
    w.records.blank.insert((ASKED.into(), at));
    assert_eq!(w.decide(), merges());
}

#[test]
fn only_the_value_of_the_test_confirms() {
    let mut w = asking();
    w.person_writes("brief");
    w.run("plan", &[(ASKED, Matters), ("code", Matters)]);
    w.person_sets(BY, "yes");
    w.person_sets(BY, "no");
    assert_eq!(w.decide(), waits());
}

#[test]
fn a_test_of_what_was_confirmed_names_a_choice_only_people_write_and_no_other_test_names_one() {
    let cells = || {
        vec![
            cell("plan", &["brief"], &[ASKED, "code"]),
            cell("judge", &["code"], &["verdict"]),
            requiring("answered", Cell::new("merge")),
        ]
    };
    let people = Outsiders {
        people: true,
        ..Outsiders::default()
    };
    let outsiders = |by: &str| [(BlockId::from(by), people.clone()), ("brief".into(), people.clone())].into();
    let flow = |test: Test, by: &str| {
        Flow::new(cells(), [(TestName::from("answered"), test)].into(), outsiders(by)).map(|_| ())
    };
    let refused: Result<(), FlowFault> = Err(FlowFault::Unconfirmable("answered".into()));
    assert_eq!(flow(confirming(), BY), Ok(()));
    assert_eq!(
        flow(
            Test {
                by: None,
                ..confirming()
            },
            BY
        ),
        refused
    );
    assert_eq!(flow(confirming(), "nobody"), refused.clone());
    let by_a_cell = Test {
        by: Some("verdict".into()),
        ..confirming()
    };
    assert_eq!(flow(by_a_cell, "verdict"), refused.clone());
    let another_kind = Test {
        kind: TestKind::All,
        ..confirming()
    };
    assert_eq!(flow(another_kind, BY), refused);
}
