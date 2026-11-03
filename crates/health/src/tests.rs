//! What reading the health numbers costs as the history grows: over a history of 100,000 rounds and 200,000 changes,
//! every measure reads the last week at a step of a minute, and SQLite reads none of the large tables whole.

use std::time::{Duration, Instant};

use db::rusqlite::{Connection, params_from_iter};
use db::{Actor, Db};

use crate::{MINUTE, MainLog, Reader, Scope, measures, series_read};

/// A day, in milliseconds.
const DAY: i64 = 86_400_000;
/// When the seeded history starts: a hundred days before its end.
const START: i64 = 1_700_000_040_000;
const END: i64 = START + 100 * DAY;

/// A history of a hundred days: 2,000 items of project p, each with a work and its session; 50 rounds of each work,
/// 100,000 in all, spread evenly, half of them an agent's with its usage and cost, one in fifty a merge and one in fifty
/// a check of main; a run of CI for one round in five; 200,000 changes to the works and the items besides those the
/// items made, one in a hundred pausing a work or letting it go on; a blocker and the owner's words handled for one
/// work in two; and the engine's samples of the last week.
fn seeded() -> Db {
    let mut db = Db::in_memory().unwrap();
    db.write(Actor::Engine, |tx| {
        tx.execute_batch(&format!(
            "INSERT INTO flow_versions (id, digest, compiled) VALUES (1, 'v', '{{}}');
             WITH RECURSIVE n(k) AS (SELECT 1 UNION ALL SELECT k + 1 FROM n WHERE k < 2000)
             INSERT INTO items (id, project, title, problem, why, deliver, level)
               SELECT k, 'p', 't', 'p', 'w', 'change', 'l' FROM n;
             UPDATE changes SET at = {START} + json_extract(row_key, '$.id') * 4320000;
             WITH RECURSIVE n(k) AS (SELECT 1 UNION ALL SELECT k + 1 FROM n WHERE k < 2000)
             INSERT INTO works (id, item, flow_version, graph, at) SELECT k, k, 1, 'engineer', 'I1' FROM n;
             WITH RECURSIVE n(k) AS (SELECT 1 UNION ALL SELECT k + 1 FROM n WHERE k < 2000)
             INSERT INTO sessions (id, work, line, cli, cli_session) SELECT k, k, 'engineer', 'claude', 's' FROM n;"
        ))?;
        Ok(())
    })
    .unwrap();
    // Rounds, tool runs, changes and samples are history, which no trigger tracks.
    db.conn()
        .execute_batch(&format!(
            "WITH RECURSIVE n(k) AS (SELECT 0 UNION ALL SELECT k + 1 FROM n WHERE k < 99999)
             INSERT INTO rounds (work, n, cell, session, input, output, transition, rejected, config_commit, usage,
                                 started_at, ended_at)
               SELECT k % 2000 + 1, k / 2000 + 1,
                 CASE WHEN k % 50 = 7 THEN 'M2' WHEN k % 50 = 8 THEN 'L1' WHEN k % 2 = 0 THEN 'I1' ELSE 'X1' END,
                 CASE WHEN k % 2 = 0 THEN k % 2000 + 1 END, '{{}}',
                 CASE WHEN k % 50 = 8 THEN '{{\"main\":{{\"kind\":\"broken_by_merge\"}}}}' END,
                 CASE WHEN k % 50 = 7 THEN 'merged' WHEN k % 10 = 4 THEN NULL ELSE 'next' END,
                 CASE WHEN k % 10 = 4 THEN '{{\"check\":\"wrong\"}}' END, 'c',
                 CASE WHEN k % 2 = 0 THEN '{{\"cost_usd\":0.1,\"usage\":{{\"iterations\":[{{\"input_tokens\":' || k
                   || ',\"cache_read_input_tokens\":1000,\"cache_creation_input_tokens\":10}}]}}}}' END,
                 {START} + k * 86400, {START} + k * 86400 + 60000
               FROM n;
             WITH RECURSIVE n(k) AS (SELECT 0 UNION ALL SELECT k + 5 FROM n WHERE k < 99995)
             INSERT INTO tool_runs (work, round, tool, args, attempt, queued_at, started_at, ended_at)
               SELECT k % 2000 + 1, k / 2000 + 1, CASE WHEN k % 3 = 0 THEN 'ci' ELSE 'full_ci' END, '{{}}', 1,
                 {START} + k * 86400, {START} + k * 86400, {START} + k * 86400 + 300000
               FROM n;
             WITH RECURSIVE n(k) AS (SELECT 0 UNION ALL SELECT k + 1 FROM n WHERE k < 199999)
             INSERT INTO changes (tx, at, actor, table_name, row_key, old_row, new_row)
               SELECT k + 10000, {START} + k * 43200, 'engine', CASE WHEN k % 4 = 0 THEN 'items' ELSE 'works' END,
                 json_object('id', k % 2000 + 1),
                 json_object('id', k % 2000 + 1, 'project', 'p', 'paused', CASE WHEN k % 200 = 101 THEN 'held' END),
                 json_object('id', k % 2000 + 1, 'project', 'p', 'paused', CASE WHEN k % 200 = 1 THEN 'held' END)
               FROM n;
             WITH RECURSIVE n(k) AS (SELECT 0 UNION ALL SELECT k + 1 FROM n WHERE k < 10079)
             INSERT INTO samples (measure, project, at, value)
               SELECT m.name, m.project, {END} - 7 * {DAY} + k * 60000, k % 7
               FROM n, (SELECT 'engine.cpu' AS name, '' AS project UNION ALL SELECT 'page.reads', ''
                        UNION ALL SELECT 'page.read_ms', '' UNION ALL SELECT 'slots.used', ''
                        UNION ALL SELECT 'works.running', '' UNION ALL SELECT 'queue.oldest', 'p') m;"
        ))
        .unwrap();
    // Blockers and matters are state, written as the engine writes them; a matter is handled by a round seeded above.
    db.write(Actor::Engine, |tx| {
        tx.execute_batch(&format!(
            "WITH RECURSIVE n(k) AS (SELECT 1 UNION ALL SELECT k + 2 FROM n WHERE k < 1999)
             INSERT INTO blockers (work, cell, kind, what, holder, at, since, ended, outcome, ended_at)
               SELECT k, 'I1', 'question', 'w', 'advisor', {START} + k * 4320000, {START} + k * 4320000,
                 'resolved', '{{}}', {START} + k * 4320000 + 3600000
               FROM n;
             WITH RECURSIVE n(k) AS (SELECT 1 UNION ALL SELECT k + 2 FROM n WHERE k < 1999)
             INSERT INTO architect_matters (project, kind, what, body, at, handled_work, handled_round)
               SELECT 'p', 'owner_words', 'w', '{{}}', {START} + k * 4320000 - 600000, k, 1 FROM n;"
        ))?;
        Ok(())
    })
    .unwrap();
    db
}

/// How SQLite reads a statement, with the arguments it ran with: each step's detail.
fn plan(conn: &Connection, sql: &str, args: &[db::rusqlite::types::Value]) -> Vec<String> {
    let mut st = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
    st.query_map(params_from_iter(args.iter()), |r| r.get::<_, String>(3))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

// Every measure reads the last week of the history at a step of a minute, all of them together within five seconds,
// each of its statements reading the rounds, the changes and the tool runs by an index, never whole: a step that scans
// one of them reads the whole history, which grows without end.
#[test]
fn every_measure_reads_a_week_by_index_however_long_the_history() {
    let db = seeded();
    let conn = db.conn();
    let rounds: i64 = conn.query_row("SELECT count(*) FROM rounds", [], |r| r.get(0)).unwrap();
    let changes: i64 = conn
        .query_row("SELECT count(*) FROM changes", [], |r| r.get(0))
        .unwrap();
    assert!(
        rounds >= 100_000 && changes >= 200_000,
        "{rounds} rounds, {changes} changes"
    );
    let (from, to) = (END - 7 * DAY, END);
    // Main's history: a commit every ten minutes over the last week and the week before it, the newest first, none of
    // them a merge the flow recorded.
    let log: String = (0..2016)
        .map(|k: i64| format!("{k:040x} {}\n", (END - 1_000 - k * 600_000) / 1000))
        .collect();
    let main = MainLog::parse(&log).unwrap();
    let mut took = Duration::ZERO;
    let mut ran = Vec::new();
    // How many values each measure folded over the week, of every project together.
    let mut folded = Vec::new();
    for scope in [
        Scope {
            project: Some("p"),
            subject: None,
            main: Some(&main),
        },
        Scope::default(),
    ] {
        for m in measures() {
            let r = Reader::new(conn);
            let started = Instant::now();
            let s = series_read(&r, m, &scope, from, to, MINUTE).unwrap();
            took += started.elapsed();
            assert_eq!(s.points.len(), 7 * 24 * 60, "{}", m.name);
            folded.push((m.name, s.points.iter().map(|p| p.n).sum::<i64>()));
            ran.extend(r.ran.into_inner().into_iter().map(|(sql, args)| (m.name, sql, args)));
        }
    }
    assert!(took < Duration::from_secs(5), "every measure, twice, took {took:?}");
    // The seeded week holds what these measures count, so that a statement that found nothing because it read nothing
    // does not pass.
    for m in [
        "merge.broken",
        "round.context",
        "round.sent_back",
        "work.stops",
        "work.stopped",
        "engine.cpu",
        "work.to_merge",
        "ci.wait",
        "items.made",
        "spend.usd",
        "queue.oldest",
        "merge.not_on_main",
        "main.direct_pushes",
    ] {
        assert!(folded.iter().any(|(name, n)| *name == m && *n > 0), "{m}: {folded:?}");
    }
    // A statement that reads the rounds whole is found so; so is one that reads the blockers raised before a time, from
    // the first, and one that reads a measure's samples of every time.
    let whole = plan(conn, "SELECT count(*) FROM rounds r WHERE r.cell = 'I1'", &[]);
    assert!(whole.iter().any(|s| scans_large(s)), "{whole:?}");
    let before = plan(conn, "SELECT count(*) FROM blockers b WHERE b.at < 5", &[]);
    assert!(before.iter().any(|s| from_the_start(s)), "{before:?}");
    let ever = plan(
        conn,
        "SELECT count(*) FROM samples WHERE measure = 'queue.oldest' AND value > 0",
        &[],
    );
    assert!(ever.iter().any(|s| from_the_start(s)), "{ever:?}");
    for (name, sql, args) in &ran {
        for step in plan(conn, sql, args) {
            assert!(!scans_large(&step), "{name} reads a large table whole: {step}\n{sql}");
            assert!(!from_the_start(&step), "{name} reads from the start of the history: {step}\n{sql}");
        }
    }
}

/// Whether a step of a plan reads an index from its first entry: up to a bound with none below it, as reading what
/// came before a time does, or the samples of a measure with no time at all. Either reads more the longer the history.
fn from_the_start(step: &str) -> bool {
    let Some(rest) = step.strip_prefix("SEARCH ") else {
        return false;
    };
    let within = rest.split_once('(').map_or("", |(_, c)| c);
    let up_to_only = within.contains('<') && !within.contains('>');
    let samples_untimed = rest.starts_with("samples ") && !within.contains("at>");
    up_to_only || samples_untimed
}

/// Whether a step of a plan reads one of the large tables whole, as SQLite names a table it reads: by its name, or by
/// the name a statement of the health numbers gives it.
fn scans_large(step: &str) -> bool {
    let large = ["rounds", "changes", "tool_runs", "r", "x", "c", "t"];
    step.strip_prefix("SCAN ")
        .and_then(|s| s.split_whitespace().next())
        .is_some_and(|t| large.contains(&t))
}
