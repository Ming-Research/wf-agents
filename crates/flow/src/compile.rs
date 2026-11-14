//! The definition checked and compiled: the flow the deciding code walks, and what the rest of the engine needs to
//! know of each cell and each block.
//!
//! Every fault a definition has is found at once and given as data, with the file and the field it is in, so that the
//! one who wrote the definition can mend all of them before compiling again.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

use decide::{BlockId, Cell, CellId, Condition, Flow, FlowFault, Outsiders, Test, TestKind, TestName};

use serde::de::DeserializeOwned;

use crate::agents::RuleSetFile;
use crate::source::{
    BLOCKS_DIR, BlockFile, CELLS_DIR, CellFile, DATA_DIR, Effort, FLOW_FILE, FlowFile, InputKind, Keys, Kind,
    LINES_DIR, LaidOutBlockFile, LaidOutFlowFile, LaidOutLineFile, LineFile, OF_A_CELL, OF_A_DATUM, OF_A_GROUP,
    OF_A_LINE, OfADatumFile, RULES_DIR, Source, TESTS_DIR, TRIGGERS_DIR, Task, TestFile, Texts, TriggerFile, WHEN_TEST,
    rules_file, who_writes,
};

/// A flow's definition compiled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Compiled {
    /// The flow's name.
    pub name: String,
    /// The cells as the deciding code walks them, with the named tests: what each tests.
    pub flow: Flow,
    /// What the engine needs to know of each cell beyond what the walk reads, in the order of the walk.
    pub cells: Vec<CellSpec>,
    /// Every block of a work's state, with who writes and who reads it.
    pub blocks: BTreeMap<BlockId, Block>,
    /// The words shown for each named test the cells use, by name; what each tests is in `flow`.
    pub tests: BTreeMap<TestName, String>,
    /// The file each named test is written in, from the flow's directory, by name.
    pub test_files: BTreeMap<TestName, String>,
    /// The lines, by name, as their files write them.
    pub lines: BTreeMap<String, Line>,
    /// The engine's tools every agent cell of the flow may ask for, in the order `flow.toml` names them; none where it
    /// names none, and each agent cell names its own.
    pub tools: Vec<String>,
    /// The groups of the flow's blocks, by name, each with its blocks as `flow.toml` names them.
    pub groups: BTreeMap<String, Vec<String>>,
    /// The limits of the flow as a whole.
    pub limits: Limits,
    /// The text of each of the flow's sets of rules for the agents, by name: each one a line names.
    pub rules: BTreeMap<String, String>,
}

/// A line: the cells of a work that share one session, as its file writes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// The file it is written in, from the flow's directory: `lines/<name>.toml`, or `flow.toml` of a flow written
    /// before it was laid out one thing to a file.
    pub file: String,
    /// Its agents only read.
    pub reads_only: bool,
    /// How many levels above its item's its rounds run at.
    pub levels_above: u32,
    /// Of a flow written before it was laid out, its `system`, as written; none of a flow laid out.
    pub system: Vec<String>,
    /// Of a flow laid out one thing to a file, its `rules`, as written; none of a flow written before.
    pub rules: Vec<String>,
}

/// The limits of everything that repeats, for a cell that sets none of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// How many runs of a cell in a row may be ways round before the work waits for the owner.
    pub runs: u32,
    /// How often a return may be sent back within one run.
    pub returns: u32,
    /// How many runs of a cell in a row may each ask for calls and deal with nothing, before the work waits for the
    /// owner instead of entering the cell again for their results.
    pub calls: u32,
}

/// The source of the block a new version of which is the owner speaking to the cell it runs (design document
/// `data-and-context`, 39).
pub const WORDS: &str = "words";

/// The source of the block a new version of which is the owner answering a question of the cell it runs with another
/// option than the one the work goes by, or one the cell waits for (design document `data-and-context`, 35, 36). The
/// engine writes it as the owner answers.
pub const ANSWER: &str = "answer";

/// The field of a return through which a cell asks the owner its questions, asks them anew or withdraws them (design
/// document `data-and-context`, 32): a block written through it is the work's questions.
pub const QUESTIONS: &str = "questions";

/// The sources outside a work a new version of whose block is the owner turning to the cells it runs: the owner's words
/// to a cell, and an answer ([`WORDS`], [`ANSWER`]). Such a block stops a round under way of a later cell
/// ([`decide::Outsiders`]).
pub const TURNING: [&str; 2] = [WORDS, ANSWER];

/// What the engine that runs a flow has, by name. A definition that names another source, act or tool is refused.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Has {
    /// The sources outside a work that write blocks.
    pub sources: BTreeSet<String>,
    /// The acts of the engine's own that carry out an engine cell.
    pub acts: BTreeSet<String>,
    /// The tools an agent may ask the engine for in its return.
    pub tools: BTreeSet<String>,
    /// The parts that the engine places in the system prompt of every line's session itself. A line names none of
    /// them, and no set of the flow's rules has the name of one.
    pub system: BTreeSet<String>,
    /// The fields of a return that are the engine's: a block written through a field names one of them.
    pub fields: BTreeSet<String>,
    /// The places the engine fills in a run with what happened: a trigger that says what happened names one of them.
    pub details: BTreeSet<String>,
}

/// What the engine needs to know of a cell beyond what the walk reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellSpec {
    pub id: CellId,
    /// What the cell is called on the chart.
    pub title: String,
    /// Who carries the cell out.
    pub kind: Kind,
    /// The line whose session an agent cell runs in; an engine cell has none.
    pub line: Option<String>,
    /// The agents of the cell's line only read, as the line says; an engine cell's do not.
    pub reads_only: bool,
    /// What an agent cell's agent is told to do; an engine cell has none.
    pub task: Option<String>,
    /// The file the task is written in, from the flow's directory, where it is not written in place.
    pub task_file: Option<String>,
    /// The act of the engine's own that carries out an engine cell; an agent cell has none.
    pub act: Option<String>,
    /// The blocks among its outputs that every hand-in of the cell includes, in the order written: a hand-in that lacks
    /// one is sent back.
    pub must_write: Vec<BlockId>,
    /// The blocks among those that run it that say what the work is where the host shows it, in the order written.
    pub describes: Vec<BlockId>,
    /// The blocks whose new version runs the cell, in the order it names them.
    pub runs_on: Vec<RunsOn>,
    /// The blocks the cell is given, its session kept supplied with their content, in the order it names them; none
    /// for an engine cell, which has no session.
    pub given: Vec<Given>,
    /// The tools of the engine's that the cell's agent may ask for in its return, in the order written: those its
    /// flow names for every agent cell, where it names any, else the cell's own; none for an engine cell.
    pub tools: Vec<String>,
    /// The flow's sets of rules for the agents that the system prompt of the session of the cell's line holds, by
    /// name, in the order its line names them, between the engine's parts; none for an engine cell.
    pub system: Vec<String>,
    /// How often a return of the cell may be sent back within one run.
    pub returns: u32,
    /// How many of the cell's runs in a row may each ask for calls and deal with nothing, before the work waits for the
    /// owner instead of entering it again for their results.
    pub calls: u32,
    /// The reasoning effort an agent cell's rounds run at; an engine cell has none.
    pub effort: Option<Effort>,
    /// How many levels above its item's the cell's rounds run at, as its line says; none for an engine cell.
    pub levels_above: u32,
    /// The work's branch takes the latest main in as each run of the cell starts, as an agent cell that changes the
    /// code says; an engine cell's never does.
    pub takes_main: bool,
    /// What runs the cell, in the order it names them. Of a cell written in a form before triggers, each block that
    /// runs it is a trigger of the block's name, which its agent reads for it where the cell is given the block.
    pub triggers: Vec<Trigger>,
}

/// A trigger of a cell: an event that runs it, worked out to the block it stands on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trigger {
    /// Its name, as the cell names it.
    pub name: String,
    /// What it is, in a sentence, where the file beside the flows that defines it says: when it fires.
    pub what: Option<String>,
    /// The block whose new version is the event; of a group's trigger, the group's name, any of whose blocks' new
    /// version is.
    pub block: BlockId,
    /// Of a group's trigger, the group's blocks, each of which runs the cell; none of another.
    pub group: Vec<BlockId>,
    /// The data the cell's agent reads for it, each among those the cell is given.
    pub read: Vec<BlockId>,
    /// What the engine says of what happened, where it says something: the name of a place it fills in a run.
    pub detail: Option<String>,
    /// What the cell does for it: as the cell says, else as the trigger says; none where neither does.
    pub does: Option<String>,
}

/// A block whose new version runs a cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunsOn {
    pub block: BlockId,
    /// What the cell's agent does when the block is new; none where the definition does not say.
    pub why: Option<String>,
}

/// A block a cell is given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Given {
    pub block: BlockId,
    /// What the context of the cell's session must hold of the block when the cell runs.
    pub kind: InputKind,
    /// What the cell's agent does with the block; none where the definition does not say.
    pub why: Option<String>,
}

/// A block of a work's state, as the engine needs to know it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    /// The file it is declared in, from the flow's directory: its own in `blocks/` or `../data/`, the trigger's that
    /// declares a block that stands only for it, or `flow.toml` of a flow written before it was laid out one thing to a file.
    pub file: String,
    /// The values of a block that is a choice; a block with none is not one.
    pub values: Option<Vec<String>>,
    /// Of a choice, the value it counts as before anyone has written it; none where it has none.
    pub default: Option<String>,
    /// What the block is, in a sentence, where the definition says it.
    pub what: Option<String>,
    /// The field of a return that a cell writes the block through, where it writes it through one.
    pub field: Option<String>,
    /// The block stands only for a trigger, which declares it: no cell is given it, writes it or describes the work by
    /// it. A source of the engine's that fires the trigger may write words into it, as its `writes` give them.
    pub trigger: bool,
    /// Of a block a source of the engine's writes, what it writes: each text by its name, in the order written, as the
    /// block's file or the trigger that declares it says; none where neither does.
    pub writes: Texts,
    /// Who writes it.
    pub writers: Writers,
    /// Who reads it.
    pub readers: Readers,
}

/// Who writes a block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Writers {
    /// The cells that have it among their outputs, in the order of the walk.
    pub cells: Vec<CellId>,
    /// People write it.
    pub people: bool,
    /// What people write in it is a note: their words to the cells that read it, which a paused work still answers.
    pub note: bool,
    /// The source outside the work that writes it, where one does.
    pub outside: Option<String>,
}

/// Who reads a block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Readers {
    /// The cells that name it, that it runs or that are given it, in the order of the walk.
    pub cells: Vec<CellId>,
    /// The cells whose `when` or `requires` reads its value, in the order of the walk.
    pub conditions: Vec<CellId>,
    /// The named tests that read its value and that a cell uses, by name.
    pub tests: Vec<TestName>,
    /// The owner's page reads it.
    pub shown: bool,
}

/// Something wrong with a definition, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
    /// The file it is in, by its path from the flow's directory: `flow.toml`, a cell's (`<cell id>.toml`, or
    /// `cells/<cell id>.toml` in a flow laid out one thing to a file), or the file of another thing it is laid out in.
    pub file: String,
    /// The path of the field at fault within the file, such as `inputs[2]` or `blocks.plan.values`; empty when the
    /// whole file is.
    pub field: String,
    /// What is wrong there.
    pub what: FaultKind,
}

/// What can be wrong with a definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FaultKind {
    /// The file is not TOML, or does not have the shape its kind of file must have; the parser's words say how.
    Unreadable { message: String },
    /// The order names no cell.
    OrderEmpty,
    /// The order names a cell that has no file.
    NoCellFile { cell: String },
    /// A cell's file is not named in the order.
    NotInOrder { cell: String },
    /// The name is given a second time in the same list; or, named `tools`, an agent cell names tools of its own in a
    /// flow whose `flow.toml` names the tools of every agent cell of it.
    Twice { name: String },
    /// A limit is not a whole number from one to the largest count the engine keeps.
    LimitOutOfRange { value: i64 },
    /// The cell names no line, and needs one: an agent cell runs in its line's session, and a trigger that every line
    /// has its own of stands on the block of its cell's line.
    NoLine,
    /// An engine cell names a line: it runs in no session.
    EngineHasLine { line: String },
    /// The line is not declared in `flow.toml`.
    NoSuchLine { line: String },
    /// An agent cell has no task, or one of no words: its agent would be told nothing to do.
    NoTask,
    /// The file a cell's task is written in is not among the flow's files.
    NoTaskFile { file: String },
    /// The file a cell's task is written in holds no words.
    TaskFileEmpty { file: String },
    /// The file a cell's task is written in is named by a path that leaves the flow's directory.
    TaskFileOutside { file: String },
    /// An engine cell has a task: no agent reads it.
    EngineHasTask,
    /// An engine cell names no act to carry it out.
    NoAct,
    /// An agent cell names an act: an agent carries the cell out.
    AgentHasAct { act: String },
    /// An engine cell names a reasoning effort: no agent runs it.
    EngineHasEffort,
    /// An agent cell names no reasoning effort: how hard its rounds think is the flow's to say.
    NoEffort,
    /// An engine cell names tools: no agent asks for them.
    EngineHasTools,
    /// The cell names an act the engine does not have.
    UnknownAct { act: String },
    /// The cell names a tool the engine does not have.
    UnknownTool { tool: String },
    /// A line names a set of rules for its agents that the flow does not have: there is no `rules/<name>.md`.
    NoSuchRules { name: String },
    /// A line names among its sets of rules a part that the engine places in every system prompt itself.
    EnginePart { name: String },
    /// A set of the flow's rules has the name of a part that the engine places in every system prompt itself.
    RulesNamedAsEnginePart { name: String },
    /// A set of the flow's rules that no line names: no agent would be given it.
    UnusedRules { name: String },
    /// A choice has no values to choose from.
    ValuesEmpty,
    /// The block is a note, and people do not write it: a note is their words to the cells that read it.
    NoteNotByPeople { block: String },
    /// The block is a note, and a choice: a note is words to a cell, which a choice is not.
    NoteIsAChoice { block: String },
    /// The block is not declared in `flow.toml`.
    NoSuchBlock { block: String },
    /// The cell must write the block each time, and it is not among its outputs.
    NotAnOutput { block: String },
    /// The cell says what the work is by the block, and the block does not run it: a change of it would not run the
    /// cell again, and what the host shows would fall behind.
    DescribedNotRunOn { block: String },
    /// In the old form, the cell says how or why it takes the block, and the block is not among its `inputs`.
    TakenNotRead { block: String },
    /// The cell names what it takes in two forms of three: `inputs`; `runs_on` and `given`; `triggered_by` and `context`.
    TwoForms,
    /// The flow names among the engine's own data one that no file of `../data/` defines.
    NoSuchData { name: String },
    /// The thing is defined in this file and in another, `also`: two files of one name, a block the flow defines that a
    /// trigger declares as well, two triggers that declare one block, or a trigger of a file and of a rule of one name.
    DefinedTwice { name: String, also: String },
    /// The cell names a trigger that nothing beside the flows defines: no file of `../triggers/` or of its
    /// `of_a_cell/` has its name, and no rule of `../triggers/of_a_datum/` makes it of any datum.
    NoSuchTrigger { trigger: String },
    /// The cell names a trigger that a rule of `../triggers/of_a_datum/` makes of a datum, and the flow has no such
    /// datum.
    NoSuchDatum { trigger: String, datum: String },
    /// A trigger of an agent cell says its agent reads the datum for it, and the cell's context does not hold it.
    ReadNotInContext { trigger: String, block: String },
    /// The cell says what it does for a trigger, and the trigger is not among its triggers.
    NotATrigger { trigger: String },
    /// The block stands only for a trigger: no cell is given it, writes it or describes the work by it.
    TriggerOnly { block: String },
    /// The cell names what it takes by `triggered_by` and `context`, which only a flow laid out one thing to a file has: the
    /// triggers are beside the flows.
    NotLaidOut,
    /// A value of a file beside the flows that stands for each cell, each datum or each group does not hold the name
    /// that stands for it, `{cell}`, `{datum}` or `{group}`.
    MissingPlaceholder { placeholder: String },
    /// The block is written through a field of a return that the engine does not have.
    UnknownReturnField { field: String },
    /// The cell writes the block, which holds the work's questions ([`QUESTIONS`]), and no block of the source
    /// [`ANSWER`] runs it: the owner's answer to a question it asks would run no cell.
    AsksWithNoAnswer { block: String },
    /// The trigger says what happened by a place the engine does not fill.
    UnknownDetail { detail: String },
    /// An engine cell says how it is given the block: it has no session to keep supplied with it.
    EngineIsGiven { block: String },
    /// In the new form, an agent cell that nothing runs: it would never be entered.
    RunByNothing,
    /// In the new form, the cell does not say of a block it is given what its session must hold of it.
    NoKind { block: String },
    /// A table that says nothing of the block it is named for.
    SaysNothing { block: String },
    /// The cell says why the block runs it, and the block is not among those that run it.
    NotRunOn { block: String },
    /// The cell says how or why it is given the block, and it is not given the block.
    NotGiven { block: String },
    /// The block has no writer: no cell of the order, no person and nothing outside writes it.
    NoWriter { block: String },
    /// The block has no reader: no cell of the order reads it, no condition and no named test reads its value, and
    /// the owner's page does not show it.
    NoReader { block: String },
    /// The block names a source outside the work that the engine does not have.
    UnknownSource { block: String, source: String },
    /// A condition or a named test reads a block that is not a choice.
    NotAChoice { block: String },
    /// A condition or a named test waits for a value the choice does not offer.
    NotAValue { block: String, is: String },
    /// The block says what it counts as before it is written, and it is not a choice.
    DefaultNotAChoice { block: String },
    /// The choice says what it counts as before it is written, and that is not one of its values.
    DefaultNotAValue { block: String, default: String },
    /// A block is named `test`, which a cell's `when` and `requires` take for a named test.
    BlockNamedTest,
    /// A cell's `when` or `requires` is a table of no keys: it says nothing that must hold.
    EmptyCondition,
    /// A group of blocks has the name of a block or a datum of the flow.
    GroupNameTaken { group: String },
    /// A group of blocks names no block.
    GroupEmpty { group: String },
    /// A condition reads a block its own cell writes, and another writes it too: the cell would decide by what it
    /// writes itself, beside another, whether it runs. Where the cell is the block's only writer, the block says which
    /// of two cells is in charge, and is the cell's own to decide.
    ConditionOnOwnOutput { block: String },
    /// A condition reads a block that neither a cell before it, people nor the outside write, so that it is decided
    /// by nothing that comes before the cell.
    ConditionNotWrittenBefore { block: String },
    /// The last cell has a condition: a work ends when its last cell has a record, so the last cell always runs.
    LastHasCondition,
    /// A cell uses a test that `flow.toml` does not declare.
    UndeclaredTest { test: String },
    /// A declared test reads no block.
    ReadsEmpty { test: String },
    /// A declared test is used by no cell.
    UnusedTest { test: String },
    /// A cell's precondition is a test that reads a block the cell itself writes, or its condition one that reads a
    /// block it writes beside another writer: the cell would decide by what it writes itself whether the work waits
    /// before it, or, beside another, whether it runs.
    TestOnOwnOutput { test: String, block: String },
    /// A cell's condition or precondition is a test that reads a block that neither a cell before it, people nor the
    /// outside write, so that it is decided by nothing that comes before the cell.
    TestNotWrittenBefore { test: String, block: String },
    /// A cell's condition or precondition is a test of what stands that reads a block this cell, after it in the
    /// order, writes. Whether a version stands is whether its writer is skipped, which the walk decides from the place
    /// of the cell asking: the versions such a test reads are to be written before that cell.
    TestWrittenAfter { test: String, block: String, cell: String },
    /// A test of what stands reads a block that this cell writes as well as another before it. Such a test reads the
    /// current version alone, and a skipped cell's write would hide the other's word.
    StandingWrittenByTwo { test: String, block: String, cell: String },
    /// The deciding code refuses the cells as a flow.
    Flow(FlowFault),
}

/// Checks a flow's definition against what the engine has, by name, and compiles it; or gives every fault it has, in
/// the order of the files (`flow.toml`, then the cells in the order of the walk, then any other) and, within a file, in
/// the order they were found. When a file cannot be read, only the files that cannot be read are reported.
pub fn compile(source: &Source, has: &Has) -> Result<Compiled, Vec<Fault>> {
    let parsed = parse(source)?;
    let flow_file = &parsed.flow;
    let def = Definition::new(&parsed);
    let mut faults = Vec::new();

    def.check_order(&mut faults);
    let mut limit = |field: &str, value: i64| {
        count(value).unwrap_or_else(|what| {
            faults.push(fault(FLOW_FILE, field, what));
            0
        })
    };
    let limits = Limits {
        runs: limit("limits.runs", flow_file.limits.runs),
        returns: limit("limits.returns", flow_file.limits.returns),
        calls: limit("limits.calls", flow_file.limits.calls),
    };
    faults.extend(parsed.faults.iter().cloned());
    for (field, what) in check_list(&parsed.tools, "tools", |tool| {
        let what = FaultKind::UnknownTool { tool: tool.to_string() };
        (!has.tools.contains(tool)).then_some(what)
    }) {
        faults.push(fault(FLOW_FILE, field, what));
    }
    def.check_lines(&has.system, &source.rules, &mut faults);
    def.check_blocks(has, &mut faults);
    def.check_groups(&mut faults);
    def.check_tests(&mut faults);
    for (path, trigger, _) in &parsed.trigger_files {
        if let Some(detail) = trigger.detail.as_ref().filter(|d| !has.details.contains(*d)) {
            let what = FaultKind::UnknownDetail { detail: detail.clone() };
            faults.push(fault(path, "detail", what));
        }
    }

    let mut cells = Vec::new();
    let mut specs = Vec::new();
    for (place, id, file) in def.ordered() {
        let (cell, spec) = def.cell(place, id, file, &limits, has, &mut faults);
        cells.push(cell);
        specs.push(spec);
    }

    if !faults.is_empty() {
        sort(&mut faults, Some(&def.place), &parsed.places);
        return Err(faults);
    }
    let tests = (flow_file.tests.iter())
        .map(|(name, test)| {
            let test = Test {
                kind: test.kind,
                reads: test.reads.iter().map(|b| BlockId(b.clone())).collect(),
                is: test.is.clone(),
            };
            (TestName(name.clone()), test)
        })
        .collect();
    let outsiders = (flow_file.blocks.iter())
        .filter(|(_, b)| b.people || b.outside.is_some())
        .map(|(name, b)| {
            let outsiders = Outsiders {
                people: b.people,
                note: b.note,
                outside: b.outside.clone(),
                interrupts: b.outside.as_deref().is_some_and(|source| TURNING.contains(&source)),
            };
            (BlockId(name.clone()), outsiders)
        })
        .collect();
    let defaults = (flow_file.blocks.iter())
        .filter_map(|(name, b)| Some((BlockId(name.clone()), b.default.clone()?)))
        .collect();
    let flow = Flow::new(cells, tests, outsiders)
        .map_err(|f| vec![fault(FLOW_FILE, "order", FaultKind::Flow(f))])?
        .with_defaults(defaults);
    Ok(Compiled {
        name: flow_file.name.clone(),
        flow,
        cells: specs,
        blocks: def.blocks(),
        tests: (flow_file.tests.iter())
            .map(|(name, test)| (TestName(name.clone()), test.says.clone()))
            .collect(),
        test_files: (flow_file.tests.keys())
            .map(|name| (TestName(name.clone()), parsed.places.file("tests", name)))
            .collect(),
        lines: (flow_file.lines.iter())
            .map(|(name, line)| {
                let laid_out = parsed.places.laid_out;
                let line = Line {
                    file: parsed.places.file("lines", name),
                    reads_only: line.reads_only,
                    levels_above: line.levels_above,
                    system: if laid_out { Vec::new() } else { line.system.clone() },
                    rules: if laid_out { line.system.clone() } else { Vec::new() },
                };
                (name.clone(), line)
            })
            .collect(),
        tools: parsed.tools.clone(),
        groups: flow_file.groups.clone(),
        limits,
        // Every set of rules is one a line names, or it would have been a fault.
        rules: source.rules.clone(),
    })
}

/// The files read into their shapes, those of a flow laid out one thing to a file gathered into the shape of one
/// written before it, with where each thing is written and what each cell's triggers are worked out to.
struct Parsed {
    flow: FlowFile,
    /// The text of each cell's task written in a file of its own, by the cell's id.
    tasks: BTreeMap<String, String>,
    /// Of a flow laid out one thing to a file, the tools its `flow.toml` names for every agent cell of it.
    tools: Vec<String>,
    cells: BTreeMap<String, CellFile>,
    places: Places,
    triggers: Resolved,
    /// The blocks that stand only for the trigger that declares each.
    trigger_blocks: BTreeSet<String>,
    /// Of a flow laid out one thing to a file, the names of the sets of rules a line may take, as
    /// [`crate::agents::RULES_FILE`] names them.
    rule_sets: BTreeSet<String>,
    /// The triggers beside the flows, by their files, each with whose own it is.
    trigger_files: Vec<(String, TriggerFile, Whose)>,
    /// What is wrong with how the files make one definition, found as they are gathered: a thing defined twice, a
    /// datum of the engine's the flow names that is not there, and a file beside the flows that lacks the name that
    /// stands for each cell or each datum.
    faults: Vec<Fault>,
}

/// Where the things of a definition are written.
#[derive(Default)]
struct Places {
    /// The flow is laid out one thing to a file.
    laid_out: bool,
    /// Of a flow laid out one thing to a file, the file of each line, block and named test, by the table that holds
    /// that kind of thing in the `flow.toml` of one written before (`lines`, `blocks`, `tests`) and the thing's name.
    files: BTreeMap<(&'static str, String), String>,
    /// The place in `data` of each datum of the engine's that the flow names.
    data: BTreeMap<String, usize>,
}

impl Places {
    /// The file a line, block or named test of this name is written in, by the table that holds that kind of thing in
    /// the `flow.toml` of a flow written before it was laid out: its own, or `flow.toml`.
    fn file(&self, table: &'static str, name: &str) -> String {
        (self.files.get(&(table, name.to_string()))).map_or_else(|| FLOW_FILE.to_string(), String::clone)
    }

    /// The file of the cell of this id.
    fn cell(&self, id: &str) -> String {
        if self.laid_out {
            format!("{CELLS_DIR}/{id}.toml")
        } else {
            format!("{id}.toml")
        }
    }

    /// The id of the cell whose file this is, where it may be a cell's.
    fn id<'f>(&self, file: &'f str) -> Option<&'f str> {
        let name = if self.laid_out {
            file.strip_prefix(CELLS_DIR)?.strip_prefix('/')?
        } else {
            file
        };
        name.strip_suffix(".toml")
    }

    /// A fault at this field of the thing of this kind and name, the field empty where the whole thing is at fault:
    /// in the file of its own where it has one, else in `flow.toml`, under the thing's table.
    fn fault(&self, table: &'static str, name: &str, field: &str, what: FaultKind) -> Fault {
        match self.files.get(&(table, name.to_string())) {
            Some(file) => fault(file, field, what),
            None if field.is_empty() => fault(FLOW_FILE, format!("{table}.{name}"), what),
            None => fault(FLOW_FILE, format!("{table}.{name}.{field}"), what),
        }
    }

    /// A fault of who writes or reads the block of this name. Of a datum of the engine's that the flow names, it is
    /// where the flow names it: the datum is every flow's, and how it is used is this flow's. Of another, it is on the
    /// block.
    fn use_fault(&self, name: &str, what: FaultKind) -> Fault {
        match self.data.get(name) {
            Some(i) => fault(FLOW_FILE, format!("data[{i}]"), what),
            None => self.fault("blocks", name, "", what),
        }
    }
}

/// Reads each file into its shape, or gives the faults of every file that does not fit.
fn parse(source: &Source) -> Result<Parsed, Vec<Fault>> {
    let mut faults = Vec::new();
    let mut places = Places {
        laid_out: source.laid_out,
        ..Places::default()
    };
    let mut cells = BTreeMap::new();
    for (id, text) in &source.cells {
        if let Some(cell) = shape::<CellFile>(&places.cell(id), text, &mut faults) {
            cells.insert(id.clone(), cell);
        }
    }
    let tasks = task_files(source, &cells, &places, &mut faults);
    let parsed = if source.laid_out {
        let flow = shape::<LaidOutFlowFile>(FLOW_FILE, &source.flow, &mut faults);
        let files = LaidOut::parse(&source.files, &mut faults);
        flow.map(|flow| (flow.data.clone(), flow.tools.clone(), FlowFile::from(flow), files))
    } else {
        let flow = shape::<FlowFile>(FLOW_FILE, &source.flow, &mut faults);
        flow.map(|flow| (Vec::new(), Vec::new(), flow, LaidOut::default()))
    };
    match parsed {
        Some((data, tools, mut flow, files)) if faults.is_empty() => {
            let rule_sets = files.gather(&mut flow, &data, &mut places, &mut faults);
            let (triggers, trigger_blocks) = files.resolve(&cells, &mut flow, &mut places, &mut faults);
            Ok(Parsed {
                flow,
                tools,
                cells,
                tasks,
                places,
                triggers,
                trigger_blocks,
                rule_sets,
                trigger_files: files.triggers,
                faults,
            })
        }
        parsed => {
            let place = (parsed.as_ref()).map(|(_, _, flow, _)| first_places(&flow.order, &source.cells));
            sort(&mut faults, place.as_ref(), &places);
            Err(faults)
        }
    }
}

/// The text of each cell's task that is written in a file of its own, by the cell's id, read from the flow's files. A
/// fault is a path that leaves the flow's directory, a file the flow does not have, and one that holds no words, each on
/// the cell's `task.file`.
fn task_files(
    source: &Source,
    cells: &BTreeMap<String, CellFile>,
    places: &Places,
    faults: &mut Vec<Fault>,
) -> BTreeMap<String, String> {
    use std::path::{Component, Path};
    let mut tasks = BTreeMap::new();
    for (id, cell) in cells {
        let Some(file) = cell.task.as_ref().and_then(Task::file) else {
            continue;
        };
        let inside = Path::new(file)
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
        let path: Vec<&str> = file.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
        let named = file.to_string();
        let what = match source.files.get(&path.join("/")) {
            _ if !inside => FaultKind::TaskFileOutside { file: named },
            None => FaultKind::NoTaskFile { file: named },
            Some(text) if text.trim().is_empty() => FaultKind::TaskFileEmpty { file: named },
            Some(text) => {
                tasks.insert(id.clone(), text.clone());
                continue;
            }
        };
        faults.push(fault(&places.cell(id), "task.file", what));
    }
    tasks
}

/// What each trigger each cell names is worked out to, by the cell's id: the trigger, or what is wrong with it, by its
/// place in the cell's `triggered_by`. A name given a second time there is not worked out again.
type Resolved = BTreeMap<String, Vec<(usize, Result<Trigger, FaultKind>)>>;

/// What stands for the id of a cell, in small letters, in a trigger each cell has its own of.
const CELL: &str = "{cell}";

/// What stands for the name of a line in a trigger each line has its own of.
const LINE: &str = "{line}";

/// Whose own a trigger beside the flows is, by where its file is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Whose {
    /// Its own: it stands on the one block it declares, whichever cell names it.
    Its,
    /// Each cell's that names it, in `of_a_cell/`: `{cell}` stands for the cell's id in small letters.
    Cell,
    /// Each line's, in `of_a_line/`: `{line}` stands for the line of the cell that names it, and the cells of one line
    /// share it.
    Line,
}

impl Whose {
    /// What stands in its texts for whose own it is, where it is not its own.
    fn placeholder(self) -> Option<&'static str> {
        match self {
            Whose::Its => None,
            Whose::Cell => Some(CELL),
            Whose::Line => Some(LINE),
        }
    }
}

/// What stands for the name of a datum in a rule that makes a trigger of each datum.
const DATUM: &str = "{datum}";

/// What stands for the name of a group of blocks in a rule that makes a trigger of each group; alone in its `read`, the
/// group's blocks.
const GROUP: &str = "{group}";

/// The text of a file read into its shape, or none, with the fault that it cannot be read.
fn shape<T: DeserializeOwned>(file: &str, text: &str, faults: &mut Vec<Fault>) -> Option<T> {
    toml::from_str(text)
        .map_err(|e| faults.push(unreadable(file.to_string(), &e)))
        .ok()
}

impl From<LaidOutFlowFile> for FlowFile {
    fn from(flow: LaidOutFlowFile) -> Self {
        FlowFile {
            name: flow.name,
            order: flow.order,
            limits: flow.limits,
            lines: BTreeMap::new(),
            blocks: BTreeMap::new(),
            tests: BTreeMap::new(),
            groups: flow.groups,
        }
    }
}

/// The name of the thing a file laid out one thing to a file defines: its file's name without `.toml`.
fn stem(path: &str) -> &str {
    let name = path.rsplit_once('/').map_or(path, |(_, name)| name);
    name.strip_suffix(".toml").unwrap_or(name)
}

/// The files of a flow laid out one thing to a file but `flow.toml` and its cells', and those beside the flows it may
/// use, each read into its shape, with its path, in the order of the paths.
#[derive(Default)]
struct LaidOut {
    lines: Vec<(String, LaidOutLineFile)>,
    blocks: Vec<(String, LaidOutBlockFile)>,
    tests: Vec<(String, TestFile)>,
    data: Vec<(String, LaidOutBlockFile)>,
    /// The triggers by their files, each with whose own it is.
    triggers: Vec<(String, TriggerFile, Whose)>,
    of_a_datum: Vec<(String, OfADatumFile)>,
    /// The rules that make a trigger of each group of the flow's blocks, by their files.
    of_a_group: Vec<(String, OfADatumFile)>,
    rule_sets: Option<BTreeMap<String, RuleSetFile>>,
}

impl LaidOut {
    /// Reads each file into its shape, by where it is; a file in no place a flow is made of is left out.
    fn parse(files: &BTreeMap<String, String>, faults: &mut Vec<Fault>) -> Self {
        let mut laid = LaidOut::default();
        let under = |path: &str, dir: &str| path.strip_prefix(dir).is_some_and(|rest| rest.starts_with('/'));
        let rules = rules_file();
        for (path, text) in files {
            let path = path.clone();
            if under(&path, LINES_DIR) {
                laid.lines.extend(shape(&path, text, faults).map(|f| (path, f)));
            } else if under(&path, BLOCKS_DIR) {
                laid.blocks.extend(shape(&path, text, faults).map(|f| (path, f)));
            } else if under(&path, TESTS_DIR) {
                laid.tests.extend(shape(&path, text, faults).map(|f| (path, f)));
            } else if under(&path, DATA_DIR) {
                laid.data.extend(shape(&path, text, faults).map(|f| (path, f)));
            } else if under(&path, &format!("{TRIGGERS_DIR}/{OF_A_DATUM}")) {
                laid.of_a_datum.extend(shape(&path, text, faults).map(|f| (path, f)));
            } else if under(&path, &format!("{TRIGGERS_DIR}/{OF_A_GROUP}")) {
                laid.of_a_group.extend(shape(&path, text, faults).map(|f| (path, f)));
            } else if under(&path, TRIGGERS_DIR) {
                let whose = if under(&path, &format!("{TRIGGERS_DIR}/{OF_A_CELL}")) {
                    Whose::Cell
                } else if under(&path, &format!("{TRIGGERS_DIR}/{OF_A_LINE}")) {
                    Whose::Line
                } else {
                    Whose::Its
                };
                laid.triggers
                    .extend(shape(&path, text, faults).map(|f| (path, f, whose)));
            } else if path == rules {
                laid.rule_sets = shape(&path, text, faults);
            }
        }
        laid
    }

    /// Gathers the lines, blocks and named tests into the flow, each by the name of its file, with the data of the
    /// engine's the flow names among `data`, where it is laid out one thing to a file; and gives the names of the sets
    /// of rules a line may take. A fault is each second file of one name, on it, naming the first; a datum the flow
    /// names that `../data/` does not define, or that the flow defines as well; a trigger each cell or each line has
    /// its own of whose block does not hold `{cell}` or `{line}`; and a rule of a datum whose name does not hold
    /// `{datum}`, or that makes the names another rule makes.
    fn gather(
        &self,
        flow: &mut FlowFile,
        data: &[String],
        places: &mut Places,
        faults: &mut Vec<Fault>,
    ) -> BTreeSet<String> {
        fn put<T: Clone>(
            things: &mut BTreeMap<String, T>,
            table: &'static str,
            files: &[(String, T)],
            places: &mut Places,
            faults: &mut Vec<Fault>,
        ) {
            for (path, file) in files {
                let name = stem(path).to_string();
                match places.files.get(&(table, name.clone())) {
                    Some(first) => {
                        let also = first.clone();
                        faults.push(fault(path, "", FaultKind::DefinedTwice { name, also }));
                    }
                    None => {
                        things.insert(name.clone(), file.clone());
                        places.files.insert((table, name), path.clone());
                    }
                }
            }
        }
        let lines: Vec<(String, LineFile)> = (self.lines.iter())
            .map(|(path, line)| {
                let line = LineFile {
                    reads_only: line.reads_only,
                    levels_above: line.levels_above,
                    system: line.rules.clone(),
                };
                (path.clone(), line)
            })
            .collect();
        put(&mut flow.lines, "lines", &lines, places, faults);
        let blocks: Vec<(String, BlockFile)> = (self.blocks.iter())
            .map(|(path, block)| (path.clone(), BlockFile::from(block.clone())))
            .collect();
        put(&mut flow.blocks, "blocks", &blocks, places, faults);
        put(&mut flow.tests, "tests", &self.tests, places, faults);

        // What is beside the flows is defined once for every flow: a second file of one name is a fault of each flow.
        let twice = |name: &str, path: &str, first: &str| {
            let what = FaultKind::DefinedTwice {
                name: name.to_string(),
                also: first.to_string(),
            };
            fault(path, "", what)
        };
        let mut defined: BTreeMap<&str, &(String, LaidOutBlockFile)> = BTreeMap::new();
        for file @ (path, _) in &self.data {
            match defined.entry(stem(path)) {
                Entry::Occupied(first) => faults.push(twice(stem(path), path, &first.get().0)),
                Entry::Vacant(entry) => {
                    entry.insert(file);
                }
            }
        }
        // A trigger of its own and one each cell or each line has its own of are named alike, by their files' names.
        let mut named: BTreeMap<&str, &str> = BTreeMap::new();
        for (path, trigger, whose) in &self.triggers {
            match named.entry(stem(path)) {
                Entry::Occupied(first) => faults.push(twice(stem(path), path, first.get())),
                Entry::Vacant(entry) => {
                    entry.insert(path);
                }
            }
            if let Some(placeholder) = whose.placeholder()
                && !trigger.block.contains(placeholder)
            {
                let what = FaultKind::MissingPlaceholder {
                    placeholder: placeholder.to_string(),
                };
                faults.push(fault(path, "block", what));
            }
        }
        for (path, rule) in &self.of_a_group {
            if !rule.name.contains(GROUP) {
                let what = FaultKind::MissingPlaceholder {
                    placeholder: GROUP.to_string(),
                };
                faults.push(fault(path, "name", what));
            }
        }
        let mut made: BTreeMap<&str, &str> = BTreeMap::new();
        for (path, rule) in &self.of_a_datum {
            if !rule.name.contains(DATUM) {
                let what = FaultKind::MissingPlaceholder {
                    placeholder: DATUM.to_string(),
                };
                faults.push(fault(path, "name", what));
            } else if let Some(first) = made.get(rule.name.as_str()) {
                let mut refused = twice(&rule.name, path, first);
                refused.field = "name".to_string();
                faults.push(refused);
            } else {
                made.insert(&rule.name, path);
            }
        }

        for (i, name) in data.iter().enumerate() {
            let what = if data[..i].contains(name) {
                FaultKind::Twice { name: name.clone() }
            } else if let Some(own) = places.files.get(&("blocks", name.clone())) {
                FaultKind::DefinedTwice {
                    name: name.clone(),
                    also: own.clone(),
                }
            } else if let Some((path, block)) = defined.get(name.as_str()) {
                flow.blocks.insert(name.clone(), BlockFile::from(block.clone()));
                places.files.insert(("blocks", name.clone()), path.clone());
                places.data.insert(name.clone(), i);
                continue;
            } else {
                FaultKind::NoSuchData { name: name.clone() }
            };
            faults.push(fault(FLOW_FILE, format!("data[{i}]"), what));
        }
        (self.rule_sets.iter()).flat_map(|sets| sets.keys().cloned()).collect()
    }

    /// The rules of `of_a_datum/` that make a trigger of this name, each by its file, with the datum it makes it of.
    fn made(&self, name: &str) -> Vec<(&String, &OfADatumFile, String)> {
        (self.of_a_datum.iter())
            .filter_map(|(path, rule)| {
                let (before, after) = rule.name.split_once(DATUM)?;
                let datum = name.strip_prefix(before)?.strip_suffix(after)?;
                (!datum.is_empty()).then(|| (path, rule, datum.to_string()))
            })
            .collect()
    }

    /// The rule of `of_a_group/` that makes a trigger of this name of a group of the flow's blocks, with the group's
    /// name. A group named as a block of the flow makes none: that is a fault of the group, and the block's own
    /// trigger stands.
    fn grouped(&self, name: &str, flow: &FlowFile) -> Option<(&OfADatumFile, String)> {
        (self.of_a_group.iter()).find_map(|(_, rule)| {
            let (before, after) = rule.name.split_once(GROUP)?;
            let group = name.strip_prefix(before)?.strip_suffix(after)?;
            (flow.groups.contains_key(group) && !flow.blocks.contains_key(group)).then(|| (rule, group.to_string()))
        })
    }

    /// Works out each trigger each cell names: a trigger of `../triggers/` by its name, which declares the block that
    /// stands only for it; one of `of_a_cell/`, whose block each cell naming it has its own of; one of `of_a_line/`,
    /// whose block each line has its own of, which a cell on no line cannot name; one a rule of
    /// `of_a_group/` makes of a group of the flow's blocks, standing on each of them; or one a rule of `of_a_datum/`
    /// makes of a datum of the flow, standing on the datum. Gives what each is worked out to, by its
    /// place in the cell's `triggered_by`, and the blocks the triggers declare. The agent of an engine cell reads nothing.
    ///
    /// A fault is a trigger whose block the flow defines as well, once, on the block's file, naming the trigger's;
    /// a block that a second trigger's file declares, on that file, or that a second cell's trigger of one file
    /// declares, its id being the same in small letters, on that cell's; and a trigger of a file that a rule makes of a
    /// datum of the flow as well, on the trigger's file. The first of two stands, so that one fault stays one.
    fn resolve(
        &self,
        cells: &BTreeMap<String, CellFile>,
        flow: &mut FlowFile,
        places: &mut Places,
        faults: &mut Vec<Fault>,
    ) -> (Resolved, BTreeSet<String>) {
        let mut named: BTreeMap<&str, &(String, TriggerFile, Whose)> = BTreeMap::new();
        for trigger in &self.triggers {
            named.entry(stem(&trigger.0)).or_insert(trigger);
        }
        // The data of the flow, of which a rule makes triggers: its own blocks and the engine's data it names.
        let data: BTreeSet<String> = flow.blocks.keys().cloned().collect();
        for (name, (path, ..)) in &named {
            if let Some((rule, ..)) = self.made(name).into_iter().find(|(.., datum)| data.contains(datum)) {
                let what = FaultKind::DefinedTwice {
                    name: name.to_string(),
                    also: rule.clone(),
                };
                faults.push(fault(path, "", what));
            }
        }
        // The first file to declare each block, with the cell whose own it is where every cell has its own: of a block
        // each line has its own of, the cells of the line share the declarer.
        let mut declarers: BTreeMap<String, (&str, Option<&str>)> = BTreeMap::new();
        // Each block a fault was given for, with the file it was given on.
        let mut told: BTreeSet<(String, String)> = BTreeSet::new();
        let mut resolved = BTreeMap::new();
        for (id, cell) in cells {
            let small = id.to_lowercase();
            let engine = cell.kind == Kind::Engine;
            let mut worked = Vec::new();
            for (i, name) in cell.triggers.iter().enumerate() {
                if cell.triggers[..i].contains(name) {
                    continue;
                }
                let does = cell.does.get(name).cloned();
                let line = cell.line.as_deref();
                let trigger = if let Some((path, file, whose)) = named.get(name.as_str()).copied() {
                    if *whose == Whose::Line && line.is_none() {
                        worked.push((i, Err(FaultKind::NoLine)));
                        continue;
                    }
                    let fill = |text: &str| match whose {
                        Whose::Its => text.to_string(),
                        Whose::Cell => text.replace(CELL, &small),
                        Whose::Line => text.replace(LINE, line.unwrap_or_default()),
                    };
                    let block = fill(&file.block);
                    let declarer = (path.as_str(), (*whose == Whose::Cell).then_some(id.as_str()));
                    let twice = if data.contains(&block) {
                        let what = FaultKind::DefinedTwice {
                            name: block.clone(),
                            also: path.clone(),
                        };
                        Some(places.fault("blocks", &block, "", what))
                    } else {
                        match declarers.get(&block).copied() {
                            None => {
                                declarers.insert(block.clone(), declarer);
                                let (people, outside) = who_writes(file.from.as_deref());
                                let blank = BlockFile {
                                    people,
                                    outside,
                                    writes: file.writes.clone(),
                                    ..BlockFile::default()
                                };
                                flow.blocks.insert(block.clone(), blank);
                                places.files.insert(("blocks", block.clone()), path.clone());
                                None
                            }
                            Some(first) if first == declarer => None,
                            Some((first_path, Some(first_cell))) if first_path == path.as_str() => {
                                let what = FaultKind::DefinedTwice {
                                    name: block.clone(),
                                    also: places.cell(first_cell),
                                };
                                Some(fault(&places.cell(id), format!("triggered_by[{i}]"), what))
                            }
                            Some((first_path, _)) => {
                                let what = FaultKind::DefinedTwice {
                                    name: block.clone(),
                                    also: first_path.to_string(),
                                };
                                Some(fault(path, "block", what))
                            }
                        }
                    };
                    if let Some(twice) = twice
                        && told.insert((block.clone(), twice.file.clone()))
                    {
                        faults.push(twice);
                    }
                    Ok(Trigger {
                        name: name.clone(),
                        what: file.what.as_deref().map(fill),
                        block: BlockId(block),
                        group: Vec::new(),
                        read: if engine {
                            Vec::new()
                        } else {
                            file.read.iter().map(|d| BlockId(fill(d))).collect()
                        },
                        detail: file.detail.as_deref().map(fill),
                        does: does.or_else(|| file.does.as_deref().map(fill)),
                    })
                } else if let Some((rule, group)) = self.grouped(name, flow) {
                    // A group's trigger stands on each of its blocks; its agent reads them all where `read` names the
                    // group alone.
                    let members: Vec<BlockId> = flow.groups[&group].iter().map(|b| BlockId(b.clone())).collect();
                    let read = (rule.read.iter())
                        .flat_map(|r| match r.as_str() {
                            GROUP => members.clone(),
                            r => vec![BlockId(r.replace(GROUP, &group))],
                        })
                        .collect();
                    Ok(Trigger {
                        name: name.clone(),
                        what: rule.what.as_ref().map(|w| w.replace(GROUP, &group)),
                        block: BlockId(group.clone()),
                        group: members,
                        read: if engine { Vec::new() } else { read },
                        detail: None,
                        does: does.or_else(|| rule.does.as_ref().map(|d| d.replace(GROUP, &group))),
                    })
                } else {
                    let made = self.made(name);
                    match made.iter().find(|(.., datum)| data.contains(datum)) {
                        Some((_, rule, datum)) => Ok(Trigger {
                            name: name.clone(),
                            what: rule.what.as_ref().map(|w| w.replace(DATUM, datum)),
                            block: BlockId(datum.clone()),
                            group: Vec::new(),
                            read: if engine {
                                Vec::new()
                            } else {
                                (rule.read.iter()).map(|d| BlockId(d.replace(DATUM, datum))).collect()
                            },
                            detail: None,
                            does: does.or_else(|| rule.does.as_ref().map(|d| d.replace(DATUM, datum))),
                        }),
                        None => Err(match made.first() {
                            Some((.., datum)) => FaultKind::NoSuchDatum {
                                trigger: name.clone(),
                                datum: datum.clone(),
                            },
                            None => FaultKind::NoSuchTrigger { trigger: name.clone() },
                        }),
                    }
                };
                worked.push((i, trigger));
            }
            if !worked.is_empty() {
                resolved.insert(id.clone(), worked);
            }
        }
        (resolved, declarers.into_keys().collect())
    }
}

fn unreadable(file: String, error: &toml::de::Error) -> Fault {
    Fault {
        file,
        field: String::new(),
        what: FaultKind::Unreadable {
            message: error.to_string(),
        },
    }
}

fn fault(file: &str, field: impl Into<String>, what: FaultKind) -> Fault {
    Fault {
        file: file.to_string(),
        field: field.into(),
        what,
    }
}

/// The keys of a cell's `when` and `requires`, in that order, each with its value.
fn keys_of(file: &CellFile) -> impl Iterator<Item = (&str, &str)> {
    (file.when.iter().chain(&file.requires)).flat_map(|keys| keys.0.iter().map(|(k, v)| (k.as_str(), v.as_str())))
}

/// The limit as a count, or why it is none.
fn count(value: i64) -> Result<u32, FaultKind> {
    match u32::try_from(value) {
        Ok(count) if count > 0 => Ok(count),
        _ => Err(FaultKind::LimitOutOfRange { value }),
    }
}

/// Where each cell first stands in the order, for the cells that have a file.
fn first_places<'a, T>(order: &'a [String], cells: &BTreeMap<String, T>) -> BTreeMap<&'a str, usize> {
    let mut place = BTreeMap::new();
    for (i, id) in order.iter().enumerate() {
        if cells.contains_key(id) {
            place.entry(id.as_str()).or_insert(i);
        }
    }
    place
}

/// Puts the faults in the order of their files, keeping the order they were found in within a file: `flow.toml`, then
/// the cells in the order of the walk, then any other by its path.
fn sort(faults: &mut [Fault], place: Option<&BTreeMap<&str, usize>>, places: &Places) {
    faults.sort_by_cached_key(|f| {
        if f.file == FLOW_FILE {
            return (0, 0, String::new());
        }
        let id = places.id(&f.file);
        match place.zip(id).and_then(|(p, id)| p.get(id)) {
            Some(&i) => (1, i, String::new()),
            None => (2, 0, f.file.clone()),
        }
    });
}

/// The faults of a list of names: each name given twice is one, and each other name that `check` refuses.
fn check_list(
    names: &[String],
    list: &str,
    mut check: impl FnMut(&str) -> Option<FaultKind>,
) -> Vec<(String, FaultKind)> {
    let mut faults = Vec::new();
    for (i, name) in names.iter().enumerate() {
        let what = if names[..i].contains(name) {
            Some(FaultKind::Twice { name: name.clone() })
        } else {
            check(name)
        };
        faults.extend(what.map(|what| (format!("{list}[{i}]"), what)));
    }
    faults
}

/// The cells of the order that write and read one block, each list in the order of the walk, and the tests that read
/// it and that a cell of the order uses, by name.
#[derive(Default)]
struct Uses<'a> {
    writers: Vec<(usize, &'a str)>,
    readers: Vec<&'a str>,
    conditions: Vec<&'a str>,
    tests: Vec<&'a str>,
}

/// Why a block a cell's condition or precondition reads is not decided before the cell.
enum NotBefore<'a> {
    /// The cell itself writes it.
    OwnOutput,
    /// This cell, after the cell in the order, writes it.
    After(&'a str),
    /// Nothing before the cell writes it: only cells after it do.
    Later,
}

/// The parsed files, with what the checks look up in them.
struct Definition<'a> {
    flow: &'a FlowFile,
    /// The text of each cell's task written in a file of its own, by the cell's id.
    tasks: &'a BTreeMap<String, String>,
    /// The tools the flow names for every agent cell of it.
    tools: &'a [String],
    cells: &'a BTreeMap<String, CellFile>,
    places: &'a Places,
    triggers: &'a Resolved,
    trigger_blocks: &'a BTreeSet<String>,
    rule_sets: &'a BTreeSet<String>,
    /// Where each cell of the order that has a file first stands.
    place: BTreeMap<&'a str, usize>,
    /// Who of the order writes and reads each block that is declared.
    uses: BTreeMap<&'a str, Uses<'a>>,
    /// The tests a cell of the order uses, as its condition or a precondition.
    used: BTreeSet<&'a str>,
}

impl<'a> Definition<'a> {
    fn new(parsed: &'a Parsed) -> Self {
        let (flow, cells) = (&parsed.flow, &parsed.cells);
        let mut def = Self {
            flow,
            tasks: &parsed.tasks,
            tools: &parsed.tools,
            cells,
            places: &parsed.places,
            triggers: &parsed.triggers,
            trigger_blocks: &parsed.trigger_blocks,
            rule_sets: &parsed.rule_sets,
            place: first_places(&flow.order, cells),
            uses: BTreeMap::new(),
            used: BTreeSet::new(),
        };
        let mut uses: BTreeMap<&'a str, Uses<'a>> = flow.blocks.keys().map(|b| (b.as_str(), Uses::default())).collect();
        for (place, id, file) in def.ordered() {
            for block in &file.outputs {
                if let Some(u) = uses.get_mut(block.as_str())
                    && !u.writers.contains(&(place, id))
                {
                    u.writers.push((place, id));
                }
            }
            let takes = def.takes(id, file);
            let named = (takes.runs_on.iter().map(|r| &r.block)).chain(takes.given.iter().map(|g| &g.block));
            for block in named {
                if let Some(u) = uses.get_mut(block.as_str())
                    && !u.readers.contains(&id)
                {
                    u.readers.push(id);
                }
            }
            for (block, _) in keys_of(file).filter(|(k, _)| *k != WHEN_TEST) {
                if let Some(u) = uses.get_mut(block)
                    && !u.conditions.contains(&id)
                {
                    u.conditions.push(id);
                }
            }
        }
        let used: BTreeSet<&'a str> = (def.ordered())
            .flat_map(|(_, _, file)| keys_of(file).filter(|(k, _)| *k == WHEN_TEST).map(|(_, test)| test))
            .collect();
        for (test, declared) in flow.tests.iter().filter(|(t, _)| used.contains(t.as_str())) {
            for block in &declared.reads {
                if let Some(u) = uses.get_mut(block.as_str())
                    && !u.tests.contains(&test.as_str())
                {
                    u.tests.push(test);
                }
            }
        }
        def.uses = uses;
        def.used = used;
        def
    }

    /// The cells of the order that have a file, each once, at its first place.
    fn ordered(&self) -> impl Iterator<Item = (usize, &'a str, &'a CellFile)> {
        self.flow.order.iter().enumerate().filter_map(|(i, id)| {
            let file = self.cells.get(id)?;
            (self.place.get(id.as_str()) == Some(&i)).then_some((i, id.as_str(), file))
        })
    }

    /// Whether a cell of the order, a person or the outside writes the block.
    fn written(&self, block: &str) -> bool {
        let declared = &self.flow.blocks[block];
        declared.people || declared.outside.is_some() || !self.uses[block].writers.is_empty()
    }

    /// Whether a cell of the order, a condition, a named test or the owner's page reads the block.
    fn read(&self, block: &str) -> bool {
        let uses = &self.uses[block];
        self.flow.blocks[block].shown
            || !uses.readers.is_empty()
            || !uses.conditions.is_empty()
            || !uses.tests.is_empty()
    }

    /// The faults of the declared blocks: values that make no choice, a source or a field of a return the engine does
    /// not have, a block with no writer, and one with no reader.
    fn check_blocks(&self, has: &Has, faults: &mut Vec<Fault>) {
        for (name, block) in &self.flow.blocks {
            let at = |field: &str, what| self.places.fault("blocks", name, field, what);
            if let Some(values) = &block.values {
                if values.is_empty() {
                    faults.push(at("values", FaultKind::ValuesEmpty));
                }
                for (field, what) in check_list(values, "values", |_| None) {
                    faults.push(at(&field, what));
                }
            }
            if let Some(default) = &block.default {
                match &block.values {
                    None => faults.push(at("default", FaultKind::DefaultNotAChoice { block: name.clone() })),
                    Some(values) if !values.contains(default) => {
                        let what = FaultKind::DefaultNotAValue {
                            block: name.clone(),
                            default: default.clone(),
                        };
                        faults.push(at("default", what));
                    }
                    Some(_) => {}
                }
            }
            if name == WHEN_TEST {
                faults.push(at("", FaultKind::BlockNamedTest));
            }
            if block.note && !block.people {
                faults.push(at("note", FaultKind::NoteNotByPeople { block: name.clone() }));
            } else if block.note && block.values.is_some() {
                faults.push(at("note", FaultKind::NoteIsAChoice { block: name.clone() }));
            }
            if let Some(source) = &block.outside
                && !has.sources.contains(source)
            {
                let what = FaultKind::UnknownSource {
                    block: name.clone(),
                    source: source.clone(),
                };
                // Laid out one thing to a file, who writes a block from outside the cells is said by `from`.
                faults.push(at(if self.places.laid_out { "from" } else { "outside" }, what));
            }
            if let Some(field) = block.field.as_ref().filter(|f| !has.fields.contains(*f)) {
                faults.push(at("field", FaultKind::UnknownReturnField { field: field.clone() }));
            }
            if !self.written(name) {
                let what = FaultKind::NoWriter { block: name.clone() };
                faults.push(self.places.use_fault(name, what));
            }
            if !self.read(name) {
                let what = FaultKind::NoReader { block: name.clone() };
                faults.push(self.places.use_fault(name, what));
            }
        }
    }

    /// The faults of the groups of blocks: a group of no block, one with the name of a block or a datum of the flow,
    /// and a block of one that is not declared, or named twice in it.
    fn check_groups(&self, faults: &mut Vec<Fault>) {
        for (group, blocks) in &self.flow.groups {
            let field = format!("groups.{group}");
            if blocks.is_empty() {
                faults.push(fault(
                    FLOW_FILE,
                    field.clone(),
                    FaultKind::GroupEmpty { group: group.clone() },
                ));
            }
            if self.flow.blocks.contains_key(group) {
                faults.push(fault(
                    FLOW_FILE,
                    field.clone(),
                    FaultKind::GroupNameTaken { group: group.clone() },
                ));
            }
            for (at, what) in check_list(blocks, &field, |block| {
                let what = FaultKind::NoSuchBlock {
                    block: block.to_string(),
                };
                (!self.flow.blocks.contains_key(block)).then_some(what)
            }) {
                faults.push(fault(FLOW_FILE, at, what));
            }
        }
    }

    /// The faults of the declared tests: one that reads no block; a block read that is not declared, is no choice or
    /// does not offer the value looked for; and a test that no cell of the order uses.
    fn check_tests(&self, faults: &mut Vec<Fault>) {
        for (test, declared) in &self.flow.tests {
            let at = |field: &str, what| self.places.fault("tests", test, field, what);
            if declared.reads.is_empty() {
                faults.push(at("reads", FaultKind::ReadsEmpty { test: test.clone() }));
            }
            for (field, what) in check_list(&declared.reads, "reads", |block| self.check_read(block, &declared.is)) {
                faults.push(at(&field, what));
            }
            if declared.kind == TestKind::NoneStanding {
                for (i, block) in declared.reads.iter().enumerate() {
                    let second = (self.uses.get(block.as_str())).and_then(|u| u.writers.get(1));
                    if let Some(&(_, cell)) = second.filter(|_| !declared.reads[..i].contains(block)) {
                        let what = FaultKind::StandingWrittenByTwo {
                            test: test.clone(),
                            block: block.clone(),
                            cell: cell.to_string(),
                        };
                        faults.push(at(&format!("reads[{i}]"), what));
                    }
                }
            }
            if !self.used.contains(test.as_str()) {
                faults.push(at("", FaultKind::UnusedTest { test: test.clone() }));
            }
        }
    }

    /// The fault of a block a named test reads, looking for this value: not declared, not a choice, or not offering
    /// the value.
    fn check_read(&self, block: &str, is: &str) -> Option<FaultKind> {
        let name = block.to_string();
        match self.flow.blocks.get(block).map(|b| &b.values) {
            None => Some(FaultKind::NoSuchBlock { block: name }),
            Some(None) => Some(FaultKind::NotAChoice { block: name }),
            Some(Some(values)) if !values.iter().any(|v| v == is) => Some(FaultKind::NotAValue {
                block: name,
                is: is.to_string(),
            }),
            Some(Some(_)) => None,
        }
    }

    /// The faults of the lines' sets of rules: a line naming one twice, one there is not, or a part the engine places
    /// itself. Of a flow written before it was laid out one thing to a file, whose sets of rules are its own, a set that
    /// has the name of such a part, or that no line names; the sets beside the flows are every flow's.
    fn check_lines(&self, parts: &BTreeSet<String>, rules: &BTreeMap<String, String>, faults: &mut Vec<Fault>) {
        let laid_out = self.places.laid_out;
        let list = if laid_out { "rules" } else { "system" };
        for (name, line) in &self.flow.lines {
            let known = |rules_name: &str| {
                let name = rules_name.to_string();
                let there = if laid_out {
                    self.rule_sets.contains(rules_name)
                } else {
                    rules.contains_key(rules_name)
                };
                if parts.contains(rules_name) {
                    Some(FaultKind::EnginePart { name })
                } else {
                    (!there).then_some(FaultKind::NoSuchRules { name })
                }
            };
            for (field, what) in check_list(&line.system, list, known) {
                faults.push(self.places.fault("lines", name, &field, what));
            }
        }
        if laid_out {
            return;
        }
        for name in rules.keys() {
            let file = format!("{RULES_DIR}/{name}.md");
            if parts.contains(name) {
                let what = FaultKind::RulesNamedAsEnginePart { name: name.clone() };
                faults.push(fault(&file, "", what));
            } else if !self.flow.lines.values().any(|line| line.system.contains(name)) {
                faults.push(fault(&file, "", FaultKind::UnusedRules { name: name.clone() }));
            }
        }
    }

    /// The faults of the order: empty, naming a cell twice or one with no file, or leaving out a cell's file.
    fn check_order(&self, faults: &mut Vec<Fault>) {
        if self.flow.order.is_empty() {
            faults.push(fault(FLOW_FILE, "order", FaultKind::OrderEmpty));
        }
        for (i, id) in self.flow.order.iter().enumerate() {
            let field = format!("order[{i}]");
            if self.flow.order[..i].contains(id) {
                faults.push(fault(FLOW_FILE, field, FaultKind::Twice { name: id.clone() }));
            } else if !self.cells.contains_key(id) {
                faults.push(fault(FLOW_FILE, field, FaultKind::NoCellFile { cell: id.clone() }));
            }
        }
        for id in self.cells.keys() {
            if !self.flow.order.contains(id) {
                faults.push(fault(
                    &self.places.cell(id),
                    "",
                    FaultKind::NotInOrder { cell: id.clone() },
                ));
            }
        }
    }

    /// Checks one cell of the order and compiles it, as the walk reads it and as the rest of the engine does.
    fn cell(
        &self,
        place: usize,
        id: &str,
        file: &CellFile,
        limits: &Limits,
        has: &Has,
        faults: &mut Vec<Fault>,
    ) -> (Cell, CellSpec) {
        let name = self.places.cell(id);
        let mut found = Vec::new();

        match (file.kind, &file.line) {
            (Kind::Agent, None) => found.push(("line".to_string(), FaultKind::NoLine)),
            (Kind::Agent, Some(line)) if !self.flow.lines.contains_key(line) => {
                found.push(("line".to_string(), FaultKind::NoSuchLine { line: line.clone() }));
            }
            (Kind::Engine, Some(line)) => {
                found.push(("line".to_string(), FaultKind::EngineHasLine { line: line.clone() }));
            }
            _ => {}
        }
        match (file.kind, &file.task) {
            (Kind::Agent, None) => found.push(("task".to_string(), FaultKind::NoTask)),
            (Kind::Agent, Some(Task::Text(task))) if task.trim().is_empty() => {
                found.push(("task".to_string(), FaultKind::NoTask));
            }
            (Kind::Engine, Some(_)) => found.push(("task".to_string(), FaultKind::EngineHasTask)),
            _ => {}
        }
        match (file.kind, &file.act) {
            (Kind::Engine, None) => found.push(("act".to_string(), FaultKind::NoAct)),
            (Kind::Engine, Some(act)) if !has.acts.contains(act) => {
                found.push(("act".to_string(), FaultKind::UnknownAct { act: act.clone() }));
            }
            (Kind::Agent, Some(act)) => {
                found.push(("act".to_string(), FaultKind::AgentHasAct { act: act.clone() }));
            }
            _ => {}
        }
        match (file.kind, file.effort) {
            (Kind::Engine, Some(_)) => found.push(("effort".to_string(), FaultKind::EngineHasEffort)),
            (Kind::Agent, None) => found.push(("effort".to_string(), FaultKind::NoEffort)),
            _ => {}
        }
        // The tools a flow names are every agent cell's: the form of a return is one for the whole flow.
        let flow_tools = file.kind == Kind::Agent && !self.tools.is_empty();
        if file.kind == Kind::Engine && !file.tools.is_empty() {
            found.push(("tools".to_string(), FaultKind::EngineHasTools));
        } else if flow_tools && !file.tools.is_empty() {
            let what = FaultKind::Twice {
                name: "tools".to_string(),
            };
            found.push(("tools".to_string(), what));
        } else {
            found.extend(check_list(&file.tools, "tools", |tool| {
                let what = FaultKind::UnknownTool { tool: tool.to_string() };
                (!has.tools.contains(tool)).then_some(what)
            }));
        }
        let runs = match file.limit.map(count) {
            None => limits.runs,
            Some(Ok(runs)) => runs,
            Some(Err(what)) => {
                found.push(("limit".to_string(), what));
                0
            }
        };

        if file.when.is_some() && place + 1 == self.flow.order.len() {
            found.push(("when".to_string(), FaultKind::LastHasCondition));
        }
        let applies = self.keys(place, file, "when", file.when.as_ref(), &mut found);
        let requires = self.keys(place, file, "requires", file.requires.as_ref(), &mut found);
        let declared = |block: &str| {
            let what = FaultKind::NoSuchBlock {
                block: block.to_string(),
            };
            (!self.flow.blocks.contains_key(block)).then_some(what)
        };
        // A block that stands only for a trigger is not given, written or the work described by it.
        let with_content = |block: &str| declared(block).or_else(|| self.trigger_only(block));
        let takes = self.takes(id, file);
        // A cell that names what it takes by triggers in a flow written before them has one fault of it, and what
        // follows from its triggers not being there is not told.
        let unread_triggers = takes.form == Form::Triggers && !self.places.laid_out;
        match takes.form {
            Form::Inputs if file.kind == Kind::Agent => found.extend(check_list(&file.inputs, "inputs", with_content)),
            Form::Inputs => found.extend(check_list(&file.inputs, "inputs", declared)),
            Form::RunsOn => found.extend(self.check_takes(file)),
            Form::Triggers if unread_triggers => {
                let named = [
                    ("triggered_by", file.triggers.is_empty()),
                    ("do", file.does.is_empty()),
                    ("context", file.context.is_empty()),
                    ("kinds", file.kinds.is_empty()),
                ];
                let key = named
                    .iter()
                    .find(|(_, empty)| !empty)
                    .map_or("triggered_by", |(key, _)| key);
                found.push((key.to_string(), FaultKind::NotLaidOut));
            }
            Form::Triggers => found.extend(self.check_triggers(id, file)),
        }
        found.extend(check_list(&file.outputs, "outputs", with_content));
        // A cell that may ask the owner is run by the answer; where its triggers could not be read, what runs it is
        // not known.
        let source = |block: &str| self.flow.blocks.get(block).and_then(|b| b.outside.as_deref());
        let answered = takes.runs_on.iter().any(|r| source(r.block.as_str()) == Some(ANSWER));
        if !unread_triggers && !answered {
            for (i, block) in file.outputs.iter().enumerate() {
                if self.flow.blocks.get(block).and_then(|b| b.field.as_deref()) == Some(QUESTIONS) {
                    let what = FaultKind::AsksWithNoAnswer { block: block.clone() };
                    found.push((format!("outputs[{i}]"), what));
                }
            }
        }
        // Whether a block is declared is said once, where the outputs name it.
        found.extend(check_list(&file.must_write, "must_write", |block| {
            let what = FaultKind::NotAnOutput {
                block: block.to_string(),
            };
            (!file.outputs.iter().any(|o| o == block)).then_some(what)
        }));
        let describes = if unread_triggers { &[][..] } else { &file.describes[..] };
        found.extend(check_list(describes, "describes", |block| {
            let what = FaultKind::DescribedNotRunOn {
                block: block.to_string(),
            };
            let run_on = takes.runs_on.iter().any(|r| r.block.as_str() == block);
            self.trigger_only(block).or_else(|| (!run_on).then_some(what))
        }));
        for (block, how) in file.input.iter().filter(|_| takes.form == Form::Inputs) {
            if !file.inputs.iter().any(|i| i == block) {
                let what = FaultKind::TakenNotRead { block: block.clone() };
                found.push((format!("input.{block}"), what));
            } else if file.kind == Kind::Engine && how.kind.is_some() {
                let what = FaultKind::EngineIsGiven { block: block.clone() };
                found.push((format!("input.{block}.kind"), what));
            }
        }
        faults.extend(found.into_iter().map(|(field, what)| fault(&name, field, what)));

        let cell = Cell {
            id: CellId(id.to_string()),
            applies,
            requires,
            runs_on: takes.runs_on.iter().map(|r| r.block.clone()).collect(),
            outputs: file.outputs.iter().map(|b| BlockId(b.clone())).collect(),
            limit: runs,
            calls: limits.calls,
            slot: file.kind == Kind::Agent,
        };
        let spec = CellSpec {
            id: CellId(id.to_string()),
            title: file.title.clone(),
            kind: file.kind,
            line: file.line.clone(),
            reads_only: (file.line.as_ref())
                .is_some_and(|line| self.flow.lines.get(line).is_some_and(|l| l.reads_only)),
            task: match &file.task {
                Some(Task::Text(text)) => Some(text.clone()),
                Some(Task::File(_)) => self.tasks.get(id).cloned(),
                None => None,
            },
            task_file: file.task.as_ref().and_then(Task::file).map(str::to_string),
            act: file.act.clone(),
            must_write: file.must_write.iter().map(|b| BlockId(b.clone())).collect(),
            describes: file.describes.iter().map(|b| BlockId(b.clone())).collect(),
            runs_on: takes.runs_on,
            given: takes.given,
            tools: if flow_tools {
                self.tools.to_vec()
            } else {
                file.tools.clone()
            },
            system: (file.line.as_ref())
                .and_then(|line| self.flow.lines.get(line))
                .map_or_else(Vec::new, |line| line.system.clone()),
            returns: limits.returns,
            calls: limits.calls,
            effort: file.effort,
            levels_above: (file.line.as_ref())
                .and_then(|line| self.flow.lines.get(line))
                .map_or(0, |line| line.levels_above),
            takes_main: file.kind == Kind::Agent && file.takes_main,
            triggers: takes.triggers,
        };
        (cell, spec)
    }

    /// The fault of naming a block that stands only for a trigger where a block of content is named; none of another
    /// (design document `data-and-context`, 5: such a block is a trigger only, never in a context).
    fn trigger_only(&self, block: &str) -> Option<FaultKind> {
        let what = FaultKind::TriggerOnly {
            block: block.to_string(),
        };
        self.trigger_blocks.contains(block).then_some(what)
    }

    /// What the cell of this id and file takes, its triggers worked out.
    fn takes(&self, id: &str, file: &CellFile) -> Takes {
        let triggers: Vec<Trigger> = (self.triggers.get(id).into_iter().flatten())
            .filter_map(|(_, worked)| worked.as_ref().ok().cloned())
            .collect();
        takes(file, triggers)
    }

    /// The faults of what a cell names that it takes by its triggers and its context: an agent cell that nothing runs;
    /// a trigger or a datum named twice; a trigger that is not there, or whose block the flow defines as well; a datum
    /// a trigger has an agent cell's agent read that its context does not hold; what the cell does for a trigger it
    /// does not name; a datum of the context not declared; one given to an engine cell, or one given to an agent cell
    /// without its kind; a kind of a datum not in the context; the file naming another form as well.
    fn check_triggers(&self, id: &str, file: &CellFile) -> Vec<(String, FaultKind)> {
        let mut found = Vec::new();
        if file.kind == Kind::Agent && file.triggers.is_empty() {
            found.push(("triggered_by".to_string(), FaultKind::RunByNothing));
        }
        let worked = self.triggers.get(id).map_or(&[][..], Vec::as_slice);
        for (i, name) in file.triggers.iter().enumerate() {
            let field = format!("triggered_by[{i}]");
            if file.triggers[..i].contains(name) {
                found.push((field, FaultKind::Twice { name: name.clone() }));
                continue;
            }
            match worked.iter().find(|(at, _)| *at == i).map(|(_, worked)| worked) {
                Some(Err(what)) => found.push((field, what.clone())),
                Some(Ok(trigger)) if file.kind == Kind::Agent => {
                    for block in trigger
                        .read
                        .iter()
                        .filter(|b| !file.context.iter().any(|c| c == b.as_str()))
                    {
                        let what = FaultKind::ReadNotInContext {
                            trigger: name.clone(),
                            block: block.to_string(),
                        };
                        found.push((field.clone(), what));
                    }
                }
                _ => {}
            }
        }
        for trigger in file.does.keys().filter(|t| !file.triggers.contains(t)) {
            let what = FaultKind::NotATrigger {
                trigger: trigger.clone(),
            };
            found.push((format!("do.{trigger}"), what));
        }
        found.extend(check_list(&file.context, "context", |datum| {
            let block = datum.to_string();
            if !self.flow.blocks.contains_key(datum) {
                Some(FaultKind::NoSuchBlock { block })
            } else if let Some(what) = self.trigger_only(datum) {
                Some(what)
            } else if file.kind == Kind::Engine {
                Some(FaultKind::EngineIsGiven { block })
            } else {
                (!file.kinds.contains_key(datum)).then_some(FaultKind::NoKind { block })
            }
        }));
        for datum in file.kinds.keys().filter(|d| !file.context.contains(d)) {
            found.push((format!("kinds.{datum}"), FaultKind::NotGiven { block: datum.clone() }));
        }
        let other = [
            ("runs_on", file.runs_on.is_empty()),
            ("run", file.run.is_empty()),
            ("given", file.given.is_empty()),
            ("give", file.give.is_empty()),
            ("inputs", file.inputs.is_empty()),
            ("input", file.input.is_empty()),
        ];
        if let Some((key, _)) = other.iter().find(|(_, empty)| !empty) {
            found.push((key.to_string(), FaultKind::TwoForms));
        }
        found
    }

    /// The faults of what a cell in the form of `runs_on` and `given` names that it takes: an agent cell that nothing
    /// runs; a block not declared, or named twice in one list; a block given to an engine cell, or one given to an
    /// agent cell without its kind; the file naming the old form as well; why a block runs the cell, of a block that
    /// does not, and how it is given one, of a block it is not given; a table that says nothing. A block named in both
    /// lists is no fault.
    fn check_takes(&self, file: &CellFile) -> Vec<(String, FaultKind)> {
        let declared = |block: &str| {
            let what = FaultKind::NoSuchBlock {
                block: block.to_string(),
            };
            (!self.flow.blocks.contains_key(block)).then_some(what)
        };
        let mut found = Vec::new();
        if file.kind == Kind::Agent && file.runs_on.is_empty() {
            found.push(("runs_on".to_string(), FaultKind::RunByNothing));
        }
        found.extend(check_list(&file.runs_on, "runs_on", declared));
        // Whether a block is declared is said once, where `runs_on` names it, if it does; what else is wrong with a
        // block given, where it is first named.
        found.extend(check_list(&file.given, "given", |block| {
            let fault = (!file.runs_on.iter().any(|r| r == block))
                .then(|| declared(block))
                .flatten();
            fault.or_else(|| self.trigger_only(block)).or_else(|| {
                let block = block.to_string();
                match file.kind {
                    Kind::Engine => Some(FaultKind::EngineIsGiven { block }),
                    Kind::Agent => {
                        (file.give.get(&block).is_none_or(|g| g.kind.is_none())).then_some(FaultKind::NoKind { block })
                    }
                }
            })
        }));
        if !file.inputs.is_empty() {
            found.push(("inputs".to_string(), FaultKind::TwoForms));
        } else if !file.input.is_empty() {
            found.push(("input".to_string(), FaultKind::TwoForms));
        }
        for (block, run) in &file.run {
            let field = format!("run.{block}");
            let block = block.clone();
            if !file.runs_on.contains(&block) {
                found.push((field, FaultKind::NotRunOn { block }));
            } else if run.why.is_none() {
                found.push((field, FaultKind::SaysNothing { block }));
            }
        }
        for (block, give) in &file.give {
            let field = format!("give.{block}");
            let block = block.clone();
            if !file.given.contains(&block) {
                found.push((field, FaultKind::NotGiven { block }));
            } else if give.kind.is_none() && give.why.is_none() {
                found.push((field, FaultKind::SaysNothing { block }));
            }
        }
        found
    }

    /// The faults of a condition on the value of a block, for the cell of this file at this place, each with its
    /// field.
    /// A cell's `when` or `requires` (`field`) compiled, each key in the order written, with the faults of each: a
    /// table of no keys; a named test not declared, or not decided before the cell, as [`Self::check_use`] says; a
    /// block's value as [`Self::check_is`] says.
    fn keys(
        &self,
        place: usize,
        file: &CellFile,
        field: &str,
        keys: Option<&Keys>,
        found: &mut Vec<(String, FaultKind)>,
    ) -> Vec<Condition> {
        let Some(keys) = keys else { return Vec::new() };
        if keys.is_empty() {
            found.push((field.to_string(), FaultKind::EmptyCondition));
        }
        let condition = field == "when";
        (keys.0.iter())
            .map(|(key, value)| {
                let at = format!("{field}.{key}");
                if key == WHEN_TEST {
                    if self.flow.tests.contains_key(value) {
                        found.extend(self.check_use(place, file, value, &at, condition));
                    } else {
                        found.push((at, FaultKind::UndeclaredTest { test: value.clone() }));
                    }
                    Condition::Test(TestName(value.clone()))
                } else {
                    found.extend(self.check_is(place, file, key, value, &at, condition));
                    Condition::Is {
                        block: BlockId(key.clone()),
                        value: value.clone(),
                    }
                }
            })
            .collect()
    }

    /// The faults of a key on the value of a block, of the cell of this file at this place, at the key's field: a block
    /// not declared, not a choice or without the value, or not decided before the cell, as [`Self::not_before`] says
    /// of a key of its `when` (`condition`) or of its `requires`.
    fn check_is(
        &self,
        place: usize,
        file: &CellFile,
        block: &str,
        is: &str,
        at: &str,
        condition: bool,
    ) -> Vec<(String, FaultKind)> {
        let field = || at.to_string();
        let name = block.to_string();
        let Some(declared) = self.flow.blocks.get(block) else {
            return vec![(field(), FaultKind::NoSuchBlock { block: name })];
        };
        let mut found = Vec::new();
        match &declared.values {
            None => found.push((field(), FaultKind::NotAChoice { block: name.clone() })),
            Some(values) if !values.iter().any(|v| v == is) => {
                let what = FaultKind::NotAValue {
                    block: name.clone(),
                    is: is.to_string(),
                };
                found.push((field(), what));
            }
            Some(_) => {}
        }
        match self.not_before(place, file, block, declared, true, condition) {
            Some(NotBefore::OwnOutput) => found.push((field(), FaultKind::ConditionOnOwnOutput { block: name })),
            Some(NotBefore::Later | NotBefore::After(_)) => {
                found.push((field(), FaultKind::ConditionNotWrittenBefore { block: name }));
            }
            None => {}
        }
        found
    }

    /// Why the block, which the condition or a precondition of the cell of this file at this place reads, is not
    /// decided before the cell; `None` where it is. A block the cell itself writes is not, but where it is the cell's
    /// `condition` and the cell is the block's only writer: the cell that decides which of two cells is in charge has
    /// that answer as a condition of its own next round (design document `data-and-context`, 46), unwritten as it is
    /// before its first. Nor is a block that neither a cell before it, people nor the outside write. What people or the
    /// outside write can be written before any cell runs. A block a cell after it writes as well is, where
    /// `later_writes_allowed`: that cell's write decides again whether this one runs, a way back. A test of what stands
    /// allows none.
    fn not_before(
        &self,
        place: usize,
        file: &CellFile,
        block: &str,
        declared: &BlockFile,
        later_writes_allowed: bool,
        condition: bool,
    ) -> Option<NotBefore<'a>> {
        let later = self.uses[block].writers.iter().find(|&&(p, _)| p > place);
        let own = file.outputs.iter().any(|o| o == block);
        let only_writer =
            !declared.people && declared.outside.is_none() && self.uses[block].writers.iter().all(|&(p, _)| p == place);
        if own && condition && only_writer {
            None
        } else if own {
            Some(NotBefore::OwnOutput)
        } else if let Some(&(_, cell)) = later.filter(|_| !later_writes_allowed) {
            Some(NotBefore::After(cell))
        } else if !declared.people
            && declared.outside.is_none()
            && !self.uses[block].writers.iter().any(|&(p, _)| p < place)
        {
            Some(NotBefore::Later)
        } else {
            None
        }
    }

    /// The faults of a declared test used by the cell of this file at this place, as its condition or a precondition,
    /// each at the field that names the test, by [`Self::not_before`]. A block the test reads that is not
    /// declared is the test's fault, not the cell's.
    fn check_use(
        &self,
        place: usize,
        file: &CellFile,
        test: &str,
        field: &str,
        condition: bool,
    ) -> Vec<(String, FaultKind)> {
        let mut found = Vec::new();
        let declared_test = &self.flow.tests[test];
        let later_writes_allowed = declared_test.kind != TestKind::NoneStanding;
        let reads = &declared_test.reads;
        for (i, block) in reads.iter().enumerate() {
            let Some(declared) = self.flow.blocks.get(block).filter(|_| !reads[..i].contains(block)) else {
                continue;
            };
            let (test, block) = (test.to_string(), block.clone());
            let what = match self.not_before(place, file, &block, declared, later_writes_allowed, condition) {
                Some(NotBefore::OwnOutput) => FaultKind::TestOnOwnOutput { test, block },
                Some(NotBefore::After(cell)) => FaultKind::TestWrittenAfter {
                    test,
                    block,
                    cell: cell.to_string(),
                },
                Some(NotBefore::Later) => FaultKind::TestNotWrittenBefore { test, block },
                None => continue,
            };
            found.push((field.to_string(), what));
        }
        found
    }

    /// Every block, with its values and who writes and reads it.
    fn blocks(&self) -> BTreeMap<BlockId, Block> {
        let ids = |cells: &[&str]| cells.iter().map(|c| CellId(c.to_string())).collect();
        self.flow
            .blocks
            .iter()
            .map(|(name, declared)| {
                let uses = &self.uses[name.as_str()];
                let writers: Vec<&str> = uses.writers.iter().map(|&(_, id)| id).collect();
                let block = Block {
                    file: self.places.file("blocks", name),
                    values: declared.values.clone(),
                    default: declared.default.clone(),
                    what: declared.what.clone(),
                    field: declared.field.clone(),
                    trigger: self.trigger_blocks.contains(name),
                    writes: declared.writes.clone(),
                    writers: Writers {
                        cells: ids(&writers),
                        people: declared.people,
                        note: declared.note,
                        outside: declared.outside.clone(),
                    },
                    readers: Readers {
                        cells: ids(&uses.readers),
                        conditions: ids(&uses.conditions),
                        tests: uses.tests.iter().map(|t| TestName(t.to_string())).collect(),
                        shown: declared.shown,
                    },
                };
                (BlockId(name.clone()), block)
            })
            .collect()
    }
}

/// What a cell names that it takes, in whichever form its file is written: the blocks that run it and those it is
/// given, each list in the order the file names them, with why and how where the file says, and its triggers.
struct Takes {
    form: Form,
    runs_on: Vec<RunsOn>,
    given: Vec<Given>,
    triggers: Vec<Trigger>,
}

/// The form a cell's file names what it takes in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Form {
    /// One list, `inputs`, for what runs the cell and what it is given, as a cell did before it named the two apart.
    Inputs,
    /// The blocks that run it, `runs_on`, and those it is given, `given`.
    RunsOn,
    /// What runs it, `triggered_by`, each an event worked out to a block, and the data it is given, `context`.
    Triggers,
}

/// What an agent cell in the old form is given of a block of its `inputs` whose `[input.<block>]` names no kind: the
/// version it last worked from and the latest, as the engine gives a round its inputs before it fills a session by
/// kind (the version last worked from and the current one of each, and the change between them to read).
const OLD_FORM_KIND: InputKind = InputKind::LastAndLatest;

/// What the cell of this file takes, with what its triggers are worked out to, those that are. A file that names any of
/// `triggered_by`, `do`, `context` and `kinds` is in the form of triggers, whatever else it names: each trigger's block
/// runs the cell, once however many of its triggers stand on it, with what the cell does for the first of them as why;
/// and an agent cell is given each datum of its context in its kind. Else a file that names any of `runs_on`, `run`,
/// `given` and `give` is in the form of `runs_on`, and names the two lists itself. One that names none of them is in
/// the old form, and names one list, `inputs`, with how it takes a block in `[input.<block>]`: each block of it runs
/// the cell, and an agent cell is given each of them as `[input.<block>]` says, by [`OLD_FORM_KIND`] where it names no
/// kind; an engine cell is given none, and what its `[input.<block>]` says is why the block runs it. Of a cell in either
/// of the forms before triggers, each block that runs it is a trigger of the block's name: its agent reads the block for
/// it where the cell is given the block, and what the cell does for it is why the block runs it.
fn takes(file: &CellFile, triggers: Vec<Trigger>) -> Takes {
    if !(file.triggers.is_empty() && file.does.is_empty() && file.context.is_empty() && file.kinds.is_empty()) {
        let mut runs_on: Vec<RunsOn> = Vec::new();
        for trigger in &triggers {
            // A group's trigger runs the cell on each of the group's blocks.
            let blocks = if trigger.group.is_empty() {
                std::slice::from_ref(&trigger.block)
            } else {
                trigger.group.as_slice()
            };
            for block in blocks {
                if !runs_on.iter().any(|r| &r.block == block) {
                    runs_on.push(RunsOn {
                        block: block.clone(),
                        why: trigger.does.clone(),
                    });
                }
            }
        }
        let given = (file.context.iter())
            .map(|datum| Given {
                block: BlockId(datum.clone()),
                // A datum given with no kind is a fault of the definition, which then compiles to nothing.
                kind: file.kinds.get(datum).copied().unwrap_or(OLD_FORM_KIND),
                why: None,
            })
            .collect();
        return Takes {
            form: Form::Triggers,
            runs_on,
            given,
            triggers,
        };
    }
    let (form, runs_on, given) = before_triggers(file);
    let triggers = (runs_on.iter())
        .map(|r| Trigger {
            name: r.block.to_string(),
            what: None,
            block: r.block.clone(),
            group: Vec::new(),
            read: if given.iter().any(|g| g.block == r.block) {
                vec![r.block.clone()]
            } else {
                Vec::new()
            },
            detail: None,
            does: r.why.clone(),
        })
        .collect();
    Takes {
        form,
        runs_on,
        given,
        triggers,
    }
}

/// What the cell of this file, written in a form before triggers, takes: its form, the blocks that run it, and those
/// it is given.
fn before_triggers(file: &CellFile) -> (Form, Vec<RunsOn>, Vec<Given>) {
    if !(file.runs_on.is_empty() && file.run.is_empty() && file.given.is_empty() && file.give.is_empty()) {
        let runs_on = (file.runs_on.iter())
            .map(|block| RunsOn {
                block: BlockId(block.clone()),
                why: file.run.get(block).and_then(|run| run.why.clone()),
            })
            .collect();
        let given = (file.given.iter())
            .map(|block| {
                let give = file.give.get(block);
                Given {
                    block: BlockId(block.clone()),
                    // A block given with no kind is a fault of the definition, which then compiles to nothing.
                    kind: give.and_then(|g| g.kind).unwrap_or(OLD_FORM_KIND),
                    why: give.and_then(|g| g.why.clone()),
                }
            })
            .collect();
        return (Form::RunsOn, runs_on, given);
    }
    let how = |block: &String| file.input.get(block);
    let agent = file.kind == Kind::Agent;
    let runs_on = (file.inputs.iter())
        .map(|block| RunsOn {
            block: BlockId(block.clone()),
            why: how(block).filter(|_| !agent).and_then(|h| h.why.clone()),
        })
        .collect();
    let given = (file.inputs.iter())
        .filter(|_| agent)
        .map(|block| Given {
            block: BlockId(block.clone()),
            kind: how(block).and_then(|h| h.kind).unwrap_or(OLD_FORM_KIND),
            why: how(block).and_then(|h| h.why.clone()),
        })
        .collect();
    (Form::Inputs, runs_on, given)
}
