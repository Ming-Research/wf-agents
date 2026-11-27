use std::collections::BTreeMap;

use store::First;
use work::Time;

use super::*;

fn n(numbers: &[u64]) -> Vec<ItemNumber> {
    numbers.iter().copied().map(ItemNumber).collect()
}

/// How far an item's work has gone, as far as a change of the item is judged: its last cell has no run open, one is
/// open, or it has ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gone {
    Going,
    Merging,
    Ended(End),
}

fn planned(number: u64, on: &[u64], gone: Gone) -> Planned {
    let first = First {
        title: format!("item {number}"),
        level: Some("2".to_string()),
        project: Some("one".to_string()),
        depends_on: n(on),
        tags: vec!["engine".to_string()],
        continues: None,
    };
    Planned {
        item: Item::new(ItemNumber(number), Time(0), first, Vec::new()),
        merging: gone == Gone::Merging,
        ended: match gone {
            Gone::Ended(end) => Some(end),
            _ => None,
        },
        letting_go: false,
        placed: false,
        starts: true,
        main_dependent: None,
    }
}

/// The items 1 to 5, none ended or merging: 2 depends on 1, 3 on 2, 4 on 1 and 3, and 5 on nothing.
fn graph() -> BTreeMap<ItemNumber, Planned> {
    [(1, n(&[])), (2, n(&[1])), (3, n(&[2])), (4, n(&[1, 3])), (5, n(&[]))]
        .into_iter()
        .map(|(item, on)| {
            let on: Vec<u64> = on.iter().map(|i| i.0).collect();
            (ItemNumber(item), planned(item, &on, Gone::Going))
        })
        .collect()
}

fn with(mut items: BTreeMap<ItemNumber, Planned>, number: u64, gone: Gone) -> BTreeMap<ItemNumber, Planned> {
    let p = items.get_mut(&ItemNumber(number)).expect("the item is there");
    p.merging = gone == Gone::Merging;
    p.ended = match gone {
        Gone::Ended(end) => Some(end),
        _ => None,
    };
    items
}

fn one(items: &BTreeMap<ItemNumber, Planned>, number: u64, asked: Asked) -> Result<bool, Refused> {
    match judge(items, &[(ItemNumber(number), asked)]) {
        Ok(changed) => Ok(changed[0]),
        Err(mut refused) => Err(refused.remove(0).why),
    }
}

fn judged(number: u64, gone: Gone, change: ItemChange) -> Result<bool, Refused> {
    one(&with(graph(), number, gone), number, Asked::Item(change))
}

fn unchangeable(why: Unchangeable) -> Result<bool, Refused> {
    Err(Refused::Unchangeable(why))
}

#[test]
fn a_circle_is_refused_with_the_way_round_it() {
    let circle = |way: &[u64]| unchangeable(Unchangeable::Circle(n(way)));
    assert_eq!(
        judged(1, Gone::Going, ItemChange::DependsOn(ItemNumber(4))),
        circle(&[4, 1])
    );
    assert_eq!(
        judged(2, Gone::Going, ItemChange::DependsOn(ItemNumber(3))),
        circle(&[3, 2])
    );
    assert_eq!(
        judged(2, Gone::Going, ItemChange::DependsOn(ItemNumber(4))),
        circle(&[4, 3, 2])
    );
    assert_eq!(
        judged(5, Gone::Going, ItemChange::DependsOn(ItemNumber(5))),
        circle(&[5])
    );
    assert_eq!(judged(5, Gone::Going, ItemChange::DependsOn(ItemNumber(4))), Ok(true));
    assert_eq!(judged(3, Gone::Going, ItemChange::DependsOn(ItemNumber(5))), Ok(true));
}

#[test]
fn what_the_item_is_already_is_no_change_and_no_refusal() {
    assert_eq!(judged(2, Gone::Going, ItemChange::Title("item 2".into())), Ok(false));
    assert_eq!(judged(2, Gone::Going, ItemChange::Level("2".into())), Ok(false));
    assert_eq!(judged(4, Gone::Going, ItemChange::DependsOn(ItemNumber(3))), Ok(false));
    assert_eq!(judged(4, Gone::Going, ItemChange::Tagged("engine".into())), Ok(false));
}

#[test]
fn a_started_work_takes_a_dependency_a_title_a_level_and_tags_and_gives_a_dependency_up() {
    assert_eq!(judged(4, Gone::Going, ItemChange::DependsOn(ItemNumber(5))), Ok(true));
    assert_eq!(
        judged(4, Gone::Going, ItemChange::NoLongerDependsOn(ItemNumber(3))),
        Ok(true)
    );
    assert_eq!(judged(4, Gone::Going, ItemChange::Title("less".into())), Ok(true));
    assert_eq!(judged(4, Gone::Going, ItemChange::Level("3".into())), Ok(true));
    assert_eq!(judged(4, Gone::Going, ItemChange::Tagged("page".into())), Ok(true));
    assert_eq!(judged(4, Gone::Going, ItemChange::Untagged("engine".into())), Ok(true));
    // A circle is refused for it as for any item.
    assert_eq!(
        judged(1, Gone::Going, ItemChange::DependsOn(ItemNumber(4))),
        unchangeable(Unchangeable::Circle(n(&[4, 1])))
    );
}

#[test]
fn a_work_merging_takes_no_dependency_on_an_item_that_has_not_ended_and_all_else() {
    let merging = |on: u64| unchangeable(Unchangeable::Merging(ItemNumber(on)));
    assert_eq!(
        judged(4, Gone::Merging, ItemChange::DependsOn(ItemNumber(5))),
        merging(5)
    );
    // An item that depends on it already is no change; one that has ended holds nothing.
    assert_eq!(
        judged(4, Gone::Merging, ItemChange::DependsOn(ItemNumber(3))),
        Ok(false)
    );
    let items = with(with(graph(), 4, Gone::Merging), 5, Gone::Ended(End::Cancelled));
    assert_eq!(
        one(&items, 4, Asked::Item(ItemChange::DependsOn(ItemNumber(5)))),
        Ok(true)
    );
    let items = with(with(graph(), 4, Gone::Merging), 5, Gone::Ended(End::Merged));
    assert_eq!(
        one(&items, 4, Asked::Item(ItemChange::DependsOn(ItemNumber(5)))),
        Ok(true)
    );
    assert_eq!(
        judged(4, Gone::Merging, ItemChange::NoLongerDependsOn(ItemNumber(3))),
        Ok(true)
    );
    assert_eq!(judged(4, Gone::Merging, ItemChange::Title("less".into())), Ok(true));
    // Once its run is over, it is no work that merges: it takes the item, as any started work does.
    assert_eq!(judged(4, Gone::Going, ItemChange::DependsOn(ItemNumber(5))), Ok(true));
}

fn continuing(number: u64, placed: bool, starts: bool, what: Option<Continues>) -> Result<bool, Refused> {
    let mut items = graph();
    let planned = items.get_mut(&ItemNumber(number)).expect("the item is there");
    planned.placed = placed;
    planned.starts = starts;
    one(&items, number, Asked::Item(ItemChange::Continues(what)))
}

fn branch(name: &str) -> Option<Continues> {
    Some(Continues::Branch(name.to_string()))
}

#[test]
fn an_item_comes_to_continue_a_branch_or_a_pull_request_and_to_continue_nothing_again() {
    assert_eq!(continuing(2, false, true, branch("old")), Ok(true));
    assert_eq!(continuing(2, false, true, Some(Continues::Pull(226))), Ok(true));
    // An item that continues nothing is not changed by being made to continue nothing.
    assert_eq!(continuing(2, false, true, None), Ok(false));
    let mut items = graph();
    let judged = judge(
        &items,
        &[
            (ItemNumber(2), Asked::Item(ItemChange::Continues(branch("old")))),
            (ItemNumber(2), Asked::Item(ItemChange::Continues(branch("old")))),
            (ItemNumber(2), Asked::Item(ItemChange::Continues(None))),
        ],
    );
    assert_eq!(judged, Ok(vec![true, false, true]));
    items.get_mut(&ItemNumber(2)).expect("the item is there").item.continues = branch("old");
    assert_eq!(one(&items, 2, Asked::Item(ItemChange::Continues(None))), Ok(true));
}

#[test]
fn what_an_item_continues_changes_until_its_branch_is_made_and_not_after() {
    // A work that started a run, as one whose start was refused has, is not yet placed.
    let mut items = with(graph(), 2, Gone::Going);
    assert_eq!(
        one(&items, 2, Asked::Item(ItemChange::Continues(branch("old")))),
        Ok(true)
    );
    items.get_mut(&ItemNumber(2)).expect("the item is there").placed = true;
    assert_eq!(
        one(&items, 2, Asked::Item(ItemChange::Continues(branch("old")))),
        unchangeable(Unchangeable::Placed)
    );
    assert_eq!(
        one(&items, 2, Asked::Item(ItemChange::Continues(None))),
        unchangeable(Unchangeable::Placed)
    );
}

#[test]
fn an_item_whose_flow_has_no_start_cannot_continue_anything_and_can_be_made_to_continue_nothing() {
    assert_eq!(continuing(2, false, false, branch("old")), Err(Refused::NoStart));
    assert_eq!(continuing(2, false, false, None), Ok(false));
}

#[test]
fn a_name_that_is_no_branch_is_refused() {
    for name in [
        "", "-x", "a b", "a\tb", "a:b", "a?b", "a*b", "a[b", "a~b", "a^b", "a\\b", "a..b", "a//b", "/a", "a/", ".a",
        "a.", "a.lock", "a@{b", "@",
    ] {
        assert_eq!(
            continuing(2, false, true, branch(name)),
            Err(Refused::BadContinues(Continues::Branch(name.to_string()))),
            "{name:?}"
        );
    }
    assert_eq!(
        continuing(2, false, true, Some(Continues::Pull(0))),
        Err(Refused::BadContinues(Continues::Pull(0)))
    );
    for name in ["work-226-0a1b2c3d", "feature/x", "a.b", "v1_2", "ünï"] {
        assert_eq!(continuing(2, false, true, branch(name)), Ok(true), "{name:?}");
    }
}

/// Whether git takes `refs/heads/<name>` as the name of a reference: `git check-ref-format`, which is what decides the
/// names a branch may have.
fn git_takes(name: &str) -> bool {
    std::process::Command::new("git")
        .args(["check-ref-format", &format!("refs/heads/{name}")])
        .status()
        .expect("git runs")
        .success()
}

#[test]
fn no_name_git_refuses_for_a_branch_is_taken_for_one_an_item_continues() {
    let mut taken_wrongly = Vec::new();
    for name in [
        // A part of the name, between slashes, may not begin with a dot or end with `.lock`.
        "a/.b",
        "a/./b",
        "a/.hidden/c",
        "a.lock/b",
        "a/b.lock/c",
        "a/.lock/b",
        // The names git takes, which an item may continue.
        "feature/x",
        "a./b",
        "a/@",
        "a@b",
        "a{b",
        "a\u{a0}b",
        "work-226-0a1b2c3d",
    ] {
        let taken = continuing(2, false, true, branch(name)) == Ok(true);
        if taken && !git_takes(name) {
            taken_wrongly.push(name);
        }
    }
    assert!(
        taken_wrongly.is_empty(),
        "names git refuses for a branch, that an item may continue: {taken_wrongly:?}"
    );
    for name in ["feature/x", "a./b", "a@b", "a{b", "a\u{a0}b", "work-226-0a1b2c3d"] {
        assert!(git_takes(name), "{name}: the reference of this test is wrong");
        assert_eq!(continuing(2, false, true, branch(name)), Ok(true), "{name}");
    }
}

#[test]
fn an_ended_work_takes_no_change_of_what_it_continues() {
    for end in [End::Merged, End::Cancelled] {
        let items = with(graph(), 5, Gone::Ended(end));
        assert_eq!(
            one(&items, 5, Asked::Item(ItemChange::Continues(branch("old")))),
            unchangeable(Unchangeable::Ended(end))
        );
    }
}

#[test]
fn an_ended_work_takes_no_change_and_no_second_cancel() {
    for end in [End::Merged, End::Cancelled] {
        let ended = unchangeable(Unchangeable::Ended(end));
        assert_eq!(judged(5, Gone::Ended(end), ItemChange::Title("other".into())), ended);
        assert_eq!(judged(5, Gone::Ended(end), ItemChange::Level("1".into())), ended);
        assert_eq!(judged(5, Gone::Ended(end), ItemChange::Tagged("page".into())), ended);
        assert_eq!(
            judged(5, Gone::Ended(end), ItemChange::NoLongerDependsOn(ItemNumber(1))),
            ended
        );
        assert_eq!(one(&with(graph(), 5, Gone::Ended(end)), 5, Asked::Cancel), ended);
    }
}

#[test]
fn a_title_of_no_words_and_a_tag_that_is_no_name_are_refused() {
    for gone in [Gone::Going, Gone::Merging] {
        for title in ["", " \t "] {
            assert_eq!(
                judged(2, gone, ItemChange::Title(title.into())),
                Err(Refused::BlankTitle)
            );
        }
        for tag in ["", "two words", "a,b", &"x".repeat(TAG_CHARS + 1)] {
            assert_eq!(
                judged(2, gone, ItemChange::Tagged(tag.to_string())),
                Err(Refused::BadTag(tag.to_string()))
            );
        }
        assert_eq!(judged(2, gone, ItemChange::Tagged("x".repeat(TAG_CHARS))), Ok(true));
    }
}

#[test]
fn an_item_there_is_not_one_not_depended_on_and_a_tag_not_carried_are_refused() {
    assert_eq!(
        judged(2, Gone::Going, ItemChange::DependsOn(ItemNumber(9))),
        Err(Refused::NoSuchItem(ItemNumber(9)))
    );
    assert_eq!(one(&graph(), 9, Asked::Cancel), Err(Refused::NoSuchItem(ItemNumber(9))));
    assert_eq!(
        judged(2, Gone::Going, ItemChange::NoLongerDependsOn(ItemNumber(5))),
        unchangeable(Unchangeable::NotDependent(ItemNumber(5)))
    );
    assert_eq!(
        judged(2, Gone::Going, ItemChange::Untagged("page".into())),
        unchangeable(Unchangeable::NotTagged("page".into()))
    );
    assert_eq!(
        judged(4, Gone::Going, ItemChange::NoLongerDependsOn(ItemNumber(1))),
        Ok(true)
    );
}

#[test]
fn an_item_is_cancelled_whatever_depends_on_it() {
    // 2 and 4 depend on 1; 3 on 2: the items that depend on 1 hold nothing back.
    assert_eq!(one(&graph(), 1, Asked::Cancel), Ok(true));
    // Nor does one whose last cell has a run open.
    let items = with(graph(), 4, Gone::Merging);
    assert_eq!(one(&items, 3, Asked::Cancel), Ok(true));
    // Cancelled together, in either order, they are all cancelled.
    for changes in [
        [(ItemNumber(1), Asked::Cancel), (ItemNumber(2), Asked::Cancel)],
        [(ItemNumber(2), Asked::Cancel), (ItemNumber(1), Asked::Cancel)],
    ] {
        assert_eq!(judge(&graph(), &changes), Ok(vec![true, true]));
    }
    // What depends on the cancelled item still does.
    let mut items = graph();
    made(&mut items, ItemNumber(1), &Asked::Cancel);
    assert_eq!(items[&ItemNumber(2)].item.depends_on, n(&[1]));
    assert_eq!(items[&ItemNumber(4)].item.depends_on, n(&[1, 3]));
}

#[test]
fn an_item_comes_to_depend_on_a_cancelled_one_and_no_circle_is_closed_through_it() {
    let items = with(graph(), 5, Gone::Ended(End::Cancelled));
    assert_eq!(
        one(&items, 3, Asked::Item(ItemChange::DependsOn(ItemNumber(5)))),
        Ok(true)
    );
    // Added in the same change after the cancel, it is let through too.
    let changes = [
        (ItemNumber(5), Asked::Cancel),
        (ItemNumber(3), Asked::Item(ItemChange::DependsOn(ItemNumber(5)))),
    ];
    assert_eq!(judge(&graph(), &changes), Ok(vec![true, true]));
    // 2 depends on 1: with 2 cancelled, 1 still may not come to depend on it.
    let items = with(graph(), 2, Gone::Ended(End::Cancelled));
    assert_eq!(
        one(&items, 1, Asked::Item(ItemChange::DependsOn(ItemNumber(2)))),
        unchangeable(Unchangeable::Circle(n(&[2, 1])))
    );
}

#[test]
fn a_cancelled_item_is_reopened_and_then_takes_changes() {
    let items = with(graph(), 5, Gone::Ended(End::Cancelled));
    assert_eq!(one(&items, 5, Asked::Reopen), Ok(true));
    // Reopened first, it takes a change in the same breath.
    let changes = [
        (ItemNumber(5), Asked::Reopen),
        (ItemNumber(5), Asked::Item(ItemChange::Title("again".into()))),
    ];
    assert_eq!(judge(&items, &changes), Ok(vec![true, true]));
    // Changed first, it is refused, as an ended item is.
    let changes = [
        (ItemNumber(5), Asked::Item(ItemChange::Title("again".into()))),
        (ItemNumber(5), Asked::Reopen),
    ];
    assert_eq!(
        judge(&items, &changes),
        Err(vec![NotMade {
            at: 0,
            item: ItemNumber(5),
            why: Refused::Unchangeable(Unchangeable::Ended(End::Cancelled)),
        }])
    );
}

#[test]
fn only_a_cancelled_item_is_reopened_and_not_while_it_lets_go_of_what_it_held() {
    assert_eq!(
        one(&graph(), 5, Asked::Reopen),
        unchangeable(Unchangeable::NotCancelled)
    );
    let merged = with(graph(), 5, Gone::Ended(End::Merged));
    assert_eq!(one(&merged, 5, Asked::Reopen), unchangeable(Unchangeable::NotCancelled));
    let mut letting_go = with(graph(), 5, Gone::Ended(End::Cancelled));
    letting_go.get_mut(&ItemNumber(5)).expect("5").letting_go = true;
    assert_eq!(
        one(&letting_go, 5, Asked::Reopen),
        unchangeable(Unchangeable::LettingGo)
    );
}

#[test]
fn an_item_is_reopened_onto_a_cancelled_one_and_under_a_started_work_but_not_under_one_that_merged_or_is_merging() {
    // 3 depends on 2: with both cancelled, 3 is reopened, and does not wait for 2.
    let items = with(
        with(graph(), 3, Gone::Ended(End::Cancelled)),
        2,
        Gone::Ended(End::Cancelled),
    );
    assert_eq!(one(&items, 3, Asked::Reopen), Ok(true));
    // 2 reopened first, 3, reopened after it, waits for it before its work goes on from where it was.
    let changes = [(ItemNumber(2), Asked::Reopen), (ItemNumber(3), Asked::Reopen)];
    assert_eq!(judge(&items, &changes), Ok(vec![true, true]));
    // 3 reopened first, 2 reopened after it: 3 waits for 2 where it stands.
    let changes = [(ItemNumber(3), Asked::Reopen), (ItemNumber(2), Asked::Reopen)];
    assert_eq!(judge(&items, &changes), Ok(vec![true, true]));
    // 2 cancelled under 3, which has started and is not merging: 3 waits for it again.
    let items = with(with(graph(), 2, Gone::Ended(End::Cancelled)), 3, Gone::Going);
    assert_eq!(one(&items, 2, Asked::Reopen), Ok(true));
    // 3 merging with 2 cancelled: reopened, 2 would be depended on by a work that merges as its run finishes.
    let items = with(with(graph(), 2, Gone::Ended(End::Cancelled)), 3, Gone::Merging);
    assert_eq!(
        one(&items, 2, Asked::Reopen),
        unchangeable(Unchangeable::MergingDependants(n(&[3])))
    );
    // Cancelled first, in the same change, 3 holds 2 back no more.
    let changes = [(ItemNumber(3), Asked::Cancel), (ItemNumber(2), Asked::Reopen)];
    assert_eq!(judge(&items, &changes), Ok(vec![true, true]));
    // 4 depends on 3: merged without waiting for it, it keeps 3 from becoming undone again.
    let items = with(
        with(graph(), 3, Gone::Ended(End::Cancelled)),
        4,
        Gone::Ended(End::Merged),
    );
    assert_eq!(
        one(&items, 3, Asked::Reopen),
        unchangeable(Unchangeable::MergedDependants(n(&[4])))
    );
}

#[test]
fn changes_asked_together_are_made_all_or_none_and_every_refusal_is_said() {
    let changes = [
        (ItemNumber(5), Asked::Item(ItemChange::Title("five".into()))),
        (ItemNumber(1), Asked::Cancel),
        (ItemNumber(2), Asked::Item(ItemChange::Title(" ".into()))),
        (ItemNumber(5), Asked::Item(ItemChange::Tagged("page".into()))),
    ];
    assert_eq!(
        judge(&graph(), &changes),
        Err(vec![NotMade {
            at: 2,
            item: ItemNumber(2),
            why: Refused::BlankTitle,
        }])
    );
    // Each is judged against what those before it make: two that make a circle only together are refused.
    let changes = [
        (ItemNumber(5), Asked::Item(ItemChange::DependsOn(ItemNumber(1)))),
        (ItemNumber(1), Asked::Item(ItemChange::DependsOn(ItemNumber(5)))),
    ];
    assert_eq!(
        judge(&graph(), &changes),
        Err(vec![NotMade {
            at: 1,
            item: ItemNumber(1),
            why: Refused::Unchangeable(Unchangeable::Circle(n(&[5, 1]))),
        }])
    );
}

#[test]
fn a_reopen_asked_with_the_cancel_that_ended_the_item_is_refused_for_the_letting_go_the_cancel_begins() {
    let changes = [(ItemNumber(5), Asked::Cancel), (ItemNumber(5), Asked::Reopen)];
    assert_eq!(
        judge(&graph(), &changes),
        Err(vec![NotMade {
            at: 1,
            item: ItemNumber(5),
            why: Refused::Unchangeable(Unchangeable::LettingGo),
        }])
    );
}

/// The items of [`graph`], with the main dependency of each item depended on: that of 2 on 1, of 3 on 2 and of 4 on 3.
fn with_mains() -> BTreeMap<ItemNumber, Planned> {
    let mut items = graph();
    for (of, main) in [(1, 2), (2, 3), (3, 4)] {
        let p = items.get_mut(&ItemNumber(of)).expect("the item is there");
        p.main_dependent = Some(ItemNumber(main));
    }
    items
}

/// Item `item`'s dependency on 1 made 1's main one.
fn main_on_1(item: u64) -> (ItemNumber, Asked) {
    (ItemNumber(item), Asked::Item(ItemChange::Main(ItemNumber(1))))
}

#[test]
fn a_dependency_is_made_the_main_one_and_is_no_change_where_it_is_the_main_one_already() {
    let items = with_mains();
    // 4 depends on 1, whose main dependency is that of 2.
    assert_eq!(one(&items, 4, main_on_1(4).1), Ok(true));
    assert_eq!(one(&items, 2, main_on_1(2).1), Ok(false));
    // An item it does not depend on, and one there is not, are refused, and so is a change of an item that ended.
    assert_eq!(
        one(&items, 5, main_on_1(5).1),
        unchangeable(Unchangeable::NotDependent(ItemNumber(1)))
    );
    assert_eq!(
        one(&items, 4, Asked::Item(ItemChange::Main(ItemNumber(9)))),
        Err(Refused::NoSuchItem(ItemNumber(9)))
    );
    let ended = with(with_mains(), 4, Gone::Ended(End::Cancelled));
    assert_eq!(
        one(&ended, 4, main_on_1(4).1),
        unchangeable(Unchangeable::Ended(End::Cancelled))
    );
    // Made the main one in turn, each is judged against the one before: the second time is no change.
    assert_eq!(
        judge(&items, &[main_on_1(4), main_on_1(4), main_on_1(2), main_on_1(2)]),
        Ok(vec![true, false, true, false])
    );
    // A dependency added and made the main one together: both are changes.
    let added = (ItemNumber(5), Asked::Item(ItemChange::DependsOn(ItemNumber(1))));
    assert_eq!(judge(&items, &[added, main_on_1(5)]), Ok(vec![true, true]));
    // Once a change before it takes the main one away, which is the main one is the store's to say, and making it so
    // again is a change.
    let gone = (ItemNumber(2), Asked::Item(ItemChange::NoLongerDependsOn(ItemNumber(1))));
    let again = (ItemNumber(2), Asked::Item(ItemChange::DependsOn(ItemNumber(1))));
    assert_eq!(judge(&items, &[gone, again, main_on_1(2)]), Ok(vec![true, true, true]));
    let cancelled = (ItemNumber(2), Asked::Cancel);
    assert_eq!(judge(&items, &[cancelled, main_on_1(4)]), Ok(vec![true, true]));
}

#[test]
fn an_items_open_children_are_those_whose_main_dependency_is_on_it_and_have_not_ended() {
    // 1, 2 and 3 are drawn under 9, and 4 under 8; 2 has ended.
    let mains: Mains = [(1, 9), (2, 9), (3, 9), (4, 8)]
        .into_iter()
        .map(|(child, parent)| (ItemNumber(child), ItemNumber(parent)))
        .collect();
    let children = open_children(&mains, |item| item != ItemNumber(2));
    assert_eq!(children, BTreeMap::from([(ItemNumber(9), 2), (ItemNumber(8), 1)]));
    let none = open_children(&mains, |_| false);
    assert!(none.is_empty(), "an item with no open child has none counted");
}

#[test]
fn an_act_is_warned_of_past_the_warning_and_refused_past_the_most_only_where_it_raises_a_count() {
    let limits = Children { warn: 10, most: 15 };
    let count = |pairs: &[(u64, usize)]| -> BTreeMap<ItemNumber, usize> {
        pairs.iter().map(|(item, n)| (ItemNumber(*item), *n)).collect()
    };
    let crowded = |item: u64, children: usize, limit: usize| Crowded {
        item: Some(ItemNumber(item)),
        children,
        limit,
    };
    // Up to the warning nothing is said; past it, warned; up to the most still done; past it, refused.
    for (after, said) in [
        (10, Ok(vec![])),
        (11, Ok(vec![crowded(1, 11, 10)])),
        (15, Ok(vec![crowded(1, 15, 10)])),
        (16, Err(vec![crowded(1, 16, 15)])),
    ] {
        assert_eq!(
            crowding(&count(&[(1, 9)]), &count(&[(1, after)]), limits, None),
            said,
            "{after}"
        );
    }
    // An item left past a limit with no more than it had, or fewer, is neither warned of nor refused.
    assert_eq!(
        crowding(&count(&[(1, 20)]), &count(&[(1, 20)]), limits, None),
        Ok(vec![])
    );
    assert_eq!(
        crowding(&count(&[(1, 20)]), &count(&[(1, 17)]), limits, None),
        Ok(vec![])
    );
    assert_eq!(
        crowding(&count(&[(1, 12)]), &count(&[(1, 11)]), limits, None),
        Ok(vec![])
    );
    // An item that had none counts from none.
    assert_eq!(
        crowding(&count(&[]), &count(&[(2, 11)]), limits, None),
        Ok(vec![crowded(2, 11, 10)])
    );
    // A refusal names every item past the most, and a warning past it says nothing of those past the warning only.
    assert_eq!(
        crowding(
            &count(&[(1, 9), (2, 9)]),
            &count(&[(1, 16), (2, 11), (3, 17)]),
            limits,
            None
        ),
        Err(vec![crowded(1, 16, 15), crowded(3, 17, 15)])
    );
    // The item the act adds is said as none.
    assert_eq!(
        crowding(&count(&[]), &count(&[(7, 16)]), limits, Some(ItemNumber(7))),
        Err(vec![Crowded {
            item: None,
            children: 16,
            limit: 15
        }])
    );
}

/// A small generator of numbers, so that the property below is tried on the same many cases on every run.
struct Dice(u64);

impl Dice {
    fn roll(&mut self, below: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % below
    }
}

/// Items 1 to 6, each depending on some of those before it, each going, merging, merged or cancelled, as a store may
/// hold them: an item merged, or merging as its last run finishes, has what it depends on ended, merged or cancelled.
fn some_items(dice: &mut Dice) -> BTreeMap<ItemNumber, Planned> {
    let mut items: BTreeMap<ItemNumber, Planned> = BTreeMap::new();
    for number in 1..=6 {
        let on: Vec<u64> = (1..number).filter(|_| dice.roll(3) == 0).collect();
        let ends: Vec<Option<End>> = on.iter().map(|o| items[&ItemNumber(*o)].ended).collect();
        let all_ended = ends.iter().all(Option::is_some);
        let gone = match dice.roll(4) {
            0 => Gone::Going,
            1 if all_ended => Gone::Merging,
            1 => Gone::Going,
            2 if all_ended => Gone::Ended(End::Merged),
            _ => Gone::Ended(End::Cancelled),
        };
        items.insert(ItemNumber(number), planned(number, &on, gone));
    }
    items
}

/// A change of some item, any kind.
fn some_change(dice: &mut Dice) -> (ItemNumber, Asked) {
    let item = ItemNumber(dice.roll(6) + 1);
    let on = ItemNumber(dice.roll(6) + 1);
    let asked = match dice.roll(7) {
        0 | 1 => Asked::Cancel,
        2 => Asked::Reopen,
        3 => Asked::Item(ItemChange::DependsOn(on)),
        4 => Asked::Item(ItemChange::NoLongerDependsOn(on)),
        5 => Asked::Item(ItemChange::Main(on)),
        _ => Asked::Item(ItemChange::Title("t".into())),
    };
    (item, asked)
}

/// The relations the changes keep: no merged item comes to depend on more than it did; and no work that merges as its
/// last run finishes, before the changes and after them and not cancelled, comes to depend on an item that has not
/// ended, nor does an item it depended on that had ended come not to have ended.
fn keeps(before: &BTreeMap<ItemNumber, Planned>, after: &BTreeMap<ItemNumber, Planned>) {
    for (number, was) in before.iter().filter(|(_, p)| p.ended == Some(End::Merged)) {
        let now = &after[number];
        assert_eq!(now.ended, Some(End::Merged), "a merged item is merged no longer");
        assert_eq!(
            now.item.depends_on, was.item.depends_on,
            "a merged item came to depend on another"
        );
    }
    let merging = |p: &Planned| p.merging && p.ended.is_none();
    for (number, was) in before
        .iter()
        .filter(|(number, p)| merging(p) && merging(&after[*number]))
    {
        for on in &after[number].item.depends_on {
            if !was.item.depends_on.contains(on) {
                assert!(
                    after[on].ended.is_some(),
                    "{number:?} merging came to depend on {on:?}, which has not ended"
                );
            }
        }
        for on in &was.item.depends_on {
            if before[on].ended.is_some() {
                assert!(
                    after[on].ended.is_some(),
                    "{number:?} merging on {on:?} ended, which has not ended any more"
                );
            }
        }
    }
}

/// Whatever is asked, in any order, one at a time or together, the changes the judge lets through keep the
/// relations; and changes asked together are let through exactly when each, asked in turn after those before it, is.
#[test]
fn no_accepted_changes_leave_a_merged_item_or_a_merging_work_on_what_has_not_ended() {
    let mut dice = Dice(0x9e37_79b9_7f4a_7c15);
    for _ in 0..400 {
        let mut items = some_items(&mut dice);
        for _ in 0..12 {
            let asked: Vec<(ItemNumber, Asked)> = (0..dice.roll(3) + 1).map(|_| some_change(&mut dice)).collect();
            let before = items.clone();
            // Asked in turn, one at a time, what each makes is made.
            let mut in_turn = items.clone();
            let mut each = Vec::new();
            for (item, asked) in &asked {
                let judged = judge(&in_turn, &[(*item, asked.clone())]);
                if let Ok(changed) = &judged
                    && changed[0]
                {
                    made(&mut in_turn, *item, asked);
                }
                each.push(judged.is_ok());
            }
            match judge(&items, &asked) {
                Ok(changed) => {
                    assert!(
                        each.iter().all(|ok| *ok),
                        "together let through, one by one refused: {asked:?}"
                    );
                    for ((item, asked), changed) in asked.iter().zip(changed) {
                        if changed {
                            made(&mut items, *item, asked);
                        }
                    }
                    assert_eq!(
                        items.values().map(|p| (&p.item, p.ended)).collect::<Vec<_>>(),
                        in_turn.values().map(|p| (&p.item, p.ended)).collect::<Vec<_>>(),
                        "together they made otherwise than in turn"
                    );
                }
                Err(_) => assert!(
                    !each.iter().all(|ok| *ok),
                    "together refused, one by one let through: {asked:?}"
                ),
            }
            keeps(&before, &items);
        }
    }
}
