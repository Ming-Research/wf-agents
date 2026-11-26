//! What the agents' rounds under way have come to so far, as what carries them out says it: kept by the work, the run
//! and the round while the round is under way, and let go of once it is not.

use driver::{Along, Going, Under};

use super::outage::two;

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
    let at = h.now();
    assert!(h.d().rounds_come_to().is_empty());
    assert!(
        h.dm()
            .along(at, a, under(run_a, round_a), Some(Along::Going(going(10_000))))
    );
    assert!(
        h.dm()
            .along(at, b, under(run_b, round_b), Some(Along::Going(going(20_000))))
    );
    assert!(
        h.dm()
            .along(at, a, under(run_a, round_a), Some(Along::Going(going(12_000))))
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
            .along(at, a, under(run_a, round_a), Some(Along::Going(going(12_000))))
    );
}

// A round that has ended is held no more, though what carries it out never said it was done: the records say it is not
// under way.
#[test]
fn what_a_round_has_come_to_is_let_go_of_once_the_round_has_ended() {
    let (mut h, a, b) = two();
    let (run_a, _, round_a, _) = h.round(a);
    let (run_b, _, round_b, _) = h.round(b);
    let at = h.now();
    for (work, run, round) in [(a, run_a, round_a), (b, run_b, round_b)] {
        h.dm()
            .along(at, work, Under::Round { run, round }, Some(Along::Going(going(7_000))));
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
    let at = h.now();
    let changed = h.dm().along(
        at,
        a,
        Under::Round { run, round },
        Some(Along::Runs {
            at: "link".into(),
            queued_for: None,
        }),
    );
    assert!(changed);
    assert!(h.d().rounds_come_to().is_empty());
}

// A round that waits for a runner has waited since the first time what carries it out said so for that runner, as long
// as it goes on saying so; once it says it runs, or names another runner, its wait begins again from the time it next
// says so. Fails where each report resets the time, where one that runs keeps it, or where a round that ended keeps it.
#[test]
fn a_round_has_waited_for_a_runner_since_it_first_said_so_for_that_runner() {
    let (mut h, a, _) = two();
    let (run, _, round, _) = h.round(a);
    let under = Under::Round { run, round };
    let runs = |queued: Option<&str>| {
        Some(Along::Runs {
            at: "link".into(),
            queued_for: queued.map(str::to_string),
        })
    };
    let first = h.now();
    h.dm().along(first, a, under, runs(Some("14900k")));
    let later = h.now();
    h.dm().along(later, a, under, runs(Some("14900k")));
    assert_eq!(h.d().queued_of(a), vec![(under, "14900k", first)]);
    h.dm().along(later, a, under, runs(None));
    assert_eq!(h.d().queued_of(a), vec![], "a run under way waits for no runner");
    let again = h.now();
    h.dm().along(again, a, under, runs(Some("14900k")));
    let other = h.now();
    h.dm().along(other, a, under, runs(Some("github")));
    assert_eq!(h.d().queued_of(a), vec![(under, "github", other)]);
    h.hands_in(a, &[("text", None)]);
    assert_eq!(h.d().queued_of(a), vec![], "the round has ended");
}
