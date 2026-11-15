//! A cell whose run asked for calls (design document `data-and-context`, 28): the return dealt with nothing, the cell
//! waits until every result is in, and is then entered again as a new run, for the results and what is still new.

use super::*;

#[test]
fn a_cell_that_asked_for_calls_waits_until_every_result_is_in_and_is_then_entered_for_them() {
    let mut w = World::with_gate(vec![
        cell("research", &["brief"], &["plan"]),
        cell("implement", &["plan"], &["code"]),
    ]);
    let brief = w.person_writes("brief");
    w.start("research");
    // The return asks for two calls, one the owner answers.
    let run = w.open("research");
    w.returns("research");
    let tool = CallId(90);
    let question = CallId(91);
    for (call, on) in [(tool, None), (question, Some(Who::Owner))] {
        let at = w.tick();
        w.records.called.push(decide::Called { call, run, on, at });
    }
    w.finish_dealing("research", &[], &[]);
    let waits = vec![
        Waited::Call { call: tool, on: None },
        Waited::Call {
            call: question,
            on: Some(Who::Owner),
        },
    ];
    assert_eq!(w.decide(), wait("research", waits));
    w.done(tool);
    assert_eq!(
        w.decide(),
        wait(
            "research",
            vec![Waited::Call {
                call: question,
                on: Some(Who::Owner)
            }]
        )
    );
    w.done(question);
    // What ran the run that asked is still new, beside the results.
    assert_eq!(
        w.decide(),
        runs(
            "research",
            vec![changed("brief", None, false), results(&[tool, question])],
            &[("brief", Some(brief)), ("plan", None)]
        )
    );
    // The run given the results deals with the brief: nothing more holds the research, and the implementation runs.
    let plan = w.run("research", &[("plan", Matters)]);
    assert_eq!(
        w.decide(),
        runs("implement", first(&["plan"]), &[("plan", Some(plan)), ("code", None)])
    );
}

#[test]
fn a_run_given_the_results_that_is_abandoned_leaves_them_to_the_next() {
    let mut w = World::with_gate(vec![when("go", "yes", cell("research", &["brief"], &["plan"]))]);
    w.person_sets("go", "yes");
    let brief = w.person_writes("brief");
    w.start("research");
    let call = w.asks("research", None);
    w.done(call);
    let given = w.start("research");
    w.returns("research");
    w.person_sets("go", "no");
    assert_eq!(w.decide(), abandon("research", given, Abandoning::NoLongerApplies));
    w.abandon();
    w.person_sets("go", "yes");
    assert_eq!(
        w.decide(),
        runs(
            "research",
            vec![changed("brief", None, false), results(&[call])],
            &[("brief", Some(brief)), ("plan", None)]
        )
    );
}

#[test]
fn a_cell_entered_again_and_again_only_to_ask_for_calls_waits_for_the_owner_at_its_limit_of_calls() {
    let mut researching = cell("research", &["brief"], &["plan"]);
    researching.calls = 2;
    let mut w = World::with_gate(vec![researching]);
    w.person_writes("brief");
    w.start("research");
    let first_call = w.asks("research", None);
    w.done(first_call);
    w.start("research");
    let second_call = w.asks("research", None);
    w.done(second_call);
    assert_eq!(w.decide(), wait("research", vec![Waited::Calls { runs: 2 }]));
    w.let_go("research");
    assert!(matches!(w.decide(), Decision::Run { cell, .. } if cell.as_str() == "research"));
}
