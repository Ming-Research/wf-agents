//! Deliverable items: an item on the deliver flow as the repository defines it (`flow/deliver`), which has nothing to
//! do of its own and is made of the items it depends on. Its one cell waits until they have all merged, runs once,
//! and the work has then ended as merged, so that an item that depends on it may start in its turn. The driver knows
//! no flow by its name: the deliver flow is one more definition with a dependencies block. What the item is to deliver
//! is its background, which people write and no cell reads.

use std::path::Path;

use driver::{DEPENDENCIES, MERGED};
use flow::{Compiled, Has};

use super::*;

/// The engineer's flow as the repository defines it (`flow/engineer`), compiled against what the engine has for it.
fn engineer() -> Compiled {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/engineer");
    let source = flow::read(&dir).expect("the engineer's flow is read");
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
    let has = Has {
        sources: names(&[DEPENDENCIES, "main", "session", "tree"]),
        acts: names(&["start", "check", "merge"]),
        ..Default::default()
    };
    flow::compile(&source, &has).expect("the engineer's flow compiles")
}

/// The driver with the flows of the tests, the deliver flow and the engineer's.
fn harness() -> H {
    let mut flows = flows();
    flows.extend([deliver(), engineer()]);
    H::with(flows)
}

/// The deliverable waits at its one cell for the items it is made of to have merged.
fn waits_to_be_delivered() -> Decision {
    Decision::Wait {
        at: "V1".into(),
        on: vec![Waited::Precondition {
            test: "all_delivered".into(),
            by: vec![dependencies()],
        }],
    }
}

/// How many runs of the work started.
fn runs(h: &H, work: WorkNumber) -> usize {
    h.d().work(work).expect("the work is there").work.records.started.len()
}

#[test]
fn a_deliverable_waits_while_an_item_it_is_made_of_has_not_merged_and_is_delivered_once_all_have() {
    let mut h = harness();
    let (a, wa) = h.add(new_item("a", &[], "short"));
    let (b, wb) = h.add(new_item("b", &[], "plain"));
    let (_, wd) = h.add(new_item("d", &[a, b], "deliver"));
    assert_eq!(h.let_on(wd), vec![]);
    assert_eq!(h.decision(wd), waits_to_be_delivered());
    h.let_on(wa);
    h.let_on(wb);
    // One of the two merges: the deliverable still waits.
    let todos = h.through(wa);
    assert_eq!(todos, vec![]);
    assert_eq!(h.decision(wd), waits_to_be_delivered());
    assert_eq!(runs(&h, wd), 0);
    // The other merges: its cell runs in that very call, once, and the work has ended.
    let todos = h.through(wb);
    assert_eq!(rounds(&todos), [(wd, "V1".to_string(), 1)]);
    let todos = h.hands_in(wd, &[("delivered", None)]);
    assert_eq!(h.released(todos), vec![]);
    assert_eq!(h.decision(wd), Decision::Ended(End::Merged));
    assert_eq!(runs(&h, wd), 1);
    assert_eq!(h.on(wd, Event::Look), vec![]);
}

#[test]
fn a_deliverable_made_of_no_item_is_delivered_as_soon_as_it_is_let_on() {
    let mut h = harness();
    let (_, wd) = h.add(new_item("d", &[], "deliver"));
    // It waits for nothing but the owner, who added it paused.
    assert_eq!(h.dependencies(wd).as_deref(), Some(MERGED));
    assert_eq!(h.decision(wd), Decision::Paused);
    let todos = h.let_on(wd);
    assert_eq!(rounds(&todos), [(wd, "V1".to_string(), 1)]);
    h.hands_in(wd, &[("delivered", None)]);
    assert_eq!(h.decision(wd), Decision::Ended(End::Merged));
    assert_eq!(runs(&h, wd), 1);
}

#[test]
fn an_engineers_item_that_depends_on_a_deliverable_starts_only_once_the_deliverable_has_ended() {
    let mut h = harness();
    let (a, wa) = h.add(new_item("a", &[], "short"));
    let (d, wd) = h.add(new_item("d", &[a], "deliver"));
    let (_, we) = h.add(new_item("e", &[d], "engineer"));
    for work in [we, wd, wa] {
        h.let_on(work);
    }
    let waits_to_start = Decision::Wait {
        at: "S1".into(),
        on: vec![Waited::Precondition {
            test: "may_start".into(),
            // The engineer's item waits for the items it depends on to merge, not for the owner.
            by: vec![dependencies()],
        }],
    };
    assert_eq!(h.decision(we), waits_to_start);
    // The item the deliverable is made of merges: the deliverable runs, and the engineer's item still waits.
    let todos = h.through(wa);
    assert_eq!(rounds(&todos), [(wd, "V1".to_string(), 1)]);
    assert_eq!(h.decision(we), waits_to_start);
    // The deliverable ends: the engineer's item starts in that very call.
    let todos = h.hands_in(wd, &[("delivered", None)]);
    assert_eq!(h.decision(wd), Decision::Ended(End::Merged));
    assert_eq!(rounds(&h.released(todos)), [(we, "S1".to_string(), 1)]);
}

#[test]
fn a_deliverable_made_of_an_item_that_was_cancelled_waits_for_good() {
    let mut h = harness();
    let (a, wa) = h.add(new_item("a", &[], "short"));
    let (_, wd) = h.add(new_item("d", &[a], "deliver"));
    h.let_on(wd);
    h.on(wa, Event::Cancelled { why: None });
    assert_eq!(h.decision(wd), waits_to_be_delivered());
    assert_eq!(runs(&h, wd), 0);
}

#[test]
fn a_deliverable_added_with_its_background_keeps_it_and_is_delivered_as_one_without() {
    let mut h = harness();
    let (a, wa) = h.add(new_item("a", &[], "short"));
    let mut new = new_item("d", &[a], "deliver");
    new.writes = vec![first_write("background", None)];
    let (_, wd) = h.add(new);
    h.let_on(wd);
    assert_eq!(h.decision(wd), waits_to_be_delivered());
    // An edit of what it is to deliver runs nothing: no cell reads it.
    let todos = h.on(wd, wrote("background", Outsider::Person, None));
    assert_eq!(todos, vec![]);
    assert_eq!(h.decision(wd), waits_to_be_delivered());
    h.let_on(wa);
    let todos = h.through(wa);
    assert_eq!(rounds(&todos), [(wd, "V1".to_string(), 1)]);
    h.hands_in(wd, &[("delivered", None)]);
    assert_eq!(h.decision(wd), Decision::Ended(End::Merged));
    assert_eq!(runs(&h, wd), 1);
    let kept = h.d().work(wd).expect("the work is there");
    let background: Vec<&str> = (kept.work.records.written.iter())
        .filter(|w| w.block == block("background"))
        .map(|w| kept.work.contents[&(w.block.clone(), w.at)].as_str())
        .collect();
    assert_eq!(
        background,
        ["background, as first written", "background, as written by Person"]
    );
}
