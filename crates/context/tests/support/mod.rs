//! What the tests of the context manager share: a small flow, its works taken through the engine's own loop, and the
//! words beside the flows of the flow crate's tests.
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

use std::collections::BTreeMap;
use std::path::Path;

use context::{Changes, Entered, For, Main};
use decide::{BlockId, CallId, CellId, Outsider, RunId, Seq};
use engine::{Came, Effect, Event, Fill, Handed, RanOn, Spent, Time, Why, Work, handle, pending};
use flow::agents::Agents;
use flow::system::Settings;
use flow::{CellSpec, Compiled, Has, Source};

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
runs_on = ["request", "opinions"]
given = ["request", "notes", "opinions", "plan", "tree"]
outputs = ["plan"]

[give.tree]
kind = "last_and_latest"

[give.request]
kind = "latest"

[give.notes]
kind = "all"

[give.opinions]
kind = "last_and_latest"

[give.plan]
kind = "latest"
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
runs_on = ["plan"]
given = ["plan", "request", "code", "tree"]
outputs = ["code"]

[give.tree]
kind = "last_and_latest"

[give.plan]
kind = "last_and_latest"

[give.request]
kind = "latest"

[give.code]
kind = "latest"
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
runs_on = ["code"]
given = ["code", "plan", "notes"]
outputs = ["opinions", "verdict"]
must_write = ["verdict"]

[give.code]
kind = "last_and_latest"

[give.plan]
kind = "latest"

[give.notes]
kind = "all"
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
runs_on = ["finish"]
given = ["finish", "verdict"]
outputs = []

[give.finish]
kind = "latest"

[give.verdict]
kind = "latest"
"#,
    ),
];

/// The small flow, compiled.
pub fn flow() -> Compiled {
    let source = Source {
        flow: FLOW.to_string(),
        cells: (CELLS.iter())
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect::<BTreeMap<_, _>>(),
        ..Default::default()
    };
    flow::compile(&source, &Has::default()).expect("the small flow compiles")
}

/// What is written beside the flows, as the flow crate's tests have it.
pub fn words() -> Agents {
    let beside = Path::new(env!("CARGO_MANIFEST_DIR")).join("../flow/tests/laid_out");
    flow::agents::read(&beside).expect("the words beside the flows of the tests are read")
}

/// Which work and item the tests' rounds are of.
pub const FOR: For<'static> = For {
    work: 1,
    item: 1,
    title: "Greet",
};

/// A round given to be carried out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Given {
    pub run: RunId,
    pub cell: CellId,
    pub number: usize,
}

/// A work of the small flow, taken one thing at a time through the engine's loop.
pub struct Sim {
    pub flow: Compiled,
    pub words: Agents,
    pub work: Work,
    /// Whether main is taken in as a run of a cell that takes it in starts ([`Sim::main`]); not where it is not set.
    pub takes_main: bool,
    /// The settings the words may name, with their values: none.
    pub named: Settings,
    time: u64,
}

impl Default for Sim {
    fn default() -> Self {
        Self::new()
    }
}

impl Sim {
    pub fn new() -> Sim {
        Sim::of(flow(), words())
    }

    /// A work of this flow, with these words beside it.
    pub fn of(flow: Compiled, words: Agents) -> Sim {
        Sim {
            flow,
            words,
            work: Work::default(),
            takes_main: false,
            named: Settings::default(),
            time: 0,
        }
    }

    /// The loop takes it.
    pub fn take(&mut self, event: Event) -> Vec<Effect> {
        self.time += 1;
        let taken = handle(&self.flow, &mut self.work, Time(self.time), event.clone());
        taken.unwrap_or_else(|refused| panic!("{event:?} is refused: {refused:?}"))
    }

    /// The block is written so, with this value where it is a choice.
    pub fn wrote(&mut self, block: &str, content: &str, value: Option<&str>, by: Outsider) {
        self.take(Event::Wrote {
            block: block.into(),
            by,
            content: content.to_string(),
            matters: true,
            value: value.map(str::to_string),
        });
    }

    /// A person writes the block.
    pub fn writes(&mut self, block: &str, content: &str) {
        self.take(Event::Wrote {
            block: block.into(),
            by: Outsider::Person,
            content: content.to_string(),
            matters: true,
            value: None,
        });
    }

    /// The round under way, where there is one.
    pub fn round(&self) -> Option<Given> {
        pending(&self.flow, &self.work)
            .into_iter()
            .find_map(|effect| match effect {
                Effect::Round { run, cell, number, .. } => Some(Given { run, cell, number }),
                _ => None,
            })
    }

    /// The calls to be made, which have no result yet.
    pub fn calls(&self) -> Vec<CallId> {
        (pending(&self.flow, &self.work).into_iter())
            .filter_map(|effect| match effect {
                Effect::Call { call, .. } => Some(call),
                _ => None,
            })
            .collect()
    }

    /// The cell of the round.
    pub fn cell(&self, round: &Given) -> &CellSpec {
        self.flow.cells.iter().find(|c| c.id == round.cell).expect("the cell")
    }

    /// The line of the round's cell.
    pub fn line(&self, round: &Given) -> String {
        self.cell(round).line.clone().expect("an agent's cell")
    }

    /// What the round is sent in its session as it stands, or in one that holds nothing, with what came of taking main
    /// in as its run started ([`Sim::main`]).
    pub fn enter(&self, round: &Given, anew: bool) -> Entered {
        self.enter_with(round, self.main(round), anew)
    }

    /// What came of taking main in as the round's run started, where its cell takes main in: by the run's name, nothing
    /// to take in, main taken in, or main in conflict.
    pub fn main(&self, round: &Given) -> Option<Main> {
        if !self.takes_main || !self.cell(round).takes_main {
            return None;
        }
        let name = "main".to_string();
        match round.run.0 % 3 {
            0 => None,
            1 => Some(Main::TookIn {
                name,
                main: format!("m{}", round.run.0),
                merge: format!("c{}", round.run.0),
            }),
            _ => Some(Main::Conflict {
                name,
                main: format!("m{}", round.run.0),
                files: vec!["b.rs".to_string()],
            }),
        }
    }

    /// What the round is sent, with this come of taking main in.
    pub fn enter_with(&self, round: &Given, main: Option<Main>, anew: bool) -> Entered {
        let (flow, words, changes) = (&self.flow, &self.words, &self.changes(round));
        let carried = context::Carried {
            about: FOR,
            main: main.as_ref(),
            changes,
            named: &self.named,
            results: &BTreeMap::new(),
        };
        let (run, number) = (round.run, round.number);
        context::enter(flow, words, &self.work, run, number, &carried, anew).expect("the round is entered")
    }

    /// What changed in the tree from each earlier version to the latest as the round began, as the carrier says it.
    pub fn changes(&self, round: &Given) -> Changes {
        let began = (self.work.records.rounds.iter())
            .filter(|r| r.run == round.run)
            .nth(round.number - 1)
            .expect("the round began")
            .at;
        let versions = decide::versions(&self.work.records.before(began));
        let block = BlockId::from("tree");
        let tree: Vec<Seq> = versions
            .get(&block)
            .map_or_else(Vec::new, |vs| vs.iter().map(|v| v.at).collect());
        let Some((latest, earlier)) = tree.split_last() else {
            return Changes::new();
        };
        (earlier.iter())
            .map(|at| ((block.clone(), *at), changed(*at, *latest)))
            .collect()
    }

    /// Whether the round begins its line's session, as the records count it.
    pub fn begins(&self, round: &Given) -> bool {
        let line = self.line(round);
        let on_line = |r: RunId| {
            let cell = self.work.records.started.iter().find(|s| s.run == r).map(|s| &s.cell);
            let spec = cell.and_then(|cell| self.flow.cells.iter().find(|c| c.id == *cell));
            spec.and_then(|c| c.line.as_deref()) == Some(line.as_str())
        };
        work::sessions::of_round(&self.work, &line, on_line, round.run, round.number).1
    }

    /// A try of the round begins and its message fills these into its session, as the carrier says them.
    pub fn tries(&mut self, round: &Given, attempt: usize, fills: Vec<Fill>) {
        let on = RanOn {
            level: "2".to_string(),
            model: "a model".to_string(),
            effort: None,
            account: None,
        };
        self.take(Event::Ran {
            run: round.run,
            attempt,
            on,
        });
        self.take(Event::Filled {
            run: round.run,
            attempt,
            fills,
        });
    }

    /// The round ends with this, having reached the model, its session compacted so many times during it: each
    /// compaction told as the session reopened while the round is under way, as the carrier tells it.
    pub fn returns(&mut self, round: &Given, came: Came, compactions: u32) {
        for _ in 0..compactions {
            let line = self.line(round);
            self.anew(&line, Why::Compacted);
        }
        let spent = Spent {
            input_tokens: 100,
            output_tokens: 10,
            compactions,
            ..Spent::default()
        };
        self.take(Event::Returned {
            run: round.run,
            came,
            spent,
        });
    }

    /// The round hands these blocks in, each with its value where it has one.
    pub fn hands_in(&mut self, round: &Given, blocks: &[(&str, &str, Option<&str>)]) {
        let came = Came::hands_in(
            (blocks.iter())
                .map(|(block, content, value)| Handed {
                    block: BlockId::from(*block),
                    content: content.to_string(),
                    matters: true,
                    value: value.map(str::to_string),
                })
                .collect(),
        );
        self.returns(round, came, 0);
    }

    /// The round failed before it reached the model, which pauses the work; the owner lets it on again.
    pub fn fails(&mut self, round: &Given) {
        self.take(Event::Failed {
            run: round.run,
            why: "it could not be run".to_string(),
            failure: decide::run::Failure::Other,
            resets: None,
            spent: Spent::default(),
        });
        self.take(Event::Resumed);
    }

    /// The line begins a new session, for this reason.
    pub fn anew(&mut self, line: &str, why: Why) {
        self.take(Event::Anew {
            line: line.to_string(),
            why,
        });
    }

    /// The call has this result.
    pub fn done(&mut self, call: CallId, result: &str) {
        self.take(Event::Done {
            call,
            result: result.to_string(),
        });
    }

    /// The latest version of the block.
    pub fn version(&self, block: &str) -> u64 {
        let versions = decide::versions(&self.work.records);
        versions[&BlockId::from(block)].last().expect("a version").at.0
    }

    /// Each version of the block, in order.
    pub fn versions(&self, block: &str) -> Vec<Seq> {
        let versions = decide::versions(&self.work.records);
        versions
            .get(&BlockId::from(block))
            .map_or_else(Vec::new, |vs| vs.iter().map(|v| v.at).collect())
    }

    /// The content of a version of the block.
    pub fn content(&self, block: &str, at: Seq) -> &str {
        self.work
            .contents
            .get(&(BlockId::from(block), at))
            .map_or("", String::as_str)
    }
}

/// What the carrier says changed in the tree from one version to another.
pub fn changed(from: Seq, to: Seq) -> String {
    format!("design/from-{}.md\ndesign/to-{}.md", from.0, to.0)
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
