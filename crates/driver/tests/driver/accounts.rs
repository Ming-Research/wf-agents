//! The accounts the agents run under: a usage limit holds the rounds only while the account that met it is in use, the
//! owner's take of another ends its wait at once, and with the switch on, at the week's limit of the account in use,
//! the driver checks the next account in the owner's order and takes the first found well, passing over those it knows
//! to be at a limit and those a check found otherwise; a limit of a few hours is waited out. What is taken and the
//! switch are kept, and come back as the driver opens again.

use decide::run::Failure;
use driver::Found;
use driver::outage::PROBE;
use engine::RanOn;
use store::{AccountTaken, Storage, Taker};

use super::*;

const HOUR: u64 = 3_600_000;

/// The policy with the accounts a, b and c listed in this order, every round on one model.
fn three() -> driver::Policy {
    driver::Policy {
        samples: 1,
        limit: 500_000,
        on: Box::new(|_, _| {
            Some(RanOn {
                level: "1".to_string(),
                model: "opus".to_string(),
                effort: None,
                account: None,
            })
        }),
        accounts: ["a", "b", "c"].map(str::to_string).to_vec(),
        account: None,
        short_limit_ms: 5 * 60 * 60 * 1000,
        warm: None,
    }
}

/// A driver with the three accounts, and two works of the plain flow let on, each with its writer's first round under
/// way.
fn two() -> (H, WorkNumber, WorkNumber) {
    let mut h = H::with_policy(flows(), 2, Some(three));
    let (_, a) = h.add(new_item("a", &[], "plain"));
    let (_, b) = h.add(new_item("b", &[], "plain"));
    h.let_on(a);
    h.let_on(b);
    (h, a, b)
}

/// The round of the work under way, its try begun under the account, fails for its usage limit, which resets at
/// `resets`.
fn limited(h: &mut H, work: WorkNumber, account: &str, resets: Option<Time>) -> Vec<Todo> {
    let (run, ..) = h.round(work);
    let on = RanOn {
        level: "1".to_string(),
        model: "opus".to_string(),
        effort: None,
        account: Some(account.to_string()),
    };
    h.on(work, Event::Ran { run, attempt: 1, on });
    let spent = h.spent();
    let event = Event::Failed {
        run,
        why: "You've hit your limit".to_string(),
        failure: Failure::UsageLimit,
        resets,
        spent,
    };
    h.on(work, event)
}

/// A reset a few days from the harness's clock: the week's.
fn in_days(h: &H) -> Time {
    Time(h.clock + 3 * 24 * HOUR)
}

/// The rounds of agents' cells given among these, by work.
fn agents_rounds(todos: &[Todo]) -> Vec<WorkNumber> {
    (todos.iter())
        .filter_map(|t| match &t.effect {
            Effect::Round { cell, .. } if cell.as_str() != "merge" => Some(t.work),
            _ => None,
        })
        .collect()
}

fn take(h: &mut H, account: &str) -> Vec<Todo> {
    let now = h.now();
    let todos = h.dm().take_account(now, account).expect("the account is taken");
    h.check();
    todos
}

fn move_on(h: &mut H, on: bool) -> Vec<Todo> {
    let now = h.now();
    let todos = h.dm().move_on(now, on).expect("the switch is set");
    h.check();
    todos
}

/// What the store keeps of the accounts taken.
fn kept(h: &H) -> Vec<AccountTaken> {
    h.reader.accounts_taken().expect("the accounts taken are read")
}

#[test]
fn a_usage_limit_holds_the_rounds_only_while_its_account_is_in_use_and_the_owners_take_ends_its_wait_at_once() {
    let (mut h, a, b) = two();
    assert_eq!(
        h.d().account().as_deref(),
        Some("a"),
        "the first listed is in use until another is taken"
    );
    let resets = in_days(&h);
    assert_eq!(agents_rounds(&limited(&mut h, a, "a", Some(resets))), vec![]);
    assert!(h.d().outage().is_some_and(|outage| outage.holds()));
    // The owner takes b: the limit is a's, so nothing holds the rounds, and the one that met it is given again.
    let todos = take(&mut h, "b");
    assert_eq!(h.d().account().as_deref(), Some("b"));
    assert!(
        h.d().outage().is_none(),
        "a limit of an account not in use holds nothing"
    );
    assert_eq!(agents_rounds(&todos), vec![a]);
    let now = h.now();
    let accounts = h.d().accounts(now);
    let standing: Vec<(&str, bool, bool, Option<Time>)> = (accounts.iter())
        .map(|a| (a.account.as_str(), a.in_use, a.limited, a.resets))
        .collect();
    assert_eq!(
        standing,
        [
            ("a", false, true, Some(resets)),
            ("b", true, false, None),
            ("c", false, false, None)
        ]
    );
    let owners = |account: &str| (account.to_string(), Taker::Owner, None, None);
    let of = |taken: Vec<AccountTaken>| -> Vec<(String, Taker, Option<String>, Option<Time>)> {
        taken.into_iter().map(|t| (t.account, t.by, t.left, t.resets)).collect()
    };
    assert_eq!(of(kept(&h)), [owners("b")]);
    // Taken back before its reset, a holds the rounds again at once: the round under b that came back is not a's.
    h.hands_in(b, &[("text", None)]);
    take(&mut h, "a");
    assert!(h.d().outage().is_some_and(|outage| outage.holds()));
    // Taking the one in use keeps nothing more, and one the settings do not list is refused.
    take(&mut h, "a");
    assert_eq!(of(kept(&h)), [owners("b"), owners("a")]);
    let now = h.now();
    assert_eq!(
        h.dm().take_account(now, "z"),
        Err(Failed::NoSuchAccount("z".to_string()))
    );
}

#[test]
fn with_the_switch_on_the_weeks_limit_of_the_account_in_use_has_the_next_checked_and_taken_once_it_is_found_well() {
    let (mut h, a, _) = two();
    assert!(!h.d().moving_on(), "the switch is off until the owner sets it");
    move_on(&mut h, true);
    let resets = in_days(&h);
    limited(&mut h, a, "a", Some(resets));
    let met = h.d().outage().expect("the wait stands").met;
    let wake = h.d().wake().expect("the driver asks to be woken");
    assert!(wake.check);
    assert_eq!((wake.at, wake.account.as_deref()), (Time(met.0 + PROBE), Some("b")));
    let todos = h.checked_under(wake.at, Some("b"), Found::Well);
    assert_eq!(h.d().account().as_deref(), Some("b"));
    assert_eq!(
        agents_rounds(&todos),
        vec![a],
        "the round that met the limit is given again under b"
    );
    let taken = kept(&h);
    assert_eq!(taken.len(), 1);
    assert_eq!(
        (
            taken[0].account.as_str(),
            taken[0].by,
            taken[0].left.as_deref(),
            taken[0].resets
        ),
        ("b", Taker::Engine, Some("a"), Some(resets))
    );
}

#[test]
fn one_found_at_its_limit_or_signed_out_is_passed_over_and_with_none_left_the_account_in_use_waits_for_its_reset() {
    let (mut h, a, _) = two();
    move_on(&mut h, true);
    let resets = in_days(&h);
    limited(&mut h, a, "a", Some(resets));
    let first = h.d().wake().expect("a wake");
    assert_eq!(first.account.as_deref(), Some("b"));
    let at_its_limit = Found::Down {
        failure: Failure::UsageLimit,
        why: "You've hit your limit".to_string(),
        resets: Some(Time(resets.0 + HOUR)),
    };
    h.checked_under(first.at, Some("b"), at_its_limit);
    let second = h.d().wake().expect("a wake");
    assert_eq!(second.account.as_deref(), Some("c"));
    assert!(second.at > first.at);
    let signed_out = Found::Down {
        failure: Failure::CannotRun,
        why: "Not logged in".to_string(),
        resets: None,
    };
    h.checked_under(second.at, Some("c"), signed_out);
    let last = h.d().wake().expect("a wake");
    assert_eq!((last.account.as_deref(), last.at), (Some("a"), resets));
    assert_eq!(h.d().account().as_deref(), Some("a"));
    assert!(kept(&h).is_empty(), "nothing was taken");
    let now = h.now();
    let b = h.d().accounts(now).into_iter().find(|n| n.account == "b").expect("b");
    assert!(b.limited, "what the check found of b is said");
}

#[test]
fn a_limit_of_a_few_hours_or_the_switch_off_leaves_the_account_in_use_waiting_for_its_reset() {
    for (moving, resets_in) in [(true, 2 * HOUR), (false, 3 * 24 * HOUR)] {
        let (mut h, a, _) = two();
        move_on(&mut h, moving);
        let resets = Time(h.now().0 + resets_in);
        limited(&mut h, a, "a", Some(resets));
        let wake = h.d().wake().expect("a wake");
        assert_eq!((wake.account.as_deref(), wake.at), (Some("a"), resets), "{moving}");
        h.checked_under(resets, Some("a"), Found::Well);
        assert_eq!(h.d().account().as_deref(), Some("a"));
        assert!(kept(&h).is_empty());
    }
}

#[test]
fn an_account_the_records_say_is_at_its_limit_is_not_checked_and_the_switch_set_during_a_wait_moves_on() {
    let (mut h, a, b) = two();
    // b met the week's limit while it was in use.
    take(&mut h, "b");
    let b_resets = in_days(&h);
    limited(&mut h, b, "b", Some(b_resets));
    take(&mut h, "a");
    let resets = in_days(&h);
    limited(&mut h, a, "a", Some(resets));
    assert_eq!(
        h.d().wake().expect("a wake").account.as_deref(),
        Some("a"),
        "the switch is off"
    );
    move_on(&mut h, true);
    let wake = h.d().wake().expect("a wake");
    assert_eq!(wake.account.as_deref(), Some("c"), "b is passed over: its limit stands");
    h.checked_under(wake.at, Some("c"), Found::Well);
    assert_eq!(h.d().account().as_deref(), Some("c"));
}

#[test]
fn the_account_taken_and_the_switch_come_back_as_the_driver_opens_again() {
    let (mut h, _, _) = two();
    take(&mut h, "c");
    move_on(&mut h, true);
    h.restart();
    assert_eq!(h.d().account().as_deref(), Some("c"));
    assert!(h.d().moving_on());
    assert_eq!(h.d().accounts_taken().len(), 1);
}

#[test]
fn an_account_taken_back_after_its_limits_reset_is_checked_at_the_next_moment_and_the_works_go_on() {
    let (mut h, a, _) = two();
    let resets = in_days(&h);
    limited(&mut h, a, "a", Some(resets));
    take(&mut h, "b");
    // The round that met the limit, given again under b, comes back: under b, it ends nothing of a's.
    h.hands_in(a, &[("text", None)]);
    // Time passes beyond a's reset, the driver told it: a's limit stands in the records, no round under a having come
    // back since.
    let later = Time(resets.0 + HOUR);
    h.tick(later, None);
    take(&mut h, "a");
    let outage = h.d().outage().expect("a's wait stands again");
    assert!(outage.holds());
    let wake = h
        .d()
        .wake()
        .expect("a wake is set though the wait's own time has passed");
    assert!(wake.check);
    assert_eq!((wake.account.as_deref(), wake.at), (Some("a"), Time(later.0 + 1)));
    h.tick(wake.at, Some(Found::Well));
    assert!(
        h.d().outage().is_none_or(|outage| !outage.holds()),
        "the check found a well: the works go on"
    );
}

#[test]
fn an_account_found_with_the_outside_down_is_checked_again_and_one_whose_limit_resets_sooner_at_its_reset() {
    let (mut h, a, b) = two();
    // c met the week's limit while in use, resetting a day before a's will.
    take(&mut h, "c");
    let soon = Time(h.clock + 2 * 24 * HOUR);
    limited(&mut h, b, "c", Some(soon));
    take(&mut h, "a");
    move_on(&mut h, true);
    let resets = in_days(&h);
    limited(&mut h, a, "a", Some(resets));
    let first = h.d().wake().expect("a wake");
    assert_eq!(first.account.as_deref(), Some("b"));
    let offline = Found::Down {
        failure: Failure::Offline,
        why: "no network".to_string(),
        resets: None,
    };
    h.checked_under(first.at, Some("b"), offline);
    let again = h.d().wake().expect("a wake");
    assert_eq!(
        again.account.as_deref(),
        Some("b"),
        "the outside down passes over no account"
    );
    assert!(again.at > first.at);
    let signed_out = Found::Down {
        failure: Failure::CannotRun,
        why: "Not logged in".to_string(),
        resets: None,
    };
    h.checked_under(again.at, Some("b"), signed_out);
    // b is signed out and c at its limit: c is checked at its reset, which comes before a's.
    let at_reset = h.d().wake().expect("a wake");
    assert_eq!((at_reset.account.as_deref(), at_reset.at), (Some("c"), soon));
    h.checked_under(soon, Some("c"), Found::Well);
    assert_eq!(h.d().account().as_deref(), Some("c"));
}

#[test]
fn the_driver_moves_on_after_a_stop_between_the_limit_and_the_take_and_takes_nothing_once_the_switch_is_off() {
    let (mut h, a, _) = two();
    move_on(&mut h, true);
    let resets = in_days(&h);
    limited(&mut h, a, "a", Some(resets));
    h.restart();
    let wake = h.d().wake().expect("a wake");
    assert_eq!(
        wake.account.as_deref(),
        Some("b"),
        "the move is worked out again from what is kept"
    );
    // The switch is turned off while the check of b is under way: found well, it is taken no more.
    move_on(&mut h, false);
    h.checked_under(wake.at, Some("b"), Found::Well);
    assert_eq!(h.d().account().as_deref(), Some("a"));
    assert!(kept(&h).is_empty());
}
