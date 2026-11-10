//! What the view shows agrees with what the walk decides on the same records.

use super::*;

/// The work after a row of changes, taken at each step.
fn steps() -> Vec<World> {
    let mut out = Vec::new();
    let mut w = planned();
    out.push(planned());
    w.person_writes("background");
    w.run("research", &[("plan", Matters)]);
    w.start("implement");
    w.returns("implement");
    w.person_writes("background");
    let mut snapshot = planned();
    snapshot.records = w.records.clone();
    snapshot.named = support::Named(w.named.0.clone());
    out.push(snapshot);
    out.push(w);
    out
}

#[test]
fn the_views_decision_is_what_the_walk_decides() {
    for w in steps() {
        assert_eq!(seen(&w).decision, w.decide());
    }
}

#[test]
fn what_a_cell_needs_is_the_reasons_the_walk_gives_when_it_runs_that_cell() {
    let mut w = planned();
    let background = w.person_writes("background");
    w.run("research", &[("plan", Matters)]);
    w.run("implement", &[("code", Matters)]);
    w.person_writes("background");
    let Decision::Run { cell, because, .. } = w.decide() else {
        panic!("the walk runs nothing: {:?}", w.decide())
    };
    assert_eq!(cell.as_str(), "research");
    assert_eq!(cell_of(&w, "research").needs, because);
    assert_eq!(
        because,
        vec![Cause::Changed {
            block: "background".into(),
            was: Some(background),
            behind: false,
        }]
    );
}
