//! The accounts the agents run under, each a directory of their program's settings and sign-in, as the installation
//! lists them in the owner's order: which one is in use, and which are at a usage limit.
//!
//! The account in use is the installation's record, not its settings': the latest account the store says was taken
//! that the settings still list, by the owner on the page or by the driver moving on; with none, the first they list.
//! Every round and every keep-warm ping is given the account in use, so that a line whose session began under another
//! begins anew at its next round (module [`crate::sessions`]).
//!
//! An account is at its usage limit while a round under it met the limit and no round under it that began after came
//! back ([`crate::outage::limit_of`]), or while a check under it found it so; and until its reset, where one was said.
//! Of the account in use, that is the cause of the agents' wait; of another, it only keeps the driver from moving to it.
//!
//! With the switch that lets it on, at the week's limit of the account in use the driver checks the next listed
//! account after it, in the owner's order, that it does not know to be at a limit, by the same check it takes of the
//! outside, and takes the first that is found well; one found otherwise is passed over until the wait ends. A limit of
//! a few hours is waited out, as with the switch off.

use store::AccountTaken;
use work::Time;

use crate::Found;

/// How long a usage limit of a few hours lasts at most from when it is met: a limit that resets later is the week's.
pub const HOURS_LIMIT: u64 = 5 * 60 * 60 * 1000;

/// The account in use: the latest taken that the listed accounts hold, else the first listed; none where none is listed,
/// when the agents run under their program's own directory.
pub fn in_use(listed: &[String], taken: &[AccountTaken]) -> Option<String> {
    (taken.iter().rev())
        .find(|t| listed.contains(&t.account))
        .map(|t| t.account.clone())
        .or_else(|| listed.first().cloned())
}

/// Whether a usage limit met at `met` that resets at `resets` is the week's: one of a few hours resets within
/// [`HOURS_LIMIT`] of being met, so a reset later than that is the week's. One whose reset is not known, or a week's
/// that resets as soon as a few hours' would, is waited out as a few hours' is.
pub fn weekly(met: Time, resets: Option<Time>) -> bool {
    resets.is_some_and(|resets| resets.0 > met.0.saturating_add(HOURS_LIMIT))
}

/// When the usage limit a check found resets, where it found the account at its limit and said when.
pub fn found_resets(found: &Found) -> Option<Time> {
    match found {
        Found::Down {
            failure: decide::run::Failure::UsageLimit,
            resets,
            ..
        } => *resets,
        _ => None,
    }
}

/// The accounts after the one in use, in the owner's order, going round: those the driver may move on to, in the order
/// it tries them.
pub fn after<'a>(listed: &'a [String], in_use: &str) -> impl Iterator<Item = &'a String> {
    let at = listed.iter().position(|a| a == in_use).unwrap_or(0);
    (1..listed.len()).map(move |k| &listed[(at + k) % listed.len()])
}

#[cfg(test)]
mod tests {
    use super::*;
    use store::Taker;

    fn taken(account: &str) -> AccountTaken {
        AccountTaken {
            account: account.to_string(),
            by: Taker::Owner,
            at: Time(1),
            left: None,
            resets: None,
        }
    }

    #[test]
    fn the_account_in_use_is_the_latest_taken_that_is_listed_and_else_the_first_listed() {
        let listed = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(in_use(&listed, &[]).as_deref(), Some("a"));
        assert_eq!(in_use(&listed, &[taken("c"), taken("b")]).as_deref(), Some("b"));
        assert_eq!(in_use(&listed, &[taken("b"), taken("gone")]).as_deref(), Some("b"));
        assert_eq!(in_use(&listed, &[taken("gone")]).as_deref(), Some("a"));
        assert_eq!(in_use(&[], &[taken("a")]), None);
    }

    #[test]
    fn a_limit_is_the_weeks_where_it_resets_later_than_a_few_hours_after_it_was_met() {
        let met = Time(1_000_000);
        assert!(!weekly(met, None));
        assert!(!weekly(met, Some(Time(met.0 + HOURS_LIMIT))));
        assert!(weekly(met, Some(Time(met.0 + HOURS_LIMIT + 1))));
    }

    #[test]
    fn the_accounts_after_the_one_in_use_go_round_in_the_owners_order() {
        let listed = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(after(&listed, "b").collect::<Vec<_>>(), ["c", "a"]);
        assert_eq!(after(&listed, "a").collect::<Vec<_>>(), ["b", "c"]);
        assert_eq!(after(&listed[..1], "a").count(), 0);
    }
}
