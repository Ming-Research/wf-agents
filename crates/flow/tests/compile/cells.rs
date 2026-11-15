//! What a cell is run with: its line, its task or its act, and its limits.

use flow::{FLOW_FILE, FaultKind::*, Has, Source};

use crate::support::{at, compiled, edit, faults, faults_with, has, laid_out, set, valid};

#[test]
fn an_agent_cell_runs_in_a_declared_line() {
    let source = edit(valid(), "plan.toml", "line = \"engineer\"\n", "");
    assert_eq!(faults(&source), [at("plan.toml", "line", NoLine)]);

    let source = edit(valid(), "plan.toml", "line = \"engineer\"", "line = \"designer\"");
    let what = NoSuchLine {
        line: "designer".to_string(),
    };
    assert_eq!(faults(&source), [at("plan.toml", "line", what)]);
}

#[test]
fn a_cell_on_a_line_that_only_reads_only_reads_and_the_others_write() {
    let source = edit(
        valid(),
        FLOW_FILE,
        "[lines.review]",
        "[lines.review]\nreads_only = true",
    );
    let flow = compiled(&source);
    let reads_only = |id: &str| {
        let spec = flow.cells.iter().find(|c| c.id.as_str() == id);
        spec.expect("the cell is compiled").reads_only
    };
    assert!(reads_only("judge"), "the judge is on the line that only reads");
    assert!(!reads_only("plan"), "the plan's line writes");
    assert!(!reads_only("check"), "an engine cell is on no line");
    assert!(
        !compiled(&valid()).cells.iter().any(|c| c.reads_only),
        "a line writes unless it says so"
    );
}

#[test]
fn an_engine_cell_runs_in_no_line() {
    let source = edit(
        valid(),
        "check.toml",
        "kind = \"engine\"",
        "kind = \"engine\"\nline = \"engineer\"",
    );
    let what = EngineHasLine {
        line: "engineer".to_string(),
    };
    assert_eq!(faults(&source), [at("check.toml", "line", what)]);
}

#[test]
fn an_agent_cell_has_a_task() {
    let source = edit(
        valid(),
        "plan.toml",
        "task = \"Write the plan from the request.\"\n",
        "",
    );
    assert_eq!(faults(&source), [at("plan.toml", "task", NoTask)]);
}

#[test]
fn a_task_of_white_space_alone_is_no_task() {
    let source = edit(
        valid(),
        "plan.toml",
        "task = \"Write the plan from the request.\"",
        "task = \" \\n\\t \"",
    );
    assert_eq!(faults(&source), [at("plan.toml", "task", NoTask)]);
}

#[test]
fn an_engine_cell_has_no_task() {
    let source = edit(
        valid(),
        "check.toml",
        "act = \"check\"",
        "act = \"check\"\ntask = \"Check the work.\"",
    );
    assert_eq!(faults(&source), [at("check.toml", "task", EngineHasTask)]);

    // A task of white space alone is still one that no agent reads.
    let source = edit(
        valid(),
        "check.toml",
        "act = \"check\"",
        "act = \"check\"\ntask = \" \"",
    );
    assert_eq!(faults(&source), [at("check.toml", "task", EngineHasTask)]);
}

#[test]
fn an_engine_cell_with_a_task_and_an_act_the_engine_lacks_has_both_faults_the_task_first() {
    let source = edit(
        valid(),
        "check.toml",
        "act = \"check\"",
        "act = \"deploy\"\ntask = \"Check the work.\"",
    );
    let what = UnknownAct {
        act: "deploy".to_string(),
    };
    assert_eq!(
        faults(&source),
        [at("check.toml", "task", EngineHasTask), at("check.toml", "act", what)]
    );
}

#[test]
fn an_engine_cell_names_an_act() {
    let source = edit(valid(), "check.toml", "act = \"check\"\n", "");
    assert_eq!(faults(&source), [at("check.toml", "act", NoAct)]);
}

#[test]
fn an_agent_cell_names_no_act() {
    let source = edit(
        valid(),
        "plan.toml",
        "line = \"engineer\"",
        "line = \"engineer\"\nact = \"check\"",
    );
    let what = AgentHasAct {
        act: "check".to_string(),
    };
    assert_eq!(faults(&source), [at("plan.toml", "act", what)]);
}

#[test]
fn an_engine_cell_names_an_act_the_engine_has() {
    let source = edit(valid(), "check.toml", "act = \"check\"", "act = \"deploy\"");
    let what = UnknownAct {
        act: "deploy".to_string(),
    };
    assert_eq!(faults(&source), [at("check.toml", "act", what)]);

    // An empty name is a name the engine has no act of, not a cell that names none.
    let source = edit(valid(), "check.toml", "act = \"check\"", "act = \"\"");
    let what = UnknownAct { act: String::new() };
    assert_eq!(faults(&source), [at("check.toml", "act", what)]);

    let no_acts = Has {
        acts: Default::default(),
        ..has()
    };
    let unknown = |act: &str| UnknownAct { act: act.to_string() };
    assert_eq!(
        faults_with(&valid(), &no_acts),
        [
            at("check.toml", "act", unknown("check")),
            at("merge.toml", "act", unknown("merge")),
        ]
    );
}

#[test]
fn every_limit_is_a_positive_count() {
    let limits = "runs = 3\nreturns = 2\ncalls = 40";
    let source = edit(valid(), FLOW_FILE, limits, "runs = 0\nreturns = -1\ncalls = 4294967296");
    let source = edit(source, "answer.toml", "limit = 5", "limit = 0");
    let out = |value| LimitOutOfRange { value };
    assert_eq!(
        faults(&source),
        [
            at(FLOW_FILE, "limits.runs", out(0)),
            at(FLOW_FILE, "limits.returns", out(-1)),
            at(FLOW_FILE, "limits.calls", out(4_294_967_296)),
            at("answer.toml", "limit", out(0)),
        ]
    );
}

/// The small flow laid out one thing to a file, with the plan's task written in a file of the flow named so, which
/// holds this text where it is there.
fn plan_task_in(file: &str, text: Option<&str>) -> Source {
    let source = edit(
        laid_out(),
        "cells/plan.toml",
        "task = \"Write the plan from the request.\"",
        &format!("task = {{ file = \"{file}\" }}"),
    );
    match text {
        Some(text) => set(source, "tasks/plan.md", text),
        None => source,
    }
}

#[test]
fn a_task_may_be_written_in_a_file_of_the_flow_and_is_read_as_if_written_in_place() {
    let text = "Write the plan from the request.\nSay what it rests on.\n";
    for file in ["tasks/plan.md", "./tasks/plan.md"] {
        let compiled = compiled(&plan_task_in(file, Some(text)));
        let plan = &compiled.cells[0];
        assert_eq!(plan.task.as_deref(), Some(text), "{file}");
        assert_eq!(plan.task_file.as_deref(), Some(file));
    }
    // A task written in place names no file.
    assert_eq!(compiled(&laid_out()).cells[0].task_file, None);
}

#[test]
fn a_task_s_file_is_one_of_the_flow_s_that_holds_words() {
    let file = |f: &str| f.to_string();
    let missing = NoTaskFile {
        file: file("tasks/plan.md"),
    };
    assert_eq!(
        faults(&plan_task_in("tasks/plan.md", None)),
        [at("cells/plan.toml", "task.file", missing)]
    );
    let empty = TaskFileEmpty {
        file: file("tasks/plan.md"),
    };
    assert_eq!(
        faults(&plan_task_in("tasks/plan.md", Some(" \n"))),
        [at("cells/plan.toml", "task.file", empty)]
    );
    let outside = TaskFileOutside {
        file: file("../tasks/plan.md"),
    };
    assert_eq!(
        faults(&plan_task_in("../tasks/plan.md", Some("Plan."))),
        [at("cells/plan.toml", "task.file", outside)]
    );
}
