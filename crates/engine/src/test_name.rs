//! What a test is called: the one place that says which ways of writing a test's name the engine reads.
//!
//! An agent names a test as its test runner prints it, and the engine reads the name in two places: against the
//! source on the work's branch ([`on_branch`], for the acceptance table), and against what a run of CI reported
//! ([`matches_run`], for the results of `ci_before`). A name is written as one of:
//!
//! - the bare name: `building_a_message_writes_nothing`;
//! - a path that ends in it: `engine::e2e::building_a_message_writes_nothing`;
//! - the test binary and the name, as cargo-nextest prints them: `engine::e2e building_a_message_writes_nothing`;
//! - the titles, as vitest prints them: `[file > ]describe > … > it title`.
//!
//! A runner whose names are none of these is read as the text after the last `:`, `/` or `.` of the name.

use std::path::Path;

use crate::git;

/// What a refusal says is read, so that a name refused is not handed in again in the same form.
const FORMS: &str = "write a test as its bare name, a path ending in it, `binary name` as nextest prints it, or \
                     `describe > it` as vitest prints it";

/// Whether the test a nextest log ends, by its binary and its name, is the one `named` names.
pub fn matches_run(named: &str, binary: &str, test: &str) -> bool {
    test == named
        || format!("{binary} {test}") == named
        || test.ends_with(&format!("::{named}"))
        || named.ends_with(&format!("::{test}"))
}

/// Whether the test `named` is on the branch: some word it may stand for is in a file at HEAD of the worktree. A
/// name that leaves no word after its last `:`, `/` or `.` is not looked for, as it never was; that is too lax, and
/// item #11's to tighten.
pub fn on_branch(worktree: &Path, named: &str) -> Result<(), String> {
    if last_segment(named).is_empty() {
        return Ok(());
    }
    let words = source_words(named);
    let mut args = vec!["grep", "-q", "-F", "-w"];
    for word in &words {
        args.extend(["-e", word.as_str()]);
    }
    args.push("HEAD");
    git::git(worktree, &args)
        .map(|_| ())
        .map_err(|_| format!("the test {named} is not on the branch: {FORMS}"))
}

/// The text after the last `:`, `/` or `.` of a name: the one word the engine first looked for, and still does, so
/// that a name once read stays read.
fn last_segment(named: &str) -> &str {
    named.rsplit([':', '/', '.']).next().unwrap_or(named).trim()
}

/// The words that may stand for the test `named` in a source file, the most specific first; a test is on the branch
/// when any of them is.
fn source_words(named: &str) -> Vec<String> {
    let named = named.trim();
    let parts: Vec<&str> = named.split(" > ").map(str::trim).collect();
    let mut words = Vec::new();
    if parts.len() > 1 {
        // Vitest prints `[file > ]describe > it`: the source holds the title alone, quoted as the source quotes it.
        words.extend(quoted_titles(parts[parts.len() - 1]));
    } else if let Some((_, name)) = named.split_once(' ')
        && !name.is_empty()
        && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == ':')
    {
        // Nextest prints `binary name`: the source defines `fn name`.
        words.push(format!("fn {}", name.rsplit("::").next().unwrap_or(name)));
    }
    words.push(named.to_string());
    words.push(last_segment(named).to_string());
    words.retain(|w| !w.is_empty());
    let mut seen: Vec<String> = Vec::new();
    words.retain(|w| {
        let new = !seen.contains(w);
        if new {
            seen.push(w.clone());
        }
        new
    });
    words
}

/// A title as it may stand in a source file: as it is printed, and with the quote of each kind of string escaped as
/// a source escapes it inside that string.
fn quoted_titles(title: &str) -> Vec<String> {
    let mut out = vec![title.to_string()];
    for quote in ['\'', '"', '`'] {
        let escaped = title.replace('\\', "\\\\").replace(quote, &format!("\\{quote}"));
        if !out.contains(&escaped) {
            out.push(escaped);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    #[test]
    fn a_name_in_a_log_matches_by_its_name_a_path_ending_in_it_or_its_binary_and_name() {
        let held = |named| matches_run(named, "engine::e2e", "a_test");
        assert!(held("a_test"));
        assert!(held("engine::e2e::a_test"));
        assert!(held("engine::e2e a_test"));
        assert!(!held("a_tes"));
        assert!(!held("e2e a_test"));
        assert!(!held("other::a_test_not"));
    }

    // Each way of writing a name gives the word the source holds; the old reading stays among the words.
    #[test]
    fn the_words_a_name_may_stand_for_in_a_source_file() {
        // A bare name and a path: the name as written, and the text after the last separator.
        assert_eq!(source_words("a_test"), ["a_test"]);
        assert_eq!(source_words("tools::tests::a_test"), ["tools::tests::a_test", "a_test"]);
        // Nextest's `binary name`: the function the source defines, then the name as written and as read before.
        assert_eq!(
            source_words("engine::e2e a_test"),
            ["fn a_test", "engine::e2e a_test", "e2e a_test"]
        );
        assert_eq!(source_words("engine tools::tests::a_test")[0], "fn a_test");
        // Vitest's titles: the last one, quoted each way a source quotes it.
        let vitest = source_words("src/x.test.tsx > a page > shows the owner's list");
        assert_eq!(vitest[0], "shows the owner's list");
        assert_eq!(vitest[1], "shows the owner\\'s list");
        assert!(vitest.contains(&"src/x.test.tsx > a page > shows the owner's list".to_string()));
        // The last word of a title with spaces is not looked for alone, nor is a function named after it.
        assert!(!source_words("a page > shows the list").contains(&"list".to_string()));
        assert!(!source_words("shows the list").iter().any(|w| w.starts_with("fn ")));
        // A name that ends in a separator leaves no word after it.
        assert_eq!(last_segment("a title that ends in a stop."), "");
    }

    fn git_in(dir: &Path, args: &[&str]) {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// A repository whose only commit holds a Rust test and some vitest tests, the way the project has them.
    fn repo_with_tests(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("wf-agents-test-name-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("crates/engine/tests")).unwrap();
        std::fs::create_dir_all(dir.join("client/src")).unwrap();
        std::fs::write(
            dir.join("crates/engine/tests/e2e.rs"),
            "#[test]\nfn building_a_message_writes_nothing() {}\n\n#[test]\nasync fn waits_for_a_slot() {}\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("client/src/page.test.ts"),
            concat!(
                "describe('a round picked beside a chart', () => {\n",
                "  it(\"shows on the architect's page the same way\", () => {});\n",
                "  it('is one page, with its rounds on a work\\'s chart', () => {});\n",
                "  it('lists the rounds (all of them)', () => {});\n",
                "  it(`names the \\`cell\\``, () => {});\n",
                "});\n",
            ),
        )
        .unwrap();
        git_in(&dir, &["init", "-q"]);
        git_in(&dir, &["add", "-A"]);
        git_in(&dir, &["commit", "-qm", "tests"]);
        dir
    }

    // A name the branch has is accepted as nextest, vitest or a path writes it; the title a source quotes with an
    // escape is found by the title as printed, and a title that ends or begins in a mark is too.
    #[test]
    fn a_test_on_the_branch_is_found_under_each_way_of_writing_its_name() {
        let dir = repo_with_tests("found");
        for named in [
            "building_a_message_writes_nothing",
            "e2e::building_a_message_writes_nothing",
            "engine::e2e building_a_message_writes_nothing",
            "engine::e2e::building_a_message_writes_nothing",
            "engine e2e::waits_for_a_slot",
            "a round picked beside a chart > shows on the architect's page the same way",
            "client/src/page.test.ts > a round picked beside a chart > shows on the architect's page the same way",
            "a round picked beside a chart > is one page, with its rounds on a work's chart",
            "client/src/page.test.ts > a round picked beside a chart > lists the rounds (all of them)",
            "a round picked beside a chart > names the `cell`",
        ] {
            assert!(on_branch(&dir, named).is_ok(), "{named}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    // A name the branch lacks is refused whichever way it is written, and the refusal lists the ways to write one.
    #[test]
    fn a_test_not_on_the_branch_is_refused_under_each_way_of_writing_its_name() {
        let dir = repo_with_tests("missing");
        for named in [
            "missing_test",
            "engine::e2e::missing_test",
            "engine::e2e missing_test",
            "a round picked beside a chart > shows nothing at all",
            "client/src/page.test.ts > a round picked beside a chart > missing",
            // A title with a word only (`list`, `page`) that the source has elsewhere is not the test.
            "a round picked beside a chart > shows the list",
            "engine::e2e shows the page",
        ] {
            let why = on_branch(&dir, named).unwrap_err();
            assert!(why.contains(&format!("the test {named} is not on the branch")), "{why}");
            for form in ["bare name", "a path ending in it", "`binary name`", "`describe > it`"] {
                assert!(why.contains(form), "{why}");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Whatever the old reading (the text after the last `:`, `/` or `.`, searched whole at HEAD; no search when
    // nothing is left of it) let through, the present reading lets through. The old reading is written out here as
    // git is asked, not taken from the code under test.
    #[test]
    fn a_name_the_old_reading_let_through_is_let_through() {
        let dir = repo_with_tests("superset");
        let old = |named: &str| {
            let word = named.rsplit([':', '/', '.']).next().unwrap_or(named).trim();
            word.is_empty()
                || Command::new("git")
                    .arg("-C")
                    .arg(&dir)
                    .args(["grep", "-q", "-F", "-w", word, "HEAD"])
                    .status()
                    .unwrap()
                    .success()
        };
        let (mut let_through, mut refused) = (0, 0);
        for named in [
            "building_a_message_writes_nothing",
            "e2e::building_a_message_writes_nothing",
            "crates/engine/tests/e2e.rs::waits_for_a_slot",
            "crates/engine/tests/e2e.rs",
            "client/src/page.test.ts",
            "tests/e2e.rs",
            "the rounds (all of them)",
            "missing_test",
            "engine::e2e missing_test",
            "a title that ends in a stop.",
            "a title that ends in a colon:",
            "src/x.test.tsx > describe > it.",
            ".",
            "",
            "   ",
            "-leading",
            "a_test.",
            "cell",
        ] {
            if old(named) {
                let_through += 1;
                assert!(on_branch(&dir, named).is_ok(), "{named:?}");
            } else {
                refused += 1;
            }
        }
        // The table holds names the old reading refused as well as names it let through, so that neither list is
        // empty and the comparison says something.
        assert!(
            let_through > 0 && refused > 0,
            "{let_through} let through, {refused} refused"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
