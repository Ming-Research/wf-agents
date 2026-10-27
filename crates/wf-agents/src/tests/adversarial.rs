//! Tests that look for what the README check lets through: wrong texts it passes, and right ones it reports. Each case
//! adds a few lines to the text that writes every command correctly. The cases that stand in the convention the check
//! reads are in one table, with the command lines they write, so that clap, which parses those command lines, can be
//! asked which of them are wrong. The forms outside the convention are in two more tables: the ones the check has to
//! report, and the ones it has to leave alone. A case stands for one branch of the reader, and its name says which;
//! two cases for the same branch are one. The texts that the acceptance conditions name are in `readme.rs`. The wrong
//! commands that the check passes in silence, and the right texts that it reports, each have a test of their own below
//! the tables, so that a row of them can be settled alone.

use clap::error::ErrorKind;

use super::readme::{self, leaves, problems, tree};

/// A way a README can write command lines in the convention, and what the check has to make of it.
struct Case {
    /// What it tries: the branch of the reader it stands for.
    name: &'static str,
    /// The Markdown that writes it, added to the text.
    text: &'static str,
    /// The command lines in it, as a shell runs them.
    runs: &'static [&'static str],
    /// The subcommand or option, without dashes, that the check has to name as not there; none when all are right.
    wrong: &'static str,
}

/// A way a README can write something outside the convention, added to the text.
struct Form {
    /// What it tries.
    name: &'static str,
    /// The Markdown that writes it.
    text: &'static str,
}

/// A wrong command line written in a form the check has to read or report, so that it does not pass in silence.
struct Hidden {
    /// What it tries.
    name: &'static str,
    /// The Markdown that writes it.
    text: &'static str,
    /// The command line it means, as a shell runs it, which clap turns down.
    runs: &'static str,
}

const CASES: &[Case] = &[
    Case {
        name: "a global option before the subcommand does not end the reading",
        text: "`wf-agents --data DIR queue add`",
        runs: &["wf-agents --data DIR queue add"],
        wrong: "add",
    },
    Case {
        name: "the equals form of a global option does not swallow the subcommand after it",
        text: "`wf-agents --data=DIR queue put 3`",
        runs: &["wf-agents --data=DIR queue put 3"],
        wrong: "",
    },
    Case {
        name: "an option of the command after a global one",
        text: "`wf-agents --data DIR run --once`",
        runs: &["wf-agents --data DIR run --once"],
        wrong: "",
    },
    Case {
        name: "options of a nested command",
        text: "`wf-agents --data DIR item add --project P --title T`",
        runs: &["wf-agents --data DIR item add --project P --title T"],
        wrong: "",
    },
    Case {
        name: "an option between the two subcommands",
        text: "`wf-agents item --data DIR list`",
        runs: &["wf-agents item --data DIR list"],
        wrong: "",
    },
    Case {
        name: "a value that is the name of a subcommand is a value",
        text: "`wf-agents --data queue put 3`",
        runs: &["wf-agents --data queue put 3"],
        wrong: "put",
    },
    Case {
        // CommonMark 0.31, fenced code blocks: tildes fence a block like backticks.
        name: "a block fenced with tildes is read",
        text: "~~~\nwf-agents queue add\n~~~",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        name: "a block fenced with backticks and an info string is read",
        text: "```sh\nwf-agents queue add\n```",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        name: "a shorter run of backticks does not close a longer fence",
        text: "````\n```\nwf-agents queue add\n````",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        name: "the end of the options",
        text: "`wf-agents say 3 -- --stop`",
        runs: &["wf-agents say 3 -- --stop"],
        wrong: "",
    },
    Case {
        name: "a short option",
        text: "`wf-agents run -h`",
        runs: &["wf-agents run -h"],
        wrong: "",
    },
    Case {
        name: "the value of an option is three dots",
        text: "`wf-agents answer 3 --text ... --why ...`",
        runs: &["wf-agents answer 3 --text ... --why ..."],
        wrong: "",
    },
    Case {
        name: "a flag after a number",
        text: "`wf-agents history 3 --full`",
        runs: &["wf-agents history 3 --full"],
        wrong: "",
    },
    Case {
        name: "options written with an equals sign",
        text: "`wf-agents rule 3 --verdict=confirm --text=ok`",
        runs: &["wf-agents rule 3 --verdict=confirm --text=ok"],
        wrong: "",
    },
    Case {
        name: "a placeholder for the command",
        text: "`wf-agents <command> --help`",
        runs: &[],
        wrong: "",
    },
    Case {
        name: "the help every command has",
        text: "`wf-agents --help`",
        runs: &["wf-agents --help"],
        wrong: "",
    },
    Case {
        name: "a shell prompt before the command does not hide it",
        text: "`$ wf-agents queue add`",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        // CommonMark 0.31, fenced code blocks: the info string of a backtick fence has no backtick in it, so a line
        // that begins with three backticks and has another after them is a code span. Read as a fence, it turns every
        // block after it inside out.
        name: "a code span of three backticks is not a fence",
        text: "```wf-agents status```\n\n```\nwf-agents run --twice\n```",
        runs: &["wf-agents status", "wf-agents run --twice"],
        wrong: "twice",
    },
    Case {
        name: "a backtick that is not closed does not hide the next line",
        text: "- not closed `wf-agents status\n- run `wf-agents queue add`",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        name: "a span on one line is read wherever it stands",
        text: "> run `wf-agents queue add` now",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        // CommonMark 0.31, fenced code blocks: a closing fence is made of the character that opened the block.
        name: "a line of tildes does not close a block fenced with backticks",
        text: "```\n~~~\nwf-agents queue add\n```",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        // CommonMark 0.31, fenced code blocks: a closing fence has nothing but spaces after it, so a fence with an
        // info string is a line of the block.
        name: "a fence with an info string does not close a block",
        text: "```\n```sh\nwf-agents queue add\n```",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        // CommonMark 0.31, fenced code blocks: a fence is three characters or more, so struck-out text opens none.
        name: "two tildes are not a fence",
        text: "~~struck out~~\nwf-agents belongs to no project",
        runs: &[],
        wrong: "",
    },
    Case {
        name: "a short option before the subcommands of a group",
        text: "`wf-agents queue -h`",
        runs: &["wf-agents queue -h"],
        wrong: "",
    },
    Case {
        name: "an option is looked up in the command it follows, not in another",
        text: "`wf-agents status --verdict confirm`",
        runs: &["wf-agents status --verdict confirm"],
        wrong: "verdict",
    },
    Case {
        name: "an option of a command written before that command",
        text: "`wf-agents --once run`",
        runs: &["wf-agents --once run"],
        wrong: "once",
    },
];

/// Forms the check does not read, in which `wf-agents` starts a command: it has to report each, whether the command in
/// it is right or wrong, because a wrong one would otherwise pass without a word.
const OUTSIDE: &[Form] = &[
    Form {
        name: "a double quote",
        text: "`wf-agents answer 3 --text \"ok\"`",
    },
    Form {
        name: "a single quote",
        text: "`wf-agents say 3 'ok'`",
    },
    Form {
        name: "a backslash at the end of a line",
        text: "```\nwf-agents item add \\\n  --project P --bogus X\n```",
    },
    Form {
        name: "a pipe",
        text: "`wf-agents status | grep work`",
    },
    Form {
        name: "a semicolon",
        text: "`wf-agents status; wf-agents queue add`",
    },
    Form {
        name: "a command substitution",
        text: "`wf-agents say 3 $(wf-agents queue add)`",
    },
    Form {
        name: "after a variable, a right command",
        text: "`WF_AGENTS_DATA=DIR wf-agents queue put 3`",
    },
    Form {
        name: "after sudo in a fenced block",
        text: "```\nsudo wf-agents run\n```",
    },
    Form {
        name: "an escaped backtick before a span",
        text: "type \\` to quote it, then run `wf-agents queue add`",
    },
    Form {
        name: "an option in prose",
        text: "Start it with wf-agents --version",
    },
    Form {
        name: "a semicolon after a command that is right",
        text: "`wf-agents status; echo done`",
    },
    Form {
        name: "a command substitution with no second command",
        text: "`wf-agents say 3 $(date)`",
    },
    Form {
        name: "an ampersand with no second command",
        text: "`wf-agents run --once &`",
    },
    Form {
        name: "a command in prose, with a full stop after it",
        text: "Start it with wf-agents run.",
    },
];

/// Places where `wf-agents` is not the start of a command: the check has to leave each alone.
const QUIET: &[Form] = &[
    Form {
        name: "a heading that is the name",
        text: "# wf-agents",
    },
    Form {
        name: "the name alone in a span",
        text: "`wf-agents`",
    },
    Form {
        name: "an indented line that goes on with a paragraph",
        text: "Prose that goes on\n    wf-agents belongs to no project",
    },
    Form {
        name: "a path to the crate in a line that is no command",
        text: "`cd crates/wf-agents && cargo test`",
    },
    Form {
        name: "the name joined by a dash to a number",
        text: "`wf-agents-1 status`",
    },
];

/// Spans that wrap right after the name, so that the name ends a line and the command is on the next one.
const WRAPPED: &[Hidden] = &[
    Hidden {
        name: "a wrong subcommand on the next line",
        text: "see `wf-agents\nqueue add` now",
        runs: "wf-agents queue add",
    },
    Hidden {
        name: "a wrong option on the next line, in a list",
        text: "- run `wf-agents\n  run --twice` now",
        runs: "wf-agents run --twice",
    },
];

/// The program run by the path of the binary, which is not the program's name alone.
const BY_PATH: &[Hidden] = &[
    Hidden {
        name: "a path in the build directory",
        text: "`./target/release/wf-agents run --twice`",
        runs: "wf-agents run --twice",
    },
    Hidden {
        name: "a path in the home directory",
        text: "`~/.local/bin/wf-agents queue add`",
        runs: "wf-agents queue add",
    },
];

/// An option written inside brackets, as the synopsis of a command writes it.
const IN_BRACKETS: &[Hidden] = &[
    Hidden {
        name: "square brackets",
        text: "`wf-agents run [--twice]`",
        runs: "wf-agents run --twice",
    },
    Hidden {
        name: "parentheses",
        text: "`wf-agents run (--twice)`",
        runs: "wf-agents run --twice",
    },
];

/// Places where `wf-agents` is the name of a package, which no command of the program starts at.
const PACKAGE: &[Form] = &[
    Form {
        name: "a package after -p, with an option after it",
        text: "`cargo test -p wf-agents --test cli`",
    },
    Form {
        name: "a package after --package, with a word after it",
        text: "`cargo test --package wf-agents readme`",
    },
];

/// What the check finds in the text that writes every command once, except `skip`, followed by `extra`.
fn found_without(skip: &str, extra: &str) -> Vec<String> {
    let root = tree();
    let mut all = Vec::new();
    leaves(&root, "wf-agents", &mut all);
    let lines: Vec<String> = all
        .iter()
        .filter(|(path, _)| path != skip)
        .map(|(path, _)| format!("`{path}`"))
        .collect();
    let text = format!("{}\n{extra}", lines.join("\n"));
    problems(&text, &root)
}

/// What the check finds in the text that writes every command once, followed by `extra`.
fn found(extra: &str) -> Vec<String> {
    found_without("", extra)
}

/// What a problem comes to, after the command line it is about: `no subcommand add` in `wf-agents queue add: no
/// subcommand add`.
fn reason(problem: &str) -> &str {
    problem.rsplit_once(": ").map_or(problem, |(_, reason)| reason)
}

/// Whether a problem says that a place was not read, and gives the convention.
fn is_unread(problem: &str) -> bool {
    problem.contains(readme::UNREAD) && problem.contains(readme::CONVENTION)
}

/// What clap, which parses the command, says to a command line that names a subcommand or an option that does not
/// exist. None when it takes the line, or stops only for what the line leaves out, or for `--help` or `--version`.
fn clap_turns_down(line: &str) -> Option<String> {
    let err = tree().try_get_matches_from(line.split_whitespace()).err()?;
    match err.kind() {
        ErrorKind::MissingRequiredArgument
        | ErrorKind::MissingSubcommand
        | ErrorKind::DisplayHelp
        | ErrorKind::DisplayVersion => None,
        _ => Some(err.to_string()),
    }
}

/// The rows among `rows` that the check passes in silence: a row that clap takes is no wrong command, and a wrong one
/// needs a problem reported for it, whatever the problem says.
fn passed_in_silence(rows: &[Hidden]) -> Vec<String> {
    let mut failures = Vec::new();
    for row in rows {
        if clap_turns_down(row.runs).is_none() {
            failures.push(format!("{}: clap takes {}", row.name, row.runs));
        } else if found(row.text).is_empty() {
            failures.push(format!("{}: wrong, and nothing is reported", row.name));
        }
    }
    failures
}

/// The forms among `forms` that the check reports though `wf-agents` starts no command in them.
fn reported_though_quiet(forms: &[Form]) -> Vec<String> {
    let mut failures = Vec::new();
    for form in forms {
        let got = found(form.text);
        if !got.is_empty() {
            failures.push(format!("{}: reported though it is no command: {got:?}", form.name));
        }
    }
    failures
}

/// The bullets of `entries`, one for each, as the README writes the data directory's entries.
fn bullets(entries: &[&str]) -> String {
    let mut lines = Vec::new();
    for entry in entries {
        lines.push(format!("- `{entry}`: what it is"));
    }
    lines.join("\n")
}

// The cases are right about the command lines they write, by clap, which parses them: it turns down a wrong case for
// the subcommand or the option the case names, and takes every line of a right one. This does not run the check.
#[test]
fn the_cases_agree_with_clap() {
    assert!(!CASES.is_empty());
    let mut failures = Vec::new();
    for case in CASES {
        let mut turned_down = Vec::new();
        for run in case.runs {
            turned_down.extend(clap_turns_down(run));
        }
        if case.wrong.is_empty() {
            if !turned_down.is_empty() {
                failures.push(format!("{}: clap turns down a right line: {turned_down:?}", case.name));
            }
        } else if !turned_down.iter().any(|message| message.contains(case.wrong)) {
            failures.push(format!(
                "{}: clap does not name {}: {turned_down:?}",
                case.name, case.wrong
            ));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

// Each case is read as it is written: the check names what is wrong with a wrong one, and reports nothing for a right
// one.
#[test]
fn the_cases_are_read_as_they_are_written() {
    let mut failures = Vec::new();
    for case in CASES {
        let got = found(case.text);
        if case.wrong.is_empty() {
            if !got.is_empty() {
                failures.push(format!("{}: reported though right: {got:?}", case.name));
            }
        } else if !got.iter().any(|problem| reason(problem).contains(case.wrong)) {
            failures.push(format!("{}: nothing names {}; found {got:?}", case.name, case.wrong));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

// A form outside the convention is reported, whether the command in it is right or wrong: the report says that the
// place was not read and gives the convention.
#[test]
fn a_form_outside_the_convention_is_reported() {
    let mut failures = Vec::new();
    for form in OUTSIDE {
        let got = found(form.text);
        if !got.iter().any(|problem| is_unread(problem)) {
            failures.push(format!("{}: not reported as unread; found {got:?}", form.name));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

// A place where `wf-agents` is a heading or a word of prose, or stands alone, is not a command and is not reported.
#[test]
fn a_place_where_the_name_is_not_a_command_is_not_reported() {
    let failures = reported_though_quiet(QUIET);
    assert!(failures.is_empty(), "{failures:#?}");
}

// At every depth: a command left out is reported, a name or an option that does not exist is reported, and every
// option a command has is accepted.
#[test]
fn every_command_is_checked_at_its_own_depth() {
    let root = tree();
    let mut all = Vec::new();
    leaves(&root, "wf-agents", &mut all);
    let paths: Vec<&str> = all.iter().map(|(path, _)| path.as_str()).collect();
    for sample in ["wf-agents run", "wf-agents queue put"] {
        assert!(paths.contains(&sample), "{sample} is not among {paths:?}");
    }
    let mut failures = Vec::new();
    for (path, cmd) in &all {
        let (parent, name) = path.rsplit_once(' ').unwrap();
        let got = found_without(path, "");
        if got.len() != 1 || !got[0].contains(path.as_str()) {
            failures.push(format!("{path} left out: found {got:?}"));
        }
        let got = found(&format!("`{parent} {name}x`"));
        if got.len() != 1 || !got[0].contains("no subcommand") {
            failures.push(format!("{path} misspelled: found {got:?}"));
        }
        let got = found(&format!("`{path} --zzz`"));
        if got.len() != 1 || !got[0].contains("zzz") {
            failures.push(format!("{path} with an option it lacks: found {got:?}"));
        }
        for arg in cmd.get_arguments() {
            if let Some(long) = arg.get_long() {
                let got = found(&format!("`{path} --{long}`"));
                if !got.is_empty() {
                    failures.push(format!("{path} --{long}: reported though it exists: {got:?}"));
                }
            }
        }
    }
    // A name that another command has, written under one that lacks it: `wf-agents queue check`.
    let mut nested = Vec::new();
    for (path, _) in &all {
        let words: Vec<&str> = path.split(' ').collect();
        if words.len() == 3 {
            nested.push((words[1], words[2]));
        }
    }
    for (group, _) in &nested {
        for (_, name) in &nested {
            if !nested.contains(&(*group, *name)) {
                let got = found(&format!("`wf-agents {group} {name}`"));
                if got.len() != 1 || !got[0].contains(*name) {
                    failures.push(format!("{group} {name}: found {got:?}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

// A span that wraps right after the name is a place where the program starts a command, though the name ends the line
// and the command is on the next: the line break between them must not hide a wrong command.
#[test]
fn a_span_that_wraps_after_the_name_is_reported() {
    let failures = passed_in_silence(WRAPPED);
    assert!(failures.is_empty(), "{failures:#?}");
}

// A binary run by its path is a place where the program starts a command: a path that ends in the name is the name of
// the program as well as a path to a directory of the crate.
#[test]
fn a_binary_run_by_its_path_is_reported() {
    let failures = passed_in_silence(BY_PATH);
    assert!(failures.is_empty(), "{failures:#?}");
}

// An option in brackets is an option the command line names, as a synopsis writes it: a wrong one must be named.
#[test]
fn an_option_hidden_in_brackets_is_named() {
    let failures = passed_in_silence(IN_BRACKETS);
    assert!(failures.is_empty(), "{failures:#?}");
}

// The word after `-p` or `--package` is the name of a package whatever follows it, as it is when it ends the span.
#[test]
fn a_package_named_wf_agents_is_not_a_command() {
    let failures = reported_though_quiet(PACKAGE);
    assert!(failures.is_empty(), "{failures:#?}");
}

// What follows `-p wf-agents` is not always a cargo option: after `--` it is the command that cargo runs, which can be
// wrong, so it is a place where the program starts a command.
#[test]
fn a_command_run_through_cargo_is_reported() {
    let got = found("`cargo run -p wf-agents -- run --twice`");
    assert!(got.iter().any(|problem| is_unread(problem)), "{got:#?}");
}

// A command line that begins with an option, or has one between its subcommands, does not begin with the words that run
// the command, so it does not write that command (acceptance A1).
#[test]
fn a_command_line_that_starts_with_an_option_does_not_write_the_command() {
    let lines = "`wf-agents --data DIR queue put 3`\n`wf-agents queue --data DIR put 3`";
    let got = found_without("wf-agents queue put", lines);
    assert_eq!(got, ["wf-agents queue put: never written"]);
}

// The bullets of the data directory count where they stand, under its heading and up to the next one: the same bullets
// before the heading or under the next one do not, and neither does a line that only names an entry.
#[test]
fn the_data_directory_section_is_what_stands_under_its_heading() {
    let entries = readme::DATA_DIRECTORY;
    let list = bullets(&entries);
    let heading = "### The data directory";
    let right = format!("## Before\n\n{heading}\n\n{list}\n\n### The next one\n");
    assert!(readme::unlisted(&right).is_empty());
    let apart = format!("{list}\n\n{heading}\n\nNo bullets.\n\n### The next one\n\n{list}\n");
    assert_eq!(readme::unlisted(&apart), entries);
    let mut named = Vec::new();
    for entry in entries {
        named.push(format!("- the file `{entry}`: what it is"));
    }
    let only_named = format!("{heading}\n\n{}\n", named.join("\n"));
    assert_eq!(readme::unlisted(&only_named), entries);
}

// A line in a code block that begins with `#` is no heading: the section can show the data directory's own `.gitignore`
// with a comment in it, and the bullets after the block still count.
#[test]
fn a_comment_in_a_code_block_does_not_end_the_data_directory_section() {
    let entries = readme::DATA_DIRECTORY;
    let (before, after) = entries.split_at(5);
    let (first, last) = (bullets(before), bullets(after));
    let block = "```\n# the data directory's own .gitignore\ndb/\n```";
    let text = format!("### The data directory\n\n{first}\n\n{block}\n\n{last}\n");
    let missing = readme::unlisted(&text);
    assert!(missing.is_empty(), "{missing:?} are not listed");
}
