//! What a cell is run with: its line and its limits.

use flow::{FLOW_FILE, FaultKind::*};

use crate::support::{at, edit, faults, valid};

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
