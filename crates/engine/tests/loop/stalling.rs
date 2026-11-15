//! A work that can go on by itself no further, as the deciding code finds it stalled, is left as it is: the engine
//! records nothing for it, gives nothing to do and pauses nothing, so that what it shows is worked out from the walk
//! each time; and whatever runs a cell, from the owner or from outside, moves it on at once.

use super::*;

/// The stalling flow beside the test flows (the flow crate's `tests/laid_out/stalled`): a writer of the text a request
/// asks for, which reads main as well; a check of the text, which alone writes whether it passed; and a merge, the
/// engine's own cell, that waits for the check to say yes.
fn checked() -> Compiled {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../flow/tests/laid_out/stalled");
    let source = flow::read(&dir).expect("the stalling flow is read");
    compile(&source, &has(&["main"])).expect("the definition compiles")
}

/// A work on the checked flow whose writer dealt with the request and handed in nothing: the work is stalled.
fn stalled() -> W {
    let mut w = W {
        flow: checked(),
        ..W::new()
    };
    let (effects, _) = w.person_writes("request");
    let (write, ..) = round(&effects);
    assert_eq!(w.returns(write, hands_in(&[])), vec![]);
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
