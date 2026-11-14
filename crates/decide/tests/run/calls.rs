//! Calls: a return that asks for calls ends the run, dealing with nothing; one that mixes calls with the cell's work is
//! sent back; and the waits the engine makes before a round is tried again are rested at.

use super::*;

#[test]
fn a_return_that_asks_for_calls_finishes_the_run_having_dealt_with_nothing() {
    assert_eq!(step(vec![asks(1)], &spec()), finish_asking());
    assert_eq!(step(vec![asks(5)], &spec()), finish_asking());
}

#[test]
fn a_return_that_asks_for_calls_is_not_held_to_what_the_cell_must_hand_in() {
    assert_eq!(step(vec![asks(2)], &must_write(&["verdict"])), finish_asking());
}

#[test]
fn a_return_that_asks_for_calls_and_deals_with_something_or_writes_a_block_is_sent_back() {
    let dealing = Round {
        returned: Some(Return::Came {
            dealt_with: vec!["request".into()],
            writes: vec![],
            calls: 1,
            found: vec![],
            waits: false,
        }),
    };
    assert_eq!(step(vec![dealing], &spec()), send_back(vec![Unmet::AsksAndWorks]));
    let writing = Round {
        returned: Some(Return::Came {
            dealt_with: vec![],
            writes: vec![prose("plan")],
            calls: 2,
            found: vec![],
            waits: false,
        }),
    };
    assert_eq!(step(vec![writing], &spec()), send_back(vec![Unmet::AsksAndWorks]));
}

#[test]
fn a_return_that_asks_for_calls_after_returns_were_sent_back_finishes_the_run() {
    // Three returns were not met: the third reached the limit of two, and the owner let the run go on.
    let rounds = vec![wanting(), wanting(), wanting(), asks(1)];
    assert_eq!(step_let_go(rounds, 3, &spec()), finish_asking());
}

#[test]
fn a_run_rests_at_the_waits_the_engine_made_that_have_no_result_in_the_order_they_were_made() {
    let rounds = vec![waited_after(Failure::Passing, &[(9, false), (3, false)])];
    let rest = Step::Rest {
        calls: vec![CallId(9), CallId(3)],
    };
    assert_eq!(step(rounds, &spec()), rest);
}
