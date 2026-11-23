//! Which cells take the turn of the work's project to check and merge: the cells whose files say so, of any kind, and
//! the deciding code's cell is told the same as the page's.

use std::path::Path;

use flow::Has;

use crate::support::{compiled, edit, names, valid};

/// Whether the cell of the compiled definition takes the turn, as the page is told and as the walk reads it.
fn takes_turn(flow: &flow::Compiled, id: &str) -> bool {
    let spec = flow.cells.iter().find(|c| c.id.as_str() == id);
    let told = spec.expect("the cell is compiled").takes_turn;
    let walked = flow
        .flow
        .place(&id.into())
        .map(|i| flow.flow.cells()[i].turn)
        .expect("the cell is in the walk");
    assert_eq!(told, walked, "the page and the walk are told the same of {id}");
    told
}

#[test]
fn a_cell_that_says_so_takes_the_turn_and_no_other_does() {
    let source = edit(
        valid(),
        "check.toml",
        "act = \"check\"",
        "act = \"check\"\ntakes_turn = true",
    );
    let flow = compiled(&source);
    assert!(takes_turn(&flow, "check"), "an engine's cell that says so takes it");
    assert!(!takes_turn(&flow, "judge"), "a cell that says nothing takes none");
    assert!(
        !compiled(&valid()).cells.iter().any(|c| c.takes_turn),
        "no cell takes the turn unless it says so"
    );
    let source = edit(
        valid(),
        "plan.toml",
        "effort = \"high\"",
        "effort = \"high\"\ntakes_turn = true",
    );
    assert!(
        takes_turn(&compiled(&source), "plan"),
        "an agent's cell that says so takes it"
    );
}

#[test]
fn the_engineer_s_check_and_merge_take_the_turn_and_no_other_cell_does() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/engineer");
    let source = flow::read(&dir).expect("the engineer's flow is read");
    let has = Has {
        sources: names(&[
            "answer",
            "asked",
            "dependencies",
            "handed_in",
            "main",
            "start",
            "head",
            "test_times",
            "tree",
            "words",
        ]),
        acts: names(&["start", "check", "merge"]),
        tools: names(&["query", "run_tests"]),
        ..Default::default()
    };
    let flow = flow::compile(&source, &has).expect("the engineer's flow compiles");
    let taking: Vec<&str> = flow
        .cells
        .iter()
        .filter(|c| c.takes_turn)
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(taking, ["M1", "M2"]);
}
