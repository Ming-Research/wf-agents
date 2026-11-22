//! The engine's moving to another account at a usage limit, where the owner lets it: with the account in use at its
//! usage limit and the switch on, the engine checks the other accounts one at a time, in the order they were added,
//! each by the short request under it, takes the first that is found well, and the agents go on under it; it records
//! that it took the account, which account it left and when that account's limit resets. An account whose limit stands
//! until a time to come is not checked before it, one found at its limit is passed over until its own reset, a cause
//! that is no account's is checked again after the spacing of the checks, and an account whose token is refused is
//! passed over for the wait. Nothing moves for a cause that is not a usage limit, with the switch off, or while
//! everything is paused.

use decide::run::Failure;
use driver::outage::spacing;
use driver::{Found, Moved, Wake};
use store::{AccountAct, AccountTaker};

use super::outage::{agents_rounds, down, fails, two};
use super::*;

const HOUR: u64 = 3_600_000;

/// Two works of the plain flow, let on, each with its writer's first round under way under the first of these
/// accounts, which the owner added in this order and took.
fn under(names: &[&str]) -> (H, WorkNumber, WorkNumber) {
    let (mut h, a, b) = two();
    for name in names {
        h.add_account(name);
    }
    h.take_account(names[0]);
    h.ran_under(a, names[0]);
    h.ran_under(b, names[0]);
    (h, a, b)
}

/// What a check under an account finds where its usage limit stands until `resets`.
fn at_limit(resets: Option<Time>) -> Found {
    Found::Down {
        failure: Failure::UsageLimit,
        why: "You've hit your limit".to_string(),
        resets,
    }
}

/// The check to move to an account that the driver asks for next: when, and under which account.
fn asked(h: &H) -> Option<(Time, String)> {
    let wake = h.d().wake()?;
    let at = wake.at;
    wake.candidate.map(|account| (at, account))
}

// The switch is off until the owner switches it on: the same history that moves with it on moves nothing with it off,
// the wait's own check waits for the reset, and a check found well under another account takes nothing; switched on
// afterwards, the account that was found well is taken at once.
#[test]
fn with_the_switch_off_the_same_history_moves_nothing() {
    let (mut h, a, _) = under(&["first", "second"]);
    assert!(!h.d().moving_on(), "off until it is switched on");
    let resets = Time(h.clock + 3 * HOUR);
    fails(&mut h, a, Failure::UsageLimit, Some(resets));
    assert!(!h.d().outage().expect("the wait stands").may_move);
    assert_eq!(
        h.d().wake(),
        Some(Wake {
            at: resets,
            check: true,
            candidate: None,
            look_at_host: false
        }),
        "only the wait's own check, at the reset"
    );
    let at = Time(h.clock + 1);
    assert_eq!(h.tick_candidate(at, "second", Found::Well), vec![]);
    assert_eq!(h.d().accounts().in_use(), Some("first"));
    assert_eq!(h.d().moves(), vec![]);
    assert!(h.d().outage().expect("the wait stands").holds());
    let todos = h.set_moving_on(true);
    assert_eq!(h.d().accounts().in_use(), Some("second"));
    assert_eq!(agents_rounds(&todos), vec![(a, "write".to_string(), 2)]);
}

#[test]
fn with_the_switch_on_a_usage_limit_moves_the_agents_to_the_first_account_a_check_finds_well() {
    let (mut h, a, _) = under(&["first", "second", "third"]);
    h.set_moving_on(true);
    assert!(h.d().moving_on());
    let resets = Time(h.clock + 3 * HOUR);
    fails(&mut h, a, Failure::UsageLimit, Some(resets));
    let outage = h.d().outage().expect("the wait stands");
    assert!(outage.may_move && outage.holds());
    // The accounts are checked in the order they were added, the first at once, none before the one before it.
    let wake = h.d().wake().expect("a check is asked for");
    assert_eq!((wake.at, wake.candidate.as_deref()), (outage.met, Some("second")));
    assert!(wake.check && !wake.look_at_host);
    let todos = h.tick_candidate(wake.at, "second", Found::Well);
    assert_eq!(h.d().accounts().in_use(), Some("second"));
    assert_eq!(h.d().outage(), None, "the wait ends: the limit was the first's");
    assert_eq!(agents_rounds(&todos), vec![(a, "write".to_string(), 2)]);
    assert_eq!(
        h.d().moves(),
        vec![Moved {
            at: wake.at,
            from: Some("first".to_string()),
            to: "second".to_string(),
            left_resets: Some(resets),
        }]
    );
    let record = h.d().accounts().records().last().cloned().expect("the take is kept");
    assert_eq!((record.what, record.taker), (AccountAct::Taken, AccountTaker::Engine));
    assert_eq!(h.d().wake(), None, "the third is checked no more");
    // The first's limit stands, though no round runs under it.
    assert_eq!(
        h.d().limits().get("first").map(|limit| limit.resets),
        Some(Some(resets))
    );
}

// What the engine took, who took it and what it left is kept in the store: opened again, the driver reads it so.
#[test]
fn a_move_survives_reopening() {
    let (mut h, a, _) = under(&["first", "second"]);
    h.set_moving_on(true);
    let resets = Time(h.clock + HOUR);
    fails(&mut h, a, Failure::UsageLimit, Some(resets));
    let wake = h.d().wake().expect("a check is asked for");
    h.tick_candidate(wake.at, "second", Found::Well);
    let moves = h.d().moves();
    assert_eq!(moves.len(), 1);
    h.reopen();
    assert!(h.d().moving_on());
    assert_eq!(h.d().accounts().in_use(), Some("second"));
    assert_eq!(h.d().moves(), moves);
}

// An account whose usage limit stands, from a round under it that met it, until a time to come is not checked: the
// next one is.
#[test]
fn an_account_whose_limit_stands_with_a_reset_to_come_is_not_checked() {
    let (mut h, a, _) = under(&["first", "second", "third"]);
    let first_resets = Time(h.clock + 3 * HOUR);
    fails(&mut h, a, Failure::UsageLimit, Some(first_resets));
    // The owner takes the second; the round that met the limit is run again under it, and meets one of its own.
    h.take_account("second");
    h.ran_under(a, "second");
    h.set_moving_on(true);
    let second_resets = Time(h.clock + HOUR);
    fails(&mut h, a, Failure::UsageLimit, Some(second_resets));
    let outage = h.d().outage().expect("the wait stands");
    // The first's limit stands until a time to come: the third is checked, and the first is not.
    assert_eq!(asked(&h), Some((outage.met, "third".to_string())));
    let todos = h.tick_candidate(outage.met, "third", Found::Well);
    assert_eq!(h.d().accounts().in_use(), Some("third"));
    assert_eq!(agents_rounds(&todos), vec![(a, "write".to_string(), 3)]);
    let moves = h.d().moves();
    assert_eq!(moves.len(), 1, "the engine took the third only");
    assert_eq!(
        (moves[0].from.as_deref(), moves[0].to.as_str(), moves[0].left_resets),
        (Some("second"), "third", Some(second_resets))
    );
}

// Two accounts at their limits are not handed the agents back and forth: the one left stands at its limit until its
// reset, and is checked again at it and not before.
#[test]
fn two_accounts_at_their_limits_do_not_hand_the_agents_back_and_forth() {
    let (mut h, a, _) = under(&["first", "second"]);
    h.set_moving_on(true);
    let first_resets = Time(h.clock + 3 * HOUR);
    fails(&mut h, a, Failure::UsageLimit, Some(first_resets));
    let wake = h.d().wake().expect("a check is asked for");
    h.tick_candidate(wake.at, "second", Found::Well);
    assert_eq!(h.d().accounts().in_use(), Some("second"));
    h.ran_under(a, "second");
    let second_resets = Time(h.clock + HOUR);
    fails(&mut h, a, Failure::UsageLimit, Some(second_resets));
    assert!(h.d().outage().expect("the wait stands").holds());
    assert!(
        !h.d().outage().expect("the wait stands").may_move,
        "the first's limit stands until its reset"
    );
    assert_eq!(
        h.d().wake(),
        Some(Wake {
            at: second_resets,
            check: true,
            candidate: None,
            look_at_host: false
        }),
        "the second's own check, at its reset, comes before the first's"
    );
    assert_eq!(h.d().moves().len(), 1, "no second move");
    assert_eq!(h.d().accounts().in_use(), Some("second"));
    // The first resets after the second: at the first's reset the first is the one checked, and is taken.
    h.tick(second_resets, Some(at_limit(Some(first_resets))));
    assert_eq!(asked(&h), Some((first_resets, "first".to_string())));
    h.tick_candidate(first_resets, "first", Found::Well);
    assert_eq!(h.d().accounts().in_use(), Some("first"));
    assert_eq!(h.d().moves().len(), 2);
}

// An account found at its limit with its reset said is passed over until that reset, and the next is checked; at the
// reset it is checked again.
#[test]
fn a_candidate_found_at_its_limit_with_a_reset_is_passed_over_until_it_and_the_next_is_checked() {
    let (mut h, a, _) = under(&["first", "second", "third"]);
    h.set_moving_on(true);
    let first_resets = Time(h.clock + 3 * HOUR);
    fails(&mut h, a, Failure::UsageLimit, Some(first_resets));
    let (at, account) = asked(&h).expect("a check is asked for");
    assert_eq!(account, "second");
    let second_resets = Time(at.0 + HOUR);
    assert_eq!(h.tick_candidate(at, "second", at_limit(Some(second_resets))), vec![]);
    assert_eq!(h.d().accounts().in_use(), Some("first"));
    // The third is next, at once; a refused token passes it over for the wait.
    let (at, account) = asked(&h).expect("the next check is asked for");
    assert_eq!(account, "third");
    h.tick_candidate(at, "third", down(Failure::CannotRun));
    // The second is checked again at its reset, before the first's own check at its reset.
    assert_eq!(asked(&h), Some((second_resets, "second".to_string())));
    let todos = h.tick_candidate(second_resets, "second", Found::Well);
    assert_eq!(h.d().accounts().in_use(), Some("second"));
    assert_eq!(agents_rounds(&todos), vec![(a, "write".to_string(), 2)]);
    assert_eq!(h.d().moves()[0].left_resets, Some(first_resets));
}

// A cause that is no account's, or a limit with no reset said, is checked again after the spacing of the checks, and
// is not passed over.
#[test]
fn a_candidate_found_at_its_limit_with_no_reset_or_met_offline_or_unavailable_is_checked_again_after_the_spacing() {
    for found in [at_limit(None), down(Failure::Offline), down(Failure::Unavailable)] {
        let (mut h, a, _) = under(&["first", "second"]);
        h.set_moving_on(true);
        let resets = Time(h.clock + 3 * HOUR);
        fails(&mut h, a, Failure::UsageLimit, Some(resets));
        let (at, _) = asked(&h).expect("a check is asked for");
        h.tick_candidate(at, "second", found.clone());
        let next = Time(at.0 + spacing(1));
        assert_eq!(asked(&h), Some((next, "second".to_string())), "{found:?}");
        assert!(h.d().outage().expect("the wait stands").may_move, "{found:?}");
        h.tick_candidate(next, "second", found.clone());
        assert_eq!(
            asked(&h),
            Some((Time(next.0 + spacing(2)), "second".to_string())),
            "{found:?}"
        );
        assert_eq!(h.d().accounts().in_use(), Some("first"));
        // Found well at last, it is taken.
        let (at, _) = asked(&h).expect("a check is asked for");
        h.tick_candidate(at, "second", Found::Well);
        assert_eq!(h.d().accounts().in_use(), Some("second"), "{found:?}");
    }
}

// An account whose token is refused is passed over for the wait, and is no account the wait may yet move to.
#[test]
fn a_candidate_whose_token_is_refused_is_passed_over_for_the_wait() {
    let (mut h, a, _) = under(&["first", "second"]);
    h.set_moving_on(true);
    let resets = Time(h.clock + 3 * HOUR);
    fails(&mut h, a, Failure::UsageLimit, Some(resets));
    let (at, _) = asked(&h).expect("a check is asked for");
    h.tick_candidate(at, "second", down(Failure::CannotRun));
    assert_eq!(asked(&h), None);
    assert!(!h.d().outage().expect("the wait stands").may_move);
    assert_eq!(
        h.d().wake(),
        Some(Wake {
            at: resets,
            check: true,
            candidate: None,
            look_at_host: false
        }),
        "only the wait's own check, at the reset"
    );
    assert_eq!(h.d().accounts().in_use(), Some("first"));
}

// Only a usage limit of the account in use is moved off: a cause every account shares is waited out as it is, and a
// check found well under another account takes nothing then.
#[test]
fn nothing_moves_for_a_cause_that_is_not_the_usage_limit_of_the_account_in_use() {
    for failure in [Failure::Offline, Failure::CannotRun, Failure::Unavailable] {
        let (mut h, a, _) = under(&["first", "second"]);
        h.set_moving_on(true);
        fails(&mut h, a, failure, None);
        assert_eq!(asked(&h), None, "{failure:?}");
        assert!(!h.d().outage().expect("the wait stands").may_move, "{failure:?}");
        let at = Time(h.clock + 1);
        assert_eq!(h.tick_candidate(at, "second", Found::Well), vec![], "{failure:?}");
        assert_eq!(h.d().accounts().in_use(), Some("first"), "{failure:?}");
        assert_eq!(h.d().moves(), vec![], "{failure:?}");
    }
}

// While everything is paused nothing is checked to move and nothing moves, though a check was found well; lifted, the
// account found well is taken.
#[test]
fn nothing_moves_while_everything_is_paused_and_the_account_found_well_is_taken_when_it_is_lifted() {
    let (mut h, a, _) = under(&["first", "second"]);
    h.set_moving_on(true);
    let resets = Time(h.clock + 3 * HOUR);
    fails(&mut h, a, Failure::UsageLimit, Some(resets));
    let (at, _) = asked(&h).expect("a check is asked for");
    h.pause_everything(true);
    assert_eq!(asked(&h), None, "no check is asked for while everything is paused");
    assert_eq!(h.tick_candidate(at, "second", Found::Well), vec![]);
    assert_eq!(h.d().accounts().in_use(), Some("first"));
    let todos = h.pause_everything(false);
    assert_eq!(h.d().accounts().in_use(), Some("second"));
    assert_eq!(agents_rounds(&todos), vec![(a, "write".to_string(), 2)]);
    assert_eq!(h.d().moves().len(), 1);
}

// A usage limit that holds the rounds, the engine moving on or not, is no wait on the owner: it is the engine's to wait
// out or move off.
#[test]
fn a_usage_limit_is_not_a_wait_on_the_owner() {
    for moving in [false, true] {
        let (mut h, a, b) = under(&["first", "second"]);
        h.set_moving_on(moving);
        let resets = Time(h.clock + HOUR);
        fails(&mut h, a, Failure::UsageLimit, Some(resets));
        assert!(h.d().outage().expect("the wait stands").holds());
        let at_once = h.d().at_once();
        for work in [a, b] {
            let view = h.d().view_at(work, &at_once).expect("the work has a view");
            let held = h.d().held_at(work, &at_once);
            assert!(
                h.d().waits_on_owner_at(work, &view, held, &at_once).is_empty(),
                "moving on: {moving}, work {}",
                work.0
            );
        }
        assert_eq!(at_once.cannot_run(), None);
    }
}
