//! Answering an act: a return that deals with what an act of the engine's after its cell wrote leaves new what runs
//! that act, by what it writes or what it committed, or has a question stand where an answer runs the act; else it is
//! sent back, since the act reads no words and would never run again.

use super::*;

/// The cell, run by the request and by the check, which an act after it writes and which runs again on the work's head
/// and, where `question`, on the questions' standing; it may ask the owner.
fn answering(question: bool) -> Spec {
    let mut runs: Vec<BlockId> = vec!["head".into()];
    if question {
        runs.push("asked".into());
    }
    let s = spec();
    Spec {
        told: Told {
            ran_by: vec!["request".into(), "check".into()],
            ..s.told
        },
        answered_by: BTreeMap::from([("check".into(), Answer { runs, question })]),
        ..s
    }
}

/// A round whose return dealt with these, wrote these, committed these and asked these questions.
fn answer(dealt_with: &[&str], writes: Vec<Write>, read: &[&str], questions: Vec<Asking>) -> Round {
    Round {
        returned: Some(Return::Came {
            dealt_with: dealt_with.iter().map(|&b| b.into()).collect(),
            writes,
            calls: 0,
            found: vec![],
            waits: false,
            questions,
            read: read.iter().map(|&b| b.into()).collect(),
        }),
    }
}

/// A question the return asks, or withdraws.
fn asking(name: &str, withdrawn: bool) -> Asking {
    Asking {
        name: name.to_string(),
        withdrawn,
    }
}

fn finish_with(dealt_with: &[&str], writes: Vec<Write>) -> Step {
    Step::Finish {
        dealt_with: dealt_with.iter().map(|&b| b.into()).collect(),
        writes,
    }
}

#[test]
fn a_return_that_answers_the_check_with_nothing_new_is_sent_back() {
    let round = answer(&["request", "check"], vec![prose("plan")], &[], vec![]);
    let unmet = vec![Unmet::Unanswered { block: "check".into() }];
    assert_eq!(step(vec![round], &answering(true)), send_back(unmet));
}

#[test]
fn a_return_that_answers_the_check_with_a_new_head_finishes_the_run_writing_the_head_after_its_own_blocks() {
    let round = answer(&["request", "check"], vec![prose("plan")], &["head"], vec![]);
    let writes = vec![prose("plan"), prose("head")];
    assert_eq!(
        step(vec![round], &answering(true)),
        finish_with(&["request", "check"], writes)
    );
}

#[test]
fn a_return_that_answers_the_check_with_a_question_finishes_the_run_where_an_answer_runs_the_check() {
    let round = || answer(&["check"], vec![], &[], vec![asking("not_this_change", false)]);
    assert_eq!(step(vec![round()], &answering(true)), finish_with(&["check"], vec![]));
    let unmet = vec![Unmet::Unanswered { block: "check".into() }];
    assert_eq!(step(vec![round()], &answering(false)), send_back(unmet));
}

#[test]
fn a_question_of_the_cell_that_still_stands_answers_the_check_and_one_withdrawn_does_not() {
    let mut s = answering(true);
    s.told.questions = BTreeMap::from([(
        "not_this_change".to_string(),
        Asked {
            own: true,
            stands: true,
        },
    )]);
    let standing = answer(&["check"], vec![], &[], vec![]);
    assert_eq!(step(vec![standing], &s), finish_with(&["check"], vec![]));
    let withdrawn = answer(&["check"], vec![], &[], vec![asking("not_this_change", true)]);
    let unmet = vec![Unmet::Unanswered { block: "check".into() }];
    assert_eq!(step(vec![withdrawn], &s), send_back(unmet));
}

#[test]
fn a_return_that_leaves_the_check_undealt_with_is_not_held_to_answer_it() {
    let round = answer(&["request"], vec![prose("plan")], &[], vec![]);
    assert_eq!(
        step(vec![round], &answering(true)),
        finish_with(&["request"], vec![prose("plan")])
    );
}

#[test]
fn a_block_committed_that_does_not_run_the_act_does_not_answer_it() {
    let round = answer(&["check"], vec![], &["tree"], vec![]);
    let unmet = vec![Unmet::Unanswered { block: "check".into() }];
    assert_eq!(step(vec![round], &answering(true)), send_back(unmet));
}
