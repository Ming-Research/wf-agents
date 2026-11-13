//! The blocks a cell says what the work is by, where the host shows it: each is one of its inputs, named once, and the
//! list is compiled into what the engine needs of the cell, in the order written.

use decide::BlockId;
use flow::FaultKind::*;

use crate::support::{at, compiled, edit, faults, valid};

const CHECK_OUTPUTS: &str = r#"outputs = ["ci"]"#;

/// The small flow whose check says what the work is by these, as the list is written.
fn check_describes(list: &str) -> flow::Source {
    edit(
        valid(),
        "check.toml",
        CHECK_OUTPUTS,
        &format!("{CHECK_OUTPUTS}\ndescribes = {list}"),
    )
}

#[test]
fn the_blocks_a_cell_describes_the_work_by_are_compiled_into_its_spec_in_the_order_written() {
    let compiled = compiled(&check_describes(r#"["design", "main"]"#));
    let check = (compiled.cells.iter())
        .find(|c| c.id.as_str() == "check")
        .expect("the check");
    let expected: Vec<BlockId> = vec!["design".into(), "main".into()];
    assert_eq!(check.describes, expected);
    let plan = (compiled.cells.iter())
        .find(|c| c.id.as_str() == "plan")
        .expect("the plan");
    assert_eq!(plan.describes, Vec::<BlockId>::new());
}

#[test]
fn a_block_a_cell_describes_the_work_by_is_among_its_inputs_and_named_once() {
    // The check writes ci and does not read it: a change of it would not run the check again.
    let source = check_describes(r#"["design", "ci", "design"]"#);
    assert_eq!(
        faults(&source),
        [
            at(
                "check.toml",
                "describes[1]",
                NotAnInput {
                    block: "ci".to_string()
                }
            ),
            at(
                "check.toml",
                "describes[2]",
                Twice {
                    name: "design".to_string()
                }
            ),
        ]
    );
}

#[test]
fn in_the_new_form_a_cell_describes_the_work_by_a_block_that_runs_it_or_that_it_is_given() {
    let source = edit(
        valid(),
        "answer.toml",
        "inputs = [\"comments\", \"plan\"]",
        "runs_on = [\"comments\"]\ngiven = [\"plan\"]\ndescribes = [\"plan\", \"comments\", \"reply\"]",
    );
    assert_eq!(
        faults(&source),
        [at(
            "answer.toml",
            "describes[2]",
            NotAnInput {
                block: "reply".to_string()
            }
        )]
    );
}
