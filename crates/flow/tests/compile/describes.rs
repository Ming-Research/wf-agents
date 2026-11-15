//! The blocks a cell says what the work is by, where the host shows it: each is a block of the flow with content,
//! whether or not it runs the cell, named once, and the list is compiled into what the engine needs of the cell, in the
//! order written.

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

// A block that does not run the cell describes the work as well as one that does: what merges is described again from
// them before the merge. A block the flow does not have, or one named twice, is a fault.
#[test]
fn a_block_a_cell_describes_the_work_by_is_one_of_the_flows_named_once_whether_or_not_it_runs_the_cell() {
    // The check writes ci and does not read it.
    let source = check_describes(r#"["design", "ci", "design", "nowhere"]"#);
    assert_eq!(
        faults(&source),
        [
            at(
                "check.toml",
                "describes[2]",
                Twice {
                    name: "design".to_string()
                }
            ),
            at(
                "check.toml",
                "describes[3]",
                NoSuchBlock {
                    block: "nowhere".to_string()
                }
            ),
        ]
    );
}

#[test]
fn in_the_new_form_a_cell_describes_the_work_by_a_block_it_is_given_or_writes_as_by_one_that_runs_it() {
    let source = edit(
        valid(),
        "answer.toml",
        "inputs = [\"comments\", \"plan\"]",
        "runs_on = [\"comments\"]\ngiven = [\"plan\"]\ndescribes = [\"plan\", \"comments\", \"reply\"]",
    );
    let source = edit(
        source,
        "answer.toml",
        "limit = 5",
        "limit = 5\n\n[give.plan]\nkind = \"latest\"",
    );
    let compiled = compiled(&source);
    let answer = (compiled.cells.iter())
        .find(|c| c.id.as_str() == "answer")
        .expect("the answer");
    let expected: Vec<BlockId> = vec!["plan".into(), "comments".into(), "reply".into()];
    assert_eq!(answer.describes, expected);
}
