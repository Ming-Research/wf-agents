//! Items and works: their numbers, each refusal the driver names, which changes nothing in the driver or the store, a
//! work added paused, and an item having another work once its work has ended.

use super::*;

#[test]
fn items_and_works_are_numbered_from_one_in_the_order_they_are_added() {
    let mut h = H::new();
    let a = h.item("a", &[]);
    let b = h.item("b", &[a]);
    let c = h.item("c", &[b, a]);
    assert_eq!((a, b, c), (ItemNumber(1), ItemNumber(2), ItemNumber(3)));
    let items: Vec<(ItemNumber, &str, Vec<ItemNumber>)> = (h.d().items())
        .map(|item| (item.number, item.title.as_str(), item.depends_on.clone()))
        .collect();
    assert_eq!(items, [(a, "a", vec![]), (b, "b", vec![a]), (c, "c", vec![a, b])]);
    // An item has no work until one is added.
    assert_eq!(h.d().works().count(), 0);
    let (first, todos) = h.work(a, "judged", false);
    assert_eq!(first, WorkNumber(1));
    assert_eq!(rounds(&todos), [(first, "write".to_string(), 1)]);
    let (second, todos) = h.work(c, "short", false);
    assert_eq!(second, WorkNumber(2));
    // Its item depends on items that have not merged: it waits.
    assert_eq!(todos, vec![]);
    let heads: Vec<(WorkNumber, ItemNumber, &str)> = (h.d().works())
        .map(|kept| (kept.head.number, kept.head.item, kept.head.flow.as_str()))
        .collect();
    assert_eq!(heads, [(first, a, "judged"), (second, c, "short")]);
    let kept = h.d().work(second).expect("the work is there");
    assert_eq!((kept.item.number, kept.flow.name.as_str()), (c, "short"));
    assert!(h.d().work(WorkNumber(3)).is_none());
    assert!(h.d().view(WorkNumber(3)).is_none());
}

#[test]
fn an_item_depending_on_one_the_driver_does_not_have_is_refused_and_nothing_changes() {
    let mut h = H::new();
    let a = h.item("a", &[]);
    let (work, _) = h.work(a, "judged", false);
    let why = h.refused(|d, now| d.add_item(now, "b", &[a, ItemNumber(7)]).map(drop));
    assert_eq!(why, Failed::NoSuchItem(ItemNumber(7)));
    // The next item is the second, and the work goes on as it was.
    assert_eq!(h.item("b", &[a]), ItemNumber(2));
    assert_eq!(
        rounds(&h.hands_in(work, &[("text", None)])),
        [(work, "judge".to_string(), 1)]
    );
}

#[test]
fn a_work_for_an_item_the_driver_does_not_have_is_refused_and_nothing_changes() {
    let mut h = H::new();
    let a = h.item("a", &[]);
    h.work(a, "judged", false);
    let why = h.refused(|d, now| d.add_work(now, ItemNumber(2), "judged", false).map(drop));
    assert_eq!(why, Failed::NoSuchItem(ItemNumber(2)));
    let b = h.item("b", &[]);
    assert_eq!(h.work(b, "short", false).0, WorkNumber(2));
}

#[test]
fn a_work_on_a_flow_the_driver_was_not_given_is_refused_and_nothing_changes() {
    let mut h = H::new();
    let a = h.item("a", &[]);
    let why = h.refused(|d, now| d.add_work(now, a, "nowhere", false).map(drop));
    assert_eq!(why, Failed::NoSuchFlow("nowhere".into()));
    assert_eq!(h.work(a, "short", false).0, WorkNumber(1));
}

#[test]
fn a_second_work_for_an_item_whose_work_goes_on_is_refused_and_nothing_changes() {
    let mut h = H::new();
    let a = h.item("a", &[]);
    let b = h.item("b", &[a]);
    let (wa, _) = h.work(a, "judged", false);
    let why = h.refused(|d, now| d.add_work(now, a, "short", false).map(drop));
    assert_eq!(why, Failed::WorkGoesOn { item: a, work: wa });
    // A work that waits for its dependencies goes on too, and so does one that is paused.
    let (wb, _) = h.work(b, "judged", false);
    let why = h.refused(|d, now| d.add_work(now, b, "judged", true).map(drop));
    assert_eq!(why, Failed::WorkGoesOn { item: b, work: wb });
    h.on(wa, Event::Paused);
    let why = h.refused(|d, now| d.add_work(now, a, "judged", false).map(drop));
    assert_eq!(why, Failed::WorkGoesOn { item: a, work: wa });
}

#[test]
fn a_work_added_paused_begins_no_round_takes_what_people_write_and_begins_once_let_on() {
    let mut h = H::new();
    let a = h.item("a", &[]);
    let (work, todos) = h.work(a, "judged", true);
    assert_eq!(todos, vec![]);
    assert_eq!(h.decision(work), Decision::Paused);
    let todos = h.on(work, wrote("request", Outsider::Person, None));
    assert_eq!(todos, vec![]);
    let request = last_written(&h, work);
    let todos = h.on(work, wrote("opinions", Outsider::Person, None));
    assert_eq!(todos, vec![]);
    let opinions = last_written(&h, work);
    let todos = h.on(work, Event::Resumed);
    let (run, ..) = h.round(work);
    assert_eq!(
        todos,
        vec![Todo {
            work,
            effect: Effect::Round {
                run,
                cell: "write".into(),
                number: 1,
                given: Given::Task,
            },
        }]
    );
    // Its first run starts from what people wrote while it was paused.
    let kept = h.d().work(work).expect("the work is there");
    let started = (kept.work.records.started.iter())
        .find(|s| s.run == run)
        .expect("the run started");
    assert_eq!(started.basis.get(&BlockId::from("request")), Some(&Some(request)));
    assert_eq!(started.basis.get(&BlockId::from("opinions")), Some(&Some(opinions)));
}

#[test]
fn once_its_work_has_merged_an_item_may_have_another() {
    let mut h = H::new();
    let a = h.item("a", &[]);
    let (first, _) = h.work(a, "judged", false);
    h.through(first);
    let (second, todos) = h.work(a, "short", false);
    assert_eq!(second, WorkNumber(2));
    assert_eq!(rounds(&todos), [(second, "draft".to_string(), 1)]);
    // The second goes on: a third is refused.
    let why = h.refused(|d, now| d.add_work(now, a, "short", false).map(drop));
    assert_eq!(why, Failed::WorkGoesOn { item: a, work: second });
}

#[test]
fn once_its_work_was_cancelled_an_item_may_have_another() {
    let mut h = H::new();
    let a = h.item("a", &[]);
    let (first, _) = h.work(a, "judged", false);
    let (run, ..) = h.round(first);
    let todos = h.on(first, Event::Cancelled);
    assert_eq!(
        todos,
        vec![Todo {
            work: first,
            effect: Effect::Stop { run },
        }]
    );
    assert_eq!(h.decision(first), Decision::Ended(decide::End::Cancelled));
    let (second, todos) = h.work(a, "judged", false);
    assert_eq!(rounds(&todos), [(second, "write".to_string(), 1)]);
    // The round of the cancelled work is still to be stopped, and its end gives nothing more.
    let spent = h.spent();
    let came = Came::Asks(vec![]);
    let todos = h.on(first, Event::Returned { run, came, spent });
    assert_eq!(todos, vec![]);
}
