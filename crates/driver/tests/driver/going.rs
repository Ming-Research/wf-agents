//! What the agents' rounds under way have come to so far, as what carries them out says it: kept by the work, the run
//! and the round while the round is under way, and let go of once it is not.

use driver::{Along, Going, Under};

use super::outage::two;
use super::*;

fn going(context: u64) -> Going {
    Going {
        context: Some(context),
        ..Going::default()
    }
}

// Each round under way is held by its work, run and round with what was said last of it, and the one whose work and
// round differ is held apart.
#[test]
fn what_each_round_under_way_has_come_to_is_held_by_its_work_run_and_round() {
    let (mut h, a, b) = two();
    let (run_a, _, round_a, _) = h.round(a);
    let (run_b, _, round_b, _) = h.round(b);
    let under = |run, round| Under::Round { run, round };
    assert!(h.d().rounds_come_to().is_empty());
    assert!(
        h.dm()
            .along(a, under(run_a, round_a), Some(Along::Going(going(10_000))))
    );
    assert!(
        h.dm()
            .along(b, under(run_b, round_b), Some(Along::Going(going(20_000))))
    );
    assert!(
        h.dm()
            .along(a, under(run_a, round_a), Some(Along::Going(going(12_000))))
    );
    let mut held = h.d().rounds_come_to();
    held.sort_by_key(|(work, ..)| *work);
    assert_eq!(
        held,
        vec![(a, run_a, round_a, going(12_000)), (b, run_b, round_b, going(20_000))]
    );
    // Said again as it was, it changes nothing.
    assert!(
        !h.dm()
            .along(a, under(run_a, round_a), Some(Along::Going(going(12_000))))
    );
}

// A round that has ended is held no more, though what carries it out never said it was done: the records say it is not
// under way.
#[test]
fn what_a_round_has_come_to_is_let_go_of_once_the_round_has_ended() {
    let (mut h, a, b) = two();
    let (run_a, _, round_a, _) = h.round(a);
    let (run_b, _, round_b, _) = h.round(b);
    for (work, run, round) in [(a, run_a, round_a), (b, run_b, round_b)] {
        h.dm()
            .along(work, Under::Round { run, round }, Some(Along::Going(going(7_000))));
    }
    assert_eq!(h.d().rounds_come_to().len(), 2);
    h.hands_in(a, &[("text", None)]);
    let held = h.d().rounds_come_to();
    assert_eq!(held.len(), 1, "only the round of the other work is under way: {held:?}");
    assert_eq!(held[0].0, b);
}

// A round that is not an agent's, whose place is where it waits or runs, is not among what the rounds have come to.
#[test]
fn a_round_that_says_where_it_waits_is_not_among_what_agents_rounds_have_come_to() {
    let (mut h, a, _) = two();
    let (run, _, round, _) = h.round(a);
    let changed = h
        .dm()
        .along(a, Under::Round { run, round }, Some(Along::Runs { at: "link".into() }));
    assert!(changed);
    assert!(h.d().rounds_come_to().is_empty());
}
