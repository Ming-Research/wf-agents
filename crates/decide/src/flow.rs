//! The flow as the deciding code reads it: the cells in their order, and the blocks each reads and writes.
//!
//! Only what decides the next step is here. What a cell is given, what it hands in and how it is run belong to the
//! parts of the engine that act.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Makes the type of a name the flow or the records use: a string that cannot be taken for a name of another kind.
macro_rules! name {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub struct $name(pub String);

        impl $name {
            /// The name as text.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_string())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

name!(
    /// The name of a cell.
    CellId
);
name!(
    /// The name of a block of a work's state: what cells, people and the outside write, and cells read.
    BlockId
);
name!(
    /// The name of a named test: a test of the values of blocks of the work, for a cell's condition or precondition.
    TestName
);

/// The cells of a flow, in the order the walk takes them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flow {
    cells: Vec<Cell>,
}

/// Why cells do not make a flow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FlowFault {
    /// A flow has at least one cell.
    Empty,
    /// Two cells have this name.
    Twice(CellId),
    /// The cell's condition reads a block the cell itself writes: it would decide by what it writes itself whether it
    /// runs.
    ConditionOnOwnOutput(CellId),
    /// The last cell has a condition: a work ends when its last cell has a record, so the last cell always runs.
    LastHasCondition(CellId),
    /// The cell's limit is zero: it could never run again for what a cell after it wrote, nor be let go on.
    NoLimit(CellId),
}

impl Flow {
    /// The flow of these cells, in this order.
    pub fn new(cells: Vec<Cell>) -> Result<Self, FlowFault> {
        let Some(last) = cells.last() else {
            return Err(FlowFault::Empty);
        };
        if last.applies.is_some() {
            return Err(FlowFault::LastHasCondition(last.id.clone()));
        }
        for (i, cell) in cells.iter().enumerate() {
            if cells[..i].iter().any(|c| c.id == cell.id) {
                return Err(FlowFault::Twice(cell.id.clone()));
            }
            if cell.limit == 0 {
                return Err(FlowFault::NoLimit(cell.id.clone()));
            }
            if let Some(Condition::Is { block, .. }) = &cell.applies
                && cell.outputs.contains(block)
            {
                return Err(FlowFault::ConditionOnOwnOutput(cell.id.clone()));
            }
        }
        Ok(Self { cells })
    }

    /// The cells, in the order the walk takes them.
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// Where the cell of this name is in the order.
    pub fn place(&self, id: &CellId) -> Option<usize> {
        self.cells.iter().position(|c| &c.id == id)
    }
}

/// A cell, as far as the walk reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub id: CellId,
    /// When the cell runs at all; with none it always may. A cell whose condition is not met is skipped: what it wrote
    /// before still counts.
    pub applies: Option<Condition>,
    /// The preconditions: while one does not hold, the work waits before this cell.
    pub requires: Vec<TestName>,
    /// The blocks the cell reads: when one has changed in meaning since the cell last ran, the cell runs again.
    pub inputs: Vec<BlockId>,
    /// The blocks the cell may write.
    pub outputs: Vec<BlockId>,
    /// How many times in a row the cell may run only for what cells after it wrote, before the work waits for the
    /// owner. At least one.
    pub limit: u32,
}

impl Cell {
    /// A cell of this name that always runs, reads nothing, writes nothing and has a limit of three: what a
    /// definition starts from.
    pub fn new(id: &str) -> Self {
        Self {
            id: id.into(),
            applies: None,
            requires: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            limit: 3,
        }
    }
}

/// When a cell runs at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Condition {
    /// The block's current value is this one (principle yes-no-forks).
    Is { block: BlockId, value: String },
    /// The named test holds.
    Test(TestName),
}
