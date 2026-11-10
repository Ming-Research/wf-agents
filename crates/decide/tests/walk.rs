//! The walk, case by case: each test names one thing the walk must do and shows it on a small flow.

mod support;

use decide::{Cause, Cell, Condition, Decision, End, Flow, FlowFault, Outcome, RefusalId, Refuser, Stale, Waited, Who};
use support::{World, Write::*, cell, gate};

fn waits(what: &str, who: Who) -> Outcome {
    Outcome::Waits {
        what: what.to_string(),
        who,
    }
}

fn changed(product: &str, built: Option<u32>, now: Option<u32>) -> Cause {
    Cause::Changed {
        product: product.into(),
        built,
        now,
    }
}

fn refused(refusal: RefusalId, product: &str, by: Refuser, why: &str) -> Cause {
    Cause::Refused {
        refusal,
        product: product.into(),
        by,
        why: why.to_string(),
    }
}

/// A cell that is on the path when the cell `by` answered this way.
fn when(mut cell: Cell, by: &str, way: &str) -> Cell {
    cell.applies = Some(Condition::Answered {
        by: by.into(),
        output: "way".into(),
        is: way.to_string(),
    });
    cell
}

/// The wait the walk decides, with the cell it waits at.
fn waiting(w: &World) -> (String, Vec<Waited>) {
    match w.decide() {
        Decision::Wait { at, on } => (at.0, on),
        other => panic!("the work should wait: {other:?}"),
    }
}

#[test]
fn cells_make_a_flow_only_when_named_once_with_conditions_on_cells_before_and_none_on_the_last() {
    assert_eq!(Flow::new(vec![]), Err(FlowFault::Empty));
    assert_eq!(
        Flow::new(vec![cell("a", &[], &[]), cell("a", &[], &[])]),
        Err(FlowFault::Twice("a".into()))
    );
    let fault = FlowFault::ConditionNotBefore {
        cell: "a".into(),
        by: "b".into(),
    };
    assert_eq!(
        Flow::new(vec![when(cell("a", &[], &[]), "b", "yes"), cell("b", &[], &[])]),
        Err(fault)
    );
    let last = when(cell("b", &[], &[]), "a", "yes");
    assert_eq!(
        Flow::new(vec![cell("a", &[], &[]), last]),
        Err(FlowFault::LastHasCondition("b".into()))
    );
}

#[test]
fn a_work_walks_its_cells_in_order_and_has_ended_once_its_last_cell_has_a_record() {
    let mut w = World::new(vec![
        cell("a", &[], &["x"]),
        cell("b", &["x"], &["y"]),
        cell("c", &["y"], &[]),
    ]);
    assert_eq!(w.runs_next().as_deref(), Some("a"));
    assert_eq!(w.because(), vec![Cause::NoRecord]);
    w.run("a", &[], &[("x", Matters)]);
    assert_eq!(w.runs_next().as_deref(), Some("b"));
    w.run("b", &[], &[("y", Matters)]);
    w.run("c", &[], &[]);
    assert_eq!(w.decide(), Decision::Ended(End::Passed));
    // Whatever changes after, an ended work is not walked.
    w.person_writes("x");
    assert_eq!(w.decide(), Decision::Ended(End::Passed));
}

#[test]
fn nothing_is_decided_while_a_cell_runs_or_the_work_is_paused_and_a_cancelled_work_has_ended() {
    let mut w = World::new(vec![cell("a", &[], &[]), cell("b", &[], &[])]);
    let run = w.start("a");
    assert_eq!(
        w.decide(),
        Decision::Busy {
            cell: "a".into(),
            run,
            deliver: vec![]
        }
    );
    w.pause();
    assert_eq!(w.decide(), Decision::Paused);
    w.cancel();
    assert_eq!(w.decide(), Decision::Ended(End::Cancelled));
}

#[test]
fn a_cell_off_the_path_is_skipped_until_the_answer_before_it_changes() {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        when(cell("design", &[], &["plan"]), "research", "design"),
        cell("implement", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    w.run("research", &[("way", "implement")], &[("plan", Matters)]);
    // The design is skipped: the research answered that the work is implemented next.
    assert_eq!(w.runs_next().as_deref(), Some("implement"));
    w.run("implement", &[], &[]);
    assert!(w.rests());
    // The background changes; the research runs again and now sends the work to design.
    w.person_writes("background");
    w.run("research", &[("way", "design")], &[("plan", Matters)]);
    assert_eq!(w.runs_next().as_deref(), Some("design"));
    w.run("design", &[], &[("plan", Matters)]);
    assert_eq!(w.runs_next().as_deref(), Some("implement"));
}

#[test]
fn a_cell_whose_condition_reads_a_cell_off_the_path_is_off_the_path_too() {
    let cells = vec![
        cell("a", &["in"], &[]),
        when(cell("b", &[], &[]), "a", "yes"),
        when(cell("c", &[], &[]), "b", "yes"),
        cell("d", &[], &[]),
    ];
    let mut w = World::new(cells);
    w.person_writes("in");
    w.run("a", &[("way", "yes")], &[]);
    w.run("b", &[("way", "yes")], &[]);
    w.run("c", &[], &[]);
    // The first cell runs again and answers no: the second is off the path, and its answer no longer counts.
    w.person_writes("in");
    w.run("a", &[("way", "no")], &[]);
    assert_eq!(w.runs_next().as_deref(), Some("d"));
}

#[test]
fn the_work_waits_before_a_cell_whose_precondition_does_not_hold_and_says_for_whom() {
    let mut implement = cell("implement", &["plan"], &[]);
    implement.requires = vec!["stop questions confirmed".into()];
    let mut w = World::new(vec![cell("research", &[], &["plan"]), implement]);
    w.set("stop questions confirmed", waits("the confirmation of q1", Who::Owner));
    w.run("research", &[], &[("plan", Matters)]);
    let on = vec![Waited::Precondition {
        test: "stop questions confirmed".into(),
        what: "the confirmation of q1".to_string(),
        who: Who::Owner,
    }];
    assert_eq!(
        w.decide(),
        Decision::Wait {
            at: "implement".into(),
            on
        }
    );
    w.set("stop questions confirmed", Outcome::Holds);
    assert_eq!(w.runs_next().as_deref(), Some("implement"));
}

#[test]
fn a_change_of_meaning_runs_only_the_cells_that_depend_on_it() {
    let cells = vec![
        cell("a", &[], &["x"]),
        cell("b", &[], &["y"]),
        cell("c", &["x"], &[]),
        cell("d", &["y"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.run("a", &[], &[("x", Matters)]);
    w.run("b", &[], &[("y", Matters)]);
    w.run("c", &[], &[]);
    w.run("d", &[], &[]);
    assert!(w.rests());
    w.person_writes("x");
    assert_eq!(w.runs_next().as_deref(), Some("c"));
    assert_eq!(w.because(), vec![changed("x", Some(1), Some(2))]);
    w.run("c", &[], &[]);
    // The cell that depends on the other product was passed.
    assert!(w.rests());
}

#[test]
fn a_change_its_writer_says_does_not_matter_runs_nothing_after_it() {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        cell("implement", &["plan"], &["change"]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    w.run("research", &[], &[("plan", Matters)]);
    w.run("implement", &[], &[("change", Matters)]);
    // The title is reworded: the research adjusts a few words of its plan and says nothing after it is touched.
    w.person_writes("background");
    assert_eq!(w.runs_next().as_deref(), Some("research"));
    w.run("research", &[], &[("plan", DoesNotMatter)]);
    assert!(w.rests());
}

#[test]
fn a_cell_is_not_run_again_for_what_it_wrote_itself_nor_for_a_later_write_said_not_to_matter() {
    let cells = vec![
        cell("research", &["background"], &["question"]),
        cell("tree", &["question"], &["question"]),
        cell("implement", &["question"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    w.run("research", &[], &[("question", Matters)]);
    w.run("tree", &[], &[("question", Matters)]);
    w.run("implement", &[], &[]);
    assert!(w.rests());
    // The research writes the question again and says it does not matter: the cell that wrote it after is built on
    // its own write, whose meaning still stands.
    w.person_writes("background");
    w.run("research", &[], &[("question", DoesNotMatter)]);
    assert!(w.rests());
}

#[test]
fn a_refusal_runs_the_writer_again_and_its_new_version_runs_the_refuser_again() {
    let cells = vec![
        cell("fix", &["findings"], &["fixed"]),
        cell("recheck", &["fixed"], &["verdict"]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("findings");
    w.run("fix", &[], &[("fixed", Matters)]);
    w.run("recheck", &[], &[("verdict", Matters)]);
    assert!(w.rests());
    let by = Refuser::Run(w.record("recheck"));
    let refusal = w.refuse("recheck", "fixed", "the second finding is not fixed");
    assert_eq!(w.runs_next().as_deref(), Some("fix"));
    assert_eq!(
        w.because(),
        vec![refused(refusal, "fixed", by, "the second finding is not fixed")]
    );
    w.run("fix", &[], &[("fixed", Matters)]);
    // A run that was given the refusal wrote the product anew: the refusal has ended, and the re-check judges again.
    assert_eq!(w.runs_next().as_deref(), Some("recheck"));
    assert_eq!(w.because(), vec![changed("fixed", Some(1), Some(2))]);
    w.run("recheck", &[], &[]);
    assert!(w.rests());
}

#[test]
fn an_answer_to_a_refusal_reaches_the_refuser_even_when_its_writer_says_it_does_not_matter() {
    let cells = vec![cell("fix", &["findings"], &["fixed"]), cell("recheck", &["fixed"], &[])];
    let mut w = World::with_gate(cells);
    w.person_writes("findings");
    w.run("fix", &[], &[("fixed", Matters)]);
    w.run("recheck", &[], &[]);
    w.refuse("recheck", "fixed", "not fixed");
    w.run("fix", &[], &[("fixed", DoesNotMatter)]);
    assert_eq!(w.runs_next().as_deref(), Some("recheck"));
    assert_eq!(
        w.because(),
        vec![Cause::Answered {
            product: "fixed".into()
        }]
    );
}

#[test]
fn a_refusal_is_not_ended_by_a_run_that_was_not_given_it_nor_by_the_mark_of_a_loss() {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        cell("implement", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    w.run("research", &[], &[("plan", Matters)]);
    w.run("implement", &[], &[]);
    // The research is running again for another reason when the owner refuses the plan as it stood.
    w.person_writes("background");
    w.start("research");
    let refusal = w.owner_refuses("plan", "take the other way");
    w.finish("research", &[], &[("plan", Matters)]);
    // The run was not given the refusal, so what it wrote does not end it: the research runs again, now given it.
    assert_eq!(w.runs_next().as_deref(), Some("research"));
    assert_eq!(
        w.because(),
        vec![refused(
            refusal,
            "plan",
            Refuser::Person(Who::Owner),
            "take the other way"
        )]
    );
    // Nor does the loss of the plan end it: the research then runs for both.
    w.lose("plan");
    assert!(w.because().iter().any(|c| matches!(c, Cause::Refused { .. })));
    assert!(w.because().iter().any(|c| matches!(c, Cause::Gone { .. })));
    w.run("research", &[], &[("plan", Matters)]);
    assert_eq!(w.runs_next().as_deref(), Some("implement"));
}

#[test]
fn a_refusal_made_while_its_writer_runs_is_put_into_that_run_which_then_answers_it() {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        cell("implement", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    w.run("research", &[], &[("plan", Matters)]);
    w.run("implement", &[], &[]);
    w.person_writes("background");
    w.start("research");
    // The owner refuses the plan while the research runs: the walk says to put the refusal into its session.
    let refusal = w.owner_refuses("plan", "take the other way");
    assert_eq!(w.deliver(), vec![refusal]);
    // Once delivered it is not delivered again, and what the run then writes ends it.
    assert_eq!(w.deliver(), vec![]);
    w.finish("research", &[], &[("plan", Matters)]);
    assert_eq!(w.runs_next().as_deref(), Some("implement"));
}

#[test]
fn what_a_run_wrote_before_a_refusal_was_put_into_it_does_not_end_the_refusal() {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        cell("implement", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    w.run("research", &[], &[("plan", Matters)]);
    w.run("implement", &[], &[]);
    w.person_writes("background");
    w.start("research");
    let refusal = w.owner_refuses("plan", "take the other way");
    // The run writes the plan before the refusal reaches it, and nothing after.
    w.writes("research", "plan", Matters);
    assert_eq!(w.deliver(), vec![refusal]);
    w.finish("research", &[], &[]);
    assert_eq!(w.runs_next().as_deref(), Some("research"));
    assert!(
        w.because()
            .iter()
            .any(|c| matches!(c, Cause::Refused { refusal: r, .. } if *r == refusal))
    );
}

#[test]
fn a_cell_that_leaves_what_was_refused_as_it_was_is_run_again_up_to_its_limit_and_then_the_owner_decides() {
    let mut research = cell("research", &[], &["plan"]);
    research.limit = 2;
    let mut w = World::with_gate(vec![research, cell("implement", &["plan"], &[])]);
    w.run("research", &[], &[("plan", Matters)]);
    w.run("implement", &[], &[]);
    w.owner_refuses("plan", "take the other way");
    // It runs again twice and changes nothing: the refusal still stands, and at the limit the work waits.
    w.run("research", &[], &[]);
    w.run("research", &[], &[]);
    assert_eq!(waiting(&w), ("research".to_string(), vec![Waited::Limit { runs: 3 }]));
    // The owner lets it go on: it may run again as often once more, and this time writes the plan anew.
    w.let_go("research");
    w.run("research", &[], &[]);
    w.run("research", &[], &[("plan", Matters)]);
    assert_eq!(w.runs_next().as_deref(), Some("implement"));
}

#[test]
fn two_cells_that_answer_each_other_with_a_new_version_each_time_come_to_the_limit() {
    let mut fix = cell("fix", &["findings"], &["fixed"]);
    fix.limit = 2;
    let mut w = World::with_gate(vec![fix, cell("recheck", &["fixed"], &[])]);
    w.person_writes("findings");
    w.run("fix", &[], &[("fixed", Matters)]);
    w.run("recheck", &[], &[]);
    // Twice the re-check refuses, the fix writes anew, and the re-check runs again.
    for _ in 0..2 {
        w.refuse("recheck", "fixed", "still not fixed");
        w.run("fix", &[], &[("fixed", Matters)]);
        w.run("recheck", &[], &[]);
    }
    // The third refusal is against a new version, and still the fix has run as often as it may.
    w.refuse("recheck", "fixed", "still not fixed");
    assert_eq!(waiting(&w), ("fix".to_string(), vec![Waited::Limit { runs: 3 }]));
}

#[test]
fn the_limit_holds_when_a_cell_depends_on_what_a_later_cell_writes() {
    let mut implement = cell("implement", &["plan", "findings"], &["change"]);
    implement.limit = 2;
    let mut w = World::with_gate(vec![implement, cell("review", &["change"], &["findings"])]);
    w.person_writes("plan");
    w.run("implement", &[], &[("change", Matters)]);
    for _ in 0..2 {
        w.run("review", &[], &[("findings", Matters)]);
        w.run("implement", &[], &[("change", Matters)]);
    }
    w.run("review", &[], &[("findings", Matters)]);
    assert_eq!(waiting(&w), ("implement".to_string(), vec![Waited::Limit { runs: 3 }]));
    // A change before the cell is no way round: the plan changes, and the cell runs for it.
    w.person_writes("plan");
    assert_eq!(w.runs_next().as_deref(), Some("implement"));
}

#[test]
fn a_lost_product_its_cell_does_not_write_again_comes_to_the_limit() {
    let mut read = cell("read", &["background"], &["context"]);
    read.limit = 1;
    let mut w = World::with_gate(vec![read, cell("research", &["context"], &[])]);
    w.person_writes("background");
    w.run("read", &[], &[("context", Matters)]);
    w.lose("context");
    assert_eq!(
        w.because(),
        vec![Cause::Gone {
            product: "context".into()
        }]
    );
    w.run("read", &[], &[]);
    assert_eq!(waiting(&w), ("read".to_string(), vec![Waited::Limit { runs: 2 }]));
}

#[test]
fn a_persons_refusal_runs_the_earliest_writer_and_has_ended_for_every_writer_once_the_product_is_written_anew() {
    let cells = vec![
        cell("research", &[], &["question"]),
        cell("tree", &["question"], &["question"]),
        cell("implement", &["question"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.run("research", &[], &[("question", Matters)]);
    w.run("tree", &[], &[("question", Matters)]);
    w.run("implement", &[], &[]);
    assert!(w.rests());
    w.owner_refuses("question", "the other option");
    assert_eq!(w.runs_next().as_deref(), Some("research"));
    w.run("research", &[], &[("question", Matters)]);
    // The later writer runs because the question changed, not because it is still refused.
    assert_eq!(w.runs_next().as_deref(), Some("tree"));
    assert_eq!(w.because(), vec![changed("question", Some(2), Some(3))]);
    w.run("tree", &[], &[("question", Matters)]);
    assert_eq!(w.runs_next().as_deref(), Some("implement"));
}

#[test]
fn a_refusal_by_a_cell_off_the_path_or_against_what_a_cell_off_the_path_wrote_has_no_force() {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        when(cell("design", &["plan"], &["plan"]), "research", "design"),
        cell("implement", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    w.run("research", &[("way", "design")], &[("plan", Matters)]);
    w.run("design", &[], &[("plan", Matters)]);
    w.run("implement", &[], &[]);
    // The design refuses the research's plan as the design left it, and the owner refuses it too.
    w.refuse("design", "plan", "too weak");
    w.owner_refuses("plan", "another way");
    assert_eq!(w.runs_next().as_deref(), Some("research"));
    // The research now sends the work straight to implementing and leaves its plan: the design is off the path, its
    // refusal has no force, and the owner's was against the version the design wrote, which is no longer seen.
    w.run("research", &[("way", "implement")], &[]);
    assert_eq!(w.runs_next().as_deref(), Some("implement"));
    assert_eq!(w.because(), vec![changed("plan", Some(2), Some(1))]);
}

#[test]
fn what_a_cell_off_the_path_handed_in_is_absent_to_the_cells_after_it_and_a_run_is_built_on_what_is_seen() {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        when(cell("design", &[], &["sketch"]), "research", "design"),
        cell("implement", &["plan", "sketch"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    w.run("research", &[("way", "design")], &[("plan", Matters)]);
    w.run("design", &[], &[("sketch", Matters)]);
    w.run("implement", &[], &[]);
    assert!(w.rests());
    // The research runs again and no longer sends the work to design; its plan it says is as before.
    w.person_writes("background");
    w.run("research", &[("way", "implement")], &[("plan", DoesNotMatter)]);
    assert_eq!(w.runs_next().as_deref(), Some("implement"));
    assert_eq!(w.because(), vec![changed("sketch", Some(1), None)]);
    // The run is built on the sketch being absent, so once it has finished the work rests.
    w.run("implement", &[], &[]);
    assert!(w.rests());
}

#[test]
fn a_refusal_no_cell_on_the_path_can_answer_for_holds_the_work_before_the_first_cell_that_depends_on_it() {
    let mut w = World::with_gate(vec![cell("implement", &["background"], &[])]);
    w.person_writes("background");
    w.run("implement", &[], &[]);
    assert!(w.rests());
    let refusal = w.owner_refuses("background", "this is not the problem");
    let on = vec![Waited::Unanswered {
        refusal,
        product: "background".into(),
        by: Refuser::Person(Who::Owner),
    }];
    assert_eq!(waiting(&w), ("implement".to_string(), on));
    // A person writes it anew: the refusal has ended, and the cell runs for the change.
    w.person_writes("background");
    assert_eq!(w.runs_next().as_deref(), Some("implement"));
}

#[test]
fn a_lost_product_runs_its_cell_again_and_rebuilt_with_its_meaning_kept_runs_nothing_after_it() {
    let cells = vec![
        cell("read", &["background"], &["context"]),
        cell("research", &["context"], &["plan"]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    w.run("read", &[], &[("context", Matters)]);
    w.run("research", &[], &[("plan", Matters)]);
    assert!(w.rests());
    w.lose("context");
    assert_eq!(w.runs_next().as_deref(), Some("read"));
    assert_eq!(
        w.because(),
        vec![Cause::Gone {
            product: "context".into()
        }]
    );
    w.run("read", &[], &[("context", DoesNotMatter)]);
    assert!(w.rests());
}

#[test]
fn a_run_that_waits_for_a_person_holds_the_walk_there_and_is_running_again_once_answered() {
    let mut w = World::new(vec![
        cell("research", &[], &["plan"]),
        cell("implement", &["plan"], &[]),
    ]);
    w.run("research", &[], &[("plan", Matters)]);
    let run = w.start("implement");
    let call = w.wait_for("implement", "a ruling on the token", Who::Owner);
    let on = vec![Waited::Call {
        call,
        what: "a ruling on the token".to_string(),
        who: Who::Owner,
    }];
    assert_eq!(
        w.decide(),
        Decision::Wait {
            at: "implement".into(),
            on
        }
    );
    w.answer(call);
    assert_eq!(
        w.decide(),
        Decision::Busy {
            cell: "implement".into(),
            run,
            deliver: vec![]
        }
    );
}

/// A work whose implementation waits for the owner when a first comment arrives, with the cell that takes comments
/// before it.
fn commented() -> (World, decide::RunId) {
    let mut comments = cell("comments", &["comments"], &["notes"]);
    comments.applies = Some(Condition::Test("the item has a comment".into()));
    let mut w = World::new(vec![comments, cell("implement", &["plan", "notes"], &["change"])]);
    w.set("the item has a comment", waits("a comment", Who::Owner));
    w.person_writes("plan");
    let waiting = w.start("implement");
    w.wait_for("implement", "a ruling", Who::Owner);
    w.person_writes("comments");
    w.set("the item has a comment", Outcome::Holds);
    (w, waiting)
}

#[test]
fn a_cell_before_a_waiting_run_may_run_and_a_comment_that_changes_nothing_leaves_the_waiting_run() {
    let (mut w, _) = commented();
    assert_eq!(w.runs_next().as_deref(), Some("comments"));
    w.run("comments", &[], &[]);
    let (at, on) = waiting(&w);
    assert_eq!(at, "implement");
    assert!(matches!(on[..], [Waited::Call { .. }]));
}

#[test]
fn a_waiting_run_is_abandoned_once_what_it_depends_on_changed() {
    let (mut w, run) = commented();
    // The comment asks something of the implementation: what the waiting run was built on no longer stands.
    w.run("comments", &[], &[("notes", Matters)]);
    let because = Stale::Outdated {
        product: "notes".into(),
    };
    assert_eq!(
        w.decide(),
        Decision::Abandon {
            cell: "implement".into(),
            run,
            because
        }
    );
    w.abandon();
    assert_eq!(w.runs_next().as_deref(), Some("implement"));
    assert_eq!(w.because(), vec![Cause::NoRecord]);
}

#[test]
fn a_waiting_run_is_abandoned_once_its_cell_is_off_the_path() {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        when(cell("design", &["plan"], &[]), "research", "design"),
        cell("implement", &["plan"], &[]),
    ];
    let mut w = World::new(cells);
    w.person_writes("background");
    w.run("research", &[("way", "design")], &[("plan", Matters)]);
    let run = w.start("design");
    w.wait_for("design", "a ruling", Who::Owner);
    // The background changes while the design waits: the research runs meanwhile, and takes the other way.
    w.person_writes("background");
    w.run("research", &[("way", "implement")], &[("plan", DoesNotMatter)]);
    assert_eq!(
        w.decide(),
        Decision::Abandon {
            cell: "design".into(),
            run,
            because: Stale::NoLongerApplies
        }
    );
}

#[test]
fn a_cell_new_to_the_flow_runs_when_the_walk_reaches_it_and_the_runs_of_one_taken_out_count_for_nothing() {
    let mut w = World::with_gate(vec![
        cell("a", &[], &["x"]),
        cell("judge", &["x"], &[]),
        cell("c", &["x"], &[]),
    ]);
    w.run("a", &[], &[("x", Matters)]);
    w.run("judge", &[], &[]);
    w.run("c", &[], &[]);
    w.refuse("judge", "x", "not good enough");
    assert_eq!(w.runs_next().as_deref(), Some("a"));
    // The flow changes: the judge is taken out, and a cell is put in before the last.
    w.flow = Flow::new(vec![
        cell("a", &[], &["x"]),
        cell("b", &["x"], &["y"]),
        cell("c", &["x"], &[]),
        gate(),
    ])
    .unwrap();
    assert_eq!(w.runs_next().as_deref(), Some("b"));
    w.run("b", &[], &[("y", Matters)]);
    // The cell after it depends on nothing that changed.
    assert!(w.rests());
}
