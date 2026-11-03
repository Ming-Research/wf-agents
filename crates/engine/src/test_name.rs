//! What a test is called: the one place that says which ways of writing a test's name the engine reads.
//!
//! An agent names a test as its test runner prints it, and the engine reads the name in three places: against the
//! source on the work's branch ([`on_branch`], for the acceptance table), against the files the work changed
//! ([`in_files`], for the tests adversarial testing is given), and against what a run of CI reported
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

/// The files among `files`, as HEAD of the worktree has them, that hold a word the test `named` may stand for: the
/// words [`on_branch`] looks for, in the files only. None for a name that leaves no word to look for, which is not
/// looked for, as [`on_branch`] does not; some for any other, with no file in the list when no file of `files` holds it.
pub fn in_files(worktree: &Path, files: &[String], named: &str) -> Result<Option<Vec<String>>, String> {
    if last_segment(named).is_empty() {
        return Ok(None);
    }
    git::grep_files(worktree, &source_words(named), files).map(Some)
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
        // Vitest prints `[file > ]describe > it`: the source holds the title alone, in quotes.
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

/// A title as a source file holds it: in a string of each kind of quote, with that quote (and the backslash) escaped
/// as a source escapes it inside that string. The quotes are part of the word, so that the same words in a comment
/// or a document, which no quote holds, are not the title of a test.
fn quoted_titles(title: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for quote in ['\'', '"', '`'] {
        let escaped = title.replace('\\', "\\\\").replace(quote, &format!("\\{quote}"));
        let quoted = format!("{quote}{escaped}{quote}");
        if !out.contains(&quoted) {
            out.push(quoted);
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
        assert_eq!(vitest[0], "'shows the owner\\'s list'");
        assert_eq!(vitest[1], "\"shows the owner's list\"");
        assert_eq!(vitest[2], "`shows the owner's list`");
        // The title is not a word of its own, outside any quote.
        assert!(!vitest.contains(&"shows the owner's list".to_string()));
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

    // Item #274: a name is looked for in the given files with the words `on_branch` looks for in the whole branch, so
    // the files are those that hold a word of it, and a name `on_branch` accepts is one some file holds, except the
    // name that leaves no word to look for, which is not looked for at all.
    #[test]
    fn a_test_is_looked_for_in_the_given_files_by_the_words_the_whole_branch_is() {
        let dir = repo_with_tests("in-files");
        let rust = "crates/engine/tests/e2e.rs".to_string();
        let page = "client/src/page.test.ts".to_string();
        let both = [rust.clone(), page.clone()];
        assert_eq!(
            in_files(&dir, &both, "building_a_message_writes_nothing").unwrap(),
            Some(vec![rust.clone()])
        );
        assert_eq!(
            in_files(&dir, &both, "engine::e2e waits_for_a_slot").unwrap(),
            Some(vec![rust.clone()])
        );
        let title = "a round picked beside a chart > shows on the architect's page the same way";
        assert_eq!(in_files(&dir, &both, title).unwrap(), Some(vec![page.clone()]));
        // Only the files given are looked in.
        assert_eq!(in_files(&dir, &[rust.clone()], title).unwrap(), Some(Vec::new()));
        assert_eq!(in_files(&dir, &[], title).unwrap(), Some(Vec::new()));
        // A name that leaves no word after its last separator is not looked for, as `on_branch` does not.
        assert_eq!(in_files(&dir, &both, "a title that ends in a stop.").unwrap(), None);
        for named in [
            "building_a_message_writes_nothing",
            "engine::e2e::building_a_message_writes_nothing",
            "a round picked beside a chart > names the `cell`",
            "client/src/page.test.ts > a round picked beside a chart > lists the rounds (all of them)",
            "missing_test",
            "engine::e2e missing_test",
            "a round picked beside a chart > shows nothing at all",
            "a round picked beside a chart > shows the list",
        ] {
            let held = in_files(&dir, &both, named).unwrap().is_some_and(|f| !f.is_empty());
            assert_eq!(held, on_branch(&dir, named).is_ok(), "{named}");
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

    // The reading of a name against a run's log is the one `ci_before` results always had, whatever the binary, the
    // test and the way of writing: the formula of the code before this change is written out here as it was.
    #[test]
    fn a_name_in_a_log_is_read_as_before_over_each_binary_test_and_way_of_writing() {
        let before = |named: &str, binary: &str, test: &str| {
            test == named
                || format!("{binary} {test}") == named
                || test.ends_with(&format!("::{named}"))
                || named.ends_with(&format!("::{test}"))
        };
        let binaries = ["engine::e2e", "e2e", "engine", "wf-agents::bin/wf-agents"];
        let tests = ["a_test", "tools::tests::a_test", "a_test_not", "b"];
        let mut names: Vec<String> = [
            "",
            " ",
            "a_tes",
            "tests::a_test",
            "::a_test",
            "a_test::",
            "b",
            "a::b",
            "x::b",
        ]
        .map(String::from)
        .to_vec();
        for binary in binaries {
            for test in tests {
                for form in [
                    test.to_string(),
                    format!("{binary} {test}"),
                    format!("{binary}::{test}"),
                    format!("x::{test}"),
                    format!("{test}::"),
                    format!("{binary} "),
                    format!(" {test}"),
                    format!("{binary}  {test}"),
                ] {
                    names.push(form);
                }
            }
        }
        let (mut held, mut total) = (0, 0);
        for binary in binaries {
            for test in tests {
                for named in &names {
                    let want = before(named, binary, test);
                    assert_eq!(matches_run(named, binary, test), want, "{named:?} in {binary} {test}");
                    held += usize::from(want);
                    total += 1;
                }
            }
        }
        assert!(0 < held && held < total, "{held} of {total}");
    }

    /// A repository with the given files, committed once.
    fn repo_with_files(tag: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("wf-agents-test-name-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (path, text) in files {
            let at = dir.join(path);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(at, text).unwrap();
        }
        git_in(&dir, &["init", "-q"]);
        git_in(&dir, &["add", "-A"]);
        git_in(&dir, &["commit", "-qm", "tests"]);
        dir
    }

    // Titles with marks a pattern or a source string treats specially (brackets, a star, a dollar, a backslash, a
    // quote of each kind, a dash, a path inside the nextest binary) are found as the source holds them; a name that
    // only extends or cuts a function's name is not the function.
    #[test]
    fn titles_with_marks_and_functions_with_longer_names_are_told_apart() {
        let dir = repo_with_files(
            "marks",
            &[
                (
                    "client/src/m.test.ts",
                    concat!(
                        "describe('marks', () => {\n",
                        "  it('matches [a-z]* and $HOME \\\\ path', () => {});\n",
                        "  it(\"says \\\"hi\\\" and it's ok\", () => {});\n",
                        "  it('says \"hi\" and it\\'s fine', () => {});\n",
                        "  it('shows \u{2014} the rows', () => {});\n",
                        "  it('handles v1.2: the end!', () => {});\n",
                        "});\n",
                    ),
                ),
                ("crates/a/src/lib.rs", "#[tokio::test] async fn waits_for_a_slot() {}\n"),
            ],
        );
        for named in [
            "marks > matches [a-z]* and $HOME \\ path",
            "marks > says \"hi\" and it's ok",
            "m.test.ts > marks > says \"hi\" and it's fine",
            "marks > shows \u{2014} the rows",
            "client/src/m.test.ts > marks > handles v1.2: the end!",
            "  engine::bin/wf-agents waits_for_a_slot  ",
            " waits_for_a_slot ",
        ] {
            assert!(on_branch(&dir, named).is_ok(), "{named}");
        }
        for named in [
            "engine::e2e waits_for_a",
            "engine::e2e waits_for_a_slot_extra",
            "engine::e2e::waits_for_a_slot_extra",
            "waits_for_a",
            "marks > matches [a-z]+ and $HOME \\ path",
            "marks > says 'hi' and it's ok",
        ] {
            assert!(on_branch(&dir, named).is_err(), "{named}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    // A name that ends in `:`, `/` or `.` is not looked for at all, as the reading before this change did not look
    // for it; so it is let through whether or not the branch holds it. That is too lax, and item #11's to tighten.
    #[test]
    fn a_name_that_ends_in_a_separator_is_not_looked_for() {
        let dir = repo_with_tests("separator");
        for named in [
            "client/src/m.test.ts > marks > handles v1.2: the end.",
            "a round picked beside a chart > shows nothing at all.",
            "engine::e2e missing_test:",
            "engine::e2e::missing_test/",
        ] {
            assert!(on_branch(&dir, named).is_ok(), "{named}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    // The title of a vitest name is looked for as a string in a source file, with its quotes. The same words in a
    // document or a comment are not a test: the name is refused, and the quoted title is found.
    #[test]
    fn a_title_in_prose_is_not_a_test() {
        let dir = repo_with_files(
            "prose",
            &[
                ("README.md", "This works, and so shows nothing at all about tests.\n"),
                (
                    "client/src/p.test.ts",
                    "// shows the list of rounds\ndescribe('a page', () => {\n  it('lists rounds', () => {});\n});\n",
                ),
            ],
        );
        for named in [
            "suite > works",
            "a page > shows nothing at all",
            "client/src/p.test.ts > a page > shows the list of rounds",
        ] {
            assert!(on_branch(&dir, named).is_err(), "{named}");
        }
        assert!(on_branch(&dir, "client/src/p.test.ts > a page > lists rounds").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // A quoted title is found whatever stands next to its quotes in the source: the quote is a mark, so the word
    // boundary of `git grep -w` is the mark's neighbour, which a call, a table, a list, a line ending may all be.
    #[test]
    fn a_quoted_title_is_found_beside_each_mark_a_source_puts_next_to_it() {
        let dir = repo_with_files(
            "beside",
            &[(
                "client/src/b.test.ts",
                concat!(
                    "describe.skip('b0', () => {\r\n",
                    "  it.skip('t1', () => {});\r\n",
                    "  test.concurrent(\n    't2',\n    () => {});\n",
                    "  const rows = [['t3', 1], { name:'t4' }, { name: \"t5\"}];\n",
                    "  const s = new Set(['t6']); x = 't7'; y = `t8`;\n",
                    "  it.each([1])('t9 %i', () => {});\n",
                    "});\n",
                ),
            )],
        );
        for title in ["b0", "t1", "t2", "t3", "t4", "t5", "t6", "t7", "t8"] {
            let named = format!("client/src/b.test.ts > b0 > {title}");
            assert!(on_branch(&dir, &named).is_ok(), "{named}");
        }
        // A title the table prints with its value is not in the source as printed.
        assert!(on_branch(&dir, "b0 > t9 1").is_err());
        // The word without its quotes is no title, nor is a quote of another kind around it.
        assert!(on_branch(&dir, "b0 > t").is_err());
        assert!(on_branch(&dir, "b0 > 't1").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // A name the branch lacks stays refused when it holds a blank line: a blank line is no word to look for, and
    // a blank pattern would match every line of every file.
    #[test]
    fn a_name_with_a_blank_line_in_it_is_still_not_found() {
        let dir = repo_with_tests("blank");
        for named in [
            "zzabsent:\n\nqqnone",
            "zzabsent > qqnone\n\nghost",
            "zzabsent\n\nqqnone",
            "engine::e2e zzabsent:\n\nqqnone",
            "zzabsent\r\nqqnone",
        ] {
            assert!(on_branch(&dir, named).is_err(), "{named:?}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Names built from two kinds of word and each separator a name uses: whatever the reading before this change
    // let through (written out as git is asked) is let through; a name made of absent words alone, with a word left
    // after its last `:`, `/` or `.`, is refused.
    #[test]
    fn names_built_from_present_and_absent_words_are_read_no_stricter_and_no_looser() {
        let dir = repo_with_tests("built");
        let before = |named: &str| {
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
        let present = ["waits_for_a_slot", "page"];
        let absent = ["zzabsent", "qqnone"];
        let joins = [".", "::", " ", " > ", "/"];
        let words: Vec<&str> = present.iter().chain(&absent).copied().collect();
        let mut names = Vec::new();
        for a in &words {
            for join in joins {
                names.push(format!("{a}{join}"));
                names.push(format!("{join}{a}"));
                for b in &words {
                    names.push(format!("{a}{join}{b}"));
                }
            }
        }
        let (mut let_through, mut refused) = (0, 0);
        for named in &names {
            let now = on_branch(&dir, named);
            if before(named) {
                assert!(now.is_ok(), "{named:?} was let through before");
                let_through += 1;
            }
            if !present.iter().any(|w| named.contains(w)) && !last_segment(named).is_empty() {
                assert!(now.is_err(), "{named:?} has no word of the branch");
                refused += 1;
            }
        }
        assert!(
            let_through > 20 && refused > 20,
            "{let_through} let through, {refused} refused"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
