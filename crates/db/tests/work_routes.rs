//! The way each work of the engineer went (`work_routes`), read from the rows its rounds took and from nothing else:
//! no mark on the item. The expected values are what each sequence of rows means by the flow's own rows, not what the
//! view says.

use db::rusqlite::params;
use db::{Actor, Db};

/// A database with the one flow version the works run on.
fn database() -> Db {
    let mut db = Db::in_memory().unwrap();
    db.write(Actor::Engine, |tx| {
        tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
        Ok(())
    })
    .unwrap();
    db
}

/// A new work of the engineer on an item of its own, whose rounds are the given ones in order: each is the cell it ran
/// at, the row it took (none for a round that took none) and the counter that row counts (none for a row that counts
/// none). Returns the work.
fn work(db: &mut Db, rounds: &[(&str, Option<&str>, Option<&str>)]) -> i64 {
    db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 'an item', 'x', 'y', 'change', 'l')",
            [],
        )?;
        let item = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, 1, 'engineer', 'R1')",
            [item],
        )?;
        let work = tx.last_insert_rowid();
        for (n, (cell, row, counts)) in rounds.iter().enumerate() {
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at, transition, counts)
                 VALUES (?1, ?2, ?3, '{}', 'c', ?4, ?5, ?6)",
                params![work, n as i64 + 1, cell, n as i64 + 1, row, counts],
            )?;
        }
        Ok(work)
    })
    .unwrap()
}

/// The way the view gives the work; none when it gives none.
fn way(db: &Db, work: i64) -> Option<String> {
    db.conn()
        .query_row("SELECT route FROM work_routes WHERE work = ?1", [work], |r| r.get(0))
        .unwrap()
}

// A work goes the way its rows say, and the item says nothing of it: one whose research took `start` and has had no
// design went direct; one whose research took `cannot_start` and then ran the design went designed; one that began on
// the research's plan and took the row counting `shallow_research` then ran the design went turned, though the design
// is also among its rounds; one whose research has not answered, or whose research took a row of an older flow version
// (`done`, `light`, `to_normal`), has no way. The research that answers more than once is read by its latest answer.
// Before the change the view gave the item's mark (light, normal, turned) and none of these.
#[test]
fn a_work_went_the_way_its_rows_say() {
    let mut db = database();
    let direct = work(&mut db, &[("R1", Some("start"), None), ("I1", Some("holds"), None)]);
    let designed = work(
        &mut db,
        &[
            ("R1", Some("cannot_start"), None),
            ("D1", Some("chosen"), None),
            ("I1", Some("holds"), None),
        ],
    );
    let turned = work(
        &mut db,
        &[
            ("R1", Some("start"), None),
            ("I1", Some("shallow_research"), Some("shallow_research")),
            ("D1", Some("chosen"), None),
            ("I1", Some("holds"), None),
        ],
    );
    let asking = work(&mut db, &[("R1", None, None)]);
    let older = work(&mut db, &[("R1", Some("light"), None), ("I1", Some("holds"), None)]);
    let again = work(
        &mut db,
        &[("R1", Some("cannot_start"), None), ("R1", Some("start"), None)],
    );
    assert_eq!(way(&db, direct).as_deref(), Some("direct"));
    assert_eq!(way(&db, designed).as_deref(), Some("designed"));
    assert_eq!(way(&db, turned).as_deref(), Some("turned"));
    assert_eq!(way(&db, asking), None);
    assert_eq!(way(&db, older), None);
    assert_eq!(way(&db, again).as_deref(), Some("direct"));
}

// A work of another graph has no way: the view is of the engineer's works alone.
#[test]
fn a_work_of_another_graph_has_no_way() {
    let mut db = database();
    let other = db
        .write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO works (flow_version, graph, at) VALUES (1, 'architect', 'R1')",
                [],
            )?;
            let work = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at, transition)
                 VALUES (?1, 1, 'R1', '{}', 'c', 1, 'start')",
                [work],
            )?;
            Ok(work)
        })
        .unwrap();
    let found: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM work_routes WHERE work = ?1", [other], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(found, 0);
}
