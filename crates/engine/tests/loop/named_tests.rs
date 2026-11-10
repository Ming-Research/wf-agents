//! The named tests: each walk asks them anew, of the records as they are then, so that a condition or a precondition
//! is decided on what was written a moment before, within the same thing that happened.

use super::*;

/// A cell that writes a choice; a cell whose condition is a named test of that choice; and the merge.
fn forked() -> Compiled {
    let flow = r#"
        name = "forked"
        order = ["ask", "extra", "merge"]
        [limits]
        runs = 3
        returns = 2
        calls = 3
        [lines.main]
        [blocks.request]
        people = true
        [blocks.choice]
        values = ["yes", "no"]
        [blocks.notes]
        [tests.more]
        says = "The asker wants more"
        kind = "any"
        reads = ["choice"]
        is = "yes"
    "#;
    let ask = "title = \"Ask\"\nkind = \"agent\"\nline = \"main\"\ninputs = [\"request\"]\noutputs = [\"choice\"]\n";
    let extra = "title = \"Extra\"\nkind = \"agent\"\nline = \"main\"\nwhen = { test = \"more\" }\n\
                 inputs = [\"request\"]\noutputs = [\"notes\"]\n";
    let merge = "title = \"Merge\"\nkind = \"engine\"\ninputs = [\"notes\"]\n";
    let source = Source {
        flow: flow.to_string(),
        cells: [("ask", ask), ("extra", extra), ("merge", merge)]
            .iter()
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect(),
    };
    compile(&source, &Default::default()).expect("the definition compiles")
}

/// The cell whose round the engine gives when the asker's run finishes having chosen this.
fn after_choosing(value: &str) -> String {
    let flow = forked();
    let mut work = Work::default();
    let request = wrote("request", Outsider::Person, true, None);
    let (ask, cell, ..) = round(&handle(&flow, &mut work, Time(START), request).expect("taken"));
    assert_eq!(cell, "ask");
    let returned = Event::Returned {
        run: ask,
        came: hands_in(&[("choice", Some(value))]),
        spent: spent(1),
    };
    let effects = handle(&flow, &mut work, Time(START + 1000), returned).expect("taken");
    assert_eq!(work.records.finished.len(), 1, "the asker's run finished");
    round(&effects).1
}

#[test]
fn a_condition_by_a_named_test_is_decided_on_the_choice_a_run_wrote_as_it_finished_in_the_same_call() {
    assert_eq!(after_choosing("yes"), "extra");
    assert_eq!(after_choosing("no"), "merge");
}

#[test]
fn a_precondition_by_a_named_test_holds_as_soon_as_what_it_reads_is_written_and_not_while_it_says_otherwise() {
    let (mut w, write, _) = W::requested();
    let judge = round(&w.returns(write, hands_in(&[("text", None)]))).0;
    assert_eq!(w.returns(judge, hands_in(&[("verdict", Some("yes"))])), vec![]);
    let word = decide::Waited::Precondition {
        test: "may_merge".into(),
        who: Who::Owner,
    };
    let waits = decide::Decision::Wait {
        at: "merge".into(),
        on: vec![word],
    };
    assert_eq!(w.decision(), waits);
    assert_eq!(w.on(wrote("go", Outsider::Person, true, Some("no"))), vec![]);
    assert_eq!(w.decision(), waits);
    let (_, cell, number, given) = round(&w.lets_merge());
    assert_eq!((cell.as_str(), number, given), ("merge", 1, Given::Task));
}
