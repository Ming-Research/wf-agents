//! The accounts the agents' program may run under, as the store keeps them: each a name the owner gives, whose token is
//! kept outside the engine, added, removed and taken by the owner as the one the agents run under (decision
//! the-account-in-use-is-the-engines-own-state). They are the engine's own state, as the pause of everything is: the
//! driver reads them as it opens and changes them as the owner acts, and the page asks the driver.
//!
//! The account in use is the latest taken that has not been removed since. It is the only credential the agents run
//! under (decision no-default-credential): with none in use, a round, a ping or a fork that needs it is handed out all
//! the same, and fails as its token cannot be given. A try of a round records the account it ran under by its name,
//! apart from the directory of the program's settings it ran with; a try kept before accounts were, none.

use store::{AccountAct, AccountRecord};
use work::Time;

/// The accounts, as the records the store keeps of them say.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Accounts {
    records: Vec<AccountRecord>,
}

/// Why a command of a session runs under the account in use and not the one the session's last try ran under, whose
/// cache holds the session: that one was removed, by its name; or the try ran under none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Instead {
    Removed(String),
    NoAccount,
}

/// An account that has been added and not removed since: its name, and when it was added.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Live {
    pub name: String,
    pub added: Time,
}

/// Why a name is no account's name, in words; none where it is one: small letters, digits and dashes, beginning with a
/// letter or a digit, so that it is safe in the line that keeps its token.
pub fn name_fault(name: &str) -> Option<String> {
    let first = name.chars().next();
    let shaped = first.is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && (name.chars()).all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    (!shaped).then(|| {
        format!(
            "{name:?} is no account's name: a name is small letters, digits and dashes, beginning with a letter or a \
             digit"
        )
    })
}

/// Why a text pasted as a token is none, in words that hold nothing of it; none where it may be one: some text, with no
/// space and no control character, as every token the agents' program makes is.
pub fn token_fault(token: &str) -> Option<String> {
    let shaped = !token.is_empty() && (token.chars()).all(|c| !c.is_whitespace() && !c.is_control());
    (!shaped).then(|| "the token is empty, or holds a space or a control character: copy it again whole".to_string())
}

impl Accounts {
    /// The accounts the records say, in the order they were kept.
    pub fn new(records: Vec<AccountRecord>) -> Accounts {
        Accounts { records }
    }

    /// Every record, in the order they were kept.
    pub fn records(&self) -> &[AccountRecord] {
        &self.records
    }

    /// Adds a record after the others.
    pub(crate) fn push(&mut self, record: AccountRecord) {
        self.records.push(record);
    }

    /// The account the agents run under: the latest taken that has not been removed since; none where none is, and no
    /// agent's call can be made.
    pub fn in_use(&self) -> Option<&str> {
        let removed_after = |name: &str, i: usize| {
            (self.records[i + 1..].iter()).any(|r| r.name == name && r.what == AccountAct::Removed)
        };
        (self.records.iter().enumerate().rev())
            .find(|(i, r)| r.what == AccountAct::Taken && !removed_after(&r.name, *i))
            .map(|(_, r)| r.name.as_str())
    }

    /// The accounts added and not removed since, in the order they were added.
    pub fn live(&self) -> Vec<Live> {
        let mut live: Vec<Live> = Vec::new();
        for record in &self.records {
            match record.what {
                AccountAct::Added => {
                    live.retain(|l| l.name != record.name);
                    live.push(Live {
                        name: record.name.clone(),
                        added: record.at,
                    });
                }
                AccountAct::Removed => live.retain(|l| l.name != record.name),
                AccountAct::Taken => {}
            }
        }
        live
    }

    /// Whether an account of this name is added and not removed since.
    pub fn is_live(&self, name: &str) -> bool {
        self.live().iter().any(|l| l.name == name)
    }

    /// The names whose latest record is a removal: the accounts whose tokens are to be let go of, until they are.
    pub fn removed(&self) -> Vec<String> {
        let mut removed: Vec<String> = Vec::new();
        for record in &self.records {
            removed.retain(|name| *name != record.name);
            if record.what == AccountAct::Removed {
                removed.push(record.name.clone());
            }
        }
        removed
    }

    /// The account a command of a session runs under, the session's last try having run under this one: that account
    /// where it is still added; else the account in use, none where none is, with why it runs under that one instead:
    /// the session's account was removed, or the try ran under none.
    pub fn of_session(&self, account: Option<&str>) -> (Option<String>, Option<Instead>) {
        let in_use = self.in_use().map(str::to_string);
        match account {
            Some(name) if self.is_live(name) => (Some(name.to_string()), None),
            Some(name) => (in_use, Some(Instead::Removed(name.to_string()))),
            None => (in_use, Some(Instead::NoAccount)),
        }
    }

    /// When the owner last took an account as the one the agents run under; none where none was ever taken.
    pub fn taken_at(&self) -> Option<Time> {
        (self.records.iter().rev())
            .find(|r| r.what == AccountAct::Taken)
            .map(|r| r.at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(name: &str, what: AccountAct, at: u64) -> AccountRecord {
        AccountRecord {
            name: name.to_string(),
            what,
            at: Time(at),
        }
    }

    #[test]
    fn the_account_in_use_is_the_latest_taken_that_was_not_removed_since() {
        let mut accounts = Accounts::default();
        assert_eq!(accounts.in_use(), None, "none before any is taken");
        accounts.push(record("work", AccountAct::Added, 1));
        assert_eq!(accounts.in_use(), None, "an account added is not taken");
        accounts.push(record("home", AccountAct::Added, 2));
        accounts.push(record("work", AccountAct::Taken, 3));
        assert_eq!(accounts.in_use(), Some("work"));
        accounts.push(record("home", AccountAct::Taken, 4));
        assert_eq!(accounts.in_use(), Some("home"));
        accounts.push(record("work", AccountAct::Removed, 5));
        assert_eq!(accounts.in_use(), Some("home"));
        // A take of a name before it was removed and added again is no take of the account added again.
        accounts.push(record("home", AccountAct::Removed, 6));
        accounts.push(record("home", AccountAct::Added, 7));
        assert_eq!(accounts.in_use(), None);
        let names: Vec<(String, Time)> = (accounts.live().into_iter()).map(|l| (l.name, l.added)).collect();
        assert_eq!(names, [("home".to_string(), Time(7))]);
        assert_eq!(accounts.taken_at(), Some(Time(4)));
    }

    #[test]
    fn a_session_runs_under_its_account_else_the_one_in_use_saying_why_and_a_removed_token_is_let_go_of() {
        let mut accounts = Accounts::default();
        accounts.push(record("work", AccountAct::Added, 1));
        accounts.push(record("home", AccountAct::Added, 2));
        accounts.push(record("home", AccountAct::Taken, 3));
        accounts.push(record("old", AccountAct::Added, 4));
        accounts.push(record("old", AccountAct::Removed, 5));
        assert_eq!(accounts.of_session(Some("work")), (Some("work".to_string()), None));
        assert_eq!(
            accounts.of_session(Some("old")),
            (Some("home".to_string()), Some(Instead::Removed("old".to_string()))),
            "a removed account's session runs under the one in use, saying which was removed"
        );
        assert_eq!(
            accounts.of_session(None),
            (Some("home".to_string()), Some(Instead::NoAccount)),
            "a session last run under no account runs under the one in use, saying so"
        );
        assert_eq!(
            Accounts::default().of_session(None),
            (None, Some(Instead::NoAccount)),
            "with none in use, under none: its token cannot be given"
        );
        assert_eq!(accounts.removed(), ["old"]);
        accounts.push(record("old", AccountAct::Added, 6));
        assert_eq!(
            accounts.removed(),
            Vec::<String>::new(),
            "added again, its token is the new one"
        );
        accounts.push(record("work", AccountAct::Removed, 7));
        assert_eq!(accounts.removed(), ["work"]);
    }

    #[test]
    fn a_token_is_some_text_with_no_space_and_no_control_character_and_its_fault_says_nothing_of_it() {
        assert_eq!(token_fault("sk-ant-oat01-a_b-1"), None);
        for token in ["", "sk-ant oat01", "sk-ant\n", "sk\tant", " sk-ant"] {
            let fault = token_fault(token).expect("no token");
            assert!(!fault.contains("sk"), "{fault}");
        }
    }

    #[test]
    fn a_name_is_small_letters_digits_and_dashes_beginning_with_a_letter_or_a_digit() {
        for name in ["work", "a", "2nd", "max-1"] {
            assert_eq!(name_fault(name), None, "{name}");
        }
        for name in ["", "-a", "Work", "a b", "a/b", "a\"b", "é"] {
            assert!(name_fault(name).is_some(), "{name:?}");
        }
    }
}
