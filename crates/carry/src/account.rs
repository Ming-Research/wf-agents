//! The account the agents' program makes its calls under, as the carrier names what the program reads of it. The owner
//! adds accounts, each a name and a long-lived token the program makes, kept in the login keychain
//! ([`crate::keychain`]); the account in use is the engine's own state, handed out with every round (decision
//! the-account-is-handed-out-with-the-round), and the program is given its token in a variable of its environment. It
//! is the only credential the agents run under (decision no-default-credential): with none in use, no token is given,
//! and nothing of the program's is run.
//!
//! The program's directory of settings and sessions is the engine's, [`PROFILE`] in the data directory, which holds the
//! program's sessions under every account; the program is given it in a variable of its own. What the service says of
//! an account's usage, the program's stream carries in each response's rate limit event, which is read here
//! ([`windows`]).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use driver::Window;
use serde_json::Value;
use service::{Token, Tokens};
use work::Time;

/// The variable of its environment that names the program's directory to it.
pub const VARIABLE: &str = "CLAUDE_CONFIG_DIR";

/// The program's directory of settings and sessions, in the data directory: the engine keeps it, and makes it where it
/// is missing (decision the-programs-directory-is-the-engines).
pub const PROFILE: &str = "profile";

/// How many days the program keeps a session before its own cleanup deletes it, as the engine gives it in the settings
/// it passes: a thousand years, so that the session of a work that waits long is never deleted under it.
pub const KEEP_DAYS: u64 = 365_000;

/// The variable of its environment that gives the program the token of the account it runs under.
pub const TOKEN: &str = "CLAUDE_CODE_OAUTH_TOKEN";

/// What the program is run with, the browser signed in to an account, to make a token of the account: it prints the
/// token and keeps it nowhere.
pub const MAKE_TOKEN: &str = "setup-token";

/// The variables that give the program a credential it takes above a token, which would run every round under that
/// credential and not under the account's token: the settings may not name them.
pub const ABOVE_TOKEN: [&str; 2] = ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"];

/// Why a call is not made with no account in use, in the carrier's words.
pub const NO_ACCOUNT: &str = "no account is in use";

/// How long a token the program makes lasts from when it is made, in milliseconds: a year, as the program's
/// documentation says of `setup-token`.
pub const LIFE_MS: u64 = service::YEAR_MS;

/// The program's directory of settings and sessions in this data directory.
pub fn profile(data: &Path) -> PathBuf {
    data.join(PROFILE)
}

/// The token of the account of this name, as what keeps the tokens gives it, off the threads that carry the rounds; or
/// why it cannot be given, in the words of what keeps it, or, with no account named, that none is in use.
pub async fn token_of(tokens: &Arc<dyn Tokens>, account: Option<&str>) -> Result<Token, String> {
    let Some(name) = account else {
        return Err(NO_ACCOUNT.to_string());
    };
    let (tokens, named) = (tokens.clone(), name.to_string());
    let given = tokio::task::spawn_blocking(move || tokens.token(&named)).await;
    match given {
        Ok(Ok(token)) => Ok(token),
        Ok(Err(why)) => Err(format!("the token of the account {name} could not be read: {why}")),
        Err(e) => Err(format!("the token of the account {name} could not be read: {e}")),
    }
}

/// The windows of usage a line of the program's stream names, where it is a rate limit event that names any: each
/// window of its `unifiedWindows`, by the name the service gives it, with its `utilization`, its `resetsAt` (in
/// seconds) and whether its `status` is `allowed`, in the order of their names; where it names none there, the one its
/// own `rateLimitType` names, with the event's own fields. None of another line, or of an event that names no window.
pub fn windows(line: &Value) -> Option<Vec<Window>> {
    if line["type"] != "rate_limit_event" {
        return None;
    }
    let info = &line["rate_limit_info"];
    let window = |name: &str, of: &Value| Window {
        name: name.to_string(),
        utilization: of["utilization"].as_f64(),
        resets: (of["resetsAt"].as_u64()).map(|at| Time(at.saturating_mul(1000))),
        allowed: of["status"] == "allowed",
    };
    let mut named: Vec<Window> = (info["unifiedWindows"].as_object().into_iter().flatten())
        .map(|(name, of)| window(name, of))
        .collect();
    named.sort_by(|a, b| a.name.cmp(&b.name));
    if !named.is_empty() {
        return Some(named);
    }
    let name = info["rateLimitType"].as_str()?;
    Some(vec![window(name, info)])
}

#[cfg(test)]
mod tests {
    use super::*;

    // A few milliseconds: a thread of the runtime's blocking pool.
    #[tokio::test]
    async fn a_token_is_given_of_the_account_named_and_none_without_one_and_its_lack_is_said_without_it() {
        let kept = service::KeptTokens::new();
        kept.keep("work", &Token::new("sk-ant-oat01-secret".to_string()))
            .unwrap();
        let tokens: Arc<dyn Tokens> = Arc::new(kept);
        assert_eq!(token_of(&tokens, None).await, Err("no account is in use".to_string()));
        let given = token_of(&tokens, Some("work")).await.expect("a token");
        assert_eq!(given.secret(), "sk-ant-oat01-secret");
        let lacking = token_of(&tokens, Some("home")).await.unwrap_err();
        assert!(
            lacking.starts_with("the token of the account home could not be read: "),
            "{lacking}"
        );
        assert!(!lacking.contains("sk-ant"), "{lacking}");
    }

    #[test]
    fn a_rate_limit_event_names_each_window_of_usage_as_the_service_gives_it() {
        // As a round kept under the data directory's rounds has it, with a second window beside it.
        let event: Value = serde_json::from_str(
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","unifiedWindows":{"seven_day":{"utilization":0.67,"resetsAt":1795129200,"status":"allowed"},"five_hour":{"utilization":0.9,"resetsAt":1795000000,"status":"allowed_warning"}}}}"#,
        )
        .unwrap();
        assert_eq!(
            windows(&event),
            Some(vec![
                Window {
                    name: "five_hour".to_string(),
                    utilization: Some(0.9),
                    resets: Some(Time(1_795_000_000_000)),
                    allowed: false,
                },
                Window {
                    name: "seven_day".to_string(),
                    utilization: Some(0.67),
                    resets: Some(Time(1_795_129_200_000)),
                    allowed: true,
                },
            ])
        );
        // An event that refuses names its window by its own fields.
        let refused: Value = serde_json::from_str(
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":1793394000,"rateLimitType":"seven_day"}}"#,
        )
        .unwrap();
        assert_eq!(
            windows(&refused),
            Some(vec![Window {
                name: "seven_day".to_string(),
                utilization: None,
                resets: Some(Time(1_793_394_000_000)),
                allowed: false,
            }])
        );
        for none in [
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed"}}"#,
            r#"{"type":"result","rate_limit_info":{"rateLimitType":"seven_day"}}"#,
        ] {
            assert_eq!(windows(&serde_json::from_str(none).unwrap()), None, "{none}");
        }
    }
}
