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
        meaning_unchanged: false,
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

/// A version of `p` put into the round of `a` under way: `a` reads `p`, a person writes it while the round is under
/// way, and it is put in after. The records hold no fault.
fn put_in_while_under_way() -> (World, RunId, Seq) {
    let mut w = World::with_gate(vec![cell("a", &["p"], &["q"])]);
    w.person_writes("p");
    let run = w.start("a");
    let version = w.person_writes("p");
    let at = w.tick();
    w.records.put_in.push(PutIn {
        run,
        block: "p".into(),
        version,
        at,
    });
    assert!(!matches!(w.decide(), Decision::Fault(_)), "{:?}", w.decide());
    (w, run, at)
}

#[test]
fn a_version_put_in_to_a_run_that_never_started_is_a_fault() {
    let (mut w, _, _) = put_in_while_under_way();
    w.records.put_in[0].run = RunId(99);
    assert_eq!(w.decide(), Decision::Fault(RecordFault::UnknownRun(RunId(99))));
}

#[test]
fn a_version_put_in_at_the_place_of_another_entry_is_a_fault() {
    let (mut w, _, _) = put_in_while_under_way();
    let at = w.records.written[1].at;
    w.records.put_in[0].at = at;
    assert_eq!(w.decide(), Decision::Fault(RecordFault::PlaceTwice(at)));
}

#[test]
fn what_is_put_in_and_is_no_version_of_its_block_is_a_fault() {
    // No entry writes anything at the place named.
    let (mut w, _, at) = put_in_while_under_way();
    w.records.put_in[0].version = Seq(50);
    assert_eq!(w.decide(), Decision::Fault(RecordFault::PutInNoVersion(at)));
    // Nor is a version of another block one of `p`'s.
    let mut w = World::with_gate(vec![cell("a", &["p"], &["q"])]);
    w.person_writes("p");
    let run = w.start("a");
    let other = w.person_writes("r");
    let at = w.tick();
    w.records.put_in.push(PutIn {
        run,
        block: "p".into(),
        version: other,
        at,
    });
    assert_eq!(w.decide(), Decision::Fault(RecordFault::PutInNoVersion(at)));
}

#[test]
fn a_version_put_in_before_it_was_written_is_a_fault() {
    let mut w = World::with_gate(vec![cell("a", &["p"], &["q"])]);
    w.person_writes("p");
    let run = w.start("a");
    let at = w.tick();
    let version = w.person_writes("p");
    w.records.put_in.push(PutIn {
        run,
        block: "p".into(),
        version,
        at,
    });
    assert_eq!(w.decide(), Decision::Fault(RecordFault::PutInBeforeWritten(at)));
}

/// A run of `a` whose round has begun, and a version of `p` a person wrote after.
fn begun() -> (World, RunId, Seq) {
    let mut w = World::with_gate(vec![cell("a", &["p"], &["q"])]);
    w.person_writes("p");
    let run = w.start("a");
    let version = w.person_writes("p");
    (w, run, version)
}

/// The version of `p` is put into the run here.
fn put(w: &mut World, run: RunId, version: Seq) -> Seq {
    let at = w.tick();
    w.records.put_in.push(PutIn {
        run,
        block: "p".into(),
        version,
        at,
    });
    at
}

#[test]
fn a_version_put_in_while_its_run_has_no_round_under_way_is_a_fault() {
    // After its round ended.
    let (mut w, run, version) = begun();
    w.returns("a");
    let at = put(&mut w, run, version);
    assert_eq!(w.decide(), Decision::Fault(RecordFault::PutInNoRound(at)));
    // After the run was abandoned, its round not yet ended.
    let (mut w, run, version) = begun();
    let gone = w.tick();
    w.records.abandoned.push(Abandoned { run, at: gone });
    let at = put(&mut w, run, version);
    assert_eq!(w.decide(), Decision::Fault(RecordFault::PutInNoRound(at)));
    // Before its next round began, the round before having ended.
    let (mut w, run, version) = begun();
    w.returns("a");
    let at = put(&mut w, run, version);
    let next = w.tick();
    w.records.rounds.push(Round { run, at: next });
    assert_eq!(w.decide(), Decision::Fault(RecordFault::PutInNoRound(at)));
}

#[test]
fn a_return_dealing_with_what_it_began_from_and_what_was_put_in_to_it_is_no_fault() {
    let (mut w, _, _) = put_in_while_under_way();
    w.finish("a", &[("q", Matters)]);
    w.records.finished[0].dealt_with = Some(vec!["p".into(), "q".into()]);
    assert!(!matches!(w.decide(), Decision::Fault(_)), "{:?}", w.decide());
    // A block only put in to it may be named too.
    let mut w = World::with_gate(vec![cell("a", &["p"], &["q"])]);
    w.person_writes("p");
    let run = w.start("a");
    let version = w.person_writes("r");
    let at = w.tick();
    w.records.put_in.push(PutIn {
        run,
        block: "r".into(),
        version,
        at,
    });
    w.finish("a", &[("q", Matters)]);
    w.records.finished[0].dealt_with = Some(vec!["r".into()]);
    assert!(!matches!(w.decide(), Decision::Fault(_)), "{:?}", w.decide());
}

#[test]
fn a_return_dealing_with_a_block_that_neither_ran_it_nor_was_put_in_to_it_is_a_fault() {
    let (mut w, run, _) = put_in_while_under_way();
    w.finish("a", &[("q", Matters)]);
    w.records.finished[0].dealt_with = Some(vec!["p".into(), "r".into()]);
    assert_eq!(w.decide(), Decision::Fault(RecordFault::DealtWithUnknown(run)));
}

#[test]
fn a_return_dealing_with_one_block_twice_is_a_fault() {
    let (mut w, run, _) = put_in_while_under_way();
    w.finish("a", &[("q", Matters)]);
    w.records.finished[0].dealt_with = Some(vec!["p".into(), "p".into()]);
    assert_eq!(w.decide(), Decision::Fault(RecordFault::DealtWithTwice(run)));
}

/// A call of the run, made here.
fn called(w: &mut World, run: RunId) {
    let at = w.tick();
    w.records.called.push(decide::Called {
        call: CallId(99),
        run,
        on: None,
        at,
    });
}

#[test]
fn a_call_made_after_its_run_ended_is_a_fault() {
    // After it finished.
    let mut w = World::with_gate(vec![cell("a", &["p"], &["q"])]);
    w.person_writes("p");
    let run = w.start("a");
    w.finish("a", &[("q", Matters)]);
    called(&mut w, run);
    assert_eq!(w.decide(), Decision::Fault(RecordFault::CalledAfterEnd(CallId(99))));
    // After it was abandoned.
    let (mut w, run, _) = begun();
    let gone = w.tick();
    w.records.abandoned.push(Abandoned { run, at: gone });
    called(&mut w, run);
    assert_eq!(w.decide(), Decision::Fault(RecordFault::CalledAfterEnd(CallId(99))));
    // A call made as the return is taken, before the run's finish, is none.
    let mut w = World::with_gate(vec![cell("a", &["p"], &["q"])]);
    w.person_writes("p");
    let run = w.start("a");
    w.returns("a");
    called(&mut w, run);
    w.finish("a", &[]);
    assert!(!matches!(w.decide(), Decision::Fault(_)), "{:?}", w.decide());
}
