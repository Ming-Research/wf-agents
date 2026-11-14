use std::collections::BTreeMap;

use store::First;
use work::Time;

use super::*;

fn n(numbers: &[u64]) -> Vec<ItemNumber> {
    numbers.iter().copied().map(ItemNumber).collect()
}

fn planned(number: u64, on: &[u64], gone: Gone) -> Planned {
    let first = First {
        title: format!("item {number}"),
        level: Some("2".to_string()),
        project: Some("one".to_string()),
        depends_on: n(on),
        tags: vec!["engine".to_string()],
    };
    Planned {
        item: Item::new(ItemNumber(number), Time(0), first, Vec::new()),
        started: gone != Gone::NotStarted,
        ended: match gone {
            Gone::Ended(end) => Some(end),
            _ => None,
        },
        letting_go: false,
    }
}

/// The items 1 to 5, none started: 2 depends on 1, 3 on 2, 4 on 1 and 3, and 5 on nothing.
fn graph() -> BTreeMap<ItemNumber, Planned> {
    [(1, n(&[])), (2, n(&[1])), (3, n(&[2])), (4, n(&[1, 3])), (5, n(&[]))]
        .into_iter()
        .map(|(item, on)| {
            let on: Vec<u64> = on.iter().map(|i| i.0).collect();
            (ItemNumber(item), planned(item, &on, Gone::NotStarted))
        })
        .collect()
}

fn with(mut items: BTreeMap<ItemNumber, Planned>, number: u64, gone: Gone) -> BTreeMap<ItemNumber, Planned> {
    let p = items.get_mut(&ItemNumber(number)).expect("the item is there");
    p.started = gone != Gone::NotStarted;
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
        judged(1, Gone::NotStarted, ItemChange::DependsOn(ItemNumber(4))),
        circle(&[4, 1])
    );
    assert_eq!(
        judged(2, Gone::NotStarted, ItemChange::DependsOn(ItemNumber(3))),
        circle(&[3, 2])
    );
    assert_eq!(
        judged(2, Gone::NotStarted, ItemChange::DependsOn(ItemNumber(4))),
        circle(&[4, 3, 2])
    );
    assert_eq!(
        judged(5, Gone::NotStarted, ItemChange::DependsOn(ItemNumber(5))),
        circle(&[5])
    );
    assert_eq!(
        judged(5, Gone::NotStarted, ItemChange::DependsOn(ItemNumber(4))),
        Ok(true)
    );
    assert_eq!(
        judged(3, Gone::NotStarted, ItemChange::DependsOn(ItemNumber(5))),
        Ok(true)
    );
}

#[test]
fn what_the_item_is_already_is_no_change_and_no_refusal() {
    assert_eq!(
        judged(2, Gone::NotStarted, ItemChange::Title("item 2".into())),
        Ok(false)
    );
    assert_eq!(judged(2, Gone::Started, ItemChange::Level("2".into())), Ok(false));
    assert_eq!(
        judged(4, Gone::NotStarted, ItemChange::DependsOn(ItemNumber(3))),
        Ok(false)
    );
    assert_eq!(judged(4, Gone::Started, ItemChange::Tagged("engine".into())), Ok(false));
}

#[test]
fn a_started_work_keeps_what_it_depends_on_and_takes_a_title_a_level_and_tags() {
    let started = unchangeable(Unchangeable::Started);
    assert_eq!(judged(4, Gone::Started, ItemChange::DependsOn(ItemNumber(5))), started);
    assert_eq!(
        judged(4, Gone::Started, ItemChange::NoLongerDependsOn(ItemNumber(3))),
        started
    );
    assert_eq!(judged(4, Gone::Started, ItemChange::Title("less".into())), Ok(true));
    assert_eq!(judged(4, Gone::Started, ItemChange::Level("3".into())), Ok(true));
    assert_eq!(judged(4, Gone::Started, ItemChange::Tagged("page".into())), Ok(true));
    assert_eq!(
        judged(4, Gone::Started, ItemChange::Untagged("engine".into())),
        Ok(true)
    );
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
    for gone in [Gone::NotStarted, Gone::Started] {
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
        judged(2, Gone::NotStarted, ItemChange::DependsOn(ItemNumber(9))),
        Err(Refused::NoSuchItem(ItemNumber(9)))
    );
    assert_eq!(one(&graph(), 9, Asked::Cancel), Err(Refused::NoSuchItem(ItemNumber(9))));
    assert_eq!(
        judged(2, Gone::NotStarted, ItemChange::NoLongerDependsOn(ItemNumber(5))),
        unchangeable(Unchangeable::NotDependent(ItemNumber(5)))
    );
    assert_eq!(
        judged(2, Gone::NotStarted, ItemChange::Untagged("page".into())),
        unchangeable(Unchangeable::NotTagged("page".into()))
    );
    assert_eq!(
        judged(4, Gone::NotStarted, ItemChange::NoLongerDependsOn(ItemNumber(1))),
        Ok(true)
    );
}

#[test]
fn an_item_is_not_cancelled_while_an_item_that_has_not_ended_depends_on_it() {
    // 2 and 4 depend on 1; 3 on 2.
    assert_eq!(
        one(&graph(), 1, Asked::Cancel),
        unchangeable(Unchangeable::OpenDependants(n(&[2, 4])))
    );
    // One that has started counts as much as one that has not.
    let items = with(graph(), 4, Gone::Started);
    assert_eq!(
        one(&items, 3, Asked::Cancel),
        unchangeable(Unchangeable::OpenDependants(n(&[4])))
    );
    // Dependants that have ended hold nothing back.
    let items = with(with(graph(), 4, Gone::Ended(End::Cancelled)), 3, Gone::NotStarted);
    assert_eq!(one(&items, 3, Asked::Cancel), Ok(true));
    let items = with(graph(), 4, Gone::Ended(End::Merged));
    assert_eq!(one(&items, 3, Asked::Cancel), Ok(true));
    // Cancelled together, the dependant first, they are all cancelled.
    let changes = [(ItemNumber(4), Asked::Cancel), (ItemNumber(3), Asked::Cancel)];
    assert_eq!(judge(&graph(), &changes), Ok(vec![true, true]));
}

#[test]
fn nothing_comes_to_depend_on_a_cancelled_item() {
    let items = with(graph(), 5, Gone::Ended(End::Cancelled));
    assert_eq!(
        one(&items, 3, Asked::Item(ItemChange::DependsOn(ItemNumber(5)))),
        unchangeable(Unchangeable::OnCancelled(n(&[5])))
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
fn an_item_is_not_reopened_onto_a_cancelled_one_nor_under_a_merged_one() {
    // 3 depends on 2: with both cancelled, 3 waits for 2 to be reopened first.
    let items = with(
        with(graph(), 3, Gone::Ended(End::Cancelled)),
        2,
        Gone::Ended(End::Cancelled),
    );
    assert_eq!(
        one(&items, 3, Asked::Reopen),
        unchangeable(Unchangeable::OnCancelled(n(&[2])))
    );
    let changes = [(ItemNumber(2), Asked::Reopen), (ItemNumber(3), Asked::Reopen)];
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
        Err(vec![
            NotMade {
                at: 1,
                item: ItemNumber(1),
                why: Refused::Unchangeable(Unchangeable::OpenDependants(n(&[2, 4]))),
            },
            NotMade {
                at: 2,
                item: ItemNumber(2),
                why: Refused::BlankTitle,
            },
        ])
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

/// Items 1 to 6, each depending on some of those before it, each not started, started, merged or cancelled, as a store
/// may hold them: no item that has not ended depends on a cancelled one, and an item merged with what it depends on
/// ended, merged or cancelled, as one whose flow waits for no item may.
fn some_items(dice: &mut Dice) -> BTreeMap<ItemNumber, Planned> {
    let mut items: BTreeMap<ItemNumber, Planned> = BTreeMap::new();
    for number in 1..=6 {
        let on: Vec<u64> = (1..number).filter(|_| dice.roll(3) == 0).collect();
        let ends: Vec<Option<End>> = on.iter().map(|o| items[&ItemNumber(*o)].ended).collect();
        let on_cancelled = ends.contains(&Some(End::Cancelled));
        let all_ended = ends.iter().all(Option::is_some);
        let gone = match dice.roll(4) {
            0 if !on_cancelled => Gone::NotStarted,
            1 if !on_cancelled => Gone::Started,
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
    let asked = match dice.roll(6) {
        0 | 1 => Asked::Cancel,
        2 => Asked::Reopen,
        3 => Asked::Item(ItemChange::DependsOn(on)),
        4 => Asked::Item(ItemChange::NoLongerDependsOn(on)),
        _ => Asked::Item(ItemChange::Title("t".into())),
    };
    (item, asked)
}

/// The two relations the changes keep, each where it held before: no item that has not ended depends on a cancelled
/// one, and no merged item comes to depend on more than it did, nor on one that was done and is no longer.
fn keeps(before: &BTreeMap<ItemNumber, Planned>, after: &BTreeMap<ItemNumber, Planned>) {
    for p in after.values() {
        if p.ended.is_none() {
            for on in &p.item.depends_on {
                assert_ne!(
                    after[on].ended,
                    Some(End::Cancelled),
                    "{p:?} depends on a cancelled {on:?}"
                );
            }
        }
    }
    for (number, was) in before.iter().filter(|(_, p)| p.ended == Some(End::Merged)) {
        let now = &after[number];
        assert_eq!(now.ended, Some(End::Merged), "a merged item is merged no longer");
        assert_eq!(
            now.item.depends_on, was.item.depends_on,
            "a merged item came to depend on another"
        );
        for on in &was.item.depends_on {
            if before[on].ended.is_some() {
                assert!(
                    after[on].ended.is_some(),
                    "{number:?} merged on {on:?}, which is not done any more"
                );
            }
        }
    }
}

/// Whatever is asked, in any order, one at a time or together, the changes the judge lets through keep the two
/// relations; and changes asked together are let through exactly when each, asked in turn after those before it, is.
#[test]
fn no_accepted_changes_leave_an_open_item_on_a_cancelled_one_or_a_merged_one_on_what_is_not_done() {
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
