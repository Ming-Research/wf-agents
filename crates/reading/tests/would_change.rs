//! What the reading says a write of a run's return would change (`Reading::would_change`): each test builds a small
//! work, as the walk's own tests do, and asks the reading about a write the return hands in. What each expects is
//! taken from the design tree (`data-and-context`, 9), never from the deciding code, which the histories hold to agree.

// The tests use only part of the helper the tests of the deciding code share.
#[allow(dead_code)]
#[path = "../../decide/tests/support/mod.rs"]
mod support;

use decide::{PutIn, Write};
use reading::Reading;
use support::{World, cell};

/// A write of the block that the return hands in as changing nothing in meaning.
fn unchanged(block: &str) -> Write {
    Write {
        block: block.into(),
        meaning_unchanged: true,
        value: None,
        empty: false,
    }
}

#[test]
fn a_write_that_changes_nothing_says_nothing_over_the_version_its_round_was_told_of() {
    let mut w = World::with_gate(vec![cell("first", &["brief"], &["b0"])]);
    w.person_writes("brief");
    let run = w.start("first");
    // A person writes prose into the block while the round is under way, which the round was not told of.
    let version = w.person_writes("b0");
    let would_change = |w: &World| Reading::new(&w.flow, &w.records).would_change(run, &unchanged("b0"));
    assert!(
        would_change(&w),
        "it was handed in over a version its round was not told of"
    );

    // The return is sent back for writing over it, and the version is put in to the round: the retry says what it says
    // over that version, so a write that changes nothing says nothing.
    let at = w.tick();
    w.records.put_in.push(PutIn {
        run,
        block: "b0".into(),
        version,
        at,
    });
    assert!(
        !would_change(&w),
        "it was handed in over the version its round was told of"
    );

    // A write that says it matters changes the block whatever the round was told of.
    let mut matters = unchanged("b0");
    matters.meaning_unchanged = false;
    assert!(Reading::new(&w.flow, &w.records).would_change(run, &matters));
}
