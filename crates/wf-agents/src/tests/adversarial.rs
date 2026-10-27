//! Tests that look for what the README check lets through: wrong texts it passes, and right ones it reports. Each case
//! adds a few lines to the text that writes every command correctly. The cases that stand in the convention the check
//! reads are in one table, with the command lines they write, so that clap, which parses those command lines, can be
//! asked which of them are wrong. The forms outside the convention are in two more tables: the ones the check has to
//! report, and the ones it has to leave alone. A case stands for one branch of the reader, and its name says which;
//! two cases for the same branch are one. The texts that the acceptance conditions name are in `readme.rs`.

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
    let mut failures = Vec::new();
    for form in QUIET {
        let got = found(form.text);
        if !got.is_empty() {
            failures.push(format!("{}: reported though it is no command: {got:?}", form.name));
        }
    }
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
