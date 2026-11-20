//! The account the agents' program makes its calls under, as the carrier names what the program reads of it. The owner
//! adds accounts, each a name and a long-lived token the program makes, kept in the login keychain ([`crate::keychain`]);
//! the account in use is the engine's own state, handed out with every round (decision
//! the-account-is-handed-out-with-the-round), and the program is given its token in a variable of its environment. With
//! none in use, the program runs on the sign-in of its directory of settings, `agents.config_dir`, which holds the
//! program's settings and sessions under every account, and is given it in a variable of its own.

use std::path::Path;
use std::sync::Arc;

use service::{Token, Tokens};

/// The variable of its environment that names the program's directory to it.
pub const VARIABLE: &str = "CLAUDE_CONFIG_DIR";

/// The variable of its environment that gives the program the token of the account it runs under, which it takes
/// above the sign-in of its directory.
pub const TOKEN: &str = "CLAUDE_CODE_OAUTH_TOKEN";

/// What the program is run with, the browser signed in to an account, to make a token of the account: it prints the
/// token and keeps it nowhere.
pub const MAKE_TOKEN: &str = "setup-token";

/// The variables that give the program a credential it takes above a token, which would run every round under that
/// credential and not under the account's token.
pub const ABOVE_TOKEN: [&str; 2] = ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"];

/// How long a token the program makes lasts from when it is made, in milliseconds: a year, as the program's
/// documentation says of `setup-token`.
pub const LIFE_MS: u64 = service::YEAR_MS;

/// The account a directory is signed in as, by the address the program keeps for it; none where it is not signed in,
/// or what the program keeps there cannot be read.
pub fn signed_in_as(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(".claude.json")).ok()?;
    let kept: serde_json::Value = serde_json::from_str(&text).ok()?;
    (kept["oauthAccount"]["emailAddress"].as_str())
        .filter(|address| !address.trim().is_empty())
        .map(str::to_string)
}

/// The token of the account of this name, as what keeps the tokens gives it, off the threads that carry the rounds;
/// none where no account is named, and the program runs on its directory's sign-in. Where it cannot be given, why, in
/// the words of what keeps it, which never fall back to the directory's sign-in.
pub async fn token_of(tokens: &Arc<dyn Tokens>, account: Option<&str>) -> Result<Option<Token>, String> {
    let Some(name) = account else {
        return Ok(None);
    };
    let (tokens, named) = (tokens.clone(), name.to_string());
    let given = tokio::task::spawn_blocking(move || tokens.token(&named)).await;
    match given {
        Ok(Ok(token)) => Ok(Some(token)),
        Ok(Err(why)) => Err(format!("the token of the account {name} could not be read: {why}")),
        Err(e) => Err(format!("the token of the account {name} could not be read: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_directory_says_the_account_it_is_signed_in_as() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(signed_in_as(dir.path()), None, "nothing kept, not signed in");
        std::fs::write(
            dir.path().join(".claude.json"),
            r#"{"oauthAccount":{"emailAddress":" "}}"#,
        )
        .unwrap();
        assert_eq!(signed_in_as(dir.path()), None, "an empty address is no account");
        std::fs::write(
            dir.path().join(".claude.json"),
            r#"{"oauthAccount":{"emailAddress":"owner@example.com"},"other":1}"#,
        )
        .unwrap();
        assert_eq!(signed_in_as(dir.path()).as_deref(), Some("owner@example.com"));
        std::fs::write(dir.path().join(".claude.json"), "not json").unwrap();
        assert_eq!(signed_in_as(dir.path()), None);
    }

    // A few milliseconds: a thread of the runtime's blocking pool.
    #[tokio::test]
    async fn a_token_is_given_of_the_account_named_and_none_without_one_and_its_lack_is_said_without_it() {
        let kept = service::KeptTokens::new();
        kept.keep("work", &Token::new("sk-ant-oat01-secret".to_string()))
            .unwrap();
        let tokens: Arc<dyn Tokens> = Arc::new(kept);
        assert_eq!(token_of(&tokens, None).await, Ok(None));
        let given = token_of(&tokens, Some("work")).await.unwrap().expect("a token");
        assert_eq!(given.secret(), "sk-ant-oat01-secret");
        let lacking = token_of(&tokens, Some("home")).await.unwrap_err();
        assert!(
            lacking.starts_with("the token of the account home could not be read: "),
            "{lacking}"
        );
        assert!(!lacking.contains("sk-ant"), "{lacking}");
    }
}
