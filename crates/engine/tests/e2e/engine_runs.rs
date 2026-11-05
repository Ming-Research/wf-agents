//! The runs of the engine's loop, as `engine_runs` records them, in the corners a single run does not reach.

use super::*;

type Stop = (Option<i64>, Option<String>);

/// When a run stopped and why, as its row says.
fn stop_of(e: &Engine, run: i64) -> Stop {
    e.db.conn()
        .query_row(
            "SELECT stopped_at, stop_reason FROM engine_runs WHERE id = ?1",
            [run],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
}

// A run's stop is written once: a later stop of the same run, as the loop's two ways out would make if both ran, leaves
// the first time and reason. A run found lost stays lost over later starts, and the table takes no row that is half
// complete: its stop time and its reason are both there or both not, a run lost excepted (the table's own comment).
#[test]
fn a_stop_is_written_once_a_lost_run_stays_lost_and_no_row_is_half_complete() {
    let s = setup("engine-runs-corners");
    let (mut e, _) = engine(&s, scripts());
    let first = e.record_start().unwrap();
    e.record_stop(first, "stop file").unwrap();
    let stopped = stop_of(&e, first);
    assert!(stopped.0.is_some());
    e.record_stop(first, "error: a later one").unwrap();
    assert_eq!(stop_of(&e, first), stopped);
    assert_eq!(stopped.1.as_deref(), Some("stop file"));
    // Three starts in a row leave the first two lost, with the time of their stop unknown.
    let second = e.record_start().unwrap();
    let third = e.record_start().unwrap();
    let fourth = e.record_start().unwrap();
    assert_eq!(stop_of(&e, second), (None, Some("lost".to_string())));
    assert_eq!(stop_of(&e, third), (None, Some("lost".to_string())));
    assert_eq!(stop_of(&e, fourth), (None, None));
    assert_eq!(stop_of(&e, first), stopped);
    // The runs are four, one row each.
    let rows: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM engine_runs", [], |r| r.get(0))
            .unwrap();
    assert_eq!(rows, 4);
    // A row with a time and no reason, or with a reason that is not `lost` and no time, is refused.
    let flow = e.flow_now;
    for (at, reason) in [(Some(1_i64), None), (None, Some("error: x"))] {
        let written = e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO engine_runs (started_at, built_from, config_commit, flow_version, stopped_at, stop_reason)
                 VALUES (1, 'b', 'c', ?1, ?2, ?3)",
                params![flow, at, reason],
            )?;
            Ok(())
        });
        assert!(written.is_err(), "{at:?} {reason:?}");
    }
    let _ = std::fs::remove_dir_all(&s.root);
}

// The engine's process killed leaves its run open; the next engine over the same data marks it lost when it starts,
// and its own run is open.
#[test]
fn an_engine_started_over_the_data_of_a_killed_one_marks_its_run_lost() {
    let s = setup("engine-runs-restart");
    let (mut killed, _) = engine(&s, scripts());
    let left = killed.record_start().unwrap();
    drop(killed);
    let (mut next, _) = engine(&s, scripts());
    let now = next.record_start().unwrap();
    assert_ne!(left, now);
    assert_eq!(stop_of(&next, left), (None, Some("lost".to_string())));
    assert_eq!(stop_of(&next, now), (None, None));
    let _ = std::fs::remove_dir_all(&s.root);
}
