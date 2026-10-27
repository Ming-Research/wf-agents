//! The check of the README against the commands. It reads command lines by one convention only:
//! - a command line is a code span that stays on one line (a single backtick on each side), or a line of a fenced
//!   block (``` or ~~~), whose first word, after an optional `$ `, is `wf-agents`;
//! - its words are separated by spaces; it has no quote, backslash, `$(`, `&&`, `||`, `|`, `;` or `&`; an option's
//!   value is one word, a placeholder such as `DIR`, or `...`.
//!
//! Every other place where `wf-agents` starts a command is reported, with this convention. What the check does not
//! see is a misspelled subcommand in a form it does not read, when that form is not a command line either; that
//! passes. A form the README needs outside the convention is a change to the README, not a new rule here.

use std::collections::BTreeSet;

use clap::{Arg, ArgAction, Command, CommandFactory};

use crate::Cli;

const README: &str = include_str!("../../../../README.md");

const NAME: &str = "wf-agents";

/// What a report says of a place where the program starts a command in a form the check does not read.
pub(super) const UNREAD: &str = "written in a form the check does not read";

/// The convention, as every report of such a place gives it.
pub(super) const CONVENTION: &str = "a command line is a code span on one line, or a line of a fenced \
    block, that begins with wf-agents (after an optional `$ `) and has no quote, backslash, `$(`, `&&`, \
    `||`, `|`, `;` or `&`";

// What the README's section on the data directory lists. This list is a copy: it keeps the README from losing an
// entry, and does not notice a path the engine starts to write.
pub(super) const DATA_DIRECTORY: [&str; 11] = [
    "install.toml",
    "shared/",
    "projects/",
    "rules/",
    "agents/",
    "db/",
    "logs/",
    "tool-output/",
    "notifications.log",
    "worktrees/",
    "stop",
];

/// Where a piece of the README stands: in code, where a command may be written, or in text.
#[derive(Clone, Copy, PartialEq)]
enum Place {
    Code,
    Text,
}

/// The command tree as clap builds it to parse: with its own `--help` and `help`, and with the global `--data`
/// passed on to every subcommand.
pub(super) fn tree() -> Command {
    let mut cmd = Cli::command();
    cmd.build();
    cmd
}

/// Every command that has no subcommands, with the words that run it, such as `wf-agents item add`.
pub(super) fn leaves<'a>(cmd: &'a Command, words: &str, out: &mut Vec<(String, &'a Command)>) {
    for sub in cmd.get_subcommands().filter(|s| s.get_name() != "help") {
        let path = format!("{words} {}", sub.get_name());
        if sub.has_subcommands() {
            leaves(sub, &path, out);
        } else {
            out.push((path, sub));
        }
    }
}

/// The name of every subcommand of the tree, at every depth, without `help`.
fn subcommand_names(cmd: &Command, out: &mut BTreeSet<String>) {
    for sub in cmd.get_subcommands().filter(|s| s.get_name() != "help") {
        out.insert(sub.get_name().to_string());
        subcommand_names(sub, out);
    }
}

/// The README as pieces: the code spans that stay on one line, the lines of fenced blocks, and what lies between
/// the spans of each line. A backtick with no partner on its line opens no span, so what follows it is text.
fn pieces(text: &str) -> Vec<(Place, &str)> {
    let mut out = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        let first_char = trimmed.chars().next();
        let marker = first_char.filter(|c| matches!(*c, '`' | '~'));
        let run = marker.map_or(0, |m| trimmed.chars().take_while(|c| *c == m).count());
        // The info string of a block fenced with backticks has no backtick in it: ```code``` is a code span.
        let opens = run >= 3 && (marker == Some('~') || !trimmed[run..].contains('`'));
        match fence {
            None if opens => fence = marker.map(|m| (m, run)),
            None => {
                let parts: Vec<&str> = line.split('`').collect();
                for (i, part) in parts.iter().enumerate() {
                    let inside = i % 2 == 1 && i + 1 < parts.len();
                    let place = if inside { Place::Code } else { Place::Text };
                    out.push((place, *part));
                }
            }
            Some((m, ticks)) if marker == Some(m) && run >= ticks && trimmed.len() == run => fence = None,
            Some(_) => out.push((Place::Code, trimmed)),
        }
    }
    out
}

/// Where the program word begins in a piece of code that is a command line: its first word, after an optional `$ `.
fn command_start(piece: &str) -> Option<usize> {
    let text = piece.trim_start();
    let text = text.strip_prefix("$ ").map_or(text, str::trim_start);
    let word = text.split_whitespace().next()?;
    if word == NAME {
        Some(piece.len() - text.len())
    } else {
        None
    }
}

/// Whether a command line has a quote, a backslash or a shell operator: a form the check does not read.
fn outside_convention(line: &str) -> bool {
    line.contains(['"', '\'', '\\', '|', ';', '&']) || line.contains("$(")
}

/// Where the program's name stands as a word of its own in a piece, not in `crates/wf-agents`, `wf-agents.sqlite` or
/// `wf-agents-1`.
fn occurrences(piece: &str) -> Vec<usize> {
    let mut out = Vec::new();
    for (at, _) in piece.match_indices(NAME) {
        let before = piece[..at].chars().next_back();
        let mut after = piece[at + NAME.len()..].chars();
        let next = after.next();
        let joined_before = before.is_some_and(|c| c.is_alphanumeric() || matches!(c, '/' | '-' | '_' | '.'));
        let joined_after = next.is_some_and(|c| c.is_alphanumeric() || matches!(c, '/' | '-' | '_'))
            || (next == Some('.') && after.next().is_some_and(char::is_alphanumeric));
        if !joined_before && !joined_after {
            out.push(at);
        }
    }
    out
}

/// The report of a piece that starts a command in a form the check does not read.
fn unread(piece: &str) -> String {
    format!("{}: {UNREAD}; {CONVENTION}", piece.trim())
}

/// Whether the option is followed by a value, which is not a subcommand or an argument.
fn takes_value(arg: &Arg) -> bool {
    matches!(arg.get_action(), ArgAction::Set | ArgAction::Append)
}

/// What a command line comes to: the words that name its command, and what is wrong with it. The words that name
/// the command are the subcommands written before the first option. Subcommands are looked up all along the line,
/// as clap does, with the options and their values left out of the way; the words after the command are its
/// arguments, and so are those after `--`. Values, positional arguments and short options are not checked.
fn read_line(root: &Command, words: &[&str]) -> (String, Vec<String>) {
    let line = words.join(" ");
    let mut cmd = root;
    let mut named = String::from(NAME);
    let mut wrong = Vec::new();
    let mut options = false;
    let mut rest = words.iter().copied().skip(1);
    while let Some(word) = rest.next() {
        if word == "--" {
            break;
        } else if let Some(long) = word.strip_prefix("--") {
            options = true;
            let (name, valued) = match long.split_once('=') {
                Some((name, _)) => (name, true),
                None => (long, false),
            };
            match cmd.get_arguments().find(|a| a.get_long() == Some(name)) {
                None => wrong.push(format!("{line}: no option --{name}")),
                Some(arg) if takes_value(arg) && !valued => {
                    rest.next();
                }
                Some(_) => {}
            }
        } else if word.starts_with('-') && word.len() > 1 {
            options = true;
        } else if !cmd.has_subcommands() {
            // A positional argument: not checked.
        } else if word.starts_with('<') {
            // A placeholder for the command: which one is not known, so nothing after it is checked.
            break;
        } else if let Some(sub) = cmd.find_subcommand(word) {
            cmd = sub;
            if !options {
                named = format!("{named} {word}");
            }
        } else {
            wrong.push(format!("{line}: no subcommand {word}"));
            break;
        }
    }
    (named, wrong)
}

/// What is wrong with a text against the commands: a command it never writes, a subcommand or a long option that one
/// of its command lines names and the command does not have, and a place where the program starts a command in a form
/// the check does not read. A command is written when a command line begins with the whole words that run it, such
/// as `wf-agents queue put`. In code, the program's name followed by any word starts a command; in text, only when
/// that word names a subcommand or begins with a dash, so that prose such as "wf-agents runs agents" is not reported.
pub(super) fn problems(text: &str, root: &Command) -> Vec<String> {
    let mut names = BTreeSet::new();
    subcommand_names(root, &mut names);
    let mut written = BTreeSet::new();
    let mut out = Vec::new();
    for (place, piece) in pieces(text) {
        let first = command_start(piece).filter(|_| place == Place::Code);
        if let Some(at) = first {
            if outside_convention(piece) {
                out.push(unread(piece));
                continue;
            }
            let words: Vec<&str> = piece[at..].split_whitespace().collect();
            let (named, wrong) = read_line(root, &words);
            written.insert(named);
            out.extend(wrong);
        }
        let found = occurrences(piece);
        for at in found.into_iter().filter(|at| Some(*at) != first) {
            let next = piece[at + NAME.len()..].split_whitespace().next();
            let reported = match (place, next) {
                (_, None) => false,
                (Place::Code, Some(_)) => true,
                (Place::Text, Some(word)) => {
                    word.starts_with('-') || names.contains(word.trim_end_matches(|c: char| !c.is_alphanumeric()))
                }
            };
            if reported {
                out.push(unread(piece));
                break;
            }
        }
    }
    let mut all = Vec::new();
    leaves(root, NAME, &mut all);
    for (path, _) in all {
        if !written.contains(&path) {
            out.push(format!("{path}: never written"));
        }
    }
    out
}

/// The text that writes `lines` as command lines: a code span for each, or the lines of one fenced block, with a shell
/// prompt before each or without.
fn as_text(lines: &[String], fenced: bool, prompt: bool) -> String {
    let prefix = if prompt { "$ " } else { "" };
    let quote = if fenced { "" } else { "`" };
    let mut shown = Vec::new();
    for line in lines {
        shown.push(format!("{quote}{prefix}{line}{quote}"));
    }
    if fenced {
        format!("```\n{}\n```", shown.join("\n"))
    } else {
        shown.join("\n")
    }
}

// The README writes every command, and names only subcommands and long options that exist. One command from each
// depth is named, so that a walk which finds no commands cannot pass.
#[test]
fn the_readme_matches_the_commands() {
    let root = tree();
    let mut all = Vec::new();
    leaves(&root, NAME, &mut all);
    let paths: Vec<&str> = all.iter().map(|(path, _)| path.as_str()).collect();
    for sample in ["wf-agents run", "wf-agents flow check", "wf-agents item add"] {
        assert!(paths.contains(&sample), "{sample} is not among {paths:?}");
    }
    let wrong = problems(README, &root);
    assert!(wrong.is_empty(), "{wrong:#?}");
}

// Each way a README can be wrong is reported once, as what it is; a README that is right reports nothing, whether it
// writes its command lines as code spans or in a fenced block, with a shell prompt before them or without.
#[test]
fn a_wrong_readme_is_reported_as_what_is_wrong_with_it() {
    let root = tree();
    let mut all = Vec::new();
    leaves(&root, NAME, &mut all);
    let paths: Vec<String> = all.iter().map(|(path, _)| path.clone()).collect();
    for (fenced, prompt) in [(false, false), (false, true), (true, false), (true, true)] {
        let right = problems(&as_text(&paths, fenced, prompt), &root);
        assert!(right.is_empty(), "fenced {fenced}, prompt {prompt}: {right:#?}");
    }
    // The text that writes every command, less the commands in `without` and with the command lines in `extra`.
    let text = |without: &[&str], extra: &[&str]| {
        let mut lines: Vec<String> = Vec::new();
        for path in &paths {
            if !without.contains(&path.as_str()) {
                lines.push(path.clone());
            }
        }
        lines.extend(extra.iter().map(|line| line.to_string()));
        as_text(&lines, false, false)
    };
    let missing = text(&["wf-agents queue put"], &[]);
    let expected = ["wf-agents queue put: never written"];
    assert_eq!(problems(&missing, &root), expected);
    // `wf-agents asks` stays in the text, and is not `wf-agents ask`.
    let confused = text(&["wf-agents ask"], &[]);
    let expected = ["wf-agents ask: never written"];
    assert_eq!(problems(&confused, &root), expected);
    let no_such_subcommand = text(&[], &["wf-agents queue add"]);
    let expected = ["wf-agents queue add: no subcommand add"];
    assert_eq!(problems(&no_such_subcommand, &root), expected);
    let no_such_option = text(&[], &["wf-agents run --twice"]);
    let expected = ["wf-agents run --twice: no option --twice"];
    assert_eq!(problems(&no_such_option, &root), expected);
}

// Every argument of every command has a help text, so `--help` describes all of them.
#[test]
fn every_argument_has_a_help_text() {
    let root = tree();
    let mut all = Vec::new();
    leaves(&root, NAME, &mut all);
    let seen: usize = all.iter().map(|(_, cmd)| cmd.get_arguments().count()).sum();
    assert!(seen > 0, "no arguments were found");
    let mut bare = Vec::new();
    for (path, cmd) in &all {
        for arg in cmd.get_arguments() {
            if arg.get_help().is_none() {
                bare.push(format!("{path}: {}", arg.get_id()));
            }
        }
    }
    assert!(bare.is_empty(), "{bare:#?}");
}

/// The lines of the section of a README that begins at the heading `title`, up to the next heading.
fn section<'a>(text: &'a str, title: &str) -> Vec<&'a str> {
    let after = text.lines().skip_while(|line| *line != title).skip(1);
    after.take_while(|line| !line.starts_with('#')).collect()
}

/// The entries of the data directory that the README's section on it does not list as a bullet that begins with the
/// entry in code font. An entry that the README names somewhere else does not count.
pub(super) fn unlisted(text: &str) -> Vec<&'static str> {
    let lines = section(text, "### The data directory");
    DATA_DIRECTORY
        .into_iter()
        .filter(|entry| {
            let bullet = format!("- `{entry}`");
            !lines.iter().any(|line| line.starts_with(&bullet))
        })
        .collect()
}

// The README's section on the data directory lists each of its entries as a bullet. An entry is also named in other
// places of the README, so taking its bullet away has to be reported with the entry's name.
#[test]
fn the_readme_names_what_is_in_the_data_directory() {
    let missing = unlisted(README);
    assert!(
        missing.is_empty(),
        "{missing:?} are not listed in the section on the data directory"
    );
    for entry in DATA_DIRECTORY {
        let bullet = format!("- `{entry}`");
        let without: Vec<&str> = README.lines().filter(|line| !line.starts_with(&bullet)).collect();
        assert_eq!(unlisted(&without.join("\n")), [entry], "{entry} without its bullet");
    }
}

// A place where `wf-agents` starts a command, outside the command lines the check reads, is reported with the
// convention; a place where `wf-agents` is a path, a file name, a package or a word of prose is not.
#[test]
fn a_command_outside_the_convention_is_reported() {
    let root = tree();
    let mut all = Vec::new();
    leaves(&root, NAME, &mut all);
    let paths: Vec<String> = all.iter().map(|(path, _)| path.clone()).collect();
    let base = as_text(&paths, false, false);
    let reported = [
        "`nohup wf-agents run --twice`",
        "`wf-agents status && wf-agents queue add`",
        "Run it:\n\n    wf-agents queue add",
        "> wf-agents queue add",
        "<code>wf-agents queue add</code>",
        "see `wf-agents queue\nadd` now",
    ];
    for extra in reported {
        let got = problems(&format!("{base}\n{extra}"), &root);
        assert_eq!(got.len(), 1, "{extra:?}: {got:#?}");
        assert!(
            got[0].contains(UNREAD) && got[0].contains(CONVENTION),
            "{extra:?}: {got:#?}"
        );
    }
    let quiet = [
        "`cargo build -p wf-agents`",
        "`crates/wf-agents`",
        "`wf-agents.sqlite`",
        "wf-agents runs coding agents",
    ];
    for extra in quiet {
        let got = problems(&format!("{base}\n{extra}"), &root);
        assert!(got.is_empty(), "{extra:?}: {got:#?}");
    }
}
