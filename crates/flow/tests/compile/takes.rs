//! How a cell takes a block it reads, and what a block is: each is carried into what the flow compiles to as it was
//! written, and a cell says how it takes only a block that is among its inputs.

use std::collections::BTreeMap;

use decide::BlockId;
use flow::FaultKind::*;
use flow::{FLOW_FILE, InputFile, InputKind};

use crate::support::{at, compiled, edit, faults, valid};

const JUDGE_OUTPUTS: &str = r#"outputs = ["needs_design", "opinions"]"#;

/// The small flow whose judge says this of the blocks it takes, after the rest of its file.
fn judge_takes(tables: &str) -> flow::Source {
    edit(
        valid(),
        "judge.toml",
        JUDGE_OUTPUTS,
        &format!("{JUDGE_OUTPUTS}\n\n{tables}"),
    )
}

#[test]
fn how_a_cell_takes_a_block_and_what_a_block_is_are_compiled_as_written() {
    let source = judge_takes("[input.plan]\nkind = \"last_and_latest\"\nwhy = \"It is what to judge.\"");
    let source = edit(
        source,
        FLOW_FILE,
        "[blocks.plan]",
        "[blocks.plan]\nwhat = \"What will be done, and in what order.\"",
    );
    let compiled = compiled(&source);
    let cell = |id: &str| (compiled.cells.iter()).find(|c| c.id.as_str() == id).expect("the cell");
    let taken = InputFile {
        kind: Some(InputKind::LastAndLatest),
        why: Some("It is what to judge.".to_string()),
    };
    assert_eq!(cell("judge").input, BTreeMap::from([(BlockId::from("plan"), taken)]));
    // A cell that says nothing of how it takes its inputs has nothing said of them.
    assert_eq!(cell("plan").input, BTreeMap::new());
    let what = |block: &str| compiled.blocks[&BlockId::from(block)].what.clone();
    assert_eq!(what("plan"), Some("What will be done, and in what order.".to_string()));
    assert_eq!(what("opinions"), None);
}

#[test]
fn each_of_the_kinds_is_written_by_its_name_and_either_field_may_be_left_out() {
    let source = judge_takes("[input.plan]\nkind = \"all\"");
    let judge = compiled(&source);
    let judge = (judge.cells.iter())
        .find(|c| c.id.as_str() == "judge")
        .expect("the judge");
    let all = InputFile {
        kind: Some(InputKind::All),
        why: None,
    };
    assert_eq!(judge.input[&BlockId::from("plan")], all);

    let source = edit(
        valid(),
        "plan.toml",
        "outputs = [\"plan\"]",
        "outputs = [\"plan\"]\n\n[input.request]\nkind = \"latest\"\n\n[input.opinions]\nwhy = \"They say what to change.\"",
    );
    let plan = compiled(&source);
    let plan = (plan.cells.iter()).find(|c| c.id.as_str() == "plan").expect("the plan");
    let latest = InputFile {
        kind: Some(InputKind::Latest),
        why: None,
    };
    let only_why = InputFile {
        kind: None,
        why: Some("They say what to change.".to_string()),
    };
    assert_eq!(plan.input[&BlockId::from("request")], latest);
    assert_eq!(plan.input[&BlockId::from("opinions")], only_why);
}

#[test]
fn a_cell_says_how_it_takes_only_a_block_among_its_inputs() {
    // The judge writes the opinions and does not read them; nowhere is declared nowhere.
    let source = judge_takes("[input.opinions]\nkind = \"latest\"\n\n[input.nowhere]\nwhy = \"None.\"");
    let not_an_input = |block: &str| NotAnInput {
        block: block.to_string(),
    };
    assert_eq!(
        faults(&source),
        [
            at("judge.toml", "input.nowhere", not_an_input("nowhere")),
            at("judge.toml", "input.opinions", not_an_input("opinions")),
        ]
    );
}
