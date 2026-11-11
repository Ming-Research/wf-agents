//! What changed of the block the check before the merge writes the files under the owner's paths into.

use flow::{CellChange, Change, FLOW_FILE};

use crate::compiled;
use crate::support::{edit, valid};

#[test]
fn a_block_the_check_now_writes_the_owners_paths_into_or_no_longer_does_is_a_change() {
    let declared = edit(
        valid(),
        FLOW_FILE,
        "[blocks.plan]",
        "[blocks.owned]\nshown = true\n\n[blocks.plan]",
    );
    let outputs = r#"outputs = ["ci", "owned"]"#;
    let written = edit(declared, "check.toml", r#"outputs = ["ci"]"#, outputs);
    let named = edit(
        written.clone(),
        "check.toml",
        outputs,
        &format!("{outputs}\nowner_paths = \"owned\""),
    );
    let change = |before: Option<&str>, after: Option<&str>| Change::Cell {
        cell: "check".into(),
        change: CellChange::OwnerPaths {
            before: before.map(Into::into),
            after: after.map(Into::into),
        },
    };
    assert_eq!(
        flow::compare(&compiled(&written), &compiled(&named)),
        [change(None, Some("owned"))]
    );
    assert_eq!(
        flow::compare(&compiled(&named), &compiled(&written)),
        [change(Some("owned"), None)]
    );
}
