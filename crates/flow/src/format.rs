//! The format of `flow.json`, the flow as people write it. Every part says what it is for; a key the format does
//! not know is refused, so a misspelt key is an error and not a part silently left out.
//!
//! A flow is a graph of cells. Each cell takes inputs, hands back an output whose shape the flow fixes, and has a
//! table of rows: the first row whose conditions hold on the output says where the work goes next, what the engine
//! does on the way and what it records. The compiler (`crate::compile`) expands what cells share and checks the whole.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    /// The graphs: for now the engineer's only.
    pub flows: Vec<Flow>,
}

/// The engine's part of the flow, by name. Each map takes a name to what it does.
#[derive(Debug, Clone, Deserialize, Serialize)]
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
    #[serde(rename = "type")]
    pub kind: Option<Kind>,
    /// What the agent is told the field is.
    pub desc: Option<String>,
    /// Whether a return may leave it out.
    pub optional: Option<bool>,
    /// An enum's values.
    pub values: Option<Values>,
    /// An object's fields.
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
    /// Parts of the graph: a row from one block into another goes to one of that block's entries.
    pub blocks: Vec<Block>,
    /// The lines: an agent's cell on a line resumes the session the line's earlier cells left.
    pub sessions: Sessions,
    /// The rules the engine gives the agents that name them, as written.
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
}

/// What the engine does with an agent's round that does not end in a return the cell takes: a return sent back, or
/// a run of the agent's command line that ended without one. Every agent's cell has this alike.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Rounds {
    pub desc: String,
    #[serde(rename = "sentBack")]
    pub sent_back: SentBack,
    pub failed: Failed,
    /// What every round records, besides the prompt and the inputs.
    pub record: Vec<String>,
    /// What an agent's command line starts with, apart from what the flow gives it.
    pub environment: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SentBack {
    pub desc: String,
    /// How many returns of one cell in a row may be sent back before the engine pauses the work.
    #[serde(rename = "pauseAfter")]
    pub pause_after: u32,
    pub pause: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Failed {
    pub desc: String,
    pub kinds: Vec<Failure>,
}

/// One kind of run that ended without a return.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Failure {
    pub name: String,
    /// What the command line reports when this happens.
    pub when: String,
    pub handle: Handle,
    /// For a failure retried: how many times the same round runs again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retries: Option<u32>,
    /// The cell the work is taken to once the engine gives up; none for a failure it waits out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub then: Option<String>,
    pub desc: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Handle {
    /// No new round starts for any work until the cause passes; then each work goes on from its cell.
    Wait,
    /// The same round runs again, then the work goes to `then`.
    Retry,
    /// The work goes to `then` at once.
    Ask,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Named {
    pub name: String,
    pub desc: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sessions {
    pub lines: Vec<Named>,
    /// How sessions are kept, for people.
    pub rules: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub name: String,
    pub desc: String,
    pub text: Vec<String>,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
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
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InField {
    pub name: String,
    pub desc: String,
}

/// One form of an agent's return: the cell's own answer, or one of the common fields.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Form {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub own: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    pub say: String,
    /// The lines whose cells outside the asking part have this form; with none, every agent's cell has it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines: Option<Vec<String>>,
    /// Fields of `every` a return of this form does not hold. It leaves them as an earlier return wrote them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub without: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gate {
    pub tools: Vec<String>,
    pub line: String,
    #[serde(default)]
    pub inputs: Vec<Input>,
    /// Said after the gate's question, with `{about}` for the kind to ask about.
    pub prompt: String,
    /// The row a gate's own answer takes.
    pub row: GateRow,
    /// For a gate whose question may not fit the work: it answers no or not applicable, with why.
    pub na: GateNa,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateRow {
    pub name: String,
    #[serde(default)]
    pub main: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateNa {
    pub row: GateRow,
    /// With `{about}`, and `{na}` for the gate's example of when it does not apply.
    pub prompt: String,
    pub out: RawField,
    pub always: Always,
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
    /// The values the engine knows that the cell's rows or prompt read: each a list of values with what each
    /// means, or the name of an enum shape.
    pub inenums: Option<BTreeMap<String, Values>>,
    /// The roles an agent's cell may ask; with none, every role but its own.
    pub asks: Option<Vec<String>>,
    /// A gate's kind of question to ask about, one of `askKinds`.
    pub about: Option<String>,
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
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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
    /// The row takes an interjection to the cell that receives it.
    pub interject: Option<bool>,
    /// The row is on the main path; a work off it is an anomaly to look at.
    pub main: Option<bool>,
    pub note: Option<String>,
}

/// An action on a row: a name, or a name done for a place of the output.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Do {
    Name(String),
    At(DoAt),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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

/// What the engine does every time a cell returns a place: shows it in a view, or carries out an action.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Always {
    pub at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(rename = "do", skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

/// A check the engine makes on a return, on a place of it or on the whole.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    pub check: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    /// The name the cell's rows, actions and checks read the input by, as `@<id>`.
    pub id: Option<String>,
    /// What the input is, as the agent is told.
    pub what: String,
    pub from: Vec<Src>,
    /// The input may be missing on some way to the cell.
    pub maybe: Option<bool>,
    /// A common input given only to the cells that have this form.
    pub with: Option<String>,
}

/// Where an input comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Value", into = "Value")]
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
/// `["inp", name, values]` (a value the engine knows or rows recorded is one of them).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Value", into = "Value")]
pub enum Atom {
    All,
    Own,
    Or(Vec<Atom>),
    Has(String),
    Is(String, BTreeSet<String>),
    AnyOf(String, BTreeSet<String>),
    NoneOf(String, BTreeSet<String>),
    Inp(String, BTreeSet<String>),
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
        }
    }
}
