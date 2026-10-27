use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const INSTALL: &str = r#"
[owner]
language = "English"
[items]
title_max = 40
[machines]
workstation = "here"
heavy = "builds"
poll_interval = 60
[machines.hosts.here]
slots = 1
worktrees = "/tmp"
[agents.clis.claude]
kind = "claude"
command = "claude"
[agents.levels.l]
desc = "all work"
cli = "claude"
model = "m"
[gh]
allow = ["pr view"]
[query]
max_rows = 10
timeout_ms = 1000
"#;

const PROJECT: &str = r#"
repo = "o/p"
checkout = "/tmp/p"
design_goal = "simple"
design_paths = ["design/"]
test_paths = ["tests/"]
rules = ["ground"]
[ci]
fast = "ci"
full = ["ci"]
merge_checks = ["check"]
[limits]
fixes = 5
"#;

/// The commands that open the database with nothing checked before, each as the owner types it.
const UNCHECKED: [&str; 15] = [
    "item list",
    "item depend 2 1 --needs x",
    "item cancel 1 --why x",
    "queue list",
    "queue put 1",
    "cards",
    "card 1",
    "answer 1 --text x",
    "say 1 hello",
    "pause 1 --why x",
    "resume 1",
    "status",
    "history 1",
    "counters 1",
    "cost",
];

/// The commands that read the committed configuration before they open the database.
const CHECKED: [&str; 3] = [
    "item add --project p --title t --problem x --why y --level l",
    "anomalies",
    "run --once",
];

/// A path under the temporary directory that is not there.
fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wf-agents-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// The built command, typed as `line` and given the data directory `data`. Git looks no higher than the temporary
/// directory, so a place under it is a repository only if it is one itself.
fn wf(data: &Path, line: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_wf-agents"))
        .args(line.split_whitespace())
        .arg("--data")
        .arg(data)
        .env("GIT_CEILING_DIRECTORIES", std::env::temp_dir())
        .output()
        .unwrap()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", stderr(&out));
}

fn commit_all(dir: &Path) {
    git(dir, &["init", "-q"]);
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "configuration"]);
}

fn write_config(dir: &Path, install: &str) {
    std::fs::create_dir_all(dir.join("projects")).unwrap();
    std::fs::create_dir_all(dir.join("rules")).unwrap();
    std::fs::write(dir.join("install.toml"), install).unwrap();
    std::fs::write(dir.join("projects/p.toml"), PROJECT).unwrap();
    std::fs::write(dir.join("rules/ground.md"), "Facts first.").unwrap();
}

/// A data directory as the owner keeps one: a git repository whose committed configuration is valid, with no database.
fn data_directory(name: &str) -> PathBuf {
    let dir = temp(name);
    write_config(&dir, INSTALL);
    commit_all(&dir);
    dir
}

/// The names directly in a directory, in order.
fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// The command fails for want of a database, saying where it looked and which command makes one.
fn refused(dir: &Path, line: &str) {
    let out = wf(dir, line);
    let err = stderr(&out);
    let path = config::database(dir).display().to_string();
    assert!(!out.status.success(), "`{line}` succeeded");
    assert!(err.contains(&path), "`{line}`: {err}");
    assert!(err.contains("wf-agents init"), "`{line}`: {err}");
}

// The built command runs and reports the workspace version: the binary builds, starts and parses its arguments.
#[test]
fn version_names_the_command_and_the_workspace_version() {
    let out = Command::new(env!("CARGO_BIN_EXE_wf-agents"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(out.status.success(), "exit status {}", out.status);
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        format!("wf-agents {}\n", env!("CARGO_PKG_VERSION"))
    );
}

// The repository's flow passes its check; a file that is not a flow fails it, saying why.
#[test]
fn flow_check_passes_the_repository_flow_and_fails_a_broken_one() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let out = Command::new(env!("CARGO_BIN_EXE_wf-agents"))
        .args(["flow", "check", &format!("{root}/flow/flow.json")])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.starts_with("engineer: 33 cells, 205 rows"), "{stdout}");
    let out = Command::new(env!("CARGO_BIN_EXE_wf-agents"))
        .args(["flow", "check", &format!("{root}/db/schema.sql")])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("flow.json"));
}

// A configuration that fits the flow passes its check; a limit on a counter the flow does not have fails it.
#[test]
fn config_check_reads_the_data_directory_against_the_flow() {
    let dir = temp("config");
    write_config(&dir, INSTALL);
    let check = || {
        Command::new(env!("CARGO_BIN_EXE_wf-agents"))
            .args(["config", "check", "--data"])
            .arg(&dir)
            .output()
            .unwrap()
    };
    let out = check();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("1 projects (p), 1 rules"));
    std::fs::write(
        dir.join("projects/p.toml"),
        PROJECT.replace("fixes = 5", "nonsense = 5"),
    )
    .unwrap();
    let out = check();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("the flow has no counter nonsense"));
    let _ = std::fs::remove_dir_all(&dir);
}

// A directory that has no database is not made into one: the commands that open it with nothing checked fail,
// saying where they looked and which command makes one, and leave the directory as it was.
#[test]
fn commands_fail_on_a_directory_without_a_database_and_leave_it_alone() {
    let dir = temp("no-database");
    std::fs::create_dir_all(&dir).unwrap();
    for line in UNCHECKED {
        refused(&dir, line);
    }
    assert!(entries(&dir).is_empty(), "{:?}", entries(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}

// The commands that read the committed configuration first do not make a database either: in a data directory that
// is valid and has none, they fail the same way and leave it as it was.
#[test]
fn commands_that_read_the_configuration_fail_without_a_database_too() {
    let dir = data_directory("checked-no-database");
    let before = entries(&dir);
    for line in CHECKED {
        refused(&dir, line);
    }
    assert_eq!(entries(&dir), before);
    let _ = std::fs::remove_dir_all(&dir);
}

// A path that is not there stays not there: no command makes the data directory, or anything in it.
#[test]
fn no_command_makes_a_data_directory_that_is_not_there() {
    let dir = temp("typo");
    for line in UNCHECKED.into_iter().chain(CHECKED) {
        let out = wf(&dir, line);
        assert!(!out.status.success(), "`{line}` succeeded");
        assert!(!dir.exists(), "`{line}` made {}", dir.display());
    }
}

// `init` makes the database where the other commands look for it, once: a second run is refused, and what was
// written in between stays.
#[test]
fn init_makes_the_database_once() {
    let dir = data_directory("init");
    let path = config::database(&dir).display().to_string();
    let add = "item add --project p --title kept --problem x --why y --level l";
    let out = wf(&dir, "init");
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(config::database(&dir).is_file());
    let out = wf(&dir, "item list");
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "(none)\n");
    let out = wf(&dir, add);
    assert!(out.status.success(), "{}", stderr(&out));
    let again = wf(&dir, "init");
    assert!(!again.status.success());
    assert!(stderr(&again).contains(&path));
    let out = wf(&dir, "item list");
    assert!(String::from_utf8(out.stdout).unwrap().contains("kept"));
    let _ = std::fs::remove_dir_all(&dir);
}

// `init` makes nothing in a place that is not a data directory: a path that is not there, a directory that is not
// a git repository, a repository with no commit, and one whose committed configuration is not valid.
#[test]
fn init_refuses_a_place_that_is_not_a_data_directory() {
    let absent = temp("init-absent");
    let plain = temp("init-plain");
    std::fs::create_dir_all(&plain).unwrap();
    let uncommitted = temp("init-uncommitted");
    write_config(&uncommitted, INSTALL);
    git(&uncommitted, &["init", "-q"]);
    let broken = temp("init-broken");
    write_config(&broken, "nonsense = 1\n");
    commit_all(&broken);
    for dir in [&plain, &uncommitted, &broken] {
        let before = entries(dir);
        let out = wf(dir, "init");
        assert!(!out.status.success(), "init succeeded in {}", dir.display());
        assert_eq!(entries(dir), before, "{}", dir.display());
    }
    let out = wf(&absent, "init");
    assert!(!out.status.success());
    assert!(!absent.exists());
    for dir in [&plain, &uncommitted, &broken] {
        let _ = std::fs::remove_dir_all(dir);
    }
}
