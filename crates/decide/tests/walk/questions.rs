//! A question a cell waits for holds the work at the cell (design document `data-and-context`, 33, 36): the cell is
//! entered for nothing and no later cell runs while it stands with no answer to its latest version.

use super::*;

/// A research that the brief and the owner's answers to it run, writing the plan, and an implementation of the plan;
/// the research ran once and asked the question `which`, waiting for it or not. Returns the question's version.
fn asked(waits: bool) -> (World, Seq) {
    let mut w = World::with_gate(vec![
        cell("research", &["brief", "research answered"], &["plan"]),
        cell("implement", &["plan"], &["code"]),
    ]);
    w.turning("research answered");
    w.person_writes("brief");
    w.run("research", &[("plan", Matters)]);
    let version = w.questions("research", "which", waits);
    (w, version)
}

/// The wait at the cell for the owner's answer to the question.
fn waits_for(at: &str, question: &str) -> Decision {
    Decision::Wait {
        at: at.into(),
        on: vec![Waited::Answer {
            question: question.to_string(),
        }],
    }
}

#[test]
fn a_question_the_cell_waits_for_holds_the_work_at_the_cell_and_no_later_cell_runs() {
    let (w, _) = asked(true);
    assert_eq!(w.decide(), waits_for("research", "which"));
}

#[test]
fn the_cell_is_entered_for_nothing_new_while_its_question_stands_unanswered() {
    let (mut w, _) = asked(true);
    w.person_writes("brief");
    assert_eq!(w.decide(), waits_for("research", "which"));
}

#[test]
fn the_answer_lets_the_cell_be_entered_and_the_work_go_on() {
    let (mut w, version) = asked(true);
    w.answers("which", version, 2);
    w.outside_writes("research answered");
    let Decision::Run { cell, because, .. } = w.decide() else {
        panic!("nothing runs: {:?}", w.decide())
    };
    assert_eq!(cell.as_str(), "research");
    assert_eq!(because, vec![changed("research answered", None, false)]);
    w.start("research");
    w.finish("research", &[]);
    assert!(matches!(w.decide(), Decision::Run { cell, .. } if cell.as_str() == "implement"));
}

#[test]
fn an_answer_to_an_earlier_version_of_the_question_answers_it_no_more() {
    let (mut w, first) = asked(true);
    w.answers("which", first, 1);
    w.questions("research", "which", true);
    assert_eq!(w.decide(), waits_for("research", "which"));
    w.answers("which", first, 2);
    assert_eq!(w.decide(), waits_for("research", "which"));
}

#[test]
fn a_question_withdrawn_holds_the_work_no_more() {
    let (mut w, _) = asked(true);
    w.withdraws("research", "which");
    assert!(matches!(w.decide(), Decision::Run { cell, .. } if cell.as_str() == "implement"));
}

#[test]
fn a_question_the_cell_does_not_wait_for_holds_nothing() {
    let (w, _) = asked(false);
    assert!(matches!(w.decide(), Decision::Run { cell, .. } if cell.as_str() == "implement"));
}

#[test]
fn a_question_of_a_later_cell_holds_the_work_at_that_cell_and_lets_an_earlier_one_run() {
    let (mut w, _) = asked(false);
    w.run("implement", &[("code", Matters)]);
    w.questions("implement", "how", true);
    assert_eq!(w.decide(), waits_for("implement", "how"));
    // The research runs for what is new to it; the implementation's question still holds the work there after it.
    w.person_writes("brief");
    w.run("research", &[("plan", DoesNotMatter)]);
    assert_eq!(w.decide(), waits_for("implement", "how"));
}

#[test]
fn an_answer_to_a_version_no_question_has_is_a_fault_of_the_records() {
    let (mut w, version) = asked(true);
    let at = w.answers("which", Seq(version.0 + 100), 1);
    assert_eq!(w.decide(), Decision::Fault(RecordFault::AnswerNoVersion(at)));
}

#[test]
fn the_owner_speaking_to_a_cell_held_by_its_question_enters_it_and_it_goes_on_waiting() {
    let mut w = World::with_gate(vec![
        cell(
            "research",
            &["brief", "research answered", "spoken to research"],
            &["plan"],
        ),
        cell("implement", &["plan"], &["code"]),
    ]);
    w.turning("research answered");
    w.turning("spoken to research");
    w.person_writes("brief");
    w.run("research", &[("plan", Matters)]);
    w.questions("research", "which", true);
    // What is not the owner's turning enters it for nothing.
    w.person_writes("brief");
    assert_eq!(w.decide(), waits_for("research", "which"));
    w.outside_writes("spoken to research");
    let Decision::Run { cell, because, .. } = w.decide() else {
        panic!("nothing runs: {:?}", w.decide())
    };
    assert_eq!(cell.as_str(), "research");
    assert_eq!(because, vec![changed("spoken to research", None, false)]);
    // The round answers the owner and is held to nothing its cell must write, since the cell still waits.
    let answering = w.start("research");
    let told = decide::told(&w.flow, &w.records, answering).expect("the run started");
    assert!(told.waits);
    w.finish("research", &[]);
    assert_eq!(w.decide(), waits_for("research", "which"));
}

#[test]
fn the_cell_spoken_to_is_the_one_of_the_line_the_work_reached_last_of_those_that_apply() {
    let mut w = World::with_gate(vec![
        cell("research", &["brief"], &["plan"]),
        cell("deep", &["plan"], &["solution"]),
        cell("implement", &["plan"], &["code"]),
    ]);
    let line: Vec<decide::CellId> = vec!["research".into(), "implement".into()];
    // Before any of them ran, the first of them.
    assert_eq!(decide::spoken_to(&w.flow, &w.records, &line), Some("research".into()));
    w.person_writes("brief");
    w.run("research", &[("plan", Matters)]);
    assert_eq!(decide::spoken_to(&w.flow, &w.records, &line), Some("research".into()));
    w.run("deep", &[("solution", Matters)]);
    w.start("implement");
    assert_eq!(decide::spoken_to(&w.flow, &w.records, &line), Some("implement".into()));
    // Of none of the line's cells, none.
    assert_eq!(decide::spoken_to(&w.flow, &w.records, &[]), None);
}
