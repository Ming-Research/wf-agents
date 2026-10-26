//! Tests that look for what the README check lets through: wrong texts it passes, and right ones it reports. Each case
//! is the text that writes every command correctly, with a few lines added.

use super::{leaves, problems, tree};

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

/// Notes in `failures` that `extra` is wrong and the check does not name `word` as what is wrong with it.
fn wrong(failures: &mut Vec<String>, case: &str, extra: &str, word: &str) {
    let got = found(extra);
    if !got.iter().any(|problem| problem.contains(word)) {
        failures.push(format!("{case}: nothing names {word}; found {got:?}"));
    }
}

/// Notes in `failures` that `extra` is right and the check reports it all the same.
fn right(failures: &mut Vec<String>, case: &str, extra: &str) {
    let got = found(extra);
    if !got.is_empty() {
        failures.push(format!("{case}: reported though right: {got:?}"));
    }
}

// A global option before the subcommand does not end the reading of the command line: the subcommand after it is
// looked up, and the options that follow are the options of that command.
#[test]
fn options_before_the_subcommand_do_not_end_the_reading() {
    let mut failures = Vec::new();
    let text = "`wf-agents --data DIR queue add`";
    wrong(&mut failures, "unknown subcommand", text, "add");
    let text = "`wf-agents --data DIR frobnicate`";
    wrong(&mut failures, "unknown command", text, "frobnicate");
    let text = "`wf-agents item --data DIR frobnicate`";
    wrong(&mut failures, "after an option", text, "frobnicate");
    let text = "`wf-agents --data DIR run --twice`";
    wrong(&mut failures, "unknown option", text, "twice");
    let text = "`wf-agents --data DIR run --once`";
    right(&mut failures, "option of the command", text);
    let text = "`wf-agents --data DIR item add --project P --title T`";
    right(&mut failures, "options of a nested one", text);
    let text = "`wf-agents --data DIR queue put 3`";
    right(&mut failures, "right subcommand", text);
    let text = "`wf-agents item --data DIR list`";
    right(&mut failures, "option between the two", text);
    assert!(failures.is_empty(), "{failures:#?}");
}

// A command line wrapped with a backslash is one command line: the words on its next line are read too.
#[test]
fn a_command_line_wrapped_with_a_backslash_is_read_whole() {
    let mut failures = Vec::new();
    let text = "```\nwf-agents item add \\\n  --project P --bogus X\n```";
    wrong(&mut failures, "unknown option", text, "bogus");
    let text = "```\nwf-agents queue \\\n  add 3\n```";
    wrong(&mut failures, "unknown subcommand", text, "add");
    let text = "```\nwf-agents item add \\\n  --project P --title T\n```";
    right(&mut failures, "right options", text);
    let text = "```\nwf-agents queue \\\n  put 3\n```";
    right(&mut failures, "right subcommand", text);
    assert!(failures.is_empty(), "{failures:#?}");
}

// A code span that wraps onto the next line is one span: what stands on its second line is read, and the spans after
// it on that line are still found.
#[test]
fn a_code_span_that_wraps_does_not_hide_the_spans_after_it() {
    let mut failures = Vec::new();
    let text = "see `wf-agents answer 3\n--verdict confirm` and `wf-agents queue add`";
    wrong(&mut failures, "span after it", text, "add");
    let text = "see `wf-agents answer 3\n--bogus X` for more";
    wrong(&mut failures, "option on line two", text, "bogus");
    let text = "see `wf-agents queue\nadd` now";
    wrong(&mut failures, "subcommand on line two", text, "add");
    let text = "see `wf-agents answer 3\n--text T` and `wf-agents queue put 3`";
    right(&mut failures, "right wrapped span", text);
    assert!(failures.is_empty(), "{failures:#?}");
}

// Two commands in one code span are both read: the one after `&&`, `;` or `|` is a command line of its own.
#[test]
fn two_commands_in_one_span_are_both_read() {
    let mut failures = Vec::new();
    let text = "`wf-agents status && wf-agents queue add`";
    wrong(&mut failures, "after &&", text, "add");
    let text = "`wf-agents status; wf-agents queue add`";
    wrong(&mut failures, "after ;", text, "add");
    let text = "`wf-agents status | wf-agents queue add`";
    wrong(&mut failures, "after |", text, "add");
    let text = "`wf-agents status && wf-agents queue put 3`";
    right(&mut failures, "right pair", text);
    let text = "`wf-agents status | grep work`";
    right(&mut failures, "piped to another one", text);
    assert!(failures.is_empty(), "{failures:#?}");
}

// Command lines in the other forms Markdown has for code are read: a block fenced with tildes, and a span in double
// backticks.
#[test]
fn other_forms_of_code_are_read() {
    let mut failures = Vec::new();
    let text = "~~~\nwf-agents queue add\n~~~";
    wrong(&mut failures, "tilde fence", text, "add");
    let text = "``wf-agents queue add``";
    wrong(&mut failures, "double backticks", text, "add");
    let text = "~~~\nwf-agents queue put 3\n~~~";
    right(&mut failures, "right tilde fence", text);
    let text = "``wf-agents queue put 3``";
    right(&mut failures, "right double ticks", text);
    assert!(failures.is_empty(), "{failures:#?}");
}

// Right wording is not reported: the end of the options, a quoted value that holds a word with dashes, and the line
// the command prints for `--version`, which a block shows as the command prints it.
#[test]
fn right_wording_is_not_reported() {
    let mut failures = Vec::new();
    let text = "`wf-agents say 3 -- --stop`";
    right(&mut failures, "end of the options", text);
    let text = "`wf-agents say 3 \"do not use --force\"`";
    right(&mut failures, "dashes in a quote", text);
    let text = "```\n$ wf-agents --version\nwf-agents 0.1.0\n```";
    right(&mut failures, "version output", text);
    let text = "`wf-agents answer 3 --text \"ok\"`";
    right(&mut failures, "plain quoted value", text);
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
