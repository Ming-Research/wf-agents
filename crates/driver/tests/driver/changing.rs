//! Changing items after they were added, several at once and all or none, each change saying why: an item's title,
//! level and tags until its work ends, and what it depends on while its work has started no run, each kept as the
//! item's own record, after which the work's dependencies block says what it now waits for; its cancel, refused while
//! an item that has not ended depends on it; and the reopening of a cancelled item, whose work is paused, and once let
//! on makes its place again. A change that would close a circle of dependencies, that its work has gone too far for, or
//! that would leave an item waiting for good, is refused and changes nothing, nor do the changes asked with it.

use driver::changes::Refused;
use driver::{Asked, MERGED, NotMade, Unchangeable, WAITING};
use store::{Change, ItemChange};

use super::*;

/// The changes kept of the item, as the driver holds it.
fn changes(h: &H, work: WorkNumber) -> Vec<Change> {
    h.d().work(work).expect("the work is there").item.changes.clone()
}

/// Why changes asked together, for a reason, are refused; the driver and the store are left as they were.
fn refused(h: &mut H, asked: Vec<(ItemNumber, Asked)>) -> Failed {
    h.refused(|driver, now| driver.change(now, Some("why not"), asked).map(drop))
}

/// The refusal of one change asked alone.
fn not_made(item: ItemNumber, why: Refused) -> Failed {
    Failed::NotChanged {
        asked: 1,
        refused: vec![NotMade { at: 0, item, why }],
    }
}

fn unchangeable(item: ItemNumber, why: Unchangeable) -> Failed {
    not_made(item, Refused::Unchangeable(why))
}

fn item(change: ItemChange) -> Asked {
    Asked::Item(change)
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
    // Each change keeps the reason it was made for.
    assert!(
        (changes(&h, wc).iter()).all(|c| c.why.as_deref() == Some("the planner says why")),
        "a change was kept without its reason"
    );
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
    let circle =
        |h: &mut H, item: ItemNumber, on: ItemNumber| refused(h, vec![(item, Asked::Item(ItemChange::DependsOn(on)))]);
    assert_eq!(
        circle(&mut h, a, c),
        unchangeable(a, Unchangeable::Circle(vec![c, b, a]))
    );
    assert_eq!(
        circle(&mut h, d, c),
        unchangeable(d, Unchangeable::Circle(vec![c, b, a, d]))
    );
    assert_eq!(circle(&mut h, c, c), unchangeable(c, Unchangeable::Circle(vec![c])));
    assert_eq!(
        circle(&mut h, c, ItemNumber(9)),
        not_made(c, Refused::NoSuchItem(ItemNumber(9)))
    );
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
fn a_started_work_keeps_what_it_depends_on_and_takes_another_title_level_and_tags() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "short"));
    let (b, _) = h.add(new_item("b", &[], "short"));
    h.let_on(wa);
    h.round(wa);
    for change in [ItemChange::DependsOn(b), ItemChange::NoLongerDependsOn(b)] {
        assert_eq!(
            refused(&mut h, vec![(a, item(change))]),
            unchangeable(a, Unchangeable::Started)
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
    assert_eq!(h.change(a, ItemChange::Tagged("parser".into())), (true, vec![]));
    assert_eq!(h.change(a, ItemChange::Tagged("parser".into())), (false, vec![]));
    let item = h.d().work(wa).unwrap().item.clone();
    assert_eq!(
        (item.title.as_str(), item.level.as_deref(), item.tags.as_slice()),
        ("what is left of a", Some("3"), ["parser".to_string()].as_slice())
    );
    assert_eq!(item.first.title, "a");
    assert_eq!(item.changes.len(), 3, "a change that changed nothing was kept");
}

#[test]
fn an_item_whose_work_ended_takes_no_change_until_it_is_reopened() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "plain"));
    let (b, wb) = h.add(new_item("b", &[], "plain"));
    h.let_on(wa);
    h.through(wa);
    let (changed, todos) = (h.changes(vec![(b, Asked::Cancel)])).expect("b is cancelled");
    assert_eq!(changed, [true]);
    h.released(todos);
    let kept = h.d().work(wb).unwrap();
    assert_eq!(
        kept.work.why_cancelled.values().collect::<Vec<_>>(),
        ["the planner says why"]
    );
    for (item, end) in [(a, End::Merged), (b, End::Cancelled)] {
        for change in [ItemChange::Title("other".into()), ItemChange::Level("1".into())] {
            assert_eq!(
                refused(&mut h, vec![(item, Asked::Item(change))]),
                unchangeable(item, Unchangeable::Ended(end))
            );
        }
        assert_eq!(
            refused(&mut h, vec![(item, Asked::Cancel)]),
            unchangeable(item, Unchangeable::Ended(end))
        );
    }
    assert_eq!(
        refused(&mut h, vec![(a, Asked::Reopen)]),
        unchangeable(a, Unchangeable::NotCancelled)
    );
    assert_eq!(
        refused(&mut h, vec![(ItemNumber(7), item(ItemChange::Title("x".into())))]),
        not_made(ItemNumber(7), Refused::NoSuchItem(ItemNumber(7)))
    );
    // Reopened, and retitled in the same breath.
    let asked = vec![(b, Asked::Reopen), (b, item(ItemChange::Title("b, again".into())))];
    assert_eq!(h.changes(asked).map(|(changed, _)| changed), Ok(vec![true, true]));
    assert_eq!(h.d().work(wb).unwrap().item.title, "b, again");
}

#[test]
fn an_item_is_not_cancelled_while_an_item_that_has_not_ended_depends_on_it() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "short"));
    let (b, wb) = h.add(new_item("b", &[a], "short"));
    let (c, _) = h.add(new_item("c", &[b], "short"));
    assert_eq!(
        refused(&mut h, vec![(a, Asked::Cancel)]),
        unchangeable(a, Unchangeable::OpenDependants(vec![b]))
    );
    // Cancelled together, those that wait first, all of them are.
    let asked = vec![(c, Asked::Cancel), (b, Asked::Cancel), (a, Asked::Cancel)];
    assert_eq!(h.changes(asked).map(|(changed, _)| changed), Ok(vec![true, true, true]));
    for work in [wa, wb] {
        assert_eq!(h.decision(work), Decision::Ended(End::Cancelled));
    }
    // A new item may not wait for a cancelled one: it would wait for good.
    let refused = h.refused(|driver, now| driver.add(now, new_item("d", &[a], "short")).map(drop));
    assert_eq!(refused, Failed::OnCancelled(vec![a]));
}

#[test]
fn changes_asked_together_are_made_all_or_none_and_every_refusal_is_said() {
    let mut h = H::new();
    let (a, _) = h.add(new_item("a", &[], "short"));
    let (b, _) = h.add(new_item("b", &[a], "short"));
    let (c, wc) = h.add(new_item("c", &[], "short"));
    let asked = vec![
        (c, item(ItemChange::Title("c, narrowed".into()))),
        (a, Asked::Cancel),
        (c, item(ItemChange::Tagged("two words".into()))),
        (c, item(ItemChange::DependsOn(b))),
    ];
    assert_eq!(
        refused(&mut h, asked),
        Failed::NotChanged {
            asked: 4,
            refused: vec![
                NotMade {
                    at: 1,
                    item: a,
                    why: Refused::Unchangeable(Unchangeable::OpenDependants(vec![b])),
                },
                NotMade {
                    at: 2,
                    item: c,
                    why: Refused::BadTag("two words".into()),
                },
            ],
        }
    );
    assert_eq!(h.d().work(wc).unwrap().item.title, "c");
    assert_eq!(h.dependencies(wc).as_deref(), Some(MERGED));
}

#[test]
fn a_cancelled_item_is_reopened_paused_and_let_on_makes_its_place_again() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "plain"));
    let starts = |h: &H| {
        let kept = h.d().work(wa).expect("the work is there");
        let block = (kept.flow.blocks.iter()).find(|(_, b)| b.writers.outside.as_deref() == Some(driver::START));
        let block = block.map(|(name, _)| name.clone()).expect("the flow has a start block");
        (kept.work.records.written.iter()).filter(|w| w.block == block).count()
    };
    h.let_on(wa);
    assert_eq!(starts(&h), 1);
    let (run, ..) = h.round(wa);
    let (_, todos) = h.changes(vec![(a, Asked::Cancel)]).expect("a is cancelled");
    assert_eq!(
        todos,
        vec![Todo {
            work: wa,
            effect: Effect::Stop { run }
        }]
    );
    // While what its cancel let go is still being let go, it is not reopened.
    let spent = h.spent();
    let todos = h.on(
        wa,
        Event::Returned {
            run,
            came: Came::default(),
            spent,
        },
    );
    assert!(
        (todos.iter()).any(|t| matches!(t.effect, Effect::Release { .. })),
        "the cancelled work lets go of what it held"
    );
    assert_eq!(
        refused(&mut h, vec![(a, Asked::Reopen)]),
        unchangeable(a, Unchangeable::LettingGo)
    );
    h.released(todos);
    // Reopened, it is paused, as an item added is, and nothing of it begins.
    let (changed, todos) = h.changes(vec![(a, Asked::Reopen)]).expect("a is reopened");
    assert_eq!((changed, todos), (vec![true], vec![]));
    assert_eq!(h.decision(wa), Decision::Paused);
    let kept = h.d().work(wa).expect("the work is there");
    assert_eq!(kept.work.records.reopens.len(), 1);
    assert_eq!(
        kept.work.why_reopened.values().collect::<Vec<_>>(),
        ["the planner says why"]
    );
    assert_eq!(starts(&h), 1, "the start is not written before the work is let on");
    // Let on, the start is written again, and the first cell runs again, in the place made again.
    let todos = h.let_on(wa);
    assert_eq!(starts(&h), 2);
    assert_eq!(rounds(&todos), [(wa, "write".to_string(), 1)]);
    // Cancelled again, it lets go again.
    let (run, ..) = h.round(wa);
    h.changes(vec![(a, Asked::Cancel)]).expect("a is cancelled again");
    let spent = h.spent();
    let todos = h.on(
        wa,
        Event::Returned {
            run,
            came: Came::default(),
            spent,
        },
    );
    assert!((todos.iter()).any(|t| matches!(t.effect, Effect::Release { .. })));
    h.released(todos);
    assert_eq!(h.d().work(wa).unwrap().work.records.releases.len(), 2);
}

#[test]
fn an_item_reopened_is_waited_for_again_by_those_that_depend_on_it() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "short"));
    let (_, wb) = h.add(new_item("b", &[a], "short"));
    // As a store kept before an item could not be cancelled under one that depends on it: a is cancelled anyway.
    let todos = h.on(wa, Event::Cancelled { why: None });
    h.released(todos);
    let text = |h: &H| {
        let kept = h.d().work(wb).expect("the work is there");
        dependencies_text(kept.flow, kept.work).unwrap_or_default()
    };
    assert!(text(&h).contains("(cancelled)"), "{}", text(&h));
    h.changes(vec![(a, Asked::Reopen)]).expect("a is reopened");
    assert_eq!(h.dependencies(wb).as_deref(), Some(WAITING));
    assert!(!text(&h).contains("(cancelled)"), "{}", text(&h));
}

#[test]
fn an_item_is_added_with_tags_and_a_tag_that_is_no_name_is_refused() {
    let mut h = H::new();
    let mut tagged = new_item("a", &[], "short");
    tagged.tags = vec!["page".into(), "engine".into(), "page".into()];
    let (_, wa) = h.add(tagged);
    assert_eq!(h.d().work(wa).unwrap().item.tags, ["engine", "page"]);
    let mut bad = new_item("b", &[], "short");
    bad.tags = vec!["a,b".into()];
    let refused = h.refused(|driver, now| driver.add(now, bad).map(drop));
    assert_eq!(refused, Failed::BadTag("a,b".into()));
}
