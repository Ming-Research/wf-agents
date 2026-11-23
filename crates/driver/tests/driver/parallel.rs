//! The list of a project's works under way, which the loop has the carrier of the project's repository make and writes
//! as what the source `parallel` says: which lists the driver gives to be made, and when.

use flow::{Compiled, Has, Source, compile};

use driver::NewItem;
use driver::parallel::Ask;

use super::*;

/// A flow whose plan names, in `plan_files`, the files it will touch, and which has the block of the list of works
/// under way and a block of the work's head.
fn planned() -> Compiled {
    let flow = r#"
        name = "planned"
        order = ["plan", "merge"]
        [limits]
        runs = 3
        returns = 2
        calls = 3
        [lines.main]
        [blocks.request]
        people = true
        [blocks.head]
        outside = "head"
        [blocks.plan_files]
        shown = true
        [blocks.parallel]
        outside = "parallel"
        planned_in = "plan_files"
        [blocks.parallel.writes]
        work = "W{work} (#{item})"
        changed = "changed:"
        planned = "planned:"
        none_yet = "nothing yet"
        not_stated = "not stated"
        unread = "could not be read: {why}"
        [blocks.seen]
        [blocks.merged]
        shown = true
    "#;
    let plan = r#"
        title = "Plan"
        kind = "agent"
        effort = "high"
        line = "main"
        task = "Plan it."
        watches = ["request"]
        reads = ["head", "parallel"]
        outputs = ["plan_files", "seen"]

        [kinds]
        request = "last_and_latest"
        head = "latest"
        parallel = "latest"
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        act = "merge"
        watches = ["seen"]
        outputs = ["merged"]
    "#;
    let source = Source {
        flow: flow.to_string(),
        cells: [("plan", plan), ("merge", merge)]
            .iter()
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect(),
        ..Default::default()
    };
    let has = Has {
        sources: ["dependencies", "head", "parallel"].map(str::to_string).into(),
        acts: ["merge".to_string()].into(),
        ..Default::default()
    };
    compile(&source, &has).expect("the flow compiles")
}

/// An item of the flow `planned` with its request written, of this project.
fn planned_item(title: &str, project: &str) -> NewItem {
    NewItem {
        project: project.to_string(),
        writes: vec![first_write("request", None)],
        ..new_item(title, &[], "planned")
    }
}

/// What a list is made of, by its work: its number, its head and the files its plan names.
fn listed(ask: &Ask) -> Vec<(u64, Option<String>, Option<String>)> {
    (ask.works.iter())
        .map(|w| (w.work.0, w.head.clone(), w.planned.clone()))
        .collect()
}

fn wrote_head(h: &mut H, work: WorkNumber, head: &str) {
    let event = Event::Wrote {
        block: "head".into(),
        by: Outsider::Outside,
        content: head.to_string(),
        matters: true,
        value: None,
    };
    h.on(work, event);
}

// A work is on its project's list once it has started a run, and the list is given to be made again only where it
// differs: a new head, the files its plan names, a work that ends or is cancelled. Fails where a work not yet started
// is listed, where a work that has ended or been cancelled stays on it, where a change that alters nothing of the list
// asks again, or where a change of one of these is not asked for. The harness checks after each call what must always
// hold. Milliseconds.
#[test]
fn a_projects_list_is_given_to_be_made_where_the_works_under_way_their_heads_or_their_plans_differ() {
    let mut h = H::with(vec![planned()]);
    let (_, a) = h.add(planned_item("a", "one"));
    assert_eq!(h.dm().parallel(), vec![], "the work has started no run");
    h.let_on(a);
    let asks = h.dm().parallel();
    assert_eq!(asks.len(), 1, "{asks:?}");
    assert_eq!(
        (asks[0].project.as_str(), listed(&asks[0])),
        ("one", vec![(a.0, None, None)])
    );
    assert_eq!(h.dm().parallel(), vec![], "the same list is asked for once");

    wrote_head(&mut h, a, "c1");
    let asks = h.dm().parallel();
    assert_eq!(listed(&asks[0]), vec![(a.0, Some("c1".to_string()), None)]);
    assert_eq!(h.dm().parallel(), vec![]);

    // The plan's writer names the files it will touch, as its round ends.
    h.hands_in(a, &[("plan_files", None), ("seen", None)]);
    let asks = h.dm().parallel();
    assert_eq!(
        listed(&asks[0]),
        vec![(a.0, Some("c1".to_string()), Some("the plan_files".to_string()))]
    );

    // A second work joins the list, in the order of the works' numbers; the first that ends leaves it.
    let (_, b) = h.add(planned_item("b", "one"));
    h.let_on(b);
    let asks = h.dm().parallel();
    assert_eq!(asks.len(), 1, "{asks:?}");
    assert_eq!(listed(&asks[0]).iter().map(|w| w.0).collect::<Vec<_>>(), [a.0, b.0]);
    h.through(a);
    let asks = h.dm().parallel();
    assert_eq!(listed(&asks[0]).iter().map(|w| w.0).collect::<Vec<_>>(), [b.0]);

    // A work that is cancelled leaves it as well.
    let (_, c) = h.add(planned_item("c", "one"));
    h.let_on(c);
    let asks = h.dm().parallel();
    assert_eq!(listed(&asks[0]).iter().map(|w| w.0).collect::<Vec<_>>(), [b.0, c.0]);
    h.on(c, Event::Cancelled { why: None });
    let asks = h.dm().parallel();
    assert_eq!(listed(&asks[0]).iter().map(|w| w.0).collect::<Vec<_>>(), [b.0]);
}

// Each project has the list of its own works, and a work of a flow with no block of the list is on none: a deliverable
// item has neither a branch nor a plan. Fails where one project's works are listed under another, or where a work of a
// flow with no such block is listed. Milliseconds.
#[test]
fn each_project_has_its_own_list_and_a_work_of_a_flow_without_the_block_is_on_none() {
    let mut h = H::with(vec![planned(), short()]);
    let (_, a) = h.add_let_on(planned_item("a", "one"));
    let (_, b) = h.add_let_on(planned_item("b", "two"));
    h.add_let_on(NewItem {
        project: "one".to_string(),
        ..new_item("c", &[], "short")
    });
    let mut asks = h.dm().parallel();
    asks.sort_by(|x, y| x.project.cmp(&y.project));
    let lists: Vec<(&str, Vec<u64>)> = (asks.iter())
        .map(|ask| (ask.project.as_str(), listed(ask).iter().map(|w| w.0).collect()))
        .collect();
    assert_eq!(lists, [("one", vec![a.0]), ("two", vec![b.0])]);
}

// What was last given is kept in memory only: after a stop the first call gives every project's list again, which is
// written where it differs from what the works hold. Fails where a restarted driver gives none, or only some of the
// projects', and the lists are not kept current. Milliseconds.
#[test]
fn after_a_restart_every_list_is_given_again() {
    let mut h = H::with(vec![planned()]);
    let (_, a) = h.add_let_on(planned_item("a", "one"));
    let (_, b) = h.add_let_on(planned_item("b", "two"));
    assert_eq!(h.dm().parallel().len(), 2);
    assert_eq!(h.dm().parallel(), vec![]);
    h.restart();
    let mut asks = h.dm().parallel();
    asks.sort_by(|x, y| x.project.cmp(&y.project));
    let lists: Vec<(&str, WorkNumber)> = asks
        .iter()
        .map(|ask| (ask.project.as_str(), ask.works[0].work))
        .collect();
    assert_eq!(lists, [("one", a), ("two", b)]);
    assert_eq!(h.dm().parallel(), vec![]);
}
