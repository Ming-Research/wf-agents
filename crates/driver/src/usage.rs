//! What the agents' service last said of each account's usage, which the owner reads to choose the account the agents
//! run under (decision usage-is-the-services-latest-word): the windows its latest rate limit event under the account
//! named, each with how much of it is used and when it resets, or, where that event refused the account's calls and
//! named no window, that the account is at its limit; and when that was said. Every call made under an account may
//! say it, under the account it was handed out with: a try of a round, a keep-warm ping, a fork, a check of the
//! outside, the check of a token as its account is added, and a look.
//!
//! A look is the check's short request under an account, taken when the owner opens the accounts, under each account
//! whose usage was last heard of more than [`LOOK_AFTER_MS`] before, or never; all at once, none twice at a time, with
//! none in use as well, that being when the owner chooses one. Each look that ends says whether it failed, and why.
//!
//! No record holds it: it is what the outside says at a moment, kept only in memory, as where a round stands is
//! ([`crate::along`]), and nothing that decides reads it. Lost in a stop, it is said again at the next call under the
//! account, or the next look.

use std::collections::BTreeMap;

use work::Time;

use crate::Found;
use crate::accounts::Accounts;

/// How long after an account's usage was last heard of a look is taken under it as the owner opens the accounts, in
/// milliseconds: usage moves slowly, and a look costs a short request (judgment).
pub const LOOK_AFTER_MS: u64 = 5 * 60 * 1000;

/// A window of an account's usage, as the service names it.
#[derive(Clone, Debug, PartialEq)]
pub struct Window {
    /// Its name, as the service gives it.
    pub name: String,
    /// How much of it is used, from none at 0 to all at 1, where the service says.
    pub utilization: Option<f64>,
    /// When it resets, where the service says.
    pub resets: Option<Time>,
    /// Whether the service lets the account's calls through in it without a warning.
    pub allowed: bool,
}

/// What the service said of an account's usage in one rate limit event that said anything of it.
#[derive(Clone, Debug, PartialEq)]
pub enum Said {
    /// The windows it named.
    Windows(Vec<Window>),
    /// It refused the account's calls and named no window: the account is at its limit, which resets then, where it
    /// said.
    Limit { resets: Option<Time> },
}

/// What is known of an account's usage.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Usage {
    /// What the service last said of it, and when; none where it never said anything.
    pub seen: Option<(Time, Said)>,
    /// Whether a look under the account is under way.
    pub looking: bool,
    /// When the latest look under the account ended, whatever it found.
    pub looked: Option<Time>,
    /// Why the latest look under the account failed, where it did.
    pub failed: Option<String>,
}

impl Usage {
    /// When the account's usage was last heard of, by a call that said it or by a look that ended.
    fn last(&self) -> Option<Time> {
        let seen = self.seen.as_ref().map(|(at, _)| *at);
        seen.max(self.looked)
    }
}

/// The usage of every account, by its name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Usages(BTreeMap<String, Usage>);

impl Usages {
    /// What is known of the usage of the account of this name.
    pub fn of(&self, name: &str) -> Option<&Usage> {
        self.0.get(name)
    }

    /// Takes what the service said of the usage of the account of this name at `now`, where it is added and not
    /// removed; gives whether what it says changed: the same said again, as each call of a round says it, changes only
    /// when it was said, which is no news to tell.
    pub fn heard(&mut self, accounts: &Accounts, now: Time, name: &str, said: Said) -> bool {
        if !accounts.is_live(name) {
            return false;
        }
        let usage = self.0.entry(name.to_string()).or_default();
        let changed = usage.seen.as_ref().map(|(_, was)| was) != Some(&said);
        usage.seen = Some((now, said));
        changed
    }

    /// The accounts a look is to be taken under at `now`, each marked as being looked at: those added and not removed
    /// whose usage was last heard of more than [`LOOK_AFTER_MS`] before, or never, and under which none is under way.
    pub fn looks(&mut self, accounts: &Accounts, now: Time) -> Vec<String> {
        let mut names = Vec::new();
        for live in accounts.live() {
            let usage = self.0.entry(live.name.clone()).or_default();
            let stale = (usage.last()).is_none_or(|at| now.0.saturating_sub(at.0) > LOOK_AFTER_MS);
            if stale && !usage.looking {
                usage.looking = true;
                names.push(live.name);
            }
        }
        names
    }

    /// Takes what a look under the account of this name found at `now`, with what the service said of its usage, where
    /// it said anything; gives whether what is known changed. The look's own outcome says whether it failed: one that
    /// found the outside well, or the account at its usage limit, which is a word on its usage, failed nothing, and
    /// puts an earlier failure by.
    pub fn looked(&mut self, accounts: &Accounts, now: Time, name: &str, found: &Found, said: Option<Said>) -> bool {
        if !accounts.is_live(name) {
            return self.0.remove(name).is_some();
        }
        let usage = self.0.entry(name.to_string()).or_default();
        let was = usage.clone();
        usage.looking = false;
        usage.looked = Some(now);
        if let Some(said) = said {
            usage.seen = Some((now, said));
        }
        usage.failed = match found {
            Found::Down { failure, why, .. } if *failure != decide::run::Failure::UsageLimit => Some(why.clone()),
            _ => None,
        };
        *usage != was
    }

    /// Lets go of what is known of every account that is no longer added.
    pub fn keep_only(&mut self, accounts: &Accounts) {
        self.0.retain(|name, _| accounts.is_live(name));
    }
}

#[cfg(test)]
mod tests {
    use store::{AccountAct, AccountRecord};

    use super::*;

    fn accounts(names: &[&str]) -> Accounts {
        Accounts::new(
            (names.iter())
                .map(|name| AccountRecord {
                    name: name.to_string(),
                    what: AccountAct::Added,
                    at: Time(0),
                })
                .collect(),
        )
    }

    fn week(utilization: f64, allowed: bool) -> Said {
        Said::Windows(vec![Window {
            name: "seven_day".to_string(),
            utilization: Some(utilization),
            resets: Some(Time(9_000_000)),
            allowed,
        }])
    }

    fn down(failure: decide::run::Failure, why: &str) -> Found {
        Found::Down {
            failure,
            why: why.to_string(),
            resets: None,
        }
    }

    // Microseconds, nothing run: what decides when a look spends a request, and what a look leaves known.
    #[test]
    fn a_look_is_taken_under_each_account_last_heard_of_long_enough_ago_or_never_and_none_twice() {
        let accounts = accounts(&["work", "home", "away"]);
        let mut usages = Usages::default();
        assert!(usages.heard(&accounts, Time(1_000), "work", week(0.04, true)));
        assert!(
            !usages.heard(&accounts, Time(1_000), "work", week(0.04, true)),
            "said again, it is no news"
        );
        assert!(
            !usages.heard(&accounts, Time(1_000), "gone", week(0.5, true)),
            "no such account"
        );
        let at = Time(1_000 + LOOK_AFTER_MS);
        assert_eq!(
            usages.looks(&accounts, at),
            ["home", "away"],
            "work was heard of five minutes ago"
        );
        assert_eq!(
            usages.looks(&accounts, at),
            Vec::<String>::new(),
            "none twice at a time"
        );
        assert!(usages.of("home").unwrap().looking);
        // A look that failed says why, and is not taken again until it is stale.
        let offline = down(
            decide::run::Failure::Offline,
            "The network does not reach the agents' service.",
        );
        assert!(usages.looked(&accounts, at, "away", &offline, None));
        let away = usages.of("away").unwrap();
        assert_eq!((away.looking, away.seen.clone()), (false, None));
        assert_eq!(
            away.failed.as_deref(),
            Some("The network does not reach the agents' service.")
        );
        // A look that met the usage limit is a word on the usage, and failed nothing.
        let limit = Found::Down {
            failure: decide::run::Failure::UsageLimit,
            why: "You've hit your weekly limit".to_string(),
            resets: Some(Time(9_000_000)),
        };
        let at_limit = Said::Limit {
            resets: Some(Time(9_000_000)),
        };
        assert!(usages.looked(&accounts, at, "home", &limit, Some(at_limit.clone())));
        let home = usages.of("home").unwrap();
        assert_eq!((home.failed.clone(), home.seen.clone()), (None, Some((at, at_limit))));
        let later = Time(at.0 + LOOK_AFTER_MS + 1);
        assert_eq!(usages.looks(&accounts, later), ["work", "home", "away"]);
        // A later look that finds the outside well puts the earlier failure by, though it heard nothing.
        assert!(usages.looked(&accounts, later, "away", &Found::Well, None));
        assert_eq!(usages.of("away").unwrap().failed, None);
        // A look that fails after one that heard the usage keeps what was heard, and says why it failed.
        usages.looks(&accounts, Time(later.0 + LOOK_AFTER_MS + 1));
        let refused = down(
            decide::run::Failure::CannotRun,
            "the token of the account work could not be read",
        );
        assert!(usages.looked(&accounts, later, "work", &refused, None));
        let work = usages.of("work").unwrap();
        assert_eq!(work.seen, Some((Time(1_000), week(0.04, true))));
        assert_eq!(
            work.failed.as_deref(),
            Some("the token of the account work could not be read")
        );
    }

    // Microseconds, nothing run: a removed account's usage would otherwise stand for a new account of its name.
    #[test]
    fn what_is_known_of_an_account_removed_is_let_go_of() {
        let mut usages = Usages::default();
        usages.heard(&accounts(&["work", "home"]), Time(1), "home", week(0.1, true));
        usages.keep_only(&accounts(&["work"]));
        assert_eq!(usages.of("home"), None);
    }
}
