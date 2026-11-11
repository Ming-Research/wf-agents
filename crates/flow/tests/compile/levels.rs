//! What a cell's rounds run on: the reasoning effort the cell sets, and how far above its item's level its line runs.

use flow::{Effort, FLOW_FILE, FaultKind};

use crate::support::{at, compiled, edit, faults, valid};

/// The cell of the compiled definition.
fn spec<'a>(flow: &'a flow::Compiled, id: &str) -> &'a flow::CellSpec {
    flow.cells
        .iter()
        .find(|c| c.id.as_str() == id)
        .expect("the cell is compiled")
}

#[test]
fn an_agent_cell_runs_at_the_effort_it_sets_and_at_the_program_s_own_where_it_sets_none() {
    let source = edit(
        valid(),
        "plan.toml",
        "line = \"engineer\"",
        "line = \"engineer\"\neffort = \"xhigh\"",
    );
    let flow = compiled(&source);
    assert_eq!(spec(&flow, "plan").effort, Some(Effort::Xhigh));
    assert_eq!(spec(&flow, "judge").effort, None);
    assert_eq!(Effort::Xhigh.name(), "xhigh");
}

#[test]
fn an_effort_the_command_line_does_not_name_is_refused_and_an_engine_cell_runs_at_none() {
    let source = edit(
        valid(),
        "plan.toml",
        "line = \"engineer\"",
        "line = \"engineer\"\neffort = \"highest\"",
    );
    match &faults(&source)[..] {
        [fault] => {
            assert_eq!(fault.file, "plan.toml");
            assert!(matches!(fault.what, FaultKind::Unreadable { .. }), "{fault:?}");
        }
        other => panic!("{other:?}"),
    }
    let source = edit(
        valid(),
        "check.toml",
        "act = \"check\"",
        "act = \"check\"\neffort = \"high\"",
    );
    assert_eq!(
        faults(&source),
        [at("check.toml", "effort", FaultKind::EngineHasEffort)]
    );
}

#[test]
fn a_cell_runs_as_many_levels_above_its_item_s_as_its_line_says() {
    let source = edit(valid(), FLOW_FILE, "[lines.review]", "[lines.review]\nlevels_above = 1");
    let flow = compiled(&source);
    assert_eq!(
        spec(&flow, "judge").levels_above,
        1,
        "the judge is on the line that runs above"
    );
    assert_eq!(
        spec(&flow, "plan").levels_above,
        0,
        "a line runs at its item's level unless it says so"
    );
    assert_eq!(spec(&flow, "check").levels_above, 0, "an engine cell is on no line");
}
