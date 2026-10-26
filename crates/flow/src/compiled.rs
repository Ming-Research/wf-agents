//! The compiled flow: every graph with what its cells share expanded, each cell's return schema, how each agent
//! gets each input and rule, and the rows. The engine runs on this, and stores it in `flow_versions` with its
//! digest; it reads it back to take a work on.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::check::Analysis;
use crate::field::{Field, segment};
use crate::format::{
    Always, Atom, Check, Counter, Do, EngineNames, EnumText, Form, Handle, Kind, Named, PromptBy, Rounds, Rule, Src,
};
use crate::graph::{Cell, CellKind, Graph, ensure};

pub const COMPILED: &str = "wf-agents compiled 1";

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledGraph {
    pub name: String,
    pub start: String,
    pub end: String,
    /// The field an agent's return holds its one form in.
    pub within: String,
    /// The cells of the asking part, and the one that receives answers to the cells of its line.
    pub asking: Vec<String>,
    pub receive: Option<String>,
    pub counters: Vec<Counter>,
    pub rules: Vec<Rule>,
    pub interject: Option<CompiledInterject>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rounds: Option<Rounds>,
    pub cells: Vec<CompiledCell>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledInterject {
    pub name: String,
    pub desc: String,
    pub text: Vec<String>,
    pub from: Vec<Named>,
}

/// How an agent's cell gets an input or a rule: its line's session already holds it on every way to the cell, or
/// the engine gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Held {
    Session,
    Given,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub what: String,
    pub from: Vec<Src>,
    pub maybe: bool,
    /// For an agent's cell; none for one it reads itself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub held: Option<Held>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleUse {
    pub name: String,
    pub held: Held,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledRow {
    pub name: String,
    #[serde(rename = "if")]
    pub conditions: Vec<Atom>,
    pub to: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sets: BTreeMap<String, String>,
    #[serde(rename = "do", default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<Do>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counts: Option<String>,
    pub waits: bool,
    pub interject: bool,
    pub main: bool,
    /// A row every agent's cell has.
    pub common: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledCell {
    pub id: String,
    /// engine, agent, tool or owner.
    pub kind: String,
    /// Written as a gate.
    pub gate: bool,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub needs: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
    pub inputs: Vec<CompiledInput>,
    /// For an agent's cell: how it gets the project's rules, which every agent is given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_rules: Option<Held>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<RuleUse>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prompt: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_by: Option<PromptBy>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forms: Vec<Form>,
    /// What the cell's return must be, as JSON Schema; for an agent, its round's prompt gives it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<Value>,
    pub rows: Vec<CompiledRow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub always: Vec<Always>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<Check>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub handoff: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inenums: BTreeMap<String, Vec<(String, String)>>,
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
        let every: Vec<Field> = g.every.iter().filter_map(|n| com(n)).collect();
        let mut s = self.object(
            vec![(
                ret.within.name.clone(),
                json!({ "description": ret.within.desc, "anyOf": alts }),
            )],
            &every,
        );
        let left_out = |n: &String| c.forms.iter().any(|f| f.without.contains(n));
        let required: Vec<Value> = std::iter::once(ret.within.name.clone())
            .chain(g.every.iter().filter(|n| !left_out(n)).cloned())
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

pub(crate) fn graph(g: &Graph, a: &Analysis) -> Result<(CompiledGraph, Stats), String> {
    let schemas = Schemas {
        text: &g.source.enum_text,
    };
    let held = |c: &Cell, i: usize| -> Option<Held> {
        if c.kind != CellKind::Agent {
            return None;
        }
        let x = &c.inputs[i];
        if x.from.iter().all(|s| *s == Src::Itself) {
            return None;
        }
        let places = x.from.iter().all(|s| matches!(s, Src::Place(..)));
        let session = places && a.held.get(&(g.at[&c.id], i)).copied().unwrap_or(false);
        Some(if session { Held::Session } else { Held::Given })
    };
    let rule = |ci: usize, r: Option<String>| {
        if a.rule_held.get(&(ci, r)).copied().unwrap_or(false) {
            Held::Session
        } else {
            Held::Given
        }
    };
    let mut cells = Vec::new();
    for (ci, c) in g.cells.iter().enumerate() {
        if c.kind == CellKind::Agent {
            for f in &c.out {
                described(&c.id, f, &f.name)?;
            }
        }
        cells.push(CompiledCell {
            id: c.id.clone(),
            kind: c.kind.name().to_string(),
            gate: c.gate,
            title: c.title.clone(),
            step: c.step.clone(),
            needs: c.needs.clone(),
            line: c.line.clone(),
            stage: g.stage.get(&ci).cloned(),
            tools: c.tools.clone(),
            inputs: c
                .inputs
                .iter()
                .enumerate()
                .map(|(i, x)| CompiledInput {
                    id: x.id.clone(),
                    what: x.what.clone(),
                    from: x.from.clone(),
                    maybe: x.maybe,
                    held: held(c, i),
                })
                .collect(),
            project_rules: (c.kind == CellKind::Agent).then(|| rule(ci, None)),
            rules: c
                .rules
                .iter()
                .map(|r| RuleUse {
                    name: r.clone(),
                    held: rule(ci, Some(r.clone())),
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
                    interject: r.interject,
                    main: r.main,
                    common: r.common,
                    note: r.note.clone(),
                })
                .collect(),
            always: c.always.clone(),
            checks: c.checks.clone(),
            handoff: c.handoff.clone(),
            inenums: c.inenums.clone(),
        });
    }
    let f = g.flow;
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
        within: f.agent_common.returns.within.name.clone(),
        asking: f.asking.nodes.clone(),
        receive: f.asking.receive.clone(),
        counters: f.limits.counters.clone(),
        rules: f.rules.clone(),
        interject: f.interject.as_ref().map(|i| CompiledInterject {
            name: i.name.clone(),
            desc: i.desc.clone(),
            text: i.text.clone(),
            from: i.from.clone(),
        }),
        rounds: match &f.rounds {
            Some(r) => Some(rounds(r, &cells)?),
            None => None,
        },
        cells,
    };
    Ok((out, stats))
}

/// The rounds section as written, once each kind of failure is known to go somewhere that exists.
fn rounds(r: &Rounds, cells: &[CompiledCell]) -> Result<Rounds, String> {
    ensure!(r.sent_back.pause_after > 0, "rounds.sentBack.pauseAfter is 0");
    let mut seen = std::collections::BTreeSet::new();
    for k in &r.failed.kinds {
        ensure!(seen.insert(&k.name), "rounds.failed: two kinds named {}", k.name);
        let to = k.then.as_deref();
        match k.handle {
            Handle::Wait => {
                ensure!(
                    to.is_none() && k.retries.is_none(),
                    "rounds.failed {}: a failure waited out goes nowhere and is not retried",
                    k.name
                );
            }
            Handle::Retry => {
                ensure!(
                    k.retries.is_some_and(|n| n > 0) && to.is_some(),
                    "rounds.failed {}: a retried failure says how many times, and where the work goes then",
                    k.name
                );
            }
            Handle::Ask => {
                ensure!(
                    k.retries.is_none() && to.is_some(),
                    "rounds.failed {}: a failure asked about at once is not retried, and says where the work goes",
                    k.name
                );
            }
        }
        if let Some(t) = to {
            ensure!(
                cells.iter().any(|c| c.id == t),
                "rounds.failed {}: no such cell: {t}",
                k.name
            );
        }
    }
    Ok(r.clone())
}
