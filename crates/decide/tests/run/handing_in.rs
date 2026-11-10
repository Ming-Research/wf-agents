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
