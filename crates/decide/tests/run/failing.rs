//! A round that failed: nothing came back. The run has its next round, given what the failed one was given, which a
//! pause holds back as it holds back any round; and a failure counts against no limit.

use super::*;

/// A round that failed.
fn failed() -> Round {
    Round {
        returned: Some(Return::Failed),
    }
}

#[test]
fn a_run_whose_round_failed_has_that_round_again() {
    assert_eq!(step(vec![failed()], &spec()), Step::Retry);
    assert_eq!(step(vec![asked_done(1, 2), failed()], &spec()), Step::Retry);
}

#[test]
fn a_run_whose_round_failed_is_held_while_its_work_is_paused() {
    assert_eq!(step_paused(vec![failed()], &spec()), Step::Held);
}

#[test]
fn failed_rounds_count_against_no_limit() {
    // However many rounds failed, none was a return sent back nor made a call.
    let s = limited(1, 2);
    let failures = || vec![failed(), failed(), failed(), failed()];
    assert_eq!(step([failures(), vec![failed()]].concat(), &s), Step::Retry);
    assert_eq!(
        step([failures(), vec![wanting()]].concat(), &s),
        send_back(wanting_unmet())
    );
    assert_eq!(step([failures(), vec![asks(2)]].concat(), &s), Step::Make);
    let between = vec![wanting(), failed(), failed(), wanting()];
    assert_eq!(step(between, &s), returns_limit(1));
}
