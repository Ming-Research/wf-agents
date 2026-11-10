//! Opening the driver: on an empty store there is nothing to do, and what it cannot go on from is refused with the
//! fault its documentation names.

use super::*;

#[test]
fn an_empty_store_gives_nothing_to_do() {
    let db = Db::new();
    let (driver, todos) = Driver::open(db.open(), flows(), tests(), Time(START)).expect("the driver opens");
    assert_eq!(todos, vec![]);
    assert_eq!(driver.pending(), vec![]);
    assert_eq!(driver.items().count(), 0);
    assert_eq!(driver.works().count(), 0);
    assert_eq!(driver.fault(), None);
    // The flows it was given, in the order of their names.
    let names: Vec<&str> = driver.flows().map(|flow| flow.name.as_str()).collect();
    assert_eq!(names, ["forked", "held", "judged", "short"]);
}

#[test]
fn a_flow_that_uses_a_named_test_the_driver_was_not_given_is_refused() {
    let db = Db::new();
    let mut tests = tests();
    tests.remove(&TestName::from("dependencies_merged"));
    let opened = Driver::open(db.open(), vec![forked(), judged()], tests, Time(START));
    let fault = Fault::NoSuchTest {
        flow: "judged".into(),
        test: "dependencies_merged".into(),
    };
    assert_eq!(opened.err(), Some(fault));
}

#[test]
fn two_flows_of_one_name_are_refused() {
    let db = Db::new();
    let opened = Driver::open(db.open(), vec![judged(), short(), judged()], tests(), Time(START));
    assert_eq!(opened.err(), Some(Fault::FlowTwice("judged".into())));
}

#[test]
fn a_stored_work_whose_flow_was_not_given_is_refused() {
    let mut h = H::new();
    let item = h.item("the item", &[]);
    let (work, _) = h.work(item, "short", false);
    let opened = Driver::open(h.db.open(), vec![judged(), forked()], tests(), Time(h.clock));
    let fault = Fault::NoSuchFlow {
        work,
        flow: "short".into(),
    };
    assert_eq!(opened.err(), Some(fault));
}

#[test]
fn opened_again_the_driver_holds_the_items_and_the_works_the_store_holds() {
    let mut h = H::new();
    let a = h.item("a", &[]);
    let b = h.item("b", &[a]);
    let (wa, _) = h.work(a, "judged", false);
    let (wb, _) = h.work(b, "short", false);
    h.hands_in(wa, &[("text", None)]);
    let before = h.state();
    let todos = h.reopen();
    assert_eq!(h.state(), before, "opened again, the driver holds otherwise");
    // The one round under way is given again, and the work that waits for its dependency still waits.
    assert_eq!(rounds(&todos), [(wa, "judge".to_string(), 1)]);
    assert_eq!(h.decision(wb), waits_for_dependencies("draft"));
}
