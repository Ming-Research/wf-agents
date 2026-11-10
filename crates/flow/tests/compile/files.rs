//! The files: what cannot be read, and the order that names the cells' files.

use flow::{FLOW_FILE, FaultKind::*};

use crate::support::{at, edit, faults, set, unreadable, valid};

#[test]
fn a_file_that_is_not_toml_cannot_be_read() {
    let source = set(valid(), "judge.toml", "title = ");
    unreadable(&source, "judge.toml");
}

#[test]
fn a_file_without_a_field_it_must_have_or_with_a_value_of_the_wrong_kind_cannot_be_read() {
    let source = edit(valid(), "judge.toml", "title = \"Judge the plan\"\n", "");
    let message = unreadable(&source, "judge.toml");
    assert!(message.contains("title"), "{message}");
    let source = edit(valid(), "judge.toml", "kind = \"agent\"", "kind = \"robot\"");
    unreadable(&source, "judge.toml");
    let source = edit(valid(), FLOW_FILE, "[limits]\nruns = 3\n", "[limits]\n");
    unreadable(&source, FLOW_FILE);
    let source = edit(valid(), FLOW_FILE, "people = true", "people = \"yes\"");
    unreadable(&source, FLOW_FILE);
}

#[test]
fn a_field_no_file_knows_is_a_fault() {
    let source = edit(
        valid(),
        "plan.toml",
        "title = \"Plan\"",
        "title = \"Plan\"\ncolour = \"red\"",
    );
    let message = unreadable(&source, "plan.toml");
    assert!(message.contains("colour"), "{message}");
    let source = edit(valid(), FLOW_FILE, "calls = 40", "calls = 40\nhours = 8");
    unreadable(&source, FLOW_FILE);
    let source = edit(valid(), FLOW_FILE, "[blocks.plan]", "[blocks.plan]\nshape = \"prose\"");
    unreadable(&source, FLOW_FILE);
    let source = edit(
        valid(),
        FLOW_FILE,
        "[lines.engineer]",
        "[lines.engineer]\nmodel = \"big\"",
    );
    unreadable(&source, FLOW_FILE);
    let source = edit(valid(), "answer.toml", "{ test = ", "{ block = \"ci\", test = ");
    unreadable(&source, "answer.toml");
}

#[test]
fn every_file_that_cannot_be_read_is_reported_in_the_order_of_the_walk_when_flow_toml_gives_one() {
    let source = set(valid(), "plan.toml", "title = ");
    let source = set(source, "merge.toml", "kind = 1");
    let files = |source| -> Vec<String> { faults(&source).into_iter().map(|f| f.file).collect() };
    assert_eq!(files(source.clone()), ["plan.toml", "merge.toml"]);
    let source = edit(source, FLOW_FILE, "name = \"small\"", "name = 1");
    assert_eq!(files(source), [FLOW_FILE, "merge.toml", "plan.toml"]);
}

#[test]
fn every_cell_of_the_order_has_a_file() {
    let source = edit(valid(), FLOW_FILE, "\"merge\"]", "\"merge\", \"ghost\"]");
    let what = NoCellFile {
        cell: "ghost".to_string(),
    };
    assert_eq!(faults(&source), [at(FLOW_FILE, "order[6]", what)]);
}

#[test]
fn every_cell_file_is_in_the_order() {
    let source = set(valid(), "extra.toml", "title = \"Extra\"\nkind = \"engine\"\n");
    let what = NotInOrder {
        cell: "extra".to_string(),
    };
    assert_eq!(faults(&source), [at("extra.toml", "", what)]);
}

#[test]
fn no_cell_is_in_the_order_twice() {
    let source = edit(valid(), FLOW_FILE, "\"merge\"]", "\"merge\", \"plan\"]");
    let what = Twice {
        name: "plan".to_string(),
    };
    assert_eq!(faults(&source), [at(FLOW_FILE, "order[6]", what)]);
}

#[test]
fn an_empty_order_is_a_fault_and_leaves_every_cell_out_of_it_and_every_test_and_block_without_its_cells() {
    let order = "order = [\"plan\", \"judge\", \"design\", \"answer\", \"check\", \"merge\"]";
    let source = edit(valid(), FLOW_FILE, order, "order = []");
    let block = |name: &str| (format!("blocks.{name}"), name.to_string());
    let unwritten = |name: &str| {
        let (field, block) = block(name);
        at(FLOW_FILE, &field, NoWriter { block })
    };
    let unread = |name: &str| {
        let (field, block) = block(name);
        at(FLOW_FILE, &field, NoReader { block })
    };
    let unused_test = |test: &str| {
        let what = UnusedTest { test: test.to_string() };
        at(FLOW_FILE, &format!("tests.{test}"), what)
    };
    let out = |cell: &str| {
        let what = NotInOrder { cell: cell.to_string() };
        at(&format!("{cell}.toml"), "", what)
    };
    assert_eq!(
        faults(&source),
        [
            at(FLOW_FILE, "order", OrderEmpty),
            // A test no cell uses reads nothing: ci and the choices people write for tests are unread too.
            unwritten("ci"),
            unread("ci"),
            unread("commented"),
            unread("comments"),
            unread("confirmed"),
            unwritten("design"),
            unread("design"),
            unread("main"),
            unwritten("needs_design"),
            unread("needs_design"),
            unread("opinions"),
            unwritten("plan"),
            unread("plan"),
            unwritten("reply"),
            unread("request"),
            unused_test("has_comment"),
            unused_test("may_merge"),
            unused_test("plan_confirmed"),
            out("answer"),
            out("check"),
            out("design"),
            out("judge"),
            out("merge"),
            out("plan"),
        ]
    );
}
