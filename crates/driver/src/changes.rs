//! Changing an item after it was added: whether a change may be made is judged from the item, how far its work has
//! gone and what each item depends on, and nothing else (principle `decide-purely`), so that the driver, which holds
//! them, and a test give it the same answer. A change is kept as a record of its own after the item's others, and what
//! the item stands as is worked out of them (crate `store`, [`store::Item`]).
//!
//! Every change waits for nothing and is refused once the item's work has ended: nothing of the work goes on, and the
//! item is what it ended as. What it depends on changes only while its work has begun no run (entry
//! `dependencies-until-the-work-begins`).

use decide::End;
use store::{Item, ItemChange, ItemNumber};

/// How far an item's work has gone, as a change of the item is judged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gone {
    /// It has begun no run.
    NotBegun,
    /// It has begun a run, and has not ended.
    Begun,
    /// It has ended, so.
    Ended(End),
}

/// Why a change of an item is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unchangeable {
    /// Its work has ended so: the item is what it ended as.
    Ended(End),
    /// Its work has begun a run: what the item depends on is what its work began on.
    Begun,
    /// It would depend on an item that depends on it already, by this way: from the item it was to depend on,
    /// through each item the one before depends on, to the item itself. An item that was to depend on itself is the
    /// way of that one item.
    Circle(Vec<ItemNumber>),
    /// It does not depend on the item it was to no longer depend on.
    NotDependent(ItemNumber),
}

/// Why a change was not made: there is no item it names, or it may not be made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    NoSuchItem(ItemNumber),
    Unchangeable(Unchangeable),
}

/// Whether the change makes the item other than it is, or why it is refused: `gone` says how far the item's work has
/// gone, and `depends_on` gives what each item depends on as it stands, none for an item there is not. A change that
/// leaves the item as it is, a title or a level it has, or an item it depends on already, is no change, and is not
/// refused for that.
pub fn judge(
    item: &Item,
    gone: Gone,
    change: &ItemChange,
    depends_on: impl Fn(ItemNumber) -> Option<Vec<ItemNumber>>,
) -> Result<bool, Refused> {
    let _ = (item, gone, change, way(item.number, item.number, &depends_on));
    Ok(false)
}

/// A way from one item to another through what each depends on, both included, where there is one: the first found,
/// taking what each depends on in the order of their numbers.
fn way(
    from: ItemNumber,
    to: ItemNumber,
    depends_on: &impl Fn(ItemNumber) -> Option<Vec<ItemNumber>>,
) -> Option<Vec<ItemNumber>> {
    let mut seen = std::collections::BTreeSet::new();
    let mut way = vec![from];
    // Each item on the way, with the items it depends on that are still to be tried.
    let mut left = vec![reversed(depends_on(from))];
    loop {
        if way.last() == Some(&to) {
            return Some(way);
        }
        let next = left.last_mut()?.pop();
        match next {
            Some(next) if seen.insert(next) => {
                left.push(reversed(depends_on(next)));
                way.push(next);
            }
            Some(_) => {}
            None => {
                left.pop();
                way.pop();
            }
        }
    }
}

/// What an item depends on, the last first, so that taking from its end takes them in the order of their numbers.
fn reversed(on: Option<Vec<ItemNumber>>) -> Vec<ItemNumber> {
    let mut on = on.unwrap_or_default();
    on.reverse();
    on
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use store::First;
    use work::Time;

    use super::*;

    fn n(numbers: &[u64]) -> Vec<ItemNumber> {
        numbers.iter().copied().map(ItemNumber).collect()
    }

    /// The items 1 to 5: 2 depends on 1, 3 on 2, 4 on 1 and 3, and 5 on nothing.
    fn graph() -> BTreeMap<ItemNumber, Vec<ItemNumber>> {
        [(1, n(&[])), (2, n(&[1])), (3, n(&[2])), (4, n(&[1, 3])), (5, n(&[]))]
            .into_iter()
            .map(|(item, on)| (ItemNumber(item), on))
            .collect()
    }

    fn item(graph: &BTreeMap<ItemNumber, Vec<ItemNumber>>, number: u64) -> Item {
        let first = First {
            title: format!("item {number}"),
            level: Some("2".to_string()),
            depends_on: graph[&ItemNumber(number)].clone(),
        };
        Item::new(ItemNumber(number), Time(0), first, Vec::new())
    }

    fn judged(number: u64, gone: Gone, change: ItemChange) -> Result<bool, Refused> {
        let graph = graph();
        judge(&item(&graph, number), gone, &change, |on| graph.get(&on).cloned())
    }

    #[test]
    fn a_circle_is_refused_with_the_way_round_it() {
        let circle = |way: &[u64]| Err(Refused::Unchangeable(Unchangeable::Circle(n(way))));
        assert_eq!(
            judged(1, Gone::NotBegun, ItemChange::DependsOn(ItemNumber(4))),
            circle(&[4, 1])
        );
        assert_eq!(
            judged(2, Gone::NotBegun, ItemChange::DependsOn(ItemNumber(3))),
            circle(&[3, 2])
        );
        assert_eq!(
            judged(2, Gone::NotBegun, ItemChange::DependsOn(ItemNumber(4))),
            circle(&[4, 3, 2])
        );
        assert_eq!(
            judged(5, Gone::NotBegun, ItemChange::DependsOn(ItemNumber(5))),
            circle(&[5])
        );
        assert_eq!(
            judged(5, Gone::NotBegun, ItemChange::DependsOn(ItemNumber(4))),
            Ok(true)
        );
        assert_eq!(
            judged(3, Gone::NotBegun, ItemChange::DependsOn(ItemNumber(5))),
            Ok(true)
        );
    }

    #[test]
    fn what_the_item_is_already_is_no_change_and_no_refusal() {
        assert_eq!(judged(2, Gone::NotBegun, ItemChange::Title("item 2".into())), Ok(false));
        assert_eq!(judged(2, Gone::Begun, ItemChange::Level("2".into())), Ok(false));
        assert_eq!(
            judged(4, Gone::NotBegun, ItemChange::DependsOn(ItemNumber(3))),
            Ok(false)
        );
    }

    #[test]
    fn a_begun_work_keeps_what_it_depends_on_and_takes_a_title_and_a_level() {
        let begun = Err(Refused::Unchangeable(Unchangeable::Begun));
        assert_eq!(judged(4, Gone::Begun, ItemChange::DependsOn(ItemNumber(5))), begun);
        assert_eq!(
            judged(4, Gone::Begun, ItemChange::NoLongerDependsOn(ItemNumber(3))),
            begun
        );
        assert_eq!(judged(4, Gone::Begun, ItemChange::Title("less".into())), Ok(true));
        assert_eq!(judged(4, Gone::Begun, ItemChange::Level("3".into())), Ok(true));
    }

    #[test]
    fn an_ended_work_takes_no_change() {
        for end in [End::Merged, End::Cancelled] {
            let ended = Err(Refused::Unchangeable(Unchangeable::Ended(end)));
            assert_eq!(judged(2, Gone::Ended(end), ItemChange::Title("other".into())), ended);
            assert_eq!(judged(2, Gone::Ended(end), ItemChange::Level("1".into())), ended);
            assert_eq!(
                judged(2, Gone::Ended(end), ItemChange::NoLongerDependsOn(ItemNumber(1))),
                ended
            );
        }
    }

    #[test]
    fn an_item_there_is_not_and_one_not_depended_on_are_refused() {
        assert_eq!(
            judged(2, Gone::NotBegun, ItemChange::DependsOn(ItemNumber(9))),
            Err(Refused::NoSuchItem(ItemNumber(9)))
        );
        assert_eq!(
            judged(2, Gone::NotBegun, ItemChange::NoLongerDependsOn(ItemNumber(5))),
            Err(Refused::Unchangeable(Unchangeable::NotDependent(ItemNumber(5))))
        );
        assert_eq!(
            judged(4, Gone::NotBegun, ItemChange::NoLongerDependsOn(ItemNumber(1))),
            Ok(true)
        );
    }
}
