//! A work that reached the end of its graph is ended as done in the write that completes its last round, and the
//! engine's loop does not load it again.

use super::*;

// A work that comes to the end of the engineer's graph ends as done in the write that closes its round at the end: the
// round is closed and the work ended together, its item is done for the views of the items on hand, and the engine's
// loop no longer reads it. The loop reads a work not ended with the flow version it runs on; the work is put on a
// version the engine cannot read, and a loop that read it would pause it for that, as it does a work it fails on.
#[test]
fn a_work_at_the_end_of_its_graph_is_ended_and_the_loop_does_not_load_it() {
    let s = setup("ended-done");
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "E1");
    let (work, _, _) = state(&e, item).unwrap();
    // At the end cell, before its step has run, the work has not ended.
    assert_eq!(ended_of(&e, work), None);
    step_until(&mut e, item, &said, "the end", |e| ended_done(e, work));
    let log = said.lock().unwrap().join("\n");
    assert_eq!(open_rounds(&e, work), 0, "{log}");
    let w = e.work(work).unwrap();
    assert!(e.done(&w).unwrap(), "{log}");

    // The views of the items on hand read the item as done, as they did while the work was not ended.
    let ready: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM ready_items WHERE item = ?1", [item], |r| r.get(0))
            .unwrap();
    let open: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM open_items WHERE item = ?1", [item], |r| r.get(0))
            .unwrap();
    assert_eq!((ready, open), (0, 0), "{log}");

    // The loop does not read the work: on a flow version it cannot read, the work is not paused by any of its passes.
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO flow_versions (digest, compiled) VALUES ('unreadable', '{}')",
            [],
        )?;
        let v = tx.last_insert_rowid();
        tx.execute("UPDATE works SET flow_version = ?2 WHERE id = ?1", params![work, v])?;
        Ok(())
    })
    .unwrap();
    assert!(!e.active().unwrap().contains(&work));
    e.slots_by_machine().unwrap();
    e.keep_warm(db::now(), None).unwrap();
    let paused: Option<String> =
        e.db.conn()
            .query_row("SELECT paused FROM works WHERE id = ?1", [work], |r| r.get(0))
            .unwrap();
    assert_eq!(paused, None, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}
