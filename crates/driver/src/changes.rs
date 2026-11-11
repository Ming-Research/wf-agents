//! Changing an item after it was added: whether a change may be made is judged from the item, how far its work has
//! gone and what each item depends on, and nothing else (principle `decide-purely`), so that the driver, which holds
//! them, and a test give it the same answer. A change is kept as a record of its own after the item's others, and what
//! the item stands as is worked out of them (crate `store`, [`store::Item`]).
//!
//! Every change is refused once the item's work has ended: nothing of the work goes on, and the item is what it ended
//! as. Its title and its level change until then; what it depends on only while its work has started no run (entry
//! `dependencies-until-the-work-starts`).

use decide::End;
use store::{Item, ItemChange, ItemNumber};

/// How far an item's work has gone, as a change of the item is judged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gone {
    /// It has started no run.
    NotStarted,
    /// It has started a run, and has not ended.
    Started,
    /// It has ended, so.
    Ended(End),
}

/// Why a change of an item is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unchangeable {
    /// Its work has ended so: the item is what it ended as.
    Ended(End),
    /// Its work has started a run: what the item depends on is what its work started on.
    Started,
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
    /// The title it was to be given has no words.
    BlankTitle,
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
    let refused = |why| Err(Refused::Unchangeable(why));
    if let Gone::Ended(end) = gone {
        return refused(Unchangeable::Ended(end));
    }
    match change {
        ItemChange::Title(title) => Ok(*title != item.title),
        ItemChange::Level(level) => Ok(item.level.as_deref() != Some(level.as_str())),
        ItemChange::DependsOn(on) | ItemChange::NoLongerDependsOn(on) if depends_on(*on).is_none() => {
            Err(Refused::NoSuchItem(*on))
        }
        ItemChange::DependsOn(_) | ItemChange::NoLongerDependsOn(_) if gone == Gone::Started => {
            refused(Unchangeable::Started)
        }
        ItemChange::DependsOn(on) if item.depends_on.contains(on) => Ok(false),
        ItemChange::DependsOn(on) => match way(*on, item.number, &depends_on) {
            Some(way) => refused(Unchangeable::Circle(way)),
            None => Ok(true),
        },
        ItemChange::NoLongerDependsOn(on) if !item.depends_on.contains(on) => refused(Unchangeable::NotDependent(*on)),
        ItemChange::NoLongerDependsOn(_) => Ok(true),
    }
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
    }

    #[test]
    fn a_started_work_keeps_what_it_depends_on_and_takes_a_title_and_a_level() {
        let started = Err(Refused::Unchangeable(Unchangeable::Started));
        assert_eq!(judged(4, Gone::Started, ItemChange::DependsOn(ItemNumber(5))), started);
        assert_eq!(
            judged(4, Gone::Started, ItemChange::NoLongerDependsOn(ItemNumber(3))),
            started
        );
        assert_eq!(judged(4, Gone::Started, ItemChange::Title("less".into())), Ok(true));
        assert_eq!(judged(4, Gone::Started, ItemChange::Level("3".into())), Ok(true));
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
    fn a_title_of_no_words_is_refused_started_or_not() {
        for gone in [Gone::NotStarted, Gone::Started] {
            for title in ["", " \t "] {
                assert_eq!(
                    judged(2, gone, ItemChange::Title(title.into())),
                    Err(Refused::BlankTitle)
                );
            }
        }
    }

    #[test]
    fn an_item_there_is_not_and_one_not_depended_on_are_refused() {
        assert_eq!(
            judged(2, Gone::NotStarted, ItemChange::DependsOn(ItemNumber(9))),
            Err(Refused::NoSuchItem(ItemNumber(9)))
        );
        assert_eq!(
            judged(2, Gone::NotStarted, ItemChange::NoLongerDependsOn(ItemNumber(5))),
            Err(Refused::Unchangeable(Unchangeable::NotDependent(ItemNumber(5))))
        );
        assert_eq!(
            judged(4, Gone::NotStarted, ItemChange::NoLongerDependsOn(ItemNumber(1))),
            Ok(true)
        );
    }
}
