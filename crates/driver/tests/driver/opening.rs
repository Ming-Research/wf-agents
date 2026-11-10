//! Opening the driver: on an empty store there is nothing to do; what it cannot go on from is refused with the fault
//! its documentation names; and a store that holds works gives exactly what was to be done and is not known as done.

use rusqlite::{Connection, params};

use super::*;

/// Opens a driver on the file with these flows, through a store that does not fail.
fn open_with(db: &Db, flows: Vec<flow::Compiled>) -> Opened {
    let (store, _) = Failing::new(db.open());
    Driver::open(Box::new(store), flows, Time(START))
}

#[test]
fn an_empty_store_gives_nothing_to_do() {
    let db = Db::new();
    let (driver, todos) = open_with(&db, flows()).expect("the driver opens");
    assert_eq!(todos, vec![]);
    assert_eq!(driver.pending(), vec![]);
    assert_eq!(driver.works().count(), 0);
    assert_eq!(driver.fault(), None);
    // The flows it was given, in the order of their names.
    let names: Vec<&str> = driver.flows().map(|flow| flow.name.as_str()).collect();
    assert_eq!(names, ["judged", "plain", "short"]);
}

#[test]
fn two_flows_of_one_name_are_refused() {
    let db = Db::new();
    let opened = open_with(&db, vec![judged(), short(), judged()]);
    assert_eq!(opened.err(), Some(Fault::FlowTwice("judged".into())));
}

#[test]
fn a_block_the_driver_writes_that_is_no_choice_of_waiting_and_merged_is_refused() {
    for (values, is) in [("[\"yes\", \"no\"]", "yes"), ("[\"waiting\", \"done\"]", "done")] {
        let flow = format!(
            r#"
            name = "odd"
            order = ["write", "merge"]
            [limits]
            runs = 2
            returns = 2
            calls = 3
            [lines.main]
            [blocks.deps]
            values = {values}
            outside = "dependencies"
            [blocks.text]
            [tests.ready]
            says = "Ready"
            kind = "all"
            reads = ["deps"]
            is = "{is}"
            "#
        );
        let write =
            "title = \"Write\"\nkind = \"agent\"\nline = \"main\"\nrequires = [\"ready\"]\noutputs = [\"text\"]\n";
        let merge = "title = \"Merge\"\nkind = \"engine\"\ninputs = [\"text\"]\n";
        let odd = compiled(&flow, &[("write", write), ("merge", merge)]);
        let db = Db::new();
        let opened = open_with(&db, vec![judged(), odd]);
        let fault = Fault::NotDependencies {
            flow: "odd".into(),
            block: block("deps"),
        };
        assert_eq!(opened.err(), Some(fault), "the values {values}");
    }
}

#[test]
fn a_stored_work_whose_flow_was_not_given_is_refused() {
    let mut h = H::new();
    let (_, work) = h.add(new_item("the item", &[], "short"));
    let opened = open_with(&h.db, vec![judged(), plain()]);
    let fault = Fault::NoSuchFlow {
        work,
        flow: "short".into(),
    };
    assert_eq!(opened.err(), Some(fault));
}

#[test]
fn a_store_holding_an_item_with_no_work_is_refused() {
    let mut h = H::new();
    h.add(new_item("a", &[], "judged"));
    let db = Connection::open(&h.db.path).expect("the database opens");
    db.execute("INSERT INTO items (title, added) VALUES ('b', ?1)", params![START])
        .expect("an item is added behind the driver's back");
    let opened = open_with(&h.db, flows());
    assert_eq!(opened.err(), Some(Fault::NotOneWork(ItemNumber(2))));
}

#[test]
fn a_store_holding_an_item_with_two_works_is_refused() {
    let mut h = H::new();
    h.add(new_item("a", &[], "judged"));
    h.add(new_item("b", &[], "judged"));
    let db = Connection::open(&h.db.path).expect("the database opens");
    db.execute(
        "INSERT INTO works (item, flow, added) VALUES (2, 'plain', ?1)",
        params![START],
    )
    .expect("a work is added behind the driver's back");
    let opened = open_with(&h.db, flows());
    assert_eq!(opened.err(), Some(Fault::NotOneWork(ItemNumber(2))));
}

#[test]
fn a_store_holding_works_gives_exactly_what_was_to_be_done_and_is_not_known_as_done() {
    let mut h = H::new();
    // A work with a round under way.
    let (_, a) = h.add(new_item("a", &[], "judged"));
    h.let_on(a);
    // A work resting at a call.
    let (_, b) = h.add(new_item("b", &[], "judged"));
    h.let_on(b);
    let todos = h.returns(b, Came::Asks(vec![tool("search")]));
    assert_eq!(todos.len(), 1);
    // A cancelled work whose round is still to be stopped.
    let (_, c) = h.add(new_item("c", &[], "short"));
    h.let_on(c);
    h.on(c, Event::Cancelled);
    // A work whose call, asked of a person, is to be withdrawn: a person wrote what its run started from; its cell
    // runs again.
    let (_, d) = h.add(new_item("d", &[], "plain"));
    h.let_on(d);
    h.returns(d, Came::Asks(vec![question(Who::Owner)]));
    h.on(d, wrote("request", Outsider::Person, None));
    // A paused work, and a merged one.
    let (_, e) = h.add(new_item("e", &[], "short"));
    let (_, f) = h.add(new_item("f", &[], "plain"));
    h.let_on(f);
    h.through(f);
    let before = h.state();
    let pending = h.d().pending();
    let kinds: Vec<(WorkNumber, &str)> = (pending.iter())
        .map(|t| {
            let kind = match t.effect {
                Effect::Round { .. } => "round",
                Effect::Call { .. } => "call",
                Effect::Stop { .. } => "stop",
                Effect::Withdraw { .. } => "withdraw",
            };
            (t.work, kind)
        })
        .collect();
    let mut expected = vec![(a, "round"), (b, "call"), (c, "stop"), (d, "round"), (d, "withdraw")];
    expected.sort();
    let mut got = kinds.clone();
    got.sort();
    assert_eq!(got, expected, "{pending:?}");
    let todos = h.reopen();
    assert_eq!(todos, pending);
    assert_eq!(h.state(), before, "opened again, the driver holds otherwise");
    assert_eq!(h.decision(e), Decision::Paused);
    assert_eq!(h.decision(f), Decision::Ended(End::Merged));
}

#[test]
fn a_store_that_fails_as_the_driver_opens_is_the_fault_and_opening_again_gives_what_is_pending() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "short"));
    let (_, wb) = h.add(new_item("b", &[a], "judged"));
    h.let_on(wa);
    h.let_on(wb);
    let pending = h.d().pending();
    // How many calls a store is asked as the driver opens.
    let (opened, calls) = open_on(&h.db, Time(h.clock));
    assert!(opened.is_ok());
    let n = calls.lock().expect("the calls are noted").ops.len();
    assert!(n >= 3, "opening asked the store only {n} times");
    for at in 1..=n {
        let (store, calls) = Failing::new(h.db.open());
        calls.lock().expect("the calls are noted").plan = Plan::Fail {
            op: None,
            n: at,
            on: false,
        };
        let opened = Driver::open(Box::new(store), flows(), Time(h.clock));
        assert_eq!(opened.err(), Some(Fault::Store(failure())), "the call {at} failed");
        let (store, _) = Failing::new(h.db.open());
        let (_, todos) = Driver::open(Box::new(store), flows(), Time(h.clock)).expect("the driver opens");
        assert_eq!(todos, pending, "opened after the call {at} failed");
    }
    h.check();
}
