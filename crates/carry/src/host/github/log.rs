//! What of a red job's log tells why it is red, as GitHub Actions keeps the log: the steps that failed, without the
//! runner's set-up and clean-up, a step's own set-up, the lines that only show progress and what came after the last
//! error; a long one cut to the failures, the summary and each error with the lines under it; and all of it within a
//! number of bytes, the first and the end kept. Brought over from the old engine's `what_failed`, which learned that
//! the last lines of a log are the runner's clean-up and the tests that passed, and miss the start of a failure.

/// The most lines of a red job's log given whole once what only sets up, cleans up or shows progress is left out; a
/// longer one is given as its failures, its summary and its errors.
const WHOLE: usize = 80;

/// The most lines given of a red job's log, however many failures and errors it has: the first of them and its end.
const MOST: usize = 120;

/// The lines given at the end of a red job's log cut to [`MOST`]: its summary and the error that ended its step.
const END: usize = 30;

/// The most lines an error is given with, its own included, up to the blank line that ends it.
const ERROR_LINES: usize = 30;

/// The words a line of cargo or nextest begins with, right-aligned to twelve columns, that only show progress: a crate
/// fetched or built, a test started, passed, skipped or slow.
const PROGRESS: [&str; 21] = [
    "Adding",
    "Archiving",
    "Blocking",
    "Checking",
    "Compiling",
    "Documenting",
    "Downloaded",
    "Downloading",
    "Finished",
    "Fresh",
    "Installed",
    "Installing",
    "Locking",
    "Packaging",
    "PASS",
    "Removed",
    "SKIP",
    "START",
    "Unpacking",
    "Updating",
    "Verifying",
];

/// What of a red job's log tells why it is red, in at most `budget` bytes: the steps that failed, those with an
/// `##[error]` line, each from its command, up to the last error, the tests still running in them kept, without the step's set-up (the shell and the
/// environment it ran with), the lines that only show progress and repeated blank lines; with no step that failed,
/// every step but the runner's set-up and clean-up. Up to [`WHOLE`] lines it is given whole; a longer one is given as
/// the failures (each test that did not pass with its output), the summary and each error with the lines under it,
/// what is left out between them said as how many lines; and at most [`MOST`] lines and `budget` bytes, the first and
/// the end.
pub(crate) fn what_failed(log: &str, budget: usize) -> String {
    let mut lines: Vec<String> = log.lines().map(clean_line).collect();
    // What comes after the last error is the clean-up of the step and of the runner.
    if let Some(last) = lines.iter().rposition(|l| l.starts_with("##[error]")) {
        lines.truncate(last + 1);
    }
    let kept = failed_steps(&lines);
    let kept = if kept.len() > WHOLE { picked(&kept) } else { kept };
    within(capped(kept, MOST), budget).join("\n")
}

/// The text in a fenced block, so that it reads as what the CI printed and not as words to its reader: its fences
/// longer than any run of backquotes it holds.
pub(crate) fn fenced(text: &str) -> String {
    let longest = (text.split(|c| c != '`')).map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest.max(2) + 1);
    format!("{fence}\n{text}\n{fence}")
}

/// The word a line of cargo or nextest begins with, right-aligned to twelve columns and followed by a space, such as
/// `   Compiling`, `        PASS` or `     Summary`; none for any other line.
fn status(line: &str) -> Option<&str> {
    let head = line.get(..12)?;
    let word = head.trim_start();
    let right_aligned = !word.is_empty() && !word.contains(' ') && line[12..].starts_with(' ');
    right_aligned.then_some(word)
}

/// Whether a line only shows progress: cargo fetching or building a crate, nextest starting, passing, skipping or
/// waiting on a test, or libtest's line of a test that passed.
fn progress(line: &str) -> bool {
    status(line).is_some_and(|w| PROGRESS.contains(&w))
        || (line.starts_with("test ") && (line.ends_with(" ... ok") || line.ends_with(" ... ignored")))
}

/// Whether a line is nextest's of a test that did not pass, as `        FAIL [   0.466s] (139/146) bin name`: failed,
/// timed out, leaked, aborted or killed by a signal.
fn failure(line: &str) -> bool {
    status(line).is_some_and(|w| !PROGRESS.contains(&w) && w.chars().all(|c| c.is_ascii_uppercase() || c == '-'))
        && line[12..].trim_start().starts_with('[')
}

/// Whether a line says that a test was still running at that time: libtest's `has been running for over`, and
/// nextest's `SLOW` line; of a job cancelled at its time limit, they say which test held it.
fn still_running(line: &str) -> bool {
    line.contains(" has been running for over ") || status(line) == Some("SLOW")
}

/// Whether a line begins an error: rustc's, cargo's, clippy's and nextest's errors and warnings, a diff of rustfmt,
/// libtest's failures and result, and the runner's `##[error]`.
fn error_begins(line: &str) -> bool {
    let rustc = ["error", "warning"].iter().any(|w| {
        line.strip_prefix(w)
            .is_some_and(|rest| rest.starts_with(':') || rest.starts_with('['))
    });
    rustc
        || line.starts_with("Diff in ")
        || line.starts_with("##[error]")
        || line.starts_with("failures:")
        || line.starts_with("test result: FAILED")
        || (line.starts_with("---- ") && line.ends_with(" ----"))
}

/// The lines of the steps of a job's log that failed, as [`what_failed`] takes them.
fn failed_steps(lines: &[String]) -> Vec<String> {
    // A step begins with the group that shows its command, `##[group]Run …`; the runner's clean-up with `Post job
    // cleanup.`; what comes before the first step is the runner's set-up.
    let mut steps: Vec<Vec<&str>> = vec![Vec::new()];
    for l in lines {
        if l.starts_with("##[group]Run ") || l.starts_with("Post job cleanup") {
            steps.push(Vec::new());
        }
        if let Some(s) = steps.last_mut() {
            s.push(l.as_str());
        }
    }
    let cleanup = |s: &&Vec<&str>| s.first().is_some_and(|l| l.starts_with("Post job cleanup"));
    let ran: Vec<&Vec<&str>> = steps.iter().skip(1).filter(|s| !cleanup(s)).collect();
    let failed: Vec<&Vec<&str>> = (ran.iter().copied())
        .filter(|s| s.iter().any(|l| l.starts_with("##[error]")))
        .collect();
    let chosen = match (failed.is_empty(), ran.is_empty()) {
        (false, _) => failed,
        (true, false) => ran,
        // A log with no step in it is all one.
        (true, true) => steps.iter().collect(),
    };
    let mut out: Vec<String> = Vec::new();
    for s in chosen {
        let mut header = s.first().is_some_and(|l| l.starts_with("##[group]Run "));
        for (i, l) in s.iter().enumerate() {
            if header {
                if i == 0 {
                    out.push(l.trim_start_matches("##[group]").to_string());
                } else if *l == "##[endgroup]" {
                    header = false;
                }
                continue;
            }
            if progress(l) {
                continue;
            }
            if l.trim().is_empty() && out.last().is_none_or(|x| x.trim().is_empty()) {
                continue;
            }
            out.push(l.to_string());
        }
    }
    out
}

/// The lines of a long red log that tell why it is red: each step's command, each failure with the lines under it up
/// to the next test's line, the summary to the end, and each error with the lines under it up to a blank line; the
/// rest said as how many lines were left out, but for blank lines between what is given.
fn picked(lines: &[String]) -> Vec<String> {
    let n = lines.len();
    let mut keep = vec![false; n];
    // Nextest's summary: from the rule before it to the end.
    let summary = (lines.iter())
        .rposition(|l| l.trim_start().starts_with("Summary ["))
        .map(|i| {
            if i > 0 && lines[i - 1].starts_with('\u{2500}') {
                i - 1
            } else {
                i
            }
        });
    let mut i = 0;
    while i < n {
        if summary == Some(i) {
            keep[i..].iter_mut().for_each(|k| *k = true);
            break;
        }
        let l = &lines[i];
        if l.starts_with("Run ") || still_running(l) {
            keep[i] = true;
        }
        if failure(l) {
            keep[i] = true;
            i += 1;
            while i < n && status(&lines[i]).is_none() && !lines[i].starts_with('\u{2500}') && summary != Some(i) {
                keep[i] = true;
                i += 1;
            }
            continue;
        }
        if error_begins(l) {
            let from = i;
            while i < n && i - from < ERROR_LINES && (i == from || !lines[i].trim().is_empty()) && summary != Some(i) {
                keep[i] = true;
                i += 1;
            }
            continue;
        }
        i += 1;
    }
    let mut out = Vec::new();
    let mut left: Vec<&str> = Vec::new();
    for (k, l) in keep.iter().zip(lines) {
        if *k {
            if left.iter().any(|x| !x.trim().is_empty()) {
                out.push(left_out(left.len()));
            } else {
                out.extend(left.iter().map(|x| x.to_string()));
            }
            left.clear();
            out.push(l.clone());
        } else {
            left.push(l);
        }
    }
    if left.iter().any(|x| !x.trim().is_empty()) {
        out.push(left_out(left.len()));
    }
    out
}

/// What stands where lines are left out.
fn left_out(n: usize) -> String {
    format!("[{n} lines left out]")
}

/// At most `most` lines: the first and the [`END`] last, with how many were left out between them.
fn capped(lines: Vec<String>, most: usize) -> Vec<String> {
    if lines.len() <= most {
        return lines;
    }
    let head = most - END - 1;
    let left = lines.len() - head - END;
    let mut out: Vec<String> = lines[..head].to_vec();
    out.push(left_out(left));
    out.extend_from_slice(&lines[lines.len() - END..]);
    out
}

/// The lines in at most `budget` bytes, each line's end counted: as many of the first as fit in two thirds of it and
/// of the last in the rest, with how many were left out between them; a line too long to fit is cut at a character.
fn within(lines: Vec<String>, budget: usize) -> Vec<String> {
    let size = |ls: &[String]| ls.iter().map(|l| l.len() + 1).sum::<usize>();
    if size(&lines) <= budget {
        return lines;
    }
    let marker = left_out(lines.len()).len() + 1;
    let room = budget.saturating_sub(marker);
    let (mut head, mut used) = (0, 0);
    while head < lines.len() && used + lines[head].len() < room * 2 / 3 {
        used += lines[head].len() + 1;
        head += 1;
    }
    let (mut tail, mut end_used) = (0, 0);
    while tail < lines.len() - head && used + end_used + lines[lines.len() - 1 - tail].len() < room {
        end_used += lines[lines.len() - 1 - tail].len() + 1;
        tail += 1;
    }
    let mut out: Vec<String> = lines[..head].to_vec();
    if head == 0 && tail == 0 {
        // One line longer than the budget: its beginning.
        let mut cut = room.min(lines[0].len());
        while !lines[0].is_char_boundary(cut) {
            cut -= 1;
        }
        out.push(lines[0][..cut].to_string());
        head = 1;
    }
    let left = lines.len() - head - tail;
    if left > 0 {
        out.push(left_out(left));
    }
    out.extend_from_slice(&lines[lines.len() - tail..]);
    out
}

/// A line of a job's log as GitHub keeps it, as a reader wants it: without the time GitHub writes before it, and
/// without its control characters but tabs, so that neither the escapes that colour it nor what moves a terminal's
/// cursor reaches the reader; what its text says is kept as it is.
fn clean_line(line: &str) -> String {
    let text = line.trim_start_matches('\u{feff}');
    let text = match text.split_once(' ') {
        Some((stamp, rest)) if is_timestamp(stamp) => rest,
        _ if is_timestamp(text) => "",
        _ => text,
    };
    without_controls(text)
}

/// Whether a word is a time as GitHub writes one in logs: 2026-10-27T09:47:49.0473549Z.
fn is_timestamp(w: &str) -> bool {
    let b = w.as_bytes();
    b.len() >= 20
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && w.ends_with('Z')
}

/// A text without its control characters, those of C0 and C1 and delete, but for tabs: an escape sequence that
/// begins with ESC [ or with the C1 introducer is taken out whole, up to the letter that ends it.
fn without_controls(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let sequence = match c {
            '\t' => {
                out.push(c);
                continue;
            }
            '\u{1b}' if chars.peek() == Some(&'[') => {
                chars.next();
                true
            }
            '\u{9b}' => true,
            c if c.is_control() => false,
            c => {
                out.push(c);
                continue;
            }
        };
        if sequence {
            for x in chars.by_ref() {
                if ('@'..='~').contains(&x) {
                    break;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Room enough for any of the logs below whole.
    const ROOM: usize = 1 << 20;

    #[test]
    fn a_line_loses_the_time_its_colours_and_every_other_control_character() {
        let line = "\u{feff}2026-10-27T09:47:49.0473549Z \u{1b}[1m\u{1b}[91merror\u{1b}[0m: test failed";
        assert_eq!(clean_line(line), "error: test failed");
        assert_eq!(clean_line("2026-10-27T09:47:49.0475850Z"), "");
        // A bell, a carriage return, a backspace, a delete and the C1 introducer of a sequence with the sequence are
        // taken out; a tab stays.
        let controls = "2026-10-27T09:47:49.0475850Z a\u{7}b\rc\u{8}d\u{9b}31me\u{7f}f\tg";
        assert_eq!(clean_line(controls), "abcdef\tg");
        // What the text of a log says is kept as it is: a tab between its words, and the two characters `^[`.
        assert_eq!(clean_line("2026-10-27T09:47:49.0475850Z a\tb\tc"), "a\tb\tc");
        assert_eq!(
            clean_line("2026-10-27T09:47:49.0475850Z press ^[[A to go up"),
            "press ^[[A to go up"
        );
        assert_eq!(clean_line("plain line"), "plain line");
    }

    // A red test partition, its log cut from a real one: what is given is the test step's command, the test that
    // failed with its output, the summary and the error that ended the step; not the runner's set-up and clean-up, the
    // steps that passed, the step's environment, cargo's progress or the tests that passed.
    #[test]
    fn a_red_test_is_given_with_its_output_and_the_summary() {
        let given = what_failed(include_str!("../../../tests/fixtures/job-logs/nextest-fail.log"), ROOM);
        for kept in [
            "Run cargo nextest run --workspace --locked --cargo-profile ci --profile ci --partition count:1/3",
            "        FAIL [   0.466s] (139/146) wf-agents::cli a_table_over_the_row_limit_says_how_many_rows_it_leaves_out",
            "    test a_table_over_the_row_limit_says_how_many_rows_it_leaves_out ... FAILED",
            "panicked at crates/wf-agents/tests/cli.rs:466:13:",
            "     Summary [ 105.999s] 142/146 tests run: 141 passed, 1 failed, 279 skipped",
            "error: test run failed",
            "##[error]Process completed with exit code 100.",
        ] {
            assert!(given.contains(kept), "{kept}\n{given}");
        }
        for gone in [
            "Current runner version",
            "actions/checkout",
            "rustup toolchain install",
            "CARGO_TERM_COLOR",
            "Downloaded",
            "Finished",
            "PASS [",
            "Post job cleanup",
            "git version",
        ] {
            assert!(!given.contains(gone), "{gone}\n{given}");
        }
    }

    // A partition that did not compile, its log cut from a real one with nine errors: too long to give whole, it is
    // given as each error with the lines under it, up to the most lines given, the first errors and the end.
    #[test]
    fn errors_are_given_with_the_lines_under_them_up_to_the_most_lines() {
        let given = what_failed(
            include_str!("../../../tests/fixtures/job-logs/compile-errors.log"),
            ROOM,
        );
        let lines: Vec<&str> = given.lines().collect();
        assert!(lines.len() <= MOST, "{}", lines.len());
        assert_eq!(
            lines[..3],
            [
                "Run cargo nextest run --workspace --locked --cargo-profile ci --profile ci --partition count:2/3",
                "error[E0432]: unresolved import `engine::ending::EndKind`",
                "    --> crates/engine/tests/e2e.rs:5680:31",
            ]
        );
        assert!(!given.contains("Compiling"), "{given}");
        assert!(given.contains(" lines left out]"), "{given}");
        assert_eq!(
            lines[lines.len() - 4..],
            [
                "error: could not compile `engine` (test \"e2e\") due to 9 previous errors",
                "warning: build failed, waiting for other jobs to finish...",
                "error: command `/home/runner/.rustup/toolchains/1.99.1-x86_64-unknown-linux-gnu/bin/cargo '--color=always' test --no-run --message-format json-render-diagnostics --workspace --profile ci --locked` exited with code 101",
                "##[error]Process completed with exit code 101.",
            ]
        );
    }

    // What fits in the bytes given is its first lines and its end, the error that ended the step among them.
    #[test]
    fn a_log_is_given_within_the_bytes_given_its_first_lines_and_its_end() {
        let log = include_str!("../../../tests/fixtures/job-logs/compile-errors.log");
        let given = what_failed(log, 2000);
        assert!(given.len() <= 2000, "{}", given.len());
        assert!(
            given.starts_with("Run cargo nextest run --workspace --locked"),
            "{given}"
        );
        assert!(
            given.ends_with("##[error]Process completed with exit code 101."),
            "{given}"
        );
        assert!(given.contains(" lines left out]\n"), "{given}");
        let long = format!("2026-10-27T09:47:49.0473549Z {}\n", "é".repeat(100));
        assert_eq!(what_failed(&long, 51), "é".repeat(16));
    }

    // A job a hung test held until its time limit, its log as GitHub kept it: the tests that passed are left out, and
    // what failed, what was still running when the job was cancelled and the cancel are given.
    #[test]
    fn a_cancelled_job_is_given_with_what_was_still_running() {
        let given = what_failed(include_str!("../../../tests/fixtures/job-logs/cancelled.log"), 4000);
        for kept in [
            "Run cargo test --workspace --locked --no-fail-fast",
            "test tests::a_copy_taken_in_the_future_does_not_displace_the_copies_taken_after_it ... FAILED",
            "test tests::a_tick_during_a_running_step_starts_no_second_one has been running for over 60 seconds",
            "##[error]The operation was canceled.",
        ] {
            assert!(given.contains(kept), "{kept}\n{given}");
        }
        assert!(!given.contains(" ... ok"), "{given}");
        // Past the lines given whole, the tests still running are kept with the failures and the errors.
        let mut long =
            String::from("2026-11-11T07:50:23.9Z ##[group]Run cargo test\n2026-11-11T07:50:23.9Z ##[endgroup]\n");
        for n in 0..100 {
            long.push_str(&format!("2026-11-11T07:50:24.0Z note {n}\n"));
        }
        long.push_str("2026-11-11T07:51:47.6Z test tests::slow has been running for over 60 seconds\n");
        long.push_str("2026-11-11T07:51:48.6Z         SLOW [> 30.000s] carry tests::slower\n");
        long.push_str("2026-11-11T08:04:08.8Z ##[error]The operation was canceled.\n");
        let given = what_failed(&long, 4000);
        assert!(
            given.contains("test tests::slow has been running for over 60 seconds"),
            "{given}"
        );
        assert!(given.contains("SLOW [> 30.000s] carry tests::slower"), "{given}");
        assert!(!given.contains("note 50"), "{given}");
    }

    // The job that only says which jobs failed is short, and given whole but for its step's set-up.
    #[test]
    fn a_short_red_log_is_given_whole_but_for_its_set_up() {
        assert_eq!(
            what_failed(include_str!("../../../tests/fixtures/job-logs/check.log"), ROOM),
            "Run echo \"lint and test: $RESULTS\"\nlint and test: success failure\n##[error]Process completed with \
             exit code 1."
        );
    }

    // A log with no step in it is given whole up to its last error: what follows is the clean-up.
    #[test]
    fn a_log_with_no_step_ends_at_its_last_error() {
        let log = "2026-11-10T04:23:10.1Z Compiling\n2026-11-10T04:23:10.2Z \u{1b}[31mfailed\u{1b}[0m\n\
                   2026-11-10T04:23:10.3Z ##[error]Process completed with exit code 1.\n2026-11-10T04:23:10.4Z Post \
                   job cleanup.\n2026-11-10T04:23:10.5Z Cleaning up";
        assert_eq!(
            what_failed(log, ROOM),
            "Compiling\nfailed\n##[error]Process completed with exit code 1."
        );
        assert_eq!(what_failed("no error at all\n", ROOM), "no error at all");
    }

    #[test]
    fn a_fence_is_longer_than_any_run_of_backquotes_in_the_text() {
        assert_eq!(fenced("a `b`"), "```\na `b`\n```");
        assert_eq!(fenced("```rust\nx\n```"), "````\n```rust\nx\n```\n````");
    }

    // Cargo's and nextest's lines that only show progress are known by their word, right-aligned to twelve columns;
    // a test that did not pass, and an error, are not progress.
    #[test]
    fn progress_is_known_by_its_word() {
        assert!(progress("   Compiling engine v0.1.0 (/w/crates/engine)"));
        assert!(progress("        PASS [   0.006s] (  1/146) api control::tests::a"));
        assert!(progress("test tools::tests::a ... ok"));
        assert!(!progress("        FAIL [   0.466s] (139/146) wf-agents::cli a"));
        assert!(failure("        FAIL [   0.466s] (139/146) wf-agents::cli a"));
        assert!(failure("     SIGSEGV [   0.466s] (139/146) wf-agents::cli a"));
        assert!(!failure("     Summary [ 105.999s] 142/146 tests run"));
        assert!(!progress("error[E0432]: unresolved import"));
        assert!(error_begins("error[E0432]: unresolved import"));
        assert!(!error_begins("errors: none"));
    }
}
