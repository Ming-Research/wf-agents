//! Single works on the engineer's flow taken through `handle` step by step, each to what its merge rests on: the check
//! waits for the review's yes, whatever main does, so that no branch the review has not seen is checked or merged; and
//! the owner's answer to a question the check asked, which only the implementation is told, runs the check again by
//! itself, so that the answer alone leads to the merge.

use decide::{Decision, End, Outsider, RunId, Seq, Waited};
use engine::{Came, Effect, Event, Handed, Question, Spent, Time, Way, Work, handle};
use flow::Compiled;

use super::engineer;

/// A work on the engineer's flow, and the clock its events are taken at.
struct W {
    flow: Compiled,
    work: Work,
    clock: u64,
}

impl W {
    /// A work the owner started, on an item that depends on nothing, with main written as its watch writes it.
    fn started() -> W {
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

    fn on(&mut self, event: Event) -> Vec<Effect> {
        self.clock += 1000;
        handle(&self.flow, &mut self.work, Time(self.clock), event).expect("the event is taken")
    }

    fn decision(&self) -> Decision {
        decide::decide(&self.flow.flow, &self.work.records)
    }

    /// The round under way, by its run and cell.
    fn round(&self) -> (RunId, String) {
        (engine::pending(&self.flow, &self.work).into_iter())
            .find_map(|effect| match effect {
                Effect::Round { run, cell, .. } => Some((run, cell.0)),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no round is under way: {:?}", self.decision()))
    }

    /// The round under way, of this cell, hands in these blocks, and the engine asks these questions beside it.
    fn hands_in(&mut self, cell: &str, blocks: &[(&str, Option<&str>)], engine: Vec<Question>) {
        let (run, under_way) = self.round();
        assert_eq!(under_way, cell, "the round under way is another cell's");
        let writes = (blocks.iter())
            .map(|(block, value)| Handed {
                block: (*block).into(),
                content: format!("the {block}"),
                matters: true,
                value: value.map(str::to_string),
            })
            .collect();
        let came = Came {
            engine,
            ..Came::hands_in(writes)
        };
        self.on(Event::Returned {
            run,
            came,
            spent: Spent::default(),
        });
    }

    /// The work from its start to the review's yes on the implementation's code: the research hands in a plan with no
    /// deeper look and no change of the tree, and the review asks for no testing.
    fn reviewed() -> W {
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

/// A write by a person or the outside, its content naming it.
fn wrote(block: &str, by: Outsider, value: Option<&str>) -> Event {
    Event::Wrote {
        block: block.into(),
        by,
        content: format!("the {block}"),
        matters: true,
        value: value.map(str::to_string),
    }
}

/// The question the check asks of a change to a file the project protects, as the engine asks it.
fn protected() -> Question {
    let way = |does: &str| Way {
        does: does.to_string(),
        follows: format!("what follows from {does}"),
    };
    Question::Asked {
        name: "protected_files".to_string(),
        waits: true,
        recommends: None,
        ask: "Whether this work's change to files that govern the agents may merge.".to_string(),
        options: vec![way("Let it merge"), way("Take it back")],
        about: Some("AGENTS.md".to_string()),
    }
}

#[test]
fn the_check_waits_for_the_reviews_yes_whatever_main_does() {
    let mut w = W::started();
    w.hands_in("S1", &[("place", None)], Vec::new());
    w.hands_in("R0", &[], Vec::new());
    // The research hands in its choices alone: nothing is built, and the review never runs.
    w.hands_in(
        "R1",
        &[("deep_needed", Some("no")), ("edits_tree", Some("no"))],
        Vec::new(),
    );
    for _ in 0..3 {
        let Decision::Wait { at, on } = w.decision() else {
            panic!("the work does not wait: {:?}", w.decision());
        };
        assert_eq!(at.as_str(), "M1");
        assert!(
            on.iter().any(|waited| matches!(
                waited,
                Waited::Precondition { on, .. } if *on == decide::Condition::Is {
                    block: "review_ok".into(),
                    value: "yes".into(),
                }
            )),
            "{on:?}"
        );
        // Main moves, as the watch writes it: the check does not run.
        assert_eq!(w.on(wrote("main", Outsider::Outside, None)), vec![]);
        assert!(
            !(w.work.records.started.iter()).any(|s| s.cell.as_str() == "M1"),
            "the check ran before the review said yes"
        );
    }
}

#[test]
fn the_owners_answer_alone_to_the_checks_question_leads_to_the_merge() {
    let mut w = W::reviewed();
    // The check finds a change to a protected file the owner has not let merge, and asks the owner.
    w.hands_in("M1", &[("checked", Some("no")), ("check", None)], vec![protected()]);
    // The implementation is told of the failed check, and has nothing to do: what failed is the owner's to decide.
    w.hands_in("I1", &[], Vec::new());
    let Decision::Wait { at, on } = w.decision() else {
        panic!("the work does not wait for the answer: {:?}", w.decision());
    };
    assert_eq!(at.as_str(), "M1");
    assert!(
        on.iter().any(|waited| matches!(waited, Waited::Answer { .. })),
        "{on:?}"
    );
    let version: Seq = (w.work.records.questions.iter())
        .filter(|q| q.name == "protected_files")
        .map(|q| q.at)
        .max()
        .expect("the question was asked");
    // The owner lets the change merge: the implementation is told, and again has nothing to do.
    w.on(Event::Answer {
        question: "protected_files".to_string(),
        version,
        option: 1,
        why: None,
    });
    w.hands_in("I1", &[], Vec::new());
    // The answer runs the check again by itself, which passes now, and the merge follows.
    w.hands_in("M1", &[("checked", Some("yes"))], Vec::new());
    w.hands_in("M2", &[("merged", None)], Vec::new());
    assert_eq!(w.decision(), Decision::Ended(End::Merged));
}
