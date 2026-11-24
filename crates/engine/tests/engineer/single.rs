//! A single work on the engineer's flow, taken through `handle` step by step. An agent's round hands in naming as dealt
//! with what it was told ran it, by their names, as an agent writes them, and its return is read as the agents' rounds
//! read one (`CellSpec::dealt_with`); an act of the engine's names none.

use decide::{BlockId, Decision, End, Outsider, Pauser, RunId};
use engine::{Came, Effect, Event, Handed, Question, Spent, Time, Work, handle};
use flow::{Compiled, Kind};

use super::{compiled, engineer, engineer_source};

/// A work on the engineer's flow, and the clock its events are taken at.
pub(super) struct W {
    pub(super) flow: Compiled,
    pub(super) work: Work,
    clock: u64,
}

impl W {
    /// A work the owner started, on an item that depends on nothing, with main written as its watch writes it.
    pub(super) fn started() -> W {
        W::started_on(engineer())
    }

    /// A work started as [`W::started`] starts one, on this flow.
    fn started_on(flow: Compiled) -> W {
        let mut w = W {
            flow,
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
        w.hands_in("R0", &[("tree_entries", None)], Vec::new());
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
// review and the review against the tree before the implementation.
#[test]
fn both_reviews_of_the_plan_run_on_a_plan_the_research_wrote() {
    let mut w = W::started();
    w.hands_in("S1", &[("place", None)], Vec::new());
    w.hands_in("R0", &[("tree_entries", None)], Vec::new());
    let (run, cell) = w.round();
    assert_eq!(cell, "R1");
    let names = w.told(run, "R1");
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let plan = [
        ("deep_needed", Some("no")),
        ("plan_owner", None),
        ("plan_agent", None),
        ("acceptance", None),
    ];
    let came = w.read("R1", &names, &plan);
    w.on(Event::Returned {
        run,
        came,
        spent: Spent::default(),
    });
    // The plan's second look finds no better way, and changes nothing.
    w.hands_in("R2", &[], Vec::new());
    assert_eq!(w.round().1, "P1");
    w.hands_in("P1", &[("plan_ok", Some("yes"))], Vec::new());
    assert_eq!(w.round().1, "P2");
    w.hands_in("P2", &[("tree_ok", Some("yes"))], Vec::new());
    assert_eq!(w.round().1, "I1");
}

/// The engineer's flow with one more block in the plan, `tree_edits`, as an earlier flow had it. The research and the
/// deep research write it with the rest of the plan, it is in the plan's group, and each cell that takes the plan is
/// given it as it is given `plan_owner`.
fn with_tree_edits() -> Compiled {
    let mut source = engineer_source();
    let group = r#"plan = ["plan_owner", "plan_agent", "acceptance"]"#;
    assert!(
        source.flow.contains(group),
        "the plan's group is not as this test knows it"
    );
    let with = r#"plan = ["plan_owner", "plan_agent", "acceptance", "tree_edits"]"#;
    source.flow = source.flow.replace(group, with);
    let what = "Each entry the plan adds, changes or deletes in the design tree, as it will read.";
    let block = format!("what = \"{what}\"\n");
    source.files.insert("blocks/plan/tree_edits.toml".to_string(), block);
    for writer in ["R1", "D1"] {
        let text = source.cells.get_mut(writer).expect("the cell is in the flow");
        let output = "  \"acceptance\",\n";
        assert_eq!(
            text.matches(output).count(),
            1,
            "{writer}'s outputs are not as this test knows them"
        );
        *text = text.replace(output, "  \"acceptance\",\n  \"tree_edits\",\n");
    }
    for text in source.cells.values_mut() {
        let kind = (text.lines())
            .find_map(|line| line.strip_prefix("plan_owner = "))
            .map(str::to_string);
        if let Some(kind) = kind {
            *text = text.replace(
                &format!("plan_owner = {kind}\n"),
                &format!("plan_owner = {kind}\ntree_edits = {kind}\n"),
            );
        }
    }
    compiled(&source)
}

/// The blocks a cell hands in, each with its value where it has one.
type Hands<'a> = &'a [(&'a str, Option<&'a str>)];

/// Takes the work from the plan's second look to the merge, every cell finding nothing to change and the test review
/// asking for no testing, as [`W::planned`] and [`W::reviewed`] do. Gives each round's cell, with the blocks it was
/// told ran it.
fn to_the_merge(w: &mut W) -> Vec<(String, Vec<BlockId>)> {
    let steps: [(&str, Hands<'_>); 9] = [
        ("R2", &[]),
        ("P1", &[("plan_ok", Some("yes"))]),
        ("P2", &[("tree_ok", Some("yes"))]),
        ("I1", &[("code", None), ("evidence", None)]),
        ("T1", &[("review_ok", Some("yes"))]),
        ("T2", &[("test_ok", Some("yes")), ("needs_testing", Some("no"))]),
        ("T3", &[("design_ok", Some("yes"))]),
        ("M1", &[("checked", Some("yes"))]),
        ("M2", &[("merged", None)]),
    ];
    let mut rounds = Vec::new();
    for (cell, blocks) in steps {
        let (run, _) = w.round();
        let told = decide::told(&w.flow.flow, &w.work.records, run).expect("the run started");
        rounds.push((cell.to_string(), told.ran_by));
        w.hands_in(cell, blocks, Vec::new());
    }
    assert_eq!(w.decision(), Decision::Ended(End::Merged));
    rounds
}

/// Each run of the work that finished, in the order they finished, by its cell, with what its return dealt with.
fn dealt_with(w: &W) -> Vec<(String, Option<Vec<BlockId>>)> {
    let records = &w.work.records;
    (records.finished.iter())
        .map(|f| {
            let started = (records.started.iter())
                .find(|s| s.run == f.run)
                .expect("the run started");
            (started.cell.0.clone(), f.dealt_with.clone())
        })
        .collect()
}

// A cell a work has passed is not run again because the flow changed (principle
// flow-change-spares-works). A work may hold a version of a block the installed flow no longer has, written by a cell
// that is still in the flow: here the research wrote `tree_edits` with its plan on a flow that had it, and the flow
// without it is installed before the next round. The walk passes over that version as if it were not there. Each cell
// runs in the same order as in a work that never had it, is told of the same blocks and deals with the same, and the
// work merges. No return is refused or sent back, and the work does not stall.
#[test]
fn a_version_of_a_block_the_installed_flow_no_longer_has_is_passed_over_and_the_work_merges() {
    // The same work on the flow that had `tree_edits` and on the flow that never had it, each paused by the owner while
    // the research's round is under way, so that its next round waits.
    let mut works = [W::started_on(with_tree_edits()), W::started()];
    for w in &mut works {
        w.hands_in("S1", &[("place", None)], Vec::new());
        w.hands_in("R0", &[("tree_entries", None)], Vec::new());
        assert_eq!(w.round().1, "R1");
        w.on(Event::Paused {
            by: Pauser::Owner,
            why: None,
            waits_for: Vec::new(),
        });
    }
    let plan = [
        ("deep_needed", Some("no")),
        ("plan_owner", None),
        ("plan_agent", None),
        ("acceptance", None),
    ];
    let [had, never] = &mut works;
    let (research, _) = had.round();
    let mut with_edits = plan.to_vec();
    with_edits.push(("tree_edits", None));
    had.hands_in("R1", &with_edits, Vec::new());
    never.hands_in("R1", &plan, Vec::new());
    let written = (had.work.records.finished.iter())
        .find(|f| f.run == research)
        .is_some_and(|f| f.writes.iter().any(|x| x.block.as_str() == "tree_edits"));
    assert!(written, "the research did not write tree_edits on the flow that had it");
    // The flow without `tree_edits` is installed, the engine looks at each work again, and the owner lets it on.
    had.flow = engineer();
    for w in &mut works {
        w.on(Event::Look);
        assert_eq!(w.decision(), Decision::Paused { cell: "R2".into() });
        w.on(Event::Resumed);
    }
    let [had, never] = &mut works;
    assert_eq!(
        to_the_merge(had),
        to_the_merge(never),
        "each round, by its cell, with what it was told ran it"
    );
    assert_eq!(dealt_with(had), dealt_with(never), "what each run dealt with");
}

// The design review's points on a plan the research wrote go to the research, which answers them in the dialogue the
// two hold, before the review against the tree judges the plan; the answer runs the design review again (design
// document `data-and-context`, 6, 36).
#[test]
fn the_design_review_s_points_on_the_research_s_plan_run_the_research_and_its_answer_runs_the_review_again() {
    let mut w = W::started();
    w.hands_in("S1", &[("place", None)], Vec::new());
    w.hands_in("R0", &[("tree_entries", None)], Vec::new());
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
    let (run, cell) = w.round();
    assert_eq!(cell, "R1", "the research takes the review's points");
    assert_eq!(w.told(run, "R1"), ["plan_review"]);
    w.hands_in("R1", &[("deep_needed", Some("no")), ("plan_review", None)], Vec::new());
    let (run, cell) = w.round();
    assert_eq!(cell, "P1", "the review takes the answer");
    assert_eq!(w.told(run, "P1"), ["plan_review"]);
    w.hands_in("P1", &[("plan_ok", Some("yes"))], Vec::new());
    assert_eq!(w.round().1, "P2", "the review against the tree judges the plan next");
}

#[test]
fn a_write_of_a_cell_s_words_holds_nothing_by_the_answer_kept_not_by_the_words_handed_in() {
    let mut w = W::started();
    w.hands_in("S1", &[("place", None)], Vec::new());
    w.hands_in("R0", &[("tree_entries", None)], Vec::new());
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

/// The version of the block the run started from, where the block had one.
fn started_from(w: &W, run: RunId, block: &str) -> Option<decide::Seq> {
    let started = (w.work.records.started.iter())
        .find(|s| s.run == run)
        .expect("the run started");
    started.basis.get(&BlockId::from(block)).copied().flatten()
}

/// The version of the block the run wrote as it finished, where it wrote one.
fn written_by(w: &W, run: RunId, block: &str) -> Option<decide::Seq> {
    let finished = (w.work.records.finished.iter()).find(|f| f.run == run)?;
    (finished.writes.iter())
        .any(|x| x.block.as_str() == block)
        .then_some(finished.at)
}

// Each review's point on the plan goes to the research before the next review judges the plan (decision
// the-walk-goes-from-the-first-cell): the review against the tree first judges the plan the design review's point led
// to, never the plan that point was about, and the design review judges again each plan the research changes. The
// better-way cell, having handed in once, looks no more (decision the-better-way-is-looked-for-once).
#[test]
fn each_review_s_point_on_the_plan_goes_to_the_research_before_the_next_review_judges_it() {
    let mut w = W::started();
    w.hands_in("S1", &[("place", None)], Vec::new());
    w.hands_in("R0", &[("tree_entries", None)], Vec::new());
    let mut ran: Vec<String> = Vec::new();
    let mut step = |w: &mut W, cell: &str, blocks: &[(&str, Option<&str>)]| {
        let (run, under_way) = w.round();
        ran.push(under_way);
        w.hands_in(cell, blocks, Vec::new());
        run
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
    let answer = [("deep_needed", Some("no")), ("plan_agent", None), ("plan_review", None)];
    let answered = step(&mut w, "R1", &answer);
    step(&mut w, "P1", &[("plan_ok", Some("yes"))]);
    let (tree_review, cell) = w.round();
    assert_eq!(cell, "P2");
    assert_eq!(w.told(tree_review, "P2"), ["plan"]);
    assert_eq!(
        started_from(&w, tree_review, "plan_agent"),
        written_by(&w, answered, "plan_agent"),
        "the review against the tree first judges the plan the research changed for the design review"
    );
    step(&mut w, "P2", &[("tree_ok", Some("no")), ("tree_review", None)]);
    let answer = [("deep_needed", Some("no")), ("plan_agent", None), ("tree_review", None)];
    step(&mut w, "R1", &answer);
    step(&mut w, "P1", &[("plan_ok", Some("yes"))]);
    step(&mut w, "P2", &[("tree_ok", Some("yes"))]);
    assert_eq!(ran, ["R1", "R2", "P1", "R1", "P1", "P2", "R1", "P1", "P2"]);
    assert_eq!(w.round().1, "I1");
}

// A better way goes to the plan's writer before the plan's reviews judge the plan (decision
// the-walk-goes-from-the-first-cell): R2 hands in `better_found` while P1 and P2 have the plan new, and R1 runs next.
// Each review then judges once, and only the plan R1 wrote for the better way, so that no review is spent on a plan
// about to be replaced.
#[test]
fn a_better_way_runs_the_research_before_the_plans_reviews_judge_the_plan() {
    let mut w = W::started();
    w.hands_in("S1", &[("place", None)], Vec::new());
    w.hands_in("R0", &[("tree_entries", None)], Vec::new());
    let plan = [
        ("deep_needed", Some("no")),
        ("plan_owner", None),
        ("plan_agent", None),
        ("acceptance", None),
    ];
    w.hands_in("R1", &plan, Vec::new());
    w.hands_in("R2", &[("better_found", None)], Vec::new());
    let (research, cell) = w.round();
    assert_eq!(
        cell, "R1",
        "the research takes the better way before the reviews judge its plan"
    );
    assert_eq!(w.told(research, "R1"), ["better_found"]);
    let better = [
        ("deep_needed", Some("no")),
        ("plan_agent", None),
        ("better_found", None),
    ];
    w.hands_in("R1", &better, Vec::new());
    let changed = written_by(&w, research, "plan_agent");
    for (review, verdict) in [("P1", "plan_ok"), ("P2", "tree_ok")] {
        let (run, cell) = w.round();
        assert_eq!(cell, review);
        assert_eq!(
            started_from(&w, run, "plan_agent"),
            changed,
            "{review} judges the plan the better way led to"
        );
        w.hands_in(review, &[(verdict, Some("yes"))], Vec::new());
    }
    assert_eq!(w.round().1, "I1");
    let started = |cell: &str| {
        (w.work.records.started.iter())
            .filter(|s| s.cell.as_str() == cell)
            .count()
    };
    assert_eq!(
        (started("P1"), started("P2")),
        (1, 1),
        "each review of the plan judged once"
    );
}

// A point of the code review goes to the implementation before the test review looks (decision
// the-walk-goes-from-the-first-cell): T1 hands in a point while T2 and T3 have the change new, and I1 runs next. T1
// judges the fix, and T2 then judges the changed code first, at the head of the fix, never the code the point was
// about.
#[test]
fn a_code_review_s_point_runs_the_implementation_before_the_test_review_which_then_judges_the_fix() {
    let mut w = W::planned();
    w.commits("I1", &[("code", None), ("evidence", None)], &[("head", "c1")]);
    w.hands_in("T1", &[("review_ok", Some("no")), ("review", None)], Vec::new());
    let (implementing, cell) = w.round();
    assert_eq!(
        cell, "I1",
        "the implementation answers the code review before the test review looks"
    );
    assert_eq!(w.told(implementing, "I1"), ["review"]);
    w.commits("I1", &[("code", None), ("review", None)], &[("head", "c2")]);
    let (run, cell) = w.round();
    assert_eq!(cell, "T1", "the code review judges the fix");
    assert_eq!(w.told(run, "T1"), ["change", "review"]);
    w.hands_in("T1", &[("review_ok", Some("yes"))], Vec::new());
    let (testing, cell) = w.round();
    assert_eq!(cell, "T2");
    assert_eq!(w.told(testing, "T2"), ["change"]);
    let head = started_from(&w, testing, "head").map(|at| w.work.contents[&(BlockId::from("head"), at)].clone());
    assert_eq!(
        head.as_deref(),
        Some("c2"),
        "the test review first judges the head of the fix"
    );
    w.hands_in(
        "T2",
        &[("test_ok", Some("yes")), ("needs_testing", Some("no"))],
        Vec::new(),
    );
    w.hands_in("T3", &[("design_ok", Some("yes"))], Vec::new());
    assert_eq!(w.round().1, "M1");
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
    w.hands_in("R0", &[("tree_entries", None)], Vec::new());
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
    w.hands_in("R0", &[("tree_entries", None)], Vec::new());
    let plan = [
        ("deep_needed", Some("no")),
        ("plan_owner", None),
        ("plan_agent", None),
        ("acceptance", None),
    ];
    w.hands_in("R1", &plan, Vec::new());
    w.hands_in("R2", &[("better_found", None)], Vec::new());
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
        "P1",
        "the answer runs nothing back, and the plan's review judges the plan next"
    );
}
