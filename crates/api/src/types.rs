//! What the interface hands out. Times are UTC milliseconds since the Unix epoch, as in the database; JSON the
//! database keeps whole (a round's input and return, a card's body) is handed out as it is kept.

use flow::format::Src;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

/// How the engine dispatches: what it starts, and when.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// It takes every work as far as it can, pass after pass.
    Run,
    /// It takes one step when the owner asks for one, and then waits for the next ask.
    Step,
    /// It starts nothing: no round, no tool, no outbox action.
    Hold,
}

/// One step the engine took in step mode, and what it came to.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct StepTaken {
    /// When the step ended.
    pub at: i64,
    /// The work it took a step of; none when no work had a step to take.
    pub work: Option<i64>,
    /// The item the step started that work on, when the step started it.
    pub started_item: Option<i64>,
    /// The rounds the step started or ended, and the cell of the last of them.
    pub rounds: Vec<i64>,
    pub cell: Option<String>,
}

/// The engine as it runs now, and what the page needs of the installation.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Status {
    pub mode: Mode,
    /// In step mode: whether the step the owner asked for is under way.
    pub stepping: bool,
    /// The last step taken in step mode since the engine started.
    pub last_step: Option<StepTaken>,
    /// The slots of every machine together, and how many of them works hold.
    pub slots: i64,
    pub slots_used: i64,
    /// The flow version new works start on.
    pub flow_version: i64,
    /// The configuration commit the engine runs with.
    pub config_commit: String,
    /// When the engine started.
    pub started_at: i64,
    /// The version of wf-agents that runs.
    pub version: String,
    /// While a failed run's cause stops every work alike, such as a usage limit or no network: no new round starts
    /// until it is over.
    pub halt: Option<Halt>,
}

/// Why the engine starts no new round for any work, and until when.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Halt {
    /// The kind of failed run, as the flow names it: `usage_limit` or `offline`.
    pub kind: String,
    /// Until when: when the usage limit resets, or when the engine next looks whether the network is back.
    pub until: i64,
    /// What the command line reported.
    pub why: String,
}

/// What changed, as the event stream gives it. An event names what to read again and never carries that state,
/// except the engine's status, which lives only in the engine. The stream gives each event an id; a client that
/// reconnects with the last id it saw is given what it missed.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    /// The stream is open and starts here: what was read before may already be out of date.
    Ready,
    /// The events since the id the client gave are no longer kept, or the engine started again: read everything
    /// again.
    Resync,
    /// The engine's status changed.
    Status { status: Box<Status> },
    /// A work started, moved, was paused, resumed or ended, or was said something to.
    Work { work: i64 },
    /// A round started or ended, or ran a tool.
    Round { work: i64, round: i64 },
    /// A card was opened, ruled or replaced.
    Card { card: i64, work: Option<i64> },
    /// An item, its dependencies or its place in the queue changed; no item when which one is no longer known.
    Item { item: Option<i64> },
    /// A flow version was stored.
    Flow { version: i64 },
}

/// Where a work stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum WorkState {
    /// A round of it is running.
    Running,
    /// Its next step is due; the engine takes it on its next pass.
    Ready,
    /// It asked the owner or the architect, and waits for the card to be answered.
    Asking,
    /// It took a row that waits: for other works, or for its turn.
    Waiting,
    /// The owner, the architect or the engine paused it.
    Paused,
    /// It reached the end of its graph.
    Done,
    /// It was stopped before its end, for example because its item was cancelled.
    Ended,
}

/// The round of a work that is running.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct RunningRound {
    pub id: i64,
    pub cell: Option<String>,
    pub started_at: i64,
}

/// A work as a list shows it: what it works on, where it stands and what it has cost.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct WorkSummary {
    pub id: i64,
    /// The item worked on, its title and its project; none for a work on no item.
    pub item: Option<i64>,
    pub title: Option<String>,
    pub project: Option<String>,
    /// The graph it runs, of the flow version it is pinned to.
    pub graph: String,
    pub flow_version: i64,
    /// The cell it is at, with the cell's title and the stage the work is at, in its flow version.
    pub at: String,
    pub at_title: Option<String>,
    pub stage: Option<String>,
    pub state: WorkState,
    /// Why it was paused, or ended before its end.
    pub paused: Option<String>,
    pub ended: Option<String>,
    pub branch: Option<String>,
    pub pr: Option<i64>,
    /// When its first round started, and when its last round started or ended.
    pub started_at: Option<i64>,
    pub last_at: Option<i64>,
    pub rounds: i64,
    /// Rounds whose return was sent back, and rounds whose run failed or was cut off.
    pub sent_back: i64,
    pub failed: i64,
    pub running: Option<RunningRound>,
    /// What its rounds cost, as the command lines reported it, in US dollars.
    pub cost_usd: f64,
    /// Its cards still waiting for a ruling.
    pub open_cards: i64,
}

/// How a round ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RoundOutcome {
    /// It has not ended.
    Running,
    /// Its return took a row of its cell.
    Took,
    /// Its return was sent back: it did not fit the cell's schema or failed a check.
    SentBack,
    /// Its run failed before it handed in a return.
    Failed,
    /// The engine stopped while it ran; its cell runs again.
    Interrupted,
    /// It ended without a row: the end of the graph.
    Ended,
}

/// A round as a work's timeline shows it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct RoundSummary {
    pub id: i64,
    /// Its cell; none for a round that only answered an interjection.
    pub cell: Option<String>,
    /// The line whose session it ran in; none for the engine's own steps, tool cells and the owner.
    pub line: Option<String>,
    pub outcome: RoundOutcome,
    /// The row its return took.
    pub transition: Option<String>,
    /// Why it was sent back, failed or was cut off.
    pub why: Option<String>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub model: Option<String>,
    pub cost_usd: Option<f64>,
    pub compactions: i64,
    pub tool_runs: i64,
    /// What the round said it did.
    pub progress: Option<String>,
}

/// A card, as the round that handed it in wrote it, with its rulings.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Card {
    pub id: i64,
    pub round: i64,
    pub work: i64,
    pub item: Option<i64>,
    /// design or ask.
    pub kind: String,
    /// open, ruled or replaced.
    pub status: String,
    pub body: Value,
    pub rulings: Value,
    pub replaced_by: Option<i64>,
}

/// What the owner or the architect said to a work.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Interjection {
    pub id: i64,
    pub work: i64,
    pub line: String,
    pub speaker: String,
    pub text: String,
    /// The round it was handed in; none while it waits.
    pub delivered_round: Option<i64>,
}

/// A work in full: where it stands as the engine keeps it, and every round it went through.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct WorkDetail {
    pub work: WorkSummary,
    pub machine: Option<String>,
    pub worktree: Option<String>,
    pub pushed_head: Option<String>,
    /// The cell the work left to ask, and the cells waiting for an answer, innermost last.
    pub resume_at: Option<String>,
    pub waiting: Value,
    /// The values its rows recorded.
    pub recorded: Value,
    pub rounds: Vec<RoundSummary>,
    pub cards: Vec<Card>,
    pub interjections: Vec<Interjection>,
}

/// The session of a line, as the command line knows it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Session {
    pub line: String,
    pub cli: String,
    pub cli_session: String,
}

/// One input a round was given, as a step that is not an agent's round records it, or as an agent's round recorded it
/// before its message was recorded with its materials: what it is, its value, and where its cell reads it from.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct RoundInput {
    pub what: String,
    pub value: Value,
    /// The sources the cell's definition names for it; empty when the cell no longer names this input.
    pub from: Vec<Src>,
}

/// One material of an agent round's message.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct RoundMaterial {
    /// Its address, such as `R12·2a·design`, `rule:ground` or `format:common`.
    pub address: String,
    /// The hash of its content, which tells two versions under one address apart.
    pub hash: String,
    /// Given in the message; otherwise the session already held it, and the message only referred to it.
    pub given: bool,
}

/// A file a tool run kept, or that its result points to, under the data directory.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct KeptFile {
    /// The path as the database holds it.
    pub path: String,
    /// Where the interface serves it: `logs/<name>` or `tool-output/<name>`, under `/api/v1/files/`.
    pub file: String,
}

/// One attempt of a tool the engine ran.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ToolRun {
    pub id: i64,
    pub tool: String,
    pub args: Value,
    pub attempt: i64,
    /// The result, a refusal and its reason included; none while it runs.
    pub result: Option<Value>,
    pub files: Vec<KeptFile>,
    pub queued_at: i64,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
}

/// A round in full: everything the engine gave, the return, what the engine made of it, and what it cost.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct RoundDetail {
    pub round: RoundSummary,
    pub work: i64,
    /// The work's flow version and graph, in which the round's cell is found.
    pub flow_version: i64,
    pub graph: String,
    pub session: Option<Session>,
    pub config_commit: String,
    /// The message exactly as the agent was sent it (for a round recorded before messages were, its prompt); none
    /// for a round no agent ran.
    pub message: Option<String>,
    /// Each material the message gave or referred to; empty for a round no agent ran, or one recorded before.
    pub materials: Vec<RoundMaterial>,
    /// How the command line was called: the whole command line, the session started or resumed, the engine's and
    /// the command line's versions; none for a round no agent ran, or one recorded before.
    pub call: Option<Value>,
    /// The inputs of a step that is not an agent's round, or of an agent's round recorded before its materials were.
    pub inputs: Vec<RoundInput>,
    /// What the engine gave, as recorded: the message, materials, call or inputs above, and what the engine knew
    /// besides, such as the head and the interjections handed in.
    pub input: Value,
    pub output: Option<Value>,
    /// Why the return was sent back or the run failed.
    pub rejected: Option<Value>,
    /// Tokens and cost as the command line reported them.
    pub usage: Option<Value>,
    pub tool_runs: Vec<ToolRun>,
    pub cards: Vec<Card>,
    /// What was said to the work and handed in with this round.
    pub interjections: Vec<Interjection>,
}

/// A stored flow version.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct FlowVersion {
    pub id: i64,
    pub digest: String,
    /// How many works are pinned to it.
    pub works: i64,
}

/// Where an item stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ItemState {
    /// Never worked on, and not in the queue.
    New,
    /// In the dispatch queue, waiting for a work to start.
    Queued,
    /// Its latest work runs, waits or is paused.
    Working,
    /// Its latest work reached its end.
    Done,
    /// Its latest work was stopped before its end.
    Stopped,
    Cancelled,
}

/// An item as a list shows it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ItemSummary {
    pub id: i64,
    pub project: String,
    pub title: String,
    /// change or report.
    pub deliver: String,
    pub level: String,
    pub cancelled: Option<String>,
    pub state: ItemState,
    /// Its place in the dispatch queue, lower first; none when it is not queued.
    pub queued: Option<f64>,
    /// Its latest work, the cell that work is at, and its pull request.
    pub work: Option<i64>,
    pub at: Option<String>,
    pub pr: Option<i64>,
}

/// A dependency: `item` cannot finish until `depends_on` is done.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ItemLink {
    pub item: i64,
    pub depends_on: i64,
    /// What `item` needs `depends_on` to hand in.
    pub needs: String,
}

/// Every item, and the dependencies between them.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Items {
    pub items: Vec<ItemSummary>,
    pub links: Vec<ItemLink>,
}

/// An item on the other end of a dependency, with what the dependency needs.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct LinkedItem {
    pub item: ItemSummary,
    pub needs: String,
}

/// An item in full: as its creator wrote it, its locked acceptance, its dependencies both ways, its works and cards.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ItemDetail {
    pub item: ItemSummary,
    pub problem: String,
    pub why: String,
    pub approach: Option<String>,
    /// The locked acceptance, and the round that locked it; none until a first design passes its checks.
    pub acceptance: Option<Value>,
    pub acceptance_round: Option<i64>,
    /// The items it depends on, and the items that depend on it.
    pub depends_on: Vec<LinkedItem>,
    pub depended_on_by: Vec<LinkedItem>,
    pub works: Vec<WorkSummary>,
    pub cards: Vec<Card>,
    /// What the rounds of its works handed back that their flow versions show on the item, oldest first.
    pub shown: Vec<Shown>,
}

/// A value a round handed back that the flow shows on the item the work is on: the owner reads it there.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Shown {
    pub work: i64,
    pub round: i64,
    pub cell: String,
    /// The path of the return it is at, as the flow writes it.
    pub path: String,
    pub value: Value,
}

/// The owner's ask to switch the engine's mode.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
pub struct SetMode {
    pub mode: Mode,
}

/// Why a request failed.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ApiError {
    pub error: String,
}

/// What the flow page shows of a compiled flow besides the flow itself, graph by graph.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct FlowView {
    pub graphs: Vec<GraphView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct GraphView {
    pub name: String,
    /// Each cell, in the order the graph has them.
    pub cells: Vec<CellView>,
}

/// One cell, as the flow page explains it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct CellView {
    pub id: String,
    /// For each row of its transition table, in order, the cells the row can lead to: its target, or the cells a
    /// target the engine keeps track of (`@self`, `@caller`, `@asker`, `@stopped`) can be.
    pub targets: Vec<Vec<String>>,
    /// Every row of every cell that can lead here, in the order of the cells and of their rows.
    pub entered_from: Vec<RowRef>,
    /// What a round of the cell is given: for an agent's cell, every material, each with its source and address;
    /// for any other cell, the sources of its inputs.
    pub materials: Vec<Material>,
    /// What the cell hands back, with whatever reads each field.
    pub returns: Returns,
}

/// A row of a cell, by its place in the cell's transition table, from 0.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct RowRef {
    pub cell: String,
    pub row: usize,
}

/// One material a round of a cell is given.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Material {
    /// Given every time the cell is entered, as the instruction: what the agent reads itself. Every other material
    /// is given when the session does not hold it, and referred to by its address when it does.
    pub always: bool,
    pub of: MaterialOf,
    /// The address it is given under, as the engine addresses it, with `‹round›`, `‹id›`, `‹cell›` or `‹name›`
    /// standing for what is known only when the round runs; none for what the agent reads itself.
    pub address: Option<String>,
    /// For a place of the cell the work came from, the cell that called or the cell the work stopped at: the cells
    /// it can be.
    pub cells: Vec<String>,
}

/// What a material is.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MaterialOf {
    /// One source of one of the cell's inputs, by their places in the cell's inputs and in the input's sources.
    Input { input: usize, source: usize },
    /// The project's rules, which every agent is given, as the project's configuration lists them.
    ProjectRules,
    /// A rule of the flow the cell is given.
    Rule { name: String },
    /// The project settings the project's rules name.
    Settings,
    /// The forms every agent's return shares.
    CommonFormat,
    /// The cell's own answer, and which of the shared forms the cell has.
    CellFormat,
}

/// What a cell hands back, as a tree of fields.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Returns {
    /// For an agent's cell: the field its return holds exactly one form in.
    pub within: Option<String>,
    /// For an agent's cell: the forms it may hand in, its own answer first.
    pub forms: Vec<ReturnForm>,
    /// The fields every return holds: for an agent's cell those beside the form, for any other cell all of them.
    pub every: Vec<Field>,
    /// What reads the return as a whole.
    pub used_by: Vec<Use>,
}

/// One form an agent's return may hold.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ReturnForm {
    /// The form's field, such as `ask` or `tools`; none for the cell's own answer.
    pub field: Option<String>,
    /// The form's fields: the own answer's fields, or the one field the form is.
    pub fields: Vec<Field>,
    /// What reads that a return is this form: the rows whose condition is the cell's own answer.
    pub used_by: Vec<Use>,
}

/// The kind of value a field holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Text,
    Integer,
    Number,
    Boolean,
    /// One of a fixed set of values.
    Enum,
    Object,
    List,
    /// One of several shapes, told apart by the value of one field.
    Union,
    /// Anything.
    Any,
}

/// A field of a return, with whatever reads it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Field {
    pub name: String,
    /// Its path as the flow writes it, such as `end.test_changes[added].catches`.
    pub path: String,
    pub kind: FieldKind,
    /// For a list, the kind of its items.
    pub item: Option<FieldKind>,
    pub desc: Option<String>,
    pub optional: bool,
    /// For a list, the fewest items it holds.
    pub min: Option<u64>,
    /// For one of a fixed set of values: each value, with what it means when the flow says.
    pub values: Vec<(String, Option<String>)>,
    pub used_by: Vec<Use>,
    /// The fields of an object, or of the objects a list holds.
    pub fields: Vec<Field>,
    /// For one of several shapes: the field whose value says which, and what reads that field.
    pub disc: Option<String>,
    pub disc_used_by: Vec<Use>,
    /// For one of several shapes: the fields every shape has, and each shape.
    pub common: Vec<Field>,
    pub branches: Vec<Branch>,
}

/// One of the shapes a field may take.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Branch {
    /// The value of the field that says which shape it is.
    pub value: String,
    /// Its path as the flow writes it, such as `result[green]`.
    pub path: String,
    pub desc: Option<String>,
    pub used_by: Vec<Use>,
    pub fields: Vec<Field>,
}

/// Something that reads a field of a return.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Use {
    /// An input of a cell, by its place in the cell's inputs.
    Input { cell: String, input: usize },
    /// The condition of a row.
    Condition { cell: String, row: usize },
    /// An action a row carries out on it.
    Action { cell: String, row: usize, action: String },
    /// What the engine does every time the cell returns it: shows it in a view, or carries out an action.
    Always {
        cell: String,
        view: Option<String>,
        action: Option<String>,
    },
    /// A check the engine makes on it before it takes a row.
    Check { cell: String, check: String },
    /// Handed to whoever takes over.
    Handoff { cell: String },
    /// Shown on the item the work is on: the owner reads it there.
    Shown { cell: String },
    /// Read outside this flow by a role, through the database.
    Role { cell: String, role: String },
}

/// What changed from one compiled flow to another, graph by graph.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct FlowDiff {
    pub graphs: Vec<GraphDiff>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct GraphDiff {
    pub name: String,
    /// Whether the graph was added or removed whole, or changed.
    pub status: ChangeKind,
    /// What changed in the graph apart from its cells: its start and end, its rules, the interjection, the counters.
    pub changes: Vec<Change>,
    /// Each cell added, removed or changed, in the order of the newer flow, the removed ones last.
    pub cells: Vec<CellDiff>,
}

/// Whether a thing was added, removed or changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Removed,
    Changed,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct CellDiff {
    pub id: String,
    /// Its title in the newer flow, or in the older one for a cell removed.
    pub title: String,
    pub status: ChangeKind,
    pub changes: Vec<Change>,
}

/// The part of a cell or a graph a change is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Part {
    Title,
    Kind,
    Gate,
    Line,
    Stage,
    Step,
    Needs,
    Input,
    Rule,
    Prompt,
    PromptBy,
    Forms,
    Field,
    Row,
    RowOrder,
    Check,
    Always,
    Handoff,
    Shown,
    ReadBy,
    Inenum,
    Start,
    End,
    Asking,
    Receive,
    Counter,
    Interjection,
    Rounds,
}

/// One change: the part, which one of it, and what it was and is, as the compiled flow writes it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Change {
    pub part: Part,
    /// Which one of the part: an input's text, a rule's or a row's name, a field's path, a check's name; empty for
    /// a part a cell has one of.
    pub key: String,
    /// What it was; none for what was added.
    pub before: Option<Value>,
    /// What it is; none for what was removed.
    pub after: Option<Value>,
}

/// A project's checkout, where the flow can be read at a git ref.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct FlowSource {
    pub project: String,
    pub checkout: String,
    /// Its branches, the most recently committed first.
    pub refs: Vec<String>,
}

/// The flow as `flow/flow.json` is at a git ref of a project's checkout, compiled by the flow's compiler.
#[derive(Debug, Clone, Serialize, TS)]
pub struct FlowAt {
    pub project: String,
    /// The ref as it was asked for, and the commit it names.
    pub rev: String,
    pub commit: String,
    pub digest: String,
    /// The stored flow version with the same digest, when there is one.
    pub version: Option<i64>,
    pub compiled: flow::Compiled,
    pub view: FlowView,
}
