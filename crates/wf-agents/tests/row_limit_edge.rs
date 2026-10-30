// Kept apart from `cli.rs` so that it can be run on the code before the change on its own: the tests there that fail
// before the change stop a run, and this one must be seen to pass.
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

/// A path under the temporary directory that is not there.
fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wf-agents-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// The built command, typed as `line` and given the data directory `data`.
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

/// A data directory as the owner keeps one: a git repository whose committed configuration is valid, with no database.
fn data_directory(name: &str) -> PathBuf {
    let dir = temp(name);
    std::fs::create_dir_all(dir.join("projects")).unwrap();
    std::fs::create_dir_all(dir.join("rules")).unwrap();
    std::fs::write(dir.join("install.toml"), INSTALL).unwrap();
    std::fs::write(dir.join("projects/p.toml"), PROJECT).unwrap();
    std::fs::write(dir.join("rules/ground.md"), "Facts first.").unwrap();
    git(&dir, &["init", "-q"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "configuration"]);
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

/// The lines of the section of `anomalies` under a heading, up to the blank line before the next.
fn section<'a>(text: &'a str, heading: &str) -> Vec<&'a str> {
    text.lines()
        .skip_while(|l| *l != heading)
        .skip(1)
        .take_while(|l| !l.is_empty())
        .collect()
}

// The old behaviour at the edge of every limit: the tables of `anomalies` and of `cost` that hold exactly as many rows
// as they may (ten thousand, and a thousand works paused) print no line for rows left out, and `cost` of one work ends
// with the row of its keep-warm pings, as it did; a work with one cell and no ping is unchanged. Worked out by hand
// from the rows written: work 1 has a round at each of 10,000 cells and a ping, and every work has one round of 2 s.
#[test]
fn tables_that_hold_exactly_the_limit_say_nothing_about_rows_left_out() {
    let dir = data_directory("row-limit-edge");
    engine::database::create(&dir).unwrap();
    fill(
        &dir,
        r#"INSERT OR IGNORE INTO flow_versions (id, digest, compiled) VALUES (1, 'd', '{}');
         WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 10000)
             INSERT INTO works (id, flow_version, graph, at, paused)
             SELECT i, 1, 'engineer', 'I1', CASE WHEN i <= 1000 THEN 'x' END FROM n;
         INSERT INTO rounds (work, n, cell, input, rejected, compactions, config_commit, usage, started_at, ended_at)
             SELECT id, 1, 'I1', '{}', '{"x": 1}', 1, 'c', '{"cost_usd": 0.25}', 0, 2000 FROM works;
         WITH RECURSIVE n(i) AS (SELECT 2 UNION ALL SELECT i + 1 FROM n WHERE i < 10000)
             INSERT INTO rounds (work, n, cell, input, config_commit, started_at, ended_at)
             SELECT 1, i, 'C' || i, '{}', 'c', 0, 1000 FROM n;
         INSERT INTO sessions (id, work, line, cli, cli_session) VALUES (1, 1, 'engineer', 'claude', 's1');
         INSERT INTO pings (session, waited, usage, started_at) VALUES (1, '{}', '{"cost_usd": 0.5}', 70000);"#,
    );
    let text = printed(&dir, "anomalies");
    assert!(!text.contains("left out"), "anomalies");
    let sent_back = section(&text, "Returns sent back:");
    assert_eq!(sent_back.len(), 10_001);
    assert_eq!(sent_back[10_000], "10000\tR10000-1\tI1\t{\"x\":1}");
    let compactions = section(&text, "Compactions:");
    assert_eq!(compactions.len(), 10_001);
    assert_eq!(compactions[10_000], "10000\tR10000-1\tI1\t1");
    let paused = section(&text, "Works paused:");
    assert_eq!(paused.len(), 1_001);
    assert_eq!(paused[1_000], "1000\t\tI1\tx");
    let text = printed(&dir, "cost");
    assert!(!text.contains("left out"), "cost");
    assert_eq!(text.lines().count(), 10_001, "a head and 10,000 works");
    assert_eq!(text.lines().last(), Some("10000\t1\t2\t0.25"));
    let text = printed(&dir, "cost 1");
    assert!(!text.contains("left out"), "cost 1");
    assert_eq!(text.lines().count(), 10_002, "a head, 10,000 cells and the pings");
    assert_eq!(text.lines().last(), Some("keep-warm pings\t1\t\t0.50"));
    assert_eq!(
        printed(&dir, "cost 2"),
        "cell\trounds\tsecs\tcost_usd\nI1\t1\t2\t0.25\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
