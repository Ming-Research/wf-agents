//! Preconditions, and everything a work waits for at one cell.

use super::*;

#[test]
fn a_precondition_holds_the_work_before_its_cell_even_when_the_cell_needs_nothing() {
    let mut w = World::with_gate(vec![requiring("a may go", cell("a", &[], &[])), cell("b", &["y"], &[])]);
    w.person_writes("y");
    w.run("a", &[]);
    w.run("b", &[]);
    w.set("a may go", Outcome::Waits { who: Who::Architect });
    w.person_writes("y");
    assert_eq!(w.decide(), wait("a", vec![precondition("a may go", Who::Architect)]));
}

#[test]
fn a_precondition_of_a_skipped_cell_holds_nothing() {
    let extra = requiring("extra may go", when("more", "yes", cell("extra", &[], &[])));
    let mut w = World::with_gate(vec![cell("ask", &[], &["more"]), extra]);
    w.set("extra may go", Outcome::Waits { who: Who::Architect });
    w.run("ask", &[("more", Value("no"))]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_call_and_an_unmet_precondition_at_one_cell_are_waited_for_at_once() {
    let mut w = World::with_gate(vec![requiring("a may go", cell("a", &[], &[]))]);
    w.start("a");
    let call = w.calls("a", Some(Who::Owner));
    w.set("a may go", Outcome::Waits { who: Who::Architect });
    let call = Waited::Call {
        call,
        on: Some(Who::Owner),
    };
    assert_eq!(
        w.decide(),
        wait("a", vec![call, precondition("a may go", Who::Architect)])
    );
}

#[test]
fn an_unmet_precondition_and_the_limit_at_one_cell_are_waited_for_at_once() {
    let (mut w, ..) = judged();
    w.install(vec![
        requiring("implement may go", cell("implement", &["plan", "opinions"], &["code"])),
        cell("review", &["code"], &["opinions"]),
        gate(),
    ]);
    for _ in 0..3 {
        send_back(&mut w);
    }
    w.run("review", &[("opinions", Matters)]);
    w.set("implement may go", Outcome::Waits { who: Who::Owner });
    assert_eq!(
        w.decide(),
        wait(
            "implement",
            vec![precondition("implement may go", Who::Owner), Waited::Limit { runs: 3 }]
        )
    );
}
