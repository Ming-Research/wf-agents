//! Times the read behind the page's list of works (`GET /api/v1/works`, `api::works`) on a database of hundreds of
//! works with a hundred or more rounds each, and prints the time of each read and the size of the answer.
//!
//! Not part of the build: the measure workflow's command copies this file to `crates/api/tests/works_read.rs` in each of
//! its three checkouts (see `report.md`), so that the code before and after the change run the same harness. It uses
//! only what both have: `api::works`, `api::Flows`, `db` and `flow`.

use std::time::Instant;

use api::Flows;
use db::{Actor, Db};

/// Works in the database, and the rounds of each: the list reads every round of every work.
const WORKS: i64 = 300;
const ROUNDS: i64 = 150;
/// Reads timed after one that is not.
const READS: usize = 30;

#[test]
#[ignore = "a measurement, run by the measure workflow's command"]
fn time_the_list_of_works() {
    let compiled = flow::compile(
        flow::FLOW,
        &flow::Context {
            relations: db::relations(db::SCHEMA).unwrap(),
            tables: db::index_of(db::SCHEMA).unwrap(),
        },
    )
    .unwrap();
    let json = compiled.json();
    let mut db = Db::in_memory().unwrap();
    db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO flow_versions (digest, compiled) VALUES ('d', ?1)",
            db::rusqlite::params![json],
        )?;
        // One item and one work for each; every fifth work is done, the others are mid-way through the graph.
        tx.execute(
            "WITH RECURSIVE seq(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM seq WHERE i < ?1)
             INSERT INTO items (project, title, problem, why, deliver, level)
             SELECT 'p', 'Item ' || i, 'x', 'y', 'change', '1' FROM seq",
            [WORKS],
        )?;
        tx.execute(
            "WITH RECURSIVE seq(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM seq WHERE i < ?1)
             INSERT INTO works (item, flow_version, graph, at) SELECT i, 1, 'engineer', 'I1' FROM seq",
            [WORKS],
        )?;
        // The rounds of each work, one after another at 4 s in 5 s: a model's cell, a tool's, a model's, a tool's and the
        // engine's own, in turn; one in ten was sent back. The input and output are the size a real round's are not, so
        // that what is measured is what the list reads of them.
        tx.execute(
            "WITH RECURSIVE w(id) AS (SELECT 1 UNION ALL SELECT id + 1 FROM w WHERE id < ?1),
                  n(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM n WHERE n < ?2)
             INSERT INTO rounds (work, n, cell, input, output, transition, rejected, config_commit, usage, started_at, ended_at)
             SELECT w.id, n.n,
                    CASE n.n % 5 WHEN 0 THEN 'R1' WHEN 1 THEN 'X1' WHEN 2 THEN 'I1' WHEN 3 THEN 'X1' ELSE 'S0' END,
                    '{}', '{}',
                    CASE WHEN n.n % 10 = 7 THEN NULL ELSE 'took' END,
                    CASE WHEN n.n % 10 = 7 THEN '{\"format\":\"no outcome\"}' END,
                    'c', '{\"cost_usd\":0.5}',
                    w.id * 1000000 + n.n * 5000, w.id * 1000000 + n.n * 5000 + 4000
             FROM w, n",
            [WORKS, ROUNDS],
        )?;
        Ok(())
    })
    .unwrap();
    let flows = Flows::default();
    // The flow is read and parsed once for the page's life; the first read is not timed.
    let works = api::works(db.conn(), &flows).unwrap();
    assert_eq!(works.len() as i64, WORKS);
    assert_eq!(works[0].rounds, ROUNDS);
    // Each read, then its answer written as the page's route writes it.
    let (mut read, mut written): (Vec<u128>, Vec<u128>) = (0..READS)
        .map(|_| {
            let start = Instant::now();
            let works = api::works(db.conn(), &flows).unwrap();
            let read = start.elapsed().as_micros();
            assert_eq!(works.len() as i64, WORKS);
            let start = Instant::now();
            let body = serde_json::to_string(&works).unwrap();
            let written = start.elapsed().as_micros();
            assert!(!body.is_empty());
            (read, written)
        })
        .unzip();
    read.sort_unstable();
    written.sort_unstable();
    let size = serde_json::to_string(&works).unwrap().len();
    println!(
        "works_read works={WORKS} rounds_each={ROUNDS} reads={READS} read_min_us={} read_median_us={} read_max_us={} \
         write_median_us={} body_bytes={size}",
        read[0],
        read[READS / 2],
        read[READS - 1],
        written[READS / 2],
    );
}
