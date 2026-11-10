//! Records that cannot be those of a work.

use super::*;

#[test]
fn two_runs_started_with_one_name_are_a_fault() {
    let mut w = World::with_gate(vec![cell("a", &[], &[])]);
    let run = w.start("a");
    let mut again = w.records.started[0].clone();
    again.at = w.tick();
    w.records.started.push(again);
    assert_eq!(w.decide(), Decision::Fault(RecordFault::RunTwice(run)));
}

#[test]
fn an_entry_naming_a_run_that_never_started_is_a_fault() {
    let mut w = World::with_gate(vec![cell("a", &[], &[])]);
    let at = w.tick();
    w.records.rounds.push(Round { run: RunId(99), at });
    assert_eq!(w.decide(), Decision::Fault(RecordFault::UnknownRun(RunId(99))));
}

#[test]
fn a_result_of_a_call_never_made_is_a_fault() {
    let mut w = World::with_gate(vec![cell("a", &[], &[])]);
    let at = w.tick();
    w.records.done.push(Done { call: CallId(99), at });
    assert_eq!(w.decide(), Decision::Fault(RecordFault::UnknownCall(CallId(99))));
}

#[test]
fn a_call_with_two_results_is_a_fault() {
    let mut w = World::with_gate(vec![cell("a", &[], &[])]);
    w.start("a");
    let call = w.calls("a", None);
    w.done(call);
    w.done(call);
    assert_eq!(w.decide(), Decision::Fault(RecordFault::DoneTwice(call)));
}

#[test]
fn a_run_finished_twice_is_a_fault() {
    let mut w = World::with_gate(vec![cell("a", &[], &[])]);
    w.run("a", &[]);
    let mut again = w.records.finished[0].clone();
    again.at = w.tick();
    let run = again.run;
    w.records.finished.push(again);
    assert_eq!(w.decide(), Decision::Fault(RecordFault::FinishedTwice(run)));
}

#[test]
fn a_block_written_twice_by_one_run_is_a_fault() {
    let mut w = World::with_gate(vec![cell("a", &[], &["p"])]);
    w.run("a", &[("p", Matters)]);
    let write = Write {
        block: "p".into(),
        matters: true,
        value: None,
    };
    w.records.finished[0].writes.push(write);
    let run = w.records.finished[0].run;
    assert_eq!(w.decide(), Decision::Fault(RecordFault::WroteTwice(run)));
}

#[test]
fn a_run_both_finished_and_abandoned_is_a_fault() {
    let mut w = World::with_gate(vec![cell("a", &[], &[])]);
    w.run("a", &[]);
    let run = w.records.finished[0].run;
    let at = w.tick();
    w.records.abandoned.push(Abandoned { run, at });
    assert_eq!(w.decide(), Decision::Fault(RecordFault::FinishedAndAbandoned(run)));
}

#[test]
fn more_rounds_ended_than_began_is_a_fault() {
    let mut w = World::with_gate(vec![cell("a", &[], &[])]);
    let run = w.start("a");
    w.returns("a");
    let at = w.tick();
    w.records.returned.push(Returned { run, at });
    assert_eq!(w.decide(), Decision::Fault(RecordFault::ReturnedWithoutRound(run)));
}

#[test]
fn two_entries_at_one_place_are_a_fault() {
    let mut w = World::with_gate(vec![cell("a", &["p"], &[])]);
    let at = w.person_writes("p");
    w.records.written.push(Written {
        block: "p".into(),
        by: decide::Outsider::Person,
        matters: true,
        value: None,
        at,
    });
    assert_eq!(w.decide(), Decision::Fault(RecordFault::PlaceTwice(at)));
}
