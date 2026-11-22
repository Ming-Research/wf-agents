//! How what a work recorded on one version of its flow reads on a later one: the cells renumbered since, and the data
//! made for those cells, by the names they have there (principle `cell-numbering`; decision
//! `a-move-is-read-through-the-renumberings`). The records keep every name as it was recorded, and are read through
//! each reading after them, in turn.

use std::collections::BTreeMap;

use crate::flow::{BlockId, CellId};
use crate::records::{Cause, Records, Seq};

/// The names that read otherwise on the version a work was put on, each by the name the records hold it by: each cell
/// whose id another renumbering gave it, or that was taken out, and each datum made for such a cell. A name not here
/// reads as it was recorded.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reading {
    pub cells: BTreeMap<CellId, CellId>,
    pub blocks: BTreeMap<BlockId, BlockId>,
}

impl Reading {
    /// Whether it reads every name as it was recorded.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty() && self.blocks.is_empty()
    }

    /// The cell recorded so, as it reads.
    pub fn cell(&self, cell: &CellId) -> CellId {
        self.cells.get(cell).unwrap_or(cell).clone()
    }

    /// The datum recorded so, as it reads.
    pub fn block(&self, block: &BlockId) -> BlockId {
        self.blocks.get(block).unwrap_or(block).clone()
    }

    /// Whether the datum recorded so reads by another name.
    pub fn renames(&self, block: &BlockId) -> bool {
        self.blocks.contains_key(block)
    }

    /// Reads every entry of the records at a place before `at` as this reading names its cells and data; an entry at
    /// `at` or after was recorded as it reads.
    pub fn read(&self, records: &mut Records, at: Seq) {
        if self.is_empty() {
            return;
        }
        let cell = |c: &mut CellId| *c = self.cell(c);
        let block = |b: &mut BlockId| *b = self.block(b);
        for s in records.started.iter_mut().filter(|e| e.at < at) {
            cell(&mut s.cell);
            s.basis = (s.basis.iter()).map(|(b, v)| (self.block(b), *v)).collect();
            for cause in &mut s.because {
                if let Cause::Changed { block: b, .. } = cause {
                    block(b);
                }
            }
        }
        for f in records.finished.iter_mut().filter(|e| e.at < at) {
            for w in &mut f.writes {
                block(&mut w.block);
            }
            for b in f.dealt_with.iter_mut().flatten() {
                block(b);
            }
            if let Some(b) = &mut f.waits_for {
                block(b);
            }
        }
        for p in records.put_in.iter_mut().filter(|e| e.at < at) {
            block(&mut p.block);
        }
        for w in records.written.iter_mut().filter(|e| e.at < at) {
            block(&mut w.block);
            if let Some(c) = &mut w.said_to {
                cell(c);
            }
        }
        for q in records.questions.iter_mut().filter(|e| e.at < at) {
            cell(&mut q.cell);
        }
        for l in records.let_go.iter_mut().filter(|e| e.at < at) {
            cell(&mut l.cell);
        }
    }
}
