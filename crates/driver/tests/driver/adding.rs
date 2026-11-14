//! Adding an item with its work: numbers, each refusal changing nothing, the work paused from its first entry with
//! its dependencies block and what people wrote for it first, and an add that a stop cuts short leaving all of it or
//! none.

use driver::{MERGED, NewItem, WAITING};
use store::ItemChange;

use super::*;

#[test]
fn items_and_their_works_are_numbered_from_one_in_the_order_they_are_added() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "judged"));
    let (b, wb) = h.add(new_item("b", &[a], "short"));
    let (c, wc) = h.add(new_item("c", &[b, a], "plain"));
    assert_eq!((a, b, c), (ItemNumber(1), ItemNumber(2), ItemNumber(3)));
    assert_eq!((wa, wb, wc), (WorkNumber(1), WorkNumber(2), WorkNumber(3)));
    let shown: Vec<(WorkNumber, ItemNumber, &str, &str, Vec<ItemNumber>)> = (h.d().works())
        .map(|k| {
            let item = k.item;
            (
                k.head.number,
                k.head.item,
                k.head.flow.as_str(),
                item.title.as_str(),
                item.depends_on.clone(),
            )
        })
        .collect();
    assert_eq!(
        shown,
        [
            (wa, a, "judged", "a", vec![]),
            (wb, b, "short", "b", vec![a]),
            (wc, c, "plain", "c", vec![a, b]),
        ]
    );
    let kept = h.d().work(wb).expect("the work is there");
    assert_eq!(kept.flow.name, "short");
    assert!(h.d().work(WorkNumber(4)).is_none());
    assert!(h.d().view(WorkNumber(4)).is_none());
}

#[test]
fn an_item_keeps_the_level_it_is_added_at_when_the_driver_opens_again() {
    let mut h = H::new();
    let (_, easy) = h.add(new_item("a", &[], "judged"));
    let (_, hard) = h.add(NewItem {
        level: "3".to_string(),
        ..new_item("b", &[], "judged")
    });
    let level = |h: &H, work| h.d().work(work).expect("the work is there").item.level.clone();
    assert_eq!(level(&h, easy).as_deref(), Some("1"));
    assert_eq!(level(&h, hard).as_deref(), Some("3"));
    h.reopen();
    assert_eq!(level(&h, hard).as_deref(), Some("3"));
}

#[test]
fn an_item_put_at_another_level_keeps_it_when_the_driver_opens_again() {
    let mut h = H::new();
    let (item, work) = h.add(new_item("a", &[], "judged"));
    let level = |item: ItemNumber| vec![(item, driver::Asked::Item(ItemChange::Level("2".into())))];
    assert_eq!(h.changes(level(item)).map(|(changed, _)| changed), Ok(vec![true]));
    assert_eq!(
        h.changes(level(item)).map(|(changed, _)| changed),
        Ok(vec![false]),
        "it is at that level already"
    );
    assert_eq!(
        h.changes(level(ItemNumber(9))).map(|(changed, _)| changed),
        Err(Failed::NotChanged {
            asked: 1,
            refused: vec![driver::NotMade {
                at: 0,
                item: ItemNumber(9),
                why: driver::changes::Refused::NoSuchItem(ItemNumber(9)),
            }],
        })
    );
    let level = |h: &H| h.d().work(work).expect("the work is there").item.level.clone();
    assert_eq!(level(&h).as_deref(), Some("2"));
    h.reopen();
    assert_eq!(level(&h).as_deref(), Some("2"));
}

#[test]
fn an_item_depending_on_one_the_driver_does_not_have_is_refused_and_nothing_changes() {
    let mut h = H::new();
    let (a, _) = h.add(new_item("a", &[], "judged"));
    let why = h.refused(|d, now| d.add(now, new_item("b", &[a, ItemNumber(7)], "judged")).map(drop));
    assert_eq!(why, Failed::NoSuchItem(ItemNumber(7)));
    assert_eq!(h.add(new_item("b", &[a], "judged")), (ItemNumber(2), WorkNumber(2)));
}

#[test]
fn an_item_whose_title_has_no_words_is_refused_and_nothing_changes() {
    let mut h = H::new();
    for title in ["", "  "] {
        let why = h.refused(|d, now| d.add(now, new_item(title, &[], "judged")).map(drop));
        assert_eq!(why, Failed::BlankTitle);
    }
}

#[test]
fn an_item_on_a_flow_the_driver_was_not_given_is_refused_and_nothing_changes() {
    let mut h = H::new();
    let why = h.refused(|d, now| d.add(now, new_item("a", &[], "nowhere")).map(drop));
    assert_eq!(why, Failed::NoSuchFlow("nowhere".into()));
    assert_eq!(h.add(new_item("a", &[], "short")), (ItemNumber(1), WorkNumber(1)));
}

#[test]
fn a_first_write_the_flow_does_not_allow_is_refused_and_nothing_changes() {
    let mut h = H::new();
    h.add(new_item("a", &[], "judged"));
    let cases: Vec<(Vec<driver::FirstWrite>, engine::Refused)> = vec![
        (
            vec![first_write("request", None), first_write("nowhere", None)],
            engine::Refused::NoSuchBlock(block("nowhere")),
        ),
        (
            vec![first_write("text", None)],
            engine::Refused::NotTheirs {
                block: block("text"),
                by: Outsider::Person,
            },
        ),
        (
            vec![first_write("verdict", Some("maybe"))],
            engine::Refused::NotAValue {
                block: block("verdict"),
                value: "maybe".into(),
            },
        ),
        (
            vec![first_write("verdict", None)],
            engine::Refused::NoValue {
                block: block("verdict"),
            },
        ),
        (
            vec![first_write("request", Some("yes"))],
            engine::Refused::NotAChoice {
                block: block("request"),
            },
        ),
        // The block the driver writes is not people's to write.
        (
            vec![first_write("deps", Some("merged"))],
            engine::Refused::NotTheirs {
                block: block("deps"),
                by: Outsider::Person,
            },
        ),
    ];
    for (writes, refused) in cases {
        let new = NewItem {
            writes,
            ..new_item("b", &[], "judged")
        };
        let why = h.refused(|d, now| d.add(now, new).map(drop));
        assert_eq!(why, Failed::Refused(refused));
    }
    assert_eq!(h.add(new_item("b", &[], "judged")), (ItemNumber(2), WorkNumber(2)));
}

#[test]
fn the_work_is_paused_from_its_first_entry_and_holds_its_dependencies_block_and_the_first_writes() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "judged"));
    let new = NewItem {
        writes: vec![first_write("request", None), first_write("verdict", Some("no"))],
        ..new_item("b", &[a], "judged")
    };
    let (_, wb) = h.add(new);
    // Nothing is to be done for a work added paused.
    assert_eq!(h.d().pending(), vec![]);
    for (work, deps) in [(wa, MERGED), (wb, WAITING)] {
        let kept = h.d().work(work).expect("the work is there");
        let r = &kept.work.records;
        assert!(r.started.is_empty() && r.rounds.is_empty(), "work {} began", work.0);
        let first = (r.pauses.first()).expect("the work has a pause");
        assert!(first.paused, "the first pause is a letting on");
        // The pause is the first entry, the dependencies block the next.
        assert!(r.pauses.iter().all(|p| p.paused), "work {} was let on", work.0);
        let earliest = (r.written.iter().map(|w| w.at))
            .chain(r.pauses.iter().map(|p| p.at))
            .min();
        assert_eq!(
            earliest,
            Some(first.at),
            "the pause is not the first entry of work {}",
            work.0
        );
        let deps_write = (r.written.first()).expect("a write");
        assert_eq!(deps_write.block, block("deps"));
        assert_eq!(deps_write.by, Outsider::Outside);
        assert_eq!(deps_write.value.as_deref(), Some(deps));
        assert_eq!(h.dependencies(work).as_deref(), Some(deps));
        // With nothing written that runs its first cell, nothing is to be done, and its own pause holds it; with the
        // request written, it waits before its first cell for its dependencies.
        if deps == MERGED {
            assert!(
                matches!(h.decision(work), Decision::Stalled { .. }),
                "{:?}",
                h.decision(work)
            );
        } else {
            assert_eq!(h.decision(work), waits_for_dependencies("write"));
        }
    }
    // What people wrote first follows, in order.
    let kept = h.d().work(wb).expect("the work is there");
    let people: Vec<(&str, Option<&str>)> = (kept.work.records.written.iter())
        .filter(|w| w.by == Outsider::Person)
        .map(|w| (w.block.as_str(), w.value.as_deref()))
        .collect();
    assert_eq!(people, [("request", None), ("verdict", Some("no"))]);
    let request = (kept.work.records.written.iter())
        .find(|w| w.block == block("request"))
        .expect("written")
        .at;
    let content = kept.work.contents.get(&(block("request"), request));
    assert_eq!(content.map(String::as_str), Some("request, as first written"));
}

#[test]
fn a_work_added_takes_what_people_write_and_begins_only_once_let_on() {
    let mut h = H::new();
    let (_, w) = h.add(new_item("a", &[], "judged"));
    assert_eq!(h.on(w, wrote("request", Outsider::Person, None)), vec![]);
    let request = last_written(&h, w);
    assert_eq!(h.on(w, wrote("opinions", Outsider::Person, None)), vec![]);
    let opinions = last_written(&h, w);
    assert_eq!(h.decision(w), Decision::Paused);
    let todos = h.let_on(w);
    let (run, ..) = h.round(w);
    assert_eq!(
        todos,
        vec![Todo {
            work: w,
            effect: Effect::Round {
                run,
                cell: "write".into(),
                number: 1,
                given: Given::Task,
            },
        }]
    );
    // Its first run starts from what people wrote while it was paused.
    let kept = h.d().work(w).expect("the work is there");
    let started = (kept.work.records.started.iter())
        .find(|s| s.run == run)
        .expect("the run started");
    assert_eq!(started.basis.get(&block("request")), Some(&Some(request)));
    assert_eq!(started.basis.get(&block("opinions")), Some(&Some(opinions)));
}

/// What the store holds of the items and works, as a store opened anew reads it.
fn held(db: &Db) -> (Vec<store::Item>, Vec<store::WorkHead>) {
    let store = db.open();
    (
        store::Storage::items(&store).expect("the items are read"),
        store::Storage::works(&store).expect("the works are read"),
    )
}

#[test]
fn an_add_cut_short_at_any_call_of_the_store_leaves_all_of_it_or_none_and_the_work_never_runs_unpaused() {
    // How many calls of the store an add makes.
    let mut h = H::new();
    let (a, _) = h.add(new_item("a", &[], "short"));
    h.arm(Plan::Never);
    let new = NewItem {
        writes: vec![first_write("request", None)],
        ..new_item("b", &[a], "judged")
    };
    h.add(new.clone());
    let n = h.ops().len();
    assert!(n >= 1);
    let mut wholes = 0;
    let mut nones = 0;
    for at in 1..=n {
        for die in [false, true] {
            let mut h = H::new();
            h.add(new_item("a", &[], "short"));
            let before = held(&h.db);
            let plan = if die {
                Plan::DieAfter { n: at }
            } else {
                Plan::Fail {
                    op: None,
                    n: at,
                    on: false,
                }
            };
            h.arm(plan);
            let now = h.now();
            let outcome = h.try_add(now, new.clone());
            assert!(h.failed() || h.dead(), "the store was never cut short at the call {at}");
            if let Outcome::Stopped { kept } = outcome {
                assert!(!kept);
            }
            let todos = h.reopen();
            assert_eq!(todos, vec![], "a work added paused has something to do");
            let after = held(&h.db);
            if after == before {
                nones += 1;
                continue;
            }
            wholes += 1;
            assert_eq!(
                after.0.len(),
                2,
                "the item is there without its work, or the reverse: {after:?}"
            );
            assert_eq!(
                after.1.len(),
                2,
                "the item is there without its work, or the reverse: {after:?}"
            );
            let w = WorkNumber(2);
            assert_eq!(h.decision(w), waits_for_dependencies("write"));
            assert_eq!(h.dependencies(w).as_deref(), Some(WAITING));
            assert!(decide::paused(&h.d().work(w).expect("the work is there").work.records));
            let kept = h.d().work(w).expect("the work is there");
            let people = (kept.work.records.written.iter())
                .filter(|x| x.by == Outsider::Person)
                .count();
            assert_eq!(people, 1, "the first write is not kept with the work");
            assert!(kept.work.records.started.is_empty(), "the work ran though never let on");
        }
    }
    assert!(
        nones > 0 && wholes > 0,
        "the cuts left none {nones} times and all {wholes} times"
    );
}
