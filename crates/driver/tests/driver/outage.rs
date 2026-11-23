//! A wait every work shares: a round of an agent that failed for a cause outside every work holds the agents' rounds
//! of every work from that first one, as one fact worked out from all their records; the engine checks the outside by
//! itself, at times that grow longer while it stays down, giving no round and touching no work's session, and every
//! work goes on once a check finds it well. The engine's own acts go on meanwhile, and no work is paused for it. An act
//! that fails for want of the network to its host holds the acts of every work, and only those, until a look at the
//! host reaches it. A usage limit is its account's own: it holds the rounds only while its account is in use, and
//! taking another account has the outside checked at once under it.

use std::collections::{BTreeMap, BTreeSet};

use decide::run::{Failure, PASSING};
use driver::outage::{LONGEST_CHECK, PROBE, spacing};
use driver::{Found, Until, Wake};

use super::*;

/// The round of the work under way fails for a cause of this kind, its usage limit resetting at `resets`.
pub(super) fn fails(h: &mut H, work: WorkNumber, failure: Failure, resets: Option<Time>) -> Vec<Todo> {
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
pub(super) fn two() -> (H, WorkNumber, WorkNumber) {
    let mut h = H::new();
    let (_, a) = h.add(new_item("a", &[], "plain"));
    let (_, b) = h.add(new_item("b", &[], "plain"));
    h.let_on(a);
    h.let_on(b);
    (h, a, b)
}

/// The rounds of agents' cells given among these.
pub(super) fn agents_rounds(todos: &[Todo]) -> Vec<(WorkNumber, String, usize)> {
    (todos.iter())
        .filter_map(|t| match &t.effect {
            Effect::Round { cell, number, .. } if cell.as_str() != "merge" => Some((t.work, cell.to_string(), *number)),
            _ => None,
        })
        .collect()
}

/// What a check finds down, of this kind.
pub(super) fn down(failure: Failure) -> Found {
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
            candidate: None,
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
            candidate: None,
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
            candidate: None,
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

/// Two works of the plain flow, let on, each with its writer's first round under way under the account `first`, which
/// the owner took of the accounts `first` and `second`.
fn two_under_accounts() -> (H, WorkNumber, WorkNumber) {
    let (mut h, a, b) = two();
    h.add_account("first");
    h.add_account("second");
    h.take_account("first");
    h.ran_under(a, "first");
    h.ran_under(b, "first");
    (h, a, b)
}

// A usage limit is its account's own (decision a-shared-cause-is-one-wait): met under one account, it holds the rounds
// while that account is in use; taking another ends the wait at once; a round coming back under the other ends nothing
// of the first's limit; and taking the first back before its reset holds the rounds again, checked at its reset.
#[test]
fn a_usage_limit_holds_the_rounds_only_while_its_account_is_in_use() {
    let (mut h, a, b) = two_under_accounts();
    let resets = Time(h.clock + 3_600_000);
    fails(&mut h, a, Failure::UsageLimit, Some(resets));
    assert!(h.d().outage().expect("the wait stands").holds());
    let limit = h.d().limits().get("first").copied().expect("the first's limit stands");
    assert_eq!(limit.resets, Some(resets));
    // Taking another account ends the wait at once: the round that met the limit is run again, under it.
    let todos = h.take_account("second");
    assert_eq!(h.d().outage(), None);
    assert_eq!(agents_rounds(&todos), vec![(a, "write".to_string(), 2)]);
    h.ran_under(a, "second");
    h.hands_in(a, &[("text", None)]);
    assert_eq!(
        h.d().limits().get("first"),
        Some(&limit),
        "a round under another account ends nothing of the first's limit"
    );
    assert!(!h.d().limits().contains_key("second"));
    // Taken back before its reset, the first holds the rounds again, and is checked at its reset and not before.
    h.take_account("first");
    assert!(h.d().outage().expect("the wait stands again").holds());
    assert_eq!(next_check(&h), resets);
    // A round under it that comes back ends its limit: the other work's, under way all along, began before it.
    h.hands_in(b, &[("text", None)]);
    assert!(
        h.d().limits().contains_key("first"),
        "a round begun before the limit ends nothing of it"
    );
    let todos = h.tick(resets, Some(Found::Well));
    assert_eq!(agents_rounds(&todos), vec![]);
}

// Taking another account drops what the checks found, which was of the account before, and the outside is checked at
// once under the one taken (decision the-engine-checks-the-outside-itself).
#[test]
fn taking_another_account_has_the_outside_checked_at_once_under_it() {
    let (mut h, a, _) = two_under_accounts();
    fails(&mut h, a, Failure::CannotRun, None);
    let first = next_check(&h);
    h.tick(first, Some(down(Failure::CannotRun)));
    assert_eq!(h.d().outage().and_then(|o| o.checked).map(|c| c.taken), Some(1));
    let todos = h.take_account("second");
    assert_eq!(
        agents_rounds(&todos),
        vec![],
        "a cause of every account holds until a check finds it well"
    );
    assert_eq!(h.d().outage().expect("the wait stands").checked, None);
    let now = Time(h.clock);
    assert_eq!(next_check(&h), now, "checked at once");
    let todos = h.tick(now, Some(Found::Well));
    assert_eq!(agents_rounds(&todos), vec![(a, "write".to_string(), 2)]);
}

/// What a history knows of the accounts from what it told the driver alone: the accounts added, the one it took, the
/// account each agent's round was handed out with, by its work, run and number, and each account whose usage limit
/// stands, from a round under it that met the limit until a check under it found the outside well.
struct Told {
    added: BTreeSet<String>,
    in_use: String,
    rounds: BTreeMap<(WorkNumber, RunId, usize), String>,
    limited: BTreeSet<String>,
}

/// The agents' rounds under way, each of a work by its run, with the account the history handed it out with.
fn under_way(h: &H, told: &Told) -> Vec<(WorkNumber, RunId, String)> {
    (h.outstanding.iter())
        .filter_map(|t| match &t.effect {
            Effect::Round { run, cell, number, .. } if cell.as_str() == "write" => {
                let account = told.rounds.get(&(t.work, *run, *number))?;
                Some((t.work, *run, account.clone()))
            }
            _ => None,
        })
        .collect()
}

/// Each agent's round among these that is new, not one given again as the engine opened, begins its try under the
/// account the history took, which it was handed out with, as its carrier says; none is handed out under an account
/// whose usage limit stands, as the history knows it. Gives how many were handed out.
fn handed_out(h: &mut H, todos: &[Todo], told: &mut Told) -> usize {
    let mut handed = 0;
    for (work, run, number) in (todos.iter()).filter_map(|t| match &t.effect {
        Effect::Round { run, cell, number, .. } if cell.as_str() == "write" => Some((t.work, *run, *number)),
        _ => None,
    }) {
        if told.rounds.contains_key(&(work, run, number)) {
            continue;
        }
        let in_use = told.in_use.clone();
        assert!(
            !told.limited.contains(&in_use),
            "a round of W{} run {} is handed out under {in_use}, whose usage limit stands",
            work.0,
            run.0
        );
        told.rounds.insert((work, run, number), in_use.clone());
        let more = h.ran_under(work, &in_use);
        handed += 1 + handed_out(h, &more, told);
    }
    handed
}

// Over random histories of three accounts, taken in turn at random, removed and added again, rounds meeting the usage
// limit, rounds coming back, the service answering a round, works added, checks finding the outside well or at the
// limit still, and the engine opened again, no agent's round is handed out under an account whose usage limit stands,
// as the history knows it from what it told the driver: from a round under the account that met the limit, until a
// check under that account found the outside well. Thirty-two histories of eighty steps, each step checked by the
// harness: about a second.
#[test]
fn no_round_is_handed_out_under_an_account_whose_usage_limit_stands() {
    const ACCOUNTS: [&str; 3] = ["first", "second", "third"];
    // Rounds handed out while another account's limit stood, limits met, accounts taken whose limit stood, limits a
    // check under their account found over, accounts removed, and openings anew.
    let (mut around, mut met, mut back, mut over, mut removed, mut reopened) = (0, 0, 0, 0, 0, 0);
    for seed in 0..32u64 {
        let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut draw = |n: u64| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state % n
        };
        let mut h = H::new();
        for name in ACCOUNTS {
            h.add_account(name);
        }
        h.take_account(ACCOUNTS[0]);
        let mut told = Told {
            added: ACCOUNTS.iter().map(|name| name.to_string()).collect(),
            in_use: ACCOUNTS[0].to_string(),
            rounds: BTreeMap::new(),
            limited: BTreeSet::new(),
        };
        let mut items = 0;
        for _ in 0..80 {
            let handed = match draw(8) {
                0 => {
                    let added: Vec<String> = told.added.iter().cloned().collect();
                    let name = added[draw(added.len() as u64) as usize].clone();
                    back += usize::from(told.limited.contains(&name));
                    let todos = h.take_account(&name);
                    told.in_use = name;
                    handed_out(&mut h, &todos, &mut told)
                }
                1 => match under_way(&h, &told).first().cloned() {
                    Some((work, run, account)) => {
                        let resets = (draw(2) == 0).then(|| Time(h.clock + 600_000));
                        let spent = h.spent();
                        let failed = Event::Failed {
                            run,
                            why: "You've hit your limit".to_string(),
                            failure: Failure::UsageLimit,
                            resets,
                            spent,
                        };
                        let todos = h.on(work, failed);
                        met += 1;
                        told.limited.insert(account);
                        handed_out(&mut h, &todos, &mut told)
                    }
                    None => 0,
                },
                2 => match under_way(&h, &told).first().cloned() {
                    Some((work, ..)) => {
                        let todos = h.hands_in(work, &[("text", None)]);
                        handed_out(&mut h, &todos, &mut told)
                    }
                    None => 0,
                },
                3 => match h.d().wake().filter(|wake| wake.check) {
                    Some(wake) => {
                        let well = draw(2) == 0;
                        let found = match well {
                            true => Found::Well,
                            false => Found::Down {
                                failure: Failure::UsageLimit,
                                why: "You've hit your limit".to_string(),
                                resets: None,
                            },
                        };
                        if well {
                            over += usize::from(told.limited.remove(&told.in_use));
                        }
                        let todos = h.tick_under(wake.at, Some(told.in_use.as_str()), found);
                        handed_out(&mut h, &todos, &mut told)
                    }
                    None => 0,
                },
                4 => match under_way(&h, &told).first().cloned() {
                    Some((work, ..)) => {
                        let todos = h.service_answers(work);
                        handed_out(&mut h, &todos, &mut told)
                    }
                    None => 0,
                },
                5 => {
                    // An account not in use is removed, or one removed is added again.
                    let gone: Vec<&str> = (ACCOUNTS.iter().copied())
                        .filter(|name| !told.added.contains(*name))
                        .collect();
                    match gone.first() {
                        Some(name) => {
                            h.add_account(name);
                            told.added.insert(name.to_string());
                        }
                        None => {
                            let others: Vec<String> = (told.added.iter())
                                .filter(|name| **name != told.in_use)
                                .cloned()
                                .collect();
                            let name = others[draw(others.len() as u64) as usize].clone();
                            h.remove_account(&name);
                            told.added.remove(&name);
                            removed += 1;
                        }
                    }
                    0
                }
                6 => {
                    // The engine opens anew, the owner lifting at once the pause of everything it makes on works going
                    // on: what was under way is given again, under the account it was handed out with.
                    reopened += 1;
                    let todos = h.restart();
                    handed_out(&mut h, &todos, &mut told)
                }
                _ => {
                    // A merge under way comes back, or else a work is added and let on.
                    let merging = (h.outstanding.iter()).find_map(|t| match &t.effect {
                        Effect::Round { cell, .. } if cell.as_str() == "merge" => Some(t.work),
                        _ => None,
                    });
                    let todos = match merging {
                        Some(work) => h.hands_in(work, &[("merged", None)]),
                        None if items < 8 => {
                            items += 1;
                            let (_, work) = h.add(new_item(&format!("item {items}"), &[], "plain"));
                            h.let_on(work)
                        }
                        None => Vec::new(),
                    };
                    handed_out(&mut h, &todos, &mut told)
                }
            };
            if handed > 0 && told.limited.iter().any(|name| *name != told.in_use) {
                around += handed;
            }
        }
    }
    for (what, n) in [
        ("rounds handed out while another account's limit stood", around),
        ("limits met", met),
        ("accounts taken back while their limit stood", back),
        ("limits a check under their account found over", over),
        ("accounts removed", removed),
        ("openings anew", reopened),
    ] {
        assert!(n > 0, "never put to the test: {what}");
    }
}

// A check speaks only of the account it was taken under: one taken under the account before a switch, which comes
// back after it, is no check of the account taken, whose wait holds until a check under it finds the outside well.
#[test]
fn a_check_taken_under_another_account_than_the_one_in_use_is_no_check_of_it() {
    let (mut h, a, _) = two_under_accounts();
    fails(&mut h, a, Failure::CannotRun, None);
    let switched = {
        h.take_account("second");
        Time(h.clock)
    };
    assert_eq!(next_check(&h), switched);
    // The check asked for under the first account, before the switch, comes back after it, finding the outside well.
    let todos = h.tick_under(switched, Some("first"), Found::Well);
    assert_eq!(agents_rounds(&todos), vec![], "it speaks of the first account alone");
    let outage = h.d().outage().expect("the wait stands");
    assert!(outage.holds());
    assert_eq!(outage.checked, None);
    // The check asked for under the second, at the switch, finds it down, and then well.
    h.tick_under(switched, Some("second"), down(Failure::CannotRun));
    assert_eq!(h.d().outage().and_then(|o| o.checked).map(|c| c.taken), Some(1));
    let at = next_check(&h);
    let todos = h.tick_under(at, Some("second"), Found::Well);
    assert_eq!(agents_rounds(&todos), vec![(a, "write".to_string(), 2)]);
}
