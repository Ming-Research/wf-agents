//! The message an agent's round is sent, and what the round's session already holds.
//!
//! A message is the round's inputs, of two kinds. The always inputs are given every time the cell is entered: what to
//! do this round (the cell's task), the addresses of the conditional inputs to work from by the number of each of the
//! cell's inputs, why the round happens and which format to hand in. A conditional input
//! is anything worked on: a rule, the item, a setting, another cell's output, a tool's results, an interjection, a
//! return format, what each numbered input is, the worktree. Each has an address, and is given only when the session
//! does not hold it: one the session already holds, by its content, is only referred to; one that is new or changed
//! is given; after a compaction the session is taken to hold nothing. What a session holds is worked out from the
//! record of what each of its rounds was given and handed back. After a return sent back the message only says why,
//! and that the return is to be rewritten.
//!
//! Addresses: a value a round produced is `R<round>·<cell>·<path>`, by the earliest round that produced the same
//! content; a value the engine reads or computes for a round is `R<round>·engine·<name>`, by the earliest round it
//! was the same for; and `rule:<name>`, `config:<setting>`, `item:<id>`, `design_card:<id>`, `ask:<id>` (an ask of
//! the owner), `ij:<id>`, `work:<id>`, `format:common`, `format:<cell>` and `inputs:<cell>`.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use flow::{CompiledCell, CompiledGraph};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::Engine;
use crate::inputs::{Gathered, Origin, Part};
use crate::work::{Item, Round, Work};

/// One conditional input of a message.
#[derive(Debug, Clone, PartialEq)]
pub struct Conditional {
    pub address: String,
    /// What it is: a text, or JSON.
    pub content: Value,
    pub hash: String,
    /// Given in this message; otherwise the session already holds it, and the message only refers to it.
    pub given: bool,
}

/// An agent round's message: its text, word for word, and every conditional input it gives or refers to.
#[derive(Debug, Clone)]
pub struct Message {
    pub text: String,
    /// The text as the round's record keeps it: the text itself, or its parts, each large one (a conditional input
    /// given, the cell's task) a reference to what `kept` holds.
    pub stored: Value,
    /// The large parts, each by the hash `contents` keeps it under.
    pub kept: Vec<(String, String)>,
    pub conditional: Vec<Conditional>,
    /// The round a short message goes on from, a return sent back or a round cut off midway: the session holds
    /// everything that round was given, and only the interjections it was given are this round's.
    pub continues: Option<i64>,
}

impl Message {
    /// The conditional inputs as a round records them: each by its address and hash, given or referred to.
    pub fn record(&self) -> Value {
        Value::Array(
            self.conditional
                .iter()
                .map(|m| json!({ "address": m.address, "hash": m.hash, "given": m.given }))
                .collect(),
        )
    }
}

/// The hash a content is known by: SHA-256 of its JSON with every object's keys in order, so that the same content is
/// one conditional input however its keys were written.
pub fn hash(v: &Value) -> String {
    let mut s = String::new();
    canonical(v, &mut s);
    Sha256::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn canonical(v: &Value, out: &mut String) {
    match v {
        Value::Object(o) => {
            let mut keys: Vec<&String> = o.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(k.clone()).to_string());
                out.push(':');
                canonical(&o[k], out);
            }
            out.push('}');
        }
        Value::Array(xs) => {
            out.push('[');
            for (i, x) in xs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                canonical(x, out);
            }
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}

/// The round an address names, for an address of a round's value.
pub fn round_of(address: &str) -> Option<i64> {
    address.strip_prefix('R')?.split('·').next()?.parse().ok()
}

/// The conditional inputs a round recorded, each as its address and hash; none for a round recorded before they
/// were. A round recorded before they were called so keeps them under `materials`.
fn recorded(input: &Value) -> Vec<(String, String)> {
    input
        .get("conditional")
        .or_else(|| input.get("materials"))
        .unwrap_or(&Value::Null)
        .as_array()
        .map(|xs| {
            xs.iter()
                .filter_map(|m| Some((m["address"].as_str()?.to_string(), m["hash"].as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

/// What a session already holds: what it was given and what it handed back since it was last compacted.
#[derive(Debug, Default)]
pub struct Held {
    /// The conditional inputs it was given or referred to, each as its address and the hash of its content.
    pub conditional: HashSet<(String, String)>,
    /// The rounds whose returns it holds: every value they produced is in the session under its address.
    pub rounds: BTreeSet<i64>,
    /// The last round in which it was compacted: it holds nothing from before that round's end.
    pub compacted: Option<i64>,
}

impl Held {
    /// What the session whose rounds these are, oldest first, holds now. A round counts once it handed in a return:
    /// a run that failed may never have reached the session.
    pub fn of(session_rounds: &[Round]) -> Held {
        let mut h = Held::default();
        for r in session_rounds {
            if r.compactions > 0 {
                h = Held {
                    compacted: Some(r.id),
                    ..Held::default()
                };
                continue;
            }
            if r.output.is_none() {
                continue;
            }
            h.rounds.insert(r.id);
            h.conditional.extend(recorded(&r.input));
        }
        h
    }

    /// Whether the session holds a conditional input: a value one of its own returns produced, or the same content under the
    /// same address as one it was given. One content has one address, so the same content is held however it came.
    pub fn holds(&self, address: &str, hash: &str) -> bool {
        let own = round_of(address).is_some_and(|r| self.rounds.contains(&r)) && !address.contains("·engine·");
        own || self.conditional.contains(&(address.to_string(), hash.to_string()))
    }
}

/// Every address each content was given or referred to by in the work, in the order they first appear, by its hash.
pub fn registry(work_rounds: &[Round]) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for r in work_rounds {
        for (address, hash) in recorded(&r.input) {
            let seen = out.entry(hash).or_default();
            if !seen.contains(&address) {
                seen.push(address);
            }
        }
    }
    out
}

/// Whether a content is something one place copies from another: a list, an object, or a text of more than one word.
/// A word or a number two places give alike is not one thing, and keeps the address of where it came from.
fn copied(v: &Value) -> bool {
    match v {
        Value::Array(_) | Value::Object(_) => true,
        Value::String(s) => s.trim().contains(char::is_whitespace),
        _ => false,
    }
}

/// Whether a value has nothing to give.
fn empty(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.trim().is_empty(),
        Value::Array(xs) => xs.is_empty(),
        Value::Object(o) => o.is_empty(),
        _ => false,
    }
}

/// How a conditional input's content is written in a message: a text as it is, anything else as compact JSON.
fn render(v: &Value) -> String {
    match v {
        Value::String(s) => s.trim_end().to_string(),
        other => other.to_string(),
    }
}

/// Merges one schema into another: the alternatives of a `oneOf` or `anyOf` are joined, each once, and the fields of
/// an object merged by name.
fn merge(a: &mut Value, b: &Value) {
    if a == b {
        return;
    }
    let (Value::Object(x), Value::Object(y)) = (a, b) else {
        return;
    };
    for (k, vb) in y {
        match x.get_mut(k) {
            Some(Value::Array(xs)) if k == "oneOf" || k == "anyOf" => {
                for v in vb.as_array().into_iter().flatten() {
                    if !xs.contains(v) {
                        xs.push(v.clone());
                    }
                }
            }
            Some(va) => merge(va, vb),
            None => {
                x.insert(k.clone(), vb.clone());
            }
        }
    }
}

/// A schema as the formats write it: `additionalProperties: false` left out, since the formats say once that no object
/// holds fields it does not list.
fn lean(schema: &Value) -> Value {
    match schema {
        Value::Object(o) => Value::Object(
            o.iter()
                .filter(|(k, v)| !(*k == "additionalProperties" && **v == Value::Bool(false)))
                .map(|(k, v)| (k.clone(), lean(v)))
                .collect(),
        ),
        Value::Array(xs) => Value::Array(xs.iter().map(lean).collect()),
        other => other.clone(),
    }
}

/// How long, as compact JSON, a part of a schema must be for the formats to give it once when it repeats.
const LARGE: usize = 300;

/// A schema with each large part it repeats given once, under `$defs`, and referred to by `$ref` wherever it
/// repeats, the longest first: a field several forms share, such as the kinds of ask under each role an agent may
/// ask, is written once.
fn once(schema: Value) -> Value {
    let mut s = schema;
    let mut defs = Map::new();
    loop {
        let mut seen: HashMap<String, (usize, String, Value)> = HashMap::new();
        repeats(&s, "part", true, &mut seen);
        let Some((key, (_, name, part))) = seen
            .into_iter()
            .filter(|(k, (n, _, _))| *n > 1 && k.len() >= LARGE)
            .max_by(|a, b| a.0.len().cmp(&b.0.len()).then_with(|| b.0.cmp(&a.0)))
        else {
            break;
        };
        let mut id = name.clone();
        let mut i = 2;
        while defs.contains_key(&id) {
            id = format!("{name}_{i}");
            i += 1;
        }
        refer(&mut s, &key, &json!({ "$ref": format!("#/$defs/{id}") }));
        defs.insert(id, part);
    }
    if let (Value::Object(o), false) = (&mut s, defs.is_empty()) {
        o.insert("$defs".into(), Value::Object(defs));
    }
    s
}

/// Counts every object in a schema by its canonical text, with the name of the field it describes; the root is not
/// counted.
fn repeats(v: &Value, name: &str, root: bool, seen: &mut HashMap<String, (usize, String, Value)>) {
    match v {
        Value::Object(o) => {
            if !root {
                let mut key = String::new();
                canonical(v, &mut key);
                seen.entry(key).or_insert_with(|| (0, name.to_string(), v.clone())).0 += 1;
            }
            for (k, x) in o {
                // A part is named by the field it describes: what holds fields, items or alternatives keeps its name.
                let structural = matches!(k.as_str(), "properties" | "items" | "oneOf" | "anyOf");
                repeats(x, if structural { name } else { k }, false, seen);
            }
        }
        Value::Array(xs) => {
            for x in xs {
                repeats(x, name, false, seen);
            }
        }
        _ => {}
    }
}

/// Puts `with` in place of every object whose canonical text is `key`.
fn refer(v: &mut Value, key: &str, with: &Value) {
    match v {
        Value::Object(o) => {
            let mut text = String::new();
            canonical(&Value::Object(o.clone()), &mut text);
            if text == key {
                *v = with.clone();
                return;
            }
            for x in o.values_mut() {
                refer(x, key, with);
            }
        }
        Value::Array(xs) => {
            for x in xs {
                refer(x, key, with);
            }
        }
        _ => {}
    }
}

fn quoted(names: &[String]) -> String {
    names.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ")
}

/// The forms every agent's return in a graph shares, with each cell's own answer apart: what `format:common` gives.
/// A form some cells have with fewer alternatives (fewer tools, fewer roles to ask) is given with all of them;
/// `format:<cell>` says which a cell has.
pub fn common_format(g: &CompiledGraph) -> String {
    let mut forms: Vec<(String, Value)> = Vec::new();
    let mut every: Map<String, Value> = Map::new();
    let mut without: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut within = Value::Null;
    for c in g.cells.iter().filter(|c| c.kind == "agent") {
        let Some(props) = c.schema.as_ref().and_then(|s| s["properties"].as_object()) else {
            continue;
        };
        let alts = props
            .get(&g.within)
            .and_then(|w| w["anyOf"].as_array())
            .cloned()
            .unwrap_or_default();
        if within.is_null() {
            within = props
                .get(&g.within)
                .and_then(|w| w.get("description"))
                .cloned()
                .unwrap_or(Value::Null);
        }
        for (f, a) in c.forms.iter().zip(&alts) {
            let Some(name) = &f.field else { continue };
            match forms.iter_mut().find(|(n, _)| n == name) {
                Some((_, s)) => merge(s, a),
                None => forms.push((name.clone(), a.clone())),
            }
            let left = without.entry(name.clone()).or_default();
            for x in &f.without {
                if !left.contains(x) {
                    left.push(x.clone());
                }
            }
        }
        for (k, v) in props.iter().filter(|(k, _)| **k != g.within) {
            match every.get_mut(k) {
                Some(s) => merge(s, v),
                None => {
                    every.insert(k.clone(), v.clone());
                }
            }
        }
    }
    let mut schema = Map::new();
    schema.insert(
        g.within.clone(),
        json!({ "description": within, "anyOf": forms.iter().map(|x| x.1.clone()).collect::<Vec<_>>() }),
    );
    let every_names: Vec<String> = every.keys().cloned().collect();
    schema.extend(every);
    let names: Vec<String> = forms.iter().map(|x| x.0.clone()).collect();
    let leaves: Vec<String> = without
        .iter()
        .filter(|(_, xs)| !xs.is_empty())
        .map(|(f, xs)| format!("a return of the form `{f}` leaves out {}", quoted(xs)))
        .collect();
    let leaves = if leaves.is_empty() {
        String::new()
    } else {
        format!("; {}", leaves.join("; "))
    };
    let write: String = g.write.iter().map(|l| format!("- {l}\n")).collect();
    let write = if write.is_empty() {
        String::new()
    } else {
        format!("\n\nHow a return is written:\n{write}")
    };
    format!(
        "The forms every agent's return shares. A return is one JSON object; its field `{w}` holds exactly one form: \
         the cell's own answer (`format:<cell>`), or one of these that `format:<cell>` names: {forms}. Every return \
         also holds {every}{leaves}.{write}\n\nEach field, as JSON Schema; no object holds fields it does not list, \
         and a part used in more than one place is under `$defs`:\n\n{json}",
        w = g.within,
        forms = quoted(&names),
        every = quoted(&every_names),
        json = once(lean(&Value::Object(schema))),
    )
}

/// A cell's own answer, and which of the forms every return shares the cell has: what `format:<cell>` gives.
pub fn cell_format(g: &CompiledGraph, cell: &CompiledCell) -> String {
    let alts = cell
        .schema
        .as_ref()
        .and_then(|s| s["properties"][&g.within]["anyOf"].as_array())
        .cloned()
        .unwrap_or_default();
    let mut own = Value::Null;
    let mut others = Vec::new();
    for (f, a) in cell.forms.iter().zip(&alts) {
        match &f.field {
            None => own = a.clone(),
            Some(name) if *name == "tools" || a["properties"][name]["items"]["oneOf"].is_array() => {
                others.push(format!("`{name}` (of {})", cell.tools.join(", ")));
            }
            Some(name) => {
                let roles: Vec<&str> = a["properties"][name]["oneOf"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|b| b["properties"].as_object())
                    .flat_map(|p| p.values().filter_map(|v| v["const"].as_str()))
                    .collect();
                if roles.is_empty() {
                    others.push(format!("`{name}`"));
                } else {
                    others.push(format!("`{name}` (to {})", roles.join(", ")));
                }
            }
        }
    }
    format!(
        "The answer of cell {id} ({title}), which `{w}` holds, as JSON Schema; instead of it, a return of this cell may \
         hold these forms of `format:common`: {others}.\n\n{json}",
        id = cell.id,
        title = cell.title,
        w = g.within,
        others = others.join(", "),
        json = once(lean(&own)),
    )
}

/// A return format or a rule with the value of each setting it names written beside the setting's name, such as
/// the owner's language beside `owner.language`: the flow and the rules name a setting, never its value, which the
/// configuration holds. In a return format, which holds the schema as JSON, the value is written as JSON writes it
/// inside a string.
fn with_settings(e: &Engine, project: &str, text: String, in_json: bool) -> String {
    let mut out = text;
    // The fences around code are not names: without them, names are the text between pairs of backticks.
    for (name, value) in e.config.config.named_settings(project, &out.replace("```", "")) {
        let value = if in_json {
            let quoted = Value::String(value).to_string();
            quoted[1..quoted.len() - 1].to_string()
        } else {
            value
        };
        out = out.replace(&format!("`{name}`"), &format!("`{name}` ({value})"));
    }
    out
}

/// What a message is built from.
pub struct Context<'a> {
    pub e: &'a Engine,
    pub w: &'a Work,
    pub g: &'a CompiledGraph,
    pub cell: &'a CompiledCell,
    pub item: &'a Item,
    /// This round's number.
    pub round: i64,
    /// Every round of the work so far, oldest first.
    pub rounds: &'a [Round],
    pub gathered: &'a [Gathered],
    /// The values the engine knows and rows recorded, which pick the lines the prompt adds.
    pub given: &'a Map<String, Value>,
    pub columns: &'a [(String, String)],
    pub held: &'a Held,
    pub registry: &'a HashMap<String, Vec<String>>,
}

/// The conditional inputs of a message as it is built: each content once, under its one address.
struct Conditionals<'a> {
    held: &'a Held,
    registry: &'a HashMap<String, Vec<String>>,
    list: Vec<Conditional>,
}

impl Conditionals<'_> {
    /// Adds a conditional input, and gives the address the message refers to it by: one already added with the same content
    /// keeps its address.
    fn add(&mut self, address: String, content: Value) -> String {
        let h = hash(&content);
        let one = copied(&content);
        if let Some(m) = self.list.iter().find(|m| m.hash == h && (one || m.address == address)) {
            return m.address.clone();
        }
        let given = !self.held.holds(&address, &h);
        self.list.push(Conditional {
            address: address.clone(),
            content,
            hash: h,
            given,
        });
        address
    }

    /// The address of a round's value: of the rounds that produced the same content in the work, wherever it was
    /// copied to, the earliest.
    fn earliest(&self, address: String, content: &Value) -> String {
        if !copied(content) {
            return address;
        }
        let known = self.registry.get(&hash(content)).into_iter().flatten();
        let first = known
            .filter(|a| !a.contains("·engine·"))
            .filter_map(|a| Some((round_of(a)?, a)))
            .min_by_key(|x| x.0);
        match first {
            Some((r, a)) if round_of(&address).is_some_and(|mine| r < mine) => a.clone(),
            _ => address,
        }
    }
}

impl Context<'_> {
    /// The address of what the engine read or computed for this round under a name: the earliest round it was the
    /// same for.
    fn computed(&self, m: &Conditionals<'_>, name: &str, content: &Value) -> String {
        let suffix = format!("·engine·{name}");
        m.registry
            .get(&hash(content))
            .and_then(|known| known.iter().find(|a| a.ends_with(&suffix)))
            .cloned()
            .unwrap_or_else(|| format!("R{}{suffix}", self.round))
    }

    fn cell_of_round(&self, id: i64) -> Option<String> {
        self.rounds
            .iter()
            .find(|r| r.id == id)
            .and_then(|r| r.cell.clone())
            .or_else(|| {
                self.e
                    .db
                    .conn()
                    .query_row("SELECT cell FROM rounds WHERE id = ?1", [id], |r| r.get(0))
                    .ok()
            })
    }

    /// The conditional inputs one source of an input gave, each with its address.
    fn conditional_of(&self, m: &Conditionals<'_>, p: &Part) -> Vec<(String, Value)> {
        match &p.origin {
            Origin::Place { first, cell, path, .. } => {
                vec![(m.earliest(format!("R{first}·{cell}·{path}"), &p.value), p.value.clone())]
            }
            Origin::Config(n) => vec![(format!("config:{n}"), p.value.clone())],
            Origin::Recorded(n) | Origin::Engine(n) => vec![(self.computed(m, n, &p.value), p.value.clone())],
            Origin::Db(rel) => {
                let rows = p.value.as_array().cloned().unwrap_or_default();
                match rel.as_str() {
                    "item_body" => rows.into_iter().map(|r| (format!("item:{}", r["item"]), r)).collect(),
                    "item_design_cards" | "parent_design_cards" => rows
                        .into_iter()
                        .map(|r| (format!("design_card:{}", r["design_card"]), r))
                        .collect(),
                    "item_asks" => rows.into_iter().map(|r| (format!("ask:{}", r["ask"]), r)).collect(),
                    "waiting_interjections" => rows
                        .into_iter()
                        .map(|r| {
                            let text = format!(
                                "From the {}: {}",
                                r["speaker"].as_str().unwrap_or("owner"),
                                r["text"].as_str().unwrap_or_default()
                            );
                            (format!("ij:{}", r["interjection"]), Value::String(text))
                        })
                        .collect(),
                    // The locked acceptance is what the round that locked it handed in.
                    "item_acceptance" => rows
                        .into_iter()
                        .filter_map(|r| {
                            let a = r.get("acceptance").filter(|a| !empty(a))?.clone();
                            let round = r["acceptance_round"].as_i64()?;
                            let path = match self.cell_of_round(round).as_deref() {
                                Some("12") => "12·reply.new_acceptance".to_string(),
                                Some(c) => format!("{c}·acceptance"),
                                None => "acceptance".to_string(),
                            };
                            Some((m.earliest(format!("R{round}·{path}"), &a), a))
                        })
                        .collect(),
                    // The problem as the engineer understood it, once research handed it in.
                    "item_problem" => rows
                        .into_iter()
                        .map(|r| {
                            let researched: Option<i64> = self
                                .e
                                .db
                                .conn()
                                .query_row(
                                    "SELECT r.id FROM rounds r JOIN works w ON w.id = r.work
                                     WHERE w.item = ?1 AND r.cell = '1' AND r.transition IS NOT NULL
                                       AND json_extract(r.output, '$.outcome.understanding') IS NOT NULL
                                     ORDER BY r.id DESC LIMIT 1",
                                    [self.item.id],
                                    |x| x.get(0),
                                )
                                .ok();
                            let address = match researched {
                                Some(k) => format!("R{k}·1·understanding.problem"),
                                None => format!("item:{}·problem", self.item.id),
                            };
                            (address, r["problem"].clone())
                        })
                        .collect(),
                    _ => vec![(self.computed(m, rel, &p.value), p.value.clone())],
                }
            }
        }
    }

    /// Why the cell's last return was sent back, when this round only rewrites it: the session holds everything else.
    fn sent_back(&self) -> Option<(i64, String)> {
        let r = self.rounds.last()?;
        if r.cell.as_deref() != Some(self.cell.id.as_str()) || r.transition.is_some() {
            return None;
        }
        let rej = r.rejected.as_ref()?;
        if ["interrupted", "failed", "error"].iter().any(|k| rej.get(k).is_some()) {
            return None;
        }
        let reason = rej
            .as_object()
            .and_then(|o| o.values().next())
            .map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_string))
            .unwrap_or_default();
        Some((r.id, reason))
    }

    /// The cell's last round, when the engine stopped in the middle of it and the session it ran in goes on: a
    /// session that handed in a return before is resumed, and holds what that round was given.
    fn cut_off(&self) -> Option<i64> {
        let r = self.rounds.last()?;
        let here = r.cell.as_deref() == Some(self.cell.id.as_str()) && r.transition.is_none();
        let cut = r.rejected.as_ref().is_some_and(|x| x.get("interrupted").is_some());
        (here && cut && !self.held.rounds.is_empty()).then_some(r.id)
    }

    /// Why this round happens, and the conditional inputs it says it with.
    fn why(&self, m: &mut Conditionals<'_>, interjections: &[String]) -> Vec<String> {
        let c = self.cell;
        let mut out = Vec::new();
        let here = |r: &&Round| r.cell.as_deref() == Some(c.id.as_str());
        let earlier = self.rounds.iter().rev().find(here);
        match self.rounds.last() {
            Some(r) if here(&r) && r.transition.is_none() => {
                let rej = r.rejected.clone().unwrap_or(Value::Null);
                if rej.get("interrupted").is_some() {
                    out.push(format!(
                        "R{} was cut off when the engine stopped: do its work again.",
                        r.id
                    ));
                } else if let Some(f) = rej.get("failed").or(rej.get("error")) {
                    let what = f.get("what").and_then(Value::as_str).or(f.as_str()).unwrap_or_default();
                    out.push(format!("R{} ended without a return ({what}): do its work again.", r.id));
                }
            }
            Some(r) => {
                let row = r.transition.as_deref().unwrap_or_default();
                let from = r.cell.as_deref().unwrap_or_default();
                let paused = here(&r) && c.rows.iter().any(|x| x.name == row && x.to == "@self");
                if paused {
                    out.push(format!("You paused in R{}: carry on.", r.id));
                } else {
                    let title = self.g.cell(from).map(|x| x.title.as_str()).unwrap_or_default();
                    out.push(format!(
                        "Came from cell {from} ({title}) by its row `{row}`, R{}.",
                        r.id
                    ));
                    // Back from asking: the answer it brought.
                    if self.g.asking.iter().any(|a| a == from) {
                        let since = earlier.map_or(0, |e| e.id);
                        let answer = self.rounds.iter().rev().take_while(|x| x.id > since).find_map(|x| {
                            let cell = x.cell.as_deref()?;
                            let kind = self.g.cell(cell)?.kind.as_str();
                            let out = x.output.as_ref().filter(|_| x.transition.is_some())?;
                            let out = if kind == "agent" {
                                self.g.unwrap(out)
                            } else {
                                out.clone()
                            };
                            let reply = out.get("reply").filter(|v| !empty(v))?.clone();
                            Some((x.id, cell.to_string(), reply))
                        });
                        if let Some((id, cell, reply)) = answer {
                            let a = m.add(m.earliest(format!("R{id}·{cell}·reply"), &reply), reply);
                            out.push(format!("The answer to what you asked: {a}."));
                        }
                    }
                }
            }
            None => {}
        }
        if let Some(k) = self.held.compacted
            && self.held.rounds.is_empty()
        {
            out.push(format!(
                "Your session was compacted in R{k}: what this round needs is given again."
            ));
        }
        if !interjections.is_empty() {
            out.push(format!(
                "Said to you: {}. Answer in this round.",
                interjections.join(", ")
            ));
        }
        out
    }

    /// The message. After a return sent back it only says why, and that the return is to be rewritten.
    pub fn build(&self) -> Message {
        let (e, w, g, c, item) = (self.e, self.w, self.g, self.cell, self.item);
        let short = |text: String, prev: i64| Message {
            stored: Value::String(text.clone()),
            text,
            kept: Vec::new(),
            conditional: Vec::new(),
            continues: Some(prev),
        };
        if let Some((prev, reason)) = self.sent_back() {
            let text = format!(
                "# R{} · cell {}\n\nYour return was sent back: {reason}\nRewrite it (format:{}).\n",
                self.round, c.id, c.id
            );
            return short(text, prev);
        }
        if let Some(prev) = self.cut_off() {
            let text = format!(
                "# R{} · cell {}\n\nR{prev} was cut off midway. First look at the worktree as it is now (files \
                 changed, commits made), then go on with its work, and hand it in as format:{} says.\n",
                self.round, c.id, c.id
            );
            return short(text, prev);
        }
        let mut m = Conditionals {
            held: self.held,
            registry: self.registry,
            list: Vec::new(),
        };
        // What every round needs from the start: the return formats (the query tool's description in them lists the
        // database's tables and views), what each numbered input of the cell is, the rules and the worktree; the value
        // of each setting the formats and the rules name is written beside its name.
        m.add(
            "format:common".into(),
            Value::String(with_settings(e, &item.project, common_format(g), true)),
        );
        m.add(
            format!("format:{}", c.id),
            Value::String(with_settings(e, &item.project, cell_format(g, c), true)),
        );
        let numbered = cell_inputs(c);
        if !numbered.is_empty() {
            m.add(format!("inputs:{}", c.id), Value::String(numbered));
        }
        let mut rules = Vec::new();
        let project = e.config.config.projects.get(&item.project);
        for name in project.map(|p| p.rules.clone()).unwrap_or_default() {
            let Some(text) = e.config.config.rules.get(&name) else {
                continue;
            };
            let text = with_settings(e, &item.project, text.trim().to_string(), false);
            rules.push(m.add(format!("rule:{name}"), Value::String(text)));
        }
        for r in &c.rules {
            if let Some(rule) = g.rules.iter().find(|x| x.name == r.name) {
                let lines: Vec<String> = rule.text.iter().map(|t| format!("- {t}")).collect();
                let text = with_settings(
                    e,
                    &item.project,
                    format!("{}\n\n{}", rule.desc, lines.join("\n")),
                    false,
                );
                rules.push(m.add(format!("rule:{}", rule.name), Value::String(text)));
            }
        }
        let workspace = workspace(w, project).map(|text| m.add(format!("work:{}", w.id), Value::String(text)));
        // The inputs, each by its number in `inputs:<cell>` and the addresses of what it holds; what the agent reads
        // itself is said in full.
        let mut work_from = Vec::new();
        let mut interjections = Vec::new();
        let mut number = 0;
        for x in self.gathered {
            if x.itself() {
                work_from.push(format!("Read yourself: {}", x.input.what));
                continue;
            }
            number += 1;
            let mut addrs: Vec<String> = Vec::new();
            for p in &x.parts {
                for (address, content) in self.conditional_of(&m, p) {
                    if empty(&content) {
                        continue;
                    }
                    let a = m.add(address, content);
                    if a.starts_with("ij:") && !interjections.contains(&a) {
                        interjections.push(a.clone());
                    }
                    if !addrs.contains(&a) {
                        addrs.push(a);
                    }
                }
            }
            if !addrs.is_empty() {
                work_from.push(format!("{number}. {}", addrs.join(", ")));
            }
        }
        if !rules.is_empty() {
            work_from.push(format!("Rules: {}", rules.join(", ")));
        }
        if let Some(a) = workspace {
            work_from.push(format!("Worktree: {a}"));
        }
        let why = self.why(&mut m, &interjections);

        let mut out = Writer::default();
        out.line(&format!("# R{} · cell {} · {}\n", self.round, c.id, c.title));
        if !why.is_empty() {
            out.line(&why.join("\n"));
            out.line("");
        }
        let given: Vec<&Conditional> = m.list.iter().filter(|x| x.given).collect();
        if !given.is_empty() {
            out.line("## Given\n");
            for x in given {
                out.line(&format!("### {}\n", x.address));
                out.keep(&render(&x.content));
                out.line("");
            }
        }
        out.line("## Task\n");
        let mut task: Vec<&str> = c.prompt.iter().map(String::as_str).collect();
        if let Some(pb) = &c.prompt_by
            && let Some(v) = self.given.get(&pb.by).and_then(Value::as_str)
            && let Some(lines) = pb.cases.get(v)
        {
            task.extend(lines.iter().map(String::as_str));
        }
        out.keep(&task.join("\n"));
        out.line("");
        if !work_from.is_empty() {
            out.line("## Work from\n");
            for l in &work_from {
                out.line(l);
            }
            out.line("");
        }
        if !self.columns.is_empty() {
            out.line("## Review table: answer each column by its number\n");
            for (id, col) in self.columns {
                out.line(&format!("{id}. {col}"));
            }
            out.line("");
        }
        out.line(&format!("Hand in one JSON object, as format:{} says.", c.id));
        let (text, stored, kept) = out.finish();
        Message {
            text,
            stored,
            kept,
            conditional: m.list,
            continues: None,
        }
    }
}

/// A message as it is written, line by line, with the large parts the round's record keeps once.
#[derive(Default)]
struct Writer {
    text: String,
    /// The parts the record keeps, and the text written since the last large one.
    parts: Vec<Value>,
    since: String,
    kept: Vec<(String, String)>,
}

impl Writer {
    fn line(&mut self, s: &str) {
        self.text.push_str(s);
        self.text.push('\n');
        self.since.push_str(s);
        self.since.push('\n');
    }

    /// A line that may be large: the record refers to a large one by its hash, and keeps it once.
    fn keep(&mut self, s: &str) {
        if s.len() < db::STORED {
            return self.line(s);
        }
        self.text.push_str(s);
        self.text.push('\n');
        if !self.since.is_empty() {
            self.parts.push(Value::String(std::mem::take(&mut self.since)));
        }
        let h = db::content_hash(s);
        self.parts.push(json!({ "content": h }));
        self.since.push('\n');
        if !self.kept.iter().any(|(k, _)| *k == h) {
            self.kept.push((h, s.to_string()));
        }
    }

    /// The text, the text as the record keeps it, and what the record keeps once.
    fn finish(mut self) -> (String, Value, Vec<(String, String)>) {
        if self.parts.is_empty() {
            let text = self.text;
            return (text.clone(), Value::String(text), Vec::new());
        }
        if !self.since.is_empty() {
            self.parts.push(Value::String(std::mem::take(&mut self.since)));
        }
        (self.text, json!({ "parts": self.parts }), self.kept)
    }
}

/// What each numbered input of a cell is, the inputs the agent reads itself left out: what `inputs:<cell>` gives, and
/// what the numbers in a message's list of inputs to work from stand for.
pub fn cell_inputs(cell: &CompiledCell) -> String {
    let lines: Vec<String> = cell
        .inputs
        .iter()
        .filter(|x| !x.from.iter().all(|s| *s == flow::format::Src::Itself))
        .enumerate()
        .map(|(i, x)| format!("{}. {}", i + 1, x.what))
        .collect();
    if lines.is_empty() {
        return String::new();
    }
    format!(
        "What each input of cell {} ({}) is, by the number a round's list of inputs gives it:\n{}",
        cell.id,
        cell.title,
        lines.join("\n")
    )
}

/// Where the work's agents work: its worktree and branch, and the project's branch the flow calls main; what
/// `work:<id>` gives. None for a work of no project the configuration knows.
fn workspace(w: &Work, project: Option<&config::Project>) -> Option<String> {
    let p = project?;
    let main = format!(
        "Main, wherever the flow says it, is the branch {}: the work branches from it, its PR merges into it, and a \
         diff against main is a diff against it.",
        p.main
    );
    Some(match (&w.worktree, &w.branch) {
        (Some(wt), Some(branch)) => format!(
            "Your working directory is the worktree {}, on the branch {branch}: commit there, and the engine pushes \
             when you ask it to with a tool. {main}",
            wt.display()
        ),
        _ => main,
    })
}

/// The columns the table review answers, each with its number, for the review of this round. A re-review that looks
/// at disputes only has none; one that looks at what changed has them all, since which columns a change concerns is
/// not worked out in this version.
pub fn columns(e: &Engine, w: &Work, g: &CompiledGraph, cell: &CompiledCell, item: &Item) -> Vec<(String, String)> {
    let rule = |name: &str| {
        g.rules
            .iter()
            .find(|r| r.name == name)
            .map(|r| r.text.clone())
            .unwrap_or_default()
    };
    let mut cols: Vec<String> = Vec::new();
    match cell.id.as_str() {
        "5a" => {
            let review = crate::inputs::last_return(e, w, g, "18", "plan")
                .ok()
                .flatten()
                .and_then(|r| crate::inputs::at_path(&r, "plan.review"));
            if review.as_ref().and_then(Value::as_str) == Some("disputes") {
                return Vec::new();
            }
            let table = rule("review_table");
            let project = e.config.config.projects.get(&item.project);
            if item.deliver == "report" {
                cols.extend(table.get(16).cloned());
            } else {
                cols.extend(table.iter().skip(2).take(7).cloned());
                for entry in project.map(|p| p.review_list.clone()).unwrap_or_default() {
                    cols.push(format!("From the review list in the project's configuration: {entry}"));
                }
            }
            let acceptance = item.acceptance.clone().unwrap_or(Value::Null);
            for a in acceptance.as_array().cloned().unwrap_or_default() {
                cols.push(format!(
                    "Acceptance condition {}: {} Can it be checked; is the evidence there, does it come from the head to be merged, and does it hold?",
                    a["id"].as_str().unwrap_or("?"),
                    a["condition"].as_str().unwrap_or_default()
                ));
            }
            if item.deliver != "report" {
                cols.push("The research's judgment that the work stands: does its reason hold?".into());
                cols.push("The judgment that there is no better design: does its reason hold?".into());
                for (c, what) in [
                    ("3a", "a gap in what is under test"),
                    ("3b", "unsure facts"),
                    ("3c", "architectural burden"),
                    ("3d", "cross-project dependency"),
                    ("3e", "splitting"),
                ] {
                    if g.cell(c).is_some() {
                        cols.push(format!(
                            "The check of {what} answered \"no\" or not applicable: does its reason hold?"
                        ));
                    }
                }
                let changes = crate::inputs::last_return(e, w, g, "4", "deviations").ok().flatten();
                for d in changes
                    .and_then(|r| r["deviations"].as_array().cloned())
                    .unwrap_or_default()
                {
                    if d["kind"] != "surprise" {
                        cols.push(format!(
                            "The {} \"{}\": does its reason hold, and did the design change with it?",
                            d["kind"].as_str().unwrap_or_default().replace('_', " "),
                            d["what"].as_str().unwrap_or_default()
                        ));
                    }
                }
                let ends = crate::inputs::last_return(e, w, g, "4", "end").ok().flatten();
                for t in ends
                    .and_then(|r| r["end"]["test_changes"].as_array().cloned())
                    .unwrap_or_default()
                {
                    let name = t["test"].as_str().unwrap_or_default();
                    match t["change"].as_str() {
                        Some("added") => cols.push(format!(
                            "The new test {name}: take the other side on its running time. Do other tests already catch what it catches, could it be smaller or faster, could it be folded into another test?"
                        )),
                        Some(c) => cols.push(format!(
                            "The test {name}, {c}: compared with before, is there a technical reason?"
                        )),
                        None => {}
                    }
                }
            }
        }
        "5b" => {
            let review = rule("design_tree_review");
            for i in [1, 2, 3, 4, 10, 11] {
                cols.extend(review.get(i).cloned());
            }
        }
        _ => {}
    }
    cols.into_iter()
        .enumerate()
        .map(|(i, c)| ((i + 1).to_string(), c))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(id: i64, input: Value, output: Option<Value>, compactions: i64) -> Round {
        Round {
            id,
            cell: Some("1".into()),
            session: Some(1),
            input,
            output,
            transition: Some("done".into()),
            rejected: None,
            compactions,
            started_at: 0,
            ended_at: Some(0),
            usage: None,
        }
    }

    fn graph() -> CompiledGraph {
        let relations = db::relations(db::SCHEMA).unwrap();
        let f = flow::compile(
            flow::FLOW,
            &flow::Context {
                relations,
                tables: db::index_of(db::SCHEMA).unwrap(),
            },
        )
        .unwrap();
        f.graph(crate::ENGINEER).unwrap().clone()
    }

    #[test]
    fn the_same_content_is_one_hash_however_its_keys_were_written() {
        assert_eq!(hash(&json!({ "a": 1, "b": [2] })), hash(&json!({ "b": [2], "a": 1 })));
        assert_ne!(hash(&json!({ "a": 1 })), hash(&json!({ "a": 2 })));
        assert_eq!(round_of("R28·4·end.test_changes[added]"), Some(28));
        assert_eq!(round_of("rule:ground"), None);
    }

    // A session holds what it was given, referred to and handed back since its last compaction, and nothing from a
    // run that handed nothing in.
    #[test]
    fn a_session_holds_what_it_was_given_and_returned_since_its_last_compaction() {
        let given = |a: &str, v: &Value| json!({ "conditional": [{ "address": a, "hash": hash(v), "given": true }] });
        let rule = json!("Facts first.");
        let rounds = [
            round(
                2,
                given("rule:ground", &rule),
                Some(json!({ "outcome": {}, "progress": "p" })),
                0,
            ),
            round(3, json!({}), Some(json!({ "outcome": { "design": "d" } })), 0),
            round(4, given("ij:1", &json!("From the owner: x")), None, 0),
        ];
        let h = Held::of(&rounds);
        assert!(h.holds("rule:ground", &hash(&rule)));
        assert!(!h.holds("rule:ground", &hash(&json!("Facts first, then the rest."))));
        assert!(h.holds("R3·2a·design", "any"));
        assert!(!h.holds("R3·engine·project_guide", "any"));
        assert!(!h.holds("ij:1", &hash(&json!("From the owner: x"))));
        let mut later = rounds.to_vec();
        later.push(round(5, json!({}), Some(json!({ "outcome": {} })), 1));
        let h = Held::of(&later);
        assert!(!h.holds("rule:ground", &hash(&rule)));
        assert!(!h.holds("R3·2a·design", "any"));
        assert_eq!(h.compacted, Some(5));
    }

    // The same content copied from one place to another is one conditional input under the earliest round's address; a word
    // two places give alike keeps each place's address.
    #[test]
    fn copied_content_has_one_address_and_a_word_keeps_its_own() {
        let held = Held::default();
        let acceptance = json!([{ "id": "A1", "condition": "It works." }]);
        let registry = HashMap::from([
            (hash(&acceptance), vec!["R25·12·reply.new_acceptance".to_string()]),
            (hash(&json!("no")), vec!["R7·engine·adversarial".to_string()]),
        ]);
        let mut m = Conditionals {
            held: &held,
            registry: &registry,
            list: Vec::new(),
        };
        assert_eq!(
            m.earliest("R30·2a·acceptance".into(), &acceptance),
            "R25·12·reply.new_acceptance"
        );
        assert_eq!(
            m.earliest("R9·3b·answer.verdict".into(), &json!("no")),
            "R9·3b·answer.verdict"
        );
        assert_eq!(m.add("R9·engine·rethought".into(), json!("no")), "R9·engine·rethought");
        assert_eq!(
            m.add("R9·3b·answer.verdict".into(), json!("no")),
            "R9·3b·answer.verdict"
        );
        assert_eq!(
            m.add("R30·2a·acceptance".into(), acceptance.clone()),
            "R30·2a·acceptance"
        );
        assert_eq!(
            m.add("R31·12·reply.new_acceptance".into(), acceptance),
            "R30·2a·acceptance"
        );
        assert_eq!(m.list.len(), 3);
    }

    // The forms every return shares hold every tool and every role any cell has; a cell's own format names the ones
    // it has.
    #[test]
    fn the_common_format_holds_every_alternative_and_a_cell_says_which_it_has() {
        let g = graph();
        let common = common_format(&g);
        for t in [
            "\"ci\"",
            "\"ci_before\"",
            "\"gh\"",
            "\"query\"",
            "\"advisor\"",
            "\"architect\"",
            "\"owner\"",
        ] {
            assert!(common.contains(t), "{t}");
        }
        assert!(common.contains("leaves out `progress`"), "{common}");
        // The kinds of ask, the same under every role asked, are written once.
        assert_eq!(
            common.matches("The existing architecture or design has a flaw").count(),
            1,
            "{common}"
        );
        assert!(common.contains("\"$ref\":\"#/$defs/"), "{common}");
        let four = cell_format(&g, g.cell("4").unwrap());
        assert!(
            four.contains("`tools` (of ci, ci_before, measure, gh, query)"),
            "{four}"
        );
        assert!(four.contains("`ask` (to advisor, architect, owner)"), "{four}");
        let twelve = cell_format(&g, g.cell("12").unwrap());
        assert!(twelve.contains("`ask` (to architect, owner)"), "{twelve}");
        assert!(!twelve.contains("`redirect`"), "{twelve}");
    }
}
