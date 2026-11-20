//! What the agents' service last said of each account's usage, which the owner reads to choose the account the agents
//! run under (decision usage-is-the-services-latest-word): the windows the latest word under the account named, each
//! with how much of it is used and when it resets, and when that was said. Every call made under an account may say it,
//! under the account it was handed out with: a try of a round, a keep-warm ping, a fork, a check of the outside, the
//! check of a token as its account is added, and a look.
//!
//! A look is the check's short request under an account, taken when the owner opens the accounts, under each account
//! whose usage was last heard more than [`LOOK_AFTER_MS`] before, or never; all at once, none twice at a time, with
//! none in use as well, that being when the owner chooses one.
//!
//! No record holds it: it is what the outside says at a moment, kept only in memory, as where a round stands is
//! ([`crate::along`]), and nothing that decides reads it. Lost in a stop, it is said again at the next call under the
//! account, or the next look.

use std::collections::BTreeMap;

use work::Time;

use crate::Found;
use crate::accounts::Accounts;

/// How long after an account's usage was last heard a look is taken under it as the owner opens the accounts, in
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

/// What is known of an account's usage.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Usage {
    /// The windows the service last named under the account, and when it named them; none where it never did.
    pub seen: Option<(Time, Vec<Window>)>,
    /// Whether a look under the account is under way.
    pub looking: bool,
    /// When the latest look under the account ended, whatever it found.
    pub looked: Option<Time>,
    /// Why the latest look failed, where it did and nothing was heard under the account since.
    pub failed: Option<String>,
}

impl Usage {
    /// When the account's usage was last heard of, by a call that named its windows or by a look that ended.
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

    /// Takes the windows the service named under the account of this name at `now`, where it is added and not removed;
    /// gives whether what it said changed: the same windows said again, as each call of a round says them, change only
    /// when they were said, which is no news to tell.
    pub fn heard(&mut self, accounts: &Accounts, now: Time, name: &str, windows: Vec<Window>) -> bool {
        if !accounts.is_live(name) || windows.is_empty() {
            return false;
        }
        let usage = self.0.entry(name.to_string()).or_default();
        let changed = usage.seen.as_ref().map(|(_, said)| said) != Some(&windows) || usage.failed.is_some();
        usage.seen = Some((now, windows));
        usage.failed = None;
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

    /// Takes what a look under the account of this name found at `now`, with the windows the service named, where it
    /// named any; gives whether what is known changed. A look that met the account's usage limit failed nothing: the
    /// limit is a window's, as the service names it.
    pub fn looked(&mut self, accounts: &Accounts, now: Time, name: &str, found: &Found, windows: Vec<Window>) -> bool {
        if !accounts.is_live(name) {
            return self.0.remove(name).is_some();
        }
        let usage = self.0.entry(name.to_string()).or_default();
        let was = usage.clone();
        usage.looking = false;
        usage.looked = Some(now);
        if !windows.is_empty() {
            usage.seen = Some((now, windows));
            usage.failed = None;
        } else if let Found::Down { failure, why, .. } = found
            && *failure != decide::run::Failure::UsageLimit
        {
            usage.failed = Some(why.clone());
        }
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

    fn week(utilization: f64, allowed: bool) -> Window {
        Window {
            name: "seven_day".to_string(),
            utilization: Some(utilization),
            resets: Some(Time(9_000_000)),
            allowed,
        }
    }

    #[test]
    fn a_look_is_taken_under_each_account_last_heard_of_long_enough_ago_or_never_and_none_twice() {
        let accounts = accounts(&["work", "home", "away"]);
        let mut usages = Usages::default();
        assert!(usages.heard(&accounts, Time(1_000), "work", vec![week(0.04, true)]));
        assert!(
            !usages.heard(&accounts, Time(1_000), "work", vec![week(0.04, true)]),
            "said again, it is no news"
        );
        assert!(
            !usages.heard(&accounts, Time(1_000), "gone", vec![week(0.5, true)]),
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
        // A look that failed is no word on the account's usage, and is not taken again until it is stale.
        let down = Found::Down {
            failure: decide::run::Failure::Offline,
            why: "The network does not reach the agents' service.".to_string(),
            resets: None,
        };
        assert!(usages.looked(&accounts, at, "away", &down, Vec::new()));
        let away = usages.of("away").unwrap();
        assert_eq!((away.looking, away.seen.clone()), (false, None));
        assert_eq!(
            away.failed.as_deref(),
            Some("The network does not reach the agents' service.")
        );
        // A look that met the usage limit says the window at its limit, and failed nothing.
        let limit = Found::Down {
            failure: decide::run::Failure::UsageLimit,
            why: "You've hit your weekly limit".to_string(),
            resets: Some(Time(9_000_000)),
        };
        assert!(usages.looked(&accounts, at, "home", &limit, vec![week(1.0, false)]));
        let home = usages.of("home").unwrap();
        assert_eq!(
            (home.failed.clone(), home.seen.clone()),
            (None, Some((at, vec![week(1.0, false)])))
        );
        let later = Time(at.0 + LOOK_AFTER_MS + 1);
        assert_eq!(usages.looks(&accounts, later), ["work", "home", "away"]);
        // A word heard under an account since its look failed puts the failure by.
        assert!(usages.heard(&accounts, later, "away", vec![week(0.2, true)]));
        assert_eq!(usages.of("away").unwrap().failed, None);
    }

    #[test]
    fn what_is_known_of_an_account_removed_is_let_go_of() {
        let mut usages = Usages::default();
        usages.heard(&accounts(&["work", "home"]), Time(1), "home", vec![week(0.1, true)]);
        usages.keep_only(&accounts(&["work"]));
        assert_eq!(usages.of("home"), None);
    }
}
