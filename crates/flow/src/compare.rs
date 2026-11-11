//! What a change to a flow's definition changes: two compiled definitions compared, each difference given as data.
//!
//! The owner approves every change to the flow, and reads it as a list of what changed, each with where it is and what
//! it was and is. The list is a function of the two definitions alone (principle `decide-purely`), in an order that
//! does not depend on how either was written: the flow's own things first (its name, its limits, its named tests),
//! then the cells in the order of the new definition, each cell taken out standing after the cell it followed (of those
//! that stay where they were), then the blocks by name.

use std::collections::{BTreeMap, BTreeSet};

use decide::{BlockId, Cell, CellId, Condition, Test, TestName};

use crate::compile::{Block, CellSpec, Compiled, Limits};
use crate::source::{Effort, Kind};

/// One difference between two definitions of a flow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// The flow's name.
    Name { before: String, after: String },
    /// A limit of the flow as a whole, which each cell that sets none of its own has.
    Limit { limit: Limit, before: u32, after: u32 },
    /// A named test was declared, with the words shown for it.
    TestAdded { test: TestName, words: String },
    /// A named test is no longer declared.
    TestRemoved { test: TestName, words: String },
    /// The words shown for a named test.
    TestReworded {
        test: TestName,
        before: String,
        after: String,
    },
    /// What a named test holds on: its kind, the blocks it reads or the value it tests for, as each definition has it;
    /// a change of its words is said apart, as a rewording.
    TestChanged { test: TestName, before: Test, after: Test },
    /// A cell was put into the order, at this place (counted from 0).
    CellAdded { cell: CellId, place: usize },
    /// A cell was taken out of the order, from this place (counted from 0).
    CellRemoved { cell: CellId, place: usize },
    /// A cell stands elsewhere among the cells both definitions have: not one that only shifted because cells were
    /// put in or taken out before it, nor one that stays among the most cells that keep their order.
    CellMoved { cell: CellId, before: usize, after: usize },
    /// Something of a cell that both definitions have.
    Cell { cell: CellId, change: CellChange },
    /// A block was declared.
    BlockAdded { block: BlockId },
    /// A block is no longer declared.
    BlockRemoved { block: BlockId },
    /// Something of a block that both definitions declare. Who of the cells and the tests writes and reads it is not
    /// here: that is a change of those cells' outputs, inputs or conditions, or of what those tests read.
    Block { block: BlockId, change: BlockChange },
}

/// The limits of what repeats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Limit {
    /// How many runs of a cell in a row may be ways round.
    Runs,
    /// How often a return may be sent back within one run.
    Returns,
    /// How many calls one run may make.
    Calls,
}

/// What changed of a cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CellChange {
    Title {
        before: String,
        after: String,
    },
    Kind {
        before: Kind,
        after: Kind,
    },
    Line {
        before: Option<String>,
        after: Option<String>,
    },
    /// Whether the agents of the cell's line only read, as its line says.
    ReadsOnly {
        before: bool,
        after: bool,
    },
    /// What an agent cell's agent is told to do; none for an engine cell.
    Task {
        before: Option<String>,
        after: Option<String>,
    },
    /// The act of the engine's own that carries out an engine cell; none for an agent cell.
    Act {
        before: Option<String>,
        after: Option<String>,
    },
    /// When the cell runs at all; none is always.
    Condition {
        before: Option<Condition>,
        after: Option<Condition>,
    },
    PreconditionAdded(TestName),
    PreconditionRemoved(TestName),
    InputAdded(BlockId),
    InputRemoved(BlockId),
    OutputAdded(BlockId),
    OutputRemoved(BlockId),
    /// A block every hand-in of the cell must now include.
    MustWriteAdded(BlockId),
    /// A block every hand-in of the cell no longer needs to include.
    MustWriteRemoved(BlockId),
    /// A block the cell now says what the work is by, where the host shows it.
    DescribesAdded(BlockId),
    /// A block the cell no longer says what the work is by.
    DescribesRemoved(BlockId),
    /// The block among the outputs of the check before the merge in which it writes the files the work changes under
    /// the paths the installation names for the owner's yes; none where it writes them nowhere.
    OwnerPaths {
        before: Option<BlockId>,
        after: Option<BlockId>,
    },
    /// A limit of the cell, other than by following the same change of the flow's limit, which is said once, as the
    /// flow's.
    Limit {
        limit: Limit,
        before: u32,
        after: u32,
    },
    /// The reasoning effort an agent cell's rounds run at; an engine cell has none.
    Effort {
        before: Option<Effort>,
        after: Option<Effort>,
    },
    /// How many levels above its item's the cell's rounds run at, as its line says.
    LevelsAbove {
        before: u32,
        after: u32,
    },
}

/// What changed of a block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockChange {
    /// The values of a choice; none is a block that is not one.
    Values {
        before: Option<Vec<String>>,
        after: Option<Vec<String>>,
    },
    /// Whether people write it.
    People { before: bool, after: bool },
    /// Whether what people write in it is a note, their words to the cells that read it.
    Note { before: bool, after: bool },
    /// The source outside the work that writes it; none where the outside does not.
    Outside {
        before: Option<String>,
        after: Option<String>,
    },
    /// Whether the owner's page reads it.
    Shown { before: bool, after: bool },
}

/// Every difference from `before` to `after`, in the order the module says.
pub fn compare(before: &Compiled, after: &Compiled) -> Vec<Change> {
    let mut changes = Vec::new();
    if before.name != after.name {
        changes.push(Change::Name {
            before: before.name.clone(),
            after: after.name.clone(),
        });
    }
    for (limit, b, a) in limits(&before.limits, &after.limits) {
        if b != a {
            changes.push(Change::Limit {
                limit,
                before: b,
                after: a,
            });
        }
    }
    tests(before, after, &mut changes);
    cells(before, after, &mut changes);
    blocks(&before.blocks, &after.blocks, &mut changes);
    changes
}

/// Each limit with its value in both, in the order runs, returns, calls.
fn limits(before: &Limits, after: &Limits) -> [(Limit, u32, u32); 3] {
    [
        (Limit::Runs, before.runs, after.runs),
        (Limit::Returns, before.returns, after.returns),
        (Limit::Calls, before.calls, after.calls),
    ]
}

/// Each test added, removed or changed, in the order of their names; of a test both definitions declare, its words
/// first, then what it holds on.
fn tests(before: &Compiled, after: &Compiled, changes: &mut Vec<Change>) {
    let names: BTreeSet<&TestName> = before.tests.keys().chain(after.tests.keys()).collect();
    for test in names {
        match (before.tests.get(test), after.tests.get(test)) {
            (None, Some(words)) => changes.push(Change::TestAdded {
                test: test.clone(),
                words: words.clone(),
            }),
            (Some(words), None) => changes.push(Change::TestRemoved {
                test: test.clone(),
                words: words.clone(),
            }),
            (Some(b), Some(a)) => {
                if b != a {
                    changes.push(Change::TestReworded {
                        test: test.clone(),
                        before: b.clone(),
                        after: a.clone(),
                    });
                }
                let (b, a) = (&before.flow.tests()[test], &after.flow.tests()[test]);
                if b != a {
                    changes.push(Change::TestChanged {
                        test: test.clone(),
                        before: b.clone(),
                        after: a.clone(),
                    });
                }
            }
            (None, None) => {}
        }
    }
}

/// A cell of a definition: what the walk reads of it and what the rest of the engine does, at its place.
struct Placed<'a> {
    place: usize,
    cell: &'a Cell,
    spec: &'a CellSpec,
}

fn placed(compiled: &Compiled) -> BTreeMap<&CellId, Placed<'_>> {
    compiled
        .flow
        .cells()
        .iter()
        .zip(&compiled.cells)
        .enumerate()
        .map(|(place, (cell, spec))| (&cell.id, Placed { place, cell, spec }))
        .collect()
}

fn cells(before: &Compiled, after: &Compiled, changes: &mut Vec<Change>) {
    let old = placed(before);
    let new = placed(after);
    let before_order: Vec<&CellId> = before.flow.cells().iter().map(|c| &c.id).collect();
    let after_order: Vec<&CellId> = after.flow.cells().iter().map(|c| &c.id).collect();
    let moved = moved(&before_order, &after_order, &old, &new);
    let flow_limits = limits(&before.limits, &after.limits);

    for id in merged(&before_order, &after_order, &moved) {
        match (old.get(id), new.get(id)) {
            (None, Some(a)) => changes.push(Change::CellAdded {
                cell: id.clone(),
                place: a.place,
            }),
            (Some(b), None) => changes.push(Change::CellRemoved {
                cell: id.clone(),
                place: b.place,
            }),
            (Some(b), Some(a)) => {
                if moved.contains(id) {
                    changes.push(Change::CellMoved {
                        cell: id.clone(),
                        before: b.place,
                        after: a.place,
                    });
                }
                for change in cell_changes(b, a, &flow_limits) {
                    changes.push(Change::Cell {
                        cell: id.clone(),
                        change,
                    });
                }
            }
            (None, None) => {}
        }
    }
}

/// The cells of both orders: those of `after` in its order, each cell only `before` has standing after the nearest
/// cell before it there that `after` has too and that did not move, or first when there is none.
fn merged<'a>(before: &[&'a CellId], after: &[&'a CellId], moved: &BTreeSet<&CellId>) -> Vec<&'a CellId> {
    let kept: BTreeSet<&CellId> = after.iter().copied().collect();
    let mut first = Vec::new();
    let mut following: BTreeMap<&CellId, Vec<&CellId>> = BTreeMap::new();
    let mut anchor = None;
    for &id in before {
        if kept.contains(id) {
            if !moved.contains(id) {
                anchor = Some(id);
            }
        } else if let Some(a) = anchor {
            following.entry(a).or_default().push(id);
        } else {
            first.push(id);
        }
    }
    let mut all = first;
    for &id in after {
        all.push(id);
        all.extend(following.remove(id).unwrap_or_default());
    }
    all
}

/// The cells both definitions have that moved: those left out of the most cells that keep their order between the
/// two. Of the ways to keep the most, the one taken keeps the cells that come first in the new order.
fn moved<'a>(
    before: &[&'a CellId],
    after: &[&'a CellId],
    old: &BTreeMap<&CellId, Placed<'_>>,
    new: &BTreeMap<&CellId, Placed<'_>>,
) -> BTreeSet<&'a CellId> {
    let a: Vec<&CellId> = before.iter().copied().filter(|id| new.contains_key(id)).collect();
    let b: Vec<&CellId> = after.iter().copied().filter(|id| old.contains_key(id)).collect();
    // longest[i][j]: how many cells keep their order between a[i..] and b[j..].
    let mut longest = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            longest[i][j] = if a[i] == b[j] {
                longest[i + 1][j + 1] + 1
            } else {
                longest[i + 1][j].max(longest[i][j + 1])
            };
        }
    }
    let mut kept = BTreeSet::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            kept.insert(a[i]);
            i += 1;
            j += 1;
        } else if longest[i + 1][j] >= longest[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    b.into_iter().filter(|id| !kept.contains(id)).collect()
}

fn cell_changes(before: &Placed, after: &Placed, flow_limits: &[(Limit, u32, u32); 3]) -> Vec<CellChange> {
    let (b, a) = (before.spec, after.spec);
    let mut changes = Vec::new();
    if b.title != a.title {
        changes.push(CellChange::Title {
            before: b.title.clone(),
            after: a.title.clone(),
        });
    }
    if b.kind != a.kind {
        changes.push(CellChange::Kind {
            before: b.kind,
            after: a.kind,
        });
    }
    if b.line != a.line {
        changes.push(CellChange::Line {
            before: b.line.clone(),
            after: a.line.clone(),
        });
    }
    if b.reads_only != a.reads_only {
        changes.push(CellChange::ReadsOnly {
            before: b.reads_only,
            after: a.reads_only,
        });
    }
    if b.task != a.task {
        changes.push(CellChange::Task {
            before: b.task.clone(),
            after: a.task.clone(),
        });
    }
    if b.act != a.act {
        changes.push(CellChange::Act {
            before: b.act.clone(),
            after: a.act.clone(),
        });
    }
    let (b_cell, a_cell) = (before.cell, after.cell);
    if b_cell.applies != a_cell.applies {
        changes.push(CellChange::Condition {
            before: b_cell.applies.clone(),
            after: a_cell.applies.clone(),
        });
    }
    list(
        &b_cell.requires,
        &a_cell.requires,
        CellChange::PreconditionRemoved,
        CellChange::PreconditionAdded,
        &mut changes,
    );
    list(
        &b_cell.inputs,
        &a_cell.inputs,
        CellChange::InputRemoved,
        CellChange::InputAdded,
        &mut changes,
    );
    list(
        &b_cell.outputs,
        &a_cell.outputs,
        CellChange::OutputRemoved,
        CellChange::OutputAdded,
        &mut changes,
    );
    list(
        &b.must_write,
        &a.must_write,
        CellChange::MustWriteRemoved,
        CellChange::MustWriteAdded,
        &mut changes,
    );
    list(
        &b.describes,
        &a.describes,
        CellChange::DescribesRemoved,
        CellChange::DescribesAdded,
        &mut changes,
    );
    if b.owner_paths != a.owner_paths {
        changes.push(CellChange::OwnerPaths {
            before: b.owner_paths.clone(),
            after: a.owner_paths.clone(),
        });
    }
    let own = [
        (Limit::Runs, b_cell.limit, a_cell.limit),
        (Limit::Returns, b.returns, a.returns),
        (Limit::Calls, b.calls, a.calls),
    ];
    for ((limit, before, after), &(_, flow_before, flow_after)) in own.into_iter().zip(flow_limits) {
        let followed_the_flow = before == flow_before && after == flow_after;
        if before != after && !followed_the_flow {
            changes.push(CellChange::Limit { limit, before, after });
        }
    }
    if b.effort != a.effort {
        changes.push(CellChange::Effort {
            before: b.effort,
            after: a.effort,
        });
    }
    if b.levels_above != a.levels_above {
        changes.push(CellChange::LevelsAbove {
            before: b.levels_above,
            after: a.levels_above,
        });
    }
    changes
}

/// The names taken out of a list, in its old order, then those put in, in its new order.
fn list<T: Clone + PartialEq>(
    before: &[T],
    after: &[T],
    removed: impl Fn(T) -> CellChange,
    added: impl Fn(T) -> CellChange,
    changes: &mut Vec<CellChange>,
) {
    changes.extend(before.iter().filter(|x| !after.contains(x)).cloned().map(removed));
    changes.extend(after.iter().filter(|x| !before.contains(x)).cloned().map(added));
}

fn blocks(before: &BTreeMap<BlockId, Block>, after: &BTreeMap<BlockId, Block>, changes: &mut Vec<Change>) {
    let names: BTreeSet<&BlockId> = before.keys().chain(after.keys()).collect();
    for id in names {
        let (b, a) = match (before.get(id), after.get(id)) {
            (None, Some(_)) => {
                changes.push(Change::BlockAdded { block: id.clone() });
                continue;
            }
            (Some(_), None) => {
                changes.push(Change::BlockRemoved { block: id.clone() });
                continue;
            }
            (Some(b), Some(a)) => (b, a),
            (None, None) => continue,
        };
        let mut found = Vec::new();
        if b.values != a.values {
            found.push(BlockChange::Values {
                before: b.values.clone(),
                after: a.values.clone(),
            });
        }
        let (before, after) = (b.writers.people, a.writers.people);
        if before != after {
            found.push(BlockChange::People { before, after });
        }
        let (before, after) = (b.writers.note, a.writers.note);
        if before != after {
            found.push(BlockChange::Note { before, after });
        }
        let (before, after) = (&b.writers.outside, &a.writers.outside);
        if before != after {
            found.push(BlockChange::Outside {
                before: before.clone(),
                after: after.clone(),
            });
        }
        let (before, after) = (b.readers.shown, a.readers.shown);
        if before != after {
            found.push(BlockChange::Shown { before, after });
        }
        changes.extend(found.into_iter().map(|change| Change::Block {
            block: id.clone(),
            change,
        }));
    }
}
