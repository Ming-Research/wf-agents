//! What changes under a round: a run at rest goes on and what it was not told of stays new to its cell; a return that
//! would write over a version its round was never told of is sent back, the next round told of it.

use super::*;

#[test]
fn a_return_taken_after_what_ran_its_round_changed_deals_with_the_version_it_was_told_and_the_cell_runs_again() {
    let (mut w, write, first) = W::requested();
    // A person rewrites the request while the round is under way.
    let (effects, request) = w.person_writes("request");
    assert_eq!(effects, vec![]);
    w.returns(write, hands_in(&[("text", None)]));
    let finished = w.work.records.finished.last().expect("the run finished");
    assert_eq!(
        (finished.run, finished.dealt_with.clone()),
        (write, Some(vec!["request".into()]))
    );
    // The request it was not told of stays new to the writer, which runs again before the judge.
    let again = w.work.records.started.last().expect("a run").run;
    let text = w.work.records.finished[0].at;
    let start = started_with(
        vec![changed("request", Some(first), false)],
        &[
            ("request", Some(request)),
            ("opinions", None),
            ("main", None),
            ("text", Some(text)),
        ],
    );
    assert_eq!(start_of(&w, again), start);
    assert_eq!(w.work.records.started[1].cell.as_str(), "write");
}

#[test]
fn a_return_that_would_write_over_a_version_its_round_was_never_told_of_is_sent_back_told_of_it() {
    let (mut w, write, _) = W::requested();
    let judge = round(&w.returns(write, hands_in(&[("text", None)]))).0;
    // A person writes the verdict while the judge's round is under way.
    let (_, verdict) = w.on_and_place(wrote("verdict", Outsider::Person, true, Some("no")));
    let effects = w.returns(judge, hands_in(&[("verdict", Some("yes"))]));
    let unmet = vec![Unmet::WrittenOver {
        block: "verdict".into(),
        version: verdict,
    }];
    assert_eq!(effects, vec![round_effect(judge, "judge", 2, Given::SentBack(unmet))]);
    let put_in = w.work.records.put_in.last().expect("the version is put in");
    assert_eq!(
        (put_in.run, put_in.block.as_str(), put_in.version),
        (judge, "verdict", verdict)
    );
    // Told of it, the next return is taken.
    w.returns(judge, hands_in(&[("verdict", Some("yes"))]));
    assert!(w.work.records.finished.iter().any(|f| f.run == judge));
}
