//! Records that cannot be those of a work: the view says so in its decision, and does not panic.

use super::*;

#[test]
fn the_view_of_faulty_records_says_they_are_faulty() {
    let mut w = planned();
    w.person_writes("background");
    w.start("research");
    let mut again = w.records.started[0].clone();
    again.at = w.tick();
    let run = again.run;
    w.records.started.push(again);
    let v = seen(&w);
    assert_eq!(v.decision, Decision::Fault(decide::RecordFault::RunTwice(run)));
}

#[test]
fn the_view_of_records_naming_a_run_that_never_started_does_not_panic() {
    let mut w = planned();
    let at = w.tick();
    w.records.finished.push(decide::Finished {
        run: RunId(99),
        at,
        writes: vec![decide::Write {
            block: "plan".into(),
            meaning_unchanged: false,
            value: None,
        }],
        dealt_with: None,
    });
    let v = seen(&w);
    assert_eq!(v.decision, Decision::Fault(decide::RecordFault::UnknownRun(RunId(99))));
}
