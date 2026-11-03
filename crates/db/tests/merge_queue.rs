//! The order of a project's merge queue: the works at the merge cell, in the order they came to it. The expected orders
//! are the orders of the writes in each test, not what the view says.

use db::rusqlite::{StatementStatus, params};
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

/// A new work, of an item of its own in the project, standing at the cell. The works are numbered from one in the order
/// they are made.
fn work(db: &mut Db, project: &str, at: &str) -> i64 {
    db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level) VALUES (?1, 'an item', 'x', 'y', 'change', 'l')",
            [project],
        )?;
        let item = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, 1, 'engineer', ?2)",
            params![item, at],
        )?;
        Ok(tx.last_insert_rowid())
    })
    .unwrap()
}

/// The works go through the cells, in the order given, in one write: each pair is a work and the cell it goes to.
fn go_to(db: &mut Db, moves: &[(i64, &str)]) {
    db.write(Actor::Engine, |tx| {
        for (work, cell) in moves {
            tx.execute("UPDATE works SET at = ?2 WHERE id = ?1", params![work, cell])?;
        }
        Ok(())
    })
    .unwrap();
}

/// The works in the project's merge queue, in its order.
fn queue(db: &Db, project: &str) -> Vec<i64> {
    let mut st = db
        .conn()
        .prepare("SELECT work FROM merge_queue WHERE project = ?1")
        .unwrap();
    st.query_map([project], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
}

// A work inserted at the merge cell comes to it by its insertion, at the place its insertion took among the others'
// arrivals; a work that leaves the cell and comes back within one write stands last, though it left no write between;
// and when several do so in one write, they stand in the order they came back.
#[test]
fn a_work_inserted_at_the_cell_or_back_within_one_write_stands_where_it_came() {
    let mut db = database();
    let w1 = work(&mut db, "p", "T6");
    let w2 = work(&mut db, "p", "M2");
    let w3 = work(&mut db, "p", "M2");
    // The first made comes to the cell after the two inserted there.
    go_to(&mut db, &[(w1, "M2")]);
    assert_eq!(queue(&db, "p"), [w2, w3, w1]);
    // The head leaves and comes back within one write.
    go_to(&mut db, &[(w2, "T6"), (w2, "M2")]);
    assert_eq!(queue(&db, "p"), [w3, w1, w2]);
    // Two leave and come back within one write: the one that came back first stands before the other.
    go_to(&mut db, &[(w3, "T6"), (w1, "T6"), (w1, "M2"), (w3, "M2")]);
    assert_eq!(queue(&db, "p"), [w2, w1, w3]);
}

// Each project has a queue of its own, in the order its works came, whatever the numbers of the works, which run to two
// digits here so that a number is not mistaken for another that begins with it.
#[test]
fn each_project_has_its_queue_in_the_order_its_works_came() {
    let mut db = database();
    // Odd-numbered works belong to project p, even-numbered to q.
    let works: Vec<i64> = (1..=12)
        .map(|n| work(&mut db, if n % 2 == 1 { "p" } else { "q" }, "T6"))
        .collect();
    assert_eq!(works, (1..=12).collect::<Vec<i64>>());
    let arrivals = [12, 3, 10, 1, 11, 2, 9, 4, 8, 5, 7, 6];
    for w in arrivals {
        go_to(&mut db, &[(w, "M2")]);
    }
    let of = |odd: bool| -> Vec<i64> { arrivals.into_iter().filter(|w| (w % 2 == 1) == odd).collect() };
    assert_eq!(queue(&db, "p"), of(true));
    assert_eq!(queue(&db, "q"), of(false));
}

/// A database in which the work `y` came to the merge cell, then the work `x`, which had passed through the cell many
/// times before, `history` times, each time leaving it again; then `x` changed its pushed head three times. Returns the
/// database and the numbers of `x` and `y`.
fn with_history(history: usize) -> (Db, i64, i64) {
    let mut db = database();
    let x = work(&mut db, "p", "T6");
    let y = work(&mut db, "p", "T6");
    db.write(Actor::Engine, |tx| {
        for n in 0..history {
            tx.execute("UPDATE works SET at = 'M2' WHERE id = ?1", [x])?;
            tx.execute("UPDATE works SET pushed_head = ?2 WHERE id = ?1", params![x, format!("a{n}")])?;
            tx.execute("UPDATE works SET at = 'T6' WHERE id = ?1", [x])?;
        }
        Ok(())
    })
    .unwrap();
    go_to(&mut db, &[(y, "M2")]);
    go_to(&mut db, &[(x, "M2")]);
    for n in 0..3 {
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE works SET pushed_head = ?2 WHERE id = ?1",
                params![x, format!("b{n}")],
            )?;
            Ok(())
        })
        .unwrap();
    }
    (db, x, y)
}

/// The number of steps the database's virtual machine takes to read project p's merge queue.
fn steps_to_read(db: &Db) -> i32 {
    let mut st = db.conn().prepare("SELECT work FROM merge_queue WHERE project = 'p'").unwrap();
    let rows = st.query_map([], |r| r.get::<_, i64>(0)).unwrap().count();
    assert_eq!(rows, 2);
    st.get_status(StatementStatus::VmStep)
}

// What the order costs does not grow with the changes a work had before it came to the cell: a work that passed
// through the cell a thousand times and came back stands after the one that came before it, as one that did not does,
// and the queue is read in about the same number of steps of the database.
#[test]
fn the_cost_of_reading_the_merge_queue_does_not_grow_with_the_history_of_its_works() {
    let (short, x_short, y_short) = with_history(0);
    let (long, x_long, y_long) = with_history(1000);
    assert_eq!(queue(&short, "p"), [y_short, x_short]);
    assert_eq!(queue(&long, "p"), [y_long, x_long]);
    let (few, many) = (steps_to_read(&short), steps_to_read(&long));
    assert!(few > 0, "the steps were not counted");
    assert!(many <= few + 50, "a work with no history takes {few} steps, one with a long history {many}");
}
