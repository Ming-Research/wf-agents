//! The flow as the page reads it: its own types, named for what the owner reads, made from what the flow crate
//! compiles. The page words everything it shows from these; nothing here is text written for the owner
//! but what the definition itself holds (titles, the words of the tests) and what the parser said of a file it could
//! not read.

use std::collections::BTreeMap;

use decide::{BlockId, Condition as Applies, FlowFault, TestName};
use flow::{Compiled, FaultKind, Kind as CellKind, TestKind as KindOfTest};
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
    /// The engine's tools every agent's cell of the flow may ask for, in the order the flow names them; none where it
    /// names none.
    pub tools: Vec<String>,
    /// The groups of the flow's blocks, by name, each with its blocks as the flow names them.
    pub groups: Vec<Group>,
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

/// A line: the cells that share one session, and what its file writes of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    pub name: String,
    /// In the order of the walk.
    pub cells: Vec<String>,
    /// How many levels above its item's its rounds run at, never above the highest an item may be at.
    pub levels_above: u32,
    /// The file it is written in, from the flow's directory.
    pub file: String,
    /// Its agents only read.
    pub reads_only: bool,
    /// Of a flow written before it was laid out one thing to a file, its `system`, as written.
    pub system: Vec<String>,
    /// Its `rules`, as written.
    pub rules: Vec<String>,
}

/// A named test: the words shown for it, and what it holds on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Test {
    pub name: String,
    /// Its `says`.
    pub words: String,
    /// The file it is written in, from the flow's directory.
    pub file: String,
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
    /// What an agent's cell's agent is told to do, with its line breaks; an engine's cell has none.
    pub task: Option<String>,
    /// The file the task is written in, from the flow's directory; none where it is written in place.
    pub task_file: Option<String>,
    /// The act of the engine's own that carries out an engine's cell; an agent's cell has none.
    pub act: Option<String>,
    /// The reasoning effort an agent's cell's rounds run at; an engine's cell has none.
    pub effort: Option<String>,
    /// The work's branch takes the latest main in as each run of the cell starts.
    pub takes_main: bool,
    /// When the cell runs at all: every key of its `when` holds, in the order written; none is always.
    pub when: Vec<Condition>,
    /// What the work waits for before the cell: every key of its `requires`, in the order written.
    pub requires: Vec<Condition>,
    /// The blocks it names: those that run it, in the order it names them, then those it is only given.
    pub inputs: Vec<String>,
    /// The blocks whose new version runs it, in the order it names them.
    pub runs_on: Vec<String>,
    /// The blocks it is given, its session kept supplied with their content, in the order it names them; none for an
    /// engine's cell.
    pub given: Vec<String>,
    /// For each block it names, by the block's name, whether it runs the cell and whether the cell is given it, with
    /// why and how where the definition says.
    pub input: BTreeMap<String, Input>,
    /// The blocks it may write, as the definition lists them.
    pub outputs: Vec<String>,
    /// The blocks among those that every hand-in of it includes, as the definition lists them.
    pub must_write: Vec<String>,
    /// The tools of the engine's that an agent's cell may ask for in its return, by name, as the definition lists
    /// them; none for an engine's cell.
    pub tools: Vec<String>,
    /// What runs it, each a trigger by its name, with the block it stands on, the data its agent reads for it and what
    /// the cell does when it fires, as the definition writes them.
    #[serde(default)]
    pub triggers: Vec<TriggerView>,
    pub limits: Limits,
}

/// A group of the flow's blocks, whose trigger fires for a new version of any of them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub name: String,
    pub blocks: Vec<String>,
}

/// A trigger of a cell, as the definition writes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerView {
    pub name: String,
    /// What it is, as its file writes it; none where it says nothing.
    pub what: Option<String>,
    /// The block it stands on; of a group's trigger, the group's name.
    pub block: String,
    /// Of a group's trigger, the group's blocks, each of which runs the cell; none of another.
    pub group: Vec<String>,
    pub read: Vec<String>,
    #[serde(rename = "do")]
    pub does: Option<String>,
}

/// One of the blocks a cell names: whether it runs the cell, and whether the cell is given it. It does one or both.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    /// A new version of it runs the cell; none where it does not.
    pub runs: Option<Runs>,
    /// The cell is given it; none where it is not.
    pub given: Option<GivenAs>,
}

/// Why a block runs a cell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Runs {
    /// None where the definition does not say.
    pub why: Option<String>,
}

/// How a cell is given a block, and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GivenAs {
    /// What the context of the cell's session must hold of the block when the cell runs; none where the definition
    /// does not say.
    pub kind: Option<InputKind>,
    /// Why the cell is given the block; none where the definition does not say.
    pub why: Option<String>,
}

impl Input {
    /// Of each block the cell names, by its name, whether it runs the cell and whether the cell is given it.
    fn of(spec: &flow::CellSpec) -> BTreeMap<String, Input> {
        let mut out: BTreeMap<String, Input> = BTreeMap::new();
        for r in &spec.runs_on {
            let input = out.entry(r.block.to_string()).or_insert(Input {
                runs: None,
                given: None,
            });
            input.runs = Some(Runs { why: r.why.clone() });
        }
        for g in &spec.given {
            let input = out.entry(g.block.to_string()).or_insert(Input {
                runs: None,
                given: None,
            });
            input.given = Some(GivenAs {
                kind: Some(InputKind::of(g.kind)),
                why: g.why.clone(),
            });
        }
        out
    }
}

/// What the context must hold of a block a cell is given: its latest version, the version the cell last consumed and
/// the latest, or every version.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InputKind {
    Latest,
    LastAndLatest,
    All,
}

impl InputKind {
    pub(crate) fn of(kind: flow::InputKind) -> InputKind {
        match kind {
            flow::InputKind::Latest => InputKind::Latest,
            flow::InputKind::LastAndLatest => InputKind::LastAndLatest,
            flow::InputKind::All => InputKind::All,
        }
    }
}

/// Who carries a cell out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Agent,
    Engine,
}

/// One key of a cell's `when` or `requires`: a block's value, or a named test.
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
    /// Of a choice, the value it counts as before anyone has written it; none where it has none.
    pub default: Option<String>,
    /// What the block is, in a sentence; none where the definition does not say.
    pub what: Option<String>,
    /// The field of a return a cell writes it through, where it is not written as a block; none where it is.
    #[serde(default)]
    pub field: Option<String>,
    /// It stands only for a trigger: no cell is given it, writes it or describes the work by it.
    #[serde(default)]
    pub trigger: bool,
    /// Of a block a source of the engine's writes, what it writes, each text by its name as its file writes them.
    #[serde(default)]
    pub writes: Vec<(String, Vec<crate::given::Piece>)>,
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
    /// What people write in it is a note: their words to the cells that read it.
    pub note: bool,
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
    NoTask,
    EngineHasTask,
    NoAct,
    AgentHasAct {
        act: String,
    },
    /// An engine cell names a reasoning effort: no agent runs it.
    EngineHasEffort,
    /// An agent cell names no reasoning effort.
    NoEffort,
    /// An engine cell names tools, which no agent asks for.
    EngineHasTools,
    UnknownAct {
        act: String,
    },
    UnknownTool {
        tool: String,
    },
    /// A line names a set of rules the flow has no file for.
    NoSuchRules {
        name: String,
    },
    /// A line names among its sets of rules a part the engine places in every system prompt itself.
    EnginePart {
        name: String,
    },
    /// A set of the flow's rules has the name of a part the engine places in every system prompt itself.
    RulesNamedAsEnginePart {
        name: String,
    },
    /// A set of the flow's rules that no line names.
    UnusedRules {
        name: String,
    },
    ValuesEmpty,
    /// The block is a note, and people do not write it.
    NoteNotByPeople {
        block: String,
    },
    /// The block is a note, and a choice.
    NoteIsAChoice {
        block: String,
    },
    NoSuchBlock {
        block: String,
    },
    /// The cell must write the block each time, and it is not among its outputs.
    NotAnOutput {
        block: String,
    },
    /// The cell says what the work is by the block, and the block does not run it.
    DescribedNotRunOn {
        block: String,
    },
    /// In the old form, the cell says how or why it takes the block, and it is not among its inputs.
    TakenNotRead {
        block: String,
    },
    /// The cell names what it takes in two forms of three.
    TwoForms,
    /// The flow names among the engine's own data one that no file beside the flows defines.
    NoSuchData {
        name: String,
    },
    /// The thing is defined in this file and again in another.
    DefinedTwice {
        name: String,
        also: String,
    },
    /// The cell names a trigger that nothing beside the flows defines.
    NoSuchTrigger {
        trigger: String,
    },
    /// The cell names a trigger a rule makes of a datum the flow does not have.
    NoSuchDatum {
        trigger: String,
        datum: String,
    },
    /// The block stands only for a trigger: no cell is given it, writes it or describes the work by it.
    TriggerOnly {
        block: String,
    },
    /// The cell names `triggered_by` and `context` in a flow not laid out one thing to a file.
    NotLaidOut,
    /// A value of a file beside the flows lacks the name that stands for each cell or each datum.
    MissingPlaceholder {
        placeholder: String,
    },
    /// The block is written through a field of a return the engine does not have.
    UnknownReturnField {
        field: String,
    },
    /// The cell writes the work's questions, and no answer of the owner's runs it.
    AsksWithNoAnswer {
        block: String,
    },
    /// The trigger says what happened by a place the engine does not fill.
    UnknownDetail {
        detail: String,
    },
    /// A trigger of an agent cell has its agent read a datum that the cell's context does not hold.
    ReadNotInContext {
        trigger: String,
        block: String,
    },
    /// The cell says what it does for a trigger that is not among its triggers.
    NotATrigger {
        trigger: String,
    },
    /// An engine cell says how it is given the block, and it has no session.
    EngineIsGiven {
        block: String,
    },
    /// In the new form, an agent cell that nothing runs.
    RunByNothing,
    /// In the new form, the cell does not say the kind of a block it is given.
    NoKind {
        block: String,
    },
    /// A table that says nothing of the block it is named for.
    SaysNothing {
        block: String,
    },
    /// The cell says why the block runs it, and it is not among those that run it.
    NotRunOn {
        block: String,
    },
    /// The cell says how or why it is given the block, and it is not given it.
    NotGiven {
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
    /// The file a cell's task is written in is not among the flow's files.
    NoTaskFile {
        file: String,
    },
    /// The file a cell's task is written in holds no words.
    TaskFileEmpty {
        file: String,
    },
    /// The file a cell's task is written in is named by a path that leaves the flow's directory.
    TaskFileOutside {
        file: String,
    },
    /// The block says what it counts as before it is written, and it is not a choice.
    DefaultNotAChoice {
        block: String,
    },
    /// The choice says what it counts as before it is written, and that is not one of its values.
    DefaultNotAValue {
        block: String,
        default: String,
    },
    /// A block is named `test`, which a cell's `when` and `requires` take for a named test.
    BlockNamedTest,
    /// A cell's `when` or `requires` is a table of no keys.
    EmptyCondition,
    /// A group of blocks has the name of a block or a datum of the flow.
    GroupNameTaken {
        group: String,
    },
    /// A group of blocks names no block.
    GroupEmpty {
        group: String,
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
    /// The test of what stands reads a block that this cell, after the one using the test, writes.
    TestWrittenAfter {
        test: String,
        block: String,
        cell: String,
    },
    /// The test of what stands reads a block that this cell writes as well as one before it.
    StandingWrittenByTwo {
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
    Unwritten,
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
                None => {
                    let written = compiled.lines.get(name);
                    lines.push(Line {
                        name: name.clone(),
                        cells: vec![spec.id.to_string()],
                        levels_above: spec.levels_above,
                        file: written.map(|l| l.file.clone()).unwrap_or_default(),
                        reads_only: written.is_some_and(|l| l.reads_only),
                        system: written.map(|l| l.system.clone()).unwrap_or_default(),
                        rules: written.map(|l| l.rules.clone()).unwrap_or_default(),
                    });
                }
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
                task: spec.task.clone(),
                task_file: spec.task_file.clone(),
                act: spec.act.clone(),
                effort: spec.effort.map(|effort| effort.name().to_string()),
                takes_main: spec.takes_main,
                when: cell.applies.iter().map(|c| Condition::of(c, compiled)).collect(),
                requires: cell.requires.iter().map(|c| Condition::of(c, compiled)).collect(),
                inputs: (spec.runs_on.iter().map(|r| &r.block))
                    .chain(
                        (spec.given.iter().map(|g| &g.block)).filter(|b| !spec.runs_on.iter().any(|r| &r.block == *b)),
                    )
                    .map(BlockId::to_string)
                    .collect(),
                runs_on: names(&cell.runs_on),
                given: spec.given.iter().map(|g| g.block.to_string()).collect(),
                input: Input::of(spec),
                outputs: names(&cell.outputs),
                must_write: names(&spec.must_write),
                tools: spec.tools.clone(),
                triggers: (spec.triggers.iter())
                    .map(|t| TriggerView {
                        name: t.name.clone(),
                        what: t.what.clone(),
                        block: t.block.to_string(),
                        group: names(&t.group),
                        read: names(&t.read),
                        does: t.does.clone(),
                    })
                    .collect(),
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
                default: block.default.clone(),
                what: block.what.clone(),
                field: block.field.clone(),
                trigger: block.trigger,
                writes: (block.writes.0.iter())
                    .map(|(name, text)| (name.clone(), flow::system::with_slots(text)))
                    .collect(),
                written_by: WrittenBy {
                    cells: names(&block.writers.cells),
                    people: block.writers.people,
                    note: block.writers.note,
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
            tools: compiled.tools.clone(),
            groups: (compiled.groups.iter())
                .map(|(name, blocks)| Group {
                    name: name.clone(),
                    blocks: blocks.clone(),
                })
                .collect(),
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
    /// The test of this name as the definition declares it: a compiled definition declares every test its cells use.
    fn of(name: &TestName, compiled: &Compiled) -> Self {
        let test = &compiled.flow.tests()[name];
        Test {
            name: name.to_string(),
            words: compiled.tests[name].clone(),
            file: compiled.test_files.get(name).cloned().unwrap_or_default(),
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
            F::NoTask => What::NoTask,
            F::EngineHasTask => What::EngineHasTask,
            F::NoAct => What::NoAct,
            F::AgentHasAct { act } => What::AgentHasAct { act },
            F::EngineHasEffort => What::EngineHasEffort,
            F::NoEffort => What::NoEffort,
            F::EngineHasTools => What::EngineHasTools,
            F::UnknownAct { act } => What::UnknownAct { act },
            F::UnknownTool { tool } => What::UnknownTool { tool },
            F::NoSuchRules { name } => What::NoSuchRules { name },
            F::EnginePart { name } => What::EnginePart { name },
            F::RulesNamedAsEnginePart { name } => What::RulesNamedAsEnginePart { name },
            F::UnusedRules { name } => What::UnusedRules { name },
            F::ValuesEmpty => What::ValuesEmpty,
            F::NoteNotByPeople { block } => What::NoteNotByPeople { block },
            F::NoteIsAChoice { block } => What::NoteIsAChoice { block },
            F::NoSuchBlock { block } => What::NoSuchBlock { block },
            F::NotAnOutput { block } => What::NotAnOutput { block },
            F::DescribedNotRunOn { block } => What::DescribedNotRunOn { block },
            F::TakenNotRead { block } => What::TakenNotRead { block },
            F::TwoForms => What::TwoForms,
            F::NoSuchData { name } => What::NoSuchData { name },
            F::DefinedTwice { name, also } => What::DefinedTwice { name, also },
            F::NoSuchTrigger { trigger } => What::NoSuchTrigger { trigger },
            F::NoSuchDatum { trigger, datum } => What::NoSuchDatum { trigger, datum },
            F::TriggerOnly { block } => What::TriggerOnly { block },
            F::NotLaidOut => What::NotLaidOut,
            F::MissingPlaceholder { placeholder } => What::MissingPlaceholder { placeholder },
            F::UnknownReturnField { field } => What::UnknownReturnField { field },
            F::AsksWithNoAnswer { block } => What::AsksWithNoAnswer { block },
            F::UnknownDetail { detail } => What::UnknownDetail { detail },
            F::ReadNotInContext { trigger, block } => What::ReadNotInContext { trigger, block },
            F::NotATrigger { trigger } => What::NotATrigger { trigger },
            F::EngineIsGiven { block } => What::EngineIsGiven { block },
            F::RunByNothing => What::RunByNothing,
            F::NoKind { block } => What::NoKind { block },
            F::SaysNothing { block } => What::SaysNothing { block },
            F::NotRunOn { block } => What::NotRunOn { block },
            F::NotGiven { block } => What::NotGiven { block },
            F::NoWriter { block } => What::NoWriter { block },
            F::NoReader { block } => What::NoReader { block },
            F::UnknownSource { block, source } => What::UnknownSource { block, source },
            F::NotAChoice { block } => What::NotAChoice { block },
            F::NoTaskFile { file } => What::NoTaskFile { file },
            F::TaskFileEmpty { file } => What::TaskFileEmpty { file },
            F::TaskFileOutside { file } => What::TaskFileOutside { file },
            F::DefaultNotAChoice { block } => What::DefaultNotAChoice { block },
            F::DefaultNotAValue { block, default } => What::DefaultNotAValue { block, default },
            F::BlockNamedTest => What::BlockNamedTest,
            F::EmptyCondition => What::EmptyCondition,
            F::GroupNameTaken { group } => What::GroupNameTaken { group },
            F::GroupEmpty { group } => What::GroupEmpty { group },
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
            F::StandingWrittenByTwo { test, block, cell } => What::StandingWrittenByTwo { test, block, cell },
            F::Flow(fault) => {
                let (reason, cell) = match fault {
                    FlowFault::Empty => (FlowRefusal::Empty, None),
                    FlowFault::Twice(c) => (FlowRefusal::Twice, Some(c)),
                    FlowFault::ConditionOnOwnOutput(c) => (FlowRefusal::ConditionOnOwnOutput, Some(c)),
                    FlowFault::LastHasCondition(c) => (FlowRefusal::LastHasCondition, Some(c)),
                    FlowFault::NoLimit(c) => (FlowRefusal::NoLimit, Some(c)),
                    FlowFault::NoSuchTest(c, _) => (FlowRefusal::NoSuchTest, Some(c)),
                    FlowFault::Unwritten(..) => (FlowRefusal::Unwritten, None),
                };
                What::FlowRefused {
                    reason,
                    cell: cell.map(|c| c.to_string()),
                }
            }
        }
    }
}
