//! Handing in: what meets everything finishes the run, and each way in which it does not is said.

use super::*;

#[test]
fn a_return_that_meets_everything_finishes_the_run_with_exactly_its_writes() {
    let writes = vec![prose("plan"), valued("verdict", "no")];
    assert_eq!(step(vec![handed_in(writes.clone())], &spec()), finish(writes));
}

#[test]
fn a_return_that_writes_nothing_meets_everything() {
    assert_eq!(step(vec![handed_in(vec![])], &spec()), finish(vec![]));
}

#[test]
fn a_return_that_lacks_a_block_the_cell_must_hand_in_is_sent_back_naming_each_block_it_lacks() {
    let s = must_write(&["verdict", "plan"]);
    let unmet = vec![Unmet::NotHandedIn {
        block: "verdict".into(),
    }];
    assert_eq!(step(vec![handed_in(vec![prose("plan")])], &s), send_back(unmet));
    let unmet = vec![
        Unmet::NotHandedIn {
            block: "verdict".into(),
        },
        Unmet::NotHandedIn { block: "plan".into() },
    ];
    assert_eq!(step(vec![handed_in(vec![])], &s), send_back(unmet));
}

#[test]
fn a_return_with_every_block_the_cell_must_hand_in_finishes_the_run() {
    let writes = vec![valued("verdict", "yes"), prose("plan")];
    assert_eq!(
        step(vec![handed_in(writes.clone())], &must_write(&["plan", "verdict"])),
        finish(writes)
    );
}

#[test]
fn a_round_entered_for_the_owner_while_its_cell_waits_is_held_to_no_block_its_cell_must_write() {
    let mut s = must_write(&["verdict", "plan"]);
    s.told.waits = true;
    assert_eq!(step(vec![handed_in(vec![])], &s), finish(vec![]));
    // What it hands in is still checked.
    let unmet = vec![Unmet::NoValue {
        block: "verdict".into(),
    }];
    assert_eq!(step(vec![handed_in(vec![prose("verdict")])], &s), send_back(unmet));
}

#[test]
fn a_block_the_cell_must_hand_in_handed_in_wrongly_is_said_only_to_be_wrong() {
    let unmet = vec![Unmet::NoValue {
        block: "verdict".into(),
    }];
    assert_eq!(
        step(vec![handed_in(vec![prose("verdict")])], &must_write(&["verdict"])),
        send_back(unmet)
    );
}

#[test]
fn what_a_return_lacks_is_said_after_what_its_writes_do_not_meet() {
    let unmet = vec![
        Unmet::NotAnOutput { block: "code".into() },
        Unmet::NotHandedIn {
            block: "verdict".into(),
        },
    ];
    assert_eq!(
        step(vec![handed_in(vec![prose("code")])], &must_write(&["verdict"])),
        send_back(unmet)
    );
}

#[test]
fn a_return_that_deals_with_only_part_of_what_ran_its_round_is_still_held_to_what_the_cell_must_hand_in() {
    let s = must_write(&["verdict"]);
    let partial = works(&["opinions"], vec![prose("plan")]);
    let unmet = vec![Unmet::NotHandedIn {
        block: "verdict".into(),
    }];
    assert_eq!(step(vec![partial], &s), send_back(unmet));
    let finish = Step::Finish {
        dealt_with: vec!["opinions".into()],
        writes: vec![prose("plan"), valued("verdict", "yes")],
    };
    let partial = works(&["opinions"], vec![prose("plan"), valued("verdict", "yes")]);
    assert_eq!(step(vec![partial], &s), finish);
}

/// The same cell, which must hand in these blocks each time, and asks for tests in `tests`.
fn asking_for_tests(blocks: &[&str]) -> Spec {
    let mut s = must_write(blocks);
    s.outputs.push("tests".into());
    s.asking = vec![Asks {
        record: "tests".into(),
        result: "tested".into(),
    }];
    s
}

// Each entry of a record is new: a return that writes an entry of a record as changing nothing in meaning is sent back,
// whatever the record, so that the run's check, which reads it as leaving its cell waiting, and the walk, which would
// read it as no entry, never read it two ways. Well under a millisecond alone; no other test holds it.
#[test]
fn an_entry_written_as_changing_nothing_in_meaning_is_sent_back() {
    let mut s = asking_for_tests(&[]);
    let record = Standing {
        told: None,
        latest: None,
        value: None,
        added: true,
    };
    s.told.outputs.insert("tests".into(), record);
    let entry = Write {
        meaning_unchanged: true,
        ..prose("tests")
    };
    let unmet = vec![Unmet::EntryUnchanged { block: "tests".into() }];
    assert_eq!(step(vec![works(&[], vec![entry])], &s), send_back(unmet));
    assert!(matches!(
        step(vec![works(&[], vec![prose("tests")])], &s),
        Step::Finish { .. }
    ));
}

// A block that would leave the cell waiting, the record it asks for tests in or the record of its questions, written in
// another shape than its own leaves it unknown whether the return waits: it is sent back for the shape alone, not also
// for dealing with nothing, for a block its cell must write or for an act's words it leaves unanswered, all of which
// a return that waits may do. Well under a millisecond alone; no other test holds a check to what depends on another.
#[test]
fn a_block_that_would_leave_its_cell_waiting_written_in_another_shape_is_sent_back_for_that_alone() {
    let mut s = asking_for_tests(&["verdict"]);
    s.outputs.push("questions".into());
    s.questions = vec!["questions".into()];
    s.answered_by = BTreeMap::from([(
        "request".into(),
        Answer {
            runs: vec!["plan".into()],
            question: false,
            new: false,
        },
    )]);
    for block in ["tests", "questions"] {
        let unmet = vec![Unmet::NotOfItsShape { block: block.into() }];
        let round = came(&[], vec![], unmet.clone(), false);
        assert_eq!(step(vec![round], &s), send_back(unmet), "{block}");
    }
    // Dealing with what an act wrote, and answering it with nothing, is no fault while it is not known to wait.
    let unmet = vec![Unmet::NotOfItsShape { block: "tests".into() }];
    let round = came(&["request"], vec![], unmet.clone(), false);
    assert_eq!(step(vec![round], &s), send_back(unmet));
    // A block of a shape that would not leave it waiting leaves the other checks as they were.
    let unmet = vec![Unmet::NotOfItsShape { block: "plan".into() }];
    let round = came(&["request"], vec![], unmet.clone(), false);
    let mut expected = unmet;
    expected.push(Unmet::NotHandedIn {
        block: "verdict".into(),
    });
    expected.push(Unmet::Unanswered {
        block: "request".into(),
    });
    assert_eq!(step(vec![round], &s), send_back(expected));
}

// A return that writes an entry in which its cell asks for tests leaves the cell waiting for the result, and so owes
// nothing else: it may leave out what its cell must write, and deal with nothing (decision a-wait-holds-its-cell).
#[test]
fn a_return_that_asks_for_tests_leaves_its_cell_waiting_and_owes_nothing_else() {
    let s = asking_for_tests(&["verdict", "plan"]);
    let finish = Step::Finish {
        dealt_with: vec![],
        writes: vec![prose("tests")],
    };
    assert_eq!(step(vec![works(&[], vec![prose("tests")])], &s), finish);
    // One that asks for nothing is held to them as before.
    let unmet = vec![
        Unmet::DealtWithNothing,
        Unmet::NotHandedIn {
            block: "verdict".into(),
        },
        Unmet::NotHandedIn { block: "plan".into() },
    ];
    assert_eq!(step(vec![works(&[], vec![])], &s), send_back(unmet));
}

// What an act after the cell said to it and the cell dealt with must leave something new that runs the act again; a
// return that leaves the cell waiting owes nothing of it either, as it comes back to the cell once its result is in.
#[test]
fn a_return_that_asks_for_tests_owes_no_answer_to_an_act_s_block_it_dealt_with() {
    let mut s = asking_for_tests(&[]);
    s.told.ran_by = vec!["check".into()];
    s.answered_by = BTreeMap::from([(
        "check".into(),
        Answer {
            runs: vec!["head".into()],
            question: false,
            new: false,
        },
    )]);
    let asks = works(&["check"], vec![prose("tests")]);
    let finish = Step::Finish {
        dealt_with: vec!["check".into()],
        writes: vec![prose("tests")],
    };
    assert_eq!(step(vec![asks], &s), finish);
    let unanswered = vec![Unmet::Unanswered { block: "check".into() }];
    assert_eq!(step(vec![works(&["check"], vec![])], &s), send_back(unanswered));
}

#[test]
fn a_return_with_a_question_that_waits_is_held_to_no_block_its_cell_must_write_and_may_deal_with_nothing() {
    let s = must_write(&["verdict", "plan"]);
    let finish = Step::Finish {
        dealt_with: vec![],
        writes: vec![],
    };
    assert_eq!(step(vec![came(&[], vec![], vec![], true)], &s), finish);
    // What it hands in is still checked.
    let unmet = vec![Unmet::NoValue {
        block: "verdict".into(),
    }];
    assert_eq!(
        step(vec![came(&["request"], vec![prose("verdict")], vec![], true)], &s),
        send_back(unmet)
    );
}

/// A round whose return asked a question that waits or none, wrote these, and, as it wrote them, would leave its cell
/// no longer applying by these.
fn skipping(waits: bool, writes: Vec<Write>, skips: &[&str]) -> Round {
    Round {
        returned: Some(Return::Came {
            dealt_with: vec!["request".into(), "opinions".into()],
            writes,
            calls: 0,
            found: Vec::new(),
            waits,
            questions: Vec::new(),
            read: Vec::new(),
            skips: skips.iter().map(|&b| b.into()).collect(),
        }),
    }
}

// A return that asks to wait, by a question that waits or by an entry asking for tests, and whose writes leave its own
// cell no longer applying, is sent back naming each block that does: the wait of a cell that does not apply would hold
// nothing (decision a-return-that-waits-keeps-its-cell). The research that waits on a question and writes that the
// deep research is needed is the case. Well under a millisecond alone; no other test holds the run's check to it.
#[test]
fn a_return_that_asks_to_wait_and_leaves_its_own_cell_is_sent_back_naming_what_leaves_it() {
    let s = asking_for_tests(&["verdict"]);
    let skips = vec![Unmet::SkipsItsWait {
        block: "verdict".into(),
    }];
    let asks = skipping(true, vec![valued("verdict", "no")], &["verdict"]);
    assert_eq!(step(vec![asks], &s), send_back(skips.clone()));
    let tests = skipping(false, vec![prose("tests"), valued("verdict", "no")], &["verdict"]);
    assert_eq!(step(vec![tests], &s), send_back(skips));
    // Leaving its cell as it waits on nothing, or waiting as it leaves nothing, is no fault.
    let leaves = skipping(false, vec![valued("verdict", "no")], &["verdict"]);
    assert!(matches!(step(vec![leaves], &s), Step::Finish { .. }));
    let waits = skipping(true, vec![valued("verdict", "yes")], &[]);
    assert!(matches!(step(vec![waits], &s), Step::Finish { .. }));
}

// A round entered for the owner while its cell waits, asking nothing itself, may leave its cell: what the owner said may
// have turned it, and the earlier wait is put away as a skipped cell's is (decision a-return-that-waits-keeps-its-cell).
// Well under a millisecond alone.
#[test]
fn a_round_entered_for_the_owner_while_its_cell_waits_may_leave_its_cell() {
    let mut s = asking_for_tests(&["verdict"]);
    s.told.waits = true;
    let leaves = skipping(false, vec![valued("verdict", "no")], &["verdict"]);
    assert!(matches!(step(vec![leaves], &s), Step::Finish { .. }));
    // One that asks to wait anew is held to the rule as any other.
    let asks = skipping(true, vec![valued("verdict", "no")], &["verdict"]);
    let skips = vec![Unmet::SkipsItsWait {
        block: "verdict".into(),
    }];
    assert_eq!(step(vec![asks], &s), send_back(skips));
}

#[test]
fn what_was_found_as_a_return_was_read_is_said_first_with_everything_else_wrong_with_it() {
    let found = vec![
        Unmet::NotATrigger {
            name: "on_nothing".to_string(),
        },
        Unmet::UnchangedNotWritten {
            block: "verdict".into(),
        },
    ];
    let round = came(&["request", "plan"], vec![prose("code")], found.clone(), false);
    let mut unmet = found;
    unmet.extend([
        Unmet::NotRunBy { block: "plan".into() },
        Unmet::NotAnOutput { block: "code".into() },
        Unmet::NotHandedIn {
            block: "verdict".into(),
        },
    ]);
    assert_eq!(step(vec![round], &must_write(&["verdict"])), send_back(unmet));
    // What was found alone sends a return back that meets everything else.
    let found = vec![Unmet::NotATool {
        name: "browse".to_string(),
    }];
    let round = came(&["request", "opinions"], vec![], found.clone(), false);
    assert_eq!(step(vec![round], &spec()), send_back(found));
}

#[test]
fn a_return_that_names_as_dealt_with_what_did_not_run_its_round_or_names_a_block_twice_is_sent_back() {
    let unmet = vec![
        Unmet::NotRunBy { block: "plan".into() },
        Unmet::DealtWithTwice {
            block: "request".into(),
        },
    ];
    let round = works(&["request", "plan", "request"], vec![]);
    assert_eq!(step(vec![round], &spec()), send_back(unmet));
}

/// The cell, run besides by a trigger on the group `planned`, of the opinions and the acceptance, which a return names
/// by the group's name: the opinions ran its round, and the acceptance did not.
fn grouped() -> Spec {
    let group = vec!["acceptance".into(), "opinions".into()];
    Spec {
        groups: BTreeMap::from([("planned".into(), group)]),
        ..spec()
    }
}

#[test]
fn a_return_that_names_a_group_s_trigger_deals_with_each_of_its_blocks_that_ran_the_round_as_one_trigger() {
    let s = grouped();
    let finish = |dealt_with: &[&str]| Step::Finish {
        dealt_with: dealt_with.iter().map(|&b| b.into()).collect(),
        writes: vec![prose("plan")],
    };
    // The group ran the round by one of its blocks: dealing with it deals with that block, and not with the one that
    // did not run it.
    assert_eq!(
        step(vec![works(&["planned"], vec![prose("plan")])], &s),
        finish(&["opinions"])
    );
    // Named beside a trigger of one of its blocks, each block is dealt with once.
    let round = works(&["opinions", "planned", "request"], vec![prose("plan")]);
    assert_eq!(step(vec![round], &s), finish(&["opinions", "request"]));
    // Named twice, it is said so, once.
    let unmet = vec![Unmet::DealtWithTwice {
        block: "planned".into(),
    }];
    let round = works(&["planned", "request", "planned"], vec![prose("plan")]);
    assert_eq!(step(vec![round], &s), send_back(unmet));
}

#[test]
fn a_group_s_trigger_none_of_whose_blocks_ran_the_round_is_said_not_to_have_run_it() {
    let mut s = grouped();
    s.told.ran_by = vec!["request".into()];
    let unmet = vec![Unmet::NotRunBy {
        block: "planned".into(),
    }];
    let round = works(&["planned", "request"], vec![prose("plan")]);
    assert_eq!(step(vec![round], &s), send_back(unmet));
    // Run by the acceptance alone, the group ran it.
    s.told.ran_by = vec!["acceptance".into()];
    let finish = Step::Finish {
        dealt_with: vec!["acceptance".into()],
        writes: vec![prose("plan")],
    };
    assert_eq!(step(vec![works(&["planned"], vec![prose("plan")])], &s), finish);
}

#[test]
fn a_return_that_writes_over_a_version_its_round_was_not_told_of_is_sent_back_and_one_that_adds_an_entry_is_not() {
    let mut s = spec();
    s.told.outputs.get_mut(&BlockId::from("plan")).expect("the plan").latest = Some(Seq(9));
    let unmet = vec![Unmet::WrittenOver {
        block: "plan".into(),
        version: Seq(9),
    }];
    assert_eq!(step(vec![handed_in(vec![prose("plan")])], &s), send_back(unmet));
    // A block it does not write it may leave as another wrote it.
    assert_eq!(
        step(vec![handed_in(vec![valued("verdict", "no")])], &s),
        finish(vec![valued("verdict", "no")])
    );
    // Of a record entries are added to, a version another added since replaces nothing, and the return's entry is
    // taken beside it.
    s.told.outputs.get_mut(&BlockId::from("plan")).expect("the plan").added = true;
    assert_eq!(
        step(vec![handed_in(vec![prose("plan")])], &s),
        finish(vec![prose("plan")])
    );
}

#[test]
fn a_choice_handed_in_as_changing_nothing_in_meaning_with_another_value_is_sent_back() {
    let unchanged = |value: &str| Write {
        meaning_unchanged: true,
        empty: false,
        ..valued("verdict", value)
    };
    let unmet = vec![Unmet::ChoiceChanged {
        block: "verdict".into(),
    }];
    assert_eq!(step(vec![handed_in(vec![unchanged("no")])], &spec()), send_back(unmet));
    assert_eq!(
        step(vec![handed_in(vec![unchanged("yes")])], &spec()),
        finish(vec![unchanged("yes")])
    );
}

#[test]
fn a_return_sent_back_for_what_it_lacks_counts_against_the_limit_of_returns() {
    let lacking = || handed_in(vec![prose("plan")]);
    assert_eq!(
        step(vec![lacking(), lacking(), lacking()], &must_write(&["verdict"])),
        returns_limit(2)
    );
}

#[test]
fn a_block_that_is_not_an_output_is_sent_back() {
    let unmet = vec![Unmet::NotAnOutput { block: "code".into() }];
    assert_eq!(step(vec![handed_in(vec![prose("code")])], &spec()), send_back(unmet));
}

#[test]
fn a_block_written_twice_in_one_return_is_sent_back() {
    let unmet = vec![Unmet::WrittenTwice { block: "plan".into() }];
    assert_eq!(
        step(vec![handed_in(vec![prose("plan"), prose("plan")])], &spec()),
        send_back(unmet)
    );
}

#[test]
fn a_choice_written_with_no_value_is_sent_back() {
    let unmet = vec![Unmet::NoValue {
        block: "verdict".into(),
    }];
    assert_eq!(step(vec![handed_in(vec![prose("verdict")])], &spec()), send_back(unmet));
}

#[test]
fn a_choice_written_with_a_value_it_does_not_offer_is_sent_back() {
    let unmet = vec![Unmet::NotAValue {
        block: "verdict".into(),
        value: "maybe".into(),
    }];
    assert_eq!(
        step(vec![handed_in(vec![valued("verdict", "maybe")])], &spec()),
        send_back(unmet)
    );
}

#[test]
fn a_block_that_is_no_choice_written_with_a_value_is_sent_back() {
    assert_eq!(step(vec![wanting()], &spec()), send_back(wanting_unmet()));
}

#[test]
fn every_way_a_return_does_not_meet_what_is_required_is_said_at_once_in_the_order_of_the_writes() {
    let writes = vec![
        valued("verdict", "maybe"),
        prose("code"),
        valued("plan", "yes"),
        prose("plan"),
    ];
    let unmet = vec![
        Unmet::NotAValue {
            block: "verdict".into(),
            value: "maybe".into(),
        },
        Unmet::NotAnOutput { block: "code".into() },
        Unmet::NotAChoice { block: "plan".into() },
        Unmet::WrittenTwice { block: "plan".into() },
    ];
    assert_eq!(step(vec![handed_in(writes)], &spec()), send_back(unmet));
}

#[test]
fn a_block_that_is_not_an_output_and_is_written_twice_is_said_to_be_both() {
    let unmet = vec![
        Unmet::NotAnOutput { block: "code".into() },
        Unmet::WrittenTwice { block: "code".into() },
    ];
    assert_eq!(
        step(vec![handed_in(vec![prose("code"), prose("code")])], &spec()),
        send_back(unmet)
    );
}

#[test]
fn a_choice_of_a_block_the_cell_may_not_write_is_said_only_not_to_be_an_output() {
    let mut s = spec();
    s.choices.insert("other".into(), vec!["yes".into()]);
    let unmet = vec![Unmet::NotAnOutput { block: "other".into() }];
    assert_eq!(
        step(vec![handed_in(vec![valued("other", "yes")])], &s),
        send_back(unmet)
    );
}

#[test]
fn a_return_handed_in_after_one_sent_back_is_judged_on_its_own() {
    let rounds = vec![wanting(), handed_in(vec![prose("plan")])];
    assert_eq!(step(rounds, &spec()), finish(vec![prose("plan")]));
}

#[test]
fn a_block_written_three_times_is_said_to_be_written_twice_at_each_later_write() {
    let unmet = vec![
        Unmet::WrittenTwice { block: "plan".into() },
        Unmet::WrittenTwice { block: "plan".into() },
    ];
    let writes = vec![prose("plan"), prose("plan"), prose("plan")];
    assert_eq!(step(vec![handed_in(writes)], &spec()), send_back(unmet));
}

#[test]
fn a_block_the_cell_must_hand_in_found_written_in_a_shape_it_does_not_take_is_said_wrong_once_for_its_shape() {
    let found = vec![Unmet::NoValue {
        block: "verdict".into(),
    }];
    let round = came(&["request", "opinions"], vec![], found.clone(), false);
    assert_eq!(step(vec![round], &must_write(&["verdict"])), send_back(found));
}
