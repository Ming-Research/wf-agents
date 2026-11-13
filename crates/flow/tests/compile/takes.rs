//! What runs a cell and what it is given, and what a block is: each is carried into what the flow compiles to as it was
//! written. In the new form an agent cell is run by something, says the kind of each block it is given, says why a
//! block runs it only of one that does and how it is given one only of one it is given, and no table of it says
//! nothing. A file in the old form, with one list `inputs`, is read as both lists, and compiles to what the same cell in
//! the new form does.

use decide::BlockId;
use flow::FaultKind::*;
use flow::{CellSpec, Compiled, FLOW_FILE, Given, InputKind, RunsOn};

use crate::support::{at, compiled, edit, faults, valid};

const JUDGE_OUTPUTS: &str = r#"outputs = ["needs_design", "opinions"]"#;
const PLAN_INPUTS: &str = r#"inputs = ["request", "opinions"]"#;

/// The small flow whose judge says this of the blocks it takes, after the rest of its file.
fn judge_takes(tables: &str) -> flow::Source {
    edit(
        valid(),
        "judge.toml",
        JUDGE_OUTPUTS,
        &format!("{JUDGE_OUTPUTS}\n\n{tables}"),
    )
}

/// The small flow whose planning cell names, in place of its `inputs`, the lists this says; the tables it says, from
/// the first, go at the end of the file, after the cell's other fields.
fn plan_names(said: &str) -> flow::Source {
    let (lists, tables) = said.split_once("\n\n[").unwrap_or((said, ""));
    let mut source = edit(valid(), "plan.toml", PLAN_INPUTS, lists);
    if !tables.is_empty() {
        let text = source.cells.get_mut("plan").expect("the planning cell's file");
        text.push_str(&format!("\n[{tables}\n"));
    }
    source
}

fn cell<'a>(compiled: &'a Compiled, id: &str) -> &'a CellSpec {
    (compiled.cells.iter()).find(|c| c.id.as_str() == id).expect("the cell")
}

fn runs_on(block: &str, why: Option<&str>) -> RunsOn {
    RunsOn {
        block: block.into(),
        why: why.map(str::to_string),
    }
}

fn given(block: &str, kind: InputKind, why: Option<&str>) -> Given {
    Given {
        block: block.into(),
        kind,
        why: why.map(str::to_string),
    }
}

#[test]
fn how_an_agent_cell_takes_a_block_in_the_old_form_and_what_a_block_is_are_compiled_as_written() {
    let source = judge_takes("[input.plan]\nkind = \"all\"\nwhy = \"It is what to judge.\"");
    let source = edit(
        source,
        FLOW_FILE,
        "[blocks.plan]",
        "[blocks.plan]\nwhat = \"What will be done, and in what order.\"",
    );
    let compiled = compiled(&source);
    // Each block of an agent cell's `inputs` runs it and is given to it, as `[input.<block>]` says.
    let judge = cell(&compiled, "judge");
    assert_eq!(judge.runs_on, [runs_on("plan", None)]);
    assert_eq!(
        judge.given,
        [given("plan", InputKind::All, Some("It is what to judge."))]
    );
    // Of a block it names no kind of, an agent cell in the old form is given the version it last worked from and the
    // latest, as the engine gives a round its inputs before it fills a session by kind.
    let plan = cell(&compiled, "plan");
    assert_eq!(plan.runs_on, [runs_on("request", None), runs_on("opinions", None)]);
    assert_eq!(
        plan.given,
        [
            given("request", InputKind::LastAndLatest, None),
            given("opinions", InputKind::LastAndLatest, None),
        ]
    );
    let what = |block: &str| compiled.blocks[&BlockId::from(block)].what.clone();
    assert_eq!(what("plan"), Some("What will be done, and in what order.".to_string()));
    assert_eq!(what("opinions"), None);
}

#[test]
fn each_of_the_kinds_is_written_by_its_name_and_the_why_may_be_left_out() {
    let source = plan_names(
        "runs_on = [\"request\", \"opinions\"]\ngiven = [\"request\", \"opinions\", \"plan\"]\n\n\
         [give.request]\nkind = \"latest\"\n\n[give.opinions]\nkind = \"last_and_latest\"\n\
         why = \"They say what to change.\"\n\n[give.plan]\nkind = \"all\"",
    );
    let compiled = compiled(&source);
    assert_eq!(
        cell(&compiled, "plan").given,
        [
            given("request", InputKind::Latest, None),
            given("opinions", InputKind::LastAndLatest, Some("They say what to change.")),
            given("plan", InputKind::All, None),
        ]
    );
}

#[test]
fn in_the_new_form_a_block_given_names_its_kind() {
    // The opinions have a why and no kind; the request has nothing said of it.
    let source = plan_names(
        "runs_on = [\"request\"]\ngiven = [\"request\", \"opinions\"]\n\n\
         [give.opinions]\nwhy = \"They say what to change.\"",
    );
    let no_kind = |block: &str| NoKind {
        block: block.to_string(),
    };
    assert_eq!(
        faults(&source),
        [
            at("plan.toml", "given[0]", no_kind("request")),
            at("plan.toml", "given[1]", no_kind("opinions")),
        ]
    );
}

#[test]
fn in_the_new_form_an_agent_cell_that_nothing_runs_is_refused() {
    let source = plan_names(
        "given = [\"request\", \"opinions\"]\n\n[give.request]\nkind = \"latest\"\n\n[give.opinions]\nkind = \"all\"",
    );
    assert_eq!(faults(&source), [at("plan.toml", "runs_on", RunByNothing)]);
}

#[test]
fn a_table_that_says_nothing_is_refused() {
    let source = plan_names(
        "runs_on = [\"request\", \"opinions\"]\ngiven = [\"request\"]\n\n[run.request]\n\n[give.request]\nkind = \"latest\"",
    );
    let says_nothing = |block: &str| SaysNothing {
        block: block.to_string(),
    };
    assert_eq!(
        faults(&source),
        [at("plan.toml", "run.request", says_nothing("request"))]
    );
    let source = plan_names(
        "runs_on = [\"request\"]\ngiven = [\"request\", \"opinions\"]\n\n[give.request]\n\n\
         [give.opinions]\nkind = \"all\"",
    );
    assert_eq!(
        faults(&source),
        [
            at(
                "plan.toml",
                "given[0]",
                NoKind {
                    block: "request".to_string(),
                },
            ),
            at("plan.toml", "give.request", says_nothing("request")),
        ]
    );
}

#[test]
fn a_cell_says_how_it_takes_only_a_block_among_its_inputs() {
    // The judge writes the opinions and does not read them; nowhere is declared nowhere.
    let source = judge_takes("[input.opinions]\nkind = \"latest\"\n\n[input.nowhere]\nwhy = \"None.\"");
    let not_read = |block: &str| TakenNotRead {
        block: block.to_string(),
    };
    assert_eq!(
        faults(&source),
        [
            at("judge.toml", "input.nowhere", not_read("nowhere")),
            at("judge.toml", "input.opinions", not_read("opinions")),
        ]
    );
}

#[test]
fn an_engine_cell_in_the_old_form_is_given_nothing_and_what_it_says_of_an_input_is_why_it_runs_it() {
    let source = edit(
        valid(),
        "check.toml",
        "outputs = [\"ci\"]",
        "outputs = [\"ci\"]\n\n[input.main]\nwhy = \"The work is checked with the latest main.\"",
    );
    let compiled = compiled(&source);
    let check = cell(&compiled, "check");
    assert_eq!(
        check.runs_on,
        [
            runs_on("main", Some("The work is checked with the latest main.")),
            runs_on("design", None),
        ]
    );
    assert_eq!(check.given, []);
    // It has no session to keep supplied, so it says of no input what its context must hold.
    let source = edit(
        source,
        "check.toml",
        "[input.main]",
        "[input.design]\nkind = \"latest\"\n\n[input.main]",
    );
    let given_design = EngineIsGiven {
        block: "design".to_string(),
    };
    assert_eq!(faults(&source), [at("check.toml", "input.design.kind", given_design)]);
}

#[test]
fn what_runs_a_cell_and_what_it_is_given_are_compiled_apart_each_in_its_own_order() {
    let source = plan_names(
        "runs_on = [\"request\"]\ngiven = [\"opinions\", \"request\"]\n\n[run.request]\nwhy = \"It is what to plan.\"\n\n\
         [give.opinions]\nkind = \"all\"\nwhy = \"They say what to change.\"\n\n[give.request]\nkind = \"latest\"",
    );
    let compiled = compiled(&source);
    let plan = cell(&compiled, "plan");
    assert_eq!(plan.runs_on, [runs_on("request", Some("It is what to plan."))]);
    assert_eq!(
        plan.given,
        [
            given("opinions", InputKind::All, Some("They say what to change.")),
            given("request", InputKind::Latest, None),
        ]
    );
    // The walk is given only what runs the cell; the cells that read a block are those it runs and those given it.
    let walked = &compiled.flow.cells()[0];
    assert_eq!(walked.runs_on, [BlockId::from("request")]);
    let readers = |block: &str| compiled.blocks[&BlockId::from(block)].readers.cells.clone();
    assert_eq!(readers("opinions"), ["plan".into()]);
}

#[test]
fn a_block_that_only_runs_a_cell_or_that_it_is_only_given_is_read_by_it() {
    // The answering cell runs on the comments without being given them, and is given the plan that does not run it.
    let source = edit(
        valid(),
        "answer.toml",
        "inputs = [\"comments\", \"plan\"]",
        "runs_on = [\"comments\"]\ngiven = [\"plan\"]",
    );
    let source = edit(
        source,
        "answer.toml",
        "limit = 5",
        "limit = 5\n\n[give.plan]\nkind = \"latest\"",
    );
    let compiled = compiled(&source);
    let answer = cell(&compiled, "answer");
    assert_eq!(answer.runs_on, [runs_on("comments", None)]);
    assert_eq!(answer.given, [given("plan", InputKind::Latest, None)]);
    let readers = |block: &str| compiled.blocks[&BlockId::from(block)].readers.cells.clone();
    assert_eq!(readers("comments"), ["answer".into()]);
    assert_eq!(readers("plan"), ["judge".into(), "design".into(), "answer".into()]);
}

#[test]
fn an_engine_cell_is_given_nothing() {
    let source = edit(
        valid(),
        "check.toml",
        "inputs = [\"main\", \"design\"]",
        "runs_on = [\"main\", \"design\"]\ngiven = [\"design\"]",
    );
    let given_design = EngineIsGiven {
        block: "design".to_string(),
    };
    assert_eq!(faults(&source), [at("check.toml", "given[0]", given_design)]);
}

#[test]
fn a_block_named_twice_in_one_list_is_refused_and_in_both_lists_is_not() {
    let source = plan_names(
        "runs_on = [\"request\", \"request\"]\ngiven = [\"opinions\", \"request\", \"opinions\"]\n\n\
         [give.opinions]\nkind = \"all\"\n\n[give.request]\nkind = \"latest\"",
    );
    let twice = |name: &str| Twice { name: name.to_string() };
    assert_eq!(
        faults(&source),
        [
            at("plan.toml", "runs_on[1]", twice("request")),
            at("plan.toml", "given[2]", twice("opinions")),
        ]
    );
}

#[test]
fn a_block_not_declared_is_refused_once_where_it_is_first_named() {
    let source = plan_names(
        "runs_on = [\"request\", \"agenda\"]\ngiven = [\"agenda\", \"opinions\", \"minutes\"]\n\n\
         [give.agenda]\nkind = \"all\"\n\n[give.opinions]\nkind = \"all\"\n\n[give.minutes]\nkind = \"all\"",
    );
    let missing = |block: &str| NoSuchBlock {
        block: block.to_string(),
    };
    assert_eq!(
        faults(&source),
        [
            at("plan.toml", "runs_on[1]", missing("agenda")),
            at("plan.toml", "given[2]", missing("minutes")),
        ]
    );
}

#[test]
fn a_cell_says_why_a_block_runs_it_only_of_one_that_does_and_how_it_is_given_one_only_of_one_it_is_given() {
    // The request only runs the cell, so it has no kind; the opinions are only given, so they have no why of running.
    let source = plan_names(
        "runs_on = [\"request\"]\ngiven = [\"opinions\"]\n\n[run.opinions]\nwhy = \"None.\"\n\n\
         [give.opinions]\nkind = \"all\"\n\n[give.request]\nkind = \"latest\"",
    );
    assert_eq!(
        faults(&source),
        [
            at(
                "plan.toml",
                "run.opinions",
                NotRunOn {
                    block: "opinions".to_string(),
                },
            ),
            at(
                "plan.toml",
                "give.request",
                NotGiven {
                    block: "request".to_string(),
                },
            ),
        ]
    );
}

#[test]
fn a_cell_names_what_it_takes_in_one_form_only() {
    let source = edit(
        valid(),
        "plan.toml",
        PLAN_INPUTS,
        &format!("{PLAN_INPUTS}\nruns_on = [\"request\", \"opinions\"]"),
    );
    assert_eq!(faults(&source), [at("plan.toml", "inputs", TwoForms)]);
    let source = plan_names("runs_on = [\"request\", \"opinions\"]\n\n[input.request]\nkind = \"latest\"");
    assert_eq!(faults(&source), [at("plan.toml", "input", TwoForms)]);
}

/// The kind each agent cell of the small flow is given each of its inputs as, in the forms below.
const KINDS: [(&str, &str, &str); 6] = [
    ("plan", "request", "latest"),
    ("plan", "opinions", "all"),
    ("judge", "plan", "last_and_latest"),
    ("design", "plan", "latest"),
    ("answer", "comments", "all"),
    ("answer", "plan", "latest"),
];

/// The small flow with every agent cell saying in the old form, in `[input.<block>]`, the kind of each of its inputs.
fn valid_with_kinds() -> flow::Source {
    let mut source = valid();
    for (cell, block, kind) in KINDS {
        let text = source.cells.get_mut(cell).expect("the cell's file");
        text.push_str(&format!("\n[input.{block}]\nkind = \"{kind}\"\n"));
    }
    source
}

/// The small flow with every cell written in the new form, saying what [`valid_with_kinds`] says in the old.
fn valid_in_the_new_form() -> flow::Source {
    let both = |list: &str| format!("runs_on = {list}\ngiven = {list}");
    let mut source = valid();
    for (file, list) in [
        ("plan.toml", r#"["request", "opinions"]"#),
        ("judge.toml", r#"["plan"]"#),
        ("design.toml", r#"["plan"]"#),
        ("answer.toml", r#"["comments", "plan"]"#),
    ] {
        source = edit(source, file, &format!("inputs = {list}"), &both(list));
    }
    for (file, list) in [("check.toml", r#"["main", "design"]"#), ("merge.toml", r#"["ci"]"#)] {
        source = edit(source, file, &format!("inputs = {list}"), &format!("runs_on = {list}"));
    }
    for (cell, block, kind) in KINDS {
        let text = source.cells.get_mut(cell).expect("the cell's file");
        text.push_str(&format!("\n[give.{block}]\nkind = \"{kind}\"\n"));
    }
    source
}

#[test]
fn a_definition_in_the_old_form_and_the_same_in_the_new_compile_to_the_same_walk_and_the_same_cells() {
    let old = compiled(&valid_with_kinds());
    let new = compiled(&valid_in_the_new_form());
    assert_eq!(new.flow, old.flow);
    assert_eq!(new.cells, old.cells);
    assert_eq!(new.blocks, old.blocks);
    assert_eq!(flow::compare(&old, &new), []);
}
