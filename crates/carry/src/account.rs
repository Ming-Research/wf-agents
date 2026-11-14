//! The account the agents' program makes its calls under: a directory of its settings and sign-in, named to it by a
//! variable of its environment. The installation names one as `agents.config_dir`; the owner signs a directory in
//! once, by hand, by running the program with the variable naming the directory and typing the command that signs in,
//! and the program keeps there the account it is signed in as.

use std::path::Path;

/// The variable of its environment that names the program's directory to it.
pub const VARIABLE: &str = "CLAUDE_CONFIG_DIR";

/// What the owner types in the program, run with the variable naming a directory, to sign that directory in.
pub const SIGN_IN: &str = "/login";

/// The account a directory is signed in as, by the address the program keeps for it; none where it is not signed in,
/// or what the program keeps there cannot be read.
pub fn signed_in_as(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(".claude.json")).ok()?;
    let kept: serde_json::Value = serde_json::from_str(&text).ok()?;
    (kept["oauthAccount"]["emailAddress"].as_str())
        .filter(|address| !address.trim().is_empty())
        .map(str::to_string)
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
}
