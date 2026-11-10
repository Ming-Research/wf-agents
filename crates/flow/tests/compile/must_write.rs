//! The blocks a cell must write: each is one of its outputs, named once, and the list is compiled into what the engine
//! needs of the cell, in the order written.

use decide::BlockId;
use flow::FaultKind::*;

use crate::support::{at, compiled, edit, faults, valid};

const JUDGE_OUTPUTS: &str = r#"outputs = ["needs_design", "opinions"]"#;

/// The small flow whose judge must write these, as the list is written.
fn judge_must_write(list: &str) -> flow::Source {
    edit(
        valid(),
        "judge.toml",
        JUDGE_OUTPUTS,
        &format!("{JUDGE_OUTPUTS}\nmust_write = {list}"),
    )
}

#[test]
fn the_blocks_a_cell_must_write_are_compiled_into_its_spec_in_the_order_written() {
    let compiled = compiled(&judge_must_write(r#"["opinions", "needs_design"]"#));
    let judge = (compiled.cells.iter())
        .find(|c| c.id.as_str() == "judge")
        .expect("the judge");
    let expected: Vec<BlockId> = vec!["opinions".into(), "needs_design".into()];
    assert_eq!(judge.must_write, expected);
    // A cell that lists none must write nothing each time.
    let plan = compiled
        .cells
        .iter()
        .find(|c| c.id.as_str() == "plan")
        .expect("the plan");
    assert_eq!(plan.must_write, Vec::<BlockId>::new());
}

#[test]
fn a_block_a_cell_must_write_is_among_its_outputs_whether_or_not_it_is_declared() {
    // The plan is declared and is the plan's output, not the judge's; nowhere is declared nowhere.
    let source = judge_must_write(r#"["needs_design", "plan", "nowhere"]"#);
    let not_an_output = |block: &str| NotAnOutput {
        block: block.to_string(),
    };
    assert_eq!(
        faults(&source),
        [
            at("judge.toml", "must_write[1]", not_an_output("plan")),
            at("judge.toml", "must_write[2]", not_an_output("nowhere")),
        ]
    );
}

#[test]
fn a_block_named_twice_among_those_a_cell_must_write_is_a_fault_at_its_second_place() {
    let source = judge_must_write(r#"["opinions", "opinions"]"#);
    let twice = Twice {
        name: "opinions".to_string(),
    };
    assert_eq!(faults(&source), [at("judge.toml", "must_write[1]", twice)]);
}

#[test]
fn an_undeclared_output_a_cell_must_write_is_a_fault_of_its_outputs_alone() {
    let source = edit(
        valid(),
        "judge.toml",
        JUDGE_OUTPUTS,
        "outputs = [\"needs_design\", \"opinions\", \"ghost\"]\nmust_write = [\"ghost\"]",
    );
    let what = NoSuchBlock {
        block: "ghost".to_string(),
    };
    assert_eq!(faults(&source), [at("judge.toml", "outputs[2]", what)]);
}
