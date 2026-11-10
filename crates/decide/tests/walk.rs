//! The walk, case by case: each test names one thing the walk must do and shows it on a small flow.

mod support;

use decide::{Cause, Cell, Condition, Decision, End, Outcome, Refuser, Stale, Waited, Who};
use support::{World, Write::*, cell};

fn run(cell: &str, because: Vec<Cause>) -> Decision {
    Decision::Run {
        cell: cell.into(),
        because,
    }
}

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

fn refused(product: &str, by: Refuser, why: &str) -> Cause {
    Cause::Refused {
        product: product.into(),
        by,
        why: why.to_string(),
    }
}

/// A cell that applies when the research answered this way.
fn on_way(mut cell: Cell, way: &str) -> Cell {
    cell.applies = Some(Condition::Answered {
        by: "research".into(),
        output: "way".into(),
        is: way.to_string(),
    });
    cell
}

#[test]
fn a_work_walks_its_cells_in_order_and_has_ended_once_its_last_cell_has_a_record() {
    let mut w = World::new(vec![
        cell("a", &[], &["x"]),
        cell("b", &["x"], &["y"]),
        cell("c", &["y"], &[]),
    ]);
    assert_eq!(w.decide(), run("a", vec![Cause::NoRecord]));
    w.run("a", &[], &[("x", Matters)]);
    assert_eq!(w.decide(), run("b", vec![Cause::NoRecord]));
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
    w.start("a");
    assert_eq!(w.decide(), Decision::Busy { cell: "a".into() });
    w.records.paused = true;
    assert_eq!(w.decide(), Decision::Paused);
    w.records.cancelled = true;
    assert_eq!(w.decide(), Decision::Ended(End::Cancelled));
}

#[test]
fn a_cell_that_does_not_apply_is_skipped_until_the_answer_before_it_changes() {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        on_way(cell("design", &[], &["plan"]), "design"),
        cell("implement", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    w.run("research", &[("way", "implement")], &[("plan", Matters)]);
    // The design is skipped: the research answered that the work is implemented next.
    assert_eq!(w.decide(), run("implement", vec![Cause::NoRecord]));
    w.run("implement", &[], &[]);
    assert!(w.rests());
    // The background changes; the research runs again and now sends the work to design.
    w.person_writes("background");
    w.run("research", &[("way", "design")], &[("plan", Matters)]);
    assert_eq!(w.decide(), run("design", vec![Cause::NoRecord]));
    w.run("design", &[], &[("plan", Matters)]);
    assert_eq!(w.runs_next().as_deref(), Some("implement"));
}

#[test]
fn the_work_waits_before_a_cell_whose_precondition_does_not_hold_and_says_for_whom() {
    let mut implement = cell("implement", &["plan"], &[]);
    implement.requires = vec!["stop questions confirmed".into()];
    let mut w = World::new(vec![cell("research", &[], &["plan"]), implement]);
    w.set("stop questions confirmed", waits("the confirmation of q1", Who::Owner));
    w.run("research", &[], &[("plan", Matters)]);
    let on = vec![Waited {
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
    assert_eq!(w.decide(), run("implement", vec![Cause::NoRecord]));
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
    assert_eq!(w.decide(), run("c", vec![changed("x", Some(1), Some(2))]));
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
    let by = Refuser::Run(w.records.record(&"recheck".into()).unwrap().id);
    w.refuse("recheck", "fixed", "the second finding is not fixed");
    assert_eq!(
        w.decide(),
        run("fix", vec![refused("fixed", by, "the second finding is not fixed")])
    );
    w.run("fix", &[], &[("fixed", Matters)]);
    // A run that was given the refusal wrote the product anew: the refusal has ended, and the re-check judges again.
    assert_eq!(w.decide(), run("recheck", vec![changed("fixed", Some(1), Some(2))]));
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
    assert_eq!(
        w.decide(),
        run(
            "recheck",
            vec![Cause::Answered {
                product: "fixed".into()
            }]
        )
    );
}

#[test]
fn a_refusal_is_not_ended_by_a_run_that_was_not_given_it() {
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
    w.owner_refuses("plan", "take the other way");
    w.finish("research", &[], &[("plan", Matters)]);
    // The run was not given the refusal, so what it wrote does not end it: the research runs again, now given it.
    let because = vec![refused("plan", Refuser::Person(Who::Owner), "take the other way")];
    assert_eq!(w.decide(), run("research", because));
    w.run("research", &[], &[("plan", Matters)]);
    assert_eq!(w.runs_next().as_deref(), Some("implement"));
}

#[test]
fn a_cell_that_leaves_what_was_refused_as_it_was_is_run_again_up_to_its_limit_and_then_the_owner_decides() {
    let mut research = cell("research", &[], &["plan"]);
    research.limit = 2;
    let mut w = World::with_gate(vec![research, cell("implement", &["plan"], &[])]);
    w.run("research", &[], &[("plan", Matters)]);
    w.run("implement", &[], &[]);
    w.owner_refuses("plan", "take the other way");
    let because = || vec![refused("plan", Refuser::Person(Who::Owner), "take the other way")];
    assert_eq!(w.decide(), run("research", because()));
    // It runs again twice and changes nothing: the refusal still stands, and at the limit the work waits.
    w.run("research", &[], &[]);
    assert_eq!(w.decide(), run("research", because()));
    w.run("research", &[], &[]);
    let Decision::Wait { at, on } = w.decide() else {
        panic!("the work should wait: {:?}", w.decide())
    };
    assert_eq!(at.as_str(), "research");
    assert_eq!(on.len(), 1);
    assert_eq!(on[0].who, Who::Owner);
    // The owner lets it go on: it is run again, and this time writes the plan anew, which ends the refusal.
    w.let_go("research");
    assert_eq!(w.decide(), run("research", because()));
    w.run("research", &[], &[("plan", Matters)]);
    assert_eq!(w.runs_next().as_deref(), Some("implement"));
}

#[test]
fn refusals_by_one_cell_against_each_new_version_count_towards_the_same_limit() {
    let mut fix = cell("fix", &["findings"], &["fixed"]);
    fix.limit = 2;
    let mut w = World::with_gate(vec![fix, cell("recheck", &["fixed"], &[])]);
    w.person_writes("findings");
    w.run("fix", &[], &[("fixed", Matters)]);
    w.run("recheck", &[], &[]);
    // Twice the re-check refuses, the fix writes anew, and the re-check runs again.
    for _ in 0..2 {
        w.refuse("recheck", "fixed", "still not fixed");
        assert_eq!(w.runs_next().as_deref(), Some("fix"));
        w.run("fix", &[], &[("fixed", Matters)]);
        w.run("recheck", &[], &[]);
    }
    // The third refusal is against a new version, and still the fix has been run again as often as it may.
    w.refuse("recheck", "fixed", "still not fixed");
    let Decision::Wait { at, on } = w.decide() else {
        panic!("the work should wait: {:?}", w.decide())
    };
    assert_eq!(at.as_str(), "fix");
    assert_eq!(on[0].who, Who::Owner);
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
    // What the cell wrote itself does not make it need to run.
    w.run("tree", &[], &[("question", Matters)]);
    w.run("implement", &[], &[]);
    assert!(w.rests());
    w.owner_refuses("question", "the other option");
    assert_eq!(w.runs_next().as_deref(), Some("research"));
    w.run("research", &[], &[("question", Matters)]);
    // The later writer runs because the question changed, not because it is still refused.
    assert_eq!(w.decide(), run("tree", vec![changed("question", Some(1), Some(3))]));
    w.run("tree", &[], &[("question", Matters)]);
    assert_eq!(w.runs_next().as_deref(), Some("implement"));
}

#[test]
fn a_refusal_by_a_cell_that_is_no_longer_on_the_path_has_no_force() {
    let cells = vec![
        cell("research", &[], &["plan"]),
        on_way(cell("design review", &["plan"], &[]), "design"),
        cell("implement", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.run("research", &[("way", "design")], &[("plan", Matters)]);
    w.run("design review", &[], &[]);
    w.refuse("design review", "plan", "too weak");
    assert_eq!(w.runs_next().as_deref(), Some("research"));
    // The research now sends the work straight to implementing and leaves the plan: the review is off the path.
    w.run("research", &[("way", "implement")], &[]);
    assert_eq!(w.decide(), run("implement", vec![Cause::NoRecord]));
}

#[test]
fn what_a_cell_no_longer_on_the_path_handed_in_is_absent_to_the_cells_after_it() {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        on_way(cell("design", &[], &["sketch"]), "design"),
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
    assert_eq!(w.decide(), run("implement", vec![changed("sketch", Some(1), None)]));
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
    assert_eq!(
        w.decide(),
        run(
            "read",
            vec![Cause::Gone {
                product: "context".into()
            }]
        )
    );
    w.run("read", &[], &[("context", DoesNotMatter)]);
    assert!(w.rests());
}

#[test]
fn a_run_that_waits_for_a_person_holds_the_walk_there_and_goes_on_once_answered() {
    let mut w = World::new(vec![
        cell("research", &[], &["plan"]),
        cell("implement", &["plan"], &[]),
    ]);
    w.run("research", &[], &[("plan", Matters)]);
    let waiting = w.start("implement");
    w.wait_for("implement", "a ruling on the token", Who::Owner);
    let on = vec![Waited {
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
    w.answer("implement");
    assert_eq!(
        w.decide(),
        Decision::GoOn {
            cell: "implement".into(),
            run: waiting
        }
    );
    w.obey();
    assert_eq!(
        w.decide(),
        Decision::Busy {
            cell: "implement".into()
        }
    );
}

#[test]
fn a_cell_before_a_waiting_run_may_run_and_the_waiting_run_is_abandoned_only_once_what_it_depends_on_changed() {
    let mut comments = cell("comments", &["comments"], &["notes"]);
    comments.applies = Some(Condition::Test("the work has a comment".into()));
    let mut w = World::new(vec![comments, cell("implement", &["plan", "notes"], &["change"])]);
    w.set("the work has a comment", waits("a comment", Who::Owner));
    w.person_writes("plan");
    let waiting = w.start("implement");
    w.wait_for("implement", "a ruling", Who::Owner);
    // A comment arrives while the implementation waits for the owner: the cell before it runs meanwhile, and the
    // waiting run stays.
    w.person_writes("comments");
    w.set("the work has a comment", Outcome::Holds);
    assert_eq!(w.decide(), run("comments", vec![Cause::NoRecord]));
    w.run("comments", &[], &[("notes", Matters)]);
    // The comment asked something of the implementation: what the waiting run was built on no longer stands.
    let because = Stale::Outdated {
        product: "notes".into(),
    };
    assert_eq!(
        w.decide(),
        Decision::Abandon {
            cell: "implement".into(),
            run: waiting,
            because
        }
    );
    w.obey();
    assert_eq!(w.decide(), run("implement", vec![Cause::NoRecord]));
}

#[test]
fn a_cell_new_to_the_flow_runs_when_the_walk_reaches_it_and_the_records_of_one_taken_out_count_for_nothing() {
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
    let merge = w.flow.cells.pop().unwrap();
    w.flow.cells = vec![
        cell("a", &[], &["x"]),
        cell("b", &["x"], &["y"]),
        cell("c", &["x"], &[]),
        merge,
    ];
    assert_eq!(w.decide(), run("b", vec![Cause::NoRecord]));
    w.run("b", &[], &[("y", Matters)]);
    // The cell after it depends on nothing that changed.
    assert!(w.rests());
}
