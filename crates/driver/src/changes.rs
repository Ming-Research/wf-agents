//! Changing items after they were added: whether changes may be made is judged from the items, how far each one's work
//! has gone and what each depends on, and nothing else (principle `decide-purely`), so that the driver, which holds
//! them, and a test give it the same answer. Changes asked together are judged in the order given, each against the
//! items as the changes before it would leave them, and are made all of them or none ([`judge`]). A change of an item
//! is kept as a record of its own after the item's others, and what the item stands as is worked out of them (crate
//! `store`, [`store::Item`]); a cancel and a reopen are the work's own records.
//!
//! Every change but a reopen is refused once the item's work has ended: nothing of the work goes on, and the item is
//! what it ended as. Its title, level, tags and what it depends on change until then, and what it continues only
//! until its work's branch is made; a work that depends on an item that has not ended waits for it where it stands
//! (decision `an-items-dependencies-change-until-its-work-ends`). A cancel takes away no dependency on the item, and
//! an item may come to depend on a cancelled one: a dependency on a cancelled item holds as one on a merged item does
//! (decision `a-cancelled-item-keeps-its-place`). One relation between items holds whatever is changed: an item that
//! has merged depends on no item that has not ended. A work whose last cell has a run open, which merges as it
//! finishes, is held to it as if it had merged: it is not made to depend on an item that has not ended, and an item it
//! depends on is not reopened (decision `a-cancelled-item-is-not-reopened-under-a-merged-item`).

use std::collections::{BTreeMap, BTreeSet};

use decide::End;
use flow::Compiled;
use store::{Continues, Item, ItemChange, ItemNumber};
use work::Work;

pub use store::TAG_CHARS;

/// A change of an item, as whoever plans the work asks for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Asked {
    /// A change of the item itself: its title, level, dependencies or tags.
    Item(ItemChange),
    /// Its work is cancelled, and ends.
    Cancel,
    /// It is reopened: it was cancelled, and its work goes on again once it is let on.
    Reopen,
}

/// An item as changes of it are judged: the item, whether its work is merging, how it ended, and whether its end is
/// letting go of what it held.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned {
    pub item: Item,
    /// Whether its work has a run of the last cell of its flow that is open: neither finished nor abandoned, so that the
    /// work merges as the run finishes.
    pub merging: bool,
    /// How its work ended, where it has.
    pub ended: Option<End>,
    /// Whether its work's end is letting go of what it held, which a reopen would leave half done.
    pub letting_go: bool,
    /// Whether its work has made its branch: the cell of its flow that starts a work has handed in a return.
    pub placed: bool,
    /// Whether its flow has a cell that starts a work, which is what reads what an item continues.
    pub starts: bool,
}

/// Why a change of an item is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unchangeable {
    /// Its work has ended so: the item is what it ended as.
    Ended(End),
    /// Its work has a run of its last cell open, and merges as it finishes: it is not to depend on an item that has not
    /// ended, which would leave a merged item depending on one.
    Merging(ItemNumber),
    /// Its work has made its branch: what an item continues is what its branch was made from.
    Placed,
    /// It would depend on an item that depends on it already, by this way: from the item it was to depend on,
    /// through each item the one before depends on, to the item itself. An item that was to depend on itself is the
    /// way of that one item.
    Circle(Vec<ItemNumber>),
    /// It does not depend on the item it was to no longer depend on.
    NotDependent(ItemNumber),
    /// It does not carry the tag it was to no longer carry.
    NotTagged(String),
    /// It is to be reopened, and these items, which have merged, depend on it: a merged item would depend on one that
    /// has not ended.
    MergedDependants(Vec<ItemNumber>),
    /// It is to be reopened, and these items, whose works have a run of their last cell open, depend on it: they merge
    /// as the run finishes, and a merged item would depend on one that has not ended.
    MergingDependants(Vec<ItemNumber>),
    /// It is to be reopened, and is not cancelled.
    NotCancelled,
    /// It is to be reopened, and its cancel is still letting go of what its work held.
    LettingGo,
}

/// Why a change was not made: there is no item it names, what it gives is not what an item takes, or it may not be
/// made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    NoSuchItem(ItemNumber),
    /// The title it was to be given has no words.
    BlankTitle,
    /// The tag is not one an item may carry: it is empty, longer than [`TAG_CHARS`], or holds a space or a comma.
    BadTag(String),
    /// What it was to continue is no branch name or pull request number an item may name.
    BadContinues(Continues),
    /// It was to continue something, and its flow has no cell that starts a work, which would read it.
    NoStart,
    Unchangeable(Unchangeable),
}

/// A change refused: the place it was given at among those asked together, the first being 0, the item it names, and
/// why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotMade {
    pub at: usize,
    pub item: ItemNumber,
    pub why: Refused,
}

/// Whether each change makes its item other than it is, in the order given, or every change that is refused: none is
/// then made. The changes are judged in turn, each against the items as the changes before it that were not refused
/// would leave them, so that every refusal is said at once. `items` gives every item as it stands. A change that leaves
/// its item as it is, a title, a level or a tag it has, or an item it depends on already, is no change, and is not
/// refused for that; a title of no words is refused, as it is when an item is added.
pub fn judge(
    items: &BTreeMap<ItemNumber, Planned>,
    changes: &[(ItemNumber, Asked)],
) -> Result<Vec<bool>, Vec<NotMade>> {
    let mut items = items.clone();
    let mut changed = Vec::new();
    let mut refused = Vec::new();
    for (at, (item, asked)) in changes.iter().enumerate() {
        match judge_one(&items, *item, asked) {
            Ok(changes) => {
                if changes {
                    made(&mut items, *item, asked);
                }
                changed.push(changes);
            }
            Err(why) => refused.push(NotMade { at, item: *item, why }),
        }
    }
    if refused.is_empty() { Ok(changed) } else { Err(refused) }
}

/// Whether the tag is one an item may carry; why not, where it is not.
pub fn tag_fault(tag: &str) -> Option<Refused> {
    store::tag_is_bad(tag).then(|| Refused::BadTag(tag.to_string()))
}

/// Whether the item may continue this: a pull request of a number, or a branch of a name git takes for one
/// (`git check-ref-format`: no part between slashes begins with a dot or ends with `.lock`, and no space, control
/// character, `..`, `//`, `@{`, a backslash or any of ~ ^ : ? * [) and a command takes for no option.
pub fn continues_fault(continues: &Continues) -> Option<Refused> {
    let bad = match continues {
        Continues::Pull(number) => *number == 0,
        Continues::Branch(name) => {
            name.is_empty()
                || name.starts_with(['-', '/'])
                || name.ends_with(['/', '.'])
                || name.contains("..")
                || name.contains("//")
                || name.contains("@{")
                || name == "@"
                || name
                    .split('/')
                    .any(|part| part.starts_with('.') || part.ends_with(".lock"))
                || name
                    .chars()
                    .any(|c| c <= ' ' || c == '\u{7f}' || matches!(c, '~' | '^' | ':' | '?' | '*' | '[' | '\\'))
        }
    };
    bad.then(|| Refused::BadContinues(continues.clone()))
}

/// Whether the flow has a cell whose act starts a work: the act that reads what an item continues.
pub fn starts(flow: &Compiled) -> bool {
    flow.cells.iter().any(|cell| cell.act.as_deref() == Some("start"))
}

/// Whether the work merges as a run finishes: a run of the last cell of its flow is open, neither finished nor
/// abandoned.
pub fn merging(flow: &Compiled, work: &Work) -> bool {
    let records = &work.records;
    let Some(last) = flow.cells.last() else {
        return false;
    };
    records.started.iter().any(|started| {
        started.cell == last.id
            && !records.finished.iter().any(|f| f.run == started.run)
            && !records.abandoned.iter().any(|a| a.run == started.run)
    })
}

/// Whether the work has made its branch: a run of a cell whose act starts a work has handed in its return. A run that
/// failed, or was refused, has not.
pub fn placed(flow: &Compiled, work: &Work) -> bool {
    let records = &work.records;
    records.started.iter().any(|started| {
        let starts = flow
            .cells
            .iter()
            .any(|cell| cell.id == started.cell && cell.act.as_deref() == Some("start"));
        starts && records.finished.iter().any(|finished| finished.run == started.run)
    })
}

/// Whether one change makes the item other than it is, or why it is refused.
fn judge_one(items: &BTreeMap<ItemNumber, Planned>, number: ItemNumber, asked: &Asked) -> Result<bool, Refused> {
    let refused = |why| Err(Refused::Unchangeable(why));
    let Some(planned) = items.get(&number) else {
        return Err(Refused::NoSuchItem(number));
    };
    let item = &planned.item;
    if *asked == Asked::Reopen {
        if planned.ended != Some(End::Cancelled) {
            return refused(Unchangeable::NotCancelled);
        }
        if planned.letting_go {
            return refused(Unchangeable::LettingGo);
        }
        // Reopened, the item holds again those that depend on it: none that merged, or that merges as its last cell's run
        // finishes, may come to wait for it. Every other, one that has started or not, waits for it again, and so does
        // one that was cancelled, once it is reopened in its turn: its walk holds it until the item has ended.
        let dependants = |what: fn(&Planned) -> bool| -> Vec<ItemNumber> {
            (items.values())
                .filter(|other| other.item.depends_on.contains(&number) && what(other))
                .map(|other| other.item.number)
                .collect()
        };
        let merged = dependants(|other| other.ended == Some(End::Merged));
        if !merged.is_empty() {
            return refused(Unchangeable::MergedDependants(merged));
        }
        let merging = dependants(|other| other.ended.is_none() && other.merging);
        if !merging.is_empty() {
            return refused(Unchangeable::MergingDependants(merging));
        }
        return Ok(true);
    }
    if let Some(end) = planned.ended {
        return refused(Unchangeable::Ended(end));
    }
    match asked {
        Asked::Reopen | Asked::Cancel => Ok(true),
        Asked::Item(change) => match change {
            ItemChange::Title(title) if title.trim().is_empty() => Err(Refused::BlankTitle),
            ItemChange::Title(title) => Ok(*title != item.title),
            ItemChange::Level(level) => Ok(item.level.as_deref() != Some(level.as_str())),
            ItemChange::Tagged(tag) => match tag_fault(tag) {
                Some(fault) => Err(fault),
                None => Ok(!item.tags.contains(tag)),
            },
            ItemChange::Untagged(tag) if !item.tags.contains(tag) => refused(Unchangeable::NotTagged(tag.clone())),
            ItemChange::Untagged(_) => Ok(true),
            ItemChange::DependsOn(on) | ItemChange::NoLongerDependsOn(on) if !items.contains_key(on) => {
                Err(Refused::NoSuchItem(*on))
            }
            ItemChange::DependsOn(on) if item.depends_on.contains(on) => Ok(false),
            // A work that merges as its run finishes takes on no item that has not ended: an item that has ended holds
            // nothing, as a merged one does.
            ItemChange::DependsOn(on) if planned.merging && items.get(on).is_some_and(|p| p.ended.is_none()) => {
                refused(Unchangeable::Merging(*on))
            }
            ItemChange::DependsOn(on) => {
                let depends_on = |n: ItemNumber| items.get(&n).map(|p| p.item.depends_on.clone());
                match way(*on, number, &depends_on) {
                    Some(way) => refused(Unchangeable::Circle(way)),
                    None => Ok(true),
                }
            }
            ItemChange::NoLongerDependsOn(on) if !item.depends_on.contains(on) => {
                refused(Unchangeable::NotDependent(*on))
            }
            ItemChange::NoLongerDependsOn(_) => Ok(true),
            ItemChange::Continues(continues) => {
                if let Some(fault) = continues.as_ref().and_then(continues_fault) {
                    return Err(fault);
                }
                if planned.placed {
                    return refused(Unchangeable::Placed);
                }
                if continues.is_some() && !planned.starts {
                    return Err(Refused::NoStart);
                }
                Ok(item.continues != *continues)
            }
        },
    }
}

/// Makes what the change does to the items as they are judged: the item changed, its work cancelled, or reopened.
fn made(items: &mut BTreeMap<ItemNumber, Planned>, number: ItemNumber, asked: &Asked) {
    let Some(planned) = items.get_mut(&number) else {
        return;
    };
    match asked {
        Asked::Item(change) => planned.item.is(change),
        // Cancelled, its work lets go of what it held, now or once what it has under way has ended: a reopen asked
        // with the cancel would come while it lets go.
        Asked::Cancel => {
            planned.ended = Some(End::Cancelled);
            planned.letting_go = true;
        }
        Asked::Reopen => planned.ended = None,
    }
}

/// A way from one item to another through what each depends on, both included, where there is one: the first found,
/// taking what each depends on in the order of their numbers.
fn way(
    from: ItemNumber,
    to: ItemNumber,
    depends_on: &impl Fn(ItemNumber) -> Option<Vec<ItemNumber>>,
) -> Option<Vec<ItemNumber>> {
    let mut seen = BTreeSet::new();
    let mut way = vec![from];
    // Each item on the way, with the items it depends on that are still to be tried.
    let mut left = vec![reversed(depends_on(from))];
    // Each time round takes an item from the list at the end, and adds a list only for an item not seen before, or
    // takes away an empty list: it ends, at the latest once every list of every item reached is empty.
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
mod tests;
