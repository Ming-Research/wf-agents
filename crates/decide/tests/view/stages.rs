//! A work at each stage: each cell's record, its open run, and whether a round of it is under way.

use super::*;

#[test]
fn a_work_where_nothing_ran_shows_every_cell_with_no_record_and_needing_to_run_and_no_version() {
    let w = planned();
    let v = seen(&w);
    assert_eq!(v.decision, w.decide());
    let cells: Vec<CellView> = vec![
        CellView {
            cell: "research".into(),
            applies: true,
            record: None,
            open: None,
            needs: vec![Cause::NoRecord],
            inputs: vec![input("background", None, None, false)],
        },
        CellView {
            cell: "implement".into(),
            applies: true,
            record: None,
            open: None,
            needs: vec![Cause::NoRecord],
            inputs: vec![input("plan", None, None, false)],
        },
        CellView {
            cell: "merge".into(),
            applies: true,
            record: None,
            open: None,
            needs: vec![Cause::NoRecord],
            inputs: vec![],
        },
    ];
    assert_eq!(v.cells, cells);
    assert_eq!(v.blocks, Default::default());
}

#[test]
fn a_cell_whose_round_is_under_way_shows_its_open_run_under_way() {
    let mut w = planned();
    w.person_writes("background");
    let run = w.start("research");
    assert_eq!(cell_of(&w, "research").open, Some(OpenRun { run, under_way: true }));
    assert_eq!(seen(&w).decision, w.decide());
}

#[test]
fn a_cell_whose_run_rests_at_a_call_shows_its_open_run_at_rest() {
    let mut w = planned();
    w.person_writes("background");
    let run = w.start("research");
    let _: CallId = w.calls("research", Some(Who::Owner));
    assert_eq!(cell_of(&w, "research").open, Some(OpenRun { run, under_way: false }));
    assert_eq!(seen(&w).decision, w.decide());
}

#[test]
fn a_cell_that_finished_shows_its_record_no_open_run_and_no_need() {
    let mut w = planned();
    let background = w.person_writes("background");
    let at = w.run("research", &[("plan", Matters)]);
    let run = finished_by(&w, at);
    let research = cell_of(&w, "research");
    assert_eq!(
        (research.record, research.open, research.needs),
        (Some(run), None, vec![])
    );
    assert_eq!(
        research.inputs,
        vec![input("background", Some(background), Some(background), false)]
    );
}

#[test]
fn a_skipped_cell_that_ran_before_does_not_apply_and_keeps_its_record() {
    let mut w = World::with_gate(vec![
        cell("ask", &["topic"], &["more"]),
        when("more", "yes", cell("extra", &[], &[])),
    ]);
    w.person_writes("topic");
    w.run("ask", &[("more", Value("yes"))]);
    let at = w.run("extra", &[]);
    let extra = finished_by(&w, at);
    w.person_writes("topic");
    w.run("ask", &[("more", Value("no"))]);
    let view = cell_of(&w, "extra");
    assert_eq!((view.applies, view.record), (false, Some(extra)));
}
