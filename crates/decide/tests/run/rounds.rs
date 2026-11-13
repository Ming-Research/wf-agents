//! Rounds: the first, and one under way.

use super::*;

#[test]
fn a_run_with_no_round_yet_has_its_first_round() {
    assert_eq!(step(vec![], &spec()), Step::Begin);
}

#[test]
fn while_a_round_is_under_way_nothing_is_decided() {
    assert_eq!(step(vec![under_way()], &spec()), Step::UnderWay);
    assert_eq!(step(vec![wanting(), under_way()], &spec()), Step::UnderWay);
}

#[test]
fn a_return_that_deals_with_nothing_writes_nothing_and_asks_for_nothing_is_sent_back() {
    assert_eq!(
        step(vec![asks_nothing()], &spec()),
        send_back(vec![Unmet::DealtWithNothing])
    );
}

#[test]
fn a_return_of_a_round_run_by_nothing_new_may_deal_with_nothing() {
    // A run entered only for the results of its calls was run by no block.
    let mut s = spec();
    s.told.ran_by.clear();
    let finish = Step::Finish {
        dealt_with: vec![],
        writes: vec![prose("plan")],
    };
    assert_eq!(step(vec![works(&[], vec![prose("plan")])], &s), finish);
    assert_eq!(step(vec![asks_nothing()], &s), finish_asking());
}
