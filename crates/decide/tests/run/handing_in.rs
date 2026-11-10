//! Handing in: what meets everything finishes the run, and each way in which it does not is said.

use super::*;

#[test]
fn a_return_that_meets_everything_finishes_the_run_with_exactly_its_writes() {
    let writes = vec![prose("plan"), valued("verdict", "no")];
    let finish = Step::Finish { writes: writes.clone() };
    assert_eq!(step(vec![handed_in(writes)], &spec()), finish);
}

#[test]
fn a_return_that_writes_nothing_meets_everything() {
    assert_eq!(step(vec![handed_in(vec![])], &spec()), Step::Finish { writes: vec![] });
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
    let finish = Step::Finish { writes: writes.clone() };
    assert_eq!(step(vec![handed_in(writes)], &must_write(&["plan", "verdict"])), finish);
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
fn a_return_that_asks_for_calls_hands_in_nothing_and_is_not_held_to_what_the_cell_must_hand_in() {
    assert_eq!(step(vec![asks(2)], &must_write(&["verdict"])), Step::Make);
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
fn a_return_handed_in_after_calls_is_judged_on_its_own() {
    let rounds = vec![asked_done(1, 2), handed_in(vec![prose("plan")])];
    assert_eq!(
        step(rounds, &spec()),
        Step::Finish {
            writes: vec![prose("plan")]
        }
    );
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
