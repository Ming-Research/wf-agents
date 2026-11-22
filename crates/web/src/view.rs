//! The flow as the page reads it: its own types, named for what the owner reads, made from what the flow crate
//! compiles. The page words everything it shows from these; nothing here is text written for the owner
//! but what the definition itself holds (titles, the words of the tests) and what the parser said of a file it could
//! not read.

use std::collections::BTreeMap;

use decide::{Condition as Applies, FlowFault, TestName};
use flow::{Compiled, FaultKind, Kind as CellKind, TestKind as KindOfTest};
use serde::{Deserialize, Serialize};

/// A definition as the page shows it: compiled, or why it is not.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Definition {
    /// The definition compiles: the flow it compiles to, kept apart so that the other answers stay small.
    Compiled { flow: Box<Flow> },
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
    /// What the item's card shows the owner of the work, in the order the flow names them; none where it names none.
    pub card: Vec<String>,
    /// What every agent's cell reads beside what its own file names, as `flow.toml` writes it.
    pub every_agent: EveryAgent,
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
    /// Its agents change only the project's tests.
    pub tests_only: bool,
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
    /// The value it looks for in each block it reads; none of a test of content.
    pub is: Option<String>,
}

/// How the blocks a named test reads make it hold: when every one, at least one, or none has the value it looks for;
/// `noneStanding`, when none has it as written by a person, the outside or a cell that is not skipped now; or,
/// `anyContent`, when at least one holds anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestKind {
    All,
    Any,
    None,
    NoneStanding,
    AnyContent,
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
    /// What runs it: each datum or group its own file's `watches` names, in order.
    pub watches: Vec<Watch>,
    /// What its own file's `reads` names, in order; what every agent's cell reads is the flow's (`every_agent`).
    pub reads: Vec<Read>,
    /// Each block it is given, in the order it is given them, with its kind and the list of the definition that gives
    /// it; none for an engine's cell.
    pub given: Vec<GivenBlock>,
    /// The blocks it may write, as the definition lists them.
    pub outputs: Vec<String>,
    /// The blocks among those that every hand-in of it includes, as the definition lists them.
    pub must_write: Vec<String>,
    /// The tools of the engine's that an agent's cell may ask for in its return, by name, as the definition lists
    /// them; none for an engine's cell.
    pub tools: Vec<String>,
    pub limits: Limits,
}

/// A group of the flow's blocks, a new version of any of which is new to a cell that watches the group.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub name: String,
    pub blocks: Vec<String>,
}

/// What a work's page reads of the version of its flow the work runs on, as the outline kept of it has it (decision
/// `a-works-past-is-drawn-from-its-outline`); of a work on no version kept, what its records name alone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineView {
    /// The version the work runs on was kept: else its cells are the ids its runs name, with no title or line, and it
    /// has no lines, no groups and no words.
    pub kept: bool,
    /// The cells, in the order of the walk; then each cell a run of the work names that the version has not, as one
    /// taken out since, in the order they first ran.
    pub cells: Vec<OutlineCell>,
    /// The lines, each with its cells in the order of the walk.
    pub lines: Vec<OutlineLine>,
    pub groups: Vec<Group>,
    /// What the item's card shows: the data the version names; of a work on no version kept, what people wrote for
    /// it before its first run, in the order first written.
    pub card: Vec<String>,
}

/// A cell of the version a work runs on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineCell {
    pub id: String,
    /// What it is called; none of a cell the version does not have.
    pub title: Option<String>,
    /// The line an agent's cell runs in.
    pub line: Option<String>,
    /// An agent runs it; else the engine's own act does, or the version does not say.
    pub agent: bool,
    pub given: Vec<GivenBlock>,
    pub watches: Vec<Watch>,
    pub reads: Vec<Read>,
    /// Its own words, the dialogue the owner speaks with it in, where it has them.
    pub words: Option<OutlineWords>,
}

/// A cell's own words: the datum, and what the engine writes of each version of it, each text by its name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineWords {
    pub block: String,
    pub writes: Vec<(String, Vec<crate::given::Piece>)>,
}

/// A line of the version a work runs on: its name, and its cells in the order of the walk.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineLine {
    pub name: String,
    pub cells: Vec<String>,
}

impl OutlineView {
    /// The outline of the version the work runs on, none where it runs on none kept, read with the work's records: a
    /// cell its runs name that the version has not, taken out since, as `came_from` gives it of the version it came
    /// from, where one kept has it.
    pub fn of(
        outline: Option<&flow::Outline>,
        records: &decide::Records,
        came_from: &dyn Fn(&decide::CellId) -> Option<flow::outline::OutlinedCell>,
    ) -> OutlineView {
        let mut ran: Vec<String> = Vec::new();
        for started in &records.started {
            let id = started.cell.to_string();
            if !ran.contains(&id) {
                ran.push(id);
            }
        }
        let bare = |id: &String| OutlineCell {
            id: id.clone(),
            title: None,
            line: None,
            agent: false,
            given: Vec::new(),
            watches: Vec::new(),
            reads: Vec::new(),
            words: None,
        };
        let Some(outline) = outline else {
            let first = records.started.first().map(|s| s.at);
            let mut card: Vec<String> = Vec::new();
            for written in &records.written {
                let before = first.is_none_or(|first| written.at < first);
                if before && written.by == decide::Outsider::Person && !card.contains(&written.block.to_string()) {
                    card.push(written.block.to_string());
                }
            }
            return OutlineView {
                kept: false,
                cells: ran.iter().map(bare).collect(),
                lines: Vec::new(),
                groups: Vec::new(),
                card,
            };
        };
        let mut cells: Vec<OutlineCell> = (outline.cells.iter())
            .map(|cell| OutlineCell {
                id: cell.id.to_string(),
                title: Some(cell.title.clone()),
                line: cell.line.clone(),
                agent: cell.agent,
                given: (cell.given.iter())
                    .map(|g| GivenBlock {
                        block: g.block.to_string(),
                        kind: InputKind::of(g.kind),
                        list: g.list.name().to_string(),
                    })
                    .collect(),
                watches: (cell.watches.iter())
                    .map(|w| Watch {
                        name: w.name.clone(),
                        blocks: names(&w.blocks),
                        does: w.does.clone(),
                    })
                    .collect(),
                reads: (cell.reads.iter())
                    .map(|r| Read {
                        name: r.name.clone(),
                        blocks: names(&r.blocks),
                    })
                    .collect(),
                words: cell.words.as_ref().map(|w| OutlineWords {
                    block: w.block.to_string(),
                    writes: (w.writes.iter())
                        .map(|(name, text)| (name.clone(), flow::system::with_slots(text)))
                        .collect(),
                }),
            })
            .collect();
        let gone: Vec<OutlineCell> = (ran.iter())
            .filter(|id| !outline.cells.iter().any(|cell| cell.id.as_str() == id.as_str()))
            .map(
                |id| match came_from(&decide::CellId::from(id.as_str())).filter(|_| false) {
                    Some(was) => OutlineCell {
                        title: Some(was.title),
                        line: was.line,
                        agent: was.agent,
                        ..bare(id)
                    },
                    None => bare(id),
                },
            )
            .collect();
        cells.extend(gone);
        let mut lines: Vec<OutlineLine> = Vec::new();
        for cell in &cells {
            let Some(name) = &cell.line else { continue };
            match lines.iter_mut().find(|l| &l.name == name) {
                Some(line) => line.cells.push(cell.id.to_string()),
                None => lines.push(OutlineLine {
                    name: name.clone(),
                    cells: vec![cell.id.to_string()],
                }),
            }
        }
        OutlineView {
            kept: true,
            cells,
            lines,
            groups: (outline.groups.iter())
                .map(|g| Group {
                    name: g.name.clone(),
                    blocks: names(&g.blocks),
                })
                .collect(),
            card: names(&outline.card),
        }
    }
}

/// A datum or group a cell watches: its name as the definition writes it, the blocks it stands for (of a datum, the
/// datum alone) and what the cell does when it is new.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Watch {
    pub name: String,
    pub blocks: Vec<String>,
    #[serde(rename = "do")]
    pub does: Option<String>,
}

/// A datum or group a cell only reads, or every agent's cell reads: its name as the definition writes it, and the
/// blocks it stands for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Read {
    pub name: String,
    pub blocks: Vec<String>,
}

/// A block a cell is given: the kind its session holds of it, and the list of the definition that gives it
/// (`every_agent`, `reads` or `watches`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GivenBlock {
    pub block: String,
    pub kind: InputKind,
    pub list: String,
}

/// What every agent's cell of a flow reads, as `flow.toml` writes it (`[every_agent]`): each name with the data it
/// stands for, and the kind of each, by its name.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EveryAgent {
    pub reads: Vec<Read>,
    pub kinds: BTreeMap<String, InputKind>,
}

impl EveryAgent {
    fn of(every: &flow::EveryAgent) -> Self {
        EveryAgent {
            reads: every.reads.iter().map(Read::of).collect(),
            kinds: (every.kinds.iter())
                .map(|(name, kind)| (name.clone(), InputKind::of(*kind)))
                .collect(),
        }
    }
}

impl Read {
    fn of(taken: &flow::Taken) -> Self {
        Read {
            name: taken.name.clone(),
            blocks: names(&taken.blocks),
        }
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
    /// Who changes what that source writes of it by their acts, as its file writes it (`owner`); none where its file
    /// says nobody.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changed_by: Option<String>,
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
    TakenTwice {
        block: String,
        first: String,
        also: String,
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
    /// A line says its agents change only the project's tests, and that they only read.
    ReadsOnlyAndTestsOnly,
    ValuesEmpty,
    /// The block is a note, and people do not write it.
    NoteNotByPeople {
        block: String,
    },
    /// The block says who changes what a source outside the work writes of it, and no such source writes it.
    ChangedByWithoutSource {
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
    /// The flow names among the engine's own data one that no file beside the flows defines.
    NoSuchData {
        name: String,
    },
    /// The thing is defined in this file and again in another.
    DefinedTwice {
        name: String,
        also: String,
    },
    /// The name of a file beside the flows of a datum each cell or each line has its own of lacks what stands for whose
    /// own it is.
    MissingPlaceholder {
        placeholder: String,
    },
    /// The cell writes its questions, and does not watch them: the owner's answer would run no cell.
    AsksWithNoAnswer {
        block: String,
    },
    /// The cell writes its questions, and watches no words of its own.
    AsksAndCannotBeSpokenTo {
        block: String,
    },
    /// The cell writes a block of the owner's answers or words that is not its own.
    NotItsOwn {
        block: String,
    },
    /// The cell writes its own words, which the owner speaks to it in, and asks the owner nothing.
    SpokenToAndAsksNothing {
        block: String,
    },
    /// The cell hands in a verdict and judges in more than one dialogue, which the verdict's points could go in.
    VerdictOfDialogues {
        verdict: String,
        dialogues: Vec<String>,
    },
    /// The agent's cell hands in a verdict and judges in no dialogue its points could go in.
    VerdictWithoutDialogue {
        verdict: String,
    },
    /// The cell watches or reads a datum of another line's own.
    LineDataOfAnother {
        block: String,
        line: String,
    },
    /// The flow names a cell for the engine to ask in that cannot change the work or cannot be spoken to.
    CannotBeAskedIn {
        cell: String,
    },
    /// The cell watches a datum or group whose every block's every writer runs only where the cell cannot.
    NeverNew {
        watched: String,
        block: String,
        needs: String,
        writers: Vec<(String, String)>,
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
    /// A `do` is said for the block, and the cell does not watch it.
    NotWatched {
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
    /// The block says what a return that writes it with this value holds nothing else but, and the value is not one of
    /// its values, or it is no choice.
    NothingButNotAValue {
        block: String,
        value: String,
    },
    /// The block says a return that writes it with this value may hold this other block, and no cell that writes the
    /// block may write that one.
    NothingButNotAnOutput {
        block: String,
        value: String,
        output: String,
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
    TestNamesNoValue {
        test: String,
    },
    ContentTestNamesValue {
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
    /// A renumbering of the table of earlier ids names what is no cell's id.
    NotACellId {
        id: String,
    },
    /// A renumbering gives this id to two cells.
    IdGivenTwice {
        id: String,
    },
    /// A renumbering gives a cell the id it had.
    IdToItself {
        id: String,
    },
    /// A renumbering both moves the cell of this id and takes it out.
    MovedAndTakenOut {
        id: String,
    },
    /// A renumbering gives this id, which a cell of its stage still had.
    IdStillHeld {
        id: String,
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
                        tests_only: written.is_some_and(|l| l.tests_only),
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
                watches: (spec.watches.iter())
                    .map(|w| Watch {
                        name: w.name.clone(),
                        blocks: names(&w.blocks),
                        does: w.does.clone(),
                    })
                    .collect(),
                reads: spec.reads.iter().map(Read::of).collect(),
                given: (spec.given.iter())
                    .map(|g| GivenBlock {
                        block: g.block.to_string(),
                        kind: InputKind::of(g.kind),
                        list: g.list.name().to_string(),
                    })
                    .collect(),
                outputs: names(&cell.outputs),
                must_write: names(&spec.must_write),
                tools: spec.tools.clone(),
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
                writes: (block.writes.0.iter())
                    .map(|(name, text)| (name.clone(), flow::system::with_slots(text)))
                    .collect(),
                written_by: WrittenBy {
                    cells: names(&block.writers.cells),
                    people: block.writers.people,
                    note: block.writers.note,
                    outside: block.writers.outside.clone(),
                    changed_by: block.writers.changed_by.map(|by| match by {
                        flow::ChangedBy::Owner => flow::OWNER.to_string(),
                    }),
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
            card: names(&compiled.card),
            every_agent: EveryAgent::of(&compiled.every_agent),
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
                KindOfTest::AnyContent => TestKind::AnyContent,
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
            F::TakenTwice { block, first, also } => What::TakenTwice { block, first, also },
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
            F::ReadsOnlyAndTestsOnly => What::ReadsOnlyAndTestsOnly,
            F::ValuesEmpty => What::ValuesEmpty,
            F::NoteNotByPeople { block } => What::NoteNotByPeople { block },
            F::ChangedByWithoutSource { block } => What::ChangedByWithoutSource { block },
            F::NoteIsAChoice { block } => What::NoteIsAChoice { block },
            F::NoSuchBlock { block } => What::NoSuchBlock { block },
            F::NotAnOutput { block } => What::NotAnOutput { block },
            F::NoSuchData { name } => What::NoSuchData { name },
            F::DefinedTwice { name, also } => What::DefinedTwice { name, also },
            F::MissingPlaceholder { placeholder } => What::MissingPlaceholder { placeholder },
            F::AsksWithNoAnswer { block } => What::AsksWithNoAnswer { block },
            F::AsksAndCannotBeSpokenTo { block } => What::AsksAndCannotBeSpokenTo { block },
            F::NotItsOwn { block } => What::NotItsOwn { block },
            F::SpokenToAndAsksNothing { block } => What::SpokenToAndAsksNothing { block },
            F::VerdictOfDialogues { verdict, dialogues } => What::VerdictOfDialogues { verdict, dialogues },
            F::VerdictWithoutDialogue { verdict } => What::VerdictWithoutDialogue { verdict },
            F::LineDataOfAnother { block, line } => What::LineDataOfAnother { block, line },
            F::CannotBeAskedIn { cell } => What::CannotBeAskedIn { cell },
            F::NeverNew {
                watched,
                block,
                needs,
                writers,
            } => What::NeverNew {
                watched,
                block,
                needs,
                writers,
            },
            F::EngineIsGiven { block } => What::EngineIsGiven { block },
            F::RunByNothing => What::RunByNothing,
            F::NoKind { block } => What::NoKind { block },
            F::NotWatched { block } => What::NotWatched { block },
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
            F::NothingButNotAValue { block, value } => What::NothingButNotAValue { block, value },
            F::NothingButNotAnOutput { block, value, output } => What::NothingButNotAnOutput { block, value, output },
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
            F::TestNamesNoValue { test } => What::TestNamesNoValue { test },
            F::ContentTestNamesValue { test } => What::ContentTestNamesValue { test },
            F::UnusedTest { test } => What::UnusedTest { test },
            F::TestOnOwnOutput { test, block } => What::TestOnOwnOutput { test, block },
            F::TestNotWrittenBefore { test, block } => What::TestNotWrittenBefore { test, block },
            F::TestWrittenAfter { test, block, cell } => What::TestWrittenAfter { test, block, cell },
            F::StandingWrittenByTwo { test, block, cell } => What::StandingWrittenByTwo { test, block, cell },
            F::NotACellId { id } => What::NotACellId { id },
            F::IdGivenTwice { id } => What::IdGivenTwice { id },
            F::IdToItself { id } => What::IdToItself { id },
            F::MovedAndTakenOut { id } => What::MovedAndTakenOut { id },
            F::IdStillHeld { id } => What::IdStillHeld { id },
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

#[cfg(test)]
mod tests {
    use decide::{Outsider, Records, RunId, Seq, Started, Written};

    use super::*;

    fn started(run: u64, cell: &str, at: u64) -> Started {
        Started {
            run: RunId(run),
            cell: cell.into(),
            at: Seq(at),
            because: Vec::new(),
            basis: Default::default(),
        }
    }

    fn written(block: &str, by: Outsider, at: u64) -> Written {
        Written {
            block: block.into(),
            by,
            matters: true,
            value: None,
            empty: false,
            said_to: None,
            at: Seq(at),
        }
    }

    // A work on no version kept is read from its records alone (decision a-works-past-is-drawn-from-its-outline): a
    // cell for each id its runs name, in the order they first ran, with no title or line; its card what people wrote
    // for it before its first run; no lines, groups or words; and it says so.
    #[test]
    fn a_work_on_no_version_kept_is_outlined_from_its_records_alone() {
        let records = Records {
            started: vec![
                started(1, "S1", 3),
                started(2, "R1", 5),
                started(3, "D4", 7),
                started(4, "R1", 9),
                started(5, "M2", 11),
            ],
            written: vec![
                written("background", Outsider::Person, 1),
                written("main", Outsider::Outside, 2),
                written("plan_owner", Outsider::Person, 8),
            ],
            ..Records::default()
        };
        let outline = OutlineView::of(None, &records, &|_| None);
        assert!(!outline.kept);
        let ids: Vec<&str> = outline.cells.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["S1", "R1", "D4", "M2"]);
        assert!(
            outline
                .cells
                .iter()
                .all(|c| c.title.is_none() && c.line.is_none() && c.words.is_none())
        );
        assert_eq!(outline.card, ["background"]);
        assert!(outline.lines.is_empty() && outline.groups.is_empty());
    }

    fn outlined(id: &str, title: &str, line: Option<&str>, agent: bool) -> flow::outline::OutlinedCell {
        flow::outline::OutlinedCell {
            id: id.into(),
            title: title.to_string(),
            line: line.map(str::to_string),
            agent,
            given: Vec::new(),
            watches: Vec::new(),
            reads: Vec::new(),
            words: None,
            questions: None,
        }
    }

    // A cell a run of the work names that the version it runs on has taken out is drawn as the version it came from has
    // it, titled and in its line (decision a-works-past-is-drawn-from-its-outline); one no version kept has stays bare.
    // Fails where a cell taken out is drawn untitled, or out of its line.
    #[test]
    fn a_cell_taken_out_since_is_drawn_as_the_version_it_came_from_has_it() {
        let outline = flow::Outline {
            name: "staged".to_string(),
            cells: vec![
                outlined("T1", "Write", Some("main"), true),
                outlined("M1", "Merge", None, false),
            ],
            lines: Vec::new(),
            groups: Vec::new(),
            card: Vec::new(),
            earlier_ids: Vec::new(),
            own: Vec::new(),
            from: BTreeMap::new(),
        };
        let records = Records {
            started: vec![started(1, "T1", 3), started(2, "T2@1", 5), started(3, "X1@1", 7)],
            ..Records::default()
        };
        let came_from =
            |id: &decide::CellId| (id.as_str() == "T2@1").then(|| outlined("T2", "Review", Some("main"), true));
        let view = OutlineView::of(Some(&outline), &records, &came_from);
        let drawn: Vec<(&str, Option<&str>, Option<&str>, bool)> = (view.cells.iter())
            .map(|c| (c.id.as_str(), c.title.as_deref(), c.line.as_deref(), c.agent))
            .collect();
        assert_eq!(
            drawn,
            [
                ("T1", Some("Write"), Some("main"), true),
                ("M1", Some("Merge"), None, false),
                ("T2@1", Some("Review"), Some("main"), true),
                ("X1@1", None, None, false),
            ]
        );
        let lines: Vec<(&str, Vec<&str>)> = (view.lines.iter())
            .map(|l| (l.name.as_str(), l.cells.iter().map(String::as_str).collect()))
            .collect();
        assert_eq!(lines, [("main", vec!["T1", "T2@1"])]);
    }
}
