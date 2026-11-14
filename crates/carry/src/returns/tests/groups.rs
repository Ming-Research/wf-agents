//! The check of a return of a cell run by a group's trigger: the small flow's `write` is run by `on_brief`, a trigger
//! on the group `brief` of what people asked for and their notes, and by the owner speaking to it. A return that names
//! the group's trigger as dealt with, by its name, is read as the agents' rounds read it and handed to the engine's
//! loop, which checks it against the records: it is taken where one of the group's blocks ran the round, and dealing
//! with it consumes each of them that did, as one trigger.

use decide::{Decision, Outsider, Pauser, RunId};
use engine::{Effect, Event, handle};
use work::{Given, Spent, Time, Work};

use super::*;

/// The small flow with the group `brief`, of what people asked for and their notes, whose trigger runs `write`.
fn grouped() -> Compiled {
    let flow = FLOW
        .replace(
            r#"data = ["request", "discussion", "answers"]"#,
            r#"data = ["request", "notes", "discussion", "answers"]"#,
        )
        .replace("[limits]", "[groups]\nbrief = [\"request\", \"notes\"]\n\n[limits]");
    let write = WRITE
        .replace(
            r#"triggered_by = ["on_request", "on_spoken", "on_answer"]"#,
            r#"triggered_by = ["on_brief", "on_spoken", "on_answer"]"#,
        )
        .replace(
            r#"context = ["request", "discussion", "answers"]"#,
            r#"context = ["request", "notes", "discussion", "answers"]"#,
        )
        .replace("[kinds]", "[kinds]\nnotes = \"latest\"");
    compiled(
        &flow,
        &write,
        &[
            ("../data/notes.toml", "from = \"owner\"\n"),
            (
                "../triggers/of_a_group/changed.toml",
                "name = \"on_{group}\"\nread = [\"{group}\"]\n",
            ),
        ],
    )
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

    /// A person writes these blocks while the work is paused, and the cell `write` is entered for them as it is let
    /// on: gives the run.
    fn written(&mut self, blocks: &[&str]) -> RunId {
        let mut effects = self.on(Event::Paused {
            by: Pauser::Owner,
            why: None,
        });
        for block in blocks {
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

    /// The run's return names these as dealt with and hands in its choice: it is read against `write` and the loop
    /// takes it. Gives why it was sent back, where it was.
    fn returns(&mut self, run: RunId, dealt_with: &[&str]) -> Option<Vec<Unmet>> {
        let r = json!({"outcome": {
            "dealt_with": dealt_with,
            "outputs": {"ok": {"value": "yes", "why": "It says how."}},
        }});
        let came = came(&r, &self.flow, cell(&self.flow), &words()).expect("it is read");
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
fn the_cell_s_group_trigger_stands_on_each_of_its_blocks() {
    let flow = grouped();
    let brief = (cell(&flow).triggers.iter())
        .find(|t| t.name == "on_brief")
        .expect("the trigger");
    assert_eq!(brief.blocks(), [BlockId::from("request"), BlockId::from("notes")]);
}

#[test]
fn a_return_that_names_the_group_s_trigger_is_taken_and_consumes_the_block_of_it_that_ran_its_round() {
    let mut w = W::new();
    let run = w.written(&["notes"]);
    assert_eq!(w.returns(run, &["on_brief"]), None);
    assert_eq!(w.dealt_with(run), Some(vec!["notes".into()]));
    // The notes are dealt with: the merge runs next, and `write` is not entered again for them.
    let decision = decide::decide(&w.flow.flow, &w.work.records);
    let merging = matches!(&decision, Decision::Busy { cell, .. } if cell.as_str() == "merge");
    assert!(merging, "{decision:?}");
    assert_eq!(w.runs(), [run]);
}

#[test]
fn a_return_that_names_the_group_s_trigger_once_deals_with_each_of_its_blocks_that_ran_its_round() {
    let mut w = W::new();
    let run = w.written(&["request", "notes"]);
    assert_eq!(w.returns(run, &["on_brief"]), None);
    assert_eq!(w.dealt_with(run), Some(vec!["request".into(), "notes".into()]));
    assert_eq!(w.runs(), [run]);
}

#[test]
fn a_return_that_names_the_group_s_trigger_where_none_of_its_blocks_ran_its_round_is_sent_back() {
    let mut w = W::new();
    let run = w.written(&["write_spoken_to"]);
    let unmet = vec![Unmet::NotRunBy { block: "brief".into() }];
    assert_eq!(w.returns(run, &["on_spoken", "on_brief"]), Some(unmet));
}

#[test]
fn a_return_that_names_a_block_of_the_group_is_sent_back_for_naming_no_trigger() {
    let mut w = W::new();
    let run = w.written(&["notes"]);
    let unmet = vec![
        Unmet::NotATrigger {
            name: "notes".to_string(),
        },
        Unmet::DealtWithNothing,
    ];
    assert_eq!(w.returns(run, &["notes"]), Some(unmet));
}
