//! The questions put to the owner as they stand, read from the records whatever order their entries are listed in, as
//! a store may give them back: a question's latest version is the one of the latest place.

use decide::{Answered, CellId, Questioned, Records, RunId, Seq};

/// A version of the question `naming`, of the plan's run 1, written at this place, waiting for its answer or not.
fn version(at: u64, waits: bool) -> Questioned {
    Questioned {
        name: "naming".to_string(),
        cell: CellId::from("plan"),
        run: RunId(1),
        engine: false,
        waits,
        withdrawn: false,
        at: Seq(at),
    }
}

#[test]
fn a_questions_latest_version_is_the_one_of_the_latest_place_whatever_order_its_entries_are_listed_in() {
    let records = Records {
        // The later version listed first, and the owner's answer to it.
        questions: vec![version(5, false), version(2, true)],
        answers: vec![Answered {
            question: "naming".to_string(),
            version: Seq(5),
            option: 1,
            why: None,
            at: Seq(6),
        }],
        ..Records::default()
    };
    let now = reading::questions(&records, Seq(u64::MAX));
    let latest = &now["naming"];
    assert_eq!((latest.at, latest.waits, latest.answered), (Seq(5), false, true));
    // Before the later version was written, the one before it stood, waiting for its answer.
    let then = reading::questions(&records, Seq(5));
    let earlier = &then["naming"];
    assert_eq!((earlier.at, earlier.waits, earlier.answered), (Seq(2), true, false));
}
