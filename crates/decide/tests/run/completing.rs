//! A return sent back is completed by the next return: what the returns before it said, but what the faults they were
//! sent back for name, stands with what the next return gives, and the next return is judged as the whole. Nothing is
//! carried out of a return that asks for calls: what stood before such a return, and was not named, stands through it
//! and is written as the run ends in the call; what such a return itself says is no work of the next return.

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
fn nothing_is_carried_out_of_a_return_that_asks_for_calls_beside_work() {
    // A return that asks for calls and names work beside is no work of the next.
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

/// The cell with a third block it may write, which a line of the design tree is written in.
fn with_entries() -> Spec {
    Spec {
        outputs: vec!["plan".into(), "verdict".into(), "entries".into()],
        ..spec()
    }
}

/// What a run finishes with whose return asked for calls, and that wrote these blocks that stood before it.
fn finish_asking_with(writes: Vec<Write>) -> Step {
    Step::Finish {
        dealt_with: vec![],
        writes,
    }
}

fn no_entry() -> Unmet {
    Unmet::NotAnEntry {
        block: Some("entries".into()),
        line: "x".to_string(),
    }
}

// Reference: decision a-sent-back-return-is-completed-by-what-is-missing, and the item's own case: a return was sent
// back, its next asks for calls, the run ends, and what was not named wrong is written all the same.
#[test]
fn a_run_that_ends_in_a_call_after_a_return_was_sent_back_writes_what_the_send_back_did_not_name() {
    let before = told(vec![no_entry()], handed_in(vec![prose("plan"), prose("entries")]));
    // The block the fault names is left out; the other stands; the run deals with nothing, so its cell is entered again.
    assert_eq!(
        step(vec![before, asks(1)], &with_entries()),
        finish_asking_with(vec![prose("plan")])
    );
    // Sent back for a fault of no block, everything stands.
    let found = vec![Unmet::NotWatched {
        name: "on_nothing".to_string(),
    }];
    let before = told(
        found.clone(),
        came(
            &["request", "opinions"],
            vec![prose("plan"), prose("entries")],
            found,
            false,
        ),
    );
    assert_eq!(
        step(vec![before, asks(1)], &with_entries()),
        finish_asking_with(vec![prose("plan"), prose("entries")])
    );
}

#[test]
fn a_return_that_asks_for_calls_and_writes_beside_them_is_still_sent_back_where_blocks_stand() {
    let before = told(vec![not_handed_in("verdict")], handed_in(vec![prose("plan")]));
    let beside = Round {
        returned: Some(Return::Came {
            dealt_with: vec![],
            writes: vec![prose("verdict")],
            calls: 1,
            found: vec![],
            waits: false,
            questions: Vec::new(),
            read: Vec::new(),
            protected: Default::default(),
        }),
        sent_back: Vec::new(),
    };
    assert_eq!(
        step(vec![before, beside], &spec()),
        send_back(vec![Unmet::AsksAndWorks])
    );
}

// A block a fault names is written wrong, and nothing is carried of it, with or without a call between.
#[test]
fn a_write_a_not_an_entry_fault_names_is_not_taken_though_the_next_return_does_not_write_it_again() {
    let before = told(vec![no_entry()], handed_in(vec![prose("plan"), prose("entries")]));
    assert_eq!(
        step(vec![before, works(&[], vec![])], &with_entries()),
        finish(vec![prose("plan")])
    );
}

// Where a later return completes what stands, it checks it again; a call does not, so what the faults say is wrong of
// a block that is judged by that check is left out of what stands into a call.
#[test]
fn a_verdict_sent_back_for_no_point_or_an_answer_in_the_tree_with_no_tree_change_does_not_stand_into_a_call() {
    let no_point = Unmet::NoPoint {
        block: "verdict".into(),
        value: "no".to_string(),
        name: "review".into(),
    };
    let rounds = || {
        vec![told(
            vec![no_point.clone()],
            handed_in(vec![prose("plan"), valued("verdict", "no")]),
        )]
    };
    // Into a call, the verdict is not written and the plan is.
    let mut calls = rounds();
    calls.push(asks(1));
    assert_eq!(step(calls, &spec()), finish_asking_with(vec![prose("plan")]));
    // Completed by a return that asks for none, the verdict stands, for the check of the whole finds it again.
    let mut completed = rounds();
    completed.push(works(&[], vec![]));
    assert_eq!(
        step(completed, &spec()),
        finish(vec![prose("plan"), valued("verdict", "no")])
    );
    let unchanged = Unmet::TreeUnchanged {
        name: "which".to_string(),
        blocks: vec!["plan".into()],
    };
    let before = told(
        vec![unchanged],
        handed_in(vec![prose("plan"), valued("verdict", "yes")]),
    );
    assert_eq!(
        step(vec![before, asks(1)], &spec()),
        finish_asking_with(vec![valued("verdict", "yes")])
    );
}

// Reference: the same decision, for a chain: a return that asks for calls and was itself sent back leaves what stood
// before it standing for the next.
#[test]
fn what_stood_before_a_return_that_asks_for_calls_and_was_sent_back_stands_for_the_return_after_it() {
    let s = must_write(&["verdict"]);
    let first = told(vec![not_handed_in("verdict")], handed_in(vec![prose("plan")]));
    let tool = told(vec![Unmet::NotATool { name: "x".to_string() }], asks(1));
    // Completed by a return that asks for none: the plan of the first stands.
    assert_eq!(
        step(
            vec![first.clone(), tool.clone(), handed_in(vec![valued("verdict", "yes")])],
            &s
        ),
        finish(vec![prose("plan"), valued("verdict", "yes")])
    );
    // Ended by another call: the plan is written all the same.
    assert_eq!(
        step(vec![first, tool, asks(1)], &s),
        finish_asking_with(vec![prose("plan")])
    );
}

#[test]
fn a_block_a_fault_names_is_left_out_at_each_return_it_stands_through_to_a_call() {
    let no_point = Unmet::NoPoint {
        block: "verdict".into(),
        value: "no".to_string(),
        name: "review".into(),
    };
    let first = told(vec![no_point], handed_in(vec![prose("plan"), valued("verdict", "no")]));
    let tool = told(vec![Unmet::NotATool { name: "x".to_string() }], asks(1));
    // The verdict was left out as the first call return took what stood, and no fault of the return before the last
    // says it again.
    assert_eq!(
        step(vec![first, tool, asks(1)], &spec()),
        finish_asking_with(vec![prose("plan")])
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
    // A return that asked for calls with nothing standing before it leaves nothing to complete.
    let asked = carried(&[asks(2)], None);
    assert!((asked.writes.is_empty() && asked.dealt_with.is_empty()) && asked.questions.is_empty());
    // A round that failed after the return leaves it as it was.
    let after = [rounds[0].clone(), failed_as(Failure::Flagged)];
    assert_eq!(carried(&after, None), completes);
}

// Reference: decision a-sent-back-return-is-completed-by-what-is-missing (what a send-back does not name stands, what it
// names is left out and given again), and the send-back's own words, which tell the cell it must hold none of the blocks
// a `NothingBut` fault lists.
#[test]
fn a_return_sent_back_for_a_choice_that_allows_nothing_beside_it_gives_up_the_blocks_beside_it_too() {
    let wrong = vec![Unmet::NothingBut {
        block: "verdict".into(),
        value: "no".to_string(),
        besides: vec!["plan".into()],
    }];
    let before = || told(wrong.clone(), handed_in(vec![prose("plan"), valued("verdict", "no")]));
    // Given the verdict alone again, the plan the fault named does not stand.
    assert_eq!(
        step(vec![before(), works(&[], vec![valued("verdict", "no")])], &spec()),
        finish(vec![valued("verdict", "no")])
    );
    // Given nothing, neither the verdict nor the plan stands, and what the return dealt with does.
    assert_eq!(step(vec![before(), works(&[], vec![])], &spec()), finish(vec![]));
}

// Reference: the same decision, for a send-back after a send-back: each return completes the one before it, and a block
// that stood from the first return stands to the third.
#[test]
fn a_chain_of_returns_each_sent_back_for_another_fault_stands_as_the_blocks_none_of_the_faults_named() {
    let s = must_write(&["verdict"]);
    let first = told(vec![not_handed_in("verdict")], handed_in(vec![prose("plan")]));
    let second = told(
        vec![Unmet::NotAChoice {
            block: "verdict".into(),
        }],
        works(&[], vec![valued("verdict", "maybe")]),
    );
    // The second is not met by what it carries with the first, and the third gives the verdict again.
    assert_eq!(
        step(vec![first.clone(), works(&[], vec![valued("verdict", "no")])], &s),
        finish(vec![prose("plan"), valued("verdict", "no")])
    );
    assert_eq!(
        step(vec![first, second, works(&[], vec![valued("verdict", "no")])], &s),
        finish(vec![prose("plan"), valued("verdict", "no")])
    );
}

// Reference: design document `data-and-context`, 35, and decision a-return-says-what-it-dealt-with: a return that names
// the cell's own words as dealt with writes an answer in them; the next return completes it by giving that answer.
#[test]
fn a_return_sent_back_for_naming_the_owners_words_with_no_answer_is_completed_by_the_answer() {
    let mut s = must_write(&["verdict"]);
    s.outputs.push("spoken".into());
    s.words = Some("spoken".into());
    s.told.ran_by = vec!["request".into(), "spoken".into()];
    let unanswered = vec![Unmet::NotAnswered {
        name: "spoken".to_string(),
    }];
    let before = told(unanswered, works(&["spoken"], vec![]));
    assert_eq!(
        step(vec![before, works(&[], vec![prose("spoken")])], &s),
        Step::Finish {
            dealt_with: vec!["spoken".into()],
            writes: vec![prose("spoken")],
        }
    );
}
