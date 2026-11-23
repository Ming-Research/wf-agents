//! What a round committed to the work's branch: the carrier reads it as the round ends, and the run hands it in with
//! the round that ends its cell's work, never with a return that asks for calls (decision
//! a-version-read-at-a-rounds-end-is-that-rounds). Each test is one call of the run's rule on a few rounds, in well
//! under a millisecond.

use super::*;

/// A round whose return dealt with these, wrote nothing, asked for this many calls, and whose carrier read the work's
/// head as new: the round committed.
fn committed(dealt_with: &[&str], calls: usize) -> Round {
    Round {
        returned: Some(Return::Came {
            dealt_with: dealt_with.iter().map(|&b| b.into()).collect(),
            writes: vec![],
            calls,
            found: vec![],
            waits: false,
            questions: Vec::new(),
            read: vec!["head".into()],
            protected: Default::default(),
        }),
        sent_back: Vec::new(),
    }
}

#[test]
fn a_return_that_does_the_cells_work_hands_in_what_the_round_committed() {
    assert_eq!(
        step(vec![committed(&["request", "opinions"], 0)], &spec()),
        finish(vec![prose("head")])
    );
}

#[test]
fn a_return_that_asks_for_calls_hands_in_none_of_what_the_round_committed() {
    assert_eq!(step(vec![committed(&[], 2)], &spec()), finish_asking());
}

#[test]
fn a_return_that_asks_for_calls_after_one_sent_back_hands_in_none_of_what_the_round_committed() {
    let rounds = vec![wanting(), committed(&[], 1)];
    assert_eq!(step(rounds, &spec()), finish_asking());
}

// The round entered for the owner's words while its cell waits for the answer to a question answers and goes on
// waiting, and what it committed is the cell's: nothing is owed to it, so no other round holds the branch back, and the
// run hands in what the carrier read. Whether the carrier reads while a cell waits for the results of calls is its
// own (the tests of `branch`), and a round it reads nothing for has nothing to hand in.
#[test]
fn a_round_entered_while_its_cell_waits_for_an_answer_hands_in_what_it_committed() {
    let waiting = Spec {
        told: Told {
            waits: true,
            ..spec().told
        },
        ..spec()
    };
    let finishes = Step::Finish {
        dealt_with: vec!["request".into()],
        writes: vec![prose("head")],
    };
    assert_eq!(step(vec![committed(&["request"], 0)], &waiting), finishes);
}
