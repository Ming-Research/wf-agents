//! An item that continues a branch or a pull request: the driver keeps what it continues with the item, refuses a name
//! no branch has and a flow that has nothing to read it, and lets it change until the work's branch is made, which is
//! when the cell that starts a work has handed in a return, and not when it has begun a run.

use driver::changes::Refused;
use driver::{Asked, NewItem, NotMade, Unchangeable};
use store::{Continues, ItemChange};

use super::*;

fn branch(name: &str) -> Option<Continues> {
    Some(Continues::Branch(name.to_string()))
}

/// The item's `continues` as the driver holds it.
fn continues(h: &H, work: WorkNumber) -> Option<Continues> {
    h.d().work(work).expect("the work is there").item.continues.clone()
}

fn continuing(title: &str, flow: &str, what: Option<Continues>) -> NewItem {
    NewItem {
        continues: what,
        ..new_item(title, &[], flow)
    }
}

#[test]
fn an_item_added_continuing_a_branch_or_a_pull_request_keeps_it_through_every_opening_of_the_driver() {
    let mut h = H::with(vec![placing()]);
    let (_, one) = h.add(continuing("a", "placing", branch("work-226-0a1b2c3d")));
    let (_, two) = h.add(continuing("b", "placing", Some(Continues::Pull(252))));
    let (_, none) = h.add(continuing("c", "placing", None));
    for _ in 0..2 {
        assert_eq!(continues(&h, one), branch("work-226-0a1b2c3d"));
        assert_eq!(continues(&h, two), Some(Continues::Pull(252)));
        assert_eq!(continues(&h, none), None);
        let kept = h.d().work(one).expect("the work is there").item.first.continues.clone();
        assert_eq!(kept, branch("work-226-0a1b2c3d"));
        h.reopen();
    }
}

#[test]
fn an_item_is_not_added_continuing_what_no_branch_is_called_or_on_a_flow_that_starts_no_work() {
    let mut h = H::with(vec![placing(), plain()]);
    for (name, bad) in [
        ("", true),
        ("-x", true),
        ("a b", true),
        ("a..b", true),
        ("ok/name", false),
    ] {
        let new = continuing("a", "placing", branch(name));
        if bad {
            let why = h.refused(|d, now| d.add(now, new).map(drop));
            assert_eq!(
                why,
                Failed::BadContinues(Continues::Branch(name.to_string())),
                "{name:?}"
            );
        } else {
            h.add(new);
        }
    }
    let why = h.refused(|d, now| {
        d.add(now, continuing("b", "placing", Some(Continues::Pull(0))))
            .map(drop)
    });
    assert_eq!(why, Failed::BadContinues(Continues::Pull(0)));
    let why = h.refused(|d, now| d.add(now, continuing("c", "plain", branch("old"))).map(drop));
    assert_eq!(why, Failed::NoStart("plain".to_string()));
    // Nothing was kept of what was refused: the one item that was added is the only one.
    assert_eq!(h.d().works().count(), 1);
    // A flow that starts no work takes an item that continues nothing.
    h.add(continuing("d", "plain", None));
}

#[test]
fn what_an_item_continues_changes_with_its_why_and_back_to_nothing_until_its_branch_is_made() {
    let mut h = H::with(vec![placing()]);
    let (a, work) = h.add(continuing("a", "placing", branch("old")));
    assert!(h.change(a, ItemChange::Continues(branch("older"))).0);
    assert!(
        !h.change(a, ItemChange::Continues(branch("older"))).0,
        "it is that already"
    );
    assert!(h.change(a, ItemChange::Continues(Some(Continues::Pull(7)))).0);
    assert!(h.change(a, ItemChange::Continues(None)).0);
    assert_eq!(continues(&h, work), None);
    let kept = h.d().work(work).expect("the work is there").item.changes.clone();
    let whats: Vec<ItemChange> = kept.iter().map(|change| change.what.clone()).collect();
    assert_eq!(
        whats,
        [
            ItemChange::Continues(branch("older")),
            ItemChange::Continues(Some(Continues::Pull(7))),
            ItemChange::Continues(None),
        ]
    );
    assert!(
        kept.iter()
            .all(|change| change.why.as_deref() == Some("the planner says why"))
    );
    h.reopen();
    assert_eq!(continues(&h, work), None);
}

#[test]
fn a_start_that_began_a_run_and_handed_nothing_in_leaves_what_the_item_continues_open_to_change_and_a_start_that_did_not()
 {
    let mut h = H::with(vec![placing()]);
    let (a, work) = h.add(continuing("a", "placing", branch("old")));
    h.let_on(work);
    assert!(
        !h.d()
            .work(work)
            .expect("the work is there")
            .work
            .records
            .started
            .is_empty(),
        "the start act began a run"
    );
    // The act was refused, as a branch that is not there is, and the owner corrects the item.
    assert!(h.change(a, ItemChange::Continues(branch("right"))).0);
    assert_eq!(continues(&h, work), branch("right"));
    // Its branch made, what it continues is what it was made from.
    h.hands_in(work, &[("place", None)]);
    for change in [ItemChange::Continues(branch("other")), ItemChange::Continues(None)] {
        let why = h.refused(|d, now| d.change(now, Some("why not"), vec![(a, Asked::Item(change))]).map(drop));
        assert_eq!(
            why,
            Failed::NotChanged {
                asked: 1,
                refused: vec![NotMade {
                    at: 0,
                    item: a,
                    why: Refused::Unchangeable(Unchangeable::Placed),
                }],
            }
        );
    }
    assert_eq!(continues(&h, work), branch("right"));
    // Its title still changes, as a started work's does.
    assert!(h.change(a, ItemChange::Title("renamed".to_string())).0);
}

#[test]
fn an_item_whose_flow_starts_no_work_comes_to_continue_nothing_and_is_refused_continuing_anything() {
    let mut h = H::with(vec![placing(), plain()]);
    let (a, _) = h.add(continuing("a", "plain", None));
    let why = h.refused(|d, now| {
        d.change(
            now,
            Some("why not"),
            vec![(a, Asked::Item(ItemChange::Continues(branch("old"))))],
        )
        .map(drop)
    });
    assert_eq!(
        why,
        Failed::NotChanged {
            asked: 1,
            refused: vec![NotMade {
                at: 0,
                item: a,
                why: Refused::NoStart,
            }],
        }
    );
    assert!(!h.change(a, ItemChange::Continues(None)).0);
    let why = h.refused(|d, now| {
        d.change(
            now,
            Some("why not"),
            vec![(a, Asked::Item(ItemChange::Continues(branch("-x"))))],
        )
        .map(drop)
    });
    assert!(matches!(
        why,
        Failed::NotChanged { refused, .. } if refused[0].why == Refused::BadContinues(Continues::Branch("-x".to_string()))
    ));
}
