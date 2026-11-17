//! The dialogues of the engineer's flow (design document `data-and-context`, 36 to 38): each between a cell that makes
//! and one that judges or doubts it, the maker answering by what its judge watches; two cells that make the same thing
//! hold none with each other; and a judge's verdict pairs with the one dialogue it judges in.

use std::path::{Path, PathBuf};

use decide::BlockId;
use flow::FaultKind::*;
use flow::{Compiled, Has, Source};

use crate::support::{at, edit, faults_with, names};

/// The directory of the engineer's flow.
fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/engineer")
}

/// What the engine has for the engineer's flow, as `wf-agents` gives it.
fn has() -> Has {
    Has {
        sources: names(&[
            "answer",
            "asked",
            "dependencies",
            "main",
            "session",
            "start",
            "head",
            "tree",
            "words",
        ]),
        acts: names(&["start", "check", "merge"]),
        tools: names(&["query", "run_tests"]),
        ..Default::default()
    }
}

fn engineer() -> Source {
    flow::read(&dir()).expect("the engineer's flow is read")
}

fn compiled(source: &Source) -> Compiled {
    flow::compile(source, &has()).unwrap_or_else(|faults| panic!("the engineer's flow should compile: {faults:#?}"))
}

/// What the other side watches, of each dialogue the cell holds as the maker, in order.
fn dialogues(compiled: &Compiled, id: &str) -> Vec<(String, Vec<String>)> {
    let i = compiled.flow.place(&id.into()).expect("the cell");
    (compiled.flow.cells()[i].dialogues.iter())
        .map(|(d, watched)| (d.to_string(), watched.iter().map(BlockId::to_string).collect()))
        .collect()
}

// The research and the deep research both make the plan: each watches something else the other writes, and neither
// judges the other. They hold no dialogue with each other, and each answers the plan's review, the tree's review and
// the implementation's doubts by what that one watches of what it writes, the plan, and by nothing the other watches.
#[test]
fn two_cells_that_make_the_plan_hold_no_dialogue_with_each_other() {
    let compiled = compiled(&engineer());
    let plan: Vec<String> = ["plan_owner", "plan_agent", "acceptance", "tree_edits"]
        .map(String::from)
        .to_vec();
    for maker in ["R1", "D1"] {
        assert_eq!(
            dialogues(&compiled, maker),
            [
                ("plan_doubts".to_string(), plan.clone()),
                ("plan_review".to_string(), plan.clone()),
                ("tree_review".to_string(), plan.clone()),
            ],
            "{maker}"
        );
    }
}

// A judge that judges in two dialogues, the review of the code writing the plan's review as well, hands in a verdict
// whose points could go in either: refused, where it would pair with none.
#[test]
fn a_verdict_whose_judge_judges_in_more_than_one_dialogue_is_refused() {
    let source = edit(
        engineer(),
        "cells/T1.toml",
        "watches = [\"change\", \"review\"]",
        "watches = [\"change\", \"review\", \"plan_review\"]",
    );
    let source = edit(
        source,
        "cells/T1.toml",
        "outputs = [\"review\", \"review_ok\", \"needs_testing\"]",
        "outputs = [\"review\", \"review_ok\", \"needs_testing\", \"plan_review\"]",
    );
    let source = edit(
        source,
        "cells/T1.toml",
        "review = \"all\"",
        "review = \"all\"\nplan_review = \"all\"",
    );
    let what = VerdictOfDialogues {
        verdict: "review_ok".to_string(),
        dialogues: vec!["review".to_string(), "plan_review".to_string()],
    };
    assert_eq!(faults_with(&source, &has()), [at("cells/T1.toml", "outputs[1]", what)]);
}

// A judge that holds no dialogue, the review of the code reading its points in place of watching them, hands in a
// verdict whose points go nowhere a maker answers: refused (36, each that judges has a dialogue with the maker).
#[test]
fn a_verdict_whose_judge_holds_no_dialogue_is_refused() {
    let source = edit(
        engineer(),
        "cells/T1.toml",
        "watches = [\"change\", \"review\"]",
        "watches = [\"change\"]",
    );
    let source = edit(
        source,
        "cells/T1.toml",
        "reads = [\"plan\", \"background\", \"tree\"]",
        "reads = [\"plan\", \"background\", \"tree\", \"review\"]",
    );
    let source = edit(
        source,
        "cells/T1.toml",
        "\nreview = \"\"\"Look at what changed: accept each answer in `review` or say why not; raise nothing new the fix did \
         not bring.\"\"\"",
        "",
    );
    let what = VerdictWithoutDialogue {
        verdict: "review_ok".to_string(),
    };
    assert_eq!(faults_with(&source, &has()), [at("cells/T1.toml", "outputs[1]", what)]);
}
