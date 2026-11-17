//! What a return that writes a choice with a value may hold beside it, and no more (`with_nothing_but`): each value is
//! one of the choice's, each block one a cell that writes the choice may write, and the declaration is compiled into
//! the choice.

use std::collections::BTreeMap;

use decide::BlockId;
use flow::FLOW_FILE;
use flow::FaultKind::*;

use crate::support::{at, compiled, edit, faults, laid_out, valid};

const NEEDS_DESIGN: &str = "[blocks.needs_design]\nvalues = [\"yes\", \"no\"]";

/// The small flow whose judge's choice says this of what a return with a value holds besides it.
fn declared(table: &str) -> flow::Source {
    edit(
        valid(),
        FLOW_FILE,
        NEEDS_DESIGN,
        &format!("{NEEDS_DESIGN}\nwith_nothing_but = {table}"),
    )
}

#[test]
fn what_a_return_with_a_value_may_hold_beside_it_is_compiled_into_the_choice() {
    let compiled = compiled(&declared(r#"{ yes = ["opinions"], no = [] }"#));
    let expected = BTreeMap::from([
        ("no".to_string(), Vec::new()),
        ("yes".to_string(), vec![BlockId::from("opinions")]),
    ]);
    assert_eq!(compiled.blocks[&"needs_design".into()].with_nothing_but, expected);
    // A choice that says none limits no return that writes it.
    assert_eq!(compiled.blocks[&"ci".into()].with_nothing_but, BTreeMap::new());
}

#[test]
fn a_value_the_choice_does_not_have_is_refused() {
    let source = declared(r#"{ maybe = ["opinions"], yes = [] }"#);
    let what = NothingButNotAValue {
        block: "needs_design".to_string(),
        value: "maybe".to_string(),
    };
    assert_eq!(
        faults(&source),
        [at(FLOW_FILE, "blocks.needs_design.with_nothing_but.maybe", what)]
    );
    // A block that is no choice has no value to say it of.
    let source = edit(
        valid(),
        FLOW_FILE,
        "[blocks.design]",
        "[blocks.design]\nwith_nothing_but = { yes = [] }",
    );
    let what = NothingButNotAValue {
        block: "design".to_string(),
        value: "yes".to_string(),
    };
    assert_eq!(
        faults(&source),
        [at(FLOW_FILE, "blocks.design.with_nothing_but.yes", what)]
    );
}

#[test]
fn a_block_no_cell_that_writes_the_choice_may_write_is_refused_at_its_place() {
    // The plan is the planning cell's and not the judge's, which alone writes the choice; nowhere is declared nowhere.
    let source = declared(r#"{ yes = ["opinions", "plan", "nowhere"] }"#);
    let not_an_output = |output: &str| NothingButNotAnOutput {
        block: "needs_design".to_string(),
        value: "yes".to_string(),
        output: output.to_string(),
    };
    assert_eq!(
        faults(&source),
        [
            at(
                FLOW_FILE,
                "blocks.needs_design.with_nothing_but.yes[1]",
                not_an_output("plan")
            ),
            at(
                FLOW_FILE,
                "blocks.needs_design.with_nothing_but.yes[2]",
                not_an_output("nowhere")
            ),
        ]
    );
}

#[test]
fn a_choice_laid_out_in_a_file_of_its_own_says_it_there_and_is_refused_there() {
    const FILE: &str = "blocks/judging/needs_design.toml";
    let values = "values = [\"yes\", \"no\"]";
    let source = edit(
        laid_out(),
        FILE,
        values,
        &format!("{values}\nwith_nothing_but = {{ yes = [\"opinions\"] }}"),
    );
    let expected = BTreeMap::from([("yes".to_string(), vec![BlockId::from("opinions")])]);
    assert_eq!(
        compiled(&source).blocks[&"needs_design".into()].with_nothing_but,
        expected
    );
    let source = edit(
        laid_out(),
        FILE,
        values,
        &format!("{values}\nwith_nothing_but = {{ maybe = [] }}"),
    );
    let what = NothingButNotAValue {
        block: "needs_design".to_string(),
        value: "maybe".to_string(),
    };
    assert_eq!(faults(&source), [at(FILE, "with_nothing_but.maybe", what)]);
}
