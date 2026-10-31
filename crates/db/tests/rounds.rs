//! The rounds a connection tells it wrote, as the engine's threads each tell the rounds of their own step.

use std::path::Path;
use std::sync::{Arc, Mutex};

use db::rusqlite::{Connection, params};
use db::{Actor, Changed, Db};

// A round another connection started and this one ended after its mark is among this one's, as a step that cuts off
// the round another thread's step started reports it; a listener registered on this connection, which takes the rows
// committed each time it is told, takes none of the rounds from it; and a round this connection wrote before its mark
// and did not change since is not among them.
#[test]
fn a_round_another_connection_started_is_among_those_this_one_ended() {
    let path = std::env::temp_dir().join(format!("wf-agents-db-cut-off-{}.db", std::process::id()));
    let files = |p: &Path| ["", "-wal", "-shm"].map(|x| format!("{}{x}", p.display()));
    for f in files(&path) {
        let _ = std::fs::remove_file(f);
    }
    let mut own = Db::create(&path).unwrap();
    let mut other = Db::open(&path).unwrap();
    let told: Arc<Mutex<Vec<Changed>>> = Arc::default();
    let t = Arc::clone(&told);
    own.on_commit(Arc::new(move |_: &Connection, rows: &[Changed]| {
        t.lock().unwrap().extend(rows.iter().cloned());
    }))
    .unwrap();
    own.write(Actor::Engine, |tx| {
        tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
        for _ in 0..3 {
            tx.execute(
                "INSERT INTO works (flow_version, graph, at) VALUES (1, 'engineer', 'R1')",
                [],
            )?;
        }
        Ok(())
    })
    .unwrap();
    let start = |db: &mut Db, work: i64, at: i64| {
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (?1, 1, 'R1', '{}', 'c', ?2)",
                params![work, at],
            )?;
            Ok(())
        })
        .unwrap();
    };
    // Written by this connection before the mark, and left alone after it.
    start(&mut own, 3, 5);
    let mark = own.rounds_mark();
    start(&mut other, 1, 10);
    start(&mut own, 2, 20);
    own.write(Actor::Engine, |tx| {
        tx.execute("UPDATE rounds SET ended_at = 30 WHERE work = 1 AND n = 1", [])?;
        Ok(())
    })
    .unwrap();
    let rowid = |work: i64| -> i64 {
        own.conn()
            .query_row("SELECT rowid FROM rounds WHERE work = ?1 AND n = 1", [work], |r| {
                r.get(0)
            })
            .unwrap()
    };
    let heard = told.lock().unwrap().clone();
    for work in [1, 2] {
        let row = Changed {
            table: "rounds".into(),
            rowid: rowid(work),
        };
        assert!(heard.contains(&row), "round of work {work} not told: {heard:?}");
    }
    let r1 = Some("R1".to_string());
    assert_eq!(own.rounds_since(mark).unwrap(), [(1, 1, r1.clone()), (2, 1, r1)]);
    assert!(own.rounds_since(own.rounds_mark()).unwrap().is_empty());
    drop((own, other));
    for f in files(&path) {
        let _ = std::fs::remove_file(f);
    }
}
