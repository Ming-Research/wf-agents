//! A fault in one work does not stop the engine's loop: the work is paused with why and the others go on.

use super::*;

/// Puts a work on a flow version whose stored text the engine cannot read, as a flow version from before a change to the
/// compiled flow's form would be: the work cannot be read, since its graph cannot be found.
fn on_an_unreadable_flow_version(e: &mut Engine, work: i64) {
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO flow_versions (digest, compiled) VALUES ('unreadable', '[]')",
            [],
        )?;
        let version = tx.last_insert_rowid();
        tx.execute(
            "UPDATE works SET flow_version = ?2 WHERE id = ?1",
            params![work, version],
        )?;
        Ok(())
    })
    .unwrap();
}

// A work the loop cannot read, here one on a flow version whose text is unreadable, is paused with why in the loop's
// turn, and the other work goes on beside it in the same turns: before, the error ended the turn and, in the loop that
// runs the engine, the loop, with no work dispatched again.
#[test]
fn a_work_the_loop_cannot_read_is_paused_and_the_other_works_go_on() {
    let (s, mut e, said, first, second) = two_works_with("unreadable-work", scripts(), Hooks::default());
    e.tick().unwrap();
    let (a, b) = (state(&e, first).unwrap().0, state(&e, second).unwrap().0);
    let before = rounds_after(&e, b, 0).0;
    on_an_unreadable_flow_version(&mut e, a);
    let moved = turns_until(&mut e, &Go::default(), |e| rounds_after(e, b, 0).0 > before);
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(moved, "the other work did not go on:\n{log}");
    let paused: Option<String> =
        e.db.conn()
            .query_row("SELECT paused FROM works WHERE id = ?1", [a], |r| r.get(0))
            .unwrap();
    let paused = paused.unwrap_or_else(|| panic!("the work was not paused:\n{log}"));
    assert!(
        paused.starts_with("the engine's loop failed on it: ") && paused.contains("flow version"),
        "{paused}"
    );
    assert_eq!(paused_by(&e, a).as_deref(), Some("engine"), "{log}");
    assert_eq!(paused_by(&e, b), None, "{log}");
    // The owner is told once, in a notice of the work.
    let notices: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM engine_notices WHERE work = ?1 AND text LIKE '%is paused: the engine''s loop failed%'",
                [a],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(notices, 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's Step of a work the loop cannot read does not end the turn either: the round asked is given back with no
// step, the work stays paused with the reason the engine found, and the other work is left as it was.
#[test]
fn the_owner_s_step_of_a_work_the_loop_cannot_read_is_given_back_and_ends_no_turn() {
    let (s, mut e, said, first, second) = two_works_with("unreadable-step", scripts(), Hooks::default());
    e.tick().unwrap();
    let (a, b) = (state(&e, first).unwrap().0, state(&e, second).unwrap().0);
    on_an_unreadable_flow_version(&mut e, a);
    let go = Go {
        pause: held_but(&[], &[]),
        steps: vec![a],
        architects: Vec::new(),
    };
    let passed = e.pass(&go).unwrap();
    let log = said.lock().unwrap().join("\n");
    let stepped = asked_in(&passed, &Ask::Work(a));
    assert_eq!(stepped.work, Some(a), "{log}");
    assert!(stepped.rounds.is_empty(), "{log}");
    let paused: Option<String> =
        e.db.conn()
            .query_row("SELECT paused FROM works WHERE id = ?1", [a], |r| r.get(0))
            .unwrap();
    assert!(
        paused.is_some_and(|p| p.starts_with("the engine's loop failed on it: ")),
        "{log}"
    );
    assert_eq!(paused_by(&e, a).as_deref(), Some("engine"), "{log}");
    assert_eq!(paused_by(&e, b), None, "{log}");
    // Asked again, it is taken the same way: nothing is left standing for the next turn.
    let again = e.pass(&go).unwrap();
    assert_eq!(asked_in(&again, &Ask::Work(a)).work, Some(a), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Pausing a work whose graph cannot be found closes the round it left open, with the reason, and writes the pause: the
// pause looks for the graph to tell a question waiting for its answer, and cannot depend on finding it.
#[test]
fn a_work_whose_graph_cannot_be_found_is_paused_with_its_open_round_closed() {
    let s = setup("pause-unreadable");
    let (mut e, _, item) = start(&s, scripts());
    e.tick().unwrap();
    let work = state(&e, item).unwrap().0;
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, config_commit, started_at)
             SELECT ?1, coalesce(max(n), 0) + 1, 'R0', '{}', 'c', ?2 FROM rounds WHERE work = ?1",
            params![work, db::now()],
        )?;
        Ok(())
    })
    .unwrap();
    on_an_unreadable_flow_version(&mut e, work);
    assert!(e.work(work).is_err());
    assert_eq!(open_rounds(&e, work), 1);
    e.pause(work, "a scripted reason").unwrap();
    assert_eq!(open_rounds(&e, work), 0);
    let (rejected, paused): (String, Option<String>) =
        e.db.conn()
            .query_row(
                "SELECT (SELECT rejected FROM rounds WHERE work = ?1 AND ended_at IS NOT NULL ORDER BY n DESC LIMIT 1),
                        (SELECT paused FROM works WHERE id = ?1)",
                [work],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert!(rejected.contains("a scripted reason"), "{rejected}");
    assert_eq!(paused.as_deref(), Some("a scripted reason"));
    assert_eq!(paused_by(&e, work).as_deref(), Some("engine"));
    let _ = std::fs::remove_dir_all(&s.root);
}
