//! The format of `flow.json`, the flow as people write it. Every part says what it is for; a key the format does
//! not know is refused, so a misspelt key is an error and not a part silently left out.
//!
//! A flow is a graph of cells. Each cell takes inputs, hands back an output whose shape the flow fixes, and has a
//! table of rows: the first row whose conditions hold on the output says where the work goes next, what the engine
//! does on the way and what it records. The compiler (`crate::compile`) expands what cells share and checks the whole.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

/// The whole of `flow.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// The format's name and version: `wf-agents flow 1`.
    pub format: String,
    /// Field shapes several places share, by name: a field `{"$ref": name}` is that shape, with any key it gives
    /// itself taking the place of the shape's.
    pub shapes: BTreeMap<String, RawField>,
    /// Every tool an agent may ask the engine to run: what it is handed and what it gives back.
    pub tools: Vec<Tool>,
    /// How the meanings of an enum's values are folded into the field's description in a return schema.
    #[serde(rename = "enumText")]
    pub enum_text: EnumText,
    /// Every name the flow gives the engine to carry out, each with what it does. The engine has one
    /// implementation for each name, and the compiler checks that every name is used.
    pub engine: EngineNames,
    /// The rules every graph shares: each is given to every agent's cell on the lines it names, in whichever graph,
    /// before the graph's own rules.
    #[serde(default)]
    pub rules: Vec<Rule>,
    /// The graphs: the engineer's, which works on items run, and the architect's, which its works run.
    pub flows: Vec<Flow>,
}

/// The engine's part of the flow, by name. Each map takes a name to what it does.
#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct EngineNames {
    /// What a cell that is not an agent does when the work reaches it: a cell's `step`.
    pub steps: BTreeMap<String, String>,
    /// What the engine does on a row, or every time a cell returns a field: a row's `do`, an `always` with `do`.
    pub actions: BTreeMap<String, String>,
    /// What the engine checks on a return before it takes a row; a return that fails goes back to the agent.
    pub checks: BTreeMap<String, String>,
    /// What the engine computes for a cell's input: an input source `{"engine": name}`.
    pub inputs: BTreeMap<String, String>,
    /// The values the engine itself knows about a work, which rows switch on with `inp` and cells name in
    /// `inenums`; a value rows record with `sets` is not one of these.
    pub values: BTreeMap<String, String>,
    /// What a gate needs to apply to a project. A gate whose need the project does not meet is passed by the
    /// engine on its own row, without a round.
    pub needs: BTreeMap<String, String>,
    /// The places the engine keeps what it writes and reads later, by name: what each holds, the layer it belongs to
    /// and the tables and views that hold it. None in a flow version stored before the flow named them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub stores: BTreeMap<String, Store>,
    /// The stores each action writes, and each step that writes one itself: every action has an entry, an empty
    /// list for one that only tells the owner or acts outside the database.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub writes: BTreeMap<String, Vec<String>>,
    /// What each of the engine's values reads, in a few words for the chart, and the stores it reads; each value has
    /// an entry, and a step that reads a store no input of its cell names has one too.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub reads: BTreeMap<String, Reads>,
    /// What a flow version stored while the setting of a policy was a graph of its own said of the tasks an action
    /// started in another graph; read only so that such a version still loads, and used for nothing.
    #[serde(default, skip_serializing)]
    #[ts(skip)]
    pub tasks: Option<serde_json::Value>,
}

/// A place the engine keeps something in, which a cell or the owner writes and a cell reads later.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Store {
    pub title: String,
    pub desc: String,
    pub layer: Layer,
    /// Worked out from the rounds when it is read, not written by anyone.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub computed: bool,
    /// The tables and views of the database that hold it: an input that reads one of them reads this store.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relations: Vec<String>,
    /// The engine's inputs computed from it: an input that names one of them reads this store.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<String>,
    /// What the owner does that writes it, when the owner writes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

/// Where a store belongs: a project holds items, an item holds works, a work holds rounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    Project,
    Item,
    Work,
    Round,
}

/// What a value of the engine, or a step, reads.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Reads {
    /// What it is worked out from, in a few words, as the chart says it under the cell; every value has it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub says: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stores: Vec<String>,
}

/// How enum values join a field's description: `withDesc` with `{desc}` and `{values}`, `alone` with `{values}`,
/// each value as `value` with `{value}` and `{means}`, joined by `valueJoin`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnumText {
    #[serde(rename = "withDesc")]
    pub with_desc: String,
    pub alone: String,
    pub value: String,
    #[serde(rename = "valueJoin")]
    pub value_join: String,
}

/// A tool an agent may ask for.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tool {
    pub name: String,
    /// What the agent is told the tool does.
    pub desc: String,
    /// What the engine does to run it.
    pub runs: String,
    /// For a tool that reads the database: what leads the index of the tables and views it reads, which the
    /// compiler adds to its description, one line for each with its columns and the start of the first line of what
    /// it holds.
    pub tables: Option<String>,
    /// What the agent hands in.
    #[serde(rename = "in")]
    pub input: Vec<RawField>,
    /// What the tool gives back.
    pub out: Vec<RawField>,
}

/// A field of an output, as written. Every key but `$ref` may be left out where a shape gives it.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawField {
    /// The shape this field is.
    #[serde(rename = "$ref")]
    pub shape: Option<String>,
    /// The field's name; none for the item of a list.
    pub name: Option<String>,
    /// What the field is, in a few words for the owner: the owner's page heads the field with it. Every field with a
    /// name has one. No agent is sent it: an agent reads the description.
    pub title: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<Kind>,
    /// What the agent is told the field is.
    pub desc: Option<String>,
    /// Whether a return may leave it out.
    pub optional: Option<bool>,
    /// An enum's values.
    pub values: Option<Values>,
    /// An object's fields. An object with none listed, not even an empty list, holds whatever fields its value has,
    /// such as a row a query found.
    pub fields: Option<Vec<RawField>>,
    /// A list's item.
    pub item: Option<Box<RawField>>,
    /// A union's branches.
    pub branches: Option<Branches>,
    /// The fields every branch of a union has.
    pub common: Option<Vec<RawField>>,
    /// The field that tells a union's branches apart.
    pub by: Option<String>,
    /// The fewest items a list holds.
    pub min: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Text,
    Int,
    Bool,
    Enum,
    Obj,
    List,
    Union,
}

/// An enum's values: each value with what it means, or a name for values given elsewhere: an enum shape,
/// `askKinds` (the kinds of question an agent may ask) or `interjectFrom` (who may interject).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Values {
    Named(String),
    List(Vec<(String, String)>),
}

/// A union's branches, or a name for branches made from elsewhere: `tools` (one per tool the cell may ask for),
/// `toolResults` (one per tool of the flow) or `askRoles` (one per role the cell may ask).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Branches {
    Named(String),
    List(Vec<RawBranch>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawBranch {
    /// The value of the union's `by` field that picks this branch.
    pub value: String,
    pub desc: Option<String>,
    pub fields: Vec<RawField>,
}

/// One graph.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Flow {
    pub name: String,
    /// The cell a work starts at, and the one it ends at.
    pub start: String,
    pub end: String,
    /// The end a work cancelled or taken over at any cell ends at: the engine ends it there, from wherever it stands,
    /// so that no row leads to it; the work keeps the cell it stopped at.
    pub cancel: Option<String>,
    /// Parts of the graph: a row from one block into another goes to one of that block's entries.
    pub blocks: Vec<Block>,
    /// The lines: an agent's cell on a line resumes the session the line's earlier cells left.
    pub sessions: Sessions,
    /// The graph's own rules the engine gives the agents that name them, as written.
    pub rules: Vec<Rule>,
    /// The stages the owner sees a work at.
    pub stages: Stages,
    /// The part of the graph that asks someone and brings the answer back.
    pub asking: Asking,
    /// The counters that keep every loop finite.
    pub limits: Limits,
    /// The kinds of question an agent may ask, in groups.
    #[serde(rename = "askKinds")]
    pub ask_kinds: Vec<AskGroup>,
    /// What every agent's cell has.
    #[serde(rename = "agentCommon")]
    pub agent_common: AgentCommon,
    /// What every gate shares. A gate is an agent's cell that asks one question whose yes is a matter to ask about.
    pub gate: Option<Gate>,
    pub nodes: Vec<Node>,
    /// The rules for the design card, as the owner reads them.
    #[serde(rename = "designCard", default)]
    pub design_card: Vec<String>,
    /// What the owner or the architect says to a running work, and who may.
    pub interject: Option<Interject>,
    /// What the engine does with an agent's round that does not end in a return the cell takes.
    pub rounds: Option<Rounds>,
    /// What a stopped task of this graph waits on: the cells it stands at while each holder holds its blocker.
    pub blockers: Option<Blockers>,
    /// The holder whose own work this graph is, as the architect's works are the architect's: a holder never stops
    /// while it holds a blocker, so no row of this graph waits but for the owner's Start after a pause, its start's
    /// where no work stands yet, and a wait an installation's setting bounds (`until`), and an ask of the owner holds
    /// only what it is about, going on without the answer.
    pub holds: Option<String>,
}

/// What a task of a graph waits on when it stops: a blocker, raised at the cell where its problem arose and held by
/// one holder at a time, at whose cell the task stands. Every move of a blocker is a row: the rows into a holder's cell
/// raise it, the rows between holders' cells hand it on, and the rows out of them resolve or withdraw it.
#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Blockers {
    pub desc: String,
    /// The cells a stopped task stands at, each with whom it is held by there.
    pub cells: Vec<BlockerCell>,
}

/// A cell a stopped task stands at while its blocker is held there.
#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct BlockerCell {
    pub cell: String,
    /// Who may hold the blocker while the task stands here: advisor, architect, owner or items.
    pub holders: Vec<String>,
    /// The rows that take the task on when its holder fails it, each named after the way it fails, such as an item it
    /// waits for cancelled.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fails: Vec<String>,
    /// The task stands here on its blocker only once it took the cell's row that waits, which raises it; without, from
    /// the row that brought it here.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub waits: bool,
}

/// What the engine does with an agent's round that does not end in a return the cell takes: a return sent back, or
/// a run of the agent's command line that ended without one. Every agent's cell has this alike.
#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Rounds {
    pub desc: String,
    #[serde(rename = "sentBack")]
    pub sent_back: SentBack,
    pub failed: Failed,
    /// What every round records.
    pub record: Vec<String>,
    /// What an agent's command line starts with, apart from what the flow gives it.
    pub environment: Vec<String>,
    /// The other ways a round ends in no return the cell takes, each the engine's own: every agent's cell has a
    /// shared row of each one's name for it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub engine: Vec<EngineCase>,
}

/// A way a round ends that is the engine's, not the agent's: a round cut off, a step it cannot take, a message it does
/// not send, a work that ended meanwhile.
#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct EngineCase {
    pub name: String,
    pub when: String,
    pub desc: String,
    /// The work pauses, and the owner is told: a blocker the owner holds at the cell.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pauses: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SentBack {
    pub desc: String,
    /// How many returns of one cell in a row may be sent back before the engine pauses the work.
    #[serde(rename = "pauseAfter")]
    pub pause_after: u32,
    pub pause: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Failed {
    pub desc: String,
    pub kinds: Vec<Failure>,
}

/// One kind of run that ended without a return; every agent's cell has a shared row of its name for it.
#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Failure {
    pub name: String,
    /// What the command line reports when this happens.
    pub when: String,
    pub handle: Handle,
    pub desc: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum Handle {
    /// No new round starts for any work until the cause passes; then each work goes on from its cell.
    Wait,
    /// The same round runs again, waiting longer each time, until it passes.
    Retry,
    /// The work pauses, and the owner is told; once the owner lets it go on, the same round runs again.
    Pause,
    /// The return is repaired, or the agent hands it in again, within the same round; when neither gives a return,
    /// the work pauses, and the owner is told.
    Repair,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Block {
    pub name: String,
    pub nodes: Option<Vec<String>>,
    pub entries: Option<Vec<String>>,
    /// The block is the asking part: its cells are `asking.nodes`, its entries the cells that are asked.
    pub asking: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Named {
    pub name: String,
    pub desc: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sessions {
    pub lines: Vec<Line>,
    /// How sessions are kept, for people.
    pub rules: Vec<String>,
}

/// A line: the agent whose cells go on in one session. The fixed places it starts a new one at are the cells a row
/// leads to with the action [`OPEN_SESSION`]: at the work's first round there.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Line {
    pub name: String,
    pub desc: String,
    /// The colour the line's role is drawn in wherever the owner's page shows it; no two lines of the flow share one.
    pub colour: Colour,
    /// Where the work goes when a new session of the line would start at one of its cells: a row every agent's cell
    /// of the line has, but those it names, which the engine takes itself in place of the cell's round.
    pub anew: Option<Anew>,
}

/// A colour a line's role is drawn in on the owner's page (its cells on a chart, its avatar and its bubbles), by the
/// name of Primer's display colour the page draws it with. Only the colours the page knows are here, and none of
/// those it keeps for other things: orange for the owner's cells, yellow for the tools', red for danger and grey for
/// the engine's own steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Colour {
    Blue,
    Teal,
    Auburn,
    Pink,
    Purple,
    Indigo,
    Lime,
}

impl std::fmt::Display for Colour {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Its name as the flow writes it.
        let name = serde_json::to_value(self).map_err(|_| std::fmt::Error)?;
        write!(f, "{}", name.as_str().unwrap_or_default())
    }
}

/// A row the cells of a line share, which the engine takes in place of a round when the line's session would start
/// afresh there.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Anew {
    pub row: String,
    pub to: String,
    /// What the row says, for people.
    pub note: String,
    /// The cells of the line that do not have the row.
    #[serde(default)]
    pub not: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub name: String,
    pub desc: String,
    pub text: Vec<String>,
    /// The lines every agent's cell of which is given the rule, besides the cells that name it; none: only those.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<String>,
    /// What a cell given any of must be given the rule with, by an engine input's name, a table's or a view's, or a
    /// cell's return place as `cell·place`: the compiler refuses an agent's cell given one of them without the rule.
    #[serde(default, rename = "givenWith", skip_serializing_if = "Vec::is_empty")]
    pub given_with: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stages {
    /// How stages are shown, for people.
    pub rules: Vec<String>,
    pub list: Vec<Stage>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stage {
    pub name: String,
    pub desc: String,
    /// The cells at this stage. A cell at no stage asks, answers or runs tools for another, and is at the stage of
    /// the cell the work stopped at.
    pub nodes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asking {
    /// The cells from the one that asks until the work is back there.
    pub nodes: Vec<String>,
    /// The cell that receives an answer to an agent's cell on its line; with none, every answer goes back to the
    /// cell that asked.
    pub receive: Option<String>,
    pub roles: Vec<Role>,
    /// The counter every question counts.
    pub counts: String,
    /// What a cell that is asked has for each role it may hand on to, in place of asking it: a row that passes the
    /// work on, the holder that hands on holding nothing then.
    #[serde(rename = "handOver")]
    pub hand_over: Option<HandOver>,
}

/// The row every cell that is asked has for each other role, which hands the blocker on to it instead of asking it:
/// the cell that is asked never stops to wait for an answer itself.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandOver {
    /// The row's name, with `{name}` for the role's.
    pub row: String,
    #[serde(rename = "do")]
    pub actions: Vec<Do>,
    /// The counter the row counts: a blocker handed back and forth is a loop.
    pub counts: String,
    /// Values the engine records for the work.
    pub sets: Option<BTreeMap<String, String>>,
    pub note: Option<String>,
}

/// Someone an agent may ask: the cell that is asked, and the row every asking cell gets for it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Role {
    pub name: String,
    pub desc: String,
    pub node: String,
    pub row: String,
    /// What a question to this role holds besides what every question holds.
    pub fields: Vec<RawField>,
    #[serde(rename = "do")]
    pub actions: Option<Vec<Do>>,
    /// Values the engine records for the work when it asks this role.
    pub sets: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    #[serde(rename = "where")]
    pub where_: String,
    pub waits: String,
    pub over: String,
    pub counters: Vec<Counter>,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Counter {
    pub name: String,
    pub desc: String,
    /// The counter is the work's total time; exactly one is.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub time: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AskGroup {
    pub group: String,
    pub kinds: Vec<(String, String)>,
    /// The one role these kinds may be asked of.
    pub only: Option<String>,
    /// Kinds of the group that only a project meeting a need can meet, each with the need, one of `engine.needs`: an
    /// agent working on a project that does not meet it is not told of the kind.
    #[serde(default)]
    pub needs: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentCommon {
    pub returns: Returns,
    /// The rows every agent's table ends with.
    pub rows: Vec<RawRow>,
    /// The inputs every agent's round is given.
    #[serde(default)]
    pub inputs: Vec<Input>,
    /// The fields every agent's return may hold besides its own.
    pub out: Vec<RawField>,
    #[serde(default)]
    pub always: Vec<Always>,
    #[serde(default)]
    pub checks: Vec<Check>,
    /// Paths of every agent's return the owner's page shows on the item the work is on.
    #[serde(default)]
    pub shown: Vec<String>,
    /// What the description of each field an agent's cell shows on the item adds: the language it is written in.
    #[serde(default, rename = "shownSay")]
    pub shown_say: String,
    /// Places of every agent's return a role reads outside this flow, through the database.
    #[serde(default, rename = "readBy")]
    pub read_by: Vec<ReadBy>,
    /// Fields of `every` that only a project meeting a need is asked for, each with the need, one of `engine.needs`:
    /// an agent working on a project that does not meet it is not asked for the field.
    #[serde(default)]
    pub needs: BTreeMap<String, String>,
    /// Fields of `every` that only the agents' cells of some lines hold, each with those lines: a cell of another line
    /// neither holds the field nor does, shows or has read what is at it.
    #[serde(default)]
    pub lines: BTreeMap<String, Vec<String>>,
}

/// A place of a return a role reads outside this flow, through the database, such as the architect reading side
/// reports: what the role does with it is not a cell of this flow.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ReadBy {
    pub at: String,
    /// The role that reads it, one of the roles an agent may ask.
    pub role: String,
}

/// What one return of an agent holds: exactly one form, inside the field `in`, and every field of `every`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Returns {
    pub forms: Vec<Form>,
    pub every: Vec<String>,
    /// The sentence the agent is told, with `{in}`, `{forms}` and `{every}`.
    pub say: String,
    #[serde(rename = "in")]
    pub within: InField,
    /// How the names in `say` are joined.
    pub join: String,
    /// How a field is named in `say`, with `{name}`.
    pub name: String,
    /// How every return is written, said once in the forms every return shares: that only the return is read, and
    /// how a field the owner reads is written.
    #[serde(default)]
    pub write: Vec<String>,
    /// Fields of the cells' own answers that several cells hand in alike: the forms every return shares give each
    /// once, and a cell's own format refers to it there.
    #[serde(default)]
    pub once: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InField {
    pub name: String,
    pub desc: String,
}

/// One form of an agent's return: the cell's own answer, or one of the common fields.
#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Form {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub own: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    pub say: String,
    /// The lines whose cells outside the asking part have this form; with none, every agent's cell has it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<Vec<String>>,
    /// Fields of `every` a return of this form does not hold. It leaves them as an earlier return wrote them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub without: Vec<String>,
}

/// What every gate shares. A gate answers its question with one of two named outcomes: the one that goes on, which
/// it hands in as its answer and whose row it takes, and the problem, which it asks about.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gate {
    pub tools: Vec<String>,
    pub line: String,
    #[serde(default)]
    pub inputs: Vec<Input>,
    /// Said after the gate's question, with `{on}` and `{problem}` for its two outcomes and `{about}` for the kind
    /// to ask about.
    pub prompt: String,
    /// What the answer is, with `{problem}`.
    pub answer: String,
    /// The answer's title, which the owner's page heads it with.
    #[serde(rename = "answerTitle")]
    pub answer_title: String,
    /// What the answer's why is.
    pub why: String,
    /// The title of the answer's why.
    #[serde(rename = "whyTitle")]
    pub why_title: String,
    /// For a gate whose question may not fit the work: it answers its outcome that goes on, or not applicable.
    pub na: GateNa,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateNa {
    /// With `{on}`, `{problem}`, `{about}`, and `{na}` for the gate's example of when it does not apply.
    pub prompt: String,
    /// The outcome not applicable, with what it means, and what its why is.
    pub value: (String, String),
    pub why: String,
}

/// A gate's two outcomes, each a name with what it means.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outcomes {
    /// The outcome that goes on: the gate hands it in as its answer, and its row is named after it.
    pub on: (String, String),
    /// The outcome that is the problem: the gate asks about it, with the gate's kind.
    pub problem: (String, String),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interject {
    pub name: String,
    pub desc: String,
    pub text: Vec<String>,
    pub from: Vec<Named>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    /// A step the engine takes itself.
    Engine,
    /// A round of an agent.
    Agent,
    /// An agent's cell that asks one question; it becomes an agent's cell.
    Gate,
    /// A step that runs tools.
    Tool,
    /// The owner's answer.
    Owner,
}

/// A cell, as written.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub title: String,
    /// What the engine does at a cell that is not an agent: one of `engine.steps`.
    pub step: Option<String>,
    pub line: Option<String>,
    /// The tools an agent's cell may ask for.
    pub tools: Option<Vec<String>>,
    #[serde(default)]
    pub inputs: Vec<Input>,
    pub rules: Option<Vec<String>>,
    pub prompt: Option<Vec<String>>,
    /// Lines added to the prompt by a value the engine knows, one list per value.
    #[serde(rename = "promptBy")]
    pub prompt_by: Option<PromptBy>,
    pub out: Option<Vec<RawField>>,
    pub cases: Option<Vec<RawRow>>,
    pub always: Option<Vec<Always>>,
    pub checks: Option<Vec<Check>>,
    /// Paths of the output that the cell hands to whoever takes over.
    pub handoff: Option<Vec<String>>,
    /// Paths of the output the owner's page shows: on the item the work is on, or for a work of the architect, which
    /// is on no item, on the architect's view. The owner reads them there.
    pub shown: Option<Vec<String>>,
    /// The values the engine knows that the cell's rows or prompt read: each a list of values with what each
    /// means, or the name of an enum shape.
    pub inenums: Option<BTreeMap<String, Values>>,
    /// The roles an agent's cell may ask; with none, every role but its own.
    pub asks: Option<Vec<String>>,
    /// The forms of the return every agent's cell shares that this cell does not have, by their fields: an agent's
    /// cell of a line that may ask, run tools or search, but that is to answer by itself, says so here.
    pub without: Option<Vec<String>>,
    /// A gate's kind of question to ask about, one of `askKinds`.
    pub about: Option<String>,
    /// A gate's two outcomes.
    pub outcomes: Option<Outcomes>,
    /// An owner's cell that rules by buttons: one for each value of its `ruling.choice`, in order.
    pub choices: Option<Vec<Choice>>,
    /// What the engine does at the cell besides what its step works out and its rows do, in order: at an end, how
    /// the work ends; elsewhere, before the step. Each is one of `engine.actions`.
    #[serde(rename = "do")]
    pub actions: Option<Vec<String>>,
    /// What a gate needs to apply, one of `engine.needs`.
    pub needs: Option<String>,
    pub question: Option<String>,
    /// For a gate that may not apply: an example of a work it does not apply to.
    pub na: Option<String>,
    /// Where a gate's own answer goes; a gate has this or `rows`.
    pub to: Option<String>,
    /// What a gate's own answer records.
    pub sets: Option<BTreeMap<String, String>>,
    /// A gate's rows, each holding on its own answer.
    pub rows: Option<Vec<RawRow>>,
    /// The reasoning effort an agent's cell runs at, whatever the model its item's level picks: one of [`EFFORTS`].
    /// Every agent's cell declares one.
    pub effort: Option<String>,
}

/// The action on a row that opens a new session of the line of the cell it leads to, at the work's first round there:
/// a place the flow fixes for a line's session to start afresh.
pub const OPEN_SESSION: &str = "open_session";

/// The action on a row into a holder's cell that raises the task's blocker there.
pub const RAISE_BLOCKER: &str = "raise_blocker";

/// The action on a row out of a holder's cell that resolves the task's blocker, with what its holder decided.
pub const RESOLVE_BLOCKER: &str = "resolve_blocker";

/// The action on a row out of a holder's cell that withdraws the task's blocker, saying why.
pub const WITHDRAW_BLOCKER: &str = "withdraw_blocker";

/// The action on a row that passes the work on to another holder's cell: the blocker is handed on, saying why and what
/// the holder that hands it on found.
pub const HAND_OVER: &str = "hand_over";

/// The row, and the engine's case it takes it on, of a cell whose numbered table it must answer has no row: where the
/// table says the work goes then ([`Empty`]), in place of the cell's round.
pub const NOTHING_TO_ANSWER: &str = "nothing_to_answer";

/// Who may hold a blocker.
pub const HOLDERS: [&str; 4] = ["advisor", "architect", "owner", "items"];

/// The reasoning efforts an agent's cell may run at, as the command line names them.
pub const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// A button an owner's cell rules by: the value of `ruling.choice` it hands back, what the button says it does, and
/// what happens next, in one line under it. The owner's page shows them where the owner rules and on the cell's panel;
/// no agent is sent them.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub value: String,
    pub title: String,
    pub desc: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PromptBy {
    pub by: String,
    pub cases: BTreeMap<String, Vec<String>>,
}

/// A row of a cell's table.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawRow {
    pub name: String,
    /// The conditions, all of which hold.
    #[serde(rename = "if")]
    pub conditions: Vec<Atom>,
    /// A cell, or `@caller`, `@asker`, `@stopped`, `@self`.
    pub to: String,
    /// Values the engine records for the work.
    pub sets: Option<BTreeMap<String, String>>,
    #[serde(rename = "do")]
    pub actions: Option<Vec<Do>>,
    /// The counter the row counts.
    pub counts: Option<String>,
    /// The row waits for someone, and counts nothing.
    pub waits: Option<bool>,
    /// For a row that waits, the installation's setting that bounds the wait: the cell takes another row once it ran
    /// out, so that a holder's own graph waits no longer than that, as the architect for the owner's reply in a talk.
    pub until: Option<String>,
    /// The row takes an interjection to the cell that receives it.
    pub interject: Option<bool>,
    /// The row is on the main path; a work off it is an anomaly to look at.
    pub main: Option<bool>,
    /// The row goes on without answering the cells that wait for an answer: they get it later, another way, such as
    /// an ask of the owner that holds only the matter it is about while the rest goes on.
    pub drops: Option<bool>,
    /// The row passes the work on, within the asking part, from a cell that answers the cell that asked to another
    /// that answers it in its place: a blocker handed to another holder, or taken to the cell that judges whether to
    /// wait longer. The cells that wait for an answer stay as they are.
    pub passes: Option<bool>,
    pub note: Option<String>,
}

/// An action on a row: a name, or a name done for a place of the output.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(untagged)]
pub enum Do {
    Name(String),
    At(DoAt),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DoAt {
    pub at: String,
    #[serde(rename = "do")]
    pub action: String,
}

impl Do {
    pub fn name(&self) -> &str {
        match self {
            Do::Name(n) => n,
            Do::At(x) => &x.action,
        }
    }

    pub fn at(&self) -> Option<&str> {
        match self {
            Do::Name(_) => None,
            Do::At(x) => Some(&x.at),
        }
    }
}

/// What the engine does every time a cell's return holds a place: the place that sets it off, and the action. The
/// flow names only actions here; a flow version stored before may name a view instead, which did nothing.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Always {
    /// The place of the return whose presence sets the action off.
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(rename = "do", default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

/// A check the engine makes on a return, on a place of it or on the whole.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Check {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    pub check: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    /// The name the cell's rows, actions and checks read the input by, as `@<id>`.
    pub id: Option<String>,
    /// What the input is called, in a few words for the owner: the owner's page heads the input with it. Every input
    /// has one. An input every agent's cell has is also named so to the agent: each cell's list of what its inputs
    /// are names it only, and the forms every return shares say once what it is. No agent is sent the name of any
    /// other input.
    pub name: Option<String>,
    /// For an input that is a table the cell answers row by row: the prefix its rows are numbered with. The engine
    /// gives the input as one table, each finding, column, test, conflict, check and entry its sources hold a row,
    /// numbered across all of them (F1, F2, ...), and the cell answers each row by its number.
    pub numbered: Option<String>,
    /// For a numbered table the cell must answer: where the work goes when the table has no row, so that a round
    /// that could answer nothing is never the only way on. Every such table names it.
    pub empty: Option<Empty>,
    /// What the input is, as the agent is told.
    pub what: String,
    pub from: Vec<Src>,
    /// The input may be missing on some way to the cell.
    pub maybe: Option<bool>,
    /// A common input given only to the cells that have this form.
    pub with: Option<String>,
    /// The input reads each place from its cell's newest answer only: a field that answer leaves out is none there,
    /// as a branch of a return is, instead of the newest answer that has it.
    pub newest: Option<bool>,
}

/// Where a work goes from a cell whose numbered table it must answer has no row: the cell's row `nothing_to_answer`
/// ([`NOTHING_TO_ANSWER`]), which the engine takes in place of the cell's round and records with why.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Empty {
    pub to: String,
    /// What the row says, for people.
    pub note: String,
}

/// Where an input comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(try_from = "Value", into = "Value")]
#[ts(
    type = "[string, string] | \"self\" | { config: string } | { db: string, by?: string } | { engine: string } | { recorded: string }"
)]
pub enum Src {
    /// A place of a cell's last return: a cell, or `@stopped`, `@caller` or `@from`, and a path.
    Place(String, String),
    /// What the agent reads itself.
    Itself,
    /// A setting of the project's configuration.
    Config(String),
    /// A table or view of the database, for the item, work or project at hand when `by` names that column.
    Db { relation: String, by: Option<String> },
    /// What the engine computes, one of `engine.inputs`.
    Engine(String),
    /// A value rows recorded for the work.
    Recorded(String),
}

impl TryFrom<Value> for Src {
    type Error = String;

    fn try_from(v: Value) -> Result<Self, String> {
        let bad = || format!("an input source fits none of the forms: {v}");
        match &v {
            Value::Array(a) => match a.as_slice() {
                [Value::String(n), Value::String(p)] => Ok(Src::Place(n.clone(), p.clone())),
                _ => Err(bad()),
            },
            Value::String(s) if s == "self" => Ok(Src::Itself),
            Value::Object(o) => {
                let s = |k: &str| o.get(k).and_then(Value::as_str).map(str::to_string);
                let keys: BTreeSet<&str> = o.keys().map(String::as_str).collect();
                let only = |want: &[&str]| keys == want.iter().copied().collect::<BTreeSet<&str>>();
                if let Some(x) = s("config")
                    && only(&["config"])
                {
                    Ok(Src::Config(x))
                } else if let Some(x) = s("engine")
                    && only(&["engine"])
                {
                    Ok(Src::Engine(x))
                } else if let Some(x) = s("recorded")
                    && only(&["recorded"])
                {
                    Ok(Src::Recorded(x))
                } else if let Some(x) = s("db")
                    && (only(&["db"]) || (only(&["db", "by"]) && s("by").is_some()))
                {
                    Ok(Src::Db {
                        relation: x,
                        by: s("by"),
                    })
                } else {
                    Err(bad())
                }
            }
            _ => Err(bad()),
        }
    }
}

impl From<Src> for Value {
    fn from(s: Src) -> Value {
        match s {
            Src::Place(n, p) => Value::Array(vec![Value::String(n), Value::String(p)]),
            Src::Itself => Value::String("self".into()),
            Src::Config(x) => serde_json::json!({ "config": x }),
            Src::Db { relation, by: None } => serde_json::json!({ "db": relation }),
            Src::Db { relation, by: Some(b) } => serde_json::json!({ "db": relation, "by": b }),
            Src::Engine(x) => serde_json::json!({ "engine": x }),
            Src::Recorded(x) => serde_json::json!({ "recorded": x }),
        }
    }
}

/// A condition of a row, written as a list: `["all"]`; `["own"]` (the return is the cell's own answer);
/// `["or", [conditions]]`; `["has", path]` (the place is there); `["is", path, values]` (its value is one of
/// them); `["any", path, values]` and `["none", path, values]` (some value, or no value, in a list is one of them);
/// `["inp", name, values]` (a value the engine knows or rows recorded is one of them). Three hold for no return, and
/// only the shared rows the compiler gives every agent's cell have them: `["sent_back"]` (the return is sent back),
/// with a number (that many of the cell's returns in a row are), `["failed", kind]` (the run ended without a return,
/// of that kind) and `["engine", case]` (the engine took the row itself, for that case: one of the rounds section's
/// own, or a line's new session).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(try_from = "Value", into = "Value")]
#[ts(
    type = "[\"all\"] | [\"own\"] | [\"or\", Array<Atom>] | [\"has\", string] | [\"is\" | \"any\" | \"none\" | \"inp\", string, Array<string>] | [\"sent_back\"] | [\"sent_back\", number] | [\"failed\", string] | [\"engine\", string]"
)]
pub enum Atom {
    All,
    Own,
    Or(Vec<Atom>),
    Has(String),
    Is(String, BTreeSet<String>),
    AnyOf(String, BTreeSet<String>),
    NoneOf(String, BTreeSet<String>),
    Inp(String, BTreeSet<String>),
    SentBack(Option<u32>),
    Failed(String),
    Engine(String),
}

impl Atom {
    /// Whether the condition is about a round that ended in no return the cell takes, which no return meets.
    pub fn about_no_return(&self) -> bool {
        match self {
            Atom::SentBack(_) | Atom::Failed(_) | Atom::Engine(_) => true,
            Atom::Or(xs) => xs.iter().any(Atom::about_no_return),
            _ => false,
        }
    }
}

impl TryFrom<Value> for Atom {
    type Error = String;

    fn try_from(v: Value) -> Result<Self, String> {
        let bad = || format!("a condition fits none of the forms: {v}");
        let Value::Array(a) = &v else { return Err(bad()) };
        let word = a.first().and_then(Value::as_str).ok_or_else(bad)?;
        let text = |i: usize| a.get(i).and_then(Value::as_str).map(str::to_string).ok_or_else(bad);
        let set = |i: usize| -> Result<BTreeSet<String>, String> {
            let Some(Value::Array(xs)) = a.get(i) else {
                return Err(bad());
            };
            xs.iter()
                .map(|x| x.as_str().map(str::to_string).ok_or_else(bad))
                .collect()
        };
        let len = |n: usize| if a.len() == n { Ok(()) } else { Err(bad()) };
        match word {
            "all" => len(1).map(|_| Atom::All),
            "own" => len(1).map(|_| Atom::Own),
            "or" => {
                len(2)?;
                let Some(Value::Array(xs)) = a.get(1) else {
                    return Err(bad());
                };
                xs.iter()
                    .cloned()
                    .map(Atom::try_from)
                    .collect::<Result<_, _>>()
                    .map(Atom::Or)
            }
            "has" => len(2).and_then(|_| text(1)).map(Atom::Has),
            "sent_back" if a.len() == 1 => Ok(Atom::SentBack(None)),
            "sent_back" => {
                len(2)?;
                let n = a[1].as_u64().and_then(|n| u32::try_from(n).ok()).ok_or_else(bad)?;
                Ok(Atom::SentBack(Some(n)))
            }
            "failed" => len(2).and_then(|_| text(1)).map(Atom::Failed),
            "engine" => len(2).and_then(|_| text(1)).map(Atom::Engine),
            "is" | "any" | "none" | "inp" => {
                len(3)?;
                let (p, s) = (text(1)?, set(2)?);
                Ok(match word {
                    "is" => Atom::Is(p, s),
                    "any" => Atom::AnyOf(p, s),
                    "none" => Atom::NoneOf(p, s),
                    _ => Atom::Inp(p, s),
                })
            }
            _ => Err(bad()),
        }
    }
}

impl From<Atom> for Value {
    fn from(a: Atom) -> Value {
        let set = |s: BTreeSet<String>| Value::Array(s.into_iter().map(Value::String).collect());
        match a {
            Atom::All => serde_json::json!(["all"]),
            Atom::Own => serde_json::json!(["own"]),
            Atom::Or(xs) => serde_json::json!(["or", xs.into_iter().map(Value::from).collect::<Vec<_>>()]),
            Atom::Has(p) => serde_json::json!(["has", p]),
            Atom::Is(p, s) => serde_json::json!(["is", p, set(s)]),
            Atom::AnyOf(p, s) => serde_json::json!(["any", p, set(s)]),
            Atom::NoneOf(p, s) => serde_json::json!(["none", p, set(s)]),
            Atom::Inp(p, s) => serde_json::json!(["inp", p, set(s)]),
            Atom::SentBack(None) => serde_json::json!(["sent_back"]),
            Atom::SentBack(Some(n)) => serde_json::json!(["sent_back", n]),
            Atom::Failed(k) => serde_json::json!(["failed", k]),
            Atom::Engine(k) => serde_json::json!(["engine", k]),
        }
    }
}
