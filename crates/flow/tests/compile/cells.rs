//! What a cell is run with: its line, its task or its act, and its limits.

use flow::{FLOW_FILE, FaultKind::*, Has};

use crate::support::{at, edit, faults, faults_with, has, valid};

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
