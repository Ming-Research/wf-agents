//! A small work taken through the loop: what the engine gives to be done at each thing that happens.

use std::collections::{BTreeMap, BTreeSet};

use decide::{Cause, Decision, Outcome, Outsider, TestName, Tests, Waited, Who, decide};
use engine::{Came, Effect, Event, Given, Handed, Spent, Time, Work, handle, pending};
use flow::{Compiled, Source, compile};

/// A writer, a judge that may write an opinion for the writer, and a merge that waits for the owner's word.
fn small() -> Compiled {
    let flow = r#"
        name = "small"
        order = ["write", "judge", "merge"]
        [limits]
        runs = 3
        returns = 2
        calls = 3
        [lines.main]
        [lines.other]
        [tests]
        may_merge = "The work may merge"
        [blocks.request]
        people = true
        [blocks.opinions]
        people = true
        [blocks.text]
        [blocks.content]
        values = ["yes", "no"]
        shown = true
        [blocks.merged]
        shown = true
    "#;
    let write = r#"
        title = "Write"
        kind = "agent"
        line = "main"
        inputs = ["request", "opinions"]
        outputs = ["text"]
    "#;
    let judge = r#"
        title = "Judge"
        kind = "agent"
        line = "other"
        inputs = ["text"]
        outputs = ["opinions", "content"]
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        requires = ["may_merge"]
        inputs = ["text"]
        outputs = ["merged"]
    "#;
    let cells = [("write", write), ("judge", judge), ("merge", merge)];
    let source = Source {
        flow: flow.to_string(),
        cells: cells
            .iter()
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect(),
    };
    let known: BTreeSet<TestName> = ["may_merge".into()].into();
    compile(&source, &known).expect("the definition compiles")
}

/// The named tests: the merge waits for the owner until the test says otherwise.
struct Named(BTreeMap<TestName, Outcome>);

impl Tests for Named {
    fn test(&self, name: &TestName) -> Outcome {
        self.0.get(name).cloned().unwrap_or(Outcome::Holds)
    }
}

fn waiting() -> Named {
    Named([("may_merge".into(), Outcome::Waits { who: Who::Owner })].into())
}

fn hands_in(block: &str, value: Option<&str>) -> Came {
    Came::HandsIn(vec![Handed {
        block: block.into(),
        content: format!("the {block}"),
        matters: true,
        value: value.map(str::to_string),
    }])
}

/// The one round the engine gives to be done: the run, the cell and what the round is given.
fn round(effects: &[Effect]) -> (decide::RunId, String, Given) {
    match effects {
        [Effect::Round { run, cell, given, .. }] => (*run, cell.0.clone(), given.clone()),
        other => panic!("not one round: {other:?}"),
    }
}

#[test]
fn a_work_goes_from_cell_to_cell_as_each_hands_in_and_rests_at_the_merge_for_the_owner() {
    let (flow, tests, mut work) = (small(), waiting(), Work::default());
    let request = Event::Wrote {
        block: "request".into(),
        by: Outsider::Person,
        content: "do it".to_string(),
        matters: true,
        value: None,
    };
    let (write, cell, given) = round(&handle(&flow, &tests, &mut work, Time(1), request).unwrap());
    assert_eq!(cell, "write");
    assert_eq!(given, Given::Task);
    assert_eq!(work.records.started[0].because, [Cause::NoRecord]);
    // While the round is under way it is what a driver that starts anew is given again.
    assert_eq!(round(&pending(&work)), (write, cell, given));

    let came = hands_in("text", None);
    let (judge, cell, _) = round(
        &handle(
            &flow,
            &tests,
            &mut work,
            Time(2),
            Event::Returned {
                run: write,
                came,
                spent: Spent::default(),
            },
        )
        .unwrap(),
    );
    assert_eq!(cell, "judge");

    let came = hands_in("content", Some("yes"));
    let effects = handle(
        &flow,
        &tests,
        &mut work,
        Time(3),
        Event::Returned {
            run: judge,
            came,
            spent: Spent::default(),
        },
    )
    .unwrap();
    assert_eq!(effects, vec![]);
    assert_eq!(pending(&work), vec![]);
    let word = Waited::Precondition {
        test: "may_merge".into(),
        who: Who::Owner,
    };
    let rests = Decision::Wait {
        at: "merge".into(),
        on: vec![word],
    };
    assert_eq!(decide(&flow.flow, &work.records, &tests), rests);
}

#[test]
fn what_is_handed_in_that_the_flow_does_not_allow_is_sent_back_with_why() {
    let (flow, tests, mut work) = (small(), waiting(), Work::default());
    let (write, ..) = round(&handle(&flow, &tests, &mut work, Time(4), Event::Look).unwrap());
    // The writer hands in the judge's block.
    let came = hands_in("content", Some("yes"));
    let (again, cell, given) = round(
        &handle(
            &flow,
            &tests,
            &mut work,
            Time(5),
            Event::Returned {
                run: write,
                came,
                spent: Spent::default(),
            },
        )
        .unwrap(),
    );
    assert_eq!((again, cell.as_str()), (write, "write"));
    let unmet = decide::run::Unmet::NotAnOutput {
        block: "content".into(),
    };
    assert_eq!(given, Given::SentBack(vec![unmet]));
}
