//! What cannot have happened to the work is refused, and leaves the work exactly as it was.

use super::*;

#[test]
fn a_write_of_a_block_the_flow_does_not_have_is_refused() {
    let (mut w, ..) = W::requested();
    let why = w.refused(wrote("elsewhere", Outsider::Person, true, None));
    assert_eq!(why, Refused::NoSuchBlock("elsewhere".into()));
}

#[test]
fn a_persons_write_of_a_block_only_cells_write_is_refused() {
    let (mut w, ..) = W::requested();
    let why = w.refused(wrote("text", Outsider::Person, true, None));
    let not = Refused::NotTheirs {
        block: "text".into(),
        by: Outsider::Person,
    };
    assert_eq!(why, not);
}

#[test]
fn a_write_from_outside_of_a_block_only_people_write_is_refused() {
    let (mut w, ..) = W::requested();
    let why = w.refused(wrote("request", Outsider::Outside, true, None));
    let not = Refused::NotTheirs {
        block: "request".into(),
        by: Outsider::Outside,
    };
    assert_eq!(why, not);
}

#[test]
fn a_choice_written_without_a_value_is_refused() {
    let (mut w, ..) = W::requested();
    let why = w.refused(wrote("verdict", Outsider::Person, true, None));
    assert_eq!(
        why,
        Refused::NoValue {
            block: "verdict".into()
        }
    );
}

#[test]
fn a_choice_written_with_a_value_it_does_not_offer_is_refused() {
    let (mut w, ..) = W::requested();
    let why = w.refused(wrote("verdict", Outsider::Person, true, Some("maybe")));
    let refused = Refused::NotAValue {
        block: "verdict".into(),
        value: "maybe".into(),
    };
    assert_eq!(why, refused);
}

#[test]
fn prose_written_with_a_value_is_refused() {
    let (mut w, ..) = W::requested();
    let why = w.refused(wrote("request", Outsider::Person, true, Some("yes")));
    assert_eq!(
        why,
        Refused::NotAChoice {
            block: "request".into()
        }
    );
}

#[test]
fn letting_go_a_cell_the_flow_does_not_have_is_refused() {
    let (mut w, ..) = W::requested();
    assert_eq!(
        w.refused(Event::LetGo { cell: "other".into() }),
        Refused::NoSuchCell("other".into())
    );
}

#[test]
fn a_return_of_a_run_with_no_round_under_way_is_refused() {
    let (mut w, write, _) = W::requested();
    w.returns(write, Came::asks(vec![tool("search")]));
    // The return that asks ended the run.
    assert_eq!(
        w.refused(Event::Returned {
            run: write,
            came: wanting(),
            spent: Spent::default()
        }),
        Refused::NoRoundUnderWay(write)
    );
    let unknown = RunId(99);
    assert_eq!(
        w.refused(Event::Returned {
            run: unknown,
            came: wanting(),
            spent: Spent::default()
        }),
        Refused::NoRoundUnderWay(unknown)
    );
}

#[test]
fn a_return_of_a_run_that_finished_is_refused() {
    let (mut w, write, _) = W::requested();
    w.returns(write, hands_in(&[("text", None)]));
    let came = hands_in(&[("text", None)]);
    assert_eq!(
        w.refused(Event::Returned {
            run: write,
            came,
            spent: Spent::default(),
        }),
        Refused::NoRoundUnderWay(write)
    );
}

// What a carrier says it read of the branch is of the blocks a source outside the work writes: a read of a block that
// only people write, that only cells write or that the flow does not have is refused, and the work is left as it was; a
// read of one the outside writes is taken. Well under a millisecond alone; no other test holds what a return may say it
// read.
#[test]
fn a_return_that_read_a_block_no_source_outside_writes_is_refused() {
    let (mut w, write, _) = W::requested();
    let returning = |block: &str| {
        let mut came = hands_in(&[("text", None)]);
        came.read = vec![handed(block, None)];
        Event::Returned {
            run: write,
            came,
            spent: Spent::default(),
        }
    };
    for (block, why) in [
        (
            "request",
            Refused::NotTheirs {
                block: "request".into(),
                by: Outsider::Outside,
            },
        ),
        (
            "go",
            Refused::NotTheirs {
                block: "go".into(),
                by: Outsider::Outside,
            },
        ),
        (
            "text",
            Refused::NotTheirs {
                block: "text".into(),
                by: Outsider::Outside,
            },
        ),
        ("elsewhere", Refused::NoSuchBlock("elsewhere".into())),
    ] {
        assert_eq!(w.refused(returning(block)), why, "a read of {block}");
    }
    // One the outside writes is taken: the return stands.
    w.on(returning("main"));
}

#[test]
fn a_result_of_a_call_the_work_does_not_have_is_refused() {
    let (mut w, ..) = W::requested();
    let call = CallId(99);
    let event = Event::Done {
        call,
        result: "x".into(),
    };
    assert_eq!(w.refused(event), Refused::NoSuchCall(call));
}

#[test]
fn a_second_result_of_a_call_is_refused() {
    let (mut w, write, _) = W::requested();
    let effects = w.returns(write, Came::asks(vec![tool("search"), tool("read")]));
    let call = calls_of(&effects)[0].0;
    w.done(call);
    let event = Event::Done {
        call,
        result: "again".into(),
    };
    assert_eq!(w.refused(event), Refused::AlreadyDone(call));
}
