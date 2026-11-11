//! The block in which the check before the merge writes the files its commit changes under the owner's paths is one of
//! the cell's outputs.

use flow::FLOW_FILE;
use flow::FaultKind::*;

use crate::support::{at, compiled, edit, faults, valid};

const OUTPUTS: &str = r#"outputs = ["ci"]"#;

#[test]
fn the_check_names_one_of_its_outputs_for_the_owners_paths() {
    let declared = edit(
        valid(),
        FLOW_FILE,
        "[blocks.plan]",
        "[blocks.owned]\nshown = true\n\n[blocks.plan]",
    );
    let named = edit(
        declared,
        "check.toml",
        OUTPUTS,
        &format!("{OUTPUTS}\nowner_paths = \"owned\""),
    );
    let unwritten = NotAnOutput {
        block: "owned".to_string(),
    };
    let wanting = faults(&named);
    assert!(
        wanting.contains(&at("check.toml", "owner_paths", unwritten)),
        "{wanting:?}"
    );
    let written = edit(named, "check.toml", OUTPUTS, r#"outputs = ["ci", "owned"]"#);
    let flow = compiled(&written);
    let check = flow.cells.iter().find(|c| c.id.as_str() == "check").unwrap();
    assert_eq!(check.owner_paths, Some("owned".into()));
}
