//! Which cells' runs take the latest main into the work's branch as they start: the agent cells that say so.

use crate::support::{compiled, edit, valid};

/// Whether the cell of the compiled definition takes main in.
fn takes_main(flow: &flow::Compiled, id: &str) -> bool {
    let spec = flow.cells.iter().find(|c| c.id.as_str() == id);
    spec.expect("the cell is compiled").takes_main
}

#[test]
fn an_agent_cell_that_says_so_takes_main_in_and_no_other_does() {
    let source = edit(
        valid(),
        "plan.toml",
        "effort = \"high\"",
        "effort = \"high\"\ntakes_main = true",
    );
    let flow = compiled(&source);
    assert!(takes_main(&flow, "plan"), "the plan says so");
    assert!(!takes_main(&flow, "judge"), "a cell that says nothing takes nothing in");
    assert!(
        !compiled(&valid()).cells.iter().any(|c| c.takes_main),
        "no cell takes main in unless it says so"
    );
    // An engine cell that says so has no agent's run to take it in for.
    let source = edit(
        valid(),
        "check.toml",
        "act = \"check\"",
        "act = \"check\"\ntakes_main = true",
    );
    assert!(!takes_main(&compiled(&source), "check"));
}
