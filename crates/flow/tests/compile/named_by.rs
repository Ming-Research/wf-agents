//! The block a cell names the work's merge by, where the host shows it: one the flow declares, compiled into what the
//! engine needs of the cell.

use decide::BlockId;
use flow::FaultKind::*;

use crate::support::{at, compiled, edit, faults, valid};

const MERGE_INPUTS: &str = r#"watches = ["ci"]"#;

/// The small flow whose merge is named by this block.
fn merge_named_by(block: &str) -> flow::Source {
    edit(
        valid(),
        "merge.toml",
        MERGE_INPUTS,
        &format!("{MERGE_INPUTS}\nnamed_by = \"{block}\""),
    )
}

#[test]
fn the_block_a_merge_is_named_by_is_compiled_into_its_spec() {
    let compiled = compiled(&merge_named_by("design"));
    let named_by = |id: &str| {
        let cell = (compiled.cells.iter()).find(|c| c.id.as_str() == id).expect("the cell");
        cell.named_by.clone()
    };
    assert_eq!(named_by("merge"), Some(BlockId::from("design")));
    assert_eq!(named_by("plan"), None);
}

#[test]
fn a_merge_is_named_by_a_block_the_flow_declares() {
    assert_eq!(
        faults(&merge_named_by("title")),
        [at(
            "merge.toml",
            "named_by",
            NoSuchBlock {
                block: "title".to_string()
            }
        )]
    );
}
