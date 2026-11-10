//! The flow as the deciding code reads it: the cells in their order, and how they are joined.
//!
//! Only what decides the next step is here. What a cell is given, what it hands in and how it is run belong to the
//! parts of the engine that act.

use std::fmt;

/// Makes the type of a name the flow or the records use: a string that cannot be taken for a name of another kind.
macro_rules! name {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    /// The name of a product: a block of the item, an output of a cell, or something outside the work that a cell
    /// depends on.
    ProductId
);
name!(
    /// The name of a test of the records: a condition that is no agent's answer, or a precondition.
    TestName
);
name!(
    /// The name of an output of a cell that a condition reads.
    OutputName
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
    /// The cell's condition reads the answer of a cell that is not before it.
    ConditionNotBefore { cell: CellId, by: CellId },
    /// The last cell has a condition: a work ends when its last cell has a record, so the last cell is always on the
    /// path.
    LastHasCondition(CellId),
    /// The cell's limit is zero: it could never run again for a reason that came from after it, nor be let go on.
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
            if let Some(Condition::Answered { by, .. }) = &cell.applies
                && !cells[..i].iter().any(|c| &c.id == by)
            {
                return Err(FlowFault::ConditionNotBefore {
                    cell: cell.id.clone(),
                    by: by.clone(),
                });
            }
        }
        Ok(Self { cells })
    }

    /// The cells, in the order the walk takes them.
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// The cell of this name.
    pub fn cell(&self, id: &CellId) -> Option<&Cell> {
        self.cells.iter().find(|c| &c.id == id)
    }

    /// Where the cell of this name stands in the order.
    pub fn place(&self, id: &CellId) -> Option<usize> {
        self.cells.iter().position(|c| &c.id == id)
    }
}

/// A cell, as far as the walk reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub id: CellId,
    /// When the cell is on the path; with none it always is. A cell off the path is skipped.
    pub applies: Option<Condition>,
    /// The preconditions: while one does not hold, the work waits before this cell.
    pub requires: Vec<TestName>,
    /// What the cell depends on: a change of its meaning makes the cell need to run.
    pub follows: Vec<ProductId>,
    /// What the cell may write.
    pub writes: Vec<ProductId>,
    /// How many times in a row the cell may run only for reasons that came from after it, before the work waits for
    /// the owner. At least one.
    pub limit: u32,
}

impl Cell {
    /// A cell of this name that is always on the path, depends on nothing, writes nothing and has a limit of three:
    /// what a definition starts from.
    pub fn new(id: &str) -> Self {
        Self {
            id: id.into(),
            applies: None,
            requires: Vec::new(),
            follows: Vec::new(),
            writes: Vec::new(),
            limit: 3,
        }
    }
}

/// When a cell is on the path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Condition {
    /// The cell `by`, which stands before it and is on the path, answered `is` in its output `output` (principle
    /// yes-no-forks).
    Answered { by: CellId, output: OutputName, is: String },
    /// The named test of the records holds.
    Test(TestName),
}
