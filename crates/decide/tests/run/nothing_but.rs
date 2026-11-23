//! A value that stands with nothing but some blocks (`with_nothing_but` of its choice's file): a return that writes the
//! choice with it holds none of the other blocks its cell may write, its questions and its words among them, and is
//! sent back saying the value and what else it held.

use super::*;

/// The cell, which may also write what it found, ask in its questions and answer the owner in its words: its verdict,
/// at yes, stands with nothing but what it found.
fn alone() -> Spec {
    let mut s = spec();
    s.outputs.extend(["found".into(), "asking".into(), "spoken".into()]);
    s.nothing_but = BTreeMap::from([(
        "verdict".into(),
        BTreeMap::from([("yes".to_string(), vec!["found".into()])]),
    )]);
    s.questions = Some("asking".into());
    s.words = Some("spoken".into());
    s
}

/// What the return is sent back for: the verdict at yes, and these besides.
fn besides(blocks: &[&str]) -> Vec<Unmet> {
    vec![Unmet::NothingBut {
        block: "verdict".into(),
        value: "yes".to_string(),
        besides: blocks.iter().map(|&b| b.into()).collect(),
    }]
}

#[test]
fn a_return_with_the_value_and_a_question_is_sent_back_naming_the_value_and_the_questions() {
    let s = alone();
    let writes = vec![valued("verdict", "yes"), prose("found")];
    for waits in [true, false] {
        let round = asks_beside(&["request", "opinions"], writes.clone(), waits);
        assert_eq!(step(vec![round], &s), send_back(besides(&["asking"])), "waits: {waits}");
    }
    // A question withdrawn is written in the questions too.
    let withdrawn = Round {
        returned: Some(Return::Came {
            dealt_with: vec!["request".into(), "opinions".into()],
            writes,
            calls: 0,
            found: vec![],
            waits: false,
            questions: vec![Asking {
                name: "which".to_string(),
                withdrawn: true,
                waits: false,
            }],
            read: Vec::new(),
            protected: Default::default(),
        }),
        sent_back: Vec::new(),
    };
    let mut told = s;
    told.told.questions = BTreeMap::from([(
        "which".to_string(),
        Asked {
            own: true,
            stands: true,
            unanswered: true,
        },
    )]);
    assert_eq!(step(vec![withdrawn], &told), send_back(besides(&["asking"])));
}

#[test]
fn a_return_with_the_value_and_a_plan_or_an_answer_is_sent_back_naming_each_block_it_holds_besides_once() {
    let s = alone();
    let round = handed_in(vec![
        valued("verdict", "yes"),
        prose("plan"),
        prose("found"),
        prose("spoken"),
    ]);
    assert_eq!(step(vec![round], &s), send_back(besides(&["plan", "spoken"])));
    // With a question as well, the questions come last; a block written twice is named once, and is said twice too.
    let round = asks_beside(
        &["request", "opinions"],
        vec![valued("verdict", "yes"), prose("plan"), prose("plan")],
        true,
    );
    let mut unmet = vec![Unmet::WrittenTwice { block: "plan".into() }];
    unmet.extend(besides(&["plan", "asking"]));
    assert_eq!(step(vec![round], &s), send_back(unmet));
}

#[test]
fn a_block_the_cell_may_not_write_is_said_only_not_to_be_an_output_and_not_held_besides() {
    let unmet = vec![Unmet::NotAnOutput { block: "code".into() }];
    let round = handed_in(vec![valued("verdict", "yes"), prose("code")]);
    assert_eq!(step(vec![round], &alone()), send_back(unmet));
}

#[test]
fn a_return_with_the_value_and_nothing_but_what_it_names_finishes_the_run() {
    let s = alone();
    for writes in [
        vec![valued("verdict", "yes"), prose("found")],
        vec![valued("verdict", "yes")],
    ] {
        assert_eq!(step(vec![handed_in(writes.clone())], &s), finish(writes));
    }
}

#[test]
fn another_value_stands_with_whatever_the_cell_may_write() {
    let writes = vec![valued("verdict", "no"), prose("plan"), prose("found"), prose("spoken")];
    let round = asks_beside(&["request", "opinions"], writes.clone(), true);
    assert_eq!(step(vec![round], &alone()), finish(writes));
}
