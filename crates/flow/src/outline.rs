//! What is kept of a version of a flow beside its files, for reading a work on it once another version is installed:
//! its outline, made from the compiled flow as the engine starts with the version, and never compiled again (decision
//! `a-version-is-kept-as-its-files-and-its-outline`); and how what a work recorded on one version reads on a later one,
//! through the table of earlier ids (decision `the-table-of-earlier-ids-is-a-list-of-renumberings`).
//!
//! An outline's parts are only added: each part a later version adds is one an outline kept before may lack, and reads
//! as absent there; none is renamed or retyped.

use std::collections::{BTreeMap, BTreeSet};

use decide::{BlockId, CellId, Reading};
use serde::{Deserialize, Serialize};

use crate::compile::Compiled;
use crate::source::{InputKind, Kind, Listed, OWNER};

/// A renumbering of the table of earlier ids: the cells whose id changed then, by the id each had, with the id it took,
/// and the cells taken out then, by the id each had.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Renumbering {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ids: BTreeMap<CellId, CellId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub taken_out: Vec<CellId>,
}

/// What stands for the id of a cell, in small letters, in the name of a datum each cell has its own of.
pub const CELL: &str = "{cell}";

/// What reading a work on a version of a flow needs of it, kept with the version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outline {
    /// The flow's name.
    pub name: String,
    /// The cells, in the order of the walk.
    pub cells: Vec<OutlinedCell>,
    /// The lines, by name.
    #[serde(default)]
    pub lines: Vec<OutlinedLine>,
    /// The groups of the flow's blocks, by name, each with its blocks as the flow names them.
    #[serde(default)]
    pub groups: Vec<OutlinedGroup>,
    /// What the item's card shows the owner of the work, in the order the flow names them.
    #[serde(default)]
    pub card: Vec<BlockId>,
    /// The table of earlier ids, oldest first.
    #[serde(default)]
    pub earlier_ids: Vec<Renumbering>,
    /// The name each file of `../data/of_a_cell/` writes, [`CELL`] in it standing for the id of the cell, in small
    /// letters, whose own the datum is.
    #[serde(default)]
    pub own: Vec<String>,
    /// Each datum written from outside the work, with where it comes from, as its file's `from` says: the owner
    /// ([`OWNER`]), or the source outside the work that writes it.
    #[serde(default)]
    pub from: BTreeMap<BlockId, String>,
}

/// A cell of a version, as reading a work on it needs it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutlinedCell {
    pub id: CellId,
    /// What the cell is called on the chart.
    pub title: String,
    /// The line an agent's cell runs in; an engine's cell has none.
    pub line: Option<String>,
    /// An agent runs it; else the engine's own act does.
    pub agent: bool,
    /// Each block it is given, in the order it is given them.
    #[serde(default)]
    pub given: Vec<OutlinedGiven>,
    /// What runs it: each datum or group its file watches, in order.
    #[serde(default)]
    pub watches: Vec<OutlinedWatch>,
    /// What its own file reads, each a datum or a group, in order.
    #[serde(default)]
    pub reads: Vec<OutlinedRead>,
    /// The cell's own words, the dialogue the owner speaks with it in, where it has them.
    #[serde(default)]
    pub words: Option<OutlinedWords>,
    /// The cell's own questions, of which the owner's answers to what it asks are versions, where it has them.
    #[serde(default)]
    pub questions: Option<BlockId>,
}

/// A block a cell is given, with what its session must hold of it and the list of the definition that gives it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutlinedGiven {
    pub block: BlockId,
    pub kind: InputKind,
    pub list: Listed,
}

/// A datum or a group a cell watches, by its name, with the blocks it stands for and what the cell does when it is new.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutlinedWatch {
    pub name: String,
    pub blocks: Vec<BlockId>,
    #[serde(default)]
    pub does: Option<String>,
}

/// A datum or a group a cell only reads, by its name, with the blocks it stands for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutlinedRead {
    pub name: String,
    pub blocks: Vec<BlockId>,
}

/// A cell's own words: the block, and each text the engine writes of a version of it, by its name, as its file says.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutlinedWords {
    pub block: BlockId,
    pub writes: Vec<(String, String)>,
}

/// A line of a version, by its name: an object, so that a later version can give it parts of its own.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutlinedLine {
    pub name: String,
    /// Whether its agents only read, as its file says.
    #[serde(default)]
    pub reads_only: Option<bool>,
}

/// A group of a version's blocks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutlinedGroup {
    pub name: String,
    pub blocks: Vec<BlockId>,
}

impl Outline {
    /// The outline of a compiled flow.
    pub fn of(flow: &Compiled) -> Outline {
        // The block among the cell's outputs that this source of the engine's writes.
        let own = |id: &CellId, source: &str| {
            let place = flow.flow.place(id)?;
            let outputs = &flow.flow.cells()[place].outputs;
            (outputs.iter()).find(|block| {
                (flow.blocks.get(*block)).is_some_and(|declared| declared.writers.outside.as_deref() == Some(source))
            })
        };
        let words = |id: &CellId| {
            let block = own(id, crate::compile::WORDS)?;
            Some(OutlinedWords {
                block: block.clone(),
                writes: flow.blocks[block].writes.0.clone(),
            })
        };
        let cells = (flow.cells.iter())
            .map(|spec| OutlinedCell {
                id: spec.id.clone(),
                title: spec.title.clone(),
                line: spec.line.clone(),
                agent: spec.kind == Kind::Agent,
                given: (spec.given.iter())
                    .map(|g| OutlinedGiven {
                        block: g.block.clone(),
                        kind: g.kind,
                        list: g.list,
                    })
                    .collect(),
                watches: (spec.watches.iter())
                    .map(|w| OutlinedWatch {
                        name: w.name.clone(),
                        blocks: w.blocks.clone(),
                        does: w.does.clone(),
                    })
                    .collect(),
                reads: (spec.reads.iter())
                    .map(|r| OutlinedRead {
                        name: r.name.clone(),
                        blocks: r.blocks.clone(),
                    })
                    .collect(),
                words: words(&spec.id),
                questions: own(&spec.id, crate::compile::ANSWER).cloned(),
            })
            .collect();
        Outline {
            name: flow.name.clone(),
            cells,
            lines: (flow.lines.iter())
                .map(|(name, line)| OutlinedLine {
                    name: name.clone(),
                    reads_only: Some(line.reads_only),
                })
                .collect(),
            groups: (flow.groups.iter())
                .map(|(name, blocks)| OutlinedGroup {
                    name: name.clone(),
                    blocks: blocks.iter().map(|b| BlockId(b.clone())).collect(),
                })
                .collect(),
            card: flow.card.clone(),
            earlier_ids: flow.earlier_ids.clone(),
            own: flow.own.clone(),
            from: (flow.blocks.iter())
                .filter_map(|(block, declared)| {
                    let w = &declared.writers;
                    let from = if w.people { Some(OWNER) } else { w.outside.as_deref() };
                    from.map(|from| (block.clone(), from.to_string()))
                })
                .collect(),
        }
    }

    /// The outline as the store keeps it: JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("an outline is written as JSON")
    }

    /// The outline the store kept as this JSON, or why it does not read as one.
    pub fn from_json(text: &str) -> Result<Outline, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }

    /// The last cell, whose record ends a work (decision `a-work-that-ended-stays-on-its-version`).
    pub fn last(&self) -> Option<&CellId> {
        self.cells.last().map(|cell| &cell.id)
    }

    /// The cell of this id.
    pub fn cell(&self, id: &CellId) -> Option<&OutlinedCell> {
        self.cells.iter().find(|cell| &cell.id == id)
    }

    /// The datum each file of `../data/of_a_cell/` this version names makes for the cell of this id.
    pub fn own_of(&self, cell: &CellId) -> impl Iterator<Item = BlockId> + '_ {
        let small = cell.as_str().to_lowercase();
        (self.own.iter()).map(move |written| BlockId(written.replace(CELL, &small)))
    }
}

/// The id a cell taken out at a renumbering reads as once its renumbering is passed: its own id with the number of the
/// renumbering, counted from one, after `@`, which no cell's id holds.
pub fn taken_out(id: &CellId, renumbering: usize) -> CellId {
    CellId(format!("{id}@{renumbering}"))
}

/// How what a work recorded on a version whose table of earlier ids held its first `from` renumberings reads on the
/// version `to`: through each renumbering of `to`'s table after those, in turn. A cell taken out at a renumbering reads
/// as [`taken_out`] from then on, and each datum made for a cell ([`Outline::own_of`]) reads as the one made for the id
/// its cell reads as (decision `a-datum-made-for-a-cell-follows-its-cell`).
pub fn reading(to: &Outline, from: usize) -> Reading {
    let after = to.earlier_ids.iter().enumerate().skip(from);
    let named: BTreeSet<&CellId> = (after.clone())
        .flat_map(|(_, r)| r.ids.keys().chain(&r.taken_out))
        .collect();
    let mut cells = BTreeMap::new();
    for id in named {
        let mut now = id.clone();
        for (i, renumbering) in after.clone() {
            if renumbering.taken_out.contains(&now) {
                now = taken_out(&now, i + 1);
                break;
            }
            if let Some(next) = renumbering.ids.get(&now) {
                now = next.clone();
            }
        }
        if &now != id {
            cells.insert(id.clone(), now);
        }
    }
    let blocks = (cells.iter())
        .flat_map(|(was, now)| to.own_of(was).zip(to.own_of(now)))
        .collect();
    Reading { cells, blocks }
}
