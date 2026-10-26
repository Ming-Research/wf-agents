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
        group: "a_command_line_wrapped_with_a_backslash_is_read_whole",
        name: "unknown option",
        text: "```\nwf-agents item add \\\n  --project P --bogus X\n```",
        runs: &["wf-agents item add --project P --bogus X"],
        wrong: "bogus",
    },
    Case {
        group: "a_command_line_wrapped_with_a_backslash_is_read_whole",
        name: "unknown subcommand",
        text: "```\nwf-agents queue \\\n  add 3\n```",
        runs: &["wf-agents queue add 3"],
        wrong: "add",
    },
    Case {
        group: "a_command_line_wrapped_with_a_backslash_is_read_whole",
        name: "right options",
        text: "```\nwf-agents item add \\\n  --project P --title T\n```",
        runs: &["wf-agents item add --project P --title T"],
        wrong: "",
    },
    Case {
        group: "a_command_line_wrapped_with_a_backslash_is_read_whole",
        name: "right subcommand",
        text: "```\nwf-agents queue \\\n  put 3\n```",
        runs: &["wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "a_code_span_that_wraps_does_not_hide_the_spans_after_it",
        name: "span after it",
        text: "see `wf-agents answer 3\n--text T` and `wf-agents queue add`",
        runs: &["wf-agents answer 3 --text T", "wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_code_span_that_wraps_does_not_hide_the_spans_after_it",
        name: "option on line two",
        text: "see `wf-agents answer 3\n--bogus X` for more",
        runs: &["wf-agents answer 3 --bogus X"],
        wrong: "bogus",
    },
    Case {
        group: "a_code_span_that_wraps_does_not_hide_the_spans_after_it",
        name: "subcommand on line two",
        text: "see `wf-agents queue\nadd` now",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_code_span_that_wraps_does_not_hide_the_spans_after_it",
        name: "right wrapped span",
        text: "see `wf-agents answer 3\n--text T` and `wf-agents queue put 3`",
        runs: &["wf-agents answer 3 --text T", "wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "two_commands_in_one_span_are_both_read",
        name: "after &&",
        text: "`wf-agents status && wf-agents queue add`",
        runs: &["wf-agents status", "wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "two_commands_in_one_span_are_both_read",
        name: "after a semicolon",
        text: "`wf-agents status; wf-agents queue add`",
        runs: &["wf-agents status", "wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "two_commands_in_one_span_are_both_read",
        name: "after a pipe",
        text: "`wf-agents status | wf-agents queue add`",
        runs: &["wf-agents status", "wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "two_commands_in_one_span_are_both_read",
        name: "right pair",
        text: "`wf-agents status && wf-agents queue put 3`",
        runs: &["wf-agents status", "wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "two_commands_in_one_span_are_both_read",
        name: "right pair with a semicolon",
        text: "`wf-agents status; wf-agents queue put 3`",
        runs: &["wf-agents status", "wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "two_commands_in_one_span_are_both_read",
        name: "piped to another program",
        text: "`wf-agents status | grep work`",
        runs: &["wf-agents status"],
        wrong: "",
    },
    Case {
        group: "other_forms_of_code_are_read",
        name: "tilde fence",
        text: "~~~\nwf-agents queue add\n~~~",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "other_forms_of_code_are_read",
        name: "double backticks",
        text: "``wf-agents queue add``",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "other_forms_of_code_are_read",
        name: "right tilde fence",
        text: "~~~\nwf-agents queue put 3\n~~~",
        runs: &["wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "other_forms_of_code_are_read",
        name: "right double backticks",
        text: "``wf-agents queue put 3``",
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
        name: "dashes in a quote",
        text: "`wf-agents say 3 \"do not use --force\"`",
        runs: &["wf-agents say 3 \"do not use --force\""],
        wrong: "",
    },
    Case {
        group: "right_wording_is_not_reported",
        name: "plain quoted value",
        text: "`wf-agents answer 3 --text \"ok\"`",
        runs: &["wf-agents answer 3 --text \"ok\""],
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
        group: "a_prompt_or_an_environment_variable_before_the_command_does_not_hide_it",
        name: "shell prompt",
        text: "```\n$ wf-agents queue add\n```",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_prompt_or_an_environment_variable_before_the_command_does_not_hide_it",
        name: "shell prompt in a span",
        text: "`$ wf-agents queue add`",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_prompt_or_an_environment_variable_before_the_command_does_not_hide_it",
        name: "environment variable",
        text: "```\nWF_AGENTS_DATA=DIR wf-agents queue add\n```",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_prompt_or_an_environment_variable_before_the_command_does_not_hide_it",
        name: "environment variable, unknown option",
        text: "`WF_AGENTS_DATA=DIR wf-agents run --twice`",
        runs: &["wf-agents run --twice"],
        wrong: "twice",
    },
    Case {
        group: "a_prompt_or_an_environment_variable_before_the_command_does_not_hide_it",
        name: "right shell prompt",
        text: "```\n$ wf-agents queue put 3\n```",
        runs: &["wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "a_prompt_or_an_environment_variable_before_the_command_does_not_hide_it",
        name: "right environment variable",
        text: "`WF_AGENTS_DATA=DIR wf-agents queue put 3`",
        runs: &["wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "a_command_after_a_shell_keyword_is_read",
        name: "in a loop",
        text: "`while true; do wf-agents run --twice; done`",
        runs: &["wf-agents run --twice"],
        wrong: "twice",
    },
    Case {
        group: "a_command_after_a_shell_keyword_is_read",
        name: "in a condition",
        text: "`if wf-agents queue add; then echo ok; fi`",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_command_after_a_shell_keyword_is_read",
        name: "right loop",
        text: "`while true; do wf-agents run --once; sleep 60; done`",
        runs: &["wf-agents run --once"],
        wrong: "",
    },
    Case {
        group: "a_program_given_by_its_path_is_read",
        name: "relative path",
        text: "`./target/release/wf-agents queue add`",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_program_given_by_its_path_is_read",
        name: "path in a block",
        text: "```\ntarget/release/wf-agents run --twice\n```",
        runs: &["wf-agents run --twice"],
        wrong: "twice",
    },
    Case {
        group: "a_program_given_by_its_path_is_read",
        name: "right path",
        text: "`./target/release/wf-agents queue put 3`",
        runs: &["wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "a_program_given_by_its_path_is_read",
        name: "a path that is not the program",
        text: "`crates/wf-agents/src/main.rs`",
        runs: &[],
        wrong: "",
    },
    Case {
        group: "a_program_given_by_its_path_is_read",
        name: "the package in a cargo command",
        text: "`cargo test -p wf-agents --test readme`",
        runs: &[],
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
        group: "a_quoted_value_that_runs_onto_the_next_line_is_one_value",
        name: "an option after the quote",
        text: "```sh\nwf-agents item add --project P --title T --problem \"A,\nB\" --why W --level L --bogus\n```",
        runs: &["wf-agents item add --project P --title T --problem \"A, B\" --why W --level L --bogus"],
        wrong: "bogus",
    },
    Case {
        group: "a_quoted_value_that_runs_onto_the_next_line_is_one_value",
        name: "after a continued line",
        text: "```sh\nwf-agents pause 3 \\\n  --why \"A,\nB\" --bogus\n```",
        runs: &["wf-agents pause 3 --why \"A, B\" --bogus"],
        wrong: "bogus",
    },
    Case {
        group: "a_quoted_value_that_runs_onto_the_next_line_is_one_value",
        name: "a second line that begins with the program",
        text: "```sh\nwf-agents say 3 \"first line\nwf-agents queue add\"\n```",
        runs: &["wf-agents say 3 \"first line wf-agents queue add\""],
        wrong: "",
    },
    Case {
        group: "a_quoted_value_that_runs_onto_the_next_line_is_one_value",
        name: "right options after the quote",
        text: "```sh\nwf-agents item add --project P --title T --problem \"A,\nB\" --why W --level L --queue\n```",
        runs: &["wf-agents item add --project P --title T --problem \"A, B\" --why W --level L --queue"],
        wrong: "",
    },
    Case {
        group: "a_code_span_does_not_run_over_into_the_next_block",
        name: "an unclosed span in a list item",
        text: "- not closed `wf-agents status\n- run `wf-agents queue add`",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_code_span_does_not_run_over_into_the_next_block",
        name: "an unclosed span in a heading",
        text: "## Not closed `here\n`wf-agents queue add`",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_code_span_does_not_run_over_into_the_next_block",
        name: "a span that wraps inside a list item",
        text: "- run `wf-agents queue\n  put 3` now",
        runs: &["wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "a_command_after_a_utility_that_runs_it_is_read",
        name: "nohup",
        text: "`nohup wf-agents run --twice &`",
        runs: &["wf-agents run --twice"],
        wrong: "twice",
    },
    Case {
        group: "a_command_after_a_utility_that_runs_it_is_read",
        name: "exec",
        text: "`exec wf-agents queue add`",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_command_after_a_utility_that_runs_it_is_read",
        name: "env with a variable",
        text: "`env WF_AGENTS_DATA=DIR wf-agents queue add`",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_command_after_a_utility_that_runs_it_is_read",
        name: "right nohup",
        text: "`nohup wf-agents run --once &`",
        runs: &["wf-agents run --once"],
        wrong: "",
    },
    Case {
        group: "a_command_in_a_subshell_or_a_substitution_is_read",
        name: "a subshell",
        text: "`(cd DIR && wf-agents status)`",
        runs: &["wf-agents status"],
        wrong: "",
    },
    Case {
        group: "a_command_in_a_subshell_or_a_substitution_is_read",
        name: "a subshell, a right option",
        text: "`(cd DIR && wf-agents run --once)`",
        runs: &["wf-agents run --once"],
        wrong: "",
    },
    Case {
        group: "a_command_in_a_subshell_or_a_substitution_is_read",
        name: "a subshell, a wrong subcommand",
        text: "`(cd DIR && wf-agents queue add)`",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_command_in_a_subshell_or_a_substitution_is_read",
        name: "a command substitution",
        text: "`wf-agents say 3 $(wf-agents queue add)`",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_command_in_a_subshell_or_a_substitution_is_read",
        name: "a right command substitution",
        text: "`wf-agents say 3 $(wf-agents status)`",
        runs: &["wf-agents status"],
        wrong: "",
    },
    Case {
        group: "a_block_quote_is_read_without_its_marker",
        name: "a fence in a block quote",
        text: "> ```sh\n> wf-agents queue add\n> ```",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_block_quote_is_read_without_its_marker",
        name: "a right fence in a block quote",
        text: "> ```sh\n> wf-agents status\n> ```",
        runs: &["wf-agents status"],
        wrong: "",
    },
    Case {
        group: "a_block_quote_is_read_without_its_marker",
        name: "a span that wraps in a block quote",
        text: "> run `wf-agents queue\n> add 3` now",
        runs: &["wf-agents queue add 3"],
        wrong: "add",
    },
    Case {
        group: "a_block_quote_is_read_without_its_marker",
        name: "a right span that wraps in a block quote",
        text: "> run `wf-agents queue\n> put 3` now",
        runs: &["wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "a_block_quote_is_read_without_its_marker",
        name: "a right span on one line of a block quote",
        text: "> run `wf-agents queue put 3` now",
        runs: &["wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "a_command_in_an_indented_code_block_is_read",
        name: "four spaces after a blank line",
        text: "Run it:\n\n    wf-agents queue add",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_command_in_an_indented_code_block_is_read",
        name: "a tab after a blank line",
        text: "Run it:\n\n\twf-agents queue add",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "a_command_in_an_indented_code_block_is_read",
        name: "a right line in an indented block",
        text: "Run it:\n\n    wf-agents queue put 3",
        runs: &["wf-agents queue put 3"],
        wrong: "",
    },
    Case {
        group: "a_command_in_an_indented_code_block_is_read",
        name: "an indented line that goes on with a paragraph",
        text: "Prose that goes on\n    wf-agents belongs to no project",
        runs: &[],
        wrong: "",
    },
    Case {
        group: "a_command_in_an_indented_code_block_is_read",
        name: "a second paragraph of a list item",
        text: "\n1. Do it first.\n\n    wf-agents runs the flow by steps.",
        runs: &[],
        wrong: "",
    },
    Case {
        group: "an_escaped_backtick_does_not_open_a_code_span",
        name: "an escaped backtick before a span",
        text: "type \\` to quote it, then run `wf-agents queue add`",
        runs: &["wf-agents queue add"],
        wrong: "add",
    },
    Case {
        group: "an_escaped_backtick_does_not_open_a_code_span",
        name: "a backslash before the closing backtick of a span",
        text: "`wf-agents status \\` `wf-agents queue add`",
        runs: &["wf-agents queue add"],
        wrong: "add",
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

// A command after a shell keyword is a command line of its own: `do`, `then` and `if` stand before a command in a loop
// or a condition, as a prompt or a variable does (POSIX shell, reserved words and compound commands).
#[test]
fn a_command_after_a_shell_keyword_is_read() {
    check_group("a_command_after_a_shell_keyword_is_read");
}

// A program given by its path is the program: `./target/release/wf-agents queue add` runs the same command as
// `wf-agents queue add`, and the README builds the command with cargo, so a path to it is likely. A path that only
// has `wf-agents` among its directories, or a cargo package of that name, is not the program.
#[test]
fn a_program_given_by_its_path_is_read() {
    check_group("a_program_given_by_its_path_is_read");
}

// A line that begins with three backticks and has a backtick after them is a code span, not the start of a fenced
// block: the info string of a backtick fence has no backtick in it (CommonMark 0.31, fenced code blocks). Read as a
// fence, it turns every block after it inside out.
#[test]
fn a_code_span_of_three_backticks_is_not_a_fence() {
    check_group("a_code_span_of_three_backticks_is_not_a_fence");
}

// A quoted value that runs onto the next line is one value, as a shell reads it: the words on the next line are part
// of the quoted text, and what follows the closing quote is again the command's options (POSIX shell command language,
// quoting: a quotation goes on, newlines in it kept, until the closing quote). `wf-agents item add` with a long
// `--problem` is the command a README is most likely to write over several lines.
#[test]
fn a_quoted_value_that_runs_onto_the_next_line_is_one_value() {
    check_group("a_quoted_value_that_runs_onto_the_next_line_is_one_value");
}

// A code span does not run over into the next block: a backtick with no match in its own block is text, and the span
// in the next list item or after a heading is still read (CommonMark 0.31, section 3.1: block structure takes
// precedence over inline structure). One missing backtick must not hide the command after it.
#[test]
fn a_code_span_does_not_run_over_into_the_next_block() {
    check_group("a_code_span_does_not_run_over_into_the_next_block");
}

// A command after a utility that runs it is a command line of its own: `nohup`, `exec` and `env` take a utility and
// its arguments (POSIX utilities and special built-ins), and a README that starts a long-running engine is likely to
// use one. A word that only has `wf-agents` after a program that does not run it, such as the cargo package, is not
// the program (see a_program_given_by_its_path_is_read).
#[test]
fn a_command_after_a_utility_that_runs_it_is_read() {
    check_group("a_command_after_a_utility_that_runs_it_is_read");
}

// A command in a subshell or a command substitution is a command line of its own, and `)` ends a word: POSIX shell
// command language, token recognition (`(` and `)` are operators), command substitution and grouping commands.
// `(cd DIR && wf-agents status)` is right, and must not be reported for `status)`.
#[test]
fn a_command_in_a_subshell_or_a_substitution_is_read() {
    check_group("a_command_in_a_subshell_or_a_substitution_is_read");
}

// A block quote is read without its marker: the `>` that begins each line is not part of a fence's code or of a code
// span that wraps (CommonMark 0.31, section 5.1: block quotes). A fence in a quote, as in a GitHub alert, is code.
#[test]
fn a_block_quote_is_read_without_its_marker() {
    check_group("a_block_quote_is_read_without_its_marker");
}

// A line indented by four spaces or a tab, after a blank line, is a code block of its own (CommonMark 0.31, section
// 4.4: indented code blocks), and the check reads it as code; one that goes on with a paragraph, or that is the second
// paragraph of a list item with less than four spaces beyond the item's text, is not code.
#[test]
fn a_command_in_an_indented_code_block_is_read() {
    check_group("a_command_in_an_indented_code_block_is_read");
}

// A backslash before a backtick makes it a backtick in the text, not the start of a code span (CommonMark 0.31,
// section 2.4: backslash escapes), and a backslash does not escape in a code span (section 6.1), so one before the
// closing backtick of a span does not hide what comes after it.
#[test]
fn an_escaped_backtick_does_not_open_a_code_span() {
    check_group("an_escaped_backtick_does_not_open_a_code_span");
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
