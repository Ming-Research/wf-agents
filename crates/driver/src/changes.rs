//! Changing items after they were added: whether changes may be made is judged from the items, how far each one's work
//! has gone and what each depends on, and nothing else (principle `decide-purely`), so that the driver, which holds
//! them, and a test give it the same answer. Changes asked together are judged in the order given, each against the
//! items as the changes before it would leave them, and are made all of them or none ([`judge`]). A change of an item
//! is kept as a record of its own after the item's others, and what the item stands as is worked out of them (crate
//! `store`, [`store::Item`]); a cancel and a reopen are the work's own records.
//!
//! Every change but a reopen is refused once the item's work has ended: nothing of the work goes on, and the item is
//! what it ended as. Its title, level and tags change until then; what it depends on only while its work has started
//! no run, and what it continues only until its work's branch is made (decision
//! `dependencies-until-the-work-starts`). A cancel takes away no dependency on the item, and an item may come to
//! depend on a cancelled one: a dependency on a cancelled item holds as one on a merged item does (decision
//! `a-cancelled-item-keeps-its-place`). Two relations between items hold whatever is changed: an item that has merged
//! depends on no item that has not ended, and a reopen makes no work wait that has started and not ended.

use std::collections::{BTreeMap, BTreeSet};

use decide::End;
use flow::Compiled;
use store::{Continues, Item, ItemChange, ItemNumber};
use work::Work;

/// The longest a tag may be, in characters.
pub const TAG_CHARS: usize = 40;

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

/// An item as changes of it are judged: the item, how far its work has gone, and whether its end is letting go of what
/// it held.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned {
    pub item: Item,
    /// Whether its work has started a run.
    pub started: bool,
    /// How its work ended, where it has.
    pub ended: Option<End>,
    /// Whether its work's end is letting go of what it held, which a reopen would leave half done.
    pub letting_go: bool,
    /// Whether its work has made its branch: the cell of its flow that starts a work has handed in a return.
    pub placed: bool,
    /// Whether its flow has a cell that starts a work, which is what reads what an item continues.
    pub starts: bool,
}

impl Planned {
    fn gone(&self) -> Gone {
        match self.ended {
            Some(end) => Gone::Ended(end),
            None if self.started => Gone::Started,
            None => Gone::NotStarted,
        }
    }
}

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
    /// It is to be reopened, and these items, whose works have started a run and have not ended, depend on it: a work
    /// goes on from what it depended on as it started, and one would come to wait for the item midway, or merge while
    /// it had not ended.
    StartedDependants(Vec<ItemNumber>),
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
    let bad = tag.is_empty() || tag.chars().count() > TAG_CHARS || tag.chars().any(|c| c.is_whitespace() || c == ',');
    bad.then(|| Refused::BadTag(tag.to_string()))
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
    let gone = planned.gone();
    if *asked == Asked::Reopen {
        if planned.ended != Some(End::Cancelled) {
            return refused(Unchangeable::NotCancelled);
        }
        if planned.letting_go {
            return refused(Unchangeable::LettingGo);
        }
        // Reopened, the item holds again those that depend on it: none that merged, or that has started and not ended,
        // may come to wait for it. One whose work has started no run waits for it again, and so does one that was
        // cancelled, once it is reopened in its turn: its walk holds it at the cell that waits for its dependencies.
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
        let started = dependants(|other| other.gone() == Gone::Started);
        if !started.is_empty() {
            return refused(Unchangeable::StartedDependants(started));
        }
        return Ok(true);
    }
    if let Gone::Ended(end) = gone {
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
            ItemChange::DependsOn(_) | ItemChange::NoLongerDependsOn(_) if gone == Gone::Started => {
                refused(Unchangeable::Started)
            }
            ItemChange::DependsOn(on) if item.depends_on.contains(on) => Ok(false),
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
