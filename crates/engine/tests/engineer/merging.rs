//! Single works on the engineer's flow taken through `handle` step by step, each to what its merge rests on: the check
//! waits for the reviews' yes, whatever main does, so that no branch the reviews have not seen is checked or merged; the
//! work's head, which the reviews take and which runs the check, holds the merge to the head the reviews saw; the
//! implementation answers a red check by what runs the check again, a commit or a question, and a return that answers
//! it with neither is sent back; the owner's answer to a question the check asked, which only the implementation is
//! told, runs the check again by itself, so that the answer alone leads to the merge; and a question of a protected
//! change asked early, as a round of the implementation ends, holds the check and not the work, which goes by the
//! option it recommends until the owner answers.

use std::path::Path;

use decide::run::{Failure, Unmet};
use decide::{BlockId, CellId, Decision, End, Outsider, Waited};
use engine::{Call, Came, Effect, Event, Given, Question, Spent, Way, pending};
use flow::agents::{AskProtected, fill};

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

/// Each of the three reviews of the change, the judges the work's head runs, having dealt last with this head.
fn all_saw(head: &str) -> Vec<(String, Option<String>)> {
    (["T1", "T2", "T3"].iter())
        .map(|judge| (judge.to_string(), Some(head.to_string())))
        .collect()
}

/// The work from its start to the reviews' yes on the implementation's code, which its round committed at the head
/// `c1`: planned ([`W::planned`]), and the test review asks for no testing.
fn reviewed_at_c1() -> W {
    let mut w = W::planned();
    w.commits("I1", &[("code", None), ("evidence", None)], &[("head", "c1")]);
    w.reviews_pass();
    assert_eq!(seen(&w), all_saw("c1"));
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
    assert_eq!(w.round().1, "T1", "the reviews run before the check");
    assert_eq!(seen(&w), all_saw("c1"));
    w.reviews_pass();
    assert_eq!(seen(&w), all_saw("c2"));
    assert_eq!(w.round().1, "M1", "the check runs on the head the reviews saw");
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
    assert_eq!(w.told(run, "I1"), ["check"]);
    let effects = w.returns("I1", &["check"], &[("code", None)]);
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
        hindrance: None,
        spent: Spent::default(),
    });
    assert_eq!(w.round().1, "T1", "the reviews run first, on the head the check read");
    w.reviews_pass();
    assert_eq!(seen(&w), all_saw("c3"));
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

/// The words of the question the engine asks of a change to a file the project protects, as `flow/agents/asks.toml`
/// has them.
fn protected_words() -> AskProtected {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
    let agents = flow::agents::read(&root).expect("the words beside the flows are read");
    agents.asks.protected
}

/// The question the engine asks of a change of `AGENTS.md`, which the work says why of, as it asks it, in the words
/// of `flow/agents/asks.toml`: holding the work where the check before the merge asks it, and not where it is asked
/// early, as a round of the implementation ends; recommending the option `merges` names, which lets the change merge.
fn protected(waits: bool) -> Question {
    let words = protected_words();
    let why = "The guide must say it.";
    let file = fill(&words.file, &[("path", "AGENTS.md"), ("why", why), ("diff", "+Rules.")]);
    Question::Asked {
        name: words.name,
        waits,
        tree: None,
        recommends: Some(words.merges),
        ask: fill(&words.ask, &[("files", file.as_str())]),
        options: (words.options.iter())
            .map(|o| Way {
                does: o.does.clone(),
                follows: o.follows.clone(),
            })
            .collect(),
        about: Some("AGENTS.md 0b5e".to_string()),
        reasons: [("AGENTS.md".to_string(), why.to_string())].into(),
    }
}

/// The number of the latest version of the question of this name, by which the owner answers it.
fn version(w: &W, name: &str) -> u64 {
    let questions = decide::questions::questions(&w.work.records);
    let asked = questions.get(name).expect("the question was asked");
    super::latest_number(&w.flow, &w.work.records, asked)
}

/// How many runs of the cell began.
fn runs(w: &W, cell: &str) -> usize {
    (w.work.records.started.iter())
        .filter(|s| s.cell.as_str() == cell)
        .count()
}

// Asked early, as a round of the implementation ends, the question of a protected change does not hold the work, which
// goes on by the option the question recommends, the one that lets the change merge (design document
// `data-and-context`, 31 and 34): the owner taking it while the review runs changes nothing the work goes by, tells no
// round and runs no cell again, and the check follows the review. Taking the change back tells the review's round under
// way to hand in what it has, and the work goes back to the implementation, which made the change.
#[test]
fn an_early_protected_question_answered_as_it_recommends_stops_nothing_and_taken_back_returns_to_the_implementation() {
    let words = protected_words();
    let other = if words.merges == 1 { 2 } else { 1 };
    for option in [words.merges, other] {
        let mut w = W::planned();
        w.hands_in("I1", &[("code", None), ("evidence", None)], vec![protected(false)]);
        let (review, cell) = w.round();
        assert_eq!(cell, "T1", "asked early, the question does not hold the work");
        let implementing = runs(&w, "I1");
        let answer = Event::Answer {
            question: words.name.clone(),
            version: version(&w, &words.name),
            option,
        };
        let effects = w.on(answer);
        let stopped = effects.contains(&Effect::Stop { run: review });
        let told = (effects.iter()).find_map(|e| match e {
            Effect::Tell { run, telling } if *run == review => Some(telling.versions.clone()),
            _ => None,
        });
        assert!(
            !stopped,
            "the review, an agent's round, is told and not stopped: {effects:?}"
        );
        if option == words.merges {
            assert!(told.is_none(), "the option it recommends tells no round: {effects:?}");
            assert_eq!(w.round(), (review, "T1".to_string()), "the review goes on");
            w.reviews_pass();
            assert_eq!(w.round().1, "M1", "the check follows the reviews");
            assert_eq!(
                (runs(&w, "I1"), runs(&w, "T1")),
                (implementing, 1),
                "no cell runs again"
            );
        } else {
            let versions = told.expect("taking the change back tells the review to hand in what it has");
            w.on(Event::SaidToRound { run: review, versions });
            w.on(Event::Returned {
                run: review,
                came: Came::hands_in(Vec::new()),
                spent: Spent::default(),
            });
            let (run, cell) = w.round();
            assert_eq!(cell, "I1", "the work goes back to the implementation");
            assert_eq!(w.told(run, "I1"), ["i1_questions"]);
        }
    }
}

// Asked early, the question lets the work go on to the review's yes, and the check waits while it stands unanswered;
// the owner's answer letting the change merge runs the check, and no cell before it.
#[test]
fn the_check_waits_for_the_answer_to_an_early_protected_question() {
    let words = protected_words();
    let mut w = W::planned();
    w.hands_in("I1", &[("code", None), ("evidence", None)], vec![protected(false)]);
    w.reviews_pass();
    let Decision::Wait { at, on } = w.decision() else {
        panic!("the work does not wait: {:?}", w.decision());
    };
    assert_eq!(at.as_str(), "M1");
    let unanswered = decide::Condition::Is {
        block: "asked".into(),
        value: "answered".into(),
    };
    assert!(
        on.iter()
            .any(|waited| matches!(waited, Waited::Precondition { on, .. } if *on == unanswered)),
        "{on:?}"
    );
    let implementing = runs(&w, "I1");
    w.on(Event::Answer {
        question: words.name.clone(),
        version: version(&w, &words.name),
        option: words.merges,
    });
    assert_eq!(w.round().1, "M1", "the answer runs the check");
    assert_eq!(runs(&w, "I1"), implementing, "the implementation does not run again");
}

// The owner rules the plan no while the tree's review is judging, as the owner may in a judge's place: the work waits
// before the implementation for the plan's verdicts (decision the-implementation-waits-for-the-plans-verdicts), and the
// check, behind it, runs on no move of main. The tree's review, whose return could not end that wait, is taken.
#[test]
fn the_check_waits_for_the_reviews_yes_whatever_main_does() {
    let mut w = W::started();
    w.hands_in("S1", &[("place", None)], Vec::new());
    w.hands_in("R0", &[("tree_entries", None)], Vec::new());
    w.hands_in(
        "R1",
        &[
            ("deep_needed", Some("no")),
            ("plan_owner", None),
            ("plan_agent", None),
            ("acceptance", None),
        ],
        Vec::new(),
    );
    w.hands_in("R2", &[], Vec::new());
    w.hands_in("P1", &[("plan_ok", Some("yes"))], Vec::new());
    w.on(wrote("plan_ok", Outsider::Person, Some("no")));
    w.hands_in("P2", &[("tree_ok", Some("yes"))], Vec::new());
    for _ in 0..3 {
        let Decision::Wait { at, on } = w.decision() else {
            panic!("the work does not wait: {:?}", w.decision());
        };
        assert_eq!(at.as_str(), "I1");
        assert!(
            on.iter().any(|waited| matches!(
                waited,
                Waited::Precondition { on, .. } if *on == decide::Condition::Is {
                    block: "plan_ok".into(),
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
    // The check finds a change to a protected file the owner has not let merge, and asks the owner, in the questions of
    // the implementation, which the flow names for what the engine asks: the work waits there for the answer, the
    // implementation entered for nothing else meanwhile.
    w.hands_in("M1", &[("checked", Some("no")), ("check", None)], vec![protected(true)]);
    let Decision::Wait { at, on } = w.decision() else {
        panic!("the work does not wait for the answer: {:?}", w.decision());
    };
    assert_eq!(at.as_str(), "I1");
    assert!(
        on.iter().any(|waited| matches!(waited, Waited::Answer { .. })),
        "{on:?}"
    );
    let version = version(&w, "protected_files");
    // The owner lets the change merge: the implementation is told of the answer and of the failed check, and has nothing
    // to do: what failed was the owner's to decide.
    w.on(Event::Answer {
        question: "protected_files".to_string(),
        version,
        option: 1,
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
