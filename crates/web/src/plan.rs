//! What whoever plans the work reads to plan it, each read answering one question, bounded, as the planner's commands
//! show it (principle `a-look-brings-back-little`): what changed in the plan since they last looked ([`PlanChanges`]),
//! the items they could let on next ([`Candidates`]), where a text is mentioned ([`Hits`]), what an item waits for and
//! what waits for it ([`Graph`]), and the tags items carry ([`Tags`]). Each is worked out of what the driver holds, and
//! decides nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use decide::{Decision, End, Pauser, Writer, decide};
use driver::{Driver, Kept};
use serde::{Deserialize, Serialize};

use crate::works::ItemChanged;

/// The most candidates an answer lists; how many more there are is counted.
pub const CANDIDATES: usize = 10;
/// The most lines of changes an answer holds; the rest come again from the mark it gives.
pub const CHANGES: usize = 30;
/// The most hits of a search an answer holds, where the asker gives no other number.
pub const HITS: usize = 20;
/// The most items a walk of dependencies shows; past it, what is left is counted.
pub const NODES: usize = 60;
/// How many characters of the text around a hit an answer shows.
const AROUND: usize = 80;

/// An item, in a row: its number, title, level, tags, how its work stands, and the cancelled items it depends on while
/// it has not ended, for which it waits for good.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub item: u64,
    pub work: u64,
    pub title: String,
    pub level: Option<String>,
    pub tags: Vec<String>,
    pub standing: Standing,
    pub on_cancelled: Vec<u64>,
}

/// How an item's work stands, for whoever plans the work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Standing {
    /// It merged.
    Merged,
    /// It was cancelled.
    Cancelled,
    /// The owner paused it, or it was added or reopened paused and not let on since.
    Paused,
    /// The engine paused it, saying why: it waits for a person.
    Held,
    /// It goes on, or waits for something of its own flow.
    Going,
    /// Its records cannot be those of a work.
    Fault,
}

impl Standing {
    fn of(kept: &Kept<'_>) -> Standing {
        match decide(&kept.flow.flow, &kept.work.records) {
            Decision::Ended(End::Merged) => Standing::Merged,
            Decision::Ended(End::Cancelled) => Standing::Cancelled,
            Decision::Fault(_) => Standing::Fault,
            _ => match decide::pause(&kept.work.records).map(|p| p.by) {
                Some(Pauser::Owner) => Standing::Paused,
                Some(Pauser::Engine) => Standing::Held,
                None => Standing::Going,
            },
        }
    }

    /// Whether the work has ended.
    pub fn ended(self) -> bool {
        matches!(self, Standing::Merged | Standing::Cancelled)
    }
}

/// Every item the driver holds, as a row, with what each depends on and what depends on each, each worked out once.
struct Plan {
    rows: BTreeMap<u64, Row>,
    /// What each item depends on.
    on: BTreeMap<u64, Vec<u64>>,
    /// What depends on each item.
    by: BTreeMap<u64, Vec<u64>>,
}

impl Plan {
    fn of(driver: &Driver) -> Plan {
        let standing: BTreeMap<u64, Standing> = (driver.works())
            .map(|kept| (kept.item.number.0, Standing::of(&kept)))
            .collect();
        let mut plan = Plan {
            rows: BTreeMap::new(),
            on: BTreeMap::new(),
            by: BTreeMap::new(),
        };
        for kept in driver.works() {
            let number = kept.item.number.0;
            let depends_on: Vec<u64> = kept.item.depends_on.iter().map(|on| on.0).collect();
            let own = standing[&number];
            let on_cancelled = match own.ended() {
                true => Vec::new(),
                false => (depends_on.iter().copied())
                    .filter(|on| standing.get(on) == Some(&Standing::Cancelled))
                    .collect(),
            };
            for on in &depends_on {
                plan.by.entry(*on).or_default().push(number);
            }
            let row = Row {
                item: number,
                work: kept.head.number.0,
                title: kept.item.title.clone(),
                level: kept.item.level.clone(),
                tags: kept.item.tags.clone(),
                standing: own,
                on_cancelled,
            };
            plan.rows.insert(number, row);
            plan.on.insert(number, depends_on);
        }
        for by in plan.by.values_mut() {
            by.sort_unstable();
        }
        plan
    }

    /// The items one step from this one the given way.
    fn next(&self, from: u64, way: Way) -> &[u64] {
        let next = match way {
            Way::Up => self.on.get(&from),
            Way::Down => self.by.get(&from),
        };
        next.map(Vec::as_slice).unwrap_or_default()
    }

    /// Every item reached from this one the given way, by way of any items.
    fn reach(&self, from: u64, way: Way) -> BTreeSet<u64> {
        let mut found = BTreeSet::new();
        let mut left = vec![from];
        while let Some(next) = left.pop() {
            for reached in self.next(next, way) {
                if found.insert(*reached) {
                    left.push(*reached);
                }
            }
        }
        found.remove(&from);
        found
    }
}

/// The items whoever plans the work could let on now, in a default order for reading, the choice being theirs: the
/// works the owner paused, or that were added or reopened paused and never let on since, every item of which they
/// depend on has merged. Those more items wait for, by way of any items, come first, then the earlier added.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidates {
    /// How many rounds of agents may go on at once, and how many are under way.
    pub slots: u64,
    pub taken: u64,
    /// Every candidate there is, listed or not.
    pub total: usize,
    /// The first [`CANDIDATES`] of them.
    pub rows: Vec<Candidate>,
}

/// An item that could be let on now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub row: Row,
    /// Whether its work has started a run: let on, it goes on from where it was paused rather than beginning.
    pub started: bool,
    /// How many items that have not ended wait for it, by way of any items.
    pub waited_by: usize,
}

impl Candidates {
    pub fn of(driver: &Driver) -> Candidates {
        let plan = Plan::of(driver);
        let started: BTreeMap<u64, bool> = (driver.works())
            .map(|kept| (kept.item.number.0, !kept.work.records.started.is_empty()))
            .collect();
        let merged = |on: &u64| plan.rows.get(on).is_some_and(|row| row.standing == Standing::Merged);
        let mut found: Vec<Candidate> = (plan.rows.values())
            .filter(|row| row.standing == Standing::Paused && plan.next(row.item, Way::Up).iter().all(merged))
            .map(|row| Candidate {
                row: row.clone(),
                started: started.get(&row.item).copied().unwrap_or(false),
                waited_by: (plan.reach(row.item, Way::Down).into_iter())
                    .filter(|n| !plan.rows[n].standing.ended())
                    .count(),
            })
            .collect();
        found.sort_by_key(|c| (std::cmp::Reverse(c.waited_by), c.row.item));
        let slots = driver.slots();
        Candidates {
            slots: slots.slots as u64,
            taken: slots.taken as u64,
            total: found.len(),
            rows: found.into_iter().take(CANDIDATES).collect(),
        }
    }
}

/// What an item waits for and what waits for it, walked to a depth: `up`, the items it depends on, each with those it
/// depends on in turn; `down`, the items that depend on it, likewise.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Graph {
    pub item: Row,
    pub up: Vec<Node>,
    pub down: Vec<Node>,
    /// How many items the walk reached past its depth or past [`NODES`], and does not show.
    pub left_out: usize,
}

/// An item reached in a walk, with those it reaches in turn.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub row: Row,
    /// Shown already higher in the walk: what it reaches is shown there.
    pub again: bool,
    pub next: Vec<Node>,
    /// How many items it reaches that are not shown under it, past the depth.
    pub beyond: usize,
}

/// Which way a walk goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Way {
    /// To what the item depends on: what it waits for.
    Up,
    /// To what depends on the item: what waits for it.
    Down,
}

impl Graph {
    /// The walk from this item, both ways where `ways` names none, to this depth; none where there is no such item.
    pub fn of(driver: &Driver, item: u64, ways: Option<Way>, depth: usize) -> Option<Graph> {
        let plan = Plan::of(driver);
        let row = plan.rows.get(&item)?;
        let mut walked = Walked {
            plan: &plan,
            shown: 0,
            left_out: 0,
        };
        let mut both = |way: Way| match ways.is_none_or(|ways| ways == way) {
            true => walked.from(item, way, depth, &mut BTreeSet::from([item])),
            false => Vec::new(),
        };
        let (up, down) = (both(Way::Up), both(Way::Down));
        Some(Graph {
            item: row.clone(),
            up,
            down,
            left_out: walked.left_out,
        })
    }
}

/// A walk under way: the plan it walks, how many items it has shown, and how many it reached and left out.
struct Walked<'a> {
    plan: &'a Plan,
    shown: usize,
    left_out: usize,
}

impl Walked<'_> {
    /// The items one step from this one the given way, each with those it reaches, to the depth left.
    fn from(&mut self, from: u64, way: Way, depth: usize, seen: &mut BTreeSet<u64>) -> Vec<Node> {
        let mut nodes = Vec::new();
        for &number in self.plan.next(from, way) {
            let Some(row) = self.plan.rows.get(&number) else {
                continue;
            };
            if self.shown >= NODES {
                self.left_out += 1;
                continue;
            }
            self.shown += 1;
            let again = !seen.insert(number);
            let (next, beyond) = match (again, depth > 1) {
                (true, _) => (Vec::new(), 0),
                (false, true) => (self.from(number, way, depth - 1, seen), 0),
                (false, false) => {
                    let beyond = self.plan.reach(number, way).len();
                    self.left_out += beyond;
                    (Vec::new(), beyond)
                }
            };
            nodes.push(Node {
                row: row.clone(),
                again,
                next,
                beyond,
            });
        }
        nodes
    }
}

/// The tags items carry, each with how many items carry it, and how many of those have not ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tags {
    pub tags: Vec<TagCount>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagCount {
    pub tag: String,
    pub items: usize,
    pub open: usize,
}

impl Tags {
    pub fn of(driver: &Driver) -> Tags {
        let mut counts: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        for row in Plan::of(driver).rows.values() {
            for tag in &row.tags {
                let count = counts.entry(tag.clone()).or_default();
                count.0 += 1;
                count.1 += usize::from(!row.standing.ended());
            }
        }
        Tags {
            tags: (counts.into_iter())
                .map(|(tag, (items, open))| TagCount { tag, items, open })
                .collect(),
        }
    }
}

/// Where a text is mentioned: the items whose title, whose blocks people wrote, as their latest versions say them, or
/// whose reasons for a change say it, ignoring case. A title comes first, then the text of a block, then a reason;
/// among each, the later added item first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hits {
    pub hits: Vec<Hit>,
    /// How many more hits there are than are listed.
    pub more: usize,
}

/// A place the text is mentioned.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub item: u64,
    pub title: String,
    pub field: Field,
    /// The text around the first mention, the mention between « and », cut to about 80 characters, `…` where cut.
    pub snippet: String,
    /// How many more times the same place mentions it.
    pub again: usize,
}

/// Where in an item a text was found.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Field {
    Title,
    /// The latest version of this block, which people write.
    Block {
        block: String,
    },
    /// Why a change was made, at this time.
    Why {
        time: u64,
    },
}

impl Hits {
    /// The hits of the text, at most `limit`; none for a text of no words.
    pub fn of(driver: &Driver, text: &str, limit: usize) -> Hits {
        let needle = text.to_lowercase();
        if needle.trim().is_empty() {
            return Hits {
                hits: Vec::new(),
                more: 0,
            };
        }
        let mut found: Vec<(u8, std::cmp::Reverse<u64>, Hit)> = Vec::new();
        for kept in driver.works() {
            let item = kept.item.number.0;
            let mut place = |rank: u8, field: Field, text: &str| {
                if let Some((snippet, again)) = snippet(text, &needle) {
                    let hit = Hit {
                        item,
                        title: kept.item.title.clone(),
                        field,
                        snippet,
                        again,
                    };
                    found.push((rank, std::cmp::Reverse(item), hit));
                }
            };
            place(0, Field::Title, &kept.item.title);
            for (block, versions) in decide::versions(&kept.work.records) {
                let people = kept.flow.blocks.get(&block).is_some_and(|b| b.writers.people);
                let Some(latest) = versions.last().filter(|v| people && v.by == Writer::Person) else {
                    continue;
                };
                if let Some(content) = kept.work.contents.get(&(block.clone(), latest.at)) {
                    place(
                        1,
                        Field::Block {
                            block: block.to_string(),
                        },
                        content,
                    );
                }
            }
            for change in &kept.item.changes {
                if let Some(why) = &change.why {
                    place(2, Field::Why { time: change.at.0 }, why);
                }
            }
            let whys = (kept.work.why_cancelled.iter()).chain(kept.work.why_reopened.iter());
            for (at, why) in whys {
                let time = kept.work.time_of(*at).map_or(0, |t| t.0);
                place(2, Field::Why { time }, why);
            }
        }
        found.sort_by_key(|(rank, item, _)| (*rank, *item));
        let more = found.len().saturating_sub(limit);
        Hits {
            hits: found.into_iter().take(limit).map(|(_, _, hit)| hit).collect(),
            more,
        }
    }
}

/// The text around the first mention of the needle, already in small letters, and how many more mentions there are;
/// none where there is none.
fn snippet(text: &str, needle: &str) -> Option<(String, usize)> {
    let lower = text.to_lowercase();
    let first = lower.find(needle)?;
    let again = lower[first + needle.len()..].matches(needle).count();
    let end = first + needle.len();
    // Lowering may change where a letter begins; where it did, the text is shown as it is lowered.
    let same = lower.len() == text.len() && text.is_char_boundary(first) && text.is_char_boundary(end);
    let text: &str = if same { text } else { &lower };
    let side = AROUND.saturating_sub(needle.chars().count()) / 2;
    let before: String = text[..first]
        .chars()
        .rev()
        .take(side)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let after: String = text[end..].chars().take(side).collect();
    // Each run of white space, a line's end among them, is one space, so that the snippet is one line.
    let flat = |s: &str| {
        let mut out = String::new();
        for c in s.chars() {
            match c.is_whitespace() {
                true if out.ends_with(' ') => {}
                true => out.push(' '),
                false => out.push(c),
            }
        }
        out
    };
    let cut_before = before.len() < first;
    let cut_after = after.len() < text.len() - end;
    let snippet = format!(
        "{}{}«{}»{}{}",
        if cut_before { "…" } else { "" },
        flat(&before),
        &text[first..end],
        flat(&after),
        if cut_after { "…" } else { "" },
    );
    Some((snippet, again))
}

/// What changed in the plan since a mark: each item added, each change of an item, each cancel and each reopen, with
/// when and why, oldest first; at most [`CHANGES`], the rest coming again from the mark given.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanChanges {
    pub lines: Vec<PlanLine>,
    pub left_out: usize,
    /// The mark to give next time.
    pub mark: String,
}

/// One change of the plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanLine {
    pub item: u64,
    pub title: String,
    pub time: u64,
    pub why: Option<String>,
    pub what: PlanChange,
}

/// What a change of the plan did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum PlanChange {
    /// The item was added so.
    Added {
        level: Option<String>,
        depends_on: Vec<u64>,
        tags: Vec<String>,
    },
    /// A change of the item itself.
    Changed {
        change: ItemChanged,
    },
    Cancelled,
    Reopened,
}

/// A place in the plan: the highest item read from its adding on, and for each item, how many of its changes and how
/// many of its cancels and reopens were read. An item up to the highest that is not named had none read; one after it
/// that is named was read from its adding, and one after it that is not is unread whole. Written as the highest, then
/// each item named with its two counts, such as `12,3.2.0,14.0.1`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanMark {
    highest: u64,
    read: BTreeMap<u64, (usize, usize)>,
}

/// Why a text is no mark of the plan, in words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unreadable(pub String);

impl PlanMark {
    /// Reads a mark as [`PlanMark`]'s text writes it.
    pub fn read(text: &str) -> Result<PlanMark, Unreadable> {
        let unreadable = || {
            Unreadable(format!(
                "{text:?} is not a mark of the plan: a mark is a number, then each item with two counts, such as \
                 12,3.2.0,14.0.1"
            ))
        };
        let number = |part: &str| -> Result<u64, Unreadable> {
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return Err(unreadable());
            }
            part.parse().map_err(|_| unreadable())
        };
        let mut parts = text.trim().split(',');
        let highest = number(parts.next().unwrap_or_default())?;
        let mut read = BTreeMap::new();
        for part in parts {
            let mut counts = part.split('.');
            let (Some(item), Some(changes), Some(ends), None) =
                (counts.next(), counts.next(), counts.next(), counts.next())
            else {
                return Err(unreadable());
            };
            let item = number(item)?;
            let counts = (number(changes)? as usize, number(ends)? as usize);
            if read.insert(item, counts).is_some() {
                return Err(unreadable());
            }
        }
        Ok(PlanMark { highest, read })
    }

    /// Whether the item's adding was read.
    fn known(&self, item: u64) -> bool {
        item <= self.highest || self.read.contains_key(&item)
    }
}

impl fmt::Display for PlanMark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.highest)?;
        for (item, (changes, ends)) in &self.read {
            // Up to the highest, an item of which nothing more was read than its adding need not be named.
            if *item > self.highest || (*changes, *ends) != (0, 0) {
                write!(f, ",{item}.{changes}.{ends}")?;
            }
        }
        Ok(())
    }
}

/// What is still to be given of one item, in one of its own orders: its adding, its changes, or its cancels and
/// reopens. Each is given from its first, so that what is given of each is how many of it, whatever the times say.
struct Queue {
    item: u64,
    /// 0 for the adding, 1 for the changes, 2 for the cancels and reopens.
    kind: u8,
    /// What is left of it, the next first: its time and its line.
    left: std::collections::VecDeque<(u64, PlanLine)>,
}

impl PlanChanges {
    /// What changed since the mark, and the mark to give next time; with none, since the plan began. Refused where the
    /// mark names an item the plan has not, or says more was read of an item than it has, as a mark of another store
    /// would. The lines given are the oldest of what is left, each item's changes and its cancels and reopens given in
    /// their own orders, each from where the mark left it: a clock that went back changes which come first, and never
    /// which were given.
    pub fn of(driver: &Driver, since: Option<&PlanMark>) -> Result<PlanChanges, Unreadable> {
        let since = since.cloned().unwrap_or_default();
        let held: BTreeSet<u64> = driver.works().map(|kept| kept.item.number.0).collect();
        let named = std::iter::once(since.highest).chain(since.read.keys().copied());
        if let Some(missing) = named.filter(|n| *n > 0).find(|n| !held.contains(n)) {
            return Err(Unreadable(format!(
                "the mark names #{missing}, and the plan has no such item: it is no mark of this plan"
            )));
        }
        let mut queues: Vec<Queue> = Vec::new();
        for kept in driver.works() {
            let number = kept.item.number.0;
            let r = &kept.work.records;
            let read = since.read.get(&number).copied().unwrap_or((0, 0));
            if read.0 > kept.item.changes.len() || read.1 > r.cancels.len() + r.reopens.len() {
                return Err(Unreadable(format!(
                    "the mark says more was read of #{number} than it has: it is no mark of this plan"
                )));
            }
            let line = |time: u64, why: Option<String>, what: PlanChange| PlanLine {
                item: number,
                title: kept.item.title.clone(),
                time,
                why,
                what,
            };
            let mut adding = std::collections::VecDeque::new();
            if !since.known(number) {
                let first = &kept.item.first;
                let added = PlanChange::Added {
                    level: first.level.clone(),
                    depends_on: first.depends_on.iter().map(|on| on.0).collect(),
                    tags: first.tags.clone(),
                };
                adding.push_back((kept.item.added.0, line(kept.item.added.0, None, added)));
            }
            let changes = (kept.item.changes.iter().skip(read.0))
                .map(|change| {
                    let what = PlanChange::Changed {
                        change: ItemChanged::of(&change.what),
                    };
                    (change.at.0, line(change.at.0, change.why.clone(), what))
                })
                .collect();
            let mut ends: Vec<(decide::Seq, bool)> = (r.cancels.iter().map(|at| (*at, true)))
                .chain(r.reopens.iter().map(|at| (*at, false)))
                .collect();
            ends.sort();
            let ends = (ends.into_iter().skip(read.1))
                .map(|(at, cancel)| {
                    let time = kept.work.time_of(at).map_or(0, |t| t.0);
                    let (why, what) = match cancel {
                        true => (kept.work.why_cancelled.get(&at).cloned(), PlanChange::Cancelled),
                        false => (kept.work.why_reopened.get(&at).cloned(), PlanChange::Reopened),
                    };
                    (time, line(time, why, what))
                })
                .collect();
            for (kind, left) in [(0, adding), (1, changes), (2, ends)] {
                queues.push(Queue {
                    item: number,
                    kind,
                    left,
                });
            }
        }
        // The oldest head of every queue, an item's changes and its cancels and reopens only once its adding is given.
        let mut mark = since.clone();
        let mut given = Vec::new();
        while given.len() < CHANGES {
            let next = (queues.iter().enumerate())
                .filter(|(_, q)| q.kind == 0 || mark.known(q.item))
                .filter_map(|(at, q)| q.left.front().map(|(time, _)| (*time, q.item, q.kind, at)))
                .min();
            let Some((_, item, kind, at)) = next else {
                break;
            };
            let (_, line) = queues[at].left.pop_front().expect("the head just found");
            let counts = mark.read.entry(item).or_insert((0, 0));
            match kind {
                0 => {}
                1 => counts.0 += 1,
                _ => counts.1 += 1,
            }
            given.push(line);
        }
        // The highest grows over every item next to it that is known, which then need not be named.
        while mark.read.contains_key(&(mark.highest + 1)) {
            mark.highest += 1;
        }
        let left_out = queues.iter().map(|q| q.left.len()).sum();
        Ok(PlanChanges {
            lines: given,
            left_out,
            mark: mark.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mark_is_read_as_it_is_written_and_what_is_no_mark_is_refused() {
        for text in ["0", "12", "12,3.2.0,7.5.1", "12,3.2.0,14.0.1", "3,5.0.0"] {
            assert_eq!(PlanMark::read(text).map(|m| m.to_string()), Ok(text.to_string()));
        }
        for text in ["", "x", "3,", "3,2.1", "3,2.1.0.4", "3,2.1.0,2.0.1", "3,2.a.0"] {
            assert!(PlanMark::read(text).is_err(), "{text:?} was read as a mark");
        }
        // An item past the highest that is named was read from its adding; one not named, not at all.
        let mark = PlanMark::read("3,5.0.0").expect("a mark");
        assert!(mark.known(2) && mark.known(5) && !mark.known(4));
        // Up to the highest, an item read no further than its adding is not named.
        assert_eq!(PlanMark::read("3,2.0.0").expect("a mark").to_string(), "3");
    }

    #[test]
    fn a_snippet_marks_the_first_mention_cuts_the_text_around_it_and_counts_the_rest() {
        assert_eq!(
            snippet("Parse the Header and the header", "header"),
            Some(("Parse the «Header» and the header".to_string(), 1))
        );
        assert_eq!(snippet("nothing here", "header"), None);
        let long = format!("{}header{}", "a".repeat(100), "b\n\nc".repeat(40));
        let (cut, again) = snippet(&long, "header").expect("a mention");
        assert_eq!(again, 0);
        assert!(cut.starts_with('…') && cut.ends_with('…'), "{cut}");
        assert!(cut.chars().count() <= AROUND + 4, "{cut}");
        assert!(!cut.contains('\n'), "{cut}");
    }
}
