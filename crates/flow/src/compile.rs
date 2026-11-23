//! The definition checked and compiled: the flow the deciding code walks, and what the rest of the engine needs to
//! know of each cell and each block.
//!
//! Every fault a definition has is found at once and given as data, with the file and the field it is in, so that the
//! one who wrote the definition can mend all of them before compiling again.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

use decide::{BlockId, Cell, CellId, Condition, Flow, FlowFault, Outsiders, Test, TestKind, TestName, Verdict};

use serde::de::DeserializeOwned;

use crate::agents::{RuleSetFile, Waits};
use crate::outline::Renumbering;
use crate::source::{
    BLOCKS_DIR, BlockFile, CELLS_DIR, CellFile, ChangedBy, DATA_DIR, Effort, EveryAgentFile, FLOW_FILE, FlowFile,
    InputKind, Keys, Kind, LINES_DIR, LaidOutBlockFile, LaidOutFlowFile, LaidOutLineFile, LineFile, Listed, OF_A_CELL,
    RULES_DIR, RenumberingFile, Source, TESTS_DIR, Task, TestFile, Texts, WHEN_TEST, rules_file, waits_file,
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
    /// What the engine says to the owner of why a work waits on them and why it paused a work, as the file beside the
    /// flows says it ([`crate::agents::WAITS_FILE`]); every text empty where there is no such file, a flow the engine
    /// refuses to run (crate `driver`, `faults`).
    pub waits: Waits,
    /// What the item's card shows the owner of the work, in the order the flow names them; none where it names none.
    pub card: Vec<BlockId>,
    /// The cell in whose questions the engine asks what it asks of a work (design document `data-and-context`, 34);
    /// none where the flow names none.
    pub engine_asks_in: Option<CellId>,
    /// What every agent cell of the flow reads beside what its own file names, as `flow.toml` writes it
    /// (`[every_agent]`), for the page to show as written.
    pub every_agent: EveryAgent,
    /// The table of earlier ids, oldest first, as `flow.toml` writes it; none where it writes none.
    pub earlier_ids: Vec<Renumbering>,
    /// The name each file of `../data/of_a_cell/` writes, `{cell}` in it, in the order of the names.
    pub own: Vec<String>,
    /// The files it was compiled from, each by its path from the flow's directory with its text ([`Source::files`]):
    /// what the version the engine starts with is kept as (decision `a-version-is-kept-as-its-files-and-its-outline`).
    pub files: Vec<(String, String)>,
}

/// What every agent cell of a flow reads, as its `flow.toml` writes it (`[every_agent]`): each name as written, with
/// the data it stands for (of a name holding `{each}`, the datum of each cell that has its own of it, in the order of
/// the walk), and the kind each is given in, by the name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EveryAgent {
    pub reads: Vec<Taken>,
    pub kinds: BTreeMap<String, InputKind>,
}

/// A name a definition writes among what a cell watches or reads, with the blocks it stands for: of a group, the
/// group's; of a name of `[every_agent]` holding `{each}`, the datum of each cell that has its own; else the datum alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Taken {
    pub name: String,
    pub blocks: Vec<BlockId>,
}

/// A line: the cells of a work that share one session, as its file writes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// The file it is written in, from the flow's directory: `lines/<name>.toml`, or `flow.toml` of a flow written
    /// before it was laid out one thing to a file.
    pub file: String,
    /// Its agents only read.
    pub reads_only: bool,
    /// Its agents change only the project's tests.
    pub tests_only: bool,
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

/// The source of a cell's words, its dialogue with the owner: the engine writes a version of it each time the owner
/// speaks to the cell, and the cell writes its answer (design document `data-and-context`, 35).
pub const WORDS: &str = "words";

/// The source of a cell's questions: the engine writes a version of them for each question the cell asks, asks anew or
/// withdraws, which changes nothing, and for each answer of the owner's, which changes their meaning where it takes
/// another option than the one the work goes by, or answers one the cell waits for (design document `data-and-context`,
/// 30, 31). A block of this source a cell writes holds the questions it asks, and the cells that write one are those
/// that may ask.
pub const ANSWER: &str = "answer";

/// The source of the design tree on the work's branch: the engine writes a version of it where the tree differs from
/// the last version, and a cell is given what changed in it as the files that changed (design document
/// `data-and-context`, 23). A block whose lines name entries of the tree (`names_tree_entries`) is given by its latest
/// version, so a flow that has one has this datum.
pub const TREE: &str = "tree";

/// The sources outside a work a new version of whose block is the owner turning to the cells it runs: the owner's words
/// to a cell, and an answer ([`WORDS`], [`ANSWER`]). Such a block stops a round under way of a later cell
/// ([`decide::Outsiders`]).
pub const TURNING: [&str; 2] = [WORDS, ANSWER];

/// The sources whose blocks are each a dialogue, every version of which is an entry beside those before it: a cell's
/// words and its questions ([`WORDS`], [`ANSWER`]).
const ENTRIES: [&str; 2] = [WORDS, ANSWER];

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
    /// The agents of the cell's line change only the project's tests, as the line says; an engine cell's do not.
    pub tests_only: bool,
    /// What an agent cell's agent is told to do; an engine cell has none.
    pub task: Option<String>,
    /// The file the task is written in, from the flow's directory, where it is not written in place.
    pub task_file: Option<String>,
    /// The act of the engine's own that carries out an engine cell; an agent cell has none.
    pub act: Option<String>,
    /// The blocks among its outputs that every hand-in of the cell includes, in the order written: a hand-in that lacks
    /// one is sent back.
    pub must_write: Vec<BlockId>,
    /// The blocks that say what the work is where the host shows it, in the order written: blocks of the flow with
    /// content, whether or not they run the cell.
    pub describes: Vec<BlockId>,
    /// The block whose first line names the work's merge where the host shows it; none where the cell names none.
    pub named_by: Option<BlockId>,
    /// What the cell's own file names in `reads`, each a datum or a group with its blocks, in the order named; what
    /// every agent cell reads is the flow's ([`Compiled::every_agent`]), which [`CellSpec::given`] gives it by.
    pub reads: Vec<Taken>,
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
    /// What runs the cell, by its data: what it watches, each in the order named.
    pub watches: Vec<Watched>,
}

/// A datum or a group a cell watches: a version of it new to the cell runs the cell (design document
/// `data-and-context`, 5, 12).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Watched {
    /// Its name, as the cell names it: a datum's, or a group's.
    pub name: String,
    /// The blocks it stands for, a new version of any of which runs the cell: of a group, the group's, in the order
    /// the flow names them; of a datum, the datum alone.
    pub blocks: Vec<BlockId>,
    /// What the cell does when it is new, as its definition says; none where it says nothing.
    pub does: Option<String>,
    /// Where the definition names it: the cell's own `watches`.
    pub list: Listed,
}

impl Watched {
    /// Whether it is a group of blocks, which a round deals with as one, by its name.
    pub fn is_group(&self) -> bool {
        self.blocks.as_slice() != [BlockId(self.name.clone())]
    }

    /// Whether it ran a round these blocks ran: any block it stands for did (`decide::flow::ran`).
    pub fn ran(&self, ran_by: &[BlockId]) -> bool {
        decide::flow::ran(&self.blocks, ran_by).next().is_some()
    }

    /// The block it stands for that this source outside the work writes, where one is: a cell's questions
    /// ([`ANSWER`]) or its words ([`WORDS`]).
    pub fn of_source<'f>(&'f self, flow: &Compiled, source: &str) -> Option<&'f BlockId> {
        let writes = |b: &&BlockId| flow.blocks.get(*b).and_then(|d| d.writers.outside.as_deref()) == Some(source);
        self.blocks.iter().find(writes)
    }
}

impl CellSpec {
    /// What the cell watches that ran a round these blocks ran, in the order it is watched: what its agent is told is
    /// new to it, and may name as dealt with.
    pub fn ran<'a>(&'a self, ran_by: &'a [BlockId]) -> impl Iterator<Item = &'a Watched> + 'a {
        self.watches.iter().filter(move |w| w.ran(ran_by))
    }

    /// What a return that names these as dealt with dealt with, in the records' terms, in the order it names them:
    /// each a datum the cell watches, a group by the group's name, which the run's check reads as the group's blocks
    /// (`decide::flow::stands_on`). A name written twice stands twice, which the run's check finds. Beside them, each
    /// name that is none of what the cell watches, in the order written.
    pub fn dealt_with(&self, names: &[String]) -> (Vec<BlockId>, Vec<String>) {
        let mut blocks: Vec<BlockId> = Vec::new();
        let mut none = Vec::new();
        for name in names {
            match self.watches.iter().find(|w| &w.name == name) {
                Some(watched) => blocks.push(BlockId(watched.name.clone())),
                None => none.push(name.clone()),
            }
        }
        (blocks, none)
    }
}

/// A block a cell is given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Given {
    pub block: BlockId,
    /// What the context of the cell's session must hold of the block when the cell runs.
    pub kind: InputKind,
    /// Where the definition gives it.
    pub list: Listed,
}

/// A block of a work's state, as the engine needs to know it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    /// The file it is declared in, from the flow's directory: its own in `blocks/` or `../data/`, or `flow.toml` of a
    /// flow written before it was laid out one thing to a file.
    pub file: String,
    /// The values of a block that is a choice; a block with none is not one.
    pub values: Option<Vec<String>>,
    /// Of a choice, the value it counts as before anyone has written it; none where it has none.
    pub default: Option<String>,
    /// What the block is, in a sentence, where the definition says it.
    pub what: Option<String>,
    /// What a cell that writes the block is told of writing it, after the list of what it returns; none where its
    /// file says nothing.
    pub words: Option<String>,
    /// The shape a return holds the block to, where its file gives one: neither a text nor a choice.
    pub shape: Option<Shape>,
    /// Of a block a source of the engine's writes, what it writes: each text by its name, in the order written, as the
    /// block's file says; none where it says nothing.
    pub writes: Texts,
    /// Of a choice, by a value of it, the other blocks a return that writes it with that value may hold, and no more of
    /// what its cell may write, as its file says (`with_nothing_but`); none where it says none.
    pub with_nothing_but: BTreeMap<String, Vec<BlockId>>,
    /// A return that writes it is sent back while a question of the work marked `tree` has the owner's answer to its
    /// latest version and the work's change against main changes nothing in the project's design tree, as its file
    /// says (`answers_in_the_tree`).
    pub answers_in_the_tree: bool,
    /// Of a block the source of the works under way writes, the block of a work in which its plan names the files it
    /// will touch, as its file says (`planned_in`); none of any other.
    pub planned_in: Option<BlockId>,
    /// Its lines name entries of the project's design tree, one to a line, as its file says (`names_tree_entries`): a
    /// return that writes it is held to lines that each name an entry the tree on the work's branch holds, and a round
    /// that lacks its latest version is given each entry named whole in its place, as the tree's latest version has
    /// it. A cell takes it only as its latest.
    pub names_tree_entries: bool,
    /// Who writes it.
    pub writers: Writers,
    /// Who reads it.
    pub readers: Readers,
    /// Of a datum each cell has its own of (`../data/of_a_cell/`), the cell whose own it is; none of any other.
    pub of_cell: Option<CellId>,
}

/// The shape a return holds a block to, as its file gives it. Two are the same where they are written the same.
#[derive(Clone, Debug)]
pub struct Shape(pub toml::Value);

impl PartialEq for Shape {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_string() == other.0.to_string()
    }
}

impl Eq for Shape {}

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
    /// Who changes what that source writes of it by their acts, where only one does.
    pub changed_by: Option<ChangedBy>,
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
    /// The cell takes the block by two names among what it watches and reads, `first` and then `also`: by itself and
    /// through a group, or through two groups.
    TakenTwice { block: String, first: String, also: String },
    /// A limit is not a whole number from one to the largest count the engine keeps.
    LimitOutOfRange { value: i64 },
    /// The cell names no line, and needs one: an agent cell runs in its line's session.
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
    /// A line says its agents change only the project's tests, and that they only read: one that only reads changes
    /// no file, tests or other.
    ReadsOnlyAndTestsOnly,
    /// A choice has no values to choose from.
    ValuesEmpty,
    /// The block is a note, and people do not write it: a note is their words to the cells that read it.
    NoteNotByPeople { block: String },
    /// The block is a note, and a choice: a note is words to a cell, which a choice is not.
    NoteIsAChoice { block: String },
    /// The block says who changes what a source outside the work writes of it, and no such source writes it.
    ChangedByWithoutSource { block: String },
    /// The block is not declared in `flow.toml`.
    NoSuchBlock { block: String },
    /// The cell must write the block each time, and it is not among its outputs.
    NotAnOutput { block: String },
    /// The flow names among the engine's own data one that no file of `../data/` defines.
    NoSuchData { name: String },
    /// The thing is defined in this file and in another, `also`: two files of one name, or a datum the flow names that
    /// it defines as well.
    DefinedTwice { name: String, also: String },
    /// The name of a file beside the flows of a datum each cell has its own of does not hold what stands for whose own
    /// it is, `{cell}`.
    MissingPlaceholder { placeholder: String },
    /// The cell writes the block, its questions to the owner (one the source [`ANSWER`] writes), and does not watch it:
    /// the owner's answer to a question it asks would run no cell.
    AsksWithNoAnswer { block: String },
    /// The cell writes the block, its questions to the owner (one the source [`ANSWER`] writes), and watches no words of
    /// its own (of the source [`WORDS`]): the owner's words about a question it asks would reach no cell.
    AsksAndCannotBeSpokenTo { block: String },
    /// The cell, which names what it watches, writes a block of the source [`ANSWER`] or [`WORDS`] that is not its own,
    /// another cell's or one another cell writes too: a cell asks in its own questions and answers the owner in its
    /// own words, which no other cell writes (design document `data-and-context`, 30, 35).
    NotItsOwn { block: String },
    /// The cell writes the block, its own words, which the owner speaks to it in, and asks the owner nothing: a cell that
    /// asks nothing is not spoken to (principle who-asks-can-be-spoken-to).
    SpokenToAndAsksNothing { block: String },
    /// The cell hands in the verdict, which pairs with the one dialogue its judge holds with the maker, and holds more
    /// than one as the judge, these: which of them its points go in cannot be told (design document
    /// `data-and-context`, 36, 37; principle one-dialogue-for-each-party).
    VerdictOfDialogues { verdict: String, dialogues: Vec<String> },
    /// The agent's cell hands in the verdict and holds no dialogue as the judge that its points could go in: each that
    /// judges has a dialogue with the maker (design document `data-and-context`, 36).
    VerdictWithoutDialogue { verdict: String },
    /// The block's lines name entries of the design tree (`names_tree_entries`), and no datum of the flow is the design
    /// tree ([`TREE`]), whose latest version the entries are given as.
    EntriesWithoutTree { block: String },
    /// The cell is given the block, whose lines name entries of the design tree (`names_tree_entries`), in another
    /// kind than its latest: what changed in its names is not what changed in the entries' text.
    EntriesNotLatest { block: String },
    /// The flow names this cell for the engine to ask in, and it cannot change the work as the answer asks, being of an
    /// engine or of a line that only reads, or writes no questions of its own to ask in, or cannot be spoken to, having
    /// no words of its own (design document `data-and-context`, 34).
    CannotBeAskedIn { cell: String },
    /// The cell watches a datum or a group, `watched`, each block of which, `block` the first, each of its writers,
    /// cells only (no person or outside writes it), writes only where a block has a value that the cell's own condition
    /// rules out, and that block no cell can change from the writer's value to the cell's: it could never be new to the
    /// cell where the cell runs. `needs` is the cell's own `when` and `requires`, and each writer is a cell with its
    /// own, as written.
    NeverNew {
        watched: String,
        block: String,
        needs: String,
        writers: Vec<(String, String)>,
    },
    /// An engine cell says how it is given the block: it has no session to keep supplied with it.
    EngineIsGiven { block: String },
    /// An agent cell that watches nothing: it would never be entered.
    RunByNothing,
    /// The cell does not say of a block it is given what its session must hold of it.
    NoKind { block: String },
    /// A `do` is said for the block, and the cell does not watch it.
    NotWatched { block: String },
    /// The cell says how it is given the block, and it is not given the block.
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
    /// The block says what a return that writes it with this value holds nothing else but (`with_nothing_but`), and
    /// the value is not one of its values, or it is no choice.
    NothingButNotAValue { block: String, value: String },
    /// The block says that a return that writes it with this value may hold this other block beside it
    /// (`with_nothing_but`), and no cell that writes the block may write that one.
    NothingButNotAnOutput {
        block: String,
        value: String,
        output: String,
    },
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
    /// A declared test of a value names none to look for: it has no `is`.
    TestNamesNoValue { test: String },
    /// A declared test of content names a value to look for, which it does not read.
    ContentTestNamesValue { test: String },
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
    /// A renumbering of the table of earlier ids names what is no cell's id: its stage's initials and a number.
    NotACellId { id: String },
    /// A renumbering gives this id to two cells.
    IdGivenTwice { id: String },
    /// A renumbering gives a cell the id it had.
    IdToItself { id: String },
    /// A renumbering both gives the cell of this id another and takes it out.
    MovedAndTakenOut { id: String },
    /// A renumbering gives this id, which a cell still had: its stage numbers its cells with no gap, and the
    /// renumbering moves or takes out a cell of the stage numbered as high or higher, without moving or taking out the
    /// cell that had the id, which would then read as the one given it.
    IdStillHeld { id: String },
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
    def.check_every_agent(&mut faults);
    def.check_card(&mut faults);
    def.check_engine_asks_in(&mut faults);
    check_earlier_ids(&parsed.earlier_ids, &mut faults);

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
                by_owner: b.changed_by == Some(ChangedBy::Owner),
                interrupts: b.outside.as_deref().is_some_and(|source| TURNING.contains(&source)),
                // A cell's questions and its words are each a dialogue, every version of which is an entry beside those
                // before it (design document `data-and-context`, 30, 35).
                entries: b.outside.as_deref().is_some_and(|source| ENTRIES.contains(&source)),
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
                    tests_only: line.tests_only,
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
        waits: parsed.waits.clone(),
        card: parsed.card.iter().map(|b| BlockId(b.clone())).collect(),
        engine_asks_in: parsed.engine_asks_in.as_ref().map(|c| CellId(c.clone())),
        every_agent: def.every_agent(),
        earlier_ids: (parsed.earlier_ids.iter())
            .map(|r| Renumbering {
                ids: (r.ids.iter())
                    .map(|(was, now)| (CellId(was.clone()), CellId(now.clone())))
                    .collect(),
                taken_out: r.taken_out.iter().map(|id| CellId(id.clone())).collect(),
            })
            .collect(),
        own: parsed.own.made.keys().cloned().collect(),
        files: source.files(),
    })
}

/// A cell's id read as its stage's initials and its number; none where it is not written so.
fn stage_and_number(id: &str) -> Option<(&str, u64)> {
    let digits = id.find(|c: char| c.is_ascii_digit())?;
    let (stage, number) = id.split_at(digits);
    if stage.is_empty() || !stage.chars().all(|c| c.is_ascii_uppercase()) {
        return None;
    }
    if !number.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    number.parse().ok().map(|n| (stage, n))
}

/// The faults of the table of earlier ids, each within one renumbering: what is no cell's id, an id given twice, a
/// cell given its own id, one both moved and taken out, a cell taken out twice, and an id given that a cell of its
/// stage still had (decision `the-table-of-earlier-ids-is-a-list-of-renumberings`).
fn check_earlier_ids(table: &[RenumberingFile], faults: &mut Vec<Fault>) {
    for (i, r) in table.iter().enumerate() {
        let mut at =
            |field: String, what: FaultKind| faults.push(fault(FLOW_FILE, format!("earlier_ids[{i}].{field}"), what));
        let not_an_id = |id: &str| FaultKind::NotACellId { id: id.to_string() };
        // The highest number the renumbering moves or takes out in each stage.
        let mut highest: BTreeMap<&str, u64> = BTreeMap::new();
        for id in r.ids.keys().chain(&r.taken_out) {
            if let Some((stage, n)) = stage_and_number(id) {
                let high = highest.entry(stage).or_insert(n);
                *high = (*high).max(n);
            }
        }
        let leaves = |id: &str| r.ids.contains_key(id) || r.taken_out.iter().any(|t| t == id);
        let mut given: BTreeSet<&str> = BTreeSet::new();
        for (was, now) in &r.ids {
            let field = format!("ids.{was}");
            let what = if stage_and_number(was).is_none() {
                Some(not_an_id(was))
            } else if stage_and_number(now).is_none() {
                Some(not_an_id(now))
            } else if was == now {
                Some(FaultKind::IdToItself { id: was.clone() })
            } else if r.taken_out.contains(was) {
                Some(FaultKind::MovedAndTakenOut { id: was.clone() })
            } else if !given.insert(now) {
                Some(FaultKind::IdGivenTwice { id: now.clone() })
            } else {
                let held =
                    stage_and_number(now).is_some_and(|(stage, n)| highest.get(stage).is_some_and(|high| n <= *high));
                (held && !leaves(now)).then(|| FaultKind::IdStillHeld { id: now.clone() })
            };
            if let Some(what) = what {
                at(field, what);
            }
        }
        for (j, id) in r.taken_out.iter().enumerate() {
            let field = format!("taken_out[{j}]");
            if stage_and_number(id).is_none() {
                at(field, not_an_id(id));
            } else if r.taken_out[..j].contains(id) {
                at(field, FaultKind::Twice { name: id.clone() });
            }
        }
    }
}

/// The files read into their shapes, those of a flow laid out one thing to a file gathered into the shape of one
/// written before it, with where each thing is written.
struct Parsed {
    flow: FlowFile,
    /// The text of each cell's task written in a file of its own, by the cell's id.
    tasks: BTreeMap<String, String>,
    /// Of a flow laid out one thing to a file, the tools its `flow.toml` names for every agent cell of it.
    tools: Vec<String>,
    cells: BTreeMap<String, CellFile>,
    places: Places,
    /// Of a flow laid out one thing to a file, the names of the sets of rules a line may take, as
    /// [`crate::agents::RULES_FILE`] names them.
    rule_sets: BTreeSet<String>,
    /// What the engine says to the owner of why a work waits on them, as the file beside the flows says it; every
    /// text empty where there is none.
    waits: Waits,
    /// What is wrong with how the files make one definition, found as they are gathered: a thing defined twice, a
    /// datum of the engine's the flow names that is not there, and a file beside the flows that lacks the name that
    /// stands for each cell.
    faults: Vec<Fault>,
    /// Of a flow laid out one thing to a file, what the item's card shows, as `flow.toml` names it.
    card: Vec<String>,
    /// The cell the engine asks in, as `flow.toml` names it.
    engine_asks_in: Option<String>,
    /// Of a flow laid out one thing to a file, what every agent cell reads, as `flow.toml` names it.
    every_agent: EveryAgentFile,
    /// The data each cell has its own of that the flow's cells name.
    own: Own,
    /// The table of earlier ids, as `flow.toml` writes it.
    earlier_ids: Vec<RenumberingFile>,
}

/// What `flow.toml` says of the flow as a whole beside its order, limits and groups: of a flow laid out one thing to a
/// file, the engine's data it uses, the tools of every agent cell, what the item's card shows and what every agent cell
/// reads; and of any flow, the cell the engine asks in and the table of earlier ids.
#[derive(Default)]
struct Whole {
    data: Vec<String>,
    tools: Vec<String>,
    card: Vec<String>,
    engine_asks_in: Option<String>,
    every_agent: EveryAgentFile,
    earlier_ids: Vec<RenumberingFile>,
}

/// The data each cell has its own of that the flow's cells name, each made from its file beside the flows
/// where a cell names it: by its name, whose own it is.
#[derive(Default)]
struct Own {
    /// Of each datum a cell has its own of, the cell's id.
    cells: BTreeMap<String, String>,
    /// The data each file of `../data/of_a_cell/` made, by the name it writes, `{cell}` in it, each list in the order of
    /// the cells whose own they are.
    made: BTreeMap<String, Vec<String>>,
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
        flow.map(|flow| {
            let whole = Whole {
                data: flow.data.clone(),
                tools: flow.tools.clone(),
                card: flow.card.clone(),
                engine_asks_in: flow.engine_asks_in.clone(),
                every_agent: flow.every_agent.clone(),
                earlier_ids: flow.earlier_ids.clone(),
            };
            (whole, FlowFile::from(flow), files)
        })
    } else {
        let flow = shape::<FlowFile>(FLOW_FILE, &source.flow, &mut faults);
        // A flow written before it was laid out takes from beside the flows only the words the engine says to the owner.
        let files = LaidOut {
            waits: (source.files.get(&waits_file())).and_then(|text| shape(&waits_file(), text, &mut faults)),
            ..LaidOut::default()
        };
        flow.map(|flow| {
            let whole = Whole {
                engine_asks_in: flow.engine_asks_in.clone(),
                earlier_ids: flow.earlier_ids.clone(),
                ..Whole::default()
            };
            (whole, flow, files)
        })
    };
    match parsed {
        Some((whole, mut flow, files)) if faults.is_empty() => {
            let rule_sets = files.gather(&mut flow, &whole.data, &mut places, &mut faults);
            let own = files.own(&cells, &mut flow, &mut places);
            Ok(Parsed {
                flow,
                tools: whole.tools,
                cells,
                tasks,
                places,
                rule_sets,
                waits: files.waits.unwrap_or_default(),
                faults,
                card: whole.card,
                engine_asks_in: whole.engine_asks_in,
                every_agent: whole.every_agent,
                own,
                earlier_ids: whole.earlier_ids,
            })
        }
        parsed => {
            let place = (parsed.as_ref()).map(|(_, flow, _)| first_places(&flow.order, &source.cells));
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

/// What stands for the id of a cell, in small letters, in the name of a datum each cell has its own of.
const CELL: &str = "{cell}";

/// The text of a file read into its shape, or none, with the fault that it cannot be read.
fn shape<T: DeserializeOwned>(file: &str, text: &str, faults: &mut Vec<Fault>) -> Option<T> {
    toml::from_str(text)
        .map_err(|e| faults.push(unreadable(file.to_string(), &e)))
        .ok()
}

/// The file of a datum each cell has its own of, read into its shape: the name it writes, which a fault
/// says is missing where it is, holding `placeholder`, beside what any datum's file writes.
fn own_datum(
    path: &str,
    text: &str,
    placeholder: &str,
    faults: &mut Vec<Fault>,
) -> Option<(String, String, LaidOutBlockFile)> {
    let mut table: toml::Table = shape(path, text, faults)?;
    let Some(toml::Value::String(name)) = table.remove("name") else {
        let what = FaultKind::MissingPlaceholder {
            placeholder: placeholder.to_string(),
        };
        faults.push(fault(path, "name", what));
        return None;
    };
    match table.try_into::<LaidOutBlockFile>() {
        Ok(file) => Some((path.to_string(), name, file)),
        Err(e) => {
            faults.push(unreadable(path.to_string(), &e));
            None
        }
    }
}

/// What stands for whose own a datum is in its name, `placeholder`, as this name of the datum fills it: what lies
/// between the name's text before and after the placeholder; none where the name is not made so.
fn whose<'n>(written: &str, placeholder: &str, name: &'n str) -> Option<&'n str> {
    let (before, after) = written.split_once(placeholder)?;
    let middle = name.strip_prefix(before)?.strip_suffix(after)?;
    (!middle.is_empty()).then_some(middle)
}

/// The file of a datum each cell has its own of, as a datum's, what stands for whose own it is filled in
/// its words.
fn own_block(file: &LaidOutBlockFile, placeholder: &str, small: &str) -> BlockFile {
    let fill = |text: &String| text.replace(placeholder, small);
    let mut block = file.clone();
    block.what = block.what.as_ref().map(fill);
    block.words = block.words.as_ref().map(fill);
    block.writes = Texts(
        (block.writes.0.iter())
            .map(|(name, text)| (name.clone(), fill(text)))
            .collect(),
    );
    BlockFile::from(block)
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
            engine_asks_in: flow.engine_asks_in,
            earlier_ids: flow.earlier_ids,
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
    /// The data each cell has its own of, by their files, each with the name it writes, which holds `{cell}`.
    cell_data: Vec<(String, String, LaidOutBlockFile)>,
    rule_sets: Option<BTreeMap<String, RuleSetFile>>,
    waits: Option<Waits>,
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
            } else if under(&path, &format!("{DATA_DIR}/{OF_A_CELL}")) {
                laid.cell_data.extend(own_datum(&path, text, CELL, faults));
            } else if under(&path, DATA_DIR) {
                laid.data.extend(shape(&path, text, faults).map(|f| (path, f)));
            } else if path == rules {
                laid.rule_sets = shape(&path, text, faults);
            } else if path == waits_file() {
                laid.waits = shape(&path, text, faults);
            }
        }
        laid
    }

    /// Gathers the lines, blocks and named tests into the flow, each by the name of its file, with the data of the
    /// engine's the flow names among `data`, where it is laid out one thing to a file; and gives the names of the sets
    /// of rules a line may take. A fault is each second file of one name, on it, naming the first; a datum the flow
    /// names that `../data/` does not define, or that the flow defines as well; and a datum each cell has its own of
    /// whose name does not hold `{cell}`, or is the name of another.
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
                    tests_only: line.tests_only,
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
        // A datum each cell has its own of is named by the name its file writes, which holds what stands
        // for whose own it is.
        let mut own: BTreeMap<&str, &str> = BTreeMap::new();
        for (path, name, placeholder) in self.cell_data.iter().map(|(p, n, _)| (p, n, CELL)) {
            if !name.contains(placeholder) {
                let what = FaultKind::MissingPlaceholder {
                    placeholder: placeholder.to_string(),
                };
                faults.push(fault(path, "name", what));
            } else if let Some(first) = own.get(name.as_str()) {
                let mut refused = twice(name, path, first);
                refused.field = "name".to_string();
                faults.push(refused);
            } else {
                own.insert(name, path);
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

    /// Makes each datum each cell has its own of that a cell of the order names among what it watches, reads or writes,
    /// or as a key of its condition or its preconditions, and that the flow does not define otherwise: a name a file of
    /// `../data/of_a_cell/` makes of the id of a cell of the order, in small letters (decision
    /// a-datum-of-a-cell-s-own-is-made-where-a-cell-names-it).
    /// Each is declared in the file that makes it.
    fn own(&self, cells: &BTreeMap<String, CellFile>, flow: &mut FlowFile, places: &mut Places) -> Own {
        let mut own = Own::default();
        let order = flow.order.clone();
        let named = (order.iter().filter_map(|id| cells.get(id))).flat_map(|file| {
            let keys = keys_of(file).map(|(key, _)| key).filter(|key| *key != WHEN_TEST);
            let listed = file.watches.iter().chain(&file.reads).chain(&file.outputs);
            listed.map(String::as_str).chain(keys)
        });
        for name in named {
            let name = &name.to_string();
            if flow.blocks.contains_key(name) {
                continue;
            }
            let of_a_cell = (self.cell_data.iter()).find_map(|(path, written, file)| {
                let small = whose(written, CELL, name)?;
                let id = order.iter().find(|id| id.to_lowercase() == small)?;
                Some((path, written, own_block(file, CELL, small), id))
            });
            if let Some((path, written, block, id)) = of_a_cell {
                flow.blocks.insert(name.clone(), block);
                places.files.insert(("blocks", name.clone()), path.clone());
                own.cells.insert(name.clone(), id.clone());
                own.made.entry(written.clone()).or_default().push(name.clone());
                continue;
            }
        }
        // Each in the order of the cells whose own it is.
        for made in own.made.values_mut() {
            made.sort_by_key(|name| order.iter().position(|id| own.cells.get(name) == Some(id)));
        }
        // The names of the files of data each cell has its own of, which `{each}_` stands for in what every agent cell
        // takes, though no cell names a datum of one.
        for (_, written, _) in &self.cell_data {
            own.made.entry(written.clone()).or_default();
        }
        own
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

/// A cell's `when` and `requires` as the file writes them, `key = "value"`, joined by commas; none where it has none.
fn written(file: &CellFile) -> String {
    let keys: Vec<String> = keys_of(file).map(|(k, v)| format!("{k} = \"{v}\"")).collect();
    if keys.is_empty() {
        "no condition".to_string()
    } else {
        keys.join(", ")
    }
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
    rule_sets: &'a BTreeSet<String>,
    /// Where each cell of the order that has a file first stands.
    place: BTreeMap<&'a str, usize>,
    /// Who of the order writes and reads each block that is declared.
    uses: BTreeMap<&'a str, Uses<'a>>,
    /// The tests a cell of the order uses, as its condition or a precondition.
    used: BTreeSet<&'a str>,
    /// What every agent cell reads, as `flow.toml` writes it.
    every: &'a EveryAgentFile,
    /// What every agent cell reads, each name with the data it stands for, in the order written.
    every_reads: Vec<Named>,
    /// The data each cell has its own of that the flow's cells name.
    own: &'a Own,
    /// What the item's card shows, as `flow.toml` names it.
    card: &'a [String],
    /// The cell the engine asks in, as `flow.toml` names it.
    engine_asks_in: Option<&'a str>,
}

/// What stands in the name every agent cell reads for each cell that has its own of a datum, the rest of the
/// name being that datum's file's name with `{cell}` in its place.
const EACH: &str = "{each}";

/// A name of what every agent cell reads, as written, with the data it stands for: the datum it names, or of
/// a name holding `{each}`, the datum of each cell that has its own of it, in the order of the walk.
struct Named {
    written: String,
    blocks: Vec<String>,
}

impl<'a> Definition<'a> {
    fn new(parsed: &'a Parsed) -> Self {
        let (flow, cells) = (&parsed.flow, &parsed.cells);
        let named = |names: &[String]| -> Vec<Named> {
            (names.iter())
                .map(|written| {
                    let blocks = match written.contains(EACH) {
                        true => (parsed.own.made.get(&written.replace(EACH, CELL)).cloned()).unwrap_or_default(),
                        false => vec![written.clone()],
                    };
                    Named {
                        written: written.clone(),
                        blocks,
                    }
                })
                .collect()
        };
        let mut def = Self {
            flow,
            tasks: &parsed.tasks,
            tools: &parsed.tools,
            cells,
            places: &parsed.places,
            rule_sets: &parsed.rule_sets,
            place: first_places(&flow.order, cells),
            uses: BTreeMap::new(),
            used: BTreeSet::new(),
            every: &parsed.every_agent,
            every_reads: named(&parsed.every_agent.reads),
            own: &parsed.own,
            card: &parsed.card,
            engine_asks_in: parsed.engine_asks_in.as_deref(),
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
            let takes = def.watching(file);
            let named = (takes.runs_on.iter().map(BlockId::as_str)).chain(takes.given.iter().map(|g| g.block.as_str()));
            // The act of a cell reads the block the cell names the work's merge by.
            for block in named.chain(file.named_by.as_deref()) {
                if let Some(u) = uses.get_mut(block)
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

    /// The faults of the declared blocks: values that make no choice, a source the engine does not have, a block with
    /// no writer, and one with no reader.
    fn check_blocks(&self, has: &Has, faults: &mut Vec<Fault>) {
        let has_tree = (self.flow.blocks.values()).any(|b| b.outside.as_deref() == Some(TREE));
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
            // What a return that writes the choice with a value may hold beside it: a value of the choice, and blocks
            // a cell that writes the choice may write.
            let writers = &self.uses[name.as_str()].writers;
            let may_write =
                |output: &str| (writers.iter()).any(|(_, id)| self.cells[*id].outputs.iter().any(|o| o == output));
            for (value, outputs) in &block.with_nothing_but {
                let field = format!("with_nothing_but.{value}");
                if !block.values.as_ref().is_some_and(|values| values.contains(value)) {
                    let what = FaultKind::NothingButNotAValue {
                        block: name.clone(),
                        value: value.clone(),
                    };
                    faults.push(at(&field, what));
                }
                for (j, output) in outputs.iter().enumerate() {
                    if !may_write(output) {
                        let what = FaultKind::NothingButNotAnOutput {
                            block: name.clone(),
                            value: value.clone(),
                            output: output.clone(),
                        };
                        faults.push(at(&format!("{field}[{j}]"), what));
                    }
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
            if let Some(planned) = &block.planned_in
                && !self.flow.blocks.contains_key(planned)
            {
                let what = FaultKind::NoSuchBlock { block: planned.clone() };
                faults.push(at("planned_in", what));
            }
            if block.changed_by.is_some() && block.outside.is_none() {
                faults.push(at(
                    "changed_by",
                    FaultKind::ChangedByWithoutSource { block: name.clone() },
                ));
            }
            if block.names_tree_entries && !has_tree {
                let what = FaultKind::EntriesWithoutTree { block: name.clone() };
                faults.push(at("names_tree_entries", what));
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

    /// The faults of the declared tests: one that reads no block; a test of a value that names none, and one of
    /// content that names one; a block read that is not declared, or, of a test of a value, is no choice or does not
    /// offer the value looked for; and a test that no cell of the order uses.
    fn check_tests(&self, faults: &mut Vec<Fault>) {
        for (test, declared) in &self.flow.tests {
            let at = |field: &str, what| self.places.fault("tests", test, field, what);
            if declared.reads.is_empty() {
                faults.push(at("reads", FaultKind::ReadsEmpty { test: test.clone() }));
            }
            let content = declared.kind == TestKind::AnyContent;
            match (content, &declared.is) {
                (false, None) => faults.push(at("is", FaultKind::TestNamesNoValue { test: test.clone() })),
                (true, Some(_)) => faults.push(at("is", FaultKind::ContentTestNamesValue { test: test.clone() })),
                _ => {}
            }
            // A test of content reads its blocks for content, a value it names besides being its fault alone.
            let is = declared.is.as_deref().filter(|_| !content);
            for (field, what) in check_list(&declared.reads, "reads", |block| self.check_read(block, is)) {
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

    /// The fault of a block a named test reads, looking for this value, or, with none, for content: not declared, or,
    /// looking for a value, not a choice or not offering the value.
    fn check_read(&self, block: &str, is: Option<&str>) -> Option<FaultKind> {
        let name = block.to_string();
        match (self.flow.blocks.get(block).map(|b| &b.values), is) {
            (None, _) => Some(FaultKind::NoSuchBlock { block: name }),
            (Some(_), None) => None,
            (Some(None), Some(_)) => Some(FaultKind::NotAChoice { block: name }),
            (Some(Some(values)), Some(is)) if !values.iter().any(|v| v == is) => Some(FaultKind::NotAValue {
                block: name,
                is: is.to_string(),
            }),
            (Some(Some(_)), Some(_)) => None,
        }
    }

    /// The faults of the lines: of their sets of rules, a line naming one twice, one there is not, or a part the engine
    /// places itself; and a line that says it only reads and changes only the project's tests. Of a flow written before
    /// it was laid out one thing to a file, whose sets of rules are its own, a set that has the name of such a part, or
    /// that no line names; the sets beside the flows are every flow's.
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
            if line.reads_only && line.tests_only {
                let what = FaultKind::ReadsOnlyAndTestsOnly;
                faults.push(self.places.fault("lines", name, "tests_only", what));
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
        let takes = self.watching(file);
        found.extend(self.check_watches(file));
        found.extend(check_list(&file.outputs, "outputs", declared));
        found.extend(self.check_own(id, file, &takes));
        found.extend(self.check_verdicts(id, file));
        found.extend(self.check_never_new(id, file, &takes.watches));
        // Whether a block is declared is said once, where the outputs name it.
        found.extend(check_list(&file.must_write, "must_write", |block| {
            let what = FaultKind::NotAnOutput {
                block: block.to_string(),
            };
            (!file.outputs.iter().any(|o| o == block)).then_some(what)
        }));
        // The work is described by blocks of the flow with content, whether or not they run the cell: what merges is
        // described again from them before the merge.
        found.extend(check_list(&file.describes, "describes", declared));
        if let Some(what) = file.named_by.as_deref().and_then(declared) {
            found.push(("named_by".to_string(), what));
        }
        faults.extend(found.into_iter().map(|(field, what)| fault(&name, field, what)));

        let cell = Cell {
            id: CellId(id.to_string()),
            applies,
            requires,
            runs_on: takes.runs_on.clone(),
            groups: (takes.watches.iter())
                .filter(|w| w.is_group())
                .map(|w| (BlockId(w.name.clone()), w.blocks.clone()))
                .collect(),
            outputs: file.outputs.iter().map(|b| BlockId(b.clone())).collect(),
            limit: runs,
            calls: limits.calls,
            slot: file.kind == Kind::Agent,
            dialogues: self.dialogues(id, file),
            verdicts: self.verdicts(id, file),
            judges: self.judges(id, file),
        };
        let spec = CellSpec {
            id: CellId(id.to_string()),
            title: file.title.clone(),
            kind: file.kind,
            line: file.line.clone(),
            reads_only: (file.line.as_ref())
                .is_some_and(|line| self.flow.lines.get(line).is_some_and(|l| l.reads_only)),
            tests_only: (file.line.as_ref())
                .is_some_and(|line| self.flow.lines.get(line).is_some_and(|l| l.tests_only)),
            task: match &file.task {
                Some(Task::Text(text)) => Some(text.clone()),
                Some(Task::File(_)) => self.tasks.get(id).cloned(),
                None => None,
            },
            task_file: file.task.as_ref().and_then(Task::file).map(str::to_string),
            act: file.act.clone(),
            must_write: file.must_write.iter().map(|b| BlockId(b.clone())).collect(),
            describes: file.describes.iter().map(|b| BlockId(b.clone())).collect(),
            named_by: file.named_by.as_ref().map(|b| BlockId(b.clone())),
            reads: takes.reads,
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
            watches: takes.watches,
        };
        (cell, spec)
    }

    /// The blocks the cell of this id watches by its own file, each group by its blocks: what should make it judge
    /// again (design document `data-and-context`, 38).
    fn judges_by(&self, id: &str) -> Vec<BlockId> {
        let file = self.cells.get(id).filter(|_| self.place.contains_key(id));
        let mut out: Vec<BlockId> = Vec::new();
        for name in file.into_iter().flat_map(|f| &f.watches) {
            for block in self.members(name) {
                if !out.contains(&block) {
                    out.push(block);
                }
            }
        }
        out
    }

    /// The other side of each dialogue the cell of this id holds (design document `data-and-context`, 36): of each datum
    /// it writes and watches by its name, the other cells of the order that write it and watch it by its name too, each
    /// with whether the cell is the maker: the other side watches something else it writes. What a cell watches as one
    /// of a group, such as the plan its makers bring up to date, is no dialogue; nor is a datum two cells write that each
    /// watches something else the other writes: both make what a third judges, as the research and the deep research
    /// make the plan, and neither judges the other.
    fn sides(&self, id: &str, file: &CellFile) -> Vec<(BlockId, &'a str, bool)> {
        let mut sides = Vec::new();
        for block in (file.outputs.iter()).filter(|b| file.watches.contains(b)) {
            let Some(uses) = self.uses.get(block.as_str()) else {
                continue;
            };
            for &(_, other) in uses.writers.iter().filter(|(_, other)| *other != id) {
                let Some(other_file) = self.cells.get(other).filter(|f| f.watches.contains(block)) else {
                    continue;
                };
                let makes = |by: &[BlockId], outputs: &[String]| {
                    (by.iter()).any(|t| t.as_str() != block.as_str() && outputs.contains(&t.0))
                };
                let maker = makes(&self.judges_by(other), &file.outputs);
                if maker && makes(&self.judges_by(id), &other_file.outputs) {
                    continue;
                }
                sides.push((BlockId(block.clone()), other, maker));
            }
        }
        sides
    }

    /// Of each dialogue the cell holds as the maker, the blocks the other side watches by its own file that the cell's
    /// round can change: those it writes, and those a source outside the work writes, which its round's end may read,
    /// such as the work's head; never a cell's own questions or words with the owner, and nothing the flow has every
    /// agent cell watch, which is not what should make the other side judge again (design document
    /// `data-and-context`, 38, 41).
    fn dialogues(&self, id: &str, file: &CellFile) -> BTreeMap<BlockId, Vec<BlockId>> {
        let mut dialogues: BTreeMap<BlockId, Vec<BlockId>> = BTreeMap::new();
        for (dialogue, other, maker) in self.sides(id, file) {
            if !maker {
                continue;
            }
            let changes = |b: &BlockId| {
                let source = self.flow.blocks.get(b.as_str()).and_then(|d| d.outside.as_deref());
                file.outputs.contains(&b.0) || source.is_some_and(|s| s != ANSWER && s != WORDS)
            };
            let watched = dialogues.entry(dialogue.clone()).or_default();
            for b in self
                .judges_by(other)
                .into_iter()
                .filter(|b| *b != dialogue && changes(b))
            {
                if !watched.contains(&b) {
                    watched.push(b);
                }
            }
        }
        dialogues
    }

    /// Of each verdict the cell hands in as a judge, the value that does not pass and the dialogue it writes its points
    /// in: a choice the cell writes that a test of what stands reads, the value that test looks for being the one that
    /// does not pass, paired with the one dialogue the cell holds as the judge, not the maker (design document
    /// `data-and-context`, 37). An agent's cell holding no such dialogue, or more than one, is refused
    /// ([`Definition::check_verdicts`]), and pairs none.
    fn verdicts(&self, id: &str, file: &CellFile) -> BTreeMap<BlockId, Verdict> {
        let judged = self.judged(id, file);
        let [dialogue] = judged.as_slice() else {
            return BTreeMap::new();
        };
        (self.verdicts_handed_in(file).into_iter())
            .map(|(_, block, fails)| {
                let verdict = Verdict {
                    fails,
                    dialogue: dialogue.clone(),
                };
                (BlockId(block.clone()), verdict)
            })
            .collect()
    }

    /// Of each dialogue the cell of this id holds as the judge, what it judges: the blocks it watches by its own file
    /// besides the dialogue (design document `data-and-context`, 38).
    fn judges(&self, id: &str, file: &CellFile) -> BTreeMap<BlockId, Vec<BlockId>> {
        (self.judged(id, file).into_iter())
            .map(|dialogue| {
                let judged = (self.judges_by(id).into_iter()).filter(|b| *b != dialogue).collect();
                (dialogue, judged)
            })
            .collect()
    }

    /// The dialogues the cell of this id holds as the judge, each once, in the order of its outputs.
    fn judged(&self, id: &str, file: &CellFile) -> Vec<BlockId> {
        let mut judged = Vec::new();
        for (dialogue, _, maker) in self.sides(id, file) {
            if !maker && !judged.contains(&dialogue) {
                judged.push(dialogue);
            }
        }
        judged
    }

    /// The verdicts the cell hands in, each with where it stands among its outputs and the value of it that does not
    /// pass: each choice among its outputs that a test of what stands reads, the value that test looks for.
    fn verdicts_handed_in<'f>(&self, file: &'f CellFile) -> Vec<(usize, &'f String, String)> {
        let mut out = Vec::new();
        for (i, block) in file.outputs.iter().enumerate() {
            let choice = self.flow.blocks.get(block).is_some_and(|b| b.values.is_some());
            let test = (self.flow.tests.values())
                .find(|t| t.kind == TestKind::NoneStanding && t.reads.contains(block) && t.is.is_some());
            if let (true, Some(test)) = (choice, test) {
                out.push((i, block, test.is.clone().unwrap_or_default()));
            }
        }
        out
    }

    /// The faults of the verdicts an agent's cell hands in: each, where the cell judges in no dialogue, so that its
    /// points go nowhere a maker answers, or in more than one, so that which of them they go in cannot be told (design
    /// document `data-and-context`, 36, 37; principle one-dialogue-for-each-party). An act of the engine is no party to
    /// a dialogue (40), and hands in no verdict.
    fn check_verdicts(&self, id: &str, file: &CellFile) -> Vec<(String, FaultKind)> {
        if file.kind != Kind::Agent {
            return Vec::new();
        }
        let judged = self.judged(id, file);
        (self.verdicts_handed_in(file).into_iter())
            .filter_map(|(i, block, _)| {
                let what = match judged.len() {
                    0 => FaultKind::VerdictWithoutDialogue { verdict: block.clone() },
                    1 => return None,
                    _ => FaultKind::VerdictOfDialogues {
                        verdict: block.clone(),
                        dialogues: judged.iter().map(BlockId::to_string).collect(),
                    },
                };
                Some((format!("outputs[{i}]"), what))
            })
            .collect()
    }

    /// The faults of what a cell watches that can never be new to it where it runs: a datum or group whose every block
    /// has only cells for writers, each of which writes it only where a block has a value that this cell's `when` or
    /// `requires` rules out, and which no cell can change from that value. A person or the outside writing a block, or a
    /// writer with no condition on a block the cell reads, makes it live; so does a test, which is not read here. Of
    /// what the cell watches, only what its own file names is looked at.
    fn check_never_new(&self, id: &str, file: &CellFile, watches: &[Watched]) -> Vec<(String, FaultKind)> {
        let needs = keys_of(file).filter(|(k, _)| *k != WHEN_TEST).collect::<Vec<_>>();
        if needs.is_empty() {
            return Vec::new();
        }
        let mut found = Vec::new();
        for watched in watches {
            let mut writers: Vec<(&str, &CellFile)> = Vec::new();
            let never = watched.blocks.iter().all(|block| {
                let Some(declared) = self.flow.blocks.get(block.as_str()) else {
                    return false;
                };
                let cells = &self.uses[block.as_str()].writers;
                if declared.people || declared.outside.is_some() || cells.is_empty() {
                    return false;
                }
                cells.iter().all(|&(_, writer)| {
                    let Some(writer_file) = self.cells.get(writer) else {
                        return false;
                    };
                    let never = writer != id && self.never_alongside(&needs, writer_file);
                    if never && !writers.iter().any(|&(w, _)| w == writer) {
                        writers.push((writer, writer_file));
                    }
                    never
                })
            });
            let Some(i) = file.watches.iter().position(|w| *w == watched.name).filter(|_| never) else {
                continue;
            };
            let what = FaultKind::NeverNew {
                watched: watched.name.clone(),
                block: watched.blocks.first().map(BlockId::to_string).unwrap_or_default(),
                needs: written(file),
                writers: writers.iter().map(|&(w, f)| (w.to_string(), written(f))).collect(),
            };
            found.push((format!("watches[{i}]"), what));
        }
        found
    }

    /// Whether the cell with these `needs` (its conditions on the values of blocks) can never run where the writer
    /// does: a block both name has other values in them, and no cell can change it, from the value the writer needs, to
    /// the one the cell does.
    fn never_alongside(&self, needs: &[(&str, &str)], writer: &CellFile) -> bool {
        let theirs: Vec<(&str, &str)> = keys_of(writer).filter(|(k, _)| *k != WHEN_TEST).collect();
        needs.iter().any(|&(block, ours)| {
            theirs
                .iter()
                .any(|&(other, value)| other == block && value != ours && self.stays(block, value))
        })
    }

    /// Whether a block that holds this value when a cell writes under it can never hold another: no person or outside
    /// writes it, and every cell that writes it runs only where it holds a different value.
    fn stays(&self, block: &str, value: &str) -> bool {
        let Some(declared) = self.flow.blocks.get(block) else {
            return false;
        };
        !declared.people
            && declared.outside.is_none()
            && self.uses[block].writers.iter().all(|&(_, w)| {
                self.cells
                    .get(w)
                    .is_some_and(|f| keys_of(f).any(|(k, v)| k == block && v != value))
            })
    }

    /// The kind `[every_agent.kinds]` gives a datum each cell has its own of, under the `{each}_` name that stands for
    /// it: what a cell that names its own takes it as, where its file gives no kind of its own.
    fn every_kind(&self, block: &str) -> Option<InputKind> {
        (self.every.kinds.iter())
            .filter(|(written, _)| written.contains(EACH))
            .find(|(written, _)| {
                (self.own.made.get(&written.replace(EACH, CELL))).is_some_and(|made| made.iter().any(|m| m == block))
            })
            .map(|(_, kind)| *kind)
    }

    /// The blocks a name a cell watches or reads stands for: a group's, where it names a group of the flow, else the
    /// datum itself.
    fn members(&self, name: &str) -> Vec<BlockId> {
        match self
            .flow
            .groups
            .get(name)
            .filter(|_| !self.flow.blocks.contains_key(name))
        {
            Some(group) => group.iter().map(|b| BlockId(b.clone())).collect(),
            None => vec![BlockId(name.to_string())],
        }
    }

    /// What a cell takes (design document `data-and-context`, 5 to 12, 17): what it watches, each datum or group of it
    /// running it on each of its blocks, what it does when it is new being what its `[do]` says; and what it is given.
    /// An agent cell is given, in this order, what every agent cell reads, then what it reads, then what it watches,
    /// each block in the kind its `[kinds]` gives it, or `[every_agent.kinds]` of what every agent cell takes; a datum
    /// it names itself it takes as it names it. An engine cell is given nothing.
    fn watching(&self, file: &CellFile) -> Takes {
        let agent = file.kind == Kind::Agent;
        let its_own = |block: &str| file.watches.iter().chain(&file.reads).any(|n| n == block);
        // Of what every agent cell reads, each block with the name written that stands for it, but those the cell
        // names itself; none of an engine cell.
        let every = |named: &[Named]| -> Vec<(String, String)> {
            let mut blocks = Vec::new();
            for n in named.iter().filter(|_| agent) {
                for block in n.blocks.iter().filter(|b| !its_own(b.as_str())) {
                    blocks.push((n.written.clone(), block.clone()));
                }
            }
            blocks
        };
        let watches: Vec<Watched> = (file.watches.iter())
            .map(|name| Watched {
                name: name.clone(),
                blocks: self.members(name),
                does: file.does.get(name).cloned(),
                list: Listed::Watches,
            })
            .collect();
        let mut runs_on: Vec<BlockId> = Vec::new();
        for block in watches.iter().flat_map(|w| &w.blocks) {
            if !runs_on.contains(block) {
                runs_on.push(block.clone());
            }
        }
        let mut given: Vec<Given> = Vec::new();
        let mut give = |block: BlockId, kind: Option<InputKind>, list: Listed| {
            if !given.iter().any(|g| g.block == block) {
                // A block given with no kind is a fault of the definition, which then compiles to nothing.
                let kind = kind.unwrap_or(InputKind::Latest);
                given.push(Given { block, kind, list });
            }
        };
        for (written, block) in every(&self.every_reads) {
            give(
                BlockId(block),
                self.every.kinds.get(&written).copied(),
                Listed::EveryAgent,
            );
        }
        let named =
            (file.reads.iter().map(|n| (n, Listed::Reads))).chain(file.watches.iter().map(|n| (n, Listed::Watches)));
        for (name, list) in named.filter(|_| agent) {
            for block in self.members(name) {
                let kind = (file.kinds.get(block.as_str()).copied()).or_else(|| self.every_kind(block.as_str()));
                give(block, kind, list);
            }
        }
        Takes {
            runs_on,
            given,
            watches,
            reads: (file.reads.iter())
                .map(|name| Taken {
                    name: name.clone(),
                    blocks: self.members(name),
                })
                .collect(),
        }
    }

    /// The faults of what a cell names that it watches and reads: an agent cell that nothing runs; a datum or group
    /// named twice, or both watched and read; one the flow does not have; a block taken by two names, by itself and
    /// through a group or through two groups (decision a-cell-names-a-block-once); one an engine cell reads; a block an
    /// agent cell is given whose kind it does not say; a kind of a block it is not given, or other than its latest, of
    /// a block whose lines name entries of the design tree; what it does for a datum it does not watch; a kind or what
    /// it does, of an engine cell.
    fn check_watches(&self, file: &CellFile) -> Vec<(String, FaultKind)> {
        let mut found = Vec::new();
        let agent = file.kind == Kind::Agent;
        if agent && file.watches.is_empty() {
            found.push(("watches".to_string(), FaultKind::RunByNothing));
        }
        let known = |name: &str| self.flow.blocks.contains_key(name) || self.flow.groups.contains_key(name);
        // Each block taken so far, with the name it was taken by, in the order the file names them.
        let mut taken: Vec<(BlockId, &String)> = Vec::new();
        for (list, names) in [("watches", &file.watches), ("reads", &file.reads)] {
            for (i, name) in names.iter().enumerate() {
                let members = self.members(name);
                let by_another = (members.iter()).find_map(|block| {
                    (taken.iter())
                        .find(|(was, by)| was == block && *by != name)
                        .map(|(_, by)| (block.clone(), *by))
                });
                taken.extend(members.into_iter().map(|block| (block, name)));
                let again = names[..i].contains(name) || (list == "reads" && file.watches.contains(name));
                let kindless = (self.members(name).into_iter())
                    .find(|b| !file.kinds.contains_key(b.as_str()) && self.every_kind(b.as_str()).is_none());
                let what = if again {
                    FaultKind::Twice { name: name.clone() }
                } else if !known(name) {
                    FaultKind::NoSuchBlock { block: name.clone() }
                } else if let Some((block, first)) = by_another {
                    FaultKind::TakenTwice {
                        block: block.to_string(),
                        first: first.clone(),
                        also: name.clone(),
                    }
                } else if !agent && list == "reads" {
                    FaultKind::EngineIsGiven { block: name.clone() }
                } else if let Some(block) = kindless.filter(|_| agent) {
                    FaultKind::NoKind {
                        block: block.to_string(),
                    }
                } else {
                    continue;
                };
                found.push((format!("{list}[{i}]"), what));
            }
        }
        let given: Vec<BlockId> = (file.watches.iter().chain(&file.reads))
            .flat_map(|name| self.members(name))
            .collect();
        let names_entries = |block: &str| self.flow.blocks.get(block).is_some_and(|b| b.names_tree_entries);
        for (block, kind) in &file.kinds {
            let what = if !agent {
                FaultKind::EngineIsGiven { block: block.clone() }
            } else if !given.iter().any(|g| g.as_str() == block) {
                FaultKind::NotGiven { block: block.clone() }
            } else if names_entries(block) && *kind != InputKind::Latest {
                FaultKind::EntriesNotLatest { block: block.clone() }
            } else {
                continue;
            };
            found.push((format!("kinds.{block}"), what));
        }
        for datum in file.does.keys() {
            let what = if !agent {
                FaultKind::EngineHasTask
            } else if !file.watches.contains(datum) {
                FaultKind::NotWatched { block: datum.clone() }
            } else {
                continue;
            };
            found.push((format!("do.{datum}"), what));
        }
        found
    }

    /// The faults of the questions and the owner's words of a cell (design document
    /// `data-and-context`, 30, 35): a block of either it writes that is not its own; its questions, where it asks, that
    /// it does not watch, so that the owner's answer would run no cell; its questions with no words of its own that it
    /// watches, so that the owner's words about them would reach no cell; and words of its own where it asks nothing.
    fn check_own(&self, id: &str, file: &CellFile, takes: &Takes) -> Vec<(String, FaultKind)> {
        let mut found = Vec::new();
        let source = |block: &str| self.flow.blocks.get(block).and_then(|b| b.outside.as_deref());
        let watched = |block: &str| takes.runs_on.iter().any(|r| r.as_str() == block);
        let (mut questions, mut words) = (None, None);
        for (i, block) in file.outputs.iter().enumerate() {
            let of = source(block.as_str());
            if of != Some(ANSWER) && of != Some(WORDS) {
                continue;
            }
            let field = format!("outputs[{i}]");
            // Another cell's own, or, of one no file beside the flows makes, written by another cell of the order too.
            let shared = || {
                (self.cells.iter()).any(|(other, f)| {
                    other != id && self.place.contains_key(other.as_str()) && f.outputs.contains(block)
                })
            };
            let not_its_own = match self.own.cells.get(block) {
                Some(whose) => whose != id,
                None => shared(),
            };
            if not_its_own {
                found.push((field, FaultKind::NotItsOwn { block: block.clone() }));
            } else if of == Some(ANSWER) {
                questions.get_or_insert((field, block));
            } else {
                words.get_or_insert((field, block));
            }
        }
        match (questions, words) {
            (Some((field, asks)), words) => {
                if !watched(asks.as_str()) {
                    let what = FaultKind::AsksWithNoAnswer { block: asks.clone() };
                    found.push((field.clone(), what));
                }
                if words.is_none_or(|(_, words): (String, &String)| !watched(words.as_str())) {
                    let what = FaultKind::AsksAndCannotBeSpokenTo { block: asks.clone() };
                    found.push((field, what));
                }
            }
            (None, Some((field, words))) => {
                let what = FaultKind::SpokenToAndAsksNothing { block: words.clone() };
                found.push((field, what));
            }
            (None, None) => {}
        }
        found
    }

    /// What every agent cell reads, as `flow.toml` writes it, each name with the data it stands for.
    fn every_agent(&self) -> EveryAgent {
        let taken = |named: &[Named]| -> Vec<Taken> {
            (named.iter())
                .map(|n| Taken {
                    name: n.written.clone(),
                    blocks: n.blocks.iter().map(|b| BlockId(b.clone())).collect(),
                })
                .collect()
        };
        EveryAgent {
            reads: taken(&self.every_reads),
            kinds: self.every.kinds.clone(),
        }
    }

    /// The faults of what every agent cell reads: a name given twice; one that is neither the engine's datum the flow
    /// names nor, with `{each}`, a datum each cell has its own of; one with no kind; a kind of what is not named. All
    /// in `flow.toml`.
    fn check_every_agent(&self, faults: &mut Vec<Fault>) {
        let every = self.every;
        let known = |written: &str| match written.contains(EACH) {
            true => self.own.made.contains_key(&written.replace(EACH, CELL)),
            false => self.places.data.contains_key(written),
        };
        for (i, name) in every.reads.iter().enumerate() {
            let what = if every.reads[..i].contains(name) {
                FaultKind::Twice { name: name.clone() }
            } else if !known(name.as_str()) {
                FaultKind::NoSuchData { name: name.clone() }
            } else if !every.kinds.contains_key(name) {
                FaultKind::NoKind { block: name.clone() }
            } else {
                continue;
            };
            faults.push(fault(FLOW_FILE, format!("every_agent.reads[{i}]"), what));
        }
        for name in every.kinds.keys() {
            if !every.reads.contains(name) {
                let what = FaultKind::NotGiven { block: name.clone() };
                faults.push(fault(FLOW_FILE, format!("every_agent.kinds.{name}"), what));
            }
        }
    }

    /// The faults of what the item's card shows: a name given twice, and one that is no datum of the flow.
    fn check_card(&self, faults: &mut Vec<Fault>) {
        for (field, what) in check_list(self.card, "card", |block| {
            let what = FaultKind::NoSuchBlock {
                block: block.to_string(),
            };
            (!self.flow.blocks.contains_key(block)).then_some(what)
        }) {
            faults.push(fault(FLOW_FILE, field, what));
        }
    }

    /// The fault of the cell the engine asks in: one the order does not name, and one that cannot change the work, of
    /// an engine or of a line that only reads, that writes no questions to ask in, or that cannot be spoken to, writing
    /// no words (design document `data-and-context`, 34).
    fn check_engine_asks_in(&self, faults: &mut Vec<Fault>) {
        let Some(cell) = self.engine_asks_in else {
            return;
        };
        let what = match self.cells.get(cell).filter(|_| self.place.contains_key(cell)) {
            None => FaultKind::NoCellFile { cell: cell.to_string() },
            Some(file) => {
                let reads_only =
                    (file.line.as_ref()).is_some_and(|l| self.flow.lines.get(l).is_some_and(|l| l.reads_only));
                let writes = |source: &str| {
                    (file.outputs.iter())
                        .any(|b| self.flow.blocks.get(b).and_then(|d| d.outside.as_deref()) == Some(source))
                };
                if file.kind == Kind::Agent && !reads_only && writes(ANSWER) && writes(WORDS) {
                    return;
                }
                FaultKind::CannotBeAskedIn { cell: cell.to_string() }
            }
        };
        faults.push(fault(FLOW_FILE, "engine_asks_in", what));
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
                    words: declared.words.clone(),
                    shape: declared.shape.clone().map(|shape| Shape(toml::Value::Table(shape))),
                    writes: declared.writes.clone(),
                    with_nothing_but: (declared.with_nothing_but.iter())
                        .map(|(value, blocks)| (value.clone(), blocks.iter().map(|b| BlockId(b.clone())).collect()))
                        .collect(),
                    answers_in_the_tree: declared.answers_in_the_tree,
                    planned_in: declared.planned_in.clone().map(BlockId),
                    names_tree_entries: declared.names_tree_entries,
                    writers: Writers {
                        cells: ids(&writers),
                        people: declared.people,
                        note: declared.note,
                        outside: declared.outside.clone(),
                        changed_by: declared.changed_by,
                    },
                    readers: Readers {
                        cells: ids(&uses.readers),
                        conditions: ids(&uses.conditions),
                        tests: uses.tests.iter().map(|t| TestName(t.to_string())).collect(),
                        shown: declared.shown,
                    },
                    of_cell: self.own.cells.get(name).map(|id| CellId(id.clone())),
                };
                (BlockId(name.clone()), block)
            })
            .collect()
    }
}

/// What a cell takes: the blocks that run it and those it is given, each list in the order the file names them, what
/// it watches, and what its own file reads.
struct Takes {
    /// The blocks whose new version runs the cell, each once, in the order of what it watches.
    runs_on: Vec<BlockId>,
    given: Vec<Given>,
    watches: Vec<Watched>,
    reads: Vec<Taken>,
}
