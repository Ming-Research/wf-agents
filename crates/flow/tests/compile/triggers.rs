//! What runs a cell named apart from what it is given: its triggers, each an event worked out to the block it stands
//! on, and its context, each datum in its kind. A cell so written runs on the blocks its triggers stand on and is given
//! its context, as a cell written before triggers runs on and is given its blocks; and of such a cell, each block that
//! runs it is a trigger of the block's name.

use decide::BlockId;
use flow::FaultKind::*;
use flow::{CellSpec, Compiled, FLOW_FILE, FaultKind, InputKind, Trigger};

use crate::support::{at, compiled, edit, faults, laid_out, set, valid};

const PLAN_TRIGGERS: &str = r#"triggers = ["request_changed", "opinions_changed"]"#;
const PLAN_CONTEXT: &str = r#"context = ["request", "opinions"]"#;
const PLAN_KINDS: &str = "opinions = \"last_and_latest\"";

fn cell<'a>(compiled: &'a Compiled, id: &str) -> &'a CellSpec {
    (compiled.cells.iter()).find(|c| c.id.as_str() == id).expect("the cell")
}

fn trigger(name: &str, block: &str, read: &[&str], does: Option<&str>) -> Trigger {
    Trigger {
        name: name.to_string(),
        block: block.into(),
        read: read.iter().map(|&b| b.into()).collect(),
        detail: None,
        does: does.map(str::to_string),
    }
}

/// The twin laid out one thing to a file, its planning cell naming these triggers in place of its own.
fn plan_triggers(triggers: &str) -> flow::Source {
    edit(laid_out(), "cells/plan.toml", PLAN_TRIGGERS, triggers)
}

/// The twin laid out one thing to a file, with this after the rest of its planning cell's file.
fn plan_adds(text: &str) -> flow::Source {
    let mut source = laid_out();
    source.cells.get_mut("plan").expect("the planning cell").push_str(text);
    source
}

#[test]
fn each_trigger_is_worked_out_to_the_block_it_stands_on_with_what_its_agent_reads_for_it() {
    let compiled = compiled(&laid_out());
    // A rule of `of_a_datum/` makes a trigger of each datum of the flow, which its agent reads for it.
    assert_eq!(
        cell(&compiled, "plan").triggers,
        [
            trigger("request_changed", "request", &["request"], None),
            trigger("opinions_changed", "opinions", &["opinions"], None),
        ]
    );
    // A trigger of its own stands on the block with no content it declares.
    assert_eq!(
        cell(&compiled, "check").triggers,
        [
            trigger("main_moved", "main", &[], None),
            trigger("design_changed", "design", &["design"], None),
        ]
    );
    let main = &compiled.blocks[&BlockId::from("main")];
    assert!(main.trigger);
    assert_eq!(main.writers.outside.as_deref(), Some("main"));
    let readers: Vec<&str> = main.readers.cells.iter().map(|c| c.as_str()).collect();
    assert_eq!(readers, ["check"]);
    // The context is given in the kinds it names, in its order.
    let given: Vec<(String, InputKind)> = (cell(&compiled, "answer").given.iter())
        .map(|g| (g.block.to_string(), g.kind))
        .collect();
    assert_eq!(
        given,
        [
            ("comments".to_string(), InputKind::LastAndLatest),
            ("plan".to_string(), InputKind::LastAndLatest),
        ]
    );
}

#[test]
fn what_a_cell_does_for_a_trigger_is_its_own_word_or_the_trigger_s_and_is_why_its_block_runs_the_cell() {
    let source = plan_adds("\n[do]\nrequest_changed = \"Plan again from what changed.\"\n");
    let compiled = compiled(&source);
    let plan = cell(&compiled, "plan");
    assert_eq!(
        plan.triggers[0],
        trigger(
            "request_changed",
            "request",
            &["request"],
            Some("Plan again from what changed.")
        )
    );
    assert_eq!(plan.runs_on[0].why.as_deref(), Some("Plan again from what changed."));
    assert_eq!(plan.runs_on[1].why, None);
}

#[test]
fn a_trigger_each_cell_has_its_own_of_gives_each_cell_that_names_it_its_own_block() {
    let source = edit(
        laid_out(),
        FLOW_FILE,
        "data = [\"request\"]",
        "data = [\"request\", \"discussion\"]",
    );
    let source = edit(
        source,
        "cells/plan.toml",
        PLAN_TRIGGERS,
        r#"triggers = ["request_changed", "opinions_changed", "spoken_to"]"#,
    );
    let source = edit(
        source,
        "cells/plan.toml",
        PLAN_CONTEXT,
        r#"context = ["request", "opinions", "discussion"]"#,
    );
    let source = edit(
        source,
        "cells/plan.toml",
        PLAN_KINDS,
        "opinions = \"last_and_latest\"\ndiscussion = \"all\"",
    );
    let source = edit(
        source,
        "cells/answer.toml",
        r#"triggers = ["comments_changed", "plan_changed"]"#,
        r#"triggers = ["comments_changed", "plan_changed", "spoken_to"]"#,
    );
    let source = edit(
        source,
        "cells/answer.toml",
        r#"context = ["comments", "plan"]"#,
        r#"context = ["comments", "plan", "discussion"]"#,
    );
    let source = edit(
        source,
        "cells/answer.toml",
        "plan = \"last_and_latest\"",
        "plan = \"last_and_latest\"\ndiscussion = \"all\"\n\n[do]\nspoken_to = \"Answer, then go on.\"",
    );
    let compiled = compiled(&source);
    assert_eq!(
        cell(&compiled, "plan").triggers[2],
        trigger(
            "spoken_to",
            "plan_spoken_to",
            &["discussion"],
            Some("Answer what the owner said.")
        )
    );
    assert_eq!(
        cell(&compiled, "answer").triggers[2],
        trigger(
            "spoken_to",
            "answer_spoken_to",
            &["discussion"],
            Some("Answer, then go on.")
        )
    );
    for (block, id) in [("plan_spoken_to", "plan"), ("answer_spoken_to", "answer")] {
        let declared = &compiled.blocks[&BlockId::from(block)];
        assert!(declared.trigger && declared.writers.people, "{block}");
        let readers: Vec<&str> = declared.readers.cells.iter().map(|c| c.as_str()).collect();
        assert_eq!(readers, [id], "{block}");
    }
    // The walk runs each cell on its own block.
    let walked = compiled
        .flow
        .cells()
        .iter()
        .find(|c| c.id.as_str() == "plan")
        .expect("plan");
    let runs_on: Vec<&str> = walked.runs_on.iter().map(|b| b.as_str()).collect();
    assert_eq!(runs_on, ["request", "opinions", "plan_spoken_to"]);
}

#[test]
fn a_cell_written_before_triggers_has_a_trigger_of_each_block_that_runs_it() {
    let mut source = edit(
        valid(),
        "plan.toml",
        r#"inputs = ["request", "opinions"]"#,
        "runs_on = [\"request\", \"opinions\"]\ngiven = [\"opinions\"]",
    );
    // The tables go after the cell's other fields.
    let plan = source.cells.get_mut("plan").expect("the planning cell");
    plan.push_str("\n[run.request]\nwhy = \"It is what to plan.\"\n\n[give.opinions]\nkind = \"all\"\n");
    let compiled = compiled(&source);
    assert_eq!(
        cell(&compiled, "plan").triggers,
        [
            trigger("request", "request", &[], Some("It is what to plan.")),
            trigger("opinions", "opinions", &["opinions"], None),
        ]
    );
}

#[test]
fn a_trigger_nothing_beside_the_flows_defines_or_named_twice_is_refused() {
    let source = plan_triggers(
        r#"triggers = ["request_changed", "opinions_changed", "ghost_changed", "nothing_happened", "request_changed"]"#,
    );
    let unknown = |name: &str| NoSuchTrigger {
        trigger: name.to_string(),
    };
    assert_eq!(
        faults(&source),
        [
            at("cells/plan.toml", "triggers[2]", unknown("ghost_changed")),
            at("cells/plan.toml", "triggers[3]", unknown("nothing_happened")),
            at(
                "cells/plan.toml",
                "triggers[4]",
                Twice {
                    name: "request_changed".into()
                }
            ),
        ]
    );
    // A block a trigger declares is no datum of the flow, which a rule of `of_a_datum/` makes a trigger of.
    let source = edit(
        laid_out(),
        "cells/check.toml",
        r#"triggers = ["main_moved", "design_changed"]"#,
        r#"triggers = ["main_moved", "design_changed", "main_changed"]"#,
    );
    assert_eq!(
        faults(&source),
        [at("cells/check.toml", "triggers[2]", unknown("main_changed"))]
    );
}

#[test]
fn a_trigger_of_an_agent_cell_has_its_agent_read_only_what_the_cell_s_context_holds() {
    let source = edit(
        laid_out(),
        "cells/judge.toml",
        r#"triggers = ["plan_changed"]"#,
        r#"triggers = ["plan_changed", "request_changed"]"#,
    );
    let what = ReadNotInContext {
        trigger: "request_changed".into(),
        block: "request".into(),
    };
    assert_eq!(faults(&source), [at("cells/judge.toml", "triggers[1]", what)]);
    // An engine cell is given nothing, and its triggers have no agent to read for them.
    let source = edit(
        laid_out(),
        "cells/check.toml",
        r#"triggers = ["main_moved", "design_changed"]"#,
        r#"triggers = ["main_moved", "design_changed", "request_changed"]"#,
    );
    compiled(&source);
}

#[test]
fn what_a_cell_does_for_a_trigger_it_does_not_name_and_a_kind_of_a_datum_outside_its_context_are_refused() {
    let source = plan_adds("comments = \"all\"\n\n[do]\nghost = \"Look again.\"\n");
    assert_eq!(
        faults(&source),
        [
            at(
                "cells/plan.toml",
                "do.ghost",
                NotATrigger {
                    trigger: "ghost".into()
                }
            ),
            at(
                "cells/plan.toml",
                "kinds.comments",
                NotGiven {
                    block: "comments".into()
                }
            ),
        ]
    );
}

#[test]
fn each_datum_of_an_agent_cell_s_context_is_declared_once_with_its_kind_and_an_engine_cell_has_none() {
    let source = edit(
        laid_out(),
        "cells/plan.toml",
        PLAN_CONTEXT,
        r#"context = ["request", "opinions", "comments", "ghost", "request"]"#,
    );
    let block = |name: &str| name.to_string();
    assert_eq!(
        faults(&source),
        [
            at(
                "cells/plan.toml",
                "context[2]",
                NoKind {
                    block: block("comments")
                }
            ),
            at("cells/plan.toml", "context[3]", NoSuchBlock { block: block("ghost") }),
            at("cells/plan.toml", "context[4]", Twice { name: block("request") }),
        ]
    );
    let source = edit(
        laid_out(),
        "cells/check.toml",
        "outputs = [\"ci\"]",
        "outputs = [\"ci\"]\ncontext = [\"design\"]",
    );
    assert_eq!(
        faults(&source),
        [at(
            "cells/check.toml",
            "context[0]",
            EngineIsGiven { block: block("design") }
        )]
    );
}

#[test]
fn an_agent_cell_that_no_trigger_runs_is_refused() {
    assert_eq!(
        faults(&plan_triggers("triggers = []")),
        [at("cells/plan.toml", "triggers", RunByNothing)]
    );
}

#[test]
fn a_cell_names_what_it_takes_in_one_form_only() {
    let source = edit(
        laid_out(),
        "cells/plan.toml",
        PLAN_CONTEXT,
        &format!("{PLAN_CONTEXT}\nruns_on = [\"request\"]"),
    );
    assert_eq!(faults(&source), [at("cells/plan.toml", "runs_on", TwoForms)]);
    let source = edit(
        laid_out(),
        "cells/plan.toml",
        PLAN_CONTEXT,
        &format!("{PLAN_CONTEXT}\ninputs = [\"request\"]"),
    );
    assert_eq!(faults(&source), [at("cells/plan.toml", "inputs", TwoForms)]);
}

#[test]
fn a_trigger_whose_block_the_flow_defines_as_well_is_refused() {
    let source = set(laid_out(), "blocks/main.toml", "outside = \"main\"");
    let twice: FaultKind = Twice { name: "main".into() };
    assert_eq!(
        faults(&source),
        [
            at("cells/check.toml", "triggers[0]", twice),
            // The flow's own block stands, and no trigger stands on it.
            at("blocks/main.toml", "", NoReader { block: "main".into() }),
        ]
    );
}
