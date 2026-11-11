//! The limit of ways round.

use super::*;

#[test]
fn a_cell_may_run_as_many_ways_round_in_a_row_as_its_limit_and_the_next_waits_for_the_owner() {
    let (mut w, ..) = judged();
    for _ in 0..3 {
        send_back(&mut w);
    }
    w.run("review", &[("opinions", Matters)]);
    at_limit(&w, 3);
    w.let_go("implement");
    assert_eq!(
        reasons(&w.decide()),
        ("implement".into(), vec![("opinions".into(), true)])
    );
}

#[test]
fn letting_a_cell_go_on_starts_its_count_anew() {
    let (mut w, ..) = judged();
    for _ in 0..3 {
        send_back(&mut w);
    }
    w.run("review", &[("opinions", Matters)]);
    w.let_go("implement");
    w.run("implement", &[("code", Matters)]);
    for _ in 0..2 {
        send_back(&mut w);
    }
    w.run("review", &[("opinions", Matters)]);
    at_limit(&w, 3);
}

#[test]
fn a_run_with_a_reason_a_person_wrote_starts_the_count_anew() {
    let (mut w, ..) = judged();
    for _ in 0..3 {
        send_back(&mut w);
    }
    w.person_writes("plan");
    assert_eq!(reasons(&w.decide()), ("implement".into(), vec![("plan".into(), false)]));
    w.run("implement", &[("code", Matters)]);
    for _ in 0..3 {
        send_back(&mut w);
    }
    w.run("review", &[("opinions", Matters)]);
    at_limit(&w, 3);
}

#[test]
fn a_persons_opinion_starts_the_count_anew() {
    let (mut w, ..) = judged();
    for _ in 0..3 {
        send_back(&mut w);
    }
    w.person_writes("opinions");
    assert_eq!(
        reasons(&w.decide()),
        ("implement".into(), vec![("opinions".into(), false)])
    );
    w.run("implement", &[("code", Matters)]);
    for _ in 0..3 {
        send_back(&mut w);
    }
    w.run("review", &[("opinions", Matters)]);
    at_limit(&w, 3);
}

#[test]
fn a_run_with_a_reason_from_outside_through_a_cell_before_it_is_no_way_round_even_beside_one_from_after_it() {
    let cells = vec![
        cell("research", &["main"], &["plan"]),
        cell("implement", &["plan", "opinions"], &["code"]),
        cell("review", &["code"], &["opinions"]),
    ];
    let mut w = World::with_gate(cells);
    w.outside_writes("main");
    w.run("research", &[("plan", Matters)]);
    w.run("implement", &[("code", Matters)]);
    for _ in 0..3 {
        send_back(&mut w);
    }
    w.run("review", &[("opinions", Matters)]);
    at_limit(&w, 3);
    // The research, before the implementation, changes the plan: the implementation's next run has a reason from
    // before it beside the opinion from after it, and is no way round.
    w.outside_writes("main");
    w.run("research", &[("plan", Matters)]);
    assert_eq!(
        reasons(&w.decide()),
        (
            "implement".into(),
            vec![("plan".into(), false), ("opinions".into(), true)]
        )
    );
    w.run("implement", &[("code", Matters)]);
    for _ in 0..3 {
        send_back(&mut w);
    }
    w.run("review", &[("opinions", Matters)]);
    at_limit(&w, 3);
}

#[test]
fn two_cells_in_a_circle_are_bounded_by_the_earlier_ones_limit() {
    let mut w = World::with_gate(vec![
        with_limit(2, cell("a", &["y"], &["x"])),
        cell("b", &["x"], &["y"]),
    ]);
    let x1 = w.run("a", &[("x", Matters)]);
    let y1 = w.run("b", &[("y", Matters)]);
    assert_eq!(
        w.decide(),
        runs("a", vec![changed("y", None, true)], &[("y", Some(y1)), ("x", Some(x1))])
    );
    let x2 = w.run("a", &[("x", Matters)]);
    // The later cell's every run has a reason from a cell before it.
    assert_eq!(
        w.decide(),
        runs(
            "b",
            vec![changed("x", Some(x1), false)],
            &[("x", Some(x2)), ("y", Some(y1))]
        )
    );
    let y2 = w.run("b", &[("y", Matters)]);
    assert_eq!(
        w.decide(),
        runs(
            "a",
            vec![changed("y", Some(y1), true)],
            &[("y", Some(y2)), ("x", Some(x2))]
        )
    );
    w.run("a", &[("x", Matters)]);
    w.run("b", &[("y", Matters)]);
    assert_eq!(w.decide(), wait("a", vec![Waited::Limit { runs: 2 }]));
}

#[test]
fn an_abandoned_run_is_not_counted_toward_the_limit() {
    let cells = vec![
        with_limit(2, on_test("implementing", cell("implement", &["opinions"], &["code"]))),
        cell("review", &["code"], &["opinions"]),
    ];
    let mut w = World::with_gate(cells);
    w.run("implement", &[("code", Matters)]);
    let o1 = w.run("review", &[("opinions", Matters)]);
    let c2 = w.run("implement", &[("code", Matters)]);
    let o2 = w.run("review", &[("opinions", Matters)]);
    // The second way round starts and is abandoned when its cell is skipped.
    let run = w.start("implement");
    w.returns("implement");
    w.says("implementing", false);
    assert_eq!(w.decide(), abandon("implement", run, Abandoning::NoLongerApplies));
    w.abandon();
    w.says("implementing", true);
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("opinions", Some(o1), true)],
            &[("opinions", Some(o2)), ("code", Some(c2))]
        )
    );
}

#[test]
fn a_limit_of_zero_is_refused() {
    let cells = vec![with_limit(0, cell("a", &[], &[])), gate()];
    assert_eq!(
        Flow::new(cells, BTreeMap::new(), BTreeMap::new()),
        Err(FlowFault::NoLimit("a".into()))
    );
}

#[test]
fn a_persons_confirmation_of_a_choice_a_later_cell_made_does_not_lift_the_limit() {
    let cells = vec![
        with_limit(1, cell("implement", &["verdict"], &["code"])),
        cell("review", &["code"], &["verdict"]),
    ];
    let mut w = World::with_gate(cells);
    w.run("implement", &[("code", Matters)]);
    w.run("review", &[("verdict", Value("no"))]);
    w.run("implement", &[("code", Matters)]);
    w.run("review", &[("verdict", Value("yes"))]);
    assert_eq!(w.decide(), wait("implement", vec![Waited::Limit { runs: 1 }]));
    // The confirmation is the latest version, but the meaning is still the one the review gave.
    w.person_confirms("verdict", "yes");
    assert_eq!(w.decide(), wait("implement", vec![Waited::Limit { runs: 1 }]));
}

#[test]
fn a_persons_change_is_no_way_round_though_a_later_cell_rewrote_it_as_not_mattering() {
    let cells = vec![
        with_limit(1, on_test("implementing", cell("implement", &["opinions"], &["code"]))),
        cell("review", &["code", "rules"], &["opinions"]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("rules");
    w.run("implement", &[("code", Matters)]);
    let o1 = w.run("review", &[("opinions", Matters)]);
    let code = w.run("implement", &[("code", Matters)]);
    w.run("review", &[]);
    assert_eq!(w.decide(), resting());
    // While the implementation is skipped, the owner writes an opinion for it, and the review, run again for its
    // rules, rewrites the opinions saying its change does not matter.
    w.says("implementing", false);
    w.person_writes("opinions");
    w.person_writes("rules");
    let rewritten = w.run("review", &[("opinions", DoesNotMatter)]);
    w.says("implementing", true);
    // The meaning the opinions have now is the owner's: the implementation's next run is no way round.
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("opinions", Some(o1), false)],
            &[("opinions", Some(rewritten)), ("code", Some(code))]
        )
    );
}

#[test]
fn letting_a_cell_go_on_starts_its_count_anew_whenever_the_owner_does_it() {
    let (mut w, ..) = judged();
    for _ in 0..2 {
        send_back(&mut w);
    }
    // The owner lets the implementation go on before it has reached its limit: the two ways round are forgotten.
    w.let_go("implement");
    send_back(&mut w);
    w.run("review", &[("opinions", Matters)]);
    let next = w.decide();
    assert!(
        matches!(&next, decide::Decision::Run { cell, .. } if cell.as_str() == "implement"),
        "the implementation runs again: {next:?}"
    );
}
