//! The flow as the page reads it: its own types, named for what the owner reads, made from what the flow crate
//! compiles and compares. The page words everything it shows from these; nothing here is text written for the owner
//! but what the definition itself holds (titles, the words of the tests) and what the parser said of a file it could
//! not read.

use decide::{BlockId, CellId, Condition as Applies, FlowFault, TestName};
use flow::{BlockChange, CellChange, Compiled, FaultKind, Kind as CellKind, Limit as LimitOf, TestKind as KindOfTest};
use serde::{Deserialize, Serialize};

/// A definition as the page shows it: compiled, or why it is not.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Definition {
    /// The definition compiles: the flow it compiles to.
    Compiled { flow: Flow },
    /// The definition does not compile: every fault it has, in the order the compiler gives them.
    Faults { faults: Vec<Fault> },
    /// The definition's directory cannot be read, and why.
    Unreadable { reason: String },
}

/// What changed, as the page shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Comparison {
    /// The page was started with no definition to compare against.
    NothingToCompare,
    /// The definition compared against, and the changes from it to the one shown; none when either does not compile,
    /// since only compiled definitions are compared.
    Compared {
        against: Definition,
        changes: Option<Vec<Change>>,
    },
}

/// A compiled flow.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Flow {
    pub name: String,
    /// The limits of the flow as a whole, which each cell that sets none of its own has.
    pub limits: Limits,
    /// The lines that cells run in, by name, each with its cells in the order of the walk.
    pub lines: Vec<Line>,
    /// The named tests, by name.
    pub tests: Vec<Test>,
    /// The cells, in the order of the walk.
    pub cells: Vec<Cell>,
    /// The blocks, by name.
    pub blocks: Vec<Block>,
}

/// The limits of what repeats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
    /// How many runs of a cell in a row may be ways round before the work waits for the owner.
    pub runs: u32,
    /// How often a return may be sent back within one run.
    pub returns: u32,
    /// How many calls one run may make.
    pub calls: u32,
}

/// A line: the cells that share one session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    pub name: String,
    pub cells: Vec<String>,
}

/// A named test: the words shown for it, and what it holds on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Test {
    pub name: String,
    pub words: String,
    /// How the blocks it reads make it hold.
    pub kind: TestKind,
    /// The blocks it reads, as the definition lists them.
    pub reads: Vec<String>,
    /// The value it looks for in each block it reads.
    pub is: String,
}

/// How the blocks a named test reads make it hold: when every one, at least one, or none has the value it looks for;
/// or, `noneStanding`, when none has it as written by a person, the outside or a cell that is not skipped now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestKind {
    All,
    Any,
    None,
    NoneStanding,
}

/// A cell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cell {
    pub id: String,
    pub title: String,
    pub kind: Kind,
    /// The line an agent's cell runs in; an engine's cell has none.
    pub line: Option<String>,
    /// When the cell runs at all; none is always.
    pub condition: Option<Condition>,
    /// What the work waits for before the cell.
    pub preconditions: Vec<Test>,
    /// The blocks it reads, as the definition lists them.
    pub inputs: Vec<String>,
    /// The blocks it may write, as the definition lists them.
    pub outputs: Vec<String>,
    pub limits: Limits,
}

/// Who carries a cell out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Agent,
    Engine,
}

/// When a cell runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "on", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Condition {
    /// When the block has this value.
    Block { block: String, value: String },
    /// When the named test holds.
    Test { test: Test },
}

/// A block of a work's state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub name: String,
    /// The values of a choice; none for a block that is not one.
    pub values: Option<Vec<String>>,
    pub written_by: WrittenBy,
    pub read_by: ReadBy,
}

/// Who writes a block.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WrittenBy {
    /// The cells that may write it, in the order of the walk.
    pub cells: Vec<String>,
    pub people: bool,
    /// The source outside the work that writes it; none where the outside does not.
    pub outside: Option<String>,
}

/// Who reads a block.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadBy {
    /// The cells that read it, in the order of the walk.
    pub cells: Vec<String>,
    /// The cells whose condition reads its value, in the order of the walk.
    pub conditions: Vec<String>,
    /// The named tests that read its value, by name.
    pub tests: Vec<String>,
    /// The owner's page reads it.
    pub shown: bool,
}

/// Something wrong with a definition, and where.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fault {
    /// The file: `flow.toml` or `<cell id>.toml`.
    pub file: String,
    /// The field within the file, such as `inputs[2]`; empty when the whole file is at fault.
    pub field: String,
    pub what: What,
}

/// What is wrong, as data: the page words it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum What {
    Unreadable {
        message: String,
    },
    OrderEmpty,
    NoCellFile {
        cell: String,
    },
    NotInOrder {
        cell: String,
    },
    Twice {
        name: String,
    },
    LimitOutOfRange {
        value: i64,
    },
    NoLine,
    EngineHasLine {
        line: String,
    },
    NoSuchLine {
        line: String,
    },
    ValuesEmpty,
    NoSuchBlock {
        block: String,
    },
    NoWriter {
        block: String,
    },
    NoReader {
        block: String,
    },
    UnknownSource {
        block: String,
        source: String,
    },
    NotAChoice {
        block: String,
    },
    NotAValue {
        block: String,
        is: String,
    },
    ConditionOnOwnOutput {
        block: String,
    },
    ConditionNotWrittenBefore {
        block: String,
    },
    LastHasCondition,
    UndeclaredTest {
        test: String,
    },
    ReadsEmpty {
        test: String,
    },
    UnusedTest {
        test: String,
    },
    TestOnOwnOutput {
        test: String,
        block: String,
    },
    TestNotWrittenBefore {
        test: String,
        block: String,
    },
    /// The test reads a block that this cell, after the one using the test, writes.
    TestWrittenAfter {
        test: String,
        block: String,
        cell: String,
    },
    /// The deciding code refused the cells as a flow, for this reason, at this cell.
    FlowRefused {
        reason: FlowRefusal,
        cell: Option<String>,
    },
}

/// Why the deciding code refused the cells as a flow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FlowRefusal {
    Empty,
    Twice,
    ConditionOnOwnOutput,
    LastHasCondition,
    NoLimit,
    NoSuchTest,
}

/// One change from the definition compared against to the one shown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Change {
    Name {
        before: String,
        after: String,
    },
    FlowLimit {
        limit: Limit,
        before: u32,
        after: u32,
    },
    TestAdded {
        test: Test,
    },
    TestRemoved {
        test: Test,
    },
    TestReworded {
        test: String,
        before: String,
        after: String,
    },
    /// What the test holds on: its kind, the blocks it reads or the value it looks for.
    TestChanged {
        test: String,
        before: Test,
        after: Test,
    },
    /// At this place of the order, counted from 0.
    CellAdded {
        cell: String,
        place: usize,
    },
    /// From this place of the order, counted from 0.
    CellRemoved {
        cell: String,
        place: usize,
    },
    CellMoved {
        cell: String,
        before: usize,
        after: usize,
    },
    Title {
        cell: String,
        before: String,
        after: String,
    },
    Kind {
        cell: String,
        before: Kind,
        after: Kind,
    },
    Line {
        cell: String,
        before: Option<String>,
        after: Option<String>,
    },
    Condition {
        cell: String,
        before: Option<Condition>,
        after: Option<Condition>,
    },
    PreconditionAdded {
        cell: String,
        test: Test,
    },
    PreconditionRemoved {
        cell: String,
        test: Test,
    },
    InputAdded {
        cell: String,
        block: String,
    },
    InputRemoved {
        cell: String,
        block: String,
    },
    OutputAdded {
        cell: String,
        block: String,
    },
    OutputRemoved {
        cell: String,
        block: String,
    },
    CellLimit {
        cell: String,
        limit: Limit,
        before: u32,
        after: u32,
    },
    BlockAdded {
        block: String,
    },
    BlockRemoved {
        block: String,
    },
    Values {
        block: String,
        before: Option<Vec<String>>,
        after: Option<Vec<String>>,
    },
    People {
        block: String,
        before: bool,
        after: bool,
    },
    Outside {
        block: String,
        before: Option<String>,
        after: Option<String>,
    },
    Shown {
        block: String,
        before: bool,
        after: bool,
    },
}

/// Which limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Limit {
    Runs,
    Returns,
    Calls,
}

impl Flow {
    /// The compiled definition as the page reads it.
    pub fn of(compiled: &Compiled) -> Self {
        let limits = Limits {
            runs: compiled.limits.runs,
            returns: compiled.limits.returns,
            calls: compiled.limits.calls,
        };
        let mut lines: Vec<Line> = Vec::new();
        for spec in &compiled.cells {
            let Some(name) = &spec.line else { continue };
            match lines.iter_mut().find(|l| &l.name == name) {
                Some(line) => line.cells.push(spec.id.to_string()),
                None => lines.push(Line {
                    name: name.clone(),
                    cells: vec![spec.id.to_string()],
                }),
            }
        }
        lines.sort_by(|a, b| a.name.cmp(&b.name));
        let cells = compiled
            .flow
            .cells()
            .iter()
            .zip(&compiled.cells)
            .map(|(cell, spec)| Cell {
                id: cell.id.to_string(),
                title: spec.title.clone(),
                kind: Kind::of(spec.kind),
                line: spec.line.clone(),
                condition: cell.applies.as_ref().map(|c| Condition::of(c, compiled)),
                preconditions: cell.requires.iter().map(|t| Test::of(t, compiled)).collect(),
                inputs: names(&cell.inputs),
                outputs: names(&cell.outputs),
                limits: Limits {
                    runs: cell.limit,
                    returns: spec.returns,
                    calls: spec.calls,
                },
            })
            .collect();
        let blocks = compiled
            .blocks
            .iter()
            .map(|(name, block)| Block {
                name: name.to_string(),
                values: block.values.clone(),
                written_by: WrittenBy {
                    cells: names(&block.writers.cells),
                    people: block.writers.people,
                    outside: block.writers.outside.clone(),
                },
                read_by: ReadBy {
                    cells: names(&block.readers.cells),
                    conditions: names(&block.readers.conditions),
                    tests: names(&block.readers.tests),
                    shown: block.readers.shown,
                },
            })
            .collect();
        Flow {
            name: compiled.name.clone(),
            limits,
            lines,
            tests: compiled.tests.keys().map(|name| Test::of(name, compiled)).collect(),
            cells,
            blocks,
        }
    }
}

fn names<T: ToString>(ids: &[T]) -> Vec<String> {
    ids.iter().map(ToString::to_string).collect()
}

impl Kind {
    fn of(kind: CellKind) -> Self {
        match kind {
            CellKind::Agent => Kind::Agent,
            CellKind::Engine => Kind::Engine,
        }
    }
}

impl Test {
    /// The test of this name as the definition declares it: a compiled definition declares every test its cells use
    /// and every test a comparison of it names.
    fn of(test: &TestName, compiled: &Compiled) -> Self {
        Test::declared(test, &compiled.tests[test], &compiled.flow.tests()[test])
    }

    /// The test of this name, with these words, testing this.
    fn declared(name: &TestName, words: &str, test: &flow::Test) -> Self {
        Test {
            name: name.to_string(),
            words: words.to_string(),
            kind: match test.kind {
                KindOfTest::All => TestKind::All,
                KindOfTest::Any => TestKind::Any,
                KindOfTest::None => TestKind::None,
                KindOfTest::NoneStanding => TestKind::NoneStanding,
            },
            reads: names(&test.reads),
            is: test.is.clone(),
        }
    }
}

impl Condition {
    fn of(condition: &Applies, compiled: &Compiled) -> Self {
        match condition {
            Applies::Is { block, value } => Condition::Block {
                block: block.to_string(),
                value: value.clone(),
            },
            Applies::Test(test) => Condition::Test {
                test: Test::of(test, compiled),
            },
        }
    }
}

impl Limit {
    fn of(limit: LimitOf) -> Self {
        match limit {
            LimitOf::Runs => Limit::Runs,
            LimitOf::Returns => Limit::Returns,
            LimitOf::Calls => Limit::Calls,
        }
    }
}

impl Fault {
    /// A fault of the compiler as the page reads it.
    pub fn of(fault: &flow::Fault) -> Self {
        Fault {
            file: fault.file.clone(),
            field: fault.field.clone(),
            what: What::of(&fault.what),
        }
    }
}

impl What {
    fn of(what: &FaultKind) -> Self {
        use FaultKind as F;
        match what.clone() {
            F::Unreadable { message } => What::Unreadable { message },
            F::OrderEmpty => What::OrderEmpty,
            F::NoCellFile { cell } => What::NoCellFile { cell },
            F::NotInOrder { cell } => What::NotInOrder { cell },
            F::Twice { name } => What::Twice { name },
            F::LimitOutOfRange { value } => What::LimitOutOfRange { value },
            F::NoLine => What::NoLine,
            F::EngineHasLine { line } => What::EngineHasLine { line },
            F::NoSuchLine { line } => What::NoSuchLine { line },
            F::ValuesEmpty => What::ValuesEmpty,
            F::NoSuchBlock { block } => What::NoSuchBlock { block },
            F::NoWriter { block } => What::NoWriter { block },
            F::NoReader { block } => What::NoReader { block },
            F::UnknownSource { block, source } => What::UnknownSource { block, source },
            F::NotAChoice { block } => What::NotAChoice { block },
            F::NotAValue { block, is } => What::NotAValue { block, is },
            F::ConditionOnOwnOutput { block } => What::ConditionOnOwnOutput { block },
            F::ConditionNotWrittenBefore { block } => What::ConditionNotWrittenBefore { block },
            F::LastHasCondition => What::LastHasCondition,
            F::UndeclaredTest { test } => What::UndeclaredTest { test },
            F::ReadsEmpty { test } => What::ReadsEmpty { test },
            F::UnusedTest { test } => What::UnusedTest { test },
            F::TestOnOwnOutput { test, block } => What::TestOnOwnOutput { test, block },
            F::TestNotWrittenBefore { test, block } => What::TestNotWrittenBefore { test, block },
            F::TestWrittenAfter { test, block, cell } => What::TestWrittenAfter { test, block, cell },
            F::Flow(fault) => {
                let (reason, cell) = match fault {
                    FlowFault::Empty => (FlowRefusal::Empty, None),
                    FlowFault::Twice(c) => (FlowRefusal::Twice, Some(c)),
                    FlowFault::ConditionOnOwnOutput(c) => (FlowRefusal::ConditionOnOwnOutput, Some(c)),
                    FlowFault::LastHasCondition(c) => (FlowRefusal::LastHasCondition, Some(c)),
                    FlowFault::NoLimit(c) => (FlowRefusal::NoLimit, Some(c)),
                    FlowFault::NoSuchTest(c, _) => (FlowRefusal::NoSuchTest, Some(c)),
                };
                What::FlowRefused {
                    reason,
                    cell: cell.map(|c| c.to_string()),
                }
            }
        }
    }
}

impl Change {
    /// A change of the flow crate as the page reads it. A test is as the definition the change is in declares it: the
    /// one compared against for what was taken out or held before, the one shown for what was put in.
    pub fn of(change: &flow::Change, before: &Compiled, after: &Compiled) -> Self {
        use flow::Change as C;
        match change {
            C::Name { before, after } => Change::Name {
                before: before.clone(),
                after: after.clone(),
            },
            C::Limit { limit, before, after } => Change::FlowLimit {
                limit: Limit::of(*limit),
                before: *before,
                after: *after,
            },
            C::TestAdded { test, .. } => Change::TestAdded {
                test: Test::of(test, after),
            },
            C::TestRemoved { test, .. } => Change::TestRemoved {
                test: Test::of(test, before),
            },
            C::TestReworded { test, before, after } => Change::TestReworded {
                test: test.to_string(),
                before: before.clone(),
                after: after.clone(),
            },
            C::TestChanged {
                test,
                before: from,
                after: to,
            } => Change::TestChanged {
                test: test.to_string(),
                before: Test::declared(test, &before.tests[test], from),
                after: Test::declared(test, &after.tests[test], to),
            },
            C::CellAdded { cell, place } => Change::CellAdded {
                cell: cell.to_string(),
                place: *place,
            },
            C::CellRemoved { cell, place } => Change::CellRemoved {
                cell: cell.to_string(),
                place: *place,
            },
            C::CellMoved {
                cell,
                before: from,
                after: to,
            } => Change::CellMoved {
                cell: cell.to_string(),
                before: *from,
                after: *to,
            },
            C::Cell { cell, change } => Change::of_cell(cell, change, before, after),
            C::BlockAdded { block } => Change::BlockAdded {
                block: block.to_string(),
            },
            C::BlockRemoved { block } => Change::BlockRemoved {
                block: block.to_string(),
            },
            C::Block { block, change } => Change::of_block(block, change),
        }
    }

    fn of_cell(cell: &CellId, change: &CellChange, before: &Compiled, after: &Compiled) -> Self {
        let cell = cell.to_string();
        let block = |b: &BlockId| b.to_string();
        match change {
            CellChange::Title { before, after } => Change::Title {
                cell,
                before: before.clone(),
                after: after.clone(),
            },
            CellChange::Kind {
                before: from,
                after: to,
            } => Change::Kind {
                cell,
                before: Kind::of(*from),
                after: Kind::of(*to),
            },
            CellChange::Line {
                before: from,
                after: to,
            } => Change::Line {
                cell,
                before: from.clone(),
                after: to.clone(),
            },
            CellChange::Condition {
                before: from,
                after: to,
            } => Change::Condition {
                cell,
                before: from.as_ref().map(|c| Condition::of(c, before)),
                after: to.as_ref().map(|c| Condition::of(c, after)),
            },
            CellChange::PreconditionAdded(test) => Change::PreconditionAdded {
                cell,
                test: Test::of(test, after),
            },
            CellChange::PreconditionRemoved(test) => Change::PreconditionRemoved {
                cell,
                test: Test::of(test, before),
            },
            CellChange::InputAdded(b) => Change::InputAdded { cell, block: block(b) },
            CellChange::InputRemoved(b) => Change::InputRemoved { cell, block: block(b) },
            CellChange::OutputAdded(b) => Change::OutputAdded { cell, block: block(b) },
            CellChange::OutputRemoved(b) => Change::OutputRemoved { cell, block: block(b) },
            CellChange::Limit {
                limit,
                before: from,
                after: to,
            } => Change::CellLimit {
                cell,
                limit: Limit::of(*limit),
                before: *from,
                after: *to,
            },
        }
    }

    fn of_block(block: &BlockId, change: &BlockChange) -> Self {
        let block = block.to_string();
        match change {
            BlockChange::Values { before, after } => Change::Values {
                block,
                before: before.clone(),
                after: after.clone(),
            },
            &BlockChange::People { before, after } => Change::People { block, before, after },
            BlockChange::Outside { before, after } => Change::Outside {
                block,
                before: before.clone(),
                after: after.clone(),
            },
            &BlockChange::Shown { before, after } => Change::Shown { block, before, after },
        }
    }
}
