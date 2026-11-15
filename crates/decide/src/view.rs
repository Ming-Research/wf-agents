//! A work as it is now, for whoever shows it: what it does next, each cell's state, and every version of every
//! block. It is worked out with the walk's own reading of the records, so that what is shown and what is decided
//! cannot differ.

use std::collections::BTreeMap;

use crate::flow::{BlockId, CellId, Flow};
use crate::records::{Cause, Outsider, Records, RunId, Seq};
use crate::room::Room;
use crate::walk::{Decision, Version, Walk, decide_in, read_runs, read_versions};

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
    /// What is new to the cell: why it would run were the walk to reach it with nothing holding it, the results of
    /// its calls aside. It is given also for a cell that is skipped or that waits.
    pub needs: Vec<Cause>,
    /// The blocks that run the cell, in the order it lists them.
    pub runs_on: Vec<InputView>,
}

/// A run that has neither finished nor been abandoned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenRun {
    pub run: RunId,
    /// Whether a round of it is under way; it is at rest where none is.
    pub under_way: bool,
}

/// A block that runs a cell, as the cell consumed it and as it is now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputView {
    pub block: BlockId,
    /// The block's latest version; `None` where it has none.
    pub current: Option<Seq>,
    /// The version the cell consumed; `None` where it consumed none.
    pub consumed: Option<Seq>,
    /// Whether the block is new to the cell.
    pub new: bool,
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

/// The work as it is now, were nothing outside its records to hold it back ([`Room::FREE`]).
pub fn view(flow: &Flow, records: &Records) -> View {
    view_in(flow, records, Room::FREE)
}

/// The work as it is now, in the room the engine leaves it.
pub fn view_in(flow: &Flow, records: &Records, room: Room) -> View {
    let walk = Walk::new(flow, records);
    let cells = (flow.cells().iter().enumerate())
        .map(|(i, cell)| {
            let record = walk.record(&cell.id);
            let open = (walk.runs.values())
                .find(|r| r.open() && r.started.cell == cell.id)
                .map(|r| OpenRun {
                    run: r.started.run,
                    under_way: r.under_way(),
                });
            let needs = walk.needs(i);
            let runs_on = (cell.runs_on.iter())
                .map(|block| InputView {
                    block: block.clone(),
                    current: walk.current(block).map(|v| v.at),
                    consumed: walk.consumed(&cell.id, block),
                    new: needs
                        .iter()
                        .any(|c| matches!(c, Cause::Changed { block: b, .. } if b == block)),
                })
                .collect();
            CellView {
                cell: cell.id.clone(),
                applies: walk.applies(i),
                record: record.map(|r| r.started.run),
                open,
                needs,
                runs_on,
            }
        })
        .collect();
    View {
        decision: decide_in(flow, records, room),
        cells,
        blocks: shown(&walk.versions),
    }
}

/// Every version of every block that has one, in the order they were written, read from the records alone, with no
/// flow.
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
