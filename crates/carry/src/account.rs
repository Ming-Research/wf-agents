//! The account the agents' program makes its calls under, as the carrier names what the program reads of it. The owner
//! adds accounts, each a name and a long-lived token the program makes, kept in the login keychain
//! ([`crate::keychain`]); the account in use is the engine's own state, handed out with every round, and the program is
//! given the token of the account a call runs under in
//! a variable of its environment. The program runs under no credential but the owner's accounts (decision
//! no-default-credential): a round and a keep-warm ping under the account in use, a fork under its session's account or
//! the one in use, a look under each account added, and the check of a token under that token, before it is kept.
//! With no account in use, a round, a ping or a fork is given no token, and nothing of the program's is run.
//!
//! The program's directory of settings and sessions is the engine's, [`PROFILE`] in the data directory, which holds the
//! program's sessions under every account; the program is given it in a variable of its own, and its settings file
//! there may name no credential and no permission mode ([`settings_file_faults`]). What the service says of an account's usage, the program's stream
//! carries in each response's rate limit event, which is read here ([`said`]).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use driver::{Said, Window};
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
/// credential and not under the account's token: neither the settings nor the program's settings file may name them.
pub const ABOVE_TOKEN: [&str; 2] = ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"];

/// The key of the program's settings that names a command giving it a key, which it takes above a token.
pub const KEY_HELPER: &str = "apiKeyHelper";

/// The program's own settings file in its directory, which it reads as the user's settings.
pub const SETTINGS: &str = "settings.json";

/// How long a token the program makes lasts from when it is made, in milliseconds: a year, as the program's
/// documentation says of `setup-token`.
pub const LIFE_MS: u64 = service::YEAR_MS;

/// The program's directory of settings and sessions in this data directory.
pub fn profile(data: &Path) -> PathBuf {
    data.join(PROFILE)
}

/// The key of the program's settings that names the permission mode it starts in.
pub const DEFAULT_MODE: &str = "defaultMode";

/// Every reason the program's own settings file in this directory is wrong, each a sentence; none where there is no
/// file or it is right. The file is read and parsed once. A file that cannot be read, or is no JSON, gives that one
/// reason, since nothing else about it can be told. Otherwise: a credential that is none of the owner's accounts, a
/// command that gives the program a key ([`KEY_HELPER`]) or a credential above the token in its `env`, and a
/// permission mode (`permissions.`[`DEFAULT_MODE`], whatever its value), which has no effect, since the program
/// starts in default permission mode under [`crate::cli::SCRUB`], which the carrier always sets. The engine does not
/// start while there is a reason.
pub fn settings_file_faults(profile: &Path) -> Vec<String> {
    let at = profile.join(SETTINGS);
    let text = match std::fs::read_to_string(&at) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => return vec![format!("{} cannot be read: {e}.", at.display())],
    };
    let settings: Value = match serde_json::from_str(&text) {
        Ok(settings) => settings,
        Err(e) => {
            return vec![format!(
                "{} is no JSON, so whether it gives the agents' program a credential or a permission mode cannot be \
                 told: {e}.",
                at.display()
            )];
        }
    };
    let mut faults = Vec::new();
    let mut named: Vec<String> = Vec::new();
    if settings.get(KEY_HELPER).is_some() {
        named.push(KEY_HELPER.to_string());
    }
    named.extend(
        (ABOVE_TOKEN.iter())
            .filter(|name| settings["env"].get(**name).is_some())
            .map(|name| format!("env.{name}")),
    );
    if !named.is_empty() {
        faults.push(format!(
            "{} names {}, which would have the agents' program run under a credential other than the owner's accounts: \
             take it out of the file.",
            at.display(),
            named.join(" and ")
        ));
    }
    if settings["permissions"].get(DEFAULT_MODE).is_some() {
        faults.push(format!(
            "{} names permissions.{DEFAULT_MODE}, which has no effect: the engine runs the program with {}, which \
             makes it start in default permission mode whatever is given; take it out of the file, and {}.",
            at.display(),
            crate::cli::SCRUB,
            crate::cli::PERMISSION_ADVICE
        ));
    }
    faults
}

/// The token of the account of this name, as what keeps the tokens gives it, off the threads that carry the rounds; or
/// why it cannot be given, in the words of what keeps it, or, with no account named, in the words `none` beside the
/// flows say that none is in use.
pub async fn token_of(tokens: &Arc<dyn Tokens>, account: Option<&str>, none: &str) -> Result<Token, String> {
    let Some(name) = account else {
        return Err(none.to_string());
    };
    let (tokens, named) = (tokens.clone(), name.to_string());
    let given = tokio::task::spawn_blocking(move || tokens.token(&named)).await;
    match given {
        Ok(Ok(token)) => Ok(token),
        Ok(Err(why)) => Err(format!("the token of the account {name} could not be read: {why}")),
        Err(e) => Err(format!("the token of the account {name} could not be read: {e}")),
    }
}

/// What a line of the program's stream says of the usage of the account it runs under, where it is a rate limit event
/// that says anything of it: each window of its `unifiedWindows`, by the name the service gives it, with its
/// `utilization`, its `resetsAt` (in seconds) and whether its `status` is `allowed`, in the order of their names; where
/// it names none there, the one its own `rateLimitType` names, with the event's own fields; and where it names no
/// window and refuses the account's calls (`rejected`), that the account is at its limit, with its reset where it says.
/// None of another line, or of an event that says nothing of the usage.
pub fn said(line: &Value) -> Option<Said> {
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
        return Some(Said::Windows(named));
    }
    if let Some(name) = info["rateLimitType"].as_str() {
        return Some(Said::Windows(vec![window(name, info)]));
    }
    (info["status"] == "rejected").then(|| Said::Limit {
        resets: (info["resetsAt"].as_u64()).map(|at| Time(at.saturating_mul(1000))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the flow's words say where no account is in use.
    const NONE: &str = "No account is in use: choose one on the Settings view.";

    // A few milliseconds: a thread of the runtime's blocking pool. It guards that a token is given only of the account
    // named, and that no words say it.
    #[tokio::test]
    async fn a_token_is_given_of_the_account_named_and_none_without_one_and_its_lack_is_said_without_it() {
        let kept = service::KeptTokens::new();
        kept.keep("work", &Token::new("sk-ant-oat01-secret".to_string()))
            .unwrap();
        let tokens: Arc<dyn Tokens> = Arc::new(kept);
        assert_eq!(token_of(&tokens, None, NONE).await, Err(NONE.to_string()));
        let given = token_of(&tokens, Some("work"), NONE).await.expect("a token");
        assert_eq!(given.secret(), "sk-ant-oat01-secret");
        let lacking = token_of(&tokens, Some("home"), NONE).await.unwrap_err();
        assert_eq!(
            lacking,
            "the token of the account home could not be read: no token is kept under the name home"
        );
    }

    // Microseconds, nothing run: the only reading of the service's word on an account's usage.
    #[test]
    fn a_rate_limit_event_says_each_window_of_usage_as_the_service_gives_it_or_the_limit_where_it_names_none() {
        // As a round kept under the data directory's rounds has it, with a second window beside it.
        let event: Value = serde_json::from_str(
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","unifiedWindows":{"seven_day":{"utilization":0.67,"resetsAt":1795129200,"status":"allowed"},"five_hour":{"utilization":0.9,"resetsAt":1795000000,"status":"allowed_warning"}}}}"#,
        )
        .unwrap();
        assert_eq!(
            said(&event),
            Some(Said::Windows(vec![
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
            ]))
        );
        // An event that refuses names its window by its own fields.
        let refused: Value = serde_json::from_str(
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":1793394000,"rateLimitType":"seven_day"}}"#,
        )
        .unwrap();
        assert_eq!(
            said(&refused),
            Some(Said::Windows(vec![Window {
                name: "seven_day".to_string(),
                utilization: None,
                resets: Some(Time(1_793_394_000_000)),
                allowed: false,
            }]))
        );
        // One that refuses and names no window says the account is at its limit, with its reset.
        let limit: Value = serde_json::from_str(
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":1793394000}}"#,
        )
        .unwrap();
        assert_eq!(
            said(&limit),
            Some(Said::Limit {
                resets: Some(Time(1_793_394_000_000))
            })
        );
        for none in [
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed"}}"#,
            r#"{"type":"result","rate_limit_info":{"rateLimitType":"seven_day"}}"#,
        ] {
            assert_eq!(said(&serde_json::from_str(none).unwrap()), None, "{none}");
        }
    }

    // Microseconds, one small file read: the engine must not start where the program's own settings would put it
    // under a credential of no account's, or name a permission mode the program ignores under the variable the
    // carrier sets.
    #[test]
    fn the_programs_settings_file_that_names_a_credential_or_a_permission_mode_is_said_in_sentences() {
        let dir = tempfile::tempdir().unwrap();
        let at = dir.path().join("settings.json");
        assert_eq!(settings_file_faults(dir.path()), Vec::<String>::new(), "no file");
        std::fs::write(
            &at,
            r#"{"cleanupPeriodDays":365000,"env":{"HTTPS_PROXY":"x"},"permissions":{"allow":["Edit(./**)"]}}"#,
        )
        .unwrap();
        assert_eq!(
            settings_file_faults(dir.path()),
            Vec::<String>::new(),
            "no credential and no mode, though it holds permission rules"
        );
        let credential = format!(
            "{} names apiKeyHelper and env.ANTHROPIC_API_KEY and env.ANTHROPIC_AUTH_TOKEN, which would have the \
             agents' program run under a credential other than the owner's accounts: take it out of the file.",
            at.display()
        );
        let mode = format!(
            "{} names permissions.defaultMode, which has no effect: the engine runs the program with \
             CLAUDE_CODE_SUBPROCESS_ENV_SCRUB, which makes it start in default permission mode whatever is given; \
             take it out of the file, and allow what agents may do with --allowedTools rules in agents.args, such \
             as Edit(./**).",
            at.display()
        );
        std::fs::write(
            &at,
            r#"{"apiKeyHelper":"./key.sh","env":{"ANTHROPIC_API_KEY":"k","ANTHROPIC_AUTH_TOKEN":"t"}}"#,
        )
        .unwrap();
        assert_eq!(settings_file_faults(dir.path()), vec![credential.clone()]);
        // A mode is refused whatever its value, as the arguments' --permission-mode is.
        for value in ["acceptEdits", "default"] {
            std::fs::write(&at, format!(r#"{{"permissions":{{"defaultMode":"{value}"}}}}"#)).unwrap();
            assert_eq!(settings_file_faults(dir.path()), vec![mode.clone()], "{value}");
        }
        // Both are said together, in one list.
        std::fs::write(
            &at,
            r#"{"apiKeyHelper":"./key.sh","env":{"ANTHROPIC_API_KEY":"k","ANTHROPIC_AUTH_TOKEN":"t"},
                "permissions":{"defaultMode":"plan"}}"#,
        )
        .unwrap();
        assert_eq!(settings_file_faults(dir.path()), [credential, mode]);
        // A file that is no JSON gives that one reason, since nothing else of it can be told.
        std::fs::write(&at, "{").unwrap();
        assert_eq!(
            settings_file_faults(dir.path()),
            [format!(
                "{} is no JSON, so whether it gives the agents' program a credential or a permission mode cannot be \
                 told: EOF while parsing an object at line 1 column 1.",
                at.display()
            )]
        );
    }
}
