//! The groups of blocks: each names blocks of the flow and has a name of its own, and a cell that watches a group is
//! run by any of its blocks.

use flow::{FLOW_FILE, FaultKind::*, Listed, Source, Watched};

use crate::support::{at, compiled, edit, faults, laid_out};

/// The small flow laid out one thing to a file with this group declared.
fn grouped(group: &str) -> Source {
    edit(
        laid_out(),
        FLOW_FILE,
        "[limits]",
        &format!("[groups]\n{group}\n\n[limits]"),
    )
}

#[test]
fn a_cell_that_watches_a_group_is_run_by_each_of_its_blocks_and_reads_them() {
    let source = grouped(r#"planned = ["plan", "opinions"]"#);
    let source = edit(
        source,
        "cells/design.toml",
        "watches = [\"plan\"]",
        "watches = [\"planned\"]",
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
        design.watches,
        [Watched {
            name: "planned".to_string(),
            blocks: vec!["plan".into(), "opinions".into()],
            does: None,
            list: Listed::Watches,
        }]
    );
    assert_eq!(compiled.flow.cells()[2].runs_on, ["plan".into(), "opinions".into()]);
    assert_eq!(compiled.groups["planned"], ["plan", "opinions"]);
    // A return names the group as dealt with by its name, whose blocks the run's check reads it as.
    assert_eq!(
        design.dealt_with(&["planned".to_string()]),
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

/// The small flow with the groups `planned`, of the plan and the opinions, and `talked`, of the opinions and the comments,
/// whose design watches and reads these, given each block it takes.
fn design_taking(watches: &str, reads: &str) -> Source {
    let source = grouped("planned = [\"plan\", \"opinions\"]\ntalked = [\"opinions\", \"comments\"]");
    let source = edit(
        source,
        "cells/design.toml",
        "watches = [\"plan\"]",
        &format!("watches = {watches}\nreads = {reads}"),
    );
    let comments = match format!("{watches}{reads}").contains("talked") {
        true => "\ncomments = \"latest\"",
        false => "",
    };
    edit(
        source,
        "cells/design.toml",
        "plan = \"last_and_latest\"",
        &format!("plan = \"last_and_latest\"\nopinions = \"latest\"{comments}"),
    )
}

// A cell takes a block by one name among what it watches and reads, the block's own or one group's: by two, what the
// block does for the cell would be said twice, and where one is watched and the other read, the two would contradict
// each other (decision a-cell-names-a-block-once). The second name is refused, saying both.
#[test]
fn a_block_a_cell_takes_by_two_names_is_refused_at_the_second() {
    let taken = |block: &str, first: &str, also: &str| TakenTwice {
        block: block.to_string(),
        first: first.to_string(),
        also: also.to_string(),
    };
    let cases = [
        (
            r#"["planned", "plan"]"#,
            "[]",
            "watches[1]",
            taken("plan", "planned", "plan"),
        ),
        (
            r#"["plan", "planned"]"#,
            "[]",
            "watches[1]",
            taken("plan", "plan", "planned"),
        ),
        (
            r#"["planned"]"#,
            r#"["plan"]"#,
            "reads[0]",
            taken("plan", "planned", "plan"),
        ),
        (
            r#"["plan"]"#,
            r#"["planned"]"#,
            "reads[0]",
            taken("plan", "plan", "planned"),
        ),
        (
            r#"["planned"]"#,
            r#"["talked"]"#,
            "reads[0]",
            taken("opinions", "planned", "talked"),
        ),
    ];
    for (watches, reads, field, what) in cases {
        assert_eq!(
            faults(&design_taking(watches, reads)),
            [at("cells/design.toml", field, what)],
            "watches = {watches}, reads = {reads}"
        );
    }
    // A group alone, its blocks taken by it once, compiles.
    compiled(&design_taking(r#"["planned"]"#, "[]"));
}
