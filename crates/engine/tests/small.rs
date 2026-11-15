//! A small work taken through the loop: what the engine gives to be done at each thing that happens.

use decide::{Cause, Decision, Maker, Outsider, Waited, decide};
use engine::{Came, Effect, Event, Given, Handed, Spent, Time, Work, handle, pending};
use flow::{Compiled, Has, Source, compile};

/// A writer, a judge that may write an opinion for the writer, and a merge that waits for the owner's word: a choice
/// people write, which a named test reads.
fn small() -> Compiled {
    let flow = r#"
        name = "small"
        order = ["write", "judge", "merge"]
        [limits]
        again = 2
        [lines.main]
        [lines.other]
        [tests.may_merge]
        says = "The owner let the work merge"
        kind = "all"
        reads = ["go"]
        is = "yes"
        [blocks.go]
        values = ["yes", "no"]
        people = true
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
        effort = "high"
        line = "main"
        task = "Write the text the request asks for."
        inputs = ["request", "opinions"]
        outputs = ["text"]
    "#;
    let judge = r#"
        title = "Judge"
        kind = "agent"
        effort = "high"
        line = "other"
        task = "Judge the text."
        inputs = ["text"]
        outputs = ["opinions", "content"]
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        act = "merge"
        requires = { test = "may_merge" }
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
        ..Default::default()
    };
    let has = Has {
        acts: ["merge".to_string()].into(),
        ..Has::default()
    };
    compile(&source, &has).expect("the definition compiles")
}

fn hands_in(block: &str, value: Option<&str>) -> Came {
    Came::hands_in(vec![Handed {
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
    let (flow, mut work) = (small(), Work::default());
    let request = Event::Wrote {
        block: "request".into(),
        by: Outsider::Person,
        content: "do it".to_string(),
        matters: true,
        value: None,
    };
    let (write, cell, given) = round(&handle(&flow, &mut work, Time(1), request).unwrap());
    assert_eq!(cell, "write");
    assert_eq!(given, Given::Task);
    let because = Cause::Changed {
        block: "request".into(),
        was: None,
        behind: false,
    };
    assert_eq!(work.records.started[0].because, [because]);
    // While the round is under way it is what a driver that starts anew is given again.
    assert_eq!(round(&pending(&flow, &work)), (write, cell, given));

    let came = hands_in("text", None);
    let (judge, cell, _) = round(
        &handle(
            &flow,
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
    assert_eq!(pending(&flow, &work), vec![]);
    let word = Waited::Precondition {
        on: "may_merge".into(),
        by: vec![Maker::Owner],
    };
    let rests = Decision::Wait {
        at: "merge".into(),
        on: vec![word],
    };
    assert_eq!(decide(&flow.flow, &work.records), rests);
}

#[test]
fn what_is_handed_in_that_the_flow_does_not_allow_is_sent_back_with_why() {
    let (flow, mut work) = (small(), Work::default());
    // Nothing runs the writer until the request is written.
    assert_eq!(handle(&flow, &mut work, Time(3), Event::Look).unwrap(), vec![]);
    let request = Event::Wrote {
        block: "request".into(),
        by: Outsider::Person,
        content: "do it".to_string(),
        matters: true,
        value: None,
    };
    let (write, ..) = round(&handle(&flow, &mut work, Time(4), request).unwrap());
    // The writer hands in the judge's block.
    let came = hands_in("content", Some("yes"));
    let (again, cell, given) = round(
        &handle(
            &flow,
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

// A block handed in holding nothing, where it holds nothing yet, takes nothing away and is no write: no version of it is
// kept, and the cell that takes it is not entered for it (decision a-write-of-nothing-only-empties). Work 3's initial
// research wrote `found` so where its task said to write none.
#[test]
fn a_block_handed_in_holding_nothing_where_it_holds_nothing_keeps_no_version_and_runs_no_cell() {
    let (flow, mut work) = (small(), Work::default());
    let request = Event::Wrote {
        block: "request".into(),
        by: Outsider::Person,
        content: "do it".to_string(),
        matters: true,
        value: None,
    };
    let (write, ..) = round(&handle(&flow, &mut work, Time(1), request).unwrap());
    let came = Came::hands_in(vec![Handed {
        block: "text".into(),
        content: " \n".to_string(),
        matters: true,
        value: None,
    }]);
    let returned = Event::Returned {
        run: write,
        came,
        spent: Spent::default(),
    };
    let effects = handle(&flow, &mut work, Time(2), returned).unwrap();
    assert!(
        !effects.iter().any(|e| matches!(e, Effect::Round { .. })),
        "a cell is entered for nothing: {effects:?}"
    );
    let finished = (work.records.finished.iter()).find(|f| f.run == write);
    assert_eq!(finished.map(|f| f.writes.clone()), Some(vec![]));
    assert!(
        !work.contents.keys().any(|(block, _)| block.as_str() == "text"),
        "{:?}",
        work.contents
    );
}
