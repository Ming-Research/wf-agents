//! The groups of blocks: each names blocks of the flow, has a name of its own, and gives a trigger that fires for any
//! of its blocks.

use flow::{FLOW_FILE, FaultKind::*, Source, Trigger};

use crate::support::{at, compiled, edit, faults, laid_out, set};

/// The trigger of a group, as `../triggers/of_a_group/` writes it.
const OF_A_GROUP: &str = r#"
name = "on_{group}"
what = "Fires when one of the blocks of `{group}` has a new version that the cell did not write itself."
read = ["{group}"]
"#;

/// The small flow laid out one thing to a file with this group declared, and the trigger of a group beside it.
fn grouped(group: &str) -> Source {
    let source = edit(
        laid_out(),
        FLOW_FILE,
        "[limits]",
        &format!("[groups]\n{group}\n\n[limits]"),
    );
    set(source, "../triggers/of_a_group/changed.toml", OF_A_GROUP)
}

#[test]
fn a_cell_run_by_a_group_s_trigger_is_run_by_each_of_its_blocks_and_reads_them() {
    let source = grouped(r#"planned = ["plan", "opinions"]"#);
    let source = edit(
        source,
        "cells/design.toml",
        "triggered_by = [\"on_plan\"]\ncontext = [\"plan\"]",
        "triggered_by = [\"on_planned\"]\ncontext = [\"plan\", \"opinions\"]",
    );
    let source = edit(
        source,
        "cells/design.toml",
        "plan = \"last_and_latest\"",
        "plan = \"last_and_latest\"\nopinions = \"latest\"",
    );
    let compiled = compiled(&source);
    let design = &compiled.cells[2];
    assert_eq!(
        design.triggers,
        [Trigger {
            name: "on_planned".to_string(),
            what: Some(
                "Fires when one of the blocks of `planned` has a new version that the cell did not write itself."
                    .to_string()
            ),
            block: "planned".into(),
            group: vec!["plan".into(), "opinions".into()],
            read: vec!["plan".into(), "opinions".into()],
            detail: None,
            does: None,
        }]
    );
    assert_eq!(compiled.flow.cells()[2].runs_on, ["plan".into(), "opinions".into()]);
    assert_eq!(compiled.groups["planned"], ["plan", "opinions"]);
    // A return names the trigger as dealt with by its name, which stands for the group, whose blocks the run's check
    // reads it as.
    assert_eq!(
        design.dealt_with(&["on_planned".to_string()]),
        (vec!["planned".into()], vec![])
    );
    let group = vec!["plan".into(), "opinions".into()];
    assert_eq!(
        compiled.flow.cells()[2].groups,
        std::collections::BTreeMap::from([("planned".into(), group)])
    );
}

#[test]
fn a_group_names_blocks_of_the_flow_and_a_name_no_block_has() {
    let what = GroupNameTaken {
        group: "plan".to_string(),
    };
    assert_eq!(
        faults(&grouped(r#"plan = ["opinions"]"#)),
        [at(FLOW_FILE, "groups.plan", what)]
    );
    let ghost = NoSuchBlock {
        block: "ghost".to_string(),
    };
    let twice = Twice {
        name: "plan".to_string(),
    };
    assert_eq!(
        faults(&grouped(r#"planned = ["plan", "ghost", "plan"]"#)),
        [
            at(FLOW_FILE, "groups.planned[1]", ghost),
            at(FLOW_FILE, "groups.planned[2]", twice),
        ]
    );
    let empty = GroupEmpty {
        group: "planned".to_string(),
    };
    assert_eq!(
        faults(&grouped("planned = []")),
        [at(FLOW_FILE, "groups.planned", empty)]
    );
}

#[test]
fn the_rule_of_a_group_s_trigger_holds_the_group_s_place() {
    let source = set(
        grouped(r#"planned = ["plan"]"#),
        "../triggers/of_a_group/changed.toml",
        "name = \"on_changed\"\nread = []\n",
    );
    let what = MissingPlaceholder {
        placeholder: "{group}".to_string(),
    };
    assert_eq!(
        faults(&source),
        [at("../triggers/of_a_group/changed.toml", "name", what)]
    );
}
