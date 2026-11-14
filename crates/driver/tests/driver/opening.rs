//! Opening the driver: on an empty store there is nothing to do; what it cannot go on from is refused with the fault
//! its documentation names; and a store that holds works gives exactly what was to be done and is not known as done.

use rusqlite::{Connection, params};
use store::Storage;

use super::*;

/// Opens a driver on the file with these flows, through a store that does not fail.
fn open_with(db: &Db, flows: Vec<flow::Compiled>) -> Opened {
    let (store, _) = Failing::new(db.open());
    Driver::open(Box::new(store), flows, SLOTS, None, Time(START))
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
        let write = "title = \"Write\"\nkind = \"agent\"\neffort = \"high\"\nline = \"main\"\ntask = \"Write the text.\"\n\
                     requires = { test = \"ready\" }\noutputs = [\"text\"]\n";
        let merge = "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\ninputs = [\"text\"]\n";
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
    let todos = h.returns(b, Came::asks(vec![tool("search")]));
    assert_eq!(todos.len(), 1);
    // A cancelled work whose round is still to be stopped.
    let (_, c) = h.add(new_item("c", &[], "short"));
    h.let_on(c);
    h.on(c, Event::Cancelled { why: None });
    // A work whose cell waits for a person's answer to its call, though a person wrote what runs it since.
    let (_, d) = h.add(new_item("d", &[], "plain"));
    h.let_on(d);
    h.returns(d, Came::asks(vec![question(Who::Owner)]));
    h.on(d, wrote("request", Outsider::Person, None));
    // A work never let on, and a merged one.
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
                Effect::Release { .. } => "release",
            };
            (t.work, kind)
        })
        .collect();
    // The merged work's release is answered as it merges.
    let mut expected = vec![(a, "round"), (b, "call"), (c, "stop"), (d, "call")];
    expected.sort();
    let mut got = kinds.clone();
    got.sort();
    assert_eq!(got, expected, "{pending:?}");
    let todos = h.reopen();
    assert_eq!(todos, pending);
    // Works were going on: the driver paused everything as it opened, and holds otherwise nothing.
    let held = EverythingPaused {
        since: Time(h.clock),
        by: Pauser::Engine,
    };
    assert_eq!(h.d().everything_paused(), Some(held));
    let mut after = h.state();
    let rows = after
        .file
        .get_mut("everything_paused")
        .expect("the table of the pause of everything");
    assert_eq!(rows.len(), 1, "the driver paused everything once: {rows:?}");
    rows.clear();
    assert_eq!(after, before, "opened again, the driver holds otherwise");
    // Never started by the owner, it has nothing new to run its first cell.
    assert!(matches!(h.decision(e), Decision::Stalled { .. }), "{:?}", h.decision(e));
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
    // Lifts, behind the driver's back, the pause of everything an opening made on these works going on, so that each
    // opening finds the store as the first did.
    let lift = |h: &mut H| {
        let clock = Time(h.clock);
        (h.reader)
            .pause_everything(false, Pauser::Owner, clock)
            .expect("the pause is lifted");
    };
    // How many calls a store is asked as the driver opens, the keeping of its pause of everything among them.
    let (opened, calls) = open_on(&h.db, Time(h.clock));
    assert!(opened.is_ok());
    let ops = calls.lock().expect("the calls are noted").ops.clone();
    assert!(
        ops.contains(&Op::PauseEverything),
        "opening on works going on paused nothing: {ops:?}"
    );
    let n = ops.len();
    assert!(n >= 3, "opening asked the store only {n} times");
    lift(&mut h);
    for at in 1..=n {
        let (store, calls) = Failing::new(h.db.open());
        calls.lock().expect("the calls are noted").plan = Plan::Fail {
            op: None,
            n: at,
            on: false,
        };
        let opened = Driver::open(Box::new(store), flows(), SLOTS, None, Time(h.clock));
        assert_eq!(opened.err(), Some(Fault::Store(failure())), "the call {at} failed");
        let (store, _) = Failing::new(h.db.open());
        let (driver, todos) =
            Driver::open(Box::new(store), flows(), SLOTS, None, Time(h.clock)).expect("the driver opens");
        assert_eq!(todos, pending, "opened after the call {at} failed");
        assert_eq!(
            driver.everything_paused().map(|p| p.by),
            Some(Pauser::Engine),
            "opened after the call {at} failed"
        );
        lift(&mut h);
    }
    h.reopen();
    h.check();
}

/// A flow of a writer and the merge with these blocks beside the writer's text, the writer waiting on the named test
/// `ready` where `tests` declares it, and writing these outputs.
fn with_blocks(blocks: &str, tests: &str, requires: &str, outputs: &str) -> flow::Compiled {
    let flow = format!(
        r#"
        name = "odd"
        order = ["write", "merge"]
        [limits]
        runs = 2
        returns = 2
        calls = 3
        [lines.main]
        [blocks.text]
        {blocks}
        {tests}
        "#
    );
    // The named test it waits on, where it waits on one.
    let requires = if requires.is_empty() {
        String::new()
    } else {
        format!("requires = {{ test = {requires} }}\n")
    };
    let write = format!(
        "title = \"Write\"\nkind = \"agent\"\neffort = \"high\"\nline = \"main\"\ntask = \"Write the text.\"\n\
         {requires}inputs = [\"deps\"]\noutputs = [{outputs}]\n"
    );
    let merge = "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\ninputs = [\"text\"]\n";
    compiled(&flow, &[("write", write.as_str()), ("merge", merge)])
}

const READY: &str = r#"
        [tests.ready]
        says = "Ready"
        kind = "all"
        reads = ["deps"]
        is = "merged"
"#;

#[test]
fn a_flow_with_two_dependencies_blocks_is_refused() {
    let blocks = r#"
        [blocks.deps]
        values = ["waiting", "merged"]
        outside = "dependencies"
        [blocks.again]
        values = ["waiting", "merged"]
        outside = "dependencies"
        shown = true
    "#;
    let odd = with_blocks(blocks, READY, "\"ready\"", "\"text\"");
    let db = Db::new();
    let opened = open_with(&db, vec![judged(), odd]);
    assert_eq!(opened.err(), Some(Fault::DependenciesTwice { flow: "odd".into() }));
}

#[test]
fn a_dependencies_block_people_or_a_cell_may_write_or_that_lacks_a_value_is_refused() {
    let people = r#"
        [blocks.deps]
        values = ["waiting", "merged"]
        outside = "dependencies"
        people = true
    "#;
    let plain = r#"
        [blocks.deps]
        values = ["waiting", "merged"]
        outside = "dependencies"
    "#;
    let lacking = r#"
        [blocks.deps]
        values = ["waiting", "done", "merged-not"]
        outside = "dependencies"
    "#;
    let lacking_test = READY.replace("is = \"merged\"", "is = \"done\"");
    let none = r#"
        [blocks.deps]
        outside = "dependencies"
    "#;
    let cases = [
        (
            "people may write it",
            with_blocks(people, READY, "\"ready\"", "\"text\""),
        ),
        (
            "a cell writes it",
            // A cell's precondition may not read what the cell writes: here the cell reads the block as an input.
            with_blocks(plain, "", "", "\"text\", \"deps\""),
        ),
        (
            "it lacks the value merged",
            with_blocks(lacking, &lacking_test, "\"ready\"", "\"text\""),
        ),
        ("it is no choice", with_blocks(none, "", "", "\"text\"")),
    ];
    for (why, odd) in cases {
        let db = Db::new();
        let opened = open_with(&db, vec![judged(), odd]);
        let fault = Fault::NotDependencies {
            flow: "odd".into(),
            block: block("deps"),
        };
        assert_eq!(opened.err(), Some(fault), "{why}");
    }
}

/// Makes the records of the work faulty behind the driver's back: an entry at the place of its first, the pause.
fn fault_behind_the_back(db: &Db, work: WorkNumber, cell: &str) {
    let conn = Connection::open(&db.path).expect("the database opens");
    let changed = conn
        .execute(
            "INSERT INTO let_go (work, at, cell) VALUES (?1, 1, ?2)",
            params![work.0, cell],
        )
        .expect("an entry is added behind the driver's back");
    assert_eq!(changed, 1);
}

#[test]
fn a_work_whose_records_are_at_fault_is_written_nothing_and_given_nothing_while_the_others_go_on() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "plain"));
    let (_, wb) = h.add(new_item("b", &[a], "short"));
    h.let_on(wa);
    fault_behind_the_back(&h.db, wb, "draft");
    h.faulty.insert(wb);
    let stored = h.reader.load(wb).expect("the work loads");
    let todos = h.restart();
    // Opening wrote nothing to it and gives nothing for it; it shows that its records are at fault.
    assert_eq!(rounds(&todos), [(wa, "write".to_string(), 1)]);
    assert_eq!(h.reader.load(wb).expect("the work loads"), stored);
    assert!(matches!(h.decision(wb), Decision::Fault(_)), "{:?}", h.decision(wb));
    // The other work goes on to its merge; the item the faulty one depends on merging writes it nothing either.
    let todos = h.through(wa);
    assert_eq!(todos, vec![]);
    assert_eq!(h.reader.load(wb).expect("the work loads"), stored);
    assert_eq!(h.dependencies(wb).as_deref(), Some(driver::WAITING));
}

#[test]
fn a_work_at_fault_with_a_round_under_way_is_given_that_round_again_as_the_driver_opens() {
    // What the documentation leaves open, stated as the driver does it: a work whose records are at fault is not
    // looked at on opening, and what was to be done for it and is not known as done is still given, as it is worked
    // out from its records.
    let mut h = H::new();
    let (_, wa) = h.add(new_item("a", &[], "plain"));
    h.let_on(wa);
    let given = h.d().pending();
    fault_behind_the_back(&h.db, wa, "write");
    h.faulty.insert(wa);
    let todos = h.reopen();
    assert!(matches!(h.decision(wa), Decision::Fault(_)));
    assert_eq!(todos, given);
}
