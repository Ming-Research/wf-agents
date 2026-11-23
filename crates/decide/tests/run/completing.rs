//! A return sent back is completed by the next return: what the returns before it said, but what the faults they were
//! sent back for name, stands with what the next return gives, and the next return is judged as the whole. Nothing is
//! carried into or out of a return that asks for calls.

use decide::run::{Together, carried, together};

use super::*;

/// The round, which the round after it was told was wrong for these.
fn told(unmet: Vec<Unmet>, round: Round) -> Round {
    Round {
        sent_back: unmet,
        ..round
    }
}

fn not_handed_in(block: &str) -> Unmet {
    Unmet::NotHandedIn { block: block.into() }
}

/// A round whose return dealt with nothing and said why it changes these protected files.
fn saying(protected: &[(&str, &str)]) -> Round {
    Round {
        returned: Some(Return::Came {
            dealt_with: vec![],
            writes: vec![],
            calls: 0,
            found: vec![],
            waits: false,
            questions: Vec::new(),
            read: Vec::new(),
            protected: (protected.iter())
                .map(|(file, why)| (file.to_string(), why.to_string()))
                .collect(),
        }),
        sent_back: Vec::new(),
    }
}

#[test]
fn a_return_that_gives_only_what_the_one_before_lacked_completes_it() {
    let before = told(vec![not_handed_in("verdict")], handed_in(vec![prose("plan")]));
    let next = works(&[], vec![valued("verdict", "yes")]);
    assert_eq!(
        step(vec![before, next], &must_write(&["verdict"])),
        finish(vec![prose("plan"), valued("verdict", "yes")])
    );
}

#[test]
fn a_return_that_gives_nothing_stands_as_the_one_before_it_where_that_was_sent_back_for_nothing_it_wrote() {
    let before = told(vec![not_handed_in("verdict")], handed_in(vec![prose("plan")]));
    assert_eq!(
        step(vec![before, works(&[], vec![])], &must_write(&["verdict"])),
        send_back(vec![not_handed_in("verdict")])
    );
    // What was sent back for no block of it, but what the reading found, leaves its writes standing.
    let found = vec![Unmet::NotWatched {
        name: "on_nothing".to_string(),
    }];
    let before = told(
        found.clone(),
        came(&["request", "opinions"], vec![prose("plan")], found, false),
    );
    assert_eq!(
        step(vec![before, works(&[], vec![])], &spec()),
        finish(vec![prose("plan")])
    );
}

#[test]
fn a_block_the_faults_name_is_left_out_of_what_stands_and_owed_again() {
    let wrong = vec![Unmet::NoValue {
        block: "verdict".into(),
    }];
    let before = || told(wrong.clone(), handed_in(vec![prose("plan"), prose("verdict")]));
    let s = must_write(&["verdict"]);
    assert_eq!(
        step(vec![before(), works(&[], vec![])], &s),
        send_back(vec![not_handed_in("verdict")])
    );
    let next = works(&[], vec![valued("verdict", "no")]);
    assert_eq!(
        step(vec![before(), next], &s),
        finish(vec![prose("plan"), valued("verdict", "no")])
    );
}

#[test]
fn a_block_given_again_replaces_the_one_that_stands_and_is_not_written_twice() {
    let before = told(vec![not_handed_in("verdict")], handed_in(vec![prose("plan")]));
    let next = works(&[], vec![prose("plan"), valued("verdict", "yes")]);
    assert_eq!(
        step(vec![before, next], &must_write(&["verdict"])),
        finish(vec![prose("plan"), valued("verdict", "yes")])
    );
}

#[test]
fn what_the_return_before_dealt_with_stands_and_a_name_given_again_is_dealt_with_once() {
    let before = told(vec![not_handed_in("verdict")], works(&["request"], vec![prose("plan")]));
    let next = works(&["opinions", "request"], vec![valued("verdict", "yes")]);
    let finish = Step::Finish {
        dealt_with: vec!["request".into(), "opinions".into()],
        writes: vec![prose("plan"), valued("verdict", "yes")],
    };
    assert_eq!(step(vec![before, next], &must_write(&["verdict"])), finish);
}

#[test]
fn a_round_that_failed_between_two_returns_leaves_the_first_to_be_completed() {
    let before = told(vec![not_handed_in("verdict")], handed_in(vec![prose("plan")]));
    let next = works(&[], vec![valued("verdict", "yes")]);
    assert_eq!(
        step(
            vec![before, failed_as(Failure::Flagged), next],
            &must_write(&["verdict"])
        ),
        finish(vec![prose("plan"), valued("verdict", "yes")])
    );
}

#[test]
fn a_question_the_faults_name_is_left_out_and_one_they_do_not_stands_with_what_it_waits_for() {
    let s = must_write(&["verdict"]);
    // A question that waits holds the cell to no block it must write; one named wrong no longer does.
    let wrong = vec![Unmet::NotAnOption {
        name: "which".to_string(),
    }];
    let before = told(wrong, asks_beside(&["request", "opinions"], vec![prose("plan")], true));
    assert_eq!(
        step(vec![before, works(&[], vec![])], &s),
        send_back(vec![not_handed_in("verdict")])
    );
    // Sent back for a write beside it, the question stands, and so does the wait.
    let beside = vec![Unmet::NotAnOutput { block: "code".into() }];
    let before = told(beside, asks_beside(&["request", "opinions"], vec![prose("code")], true));
    let finish = Step::Finish {
        dealt_with: vec!["request".into(), "opinions".into()],
        writes: vec![],
    };
    assert_eq!(step(vec![before, works(&[], vec![])], &s), finish);
}

// A choice written with a value that stands with nothing but some blocks says the return must hold none of the cell's
// questions, where it asked one: each of them is wrong, and the wait of one that waited goes with them.
#[test]
fn a_return_sent_back_for_holding_questions_beside_a_value_that_allows_none_gives_up_each_of_them() {
    let s = Spec {
        questions: Some("questions".into()),
        ..must_write(&["verdict"])
    };
    let wrong = vec![Unmet::NothingBut {
        block: "verdict".into(),
        value: "no".to_string(),
        besides: vec!["questions".into()],
    }];
    let before = told(
        wrong,
        asks_beside(&["request", "opinions"], vec![valued("verdict", "no")], true),
    );
    // The question stood and was waited for, the verdict not being owed; it is given up with the verdict, which is owed.
    assert_eq!(
        step(vec![before, works(&[], vec![])], &s),
        send_back(vec![not_handed_in("verdict")])
    );
}

#[test]
fn nothing_is_carried_into_a_return_that_asks_for_calls_nor_out_of_one() {
    // Into: what the return before dealt with and wrote is not the call's.
    assert_eq!(step(vec![wanting(), asks(1)], &spec()), finish_asking());
    // Out of: a return that asks for calls and names work beside is no work of the next.
    let beside = Round {
        returned: Some(Return::Came {
            dealt_with: vec!["request".into()],
            writes: vec![prose("plan")],
            calls: 1,
            found: vec![],
            waits: false,
            questions: Vec::new(),
            read: Vec::new(),
            protected: Default::default(),
        }),
        sent_back: vec![Unmet::AsksAndWorks],
    };
    assert_eq!(
        step(vec![beside, works(&[], vec![])], &spec()),
        send_back(vec![Unmet::DealtWithNothing])
    );
}

#[test]
fn the_reasons_for_the_protected_files_stand_and_a_file_said_again_has_the_later_reason() {
    let before = told(
        vec![Unmet::DealtWithNothing],
        saying(&[("AGENTS.md", "one"), ("flow/cell.toml", "two")]),
    );
    let rounds = [before, saying(&[("AGENTS.md", "three")])];
    let taken = together(&rounds, None).expect("a return came back");
    let expected: BTreeMap<String, String> = [("AGENTS.md", "three"), ("flow/cell.toml", "two")]
        .into_iter()
        .map(|(file, why)| (file.to_string(), why.to_string()))
        .collect();
    assert_eq!(taken.protected, expected);
}

#[test]
fn what_the_next_return_completes_is_what_the_last_was_sent_back_for_not_naming() {
    assert_eq!(carried(&[], None), Together::default());
    let rounds = [told(
        vec![Unmet::NoValue {
            block: "verdict".into(),
        }],
        handed_in(vec![prose("plan"), prose("verdict")]),
    )];
    let completes = carried(&rounds, None);
    assert_eq!(completes.writes, [prose("plan")]);
    assert_eq!(
        completes.dealt_with,
        [BlockId::from("request"), BlockId::from("opinions")]
    );
    // The last return as it stands, not yet judged, is of the run taken together, with nothing left out.
    let whole = together(&rounds, None).expect("a return came back");
    assert_eq!(whole.writes, [prose("plan"), prose("verdict")]);
    // A return that asked for calls leaves nothing to complete.
    assert_eq!(carried(&[asks(2)], None), Together::default());
    // A round that failed after the return leaves it as it was.
    let after = [rounds[0].clone(), failed_as(Failure::Flagged)];
    assert_eq!(carried(&after, None), completes);
}
