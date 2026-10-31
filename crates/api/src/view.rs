//! What the flow page shows of a compiled flow besides the flow itself: the cells each row can lead to and the rows
//! that lead into each cell; what a round of each cell is given, each input with its source and its address as
//! the engine addresses it; and each cell's return as a tree of fields, with whatever reads each field.
//!
//! A row's target the engine keeps track of is resolved to the cells it can be from the rows alone: `@caller` is a
//! cell with a row into the cell, `@asker` the same, except that an answer to a cell on the line of the graph's
//! receiving cell goes to that cell, and `@stopped` a cell outside the asking part with a row into the receiving
//! cell (into the asking part, when the graph has no receiving cell).
//!
//! Besides, the lines of the flow built into this program, which every part of the page that shows a role reads.

use std::collections::{BTreeMap, BTreeSet};

use flow::format::{Atom, Do, Src};
use flow::{Compiled, CompiledCell, CompiledGraph};
use serde_json::{Map, Value};

use crate::{
    Branch, CellInput, CellInputOf, CellView, Field, FieldKind, FlowView, GraphLines, GraphView, LineView, ReturnForm,
    Returns, RowRef, Use,
};

/// The targets the engine keeps track of, as rows and inputs name them.
const SELF: &str = "@self";
const CALLER: &str = "@caller";
const ASKER: &str = "@asker";
const STOPPED: &str = "@stopped";
const FROM: &str = "@from";

pub fn view(c: &Compiled) -> FlowView {
    FlowView {
        graphs: c.graphs.iter().map(graph).collect(),
    }
}

/// The lines of each graph of the flow built into this program, with what each is and the colour its role is drawn in
/// wherever the page shows it: a compiled flow version keeps the lines, not what they are, and a role has one colour
/// on every page, whatever flow version a work runs on.
pub fn flow_lines() -> Vec<GraphLines> {
    // The flow built into this program compiles, every line of it with a colour, as its tests hold; a source that does
    // not read has no lines, and a line with no colour is not given.
    let Ok(source) = serde_json::from_str::<flow::format::Source>(flow::FLOW) else {
        return Vec::new();
    };
    source
        .flows
        .iter()
        .map(|f| GraphLines {
            graph: f.name.clone(),
            lines: f
                .sessions
                .lines
                .iter()
                .filter_map(|l| {
                    Some(LineView {
                        name: l.name.clone(),
                        desc: l.desc.clone(),
                        colour: l.colour?,
                    })
                })
                .collect(),
        })
        .collect()
}

fn graph(g: &CompiledGraph) -> GraphView {
    let ways = Ways::new(g);
    let mut trees: BTreeMap<&str, Returns> = g.cells.iter().map(|c| (c.id.as_str(), returns(g, c))).collect();
    let mut uses: Vec<(String, String, Use)> = Vec::new();
    for c in &g.cells {
        uses.extend(ways.uses(c, &trees));
    }
    for (cell, path, u) in uses {
        if let Some(t) = trees.get_mut(cell.as_str()) {
            t.attach(&path, &u);
        }
    }
    let mut entered: BTreeMap<&str, Vec<RowRef>> = BTreeMap::new();
    for c in &g.cells {
        for (i, targets) in ways.rows(c).into_iter().enumerate() {
            for t in targets {
                let list = entered.entry(ways.id(&t)).or_default();
                let r = RowRef {
                    cell: c.id.clone(),
                    row: i,
                };
                if !list.contains(&r) {
                    list.push(r);
                }
            }
        }
    }
    let inputs: Vec<Vec<CellInput>> = g.cells.iter().map(|c| ways.inputs(c, &trees)).collect();
    let cells = g
        .cells
        .iter()
        .zip(inputs)
        .map(|(c, inputs)| CellView {
            id: c.id.clone(),
            targets: ways.rows(c),
            entered_from: entered.remove(c.id.as_str()).unwrap_or_default(),
            inputs,
            returns: trees.remove(c.id.as_str()).unwrap_or_else(Returns::empty),
        })
        .collect();
    GraphView {
        name: g.name.clone(),
        cells,
        common_format: flow::formats::common_format(g, &[]),
    }
}

/// The rows of a graph, and where they lead.
struct Ways<'a> {
    g: &'a CompiledGraph,
    /// For each cell, the cells with a row whose target is that cell by name, in the graph's order.
    into: BTreeMap<&'a str, Vec<&'a str>>,
    /// For each cell, the cells any of whose rows can lead to it, `@`-targets resolved.
    from: BTreeMap<String, Vec<String>>,
}

impl<'a> Ways<'a> {
    fn new(g: &'a CompiledGraph) -> Ways<'a> {
        let mut into: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for c in &g.cells {
            for r in &c.rows {
                if r.to != c.id && g.cell(&r.to).is_some() {
                    let list = into.entry(r.to.as_str()).or_default();
                    if !list.contains(&c.id.as_str()) {
                        list.push(c.id.as_str());
                    }
                }
            }
        }
        let mut w = Ways {
            g,
            into,
            from: BTreeMap::new(),
        };
        let mut from: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for c in &g.cells {
            for t in w.rows(c).into_iter().flatten() {
                let list = from.entry(t).or_default();
                if !list.contains(&c.id) {
                    list.push(c.id.clone());
                }
            }
        }
        w.from = from;
        w
    }

    /// The id as the graph holds it, for a borrowed key.
    fn id(&self, id: &str) -> &'a str {
        self.g.cell(id).map_or("", |c| c.id.as_str())
    }

    fn cell(&self, id: &str) -> Option<&'a CompiledCell> {
        self.g.cell(id)
    }

    fn asking(&self, id: &str) -> bool {
        self.g.asking.iter().any(|a| a == id)
    }

    /// The cells with a row whose target is `id` by name.
    fn callers(&self, id: &str) -> Vec<String> {
        self.into.get(id).into_iter().flatten().map(|s| s.to_string()).collect()
    }

    /// The cells an answer from `id` goes back to: each cell that went there, except that one on the line of the
    /// receiving cell is answered through it, and one that passed the work on to it is answered in its place: its own
    /// askers are.
    fn askers(&self, id: &str) -> Vec<String> {
        self.askers_past(id, &mut BTreeSet::new())
    }

    fn askers_past(&self, id: &str, seen: &mut BTreeSet<String>) -> Vec<String> {
        if !seen.insert(id.to_string()) {
            return Vec::new();
        }
        let receive = self.g.receive.as_deref().and_then(|r| self.cell(r));
        let mut out: Vec<String> = Vec::new();
        for c in self.callers(id) {
            let rows: Vec<bool> = self
                .cell(&c)
                .map(|x| x.rows.iter().filter(|r| r.to == id).map(|r| r.passes).collect())
                .unwrap_or_default();
            if rows.iter().any(|p| *p) {
                for a in self.askers_past(&c, seen) {
                    if !out.contains(&a) {
                        out.push(a);
                    }
                }
            }
            if !rows.is_empty() && rows.iter().all(|p| *p) {
                continue;
            }
            let through = receive.filter(|r| {
                r.id != c
                    && !self.asking(&c)
                    && r.line.is_some()
                    && self.cell(&c).and_then(|x| x.line.as_ref()) == r.line.as_ref()
            });
            let to = through.map_or(c, |r| r.id.clone());
            if !out.contains(&to) {
                out.push(to);
            }
        }
        out
    }

    /// The cells a work can have stopped at before it went into the asking part.
    fn stopped(&self) -> Vec<String> {
        let into: Vec<&str> = match &self.g.receive {
            Some(r) => vec![r.as_str()],
            None => self.g.asking.iter().map(String::as_str).collect(),
        };
        self.g
            .cells
            .iter()
            .filter(|c| !self.asking(&c.id))
            .filter(|c| c.rows.iter().any(|r| into.contains(&r.to.as_str())))
            .map(|c| c.id.clone())
            .collect()
    }

    /// The cells a row's target can be, for a row of `c`.
    fn target(&self, c: &CompiledCell, to: &str) -> Vec<String> {
        match to {
            SELF => vec![c.id.clone()],
            CALLER => self.callers(&c.id),
            ASKER => self.askers(&c.id),
            STOPPED => self.stopped(),
            id if self.cell(id).is_some() => vec![id.to_string()],
            _ => Vec::new(),
        }
    }

    /// For each row of `c`, the cells it can lead to.
    fn rows(&self, c: &CompiledCell) -> Vec<Vec<String>> {
        c.rows.iter().map(|r| self.target(c, &r.to)).collect()
    }

    /// The cells a place an input of `c` reads can be in: the cell it names, or the cells a target the engine keeps
    /// track of can be that have the place.
    fn place(&self, c: &CompiledCell, at: &str, path: &str, trees: &BTreeMap<&str, Returns>) -> Vec<String> {
        let candidates = match at {
            FROM => self.from.get(&c.id).cloned().unwrap_or_default(),
            CALLER => self.callers(&c.id),
            STOPPED => self.stopped(),
            id => return vec![id.to_string()],
        };
        candidates
            .into_iter()
            .filter(|x| trees.get(x.as_str()).is_some_and(|t| t.has(path)))
            .collect()
    }

    /// The places of returns a path a cell reads names: its own return's, or for `@<id>.<rest>`, the places the
    /// cell's input `<id>` reads, with `<rest>` after them.
    fn resolve(&self, c: &CompiledCell, path: &str, trees: &BTreeMap<&str, Returns>) -> Vec<(String, String)> {
        let Some(rest) = path.strip_prefix('@') else {
            return vec![(c.id.clone(), path.to_string())];
        };
        let (id, tail) = rest.split_once('.').unwrap_or((rest, ""));
        let Some(input) = c.inputs.iter().find(|x| x.id.as_deref() == Some(id)) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for s in &input.from {
            if let Src::Place(at, p) = s {
                let full = if tail.is_empty() {
                    p.clone()
                } else {
                    format!("{p}.{tail}")
                };
                for x in self.place(c, at, &full, trees) {
                    out.push((x, full.clone()));
                }
            }
        }
        out
    }

    /// What `c` reads of the returns of the graph's cells, each as the cell, the path and the use.
    fn uses(&self, c: &CompiledCell, trees: &BTreeMap<&str, Returns>) -> Vec<(String, String, Use)> {
        let mut out = Vec::new();
        for (i, x) in c.inputs.iter().enumerate() {
            for s in &x.from {
                if let Src::Place(at, p) = s {
                    for cell in self.place(c, at, p, trees) {
                        out.push((
                            cell,
                            p.clone(),
                            Use::Input {
                                cell: c.id.clone(),
                                input: i,
                            },
                        ));
                    }
                }
            }
        }
        let read = |path: &str, u: Use, out: &mut Vec<(String, String, Use)>| {
            for (cell, p) in self.resolve(c, path, trees) {
                out.push((cell, p, u.clone()));
            }
        };
        for (i, r) in c.rows.iter().enumerate() {
            let cond = Use::Condition {
                cell: c.id.clone(),
                row: i,
            };
            for a in &r.conditions {
                for p in atom_paths(a) {
                    read(p, cond.clone(), &mut out);
                }
                if atom_own(a) {
                    out.push((c.id.clone(), OWN.to_string(), cond.clone()));
                }
            }
            for d in &r.actions {
                if let Do::At(x) = d {
                    let u = Use::Action {
                        cell: c.id.clone(),
                        row: i,
                        action: x.action.clone(),
                    };
                    read(&x.at, u, &mut out);
                }
            }
        }
        for a in &c.always {
            let u = Use::Always {
                cell: c.id.clone(),
                view: a.view.clone(),
                action: a.action.clone(),
            };
            read(&a.at, u, &mut out);
        }
        for k in &c.checks {
            let u = Use::Check {
                cell: c.id.clone(),
                check: k.check.clone(),
            };
            match &k.at {
                Some(at) => read(at, u, &mut out),
                None => out.push((c.id.clone(), WHOLE.to_string(), u)),
            }
        }
        for h in &c.handoff {
            read(h, Use::Handoff { cell: c.id.clone() }, &mut out);
        }
        for p in &c.shown {
            read(p, Use::Shown { cell: c.id.clone() }, &mut out);
        }
        for x in &c.read_by {
            let u = Use::Role {
                cell: c.id.clone(),
                role: x.role.clone(),
            };
            read(&x.at, u, &mut out);
        }
        out
    }

    /// What a round of `c` is given: for an agent's cell, the return formats, what each numbered input is, the rules
    /// and the worktree, then the sources of its inputs; for any other cell, the sources of its inputs.
    fn inputs(&self, c: &CompiledCell, trees: &BTreeMap<&str, Returns>) -> Vec<CellInput> {
        let agent = c.kind == AGENT;
        let mut out = Vec::new();
        let given = |of: CellInputOf, address: String| CellInput {
            always: false,
            of,
            address: Some(address),
            cells: Vec::new(),
            holds: None,
        };
        if agent {
            out.push(given(CellInputOf::CommonFormat, "format:common".into()));
            out.push(CellInput {
                holds: Some(flow::formats::cell_format(self.g, c, &[])),
                ..given(CellInputOf::CellFormat, format!("format:{}", c.id))
            });
            if c.inputs.iter().any(|x| x.from.iter().any(|s| *s != Src::Itself)) {
                out.push(CellInput {
                    holds: Some(flow::formats::cell_inputs(c)),
                    ..given(CellInputOf::CellInputs, format!("inputs:{}", c.id))
                });
            }
            out.push(given(CellInputOf::ProjectRules, "rule:‹name›".into()));
            for r in &c.rules {
                out.push(given(
                    CellInputOf::Rule { name: r.name.clone() },
                    format!("rule:{}", r.name),
                ));
            }
            // A cell of an architect's on no line knows only what its inputs hold, and is told of no working directory.
            let blind = c.line.is_none() && matches!(self.g.name.as_str(), "architect" | "policy");
            if !blind {
                out.push(given(CellInputOf::Workspace, "work:‹id›".into()));
            }
        }
        for (i, x) in c.inputs.iter().enumerate() {
            for (j, s) in x.from.iter().enumerate() {
                let (address, cells) = self.source(c, s, trees);
                // A numbered table is one input the engine makes of all its sources.
                let address = match &x.numbered {
                    Some(_) => Some("R‹round›·engine·table".to_string()),
                    None => address,
                };
                out.push(CellInput {
                    always: *s == Src::Itself,
                    of: CellInputOf::Input { input: i, source: j },
                    address,
                    cells,
                    holds: None,
                });
            }
        }
        out
    }

    /// The address the engine gives what a source of an input of `c` holds under, and the cells a place it reads can
    /// be in; no address for what the agent reads itself. The addresses are those the engine's message gives
    /// (`conditional_of` in the engine's message builder): a value a round produced is addressed by the round, the
    /// cell and the path; what the engine reads or computes for a round by the round and its name; the item, its
    /// design cards, its asks of the owner and the interjections by their ids; and the locked acceptance and the
    /// researched problem by the round that wrote them.
    fn source(&self, c: &CompiledCell, s: &Src, trees: &BTreeMap<&str, Returns>) -> (Option<String>, Vec<String>) {
        let at = |address: String| (Some(address), Vec::new());
        match s {
            Src::Itself => (None, Vec::new()),
            Src::Config(n) => at(format!("config:{n}")),
            Src::Engine(n) | Src::Recorded(n) => at(format!("R‹work›-‹n›·engine·{n}")),
            Src::Db { relation, .. } => match relation.as_str() {
                "item_body" => at("item:‹id›".into()),
                "item_design_cards" | "parent_design_cards" | "work_design_card" => at("design_card:‹id›".into()),
                "item_asks" => at("ask:‹id›".into()),
                "waiting_interjections" => at("ij:‹id›".into()),
                "item_acceptance" => at("R‹work›-‹n›·‹cell›·acceptance".into()),
                "item_problem" => at("R‹work›-‹n›·R1·understanding.problem".into()),
                other => at(format!("R‹work›-‹n›·engine·{other}")),
            },
            Src::Place(at, path) => {
                let cells = self.place(c, at, path, trees);
                let address = if at.starts_with('@') {
                    format!("R‹work›-‹n›·‹cell›·{path}")
                } else {
                    format!("R‹work›-‹n›·{at}·{path}")
                };
                (Some(address), cells)
            }
        }
    }
}

const AGENT: &str = "agent";
/// The paths uses of the own answer as a whole and of the whole return are attached by.
const OWN: &str = "\u{0}own";
const WHOLE: &str = "\u{0}whole";

/// The places of the return a condition reads; none for a value the engine knows or rows recorded.
fn atom_paths(a: &Atom) -> Vec<&str> {
    match a {
        Atom::All | Atom::Own | Atom::Inp(..) | Atom::SentBack(_) | Atom::Failed(_) | Atom::Engine(_) => Vec::new(),
        Atom::Or(xs) => xs.iter().flat_map(atom_paths).collect(),
        Atom::Has(p) | Atom::Is(p, _) | Atom::AnyOf(p, _) | Atom::NoneOf(p, _) => vec![p.as_str()],
    }
}

/// Whether a condition reads that the return is the cell's own answer.
fn atom_own(a: &Atom) -> bool {
    match a {
        Atom::Own => true,
        Atom::Or(xs) => xs.iter().any(atom_own),
        _ => false,
    }
}

/// One step of a path: a field's name, and the shape it is taken in when the path names one.
struct Seg<'a> {
    name: &'a str,
    branch: Option<&'a str>,
}

fn segments(path: &str) -> Vec<Seg<'_>> {
    path.split('.')
        .map(|s| match s.split_once('[') {
            Some((name, b)) => Seg {
                name,
                branch: Some(b.trim_end_matches(']')),
            },
            None => Seg { name: s, branch: None },
        })
        .collect()
}

fn add(list: &mut Vec<Use>, u: &Use) {
    if !list.contains(u) {
        list.push(u.clone());
    }
}

impl Returns {
    fn empty() -> Returns {
        Returns {
            within: None,
            forms: Vec::new(),
            every: Vec::new(),
            used_by: Vec::new(),
        }
    }

    /// Whether the return has a field at `path`.
    fn has(&self, path: &str) -> bool {
        let segs = segments(path);
        self.forms.iter().any(|f| has(&f.fields, &segs)) || has(&self.every, &segs)
    }

    /// Attaches a use to the field at `path`, or to the deepest field on the way to it that the return has.
    fn attach(&mut self, path: &str, u: &Use) {
        if path == WHOLE {
            add(&mut self.used_by, u);
            return;
        }
        if path == OWN {
            if let Some(f) = self.forms.iter_mut().find(|f| f.field.is_none()) {
                add(&mut f.used_by, u);
            }
            return;
        }
        let segs = segments(path);
        for f in &mut self.forms {
            if attach(&mut f.fields, &segs, u) {
                return;
            }
        }
        if !attach(&mut self.every, &segs, u) {
            add(&mut self.used_by, u);
        }
    }
}

fn has(fields: &[Field], segs: &[Seg<'_>]) -> bool {
    let Some((first, rest)) = segs.split_first() else {
        return true;
    };
    let Some(f) = fields.iter().find(|f| f.name == first.name) else {
        return false;
    };
    if let Some(b) = first.branch {
        return match f.branches.iter().find(|x| x.value == b) {
            Some(br) => has(&br.fields, rest) || has(&f.common, rest),
            None => false,
        };
    }
    if rest.is_empty() {
        return true;
    }
    if rest.len() == 1 && rest[0].branch.is_none() && f.disc.as_deref() == Some(rest[0].name) {
        return true;
    }
    has(&f.fields, rest) || has(&f.common, rest) || f.branches.iter().any(|b| has(&b.fields, rest))
}

/// Attaches a use to the field at `segs`, or to the deepest field on the way; false when not even the first is there.
fn attach(fields: &mut [Field], segs: &[Seg<'_>], u: &Use) -> bool {
    let Some((first, rest)) = segs.split_first() else {
        return false;
    };
    let Some(f) = fields.iter_mut().find(|f| f.name == first.name) else {
        return false;
    };
    if let Some(b) = first.branch
        && let Some(i) = f.branches.iter().position(|x| x.value == b)
    {
        if !rest.is_empty() && has(&f.branches[i].fields, rest) {
            attach(&mut f.branches[i].fields, rest, u);
        } else if !rest.is_empty() && has(&f.common, rest) {
            attach(&mut f.common, rest, u);
        } else {
            add(&mut f.branches[i].used_by, u);
        }
        return true;
    }
    if rest.is_empty() {
        add(&mut f.used_by, u);
        return true;
    }
    if rest.len() == 1 && rest[0].branch.is_none() && f.disc.as_deref() == Some(rest[0].name) {
        add(&mut f.disc_used_by, u);
        return true;
    }
    if has(&f.fields, rest) {
        attach(&mut f.fields, rest, u);
        return true;
    }
    if has(&f.common, rest) {
        attach(&mut f.common, rest, u);
        return true;
    }
    // A path into fields some shapes have: each shape that has it.
    let mut any = false;
    for b in &mut f.branches {
        if has(&b.fields, rest) {
            attach(&mut b.fields, rest, u);
            any = true;
        }
    }
    if !any {
        add(&mut f.used_by, u);
    }
    true
}

/// A cell's return as a tree of fields, from its schema: for an agent's cell, each form it may hand in, then the
/// fields every return holds beside the form.
pub(crate) fn returns(g: &CompiledGraph, c: &CompiledCell) -> Returns {
    let mut out = Returns::empty();
    let Some(schema) = c.schema.as_ref() else {
        return out;
    };
    if c.kind != AGENT {
        out.every = fields(schema, "");
        return out;
    }
    out.within = Some(g.within.clone());
    let alternatives = schema["properties"][&g.within]["anyOf"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for (f, alt) in c.forms.iter().zip(&alternatives) {
        out.forms.push(ReturnForm {
            field: f.field.clone(),
            fields: fields(alt, ""),
            used_by: Vec::new(),
        });
    }
    let mut beside = schema.clone();
    if let Some(p) = beside.get_mut("properties").and_then(Value::as_object_mut) {
        p.remove(&g.within);
    }
    out.every = fields(&beside, "");
    out
}

/// The fields of an object's schema, under `prefix`.
fn fields(schema: &Value, prefix: &str) -> Vec<Field> {
    let required: BTreeSet<&str> = schema["required"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let empty = Map::new();
    let props = schema["properties"].as_object().unwrap_or(&empty);
    props
        .iter()
        .map(|(name, s)| {
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}.{name}")
            };
            field(name, s, path, !required.contains(name.as_str()))
        })
        .collect()
}

fn kind(s: &Value) -> FieldKind {
    if s.get("oneOf").is_some() {
        return FieldKind::Union;
    }
    if s.get("enum").is_some() || s.get("const").is_some() {
        return FieldKind::Enum;
    }
    let t = match &s["type"] {
        Value::String(t) => t.as_str(),
        Value::Array(ts) => ts.iter().filter_map(Value::as_str).find(|t| *t != "null").unwrap_or(""),
        _ => "",
    };
    match t {
        "string" => FieldKind::Text,
        "integer" => FieldKind::Integer,
        "number" => FieldKind::Number,
        "boolean" => FieldKind::Boolean,
        "object" => FieldKind::Object,
        "array" => FieldKind::List,
        _ => FieldKind::Any,
    }
}

fn text(s: &Value, key: &str) -> Option<String> {
    s.get(key).and_then(Value::as_str).map(str::to_string)
}

fn field(name: &str, s: &Value, path: String, optional: bool) -> Field {
    let k = kind(s);
    let mut f = Field {
        name: name.to_string(),
        path: path.clone(),
        kind: k,
        item: None,
        desc: text(s, "description"),
        optional,
        min: None,
        values: Vec::new(),
        used_by: Vec::new(),
        fields: Vec::new(),
        disc: None,
        disc_used_by: Vec::new(),
        common: Vec::new(),
        branches: Vec::new(),
    };
    let shape = if k == FieldKind::List {
        f.min = s["minItems"].as_u64();
        let items = &s["items"];
        f.item = Some(kind(items));
        if f.desc.is_none() {
            f.desc = text(items, "description");
        }
        items
    } else {
        s
    };
    match kind(shape) {
        FieldKind::Object => f.fields = fields(shape, &path),
        FieldKind::Union => union(&mut f, shape, &path),
        FieldKind::Enum => {
            let values: Vec<String> = match shape.get("const") {
                Some(Value::String(v)) => vec![v.clone()],
                _ => shape["enum"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect(),
            };
            // The values are said in the description of what holds them: the field, or a list's items.
            let says = text(shape, "description");
            let (rest, meant) = split_values(says.as_deref(), &values);
            if f.desc == says {
                f.desc = rest;
            }
            f.values = values.into_iter().zip(meant).collect();
        }
        _ => {}
    }
    f
}

/// One of several object shapes: the field every shape names with a fixed value tells them apart; the fields every
/// shape has alike are given once.
fn union(f: &mut Field, s: &Value, path: &str) {
    let alts: Vec<&Value> = s["oneOf"].as_array().into_iter().flatten().collect();
    let consts = |a: &Value| -> BTreeSet<String> {
        a["properties"]
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(_, v)| v.get("const").is_some())
            .map(|(k, _)| k.clone())
            .collect()
    };
    let first_consts = match alts.first() {
        Some(a) => consts(a),
        None => BTreeSet::new(),
    };
    let disc = first_consts
        .into_iter()
        .find(|k| alts.iter().all(|a| consts(a).contains(k)));
    let Some(disc) = disc else {
        // Shapes nothing tells apart: each by its place.
        for (i, a) in alts.iter().enumerate() {
            let value = (i + 1).to_string();
            let bp = format!("{path}[{value}]");
            f.branches.push(Branch {
                value,
                path: bp.clone(),
                desc: text(a, "description"),
                used_by: Vec::new(),
                fields: fields(a, &bp),
            });
        }
        return;
    };
    let props = |a: &Value| a["properties"].as_object().cloned().unwrap_or_default();
    let first = match alts.first() {
        Some(a) => props(a),
        None => Map::new(),
    };
    let common: Vec<String> = first
        .iter()
        .filter(|(k, v)| **k != disc && alts.iter().all(|a| props(a).get(*k) == Some(*v)))
        .map(|(k, _)| k.clone())
        .collect();
    if let Some(a) = alts.first() {
        let mut shared = (*a).clone();
        if let Some(p) = shared.get_mut("properties").and_then(Value::as_object_mut) {
            p.retain(|k, _| common.contains(k));
        }
        f.common = fields(&shared, path);
    }
    for a in &alts {
        let value = a["properties"][&disc]["const"]
            .as_str()
            .map_or_else(|| a["properties"][&disc]["const"].to_string(), str::to_string);
        let bp = format!("{path}[{value}]");
        let mut own = (*a).clone();
        if let Some(p) = own.get_mut("properties").and_then(Value::as_object_mut) {
            p.retain(|k, _| *k != disc && !common.contains(k));
        }
        f.branches.push(Branch {
            value,
            path: bp.clone(),
            desc: text(&a["properties"][&disc], "description"),
            used_by: Vec::new(),
            fields: fields(&own, &bp),
        });
    }
    f.disc = Some(disc);
}

/// A description with the meaning of each value folded into it, as the compiler writes it (`<desc>. Values: <value>:
/// <means>; …`), taken apart: the description before, and each value's meaning. A description written otherwise is
/// kept whole, and no value's meaning is known.
fn split_values(desc: Option<&str>, values: &[String]) -> (Option<String>, Vec<Option<String>>) {
    let none = || (desc.map(str::to_string), vec![None; values.len()]);
    let (Some(d), Some(first)) = (desc, values.first()) else {
        return none();
    };
    let marker = format!("Values: {first}: ");
    let Some(at) = d.rfind(&marker) else {
        return none();
    };
    let before = d[..at].trim_end().trim_end_matches('.').trim_end();
    let mut rest = &d[at + "Values: ".len()..];
    let mut meant = Vec::new();
    for (i, v) in values.iter().enumerate() {
        let Some(after) = rest.strip_prefix(&format!("{v}: ")) else {
            return none();
        };
        let end = match values.get(i + 1) {
            Some(next) => match after.find(&format!("; {next}: ")) {
                Some(e) => e,
                None => return none(),
            },
            None => after.len(),
        };
        meant.push(Some(after[..end].to_string()));
        rest = after[end..].strip_prefix("; ").unwrap_or("");
    }
    let before = (!before.is_empty()).then(|| before.to_string());
    (before, meant)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn graph() -> CompiledGraph {
        let agent = |id: &str, line: &str, rows: Value, inputs: Value, own: Value| {
            json!({
                "id": id, "kind": "agent", "gate": false, "title": format!("cell {id}"), "line": line,
                "inputs": inputs, "prompt_by": null,
                "forms": [{"own": true, "say": "own"}, {"field": "tools", "say": "`tools`"}],
                "schema": {
                    "type": "object",
                    "properties": {
                        "outcome": {"anyOf": [
                            own,
                            {"type": "object", "properties": {"tools": {"type": "array", "items": {"type": "string"}}},
                             "required": ["tools"]}
                        ]},
                        "progress": {"type": "string", "description": "What it did"}
                    },
                    "required": ["outcome"]
                },
                "rows": rows,
            })
        };
        let row = |name: &str, cond: Value, to: &str| {
            json!({"name": name, "if": cond, "to": to, "waits": false, "interject": false, "main": false,
                   "common": false})
        };
        let g = json!({
            "name": "g", "start": "a", "end": "z", "within": "outcome", "asking": ["q"], "receive": "q",
            "counters": [], "rules": [], "interject": null,
            "cells": [
                agent("a", "eng",
                    json!([row("done", json!([["own"]]), "b"), row("tools", json!([["has", "tools"]]), "t"),
                           row("ask", json!([["has", "tools"]]), "q")]),
                    json!([{"what": "tool results", "from": [["@from", "results"]], "maybe": true}]),
                    json!({"type": "object", "properties": {
                        "design": {"type": "string", "description": "The design"},
                        "verdict": {"enum": ["yes", "no"], "description": "Whether. Values: yes: it holds; no: it does not"}
                    }, "required": ["design", "verdict"]})),
                agent("b", "rev",
                    json!([row("done", json!([["is", "@v", ["yes"]]]), "z")]),
                    json!([{"what": "the design", "from": [["a", "design"]], "maybe": false},
                           {"id": "v", "what": "the verdict", "from": [["a", "verdict"]], "maybe": false},
                           {"what": "the item", "from": [{"db": "item_body", "by": "item"}], "maybe": false}]),
                    json!({"type": "object", "properties": {}, "required": []})),
                {"id": "t", "kind": "tool", "gate": false, "title": "Run tools", "inputs": [
                    {"what": "the tools", "from": [["@caller", "tools"]], "maybe": false}], "prompt_by": null,
                 "schema": {"type": "object", "properties": {"results": {"type": "array", "items": {
                    "oneOf": [
                        {"type": "object", "properties": {"kind": {"const": "ok", "description": "It ran"},
                            "out": {"type": "string"}}, "required": ["kind", "out"]},
                        {"type": "object", "properties": {"kind": {"const": "failed", "description": "It failed"},
                            "why": {"type": "string"}}, "required": ["kind", "why"]}
                    ]}}}, "required": ["results"]},
                 "rows": [row("ran", json!([["all"]]), "@caller")]},
                {"id": "q", "kind": "engine", "gate": false, "title": "Answer", "inputs": [], "prompt_by": null,
                 "schema": null, "rows": [row("answered", json!([["all"]]), "@asker")]},
                {"id": "z", "kind": "engine", "gate": false, "title": "End", "inputs": [], "prompt_by": null,
                 "schema": null, "rows": []}
            ]
        });
        serde_json::from_value(g).unwrap()
    }

    fn cell<'a>(v: &'a GraphView, id: &str) -> &'a CellView {
        v.cells.iter().find(|c| c.id == id).unwrap()
    }

    fn field<'a>(fs: &'a [Field], name: &str) -> &'a Field {
        fs.iter().find(|f| f.name == name).unwrap()
    }

    #[test]
    fn targets_the_engine_keeps_track_of_resolve_to_the_cells_they_can_be() {
        let v = super::graph(&graph());
        // The tool cell goes back to the cell that called it.
        assert_eq!(cell(&v, "t").targets, vec![vec!["a".to_string()]]);
        // An answer to a cell on the receiving cell's line would go through it; `a` is not on its line.
        assert_eq!(cell(&v, "q").targets, vec![vec!["a".to_string()]]);
        let from: Vec<(String, usize)> = cell(&v, "a")
            .entered_from
            .iter()
            .map(|r| (r.cell.clone(), r.row))
            .collect();
        assert_eq!(from, vec![("t".to_string(), 0), ("q".to_string(), 0)]);
        assert_eq!(
            cell(&v, "z").entered_from,
            vec![RowRef {
                cell: "b".into(),
                row: 0
            }]
        );
    }

    #[test]
    fn each_field_says_what_reads_it() {
        let v = super::graph(&graph());
        let a = &cell(&v, "a").returns;
        let own = &a.forms[0];
        assert_eq!(
            own.used_by,
            vec![Use::Condition {
                cell: "a".into(),
                row: 0
            }]
        );
        assert_eq!(
            field(&own.fields, "design").used_by,
            vec![Use::Input {
                cell: "b".into(),
                input: 0
            }]
        );
        let verdict = field(&own.fields, "verdict");
        assert_eq!(
            verdict.used_by,
            vec![
                Use::Input {
                    cell: "b".into(),
                    input: 1
                },
                Use::Condition {
                    cell: "b".into(),
                    row: 0
                }
            ]
        );
        assert_eq!(verdict.desc.as_deref(), Some("Whether"));
        assert_eq!(
            verdict.values,
            vec![
                ("yes".into(), Some("it holds".into())),
                ("no".into(), Some("it does not".into()))
            ]
        );
        // The tools form is read by the rows that test it, and by the tool cell, which reads the caller's tools.
        let tools = field(&a.forms[1].fields, "tools");
        assert!(tools.used_by.contains(&Use::Input {
            cell: "t".into(),
            input: 0
        }));
        assert!(tools.used_by.contains(&Use::Condition {
            cell: "a".into(),
            row: 1
        }));
        // The tool cell's results are one of two shapes; the cell the work came from reads them.
        let t = &cell(&v, "t").returns;
        let results = field(&t.every, "results");
        assert_eq!(results.kind, FieldKind::List);
        assert_eq!(results.item, Some(FieldKind::Union));
        assert_eq!(results.disc.as_deref(), Some("kind"));
        assert_eq!(results.branches.len(), 2);
        assert_eq!(results.branches[1].path, "results[failed]");
        assert_eq!(
            results.used_by,
            vec![Use::Input {
                cell: "a".into(),
                input: 0
            }]
        );
    }

    #[test]
    fn an_input_has_its_address() {
        let v = super::graph(&graph());
        let b = cell(&v, "b");
        let input = |i: usize, s: usize| {
            b.inputs
                .iter()
                .find(|m| m.of == CellInputOf::Input { input: i, source: s })
                .unwrap()
        };
        assert_eq!(input(0, 0).address.as_deref(), Some("R‹work›-‹n›·a·design"));
        assert!(!input(2, 0).always);
        assert_eq!(input(2, 0).address.as_deref(), Some("item:‹id›"));
        assert!(
            b.inputs
                .iter()
                .any(|m| m.of == CellInputOf::CellFormat && m.address.as_deref() == Some("format:b"))
        );
        let a = cell(&v, "a");
        let results = a
            .inputs
            .iter()
            .find(|m| m.of == CellInputOf::Input { input: 0, source: 0 })
            .unwrap();
        assert_eq!(results.cells, vec!["t".to_string()]);
        assert_eq!(results.address.as_deref(), Some("R‹work›-‹n›·‹cell›·results"));
    }

    /// Every use attached anywhere in a list of fields.
    fn all_uses(fs: &[Field], out: &mut Vec<Use>) {
        for f in fs {
            out.extend(f.used_by.iter().cloned());
            out.extend(f.disc_used_by.iter().cloned());
            all_uses(&f.fields, out);
            all_uses(&f.common, out);
            for b in &f.branches {
                out.extend(b.used_by.iter().cloned());
                all_uses(&b.fields, out);
            }
        }
    }

    #[test]
    fn the_flow_explained_agrees_with_its_rows_and_inputs() {
        let relations = db::relations(db::SCHEMA).unwrap();
        let c = flow::compile(
            flow::FLOW,
            &flow::Context {
                relations,
                tables: db::index_of(db::SCHEMA).unwrap(),
            },
        )
        .unwrap();
        let v = view(&c);
        for (g, gv) in c.graphs.iter().zip(&v.graphs) {
            for (cell, cv) in g.cells.iter().zip(&gv.cells) {
                assert_eq!(cell.id, cv.id);
                // Every row to a cell is a way into that cell.
                for (i, r) in cell.rows.iter().enumerate() {
                    if let Some(t) = gv.cells.iter().find(|x| x.id == r.to) {
                        assert!(
                            t.entered_from.contains(&RowRef {
                                cell: cell.id.clone(),
                                row: i
                            }),
                            "{} row {} into {}",
                            cell.id,
                            r.name,
                            r.to
                        );
                    }
                }
                // Every input that reads a place of a named cell is among what reads that cell's return.
                for (i, x) in cell.inputs.iter().enumerate() {
                    for s in &x.from {
                        let Src::Place(at, _) = s else { continue };
                        let Some(src) = gv.cells.iter().find(|y| y.id == *at) else {
                            continue;
                        };
                        let mut uses = src.returns.used_by.clone();
                        for f in &src.returns.forms {
                            uses.extend(f.used_by.iter().cloned());
                            all_uses(&f.fields, &mut uses);
                        }
                        all_uses(&src.returns.every, &mut uses);
                        let want = Use::Input {
                            cell: cell.id.clone(),
                            input: i,
                        };
                        assert!(uses.contains(&want), "{} input {i} reads {at}", cell.id);
                    }
                }
                // An agent's cell is given the return formats, and every source of its inputs is an input of the view.
                if cell.kind == AGENT {
                    let formats = cv
                        .inputs
                        .iter()
                        .filter(|m| matches!(m.of, CellInputOf::CommonFormat | CellInputOf::CellFormat))
                        .count();
                    assert_eq!(formats, 2, "{}", cell.id);
                    let sources: usize = cell.inputs.iter().map(|x| x.from.len()).sum();
                    let inputs = cv
                        .inputs
                        .iter()
                        .filter(|m| matches!(m.of, CellInputOf::Input { .. }))
                        .count();
                    assert_eq!(inputs, sources, "{}", cell.id);
                    assert_eq!(cv.returns.forms.len(), cell.forms.len(), "{}", cell.id);
                }
            }
        }
    }

    #[test]
    fn values_folded_into_a_description_come_apart() {
        let vs = vec!["a".to_string(), "b".to_string()];
        assert_eq!(
            split_values(Some("Pick. Values: a: one; two; b: three"), &vs),
            (Some("Pick".into()), vec![Some("one; two".into()), Some("three".into())])
        );
        assert_eq!(
            split_values(Some("Values: a: x; b: y"), &vs),
            (None, vec![Some("x".into()), Some("y".into())])
        );
        assert_eq!(
            split_values(Some("Just text"), &vs),
            (Some("Just text".into()), vec![None, None])
        );
    }

    // Every part of the page that shows a role reads its colour here: each line of the flow built into the program,
    // graph by graph, with what it is and the colour the flow gives it.
    #[test]
    fn the_lines_are_the_flow_s_each_with_its_colour() {
        use flow::format::Colour;
        let graphs = flow_lines();
        let names: Vec<&str> = graphs.iter().map(|g| g.graph.as_str()).collect();
        assert_eq!(names, ["engineer", "architect"]);
        let coloured: Vec<(&str, Colour)> = graphs
            .iter()
            .flat_map(|g| g.lines.iter().map(|l| (l.name.as_str(), l.colour)))
            .collect();
        assert_eq!(
            coloured,
            [
                ("engineer", Colour::Blue),
                ("review", Colour::Teal),
                ("adversarial", Colour::Auburn),
                ("advisor", Colour::Pink),
                ("architect", Colour::Purple),
                ("architect_review", Colour::Indigo),
                ("observer", Colour::Lime),
            ]
        );
        assert!(graphs.iter().flat_map(|g| &g.lines).all(|l| !l.desc.is_empty()));
    }
}
