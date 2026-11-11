//! A run started for what a person wrote, while its work is paused: it has every round it needs, but the round after
//! one that failed, which waits for the work to be let on as it does in any run.

use super::*;

#[test]
fn a_paused_run_for_what_a_person_wrote_begins_resumes_and_is_sent_back() {
    let s = spec();
    assert_eq!(step_paused_answering(vec![], &s), Step::Begin);
    assert_eq!(step_paused_answering(vec![asked_done(1, 2)], &s), Step::Resume);
    assert_eq!(step_paused_answering(vec![wanting()], &s), send_back(wanting_unmet()));
    assert_eq!(
        step_paused_answering(vec![asks_nothing()], &s),
        send_back(vec![Unmet::AsksNothing])
    );
}

#[test]
fn a_paused_run_for_what_a_person_wrote_whose_round_failed_is_held() {
    let failed = failed_as(Failure::Other);
    assert_eq!(step_paused_answering(vec![failed], &spec()), Step::Held);
}

#[test]
fn a_paused_run_for_what_a_person_wrote_does_what_begins_no_round_as_any_run_does() {
    let s = spec();
    assert_eq!(step_paused_answering(vec![under_way()], &s), Step::UnderWay);
    assert_eq!(step_paused_answering(vec![asks(2)], &s), Step::Make);
    let finish = Step::Finish {
        writes: vec![prose("plan")],
    };
    assert_eq!(step_paused_answering(vec![handed_in(vec![prose("plan")])], &s), finish);
    assert_eq!(
        step_paused_answering(vec![wanting(), wanting(), wanting()], &s),
        returns_limit(2)
    );
}
