//! A cell that asks for what takes time in a record, such as a run of the tests, and the cell that record runs, which
//! hands the result in as a block of the asking cell's own (decision a-wait-holds-its-cell): the asking cell waits from
//! its entry until a run of it is given a result written after it; meanwhile no later cell runs but the one that gives
//! the result, and the owner turning to it still enters it; a cell skipped has its entry put away (decision
//! a-wait-of-a-skipped-cell-is-put-away). And the count of the times each sender sends a cell back (decision
//! the-same-sender-again), which counts no result under anyone.

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

/// The cell that runs the tests the cells ask for in `tests`: an act of the engine's, which takes no slot and hands in
/// the result of each into the asking cell's own block.
fn run_tests() -> Cell {
    let mut run_tests = cell("run tests", &["tests"], &["research_tested", "implement_tested"]);
    run_tests.slot = false;
    run_tests
}

/// The research and the implementation each ask for tests in `tests`, their results coming in blocks of their own,
/// which the cell that runs the tests hands in; the review judges the code, and the cell that runs the tests is run by
/// the record. The owner's words turn to the implementation. The research ran, and wrote the plan.
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
    w.written_by_cells("research_tested");
    w.written_by_cells("implement_tested");
    w.turning("implement_spoken");
    w.person_writes("background");
    w.run("research", &[("plan", Matters)]);
    w
}

/// The implementation asks for tests, saying no in `gives`, a choice of the cells' alone where `cells_alone`, and that a
/// source outside the work writes as well otherwise; the cell that runs the tests runs only while it is yes.
fn gated(cells_alone: bool) -> World {
    let mut run_tests = when("gives", "yes", run_tests());
    run_tests.outputs = vec!["implement_tested".into()];
    let cells = vec![
        asking(
            "tests",
            "implement_tested",
            cell(
                "implement",
                &["background", "implement_tested"],
                &["code", "tests", "gives"],
            ),
        ),
        run_tests,
    ];
    let mut w = World::with_gate(cells);
    w.written_by_cells("implement_tested");
    w.written_by_cells("tests");
    if cells_alone {
        w.written_by_cells("gives");
    } else {
        w.written_outside("gives", "main");
    }
    w.person_writes("background");
    w.start("implement");
    let ran = w.ran_by("implement");
    let ran: Vec<&str> = ran.iter().map(String::as_str).collect();
    let writes = [("code", Matters), ("tests", Matters), ("gives", Value("no"))];
    w.finish_dealing("implement", &ran, &writes);
    w
}

// A wait for the result of what a cell asked for in a record, which only the cell that gives it brings, is stalled
// where that cell does not run: here its condition, on a choice the asking cell wrote, does not hold, and the asking
// cell's wait holds every cell after it (decision a-work-no-cell-can-move-is-stalled). Whoever else writes the choice,
// a source outside the work here, the work is stalled all the same: it waits on no one until someone writes what runs
// a cell, and the stall says what that is, the giver skipped for the choice.
// Well under a millisecond alone; no other test holds a wait for a result no cell will give.
#[test]
fn a_wait_for_a_result_no_cell_will_give_is_stalled_whoever_writes_what_skips_its_giver() {
    // It says the block the result comes in, and that the cell that gives it is skipped, its condition not holding.
    let skipped = Unrun::Skipped {
        on: vec![Condition::Is {
            block: "gives".into(),
            value: "yes".into(),
        }],
    };
    let by = [("run tests", skipped)];
    for cells_alone in [true, false] {
        assert_eq!(
            gated(cells_alone).decide(),
            stalled_for("implement", &[], &["implement_tested"], &by),
            "the choice written by the cells alone {cells_alone}"
        );
    }
}

/// The plan, which the background runs, writes whether the implementation is ready, a choice only cells write; the
/// implementation, which the plan and the owner's words run, requires it and asks for tests; the cell that runs the
/// tests gives the implementation's result, while `gives` says yes where `gated`. The plan said yes, the implementation
/// asked for tests, and the plan, run again, said no: only the plan makes the implementation's precondition hold, and
/// nothing new runs it.
fn unready(gated: bool) -> World {
    let mut implement = asking(
        "tests",
        "implement_tested",
        cell(
            "implement",
            &["plan", "implement_spoken", "implement_tested"],
            &["code", "tests"],
        ),
    );
    implement.requires = vec![Condition::Is {
        block: "ready".into(),
        value: "yes".into(),
    }];
    let mut tests = run_tests();
    tests.outputs = vec!["implement_tested".into()];
    if gated {
        tests = when("gives", "yes", tests);
    }
    let cells = vec![cell("plan", &["background"], &["plan", "ready"]), implement, tests];
    let mut w = World::with_gate(cells);
    for block in ["implement_tested", "tests", "ready", "gives"] {
        w.written_by_cells(block);
    }
    w.turning("implement_spoken");
    w.person_writes("background");
    w.run("plan", &[("plan", Matters), ("ready", Value("yes"))]);
    asks_for_tests(&mut w);
    w.person_writes("background");
    w.run("plan", &[("ready", Value("no"))]);
    w
}

// The owner turning to a cell that waits for its tests and that its preconditions hold: the cell that gives the result
// runs while it waits, as it does however the walk comes to the wait (decision a-wait-holds-its-cell), and the work is
// not stalled with the cell that would give the result held. Well under a millisecond alone; no other test turns to a
// waiting cell its preconditions hold.
#[test]
fn the_owner_turning_to_a_waiting_cell_its_preconditions_hold_runs_the_cell_that_gives_its_result() {
    let mut w = unready(false);
    w.outside_writes("implement_spoken");
    assert_eq!(
        reasons(&w.decide()),
        ("run tests".into(), vec![("tests".into(), false)])
    );
}

// A cell the record runs that gives no result is no giver: it does not run while the asking cell waits, as no later
// cell does but those that give the result, and where none of those applies the stall names none but them. Well under
// a millisecond alone; no other test has a record run a cell that gives nothing.
#[test]
fn a_cell_the_record_runs_that_gives_no_result_neither_runs_during_the_wait_nor_is_named_its_giver() {
    let reader = cell("reader", &["tests"], &["notes"]);
    let mut tests = run_tests();
    tests.outputs = vec!["implement_tested".into()];
    let implement = asking(
        "tests",
        "implement_tested",
        cell("implement", &["background", "implement_tested"], &["code", "tests"]),
    );
    let mut w = World::with_gate(vec![implement, reader, tests]);
    for block in ["implement_tested", "tests", "notes", "gives"] {
        w.written_by_cells(block);
    }
    w.person_writes("background");
    asks_for_tests(&mut w);
    assert_eq!(
        reasons(&w.decide()),
        ("run tests".into(), vec![("tests".into(), false)])
    );
    let mut cells = w.flow.cells().to_vec();
    let place = w.flow.place(&"run tests".into()).expect("the flow has it");
    cells[place] = when("gives", "yes", cells[place].clone());
    w.install(cells);
    let skipped = Unrun::Skipped {
        on: vec![Condition::Is {
            block: "gives".into(),
            value: "yes".into(),
        }],
    };
    assert_eq!(
        w.decide(),
        stalled_for("implement", &[], &["implement_tested"], &[("run tests", skipped)])
    );
}

// A question of a cell that waits for its answer is waited for however the walk comes to the cell: the owner turning
// to it, at a precondition only cells make hold, leaves the work waiting for the answer, not stalled; and a result no
// cell that gives it applies to is no part of a wait, nothing being able to bring it, while the question is. Well under
// a millisecond alone; no other test turns to a cell that waits for an answer at a precondition.
#[test]
fn a_question_that_waits_is_waited_for_however_the_walk_comes_to_its_cell_and_a_result_none_can_give_is_not() {
    let answer = Waited::Answer {
        question: "which".to_string(),
    };
    let unmet = Waited::Precondition {
        on: Condition::Is {
            block: "ready".into(),
            value: "yes".into(),
        },
        by: vec![by_cell("plan")],
    };
    for turned in [false, true] {
        let mut w = unready(true);
        w.questions("implement", "which", true);
        if turned {
            w.outside_writes("implement_spoken");
        }
        let Decision::Wait { at, on } = w.decide() else {
            panic!("the owner turning to it {turned}: {:?}", w.decide());
        };
        assert_eq!(at.as_str(), "implement");
        assert!(
            on.len() == 2 && on.contains(&answer) && on.contains(&unmet),
            "the owner turning to it {turned}: {on:?}"
        );
    }
}

/// The implementation writes code and asks for tests, as one return dealing with all that ran it.
fn asks_for_tests(w: &mut World) -> Seq {
    w.start("implement");
    let ran = w.ran_by("implement");
    let ran: Vec<&str> = ran.iter().map(String::as_str).collect();
    w.finish_dealing("implement", &ran, &[("code", Matters), ("tests", Matters)])
}

// The owner's words the implementation was told before it asked for tests are no turning to it while it waits: the
// round that runs the tests is not stopped for them, and the run given the result takes the words up with it.
#[test]
fn words_a_waiting_cell_was_told_stop_no_round_and_come_back_with_its_result() {
    let mut w = testing();
    w.outside_writes("implement_spoken");
    w.start("implement");
    w.finish_dealing("implement", &[], &[("tests", Matters)]);
    assert_eq!(reasons(&w.decide()).0, "run tests");
    let run = w.start("run tests");
    w.person_writes("background");
    assert_eq!(
        w.decide(),
        Decision::Busy {
            cell: "run tests".into(),
            run
        }
    );
    w.finish("run tests", &[("implement_tested", Matters)]);
    assert_eq!(reasons(&w.decide()).0, "research");
    w.run("research", &[]);
    assert_eq!(
        reasons(&w.decide()),
        (
            "implement".into(),
            vec![
                ("plan".into(), false),
                ("implement_spoken".into(), false),
                ("implement_tested".into(), true)
            ]
        )
    );
}

// A cell that waits holds the work: the owner's words to a cell after it stop no round of the cell that gives the
// result, since the cell they turn to does not run until the wait is over.
#[test]
fn words_to_a_cell_after_a_waiting_one_stop_no_round_of_what_ends_the_wait() {
    let mut w = testing();
    w.person_writes("background");
    w.start("research");
    w.finish_dealing("research", &[], &[("tests", Matters)]);
    assert_eq!(reasons(&w.decide()).0, "run tests");
    let run = w.start("run tests");
    w.outside_writes("implement_spoken");
    assert_eq!(
        w.decide(),
        Decision::Busy {
            cell: "run tests".into(),
            run
        }
    );
}

/// The tests run for what was asked, and hand the implementation its result in.
fn tests_run(w: &mut World) {
    w.start("run tests");
    w.finish("run tests", &[("implement_tested", Matters)]);
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
    w.start("run tests");
    w.finish("run tests", &[]);
    // A run that gave nothing for an entry of a cell that applies leaves it waited for: the cell that runs the tests
    // is entered for it again, though nothing of the record is new to it, and the review still does not run.
    assert_eq!(
        reasons(&w.decide()),
        ("run tests".into(), vec![("tests".into(), false)])
    );
    // With no cell to give the result, and nothing that can make one give it, the work is stalled at the
    // implementation (decision a-work-no-cell-can-move-is-stalled).
    let mut held = testing();
    asks_for_tests(&mut held);
    let mut gated = held.flow.cells().to_vec();
    let place = held.flow.place(&"run tests".into()).expect("the flow has it");
    gated[place].applies = vec![Condition::Is {
        block: "never".into(),
        value: "yes".into(),
    }];
    held.install(gated);
    let never = Unrun::Skipped {
        on: vec![Condition::Is {
            block: "never".into(),
            value: "yes".into(),
        }],
    };
    assert_eq!(
        held.decide(),
        stalled_for("implement", &[], &["implement_tested"], &[("run tests", never)])
    );
}

#[test]
fn the_result_runs_the_asking_cell_alone_and_is_taken_by_the_run_given_it() {
    let mut w = testing();
    asks_for_tests(&mut w);
    tests_run(&mut w);
    assert_eq!(
        reasons(&w.decide()),
        ("implement".into(), vec![("implement_tested".into(), true)])
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

// A cell that asks for tests and skips itself in the same return has its entry put away: it holds nothing, the cell
// that runs the tests gives none for it, and once it applies again it waits for it again, where no run of that cell
// passed over it meanwhile.
#[test]
fn a_skipped_cells_entry_is_put_away_and_waits_again_as_the_cell_applies_again() {
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
    assert_eq!(asked_of(&w.flow, &w.records, &"run tests".into()), Vec::new());
    // Its entry holds nothing: the implementation runs on the plan.
    assert_eq!(reasons(&w.decide()).0, "implement");
    w.says("researching", true);
    assert_eq!(
        asked_of(&w.flow, &w.records, &"run tests".into())
            .iter()
            .map(|e| e.at)
            .collect::<Vec<_>>(),
        vec![entry]
    );
    assert_eq!(reasons(&w.decide()).0, "run tests");
}

// A run of the cell that runs the tests that passed over a skipped cell's entry, as it gave the results of the cells
// that applied, gave it no result: once the cell applies again it waits for it again, and the cell that runs the tests
// is entered for it, though nothing of the record is new to it.
#[test]
fn an_entry_the_tests_passed_over_while_its_cell_was_skipped_is_run_once_it_applies_again() {
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
    w.finish_dealing("research", &["background"], &[("tests", Matters)]);
    w.says("researching", false);
    w.run("implement", &[("code", Matters)]);
    assert_eq!(reasons(&w.decide()).0, "run tests");
    w.run("run tests", &[]);
    w.says("researching", true);
    assert_eq!(asked_of(&w.flow, &w.records, &"run tests".into()).len(), 1);
    let Decision::Run { cell, because, .. } = w.decide() else {
        panic!("the walk runs no cell: {:?}", w.decide());
    };
    assert_eq!(cell.as_str(), "run tests");
    let Some(Cause::Changed { block, was, behind }) = because.first() else {
        panic!("{because:?}");
    };
    assert_eq!((block.as_str(), was.is_some(), *behind), ("tests", true, false));
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
        ("implement".into(), vec![("implement_tested".into(), true)])
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
