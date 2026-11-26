//! A work of a flow taken one thing at a time through the engine's own loop, with the context manager entering its
//! rounds as a carrier does: what the tests of the manager and the search for the situations a work can reach
//! ([`crate::witness`]) both run a work with. It names no cell, block or value of any flow, and holds what the search
//! and the tests need of a carrier and no more: the round under way, what it is sent, the try that fills it into its
//! session, and how it returns, fails or is compacted.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use decide::{BlockId, CallId, CellId, Outsider, Room, RunId, Seq};
use engine::{Came, Effect, Event, Fill, Gate, Handed, RanOn, Refused, Spent, Time, Why, Work, handle_then, pending};
use flow::agents::Agents;
use flow::system::Settings;
use flow::{CellSpec, Compiled};

use crate::{Changes, Entered, For, Main};

/// Which work and item the rounds of a simulated work are of.
pub const FOR: For<'static> = For {
    work: "W1",
    item: "#1",
    title: "Greet",
};

/// A round given to be carried out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Given {
    pub run: RunId,
    pub cell: CellId,
    pub number: usize,
}

/// A work of a flow taken one thing at a time through the engine's loop. It is cloned where a search goes on from a
/// state it reached, so the flow and the words beside it, which no state changes, are shared by every copy.
#[derive(Clone)]
pub struct Sim {
    pub flow: Arc<Compiled>,
    pub words: Arc<Agents>,
    pub work: Work,
    /// Whether main is taken in as a run of a cell that takes it in starts ([`Sim::main`]); not where it is not set.
    pub takes_main: bool,
    /// The settings the words may name, with their values: none.
    pub named: Settings,
    /// The blocks whose versions the carrier compares itself, and so says what changed in.
    pub compared: BTreeSet<BlockId>,
    time: u64,
    /// What the driver records for the work after the next thing that happens to it, before the walk begins any round
    /// ([`Sim::anew_before_the_next_round`]).
    follows: Vec<Event>,
}

impl Sim {
    /// A work of this flow, with these words beside it, where the carrier compares no block itself and takes no main
    /// in: whoever runs the work says which it does.
    pub fn of(flow: Compiled, words: Agents) -> Sim {
        Sim {
            flow: Arc::new(flow),
            words: Arc::new(words),
            work: Work::default(),
            takes_main: false,
            named: Settings::default(),
            compared: BTreeSet::new(),
            time: 0,
            follows: Vec::new(),
        }
    }

    /// The loop takes it, and then, before the walk, what the driver records after it
    /// ([`Sim::anew_before_the_next_round`]); or says why the work refuses it. A work that refused is left as the loop
    /// found it, which a search that goes on from a copy drops.
    pub fn try_take(&mut self, event: Event) -> Result<Vec<Effect>, Refused> {
        self.time += 1;
        let follows = std::mem::take(&mut self.follows);
        let then = |_: &Compiled, _: &Work| follows.clone();
        handle_then(
            &self.flow,
            &mut self.work,
            Time(self.time),
            event,
            Room::FREE,
            Gate::Open,
            &then,
        )
    }

    /// The loop takes it, and then, before the walk, what the driver records after it
    /// ([`Sim::anew_before_the_next_round`]).
    pub fn take(&mut self, event: Event) -> Vec<Effect> {
        let shown = format!("{event:?}");
        self.try_take(event)
            .unwrap_or_else(|refused| panic!("{shown} is refused: {refused:?}"))
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

    /// The situation the round is entered in, as [`Sim::enter`] has it, with no message made.
    pub fn situation(&self, round: &Given, anew: bool) -> Entered {
        self.entered(round, self.main(round), anew, false)
    }

    /// What came of taking main in as the round's run started, where its cell takes main in: by the run's number, each
    /// of the kinds in turn or nothing to take in.
    pub fn main(&self, round: &Given) -> Option<Main> {
        if !self.takes_main || !self.cell(round).takes_main {
            return None;
        }
        let name = "main".to_string();
        let main = format!("m{}", round.run.0);
        let files = vec!["b.rs".to_string()];
        match round.run.0 % 7 {
            0 | 6 => None,
            1 => Some(Main::TookIn {
                name,
                main,
                merge: format!("c{}", round.run.0),
            }),
            2 => Some(Main::Conflict { name, main, files }),
            3 => Some(Main::Uncommitted {
                name,
                main,
                files,
                merging: false,
            }),
            4 => Some(Main::Uncommitted {
                name,
                main,
                files,
                merging: true,
            }),
            _ => Some(Main::Failed { name }),
        }
    }

    /// What the round is sent, with this come of taking main in.
    pub fn enter_with(&self, round: &Given, main: Option<Main>, anew: bool) -> Entered {
        self.entered(round, main, anew, true)
    }

    fn entered(&self, round: &Given, main: Option<Main>, anew: bool, message: bool) -> Entered {
        let (flow, words, changes) = (&self.flow, &self.words, &self.changes(round));
        let carried = crate::Carried {
            about: FOR,
            main: main.as_ref(),
            compared: &self.compared,
            changes,
            given_as: &BTreeMap::new(),
            named: &self.named,
            results: &BTreeMap::new(),
        };
        let (run, number) = (round.run, round.number);
        let enter = if message { crate::enter } else { crate::situation_of };
        enter(flow, words, &self.work, run, number, &carried, anew).expect("the round is entered")
    }

    /// What changed in the tree from each earlier version to the latest as the round began, as the carrier says it.
    pub fn changes(&self, round: &Given) -> Changes {
        let began = (self.work.records.rounds.iter())
            .filter(|r| r.run == round.run)
            .nth(round.number - 1)
            .expect("the round began")
            .at;
        let versions = decide::versions(&self.work.records.before(began));
        let mut changes = Changes::new();
        for block in &self.compared {
            let all: Vec<Seq> = versions
                .get(block)
                .map_or_else(Vec::new, |vs| vs.iter().map(|v| v.at).collect());
            if let Some((latest, earlier)) = all.split_last() {
                changes.extend(earlier.iter().map(|at| ((block.clone(), *at), changed(*at, *latest))));
            }
        }
        changes
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
            directory: None,
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
            uncached_tokens: 100,
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
            hindrance: None,
            spent: Spent::default(),
        });
        self.take(Event::Resumed);
    }

    /// The round failed for a cause of this kind, having taken nothing, its program having said, or not, as its stream
    /// began, that it had taken its message in.
    pub fn fails_as(&mut self, round: &Given, failure: decide::run::Failure, received: bool) {
        // What its program's stream said before it failed: that it had taken its message in, or nothing.
        let measures = work::Measures {
            received,
            ..work::Measures::default()
        };
        self.take(Event::Measured {
            run: round.run,
            attempt: 1,
            measures,
        });
        self.take(Event::Failed {
            run: round.run,
            why: "it failed".to_string(),
            failure,
            resets: None,
            hindrance: None,
            spent: Spent::default(),
        });
    }

    /// The line begins a new session, for this reason, now: a round of the line that has begun goes on in the session
    /// it began in, and the line's next round runs in the new one.
    pub fn anew(&mut self, line: &str, why: Why) {
        self.take(Event::Anew {
            line: line.to_string(),
            why,
        });
    }

    /// The line begins a new session, for this reason, as the driver records one: after the next thing that happens to
    /// the work and before the walk, so that a round that thing begins runs in it.
    pub fn anew_before_the_next_round(&mut self, line: &str, why: Why) {
        self.follows.push(Event::Anew {
            line: line.to_string(),
            why,
        });
    }

    /// The work is put on another version of its flow, which reads each datum recorded before it by the first of these
    /// names by the second, and its cells as they were.
    pub fn moves(&mut self, renamed: &[(&str, &str)]) {
        self.time += 1;
        let reading = decide::Reading {
            cells: BTreeMap::new(),
            blocks: (renamed.iter())
                .map(|(from, to)| (BlockId::from(*from), BlockId::from(*to)))
                .collect(),
        };
        self.work
            .onto(Time(self.time), Some(format!("moved at {}", self.time)), reading);
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

    /// The number a round is told the block's latest version by: how many versions the block has, its first being 1.
    pub fn number(&self, block: &str) -> u64 {
        self.versions(block).len() as u64
    }

    /// The number a round is told the block's version written at this place by: its place among the block's versions,
    /// the first being 1.
    pub fn number_at(&self, block: &str, at: Seq) -> u64 {
        let place = self.versions(block).iter().position(|v| *v == at);
        place.expect("a version of the block") as u64 + 1
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
