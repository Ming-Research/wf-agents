//! Tests that look for what the README check lets through: wrong texts it passes, and right ones it reports. Each case
//! adds a few lines to the text that writes every command correctly. The cases stand in one table, with the command
//! lines they write, so that clap, which parses those command lines, can be asked which of them are wrong.

use clap::error::ErrorKind;

use super::{leaves, problems, tree};

/// A way a README can write command lines, and what the check has to make of it.
struct Case {
    /// The test that holds it.
    group: &'static str,
    /// What it tries.
    name: &'static str,
    /// The command that the text leaves out, as `wf-agents queue put`; none when the text writes every one.
    skip: &'static str,
    /// The Markdown that writes it, added to the text.
    text: &'static str,
    /// The command lines in it, as a shell runs them.
    runs: &'static [&'static str],
    /// The subcommand or option, without dashes, that the check has to name as not there; none when all are right.
    wrong: &'static str,
}

const CASES: &[Case] = &[
    Case {
        group: "options_before_the_subcommand_do_not_end_the_reading",
        name: "unknown subcommand",
        skip: "",
        text: "`wf-agents --data DIR queue add`",
        runs: &["wf-agents --data DIR queue add"],
        wrong: "add",
    },
    Case {
        group: "options_before_the_subcommand_do_not_end_the_reading",
        name: "unknown command",
        skip: "",
        text: "`wf-agents --data DIR frobnicate`",
        runs: &["wf-agents --data DIR frobnicate"],
        wrong: "frobnicate",
    },
    Case {
        group: "options_before_the_subcommand_do_not_end_the_reading",
        name: "after an option",
        skip: "",
        text: "`wf-agents item --data DIR frobnicate`",
        runs: &["wf-agents item --data DIR frobnicate"],
        wrong: "frobnicate",
    },
    Case {
        group: "options_before_the_subcommand_do_not_end_the_reading",
        name: "the equals form",
        skip: "",
        text: "`wf-agents --data=DIR queue add`",
        runs: &["wf-agents --data=DIR queue add"],
        wrong: "add",
    },
    Case {
        group: "options_before_the_subcommand_do_not_end_the_reading",
        name: "unknown option",
        skip: "",
        text: "`wf-agents --data DIR run --twice`",
        runs: &["wf-agents --data DIR run --twice"],
        wrong: "twice",
    },
    Case {
        group: "options_before_the_subcommand_do_not_end_the_reading",
        name: "option of the command",
        skip: "",
        text: "`wf-agents --data DIR run --once`",
        runs: &["wf-agents --data DIR run --once"],
        wrong: "",
    },
    Case {
        group: "options_before_the_subcommand_do_not_end_the_reading",
        name: "options of a nested one",
        skip: "",
        text: "`wf-agents --data DIR item add --project P --title T`",
        runs: &["wf-agents --data DIR item add --project P --title T"],
        wrong: "",
    },
    Case {
        group: "options_before_the_subcommand_do_not_end_the_reading",
        name: "right subcommand",
        skip: "",
        text: "`wf-agents --data DIR queue put 3`",
        runs: &["wf-agents --data DIR queue put 3"],
        wrong: "",
    },
    Case {
        group: "options_before_the_subcommand_do_not_end_the_reading",
        name: "option between the two",
        skip: "",
        text: "`wf-agents item --data DIR list`",
        runs: &["wf-agents item --data DIR list"],
        wrong: "",
    },
    Case {
        group: "options_before_the_subcommand_do_not_end_the_reading",
        name: "the only line that writes the command",
        skip: "wf-agents queue put",
        text: "`wf-agents --data DIR queue put 3`",
        runs: &["wf-agents --data DIR queue put 3"],
        wrong: "",
    },
    Case {
        group: "a_command_line_wrapped_with_a_backslash_is_read_whole",
        name: "unknown option",
        skip: "",
        text: "```\nwf-agents item add \\\n  --project P --bogus X\n```",
        runs: &["wf-agents item add --project P --bogus X"],
        wrong: "bogus",
    },
    Case {
        group: "a_command_line_wrapped_with_a_backslash_is_read_whole",
        name: "unknown subcommand",
        skip: "",
        text: "```\nwf-agents queue \\\n  add 3\n```",
        runs: &["wf-agents queue add 3"],
        wrong: "add",
    },
    Case {
        group: "a_command_line_wrapped_with_a_backslash_is_read_whole",
        name: "right options",
        skip: "",
        text: "```\nwf-agents item add \\\n  --project P --title T\n```",
        runs: &["wf-agents item add --project P --title T"],
        wrong: "",
    },
    Case {
        group: "a_command_line_wrapped_with_a_backslash_is_read_whole",
        name: "right subcommand",
        skip: "",
        text: "```\nwf-agents queue \\\n  put 3\n```",
        runs: &["wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "a_code_span_that_wraps_does_not_hide_the_spans_after_it",
        name: "span after it",
        skip: "",
        text: "see `wf-agents answer 3\n--text T` and `wf-agents queue add`",
        runs: &["wf-agents answer 3 --text T", "wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_code_span_that_wraps_does_not_hide_the_spans_after_it",
        name: "option on line two",
        skip: "",
        text: "see `wf-agents answer 3\n--bogus X` for more",
        runs: &["wf-agents answer 3 --bogus X"],
        wrong: "bogus",
    },
    Case {
        group: "a_code_span_that_wraps_does_not_hide_the_spans_after_it",
        name: "subcommand on line two",
        skip: "",
        text: "see `wf-agents queue\nadd` now",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_code_span_that_wraps_does_not_hide_the_spans_after_it",
        name: "right wrapped span",
        skip: "",
        text: "see `wf-agents answer 3\n--text T` and `wf-agents queue put 3`",
        runs: &["wf-agents answer 3 --text T", "wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "two_commands_in_one_span_are_both_read",
        name: "after &&",
        skip: "",
        text: "`wf-agents status && wf-agents queue add`",
        runs: &["wf-agents status", "wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "two_commands_in_one_span_are_both_read",
        name: "after a semicolon",
        skip: "",
        text: "`wf-agents status; wf-agents queue add`",
        runs: &["wf-agents status", "wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "two_commands_in_one_span_are_both_read",
        name: "after a pipe",
        skip: "",
        text: "`wf-agents status | wf-agents queue add`",
        runs: &["wf-agents status", "wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "two_commands_in_one_span_are_both_read",
        name: "right pair",
        skip: "",
        text: "`wf-agents status && wf-agents queue put 3`",
        runs: &["wf-agents status", "wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "two_commands_in_one_span_are_both_read",
        name: "right pair with a semicolon",
        skip: "",
        text: "`wf-agents status; wf-agents queue put 3`",
        runs: &["wf-agents status", "wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "two_commands_in_one_span_are_both_read",
        name: "piped to another program",
        skip: "",
        text: "`wf-agents status | grep work`",
        runs: &["wf-agents status"],
        wrong: "",
    },
    Case {
        group: "other_forms_of_code_are_read",
        name: "tilde fence",
        skip: "",
        text: "~~~\nwf-agents queue add\n~~~",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "other_forms_of_code_are_read",
        name: "double backticks",
        skip: "",
        text: "``wf-agents queue add``",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "other_forms_of_code_are_read",
        name: "right tilde fence",
        skip: "",
        text: "~~~\nwf-agents queue put 3\n~~~",
        runs: &["wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "other_forms_of_code_are_read",
        name: "right double backticks",
        skip: "",
        text: "``wf-agents queue put 3``",
        runs: &["wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "right_wording_is_not_reported",
        name: "end of the options",
        skip: "",
        text: "`wf-agents say 3 -- --stop`",
        runs: &["wf-agents say 3 -- --stop"],
        wrong: "",
    },
    Case {
        group: "right_wording_is_not_reported",
        name: "dashes in a quote",
        skip: "",
        text: "`wf-agents say 3 \"do not use --force\"`",
        runs: &["wf-agents say 3 \"do not use --force\""],
        wrong: "",
    },
    Case {
        group: "right_wording_is_not_reported",
        name: "plain quoted value",
        skip: "",
        text: "`wf-agents answer 3 --text \"ok\"`",
        runs: &["wf-agents answer 3 --text \"ok\""],
        wrong: "",
    },
    Case {
        group: "right_wording_is_not_reported",
        name: "a short option",
        skip: "",
        text: "`wf-agents run -h`",
        runs: &["wf-agents run -h"],
        wrong: "",
    },
    Case {
        group: "a_prompt_or_an_environment_variable_before_the_command_does_not_hide_it",
        name: "shell prompt",
        skip: "",
        text: "```\n$ wf-agents queue add\n```",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_prompt_or_an_environment_variable_before_the_command_does_not_hide_it",
        name: "shell prompt in a span",
        skip: "",
        text: "`$ wf-agents queue add`",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_prompt_or_an_environment_variable_before_the_command_does_not_hide_it",
        name: "environment variable",
        skip: "",
        text: "```\nWF_AGENTS_DATA=DIR wf-agents queue add\n```",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_prompt_or_an_environment_variable_before_the_command_does_not_hide_it",
        name: "environment variable, unknown option",
        skip: "",
        text: "`WF_AGENTS_DATA=DIR wf-agents run --twice`",
        runs: &["wf-agents run --twice"],
        wrong: "twice",
    },
    Case {
        group: "a_prompt_or_an_environment_variable_before_the_command_does_not_hide_it",
        name: "right shell prompt",
        skip: "",
        text: "```\n$ wf-agents queue put 3\n```",
        runs: &["wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "a_prompt_or_an_environment_variable_before_the_command_does_not_hide_it",
        name: "right environment variable",
        skip: "",
        text: "`WF_AGENTS_DATA=DIR wf-agents queue put 3`",
        runs: &["wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "placeholders_and_the_options_every_command_has_are_not_reported",
        name: "placeholder for the command",
        skip: "",
        text: "`wf-agents <command> --help`",
        runs: &[],
        wrong: "",
    },
    Case {
        group: "placeholders_and_the_options_every_command_has_are_not_reported",
        name: "placeholder under a group",
        skip: "",
        text: "`wf-agents item <command> --help`",
        runs: &[],
        wrong: "",
    },
    Case {
        group: "placeholders_and_the_options_every_command_has_are_not_reported",
        name: "help",
        skip: "",
        text: "`wf-agents --help`",
        runs: &["wf-agents --help"],
        wrong: "",
    },
    Case {
        group: "placeholders_and_the_options_every_command_has_are_not_reported",
        name: "version",
        skip: "",
        text: "`wf-agents --version`",
        runs: &["wf-agents --version"],
        wrong: "",
    },
    Case {
        group: "placeholders_and_the_options_every_command_has_are_not_reported",
        name: "options written with an equals sign",
        skip: "",
        text: "`wf-agents answer 3 --verdict=confirm --text=ok`",
        runs: &["wf-agents answer 3 --verdict=confirm --text=ok"],
        wrong: "",
    },
    Case {
        group: "placeholders_and_the_options_every_command_has_are_not_reported",
        name: "an option with a dash in its name",
        skip: "",
        text: "`wf-agents item add --depends-on 2:the-api --project P`",
        runs: &["wf-agents item add --depends-on 2:the-api --project P"],
        wrong: "",
    },
    Case {
        group: "placeholders_and_the_options_every_command_has_are_not_reported",
        name: "a flag after a number",
        skip: "",
        text: "`wf-agents history 3 --full`",
        runs: &["wf-agents history 3 --full"],
        wrong: "",
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

/// Runs the cases of one group through the check: it has to name what is wrong with a wrong one, and report nothing
/// for a right one.
fn check_group(group: &str) {
    let cases: Vec<&Case> = CASES.iter().filter(|case| case.group == group).collect();
    assert!(!cases.is_empty(), "no case is in {group}");
    let mut failures = Vec::new();
    for case in cases {
        let got = found_without(case.skip, case.text);
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
    // A word in double quotes is one argument, as a shell reads it.
    let mut words: Vec<String> = Vec::new();
    for (i, part) in line.split('"').enumerate() {
        if i % 2 == 1 {
            words.push(part.to_string());
        } else {
            words.extend(part.split_whitespace().map(str::to_string));
        }
    }
    let err = tree().try_get_matches_from(words).err()?;
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

// A command line wrapped with a backslash is one command line: the words on its next line are read too.
#[test]
fn a_command_line_wrapped_with_a_backslash_is_read_whole() {
    check_group("a_command_line_wrapped_with_a_backslash_is_read_whole");
}

// A code span that wraps onto the next line is one span: what stands on its second line is read, and the spans after
// it on that line are still found.
#[test]
fn a_code_span_that_wraps_does_not_hide_the_spans_after_it() {
    check_group("a_code_span_that_wraps_does_not_hide_the_spans_after_it");
}

// Two commands in one code span are both read: the one after `&&`, `;` or `|` is a command line of its own.
#[test]
fn two_commands_in_one_span_are_both_read() {
    check_group("two_commands_in_one_span_are_both_read");
}

// Command lines in the other forms Markdown has for code are read: a block fenced with tildes, and a span in double
// backticks.
#[test]
fn other_forms_of_code_are_read() {
    check_group("other_forms_of_code_are_read");
}

// Right wording is not reported: the end of the options, a quoted value that holds a word with dashes, and a short
// option.
#[test]
fn right_wording_is_not_reported() {
    check_group("right_wording_is_not_reported");
}

// A shell prompt or an environment variable before the command does not hide it: the README gives the data directory
// in `WF_AGENTS_DATA`, and a shell session shows the command after its prompt.
#[test]
fn a_prompt_or_an_environment_variable_before_the_command_does_not_hide_it() {
    check_group("a_prompt_or_an_environment_variable_before_the_command_does_not_hide_it");
}

// Placeholders and the options every command has are not reported, whatever the check does with the words after them.
#[test]
fn placeholders_and_the_options_every_command_has_are_not_reported() {
    check_group("placeholders_and_the_options_every_command_has_are_not_reported");
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
