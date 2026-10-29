//! The compiled flow: every graph with what its cells share expanded, each cell's return schema, the inputs and rules
//! each agent's cell is given, and the rows. The engine runs on this, and stores it in `flow_versions` with its
//! digest; it reads it back to take a work on.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use ts_rs::TS;

use crate::check::Analysis;
use crate::field::{Field, segment};
use crate::format::{
    Always, Atom, BlockerCell, Blockers, Check, Counter, Do, EngineNames, EnumText, Form, Handle, Kind, Named,
    OPEN_SESSION, PromptBy, ReadBy, Rounds, Rule, Src,
};
use crate::graph::{Cell, CellKind, Graph, ensure};

pub const COMPILED: &str = "wf-agents compiled 1";

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Compiled {
    pub format: String,
    pub engine: EngineNames,
    pub tools: Vec<ToolUse>,
    pub graphs: Vec<CompiledGraph>,
    /// The configuration settings the flow reads, for `config check`.
    pub settings: Vec<String>,
    /// How big each graph came out; not part of what the engine stores.
    #[serde(skip)]
    pub stats: Vec<Stats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ToolUse {
    pub name: String,
    pub runs: String,
}

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub graph: String,
    pub cells: usize,
    pub rows: usize,
    pub states: usize,
    pub places: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CompiledGraph {
    pub name: String,
    pub start: String,
    pub end: String,
    /// The end a work cancelled or taken over ends at, from wherever it stands; none in a graph without one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancel: Option<String>,
    /// The field an agent's return holds its one form in.
    pub within: String,
    /// The cells of the asking part, and the one that receives answers to the cells of its line.
    pub asking: Vec<String>,
    pub receive: Option<String>,
    pub counters: Vec<Counter>,
    pub rules: Vec<Rule>,
    pub interject: Option<CompiledInterject>,
    /// The kinds of question that only a project meeting a need can meet: an agent working on a project that does
    /// not meet it is not told of the kind.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ask_needs: Vec<AskNeed>,
    /// The fields of every agent's return that only a project meeting a need is asked for, each with the need: an
    /// agent working on a project that does not meet it is not asked for the field.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub every_needs: BTreeMap<String, String>,
    /// How every agent's return is written, which the forms every return shares say once.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub write: Vec<String>,
    /// Fields of agents' own answers that several cells hand in alike, which the forms every return shares give once
    /// and each cell's own format refers to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub once: Vec<String>,
    /// What the engine does with an agent's round that does not end in a return the cell takes; none in a flow
    /// version stored before the flow said it in this form, which runs by the section of the flow built into the
    /// engine.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "stored_rounds")]
    pub rounds: Option<Rounds>,
    /// The cells a stopped task stands at while its blocker is held, each with who holds it there; none in a graph whose
    /// works are no tasks, or in a flow version stored before blockers were.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blockers: Option<Blockers>,
    pub cells: Vec<CompiledCell>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CompiledInterject {
    pub name: String,
    pub desc: String,
    pub text: Vec<String>,
    pub from: Vec<Named>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CompiledInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// What the input is called, as the owner's page heads it; none in a flow version stored before every input had
    /// a name, where only an input every agent's cell has had one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// An input every agent's cell has: the agent knows it by its name. A cell's list of what its inputs are names
    /// it only; the forms every return shares say what it is.
    #[serde(default, skip_serializing_if = "is_false")]
    pub common: bool,
    /// For a table the cell answers row by row: the prefix the engine numbers its rows with, across all its sources.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numbered: Option<String>,
    pub what: String,
    pub from: Vec<Src>,
    pub maybe: bool,
}

/// A kind of question only a project meeting a need can meet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AskNeed {
    pub kind: String,
    /// One of `engine.needs`.
    pub need: String,
    /// The kind with what it means, as a return format's description of the field that names a kind says it: what
    /// is taken out of that description when the kind is left out.
    pub text: String,
}

/// A rule of the flow an agent's cell is given, by its name.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RuleUse {
    pub name: String,
    /// Given to every agent's cell of the cell's line, not named by the cell itself.
    #[serde(default)]
    pub shared: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CompiledRow {
    pub name: String,
    #[serde(rename = "if")]
    pub conditions: Vec<Atom>,
    pub to: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sets: BTreeMap<String, String>,
    #[serde(rename = "do", default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<Do>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counts: Option<String>,
    pub waits: bool,
    /// The row pauses the work and tells the owner; once the owner lets it go on, the row goes where it says.
    #[serde(default)]
    pub pauses: bool,
    pub interject: bool,
    pub main: bool,
    /// The row goes on without answering the cells that wait for an answer, which get it later another way.
    #[serde(default, skip_serializing_if = "is_false")]
    pub drops: bool,
    /// The row passes the work on to another cell that answers the cell that asked in place of the one it leaves: a
    /// blocker handed to another holder. The cells that wait for an answer stay as they are.
    #[serde(default, skip_serializing_if = "is_false")]
    pub passes: bool,
    /// A row every agent's cell has that its agent chooses: to ask, run tools, search, pause, answer an interjection
    /// or take it to choose the way on; or that the engine takes for the cell's line, as a new session's.
    pub common: bool,
    /// A row of the engine's fallback, the same for every agent's cell: what the engine does with a round that ends
    /// in no return the cell takes. No return takes it; the engine does what it says itself.
    #[serde(default, skip_serializing_if = "is_false")]
    pub fallback: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CompiledCell {
    pub id: String,
    /// engine, agent, tool or owner.
    pub kind: String,
    /// Written as a gate.
    pub gate: bool,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
    /// A row leads here with the action `open_session`: the cell starts a new session of its line at its first round in
    /// a work.
    #[serde(default, skip_serializing_if = "is_false")]
    pub new_session: bool,
    /// The reasoning effort an agent's cell runs at, whatever the model; none in a flow version stored before cells
    /// declared one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
    pub inputs: Vec<CompiledInput>,
    /// The flow's rules an agent's cell is given, besides the project's rules, which every agent is given.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<RuleUse>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prompt: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_by: Option<PromptBy>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forms: Vec<Form>,
    /// What the cell's return must be, as JSON Schema; for an agent, its round's prompt gives it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<Value>,
    pub rows: Vec<CompiledRow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub always: Vec<Always>,
    /// What an engine's cell does besides its step and its rows, in order: at an end, how the work ends.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,
    /// A gate's two outcomes, by name: the one it hands in to go on, whose row is named after it, and the problem,
    /// which it asks about.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outcomes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<Check>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub handoff: Vec<String>,
    /// Paths of the return the owner's page shows on the item the work is on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shown: Vec<String>,
    /// Places of the return a role reads outside this flow, through the database.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read_by: Vec<ReadBy>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inenums: BTreeMap<String, Vec<(String, String)>>,
    /// Each field of the return by its path (`understanding.grounds[verified].claim`), with the title the owner's
    /// page heads it with. No agent is sent these.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub titles: BTreeMap<String, String>,
}

/// Whether a flag is off, which the compiled flow leaves out.
fn is_false(b: &bool) -> bool {
    !*b
}

impl Compiled {
    /// The JSON the engine stores, and its digest.
    pub fn json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn digest(&self) -> String {
        Sha256::digest(self.json().as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    pub fn graph(&self, name: &str) -> Option<&CompiledGraph> {
        self.graphs.iter().find(|g| g.name == name)
    }
}

impl CompiledGraph {
    pub fn cell(&self, id: &str) -> Option<&CompiledCell> {
        self.cells.iter().find(|c| c.id == id)
    }

    /// The cell a stopped task stands at while its blocker is held there, with who may hold it; none for any other
    /// cell.
    pub fn holder_cell(&self, id: &str) -> Option<&BlockerCell> {
        self.blockers.as_ref()?.cells.iter().find(|c| c.cell == id)
    }

    /// The graph as a project's agents are asked it: each field of every return whose need the project does not meet
    /// (`meets` says which it meets) taken out of every agent's cell's schema, and out of what the cell does every
    /// time, shows and has read outside the flow at it, so that no agent is asked for it and nothing reads it.
    pub fn as_asked(mut self, meets: impl Fn(&str) -> bool) -> CompiledGraph {
        let gone: Vec<String> = self
            .every_needs
            .iter()
            .filter(|(_, need)| !meets(need.as_str()))
            .map(|(field, _)| field.clone())
            .collect();
        if gone.is_empty() {
            return self;
        }
        let at = |p: &str| gone.iter().any(|f| p == f || p.starts_with(&format!("{f}.")));
        for c in self.cells.iter_mut().filter(|c| c.kind == CellKind::Agent.name()) {
            if let Some(Value::Object(s)) = &mut c.schema {
                if let Some(Value::Object(props)) = s.get_mut("properties") {
                    props.retain(|k, _| !gone.contains(k));
                }
                if let Some(Value::Array(required)) = s.get_mut("required") {
                    required.retain(|r| !r.as_str().is_some_and(|r| gone.iter().any(|f| f == r)));
                }
            }
            c.always.retain(|a| !at(&a.at));
            c.shown.retain(|p| !at(p));
            c.read_by.retain(|x| !at(&x.at));
        }
        self.every_needs.retain(|field, _| !gone.contains(field));
        self
    }

    /// The row a return takes: the return checked against the cell's schema, an agent's form taken out of the
    /// field it sits in, and the first row whose conditions hold. `given` holds the values the engine knows and
    /// rows recorded, and each input read by id under `@<id>`.
    pub fn take(&self, cell: &str, ret: &Value, given: &Map<String, Value>) -> Result<&CompiledRow, String> {
        let c = self.cell(cell).ok_or_else(|| format!("no such cell: {cell}"))?;
        if let Some(s) = &c.schema
            && let Some(why) = misfit(s, ret, "")
        {
            return Err(format!("the return does not fit the schema of {cell}: {why}"));
        }
        let x = if c.kind == CellKind::Agent.name() {
            self.unwrap(ret)
        } else {
            ret.clone()
        };
        let forms: Vec<&str> = c.forms.iter().filter_map(|f| f.field.as_deref()).collect();
        c.rows
            .iter()
            .find(|r| r.conditions.iter().all(|a| holds(a, &x, given, &forms)))
            .ok_or_else(|| format!("no row of {cell} takes the return"))
    }

    /// What the rows read of an agent's return: the form taken out of its field, beside the fields of every return.
    pub fn unwrap(&self, x: &Value) -> Value {
        let Some(o) = x.as_object() else { return x.clone() };
        let mut out = o
            .get(&self.within)
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for (k, v) in o {
            if *k != self.within {
                out.insert(k.clone(), v.clone());
            }
        }
        Value::Object(out)
    }
}

/// The values at a path of a return; a branch keeps only the items of that branch.
pub fn found_at(x: &Value, path: &str) -> Vec<Value> {
    let flat = |v: &Value| -> Vec<Value> {
        match v {
            Value::Array(xs) => xs.clone(),
            _ => vec![v.clone()],
        }
    };
    let mut cur = vec![x.clone()];
    for seg in path.split('.') {
        let Some((name, b)) = segment(seg) else {
            return Vec::new();
        };
        let mut next = Vec::new();
        for c in &cur {
            for it in flat(c) {
                if let Some(v) = it.get(name) {
                    next.push(v.clone());
                }
            }
        }
        if let Some(b) = b {
            let want = Value::String(b.to_string());
            next = next
                .iter()
                .flat_map(flat)
                .filter(|it| it.as_object().is_some_and(|o| o.values().any(|v| *v == want)))
                .collect();
        }
        cur = next;
    }
    cur.iter().flat_map(flat).collect()
}

fn holds(a: &Atom, x: &Value, given: &Map<String, Value>, forms: &[&str]) -> bool {
    let values = |p: &str| -> Vec<Value> {
        if !p.starts_with('@') {
            return found_at(x, p);
        }
        // What the cell read by id: given whole under its path, or as the input it is a path into.
        if let Some(v) = given.get(p) {
            return match v {
                Value::Array(xs) => xs.clone(),
                _ => vec![v.clone()],
            };
        }
        let (id, rest) = crate::field::by_id(p).unwrap_or(("", ""));
        match (given.get(&format!("@{id}")), rest.strip_prefix('.')) {
            (Some(v), Some(rest)) => found_at(v, rest),
            (Some(v), None) if rest.is_empty() => found_at(&json!({ "x": v }), "x"),
            _ => Vec::new(),
        }
    };
    let hit = |p: &str, vs: &std::collections::BTreeSet<String>| {
        values(p).iter().any(|v| v.as_str().is_some_and(|s| vs.contains(s)))
    };
    match a {
        Atom::All => true,
        Atom::Own => !forms.iter().any(|f| x.get(*f).is_some()),
        Atom::Or(xs) => xs.iter().any(|y| holds(y, x, given, forms)),
        Atom::Inp(n, vs) => given.get(n).and_then(Value::as_str).is_some_and(|s| vs.contains(s)),
        Atom::Has(p) => !values(p).is_empty(),
        Atom::Is(p, vs) | Atom::AnyOf(p, vs) => hit(p, vs),
        Atom::NoneOf(p, vs) => !hit(p, vs),
        Atom::SentBack(_) | Atom::Failed(_) | Atom::Engine(_) => false,
    }
}

/// Where `x` does not fit `s`, and why, in words that say how to put it right; none when it fits. Of alternatives,
/// the one the value picks by a field whose fixed value tells them apart, or else the only one that has every field
/// the value has, is followed.
pub fn misfit(s: &Value, x: &Value, at: &str) -> Option<String> {
    if valid(s, x) {
        return None;
    }
    let o = s.as_object()?;
    let here = if at.is_empty() { "the return" } else { at };
    if let Some(c) = o.get("const") {
        return Some(format!("{here} is {x}, and must be {c}"));
    }
    if let Some(Value::Array(e)) = o.get("enum") {
        return Some(format!("{here} is {x}, and must be one of {}", joined(e.iter())));
    }
    match (o.get("oneOf"), o.get("anyOf")) {
        (Some(Value::Array(bs)), _) => return Some(alternatives(bs, x, at, here)),
        (_, Some(Value::Array(bs))) if !o.contains_key("type") => return Some(alternatives(bs, x, at, here)),
        _ => {}
    }
    let kind = kind_of(x);
    match o.get("type").and_then(Value::as_str).unwrap_or_default() {
        "array" => {
            let Some(xs) = x.as_array() else {
                return Some(format!("{here} must be a list, and is {kind}"));
            };
            let min = o.get("minItems").and_then(Value::as_u64).unwrap_or(0);
            if (xs.len() as u64) < min {
                return Some(format!("{here} must hold at least {min}, and holds {}", xs.len()));
            }
            let it = o.get("items")?;
            xs.iter()
                .enumerate()
                .find_map(|(i, v)| misfit(it, v, &format!("{at}[{i}]")))
        }
        "object" => {
            let Some(xo) = x.as_object() else {
                return Some(format!("{here} must be an object, and is {kind}"));
            };
            let empty = Map::new();
            let props = o.get("properties").and_then(Value::as_object).unwrap_or(&empty);
            let required = o.get("required").and_then(Value::as_array).cloned().unwrap_or_default();
            if let Some(r) = required.iter().filter_map(Value::as_str).find(|r| !xo.contains_key(*r)) {
                return Some(format!("{here} has no {r}"));
            }
            if o.get("additionalProperties") == Some(&Value::Bool(false))
                && let Some(k) = xo.keys().find(|k| !props.contains_key(*k))
            {
                return Some(format!(
                    "{here} has {k}, which is not one of its fields: {}",
                    props.keys().cloned().collect::<Vec<_>>().join(", ")
                ));
            }
            if let Some(m) = xo
                .iter()
                .find_map(|(k, v)| props.get(k).and_then(|p| misfit(p, v, &field(at, k))))
            {
                return Some(m);
            }
            match o.get("anyOf") {
                Some(Value::Array(bs)) => Some(alternatives(bs, x, at, here)),
                _ => Some(format!("{here} does not fit")),
            }
        }
        t => Some(format!("{here} must be {}, and is {kind}", a_kind(t))),
    }
}

fn alternatives(bs: &[Value], x: &Value, at: &str, here: &str) -> String {
    if bs.iter().filter(|b| valid(b, x)).count() > 1 {
        return format!("{here} fits more than one of its alternatives");
    }
    let Some(xo) = x.as_object() else {
        if bs.iter().all(|b| b["type"] == "object") {
            return format!("{here} must be an object, and is {}", kind_of(x));
        }
        return format!("{here} is {}, and fits none of its alternatives", kind_of(x));
    };
    // A field every alternative fixes, which tells them apart.
    for (k, v) in xo {
        let fixed: Vec<&Value> = bs
            .iter()
            .filter_map(|b| b.get("properties").and_then(|p| p.get(k)).and_then(|p| p.get("const")))
            .collect();
        if fixed.len() != bs.len() {
            continue;
        }
        let picked: Vec<&Value> = bs.iter().filter(|b| b["properties"][k]["const"] == *v).collect();
        match picked.as_slice() {
            [] => {
                return format!(
                    "{} is {v}, and must be one of {}",
                    field(at, k),
                    joined(fixed.into_iter())
                );
            }
            [b] => return misfit(b, x, at).unwrap_or_else(|| format!("{here} does not fit")),
            _ => {}
        }
    }
    let has_all = |b: &&Value| {
        b.get("properties")
            .and_then(Value::as_object)
            .is_some_and(|p| !xo.is_empty() && xo.keys().all(|k| p.contains_key(k)))
    };
    let named: Vec<&Value> = bs.iter().filter(has_all).collect();
    if let [b] = named.as_slice()
        && let Some(m) = misfit(b, x, at)
    {
        return m;
    }
    let shapes: Vec<String> = bs
        .iter()
        .map(|b| match b.get("properties").and_then(Value::as_object) {
            Some(p) if !p.is_empty() => p.keys().cloned().collect::<Vec<_>>().join(", "),
            _ => "nothing".to_string(),
        })
        .collect();
    format!(
        "{here} holds {}, and fits none of its alternatives, which hold: {}",
        xo.keys().cloned().collect::<Vec<_>>().join(", "),
        shapes.join("; or ")
    )
}

fn field(at: &str, k: &str) -> String {
    if at.is_empty() {
        k.to_string()
    } else {
        format!("{at}.{k}")
    }
}

fn joined<'a>(vs: impl Iterator<Item = &'a Value>) -> String {
    vs.map(Value::to_string).collect::<Vec<_>>().join(", ")
}

fn kind_of(x: &Value) -> &'static str {
    match x {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "a list",
        Value::Object(_) => "an object",
    }
}

fn a_kind(t: &str) -> String {
    match t {
        "string" => "a string".into(),
        "integer" => "an integer".into(),
        "boolean" => "a boolean".into(),
        other => format!("of type {other}"),
    }
}

/// A value with each field `s` lets it leave out that it holds as null left out, at every depth, as an agent may write
/// null for a field it has nothing for. Of alternatives, each the value may be is followed: one whose fixed values the
/// value's agree with, and whose fields hold every field the value has.
pub fn without_null_options(s: &Value, x: &mut Value) {
    let Some(o) = s.as_object() else { return };
    for key in ["oneOf", "anyOf"] {
        for b in o.get(key).and_then(Value::as_array).into_iter().flatten() {
            if may_be(b, x) {
                without_null_options(b, x);
            }
        }
    }
    match x {
        Value::Array(xs) => {
            if let Some(it) = o.get("items") {
                for v in xs {
                    without_null_options(it, v);
                }
            }
        }
        Value::Object(xo) => {
            let Some(props) = o.get("properties").and_then(Value::as_object) else {
                return;
            };
            let required = |k: &str| {
                o.get("required")
                    .and_then(Value::as_array)
                    .is_some_and(|r| r.iter().any(|x| x == k))
            };
            xo.retain(|k, v| !(v.is_null() && props.contains_key(k) && !required(k)));
            for (k, v) in xo.iter_mut() {
                if let Some(p) = props.get(k) {
                    without_null_options(p, v);
                }
            }
        }
        _ => {}
    }
}

/// Whether a value may be of an alternative: its fixed values agree, and every field the value holds, other than a
/// null, is one of the alternative's.
fn may_be(b: &Value, x: &Value) -> bool {
    let (Some(props), Some(xo)) = (b.get("properties").and_then(Value::as_object), x.as_object()) else {
        return false;
    };
    let consts = props
        .iter()
        .all(|(k, p)| p.get("const").is_none_or(|c| xo.get(k).is_none_or(|v| v == c)));
    consts && xo.iter().all(|(k, v)| v.is_null() || props.contains_key(k))
}

/// Whether `x` fits `s`, for the part of JSON Schema the compiler writes.
pub fn valid(s: &Value, x: &Value) -> bool {
    let Some(o) = s.as_object() else { return true };
    if o.is_empty() {
        return true;
    }
    if let Some(c) = o.get("const") {
        return x == c;
    }
    if let Some(Value::Array(e)) = o.get("enum") {
        return e.contains(x);
    }
    if let Some(Value::Array(bs)) = o.get("oneOf") {
        return bs.iter().filter(|b| valid(b, x)).count() == 1;
    }
    if let Some(Value::Array(alts)) = o.get("anyOf")
        && !o.contains_key("type")
    {
        return alts.iter().any(|b| valid(b, x));
    }
    match o.get("type").and_then(Value::as_str) {
        Some("string") => x.is_string(),
        Some("integer") => x.is_i64() || x.is_u64(),
        Some("boolean") => x.is_boolean(),
        Some("array") => {
            let Some(xs) = x.as_array() else { return false };
            let min = o.get("minItems").and_then(Value::as_u64).unwrap_or(0);
            xs.len() as u64 >= min && o.get("items").is_none_or(|it| xs.iter().all(|v| valid(it, v)))
        }
        Some("object") => {
            let Some(xo) = x.as_object() else { return false };
            let required = o.get("required").and_then(Value::as_array).cloned().unwrap_or_default();
            if !required.iter().filter_map(Value::as_str).all(|r| xo.contains_key(r)) {
                return false;
            }
            let props = o
                .get("properties")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            if o.get("additionalProperties") == Some(&Value::Bool(false)) && !xo.keys().all(|k| props.contains_key(k)) {
                return false;
            }
            if !xo.iter().all(|(k, v)| props.get(k).is_none_or(|p| valid(p, v))) {
                return false;
            }
            match o.get("anyOf").and_then(Value::as_array) {
                Some(alts) => alts.iter().any(|b| valid(b, x)),
                None => true,
            }
        }
        _ => false,
    }
}

struct Schemas<'a> {
    text: &'a EnumText,
}

impl Schemas<'_> {
    fn field(&self, f: &Field) -> Value {
        let mut s = match f.kind {
            Kind::Text => json!({ "type": "string" }),
            Kind::Int => json!({ "type": "integer" }),
            Kind::Bool => json!({ "type": "boolean" }),
            Kind::Enum => json!({ "enum": f.values.iter().map(|x| x.0.clone()).collect::<Vec<_>>() }),
            Kind::Obj if f.open => json!({ "type": "object" }),
            Kind::Obj => self.object(Vec::new(), &f.fields),
            Kind::List => {
                let item = f.item.as_deref().map(|i| self.field(i)).unwrap_or_default();
                let mut s = json!({ "type": "array", "items": item });
                if f.min > 0 {
                    s["minItems"] = json!(f.min);
                }
                s
            }
            Kind::Union => {
                let alts: Vec<Value> = f
                    .branches
                    .iter()
                    .map(|b| {
                        let mut marker = json!({ "const": b.value });
                        if !b.desc.is_empty() {
                            marker["description"] = json!(b.desc);
                        }
                        let fs: Vec<Field> = f.common.iter().chain(&b.fields).cloned().collect();
                        self.object(vec![(f.by.clone(), marker)], &fs)
                    })
                    .collect();
                json!({ "oneOf": alts })
            }
        };
        let mut d = f.desc.clone();
        if f.kind == Kind::Enum && f.values.iter().any(|x| !x.1.is_empty()) {
            // The values' meanings go into the field's description, joined as the flow says.
            let vs: Vec<String> = f
                .values
                .iter()
                .map(|(v, m)| {
                    if m.is_empty() {
                        v.clone()
                    } else {
                        self.text.value.replace("{value}", v).replace("{means}", m)
                    }
                })
                .collect();
            let vs = vs.join(&self.text.value_join);
            d = if d.is_empty() {
                self.text.alone.replace("{values}", &vs)
            } else {
                self.text.with_desc.replace("{desc}", &d).replace("{values}", &vs)
            };
        }
        if !d.is_empty() {
            s["description"] = json!(d);
        }
        s
    }

    /// An object of the fields, after the leading properties, which are required.
    fn object(&self, lead: Vec<(String, Value)>, fs: &[Field]) -> Value {
        let mut props = Map::new();
        let mut required = Vec::new();
        for (k, v) in lead {
            required.push(json!(k));
            props.insert(k, v);
        }
        for f in fs {
            props.insert(f.name.clone(), self.field(f));
            if !f.optional {
                required.push(json!(f.name));
            }
        }
        json!({ "type": "object", "properties": props, "required": required, "additionalProperties": false })
    }

    /// An agent returns exactly one of its forms, inside the field the flow names, with the fields that come every
    /// time. The forms are an anyOf one level down, in that field. A field a form leaves out is not required; the
    /// engine checks it is there for the other forms.
    fn cell(&self, g: &Graph, c: &Cell) -> Option<Value> {
        if c.out.is_empty() {
            return None;
        }
        if c.kind != CellKind::Agent {
            return Some(self.object(Vec::new(), &c.out));
        }
        let com = |n: &str| c.out.iter().find(|f| f.name == n).cloned();
        let ret = &g.flow.agent_common.returns;
        let mut alts = Vec::new();
        for form in &c.forms {
            let fs: Vec<Field> = if form.own {
                c.main.clone()
            } else {
                com(form.field.as_deref().unwrap_or_default()).into_iter().collect()
            };
            let mut a = self.object(Vec::new(), &fs);
            if let Some(name) = &form.field {
                a["required"] = json!([name]);
            }
            alts.push(a);
        }
        // The short fields every return holds come first and the field its forms are under last, so that the model
        // writes them before its long answer, after which only closing brackets are left to write.
        let every: Vec<Field> = g.every.iter().filter_map(|n| com(n)).collect();
        let mut s = self.object(Vec::new(), &every);
        if let Some(props) = s["properties"].as_object_mut() {
            props.insert(
                ret.within.name.clone(),
                json!({ "description": ret.within.desc, "anyOf": alts }),
            );
        }
        let left_out = |n: &String| c.forms.iter().any(|f| f.without.contains(n));
        let required: Vec<Value> = g
            .every
            .iter()
            .filter(|n| !left_out(n))
            .cloned()
            .chain(std::iter::once(ret.within.name.clone()))
            .map(Value::String)
            .collect();
        s["required"] = Value::Array(required);
        s["description"] = json!(c.say);
        Some(s)
    }
}

/// An agent reads every description: each of its fields, values and branches has one.
fn described(cell: &str, f: &Field, path: &str) -> Result<(), String> {
    if !f.name.is_empty() || matches!(f.kind, Kind::Text | Kind::Int | Kind::Bool | Kind::Enum) {
        ensure!(
            !f.desc.is_empty(),
            "an agent's field with no description: {cell} {path}"
        );
    }
    for (v, d) in &f.values {
        ensure!(
            !d.is_empty(),
            "an agent's enum value with no description: {cell} {path} {v}"
        );
    }
    for x in &f.fields {
        described(cell, x, &format!("{path}.{}", x.name))?;
    }
    if let Some(i) = &f.item {
        described(cell, i, &format!("{path}[]"))?;
    }
    for x in &f.common {
        described(cell, x, &format!("{path}.{}", x.name))?;
    }
    for b in &f.branches {
        ensure!(
            !b.desc.is_empty(),
            "an agent's branch with no description: {cell} {path} {}",
            b.value
        );
        for x in &b.fields {
            described(cell, x, &format!("{path}[{}].{}", b.value, x.name))?;
        }
    }
    Ok(())
}

/// Each field's title by its path, as the owner's page finds a field (`understanding.grounds[verified].claim`): a
/// list's item is followed, the fields every branch of a union has are under the union, and a branch's own under the
/// union with the branch's value in brackets. The owner's page heads every field with its title, so every field with
/// a name has one.
fn titled(cell: &str, fs: &[Field], prefix: &str, out: &mut BTreeMap<String, String>) -> Result<(), String> {
    for f in fs {
        let path = if prefix.is_empty() {
            f.name.clone()
        } else {
            format!("{prefix}.{}", f.name)
        };
        ensure!(
            !f.title.trim().is_empty(),
            "a return field with no title: {cell} {path}"
        );
        out.insert(path.clone(), f.title.clone());
        titled_under(cell, f, &path, out)?;
    }
    Ok(())
}

/// The titles of what a field holds, under its path.
fn titled_under(cell: &str, f: &Field, path: &str, out: &mut BTreeMap<String, String>) -> Result<(), String> {
    titled(cell, &f.fields, path, out)?;
    if let Some(i) = &f.item {
        titled_under(cell, i, path, out)?;
    }
    titled(cell, &f.common, path, out)?;
    for b in &f.branches {
        titled(cell, &b.fields, &format!("{path}[{}]", b.value), out)?;
    }
    Ok(())
}

/// The schema of an agent's cell's own answer, the form of its return that holds no field of the forms every return
/// shares; none for a cell that is no agent's.
pub fn own_answer<'c>(cell: &'c CompiledCell, within: &str) -> Option<&'c Value> {
    let alts = cell
        .schema
        .as_ref()?
        .get("properties")?
        .get(within)?
        .get("anyOf")?
        .as_array()?;
    cell.forms
        .iter()
        .zip(alts)
        .find(|(f, _)| f.field.is_none())
        .map(|(_, a)| a)
}

pub(crate) fn graph(g: &Graph, a: &Analysis) -> Result<(CompiledGraph, Stats), String> {
    let schemas = Schemas {
        text: &g.source.enum_text,
    };
    let mut cells = Vec::new();
    for (ci, c) in g.cells.iter().enumerate() {
        if c.kind == CellKind::Agent {
            for f in &c.out {
                described(&c.id, f, &f.name)?;
            }
        }
        let mut titles = BTreeMap::new();
        titled(&c.id, &c.out, "", &mut titles)?;
        cells.push(CompiledCell {
            id: c.id.clone(),
            kind: c.kind.name().to_string(),
            gate: c.gate,
            title: c.title.clone(),
            step: c.step.clone(),
            needs: c.needs.clone(),
            line: c.line.clone(),
            new_session: c.line.is_some()
                && g.cells.iter().any(|m| {
                    m.rows
                        .iter()
                        .any(|r| r.to == c.id && r.actions.iter().any(|d| d.name() == OPEN_SESSION))
                }),
            effort: c.effort.clone(),
            stage: g.stage.get(&ci).cloned(),
            tools: c.tools.clone(),
            inputs: c
                .inputs
                .iter()
                .map(|x| CompiledInput {
                    id: x.id.clone(),
                    name: x.name.clone(),
                    common: x.common,
                    numbered: x.numbered.clone(),
                    what: x.what.clone(),
                    from: x.from.clone(),
                    maybe: x.maybe,
                })
                .collect(),
            rules: c
                .rules
                .iter()
                .map(|r| RuleUse {
                    name: r.clone(),
                    shared: g
                        .rules
                        .iter()
                        .any(|x| &x.name == r && c.line.as_ref().is_some_and(|l| x.lines.contains(l))),
                })
                .collect(),
            prompt: c.prompt.clone(),
            prompt_by: c.prompt_by.clone(),
            forms: c.forms.clone(),
            schema: schemas.cell(g, c),
            rows: c
                .rows
                .iter()
                .map(|r| CompiledRow {
                    name: r.name.clone(),
                    conditions: r.atoms.clone(),
                    to: r.to.clone(),
                    sets: r.sets.clone(),
                    actions: r.actions.clone(),
                    counts: r.counts.clone(),
                    waits: r.waits,
                    pauses: false,
                    interject: r.interject,
                    main: r.main,
                    drops: r.drops,
                    passes: r.passes,
                    common: r.common,
                    fallback: false,
                    note: r.note.clone(),
                })
                .chain(anew_row(g, c))
                .chain(match (&g.flow.rounds, c.kind) {
                    (Some(r), CellKind::Agent) => no_return_rows(r),
                    _ => Vec::new(),
                })
                .collect(),
            always: c.always.clone(),
            actions: c.actions.clone(),
            outcomes: c.outcomes.clone(),
            checks: c.checks.clone(),
            handoff: c.handoff.clone(),
            shown: c.shown.clone(),
            read_by: c.read_by.clone(),
            inenums: c.inenums.clone(),
            titles,
        });
    }
    let f = g.flow;
    // A field given once is one that several cells' own answers hold alike.
    let within = &f.agent_common.returns.within.name;
    for name in &f.agent_common.returns.once {
        let held: Vec<&Value> = cells
            .iter()
            .filter_map(|c| own_answer(c, within)?.get("properties")?.get(name))
            .collect();
        ensure!(
            held.len() > 1,
            "a field given once is in fewer than two cells' own answers: {name}"
        );
        ensure!(
            held.iter().all(|x| *x == held[0]),
            "a field given once is not alike in every cell's own answer: {name}"
        );
    }
    let stats = Stats {
        graph: f.name.clone(),
        cells: g.cells.len(),
        rows: g.cells.iter().map(|c| c.rows.len()).sum(),
        states: a.states.len(),
        places: a.places.iter().map(Vec::len).sum(),
    };
    let out = CompiledGraph {
        name: f.name.clone(),
        start: f.start.clone(),
        end: f.end.clone(),
        cancel: f.cancel.clone(),
        within: f.agent_common.returns.within.name.clone(),
        asking: f.asking.nodes.clone(),
        receive: f.asking.receive.clone(),
        counters: f.limits.counters.clone(),
        rules: g.rules.clone(),
        interject: f.interject.as_ref().map(|i| CompiledInterject {
            name: i.name.clone(),
            desc: i.desc.clone(),
            text: i.text.clone(),
            from: i.from.clone(),
        }),
        ask_needs: f
            .ask_kinds
            .iter()
            .flat_map(|grp| {
                grp.kinds.iter().filter_map(|(kind, means)| {
                    grp.needs.get(kind).map(|need| AskNeed {
                        kind: kind.clone(),
                        need: need.clone(),
                        text: if means.is_empty() {
                            kind.clone()
                        } else {
                            g.source
                                .enum_text
                                .value
                                .replace("{value}", kind)
                                .replace("{means}", means)
                        },
                    })
                })
            })
            .collect(),
        every_needs: f.agent_common.needs.clone(),
        write: f.agent_common.returns.write.clone(),
        once: f.agent_common.returns.once.clone(),
        rounds: match &f.rounds {
            Some(r) => Some(rounds(r, &cells)?),
            None => None,
        },
        blockers: f.blockers.clone(),
        cells,
    };
    Ok((out, stats))
}

/// The rounds section as written, once every agent's cell can have a shared row of each kind's name.
fn rounds(r: &Rounds, cells: &[CompiledCell]) -> Result<Rounds, String> {
    ensure!(r.sent_back.pause_after > 1, "rounds.sentBack.pauseAfter is less than 2");
    for c in cells {
        let mut seen = std::collections::BTreeSet::new();
        for row in &c.rows {
            ensure!(
                seen.insert(&row.name),
                "a row of cell {} has the name of a shared row the rounds section gives: {}",
                c.id,
                row.name
            );
        }
    }
    Ok(r.clone())
}

/// The row a cell of a line with a new session's row has: the engine takes it in place of the cell's round when the
/// line's session would start afresh there; none for any other cell.
fn anew_row(g: &Graph, c: &Cell) -> Option<CompiledRow> {
    let line = c.line.as_ref()?;
    let a = g.flow.sessions.lines.iter().find(|l| &l.name == line)?.anew.as_ref()?;
    if c.kind != CellKind::Agent || a.not.contains(&c.id) {
        return None;
    }
    Some(CompiledRow {
        name: a.row.clone(),
        conditions: vec![Atom::Engine(a.row.clone())],
        to: a.to.clone(),
        sets: BTreeMap::new(),
        actions: Vec::new(),
        counts: None,
        waits: false,
        pauses: false,
        interject: false,
        main: false,
        drops: true,
        passes: false,
        common: true,
        fallback: false,
        note: Some(a.note.clone()),
    })
}

/// The engine's fallback, the shared rows every agent's cell has for a round that ends in no return it takes, as the
/// rounds section says: a return sent back runs the cell again, and so many in a row pause the work first; a failed
/// run of each kind runs it again once the cause passes, or after a wait, or once the owner lets the paused work go
/// on; and each of the engine's own cases does as it says.
fn no_return_rows(r: &Rounds) -> Vec<CompiledRow> {
    let row = |name: &str, atom: Atom, note: &str| CompiledRow {
        name: name.to_string(),
        conditions: vec![atom],
        to: "@self".to_string(),
        sets: BTreeMap::new(),
        actions: Vec::new(),
        counts: None,
        waits: false,
        pauses: false,
        interject: false,
        main: false,
        drops: false,
        passes: false,
        common: false,
        fallback: true,
        note: Some(note.to_string()),
    };
    let mut out = vec![
        CompiledRow {
            pauses: true,
            ..row(
                "sent_back_too_often",
                Atom::SentBack(Some(r.sent_back.pause_after)),
                &r.sent_back.pause,
            )
        },
        row("sent_back", Atom::SentBack(None), &r.sent_back.desc),
    ];
    for k in &r.failed.kinds {
        out.push(CompiledRow {
            waits: k.handle == Handle::Wait,
            pauses: matches!(k.handle, Handle::Pause | Handle::Repair),
            ..row(&k.name, Atom::Failed(k.name.clone()), &format!("{} {}", k.when, k.desc))
        });
    }
    for k in &r.engine {
        out.push(CompiledRow {
            pauses: k.pauses,
            ..row(&k.name, Atom::Engine(k.name.clone()), &format!("{} {}", k.when, k.desc))
        });
    }
    out
}

/// The rounds section of a stored flow version; none when it was stored in a form this engine no longer reads, so
/// that the version stays readable and runs by the section of the flow built into the engine.
fn stored_rounds<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Rounds>, D::Error> {
    let v = Option::<Value>::deserialize(d)?;
    Ok(v.and_then(|v| serde_json::from_value(v).ok()))
}
