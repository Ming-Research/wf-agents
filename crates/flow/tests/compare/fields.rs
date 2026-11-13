//! What changed of a cell that both definitions have.

use decide::Condition;
use flow::{CellChange, Change, Effort, FLOW_FILE, InputKind, Kind, Limit};

use crate::support::{edit, valid};
use crate::{compiled, from_valid};

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
fn a_cell_s_title_kind_line_task_and_act_are_each_a_change_with_both_values() {
    let after = edit(valid(), "judge.toml", "title = \"Judge the plan\"", "title = \"Judge\"");
    let after = edit(after, "judge.toml", "line = \"review\"", "line = \"engineer\"");
    let after = edit(
        after,
        "check.toml",
        "kind = \"engine\"\nact = \"check\"",
        "kind = \"agent\"\neffort = \"high\"\nline = \"engineer\"\ntask = \"Check the work with the latest main.\"",
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
            of(
                "check",
                CellChange::Task {
                    before: None,
                    after: Some("Check the work with the latest main.".to_string()),
                },
            ),
            of(
                "check",
                CellChange::Act {
                    before: Some("check".to_string()),
                    after: None,
                },
            ),
            of(
                "check",
                CellChange::Effort {
                    before: None,
                    after: Some(Effort::High),
                },
            ),
        ]
    );
}

#[test]
fn a_line_that_comes_to_only_read_is_a_change_of_each_cell_on_it() {
    let after = edit(
        valid(),
        FLOW_FILE,
        "[lines.review]",
        "[lines.review]\nreads_only = true",
    );
    let only_reads = CellChange::ReadsOnly {
        before: false,
        after: true,
    };
    assert_eq!(from_valid(&after), [of("judge", only_reads)]);
    // A cell moved to a line that writes from one that only reads is a change of its line and of what it may do.
    let moved = edit(after.clone(), "judge.toml", "line = \"review\"", "line = \"engineer\"");
    let line = CellChange::Line {
        before: Some("review".to_string()),
        after: Some("engineer".to_string()),
    };
    let writes = CellChange::ReadsOnly {
        before: true,
        after: false,
    };
    assert_eq!(
        flow::compare(&compiled(&after), &compiled(&moved)),
        [of("judge", line), of("judge", writes)]
    );
}

#[test]
fn a_task_reworded_and_an_act_renamed_are_each_a_change_of_their_cell() {
    let after = edit(
        valid(),
        "plan.toml",
        "task = \"Write the plan from the request.\"",
        "task = \"Write the plan from the request.\\nName what it leaves out.\"",
    );
    let after = edit(after, "check.toml", "act = \"check\"", "act = \"test\"");
    assert_eq!(
        from_valid(&after),
        [
            of(
                "plan",
                CellChange::Task {
                    before: Some("Write the plan from the request.".to_string()),
                    after: Some("Write the plan from the request.\nName what it leaves out.".to_string()),
                },
            ),
            of(
                "check",
                CellChange::Act {
                    before: Some("check".to_string()),
                    after: Some("test".to_string()),
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
            // In the old form a block of an agent cell's `inputs` runs it and is given to it.
            of("judge", CellChange::RunsOnAdded("request".into())),
            of("judge", CellChange::GivenAdded("request".into())),
            of("answer", CellChange::RunsOnRemoved("plan".into())),
            of("answer", CellChange::GivenRemoved("plan".into())),
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
fn a_block_a_cell_must_write_put_in_or_taken_out_is_a_change_after_its_outputs() {
    let outputs = "outputs = [\"needs_design\", \"opinions\"]";
    let before = edit(
        valid(),
        "judge.toml",
        outputs,
        &format!("{outputs}\nmust_write = [\"opinions\"]"),
    );
    let after = edit(
        valid(),
        "judge.toml",
        outputs,
        "outputs = [\"needs_design\", \"opinions\", \"plan\"]\nmust_write = [\"needs_design\"]",
    );
    assert_eq!(
        flow::compare(&compiled(&before), &compiled(&after)),
        [
            of("judge", CellChange::OutputAdded("plan".into())),
            of("judge", CellChange::MustWriteRemoved("opinions".into())),
            of("judge", CellChange::MustWriteAdded("needs_design".into())),
        ]
    );
    // The same blocks in another order change nothing.
    let both = |list: &str| {
        edit(
            valid(),
            "judge.toml",
            outputs,
            &format!("{outputs}\nmust_write = {list}"),
        )
    };
    let (a, b) = (
        both("[\"opinions\", \"needs_design\"]"),
        both("[\"needs_design\", \"opinions\"]"),
    );
    assert_eq!(flow::compare(&compiled(&a), &compiled(&b)), []);
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

/// The small flow whose judge names what it takes in the new form: the lists this says in place of its `inputs`, and
/// the tables it says, from the first, at the end of the file, after the cell's other fields.
fn judge_takes(said: &str) -> flow::Source {
    let (lists, tables) = said.split_once("\n\n[").unwrap_or((said, ""));
    let mut source = edit(valid(), "judge.toml", "inputs = [\"plan\"]", lists);
    if !tables.is_empty() {
        let text = source.cells.get_mut("judge").expect("the judge's file");
        text.push_str(&format!("\n[{tables}\n"));
    }
    source
}

#[test]
fn how_a_cell_is_given_a_block_and_why_a_block_runs_it_are_each_a_change_with_both_values() {
    let before = judge_takes("runs_on = [\"plan\"]\ngiven = [\"plan\"]\n\n[give.plan]\nkind = \"latest\"");
    let after = judge_takes(
        "runs_on = [\"plan\"]\ngiven = [\"plan\"]\n\n[run.plan]\nwhy = \"It changed what to judge.\"\n\n\
         [give.plan]\nkind = \"all\"\nwhy = \"It is what to judge.\"",
    );
    assert_eq!(
        flow::compare(&compiled(&before), &compiled(&after)),
        [
            of(
                "judge",
                CellChange::RunsWhy {
                    block: "plan".into(),
                    before: None,
                    after: Some("It changed what to judge.".to_string()),
                },
            ),
            of(
                "judge",
                CellChange::GivenKind {
                    block: "plan".into(),
                    before: Some(InputKind::Latest),
                    after: Some(InputKind::All),
                },
            ),
            of(
                "judge",
                CellChange::GivenWhy {
                    block: "plan".into(),
                    before: None,
                    after: Some("It is what to judge.".to_string()),
                },
            ),
        ]
    );
    // Said for the first time, each is a change from nothing said; said alike in both, neither is one. The old form
    // and the new that say the same are the same definition.
    let old = edit(
        valid(),
        "judge.toml",
        "outputs = [\"needs_design\", \"opinions\"]",
        "outputs = [\"needs_design\", \"opinions\"]\n\n[input.plan]\nkind = \"latest\"",
    );
    assert_eq!(flow::compare(&compiled(&old), &compiled(&before)), []);
    assert_eq!(
        from_valid(&before),
        [of(
            "judge",
            CellChange::GivenKind {
                block: "plan".into(),
                before: None,
                after: Some(InputKind::Latest),
            },
        )]
    );
    assert_eq!(flow::compare(&compiled(&after), &compiled(&after)), []);
}

#[test]
fn a_block_put_in_or_taken_out_with_what_is_said_of_it_is_one_change_of_each_list_it_is_in() {
    // What is said of the plan comes and goes with it; what is said of the request, which runs the cell in both, is a
    // change of its own.
    let with = judge_takes(
        "runs_on = [\"request\", \"plan\"]\ngiven = [\"plan\"]\n\n[run.request]\nwhy = \"It is what was asked.\"\n\n\
         [run.plan]\nwhy = \"It changed what to judge.\"\n\n[give.plan]\nkind = \"latest\"",
    );
    let without = judge_takes("runs_on = [\"request\"]");
    assert_eq!(
        flow::compare(&compiled(&with), &compiled(&without)),
        [
            of("judge", CellChange::RunsOnRemoved("plan".into())),
            of("judge", CellChange::GivenRemoved("plan".into())),
            of(
                "judge",
                CellChange::RunsWhy {
                    block: "request".into(),
                    before: Some("It is what was asked.".to_string()),
                    after: None,
                },
            ),
        ]
    );
    assert_eq!(
        flow::compare(&compiled(&without), &compiled(&with)),
        [
            of("judge", CellChange::RunsOnAdded("plan".into())),
            of("judge", CellChange::GivenAdded("plan".into())),
            of(
                "judge",
                CellChange::RunsWhy {
                    block: "request".into(),
                    before: None,
                    after: Some("It is what was asked.".to_string()),
                },
            ),
        ]
    );
}

#[test]
fn the_tools_a_cell_may_ask_for_and_whether_it_takes_main_in_are_each_a_change() {
    let after = edit(
        valid(),
        "judge.toml",
        "inputs = [\"plan\"]",
        "inputs = [\"plan\"]\ntools = [\"run_tests\", \"query\"]",
    );
    let after = edit(
        after,
        "design.toml",
        "inputs = [\"plan\"]",
        "inputs = [\"plan\"]\ntakes_main = true",
    );
    assert_eq!(
        from_valid(&after),
        [
            of("judge", CellChange::ToolAdded("run_tests".to_string())),
            of("judge", CellChange::ToolAdded("query".to_string())),
            of(
                "design",
                CellChange::TakesMain {
                    before: false,
                    after: true,
                },
            ),
        ]
    );
    let fewer = edit(after.clone(), "judge.toml", "[\"run_tests\", \"query\"]", "[\"query\"]");
    assert_eq!(
        flow::compare(&compiled(&after), &compiled(&fewer)),
        [of("judge", CellChange::ToolRemoved("run_tests".to_string()))]
    );
}

#[test]
fn the_parts_of_a_line_s_system_prompt_are_a_change_of_each_cell_of_the_line() {
    let after = edit(
        valid(),
        FLOW_FILE,
        "[lines.review]",
        "[lines.review]\nsystem = [\"guide\"]",
    );
    let system = |before: &[&str], after: &[&str]| CellChange::System {
        before: before.iter().map(|p| p.to_string()).collect(),
        after: after.iter().map(|p| p.to_string()).collect(),
    };
    assert_eq!(from_valid(&after), [of("judge", system(&[], &["guide"]))]);
    let reordered = edit(
        after.clone(),
        FLOW_FILE,
        "system = [\"guide\"]",
        "system = [\"tree_rules\", \"guide\"]",
    );
    assert_eq!(
        flow::compare(&compiled(&after), &compiled(&reordered)),
        [of("judge", system(&["guide"], &["tree_rules", "guide"]))]
    );
}
