//! Changing an item after it was added: its title and its level until its work ends, and what it depends on while its
//! work has begun no run, each change kept as the item's own record, after which the work's dependencies block says
//! what it now waits for. A change that would close a circle of dependencies, or that its work has gone too far for,
//! is refused and changes nothing.

use driver::{MERGED, Unchangeable, WAITING};
use store::{Change, ItemChange};

use super::*;

/// The changes kept of the item, as the driver holds it.
fn changes(h: &H, work: WorkNumber) -> Vec<Change> {
    h.d().work(work).expect("the work is there").item.changes.clone()
}

#[test]
fn an_item_not_begun_comes_to_wait_for_an_item_and_goes_on_once_that_one_no_longer_holds_it() {
    let mut h = H::new();
    let (a, _) = h.add(new_item("a", &[], "plain"));
    let (b, wb) = h.add(new_item("b", &[], "short"));
    h.let_on(wb);
    // c, added to wait for b, comes to wait for a as well.
    let (c, wc) = h.add(new_item("c", &[b], "short"));
    assert_eq!(h.change(c, ItemChange::DependsOn(a)), (true, vec![]));
    assert_eq!(h.dependencies(wc).as_deref(), Some(WAITING));
    assert_eq!(h.d().work(wc).unwrap().item.depends_on, [a, b]);
    h.let_on(wc);
    assert_eq!(h.decision(wc), waits_for_dependencies("draft"));
    // Taken away again, c waits only for b; and once b merges it begins.
    let (changed, todos) = h.change(c, ItemChange::NoLongerDependsOn(a));
    assert!(changed);
    assert_eq!(todos, vec![]);
    assert_eq!(h.d().work(wc).unwrap().item.depends_on, [b]);
    let todos = h.through(wb);
    assert_eq!(rounds(&todos), [(wc, "draft".to_string(), 1)]);
    let made: Vec<ItemChange> = changes(&h, wc).into_iter().map(|c| c.what).collect();
    assert_eq!(made, [ItemChange::DependsOn(a), ItemChange::NoLongerDependsOn(a)]);
}

#[test]
fn a_dependency_taken_away_lets_a_waiting_work_begin_in_that_very_call() {
    let mut h = H::new();
    let (a, _) = h.add(new_item("a", &[], "plain"));
    let (b, wb) = h.add(new_item("b", &[a], "short"));
    h.let_on(wb);
    assert_eq!(h.decision(wb), waits_for_dependencies("draft"));
    let (changed, todos) = h.change(b, ItemChange::NoLongerDependsOn(a));
    assert!(changed);
    assert_eq!(rounds(&todos), [(wb, "draft".to_string(), 1)]);
    assert_eq!(h.dependencies(wb).as_deref(), Some(MERGED));
}

#[test]
fn an_item_may_come_to_depend_on_one_added_after_it_but_never_in_a_circle() {
    let mut h = H::new();
    let (a, _) = h.add(new_item("a", &[], "short"));
    let (b, _) = h.add(new_item("b", &[a], "short"));
    let (c, wc) = h.add(new_item("c", &[b], "short"));
    let (d, _) = h.add(new_item("d", &[], "short"));
    assert!(h.change(a, ItemChange::DependsOn(d)).0);
    let circle = |h: &mut H, item: ItemNumber, on: ItemNumber| {
        h.refused(|driver, now| driver.change(now, item, ItemChange::DependsOn(on)).map(drop))
    };
    assert_eq!(
        circle(&mut h, a, c),
        Failed::Unchangeable {
            item: a,
            why: Unchangeable::Circle(vec![c, b, a]),
        }
    );
    assert_eq!(
        circle(&mut h, d, c),
        Failed::Unchangeable {
            item: d,
            why: Unchangeable::Circle(vec![c, b, a, d]),
        }
    );
    assert_eq!(
        circle(&mut h, c, c),
        Failed::Unchangeable {
            item: c,
            why: Unchangeable::Circle(vec![c]),
        }
    );
    assert_eq!(circle(&mut h, c, ItemNumber(9)), Failed::NoSuchItem(ItemNumber(9)));
    // What is changed is what the driver holds when it opens again.
    h.reopen();
    assert_eq!(h.d().work(wc).unwrap().item.depends_on, [b]);
    let first = h.d().works().next().expect("a is there").item.clone();
    assert_eq!((first.number, first.depends_on.clone()), (a, vec![d]));
    assert_eq!(
        first.first.depends_on,
        Vec::<ItemNumber>::new(),
        "it was added depending on nothing"
    );
}

#[test]
fn a_begun_work_keeps_what_it_depends_on_and_takes_another_title_and_level() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "short"));
    let (b, _) = h.add(new_item("b", &[], "short"));
    h.let_on(wa);
    h.round(wa);
    for change in [ItemChange::DependsOn(b), ItemChange::NoLongerDependsOn(b)] {
        let why = h.refused(|driver, now| driver.change(now, a, change).map(drop));
        assert_eq!(
            why,
            Failed::Unchangeable {
                item: a,
                why: Unchangeable::Begun,
            }
        );
    }
    assert_eq!(
        h.change(a, ItemChange::Title("what is left of a".into())),
        (true, vec![])
    );
    assert_eq!(
        h.change(a, ItemChange::Title("what is left of a".into())),
        (false, vec![])
    );
    assert_eq!(h.change(a, ItemChange::Level("3".into())), (true, vec![]));
    let item = h.d().work(wa).unwrap().item.clone();
    assert_eq!(
        (item.title.as_str(), item.level.as_deref()),
        ("what is left of a", Some("3"))
    );
    assert_eq!(item.first.title, "a");
    assert_eq!(item.changes.len(), 2, "a change that changed nothing was kept");
}

#[test]
fn an_item_whose_work_ended_takes_no_change() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "plain"));
    let (b, wb) = h.add(new_item("b", &[], "plain"));
    h.let_on(wa);
    h.through(wa);
    h.on(
        wb,
        Event::Cancelled {
            why: Some("joined into item 1".into()),
        },
    );
    let kept = h.d().work(wb).unwrap();
    assert_eq!(kept.work.why_cancelled.as_deref(), Some("joined into item 1"));
    for (item, end) in [(a, End::Merged), (b, End::Cancelled)] {
        for change in [ItemChange::Title("other".into()), ItemChange::Level("1".into())] {
            let why = h.refused(|driver, now| driver.change(now, item, change).map(drop));
            assert_eq!(
                why,
                Failed::Unchangeable {
                    item,
                    why: Unchangeable::Ended(end),
                }
            );
        }
    }
    assert_eq!(
        h.refused(|driver, now| driver
            .change(now, ItemNumber(7), ItemChange::Title("x".into()))
            .map(drop)),
        Failed::NoSuchItem(ItemNumber(7))
    );
}
