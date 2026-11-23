//! What the program says of the commit it was built from. The installation gives it to the build in `WF_BUILD_COMMIT`
//! (`wf-agents --version` and the engine's start line say it); the build does not run git, so that nothing here depends
//! on the state of the checkout. Shared by the build script and, in the tests, by the program, which hold it to what it
//! should say.

/// What the program says where the installation gave no commit.
pub const UNKNOWN: &str = "unknown";

/// The commit the installation gave: the first line of what it gave, trimmed; `unknown` where it gave none, or only
/// white space.
pub fn commit(given: Option<&str>) -> String {
    let first = given.and_then(|text| text.lines().next()).unwrap_or_default().trim();
    if first.is_empty() {
        UNKNOWN.to_string()
    } else {
        first.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_commit_given_is_the_commit_said() {
        assert_eq!(commit(Some("0123abc")), "0123abc");
        assert_eq!(commit(Some("  0123abc-dirty \n")), "0123abc-dirty");
        // Only the first line: a command that said more than a commit does not put its words into the version.
        assert_eq!(commit(Some("0123abc\nsomething else")), "0123abc");
    }

    #[test]
    fn no_commit_given_is_said_to_be_unknown() {
        assert_eq!(commit(None), "unknown");
        assert_eq!(commit(Some("")), "unknown");
        assert_eq!(commit(Some("  \n")), "unknown");
    }
}
