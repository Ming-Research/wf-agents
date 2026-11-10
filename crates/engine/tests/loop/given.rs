//! What a first round is given: the cell's task, its run having started with exactly the reasons and the basis the
//! decision gave.

use super::*;

#[test]
fn a_task_is_given_with_its_run_started_for_exactly_why_the_cell_runs_and_from_what_it_starts_from() {
    let (mut w, write, request) = W::requested();
    let judge = round(&w.returns(write, hands_in(&[("text", None)]))).0;
    let text = w.last_finished();
    let effects = w.returns(judge, hands_in(&[("opinions", None), ("verdict", Some("no"))]));
    let opinions = w.last_finished();
    let again = w.work.records.started.last().expect("a run").run;
    // The judge, after the writer, gave the opinions their meaning.
    let start = started_with(
        vec![changed("opinions", None, true)],
        &[
            ("request", Some(request)),
            ("opinions", Some(opinions)),
            ("main", None),
            ("text", Some(text)),
        ],
    );
    assert_eq!(effects, vec![round_effect(again, "write", 1, Given::Task)]);
    assert_eq!(start_of(&w, again), start);
}
