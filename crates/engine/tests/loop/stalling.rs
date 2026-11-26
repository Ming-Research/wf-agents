//! A work that can go on by itself no further, as the deciding code finds it stalled, is left as it is: the engine
//! records nothing for it, gives nothing to do and pauses nothing, so that what it shows is worked out from the walk
//! each time; and whatever runs a cell, from the owner or from outside, moves it on at once.

use super::*;

/// A writer of the text a request asks for, which reads main as well; a check of the text, which alone writes whether
/// it passed; and a merge, the engine's own cell, that waits for the check to say yes.
fn checked() -> Compiled {
    let flow = r#"
        name = "checked"
        order = ["write", "check", "merge"]
        [limits]
        runs = 3
        returns = 2
        calls = 3
        [lines.main]
        [blocks.request]
        people = true
        [blocks.main]
        outside = "main"
        [blocks.text]
        [blocks.checked]
        values = ["yes", "no"]
        [blocks.merged]
        shown = true
    "#;
    let write = r#"
        title = "Write"
        kind = "agent"
        effort = "high"
        line = "main"
        task = "Write the text the request asks for."
        watches = ["request", "main"]
        outputs = ["text"]

        [kinds]
        request = "last_and_latest"
        main = "last_and_latest"
    "#;
    let check = r#"
        title = "Check"
        kind = "agent"
        effort = "high"
        line = "main"
        task = "Check the text."
        watches = ["text"]
        outputs = ["checked"]

        [kinds]
        text = "last_and_latest"
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        act = "merge"
        requires = { checked = "yes" }
        watches = ["checked"]
        outputs = ["merged"]
    "#;
    let source = Source {
        flow: flow.to_string(),
        cells: [("write", write), ("check", check), ("merge", merge)]
            .iter()
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect(),
        ..Default::default()
    };
    compile(&source, &has(&["main"])).expect("the definition compiles")
}

/// A work on the checked flow whose writer wrote the text and whose check dealt with it and handed in nothing: the work
/// is stalled. The check's one output is a choice, so no block of the check, written, would end the stall, and its
/// return is taken.
fn stalled() -> W {
    let mut w = W {
        flow: checked(),
        ..W::new()
    };
    let (effects, _) = w.person_writes("request");
    let (write, ..) = round(&effects);
    let (check, cell, ..) = round(&w.returns(write, hands_in(&[("text", None)])));
    assert_eq!(cell, "check");
    assert_eq!(w.returns(check, hands_in(&[])), vec![]);
    w
}

/// The work stalled at the merge for want of the check's yes, which the check, with nothing new, does not write.
fn at_the_merge() -> decide::Decision {
    decide::Decision::Stalled {
        at: "merge".into(),
        on: vec![decide::Condition::Is {
            block: "checked".into(),
            value: "yes".into(),
        }],
        by: vec![decide::Stalling {
            cell: "check".into(),
            why: decide::Unrun::NothingNew,
        }],
    }
}

#[test]
fn a_return_that_would_leave_the_work_stalled_is_sent_back_naming_the_block_that_would_end_it() {
    let mut w = W {
        flow: checked(),
        ..W::new()
    };
    let (effects, _) = w.person_writes("request");
    let (write, ..) = round(&effects);
    // The writer deals with the request and hands in no text, so that the check never runs and the merge waits for ever.
    let effects = w.returns(write, hands_in(&[]));
    assert_eq!(
        round(&effects),
        (
            write,
            "write".to_string(),
            2,
            Given::SentBack(vec![Unmet::WouldStall {
                blocks: vec!["text".into()]
            }])
        )
    );
    // Handing in the text, the work can go on: the check runs.
    let (check, cell, ..) = round(&w.returns(write, hands_in(&[("text", None)])));
    assert_eq!(cell, "check");
    assert_ne!(check, write);
}

#[test]
fn a_stalled_work_is_left_as_it_is_and_paused_by_nothing() {
    let mut w = stalled();
    assert_eq!(w.decision(), at_the_merge());
    assert_eq!(w.work.records.pauses, vec![]);
    let started = w.work.records.started.clone();
    // Looked at again, nothing is added and nothing is to be done.
    assert_eq!(w.on(Event::Look), vec![]);
    assert_eq!(w.work.records.started, started);
    assert_eq!(w.work.records.pauses, vec![]);
}

#[test]
fn whatever_runs_a_cell_moves_a_stalled_work_on_at_once() {
    let mut w = stalled();
    // Main moves: the writer runs on it.
    let effects = w.on(wrote("main", Outsider::Outside, true, None));
    let (_, cell, number, given) = round(&effects);
    assert_eq!((cell.as_str(), number, given), ("write", 1, Given::Task));
    assert!(
        !matches!(w.decision(), decide::Decision::Stalled { .. }),
        "{:?}",
        w.decision()
    );
}
