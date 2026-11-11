//! A wait every work shares: a round of an agent that failed for a cause no one work has holds the agents' rounds of
//! every work, as one fact worked out from all their records, until the wait is over: at the usage limit's reset, at
//! a round that tries once in a while and comes back, or, without the network, once a look finds it reached. The
//! engine's own acts go on meanwhile, and no work is paused for it.

use decide::run::Failure;
use driver::outage::{PROBE, UNKNOWN_RESET};
use driver::{Until, Wake};

use super::*;

/// The round of the work under way fails for a cause of this kind, its usage limit resetting at `resets`.
fn fails(h: &mut H, work: WorkNumber, failure: Failure, resets: Option<Time>) -> Vec<Todo> {
    let (run, ..) = h.round(work);
    let spent = h.spent();
    let event = Event::Failed {
        run,
        why: format!("it failed: {failure:?}"),
        failure,
        resets,
        spent,
    };
    h.on(work, event)
}

/// Two works of the plain flow, let on, each with its writer's first round under way.
fn two() -> (H, WorkNumber, WorkNumber) {
    let mut h = H::new();
    let (_, a) = h.add(new_item("a", &[], "plain"));
    let (_, b) = h.add(new_item("b", &[], "plain"));
    h.let_on(a);
    h.let_on(b);
    (h, a, b)
}

/// The rounds of agents' cells given among these.
fn agents_rounds(todos: &[Todo]) -> Vec<(WorkNumber, String, usize)> {
    (todos.iter())
        .filter_map(|t| match &t.effect {
            Effect::Round { cell, number, .. } if cell.as_str() != "merge" => Some((t.work, cell.to_string(), *number)),
            _ => None,
        })
        .collect()
}

#[test]
fn at_the_usage_limit_no_agents_round_of_any_work_begins_until_it_resets_and_no_work_is_paused() {
    let (mut h, a, b) = two();
    let resets = Time(h.clock + 3_600_000);
    assert_eq!(fails(&mut h, a, Failure::UsageLimit, Some(resets)), vec![]);
    assert!(
        !decide::paused(&h.d().work(a).unwrap().work.records),
        "the limit pauses no work"
    );
    let outage = h.d().outage().expect("the wait stands");
    assert_eq!(
        (outage.failure, outage.work, outage.resets),
        (Failure::UsageLimit, a, Some(resets))
    );
    assert_eq!(outage.until(), Until::At(resets));
    assert_eq!(
        h.d().wake(),
        Some(Wake {
            at: resets,
            look: false
        })
    );
    // The other work's round, under way as the limit was met, ends; its merge, the engine's own act, goes on.
    let todos = h.hands_in(b, &[("text", None)]);
    assert!(matches!(&todos[..], [Todo { effect: Effect::Round { cell, .. }, .. }] if cell.as_str() == "merge"));
    assert!(
        h.d().outage().is_some(),
        "a round that began before the limit was met lifts nothing"
    );
    // Before the reset nothing begins; at it, the work whose round failed has that round again.
    assert_eq!(h.tick(Time(resets.0 - 1), None), vec![]);
    let todos = h.tick(resets, None);
    assert_eq!(agents_rounds(&todos), vec![(a, "write".to_string(), 2)]);
    // It comes back: the wait is over.
    h.hands_in(a, &[("text", None)]);
    assert_eq!(h.d().outage(), None);
    assert_eq!(h.d().wake(), None);
}

#[test]
fn a_usage_limit_whose_reset_was_not_said_is_waited_out_half_an_hour() {
    let (mut h, a, _) = two();
    fails(&mut h, a, Failure::UsageLimit, None);
    let outage = h.d().outage().expect("the wait stands");
    assert_eq!(outage.until(), Until::At(Time(outage.since.0 + UNKNOWN_RESET)));
}

#[test]
fn where_the_command_line_cannot_run_one_round_tries_every_half_minute_and_one_that_comes_back_lets_every_work_on() {
    let (mut h, a, b) = two();
    fails(&mut h, a, Failure::CannotRun, None);
    let since = h.d().outage().expect("the wait stands").since;
    assert_eq!(
        h.d().wake(),
        Some(Wake {
            at: Time(since.0 + PROBE),
            look: false
        })
    );
    // The other work's round, under way, fails so too: the wait counts from it.
    fails(&mut h, b, Failure::CannotRun, None);
    let since = h.d().outage().expect("the wait stands").since;
    assert_eq!(h.d().outage().unwrap().work, b);
    // At the time, one round tries, of the first work to come to one; the other waits for it.
    let todos = h.tick(Time(since.0 + PROBE), None);
    assert_eq!(agents_rounds(&todos), vec![(a, "write".to_string(), 2)]);
    assert_eq!(h.d().outage().unwrap().until(), Until::Tried);
    assert_eq!(h.d().wake(), None);
    // It fails so again: the next tries half a minute after it.
    fails(&mut h, a, Failure::CannotRun, None);
    let since = h.d().outage().expect("the wait stands").since;
    let todos = h.tick(Time(since.0 + PROBE), None);
    assert_eq!(agents_rounds(&todos), vec![(a, "write".to_string(), 3)]);
    // It comes back: every work goes on, in the same step.
    let todos = h.hands_in(a, &[("text", None)]);
    assert_eq!(h.d().outage(), None);
    assert!(
        agents_rounds(&todos).contains(&(b, "write".to_string(), 2)),
        "{todos:?}"
    );
}

#[test]
fn without_the_network_it_is_looked_at_every_half_minute_and_a_round_tries_once_it_is_reached() {
    let (mut h, a, _) = two();
    fails(&mut h, a, Failure::Offline, None);
    let since = h.d().outage().expect("the wait stands").since;
    let first = Time(since.0 + PROBE);
    assert_eq!(h.d().wake(), Some(Wake { at: first, look: true }));
    // A look that does not reach it begins nothing, and the next look is half a minute later.
    assert_eq!(h.tick(first, Some(false)), vec![]);
    assert_eq!(
        h.d().wake(),
        Some(Wake {
            at: Time(first.0 + PROBE),
            look: true
        })
    );
    let todos = h.tick(Time(first.0 + PROBE), Some(true));
    assert_eq!(agents_rounds(&todos), vec![(a, "write".to_string(), 2)]);
}

#[test]
fn without_the_network_a_round_tries_every_five_minutes_whatever_the_looks_find() {
    let (mut h, a, _) = two();
    fails(&mut h, a, Failure::Offline, None);
    let outage = h.d().outage().expect("the wait stands");
    let Until::Trial { latest, .. } = outage.until() else {
        panic!("{:?}", outage.until());
    };
    let mut at = Time(outage.since.0 + PROBE);
    while at < latest {
        assert_eq!(h.tick(at, Some(false)), vec![]);
        at = Time(at.0 + PROBE);
    }
    assert_eq!(
        h.d().wake(),
        Some(Wake {
            at: latest,
            look: false
        })
    );
    let todos = h.tick(latest, None);
    assert_eq!(agents_rounds(&todos), vec![(a, "write".to_string(), 2)]);
}

#[test]
fn a_cause_one_work_has_holds_no_other_work() {
    let (mut h, a, b) = two();
    fails(&mut h, a, Failure::Passing, None);
    assert_eq!(h.d().outage(), None);
    let todos = h.hands_in(b, &[("text", None)]);
    assert!(!todos.is_empty());
}

#[test]
fn without_the_network_the_engines_own_acts_wait_as_well_and_go_on_with_the_rest() {
    let (mut h, a, b) = two();
    fails(&mut h, a, Failure::Offline, None);
    // The other work's round, under way as the network went, ends: its merge, which reaches the network on a host,
    // waits with every agent's round.
    let todos = h.hands_in(b, &[("text", None)]);
    assert_eq!(todos, vec![], "an act began while the network could not be reached");
    let since = h.d().outage().expect("the wait stands").since;
    let todos = h.tick(Time(since.0 + PROBE), Some(true));
    assert_eq!(agents_rounds(&todos), vec![(a, "write".to_string(), 2)]);
    // The round that tried comes back: every work goes on, the merge among them.
    let todos = h.hands_in(a, &[("text", None)]);
    let merges = (todos.iter()).filter(|t| matches!(&t.effect, Effect::Round { cell, .. } if cell.as_str() == "merge"));
    assert_eq!(merges.map(|t| t.work).collect::<Vec<_>>(), vec![a, b]);
}

#[test]
fn a_shared_failure_of_a_round_whose_work_was_cancelled_begins_no_wait() {
    let (mut h, a, b) = two();
    let todos = h.on(a, Event::Cancelled { why: None });
    assert!(
        matches!(
            &todos[..],
            [Todo {
                effect: Effect::Stop { .. },
                ..
            }]
        ),
        "{todos:?}"
    );
    // The carrier's failure comes after the stop: what came of the round is dropped.
    let run = h.d().work(a).unwrap().work.records.started[0].run;
    let spent = h.spent();
    let failed = Event::Failed {
        run,
        why: "You've hit your limit".to_string(),
        failure: Failure::UsageLimit,
        resets: None,
        spent,
    };
    h.on(a, failed);
    assert_eq!(h.d().outage(), None);
    assert!(!h.hands_in(b, &[("text", None)]).is_empty(), "the other work was held");
}
