//! Item 226, acceptance A5: how long the views of counters took before version 68 of the database and take now, on a
//! database of the size of the engine's own. It is on one commit only, run once by CI, and deleted by the next commit:
//! it ends in a panic on purpose, so that the log of the run holds its numbers, which go to the pull request.

use std::time::Instant;

use db::rusqlite::{Connection, params};
use db::{Actor, Db};

/// Works, the one the views are read for, and the rounds it has and each of the others has.
const WORKS: i64 = 200;
const BIG: i64 = 150;
const BIG_ROUNDS: i64 = 1100;
const OTHER_ROUNDS: i64 = 27;
/// A round's input and output, in characters: the real ones range from 0.5 KB to 175 KB, and average 5-46 KB for the
/// input and 0.5-3.5 KB for the output in the two works looked at.
const INPUT: usize = 20_000;
const OUTPUT: usize = 2_000;
/// Pairs run: at least `MIN_PAIRS`, then more while the run is under `BUDGET` seconds, up to `MAX_PAIRS`.
const MIN_PAIRS: usize = 3;
const MAX_PAIRS: usize = 8;
const BUDGET: f64 = 270.0;
/// The two-sided 95% points of Student's t for 1 to 9 degrees of freedom.
const T95: [f64; 9] = [12.706, 4.303, 3.182, 2.776, 2.571, 2.447, 2.365, 2.306, 2.262];

/// The views of migration 0033, as they were before version 68, under names that begin with `prefix`.
fn old_views(prefix: &str) -> String {
    let text = include_str!("../../../db/migrations/0033.sql");
    let from = text
        .find("CREATE VIEW work_counters")
        .expect("0033 defines work_counters");
    text[from..]
        .replace("work_counters", &format!("{prefix}work_counters"))
        .replace("work_asking", &format!("{prefix}work_asking"))
}

/// Reads a view for the work, and says how long it took, in milliseconds, and the rows it gave, sorted.
fn read(conn: &Connection, view: &str, work: i64) -> (f64, Vec<String>) {
    let start = Instant::now();
    let mut st = conn.prepare(&format!("SELECT * FROM {view} WHERE work = ?1")).unwrap();
    let columns = st.column_count();
    let mut rows: Vec<String> = st
        .query_map([work], |r| {
            let mut cells = Vec::new();
            for i in 0..columns {
                cells.push(format!("{:?}", r.get_ref(i)?));
            }
            Ok(cells.join("|"))
        })
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    rows.sort();
    (ms, rows)
}

/// The mean of the logarithms of `ratios`, and the ends of its 95% interval, as ratios.
fn interval(ratios: &[f64]) -> (f64, f64, f64) {
    let logs: Vec<f64> = ratios.iter().map(|r| r.ln()).collect();
    let n = logs.len() as f64;
    let mean = logs.iter().sum::<f64>() / n;
    let var = logs.iter().map(|l| (l - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let half = T95[logs.len() - 2] * (var / n).sqrt();
    (mean.exp(), (mean - half).exp(), (mean + half).exp())
}

#[test]
fn the_counters_views_before_and_after_version_68() {
    let dir = std::env::temp_dir().join(format!("wf-agents-counters-timing-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("wf.db");
    let mut db = Db::create(&path).unwrap();
    // The flow the engine runs, compiled against the database it makes, as the engine stores it.
    let relations = db::relations(db::SCHEMA).unwrap();
    let tables = db::index_of(db::SCHEMA).unwrap();
    let compiled = flow::compile(flow::FLOW, &flow::Context { relations, tables }).unwrap();
    let text = compiled.json();
    let engineer = compiled.graphs.iter().find(|g| g.name == "engineer").unwrap();
    // Every row of the engineer's graph, with the cell it is at and the counter it counts.
    let rows: Vec<(String, String, Option<String>)> = engineer
        .cells
        .iter()
        .flat_map(|c| c.rows.iter().map(|r| (c.id.clone(), r.name.clone(), r.counts.clone())))
        .collect();
    let counting = rows.iter().filter(|r| r.2.is_some()).count();
    let input = format!("{{\"prompt\":\"{}\"}}", "a".repeat(INPUT));
    let output = format!("{{\"outcome\":\"{}\"}}", "b".repeat(OUTPUT));
    db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO flow_versions (digest, compiled) VALUES ('timing', ?1)",
            [&text],
        )?;
        for _ in 0..WORKS {
            tx.execute(
                "INSERT INTO works (flow_version, graph, at) VALUES (1, 'engineer', 'I1')",
                [],
            )?;
        }
        let mut st = tx.prepare(
            "INSERT INTO rounds (work, n, cell, input, output, transition, counts, config_commit, started_at, ended_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'c', ?8, ?9)",
        )?;
        let mut x: u64 = 12345;
        for work in 1..=WORKS {
            for n in 1..=if work == BIG { BIG_ROUNDS } else { OTHER_ROUNDS } {
                x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let (cell, row, counts) = &rows[(x >> 33) as usize % rows.len()];
                let at = 1_000_000 * work + 100_000 * n;
                st.execute(params![work, n, cell, input, output, row, counts, at, at + 90_000])?;
            }
        }
        Ok(())
    })
    .unwrap();
    let conn = db.conn();
    conn.execute_batch(&old_views("old_")).unwrap();
    conn.execute_batch(&old_views("twin_")).unwrap();
    // The data are in the file, as those of a database that has been running are.
    conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
        .unwrap();
    let total: i64 = conn.query_row("SELECT count(*) FROM rounds", [], |r| r.get(0)).unwrap();
    let pages: i64 = conn.query_row("PRAGMA page_count", [], |r| r.get(0)).unwrap();
    let page_size: i64 = conn.query_row("PRAGMA page_size", [], |r| r.get(0)).unwrap();
    let sqlite: String = conn.query_row("SELECT sqlite_version()", [], |r| r.get(0)).unwrap();

    // Each pair reads both views on each of the three sides: the old definition, an identical twin of it that tells
    // the noise, and the new one; the order of the sides turns round from pair to pair.
    let prefixes = ["old_", "twin_", ""];
    let views = ["work_asking", "work_counters"];
    let mut ms = [
        [Vec::new(), Vec::new(), Vec::new()],
        [Vec::new(), Vec::new(), Vec::new()],
    ];
    let mut first: [[Vec<String>; 3]; 2] = Default::default();
    let begun = Instant::now();
    let mut pairs = 0;
    loop {
        let before = begun.elapsed().as_secs_f64();
        for turn in 0..3 {
            let side = (pairs + turn) % 3;
            for (v, view) in views.iter().enumerate() {
                let (took, rows) = read(conn, &format!("{}{view}", prefixes[side]), BIG);
                ms[v][side].push(took);
                if pairs == 0 {
                    first[v][side] = rows;
                }
            }
        }
        pairs += 1;
        let pair = begun.elapsed().as_secs_f64() - before;
        if pairs >= MAX_PAIRS || (pairs >= MIN_PAIRS && begun.elapsed().as_secs_f64() + pair > BUDGET) {
            break;
        }
    }

    let mut report = format!(
        "{total} rounds in {WORKS} works, {BIG_ROUNDS} in work {BIG}, input {INPUT} and output {OUTPUT} characters each; \
         {counting} of {} rows of the engineer's graph count; the compiled flow is {} bytes; the database is {} MB \
         ({pages} pages of {page_size}); sqlite {sqlite}; {} CPUs; {pairs} pairs in {:.0} s\n",
        rows.len(),
        text.len(),
        pages * page_size / 1_000_000,
        std::thread::available_parallelism().map_or(0, |n| n.get()),
        begun.elapsed().as_secs_f64(),
    );
    for (v, view) in views.iter().enumerate() {
        let same = first[v][0] == first[v][1] && first[v][0] == first[v][2];
        report.push_str(&format!(
            "\n{view} WHERE work = {BIG}: {} rows; old, twin and new give the same rows: {same}\n  rows: {:?}\n",
            first[v][0].len(),
            first[v][2],
        ));
        report.push_str("  pair: old ms, twin ms, new ms\n");
        for (p, ((old, twin), new)) in ms[v][0].iter().zip(&ms[v][1]).zip(&ms[v][2]).enumerate() {
            report.push_str(&format!("  {p}: {old:.1}, {twin:.1}, {new:.3}\n"));
        }
        let ratio = |side: usize| -> Vec<f64> { (0..pairs).map(|p| ms[v][side][p] / ms[v][0][p]).collect() };
        let (twin, twin_lo, twin_hi) = interval(&ratio(1));
        let (new, new_lo, new_hi) = interval(&ratio(2));
        let noise = (1.0 - twin_lo).abs().max((twin_hi - 1.0).abs());
        report.push_str(&format!(
            "  twin/old: {twin:.4} [{twin_lo:.4}, {twin_hi:.4}]; noise {noise:.4}\n  new/old: {new:.5} [{new_lo:.5}, {new_hi:.5}]\n  \
             passes (new/old wholly below 1 and the noise no more than its distance from 1): {}\n",
            new_hi < 1.0 && noise <= 1.0 - new_hi
        ));
    }
    let _ = std::fs::remove_dir_all(&dir);
    panic!("TIMING REPORT (this test ends in a panic on purpose, so that the log holds the numbers)\n{report}");
}
