//! What changed of a cell that both definitions have.

use decide::Condition;
use flow::{CellChange, Change, FLOW_FILE, Kind, Limit};

use crate::from_valid;
use crate::support::{edit, valid};

fn of(cell: &str, change: CellChange) -> Change {
    Change::Cell {
        cell: cell.into(),
        change,
    }
}

fn is(block: &str, value: &str) -> Option<Condition> {
    Some(Condition::Is {
        block: block.into(),
        value: value.to_string(),
    })
}

#[test]
fn a_cell_s_title_kind_and_line_are_each_a_change_with_both_values() {
    let after = edit(valid(), "judge.toml", "title = \"Judge the plan\"", "title = \"Judge\"");
    let after = edit(after, "judge.toml", "line = \"review\"", "line = \"engineer\"");
    let after = edit(
        after,
        "check.toml",
        "kind = \"engine\"",
        "kind = \"agent\"\nline = \"engineer\"",
    );
    assert_eq!(
        from_valid(&after),
        [
            of(
                "judge",
                CellChange::Title {
                    before: "Judge the plan".to_string(),
                    after: "Judge".to_string(),
                },
            ),
            of(
                "judge",
                CellChange::Line {
                    before: Some("review".to_string()),
                    after: Some("engineer".to_string()),
                },
            ),
            of(
                "check",
                CellChange::Kind {
                    before: Kind::Engine,
                    after: Kind::Agent,
                },
            ),
            of(
                "check",
                CellChange::Line {
                    before: None,
                    after: Some("engineer".to_string()),
                },
            ),
        ]
    );
}

#[test]
fn a_condition_that_changed_is_one_change_with_the_condition_before_and_after() {
    let after = edit(valid(), "design.toml", "is = \"yes\"", "is = \"no\"");
    let after = edit(
        after,
        "answer.toml",
        "when = { test = \"has_comment\" }",
        "when = { block = \"needs_design\", is = \"yes\" }",
    );
    let after = edit(
        after,
        "answer.toml",
        "requires = [\"plan_confirmed\"]",
        "requires = [\"plan_confirmed\", \"has_comment\"]",
    );
    assert_eq!(
        from_valid(&after),
        [
            of(
                "design",
                CellChange::Condition {
                    before: is("needs_design", "yes"),
                    after: is("needs_design", "no"),
                },
            ),
            of(
                "answer",
                CellChange::Condition {
                    before: Some(Condition::Test("has_comment".into())),
                    after: is("needs_design", "yes"),
                },
            ),
            of("answer", CellChange::PreconditionAdded("has_comment".into())),
        ]
    );
}

#[test]
fn a_condition_put_on_a_cell_that_had_none_is_a_change_from_none() {
    let after = edit(
        valid(),
        "check.toml",
        "kind = \"engine\"",
        "kind = \"engine\"\nwhen = { test = \"has_comment\" }",
    );
    assert_eq!(
        from_valid(&after),
        [of(
            "check",
            CellChange::Condition {
                before: None,
                after: Some(Condition::Test("has_comment".into())),
            },
        )]
    );
}

#[test]
fn each_input_output_and_precondition_taken_out_or_put_in_is_a_change() {
    let after = edit(
        valid(),
        "plan.toml",
        "outputs = [\"plan\"]",
        "outputs = [\"plan\", \"design\"]",
    );
    let after = edit(
        after,
        "judge.toml",
        "inputs = [\"plan\"]",
        "inputs = [\"request\", \"plan\"]",
    );
    let after = edit(
        after,
        "answer.toml",
        "inputs = [\"comments\", \"plan\"]",
        "inputs = [\"comments\"]",
    );
    let after = edit(
        after,
        "merge.toml",
        "requires = [\"may_merge\"]",
        "requires = [\"may_merge\", \"plan_confirmed\"]",
    );
    assert_eq!(
        from_valid(&after),
        [
            of("plan", CellChange::OutputAdded("design".into())),
            of("judge", CellChange::InputAdded("request".into())),
            of("answer", CellChange::InputRemoved("plan".into())),
            of("merge", CellChange::PreconditionAdded("plan_confirmed".into())),
        ]
    );
}

#[test]
fn a_list_in_another_order_with_the_same_names_changes_nothing() {
    let after = edit(
        valid(),
        "plan.toml",
        "inputs = [\"request\", \"opinions\"]",
        "inputs = [\"opinions\", \"request\"]",
    );
    let after = edit(
        after,
        "judge.toml",
        "outputs = [\"needs_design\", \"opinions\"]",
        "outputs = [\"opinions\", \"needs_design\"]",
    );
    assert_eq!(from_valid(&after), []);
}

#[test]
fn a_cell_s_own_limit_is_a_change_and_a_limit_that_only_follows_the_flow_s_is_not() {
    let after = edit(valid(), FLOW_FILE, "runs = 3", "runs = 4");
    // Plan sets the limit it had from the flow, judge one of its own, and answer changes its own.
    let after = edit(after, "plan.toml", "title = \"Plan\"", "title = \"Plan\"\nlimit = 3");
    let after = edit(
        after,
        "judge.toml",
        "title = \"Judge the plan\"",
        "title = \"Judge the plan\"\nlimit = 7",
    );
    let after = edit(after, "answer.toml", "limit = 5", "limit = 6");
    let limit = |before, after| CellChange::Limit {
        limit: Limit::Runs,
        before,
        after,
    };
    assert_eq!(
        from_valid(&after),
        [
            Change::Limit {
                limit: Limit::Runs,
                before: 3,
                after: 4,
            },
            of("judge", limit(3, 7)),
            of("answer", limit(5, 6)),
        ]
    );
}
