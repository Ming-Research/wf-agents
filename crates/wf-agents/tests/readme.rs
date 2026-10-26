//! Tests of the README and of the command's help that read the built command, not its command tree: what the command
//! prints is what a reader sees, and the commands are found from it.

use std::process::Command;

const README: &str = include_str!("../../../README.md");

/// The help the command prints for the command that the words in `path` run.
fn help(path: &[String]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_wf-agents"))
        .args(path)
        .arg("--help")
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{path:?}: {stderr}");
    String::from_utf8(out.stdout).unwrap()
}

/// How far a line is indented.
fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// The lines of the section of a help text that starts at the line `title`, up to the next blank line.
fn section<'a>(help: &'a str, title: &str) -> Vec<&'a str> {
    let after = help.lines().skip_while(|line| *line != title).skip(1);
    after.take_while(|line| !line.trim().is_empty()).collect()
}

/// The words that run each command that has no subcommands, found from the help of each command.
fn commands(path: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
    let text = help(path);
    let mut names = Vec::new();
    for line in section(&text, "Commands:") {
        if indent(line) == 2 {
            let name = line.split_whitespace().next().unwrap();
            if name != "help" {
                names.push(name.to_string());
            }
        }
    }
    if names.is_empty() {
        out.push(path.clone());
    }
    for name in names {
        path.push(name);
        commands(path, out);
        path.pop();
    }
}

/// The text without the notes in square brackets that the help adds, such as `[default: x]` and `[env: X=]`.
fn plain(text: &str) -> String {
    let mut depth = 0;
    let mut kept = String::new();
    for c in text.chars() {
        match c {
            '[' => depth += 1,
            ']' if depth > 0 => depth -= 1,
            _ if depth == 0 => kept.push(c),
            _ => {}
        }
    }
    kept
}

/// The arguments a help text lists with nothing said about them: the entry stands alone, with no text after its name
/// and none on the lines under it.
fn bare(help: &str) -> Vec<String> {
    let mut out = Vec::new();
    for title in ["Arguments:", "Options:"] {
        let lines = section(help, title);
        for (i, line) in lines.iter().enumerate() {
            let entry = line.trim_start();
            if indent(line) > 6 || !entry.starts_with(['-', '<', '[']) {
                continue;
            }
            let tail = entry.split_once("  ").map_or("", |(_, tail)| tail);
            let below = lines.get(i + 1).is_some_and(|next| indent(next) >= 8);
            if plain(tail).trim().is_empty() && !below {
                out.push(entry.to_string());
            }
        }
    }
    out
}

/// Whether `words` stand in `text` as whole words: nothing that continues a word is just before or just after them.
fn written(text: &str, words: &str) -> bool {
    let continues = |c: char| c.is_alphanumeric() || matches!(c, '-' | '_' | '/');
    for (i, _) in text.match_indices(words) {
        let before = text[..i].chars().next_back();
        let after = text[i + words.len()..].chars().next();
        if !before.is_some_and(continues) && !after.is_some_and(continues) {
            return true;
        }
    }
    false
}

// The help of every command describes each of its arguments, as `wf-agents <command> --help` prints it: no entry of
// its lists stands alone.
#[test]
fn the_help_of_every_command_describes_each_of_its_arguments() {
    let mut all = Vec::new();
    commands(&mut Vec::new(), &mut all);
    assert_eq!(all.len(), 20, "{all:?}");
    let mut undescribed = Vec::new();
    for path in &all {
        for entry in bare(&help(path)) {
            undescribed.push(format!("wf-agents {}: {entry}", path.join(" ")));
        }
    }
    assert!(undescribed.is_empty(), "{undescribed:#?}");
}

// The README writes every command that the command lists as the whole words `wf-agents <command>`: `wf-agents card`
// is not found in `wf-agents cards`.
#[test]
fn the_readme_writes_every_command_the_command_lists() {
    let mut all = Vec::new();
    commands(&mut Vec::new(), &mut all);
    assert_eq!(all.len(), 20, "{all:?}");
    let mut missing = Vec::new();
    for path in &all {
        let words = format!("wf-agents {}", path.join(" "));
        if !written(README, &words) {
            missing.push(words);
        }
    }
    assert!(missing.is_empty(), "{missing:#?}");
}

// The command keeps its database where the README says: the first command to open it makes `db/` in the data
// directory, with `wf-agents.sqlite` in it, and the README names both.
#[test]
fn the_database_is_where_the_readme_says() {
    let dir = std::env::temp_dir().join(format!("wf-agents-readme-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = Command::new(env!("CARGO_BIN_EXE_wf-agents"))
        .args(["status", "--data"])
        .arg(&dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let mut made = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        made.push(entry.unwrap().file_name().to_string_lossy().into_owned());
    }
    made.sort();
    let made_file = dir.join("db").join("wf-agents.sqlite").is_file();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(made, ["db"]);
    assert!(made_file, "db/wf-agents.sqlite was not made");
    for named in ["`db/`", "`wf-agents.sqlite`"] {
        assert!(README.contains(named), "the README does not name {named}");
    }
}
