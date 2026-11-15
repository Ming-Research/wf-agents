//! The accounts the agents run under, each a directory of their program's settings and sign-in, as the installation
//! lists them in the owner's order: which one is in use, and which are at a usage limit.
//!
//! The account in use is the installation's record, not its settings': the latest account the store says was taken
//! that the settings still list, by the owner on the page or by the driver moving on; with none, the first they list.
//! Every round and every keep-warm ping is given the account in use, so that a line whose session began under another
//! begins anew at its next round (module [`crate::sessions`]).
//!
//! An account is at its usage limit while a round under it met the limit and no round under it that began after came
//! back ([`crate::outage::limit_of`]), or while a check under it found it so; and until its reset, where one was said,
//! or, where none was, as long as a limit of a few hours lasts. Of the account in use, that is the cause of the agents'
//! wait; of another, it only keeps the driver from moving to it.
//!
//! With the switch that lets it on, at the week's limit of the account in use the driver checks the next listed
//! account after it, in the owner's order, that it does not know to be at a limit, by the same check it takes of the
//! outside, and takes the first that is found well; one found signed out is passed over until the wait ends, and one
//! found with the outside down is checked again after the wait's spacing. A limit of a few hours is waited out, as with
//! the switch off. How long such a limit lasts is the agents' program's, which its carrier gives in the policy.

use store::AccountTaken;
use work::Time;

use crate::Found;

/// A usage limit an account is at: when it was met, by a round under it or a check, and when it resets, where that
/// was said.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limit {
    pub met: Time,
    pub resets: Option<Time>,
}

impl Limit {
    /// When it is over: at its reset, or, where that was not said, as long after it was met as a limit of a few hours
    /// lasts, `short_ms`, which is how a limit whose reset is not known is taken.
    pub fn until(&self, short_ms: u64) -> Time {
        self.resets.unwrap_or(Time(self.met.0.saturating_add(short_ms)))
    }
}

/// What the latest check under an account the driver may move on to found: in the wait that began at `since`, at
/// `at`, and how many checks of it before in that wait found it other than well.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub since: Time,
    pub at: Time,
    pub found: Found,
    pub tries: u32,
}

/// The account in use: the latest taken that the listed accounts hold, else the first listed; none where none is
/// listed, when the agents run under their program's own directory.
pub fn in_use(listed: &[String], taken: &[AccountTaken]) -> Option<String> {
    (taken.iter().rev())
        .find(|t| listed.contains(&t.account))
        .map(|t| t.account.clone())
        .or_else(|| listed.first().cloned())
}

/// Whether a usage limit met at `met` that resets at `resets` is the week's: one of a few hours resets within
/// `short_ms` of being met, so a reset later than that is the week's. One whose reset is not known, or a week's that
/// resets as soon as a few hours' would, is waited out as a few hours' is.
pub fn weekly(met: Time, resets: Option<Time>, short_ms: u64) -> bool {
    resets.is_some_and(|resets| resets.0 > met.0.saturating_add(short_ms))
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

    const SHORT: u64 = 5 * 60 * 60 * 1000;

    #[test]
    fn a_limit_is_the_weeks_where_it_resets_later_than_a_few_hours_after_it_was_met() {
        let met = Time(1_000_000);
        assert!(!weekly(met, None, SHORT));
        assert!(!weekly(met, Some(Time(met.0 + SHORT)), SHORT));
        assert!(weekly(met, Some(Time(met.0 + SHORT + 1)), SHORT));
    }

    #[test]
    fn a_limit_whose_reset_is_not_known_is_over_as_long_after_it_was_met_as_a_few_hours_one_lasts() {
        let met = Time(1_000_000);
        assert_eq!(Limit { met, resets: None }.until(SHORT), Time(met.0 + SHORT));
        let resets = Some(Time(met.0 + 7));
        assert_eq!(Limit { met, resets }.until(SHORT), Time(met.0 + 7));
    }

    #[test]
    fn the_accounts_after_the_one_in_use_go_round_in_the_owners_order() {
        let listed = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(after(&listed, "b").collect::<Vec<_>>(), ["c", "a"]);
        assert_eq!(after(&listed, "a").collect::<Vec<_>>(), ["b", "c"]);
        assert_eq!(after(&listed[..1], "a").count(), 0);
    }
}
