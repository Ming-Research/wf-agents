//! A wait every work shares: a round of an agent that failed for a cause outside every work holds the agents' rounds
//! of every work from that first one, as one fact worked out from all their records; the engine checks the outside by
//! itself, at times that grow longer while it stays down, giving no round and touching no work's session, and every
//! work goes on once a check finds it well. The engine's own acts go on meanwhile, and no work is paused for it. An act
//! that fails for want of the network to its host holds the acts of every work, and only those, until a look at the
//! host reaches it.

use decide::run::{Failure, PASSING};
use driver::outage::{LONGEST_CHECK, PROBE, spacing};
use driver::{Found, Until, Wake};

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

/// What a check finds down, of this kind.
fn down(failure: Failure) -> Found {
    Found::Down {
        failure,
        why: format!("the check found: {failure:?}"),
        resets: None,
    }
}

/// The check the driver asks for next: when, as the wake says it.
fn next_check(h: &H) -> Time {
    let wake = h.d().wake().expect("the driver asks to be woken");
    assert!(wake.check, "{wake:?}");
    wake.at
}

/// The kinds of cause outside every work, the same for all.
const OUTSIDE: [Failure; 4] = [
    Failure::CannotRun,
    Failure::UsageLimit,
    Failure::Offline,
    Failure::Unavailable,
];

// The first round that meets a cause outside every work holds every work's next agent's round, its own included, and
// pauses none; a work let on meanwhile begins nothing. A round that began before goes on to its end, and its work's
// engine act after it goes on.
#[test]
fn the_first_round_that_meets_the_outside_down_holds_every_works_next_round_and_pauses_none() {
    for failure in OUTSIDE {
        let (mut h, a, b) = two();
        assert_eq!(fails(&mut h, a, failure, None), vec![], "{failure:?}");
        assert!(!decide::paused(&h.d().work(a).unwrap().work.records), "{failure:?}");
        let outage = h.d().outage().expect("the wait stands");
        assert_eq!((outage.failure, outage.work), (failure, a));
        assert!(outage.holds(), "{failure:?}");
        let todos = h.hands_in(b, &[("text", None)]);
        assert_eq!(rounds(&todos), vec![(b, "merge".to_string(), 1)], "{failure:?}");
        assert!(
            h.d().outage().is_some(),
            "a round begun before the failure lifts nothing"
        );
        let (_, c) = h.add(new_item("c", &[], "plain"));
        assert_eq!(agents_rounds(&h.let_on(c)), vec![], "{failure:?}");
    }
}

// While the outside is down the driver asks for nothing but checks of it, each later than the one before, from half
// a minute after the round that met it up to a bound, and gives no round of any work: no work's session is touched.
#[test]
fn while_the_checks_find_the_outside_down_no_round_begins_and_the_checks_grow_longer_to_a_bound() {
    for failure in OUTSIDE {
        let (mut h, a, b) = two();
        fails(&mut h, a, failure, None);
        fails(&mut h, b, failure, None);
        let met = h.d().outage().expect("the wait stands").met;
        let mut at = Time(met.0 + PROBE);
        for taken in 0..12 {
            assert_eq!(next_check(&h), at, "{failure:?}, check {taken}");
            assert_eq!(h.tick(Time(at.0 - 1), None), vec![], "nothing begins before its time");
            assert_eq!(h.tick(at, Some(down(failure))), vec![], "{failure:?}: a round began");
            at = Time(at.0 + spacing(taken + 1));
        }
        assert_eq!(spacing(12), LONGEST_CHECK);
        let outage = h.d().outage().expect("the wait stands");
        assert_eq!(outage.checked.as_ref().map(|c| c.taken), Some(12));
        assert_eq!(
            outage.down(),
            (failure, format!("the check found: {failure:?}").as_str())
        );
        assert_eq!(outage.until(), Until::Checked { at });
    }
}

// A check that finds the outside well lets every work go on from where it was: the round that met the failure is run
// again, and a work that waited to begin begins. The wait stands, holding nothing, until a round begun after it comes
// back.
#[test]
fn a_check_that_finds_the_outside_well_lets_every_work_go_on_from_where_it_was() {
    for failure in OUTSIDE {
        let (mut h, a, b) = two();
        fails(&mut h, a, failure, None);
        fails(&mut h, b, failure, None);
        let first = next_check(&h);
        h.tick(first, Some(down(failure)));
        let well = next_check(&h);
        let todos = h.tick(well, Some(Found::Well));
        let mut given = agents_rounds(&todos);
        given.sort();
        assert_eq!(
            given,
            vec![(a, "write".to_string(), 2), (b, "write".to_string(), 2)],
            "{failure:?}"
        );
        let outage = h.d().outage().expect("the wait stands until a round comes back");
        assert!(!outage.holds());
        assert_eq!(outage.until(), Until::Well { at: well });
        assert_eq!(h.d().wake(), None, "nothing more is checked");
        h.hands_in(a, &[("text", None)]);
        assert_eq!(h.d().outage(), None, "{failure:?}");
        assert!(!decide::paused(&h.d().work(a).unwrap().work.records));
    }
}

// At a usage limit whose reset was said, nothing is checked before the reset; a check at it that finds the limit still
// there, with a reset of its own, waits for that one.
#[test]
fn at_a_usage_limit_whose_reset_was_said_the_outside_is_checked_at_the_reset_and_not_before() {
    let (mut h, a, b) = two();
    let resets = Time(h.clock + 3_600_000);
    fails(&mut h, a, Failure::UsageLimit, Some(resets));
    assert_eq!(
        h.d().wake(),
        Some(Wake {
            at: resets,
            check: true,
            look_at_host: false
        })
    );
    assert_eq!(h.tick(Time(resets.0 - 1), None), vec![]);
    let later = Time(resets.0 + 7_200_000);
    let still = Found::Down {
        failure: Failure::UsageLimit,
        why: "You've hit your limit".to_string(),
        resets: Some(later),
    };
    assert_eq!(h.tick(resets, Some(still)), vec![]);
    assert_eq!(next_check(&h), later);
    let todos = h.tick(later, Some(Found::Well));
    let mut given = agents_rounds(&todos);
    given.sort();
    assert_eq!(given, vec![(a, "write".to_string(), 2)]);
    // The other work's round, under way all along, ends: its merge goes on.
    let todos = h.hands_in(b, &[("text", None)]);
    assert_eq!(rounds(&todos), vec![(b, "merge".to_string(), 1)]);
}

// A round that meets the outside down again after a check found it well holds every round again, and the checks go on
// growing over the whole wait: a cause the check does not find is met no more often than the checks are taken. Past
// the bound on tries in a row, the round's work is paused, and only it.
#[test]
fn a_round_that_meets_the_outside_again_after_a_check_holds_every_round_again_and_the_checks_grow_on() {
    let (mut h, a, b) = two();
    fails(&mut h, a, Failure::Unavailable, None);
    let since = h.d().outage().expect("the wait stands").since;
    let mut taken = 0;
    for round in 2..=PASSING as usize + 1 {
        let at = next_check(&h);
        let todos = h.tick(at, Some(Found::Well));
        assert_eq!(agents_rounds(&todos), vec![(a, "write".to_string(), round)]);
        taken += 1;
        let todos = fails(&mut h, a, Failure::Unavailable, None);
        assert_eq!(agents_rounds(&todos), vec![]);
        let outage = h.d().outage().expect("the wait stands");
        assert_eq!(outage.since, since, "one wait");
        assert!(outage.holds());
        assert_eq!(next_check(&h), Time(outage.met.0 + spacing(taken)));
    }
    let records = &h.d().work(a).unwrap().work.records;
    assert!(decide::paused(records), "past the bound the work waits for the owner");
    let why = records.pauses.last().and_then(|p| p.why.clone()).unwrap_or_default();
    assert!(why.contains("for a cause outside every work"), "{why}");
    assert!(!decide::paused(&h.d().work(b).unwrap().work.records));
}

#[test]
fn a_cause_one_work_has_holds_no_other_work() {
    for failure in [Failure::Passing, Failure::PromptTooLong, Failure::Other] {
        let (mut h, a, b) = two();
        fails(&mut h, a, failure, None);
        assert_eq!(h.d().outage(), None, "{failure:?}");
        let paused = decide::paused(&h.d().work(a).unwrap().work.records);
        assert_eq!(paused, failure != Failure::Passing, "{failure:?}");
        let todos = h.hands_in(b, &[("text", None)]);
        assert_eq!(rounds(&todos), vec![(b, "merge".to_string(), 1)], "{failure:?}");
        assert!(!decide::paused(&h.d().work(b).unwrap().work.records));
    }
}

// Each service has its own wait: without the network to the agents' service the engine's acts, which reach their own
// host, go on; and an act that fails for want of the network to its host holds the acts of every work, and only those,
// until a look at the host reaches it, an agent's round that comes back meanwhile ending nothing of it.
#[test]
fn without_the_network_to_the_agents_service_the_engines_own_acts_go_on() {
    let (mut h, a, b) = two();
    fails(&mut h, a, Failure::Offline, None);
    let todos = h.hands_in(b, &[("text", None)]);
    assert_eq!(rounds(&todos), vec![(b, "merge".to_string(), 1)]);
    assert_eq!(h.d().host_outage(), None);
}

#[test]
fn an_act_that_fails_for_want_of_its_hosts_network_holds_the_acts_alone_until_a_look_at_the_host_reaches_it() {
    let (mut h, a, b) = two();
    let todos = h.hands_in(a, &[("text", None)]);
    assert_eq!(rounds(&todos), vec![(a, "merge".to_string(), 1)]);
    fails(&mut h, a, Failure::Offline, None);
    assert!(
        !decide::paused(&h.d().work(a).unwrap().work.records),
        "the act pauses no work"
    );
    assert_eq!(h.d().outage(), None, "the agents' service is not held");
    let outage = h.d().host_outage().expect("the acts' wait stands");
    assert_eq!((outage.failure, outage.work), (Failure::Offline, a));
    let at = Time(outage.met.0 + PROBE);
    assert_eq!(outage.until(), Until::Looked { at });
    assert_eq!(
        h.d().wake(),
        Some(Wake {
            at,
            check: false,
            look_at_host: true
        })
    );
    // The other work's agent goes on, and comes back: the acts' wait stands, and its merge waits.
    let todos = h.hands_in(b, &[("text", None)]);
    assert_eq!(rounds(&todos), vec![]);
    assert!(h.d().host_outage().is_some());
    // A look at the host that does not reach it lets nothing on; one that does, every act at once.
    assert_eq!(h.tick_host(at, false), vec![]);
    assert_eq!(
        h.d().wake(),
        Some(Wake {
            at: Time(at.0 + PROBE),
            check: false,
            look_at_host: true
        })
    );
    let todos = h.tick_host(Time(at.0 + PROBE), true);
    assert_eq!(
        rounds(&todos),
        vec![(a, "merge".to_string(), 2), (b, "merge".to_string(), 1)]
    );
    h.hands_in(a, &[("merged", None)]);
    assert_eq!(h.d().host_outage(), None);
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

// Once a check has let the works on, the first answer of the service to a round begun after the failure ends the
// wait, not that round's end, which may be long after.
#[test]
fn a_wait_is_over_once_the_service_answers_a_round_begun_after_the_check() {
    for failure in OUTSIDE {
        let (mut h, a, b) = two();
        fails(&mut h, a, failure, None);
        fails(&mut h, b, failure, None);
        let at = next_check(&h);
        h.tick(at, Some(Found::Well));
        assert_eq!(h.service_answers(a), vec![], "{failure:?}");
        assert_eq!(h.d().outage(), None, "{failure:?}");
        // Answered again, it changes nothing.
        assert_eq!(h.service_answers(a), vec![]);
    }
}

// A check taken while no wait stands, as one whose wait a round's coming back ended meanwhile, is of no wait.
#[test]
fn a_check_of_a_wait_that_ended_is_of_no_other() {
    let (mut h, a, b) = two();
    fails(&mut h, a, Failure::CannotRun, None);
    let at = next_check(&h);
    h.tick(at, Some(Found::Well));
    h.hands_in(a, &[("text", None)]);
    assert_eq!(h.d().outage(), None);
    let late = Time(h.clock + 1);
    h.tick(late, Some(down(Failure::CannotRun)));
    // A new wait begins its checks anew, half a minute after the round that met it.
    fails(&mut h, b, Failure::CannotRun, None);
    let outage = h.d().outage().expect("a new wait stands");
    assert_eq!(outage.checked, None);
    assert_eq!(next_check(&h), Time(outage.met.0 + PROBE));
}
