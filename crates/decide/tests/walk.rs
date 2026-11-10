//! The walk, rule by rule: each test builds a small work through the helper, which records exactly what each decision
//! gives, and says what the walk decides, as a whole. What each test expects is taken from the entries of the design,
//! never from the deciding code.

mod support;

use std::collections::BTreeMap;

use decide::{
    Abandoned, CallId, Cause, Cell, Condition, Decision, Done, End, Flow, FlowFault, Outcome, ProductId, RecordFault,
    RefusalId, Refuser, Returned, Round, RunId, Seq, Stale, Waited, Who, Written,
};
use support::Says::{DoesNotMatter, Matters};
use support::{World, cell, gate, merge_word, resting};

// What the tests build their expectations from.

/// Versions of products, by name.
type Versions<'a> = &'a [(&'a str, Option<Seq>)];

/// No versions: the cell depends on nothing, or may write nothing.
const NONE: Versions<'static> = &[];

fn versions(of: Versions<'_>) -> BTreeMap<ProductId, Option<Seq>> {
    of.iter().map(|(p, v)| (ProductId::from(*p), *v)).collect()
}

/// The decision to run the cell, because of these, from these versions of what it depends on, over these versions of
/// what it may write, given these refusals.
fn runs(cell: &str, because: Vec<Cause>, basis: Versions<'_>, over: Versions<'_>, given: Vec<RefusalId>) -> Decision {
    Decision::Run {
        cell: cell.into(),
        because,
        basis: versions(basis),
        over: versions(over),
        given,
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

fn changed(product: &str) -> Cause {
    Cause::Changed {
        product: product.into(),
    }
}

fn refused(refusal: RefusalId, product: &str, by: Refuser) -> Cause {
    Cause::Refused {
        refusal,
        product: product.into(),
        by,
    }
}

fn answered(refusal: RefusalId, product: &str) -> Cause {
    Cause::Answered {
        refusal,
        product: product.into(),
    }
}

fn gone(product: &str) -> Cause {
    Cause::Gone {
        product: product.into(),
    }
}

fn unanswered(refusal: RefusalId, product: &str, by: Refuser) -> Waited {
    Waited::Unanswered {
        refusal,
        product: product.into(),
        by,
    }
}

fn outdated(product: &str) -> Stale {
    Stale::Outdated {
        product: product.into(),
    }
}

const OWNER: Refuser = Refuser::Person(Who::Owner);

/// A cell on the path only when the cell `by` answered yes to "go".
fn on_fork(id: &str, by: &str, follows: &[&str], writes: &[&str]) -> Cell {
    let mut c = cell(id, follows, writes);
    c.applies = Some(Condition::Answered {
        by: by.into(),
        output: "go".into(),
        is: "yes".into(),
    });
    c
}

/// A cell on the path only while the named test holds.
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

const YES: &[(&str, &str)] = &[("go", "yes")];
const NO: &[(&str, &str)] = &[("go", "no")];
const COMMENT: &str = "the item has a comment";

// Order and ending.

#[test]
fn the_cells_run_in_their_order_and_a_work_ends_when_the_merge_has_a_record() {
    let mut w = World::new(vec![cell("a", &[], &["p"]), cell("b", &["p"], &[]), Cell::new("merge")]);
    assert_eq!(
        w.decide(),
        runs("a", vec![Cause::NoRecord], NONE, &[("p", None)], vec![])
    );
    let a = w.run("a", &[], &[("p", Matters)]);
    let p = w.version(a);
    assert_eq!(
        w.decide(),
        runs("b", vec![Cause::NoRecord], &[("p", Some(p))], NONE, vec![])
    );
    w.run("b", &[], &[]);
    assert_eq!(w.decide(), runs("merge", vec![Cause::NoRecord], NONE, NONE, vec![]));
    w.run("merge", &[], &[]);
    assert_eq!(w.decide(), Decision::Ended(End::Passed));
}

#[test]
fn a_cell_whose_condition_on_the_answer_before_it_is_not_met_is_skipped() {
    let mut w = World::with_gate(vec![cell("ask", &[], &[]), on_fork("extra", "ask", &[], &[])]);
    w.run("ask", NO, &[]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_cell_whose_condition_on_the_answer_before_it_is_met_runs() {
    let mut w = World::with_gate(vec![cell("ask", &[], &[]), on_fork("extra", "ask", &[], &[])]);
    w.run("ask", YES, &[]);
    assert_eq!(w.decide(), runs("extra", vec![Cause::NoRecord], NONE, NONE, vec![]));
}

#[test]
fn a_condition_on_the_answer_of_a_cell_that_is_off_the_path_is_not_met() {
    let cells = vec![
        cell("first", &["topic"], &[]),
        on_fork("second", "first", &[], &["sketch"]),
        on_fork("third", "second", &["sketch"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("topic");
    w.run("first", YES, &[]);
    w.run("second", YES, &[("sketch", Matters)]);
    w.run("third", &[], &[]);
    assert_eq!(w.decide(), resting());
    let topic = w.person_writes("topic");
    assert_eq!(
        w.decide(),
        runs("first", vec![changed("topic")], &[("topic", Some(topic))], NONE, vec![])
    );
    w.run("first", NO, &[]);
    // The second cell is off the path, so its record, which answered yes, counts for nothing: the third is off the
    // path too, though the sketch it was built on is now absent and, on the path, it would run again.
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_condition_that_is_a_named_test_puts_its_cell_on_the_path_while_the_test_holds() {
    let mut w = World::with_gate(vec![on_test(COMMENT, cell("comments", &[], &[]))]);
    w.set(COMMENT, Outcome::Waits { who: Who::Owner });
    assert_eq!(w.decide(), resting());
    w.set(COMMENT, Outcome::Holds);
    assert_eq!(w.decide(), runs("comments", vec![Cause::NoRecord], NONE, NONE, vec![]));
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
fn an_ended_work_stays_ended_whatever_is_written_after() {
    let mut w = World::new(vec![cell("a", &["topic"], &["plan"]), cell("merge", &["plan"], &[])]);
    w.person_writes("topic");
    w.run("a", &[], &[("plan", Matters)]);
    w.run("merge", &[], &[]);
    assert_eq!(w.decide(), Decision::Ended(End::Passed));
    w.person_writes("topic");
    assert_eq!(w.decide(), Decision::Ended(End::Passed));
    w.owner_refuses("plan");
    assert_eq!(w.decide(), Decision::Ended(End::Passed));
    w.person_writes("plan");
    w.lose("plan");
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
    w.finish("a", &[], &[]);
    assert_eq!(w.decide(), runs("b", vec![Cause::NoRecord], NONE, NONE, vec![]));
}

/// A work with the cell that takes comments before its implementation, the implementation resting between two rounds
/// when a first comment arrives.
fn commented() -> (World, RunId, Seq) {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        on_test(COMMENT, cell("comments", &["comments"], &["notes"])),
        cell("implement", &["plan", "notes"], &["code"]),
    ];
    let mut w = World::with_gate(cells);
    w.set(COMMENT, Outcome::Waits { who: Who::Owner });
    w.person_writes("background");
    w.run("research", &[], &[("plan", Matters)]);
    let implement = w.start("implement");
    w.returns("implement");
    w.set(COMMENT, Outcome::Holds);
    let comments = w.person_writes("comments");
    (w, implement, comments)
}

#[test]
fn a_cell_before_a_run_at_rest_runs_first_and_the_run_then_goes_on_if_nothing_it_started_from_changed() {
    let (mut w, implement, comments) = commented();
    assert_eq!(
        w.decide(),
        runs(
            "comments",
            vec![Cause::NoRecord],
            &[("comments", Some(comments))],
            &[("notes", None)],
            vec![]
        )
    );
    // The comment changes nothing: the cell writes no note, and the implementation goes on where it was.
    w.run("comments", &[], &[]);
    assert_eq!(w.decide(), go_on("implement", implement));
}

#[test]
fn a_cell_after_a_run_at_rest_does_not_run_while_the_run_is_open() {
    let mut w = World::with_gate(vec![cell("a", &["x"], &[]), cell("b", &["y"], &[])]);
    w.person_writes("x");
    w.person_writes("y");
    w.run("a", &[], &[]);
    w.run("b", &[], &[]);
    w.person_writes("x");
    let a = w.start("a");
    w.returns("a");
    let y = w.person_writes("y");
    assert_eq!(w.decide(), go_on("a", a));
    w.go_on("a");
    w.finish("a", &[], &[]);
    assert_eq!(
        w.decide(),
        runs("b", vec![changed("y")], &[("y", Some(y))], NONE, vec![])
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

/// The implementation rests at a call for the owner; a first comment makes the cell before it run, and the owner
/// answers the call while that cell's round is under way.
fn answered_during_a_comment() -> (World, RunId, RunId) {
    let (mut w, implement, _) = commented();
    // `commented` left the implementation at rest with no call: give it one, in a round of its own.
    w.set(COMMENT, Outcome::Waits { who: Who::Owner });
    w.go_on("implement");
    let call = w.calls("implement", Some(Who::Owner));
    w.set(COMMENT, Outcome::Holds);
    w.person_writes("comments");
    let comments = w.start("comments");
    w.done(call);
    (w, implement, comments)
}

#[test]
fn an_answered_call_does_not_make_its_run_go_on_while_a_round_of_another_run_is_under_way() {
    let (mut w, implement, comments) = answered_during_a_comment();
    assert_eq!(w.decide(), busy("comments", comments));
    w.finish("comments", &[], &[]);
    assert_eq!(w.decide(), go_on("implement", implement));
}

#[test]
fn a_run_whose_call_was_answered_during_another_round_is_abandoned_if_that_round_changed_what_it_started_from() {
    let (mut w, implement, comments) = answered_during_a_comment();
    assert_eq!(w.decide(), busy("comments", comments));
    w.finish("comments", &[], &[("notes", Matters)]);
    assert_eq!(w.decide(), abandon("implement", implement, outdated("notes")));
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
    w.finish("a", &[], &[]);
    w.pause();
    assert_eq!(w.decide(), Decision::Paused);
    w.resume();
    assert_eq!(w.decide(), runs("b", vec![Cause::NoRecord], NONE, NONE, vec![]));
}

// Stale runs.

#[test]
fn a_run_at_rest_is_abandoned_when_its_cell_goes_off_the_path() {
    let mut w = World::with_gate(vec![cell("ask", &["topic"], &[]), on_fork("extra", "ask", &[], &[])]);
    w.person_writes("topic");
    w.run("ask", YES, &[]);
    let extra = w.start("extra");
    w.returns("extra");
    let topic = w.person_writes("topic");
    assert_eq!(
        w.decide(),
        runs("ask", vec![changed("topic")], &[("topic", Some(topic))], NONE, vec![])
    );
    w.run("ask", NO, &[]);
    assert_eq!(w.decide(), abandon("extra", extra, Stale::NoLongerApplies));
    w.abandon();
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_run_at_rest_is_abandoned_when_a_cell_before_it_changes_the_meaning_of_what_it_started_from() {
    let (mut w, implement, _) = commented();
    let plan = w.version(w.record("research"));
    w.run("comments", &[], &[("notes", Matters)]);
    assert_eq!(w.decide(), abandon("implement", implement, outdated("notes")));
    w.abandon();
    let notes = w.version(w.record("comments"));
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![Cause::NoRecord],
            &[("plan", Some(plan)), ("notes", Some(notes))],
            &[("code", None)],
            vec![]
        )
    );
}

#[test]
fn a_run_at_rest_goes_on_when_what_it_started_from_was_rewritten_in_a_way_said_not_to_matter() {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        cell("implement", &["plan"], &["code"]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    let research = w.run("research", &[], &[("plan", Matters)]);
    let plan = w.version(research);
    let implement = w.start("implement");
    w.returns("implement");
    let background = w.person_writes("background");
    assert_eq!(
        w.decide(),
        runs(
            "research",
            vec![changed("background")],
            &[("background", Some(background))],
            &[("plan", Some(plan))],
            vec![]
        )
    );
    w.run("research", &[], &[("plan", DoesNotMatter)]);
    assert_eq!(w.decide(), go_on("implement", implement));
}

#[test]
fn a_run_at_rest_is_abandoned_when_a_person_writes_a_product_it_is_to_write_over() {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        cell("implement", &["plan"], &["code"]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    w.run("research", &[], &[("plan", Matters)]);
    w.run("implement", &[], &[("code", Matters)]);
    w.person_writes("background");
    let research = w.run("research", &[], &[("plan", Matters)]);
    let plan = w.version(research);
    let implement = w.start("implement");
    w.returns("implement");
    let code = w.person_writes("code");
    assert_eq!(w.decide(), abandon("implement", implement, outdated("code")));
    w.abandon();
    // The implementation does not depend on the code, so the person's write is no reason of its own: it runs again
    // for the plan that changed, over the person's code.
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("plan")],
            &[("plan", Some(plan))],
            &[("code", Some(code))],
            vec![]
        )
    );
}

#[test]
fn a_later_cells_write_does_not_make_a_run_at_rest_of_an_earlier_cell_stale() {
    // The later cell is on the path by a named test; what it wrote while on the path comes back when the test holds
    // again, while the earlier cell's run is at rest.
    let cells = vec![
        cell("early", &["topic", "plan"], &["plan"]),
        on_test("late is on", cell("late", &[], &["plan"])),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("topic");
    let plan = w.person_writes("plan");
    w.run("early", &[], &[]);
    w.run("late", &[], &[("plan", Matters)]);
    assert_eq!(w.decide(), resting());
    w.set("late is on", Outcome::Waits { who: Who::Owner });
    let topic = w.person_writes("topic");
    assert_eq!(
        w.decide(),
        runs(
            "early",
            vec![changed("topic")],
            &[("topic", Some(topic)), ("plan", Some(plan))],
            &[("plan", Some(plan))],
            vec![]
        )
    );
    let early = w.start("early");
    w.returns("early");
    w.set("late is on", Outcome::Holds);
    assert_eq!(w.decide(), go_on("early", early));
}

#[test]
fn a_run_at_rest_is_abandoned_when_a_person_refuses_what_its_cell_answers_for_and_the_next_run_is_given_it() {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        cell("implement", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    let research = w.run("research", &[], &[("plan", Matters)]);
    let plan = w.version(research);
    w.run("implement", &[], &[]);
    let background = w.person_writes("background");
    let run = w.start("research");
    let refusal = w.owner_refuses("plan");
    // The round under way is not interrupted; when it ends the run is found not to have been given the refusal.
    assert_eq!(w.decide(), busy("research", run));
    w.returns("research");
    assert_eq!(w.decide(), abandon("research", run, Stale::Refused { refusal }));
    w.abandon();
    assert_eq!(
        w.decide(),
        runs(
            "research",
            vec![changed("background"), refused(refusal, "plan", OWNER)],
            &[("background", Some(background))],
            &[("plan", Some(plan))],
            vec![refusal]
        )
    );
    let research = w.run("research", &[], &[("plan", Matters)]);
    let plan = w.version(research);
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("plan")],
            &[("plan", Some(plan))],
            NONE,
            vec![]
        )
    );
}

#[test]
fn a_run_at_rest_is_abandoned_when_a_cell_refuses_what_its_cell_answers_for_and_the_next_run_is_given_it() {
    let mut w = World::with_gate(vec![
        cell("implement", &["plan"], &["code"]),
        cell("review", &["code"], &[]),
    ]);
    w.person_writes("plan");
    let first = w.run("implement", &[], &[("code", Matters)]);
    let code = w.version(first);
    let review = w.run("review", &[], &[]);
    let plan = w.person_writes("plan");
    let implement = w.start("implement");
    w.returns("implement");
    let refusal = w.refuse("review", "code");
    assert_eq!(w.decide(), abandon("implement", implement, Stale::Refused { refusal }));
    w.abandon();
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("plan"), refused(refusal, "code", Refuser::Run(review))],
            &[("plan", Some(plan))],
            &[("code", Some(code))],
            vec![refusal]
        )
    );
}

#[test]
fn a_run_at_rest_is_not_abandoned_for_a_refusal_an_earlier_cell_answers() {
    // The implementation may write the plan too, but the research, first on the path, answers for it.
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        cell("implement", &["plan"], &["code", "plan"]),
    ];
    let mut w = World::with_gate(cells);
    let background = w.person_writes("background");
    let research = w.run("research", &[], &[("plan", Matters)]);
    let plan = w.version(research);
    let implement = w.start("implement");
    w.returns("implement");
    let refusal = w.owner_refuses("plan");
    assert_eq!(
        w.decide(),
        runs(
            "research",
            vec![refused(refusal, "plan", OWNER)],
            &[("background", Some(background))],
            &[("plan", Some(plan))],
            vec![refusal]
        )
    );
    w.run("research", &[], &[("plan", DoesNotMatter)]);
    assert_eq!(w.decide(), go_on("implement", implement));
}

// When a cell needs to run.

#[test]
fn a_change_in_meaning_runs_only_the_cells_that_depend_on_what_changed() {
    let cells = vec![
        cell("a", &["x"], &["p"]),
        cell("b", &["y"], &[]),
        cell("c", &["p"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("x");
    w.person_writes("y");
    let a = w.run("a", &[], &[("p", Matters)]);
    let p = w.version(a);
    w.run("b", &[], &[]);
    w.run("c", &[], &[]);
    assert_eq!(w.decide(), resting());
    let x = w.person_writes("x");
    assert_eq!(
        w.decide(),
        runs("a", vec![changed("x")], &[("x", Some(x))], &[("p", Some(p))], vec![])
    );
    let a = w.run("a", &[], &[("p", Matters)]);
    let p = w.version(a);
    assert_eq!(
        w.decide(),
        runs("c", vec![changed("p")], &[("p", Some(p))], NONE, vec![])
    );
    w.run("c", &[], &[]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_write_its_writer_says_does_not_matter_runs_nobody() {
    let mut w = World::with_gate(vec![cell("a", &["x"], &["p"]), cell("c", &["p"], &[])]);
    w.person_writes("x");
    w.run("a", &[], &[("p", Matters)]);
    w.run("c", &[], &[]);
    w.person_writes("x");
    w.run("a", &[], &[("p", DoesNotMatter)]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_cell_does_not_run_for_its_own_write_of_what_it_depends_on() {
    let mut w = World::with_gate(vec![
        cell("design", &["plan"], &["plan"]),
        cell("implement", &["plan"], &[]),
    ]);
    let design = w.run("design", &[], &[("plan", Matters)]);
    let plan = w.version(design);
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![Cause::NoRecord],
            &[("plan", Some(plan))],
            NONE,
            vec![]
        )
    );
    w.run("implement", &[], &[]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_cell_does_not_run_for_a_later_cells_write_of_a_product_it_wrote_and_depends_on() {
    let mut w = World::with_gate(vec![
        cell("design", &["plan"], &["plan"]),
        cell("revise", &["plan"], &["plan"]),
    ]);
    let design = w.run("design", &[], &[("plan", Matters)]);
    let plan = w.version(design);
    assert_eq!(
        w.decide(),
        runs(
            "revise",
            vec![Cause::NoRecord],
            &[("plan", Some(plan))],
            &[("plan", Some(plan))],
            vec![]
        )
    );
    w.run("revise", &[], &[("plan", Matters)]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_cell_between_two_writers_of_a_block_sees_the_earlier_ones_version_and_a_cell_after_both_the_later_ones() {
    let cells = vec![
        cell("draft", &[], &["plan"]),
        cell("judge", &["plan"], &[]),
        cell("amend", &[], &["plan"]),
        cell("build", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    let draft = w.run("draft", &[], &[("plan", Matters)]);
    let drafted = w.version(draft);
    assert_eq!(
        w.decide(),
        runs("judge", vec![Cause::NoRecord], &[("plan", Some(drafted))], NONE, vec![])
    );
    w.run("judge", &[], &[]);
    // The later writer writes over the earlier one's version.
    assert_eq!(
        w.decide(),
        runs("amend", vec![Cause::NoRecord], NONE, &[("plan", Some(drafted))], vec![])
    );
    let amend = w.run("amend", &[], &[("plan", Matters)]);
    let amended = w.version(amend);
    assert_eq!(
        w.decide(),
        runs("build", vec![Cause::NoRecord], &[("plan", Some(amended))], NONE, vec![])
    );
    w.run("build", &[], &[]);
    // The judge, between the two, does not see the amendment.
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_cell_that_depends_on_and_may_write_a_product_starts_from_what_was_before_it_and_writes_over_its_own() {
    let cells = vec![
        cell("research", &[], &["plan"]),
        cell("design", &["plan", "style"], &["plan"]),
    ];
    let mut w = World::with_gate(cells);
    let research = w.run("research", &[], &[("plan", Matters)]);
    let researched = w.version(research);
    let style = w.person_writes("style");
    assert_eq!(
        w.decide(),
        runs(
            "design",
            vec![Cause::NoRecord],
            &[("plan", Some(researched)), ("style", Some(style))],
            &[("plan", Some(researched))],
            vec![]
        )
    );
    let design = w.run("design", &[], &[("plan", Matters)]);
    let designed = w.version(design);
    let style = w.person_writes("style");
    assert_eq!(
        w.decide(),
        runs(
            "design",
            vec![changed("style")],
            &[("plan", Some(researched)), ("style", Some(style))],
            &[("plan", Some(designed))],
            vec![]
        )
    );
}

#[test]
fn a_lost_product_runs_the_cell_that_wrote_it_and_rebuilt_as_not_mattering_runs_nobody_after() {
    let mut w = World::with_gate(vec![cell("load", &[], &["session"]), cell("work", &["session"], &[])]);
    let load = w.run("load", &[], &[("session", Matters)]);
    let session = w.version(load);
    w.run("work", &[], &[]);
    w.lose("session");
    assert_eq!(
        w.decide(),
        runs(
            "load",
            vec![gone("session")],
            NONE,
            &[("session", Some(session))],
            vec![]
        )
    );
    w.run("load", &[], &[("session", DoesNotMatter)]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_loss_followed_by_a_write_of_a_cell_that_goes_off_the_path_still_counts_as_lost() {
    let cells = vec![
        cell("ask", &["topic"], &[]),
        on_fork("alt", "ask", &[], &["session"]),
        cell("load", &[], &["session"]),
        cell("work", &["session"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("topic");
    w.run("ask", NO, &[]);
    let load = w.run("load", &[], &[("session", Matters)]);
    let session = w.version(load);
    w.run("work", &[], &[]);
    w.lose("session");
    assert_eq!(
        w.decide(),
        runs(
            "load",
            vec![gone("session")],
            NONE,
            &[("session", Some(session))],
            vec![]
        )
    );
    // Before the loss is made good, the path changes: the other cell that may write the session comes on the path,
    // writes it, and goes off the path again. What it wrote is then absent, and the loss stands.
    w.person_writes("topic");
    w.run("ask", YES, &[]);
    // The other cell comes before the load: it sees nothing the load wrote.
    assert_eq!(
        w.decide(),
        runs("alt", vec![Cause::NoRecord], NONE, &[("session", None)], vec![])
    );
    w.run("alt", &[], &[("session", Matters)]);
    let topic = w.person_writes("topic");
    assert_eq!(
        w.decide(),
        runs("ask", vec![changed("topic")], &[("topic", Some(topic))], NONE, vec![])
    );
    w.run("ask", NO, &[]);
    assert_eq!(
        w.decide(),
        runs(
            "load",
            vec![gone("session")],
            NONE,
            &[("session", Some(session))],
            vec![]
        )
    );
}

#[test]
fn a_persons_write_always_changes_the_meaning() {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        cell("implement", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    w.run("research", &[], &[("plan", Matters)]);
    w.run("implement", &[], &[]);
    let plan = w.person_writes("plan");
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("plan")],
            &[("plan", Some(plan))],
            NONE,
            vec![]
        )
    );
}

#[test]
fn a_write_from_outside_the_work_always_changes_the_meaning() {
    let mut w = World::with_gate(vec![cell("check", &["main"], &[])]);
    w.outside_writes("main");
    w.run("check", &[], &[]);
    assert_eq!(w.decide(), resting());
    let main = w.outside_writes("main");
    assert_eq!(
        w.decide(),
        runs("check", vec![changed("main")], &[("main", Some(main))], NONE, vec![])
    );
}

// Meaning.

#[test]
fn a_write_said_not_to_matter_after_a_version_now_absent_changes_the_meaning_for_who_was_built_on_that_version() {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        on_fork("design", "research", &["plan"], &["plan"]),
        cell("implement", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    let research = w.run("research", YES, &[("plan", Matters)]);
    let researched = w.version(research);
    let design = w.run("design", &[], &[("plan", Matters)]);
    let designed = w.version(design);
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![Cause::NoRecord],
            &[("plan", Some(designed))],
            NONE,
            vec![]
        )
    );
    w.run("implement", &[], &[]);
    let background = w.person_writes("background");
    // The research writes over its own plan: the design's comes after it.
    assert_eq!(
        w.decide(),
        runs(
            "research",
            vec![changed("background")],
            &[("background", Some(background))],
            &[("plan", Some(researched))],
            vec![]
        )
    );
    // The research no longer sends the work to design, and says its own plan does not matter compared with its
    // earlier one. The design is now off the path and its plan absent: the implementation was built on a version
    // that no longer counts, and runs.
    let research = w.run("research", NO, &[("plan", DoesNotMatter)]);
    let plan = w.version(research);
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("plan")],
            &[("plan", Some(plan))],
            NONE,
            vec![]
        )
    );
}

// Refusals.

/// An implementation reviewed once, the review resting at the gate.
fn reviewed() -> World {
    let mut w = World::with_gate(vec![
        cell("implement", &["plan"], &["code"]),
        cell("review", &["code"], &[]),
    ]);
    w.person_writes("plan");
    w.run("implement", &[], &[("code", Matters)]);
    w.run("review", &[], &[]);
    w
}

#[test]
fn a_cells_refusal_runs_the_writer_given_it_whose_write_ends_it_and_brings_the_refuser_back_even_when_not_mattering() {
    let mut w = reviewed();
    let plan = w.basis_of(w.record("implement"), "plan");
    let code = w.version(w.record("implement"));
    let refusal = w.refuse("review", "code");
    let by = Refuser::Run(w.record("review"));
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![refused(refusal, "code", by)],
            &[("plan", plan)],
            &[("code", Some(code))],
            vec![refusal]
        )
    );
    let implement = w.run("implement", &[], &[("code", DoesNotMatter)]);
    let code = w.version(implement);
    assert_eq!(
        w.decide(),
        runs(
            "review",
            vec![answered(refusal, "code")],
            &[("code", Some(code))],
            NONE,
            vec![]
        )
    );
    w.run("review", &[], &[]);
    assert_eq!(w.decide(), resting());
}

/// A work whose comment cell, before the implementation, has a record; the item has had a comment from the start.
fn with_comments() -> World {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        on_test(COMMENT, cell("comments", &["comments"], &["notes"])),
        cell("implement", &["plan", "notes"], &["code"]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    w.person_writes("comments");
    w.run("research", &[], &[("plan", Matters)]);
    w.run("comments", &[], &[]);
    w.run("implement", &[], &[("code", Matters)]);
    assert_eq!(w.decide(), resting());
    w
}

#[test]
fn a_refuser_that_does_not_depend_on_what_it_refused_is_brought_back_by_the_answer() {
    let mut w = with_comments();
    let background = w.basis_of(w.record("research"), "background");
    let comments = w.basis_of(w.record("comments"), "comments");
    let plan = w.version(w.record("research"));
    let refusal = w.refuse_version("comments", "plan", plan);
    let by = Refuser::Run(w.record("comments"));
    assert_eq!(
        w.decide(),
        runs(
            "research",
            vec![refused(refusal, "plan", by)],
            &[("background", background)],
            &[("plan", Some(plan))],
            vec![refusal]
        )
    );
    w.run("research", &[], &[("plan", DoesNotMatter)]);
    assert_eq!(
        w.decide(),
        runs(
            "comments",
            vec![answered(refusal, "plan")],
            &[("comments", comments)],
            &[("notes", None)],
            vec![]
        )
    );
}

/// A research whose plan an implementation was built on.
fn planned() -> World {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        cell("implement", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    w.run("research", &[], &[("plan", Matters)]);
    w.run("implement", &[], &[]);
    w
}

#[test]
fn a_persons_refusal_runs_the_writer_given_it_and_brings_back_no_cell() {
    let mut w = planned();
    let background = w.basis_of(w.record("research"), "background");
    let plan = w.version(w.record("research"));
    let refusal = w.owner_refuses("plan");
    assert_eq!(
        w.decide(),
        runs(
            "research",
            vec![refused(refusal, "plan", OWNER)],
            &[("background", background)],
            &[("plan", Some(plan))],
            vec![refusal]
        )
    );
    w.run("research", &[], &[("plan", DoesNotMatter)]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_refusal_is_not_ended_by_the_write_of_a_run_that_was_not_given_it() {
    let mut w = planned();
    let background = w.person_writes("background");
    w.start("research");
    let refusal = w.owner_refuses("plan");
    // The round under way finishes the run: it was not given the refusal, so what it writes does not end it.
    w.finish("research", &[], &[("plan", Matters)]);
    let plan = w.version(w.record("research"));
    assert_eq!(
        w.decide(),
        runs(
            "research",
            vec![refused(refusal, "plan", OWNER)],
            &[("background", Some(background))],
            &[("plan", Some(plan))],
            vec![refusal]
        )
    );
}

#[test]
fn of_two_cells_that_wrote_a_refused_product_the_first_on_the_path_answers() {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        cell("design", &["plan"], &["plan"]),
        cell("implement", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    let background = w.person_writes("background");
    let research = w.run("research", &[], &[("plan", Matters)]);
    let researched = w.version(research);
    w.run("design", &[], &[("plan", Matters)]);
    w.run("implement", &[], &[]);
    let refusal = w.owner_refuses("plan");
    // The research writes over its own version: the design's, which the owner refused, comes after it.
    assert_eq!(
        w.decide(),
        runs(
            "research",
            vec![refused(refusal, "plan", OWNER)],
            &[("background", Some(background))],
            &[("plan", Some(researched))],
            vec![refusal]
        )
    );
    let research = w.run("research", &[], &[("plan", Matters)]);
    let plan = w.version(research);
    // The research's write ends the refusal: the design runs for the change, and is given nothing.
    assert_eq!(
        w.decide(),
        runs(
            "design",
            vec![changed("plan")],
            &[("plan", Some(plan))],
            &[("plan", Some(plan))],
            vec![]
        )
    );
}

#[test]
fn a_cell_that_given_a_refusal_does_not_write_the_product_again_leaves_it_to_the_next_that_may_write_it() {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        on_fork("design", "research", &["plan"], &["plan"]),
        cell("implement", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    let background = w.person_writes("background");
    let research = w.run("research", NO, &[("plan", Matters)]);
    let plan = w.version(research);
    w.run("implement", &[], &[]);
    let refusal = w.owner_refuses("plan");
    assert_eq!(
        w.decide(),
        runs(
            "research",
            vec![refused(refusal, "plan", OWNER)],
            &[("background", Some(background))],
            &[("plan", Some(plan))],
            vec![refusal]
        )
    );
    // The research hands the plan to design: it says design is needed and writes no plan. Nothing is withdrawn. The
    // design has no record, which is its reason to run (the others are reasons of a record); it is given the refusal.
    w.run("research", YES, &[]);
    assert_eq!(
        w.decide(),
        runs(
            "design",
            vec![Cause::NoRecord],
            &[("plan", Some(plan))],
            &[("plan", Some(plan))],
            vec![refusal]
        )
    );
    let design = w.run("design", &[], &[("plan", Matters)]);
    let designed = w.version(design);
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("plan")],
            &[("plan", Some(designed))],
            NONE,
            vec![]
        )
    );
}

#[test]
fn every_run_of_a_cell_that_may_write_a_refused_product_is_given_the_refusal_even_when_it_runs_for_another_reason() {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        on_fork("design", "research", &[], &["plan"]),
        cell("implement", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    w.run("research", YES, &[]);
    let design = w.run("design", &[], &[("plan", Matters)]);
    let plan = w.version(design);
    w.run("implement", &[], &[]);
    let refusal = w.owner_refuses("plan");
    // The research wrote no plan in its record, so the design answers.
    assert_eq!(
        w.decide(),
        runs(
            "design",
            vec![refused(refusal, "plan", OWNER)],
            NONE,
            &[("plan", Some(plan))],
            vec![refusal]
        )
    );
    // The research writes over nothing (the design's plan comes after it), and is given the refusal all the same.
    let background = w.person_writes("background");
    assert_eq!(
        w.decide(),
        runs(
            "research",
            vec![changed("background")],
            &[("background", Some(background))],
            &[("plan", None)],
            vec![refusal]
        )
    );
}

#[test]
fn a_refusal_no_cell_answers_holds_the_work_before_the_first_cell_that_depends_on_the_product() {
    let mut w = planned();
    let refusal = w.owner_refuses("plan");
    w.run("research", &[], &[]);
    assert_eq!(w.decide(), wait("implement", vec![unanswered(refusal, "plan", OWNER)]));
    // A person writing the product ends the refusal.
    let plan = w.person_writes("plan");
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("plan")],
            &[("plan", Some(plan))],
            NONE,
            vec![]
        )
    );
}

#[test]
fn a_refusal_no_cell_answers_of_a_product_nobody_depends_on_holds_the_work_before_the_merge() {
    let mut w = World::new(vec![cell("research", &["background"], &["plan"]), Cell::new("merge")]);
    w.person_writes("background");
    w.run("research", &[], &[("plan", Matters)]);
    let refusal = w.owner_refuses("plan");
    w.run("research", &[], &[]);
    assert_eq!(w.decide(), wait("merge", vec![unanswered(refusal, "plan", OWNER)]));
}

#[test]
fn a_cells_refusal_loses_force_when_that_cell_runs_again() {
    let mut w = with_comments();
    let code = w.version(w.record("implement"));
    let refusal = w.refuse_version("comments", "code", code);
    let by = Refuser::Run(w.record("comments"));
    let plan = w.version(w.record("research"));
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![refused(refusal, "code", by)],
            &[("plan", Some(plan)), ("notes", None)],
            &[("code", Some(code))],
            vec![refusal]
        )
    );
    let comments = w.person_writes("comments");
    assert_eq!(
        w.decide(),
        runs(
            "comments",
            vec![changed("comments")],
            &[("comments", Some(comments))],
            &[("notes", None)],
            vec![]
        )
    );
    // Its record is now another run, which refused nothing.
    w.run("comments", &[], &[]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_cells_refusal_loses_force_when_that_cell_goes_off_the_path() {
    let mut w = with_comments();
    let code = w.version(w.record("implement"));
    w.refuse_version("comments", "code", code);
    w.set(COMMENT, Outcome::Waits { who: Who::Owner });
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_refusal_of_a_version_written_by_a_cell_now_off_the_path_has_no_force_even_a_persons() {
    let cells = vec![
        cell("research", &["background"], &[]),
        on_fork("alt", "research", &[], &["plan"]),
        cell("implement", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    w.run("research", YES, &[]);
    let alt = w.run("alt", &[], &[("plan", Matters)]);
    let plan = w.version(alt);
    w.run("implement", &[], &[]);
    let refusal = w.owner_refuses("plan");
    assert_eq!(
        w.decide(),
        runs(
            "alt",
            vec![refused(refusal, "plan", OWNER)],
            NONE,
            &[("plan", Some(plan))],
            vec![refusal]
        )
    );
    let background = w.person_writes("background");
    assert_eq!(
        w.decide(),
        runs(
            "research",
            vec![changed("background")],
            &[("background", Some(background))],
            NONE,
            vec![]
        )
    );
    w.run("research", NO, &[]);
    // The refused plan is absent: nobody answers, and nothing is held; the implementation runs for its loss.
    assert_eq!(
        w.decide(),
        runs("implement", vec![changed("plan")], &[("plan", None)], NONE, vec![])
    );
}

// The limit.

/// The review refuses the code, the implementation writes it again and the review judges again.
fn send_back(w: &mut World) {
    w.refuse("review", "code");
    w.run("implement", &[], &[("code", Matters)]);
    w.run("review", &[], &[]);
}

fn at_limit(w: &mut World, runs: u32) {
    w.refuse("review", "code");
    assert_eq!(w.decide(), wait("implement", vec![Waited::Limit { runs }]));
}

/// The decision to run the implementation for the latest refusal, given it.
fn runs_for_refusal(w: &World) -> Decision {
    let refusal = w.records.refusals.last().expect("a refusal").id;
    let by = w.records.refusals.last().expect("a refusal").by.clone();
    let record = w.record("implement");
    runs(
        "implement",
        vec![refused(refusal, "code", by)],
        &[("plan", w.basis_of(record, "plan"))],
        &[("code", Some(w.version(record)))],
        vec![refusal],
    )
}

#[test]
fn a_cell_may_run_as_many_ways_round_in_a_row_as_its_limit_and_the_next_waits_for_the_owner() {
    let mut w = reviewed();
    for _ in 0..3 {
        send_back(&mut w);
    }
    at_limit(&mut w, 3);
    w.let_go("implement");
    assert_eq!(w.decide(), runs_for_refusal(&w));
}

#[test]
fn letting_a_cell_go_on_starts_its_count_anew() {
    let mut w = reviewed();
    for _ in 0..3 {
        send_back(&mut w);
    }
    at_limit(&mut w, 3);
    w.let_go("implement");
    w.run("implement", &[], &[("code", Matters)]);
    w.run("review", &[], &[]);
    for _ in 0..2 {
        send_back(&mut w);
    }
    at_limit(&mut w, 3);
}

#[test]
fn a_run_for_a_reason_from_before_its_cell_starts_the_count_anew() {
    let mut w = reviewed();
    for _ in 0..3 {
        send_back(&mut w);
    }
    let plan = w.person_writes("plan");
    let code = w.version(w.record("implement"));
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("plan")],
            &[("plan", Some(plan))],
            &[("code", Some(code))],
            vec![]
        )
    );
    w.run("implement", &[], &[("code", Matters)]);
    w.run("review", &[], &[]);
    for _ in 0..3 {
        send_back(&mut w);
    }
    at_limit(&mut w, 3);
}

#[test]
fn a_persons_refusal_starts_the_count_anew() {
    let mut w = reviewed();
    for _ in 0..3 {
        send_back(&mut w);
    }
    w.owner_refuses("code");
    assert_eq!(w.decide(), runs_for_refusal(&w));
    w.run("implement", &[], &[("code", Matters)]);
    w.run("review", &[], &[]);
    for _ in 0..3 {
        send_back(&mut w);
    }
    at_limit(&mut w, 3);
}

#[test]
fn two_cells_sending_each_other_back_by_refusals_are_bounded_by_the_earlier_ones_limit() {
    // The later cell refuses what the earlier writes and writes what nobody before it depends on.
    let mut w = World::with_gate(vec![with_limit(2, cell("a", &[], &["x"])), cell("b", &["x"], &["y"])]);
    w.run("a", &[], &[("x", Matters)]);
    w.run("b", &[], &[("y", Matters)]);
    for _ in 0..2 {
        let refusal = w.refuse("b", "x");
        let by = Refuser::Run(w.record("b"));
        let x = w.version(w.record("a"));
        assert_eq!(
            w.decide(),
            runs(
                "a",
                vec![refused(refusal, "x", by)],
                NONE,
                &[("x", Some(x))],
                vec![refusal]
            )
        );
        let a = w.run("a", &[], &[("x", Matters)]);
        let x = w.version(a);
        let y = w.version(w.record("b"));
        // The later cell's every run has a reason from before it: the earlier one's write.
        assert_eq!(
            w.decide(),
            runs(
                "b",
                vec![changed("x"), answered(refusal, "x")],
                &[("x", Some(x))],
                &[("y", Some(y))],
                vec![]
            )
        );
        w.run("b", &[], &[("y", Matters)]);
    }
    w.refuse("b", "x");
    assert_eq!(w.decide(), wait("a", vec![Waited::Limit { runs: 2 }]));
}

#[test]
fn a_later_cells_write_of_what_an_earlier_one_depends_on_changes_nothing_for_the_earlier_one() {
    let mut w = World::with_gate(vec![cell("a", &["y"], &["x"]), cell("b", &["x"], &["y"])]);
    w.run("a", &[], &[("x", Matters)]);
    w.run("b", &[], &[("y", Matters)]);
    assert_eq!(w.decide(), resting());
}

/// A circle through a condition: the first cell depends on what the third writes, the third is on the path by the
/// second's answer, and the second depends on what the first writes and on a topic people write.
fn circle() -> World {
    let cells = vec![
        with_limit(1, cell("first", &["p"], &["x"])),
        cell("second", &["x", "topic"], &[]),
        on_fork("third", "second", &[], &["p"]),
    ];
    World::with_gate(cells)
}

#[test]
fn a_later_cell_leaving_the_path_changes_nothing_for_a_cell_before_it() {
    let mut w = circle();
    w.run("first", &[], &[("x", Matters)]);
    w.run("second", YES, &[]);
    w.run("third", &[], &[("p", Matters)]);
    // The first does not see what the third wrote.
    assert_eq!(w.decide(), resting());
    let topic = w.person_writes("topic");
    let x = w.version(w.record("first"));
    assert_eq!(
        w.decide(),
        runs(
            "second",
            vec![changed("topic")],
            &[("x", Some(x)), ("topic", Some(topic))],
            NONE,
            vec![]
        )
    );
    w.run("second", NO, &[]);
    // The third's write is now absent; the first never saw it.
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_circle_closed_by_a_condition_comes_to_rest_whatever_the_later_cell_answers() {
    let mut w = circle();
    // The agents: the first and the third write anew at every run, the second answers yes and no in turn.
    let mut yes = true;
    for _ in 0..30 {
        match w.decide() {
            Decision::Run { cell, .. } if cell.as_str() == "second" => {
                w.run("second", if yes { YES } else { NO }, &[]);
                yes = !yes;
            }
            Decision::Run { cell, .. } if cell.as_str() == "first" => {
                w.run("first", &[], &[("x", Matters)]);
            }
            Decision::Run { cell, .. } if cell.as_str() == "third" => {
                w.run("third", &[], &[("p", Matters)]);
            }
            Decision::Wait { .. } => return,
            other => panic!("the walk decides {other:?}"),
        }
    }
    let first: Vec<_> = (w.records.started.iter())
        .filter(|s| s.cell.as_str() == "first")
        .map(|s| s.because.clone())
        .collect();
    panic!("thirty runs and the work has not come to rest; the first cell ran because of {first:?}");
}

#[test]
fn a_cell_rebuilding_a_lost_product_again_and_again_is_bounded_by_its_limit() {
    let mut w = World::with_gate(vec![
        with_limit(2, cell("load", &[], &["session"])),
        cell("work", &["session"], &[]),
    ]);
    w.run("load", &[], &[("session", Matters)]);
    w.run("work", &[], &[]);
    for _ in 0..2 {
        w.lose("session");
        let session = w.version(w.record("load"));
        assert_eq!(
            w.decide(),
            runs(
                "load",
                vec![gone("session")],
                NONE,
                &[("session", Some(session))],
                vec![]
            )
        );
        w.run("load", &[], &[("session", DoesNotMatter)]);
    }
    w.lose("session");
    assert_eq!(w.decide(), wait("load", vec![Waited::Limit { runs: 2 }]));
}

#[test]
fn a_limit_of_zero_is_refused() {
    let cells = vec![with_limit(0, cell("a", &[], &[])), gate()];
    assert_eq!(Flow::new(cells), Err(FlowFault::NoLimit("a".into())));
}

// What a work waits for.

#[test]
fn a_precondition_holds_the_work_before_its_cell_even_when_the_cell_needs_nothing() {
    let mut w = World::with_gate(vec![requiring("a may go", cell("a", &[], &[])), cell("b", &["y"], &[])]);
    w.person_writes("y");
    w.run("a", &[], &[]);
    w.run("b", &[], &[]);
    w.set("a may go", Outcome::Waits { who: Who::Architect });
    w.person_writes("y");
    let precondition = Waited::Precondition {
        test: "a may go".into(),
        who: Who::Architect,
    };
    assert_eq!(w.decide(), wait("a", vec![precondition]));
}

#[test]
fn a_precondition_of_a_cell_off_the_path_holds_nothing() {
    let extra = requiring("extra may go", on_fork("extra", "ask", &[], &[]));
    let mut w = World::with_gate(vec![cell("ask", &[], &[]), extra]);
    w.set("extra may go", Outcome::Waits { who: Who::Architect });
    w.run("ask", NO, &[]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_call_and_an_unmet_precondition_at_one_cell_are_waited_for_at_once() {
    let mut w = World::with_gate(vec![requiring("a may go", cell("a", &[], &[]))]);
    w.start("a");
    let call = w.calls("a", Some(Who::Owner));
    w.set("a may go", Outcome::Waits { who: Who::Architect });
    let precondition = Waited::Precondition {
        test: "a may go".into(),
        who: Who::Architect,
    };
    let call = Waited::Call {
        call,
        on: Some(Who::Owner),
    };
    assert_eq!(w.decide(), wait("a", vec![call, precondition]));
}

#[test]
fn an_unmet_precondition_and_the_limit_at_one_cell_are_waited_for_at_once() {
    let mut w = World::with_gate(vec![
        requiring("implement may go", cell("implement", &["plan"], &["code"])),
        cell("review", &["code"], &[]),
    ]);
    w.person_writes("plan");
    w.run("implement", &[], &[("code", Matters)]);
    w.run("review", &[], &[]);
    for _ in 0..3 {
        send_back(&mut w);
    }
    w.refuse("review", "code");
    w.set("implement may go", Outcome::Waits { who: Who::Owner });
    let precondition = Waited::Precondition {
        test: "implement may go".into(),
        who: Who::Owner,
    };
    assert_eq!(
        w.decide(),
        wait("implement", vec![precondition, Waited::Limit { runs: 3 }])
    );
}

#[test]
fn an_unmet_precondition_of_the_merge_and_a_refusal_no_cell_answers_are_waited_for_at_once() {
    let mut w = World::with_gate(vec![cell("research", &["background"], &["plan"])]);
    w.person_writes("background");
    w.run("research", &[], &[("plan", Matters)]);
    let refusal = w.owner_refuses("plan");
    w.run("research", &[], &[]);
    assert_eq!(
        w.decide(),
        wait("merge", vec![unanswered(refusal, "plan", OWNER), merge_word()])
    );
}

// A changed flow.

#[test]
fn a_cell_new_to_the_flow_has_no_record_and_runs_when_the_walk_reaches_it() {
    let mut w = World::with_gate(vec![cell("a", &[], &[]), cell("b", &[], &[])]);
    w.run("a", &[], &[]);
    w.run("b", &[], &[]);
    assert_eq!(w.decide(), resting());
    w.install(vec![
        cell("a", &[], &[]),
        cell("new", &[], &[]),
        cell("b", &[], &[]),
        gate(),
    ]);
    assert_eq!(w.decide(), runs("new", vec![Cause::NoRecord], NONE, NONE, vec![]));
}

#[test]
fn what_a_cell_taken_out_of_the_flow_wrote_is_absent() {
    let mut w = World::with_gate(vec![cell("a", &[], &["p"]), cell("b", &["p"], &[])]);
    w.run("a", &[], &[("p", Matters)]);
    w.run("b", &[], &[]);
    w.install(vec![cell("b", &["p"], &[]), gate()]);
    assert_eq!(w.decide(), runs("b", vec![changed("p")], &[("p", None)], NONE, vec![]));
}

#[test]
fn a_refusal_by_a_cell_taken_out_of_the_flow_has_no_force() {
    let mut w = reviewed();
    w.refuse("review", "code");
    w.install(vec![cell("implement", &["plan"], &["code"]), gate()]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_run_at_rest_of_a_cell_taken_out_of_the_flow_is_abandoned() {
    let mut w = World::with_gate(vec![cell("a", &[], &[]), cell("b", &[], &[])]);
    w.run("a", &[], &[]);
    let b = w.start("b");
    w.returns("b");
    w.install(vec![cell("a", &[], &[]), gate()]);
    assert_eq!(w.decide(), abandon("b", b, Stale::NoLongerApplies));
    w.abandon();
    assert_eq!(w.decide(), resting());
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
fn a_refusal_by_a_run_that_never_started_is_a_fault() {
    let mut w = World::with_gate(vec![cell("a", &["p"], &[])]);
    let p = w.person_writes("p");
    let at = w.tick();
    w.records.refusals.push(decide::Refusal {
        id: RefusalId(98),
        by: Refuser::Run(RunId(99)),
        product: "p".into(),
        version: p,
        at,
    });
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
fn a_run_finished_twice_is_a_fault() {
    let mut w = World::with_gate(vec![cell("a", &[], &[])]);
    let run = w.run("a", &[], &[]);
    let mut again = w.records.finished[0].clone();
    again.at = w.tick();
    w.records.finished.push(again);
    assert_eq!(w.decide(), Decision::Fault(RecordFault::FinishedTwice(run)));
}

#[test]
fn a_run_both_finished_and_abandoned_is_a_fault() {
    let mut w = World::with_gate(vec![cell("a", &[], &[])]);
    let run = w.run("a", &[], &[]);
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
        product: "p".into(),
        by: decide::Outsider::Person,
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
fn a_condition_on_the_cells_own_answer_is_refused() {
    let cells = vec![on_fork("a", "a", &[], &[]), gate()];
    let fault = FlowFault::ConditionNotBefore {
        cell: "a".into(),
        by: "a".into(),
    };
    assert_eq!(Flow::new(cells), Err(fault));
}

#[test]
fn a_condition_on_a_later_cells_answer_is_refused() {
    let cells = vec![on_fork("a", "b", &[], &[]), cell("b", &[], &[]), gate()];
    let fault = FlowFault::ConditionNotBefore {
        cell: "a".into(),
        by: "b".into(),
    };
    assert_eq!(Flow::new(cells), Err(fault));
}

#[test]
fn a_merge_with_a_condition_on_an_answer_is_refused() {
    let cells = vec![cell("a", &[], &[]), on_fork("merge", "a", &[], &[])];
    assert_eq!(Flow::new(cells), Err(FlowFault::LastHasCondition("merge".into())));
}

#[test]
fn a_merge_with_a_condition_on_a_named_test_is_refused() {
    let cells = vec![cell("a", &[], &[]), on_test(COMMENT, Cell::new("merge"))];
    assert_eq!(Flow::new(cells), Err(FlowFault::LastHasCondition("merge".into())));
}
