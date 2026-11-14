//! Single works on the engineer's flow taken through `handle` step by step, each to what its merge rests on: the check
//! waits for the review's yes, whatever main does, so that no branch the review has not seen is checked or merged; and
//! the owner's answer to a question the check asked, which only the implementation is told, runs the check again by
//! itself, so that the answer alone leads to the merge.

use decide::{Decision, End, Outsider, Seq, Waited};
use engine::{Event, Question, Way};

use super::single::{W, wrote};

/// The question the check asks of a change to a file the project protects, as the engine asks it.
fn protected() -> Question {
    let way = |does: &str| Way {
        does: does.to_string(),
        follows: format!("what follows from {does}"),
    };
    Question::Asked {
        name: "protected_files".to_string(),
        waits: true,
        recommends: None,
        ask: "Whether this work's change to files that govern the agents may merge.".to_string(),
        options: vec![way("Let it merge"), way("Take it back")],
        about: Some("AGENTS.md".to_string()),
    }
}

#[test]
fn the_check_waits_for_the_reviews_yes_whatever_main_does() {
    let mut w = W::started();
    w.hands_in("S1", &[("place", None)], Vec::new());
    w.hands_in("R0", &[], Vec::new());
    // The research hands in its choices alone: nothing is built, and the review never runs.
    w.hands_in(
        "R1",
        &[("deep_needed", Some("no")), ("edits_tree", Some("no"))],
        Vec::new(),
    );
    for _ in 0..3 {
        let Decision::Wait { at, on } = w.decision() else {
            panic!("the work does not wait: {:?}", w.decision());
        };
        assert_eq!(at.as_str(), "M1");
        assert!(
            on.iter().any(|waited| matches!(
                waited,
                Waited::Precondition { on, .. } if *on == decide::Condition::Is {
                    block: "review_ok".into(),
                    value: "yes".into(),
                }
            )),
            "{on:?}"
        );
        // Main moves, as the watch writes it: the check does not run.
        assert_eq!(w.on(wrote("main", Outsider::Outside, None)), vec![]);
        assert!(
            !(w.work.records.started.iter()).any(|s| s.cell.as_str() == "M1"),
            "the check ran before the review said yes"
        );
    }
}

#[test]
fn the_owners_answer_alone_to_the_checks_question_leads_to_the_merge() {
    let mut w = W::reviewed();
    // The check finds a change to a protected file the owner has not let merge, and asks the owner.
    w.hands_in("M1", &[("checked", Some("no")), ("check", None)], vec![protected()]);
    // The implementation is told of the failed check, and has nothing to do: what failed is the owner's to decide.
    w.hands_in("I1", &[], Vec::new());
    let Decision::Wait { at, on } = w.decision() else {
        panic!("the work does not wait for the answer: {:?}", w.decision());
    };
    assert_eq!(at.as_str(), "M1");
    assert!(
        on.iter().any(|waited| matches!(waited, Waited::Answer { .. })),
        "{on:?}"
    );
    let version: Seq = (w.work.records.questions.iter())
        .filter(|q| q.name == "protected_files")
        .map(|q| q.at)
        .max()
        .expect("the question was asked");
    // The owner lets the change merge: the implementation is told, and again has nothing to do.
    w.on(Event::Answer {
        question: "protected_files".to_string(),
        version,
        option: 1,
        why: None,
    });
    w.hands_in("I1", &[], Vec::new());
    // The answer runs the check again by itself, which passes now, and the merge follows.
    w.hands_in("M1", &[("checked", Some("yes"))], Vec::new());
    w.hands_in("M2", &[("merged", None)], Vec::new());
    assert_eq!(w.decision(), Decision::Ended(End::Merged));
}
