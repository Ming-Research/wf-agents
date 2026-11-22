//! The study flow as the repository defines it (`flow/study`), through `handle` only: a work that reads the design tree,
//! researches and hands in a conclusion, and ends with it. Its last cell is an act of the engine's own that requires
//! every question of the work to be answered, because a run that asks a question its cell waits for is a finished run:
//! were the cell that writes the conclusion the last, a work would end with no conclusion, and an answer other than the
//! recommended one would run nothing. What the owner's answers do is the engine's own (`questions.rs`); these tests
//! hold the flow to what it is for: the work ends with its conclusion and with nothing standing unanswered.

use std::path::Path;

use decide::{Decision, End, Waited};

use engine::Question;

use super::questions::asked;
use super::*;

/// The study flow as the repository defines it, compiled against what the engine has for it.
fn study() -> Compiled {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/study");
    let source = flow::read(&dir).expect("the study flow is read");
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
    let has = Has {
        sources: names(&[
            "answer",
            "asked",
            "dependencies",
            "handed_in",
            "session",
            "start",
            "tree",
            "words",
        ]),
        acts: names(&["start", "conclude"]),
        tools: names(&["query", "run_tests"]),
        ..Default::default()
    };
    compile(&source, &has).unwrap_or_else(|faults| panic!("the study flow does not compile: {faults:#?}"))
}

/// A work on the study flow whose study, its third cell, has its first round under way: the work and the study's run.
fn studying() -> (W, RunId) {
    let mut w = W {
        flow: study(),
        work: Work::default(),
        clock: START,
    };
    w.on(wrote("background", Outsider::Person, true, None));
    w.on(wrote("start", Outsider::Outside, true, None));
    let effects = w.on(wrote("dependencies", Outsider::Outside, true, Some("merged")));
    let (start, cell, ..) = round(&effects);
    assert_eq!(cell, "S1");
    let effects = w.returns(start, hands_in(&[("place", None)]));
    let (read, cell, ..) = round(&effects);
    assert_eq!(cell, "R0");
    let effects = w.returns(read, Came::hands_in(Vec::new()));
    let (run, cell, ..) = round(&effects);
    assert_eq!(cell, "R1");
    (w, run)
}

/// What the study hands in: its conclusion, asking these questions beside it.
fn concludes(questions: Vec<Question>) -> Came {
    let mut came = hands_in(&[("conclusion", None)]);
    came.said.questions = questions;
    came
}

/// The study asks these questions and hands in no conclusion, as a return that waits for an answer may.
fn asks(questions: Vec<Question>) -> Came {
    let mut came = Came::works(Vec::new(), Vec::new());
    came.said.questions = questions;
    came
}

/// The cells whose runs finished, in the order they finished.
fn finished_cells(w: &W) -> Vec<String> {
    let cell = |run: RunId| {
        (w.work.records.started.iter())
            .find(|s| s.run == run)
            .map(|s| s.cell.to_string())
            .expect("the run started")
    };
    w.work.records.finished.iter().map(|f| cell(f.run)).collect()
}

/// The act that concludes the study is given to be done, and the work ends as it hands in what it writes, whereupon
/// what the work held outside the engine is let go.
fn ends_at_the_act(w: &mut W, effects: &[Effect]) {
    let (run, cell, ..) = round(effects);
    assert_eq!(cell, "C1");
    let released = Effect::Release { end: End::Merged };
    assert_eq!(w.returns(run, hands_in(&[("concluded", None)])), [released]);
    assert_eq!(w.decision(), Decision::Ended(End::Merged));
}

#[test]
fn a_study_ends_with_its_conclusion_at_the_act_that_concludes_it() {
    let (mut w, run) = studying();
    let effects = w.returns(run, concludes(Vec::new()));
    ends_at_the_act(&mut w, &effects);
    assert_eq!(finished_cells(&w), ["S1", "R0", "R1", "C1"]);
}

#[test]
fn a_study_that_asks_a_question_it_waits_for_and_hands_in_no_conclusion_does_not_end_until_it_has_one() {
    let (mut w, run) = studying();
    let effects = w.returns(run, asks(vec![asked("which", true)]));
    assert_eq!(effects, [], "nothing begins");
    let waits = Decision::Wait {
        at: "R1".into(),
        on: vec![Waited::Answer {
            question: "which".to_string(),
        }],
    };
    assert_eq!(w.decision(), waits, "the run finished, and the work waits at the study");
    // The answer runs the study again, which hands in its conclusion; the act then ends the work.
    let version = w.version("which");
    let effects = w.answers("which", version, 1);
    let (again, cell, ..) = round(&effects);
    assert_eq!(cell, "R1");
    let effects = w.returns(again, concludes(Vec::new()));
    ends_at_the_act(&mut w, &effects);
    assert_eq!(finished_cells(&w), ["S1", "R0", "R1", "R1", "C1"]);
}

#[test]
fn a_conclusion_handed_in_with_a_question_the_study_waits_for_does_not_end_the_work_before_the_answer() {
    let (mut w, run) = studying();
    let effects = w.returns(run, concludes(vec![asked("which", true)]));
    assert_eq!(effects, [], "the act does not begin");
    assert!(matches!(w.decision(), Decision::Wait { at, .. } if at.as_str() == "R1"));
    let version = w.version("which");
    let effects = w.answers("which", version, 2);
    let (again, cell, ..) = round(&effects);
    assert_eq!(cell, "R1", "the study runs again for the answer, before the act");
    let effects = w.returns(again, concludes(Vec::new()));
    ends_at_the_act(&mut w, &effects);
}

#[test]
fn a_question_that_does_not_wait_holds_the_act_and_an_answer_other_than_the_recommended_one_runs_the_study_first() {
    // The recommended option runs nothing, and the act begins.
    let (mut w, run) = studying();
    let effects = w.returns(run, concludes(vec![asked("which", false)]));
    assert_eq!(effects, [], "the act waits for the answer");
    assert!(matches!(w.decision(), Decision::Wait { at, .. } if at.as_str() == "C1"));
    let version = w.version("which");
    let effects = w.answers("which", version, 1);
    ends_at_the_act(&mut w, &effects);
    // Another option runs the study again before the act, whose conclusion the act then ends the work with.
    let (mut w, run) = studying();
    w.returns(run, concludes(vec![asked("which", false)]));
    let version = w.version("which");
    let effects = w.answers("which", version, 2);
    let (again, cell, ..) = round(&effects);
    assert_eq!(cell, "R1");
    let effects = w.returns(again, concludes(Vec::new()));
    ends_at_the_act(&mut w, &effects);
}

#[test]
fn a_study_that_hands_in_no_conclusion_and_asks_nothing_it_waits_for_is_sent_back_and_does_not_end() {
    let (mut w, run) = studying();
    let effects = w.returns(run, Came::hands_in(Vec::new()));
    let (_, cell, _, given) = round(&effects);
    assert_eq!(cell, "R1");
    assert_eq!(
        given,
        Given::SentBack(vec![Unmet::NotHandedIn {
            block: "conclusion".into()
        }])
    );
    assert!(!matches!(w.decision(), Decision::Ended(_)));
}
