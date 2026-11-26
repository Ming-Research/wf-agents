//! How long a call the host could not carry out for now waits before it is asked for again: the longer of what the host
//! said and the wait of a round that failed in passing, and not before the time the host named.

use decide::Retry;
use decide::run::{FIRST_WAIT, HeldWait, LONGEST_WAIT, held_wait, wait};

#[test]
fn a_call_held_with_no_word_of_when_waits_as_a_round_that_failed_in_passing_does() {
    for held in 1..=10 {
        assert_eq!(
            held_wait(held, None),
            HeldWait {
                ms: wait(held),
                not_before: None
            }
        );
    }
    assert_eq!(held_wait(1, None).ms, FIRST_WAIT);
    assert_eq!(held_wait(30, None).ms, LONGEST_WAIT);
}

#[test]
fn a_call_held_for_so_many_seconds_waits_that_long_and_a_second_more_where_that_is_longer_than_the_backoff() {
    assert_eq!(
        held_wait(1, Some(Retry::In(3))).ms,
        FIRST_WAIT,
        "a short retry is the backoff"
    );
    assert_eq!(held_wait(1, Some(Retry::In(60))).ms, 61_000);
    assert_eq!(held_wait(1, Some(Retry::In(3600))).ms, 3_601_000);
    assert_eq!(held_wait(1, Some(Retry::In(60))).not_before, None);
}

#[test]
fn a_call_held_until_a_time_waits_not_before_it_and_a_second_more() {
    let wait = held_wait(2, Some(Retry::At(1_764_000_000)));
    assert_eq!(
        wait,
        HeldWait {
            ms: 2 * FIRST_WAIT,
            not_before: Some(1_764_000_001_000)
        }
    );
}
