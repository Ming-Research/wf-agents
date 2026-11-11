//! A test of what was confirmed, through the loop: the engine tells the deciding code which versions were written
//! blank, so that a block that says nothing holds no work, and a person's confirmation lets the work on.

use super::*;

/// A plan that may put questions, and a merge that waits until the owner confirmed them.
fn asking() -> Compiled {
    let flow = r#"
        name = "asking"
        order = ["plan", "merge"]
        [limits]
        runs = 3
        returns = 2
        calls = 3
        [lines.main]
        [blocks.request]
        people = true
        [blocks.questions]
        shown = true
        [blocks.code]
        [blocks.answered]
        values = ["yes"]
        people = true
        [tests.answered]
        says = "The owner answered the questions, or said the recommendations stand"
        kind = "confirmed"
        reads = ["questions"]
        by = "answered"
        is = "yes"
    "#;
    let plan = "title = \"Plan\"\nkind = \"agent\"\neffort = \"high\"\nline = \"main\"\ntask = \"Plan, asking what you \
                must.\"\ninputs = [\"request\"]\noutputs = [\"questions\", \"code\"]\n";
    let merge =
        "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\nrequires = [\"answered\"]\ninputs = [\"code\"]\n";
    let source = Source {
        flow: flow.to_string(),
        cells: [("plan", plan), ("merge", merge)]
            .iter()
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect(),
    };
    compile(&source, &has(&[])).expect("the definition compiles")
}

/// The effects of the plan's run finishing, having handed in the questions with this content, and the work.
fn planned(questions: &str) -> (Vec<Effect>, Work) {
    let flow = asking();
    let mut work = Work::default();
    let request = wrote("request", Outsider::Person, true, None);
    let (plan, cell, ..) = round(&handle(&flow, &mut work, Time(START), request).expect("taken"));
    assert_eq!(cell, "plan");
    let mut asked = handed("questions", None);
    asked.content = questions.to_string();
    let returned = Event::Returned {
        run: plan,
        came: Came::HandsIn(vec![asked, handed("code", None)]),
        spent: spent(1),
    };
    let effects = handle(&flow, &mut work, Time(START + 1000), returned).expect("taken");
    assert_eq!(work.records.finished.len(), 1, "the plan's run finished");
    (effects, work)
}

#[test]
fn questions_hold_the_merge_until_a_person_confirms_them() {
    let (effects, mut work) = planned("[q1] Which way? Recommended: the first.");
    assert!(work.records.blank.is_empty());
    assert!(effects.is_empty(), "the merge waits: {effects:?}");
    let yes = wrote("answered", Outsider::Person, true, Some("yes"));
    let effects = handle(&asking(), &mut work, Time(START + 2000), yes).expect("taken");
    assert_eq!(round(&effects).1, "merge");
}

#[test]
fn questions_handed_in_blank_are_recorded_so_and_hold_nothing() {
    let (effects, work) = planned(" \n");
    let finished = work.records.finished[0].at;
    assert_eq!(work.records.blank, [("questions".into(), finished)].into());
    assert_eq!(round(&effects).1, "merge");
}

#[test]
fn a_person_writing_a_block_blank_is_recorded_so() {
    let (_, mut work) = planned("[q1] Which way?");
    let blank = Event::Wrote {
        block: "request".into(),
        by: Outsider::Person,
        content: String::new(),
        matters: true,
        value: None,
    };
    handle(&asking(), &mut work, Time(START + 2000), blank).expect("taken");
    let at = work.records.written.last().expect("written").at;
    assert!(work.records.blank.contains(&("request".into(), at)));
}
