//! A work as it is now, for whoever shows it: what it does next, each cell's state, and every version of every
//! block. It is worked out with the walk's own reading of the records, so that what is shown and what is decided
//! cannot differ.

use std::collections::BTreeMap;

use crate::flow::{BlockId, CellId, Flow};
use crate::records::{Cause, Outsider, Records, RunId, Seq, Tests};
use crate::walk::{Decision, Version, Walk, decide, read_runs, read_versions};

/// A work as it is now. On records that cannot be those of a work the decision says so, and the rest is what those
/// records give: to be shown for the tracing of the fault, not relied on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    /// What the work does next.
    pub decision: Decision,
    /// The cells, in the order of the walk.
    pub cells: Vec<CellView>,
    /// Every version of every block that has one, in the order they were written.
    pub blocks: BTreeMap<BlockId, Vec<VersionView>>,
}

/// A cell of a work as it is now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellView {
    pub cell: CellId,
    /// Whether the cell's condition is met; where it is not, the cell is skipped.
    pub applies: bool,
    /// The cell's record: its latest finished run.
    pub record: Option<RunId>,
    /// The cell's run that has neither finished nor been abandoned.
    pub open: Option<OpenRun>,
    /// Why the cell would run were the walk to reach it with nothing holding it; empty where its record knows the
    /// current meaning of every block it reads. It is given also for a cell that is skipped or that waits.
    pub needs: Vec<Cause>,
    /// The blocks the cell reads, in the order it lists them.
    pub inputs: Vec<InputView>,
}

/// A run that has neither finished nor been abandoned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenRun {
    pub run: RunId,
    /// Whether a round of it is under way; it is at rest where none is.
    pub under_way: bool,
}

/// A block a cell reads, as the cell's record knows it and as it is now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputView {
    pub block: BlockId,
    /// The block's current version; `None` where it has none.
    pub current: Option<Seq>,
    /// The version the cell's record knows: the one its run started from, or the one it wrote itself. `None` where
    /// the cell has no record, or the block had no version then.
    pub known: Option<Seq>,
    /// Whether the block now has another meaning than the one the cell's record knows; never where the cell has no
    /// record.
    pub changed: bool,
}

/// A version of a block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionView {
    /// The place of the entry that wrote it, which names it.
    pub at: Seq,
    pub by: Writer,
    /// The version at which the meaning it has last changed: itself, or an earlier one.
    pub meaning: Seq,
    /// The block's value, where it is a choice.
    pub value: Option<String>,
}

/// Who wrote a version of a block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Writer {
    Person,
    Outside,
    /// A cell, by this run of it.
    Run(RunId),
}

/// The work as it is now.
pub fn view(flow: &Flow, records: &Records, tests: &dyn Tests) -> View {
    let walk = Walk::new(flow, records, tests);
    let cells = (flow.cells().iter().enumerate())
        .map(|(i, cell)| {
            let record = walk.record(&cell.id);
            let open = (walk.runs.values())
                .find(|r| r.open() && r.started.cell == cell.id)
                .map(|r| OpenRun {
                    run: r.started.run,
                    under_way: r.under_way(),
                });
            let inputs = (cell.inputs.iter())
                .map(|block| {
                    let known = record.and_then(|r| walk.known(r, block));
                    InputView {
                        block: block.clone(),
                        current: walk.current(block).map(|v| v.at),
                        known,
                        changed: record.is_some() && walk.meaning_of(block, known) != walk.meaning_now(block),
                    }
                })
                .collect();
            CellView {
                cell: cell.id.clone(),
                applies: walk.applies(cell),
                record: record.map(|r| r.started.run),
                open,
                needs: walk.needs(i),
                inputs,
            }
        })
        .collect();
    View {
        decision: decide(flow, records, tests),
        cells,
        blocks: shown(&walk.versions),
    }
}

/// Every version of every block that has one, in the order they were written, read from the records alone: no cell
/// and no named test is asked, so a named test may read them.
pub fn versions(records: &Records) -> BTreeMap<BlockId, Vec<VersionView>> {
    shown(&read_versions(records, &read_runs(records)))
}

/// The versions as the walk reads them, for whoever shows them.
fn shown(versions: &BTreeMap<&BlockId, Vec<Version<'_>>>) -> BTreeMap<BlockId, Vec<VersionView>> {
    (versions.iter())
        .map(|(block, versions)| {
            let versions = (versions.iter())
                .map(|v| VersionView {
                    at: v.at,
                    by: match (v.by, v.outsider) {
                        (Some(run), _) => Writer::Run(run),
                        (None, Some(Outsider::Outside)) => Writer::Outside,
                        (None, _) => Writer::Person,
                    },
                    meaning: v.meaning,
                    value: v.value.map(str::to_string),
                })
                .collect();
            ((*block).clone(), versions)
        })
        .collect()
}
