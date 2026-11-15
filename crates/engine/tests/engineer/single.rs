//! A single work on the engineer's flow, taken through `handle` step by step. An agent's round hands in naming as dealt
//! with the triggers it was told ran it, by their names, as an agent writes them, and its return is read as the agents'
//! rounds read one (`CellSpec::dealt_with`); an act of the engine's names none.

use decide::{BlockId, Decision, Outsider, RunId};
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

    /// The round under way, of this agent cell, hands in these blocks, its carrier having read these versions of the
    /// work's branch as it ended, each a block and what it says: what the round committed. Its return names as dealt
    /// with every trigger its round was told ran it.
    pub(super) fn commits(&mut self, cell: &str, blocks: &[(&str, Option<&str>)], read: &[(&str, &str)]) {
        let (run, under_way) = self.round();
        assert_eq!(under_way, cell, "the round under way is another cell's");
        let names = self.told(run, cell);
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let mut came = self.read(cell, &names, blocks);
        came.read = (read.iter())
            .map(|(block, content)| Handed {
                block: (*block).into(),
                content: content.to_string(),
                matters: true,
                value: None,
            })
            .collect();
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
                ("plan_owner", None),
                ("plan_agent", None),
                ("acceptance", None),
            ],
            Vec::new(),
        );
        w.hands_in("R2", &[], Vec::new());
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

#[test]
fn the_review_of_the_tree_runs_where_the_plan_s_edits_of_it_hold_an_entry_and_is_skipped_where_they_hold_none() {
    for (edits, next) in [("A decision that a call keeps what it took.", "D4"), (" \n", "I1")] {
        let mut w = W::started();
        w.hands_in("S1", &[("place", None)], Vec::new());
        w.hands_in("R0", &[], Vec::new());
        let (run, cell) = w.round();
        assert_eq!(cell, "R1");
        let names = w.told(run, "R1");
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let plan = [
            ("deep_needed", Some("no")),
            ("plan_owner", None),
            ("plan_agent", None),
            ("acceptance", None),
            ("tree_edits", None),
        ];
        let mut came = w.read("R1", &names, &plan);
        for h in came.writes.iter_mut().filter(|h| h.block.as_str() == "tree_edits") {
            h.content = edits.to_string();
        }
        w.on(Event::Returned {
            run,
            came,
            spent: Spent::default(),
        });
        // The plan's second look finds no better way, and changes nothing.
        w.hands_in("R2", &[], Vec::new());
        assert_eq!(w.round().1, next, "with tree_edits {edits:?}");
    }
}

// Of the record of what is said, words that hold nothing add no entry, though the cell's entry of them would hold the
// reply's words around them (decision a-write-of-nothing-only-empties); and each version kept says whether it holds
// nothing by the content kept of it, as a test of content and the store's migrations read it: an answer kept holds
// something, and one kept by a record whose file gives no words for a reply holds nothing and says so.
#[test]
fn words_that_hold_nothing_add_no_entry_to_the_record_of_what_is_said_and_each_write_holds_what_its_content_does() {
    let discussion = BlockId::from("discussion");
    // R1's return, the discussion holding these words; what its run wrote, each with the content kept of it.
    let answered = |w: &mut W, words: &str| {
        w.hands_in("S1", &[("place", None)], Vec::new());
        w.hands_in("R0", &[], Vec::new());
        let (run, _) = w.round();
        let names = w.told(run, "R1");
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let plan = [
            ("deep_needed", Some("no")),
            ("plan_owner", None),
            ("plan_agent", None),
            ("acceptance", None),
            ("discussion", None),
        ];
        let mut came = w.read("R1", &names, &plan);
        for h in came.writes.iter_mut().filter(|h| h.block.as_str() == "discussion") {
            h.content = words.to_string();
        }
        w.on(Event::Returned {
            run,
            came,
            spent: Spent::default(),
        });
        let finished = (w.work.records.finished.iter())
            .find(|f| f.run == run)
            .expect("the run finished");
        (finished.writes.iter())
            .map(|x| {
                let kept = w.work.contents.get(&(x.block.clone(), finished.at)).cloned();
                (x.clone(), kept.expect("the content of a write is kept"))
            })
            .collect::<Vec<_>>()
    };
    let holds_what_it_keeps = |writes: &[(decide::Write, String)]| {
        for (write, kept) in writes {
            assert_eq!(
                write.empty,
                decide::holds_nothing(write.value.as_deref(), kept),
                "{}: {kept:?}",
                write.block
            );
        }
    };
    let of_discussion = |writes: &[(decide::Write, String)]| {
        (writes.iter())
            .find(|(x, _)| x.block == discussion)
            .map(|(x, kept)| (x.empty, kept.clone()))
    };

    let mut w = W::started();
    let writes = answered(&mut w, " \n");
    assert_eq!(of_discussion(&writes), None, "{writes:?}");
    for block in ["deep_needed", "plan_owner", "plan_agent", "acceptance"] {
        assert!(
            writes.iter().any(|(x, _)| x.block.as_str() == block),
            "{block}: {writes:?}"
        );
    }
    holds_what_it_keeps(&writes);

    let mut w = W::started();
    let writes = answered(&mut w, "The plan stands.");
    let (empty, kept) = of_discussion(&writes).expect("the answer is kept");
    assert!(kept.contains("The plan stands.") && !empty, "{kept:?}");
    holds_what_it_keeps(&writes);
}
