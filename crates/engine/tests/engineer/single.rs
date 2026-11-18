//! A single work on the engineer's flow, taken through `handle` step by step. An agent's round hands in naming as dealt
//! with what it was told ran it, by their names, as an agent writes them, and its return is read as the agents' rounds
//! read one (`CellSpec::dealt_with`); an act of the engine's names none.

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

    /// The names of what the cell watches that the run's round was told ran it, as its agent is told them.
    pub(super) fn told(&self, run: RunId, cell: &str) -> Vec<String> {
        let spec = self.spec(cell);
        let told = decide::told(&self.flow.flow, &self.work.records, run).expect("the run started");
        spec.ran(&told.ran_by).map(|t| t.name.clone()).collect()
    }

    /// The round under way, of this cell, hands in these blocks, and the engine asks these questions beside it. An
    /// agent's return names as dealt with everything its round was told ran it.
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
    /// with everything its round was told ran it.
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
        assert!(none.is_empty(), "{cell} watches none of {none:?}");
        Came::works(dealt_with, writes(blocks))
    }

    /// The cell's definition.
    pub(super) fn spec(&self, cell: &str) -> &flow::CellSpec {
        (self.flow.cells.iter())
            .find(|c| c.id.as_str() == cell)
            .expect("the cell is in the flow")
    }

    /// The work from its start to the implementation's round: the research hands in a plan with no deeper look and no
    /// change of the tree, and both reviews of the plan find nothing to change.
    pub(super) fn planned() -> W {
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
        w.hands_in("P1", &[("plan_ok", Some("yes"))], Vec::new());
        w.hands_in("P2", &[("tree_ok", Some("yes"))], Vec::new());
        assert_eq!(w.round().1, "I1");
        w
    }

    /// The work from its start to the reviews' yes on the implementation's code: planned ([`W::planned`]), and the test
    /// review asks for no testing.
    pub(super) fn reviewed() -> W {
        let mut w = W::planned();
        w.hands_in("I1", &[("code", None), ("evidence", None)], Vec::new());
        w.reviews_pass();
        w
    }

    /// The three reviews of the change, in their order, each find nothing to change, the test review asking for no
    /// testing.
    pub(super) fn reviews_pass(&mut self) {
        self.hands_in("T1", &[("review_ok", Some("yes"))], Vec::new());
        self.hands_in(
            "T2",
            &[("test_ok", Some("yes")), ("needs_testing", Some("no"))],
            Vec::new(),
        );
        self.hands_in("T3", &[("design_ok", Some("yes"))], Vec::new());
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

// Every plan is reviewed before it is built, for its design and against the tree (principle
// every-plan-is-reviewed-before-it-is-built): a plan the research wrote, with no deep research, goes through the design
// review and the review against the tree before the implementation, whether its edits of the tree hold an entry or none.
#[test]
fn both_reviews_of_the_plan_run_on_a_plan_the_research_wrote_whether_or_not_it_edits_the_tree() {
    for edits in ["A decision that a call keeps what it took.", " \n"] {
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
        assert_eq!(w.round().1, "P1", "with tree_edits {edits:?}");
        w.hands_in("P1", &[("plan_ok", Some("yes"))], Vec::new());
        assert_eq!(w.round().1, "P2", "with tree_edits {edits:?}");
        w.hands_in("P2", &[("tree_ok", Some("yes"))], Vec::new());
        assert_eq!(w.round().1, "I1", "with tree_edits {edits:?}");
    }
}

// The design review's points on a plan the research wrote go to the research, which answers them in the dialogue the
// two hold once the review against the tree has judged the same plan, and its answer runs the review again (design
// document `data-and-context`, 6, 36).
#[test]
fn the_design_review_s_points_on_the_research_s_plan_run_the_research_and_its_answer_runs_the_review_again() {
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
    w.hands_in("P1", &[("plan_ok", Some("no")), ("plan_review", None)], Vec::new());
    assert_eq!(
        w.round().1,
        "P2",
        "the review against the tree judges the same plan first"
    );
    w.hands_in("P2", &[("tree_ok", Some("yes"))], Vec::new());
    let (run, cell) = w.round();
    assert_eq!(cell, "R1", "the research takes the review's points");
    assert_eq!(w.told(run, "R1"), ["plan_review"]);
    w.hands_in("R1", &[("deep_needed", Some("no")), ("plan_review", None)], Vec::new());
    let (run, cell) = w.round();
    assert_eq!(cell, "P1", "the review takes the answer");
    assert_eq!(w.told(run, "P1"), ["plan_review"]);
    w.hands_in("P1", &[("plan_ok", Some("yes"))], Vec::new());
    assert_eq!(w.round().1, "I1", "both reviews passed the plan");
}

#[test]
fn a_write_of_a_cell_s_words_holds_nothing_by_the_answer_kept_not_by_the_words_handed_in() {
    let mut w = W::started();
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
        ("r1_words", None),
    ];
    let mut came = w.read("R1", &names, &plan);
    for h in came.writes.iter_mut().filter(|h| h.block.as_str() == "r1_words") {
        h.content = " \n".to_string();
    }
    w.on(Event::Returned {
        run,
        came,
        spent: Spent::default(),
    });
    let finished = (w.work.records.finished.iter())
        .find(|f| f.run == run)
        .expect("the run finished");
    let at = finished.at;
    let kept = w
        .work
        .contents
        .get(&("r1_words".into(), at))
        .expect("the entry is kept");
    assert!(
        !kept.trim().is_empty(),
        "the entry has the reply's words around the handed in ones: {kept:?}"
    );
    let write = (finished.writes.iter())
        .find(|x| x.block.as_str() == "r1_words")
        .expect("written");
    assert!(!write.empty, "the entry kept holds something");
}

// The cells a change reaches run in one pass, and the plan's writer answers what they send back at once (design document
// `data-and-context`, 6): both reviews of the plan judge its first version, each failing it, before the research
// answers them, and the better-way cell, having handed in once, looks no more (decision
// the-better-way-is-looked-for-once). With the walk taking the first cell with anything new, the plan stage ran R1, R2,
// P1, R1, R2, P1, P2, R1, R2, P1, P2.
#[test]
fn the_plans_reviews_judge_one_version_in_one_pass_and_the_research_answers_both_at_once() {
    let mut w = W::started();
    w.hands_in("S1", &[("place", None)], Vec::new());
    w.hands_in("R0", &[], Vec::new());
    let mut ran: Vec<String> = Vec::new();
    let mut step = |w: &mut W, cell: &str, blocks: &[(&str, Option<&str>)]| {
        ran.push(w.round().1);
        w.hands_in(cell, blocks, Vec::new());
    };
    let plan = [
        ("deep_needed", Some("no")),
        ("plan_owner", None),
        ("plan_agent", None),
        ("acceptance", None),
    ];
    step(&mut w, "R1", &plan);
    step(&mut w, "R2", &[]);
    step(&mut w, "P1", &[("plan_ok", Some("no")), ("plan_review", None)]);
    step(&mut w, "P2", &[("tree_ok", Some("no")), ("tree_review", None)]);
    let (run, _) = w.round();
    assert_eq!(w.told(run, "R1"), ["plan_review", "tree_review"]);
    let answer = [
        ("deep_needed", Some("no")),
        ("plan_agent", None),
        ("plan_review", None),
        ("tree_review", None),
    ];
    step(&mut w, "R1", &answer);
    step(&mut w, "P1", &[("plan_ok", Some("yes"))]);
    step(&mut w, "P2", &[("tree_ok", Some("yes"))]);
    assert_eq!(ran, ["R1", "R2", "P1", "P2", "R1", "P1", "P2"]);
    assert_eq!(w.round().1, "I1");
}

// The better-way cell runs once a work: the engine writes that it handed in as its first run that dealt with something
// finishes, and the cell is skipped from then on, though the plan changes (decision the-better-way-is-looked-for-once).
// A round that failed and a run that only asked for a tool dealt with nothing, and do not count.
#[test]
fn the_better_way_cell_runs_until_it_has_handed_in_once() {
    let handed_in = |w: &W| {
        let yes = |x: &decide::Written| x.block.as_str() == "r2_handed_in" && x.value.as_deref() == Some("yes");
        w.work.records.written.iter().filter(|x| yes(x)).count()
    };
    let mut w = W::started();
    w.hands_in("S1", &[("place", None)], Vec::new());
    w.hands_in("R0", &[], Vec::new());
    let plan = [
        ("deep_needed", Some("no")),
        ("plan_owner", None),
        ("plan_agent", None),
        ("acceptance", None),
    ];
    w.hands_in("R1", &plan, Vec::new());
    // The cell's first round fails: nothing came back, and the engine pauses the work until the owner lets it on.
    let (run, cell) = w.round();
    assert_eq!(cell, "R2");
    w.on(Event::Failed {
        run,
        why: "The agent's program could not start.".to_string(),
        failure: decide::run::Failure::Other,
        resets: None,
        spent: Spent::default(),
    });
    assert_eq!(handed_in(&w), 0, "a round that failed hands in nothing");
    w.on(Event::Resumed);
    // Its next round asks for a tool: its run finishes, having dealt with nothing.
    let (again, cell) = w.round();
    assert_eq!(
        (again, cell.as_str()),
        (run, "R2"),
        "the run whose round failed has its next round"
    );
    let asks = Came::asks(vec![engine::Ask::Tool {
        name: "query".to_string(),
        input: "{}".to_string(),
    }]);
    let effects = w.on(Event::Returned {
        run,
        came: asks,
        spent: Spent::default(),
    });
    assert_eq!(handed_in(&w), 0, "a run that only asked for a tool hands in nothing");
    for effect in effects {
        if let Effect::Call { call, .. } = effect {
            w.on(Event::Done {
                call,
                result: "no rows".to_string(),
            });
        }
    }
    // Entered again for the result, it hands in what it found: nothing better.
    w.hands_in("R2", &[], Vec::new());
    assert_eq!(handed_in(&w), 1);
    w.hands_in("P1", &[("plan_ok", Some("no")), ("plan_review", None)], Vec::new());
    w.hands_in("P2", &[("tree_ok", Some("yes"))], Vec::new());
    w.hands_in(
        "R1",
        &[("deep_needed", Some("no")), ("plan_agent", None), ("plan_review", None)],
        Vec::new(),
    );
    // The plan changed, and the better-way cell is skipped: the review judges it next, and nothing writes the flag again.
    assert_eq!(w.round().1, "P1");
    assert!(!decide::applies(&w.flow.flow, &w.work.records, &"R2".into()));
    assert_eq!(handed_in(&w), 1);
}

// What the better-way cell finds is said to the plan's writer in the dialogue of the two (design document
// `data-and-context`, 28, 36): the writer, woken by it, owes its answer, and a return that neither changes the plan nor
// answers in the dialogue is sent back (42). An answer runs nothing, the cell that looked having handed in.
#[test]
fn the_research_woken_by_a_better_way_owes_its_answer() {
    let mut w = W::started();
    w.hands_in("S1", &[("place", None)], Vec::new());
    w.hands_in("R0", &[], Vec::new());
    let plan = [
        ("deep_needed", Some("no")),
        ("plan_owner", None),
        ("plan_agent", None),
        ("acceptance", None),
    ];
    w.hands_in("R1", &plan, Vec::new());
    w.hands_in("R2", &[("better_found", None)], Vec::new());
    // The reviews judge the same plan in the pass before the research answers.
    w.hands_in("P1", &[("plan_ok", Some("yes"))], Vec::new());
    w.hands_in("P2", &[("tree_ok", Some("yes"))], Vec::new());
    let (run, cell) = w.round();
    assert_eq!(cell, "R1");
    assert_eq!(w.told(run, "R1"), ["better_found"]);
    w.returns("R1", &["better_found"], &[("deep_needed", Some("no"))]);
    assert_eq!(
        w.round(),
        (run, "R1".to_string()),
        "a return that answers nothing is sent back"
    );
    w.returns(
        "R1",
        &["better_found"],
        &[("deep_needed", Some("no")), ("better_found", None)],
    );
    assert_eq!(
        w.round().1,
        "I1",
        "the answer runs nothing, and both reviews passed the plan"
    );
}
