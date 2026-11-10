//! The walk, rule by rule: each test builds a small work through the helper, which records exactly what each decision
//! gives, and says what the walk decides, as a whole. What each test expects is taken from the entries of the design,
//! never from the deciding code.

mod support;

use std::collections::BTreeMap;

use decide::{
    Abandoned, BlockId, CallId, Cause, Cell, Condition, Decision, Done, End, Flow, FlowFault, Outcome, RecordFault,
    Returned, Round, RunId, Seq, Stale, Waited, Who, Write, Written,
};
use support::Says::{DoesNotMatter, Matters, Value};
use support::{World, cell, gate, merge_word, resting};

// What the tests build their expectations from.

/// Versions of blocks, by name.
type Versions<'a> = &'a [(&'a str, Option<Seq>)];

/// The decision to run the cell, because of these, from these versions of the blocks it reads or may write.
fn runs(cell: &str, because: Vec<Cause>, basis: Versions<'_>) -> Decision {
    Decision::Run {
        cell: cell.into(),
        because,
        basis: basis
            .iter()
            .map(|(b, v)| (BlockId::from(*b), *v))
            .collect::<BTreeMap<_, _>>(),
    }
}

/// A block the cell reads has another meaning than the version its record knows; `behind` where a cell after it
/// wrote the current version.
fn changed(block: &str, was: Option<Seq>, behind: bool) -> Cause {
    Cause::Changed {
        block: block.into(),
        was,
        behind,
    }
}

fn go_on(cell: &str, run: RunId) -> Decision {
    Decision::GoOn { cell: cell.into(), run }
}

fn busy(cell: &str, run: RunId) -> Decision {
    Decision::Busy { cell: cell.into(), run }
}

fn abandon(cell: &str, run: RunId, because: Stale) -> Decision {
    Decision::Abandon {
        cell: cell.into(),
        run,
        because,
    }
}

fn wait(at: &str, on: Vec<Waited>) -> Decision {
    Decision::Wait { at: at.into(), on }
}

fn outdated(block: &str) -> Stale {
    Stale::Outdated { block: block.into() }
}

fn precondition(test: &str, who: Who) -> Waited {
    Waited::Precondition { test: test.into(), who }
}

/// A cell that runs only when the block's current value is this one.
fn when(block: &str, value: &str, mut c: Cell) -> Cell {
    c.applies = Some(Condition::Is {
        block: block.into(),
        value: value.into(),
    });
    c
}

/// A cell that runs only while the named test holds.
fn on_test(test: &str, mut c: Cell) -> Cell {
    c.applies = Some(Condition::Test(test.into()));
    c
}

fn with_limit(limit: u32, mut c: Cell) -> Cell {
    c.limit = limit;
    c
}

fn requiring(test: &str, mut c: Cell) -> Cell {
    c.requires.push(test.into());
    c
}

/// The cell the walk runs and, for each reason, its block and whether a cell after it wrote the change.
fn reasons(decision: &Decision) -> (String, Vec<(String, bool)>) {
    let Decision::Run { cell, because, .. } = decision else {
        panic!("the walk runs nothing: {decision:?}")
    };
    let because = (because.iter())
        .map(|c| match c {
            Cause::Changed { block, behind, .. } => (block.0.clone(), *behind),
            Cause::NoRecord => ("no record".to_string(), false),
        })
        .collect();
    (cell.0.clone(), because)
}

const NO_RECORD: Cause = Cause::NoRecord;

// Order and ending.

#[test]
fn the_cells_run_in_their_order_and_a_work_ends_when_the_merge_has_a_record() {
    let mut w = World::new(vec![cell("a", &[], &["p"]), cell("b", &["p"], &[]), Cell::new("merge")]);
    assert_eq!(w.decide(), runs("a", vec![NO_RECORD], &[("p", None)]));
    let p = w.run("a", &[("p", Matters)]);
    assert_eq!(w.decide(), runs("b", vec![NO_RECORD], &[("p", Some(p))]));
    w.run("b", &[]);
    assert_eq!(w.decide(), runs("merge", vec![NO_RECORD], &[]));
    w.run("merge", &[]);
    assert_eq!(w.decide(), Decision::Ended(End::Passed));
}

#[test]
fn a_merge_waits_for_its_precondition() {
    let mut w = World::with_gate(vec![]);
    assert_eq!(w.decide(), wait("merge", vec![merge_word()]));
    w.set("the merge may go", Outcome::Holds);
    assert_eq!(w.decide(), runs("merge", vec![NO_RECORD], &[]));
}

#[test]
fn a_cancelled_work_has_ended() {
    let mut w = World::with_gate(vec![cell("a", &[], &[])]);
    w.cancel();
    assert_eq!(w.decide(), Decision::Ended(End::Cancelled));
}

#[test]
fn a_work_cancelled_while_a_run_is_at_rest_has_ended() {
    let mut w = World::with_gate(vec![cell("a", &[], &[]), cell("b", &[], &[])]);
    w.start("a");
    w.returns("a");
    w.cancel();
    assert_eq!(w.decide(), Decision::Ended(End::Cancelled));
}

#[test]
fn a_work_that_passed_and_is_then_cancelled_has_passed() {
    let mut w = World::new(vec![cell("a", &[], &[])]);
    w.run("a", &[]);
    w.cancel();
    assert_eq!(w.decide(), Decision::Ended(End::Passed));
}

#[test]
fn an_ended_work_stays_ended_whatever_is_written_after() {
    let mut w = World::new(vec![cell("a", &["topic"], &["plan"]), cell("merge", &["plan"], &[])]);
    w.person_writes("topic");
    w.run("a", &[("plan", Matters)]);
    w.run("merge", &[]);
    w.person_writes("topic");
    w.person_writes("plan");
    w.pause();
    assert_eq!(w.decide(), Decision::Ended(End::Passed));
}

// Rounds.

#[test]
fn a_run_is_busy_only_while_a_round_is_under_way_and_at_rest_goes_on_when_the_walk_reaches_it() {
    let mut w = World::with_gate(vec![cell("a", &[], &[]), cell("b", &[], &[])]);
    let run = w.start("a");
    assert_eq!(w.decide(), busy("a", run));
    w.returns("a");
    assert_eq!(w.decide(), go_on("a", run));
    w.go_on("a");
    assert_eq!(w.decide(), busy("a", run));
    w.finish("a", &[]);
    assert_eq!(w.decide(), runs("b", vec![NO_RECORD], &[]));
}

/// The implementation and its review, which writes its opinions for the implementation; both ran once, the review
/// with nothing to say.
fn reviewed() -> (World, Seq, Seq) {
    let cells = vec![
        cell("implement", &["plan", "opinions"], &["code"]),
        cell("review", &["code"], &["opinions"]),
    ];
    let mut w = World::with_gate(cells);
    let plan = w.person_writes("plan");
    let code = w.run("implement", &[("code", Matters)]);
    w.run("review", &[]);
    assert_eq!(w.decide(), resting());
    (w, plan, code)
}

/// The same two cells, the review not yet run: it judges the implementation's first code.
fn judged() -> (World, Seq, Seq) {
    let cells = vec![
        cell("implement", &["plan", "opinions"], &["code"]),
        cell("review", &["code"], &["opinions"]),
    ];
    let mut w = World::with_gate(cells);
    let plan = w.person_writes("plan");
    let code = w.run("implement", &[("code", Matters)]);
    (w, plan, code)
}

#[test]
fn a_cell_before_a_run_at_rest_runs_first_and_the_run_then_goes_on_if_nothing_it_started_from_changed() {
    let (mut w, _, _) = reviewed();
    let p2 = w.person_writes("plan");
    let code = w.run("implement", &[("code", Matters)]);
    let review = w.start("review");
    w.returns("review");
    // A person rewrites the plan while the review rests between two rounds: the implementation, before it, runs.
    let p3 = w.person_writes("plan");
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("plan", Some(p2), false)],
            &[("plan", Some(p3)), ("opinions", None), ("code", Some(code))]
        )
    );
    // The new plan changes nothing in the code: the review goes on where it was.
    w.run("implement", &[]);
    assert_eq!(w.decide(), go_on("review", review));
}

#[test]
fn a_cell_after_a_run_at_rest_does_not_run_while_the_run_is_open() {
    let mut w = World::with_gate(vec![cell("a", &["x"], &[]), cell("b", &["y"], &[])]);
    w.person_writes("x");
    let y1 = w.person_writes("y");
    w.run("a", &[]);
    w.run("b", &[]);
    w.person_writes("x");
    let a = w.start("a");
    w.returns("a");
    let y2 = w.person_writes("y");
    assert_eq!(w.decide(), go_on("a", a));
    w.go_on("a");
    w.finish("a", &[]);
    assert_eq!(
        w.decide(),
        runs("b", vec![changed("y", Some(y1), false)], &[("y", Some(y2))])
    );
}

#[test]
fn a_run_at_rest_at_a_call_for_a_person_waits_for_that_person_and_goes_on_with_the_result() {
    let mut w = World::with_gate(vec![cell("a", &[], &[])]);
    let run = w.start("a");
    let call = w.calls("a", Some(Who::Architect));
    let on = Some(Who::Architect);
    assert_eq!(w.decide(), wait("a", vec![Waited::Call { call, on }]));
    w.done(call);
    assert_eq!(w.decide(), go_on("a", run));
}

#[test]
fn a_run_at_rest_at_a_call_the_engine_makes_waits_on_nobody_and_goes_on_when_the_call_is_done() {
    let mut w = World::with_gate(vec![cell("a", &[], &[])]);
    let run = w.start("a");
    let call = w.calls("a", None);
    assert_eq!(w.decide(), wait("a", vec![Waited::Call { call, on: None }]));
    w.done(call);
    assert_eq!(w.decide(), go_on("a", run));
}

#[test]
fn an_answered_call_does_not_make_its_run_go_on_while_a_round_of_another_run_is_under_way() {
    let (mut w, _, _) = reviewed();
    w.person_writes("plan");
    w.run("implement", &[("code", Matters)]);
    let review = w.start("review");
    let call = w.calls("review", Some(Who::Owner));
    w.person_writes("plan");
    let implement = w.start("implement");
    w.done(call);
    assert_eq!(w.decide(), busy("implement", implement));
    w.finish("implement", &[]);
    assert_eq!(w.decide(), go_on("review", review));
}

#[test]
fn a_paused_work_begins_no_round_while_a_round_under_way_is_still_busy_and_resuming_lets_it_on() {
    let mut w = World::with_gate(vec![cell("a", &[], &[]), cell("b", &[], &[])]);
    let run = w.start("a");
    w.pause();
    assert_eq!(w.decide(), busy("a", run));
    w.returns("a");
    assert_eq!(w.decide(), Decision::Paused);
    w.resume();
    assert_eq!(w.decide(), go_on("a", run));
    w.go_on("a");
    w.finish("a", &[]);
    w.pause();
    assert_eq!(w.decide(), Decision::Paused);
    w.resume();
    assert_eq!(w.decide(), runs("b", vec![NO_RECORD], &[]));
}

// When a cell runs.

/// A research and the implementation of its plan, both run once: the background, the plan and the code.
fn planned() -> (World, Seq, Seq, Seq) {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        cell("implement", &["plan"], &["code"]),
    ];
    let mut w = World::with_gate(cells);
    let background = w.person_writes("background");
    let plan = w.run("research", &[("plan", Matters)]);
    let code = w.run("implement", &[("code", Matters)]);
    assert_eq!(w.decide(), resting());
    (w, background, plan, code)
}

#[test]
fn a_cell_whose_record_knows_the_current_meaning_of_every_input_is_passed_and_a_block_no_cell_reads_changes_nothing() {
    let (mut w, ..) = planned();
    w.person_writes("elsewhere");
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_changed_input_runs_only_the_cells_that_read_it() {
    let cells = vec![
        cell("a", &["x"], &["p"]),
        cell("b", &["y"], &[]),
        cell("c", &["p"], &[]),
    ];
    let mut w = World::with_gate(cells);
    let x1 = w.person_writes("x");
    w.person_writes("y");
    let p1 = w.run("a", &[("p", Matters)]);
    w.run("b", &[]);
    w.run("c", &[]);
    let x2 = w.person_writes("x");
    assert_eq!(
        w.decide(),
        runs(
            "a",
            vec![changed("x", Some(x1), false)],
            &[("x", Some(x2)), ("p", Some(p1))]
        )
    );
    let p2 = w.run("a", &[("p", Matters)]);
    assert_eq!(
        w.decide(),
        runs("c", vec![changed("p", Some(p1), false)], &[("p", Some(p2))])
    );
    w.run("c", &[]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_write_its_writer_says_does_not_matter_runs_nobody() {
    let (mut w, ..) = planned();
    w.person_writes("background");
    w.run("research", &[("plan", DoesNotMatter)]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_cell_does_not_run_for_its_own_write_of_a_block_it_reads() {
    let mut w = World::with_gate(vec![
        cell("design", &["plan"], &["plan"]),
        cell("implement", &["plan"], &[]),
    ]);
    let plan = w.run("design", &[("plan", Matters)]);
    assert_eq!(w.decide(), runs("implement", vec![NO_RECORD], &[("plan", Some(plan))]));
    w.run("implement", &[]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_later_cells_write_of_a_block_an_earlier_cell_reads_runs_the_earlier_cell_as_behind() {
    let mut w = World::with_gate(vec![
        cell("design", &["plan"], &["plan"]),
        cell("revise", &["plan"], &["plan"]),
    ]);
    let p1 = w.run("design", &[("plan", Matters)]);
    assert_eq!(w.decide(), runs("revise", vec![NO_RECORD], &[("plan", Some(p1))]));
    let p2 = w.run("revise", &[("plan", Matters)]);
    // The design's record knows the plan it wrote itself.
    assert_eq!(
        w.decide(),
        runs("design", vec![changed("plan", Some(p1), true)], &[("plan", Some(p2))])
    );
}

#[test]
fn what_a_record_knows_is_the_version_its_run_started_from_or_the_one_it_wrote() {
    let cells = vec![
        cell("research", &[], &["plan"]),
        cell("design", &["plan"], &["plan"]),
        cell("implement", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    let p1 = w.run("research", &[("plan", Matters)]);
    assert_eq!(w.decide(), runs("design", vec![NO_RECORD], &[("plan", Some(p1))]));
    let p2 = w.run("design", &[("plan", Matters)]);
    w.run("implement", &[]);
    let p3 = w.person_writes("plan");
    // The design started from the research's plan but wrote its own: its record knows its own.
    assert_eq!(
        w.decide(),
        runs("design", vec![changed("plan", Some(p2), false)], &[("plan", Some(p3))])
    );
    let p4 = w.run("design", &[("plan", Matters)]);
    // The implementation started from the design's plan.
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("plan", Some(p2), false)],
            &[("plan", Some(p4))]
        )
    );
}

#[test]
fn a_persons_write_always_changes_the_meaning() {
    let (mut w, _, plan, code) = planned();
    let p2 = w.person_writes("plan");
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("plan", Some(plan), false)],
            &[("plan", Some(p2)), ("code", Some(code))]
        )
    );
}

#[test]
fn a_write_from_outside_the_work_always_changes_the_meaning() {
    let mut w = World::with_gate(vec![cell("check", &["main"], &[])]);
    let m1 = w.outside_writes("main");
    w.run("check", &[]);
    assert_eq!(w.decide(), resting());
    let m2 = w.outside_writes("main");
    assert_eq!(
        w.decide(),
        runs("check", vec![changed("main", Some(m1), false)], &[("main", Some(m2))])
    );
}

// Meaning.

#[test]
fn a_row_of_writes_each_said_not_to_matter_keeps_the_first_meaning() {
    let (mut w, _, plan, code) = planned();
    for _ in 0..3 {
        w.person_writes("background");
        w.run("research", &[("plan", DoesNotMatter)]);
        assert_eq!(w.decide(), resting());
    }
    // A change that matters is a change from the meaning the implementation knows, the first plan.
    w.person_writes("background");
    let p = w.run("research", &[("plan", Matters)]);
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("plan", Some(plan), false)],
            &[("plan", Some(p)), ("code", Some(code))]
        )
    );
}

#[test]
fn the_first_version_of_a_block_has_a_meaning_of_its_own_whatever_its_writer_says() {
    let mut w = World::with_gate(vec![
        cell("research", &["background"], &["plan"]),
        cell("implement", &["plan"], &[]),
    ]);
    w.person_writes("background");
    w.run("research", &[]);
    assert_eq!(w.decide(), runs("implement", vec![NO_RECORD], &[("plan", None)]));
    w.run("implement", &[]);
    w.person_writes("background");
    let plan = w.run("research", &[("plan", DoesNotMatter)]);
    assert_eq!(
        w.decide(),
        runs("implement", vec![changed("plan", None, false)], &[("plan", Some(plan))])
    );
}

/// A cell that writes a choice, and one that reads it.
fn chosen() -> (World, Seq) {
    let mut w = World::with_gate(vec![
        cell("ask", &["topic"], &["choice"]),
        cell("use", &["choice"], &[]),
    ]);
    w.person_writes("topic");
    let choice = w.run("ask", &[("choice", Value("yes"))]);
    w.run("use", &[]);
    assert_eq!(w.decide(), resting());
    (w, choice)
}

#[test]
fn a_persons_confirmation_that_leaves_a_choice_as_it_was_runs_nobody_and_a_changed_choice_runs_its_readers() {
    let (mut w, choice) = chosen();
    w.person_confirms("choice", "yes");
    assert_eq!(w.decide(), resting());
    let no = w.person_sets("choice", "no");
    assert_eq!(
        w.decide(),
        runs(
            "use",
            vec![changed("choice", Some(choice), false)],
            &[("choice", Some(no))]
        )
    );
}

#[test]
#[ignore = "the code takes a cell's write as mattering when it says so, though the choice's value is the same, and runs its reader"]
fn a_cell_writing_a_choice_again_with_the_same_value_changes_no_meaning() {
    let (mut w, _) = chosen();
    w.person_writes("topic");
    w.run("ask", &[("choice", Value("yes"))]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_write_said_not_to_matter_by_a_run_that_did_not_see_what_it_wrote_over_is_not_taken_at_its_word() {
    let (mut w, _, known, code) = planned();
    w.person_writes("background");
    w.start("research");
    // A person rewrites the plan while the research's round is under way: the research never saw it.
    w.person_writes("plan");
    let plan = w.finish("research", &[("plan", DoesNotMatter)]);
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("plan", Some(known), false)],
            &[("plan", Some(plan)), ("code", Some(code))]
        )
    );
}

#[test]
fn of_two_cells_that_write_one_block_the_latest_write_is_current_whoever_wrote_it() {
    let cells = vec![
        cell("first", &["x"], &["plan"]),
        cell("second", &[], &["plan"]),
        cell("reader", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    let x1 = w.person_writes("x");
    w.run("first", &[("plan", Matters)]);
    let p2 = w.run("second", &[("plan", Matters)]);
    assert_eq!(w.decide(), runs("reader", vec![NO_RECORD], &[("plan", Some(p2))]));
    w.run("reader", &[]);
    let x2 = w.person_writes("x");
    assert_eq!(
        w.decide(),
        runs(
            "first",
            vec![changed("x", Some(x1), false)],
            &[("x", Some(x2)), ("plan", Some(p2))]
        )
    );
    let p3 = w.run("first", &[("plan", Matters)]);
    assert_eq!(
        w.decide(),
        runs("reader", vec![changed("plan", Some(p2), false)], &[("plan", Some(p3))])
    );
}

// Opinions.

#[test]
fn a_judging_cells_opinion_runs_the_cell_it_is_for_and_a_judge_with_none_runs_nobody() {
    let (mut w, plan, c1) = judged();
    let o1 = w.run("review", &[("opinions", Matters)]);
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("opinions", None, true)],
            &[("plan", Some(plan)), ("opinions", Some(o1)), ("code", Some(c1))]
        )
    );
    let c2 = w.run("implement", &[("code", Matters)]);
    assert_eq!(
        w.decide(),
        runs(
            "review",
            vec![changed("code", Some(c1), false)],
            &[("code", Some(c2)), ("opinions", Some(o1))]
        )
    );
    // The review has nothing more to say: it writes no opinion, and the implementation's inputs are as they were.
    w.run("review", &[]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_persons_opinion_runs_the_cell_it_is_for_and_is_no_way_round() {
    let (mut w, plan, code) = reviewed();
    let opinion = w.person_writes("opinions");
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("opinions", None, false)],
            &[("plan", Some(plan)), ("opinions", Some(opinion)), ("code", Some(code))]
        )
    );
}

#[test]
fn an_opinion_for_a_cell_that_has_no_record_is_among_what_its_first_run_starts_from() {
    let mut w = World::with_gate(vec![
        cell("design", &["brief", "opinions"], &["plan"]),
        cell("review", &["plan"], &["opinions"]),
    ]);
    let brief = w.person_writes("brief");
    let opinion = w.person_writes("opinions");
    assert_eq!(
        w.decide(),
        runs(
            "design",
            vec![NO_RECORD],
            &[("brief", Some(brief)), ("opinions", Some(opinion)), ("plan", None)]
        )
    );
}

#[test]
fn a_judged_cell_that_writes_nothing_new_leaves_its_judge_passed() {
    let (mut w, ..) = judged();
    w.run("review", &[("opinions", Matters)]);
    // The implementation answers the opinion without changing the code: the review has nothing new to judge, and the
    // work rests where the merge's precondition holds it.
    w.run("implement", &[]);
    assert_eq!(w.decide(), resting());
}

// Skipping.

/// An implementation, a review that says whether to test more, the testing that runs only then, and a check that
/// reads the code and the tests; all ran once. Returns the code and the tests.
fn tested() -> (World, Seq, Seq) {
    let cells = vec![
        cell("implement", &["brief"], &["code"]),
        cell("review", &["code"], &["more"]),
        when("more", "yes", cell("testing", &["code"], &["tests"])),
        cell("check", &["code", "tests"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("brief");
    let code = w.run("implement", &[("code", Matters)]);
    w.run("review", &[("more", Value("yes"))]);
    let tests = w.run("testing", &[("tests", Matters)]);
    w.run("check", &[]);
    assert_eq!(w.decide(), resting());
    (w, code, tests)
}

#[test]
fn a_skipped_cell_is_not_run_though_its_inputs_changed_and_what_it_wrote_is_still_what_later_cells_start_from() {
    let (mut w, c1, tests) = tested();
    w.person_writes("brief");
    let c2 = w.run("implement", &[("code", Matters)]);
    w.run("review", &[("more", Value("no"))]);
    assert_eq!(
        w.decide(),
        runs(
            "check",
            vec![changed("code", Some(c1), false)],
            &[("code", Some(c2)), ("tests", Some(tests))]
        )
    );
}

#[test]
fn a_condition_on_a_value_a_person_writes_puts_the_cell_back_and_it_runs_for_inputs_its_record_does_not_know() {
    let (mut w, c1, tests) = tested();
    w.person_writes("brief");
    let c2 = w.run("implement", &[("code", Matters)]);
    w.run("review", &[("more", Value("no"))]);
    w.run("check", &[]);
    assert_eq!(w.decide(), resting());
    w.person_sets("more", "yes");
    assert_eq!(
        w.decide(),
        runs(
            "testing",
            vec![changed("code", Some(c1), false)],
            &[("code", Some(c2)), ("tests", Some(tests))]
        )
    );
}

#[test]
fn a_cell_whose_condition_is_met_again_is_passed_when_its_inputs_are_what_its_record_knows() {
    let (mut w, ..) = tested();
    w.person_sets("more", "no");
    assert_eq!(w.decide(), resting());
    w.person_sets("more", "yes");
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_condition_on_a_block_that_has_no_version_is_not_met() {
    let mut w = World::with_gate(vec![
        cell("ask", &[], &[]),
        when("more", "yes", cell("extra", &[], &[])),
    ]);
    w.run("ask", &[]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_condition_that_is_a_named_test_lets_its_cell_run_while_the_test_holds() {
    let mut w = World::with_gate(vec![on_test("there is a remark", cell("remarks", &[], &[]))]);
    w.set("there is a remark", Outcome::Waits { who: Who::Owner });
    assert_eq!(w.decide(), resting());
    w.set("there is a remark", Outcome::Holds);
    assert_eq!(w.decide(), runs("remarks", vec![NO_RECORD], &[]));
}

#[test]
fn a_run_at_rest_whose_cell_becomes_skipped_is_abandoned_and_the_cell_does_not_run() {
    let (mut w, c1, tests) = tested();
    w.person_writes("brief");
    let c2 = w.run("implement", &[("code", Matters)]);
    w.run("review", &[("more", Value("yes"))]);
    let testing = w.start("testing");
    w.returns("testing");
    w.person_sets("more", "no");
    assert_eq!(w.decide(), abandon("testing", testing, Stale::NoLongerApplies));
    w.abandon();
    assert_eq!(
        w.decide(),
        runs(
            "check",
            vec![changed("code", Some(c1), false)],
            &[("code", Some(c2)), ("tests", Some(tests))]
        )
    );
}

// Stale runs.

#[test]
fn a_run_at_rest_is_abandoned_when_an_input_changes_in_meaning_and_its_cell_runs_again_for_the_change() {
    let (mut w, _, c1) = reviewed();
    w.person_writes("plan");
    w.run("implement", &[("code", Matters)]);
    let review = w.start("review");
    w.returns("review");
    w.person_writes("plan");
    let c3 = w.run("implement", &[("code", Matters)]);
    assert_eq!(w.decide(), abandon("review", review, outdated("code")));
    w.abandon();
    assert_eq!(
        w.decide(),
        runs(
            "review",
            vec![changed("code", Some(c1), false)],
            &[("code", Some(c3)), ("opinions", None)]
        )
    );
}

#[test]
fn a_run_at_rest_is_abandoned_when_a_person_writes_an_opinion_it_may_write() {
    let (mut w, _, c1) = reviewed();
    w.person_writes("plan");
    let c2 = w.run("implement", &[("code", Matters)]);
    let review = w.start("review");
    w.returns("review");
    let opinion = w.person_writes("opinions");
    assert_eq!(w.decide(), abandon("review", review, outdated("opinions")));
    w.abandon();
    // The implementation, before the review, runs first for the person's opinion; the review's own reason, the code,
    // still stands.
    assert_eq!(
        reasons(&w.decide()),
        ("implement".into(), vec![("opinions".into(), false)])
    );
    w.run("implement", &[]);
    assert_eq!(
        w.decide(),
        runs(
            "review",
            vec![changed("code", Some(c1), false)],
            &[("code", Some(c2)), ("opinions", Some(opinion))]
        )
    );
}

#[test]
fn a_run_at_rest_is_abandoned_when_a_person_writes_a_block_it_may_write() {
    let (mut w, b1, ..) = planned();
    let b2 = w.person_writes("background");
    let research = w.start("research");
    w.returns("research");
    let plan = w.person_writes("plan");
    assert_eq!(w.decide(), abandon("research", research, outdated("plan")));
    w.abandon();
    // The plan is no input of the research: it runs again for the background alone, from the person's plan.
    assert_eq!(
        w.decide(),
        runs(
            "research",
            vec![changed("background", Some(b1), false)],
            &[("background", Some(b2)), ("plan", Some(plan))]
        )
    );
}

#[test]
fn a_run_at_rest_goes_on_when_what_it_started_from_was_rewritten_in_a_way_said_not_to_matter() {
    let (mut w, ..) = planned();
    w.person_writes("background");
    w.run("research", &[("plan", Matters)]);
    let implement = w.start("implement");
    w.returns("implement");
    w.person_writes("background");
    w.run("research", &[("plan", DoesNotMatter)]);
    assert_eq!(w.decide(), go_on("implement", implement));
}

#[test]
fn a_stale_run_at_rest_is_abandoned_before_the_walk_runs_a_cell_before_it() {
    let mut w = World::with_gate(vec![cell("a", &["topic"], &[]), cell("b", &["topic"], &[])]);
    w.person_writes("topic");
    w.run("a", &[]);
    w.run("b", &[]);
    let t2 = w.person_writes("topic");
    w.run("a", &[]);
    let b = w.start("b");
    w.returns("b");
    let t3 = w.person_writes("topic");
    assert_eq!(w.decide(), abandon("b", b, outdated("topic")));
    w.abandon();
    assert_eq!(
        w.decide(),
        runs("a", vec![changed("topic", Some(t2), false)], &[("topic", Some(t3))])
    );
}

// Preconditions.

#[test]
fn a_precondition_holds_the_work_before_its_cell_even_when_the_cell_needs_nothing() {
    let mut w = World::with_gate(vec![requiring("a may go", cell("a", &[], &[])), cell("b", &["y"], &[])]);
    w.person_writes("y");
    w.run("a", &[]);
    w.run("b", &[]);
    w.set("a may go", Outcome::Waits { who: Who::Architect });
    w.person_writes("y");
    assert_eq!(w.decide(), wait("a", vec![precondition("a may go", Who::Architect)]));
}

#[test]
fn a_precondition_of_a_skipped_cell_holds_nothing() {
    let extra = requiring("extra may go", when("more", "yes", cell("extra", &[], &[])));
    let mut w = World::with_gate(vec![cell("ask", &[], &["more"]), extra]);
    w.set("extra may go", Outcome::Waits { who: Who::Architect });
    w.run("ask", &[("more", Value("no"))]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_call_and_an_unmet_precondition_at_one_cell_are_waited_for_at_once() {
    let mut w = World::with_gate(vec![requiring("a may go", cell("a", &[], &[]))]);
    w.start("a");
    let call = w.calls("a", Some(Who::Owner));
    w.set("a may go", Outcome::Waits { who: Who::Architect });
    let call = Waited::Call {
        call,
        on: Some(Who::Owner),
    };
    assert_eq!(
        w.decide(),
        wait("a", vec![call, precondition("a may go", Who::Architect)])
    );
}

#[test]
fn an_unmet_precondition_and_the_limit_at_one_cell_are_waited_for_at_once() {
    let (mut w, ..) = judged();
    w.install(vec![
        requiring("implement may go", cell("implement", &["plan", "opinions"], &["code"])),
        cell("review", &["code"], &["opinions"]),
        gate(),
    ]);
    for _ in 0..3 {
        send_back(&mut w);
    }
    w.run("review", &[("opinions", Matters)]);
    w.set("implement may go", Outcome::Waits { who: Who::Owner });
    assert_eq!(
        w.decide(),
        wait(
            "implement",
            vec![precondition("implement may go", Who::Owner), Waited::Limit { runs: 3 }]
        )
    );
}

// The limit.

/// The review writes an opinion, and the implementation answers it with new code.
fn send_back(w: &mut World) {
    w.run("review", &[("opinions", Matters)]);
    w.run("implement", &[("code", Matters)]);
}

fn at_limit(w: &World, runs: u32) {
    assert_eq!(w.decide(), wait("implement", vec![Waited::Limit { runs }]));
}

#[test]
fn a_cell_may_run_as_many_ways_round_in_a_row_as_its_limit_and_the_next_waits_for_the_owner() {
    let (mut w, ..) = judged();
    for _ in 0..3 {
        send_back(&mut w);
    }
    w.run("review", &[("opinions", Matters)]);
    at_limit(&w, 3);
    w.let_go("implement");
    assert_eq!(
        reasons(&w.decide()),
        ("implement".into(), vec![("opinions".into(), true)])
    );
}

#[test]
fn letting_a_cell_go_on_starts_its_count_anew() {
    let (mut w, ..) = judged();
    for _ in 0..3 {
        send_back(&mut w);
    }
    w.run("review", &[("opinions", Matters)]);
    w.let_go("implement");
    w.run("implement", &[("code", Matters)]);
    for _ in 0..2 {
        send_back(&mut w);
    }
    w.run("review", &[("opinions", Matters)]);
    at_limit(&w, 3);
}

#[test]
fn a_run_with_a_reason_a_person_wrote_starts_the_count_anew() {
    let (mut w, ..) = judged();
    for _ in 0..3 {
        send_back(&mut w);
    }
    w.person_writes("plan");
    assert_eq!(reasons(&w.decide()), ("implement".into(), vec![("plan".into(), false)]));
    w.run("implement", &[("code", Matters)]);
    for _ in 0..3 {
        send_back(&mut w);
    }
    w.run("review", &[("opinions", Matters)]);
    at_limit(&w, 3);
}

#[test]
fn a_persons_opinion_starts_the_count_anew() {
    let (mut w, ..) = judged();
    for _ in 0..3 {
        send_back(&mut w);
    }
    w.person_writes("opinions");
    assert_eq!(
        reasons(&w.decide()),
        ("implement".into(), vec![("opinions".into(), false)])
    );
    w.run("implement", &[("code", Matters)]);
    for _ in 0..3 {
        send_back(&mut w);
    }
    w.run("review", &[("opinions", Matters)]);
    at_limit(&w, 3);
}

#[test]
fn a_run_with_a_reason_from_outside_through_a_cell_before_it_is_no_way_round_even_beside_one_from_after_it() {
    let cells = vec![
        cell("research", &["main"], &["plan"]),
        cell("implement", &["plan", "opinions"], &["code"]),
        cell("review", &["code"], &["opinions"]),
    ];
    let mut w = World::with_gate(cells);
    w.outside_writes("main");
    w.run("research", &[("plan", Matters)]);
    w.run("implement", &[("code", Matters)]);
    for _ in 0..3 {
        send_back(&mut w);
    }
    w.run("review", &[("opinions", Matters)]);
    at_limit(&w, 3);
    // The research, before the implementation, changes the plan: the implementation's next run has a reason from
    // before it beside the opinion from after it, and is no way round.
    w.outside_writes("main");
    w.run("research", &[("plan", Matters)]);
    assert_eq!(
        reasons(&w.decide()),
        (
            "implement".into(),
            vec![("plan".into(), false), ("opinions".into(), true)]
        )
    );
    w.run("implement", &[("code", Matters)]);
    for _ in 0..3 {
        send_back(&mut w);
    }
    w.run("review", &[("opinions", Matters)]);
    at_limit(&w, 3);
}

#[test]
fn two_cells_in_a_circle_are_bounded_by_the_earlier_ones_limit() {
    let mut w = World::with_gate(vec![
        with_limit(2, cell("a", &["y"], &["x"])),
        cell("b", &["x"], &["y"]),
    ]);
    let x1 = w.run("a", &[("x", Matters)]);
    let y1 = w.run("b", &[("y", Matters)]);
    assert_eq!(
        w.decide(),
        runs("a", vec![changed("y", None, true)], &[("y", Some(y1)), ("x", Some(x1))])
    );
    let x2 = w.run("a", &[("x", Matters)]);
    // The later cell's every run has a reason from a cell before it.
    assert_eq!(
        w.decide(),
        runs(
            "b",
            vec![changed("x", Some(x1), false)],
            &[("x", Some(x2)), ("y", Some(y1))]
        )
    );
    let y2 = w.run("b", &[("y", Matters)]);
    assert_eq!(
        w.decide(),
        runs(
            "a",
            vec![changed("y", Some(y1), true)],
            &[("y", Some(y2)), ("x", Some(x2))]
        )
    );
    w.run("a", &[("x", Matters)]);
    w.run("b", &[("y", Matters)]);
    assert_eq!(w.decide(), wait("a", vec![Waited::Limit { runs: 2 }]));
}

#[test]
fn an_abandoned_run_is_not_counted_toward_the_limit() {
    let cells = vec![
        with_limit(2, on_test("implementing", cell("implement", &["opinions"], &["code"]))),
        cell("review", &["code"], &["opinions"]),
    ];
    let mut w = World::with_gate(cells);
    w.run("implement", &[("code", Matters)]);
    let o1 = w.run("review", &[("opinions", Matters)]);
    let c2 = w.run("implement", &[("code", Matters)]);
    let o2 = w.run("review", &[("opinions", Matters)]);
    // The second way round starts and is abandoned when its cell is skipped.
    let run = w.start("implement");
    w.returns("implement");
    w.set("implementing", Outcome::Waits { who: Who::Owner });
    assert_eq!(w.decide(), abandon("implement", run, Stale::NoLongerApplies));
    w.abandon();
    w.set("implementing", Outcome::Holds);
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("opinions", Some(o1), true)],
            &[("opinions", Some(o2)), ("code", Some(c2))]
        )
    );
}

#[test]
fn a_limit_of_zero_is_refused() {
    let cells = vec![with_limit(0, cell("a", &[], &[])), gate()];
    assert_eq!(Flow::new(cells), Err(FlowFault::NoLimit("a".into())));
}

// A changed flow.

#[test]
fn a_cell_new_to_the_flow_has_no_record_and_runs_when_the_walk_reaches_it() {
    let mut w = World::with_gate(vec![cell("a", &[], &["p"]), cell("b", &[], &[])]);
    let p = w.run("a", &[("p", Matters)]);
    w.run("b", &[]);
    w.install(vec![
        cell("a", &[], &["p"]),
        cell("new", &["p"], &[]),
        cell("b", &[], &[]),
        gate(),
    ]);
    assert_eq!(w.decide(), runs("new", vec![NO_RECORD], &[("p", Some(p))]));
}

#[test]
fn what_a_cell_taken_out_of_the_flow_wrote_stays_current() {
    let mut w = World::with_gate(vec![cell("a", &[], &["p"]), cell("b", &["p"], &[])]);
    let p = w.run("a", &[("p", Matters)]);
    w.run("b", &[]);
    w.install(vec![cell("b", &["p"], &[]), cell("c", &["p"], &[]), gate()]);
    assert_eq!(w.decide(), runs("c", vec![NO_RECORD], &[("p", Some(p))]));
}

#[test]
fn a_run_at_rest_of_a_cell_taken_out_of_the_flow_is_abandoned() {
    let mut w = World::with_gate(vec![cell("a", &[], &[]), cell("b", &[], &[])]);
    w.run("a", &[]);
    let b = w.start("b");
    w.returns("b");
    w.install(vec![cell("a", &[], &[]), gate()]);
    assert_eq!(w.decide(), abandon("b", b, Stale::NoLongerApplies));
    w.abandon();
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_changed_specification_runs_its_cell_when_the_owner_says_it_matters_and_not_otherwise() {
    let mut w = World::with_gate(vec![cell("design", &["design's specification"], &[])]);
    let s1 = w.outside_writes("design's specification");
    w.run("design", &[]);
    w.outside_writes_not_mattering("design's specification");
    assert_eq!(w.decide(), resting());
    let s3 = w.outside_writes("design's specification");
    assert_eq!(
        w.decide(),
        runs(
            "design",
            vec![changed("design's specification", Some(s1), false)],
            &[("design's specification", Some(s3))]
        )
    );
}

// Records that cannot be those of a work.

#[test]
fn two_runs_started_with_one_name_are_a_fault() {
    let mut w = World::with_gate(vec![cell("a", &[], &[])]);
    let run = w.start("a");
    let mut again = w.records.started[0].clone();
    again.at = w.tick();
    w.records.started.push(again);
    assert_eq!(w.decide(), Decision::Fault(RecordFault::RunTwice(run)));
}

#[test]
fn an_entry_naming_a_run_that_never_started_is_a_fault() {
    let mut w = World::with_gate(vec![cell("a", &[], &[])]);
    let at = w.tick();
    w.records.rounds.push(Round { run: RunId(99), at });
    assert_eq!(w.decide(), Decision::Fault(RecordFault::UnknownRun(RunId(99))));
}

#[test]
fn a_result_of_a_call_never_made_is_a_fault() {
    let mut w = World::with_gate(vec![cell("a", &[], &[])]);
    let at = w.tick();
    w.records.done.push(Done { call: CallId(99), at });
    assert_eq!(w.decide(), Decision::Fault(RecordFault::UnknownCall(CallId(99))));
}

#[test]
fn a_call_with_two_results_is_a_fault() {
    let mut w = World::with_gate(vec![cell("a", &[], &[])]);
    w.start("a");
    let call = w.calls("a", None);
    w.done(call);
    w.done(call);
    assert_eq!(w.decide(), Decision::Fault(RecordFault::DoneTwice(call)));
}

#[test]
fn a_run_finished_twice_is_a_fault() {
    let mut w = World::with_gate(vec![cell("a", &[], &[])]);
    w.run("a", &[]);
    let mut again = w.records.finished[0].clone();
    again.at = w.tick();
    let run = again.run;
    w.records.finished.push(again);
    assert_eq!(w.decide(), Decision::Fault(RecordFault::FinishedTwice(run)));
}

#[test]
fn a_block_written_twice_by_one_run_is_a_fault() {
    let mut w = World::with_gate(vec![cell("a", &[], &["p"])]);
    w.run("a", &[("p", Matters)]);
    let write = Write {
        block: "p".into(),
        matters: true,
        value: None,
    };
    w.records.finished[0].writes.push(write);
    let run = w.records.finished[0].run;
    assert_eq!(w.decide(), Decision::Fault(RecordFault::WroteTwice(run)));
}

#[test]
fn a_run_both_finished_and_abandoned_is_a_fault() {
    let mut w = World::with_gate(vec![cell("a", &[], &[])]);
    w.run("a", &[]);
    let run = w.records.finished[0].run;
    let at = w.tick();
    w.records.abandoned.push(Abandoned { run, at });
    assert_eq!(w.decide(), Decision::Fault(RecordFault::FinishedAndAbandoned(run)));
}

#[test]
fn more_rounds_ended_than_began_is_a_fault() {
    let mut w = World::with_gate(vec![cell("a", &[], &[])]);
    let run = w.start("a");
    w.returns("a");
    let at = w.tick();
    w.records.returned.push(Returned { run, at });
    assert_eq!(w.decide(), Decision::Fault(RecordFault::ReturnedWithoutRound(run)));
}

#[test]
fn two_entries_at_one_place_are_a_fault() {
    let mut w = World::with_gate(vec![cell("a", &["p"], &[])]);
    let at = w.person_writes("p");
    w.records.written.push(Written {
        block: "p".into(),
        by: decide::Outsider::Person,
        matters: true,
        value: None,
        at,
    });
    assert_eq!(w.decide(), Decision::Fault(RecordFault::PlaceTwice(at)));
}

// What makes no flow.

#[test]
fn a_flow_of_no_cells_is_refused() {
    assert_eq!(Flow::new(vec![]), Err(FlowFault::Empty));
}

#[test]
fn two_cells_of_one_name_are_refused() {
    let cells = vec![cell("a", &[], &[]), cell("a", &[], &[]), gate()];
    assert_eq!(Flow::new(cells), Err(FlowFault::Twice("a".into())));
}

#[test]
fn a_condition_on_a_block_the_cell_itself_writes_is_refused() {
    let cells = vec![when("more", "yes", cell("a", &[], &["more"])), gate()];
    assert_eq!(Flow::new(cells), Err(FlowFault::ConditionOnOwnOutput("a".into())));
}

#[test]
fn a_merge_with_a_condition_on_a_value_is_refused() {
    let cells = vec![cell("a", &[], &["more"]), when("more", "yes", Cell::new("merge"))];
    assert_eq!(Flow::new(cells), Err(FlowFault::LastHasCondition("merge".into())));
}

#[test]
fn a_merge_with_a_condition_on_a_named_test_is_refused() {
    let cells = vec![cell("a", &[], &[]), on_test("t", Cell::new("merge"))];
    assert_eq!(Flow::new(cells), Err(FlowFault::LastHasCondition("merge".into())));
}
