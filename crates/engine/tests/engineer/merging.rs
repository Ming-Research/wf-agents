//! Single works on the engineer's flow taken through `handle` step by step, each to what its merge rests on: the check
//! waits for the review's yes, whatever main does, so that no branch the review has not seen is checked or merged; the
//! work's head, which the review takes and which runs the check, holds the merge to the head the review saw; the
//! implementation answers a red check by what runs the check again, a commit or a question, and a return that answers
//! it with neither is sent back; and the owner's answer to a question the check asked, which only the implementation is
//! told, runs the check again by itself, so that the answer alone leads to the merge.

use decide::run::{Failure, Unmet};
use decide::{BlockId, CellId, Decision, End, Outsider, Seq, Waited};
use engine::{Call, Effect, Event, Given, Question, Spent, Way, pending};

use super::single::{W, wrote};

/// What each judge the work's head runs dealt with last of it, as the check holds the head to them, by the judge.
fn seen(w: &W) -> Vec<(String, Option<String>)> {
    let head = BlockId::from("head");
    let judges = decide::judges(&w.flow.flow, &w.work.records, &CellId::from("M1"), &head);
    (judges.into_iter())
        .map(|judge| {
            let said = judge
                .consumed
                .and_then(|at| w.work.contents.get(&(head.clone(), at)).cloned());
            (judge.cell.0, said)
        })
        .collect()
}

/// The work from its start to the review's yes on the implementation's code, which its round committed at the head
/// `c1`: the research hands in a plan with no deeper look and no change of the tree, and the review asks for no
/// testing.
fn reviewed_at_c1() -> W {
    let mut w = W::started();
    w.hands_in("S1", &[("place", None)], Vec::new());
    w.hands_in("R0", &[], Vec::new());
    w.hands_in(
        "R1",
        &[
            ("deep_needed", Some("no")),
            ("edits_tree", Some("no")),
            ("plan_owner", None),
            ("plan_agent", None),
            ("acceptance", None),
        ],
        Vec::new(),
    );
    w.hands_in("R2", &[("better_found", Some("no"))], Vec::new());
    w.commits("I1", &[("code", None), ("evidence", None)], &[("head", "c1")]);
    w.hands_in(
        "T1",
        &[("review_ok", Some("yes")), ("needs_testing", Some("no"))],
        Vec::new(),
    );
    assert_eq!(seen(&w), [("T1".to_string(), Some("c1".to_string()))]);
    w
}

// W9's course: the review saw the change at one head; the check found main in conflict; the implementation merged main
// in, resolving the conflict, and handed in nothing but what it committed. Its new head runs the review and the check,
// and the review comes before the check, so that the merge lands only the head it saw. Run by what names a commit, the
// check passed and the merge landed a change of a design decision no review had seen.
#[test]
fn a_fix_run_by_a_red_check_reaches_the_review_and_the_check_through_the_head() {
    let mut w = reviewed_at_c1();
    w.hands_in("M1", &[("checked", Some("no")), ("check", None)], Vec::new());
    w.commits("I1", &[], &[("head", "c2")]);
    assert_eq!(w.round().1, "T1", "the review runs before the check");
    assert_eq!(seen(&w), [("T1".to_string(), Some("c1".to_string()))]);
    w.hands_in(
        "T1",
        &[("review_ok", Some("yes")), ("needs_testing", Some("no"))],
        Vec::new(),
    );
    assert_eq!(seen(&w), [("T1".to_string(), Some("c2".to_string()))]);
    assert_eq!(w.round().1, "M1", "the check runs on the head the review saw");
    w.hands_in("M1", &[("checked", Some("yes"))], Vec::new());
    w.hands_in("M2", &[("merged", None)], Vec::new());
    assert_eq!(w.decision(), Decision::Ended(End::Merged));
}

// The check reads no words: a return that deals with it and commits nothing and asks nothing would leave nothing to run
// it again, and is sent back. The round given again commits its fix, and the work goes on to the review.
#[test]
fn a_return_that_answers_the_check_with_nothing_new_is_sent_back() {
    let mut w = reviewed_at_c1();
    w.hands_in("M1", &[("checked", Some("no")), ("check", None)], Vec::new());
    let (run, cell) = w.round();
    assert_eq!(cell, "I1");
    assert_eq!(w.told(run, "I1"), ["on_check"]);
    let effects = w.returns("I1", &["on_check"], &[("code", None)]);
    let unmet = (effects.iter()).find_map(|e| match e {
        Effect::Round {
            run: r,
            given: Given::SentBack(unmet),
            ..
        } if *r == run => Some(unmet.clone()),
        _ => None,
    });
    assert_eq!(unmet, Some(vec![Unmet::Unanswered { block: "check".into() }]));
    w.commits("I1", &[("code", None)], &[("head", "c2")]);
    assert_eq!(w.round().1, "T1", "the fix runs the review");
}

// A commit no round handed in, as a person's, is read by the check from outside for its round, which fails in passing
// as the review has not seen it: the review runs on it first, and the check is taken again after its wait.
#[test]
fn a_head_the_check_reads_that_the_review_has_not_seen_runs_the_review_before_the_check_again() {
    let mut w = reviewed_at_c1();
    let (run, cell) = w.round();
    assert_eq!(cell, "M1");
    w.on(Event::WroteForRound {
        run,
        block: "head".into(),
        content: "c3".to_string(),
    });
    w.on(Event::Failed {
        run,
        why: "The work's head is c3, which the review has not seen.".to_string(),
        failure: Failure::Passing,
        resets: None,
        spent: Spent::default(),
    });
    assert_eq!(w.round().1, "T1", "the review runs first, on the head the check read");
    w.hands_in(
        "T1",
        &[("review_ok", Some("yes")), ("needs_testing", Some("no"))],
        Vec::new(),
    );
    assert_eq!(seen(&w), [("T1".to_string(), Some("c3".to_string()))]);
    let wait = (pending(&w.flow, &w.work).into_iter())
        .find_map(|effect| match effect {
            Effect::Call {
                call,
                what: Call::Wait { .. },
                ..
            } => Some(call),
            _ => None,
        })
        .unwrap_or_else(|| panic!("the check does not wait: {:?}", w.decision()));
    w.on(Event::Done {
        call: wait,
        result: "waited".to_string(),
    });
    let (again, cell) = w.round();
    assert_eq!((again, cell.as_str()), (run, "M1"), "the check's run is taken again");
    w.hands_in("M1", &[("checked", Some("yes"))], Vec::new());
    w.hands_in("M2", &[("merged", None)], Vec::new());
    assert_eq!(w.decision(), Decision::Ended(End::Merged));
}

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
        reasons: Default::default(),
    }
}

#[test]
fn the_check_waits_for_the_reviews_yes_whatever_main_does() {
    let mut w = W::started();
    w.hands_in("S1", &[("place", None)], Vec::new());
    w.hands_in("R0", &[], Vec::new());
    // The research hands in its choices alone: nothing is built, and the review never runs.
    w.hands_in("R1", &[("deep_needed", Some("no"))], Vec::new());
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

// A check held because what fails also fails on main says that what it wrote waits for main: the work waits for main,
// with the check's run and its choice, and not for the owner; main moving runs the check again.
#[test]
fn a_check_held_for_main_has_the_work_wait_for_main_until_main_moves() {
    let mut w = W::reviewed();
    let (run, cell) = w.round();
    assert_eq!(cell, "M1");
    let held = engine::Handed {
        block: "checked".into(),
        content: "It did not pass, and what fails also fails on main.".to_string(),
        matters: true,
        value: Some("no".to_string()),
    };
    let came = engine::Came {
        waits_for: Some("main".into()),
        ..engine::Came::hands_in(vec![held])
    };
    w.on(Event::Returned {
        run,
        came,
        spent: engine::Spent::default(),
    });
    let outside = Waited::Outside {
        source: "main".to_string(),
        run,
        block: "checked".into(),
    };
    assert_eq!(
        w.decision(),
        Decision::Wait {
            at: "M2".into(),
            on: vec![outside]
        }
    );
    w.on(wrote("main", Outsider::Outside, None));
    assert_eq!(w.round().1, "M1");
}
