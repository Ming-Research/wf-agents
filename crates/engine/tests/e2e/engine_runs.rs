//! The table `engine_runs`, which records the runs of the engine's loop, as the database holds its rows.

use super::*;

/// Writes a row of `engine_runs` with the given stop time and reason; whether the database took it.
fn written(e: &mut Engine, stopped_at: Option<i64>, reason: Option<&str>) -> bool {
    let flow = e.flow_now;
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO engine_runs (started_at, built_from, config_commit, flow_version, stopped_at, stop_reason)
             VALUES (1, 'b', 'c', ?1, ?2, ?3)",
            params![flow, stopped_at, reason],
        )?;
        Ok(())
    })
    .is_ok()
}

// The table takes no row that is half complete: its stop time and its reason are both there or both not, a run lost
// excepted (the table's own comment on `stop_reason` and on the check). A row that runs and a row that stopped with
// both are taken, so that the refusals below are the check's and not those of a table that is not there.
#[test]
fn the_table_of_runs_takes_no_row_with_a_time_and_no_reason_or_a_reason_and_no_time() {
    let s = setup("engine-runs-check");
    let (mut e, _) = engine(&s, scripts());
    assert!(written(&mut e, None, None), "a run that goes on is refused");
    assert!(
        written(&mut e, Some(2), Some("stop file")),
        "a run that stopped is refused"
    );
    assert!(written(&mut e, None, Some("lost")), "a run found lost is refused");
    assert!(!written(&mut e, Some(2), None), "a stop time with no reason was taken");
    assert!(
        !written(&mut e, None, Some("error: x")),
        "a reason that is not lost with no stop time was taken"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}
