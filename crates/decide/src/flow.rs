//! The flow as the deciding code reads it: the cells in their order, the blocks that run each and those it writes, the
//! named tests their conditions and preconditions are, and which blocks are notes or the owner turning to a cell.
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
    /// The value each choice that has one counts as before anyone has written it, by block.
    defaults: BTreeMap<BlockId, String>,
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
    /// A new version of it is the owner turning to the cells it runs: the owner's words to a cell, or an answer that
    /// takes another option than the one the work goes by (design document `data-and-context`, 35, 39). It stops a
    /// round under way of a cell after the one it runs, enters a cell that waits for the results of its calls, and is
    /// answered while the work is paused, as a note is.
    pub interrupts: bool,
}

/// A named test: a test of the values of blocks of the work itself, which the walk works out from the versions it
/// reads. What people and the outside do reaches it only as blocks they write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Test {
    /// How the blocks it reads make it hold.
    pub kind: TestKind,
    /// The blocks it reads, each a choice.
    pub reads: Vec<BlockId>,
    /// The value it tests for in each block it reads.
    pub is: String,
}

/// How the blocks a named test reads make it hold, by which of them have the value it tests for now. A block never
/// written has no value, or its default where the flow gives it one. A new kind is a new function of the walk.
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
}

/// Why cells do not make a flow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FlowFault {
    /// A flow has at least one cell.
    Empty,
    /// Two cells have this name.
    Twice(CellId),
    /// The cell's condition reads a block the cell itself writes, and another writer writes it too. A cell that is the
    /// block's only writer may: the block says which of two cells is in charge, which the cell decides for its own
    /// next round (design document `data-and-context`, 46).
    ConditionOnOwnOutput(CellId),
    /// The last cell has a condition: a work ends when its last cell has a record, so the last cell always runs.
    LastHasCondition(CellId),
    /// The cell's limit is zero: it could never run again for what a cell after it wrote, nor be let go on.
    NoLimit(CellId),
    /// A key of the cell's condition or of its preconditions names a named test the flow does not have.
    NoSuchTest(CellId, TestName),
    /// A named test a cell uses reads a block that no cell, person or source outside the work writes: nobody could
    /// change whether it holds, and a wait on it would name nobody.
    Unwritten(TestName, BlockId),
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
        if !last.applies.is_empty() {
            return Err(FlowFault::LastHasCondition(last.id.clone()));
        }
        for (i, cell) in cells.iter().enumerate() {
            if cells[..i].iter().any(|c| c.id == cell.id) {
                return Err(FlowFault::Twice(cell.id.clone()));
            }
            if cell.limit == 0 {
                return Err(FlowFault::NoLimit(cell.id.clone()));
            }
            // A block the cell writes that another writer writes too.
            let shared = |block: &BlockId| {
                cell.outputs.contains(block)
                    && (cells.iter().any(|c| c.id != cell.id && c.outputs.contains(block))
                        || outsiders.get(block).is_some_and(|o| o.people || o.outside.is_some()))
            };
            if (cell.applies.iter()).any(|key| matches!(key, Condition::Is { block, .. } if shared(block))) {
                return Err(FlowFault::ConditionOnOwnOutput(cell.id.clone()));
            }
            if let Some(test) = cell.tests().find(|t| !tests.contains_key(t)) {
                return Err(FlowFault::NoSuchTest(cell.id.clone(), test.clone()));
            }
        }
        let flow = Self {
            cells,
            tests,
            outsiders,
            defaults: BTreeMap::new(),
        };
        for cell in &flow.cells {
            for name in cell.tests() {
                if let Some(block) = flow.tests[name].reads.iter().find(|b| !flow.is_written(b)) {
                    return Err(FlowFault::Unwritten(name.clone(), block.clone()));
                }
            }
        }
        Ok(flow)
    }

    /// The flow with these defaults: the value each of these choices counts as before anyone has written it.
    pub fn with_defaults(mut self, defaults: BTreeMap<BlockId, String>) -> Self {
        self.defaults = defaults;
        self
    }

    /// The value the block counts as before anyone has written it; none where it has no default.
    pub fn default_of(&self, block: &BlockId) -> Option<&str> {
        self.defaults.get(block).map(String::as_str)
    }

    /// Whether the block is a note: what people write in it are their words to the cells that read it.
    pub fn is_note(&self, block: &BlockId) -> bool {
        self.outsiders.get(block).is_some_and(|o| o.note)
    }

    /// Whether a new version of the block is the owner turning to the cells it runs ([`Outsiders::interrupts`]).
    pub fn interrupts(&self, block: &BlockId) -> bool {
        self.outsiders.get(block).is_some_and(|o| o.interrupts)
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
    /// When the cell runs at all: every one of these keys holds; with none it always may. A cell whose condition is not
    /// met is skipped: what it wrote before still counts.
    pub applies: Vec<Condition>,
    /// The preconditions: while one of these keys does not hold, the work waits before this cell for whoever writes a
    /// block that keeps it from holding.
    pub requires: Vec<Condition>,
    /// The blocks whose new version runs the cell (design document `data-and-context`, 5 and 6): a block is new to the
    /// cell while it has a version, changing its meaning, later than the one the cell consumed, that the cell did not
    /// write itself. What the cell is given besides is no matter of the walk's.
    pub runs_on: Vec<BlockId>,
    /// Of each trigger of the cell that stands on a group of blocks, by the group's name, the group's blocks, each
    /// among those that run the cell. A return names such a trigger as dealt with by the group's name ([`stands_on`]).
    pub groups: BTreeMap<BlockId, Vec<BlockId>>,
    /// The blocks the cell may write.
    pub outputs: Vec<BlockId>,
    /// How many times in a row the cell may run only for what cells after it wrote, before the work waits for the
    /// owner. At least one.
    pub limit: u32,
    /// How many of the cell's runs in a row may each ask for calls and deal with nothing, before the work waits for the
    /// owner instead of entering the cell again for their results. A definition gives at least one.
    pub calls: u32,
    /// Whether each round of the cell takes one of the slots, which bound how many such rounds go on at once: an
    /// agent's rounds do, and the engine's own acts do not.
    pub slot: bool,
}

impl Cell {
    /// The named tests its condition and its preconditions name, in that order.
    pub fn tests(&self) -> impl Iterator<Item = &TestName> {
        (self.applies.iter().chain(&self.requires)).filter_map(|key| match key {
            Condition::Test(test) => Some(test),
            Condition::Is { .. } => None,
        })
    }

    /// A cell of this name that always applies, that nothing runs, that writes nothing, has a limit of three runs and
    /// of three calls and takes a slot for each of its rounds: what a definition starts from.
    pub fn new(id: &str) -> Self {
        Self {
            id: id.into(),
            applies: Vec::new(),
            requires: Vec::new(),
            runs_on: Vec::new(),
            groups: BTreeMap::new(),
            outputs: Vec::new(),
            limit: 3,
            calls: 3,
            slot: true,
        }
    }
}

/// The blocks a trigger of a cell stands on, by the name a return deals with it by: of a group's trigger, by the
/// group's name, the group's blocks, as the cell's groups ([`Cell::groups`]) hold them; of another, the block of that
/// name.
pub fn stands_on<'a>(groups: &'a BTreeMap<BlockId, Vec<BlockId>>, named: &'a BlockId) -> &'a [BlockId] {
    groups.get(named).map_or(std::slice::from_ref(named), Vec::as_slice)
}

/// Of the blocks a trigger stands on, those among the blocks that ran a round: the trigger ran the round where any of
/// them did, and a return that names it as dealt with deals with each of them, as one trigger (design document
/// `data-and-context`, 9). A group's trigger runs its cell once for a new version of any of the group's blocks, and is
/// dealt with once for all of them.
pub fn ran<'a>(stands_on: &'a [BlockId], ran_by: &'a [BlockId]) -> impl Iterator<Item = &'a BlockId> + 'a {
    stands_on.iter().filter(|b| ran_by.contains(b))
}

/// One key of a cell's condition or of its preconditions, every one of which holds where the cell runs, or goes on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Condition {
    /// The block's current value is this one (principle yes-no-forks).
    Is { block: BlockId, value: String },
    /// The named test of the flow holds.
    Test(TestName),
}

/// The named test of this name, as a key.
impl From<&str> for Condition {
    fn from(test: &str) -> Self {
        Condition::Test(test.into())
    }
}
