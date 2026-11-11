//! The flow as the deciding code reads it: the cells in their order, the blocks each reads and writes, the named tests
//! their conditions and preconditions are, and which blocks are notes.
//!
//! Only what decides the next step is here. What a cell is given, what it hands in and how it is run belong to the
//! parts of the engine that act.

use std::collections::BTreeMap;
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

/// The cells of a flow, in the order the walk takes them, the named tests they use, and who besides the cells writes
/// each block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flow {
    cells: Vec<Cell>,
    tests: BTreeMap<TestName, Test>,
    /// Who besides the cells writes each block; a block not here is written by cells alone.
    outsiders: BTreeMap<BlockId, Outsiders>,
}

/// Who writes a block without running a cell: the cells that write it are those with it among their outputs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outsiders {
    /// People write it.
    pub people: bool,
    /// What people write in it is a note: their words to the cells that read it, which a paused work still answers.
    pub note: bool,
    /// The source outside the work that writes it, where one does.
    pub outside: Option<String>,
}

/// A named test: a test of the values of blocks of the work itself, which the walk works out from the versions it
/// reads. What people and the outside do reaches it only as blocks they write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Test {
    /// How the blocks it reads make it hold.
    pub kind: TestKind,
    /// The blocks it reads, each a choice.
    pub reads: Vec<BlockId>,
    /// The value it tests for in each block it reads; for a test of what was confirmed, the value of the confirmation.
    pub is: String,
    /// For a test of what was confirmed, the choice only people write in which they confirm what it reads; none for
    /// any other kind.
    pub by: Option<BlockId>,
}

/// How the blocks a named test reads make it hold, by which of them have the value it tests for now. A block never
/// written has no value. A new kind is a new function of the walk.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestKind {
    /// Every block read has the value.
    All,
    /// At least one block read has the value.
    Any,
    /// No block read has the value.
    None,
    /// No block read both stands and has the value. A block stands unless its current version was written by a run
    /// of a cell that is skipped now; what a person or the outside wrote always stands. So a judging cell's word holds
    /// while the cell is not skipped, and holds nothing once it is. The kind reads the current version alone, and so
    /// is defined for a block that one cell writes, people and the outside besides: of two cells writing it, a skipped
    /// one's write would hide the other's word. The compiler holds definitions to that.
    NoneStanding,
    /// Every block read that says something was confirmed since what it holds last changed: people wrote the test's
    /// choice of confirmation with the value after the latest version that holds another content or value than the
    /// one before it. A block never written, or whose current version is blank, says nothing and needs no
    /// confirmation; a confirmation is of what the block holds when it is given, so any change of it is confirmed
    /// anew, whatever its writer says of the change, and the same content written again is not. The blocks read need
    /// not be choices.
    Confirmed,
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
    /// The cell's condition or a precondition is a named test the flow does not have.
    NoSuchTest(CellId, TestName),
    /// A named test a cell uses reads a block that no cell, person or source outside the work writes: nobody could
    /// change whether it holds, and a wait on it would name nobody.
    Unwritten(TestName, BlockId),
    /// A test of what was confirmed names no choice of confirmation that people, and no cell, write; or a test of
    /// another kind names one.
    Unconfirmable(TestName),
}

impl Flow {
    /// The flow of these cells, in this order, with these named tests and these writers of blocks besides the cells.
    pub fn new(
        cells: Vec<Cell>,
        tests: BTreeMap<TestName, Test>,
        outsiders: BTreeMap<BlockId, Outsiders>,
    ) -> Result<Self, FlowFault> {
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
            let condition = match &cell.applies {
                Some(Condition::Test(test)) => Some(test),
                _ => None,
            };
            if let Some(test) = condition
                .into_iter()
                .chain(&cell.requires)
                .find(|t| !tests.contains_key(t))
            {
                return Err(FlowFault::NoSuchTest(cell.id.clone(), test.clone()));
            }
        }
        let flow = Self {
            cells,
            tests,
            outsiders,
        };
        for (name, test) in &flow.tests {
            let confirmable = match (&test.by, test.kind) {
                (Some(by), TestKind::Confirmed) => {
                    flow.outsiders(by).is_some_and(|o| o.people) && flow.writers(by).next().is_none()
                }
                (None, TestKind::Confirmed) | (Some(_), _) => false,
                (None, _) => true,
            };
            if !confirmable {
                return Err(FlowFault::Unconfirmable(name.clone()));
            }
        }
        for cell in &flow.cells {
            let condition = match &cell.applies {
                Some(Condition::Test(test)) => Some(test),
                _ => None,
            };
            for name in condition.into_iter().chain(&cell.requires) {
                if let Some(block) = flow.tests[name].reads.iter().find(|b| !flow.is_written(b)) {
                    return Err(FlowFault::Unwritten(name.clone(), block.clone()));
                }
            }
        }
        Ok(flow)
    }

    /// Whether the block is a note: what people write in it are their words to the cells that read it.
    pub fn is_note(&self, block: &BlockId) -> bool {
        self.outsiders.get(block).is_some_and(|o| o.note)
    }

    /// Who besides the cells writes the block; none where only cells do.
    pub fn outsiders(&self, block: &BlockId) -> Option<&Outsiders> {
        self.outsiders.get(block)
    }

    /// The cells that have the block among their outputs, in the order of the walk.
    pub fn writers<'f>(&'f self, block: &'f BlockId) -> impl Iterator<Item = &'f Cell> {
        self.cells.iter().filter(move |c| c.outputs.contains(block))
    }

    /// Whether a cell, people or a source outside the work writes the block.
    fn is_written(&self, block: &BlockId) -> bool {
        self.outsiders(block).is_some_and(|o| o.people || o.outside.is_some()) || self.writers(block).next().is_some()
    }

    /// The cells, in the order the walk takes them.
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// The named tests, by name: each that a cell uses, and any other the flow was given.
    pub fn tests(&self) -> &BTreeMap<TestName, Test> {
        &self.tests
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
    /// The preconditions, each a named test of the flow: while one does not hold, the work waits before this cell for
    /// whoever writes a block that keeps it from holding.
    pub requires: Vec<TestName>,
    /// The blocks the cell reads: when one has changed in meaning since the cell last ran, the cell runs again.
    pub inputs: Vec<BlockId>,
    /// The blocks the cell may write.
    pub outputs: Vec<BlockId>,
    /// How many times in a row the cell may run only for what cells after it wrote, before the work waits for the
    /// owner. At least one.
    pub limit: u32,
    /// Whether each round of the cell takes one of the slots, which bound how many such rounds go on at once: an
    /// agent's rounds do, and the engine's own acts do not.
    pub slot: bool,
}

impl Cell {
    /// A cell of this name that always runs, reads nothing, writes nothing, has a limit of three and takes a slot for
    /// each of its rounds: what a definition starts from.
    pub fn new(id: &str) -> Self {
        Self {
            id: id.into(),
            applies: None,
            requires: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            limit: 3,
            slot: true,
        }
    }
}

/// When a cell runs at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Condition {
    /// The block's current value is this one (principle yes-no-forks).
    Is { block: BlockId, value: String },
    /// The named test of the flow holds.
    Test(TestName),
}
