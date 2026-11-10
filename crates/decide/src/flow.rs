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
#[derive(Clone, Debug, Default)]
pub struct Flow {
    pub cells: Vec<Cell>,
}

impl Flow {
    /// The cell of this name.
    pub fn cell(&self, id: &CellId) -> Option<&Cell> {
        self.cells.iter().find(|c| &c.id == id)
    }
}

/// A cell, as far as the walk reads it.
#[derive(Clone, Debug)]
pub struct Cell {
    pub id: CellId,
    /// When the cell applies; with none it always does. A cell that does not apply is skipped.
    pub applies: Option<Condition>,
    /// The preconditions: while one does not hold, the work waits before this cell.
    pub requires: Vec<TestName>,
    /// What the cell depends on: a change of its meaning makes the cell need to run.
    pub follows: Vec<ProductId>,
    /// What the cell writes.
    pub writes: Vec<ProductId>,
    /// How often the cell is run again for one refusal before the work waits for the owner.
    pub limit: u32,
}

impl Cell {
    /// A cell of this name that always applies, depends on nothing, writes nothing and is run again three times for
    /// one refusal: what a definition starts from.
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

/// When a cell applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Condition {
    /// The cell `by`, which stands before it, answered `is` in its output `output` (principle yes-no-forks).
    Answered { by: CellId, output: OutputName, is: String },
    /// The named test of the records holds.
    Test(TestName),
}
