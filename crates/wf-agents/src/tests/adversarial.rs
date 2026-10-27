//! Tests that look for what the README check lets through: wrong texts it passes, and right ones it reports. Each case
//! adds a few lines to the text that writes every command correctly. The cases that stand in the convention the check
//! reads are in one table, with the command lines they write, so that clap, which parses those command lines, can be
//! asked which of them are wrong. The forms outside the convention are in two more tables: the ones the check has to
//! report, and the ones it has to leave alone.

use clap::error::ErrorKind;

use super::readme::{self, leaves, problems, tree};

/// A way a README can write command lines in the convention, and what the check has to make of it.
struct Case {
    /// The test that holds it.
    group: &'static str,
    /// What it tries.
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
        group: "options_before_the_subcommand_do_not_end_the_reading",
        name: "unknown subcommand",
        text: "`wf-agents --data DIR queue add`",
        runs: &["wf-agents --data DIR queue add"],
        wrong: "add",
    },
    Case {
        group: "options_before_the_subcommand_do_not_end_the_reading",
        name: "unknown command",
        text: "`wf-agents --data DIR frobnicate`",
        runs: &["wf-agents --data DIR frobnicate"],
        wrong: "frobnicate",
    },
    Case {
        group: "options_before_the_subcommand_do_not_end_the_reading",
        name: "after an option",
        text: "`wf-agents item --data DIR frobnicate`",
        runs: &["wf-agents item --data DIR frobnicate"],
        wrong: "frobnicate",
    },
    Case {
        group: "options_before_the_subcommand_do_not_end_the_reading",
        name: "the equals form",
        text: "`wf-agents --data=DIR queue add`",
        runs: &["wf-agents --data=DIR queue add"],
        wrong: "add",
    },
    Case {
        group: "options_before_the_subcommand_do_not_end_the_reading",
        name: "unknown option",
        text: "`wf-agents --data DIR run --twice`",
        runs: &["wf-agents --data DIR run --twice"],
        wrong: "twice",
    },
    Case {
        group: "options_before_the_subcommand_do_not_end_the_reading",
        name: "option of the command",
        text: "`wf-agents --data DIR run --once`",
        runs: &["wf-agents --data DIR run --once"],
        wrong: "",
    },
    Case {
        group: "options_before_the_subcommand_do_not_end_the_reading",
        name: "options of a nested one",
        text: "`wf-agents --data DIR item add --project P --title T`",
        runs: &["wf-agents --data DIR item add --project P --title T"],
        wrong: "",
    },
    Case {
        group: "options_before_the_subcommand_do_not_end_the_reading",
        name: "right subcommand",
        text: "`wf-agents --data DIR queue put 3`",
        runs: &["wf-agents --data DIR queue put 3"],
        wrong: "",
    },
    Case {
        group: "options_before_the_subcommand_do_not_end_the_reading",
        name: "option between the two",
        text: "`wf-agents item --data DIR list`",
        runs: &["wf-agents item --data DIR list"],
        wrong: "",
    },
    Case {
        group: "options_before_the_subcommand_do_not_end_the_reading",
        name: "a value that is the name of a subcommand",
        text: "`wf-agents --data queue put 3`",
        runs: &["wf-agents --data queue put 3"],
        wrong: "put",
    },
    Case {
        group: "options_before_the_subcommand_do_not_end_the_reading",
        name: "a value that is the name of a command",
        text: "`wf-agents --data run status`",
        runs: &["wf-agents --data run status"],
        wrong: "",
    },
    Case {
        group: "a_block_fenced_with_tildes_is_read",
        name: "tilde fence",
        text: "~~~\nwf-agents queue add\n~~~",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_block_fenced_with_tildes_is_read",
        name: "right tilde fence",
        text: "~~~\nwf-agents queue put 3\n~~~",
        runs: &["wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "right_wording_is_not_reported",
        name: "end of the options",
        text: "`wf-agents say 3 -- --stop`",
        runs: &["wf-agents say 3 -- --stop"],
        wrong: "",
    },
    Case {
        group: "right_wording_is_not_reported",
        name: "a short option",
        text: "`wf-agents run -h`",
        runs: &["wf-agents run -h"],
        wrong: "",
    },
    Case {
        group: "right_wording_is_not_reported",
        name: "the value of an option is three dots",
        text: "`wf-agents answer 3 --text ... --why ...`",
        runs: &["wf-agents answer 3 --text ... --why ..."],
        wrong: "",
    },
    Case {
        group: "right_wording_is_not_reported",
        name: "the value of an option is a placeholder",
        text: "`wf-agents config check --data DIR`",
        runs: &["wf-agents config check --data DIR"],
        wrong: "",
    },
    Case {
        group: "a_shell_prompt_before_the_command_does_not_hide_it",
        name: "shell prompt",
        text: "```\n$ wf-agents queue add\n```",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_shell_prompt_before_the_command_does_not_hide_it",
        name: "shell prompt in a span",
        text: "`$ wf-agents queue add`",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_shell_prompt_before_the_command_does_not_hide_it",
        name: "right shell prompt",
        text: "```\n$ wf-agents queue put 3\n```",
        runs: &["wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "a_shell_prompt_before_the_command_does_not_hide_it",
        name: "right shell prompt in a span",
        text: "`$ wf-agents queue put 3`",
        runs: &["wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "a_code_span_of_three_backticks_is_not_a_fence",
        name: "span on a line of its own",
        text: "```wf-agents queue add```",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_code_span_of_three_backticks_is_not_a_fence",
        name: "span after it",
        text: "```wf-agents status```\n\nsee `wf-agents queue add`",
        runs: &["wf-agents status", "wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_code_span_of_three_backticks_is_not_a_fence",
        name: "a real block after it",
        text: "```wf-agents status```\n\n```\nwf-agents run --twice\n```",
        runs: &["wf-agents status", "wf-agents run --twice"],
        wrong: "twice",
    },
    Case {
        group: "a_code_span_of_three_backticks_is_not_a_fence",
        name: "right span and span after it",
        text: "```wf-agents queue put 3```\n\nsee `wf-agents status`",
        runs: &["wf-agents queue put 3", "wf-agents status"],
        wrong: "",
    },
    Case {
        group: "placeholders_and_the_options_every_command_has_are_not_reported",
        name: "placeholder for the command",
        text: "`wf-agents <command> --help`",
        runs: &[],
        wrong: "",
    },
    Case {
        group: "placeholders_and_the_options_every_command_has_are_not_reported",
        name: "placeholder under a group",
        text: "`wf-agents item <command> --help`",
        runs: &[],
        wrong: "",
    },
    Case {
        group: "placeholders_and_the_options_every_command_has_are_not_reported",
        name: "help",
        text: "`wf-agents --help`",
        runs: &["wf-agents --help"],
        wrong: "",
    },
    Case {
        group: "placeholders_and_the_options_every_command_has_are_not_reported",
        name: "version",
        text: "`wf-agents --version`",
        runs: &["wf-agents --version"],
        wrong: "",
    },
    Case {
        group: "placeholders_and_the_options_every_command_has_are_not_reported",
        name: "options written with an equals sign",
        text: "`wf-agents answer 3 --verdict=confirm --text=ok`",
        runs: &["wf-agents answer 3 --verdict=confirm --text=ok"],
        wrong: "",
    },
    Case {
        group: "placeholders_and_the_options_every_command_has_are_not_reported",
        name: "an option with a dash in its name",
        text: "`wf-agents item add --depends-on 2:the-api --project P`",
        runs: &["wf-agents item add --depends-on 2:the-api --project P"],
        wrong: "",
    },
    Case {
        group: "placeholders_and_the_options_every_command_has_are_not_reported",
        name: "a flag after a number",
        text: "`wf-agents history 3 --full`",
        runs: &["wf-agents history 3 --full"],
        wrong: "",
    },
    Case {
        group: "a_backtick_that_is_not_closed_does_not_hide_the_next_line",
        name: "an unclosed backtick in a list item",
        text: "- not closed `wf-agents status\n- run `wf-agents queue add`",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_backtick_that_is_not_closed_does_not_hide_the_next_line",
        name: "an unclosed backtick in a heading",
        text: "## Not closed `here\n`wf-agents queue add`",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_span_on_one_line_is_read_wherever_it_stands",
        name: "in a block quote",
        text: "> run `wf-agents queue add` now",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_span_on_one_line_is_read_wherever_it_stands",
        name: "in a list item",
        text: "- run `wf-agents queue add` now",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_span_on_one_line_is_read_wherever_it_stands",
        name: "a right one in a block quote",
        text: "> run `wf-agents queue put 3` now",
        runs: &["wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "a_span_on_one_line_is_read_wherever_it_stands",
        name: "a right one in a list item",
        text: "- run `wf-agents queue put 3` now",
        runs: &["wf-agents queue put 3"],
        wrong: "",
    },
];

/// Forms the check does not read, in which `wf-agents` starts a command: it has to report each, whether the command in
/// it is right or wrong, because a wrong one would otherwise pass without a word.
const OUTSIDE: &[Form] = &[
    Form {
        name: "after another word",
        text: "`nohup wf-agents run --twice`",
    },
    Form {
        name: "after a utility and a variable",
        text: "`env WF_AGENTS_DATA=DIR wf-agents queue add`",
    },
    Form {
        name: "after a variable",
        text: "`WF_AGENTS_DATA=DIR wf-agents run --twice`",
    },
    Form {
        name: "after a variable, a right command",
        text: "`WF_AGENTS_DATA=DIR wf-agents queue put 3`",
    },
    Form {
        name: "in a loop",
        text: "`while true; do wf-agents run --twice; done`",
    },
    Form {
        name: "in a loop, a right command",
        text: "`while true; do wf-agents run --once; sleep 60; done`",
    },
    Form {
        name: "after sudo in a fenced block",
        text: "```\nsudo wf-agents run\n```",
    },
    Form {
        name: "after &&",
        text: "`wf-agents status && wf-agents queue add`",
    },
    Form {
        name: "after a semicolon",
        text: "`wf-agents status; wf-agents queue add`",
    },
    Form {
        name: "after a pipe",
        text: "`wf-agents status | wf-agents queue add`",
    },
    Form {
        name: "piped to another program",
        text: "`wf-agents status | grep work`",
    },
    Form {
        name: "in a subshell",
        text: "`(cd DIR && wf-agents status)`",
    },
    Form {
        name: "in a command substitution",
        text: "`wf-agents say 3 $(wf-agents queue add)`",
    },
    Form {
        name: "in a right command substitution",
        text: "`wf-agents say 3 $(wf-agents status)`",
    },
    Form {
        name: "a quoted value",
        text: "`wf-agents answer 3 --text \"ok\"`",
    },
    Form {
        name: "dashes in a quote",
        text: "`wf-agents say 3 \"do not use --force\"`",
    },
    Form {
        name: "a quoted value that runs onto the next line",
        text: "```sh\nwf-agents say 3 \"first line\nsecond line\"\n```",
    },
    Form {
        name: "a backslash at the end of a line",
        text: "```\nwf-agents item add \\\n  --project P --bogus X\n```",
    },
    Form {
        name: "a right command wrapped with a backslash",
        text: "```\nwf-agents queue \\\n  put 3\n```",
    },
    Form {
        name: "indented by four spaces",
        text: "Run it:\n\n    wf-agents queue add",
    },
    Form {
        name: "indented by a tab",
        text: "Run it:\n\n\twf-agents queue add",
    },
    Form {
        name: "in a block quote",
        text: "> wf-agents queue add",
    },
    Form {
        name: "a right command in a block quote",
        text: "> wf-agents status",
    },
    Form {
        name: "a fence in a block quote",
        text: "> ```sh\n> wf-agents queue add\n> ```",
    },
    Form {
        name: "in HTML",
        text: "<code>wf-agents queue add</code>",
    },
    Form {
        name: "a span that wraps",
        text: "see `wf-agents queue\nadd` now",
    },
    Form {
        name: "a right span that wraps",
        text: "see `wf-agents answer 3\n--text T` and `wf-agents queue put 3`",
    },
    Form {
        name: "a span that wraps in a list item",
        text: "- run `wf-agents queue\n  put 3` now",
    },
    Form {
        name: "double backticks",
        text: "``wf-agents queue add``",
    },
    Form {
        name: "right double backticks",
        text: "``wf-agents queue put 3``",
    },
    Form {
        name: "an escaped backtick before a span",
        text: "type \\` to quote it, then run `wf-agents queue add`",
    },
    Form {
        name: "a backslash before the closing backtick of a span",
        text: "`wf-agents status \\` `wf-agents queue add`",
    },
    Form {
        name: "an option in prose",
        text: "Start it with wf-agents --version",
    },
];

/// Places where `wf-agents` is not the start of a command: the check has to leave each alone.
const QUIET: &[Form] = &[
    Form {
        name: "the package in a cargo command",
        text: "`cargo build -p wf-agents`",
    },
    Form {
        name: "a path that has the name among its directories",
        text: "`crates/wf-agents/src/main.rs`",
    },
    Form {
        name: "a file name",
        text: "`wf-agents.sqlite`",
    },
    Form {
        name: "a word of prose",
        text: "wf-agents runs coding agents through a defined flow.",
    },
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
        name: "a second paragraph of a list item",
        text: "\n1. Do it first.\n\n    wf-agents runs the flow by steps.",
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

/// Runs the cases of one group through the check: it has to name what is wrong with a wrong one, and report nothing
/// for a right one.
fn check_group(group: &str) {
    let cases: Vec<&Case> = CASES.iter().filter(|case| case.group == group).collect();
    assert!(!cases.is_empty(), "no case is in {group}");
    let mut failures = Vec::new();
    for case in cases {
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

// A global option before the subcommand does not end the reading of the command line: the subcommand after it is
// looked up, and the options that follow are the options of that command.
#[test]
fn options_before_the_subcommand_do_not_end_the_reading() {
    check_group("options_before_the_subcommand_do_not_end_the_reading");
}

// A block fenced with tildes is a block of code like one fenced with backticks (CommonMark 0.31, fenced code blocks).
#[test]
fn a_block_fenced_with_tildes_is_read() {
    check_group("a_block_fenced_with_tildes_is_read");
}

// Right wording is not reported: the end of the options, a short option, and the values the README writes in place of
// a real one.
#[test]
fn right_wording_is_not_reported() {
    check_group("right_wording_is_not_reported");
}

// A shell prompt before the command does not hide it: a shell session shows the command after its prompt.
#[test]
fn a_shell_prompt_before_the_command_does_not_hide_it() {
    check_group("a_shell_prompt_before_the_command_does_not_hide_it");
}

// A line that begins with three backticks and has a backtick after them is a code span, not the start of a fenced
// block: the info string of a backtick fence has no backtick in it (CommonMark 0.31, fenced code blocks). Read as a
// fence, it turns every block after it inside out.
#[test]
fn a_code_span_of_three_backticks_is_not_a_fence() {
    check_group("a_code_span_of_three_backticks_is_not_a_fence");
}

// Placeholders and the options every command has are not reported, whatever the check does with the words after them.
#[test]
fn placeholders_and_the_options_every_command_has_are_not_reported() {
    check_group("placeholders_and_the_options_every_command_has_are_not_reported");
}

// A backtick with no partner on its line opens no code span, so one missing backtick does not hide the command on the
// next line: spans are paired line by line.
#[test]
fn a_backtick_that_is_not_closed_does_not_hide_the_next_line() {
    check_group("a_backtick_that_is_not_closed_does_not_hide_the_next_line");
}

// A code span on one line is read wherever it stands: in a block quote, in a list item.
#[test]
fn a_span_on_one_line_is_read_wherever_it_stands() {
    check_group("a_span_on_one_line_is_read_wherever_it_stands");
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

// A place where `wf-agents` is a package, a path, a file name, a heading or a word of prose is not a command and is
// not reported.
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
    assert_eq!(all.len(), 20);
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
