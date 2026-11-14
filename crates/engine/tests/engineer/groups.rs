//! Cells of the engineer's flow run by a group's trigger: the plan's second look (`R2`, run by `on_plan`) and the
//! review (`T1`, run by `on_change`). The agent is told the group's trigger ran its round and names it as dealt with,
//! by that name; the return is taken, the versions of the group's blocks that ran the round are consumed, and the cell
//! does not run again for them. And of every cell of the flow, each trigger named as dealt with is taken by the run's
//! check for any block it stands on that ran the round, and consumes that block.

use std::collections::BTreeMap;

use decide::run::{Log, Requires, Return, Round, Step, Told, next};
use decide::{BlockId, Decision, End, RunId};
use engine::{Effect, Given};

use super::engineer;
use super::single::W;

/// The runs of the cell that started, in order.
fn runs(w: &W, cell: &str) -> Vec<RunId> {
    (w.work.records.started.iter())
        .filter(|s| s.cell.as_str() == cell)
        .map(|s| s.run)
        .collect()
}

/// What the run, which finished, dealt with.
fn dealt_with(w: &W, run: RunId) -> Option<Vec<BlockId>> {
    (w.work.records.finished.iter())
        .find(|f| f.run == run)
        .unwrap_or_else(|| panic!("the run did not finish: {:?}", w.decision()))
        .dealt_with
        .clone()
}

/// Whether what the engine gave to be done sends a return of the run back.
fn sent_back(effects: &[Effect], run: RunId) -> bool {
    (effects.iter()).any(|e| matches!(e, Effect::Round { run: r, given: Given::SentBack(_), .. } if *r == run))
}

fn blocks(names: &[&str]) -> Option<Vec<BlockId>> {
    Some(names.iter().map(|&b| b.into()).collect())
}

/// The work from its start to the research's plan: no deeper look and no change of the tree.
fn planned() -> W {
    let mut w = W::started();
    w.hands_in("S1", &[("place", None)], Vec::new());
    w.hands_in("R0", &[], Vec::new());
    w.hands_in(
        "R1",
        &[
            ("deep_needed", Some("no")),
            ("edits_tree", Some("no")),
            ("plan_owner", None),
            ("plan_agent", None),
            ("acceptance", None),
        ],
        Vec::new(),
    );
    w
}

/// From the review's yes, the check passes and the work merges.
fn merges(w: &mut W) {
    w.hands_in("M1", &[("checked", Some("yes"))], Vec::new());
    w.hands_in("M2", &[("merged", None)], Vec::new());
    assert_eq!(w.decision(), Decision::Ended(End::Merged));
}

#[test]
fn the_plan_s_second_look_run_by_the_plan_s_group_deals_with_it_by_its_name_once() {
    let mut w = planned();
    let (run, cell) = w.round();
    assert_eq!(cell, "R2");
    assert_eq!(w.told(run, "R2"), ["on_plan"]);
    let effects = w.returns("R2", &["on_plan"], &[("better_found", Some("no"))]);
    assert!(!sent_back(&effects, run), "the return is sent back: {effects:?}");
    // Each block of the plan the research wrote ran the round, and is dealt with.
    assert_eq!(dealt_with(&w, run), blocks(&["plan_owner", "plan_agent", "acceptance"]));
    // The implementation runs next, and the second look never again.
    assert_eq!(w.round().1, "I1");
    w.hands_in("I1", &[("code", None), ("evidence", None)], Vec::new());
    w.hands_in(
        "T1",
        &[("review_ok", Some("yes")), ("needs_testing", Some("no"))],
        Vec::new(),
    );
    merges(&mut w);
    assert_eq!(runs(&w, "R2"), [run]);
}

#[test]
fn the_review_run_by_the_change_s_group_deals_with_it_by_its_name_once() {
    let mut w = planned();
    w.hands_in("R2", &[("better_found", Some("no"))], Vec::new());
    w.hands_in("I1", &[("code", None), ("evidence", None)], Vec::new());
    let (run, cell) = w.round();
    assert_eq!(cell, "T1");
    assert_eq!(w.told(run, "T1"), ["on_change"]);
    let effects = w.returns(
        "T1",
        &["on_change"],
        &[("review_ok", Some("yes")), ("needs_testing", Some("no"))],
    );
    assert!(!sent_back(&effects, run), "the return is sent back: {effects:?}");
    // Each block of the change the implementation wrote ran the round, and is dealt with.
    assert_eq!(dealt_with(&w, run), blocks(&["code", "evidence"]));
    // The check runs next, and the merge after it; the review never again.
    assert_eq!(w.round().1, "M1");
    merges(&mut w);
    assert_eq!(runs(&w, "T1"), [run]);
}

#[test]
fn each_trigger_of_each_cell_named_as_dealt_with_is_taken_for_any_block_it_stands_on_that_ran_the_round() {
    let flow = engineer();
    let choices = BTreeMap::new();
    for (spec, cell) in flow.cells.iter().zip(flow.flow.cells()) {
        for trigger in &spec.triggers {
            let (named, none) = spec.dealt_with(std::slice::from_ref(&trigger.name));
            assert!(none.is_empty(), "{} {}", spec.id, trigger.name);
            for block in trigger.blocks() {
                let told = Told {
                    ran_by: vec![block.clone()],
                    ..Told::default()
                };
                // Its agent is told the trigger ran the round.
                assert!(
                    spec.ran(&told.ran_by).any(|t| t.name == trigger.name),
                    "{} is not told {} ran a round {block} ran",
                    spec.id,
                    trigger.name
                );
                let requires = Requires {
                    outputs: &cell.outputs,
                    groups: &cell.groups,
                    must_write: &[],
                    choices: &choices,
                    returns: 3,
                    told: &told,
                };
                let log = Log {
                    rounds: vec![Round {
                        returned: Some(Return::Came {
                            dealt_with: named.clone(),
                            writes: Vec::new(),
                            calls: 0,
                            found: Vec::new(),
                            waits: false,
                            questions: Vec::new(),
                        }),
                    }],
                    ..Log::default()
                };
                let finish = Step::Finish {
                    dealt_with: vec![block.clone()],
                    writes: Vec::new(),
                };
                assert_eq!(
                    next(&log, requires, decide::run::Hold::Nothing),
                    finish,
                    "{} naming {} where {block} ran it",
                    spec.id,
                    trigger.name
                );
            }
        }
    }
}
