//! The named tests: each a function of what it is shown of a work.

use std::collections::{BTreeMap, BTreeSet};

use decide::{BlockId, Outcome, TestName, Who};
use flow::Compiled;
use store::{Item, ItemNumber};
use work::Work;

/// What a named test may read: the work as it is at the walk that asks, the flow it runs on, its item, and what the
/// driver knows of the other items. It is shown no run: whether a cell is running, at rest or finished is the walk's
/// to know (entry `conditions-and-preconditions`).
pub struct Seen<'a> {
    pub flow: &'a Compiled,
    pub work: &'a Work,
    /// The item the work is done for.
    pub item: &'a Item,
    /// The items that have a work that merged.
    pub merged: &'a BTreeSet<ItemNumber>,
}

impl Seen<'_> {
    /// The value the block has now; none where it was never written, or is no choice.
    pub fn value(&self, block: &BlockId) -> Option<String> {
        let mut versions = decide::versions(&self.work.records);
        versions.remove(block)?.pop()?.value
    }

    /// The content of the block's current version; none where it was never written.
    pub fn content(&self, block: &BlockId) -> Option<&str> {
        let versions = decide::versions(&self.work.records);
        let at = versions.get(block)?.last()?.at;
        self.work.contents.get(&(block.clone(), at)).map(String::as_str)
    }
}

/// A named test: a function in code (entry `engine-knows-no-cell`).
pub type Test = fn(&Seen<'_>) -> Outcome;

/// The named tests the driver has, by the names the flows use.
pub type Tests = BTreeMap<TestName, Test>;

/// Every item the work's item depends on has a work that merged. Who can make it hold is the owner, who orders the
/// items; the works of those items make it hold by merging.
pub fn dependencies_merged(seen: &Seen<'_>) -> Outcome {
    if seen.item.depends_on.iter().all(|on| seen.merged.contains(on)) {
        Outcome::Holds
    } else {
        Outcome::Waits { who: Who::Owner }
    }
}
