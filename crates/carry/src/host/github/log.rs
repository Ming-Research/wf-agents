//! What a job's log on GitHub says of the step that failed the job, as a reader wants it. GitHub keeps a job's log as
//! plain text: each line after the time it was written, each step begun by a line `##[group]Run …` that names its
//! command or the action it uses, and a step that failed ended by a line `##[error]…`, such as `##[error]Process
//! completed with exit code 101.`; after the steps come the clean-up of the actions and of the runner. Only that form,
//! which is GitHub's own, is read here: what the project's tools print is given as they printed it.

use crate::process;

/// How GitHub begins the line that begins a step.
const STEP: &str = "##[group]Run ";

/// How GitHub begins the line of an error, such as the one that ends a step that failed.
const ERROR: &str = "##[error]";

/// The end of what the step that failed the job printed, as its log holds it: the lines from the one that begins the
/// step to the last error, which ended it, each without the time before it and without its control characters but
/// tabs, so that neither the escapes that colour it nor what moves a terminal's cursor reaches the reader; and of them
/// the end, as much as [`process::tail`] keeps of what a check said. With no error, the lines run to the log's end;
/// with no step begun before them, from its start.
pub(super) fn failed_step(log: &str) -> String {
    let lines: Vec<String> = log.lines().map(line).collect();
    let end = (lines.iter())
        .rposition(|l| l.starts_with(ERROR))
        .map_or(lines.len(), |at| at + 1);
    let begin = (lines[..end].iter()).rposition(|l| l.starts_with(STEP)).unwrap_or(0);
    process::tail(lines[begin..end].join("\n").as_bytes())
}

/// A line of a log as a reader wants it: without the mark of UTF-8 that may begin the log, without the time GitHub
/// writes before the line, and without its control characters but tabs.
fn line(line: &str) -> String {
    let line = line.strip_prefix('\u{feff}').unwrap_or(line);
    let text = match line.split_once(' ') {
        Some((stamp, rest)) if is_stamp(stamp) => rest,
        None if is_stamp(line) => "",
        _ => line,
    };
    super::output::without_controls(text)
}

/// Whether a word is a time as GitHub writes one before each line of a log, `2026-11-21T09:47:49.0473549Z`: in UTC,
/// with a fraction of a second of any length or none.
fn is_stamp(word: &str) -> bool {
    let Some(rest) = word.strip_suffix('Z') else {
        return false;
    };
    let (whole, fraction) = match rest.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (rest, None),
    };
    let form = "dddd-dd-ddTdd:dd:dd";
    let shaped = whole.len() == form.len()
        && (whole.chars().zip(form.chars())).all(|(c, f)| if f == 'd' { c.is_ascii_digit() } else { c == f });
    shaped && fraction.is_none_or(|f| !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // With no error in it, a log's last step runs to its end; with no step begun, what failed is the log from its
    // start, the runner's own error ending it. A step longer than the end given is given its end, the error that ended
    // it last. Microseconds; the test of the host holds a whole log of a job, and this one the edges of it.
    #[test]
    fn a_log_with_no_error_gives_its_last_step_one_with_no_step_its_start_and_a_long_step_its_end() {
        let log = "2026-11-21T09:00:00.1Z ##[group]Run first\n2026-11-21T09:00:01Z one\n\
                   2026-11-21T09:00:02.0000000Z ##[group]Run second\n2026-11-21T09:00:03.0000000Z\n\
                   2026-11-21T09:00:03.0000000Z two\n";
        assert_eq!(failed_step(log), "##[group]Run second\n\ntwo");
        let log = "\u{feff}2026-11-21T09:00:00.0000000Z Current runner version: '2.329.0'\n\
                   2026-11-21T09:00:01.0000000Z ##[error]The runner could not download the action.\n\
                   2026-11-21T09:00:02.0000000Z Cleaning up orphan processes\n";
        assert_eq!(
            failed_step(log),
            "Current runner version: '2.329.0'\n##[error]The runner could not download the action."
        );
        let lines: String = (0..1000)
            .map(|n| format!("2026-11-21T09:00:00.0000000Z line {n:04}\n"))
            .collect();
        let log = format!("{lines}2026-11-21T09:00:01.0000000Z ##[error]Process completed with exit code 1.\n");
        let given = failed_step(&log);
        assert!(given.len() <= process::TAIL, "{}", given.len());
        assert!(
            given.ends_with("line 0999\n##[error]Process completed with exit code 1."),
            "{given}"
        );
    }

    // Only GitHub's time is taken from the start of a line: a word the job printed that only looks like one is kept.
    #[test]
    fn only_githubs_time_is_taken_from_a_line() {
        for (given, kept) in [
            ("2026-11-21T09:00:00Z x", "x"),
            ("2026-11-21 x", "2026-11-21 x"),
            ("2026-11-21T09:00:00.Z x", "2026-11-21T09:00:00.Z x"),
            ("2026-11-21T09:00:00+01:00 x", "2026-11-21T09:00:00+01:00 x"),
        ] {
            assert_eq!(line(given), kept);
        }
    }
}
