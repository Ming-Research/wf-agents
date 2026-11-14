//! A single work on the engineer's flow, taken through `handle` step by step. An agent's round hands in naming as dealt
//! with the triggers it was told ran it, by their names, as an agent writes them, and its return is read as the agents'
//! rounds read one (`CellSpec::dealt_with`); an act of the engine's names none.

use decide::{Decision, Outsider, RunId};
use engine::{Came, Effect, Event, Handed, Question, Spent, Time, Work, handle};
use flow::{Compiled, Kind};

use super::engineer;

/// A work on the engineer's flow, and the clock its events are taken at.
pub(super) struct W {
    pub(super) flow: Compiled,
    pub(super) work: Work,
    clock: u64,
}

impl W {
    /// A work the owner started, on an item that depends on nothing, with main written as its watch writes it.
    pub(super) fn started() -> W {
        let mut w = W {
            flow: engineer(),
            work: Work::default(),
            clock: 0,
        };
        w.on(wrote("dependencies", Outsider::Outside, Some("merged")));
        w.on(wrote("main", Outsider::Outside, None));
        w.on(wrote("start", Outsider::Outside, None));
        // Written while the start's round is under way, the background runs the reading of the tree after it.
        w.on(wrote("background", Outsider::Person, None));
        w
    }

    pub(super) fn on(&mut self, event: Event) -> Vec<Effect> {
        self.clock += 1000;
        handle(&self.flow, &mut self.work, Time(self.clock), event).expect("the event is taken")
    }

    pub(super) fn decision(&self) -> Decision {
        decide::decide(&self.flow.flow, &self.work.records)
    }

    /// The round under way, by its run and cell.
    pub(super) fn round(&self) -> (RunId, String) {
        (engine::pending(&self.flow, &self.work).into_iter())
            .find_map(|effect| match effect {
                Effect::Round { run, cell, .. } => Some((run, cell.0)),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no round is under way: {:?}", self.decision()))
    }

    /// The names of the triggers of the cell that the run's round was told ran it, as its agent is told them.
    pub(super) fn told(&self, run: RunId, cell: &str) -> Vec<String> {
        let spec = self.spec(cell);
        let told = decide::told(&self.flow.flow, &self.work.records, run).expect("the run started");
        spec.ran(&told.ran_by).map(|t| t.name.clone()).collect()
    }

    /// The round under way, of this cell, hands in these blocks, and the engine asks these questions beside it. An
    /// agent's return names as dealt with every trigger its round was told ran it.
    pub(super) fn hands_in(&mut self, cell: &str, blocks: &[(&str, Option<&str>)], engine: Vec<Question>) {
        let (run, under_way) = self.round();
        assert_eq!(under_way, cell, "the round under way is another cell's");
        let came = if self.spec(cell).kind == Kind::Engine {
            Came::hands_in(writes(blocks))
        } else {
            let names = self.told(run, cell);
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            self.read(cell, &names, blocks)
        };
        let came = Came { engine, ..came };
        self.on(Event::Returned {
            run,
            came,
            spent: Spent::default(),
        });
    }

    /// The round under way, of this agent cell, hands in these blocks, naming these as dealt with, as its agent writes
    /// them; gives what the engine gave to be done of it.
    pub(super) fn returns(&mut self, cell: &str, dealt_with: &[&str], blocks: &[(&str, Option<&str>)]) -> Vec<Effect> {
        let (run, under_way) = self.round();
        assert_eq!(under_way, cell, "the round under way is another cell's");
        let came = self.read(cell, dealt_with, blocks);
        self.on(Event::Returned {
            run,
            came,
            spent: Spent::default(),
        })
    }

    /// A return of the agent cell that names these as dealt with and hands in these blocks, read against the cell as
    /// the agents' rounds read one.
    fn read(&self, cell: &str, dealt_with: &[&str], blocks: &[(&str, Option<&str>)]) -> Came {
        let names: Vec<String> = dealt_with.iter().map(|n| n.to_string()).collect();
        let (dealt_with, none) = self.spec(cell).dealt_with(&names);
        assert!(none.is_empty(), "{cell} has no trigger {none:?}");
        Came::works(dealt_with, writes(blocks))
    }

    /// The cell's definition.
    pub(super) fn spec(&self, cell: &str) -> &flow::CellSpec {
        (self.flow.cells.iter())
            .find(|c| c.id.as_str() == cell)
            .expect("the cell is in the flow")
    }

    /// The work from its start to the review's yes on the implementation's code: the research hands in a plan with no
    /// deeper look and no change of the tree, and the review asks for no testing.
    pub(super) fn reviewed() -> W {
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
        w.hands_in("R2", &[("better_found", Some("no"))], Vec::new());
        w.hands_in("I1", &[("code", None), ("evidence", None)], Vec::new());
        w.hands_in(
            "T1",
            &[("review_ok", Some("yes")), ("needs_testing", Some("no"))],
            Vec::new(),
        );
        w
    }
}

/// The blocks handed in, each mattering, its content naming it.
fn writes(blocks: &[(&str, Option<&str>)]) -> Vec<Handed> {
    (blocks.iter())
        .map(|(block, value)| Handed {
            block: (*block).into(),
            content: format!("the {block}"),
            matters: true,
            value: value.map(str::to_string),
        })
        .collect()
}

/// A write by a person or the outside, its content naming it.
pub(super) fn wrote(block: &str, by: Outsider, value: Option<&str>) -> Event {
    Event::Wrote {
        block: block.into(),
        by,
        content: format!("the {block}"),
        matters: true,
        value: value.map(str::to_string),
    }
}
