//! A return that meets everything else is asked whether, taken, it would leave the work unable to go on, which only the
//! walk can say (`decide::stuck_blocks`): where it would, and a block of its cell would end that, it is sent back
//! naming the blocks. The walk's answer is given to the run as a function of what the run would finish with.

use std::cell::RefCell;

use decide::run::Stuck;

use super::*;

/// What the check was asked, each time: what the run would finish having dealt with, and the blocks it would write.
type Asks = Vec<(Vec<BlockId>, Vec<Write>)>;

/// What the run does next on these rounds, the walk saying these blocks would let the work go on, whatever the run
/// would finish dealing with and writing; and what it was asked, each time.
fn step_asking(rounds: Vec<Round>, spec: &Spec, blocks: &[&str]) -> (Step, Asks) {
    let asked = RefCell::new(Vec::new());
    let stuck = |dealt_with: &[BlockId], writes: &[Write]| -> Vec<BlockId> {
        asked.borrow_mut().push((dealt_with.to_vec(), writes.to_vec()));
        blocks.iter().map(|&b| b.into()).collect()
    };
    let requires = Requires {
        stuck: Some(Stuck(&stuck)),
        ..spec.requires()
    };
    let log = Log {
        rounds,
        ..Log::default()
    };
    let step = next(&log, requires, Hold::Nothing);
    (step, asked.into_inner())
}

/// What the run does next on these rounds, the walk saying the work is stuck unless the run writes this block.
fn step_unless_written(rounds: Vec<Round>, spec: &Spec, block: &str) -> Step {
    let stuck = |_: &[BlockId], writes: &[Write]| -> Vec<BlockId> {
        match writes.iter().any(|w| w.block.as_str() == block) {
            true => Vec::new(),
            false => vec![block.into()],
        }
    };
    let requires = Requires {
        stuck: Some(Stuck(&stuck)),
        ..spec.requires()
    };
    let log = Log {
        rounds,
        ..Log::default()
    };
    next(&log, requires, Hold::Nothing)
}

fn would_stall(blocks: &[&str]) -> Unmet {
    Unmet::WouldStall {
        blocks: blocks.iter().map(|&b| b.into()).collect(),
    }
}

/// A round whose return did the cell's work, asked one question, which waits for the owner's answer or not, and read
/// these blocks new off the branch.
fn reading(blocks: &[&str]) -> Round {
    Round {
        returned: Some(Return::Came {
            dealt_with: vec!["request".into(), "opinions".into()],
            writes: vec![],
            calls: 0,
            found: vec![],
            waits: false,
            questions: Vec::new(),
            read: blocks.iter().map(|&b| b.into()).collect(),
            protected: Default::default(),
        }),
        sent_back: Vec::new(),
    }
}

#[test]
fn a_return_that_would_leave_the_work_stuck_is_sent_back_naming_the_blocks_that_would_end_it() {
    let (step, asked) = step_asking(vec![handed_in(vec![valued("verdict", "no")])], &spec(), &["plan"]);
    assert_eq!(step, send_back(vec![would_stall(&["plan"])]));
    // It was asked what the run would finish with: what the return dealt with that ran the round, and what it wrote.
    assert_eq!(
        asked,
        [(
            vec![BlockId::from("request"), BlockId::from("opinions")],
            vec![valued("verdict", "no")]
        )]
    );
}

#[test]
fn a_return_that_would_leave_the_work_able_to_go_on_is_taken() {
    let (step, asked) = step_asking(vec![handed_in(vec![prose("plan")])], &spec(), &[]);
    assert_eq!(step, finish(vec![prose("plan")]));
    assert_eq!(asked.len(), 1);
}

#[test]
fn what_a_round_committed_is_among_what_the_check_is_told_the_run_would_write() {
    let (step, asked) = step_asking(vec![reading(&["plan"])], &spec(), &[]);
    assert_eq!(step, finish(vec![prose("plan")]));
    assert_eq!(asked[0].1, [prose("plan")]);
}

#[test]
fn a_return_that_only_hands_in_a_commit_is_taken_where_the_block_it_commits_ends_the_stall() {
    // The check answers from what the run would write: without the block the work is stuck, with it it is not.
    assert_eq!(
        step_unless_written(vec![handed_in(vec![])], &spec(), "plan"),
        send_back(vec![would_stall(&["plan"])])
    );
    assert_eq!(
        step_unless_written(vec![reading(&["plan"])], &spec(), "plan"),
        finish(vec![prose("plan")])
    );
}

#[test]
fn a_return_sent_back_for_stalling_counts_against_the_limit_of_returns_sent_back() {
    let stuck = Round {
        sent_back: vec![would_stall(&["plan"])],
        ..handed_in(vec![])
    };
    let rounds = vec![stuck.clone(), stuck.clone(), handed_in(vec![])];
    let (step, _) = step_asking(rounds, &spec(), &["plan"]);
    assert_eq!(step, returns_limit(2));
}

#[test]
fn a_return_not_held_to_what_its_cell_must_hand_in_is_not_asked_whether_it_would_stall() {
    // Asks for calls: the results of the calls enter the cell again.
    let (step, asked) = step_asking(vec![asks(1)], &spec(), &["plan"]);
    assert_eq!(step, finish_asking());
    assert!(asked.is_empty());
    // Asks a question, whether it waits for the answer or not: the answer may run what nothing else would.
    for waits in [true, false] {
        let rounds = vec![asks_beside(&["request", "opinions"], vec![prose("plan")], waits)];
        let (step, asked) = step_asking(rounds, &spec(), &["plan"]);
        assert!(matches!(step, Step::Finish { .. }), "{step:?}");
        assert!(asked.is_empty(), "waits {waits}");
    }
    // Its cell still waits for the results of calls, or was told to hand in what it has.
    for told in [
        Told {
            waits: true,
            ..spec().told
        },
        Told {
            hurried: true,
            ..spec().told
        },
    ] {
        let s = Spec { told, ..spec() };
        let (step, asked) = step_asking(vec![handed_in(vec![prose("plan")])], &s, &["plan"]);
        assert!(matches!(step, Step::Finish { .. }), "{step:?}");
        assert!(asked.is_empty());
    }
}

#[test]
fn a_return_sent_back_for_something_else_is_not_asked_whether_it_would_stall() {
    let s = must_write(&["verdict"]);
    let (step, asked) = step_asking(vec![handed_in(vec![prose("plan")])], &s, &["plan"]);
    assert_eq!(
        step,
        send_back(vec![Unmet::NotHandedIn {
            block: "verdict".into()
        }])
    );
    assert!(asked.is_empty());
}

#[test]
fn a_cell_given_no_such_check_takes_its_return_as_it_did() {
    assert_eq!(step(vec![handed_in(vec![])], &spec()), finish(vec![]));
}
