//! The check of a return of a cell that watches a group: the small flow's `write` watches `brief`, the group of what
//! people asked for and their notes, and its own words with the owner. A return that names the group as dealt with, by
//! its name, is read as the agents' rounds read it and handed to the engine's loop, which checks it against the
//! records: it is taken where one of the group's blocks ran the round, and dealing with it consumes each of them that
//! did, as one.

use decide::{Decision, Outsider, Pauser, RunId, Written};
use engine::{Effect, Event, handle};
use work::{Given, Spent, Time, Work};

use super::*;

/// The small flow with the group `brief`, of what people asked for and their notes, which `write` watches.
fn grouped() -> Compiled {
    let flow = FLOW
        .replace(r#"data = ["request"]"#, r#"data = ["request", "notes"]"#)
        .replace("[limits]", "[groups]\nbrief = [\"request\", \"notes\"]\n\n[limits]");
    let write = WRITE
        .replace(
            r#"watches = ["request", "write_questions", "write_words"]"#,
            r#"watches = ["brief", "write_questions", "write_words"]"#,
        )
        .replace("[kinds]", "[kinds]\nnotes = \"latest\"");
    compiled(&flow, &write, &[("../data/notes.toml", "from = \"owner\"\n")])
}

/// The work, its events taken one after another.
struct W {
    flow: Compiled,
    work: Work,
}

impl W {
    fn new() -> W {
        W {
            flow: grouped(),
            work: Work::default(),
        }
    }

    fn on(&mut self, event: Event) -> Vec<Effect> {
        handle(&self.flow, &mut self.work, Time(0), event).expect("the loop takes it")
    }

    /// A person writes these blocks while the work is paused, the engine writing as the owner speaks to the cell
    /// those its words write, and the cell `write` is entered for them as it is let on: gives the run.
    fn written(&mut self, blocks: &[&str]) -> RunId {
        let mut effects = self.on(Event::Paused {
            by: Pauser::Owner,
            why: None,
        });
        for block in blocks {
            let declared = &self.flow.blocks[&BlockId::from(*block)];
            if declared.writers.outside.as_deref() == Some(flow::WORDS) {
                let at = self.work.tick();
                self.work.records.written.push(Written {
                    block: (*block).into(),
                    by: Outsider::Outside,
                    matters: true,
                    value: None,
                    said_to: None,
                    empty: false,
                    at,
                });
                continue;
            }
            effects.extend(self.on(Event::Wrote {
                block: (*block).into(),
                by: Outsider::Person,
                content: format!("the {block}"),
                matters: true,
                value: None,
            }));
        }
        effects.extend(self.on(Event::Resumed));
        (effects.iter())
            .find_map(|e| match e {
                Effect::Round {
                    run, cell, number: 1, ..
                } if cell.as_str() == "write" => Some(*run),
                _ => None,
            })
            .unwrap_or_else(|| panic!("write is not entered for {blocks:?}: {effects:?}"))
    }

    /// The run's return names these as dealt with and hands in its choice, and answers the owner where it deals with
    /// the owner's words: it is read against `write` and the loop takes it. Gives why it was sent back, where it was.
    fn returns(&mut self, run: RunId, dealt_with: &[&str]) -> Option<Vec<Unmet>> {
        let mut r = json!({"outcome": {
            "dealt_with": dealt_with,
            "outputs": {"ok": {"value": "yes", "why": "It says how."}},
        }});
        if dealt_with.contains(&"write_words") {
            r["outcome"]["outputs"]["write_words"] = json!("As you said.");
        }
        let came = came(&r, &self.flow, cell(&self.flow), &words())
            .expect("it is read")
            .came;
        let event = Event::Returned {
            run,
            came,
            spent: Spent::default(),
        };
        (self.on(event).into_iter()).find_map(|e| match e {
            Effect::Round {
                run: r,
                given: Given::SentBack(unmet),
                ..
            } if r == run => Some(unmet),
            _ => None,
        })
    }

    /// What the finished run dealt with.
    fn dealt_with(&self, run: RunId) -> Option<Vec<BlockId>> {
        (self.work.records.finished.iter())
            .find(|f| f.run == run)
            .expect("the run finished")
            .dealt_with
            .clone()
    }

    /// The runs of `write` that started.
    fn runs(&self) -> Vec<RunId> {
        (self.work.records.started.iter())
            .filter(|s| s.cell.as_str() == "write")
            .map(|s| s.run)
            .collect()
    }
}

#[test]
fn a_group_the_cell_watches_stands_for_each_of_its_blocks() {
    let flow = grouped();
    let brief = (cell(&flow).watches.iter())
        .find(|w| w.name == "brief")
        .expect("the group watched");
    assert_eq!(brief.blocks, [BlockId::from("request"), BlockId::from("notes")]);
}

#[test]
fn a_return_that_names_the_group_is_taken_and_consumes_the_block_of_it_that_ran_its_round() {
    let mut w = W::new();
    let run = w.written(&["notes"]);
    assert_eq!(w.returns(run, &["brief"]), None);
    assert_eq!(w.dealt_with(run), Some(vec!["notes".into()]));
    // The notes are dealt with: the merge runs next, and `write` is not entered again for them.
    let decision = decide::decide(&w.flow.flow, &w.work.records);
    let merging = matches!(&decision, Decision::Busy { cell, .. } if cell.as_str() == "merge");
    assert!(merging, "{decision:?}");
    assert_eq!(w.runs(), [run]);
}

#[test]
fn a_return_that_names_the_group_once_deals_with_each_of_its_blocks_that_ran_its_round() {
    let mut w = W::new();
    let run = w.written(&["request", "notes"]);
    assert_eq!(w.returns(run, &["brief"]), None);
    assert_eq!(w.dealt_with(run), Some(vec!["request".into(), "notes".into()]));
    assert_eq!(w.runs(), [run]);
}

#[test]
fn a_return_that_names_the_group_where_none_of_its_blocks_ran_its_round_is_taken_for_what_did_run_it() {
    let mut w = W::new();
    let run = w.written(&["write_words"]);
    assert_eq!(w.returns(run, &["write_words", "brief"]), None);
    assert_eq!(w.dealt_with(run), Some(vec!["write_words".into()]));
}

#[test]
fn a_return_that_names_only_the_group_where_none_of_its_blocks_ran_its_round_deals_with_nothing() {
    let mut w = W::new();
    let run = w.written(&["write_words"]);
    assert_eq!(w.returns(run, &["brief"]), Some(vec![Unmet::DealtWithNothing]));
}

// A block of the group is a name the cell is given, so naming it is not sent back: it is dealt with alone, and the
// group's other block that ran the round stays new and runs the cell again.
#[test]
fn a_return_that_names_a_block_of_the_group_deals_with_that_block_alone() {
    let mut w = W::new();
    let run = w.written(&["request", "notes"]);
    assert_eq!(w.returns(run, &["notes"]), None);
    assert_eq!(w.dealt_with(run), Some(vec!["notes".into()]));
    assert_eq!(w.runs().len(), 2, "`request` is still new to `write`: {:?}", w.runs());
}

// A block the cell writes and is not given as something that ran it is no name it is given.
#[test]
fn a_return_that_names_what_the_cell_is_not_given_is_sent_back_whatever_ran_its_round() {
    let mut w = W::new();
    let run = w.written(&["notes"]);
    let unmet = vec![Unmet::NotWatched {
        name: "plan".to_string(),
    }];
    assert_eq!(w.returns(run, &["notes", "plan"]), Some(unmet));
}
