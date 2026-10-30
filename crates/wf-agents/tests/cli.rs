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
const UNCHECKED: [&str; 18] = [
    "item list",
    "item depend 2 1 --needs x --kind blocking",
    "item cancel 1 --why x",
    "queue list",
    "queue put 1",
    "asks",
    "ask 1",
    "answer 1 --text x",
    "design-cards",
    "design-card 1",
    "rule 1 --confirm all",
    "say 1 hello",
    "pause 1 --why x",
    "resume 1",
    "status",
    "history 1",
    "counters 1",
    "cost",
];

/// The commands that read the committed configuration before they open the database.
const CHECKED: [&str; 4] = [
    "item add --project p --title t --problem x --why y --level l",
    "anomalies",
    "message 1",
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

/// The command ran and failed, which is exit status 1; the argument parser refuses what it does not know with 2, so
/// a mistyped command line or a command that does not exist is not taken for a failure.
fn failed(out: &Output) -> bool {
    out.status.code() == Some(1)
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
    assert!(failed(&out), "`{line}`: {:?}", out.status);
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
    assert!(stdout.starts_with("engineer: 36 cells, 221 rows"), "{stdout}");
    // It says where each line's sessions start anew and how large the forms its returns share are.
    assert!(
        stdout.contains("engineer: format:common ")
            && stdout.contains("engineer ")
            && stdout.contains("new sessions at R1, I1, T6"),
        "{stdout}"
    );
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
        assert!(failed(&out), "`{line}`: {:?}", out.status);
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
    assert!(failed(&again), "{:?}", again.status);
    assert!(stderr(&again).contains(&path));
    let out = wf(&dir, "item list");
    assert!(String::from_utf8(out.stdout).unwrap().contains("kept"));
    let _ = std::fs::remove_dir_all(&dir);
}

// `init` makes nothing in a place that is not a data directory: a path that is not there, a directory that is not
// a git repository, a repository with no commit, one whose committed configuration is not valid, and one whose
// configuration reads but does not fit the flow, which the engine would refuse to start on.
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
    let off_flow = temp("init-off-flow");
    write_config(&off_flow, INSTALL);
    std::fs::write(
        off_flow.join("projects/p.toml"),
        PROJECT.replace("fixes = 5", "nonsense = 5"),
    )
    .unwrap();
    commit_all(&off_flow);
    for dir in [&plain, &uncommitted, &broken] {
        let before = entries(dir);
        let out = wf(dir, "init");
        assert!(failed(&out), "init in {}: {:?}", dir.display(), out.status);
        assert_eq!(entries(dir), before, "{}", dir.display());
    }
    // The one place `init` refuses by the flow check, which costs a compile of the flow: run once, and read both the
    // refusal and what it says from that run.
    let before = entries(&off_flow);
    let out = wf(&off_flow, "init");
    assert!(failed(&out), "init in {}: {:?}", off_flow.display(), out.status);
    assert!(
        stderr(&out).contains("the flow has no counter nonsense"),
        "{}",
        stderr(&out)
    );
    assert_eq!(entries(&off_flow), before);
    let out = wf(&absent, "init");
    assert!(failed(&out), "init in {}: {:?}", absent.display(), out.status);
    assert!(!absent.exists());
    for dir in [&plain, &uncommitted, &broken, &off_flow] {
        let _ = std::fs::remove_dir_all(dir);
    }
}

// The owner's cancel ends the item's work as every work ends: its session ends, and its worktree's removal and its
// pull request's closing are queued, saying why.
#[test]
fn the_owner_s_cancel_ends_the_work_as_every_work_ends() {
    let dir = data_directory("cancel");
    for line in [
        "init",
        "item add --project p --title kept --problem x --why y --level l",
    ] {
        let out = wf(&dir, line);
        assert!(out.status.success(), "`{line}`: {}", stderr(&out));
    }
    let mut base = db::Db::open(&config::database(&dir)).unwrap();
    base.write(db::Actor::Engine, |tx| {
        tx.execute(
            "INSERT OR IGNORE INTO flow_versions (id, digest, compiled) VALUES (1, 'd', '{}')",
            [],
        )?;
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, worktree, pr) VALUES (1, 1, 'engineer', 'I1', '/w/1', 5)",
            [],
        )?;
        tx.execute(
            "INSERT INTO sessions (work, line, cli, cli_session) VALUES (1, 'engineer', 'claude', 's1')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    drop(base);
    let out = wf(&dir, "item cancel 1 --why duplicate");
    assert!(out.status.success(), "{}", stderr(&out));
    let base = db::Db::open(&config::database(&dir)).unwrap();
    let going: i64 = base
        .conn()
        .query_row("SELECT count(*) FROM sessions WHERE ended IS NULL", [], |r| r.get(0))
        .unwrap();
    assert_eq!(going, 0);
    let mut st = base
        .conn()
        .prepare("SELECT kind, json_extract(args, '$.comment') FROM outbox ORDER BY id")
        .unwrap();
    let queued: Vec<(String, Option<String>)> = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    assert_eq!(
        queued,
        [
            ("remove_worktree".to_string(), None),
            (
                "close_pr".to_string(),
                Some("Its work ended without merging: item cancelled: duplicate.".to_string())
            )
        ]
    );
    drop(st);
    drop(base);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A data directory with a database made by `init`, which holds nothing yet.
fn initialised(name: &str) -> PathBuf {
    let dir = data_directory(name);
    let out = wf(&dir, "init");
    assert!(out.status.success(), "`init`: {}", stderr(&out));
    dir
}

/// Writes rows into the database of a data directory, as the engine writes them: the SQL is a batch of statements.
fn fill(dir: &Path, rows: &str) {
    let mut base = db::Db::open(&config::database(dir)).unwrap();
    base.write(db::Actor::Engine, |tx| {
        tx.execute_batch(rows)?;
        Ok(())
    })
    .unwrap();
}

/// What the command prints, which it does with success.
fn printed(dir: &Path, line: &str) -> String {
    let out = wf(dir, line);
    assert!(out.status.success(), "`{line}`: {}", stderr(&out));
    String::from_utf8(out.stdout).unwrap()
}

/// Two works with rounds, in milliseconds and dollars: work 1 ran from 0 to 100 s in two rounds, the second with no
/// usage reported, and its session was pinged once for 0.5; work 2 ran one round of 20 s for 0.5.
const TWO_WORKS: &str = r#"
    INSERT OR IGNORE INTO flow_versions (id, digest, compiled) VALUES (1, 'd', '{}');
    INSERT INTO works (id, flow_version, graph, at) VALUES (1, 1, 'engineer', 'I1'), (2, 1, 'engineer', 'I1');
    INSERT INTO sessions (id, work, line, cli, cli_session) VALUES (1, 1, 'engineer', 'claude', 's1');
    INSERT INTO rounds (work, n, cell, input, config_commit, usage, started_at, ended_at) VALUES
        (1, 1, 'D1', '{}', 'c', '{"cost_usd": 1.0}', 0, 40000),
        (1, 2, 'I1', '{}', 'c', NULL, 60000, 100000),
        (2, 1, 'I1', '{}', 'c', '{"cost_usd": 0.5}', 5000, 25000);
    INSERT INTO pings (session, waited, usage, started_at) VALUES (1, '{}', '{"cost_usd": 0.5}', 70000);
"#;

// The table of every work ends with a row of totals: the rounds, the seconds and the cost added over the works, in
// the columns and the writing the works' rows have. The expected text is worked out by hand from `TWO_WORKS`: 2 + 1
// rounds, 100 + 20 seconds, and 1.0 + 0.5 of work 1 (the ping's included) and 0.5 of work 2.
#[test]
fn cost_ends_with_a_total_row() {
    let dir = initialised("cost-total");
    fill(&dir, TWO_WORKS);
    assert_eq!(
        printed(&dir, "cost"),
        "work\trounds\tsecs\tcost_usd\n1\t2\t100\t1.5\n2\t1\t20\t0.5\ntotal\t3\t120\t2.0\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// The total adds every work, not the rows the table has room for: with one more work than the table lists, it still
// comes last and counts the work left out. Each work is one round of 2 s for 0.25, a cost every sum of is exact.
#[test]
fn cost_total_adds_the_works_the_table_leaves_out() {
    let dir = initialised("cost-total-all");
    fill(
        &dir,
        r#"
        INSERT OR IGNORE INTO flow_versions (id, digest, compiled) VALUES (1, 'd', '{}');
        WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 10001)
            INSERT INTO works (id, flow_version, graph, at) SELECT i, 1, 'engineer', 'I1' FROM n;
        INSERT INTO rounds (work, n, input, config_commit, usage, started_at, ended_at)
            SELECT id, 1, '{}', 'c', '{"cost_usd": 0.25}', 0, 2000 FROM works;
        "#,
    );
    let text = printed(&dir, "cost");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 10_002, "a head, 10,000 works and the total");
    assert_eq!(lines[0], "work\trounds\tsecs\tcost_usd");
    assert_eq!(lines[1], "1\t1\t2\t0.25");
    assert_eq!(lines[10_000], "10000\t1\t2\t0.25");
    assert_eq!(lines[10_001], "total\t10001\t20002\t2500.25");
    let _ = std::fs::remove_dir_all(&dir);
}

// The cost total is the sum of the costs before they are rounded, rounded once: three works of 0.004 each, one of
// them 0.002 in a round and 0.002 in a ping, show 0.0 in their rows, and the total is 0.01, not 0.0.
#[test]
fn cost_total_rounds_the_sum_once() {
    let dir = initialised("cost-total-rounding");
    fill(
        &dir,
        r#"
        INSERT OR IGNORE INTO flow_versions (id, digest, compiled) VALUES (1, 'd', '{}');
        INSERT INTO works (id, flow_version, graph, at) VALUES
            (1, 1, 'engineer', 'I1'), (2, 1, 'engineer', 'I1'), (3, 1, 'engineer', 'I1');
        INSERT INTO sessions (id, work, line, cli, cli_session) VALUES (1, 1, 'engineer', 'claude', 's1');
        INSERT INTO rounds (work, n, input, config_commit, usage, started_at, ended_at) VALUES
            (1, 1, '{}', 'c', '{"cost_usd": 0.002}', 0, 1000),
            (2, 1, '{}', 'c', '{"cost_usd": 0.004}', 0, 1000),
            (3, 1, '{}', 'c', '{"cost_usd": 0.004}', 0, 1000);
        INSERT INTO pings (session, waited, usage, started_at) VALUES (1, '{}', '{"cost_usd": 0.002}', 2000);
        "#,
    );
    assert_eq!(
        printed(&dir, "cost"),
        "work\trounds\tsecs\tcost_usd\n1\t1\t1\t0.0\n2\t1\t1\t0.0\n3\t1\t1\t0.0\ntotal\t3\t3\t0.01\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// What `cost` printed before the total row stays: with no round there is no table and no total, and one work's cost
// by cell is as it was, its keep-warm pings in a row of their own only where it has pings. The expected text is
// worked out by hand from `TWO_WORKS`.
#[test]
fn cost_prints_as_before_where_there_is_no_total() {
    let dir = initialised("cost-before");
    assert_eq!(printed(&dir, "cost"), "(none)\n");
    fill(&dir, TWO_WORKS);
    assert_eq!(
        printed(&dir, "cost 1"),
        "cell\trounds\tsecs\tcost_usd\nD1\t1\t40\t1.0\nI1\t1\t40\t0.0\nkeep-warm pings\t1\t\t0.50\n"
    );
    assert_eq!(
        printed(&dir, "cost 2"),
        "cell\trounds\tsecs\tcost_usd\nI1\t1\t20\t0.5\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
