//! The message an agent's round is sent, and what the round's session already holds.
//!
//! A message is an instruction and materials. The instruction is what to do this round: the cell's task, the
//! materials to work from by their addresses, and why the round happens; it is given every time the cell is entered.
//! A material is anything worked on: a rule, the item, a setting, another cell's output, a tool's results, an
//! interjection, the database's index, a return format. Each has an address, and is given once in a session: a
//! material the session already holds, by its content, is only referred to; one that is new or changed is given;
//! after a compaction the session is taken to hold nothing. What a session holds is worked out from the record of
//! what each of its rounds was given and handed back.
//!
//! Addresses: a value a round produced is `R<round>·<cell>·<path>`, by the earliest round that produced the same
//! content; a value the engine reads or computes for a round is `R<round>·engine·<name>`, by the earliest round it
//! was the same for; and `rule:<name>`, `config:<setting>`, `item:<id>`, `card:<id>`, `ij:<id>`,
//! `format:common` and `format:<cell>`.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use flow::{CompiledCell, CompiledGraph};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::Engine;
use crate::inputs::{Gathered, Origin, Part};
use crate::work::{Item, Round, Work};

/// One material of a message.
#[derive(Debug, Clone, PartialEq)]
pub struct Material {
    pub address: String,
    /// What it is: a text, or JSON.
    pub content: Value,
    pub hash: String,
    /// Given in this message; otherwise the session already holds it, and the message only refers to it.
    pub given: bool,
}

/// An agent round's message: its text, word for word, and every material it gives or refers to.
#[derive(Debug, Clone)]
pub struct Message {
    pub text: String,
    pub materials: Vec<Material>,
}

impl Message {
    /// The materials as a round records them: each by its address and hash, given or referred to.
    pub fn record(&self) -> Value {
        Value::Array(
            self.materials
                .iter()
                .map(|m| json!({ "address": m.address, "hash": m.hash, "given": m.given }))
                .collect(),
        )
    }
}

/// The hash a content is known by: SHA-256 of its JSON with every object's keys in order, so that the same content is
/// one material however its keys were written.
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

/// The materials a round recorded, each as its address and hash; none for a round recorded before materials were.
fn recorded(input: &Value) -> Vec<(String, String)> {
    input["materials"]
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
    /// The materials it was given or referred to, each as its address and the hash of its content.
    pub materials: HashSet<(String, String)>,
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
            h.materials.extend(recorded(&r.input));
        }
        h
    }

    /// Whether the session holds a material: a value one of its own returns produced, or the same content under the
    /// same address as one it was given. One content has one address, so the same content is held however it came.
    pub fn holds(&self, address: &str, hash: &str) -> bool {
        let own = round_of(address).is_some_and(|r| self.rounds.contains(&r)) && !address.contains("·engine·");
        own || self.materials.contains(&(address.to_string(), hash.to_string()))
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

/// How a material's content is written in a message.
fn render(v: &Value) -> String {
    match v {
        Value::String(s) => s.trim_end().to_string(),
        other => format!(
            "```json\n{}\n```",
            serde_json::to_string_pretty(other).unwrap_or_default()
        ),
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
    format!(
        "The forms every agent's return shares. A return is one JSON object. Its field `{w}` holds exactly one form: \
         the cell's own answer, as `format:<cell>` gives it, or one of the forms here that `format:<cell>` says the \
         cell has: {forms}. Every return also holds {every}{leaves}. As JSON Schema, each field of a return:\n\n\
         ```json\n{json}\n```",
        w = g.within,
        forms = quoted(&names),
        every = quoted(&every_names),
        json = Value::Object(schema),
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
        "The answer of cell {id} ({title}): what `{w}` holds when a round of this cell hands in its own answer, as JSON \
         Schema. Instead of it, a return of this cell may hold these forms of `format:common`: {others}.\n\n\
         ```json\n{json}\n```",
        id = cell.id,
        title = cell.title,
        w = g.within,
        others = others.join(", "),
        json = own,
    )
}

/// A return format with the value of each setting it names written beside the setting's name, such as the owner's
/// language beside `owner.language`: the compiled flow can name a setting, never its value, which the configuration
/// holds. The value is written as JSON writes it inside a string, since the format holds the schema as JSON.
fn with_settings(e: &Engine, project: &str, text: String) -> String {
    let mut out = text;
    for (name, value) in e.config.config.named_settings(project, &out) {
        let quoted = Value::String(value).to_string();
        let inner = &quoted[1..quoted.len() - 1];
        out = out.replace(&format!("`{name}`"), &format!("`{name}` ({inner})"));
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

/// The materials of a message as it is built: each content once, under its one address.
struct Materials<'a> {
    held: &'a Held,
    registry: &'a HashMap<String, Vec<String>>,
    list: Vec<Material>,
}

impl Materials<'_> {
    /// Adds a material, and gives the address the message refers to it by: one already added with the same content
    /// keeps its address.
    fn add(&mut self, address: String, content: Value) -> String {
        let h = hash(&content);
        let one = copied(&content);
        if let Some(m) = self.list.iter().find(|m| m.hash == h && (one || m.address == address)) {
            return m.address.clone();
        }
        let given = !self.held.holds(&address, &h);
        self.list.push(Material {
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
    fn computed(&self, m: &Materials<'_>, name: &str, content: &Value) -> String {
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

    /// The materials one source of an input gave, each with its address.
    fn materials_of(&self, m: &Materials<'_>, p: &Part) -> Vec<(String, Value)> {
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
                    "item_cards" | "parent_design_cards" => {
                        rows.into_iter().map(|r| (format!("card:{}", r["card"]), r)).collect()
                    }
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

    /// Why this round happens, and the materials that says it with.
    fn why(&self, m: &mut Materials<'_>, interjections: &[String]) -> Vec<String> {
        let c = self.cell;
        let mut out = Vec::new();
        let here = |r: &&Round| r.cell.as_deref() == Some(c.id.as_str());
        let earlier = self.rounds.iter().rev().find(here);
        if earlier.is_none() {
            out.push("This is the first round at this cell.".to_string());
        }
        match self.rounds.last() {
            Some(r) if here(&r) && r.transition.is_none() => {
                let rej = r.rejected.clone().unwrap_or(Value::Null);
                if rej.get("interrupted").is_some() {
                    out.push(format!(
                        "R{} at this cell did not end: the engine stopped while it ran. This round does its work again.",
                        r.id
                    ));
                } else if let Some(f) = rej.get("failed").or(rej.get("error")) {
                    let what = f.get("what").and_then(Value::as_str).or(f.as_str()).unwrap_or_default();
                    out.push(format!(
                        "R{} at this cell ended without a return: {what}. This round does its work again.",
                        r.id
                    ));
                } else {
                    let reason = rej
                        .as_object()
                        .and_then(|o| o.values().next())
                        .map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_string))
                        .unwrap_or_default();
                    out.push(format!(
                        "Your return to R{} was sent back: {reason}\nHand it in again, put right.",
                        r.id
                    ));
                }
            }
            Some(r) => {
                let row = r.transition.as_deref().unwrap_or_default();
                let from = r.cell.as_deref().unwrap_or_default();
                let paused = here(&r) && c.rows.iter().any(|x| x.name == row && x.to == "@self");
                if paused {
                    out.push(format!("You paused in R{}. Carry on.", r.id));
                } else {
                    let title = self.g.cell(from).map(|x| x.title.as_str()).unwrap_or_default();
                    out.push(format!(
                        "The work came here from cell {from} ({title}), R{}, by its row \"{row}\".",
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
                            out.push(format!("It comes back after asking; the answer: {a}."));
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
                "Said to you while this work runs: {}. Answer in this round as `format:common` says.",
                interjections.join(", ")
            ));
        }
        out
    }

    /// The message.
    pub fn build(&self) -> Message {
        let (e, w, g, c, item) = (self.e, self.w, self.g, self.cell, self.item);
        let mut m = Materials {
            held: self.held,
            registry: self.registry,
            list: Vec::new(),
        };
        // What every round needs from the start: the return formats (the query tool's description in them lists the
        // database's tables and views), with the value of each setting they name beside its name, and the rules and
        // the settings they name.
        m.add(
            "format:common".into(),
            Value::String(with_settings(e, &item.project, common_format(g))),
        );
        m.add(
            format!("format:{}", c.id),
            Value::String(with_settings(e, &item.project, cell_format(g, c))),
        );
        let mut rules = Vec::new();
        let mut settings: Vec<(String, String)> = Vec::new();
        let project = e.config.config.projects.get(&item.project);
        for name in project.map(|p| p.rules.clone()).unwrap_or_default() {
            let Some(text) = e.config.config.rules.get(&name) else {
                continue;
            };
            rules.push(m.add(format!("rule:{name}"), Value::String(text.trim().to_string())));
            for (k, v) in e.config.config.named_settings(&item.project, text) {
                if !settings.iter().any(|(x, _)| *x == k) {
                    settings.push((k, v));
                }
            }
        }
        for r in &c.rules {
            if let Some(rule) = g.rules.iter().find(|x| x.name == r.name) {
                let lines: Vec<String> = rule.text.iter().map(|t| format!("- {t}")).collect();
                let text = format!("{}\n\n{}", rule.desc, lines.join("\n"));
                rules.push(m.add(format!("rule:{}", rule.name), Value::String(text)));
            }
        }
        let settings: Vec<String> = settings
            .into_iter()
            .map(|(k, v)| m.add(format!("config:{k}"), Value::String(v)))
            .collect();
        // The inputs, each by the addresses of what it holds.
        let mut work_from = Vec::new();
        let mut interjections = Vec::new();
        for x in self.gathered {
            if x.itself() {
                work_from.push(format!("- {}: read it yourself.", x.input.what));
                continue;
            }
            let mut addrs: Vec<String> = Vec::new();
            for p in &x.parts {
                for (address, content) in self.materials_of(&m, p) {
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
                work_from.push(format!("- {}: {}", x.input.what, addrs.join(", ")));
            }
        }
        let why = self.why(&mut m, &interjections);

        let mut out = String::new();
        let mut push = |s: &str| {
            out.push_str(s);
            out.push('\n');
        };
        push(&format!("# R{} · cell {} · {}\n", self.round, c.id, c.title));
        if !why.is_empty() {
            push("## Why this round\n");
            push(&why.join("\n"));
            push("");
        }
        let given: Vec<&Material> = m.list.iter().filter(|x| x.given).collect();
        if !given.is_empty() {
            push("## Given in this message\n");
            for x in given {
                push(&format!("### {}\n", x.address));
                push(&render(&x.content));
                push("");
            }
        }
        push("## What to do\n");
        for p in &c.prompt {
            push(p);
        }
        if let Some(pb) = &c.prompt_by
            && let Some(v) = self.given.get(&pb.by).and_then(Value::as_str)
            && let Some(lines) = pb.cases.get(v)
        {
            for l in lines {
                push(l);
            }
        }
        push("");
        push("## Work from\n");
        for l in &work_from {
            push(l);
        }
        if !rules.is_empty() {
            push(&format!("- The rules: {}", rules.join(", ")));
        }
        if !settings.is_empty() {
            push(&format!(
                "- The settings the rules name, for this project: {}",
                settings.join(", ")
            ));
        }
        if let (Some(wt), Some(branch)) = (&w.worktree, &w.branch) {
            push(&format!(
                "\nYour working directory is this work's worktree, {}, on the branch {branch}. Commit your changes there; the engine pushes them when you ask it to with a tool.",
                wt.display()
            ));
        }
        if let Some(p) = project {
            push(&format!(
                "Main, wherever the flow says it, is this project's branch {}: the work branches from it, its PR merges into it, and a diff against main is a diff against it.",
                p.main
            ));
        }
        push("");
        if !self.columns.is_empty() {
            push("## The review table: answer one row for each column, by its number\n");
            for (id, col) in self.columns {
                push(&format!("{id}. {col}"));
            }
            push("");
        }
        push("## Hand in\n");
        push(&format!(
            "One JSON object, and nothing else: this cell's own answer as format:{id} gives it, or one of the forms of format:common that format:{id} says this cell has.",
            id = c.id
        ));
        Message {
            text: out,
            materials: m.list,
        }
    }
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
        let given = |a: &str, v: &Value| json!({ "materials": [{ "address": a, "hash": hash(v), "given": true }] });
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

    // The same content copied from one place to another is one material under the earliest round's address; a word
    // two places give alike keeps each place's address.
    #[test]
    fn copied_content_has_one_address_and_a_word_keeps_its_own() {
        let held = Held::default();
        let acceptance = json!([{ "id": "A1", "condition": "It works." }]);
        let registry = HashMap::from([
            (hash(&acceptance), vec!["R25·12·reply.new_acceptance".to_string()]),
            (hash(&json!("no")), vec!["R7·engine·adversarial".to_string()]),
        ]);
        let mut m = Materials {
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
