//! A cell that asks for what takes time in a record, such as a run of the tests, and the cell that record runs, which
//! gives the result into a block of the asking cell's own (decision a-wait-holds-its-cell): the asking cell waits from
//! its entry until a run of it is given a result written after it; meanwhile no later cell runs but the one that gives
//! the result, and the owner turning to it still enters it. And the count of the times each sender sends a cell back
//! (decision the-same-sender-again), which counts no result under anyone.

use decide::{Asks, Entry, Outsiders, asked_of, told};

use super::*;

/// The cell asks in the record, its result coming in the block.
fn asking(record: &str, result: &str, mut c: Cell) -> Cell {
    c.asks.push(Asks {
        record: record.into(),
        result: result.into(),
    });
    c
}

/// The cell that runs the tests the cells ask for in `tests`: an act of the engine's, which takes no slot.
fn run_tests() -> Cell {
    let mut run_tests = cell("run tests", &["tests"], &[]);
    run_tests.slot = false;
    run_tests
}

/// The research and the implementation each ask for tests in `tests`, their results coming in blocks of their own,
/// which the source `tested` writes; the review judges the code, and the cell that runs the tests is run by the
/// record. The owner's words turn to the implementation. The research ran, and wrote the plan.
fn testing() -> World {
    let cells = vec![
        asking(
            "tests",
            "research_tested",
            cell("research", &["background", "research_tested"], &["plan", "tests"]),
        ),
        asking(
            "tests",
            "implement_tested",
            cell(
                "implement",
                &["plan", "opinions", "implement_spoken", "implement_tested"],
                &["code", "tests"],
            ),
        ),
        cell("review", &["code"], &["opinions"]),
        run_tests(),
    ];
    let mut w = World::with_gate(cells);
    w.written_outside("research_tested", "tested");
    w.written_outside("implement_tested", "tested");
    w.turning("implement_spoken");
    w.person_writes("background");
    w.run("research", &[("plan", Matters)]);
    w
}

/// The implementation writes code and asks for tests, as one return dealing with all that ran it.
fn asks_for_tests(w: &mut World) -> Seq {
    w.start("implement");
    let ran = w.ran_by("implement");
    let ran: Vec<&str> = ran.iter().map(String::as_str).collect();
    w.finish_dealing("implement", &ran, &[("code", Matters), ("tests", Matters)])
}

// The owner's words the implementation was told before it asked for tests are no turning to it once the result comes:
// the round that runs the tests, under way as the act writes the result, is not stopped, and the run given the result
// takes the words up with it.
#[test]
fn a_result_written_while_its_tests_run_stops_nothing_for_words_the_cell_was_told() {
    let mut w = testing();
    w.outside_writes("implement_spoken");
    w.start("implement");
    w.finish_dealing("implement", &[], &[("tests", Matters)]);
    assert_eq!(reasons(&w.decide()).0, "run tests");
    let run = w.start("run tests");
    w.outside_writes("implement_tested");
    assert_eq!(
        w.decide(),
        Decision::Busy {
            cell: "run tests".into(),
            run
        }
    );
}

/// The tests run for what was asked, and give the implementation its result.
fn tests_run(w: &mut World) {
    w.start("run tests");
    w.outside_writes("implement_tested");
    w.finish("run tests", &[]);
}

#[test]
fn while_a_cell_waits_for_its_tests_no_later_cell_runs_but_the_one_that_runs_them() {
    let mut w = testing();
    let entry = asks_for_tests(&mut w);
    // The code is new to the review, which stands after the implementation; the tests run first, and only they.
    let Decision::Run { cell, because, .. } = w.decide() else {
        panic!("the walk runs no cell: {:?}", w.decide());
    };
    assert_eq!(cell.as_str(), "run tests");
    assert_eq!(because, vec![changed("tests", None, false)]);
    assert_eq!(
        asked_of(&w.flow, &w.records, &"run tests".into()),
        vec![Entry {
            cell: "implement".into(),
            record: "tests".into(),
            at: entry,
            result: "implement_tested".into(),
            replaced: Vec::new(),
        }]
    );
    w.run("run tests", &[]);
    // Nothing is new to the cell that runs the tests, and the result has not come: the work waits at the
    // implementation for it, and the review still does not run.
    assert_eq!(
        w.decide(),
        wait(
            "implement",
            vec![Waited::Result {
                record: "tests".into(),
                result: "implement_tested".into(),
                by: vec!["run tests".into()],
            }]
        )
    );
}

#[test]
fn the_result_runs_the_asking_cell_alone_and_is_taken_by_the_run_given_it() {
    let mut w = testing();
    asks_for_tests(&mut w);
    tests_run(&mut w);
    assert_eq!(
        reasons(&w.decide()),
        ("implement".into(), vec![("implement_tested".into(), false)])
    );
    let run = w.start("implement");
    // The result runs no trigger its return deals with: the round is given it as what it asked for.
    let told = told(&w.flow, &w.records, run).expect("the run started");
    assert!(told.ran_by.is_empty(), "{:?}", told.ran_by);
    assert!(!told.waits);
    // Handing in its work and naming nothing, the run takes the result: the review runs on the code, and the
    // implementation is not run by the result again.
    w.finish_dealing("implement", &[], &[("code", Matters)]);
    assert_eq!(reasons(&w.decide()), ("review".into(), vec![("code".into(), false)]));
    w.run("review", &[]);
    assert_eq!(w.decide(), resting());
}

// A commit made by a round that leaves its cell waiting moves the work's head, which runs the review: the review would
// judge half-done work, and no later cell runs while the cell waits, a round of it entered for the owner meanwhile and
// committing again included.
#[test]
fn what_a_round_that_leaves_its_cell_waiting_committed_runs_no_review_until_the_cell_hands_in() {
    let mut w = testing();
    asks_for_tests(&mut w);
    let tests = w.start("run tests");
    w.outside_writes("implement_spoken");
    w.abandon();
    w.stopped(tests);
    w.start("implement");
    w.finish_dealing("implement", &["implement_spoken"], &[("code", Matters)]);
    assert_eq!(
        reasons(&w.decide()),
        ("run tests".into(), vec![("tests".into(), false)])
    );
    tests_run(&mut w);
    assert_eq!(reasons(&w.decide()).0, "implement");
}

#[test]
fn the_owner_turning_to_a_cell_that_waits_for_its_tests_stops_the_run_of_the_tests_and_enters_it() {
    let mut w = testing();
    asks_for_tests(&mut w);
    let tests = w.start("run tests");
    w.outside_writes("implement_spoken");
    assert_eq!(
        w.decide(),
        abandon(
            "run tests",
            tests,
            Abandoning::Stopped {
                cell: "implement".into(),
                block: "implement_spoken".into(),
            }
        )
    );
    w.abandon();
    w.stopped(tests);
    assert_eq!(
        reasons(&w.decide()),
        ("implement".into(), vec![("implement_spoken".into(), false)])
    );
    let run = w.start("implement");
    // Entered for the owner while it waits, the round answers and goes on waiting: it owes nothing else.
    assert!(told(&w.flow, &w.records, run).expect("the run started").waits);
    w.finish_dealing("implement", &["implement_spoken"], &[]);
    // The tests, stopped, run again for the same entry.
    assert_eq!(
        reasons(&w.decide()),
        ("run tests".into(), vec![("tests".into(), false)])
    );
}

#[test]
fn a_newer_entry_of_the_same_cell_replaces_the_one_whose_tests_have_not_run() {
    let mut w = testing();
    let first = asks_for_tests(&mut w);
    let tests = w.start("run tests");
    w.outside_writes("implement_spoken");
    w.abandon();
    w.stopped(tests);
    w.start("implement");
    let second = w.finish_dealing("implement", &["implement_spoken"], &[("tests", Matters)]);
    assert_eq!(
        asked_of(&w.flow, &w.records, &"run tests".into()),
        vec![Entry {
            cell: "implement".into(),
            record: "tests".into(),
            at: second,
            result: "implement_tested".into(),
            replaced: vec![first],
        }]
    );
}

#[test]
fn a_cell_skipped_now_still_has_its_entry_run_so_that_its_result_is_there_should_it_apply_again() {
    let mut w = testing();
    w.install(vec![
        asking(
            "tests",
            "research_tested",
            on_test(
                "researching",
                cell("research", &["background", "research_tested"], &["plan", "tests"]),
            ),
        ),
        cell("implement", &["plan"], &["code"]),
        run_tests(),
        gate(),
    ]);
    w.person_writes("background");
    w.start("research");
    let entry = w.finish_dealing("research", &["background"], &[("tests", Matters)]);
    w.says("researching", false);
    let given = asked_of(&w.flow, &w.records, &"run tests".into());
    assert_eq!(given.iter().map(|e| e.at).collect::<Vec<_>>(), vec![entry]);
}

#[test]
fn a_record_of_asking_is_a_record_each_cells_entry_written_over_by_no_other() {
    let w = testing();
    assert!(w.flow.adds_entries(&"tests".into()));
    assert!(w.flow.is_result(&"implement_tested".into()));
    assert!(!w.flow.is_result(&"tests".into()));
}

#[test]
fn an_ask_whose_record_no_cell_runs_is_refused() {
    let cells = vec![
        asking("tests", "tested", cell("implement", &["plan", "tested"], &["tests"])),
        gate(),
    ];
    let outsiders = [(BlockId::from("tested"), Outsiders::default())].into_iter().collect();
    assert_eq!(
        Flow::new(cells, BTreeMap::new(), outsiders),
        Err(FlowFault::AsksAmiss("implement".into()))
    );
}

// Asking for tests again and again is no send-back: the result of what the cell asked for counts under no sender, and
// starts no count anew.
#[test]
fn the_result_of_what_a_cell_asked_for_counts_under_no_sender() {
    let mut w = testing();
    for _ in 0..5 {
        asks_for_tests(&mut w);
        tests_run(&mut w);
    }
    assert_eq!(
        reasons(&w.decide()),
        ("implement".into(), vec![("implement_tested".into(), false)])
    );
}

/// The implementation, its review, its testing and the check before the merge, each judging it after it: three
/// senders of the implementation. The implementation ran once, and its judges have not yet run on its code.
fn three_senders() -> World {
    let cells = vec![
        cell("implement", &["plan", "opinions", "testing", "check"], &["code"]),
        cell("review", &["code"], &["opinions"]),
        cell("test", &["code"], &["testing"]),
        cell("check", &["code"], &["check"]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("plan");
    w.run("implement", &[("code", Matters)]);
    w
}

/// The block a judge sends the implementation back by.
fn block_of(judge: &str) -> &'static str {
    match judge {
        "review" => "opinions",
        "test" => "testing",
        _ => "check",
    }
}

/// The judges run on the latest code in the walk's order until `sender` sends the implementation back, each before it
/// saying nothing.
fn sends(w: &mut World, sender: &str) {
    loop {
        let Decision::Run { cell, .. } = w.decide() else {
            panic!("the walk runs no judge before {sender}: {:?}", w.decide());
        };
        let judge = cell.0.clone();
        if judge == sender {
            w.run(&judge, &[(block_of(sender), Matters)]);
            return;
        }
        w.run(&judge, &[]);
    }
}

/// The judges send the implementation back in this order, and it answers each time with new code.
fn course(w: &mut World, senders: &[&str]) {
    for sender in senders {
        sends(w, sender);
        assert_eq!(reasons(&w.decide()).0, "implement", "after {sender}");
        w.run("implement", &[("code", Matters)]);
    }
}

// Work 9's implementation was sent back by its testing, its review and the check in turn, each with a cause of its
// own: each sender counts apart, and none sent it back more than its limit allows.
#[test]
fn senders_in_turn_each_count_apart_and_the_course_of_work_9_does_not_stop() {
    let mut w = three_senders();
    course(
        &mut w,
        &[
            "test", "review", "check", "test", "review", "check", "test", "review", "check",
        ],
    );
    for judge in ["review", "test", "check"] {
        assert_eq!(reasons(&w.decide()).0, judge);
        w.run(judge, &[]);
    }
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_senders_fourth_time_waits_for_the_owner_whatever_the_others_did_between() {
    let mut w = three_senders();
    course(&mut w, &["review", "check", "review", "check", "review"]);
    sends(&mut w, "review");
    assert_eq!(
        w.decide(),
        wait(
            "implement",
            vec![Waited::Limit {
                by: "review".into(),
                runs: 3
            }]
        )
    );
    // The owner lets it go on: every count of the cell begins anew.
    w.let_go("implement");
    assert_eq!(
        reasons(&w.decide()),
        ("implement".into(), vec![("opinions".into(), true)])
    );
}

#[test]
fn a_version_an_earlier_cell_wrote_starts_every_count_of_the_cell_anew() {
    let mut w = three_senders();
    course(&mut w, &["review", "review", "review"]);
    w.person_writes("plan");
    assert_eq!(reasons(&w.decide()), ("implement".into(), vec![("plan".into(), false)]));
    w.run("implement", &[("code", Matters)]);
    sends(&mut w, "review");
    assert_eq!(
        reasons(&w.decide()),
        ("implement".into(), vec![("opinions".into(), true)])
    );
}
