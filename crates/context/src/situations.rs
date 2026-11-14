//! Every situation an entry of an agent's cell can be in, each with a name that says it, for whoever reads what the
//! cell is sent in each: the entry, the triggers that ran it, and how its session stands with each datum of its
//! context; and, apart, each other thing a round may be given.
//!
//! An entry is `first`, where the cell has worked from nothing and its session holds nothing of it; `later`, where the
//! cell ran before in the same session, which holds every part of the message it keeps, and whose last return handed
//! in blocks; or `reopened`, where the cell ran before and its session was begun anew, or compacted, since, and holds
//! nothing of it. The triggers that ran it are each alone, those whose blocks one cell's return writes together, and
//! all of them at once; a trigger on what is written as a line's session begins anew runs no later entry.
//!
//! A datum of a trigger that ran the cell is new to it: the session lacks it or holds it, the version last worked from
//! or the latest, as the entry allows; of a group's trigger, one datum of the group at least is new. A datum of a
//! trigger that did not run the cell, or one the cell alone writes, is new to it never: unwritten on a first entry,
//! held or unwritten on a later one, and lacking, held or unwritten on one reopened. Any other datum may stand any way
//! the entry allows: unwritten, lacking, or held, and, where the cell worked from it before, held as last worked from
//! or held latest and new to the cell.
//!
//! A datum's text is the builder's from its kind and from how the session stands with it alone, so a product of every
//! datum's own states repeats the same texts: it is counted ([`combinations`]), and the situations hold every state of
//! each datum with the data in two classes, each standing alike: those of the triggers that ran the cell, and the
//! rest; a datum never new stands as the entry has it most, unwritten on a first entry, held on a later one, lacking
//! on one reopened. Besides, on a later entry run by the first trigger, with the data of what ran it held as last
//! worked from and the rest held: the result of a call of each kind, why a return was sent back, each come of taking
//! main in where the cell takes it, new to the session, and an entry after a return that handed in nothing.

use std::collections::BTreeMap;

use decide::BlockId;
use flow::agents::{Agents, Part};
use flow::{CellSpec, Compiled, Trigger};

use crate::message::{CALLS, MAIN, Situation, Stands};

/// A situation, with a name that says it, its parts joined by `-`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Named {
    pub name: String,
    pub situation: Situation,
}

/// The entries of a cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Entry {
    First,
    Later,
    Reopened,
}

/// Every state of a datum, where the cell worked from it before.
const ANY: [Stands; 5] = [
    Stands::Unwritten,
    Stands::Lacks,
    Stands::HoldsLast,
    Stands::HoldsLatest,
    Stands::Holds,
];

impl Entry {
    const ALL: [Entry; 3] = [Entry::First, Entry::Later, Entry::Reopened];

    fn name(self) -> &'static str {
        match self {
            Entry::First => "first",
            Entry::Later => "later",
            Entry::Reopened => "reopened",
        }
    }

    /// The states of a datum new to the cell.
    fn newly(self) -> &'static [Stands] {
        match self {
            Entry::First => &[Stands::Lacks, Stands::Holds],
            Entry::Later => &[Stands::Lacks, Stands::HoldsLast, Stands::HoldsLatest],
            Entry::Reopened => &[Stands::Lacks, Stands::HoldsLatest],
        }
    }

    /// The states of a datum nothing of which is new to the cell, the one it stands in most first.
    fn not_new(self) -> &'static [Stands] {
        match self {
            Entry::First => &[Stands::Unwritten],
            Entry::Later => &[Stands::Holds, Stands::Unwritten],
            Entry::Reopened => &[Stands::Lacks, Stands::Holds, Stands::Unwritten],
        }
    }

    /// Every state of a datum.
    fn any(self) -> &'static [Stands] {
        match self {
            Entry::First => &[Stands::Unwritten, Stands::Lacks, Stands::Holds],
            Entry::Later | Entry::Reopened => &ANY,
        }
    }
}

/// A set of the cell's triggers that ran it, with the name that says it.
struct Ran<'a> {
    name: String,
    triggers: Vec<&'a Trigger>,
}

/// How the cell's data stand apart, for the triggers that ran it.
struct Classes {
    /// The data of the triggers that are no group's.
    fired: Vec<BlockId>,
    /// Of each group's trigger that ran it, the group's data that are not among `fired`, one of which is new.
    groups: Vec<Vec<BlockId>>,
    /// The data never new to it.
    never: Vec<BlockId>,
    /// The rest.
    rest: Vec<BlockId>,
}

/// Every situation an entry of the agent's cell can be in, as the module says, in order: the first entry, the later and
/// the reopened, each by the sets of triggers, then the other things a round may be given. `reopened` is the source that
/// writes a block as a line's session begins anew.
pub fn situations(flow: &Compiled, words: &Agents, cell: &CellSpec, reopened: &str) -> Vec<Named> {
    let mut out = Vec::new();
    for entry in Entry::ALL {
        for ran in ran(flow, cell, entry, reopened) {
            let classes = classes(flow, cell, &ran.triggers);
            let fired: Vec<BlockId> = classes
                .fired
                .iter()
                .chain(classes.groups.iter().flatten())
                .cloned()
                .collect();
            // A class with no datum has one state: none.
            let states = |data: &[BlockId], states: &'static [Stands]| match data.is_empty() {
                true => vec![None],
                false => states.iter().copied().map(Some).collect(),
            };
            for new in states(&fired, entry.newly()) {
                for rest in states(&classes.rest, entry.any()) {
                    let mut name = vec![entry.name().to_string(), ran.name.clone()];
                    name.extend(new.map(|s| format!("ran_{}", s.name())));
                    name.extend(rest.map(|s| format!("rest_{}", s.name())));
                    let data = data(&classes, entry, new, rest);
                    out.push(Named {
                        name: name.join("-"),
                        situation: situation(words, cell, entry, &ran, data),
                    });
                }
            }
        }
    }
    out.extend(besides(flow, words, cell, reopened));
    out
}

/// How many combinations of the entry, the triggers that ran the cell and each datum's own state can happen, every
/// datum apart, as the module says.
pub fn combinations(flow: &Compiled, cell: &CellSpec, reopened: &str) -> u128 {
    let mut count: u128 = 0;
    for entry in Entry::ALL {
        for ran in ran(flow, cell, entry, reopened) {
            let classes = classes(flow, cell, &ran.triggers);
            let n = |states: &[Stands]| states.len() as u128;
            let mut product = n(entry.newly()).pow(classes.fired.len() as u32);
            product *= n(entry.not_new()).pow(classes.never.len() as u32);
            product *= n(entry.any()).pow(classes.rest.len() as u32);
            // The data of the groups' triggers, each new or not, every group holding a new one.
            let mut grouped: Vec<&BlockId> = Vec::new();
            for block in classes.groups.iter().flatten() {
                if !grouped.contains(&block) {
                    grouped.push(block);
                }
            }
            let mut ways: u128 = 0;
            for mask in 0u64..(1 << grouped.len()) {
                let is_new = |b: &BlockId| {
                    grouped
                        .iter()
                        .position(|g| *g == b)
                        .is_some_and(|i| mask & (1 << i) != 0)
                };
                if classes.groups.iter().all(|g| g.iter().any(is_new)) {
                    ways += (grouped.iter())
                        .map(|b| match is_new(b) {
                            true => n(entry.newly()),
                            false => n(entry.any()) - n(entry.newly()),
                        })
                        .product::<u128>();
                }
            }
            count += product * ways;
        }
    }
    count
}

/// How each datum stands, by its name, the classes standing as given.
fn data(classes: &Classes, entry: Entry, new: Option<Stands>, rest: Option<Stands>) -> BTreeMap<String, Stands> {
    let mut data = BTreeMap::new();
    for block in classes.fired.iter().chain(classes.groups.iter().flatten()) {
        data.insert(block.to_string(), new.unwrap_or(Stands::Lacks));
    }
    for block in &classes.never {
        data.insert(block.to_string(), entry.not_new()[0]);
    }
    for block in &classes.rest {
        data.insert(block.to_string(), rest.unwrap_or(Stands::Lacks));
    }
    data
}

/// The situation of an entry run by these triggers, its data standing so, and nothing else given.
fn situation(words: &Agents, cell: &CellSpec, entry: Entry, ran: &Ran, data: BTreeMap<String, Stands>) -> Situation {
    let kept = (words.message.parts.iter())
        .filter(|p| p.kind.is_some() && p.part != Part::Context)
        .map(|p| p.part.name().to_string());
    let later = entry == Entry::Later;
    Situation {
        holds: if later { kept.collect() } else { Vec::new() },
        after_hand_in: later,
        triggers: (cell.triggers.iter())
            .filter(|t| ran.triggers.iter().any(|r| r.name == t.name))
            .map(|t| t.name.clone())
            .collect(),
        data,
        main: None,
        results: Vec::new(),
        sent_back: false,
    }
}

/// The other things a round may be given, each alone, on a later entry run by the first trigger that runs one.
fn besides(flow: &Compiled, words: &Agents, cell: &CellSpec, reopened: &str) -> Vec<Named> {
    let entry = Entry::Later;
    let Some(ran) = ran(flow, cell, entry, reopened).into_iter().next() else {
        return Vec::new();
    };
    let classes = classes(flow, cell, &ran.triggers);
    let base = situation(
        words,
        cell,
        entry,
        &ran,
        data(&classes, entry, Some(Stands::HoldsLast), Some(Stands::Holds)),
    );
    let named = |what: String, situation: Situation| Named {
        name: format!("{}-{}-{what}", entry.name(), ran.name),
        situation,
    };
    let mut out = Vec::new();
    for key in CALLS {
        let results = vec![key.to_string()];
        out.push(named(
            format!("results_{key}"),
            Situation {
                results,
                ..base.clone()
            },
        ));
    }
    out.push(named(
        "sent_back".into(),
        Situation {
            sent_back: true,
            ..base.clone()
        },
    ));
    if cell.takes_main {
        // What came of taking main in is new to the session, which holds none of it.
        let holds: Vec<String> = (base.holds.iter())
            .filter(|h| *h != Part::Main.name())
            .cloned()
            .collect();
        for key in MAIN {
            let main = Some(key.to_string());
            let situation = Situation {
                main,
                holds: holds.clone(),
                ..base.clone()
            };
            out.push(named(format!("main_{key}"), situation));
        }
    }
    let after_hand_in = false;
    out.push(named("nothing_handed_in".into(), Situation { after_hand_in, ..base }));
    out
}

/// The blocks a trigger stands on: a group's trigger, the group's; another, its block.
fn blocks(trigger: &Trigger) -> Vec<&BlockId> {
    match trigger.group.is_empty() {
        true => vec![&trigger.block],
        false => trigger.group.iter().collect(),
    }
}

/// The sets of the cell's triggers that can run an entry, as the module says, each once, in the order the cell names
/// its triggers.
fn ran<'a>(flow: &Compiled, cell: &'a CellSpec, entry: Entry, reopened: &str) -> Vec<Ran<'a>> {
    let reopens = |t: &Trigger| {
        (blocks(t).iter()).any(|b| {
            flow.blocks
                .get(*b)
                .is_some_and(|d| d.writers.outside.as_deref() == Some(reopened))
        })
    };
    let mut sets: Vec<Ran<'a>> = (cell.triggers.iter())
        .map(|t| Ran {
            name: t.name.clone(),
            triggers: vec![t],
        })
        .collect();
    for writer in &flow.cells {
        let writes = |t: &&Trigger| {
            (blocks(t).iter()).any(|b| {
                flow.blocks
                    .get(*b)
                    .is_some_and(|d| d.writers.cells.contains(&writer.id))
            })
        };
        let together: Vec<&Trigger> = cell.triggers.iter().filter(writes).collect();
        if together.len() > 1 {
            sets.push(Ran {
                name: format!("by_{}", writer.id.as_str()),
                triggers: together,
            });
        }
    }
    if cell.triggers.len() > 1 {
        sets.push(Ran {
            name: "all".to_string(),
            triggers: cell.triggers.iter().collect(),
        });
    }
    let names = |ran: &Ran| ran.triggers.iter().map(|t| t.name.clone()).collect::<Vec<_>>();
    let mut out: Vec<Ran<'a>> = Vec::new();
    for set in sets {
        let reopening = set.triggers.iter().any(|t| reopens(t));
        if (entry == Entry::Later && reopening) || out.iter().any(|o| names(o) == names(&set)) {
            continue;
        }
        out.push(set);
    }
    out
}

/// The classes of the cell's data, for these triggers that ran it.
fn classes(flow: &Compiled, cell: &CellSpec, ran: &[&Trigger]) -> Classes {
    let given: Vec<&BlockId> = cell.given.iter().map(|g| &g.block).collect();
    let mut fired: Vec<BlockId> = Vec::new();
    let mut groups: Vec<Vec<BlockId>> = Vec::new();
    for trigger in ran {
        if trigger.group.is_empty() {
            // What the agent reads for it is new with it: what the owner said, for words to the cell.
            for block in std::iter::once(&trigger.block).chain(&trigger.read) {
                if given.contains(&block) && !fired.contains(block) {
                    fired.push(block.clone());
                }
            }
        }
    }
    for trigger in ran.iter().filter(|t| !t.group.is_empty()) {
        // A group with a datum among those new by another trigger holds a new one already.
        if trigger.group.iter().any(|b| fired.contains(b)) {
            continue;
        }
        let group: Vec<BlockId> = (trigger.group.iter()).filter(|b| given.contains(b)).cloned().collect();
        if !group.is_empty() {
            groups.push(group);
        }
    }
    let new = |b: &BlockId| fired.contains(b) || groups.iter().flatten().any(|g| g == b);
    let of_others = |b: &BlockId| {
        (cell.triggers.iter())
            .filter(|t| !ran.iter().any(|r| r.name == t.name))
            .any(|t| blocks(t).contains(&b))
    };
    let alone = |b: &BlockId| {
        flow.blocks.get(b).is_some_and(|d| {
            let w = &d.writers;
            w.cells == [cell.id.clone()] && !w.people && w.outside.is_none()
        })
    };
    let (mut never, mut rest) = (Vec::new(), Vec::new());
    for block in given.into_iter().filter(|b| !new(b)) {
        if of_others(block) || alone(block) {
            never.push(block.clone());
        } else {
            rest.push(block.clone());
        }
    }
    Classes {
        fired,
        groups,
        never,
        rest,
    }
}
