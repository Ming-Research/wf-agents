//! What the tests of the context manager share: a small flow and the engineer's flow, their works taken through the
//! engine's own loop, and the words beside the flows of the flow crate's tests.
//!
//! The flow: `plan` and `build` on the line `engineer`, `review` on the line `reviewer`, and `close`, which nothing
//! runs, so that a work never ends. People write `request` and `notes`, and the plan, the opinions and the verdict
//! as well as the cells that write them, so that a return may write over a version it never saw. `plan` is run by the request and by
//! the review's opinions, and given the request, the notes, the opinions and its own plan; `build` is run by the plan
//! and given it, the request and its own code, and takes main in; `review` is run by the code and given it, the plan
//! and the notes, and hands in its verdict every time, which `close` is given. Each kind of datum is given by some
//! cell: `latest`, `last_and_latest` and `all`. People write `tree` too, which stands for the design tree: `plan` and
//! `build` are given it, and what changed in it the carrier says, as git would say the files that changed.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use context::text::as_written;
use context::witness::{self, Entry, Found};
use context::{Entered, Piece, Values, situations};
use decide::BlockId;
use flow::agents::Agents;
use flow::{Compiled, Has, Kind, Source};

const FLOW: &str = r#"
name = "sessions"
order = ["plan", "build", "review", "close"]

[limits]
runs = 1000
returns = 1000
calls = 1000

[lines.engineer]
[lines.reviewer]

[blocks.request]
people = true

[blocks.notes]
people = true

[blocks.finish]
people = true

[blocks.tree]
people = true

[blocks.plan]
people = true

[blocks.code]

[blocks.opinions]
people = true

[blocks.verdict]
values = ["yes", "no"]
people = true
"#;

const CELLS: [(&str, &str); 4] = [
    (
        "plan",
        r#"
title = "Plan"
kind = "agent"
effort = "high"
line = "engineer"
task = "Write the plan for the request."
watches = ["request", "opinions"]
reads = ["notes", "plan", "tree"]
outputs = ["plan"]

[kinds]
tree = "last_and_latest"
request = "latest"
notes = "all"
opinions = "last_and_latest"
plan = "latest"
"#,
    ),
    (
        "build",
        r#"
title = "Build"
kind = "agent"
effort = "high"
line = "engineer"
task = "Build what the plan says."
takes_main = true
watches = ["plan"]
reads = ["request", "code", "tree"]
outputs = ["code"]

[kinds]
tree = "last_and_latest"
plan = "last_and_latest"
request = "latest"
code = "latest"
"#,
    ),
    (
        "review",
        r#"
title = "Review"
kind = "agent"
effort = "high"
line = "reviewer"
task = "Review the code."
watches = ["code"]
reads = ["plan", "notes"]
outputs = ["opinions", "verdict"]
must_write = ["verdict"]

[kinds]
code = "last_and_latest"
plan = "latest"
notes = "all"
"#,
    ),
    (
        "close",
        r#"
title = "Close"
kind = "agent"
effort = "high"
line = "engineer"
task = "Close the work."
watches = ["finish"]
reads = ["verdict"]
outputs = []

[kinds]
finish = "latest"
verdict = "latest"
"#,
    ),
];

/// The small flow, compiled.
pub fn flow() -> Compiled {
    compiled(FLOW)
}

/// The small flow with a limit of one on the returns of a run sent back, so that the owner is asked to let a run go on.
pub fn limited() -> Compiled {
    compiled(&FLOW.replace("returns = 1000", "returns = 1"))
}

fn compiled(flow: &str) -> Compiled {
    let source = Source {
        flow: flow.to_string(),
        cells: (CELLS.iter())
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect::<BTreeMap<_, _>>(),
        ..Default::default()
    };
    flow::compile(&source, &Has::default()).expect("the small flow compiles")
}

/// What is written beside the flows, as the flow crate's tests have it.
pub fn words() -> Agents {
    let beside = Path::new(env!("CARGO_MANIFEST_DIR")).join("../flow/tests/project");
    flow::agents::read(&beside).expect("the words beside the flows of the tests are read")
}

/// The source outside the work that writes what the branch holds for the plan.
pub const BRANCH: &str = "branch";

/// The small flow with `branch`, written from outside, which the plan reads, given its latest.
pub fn branched() -> Compiled {
    let flow = format!("{FLOW}\n[blocks.branch]\noutside = \"{BRANCH}\"\n");
    let cells = (CELLS.iter()).map(|(id, text)| {
        let text = match *id {
            "plan" => text
                .replace(
                    "reads = [\"notes\", \"plan\", \"tree\"]",
                    "reads = [\"notes\", \"plan\", \"tree\", \"branch\"]",
                )
                .replace("[kinds]\n", "[kinds]\nbranch = \"latest\"\n"),
            _ => text.to_string(),
        };
        (id.to_string(), text)
    });
    let source = Source {
        flow,
        cells: cells.collect::<BTreeMap<_, _>>(),
        ..Default::default()
    };
    let has = Has {
        sources: [BRANCH.to_string()].into(),
        ..Has::default()
    };
    flow::compile(&source, &has).expect("the small flow with what the branch holds compiles")
}

// Each test binary uses some of these.
#[allow(unused_imports)]
pub use context::sim::{FOR, Given, Sim, changed};

/// A work of the small flow, the blocks the carrier compares itself being the tree.
pub fn small() -> Sim {
    sim_of(flow(), words())
}

/// A work of this flow, with these words beside it, the blocks the carrier compares itself being the tree.
pub fn sim_of(flow: Compiled, words: Agents) -> Sim {
    let mut sim = Sim::of(flow, words);
    sim.compared = compared();
    sim
}

/// The blocks whose versions the carrier compares itself: the tree.
pub fn compared() -> BTreeSet<BlockId> {
    BTreeSet::from([BlockId::from("tree")])
}

/// Carries the round out once, in the session as it stands, as the carrier does: its try fills what its message fills,
/// and it hands these blocks in. Gives what it was sent.
pub fn carry(sim: &mut Sim, blocks: &[(&str, &str, Option<&str>)]) -> Entered {
    let round = sim.round().expect("a round is given");
    let entered = sim.enter(&round, sim.begins(&round));
    sim.tries(&round, 1, entered.fills.clone());
    sim.hands_in(&round, blocks);
    entered
}

/// The directory that holds the repository's flows.
pub fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow")
}

/// The blocks of the flow whose versions the carrier compares itself: those a source of the work's branch writes, the
/// tree and the head, as the carrier names them (`carry::compared`).
pub fn compared_in(flow: &Compiled) -> BTreeSet<BlockId> {
    (flow.blocks.iter())
        .filter(|(_, declared)| matches!(declared.writers.outside.as_deref(), Some("tree" | "head")))
        .map(|(block, _)| block.clone())
        .collect()
}

/// The engineer's flow, compiled against what the engine has for it, as `wf-agents` compiles it.
pub fn engineer() -> Compiled {
    let beside = flow::agents::read(&root()).expect("the files beside the flows are read");
    let source = flow::read(&root().join("engineer")).expect("the engineer's flow is read");
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<BTreeSet<_>>();
    let has = Has {
        sources: names(&[
            "answer",
            "asked",
            "dependencies",
            "handed_in",
            "main",
            "start",
            "head",
            "test_times",
            "tree",
            "words",
        ]),
        acts: names(&["start", "check", "merge", "deliver"]),
        tools: names(&["query", "run_tests"]),
        system: flow::system::reserved(&beside).into_iter().collect(),
    };
    flow::compile(&source, &has).expect("the engineer's flow compiles")
}

/// A work of the engineer's flow, taken in the way the carrier takes it: main taken in as runs of the cells that take
/// it in start, and what changed in the tree and the head said by the carrier.
pub fn engineer_sim() -> Sim {
    let beside = flow::agents::read(&root()).expect("the files beside the flows are read");
    let flow = engineer();
    let mut sim = Sim::of(flow, beside);
    sim.compared = compared_in(&sim.flow);
    sim.takes_main = true;
    sim
}

/// Whether the text is the pieces with each place filled by some text: a place, and the places right after it, stand
/// for what comes before a place where the text after them is found.
pub fn fits(pieces: &[Piece], text: &str) -> bool {
    match pieces.split_first() {
        None => text.is_empty(),
        Some((Piece::Text { text: t }, rest)) => text.strip_prefix(t.as_str()).is_some_and(|after| fits(rest, after)),
        Some((Piece::Slot { .. }, rest)) => match rest.iter().position(|p| matches!(p, Piece::Text { .. })) {
            None => true,
            Some(next) => {
                let Piece::Text { text: t } = &rest[next] else {
                    return false;
                };
                text.match_indices(t.as_str())
                    .any(|(at, _)| fits(&rest[next..], &text[at..]))
            }
        },
    }
}

/// What the tool writes for every agent's cell of a flow, held to the rounds the search carried.
#[derive(Default)]
pub struct Written {
    /// Every line, as [`context::situations::stood`] says them, a written situation stands in.
    pub lines: BTreeSet<String>,
    /// How many situations are written, and how many of those are of a session begun anew since the cell ran in which
    /// another cell of its line was entered first.
    pub situations: usize,
    pub after_read: usize,
    /// The agents' cells that no situation is written for.
    pub without: Vec<String>,
    /// What is wrong with a situation: its script, run again on a fresh work, does not carry out the round that stands in
    /// it, or what the page shows for it, its places open, is not what the engine sent in it, or it gives the guide, which
    /// the system prompt of every session holds, as a datum.
    pub wrong: Vec<String>,
}

/// Holds each situation the tool writes for every agent's cell of the flow to the round the search carried out: its
/// script, run again on a fresh work, carries out a round in the same situation, and what the page shows for it, its
/// places open, has the parts, the forms and the words of what the engine sent in it, each place standing for what fills
/// it.
pub fn written(sim: &Sim, found: &Found) -> Written {
    let mut all = Vec::new();
    let mut out = Written::default();
    for cell in sim.flow.cells.iter().filter(|c| c.kind == Kind::Agent) {
        let of_cell = situations::situations(found, cell);
        if of_cell.is_empty() {
            out.without.push(cell.id.to_string());
        }
        all.extend(of_cell.into_iter().map(|named| (cell, named)));
    }
    let scripts: Vec<&[witness::Action]> = (all.iter())
        .map(|(_, named)| found.witnesses[named.witness].script.as_slice())
        .collect();
    let ran = witness::replay_all(&sim.flow, &sim.words, &sim.compared, &scripts);
    for ((cell, named), ran) in all.iter().zip(ran) {
        out.situations += 1;
        out.after_read += usize::from(named.entry == Entry::ReopenedAfterRead);
        out.lines.extend(situations::stood(cell, &named.situation));
        let label = format!("{}: {}", cell.id, named.name);
        let situation = &named.situation;
        if situation.data.contains_key("guide") || situation.holds.iter().any(|h| h == "guide") {
            out.wrong
                .push(format!("{label}: it gives the guide, which the system prompt holds"));
        }
        let sights = match ran {
            Ok(sights) => sights,
            Err(why) => {
                out.wrong.push(format!("{label}: its script does not run: {why}"));
                continue;
            }
        };
        let Some(sight) = sights.get(found.witnesses[named.witness].nth) else {
            out.wrong
                .push(format!("{label}: its script carries out too few rounds"));
            continue;
        };
        let Some(entered) = sight
            .entered
            .as_ref()
            .filter(|_| sight.cell == cell.id && sight.situation == *situation)
        else {
            out.wrong
                .push(format!("{label}: its script carries out a round in another situation"));
            continue;
        };
        let open = context::message(&sim.flow, &sim.words, cell, situation, Values::Slots(&sim.named));
        if open.len() != entered.sent.len() {
            out.wrong.push(format!(
                "{label}: the page shows {} parts, the round was sent {}",
                open.len(),
                entered.sent.len()
            ));
            continue;
        }
        for (open, sent) in open.iter().zip(&entered.sent) {
            let text = as_written(&sent.pieces);
            if (&open.part, &open.block, open.form) != (&sent.part, &sent.block, sent.form)
                || !fits(&open.pieces, &text)
            {
                out.wrong.push(format!(
                    "{label}: the page shows {:?} for what the round was sent as {text:?}",
                    open.pieces
                ));
                break;
            }
        }
    }
    out
}
