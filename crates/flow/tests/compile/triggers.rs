//! What runs a cell named apart from what it is given: its triggers, each an event worked out to the block it stands
//! on, and its context, each datum in its kind. A cell so written runs on the blocks its triggers stand on and is given
//! its context, as a cell written before triggers runs on and is given its blocks; and of such a cell, each block that
//! runs it is a trigger of the block's name. What runs a cell is defined beside the flows, once, and a block that
//! stands only for a trigger is never given, written or described by.

use decide::BlockId;
use flow::FaultKind::*;
use flow::{CellSpec, Compiled, FLOW_FILE, FaultKind, InputKind, Source, Trigger};

use crate::support::{at, compiled, edit, faults, laid_out, set, valid};

const PLAN_TRIGGERS: &str = r#"triggered_by = ["on_request", "on_opinions"]"#;
const PLAN_CONTEXT: &str = r#"context = ["request", "opinions"]"#;
const PLAN_KINDS: &str = "opinions = \"last_and_latest\"";

fn cell<'a>(compiled: &'a Compiled, id: &str) -> &'a CellSpec {
    (compiled.cells.iter()).find(|c| c.id.as_str() == id).expect("the cell")
}

fn trigger(name: &str, block: &str, read: &[&str], does: Option<&str>) -> Trigger {
    Trigger {
        name: name.to_string(),
        what: None,
        block: block.into(),
        group: Vec::new(),
        read: read.iter().map(|&b| b.into()).collect(),
        detail: None,
        does: does.map(str::to_string),
    }
}

fn defined_twice(name: &str, also: &str) -> FaultKind {
    DefinedTwice {
        name: name.to_string(),
        also: also.to_string(),
    }
}

/// The twin laid out one thing to a file, its planning cell naming these triggers in place of its own.
fn plan_triggers(triggers: &str) -> Source {
    edit(laid_out(), "cells/plan.toml", PLAN_TRIGGERS, triggers)
}

/// The twin laid out one thing to a file, with this after the rest of its planning cell's file.
fn plan_adds(text: &str) -> Source {
    let mut source = laid_out();
    source.cells.get_mut("plan").expect("the planning cell").push_str(text);
    source
}

/// The twin laid out one thing to a file, naming `discussion` among its data, its answering cell renamed `R1` and
/// run when the owner speaks to it as well, with the record of what was said in its context.
fn r1_spoken_to() -> Source {
    let source = edit(
        laid_out(),
        FLOW_FILE,
        "data = [\"request\"]",
        "data = [\"request\", \"discussion\"]",
    );
    let mut source = edit(source, FLOW_FILE, "\"answer\"", "\"R1\"");
    let answer = source.cells.remove("answer").expect("the answering cell");
    source.cells.insert("R1".to_string(), answer);
    let source = edit(
        source,
        "cells/R1.toml",
        r#"triggered_by = ["on_comments", "on_plan"]"#,
        r#"triggered_by = ["on_comments", "on_plan", "on_spoken"]"#,
    );
    let source = edit(
        source,
        "cells/R1.toml",
        r#"context = ["comments", "plan"]"#,
        r#"context = ["comments", "plan", "discussion"]"#,
    );
    edit(
        source,
        "cells/R1.toml",
        "plan = \"last_and_latest\"",
        "plan = \"last_and_latest\"\ndiscussion = \"all\"",
    )
}

#[test]
fn each_trigger_is_worked_out_to_the_block_it_stands_on_with_what_its_agent_reads_for_it() {
    let compiled = compiled(&laid_out());
    // A rule of `of_a_datum/` makes a trigger of each datum of the flow, which its agent reads for it.
    assert_eq!(
        cell(&compiled, "plan").triggers,
        [
            trigger("on_request", "request", &["request"], None),
            trigger("on_opinions", "opinions", &["opinions"], None),
        ]
    );
    // A trigger of its own stands on the block with no content it declares; an engine cell has no agent to read.
    assert_eq!(
        cell(&compiled, "check").triggers,
        [
            trigger("on_main", "main", &[], None),
            trigger("on_design", "design", &[], None),
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
    let source = plan_adds("\n[do]\non_request = \"Plan again from what changed.\"\n");
    let source = set(
        source,
        "../triggers/of_a_datum/changed.toml",
        "name = \"on_{datum}\"\nread = [\"{datum}\"]\ndo = \"See what changed in `{datum}`.\"",
    );
    let compiled = compiled(&source);
    let plan = cell(&compiled, "plan");
    assert_eq!(
        plan.triggers,
        [
            trigger(
                "on_request",
                "request",
                &["request"],
                Some("Plan again from what changed.")
            ),
            trigger(
                "on_opinions",
                "opinions",
                &["opinions"],
                Some("See what changed in `opinions`.")
            ),
        ]
    );
    assert_eq!(plan.runs_on[0].why.as_deref(), Some("Plan again from what changed."));
    assert_eq!(plan.runs_on[1].why.as_deref(), Some("See what changed in `opinions`."));
}

#[test]
fn what_a_trigger_says_of_what_happened_is_a_place_the_engine_fills() {
    let on_main = "block = \"main\"\nfrom = \"main\"\ndetail = \"{detail}\"";
    let source = set(
        laid_out(),
        "../triggers/on_main.toml",
        &on_main.replace("{detail}", "files"),
    );
    assert_eq!(
        cell(&compiled(&source), "check").triggers[0].detail.as_deref(),
        Some("files")
    );
    let source = set(
        laid_out(),
        "../triggers/on_main.toml",
        &on_main.replace("{detail}", "lines"),
    );
    let what = UnknownDetail { detail: "lines".into() };
    assert_eq!(faults(&source), [at("../triggers/on_main.toml", "detail", what)]);
}

#[test]
fn a_trigger_each_cell_has_its_own_of_gives_each_cell_that_names_it_its_own_block_by_its_id_in_small_letters() {
    let source = edit(
        r1_spoken_to(),
        "cells/plan.toml",
        PLAN_TRIGGERS,
        r#"triggered_by = ["on_request", "on_opinions", "on_spoken"]"#,
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
        "opinions = \"last_and_latest\"\ndiscussion = \"all\"\n\n[do]\non_spoken = \"Answer, then go on.\"",
    );
    let compiled = compiled(&source);
    assert_eq!(
        cell(&compiled, "plan").triggers[2],
        trigger(
            "on_spoken",
            "plan_spoken_to",
            &["discussion"],
            Some("Answer, then go on.")
        )
    );
    assert_eq!(
        cell(&compiled, "R1").triggers[2],
        trigger(
            "on_spoken",
            "r1_spoken_to",
            &["discussion"],
            Some("Answer what the owner said.")
        )
    );
    for (block, id) in [("plan_spoken_to", "plan"), ("r1_spoken_to", "R1")] {
        let declared = &compiled.blocks[&BlockId::from(block)];
        assert!(declared.trigger && declared.writers.people, "{block}");
        let readers: Vec<&str> = declared.readers.cells.iter().map(|c| c.as_str()).collect();
        assert_eq!(readers, [id], "{block}");
    }
    // The walk runs each cell on its own block.
    let walked = (compiled.flow.cells().iter())
        .find(|c| c.id.as_str() == "plan")
        .expect("plan");
    let runs_on: Vec<&str> = walked.runs_on.iter().map(|b| b.as_str()).collect();
    assert_eq!(runs_on, ["request", "opinions", "plan_spoken_to"]);
}

#[test]
fn the_name_of_a_cell_stands_in_all_a_trigger_each_cell_has_its_own_of_says() {
    let source = plan_triggers(r#"triggered_by = ["on_request", "on_opinions", "noted"]"#);
    let source = set(
        source,
        "../triggers/of_a_cell/noted.toml",
        "block = \"{cell}_noted\"\nfrom = \"owner\"\nread = [\"{cell}_notes\"]\ndo = \"Take in the notes on {cell}.\"",
    );
    let mut source = set(source, "blocks/plan_notes.toml", "from = \"owner\"");
    let plan = source.cells.get_mut("plan").expect("the planning cell");
    *plan = plan.replace(PLAN_CONTEXT, r#"context = ["request", "opinions", "plan_notes"]"#);
    plan.push_str("plan_notes = \"latest\"\n");
    let compiled = compiled(&source);
    assert_eq!(
        cell(&compiled, "plan").triggers[2],
        trigger(
            "noted",
            "plan_noted",
            &["plan_notes"],
            Some("Take in the notes on plan.")
        )
    );
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
fn a_cell_names_its_triggers_only_in_a_flow_laid_out_one_thing_to_a_file() {
    let mut source = edit(
        valid(),
        "plan.toml",
        r#"inputs = ["request", "opinions"]"#,
        &format!("{PLAN_TRIGGERS}\n{PLAN_CONTEXT}"),
    );
    let plan = source.cells.get_mut("plan").expect("the planning cell");
    plan.push_str("\n[kinds]\nrequest = \"latest\"\nopinions = \"latest\"\n");
    assert_eq!(faults(&source), [at("plan.toml", "triggered_by", NotLaidOut)]);
}

#[test]
fn a_trigger_nothing_beside_the_flows_defines_or_named_twice_is_refused() {
    let source =
        plan_triggers(r#"triggered_by = ["on_request", "on_opinions", "on_ghost", "nothing_happened", "on_request"]"#);
    let no_datum = |trigger: &str, datum: &str| NoSuchDatum {
        trigger: trigger.to_string(),
        datum: datum.to_string(),
    };
    assert_eq!(
        faults(&source),
        [
            at("cells/plan.toml", "triggered_by[2]", no_datum("on_ghost", "ghost")),
            at(
                "cells/plan.toml",
                "triggered_by[3]",
                NoSuchTrigger {
                    trigger: "nothing_happened".into()
                }
            ),
            at(
                "cells/plan.toml",
                "triggered_by[4]",
                Twice {
                    name: "on_request".into()
                }
            ),
        ]
    );
    // A block a trigger declares is no datum of the flow, which a rule of `of_a_datum/` makes a trigger of: `on_answer`
    // declares `check_answered` for the cell, and `on_check_answered` is not made of it.
    let source = edit(
        laid_out(),
        "cells/check.toml",
        r#"triggered_by = ["on_main", "on_design"]"#,
        r#"triggered_by = ["on_main", "on_design", "on_answer", "on_check_answered"]"#,
    );
    assert_eq!(
        faults(&source),
        [at(
            "cells/check.toml",
            "triggered_by[3]",
            no_datum("on_check_answered", "check_answered")
        )]
    );
}

#[test]
fn a_trigger_of_an_agent_cell_has_its_agent_read_only_what_the_cell_s_context_holds() {
    let source = edit(
        laid_out(),
        "cells/judge.toml",
        r#"triggered_by = ["on_plan"]"#,
        r#"triggered_by = ["on_plan", "on_request"]"#,
    );
    let what = ReadNotInContext {
        trigger: "on_request".into(),
        block: "request".into(),
    };
    assert_eq!(faults(&source), [at("cells/judge.toml", "triggered_by[1]", what)]);
    // An engine cell is given nothing, and its triggers have no agent to read for them.
    let source = edit(
        laid_out(),
        "cells/check.toml",
        r#"triggered_by = ["on_main", "on_design"]"#,
        r#"triggered_by = ["on_main", "on_design", "on_request"]"#,
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
fn a_block_that_stands_only_for_a_trigger_is_in_no_context_and_no_cell_writes_it() {
    let source = edit(
        laid_out(),
        "cells/plan.toml",
        PLAN_CONTEXT,
        r#"context = ["request", "opinions", "main"]"#,
    );
    let source = edit(
        source,
        "cells/plan.toml",
        "outputs = [\"plan\"]",
        "outputs = [\"plan\", \"main\"]",
    );
    let source = edit(
        source,
        "cells/plan.toml",
        PLAN_KINDS,
        &format!("{PLAN_KINDS}\nmain = \"latest\""),
    );
    let only = || TriggerOnly { block: "main".into() };
    assert_eq!(
        faults(&source),
        [
            at("cells/plan.toml", "context[2]", only()),
            at("cells/plan.toml", "outputs[1]", only()),
        ]
    );
}

#[test]
fn an_agent_cell_that_no_trigger_runs_is_refused() {
    assert_eq!(
        faults(&plan_triggers("triggered_by = []")),
        [at("cells/plan.toml", "triggered_by", RunByNothing)]
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
fn a_trigger_whose_block_the_flow_defines_as_well_is_refused_once_on_the_flow_s_block() {
    let source = set(laid_out(), "blocks/main.toml", "from = \"main\"");
    // Two cells naming the trigger make one fault.
    let source = edit(
        source,
        "cells/merge.toml",
        r#"triggered_by = ["on_ci"]"#,
        r#"triggered_by = ["on_ci", "on_main"]"#,
    );
    // With `main` a datum of the flow, the rule `on_{datum}` makes `on_main` of it as well.
    assert_eq!(
        faults(&source),
        [
            at(
                "../triggers/on_main.toml",
                "",
                defined_twice("on_main", "../triggers/of_a_datum/changed.toml")
            ),
            at(
                "blocks/main.toml",
                "",
                defined_twice("main", "../triggers/on_main.toml")
            ),
        ]
    );
}

#[test]
fn a_block_that_two_triggers_declare_is_refused_on_the_second() {
    // Two files of triggers of their own.
    let source = set(
        laid_out(),
        "../triggers/on_merged.toml",
        "block = \"main\"\nfrom = \"main\"",
    );
    let source = edit(
        source,
        "cells/merge.toml",
        r#"triggered_by = ["on_ci"]"#,
        r#"triggered_by = ["on_ci", "on_merged"]"#,
    );
    assert_eq!(
        faults(&source),
        [at(
            "../triggers/on_merged.toml",
            "block",
            defined_twice("main", "../triggers/on_main.toml")
        )]
    );
    // One file of a trigger each cell has its own of, and two cells whose ids are the same in small letters.
    let source = edit(r1_spoken_to(), FLOW_FILE, "\"merge\"]", "\"r1\", \"merge\"]");
    let source = set(
        source,
        "cells/r1.toml",
        "title = \"Look\"\nkind = \"engine\"\nact = \"check\"\ntriggered_by = [\"on_spoken\"]",
    );
    assert_eq!(
        faults(&source),
        [at(
            "cells/r1.toml",
            "triggered_by[0]",
            defined_twice("r1_spoken_to", "cells/R1.toml")
        )]
    );
}

#[test]
fn a_trigger_of_a_file_that_a_rule_makes_of_a_datum_of_the_flow_as_well_is_refused() {
    let source = set(
        laid_out(),
        "../triggers/on_plan.toml",
        "block = \"plan_poked\"\nfrom = \"owner\"",
    );
    assert_eq!(
        faults(&source),
        [at(
            "../triggers/on_plan.toml",
            "",
            defined_twice("on_plan", "../triggers/of_a_datum/changed.toml")
        )]
    );
}

#[test]
fn a_file_beside_the_flows_that_stands_for_each_cell_or_each_datum_holds_the_name_that_stands_for_it() {
    let source = set(
        laid_out(),
        "../triggers/of_a_cell/on_spoken.toml",
        "block = \"spoken_to\"\nfrom = \"owner\"",
    );
    let cell_missing = MissingPlaceholder {
        placeholder: "{cell}".into(),
    };
    assert_eq!(
        faults(&source),
        [at("../triggers/of_a_cell/on_spoken.toml", "block", cell_missing)]
    );
    let source = set(laid_out(), "../triggers/of_a_datum/new.toml", "name = \"news\"");
    let datum_missing = MissingPlaceholder {
        placeholder: "{datum}".into(),
    };
    assert_eq!(
        faults(&source),
        [at("../triggers/of_a_datum/new.toml", "name", datum_missing)]
    );
}
