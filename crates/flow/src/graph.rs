//! One graph with what its cells share expanded into each cell: gates become agents' cells, and every agent's cell
//! gets the rows for whom it may ask, the common rows, fields, inputs, actions and checks. The structural checks are
//! here: every name a cell uses is one the flow has, and the graph's parts fit together.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::field::{Field, Resolved, Shapes, by_id, head, is_path, resolve};
use crate::format::{
    Always, Atom, Check, Do, Flow, Form, Input, Node, NodeKind, PromptBy, RawField, RawRow, Role, Source, Src,
};

macro_rules! ensure {
    ($cond:expr, $($arg:tt)+) => {
        let holds: bool = $cond;
        if !holds {
            return Err(format!($($arg)+));
        }
    };
}
pub(crate) use ensure;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CellKind {
    Engine,
    Agent,
    Tool,
    Owner,
}

impl CellKind {
    pub fn name(self) -> &'static str {
        match self {
            CellKind::Engine => "engine",
            CellKind::Agent => "agent",
            CellKind::Tool => "tool",
            CellKind::Owner => "owner",
        }
    }
}

/// Where a row goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    /// Back to the cell that called.
    Caller,
    /// Back to the cell that asked, or to the cell that receives the answer.
    Asker,
    /// To the cell the work stopped at before it went into the asking part.
    Stopped,
    /// The same cell, in the same round: a pause.
    Same,
    Cell(usize),
}

pub(crate) const POINTERS: [&str; 3] = ["@stopped", "@caller", "@from"];

#[derive(Debug, Clone)]
pub(crate) struct In {
    pub id: Option<String>,
    pub what: String,
    pub from: Vec<Src>,
    pub maybe: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Row {
    pub name: String,
    pub atoms: Vec<Atom>,
    pub to: String,
    pub target: Target,
    pub sets: BTreeMap<String, String>,
    /// A row every agent's cell has.
    pub common: bool,
    pub counts: Option<String>,
    pub waits: bool,
    pub interject: bool,
    pub actions: Vec<Do>,
    pub main: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct Cell {
    pub id: String,
    pub kind: CellKind,
    /// Written as a gate.
    pub gate: bool,
    pub title: String,
    pub step: Option<String>,
    pub needs: Option<String>,
    pub line: Option<String>,
    pub tools: Vec<String>,
    pub inputs: Vec<In>,
    pub rules: Vec<String>,
    pub prompt: Vec<String>,
    pub prompt_by: Option<PromptBy>,
    /// An agent's own answer; for any other cell, its output.
    pub main: Vec<Field>,
    /// Everything a return may hold: the own answer and, for an agent, the common fields its forms have.
    pub out: Vec<Field>,
    pub forms: Vec<Form>,
    pub say: String,
    pub rows: Vec<Row>,
    pub always: Vec<Always>,
    pub checks: Vec<Check>,
    pub handoff: Vec<String>,
    /// What the owner's page shows on the item: the owner reads it there.
    pub shown: Vec<String>,
    pub inenums: BTreeMap<String, Vec<(String, String)>>,
    pub by_id: BTreeMap<String, usize>,
    /// What the rows switch on: the output and, as fields named `@<id>`, the inputs read by id.
    pub sw: Vec<Field>,
    /// The cell reads all its inputs to make its output; an engine step with no output reads only what its rows,
    /// actions and checks name by id.
    pub reads_all: bool,
}

pub(crate) struct Graph<'s> {
    pub source: &'s Source,
    pub flow: &'s Flow,
    pub cells: Vec<Cell>,
    pub at: BTreeMap<String, usize>,
    pub start: usize,
    pub end: usize,
    pub region: BTreeSet<usize>,
    pub receive: Option<usize>,
    /// The cells with a row back to the caller or the asker.
    pub called: BTreeSet<usize>,
    /// The cells with a row to each cell.
    pub into: BTreeMap<usize, BTreeSet<usize>>,
    pub common_names: BTreeSet<String>,
    pub every: Vec<String>,
    pub stage: BTreeMap<usize, String>,
    pub block: Vec<usize>,
    pub entries: Vec<BTreeSet<usize>>,
    pub block_names: Vec<String>,
    /// The key of every path a cell's rows, actions and checks name.
    pub akeys: HashMap<(usize, String), String>,
    out_keys: RefCell<HashMap<(usize, String), Option<String>>>,
}

fn heads(atoms: &[Atom], out: &mut BTreeSet<String>) {
    for a in atoms {
        match a {
            Atom::Or(xs) => heads(xs, out),
            Atom::Has(p) | Atom::Is(p, _) | Atom::AnyOf(p, _) | Atom::NoneOf(p, _) => {
                out.insert(head(p).to_string());
            }
            _ => {}
        }
    }
}

pub(crate) fn flat(atoms: &[Atom], out: &mut Vec<Atom>) {
    for a in atoms {
        match a {
            Atom::Or(xs) => flat(xs, out),
            _ => out.push(a.clone()),
        }
    }
}

/// The paths a condition reads: with `reading`, only those that read a value, not whether it is there.
fn atom_paths(atoms: &[Atom], reading: bool, out: &mut Vec<String>) {
    let mut all = Vec::new();
    flat(atoms, &mut all);
    for a in all {
        match a {
            Atom::Has(p) if !reading => out.push(p),
            Atom::Is(p, _) | Atom::AnyOf(p, _) | Atom::NoneOf(p, _) => out.push(p),
            _ => {}
        }
    }
}

impl Cell {
    /// Every path the cell's rows, actions and checks name.
    pub fn paths_named(&self, reading: bool) -> Vec<String> {
        let mut out = Vec::new();
        for r in &self.rows {
            atom_paths(&r.atoms, reading, &mut out);
        }
        out.extend(
            self.rows
                .iter()
                .flat_map(|r| r.actions.iter().filter_map(|d| d.at().map(str::to_string))),
        );
        self.readers(&mut out);
        out
    }

    /// The paths read on every return: what the cell always does and checks, what it hands off, and what the owner
    /// reads on the item.
    pub fn readers(&self, out: &mut Vec<String>) {
        out.extend(self.always.iter().map(|x| x.at.clone()));
        out.extend(self.checks.iter().filter_map(|x| x.at.clone()));
        out.extend(self.handoff.iter().cloned());
        out.extend(self.shown.iter().cloned());
    }

    /// The paths a row itself reads, besides what every return reads.
    pub fn row_paths(&self, r: &Row) -> Vec<String> {
        let mut out = Vec::new();
        atom_paths(&r.atoms, true, &mut out);
        out.extend(r.actions.iter().filter_map(|d| d.at().map(str::to_string)));
        self.readers(&mut out);
        out
    }
}

fn input(x: &Input) -> In {
    In {
        id: x.id.clone(),
        what: x.what.clone(),
        from: x.from.clone(),
        maybe: x.maybe.unwrap_or(false),
    }
}

impl<'s> Graph<'s> {
    pub fn build(source: &'s Source, flow: &'s Flow) -> Result<Graph<'s>, String> {
        let common = &flow.agent_common;
        let ret = &common.returns;
        let all_tools: BTreeMap<&str, _> = source.tools.iter().map(|t| (t.name.as_str(), t)).collect();
        ensure!(all_tools.len() == source.tools.len(), "two tools with one name");
        let ask_kinds: Vec<(String, String)> = flow.ask_kinds.iter().flat_map(|g| g.kinds.iter().cloned()).collect();
        let mut uses: BTreeSet<&str> = flow
            .nodes
            .iter()
            .flat_map(|n| n.tools.iter().flatten().map(String::as_str))
            .collect();
        uses.extend(flow.gate.iter().flat_map(|g| g.tools.iter().map(String::as_str)));
        let unknown: Vec<&&str> = uses.iter().filter(|t| !all_tools.contains_key(**t)).collect();
        ensure!(unknown.is_empty(), "no such tool: {unknown:?}");
        let interject_from: Vec<(String, String)> = flow
            .interject
            .iter()
            .flat_map(|i| i.from.iter().map(|x| (x.name.clone(), x.desc.clone())))
            .collect();
        let ij_names: BTreeSet<&String> = interject_from.iter().map(|x| &x.0).collect();
        ensure!(
            ij_names.len() == interject_from.len(),
            "two who may interject with one name"
        );
        let shapes = Shapes {
            shapes: &source.shapes,
            tools: source.tools.iter().filter(|t| uses.contains(t.name.as_str())).collect(),
            ask_kinds: ask_kinds.clone(),
            interject_from,
        };
        let roles = &flow.asking.roles;
        let role_names: BTreeSet<&str> = roles.iter().map(|r| r.name.as_str()).collect();
        for g in &flow.ask_kinds {
            let ok = g.only.as_deref().is_none_or(|o| role_names.contains(o));
            ensure!(ok, "a group of kinds names no such role: {}", g.group);
        }
        let counters: BTreeSet<&str> = flow.limits.counters.iter().map(|c| c.name.as_str()).collect();
        ensure!(
            counters.len() == flow.limits.counters.len(),
            "two counters with one name"
        );
        ensure!(
            flow.limits.counters.iter().filter(|c| c.time).count() == 1,
            "exactly one counter is the total time"
        );
        let gate = flow.gate.as_ref();
        ensure!(
            gate.is_some() || flow.nodes.iter().all(|n| n.kind != NodeKind::Gate),
            "gate cells in a flow with no gate"
        );
        if let Some(g) = gate {
            ensure!(
                g.prompt.contains("{about}"),
                "gate.prompt does not name the kind: {}",
                g.prompt
            );
            ensure!(
                g.na.prompt.contains("{about}") && g.na.prompt.contains("{na}"),
                "gate.na.prompt does not name the kind and the example: {}",
                g.na.prompt
            );
        }
        let lines: Vec<String> = flow.sessions.lines.iter().map(|x| x.name.clone()).collect();
        ensure!(
            lines.iter().collect::<BTreeSet<_>>().len() == lines.len(),
            "two lines with one name"
        );
        let rule_names: BTreeSet<&str> = flow.rules.iter().map(|r| r.name.as_str()).collect();
        ensure!(rule_names.len() == flow.rules.len(), "two rules with one name");
        let stage_names: BTreeSet<&str> = flow.stages.list.iter().map(|s| s.name.as_str()).collect();
        ensure!(stage_names.len() == flow.stages.list.len(), "two stages with one name");

        // Gates become agents' cells.
        let mut nodes = Vec::new();
        for n in &flow.nodes {
            let gated = n.about.is_some() || n.needs.is_some() || n.question.is_some() || n.na.is_some();
            let gated = gated || n.to.is_some() || n.sets.is_some() || n.rows.is_some();
            ensure!(
                n.kind == NodeKind::Gate || !gated,
                "only a gate has about, needs, question, na, to, sets or rows: {}",
                n.id
            );
            nodes.push(match (n.kind, gate) {
                (NodeKind::Gate, Some(g)) => from_gate(n, g, &ask_kinds)?,
                _ => n.clone(),
            });
        }

        // The forms one return may hold.
        let common_out: Vec<RawField> = common.out.iter().map(|f| shapes.merged(f)).collect::<Result<_, _>>()?;
        let common_names: Vec<String> = common_out.iter().map(|f| f.name.clone().unwrap_or_default()).collect();
        ensure!(
            ret.forms.iter().filter(|f| f.own).count() == 1,
            "exactly one form is the cell's own answer"
        );
        for f in &ret.forms {
            ensure!(
                f.own == f.field.is_none(),
                "a form is the own answer or names its field: {}",
                f.say
            );
        }
        let form_fields: Vec<String> = ret.forms.iter().filter_map(|f| f.field.clone()).collect();
        let mut both: Vec<&String> = form_fields.iter().chain(&ret.every).collect();
        both.sort();
        let mut names_sorted: Vec<&String> = common_names.iter().collect();
        names_sorted.sort();
        ensure!(
            both == names_sorted,
            "every common field is one form or comes every time: {form_fields:?} {:?} {common_names:?}",
            ret.every
        );
        ensure!(
            !common_names.contains(&ret.within.name),
            "the field the form sits in is a field of its own"
        );
        let ask: Vec<&RawField> = common_out
            .iter()
            .filter(|f| matches!(&f.branches, Some(crate::format::Branches::Named(b)) if b == "askRoles"))
            .collect();
        let [ask] = ask.as_slice() else {
            return Err("exactly one form asks, one branch per role".into());
        };
        let ask_name = ask.name.clone().unwrap_or_default();
        ensure!(
            form_fields.contains(&ask_name),
            "exactly one form asks, one branch per role"
        );
        let ask_to = format!("{ask_name}.{}", ask.by.clone().unwrap_or_default());
        let mut tool_forms = Vec::new();
        for f in &common_out {
            if asks_for(&shapes, f, "tools")? {
                tool_forms.push(f.name.clone().unwrap_or_default());
            }
        }
        let [tool_form] = tool_forms.as_slice() else {
            return Err("one common field asks for tools".into());
        };
        let tool_form = tool_form.clone();
        for x in &common.inputs {
            let ok = x.with.as_ref().is_none_or(|w| form_fields.contains(w));
            ensure!(ok, "a common input brought back by no form: {:?}", x.with);
        }
        for p in ["{forms}", "{every}", "{in}"] {
            ensure!(ret.say.contains(p), "returns.say does not hold {p}: {}", ret.say);
        }
        for f in &ret.forms {
            let ok = f.lines.iter().flatten().all(|l| lines.contains(l));
            ensure!(ok, "a form for no such line: {:?} {:?}", f.field, f.lines);
            let ok = f.without.iter().all(|w| ret.every.contains(w));
            ensure!(
                ok,
                "a form leaves out a field that does not come every time: {:?}",
                f.field
            );
        }

        // The asking part.
        let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        let at: BTreeMap<String, usize> = ids.iter().enumerate().map(|(i, x)| (x.to_string(), i)).collect();
        ensure!(at.len() == ids.len(), "two cells with one id");
        let idx = |x: &str| at.get(x).copied().ok_or_else(|| format!("no such cell: {x}"));
        let region: BTreeSet<usize> = flow
            .asking
            .nodes
            .iter()
            .map(|x| idx(x))
            .collect::<Result<_, _>>()
            .map_err(|e| format!("the asking part names {e}"))?;
        let receive = flow.asking.receive.as_deref().map(idx).transpose()?;
        let asked: BTreeSet<usize> = roles.iter().map(|r| idx(&r.node)).collect::<Result<_, _>>()?;
        ensure!(
            receive.is_none_or(|r| region.contains(&r)) && asked.is_subset(&region),
            "the asking part does not hold the cell that receives or one that is asked"
        );
        let mut stage = BTreeMap::new();
        for s in &flow.stages.list {
            for x in &s.nodes {
                let i = idx(x).map_err(|e| format!("a stage names {e}"))?;
                ensure!(stage.insert(i, s.name.clone()).is_none(), "a cell at two stages: {x}");
            }
        }

        let has_form = |n: &Node, f: &Form| match &f.lines {
            None => true,
            Some(ls) => n.line.as_ref().is_some_and(|l| ls.contains(l)) && !flow.asking.nodes.contains(&n.id),
        };
        let say_of = |forms: &[Form]| {
            let nm = |x: &str| ret.name.replace("{name}", x);
            let fs: Vec<String> = forms.iter().map(|f| f.say.clone()).collect();
            let ev: Vec<String> = ret.every.iter().map(|x| nm(x)).collect();
            ret.say
                .replace("{in}", &nm(&ret.within.name))
                .replace("{forms}", &fs.join(&ret.join))
                .replace("{every}", &ev.join(&ret.join))
        };

        let mut cells = Vec::new();
        for n in &nodes {
            let nid = &n.id;
            let agent = n.kind == NodeKind::Agent;
            ensure!(agent || n.asks.is_none(), "only an agent asks: {nid}");
            let names: Vec<String> = match &n.asks {
                Some(xs) => xs.clone(),
                None => roles
                    .iter()
                    .filter(|r| &r.node != nid)
                    .map(|r| r.name.clone())
                    .collect(),
            };
            ensure!(
                names.iter().all(|x| role_names.contains(x.as_str())),
                "asks no such role: {nid} {names:?}"
            );
            ensure!(
                !roles.iter().any(|r| &r.node == nid && names.contains(&r.name)),
                "a cell that asks itself: {nid}"
            );
            // The common fields whose form this cell does not have; without the ask, it asks no one.
            let gone: BTreeSet<String> = if agent {
                ret.forms
                    .iter()
                    .filter(|f| !has_form(n, f))
                    .filter_map(|f| f.field.clone())
                    .collect()
            } else {
                BTreeSet::new()
            };
            let my_roles: Vec<&Role> = if agent && !gone.contains(&ask_name) {
                roles.iter().filter(|r| names.contains(&r.name)).collect()
            } else {
                Vec::new()
            };
            let mut raw_rows: Vec<(RawRow, bool)> = Vec::new();
            if agent {
                for r in &my_roles {
                    let row = RawRow {
                        name: r.row.clone(),
                        conditions: vec![Atom::Is(ask_to.clone(), BTreeSet::from([r.name.clone()]))],
                        to: r.node.clone(),
                        sets: None,
                        actions: r.actions.clone(),
                        counts: Some(flow.asking.counts.clone()),
                        waits: None,
                        interject: None,
                        main: None,
                        note: None,
                    };
                    raw_rows.push((row, true));
                }
            }
            raw_rows.extend(n.cases.iter().flatten().map(|r| (r.clone(), false)));
            if agent {
                for r in &common.rows {
                    let mut hs = BTreeSet::new();
                    heads(&r.conditions, &mut hs);
                    if hs.is_disjoint(&gone) {
                        raw_rows.push((r.clone(), true));
                    }
                }
            }
            let row_names: BTreeSet<&String> = raw_rows.iter().map(|r| &r.0.name).collect();
            ensure!(row_names.len() == raw_rows.len(), "two rows with one name: {nid}");
            let mut ins: Vec<In> = n.inputs.iter().map(input).collect();
            if agent {
                let with = |x: &&Input| x.with.as_ref().is_none_or(|w| !gone.contains(w));
                ins.extend(common.inputs.iter().filter(with).map(input));
            }
            let tools = n.tools.clone().unwrap_or_default();
            ensure!(tools.iter().all(|t| uses.contains(t.as_str())), "no such tool: {nid}");
            ensure!(
                !tools.is_empty() == (agent && !gone.contains(&tool_form)),
                "every agent that may ask for tools, and only such an agent, says which: {nid}"
            );
            let inenums: BTreeMap<String, Vec<(String, String)>> = n
                .inenums
                .iter()
                .flatten()
                .map(|(k, v)| shapes.enum_values(v).map(|vs| (k.clone(), vs)))
                .collect::<Result<_, _>>()?;
            if let Some(pb) = &n.prompt_by {
                let vs = inenums.get(&pb.by);
                ensure!(
                    n.prompt.is_some() && vs.is_some(),
                    "promptBy reads a value the cell does not name: {nid}"
                );
                let want: BTreeSet<&String> = vs.into_iter().flatten().map(|x| &x.0).collect();
                let have: BTreeSet<&String> = pb.cases.keys().collect();
                ensure!(want == have, "promptBy has a case for each value: {nid}");
            }
            let mut rows = Vec::new();
            for (r, is_common) in raw_rows {
                let to = r.to.clone();
                let goto = matches!(to.as_str(), "@caller" | "@asker" | "@stopped" | "@self");
                ensure!(goto || !to.starts_with('@'), "no such target: {nid} {} {to}", r.name);
                let own = r.conditions.contains(&Atom::Own);
                ensure!(agent || !own, "only an agent has an own answer to tell apart: {nid}");
                ensure!(
                    to != "@asker" || asked.contains(&idx(nid)?),
                    "only a cell that is asked answers the one that asked: {nid} {}",
                    r.name
                );
                let counts_ok = r.counts.as_deref().is_none_or(|c| counters.contains(c));
                ensure!(counts_ok, "no such counter: {nid} {} {:?}", r.name, r.counts);
                let waits = r.waits.unwrap_or(false);
                ensure!(
                    !(r.counts.is_some() && waits),
                    "a row that waits counts nothing: {nid} {}",
                    r.name
                );
                let interject = r.interject.unwrap_or(false);
                ensure!(
                    !interject || flow.asking.receive.as_ref() == Some(&to),
                    "a row that takes an interjection goes to the cell that receives it: {nid} {} {to}",
                    r.name
                );
                rows.push(Row {
                    name: r.name,
                    atoms: r.conditions,
                    to,
                    target: Target::Same,
                    sets: r.sets.unwrap_or_default(),
                    common: is_common,
                    counts: r.counts,
                    waits,
                    interject,
                    actions: r.actions.unwrap_or_default(),
                    main: r.main.unwrap_or(false),
                    note: r.note,
                });
            }
            let i = idx(nid)?;
            ensure!(
                !asked.contains(&i) || rows.iter().any(|r| r.to == "@asker"),
                "a cell that is asked does not answer: {nid}"
            );
            let main: Vec<Field> = n
                .out
                .iter()
                .flatten()
                .map(|f| shapes.field(f, &[], &[]))
                .collect::<Result<_, _>>()?;
            let mut out = main.clone();
            let mut forms = Vec::new();
            let mut say = String::new();
            let mut always = n.always.clone().unwrap_or_default();
            let mut checks = n.checks.clone().unwrap_or_default();
            let mut shown = n.shown.clone().unwrap_or_default();
            if agent {
                for f in &common.out {
                    let field = shapes.field(f, &tools, &my_roles)?;
                    if !gone.contains(&field.name) {
                        out.push(field);
                    }
                }
                forms = ret.forms.iter().filter(|f| has_form(n, f)).cloned().collect();
                say = say_of(&forms);
                always.extend(common.always.iter().filter(|x| !gone.contains(head(&x.at))).cloned());
                shown.extend(common.shown.iter().filter(|p| !gone.contains(head(p))).cloned());
                checks.extend(
                    common
                        .checks
                        .iter()
                        .filter(|x| !gone.contains(head(x.at.as_deref().unwrap_or(""))))
                        .cloned(),
                );
            }
            let line = n.line.clone();
            let rules = n.rules.clone().unwrap_or_default();
            ensure!(agent || line.is_none(), "only an agent is on a line: {nid}");
            ensure!(agent || rules.is_empty(), "only an agent is given rules: {nid}");
            let distinct: BTreeSet<&String> = rules.iter().collect();
            ensure!(
                rules.iter().all(|r| rule_names.contains(r.as_str())) && distinct.len() == rules.len(),
                "no such rule, or one twice: {nid} {rules:?}"
            );
            ensure!(
                line.as_ref().is_none_or(|l| lines.contains(l)),
                "no such line: {nid} {line:?}"
            );
            let kind = match n.kind {
                NodeKind::Agent | NodeKind::Gate => CellKind::Agent,
                NodeKind::Engine => CellKind::Engine,
                NodeKind::Tool => CellKind::Tool,
                NodeKind::Owner => CellKind::Owner,
            };
            ensure!(
                (kind == CellKind::Agent) == n.step.is_none() || kind == CellKind::Engine,
                "a cell that is not an agent names its step, and an agent names none: {nid}"
            );
            ensure!(agent || n.prompt.is_none(), "only an agent is given a prompt: {nid}");
            let mut by_id = BTreeMap::new();
            for (j, x) in ins.iter().enumerate() {
                if let Some(id) = &x.id {
                    ensure!(
                        by_id.insert(id.clone(), j).is_none(),
                        "two inputs with one id: {nid} {id}"
                    );
                }
            }
            cells.push(Cell {
                id: nid.clone(),
                kind,
                gate: flow.nodes.iter().any(|m| &m.id == nid && m.kind == NodeKind::Gate),
                title: n.title.clone(),
                step: n.step.clone(),
                needs: n.needs.clone(),
                line,
                tools,
                inputs: ins,
                rules,
                prompt: n.prompt.clone().unwrap_or_default(),
                prompt_by: n.prompt_by.clone(),
                main,
                out,
                forms,
                say,
                rows,
                always,
                checks,
                handoff: n.handoff.clone().unwrap_or_default(),
                shown,
                inenums,
                by_id,
                sw: Vec::new(),
                reads_all: true,
            });
        }
        let start = idx(&flow.start).map_err(|e| format!("no such start: {e}"))?;
        let end = idx(&flow.end).map_err(|e| format!("no such end: {e}"))?;
        for (i, c) in cells.iter_mut().enumerate() {
            ensure!(
                c.rows.is_empty() == (i == end),
                "the end, and only the end, has no rows: {}",
                c.id
            );
            for r in &mut c.rows {
                r.target = match r.to.as_str() {
                    "@caller" => Target::Caller,
                    "@asker" => Target::Asker,
                    "@stopped" => Target::Stopped,
                    "@self" => Target::Same,
                    t => Target::Cell(
                        at.get(t)
                            .copied()
                            .ok_or_else(|| format!("no such cell: {} {} {t}", c.id, r.name))?,
                    ),
                };
            }
        }
        for c in &cells {
            for r in &c.rows {
                let off = match receive {
                    Some(rc) => c.kind == CellKind::Agent && c.line != cells[rc].line,
                    None => false,
                };
                ensure!(
                    !(r.interject && off),
                    "an agent off the line of the cell that receives takes an interjection there: {} {}",
                    c.id,
                    r.name
                );
            }
        }

        // The blocks: every cell in exactly one; the asking part is one, its entries the cells that are asked.
        let mut block = vec![usize::MAX; cells.len()];
        let mut entries: Vec<BTreeSet<usize>> = Vec::new();
        let mut block_names = Vec::new();
        for (bi, b) in flow.blocks.iter().enumerate() {
            let (members, ents): (Vec<usize>, Vec<usize>) = if b.asking == Some(true) {
                (region.iter().copied().collect(), asked.iter().copied().collect())
            } else {
                let (Some(ns), Some(es)) = (&b.nodes, &b.entries) else {
                    return Err(format!("a block lists its cells and entries: {}", b.name));
                };
                let ns = ns.iter().map(|x| idx(x)).collect::<Result<Vec<_>, _>>();
                let es = es.iter().map(|x| idx(x)).collect::<Result<Vec<_>, _>>();
                match (ns, es) {
                    (Ok(ns), Ok(es)) => (ns, es),
                    _ => return Err(format!("a cell of no such name, or in two blocks: {}", b.name)),
                }
            };
            ensure!(
                ents.iter().all(|e| members.contains(e)),
                "a block enters at a cell it does not hold: {}",
                b.name
            );
            for m in members {
                ensure!(
                    block[m] == usize::MAX,
                    "a cell of no such name, or in two blocks: {} {}",
                    b.name,
                    cells[m].id
                );
                block[m] = bi;
            }
            entries.push(ents.into_iter().collect());
            block_names.push(b.name.clone());
        }
        let loose: Vec<&str> = cells
            .iter()
            .zip(&block)
            .filter(|(_, b)| **b == usize::MAX)
            .map(|(c, _)| c.id.as_str())
            .collect();
        ensure!(loose.is_empty(), "a cell in no block: {loose:?}");
        let counted: BTreeSet<&str> = cells
            .iter()
            .flat_map(|c| c.rows.iter().filter_map(|r| r.counts.as_deref()))
            .collect();
        let idle: Vec<&&str> = counters.difference(&counted).collect();
        ensure!(idle.is_empty(), "a counter no row counts: {idle:?}");
        let used: BTreeSet<&str> = cells.iter().flat_map(|c| c.rules.iter().map(String::as_str)).collect();
        let idle: Vec<&&str> = rule_names.difference(&used).collect();
        ensure!(idle.is_empty(), "a rule no cell uses: {idle:?}");
        // A cell at no stage asks, answers or runs tools for another: it goes on only back to a cell the engine
        // keeps track of, or into the asking part.
        for (i, c) in cells.iter().enumerate() {
            let ok = stage.contains_key(&i)
                || region.contains(&i)
                || c.rows
                    .iter()
                    .all(|r| !matches!(r.target, Target::Cell(t) if !region.contains(&t)));
            ensure!(ok, "a cell at no stage that goes on by itself: {}", c.id);
        }
        let mut into: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
        for (i, c) in cells.iter().enumerate() {
            for r in &c.rows {
                if let Target::Cell(t) = r.target {
                    into.entry(t).or_default().insert(i);
                }
            }
        }
        let called = cells
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                c.rows
                    .iter()
                    .any(|r| matches!(r.target, Target::Caller | Target::Asker))
            })
            .map(|(i, _)| i)
            .collect();
        let mut g = Graph {
            source,
            flow,
            cells,
            at,
            start,
            end,
            region,
            receive,
            called,
            into,
            common_names: common_names.into_iter().collect(),
            every: ret.every.clone(),
            stage,
            block,
            entries,
            block_names,
            akeys: HashMap::new(),
            out_keys: RefCell::new(HashMap::new()),
        };
        g.sources()?;
        g.pseudo()?;
        Ok(g)
    }

    /// The key of path `p` in the output of cell `n`, or none when its output has no such place.
    pub fn out_key(&self, n: usize, p: &str) -> Option<String> {
        let k = (n, p.to_string());
        if let Some(v) = self.out_keys.borrow().get(&k) {
            return v.clone();
        }
        let v = resolve(&self.cells[n].out, p).ok().map(|x| x.0);
        self.out_keys.borrow_mut().insert(k, v.clone());
        v
    }

    /// The key of a path a cell's rows, actions or checks name.
    pub fn akey(&self, c: usize, p: &str) -> &str {
        &self.akeys[&(c, p.to_string())]
    }

    pub fn is_pointer(n: &str) -> bool {
        POINTERS.contains(&n)
    }

    /// Concrete sources name a place that exists; a source that names a cell the engine keeps track of is checked
    /// against the cells it turns out to be, once the states are known.
    fn sources(&self) -> Result<(), String> {
        for c in &self.cells {
            for x in &c.inputs {
                for s in &x.from {
                    let Src::Place(n, p) = s else { continue };
                    if Self::is_pointer(n) {
                        ensure!(is_path(p), "not a path: {} {n} {p}", c.id);
                    } else {
                        let ok = self.at.get(n).is_some_and(|&m| self.out_key(m, p).is_some());
                        ensure!(ok, "no such place: {} {n} {p}", c.id);
                    }
                }
            }
        }
        Ok(())
    }

    /// The cells a source of `m` that names `ptr` can point at, from the rows alone: for the shape of what it reads.
    fn static_points(&self, m: usize, ptr: &str) -> BTreeSet<usize> {
        if ptr == "@stopped" {
            let goes_in = |c: &Cell| {
                c.rows
                    .iter()
                    .any(|r| matches!(r.target, Target::Cell(t) if self.region.contains(&t)))
            };
            return (0..self.cells.len())
                .filter(|i| !self.region.contains(i) && goes_in(&self.cells[*i]))
                .collect();
        }
        self.into.get(&m).cloned().unwrap_or_default()
    }

    /// A cell whose rows read an input by id switches on that input's field as on a field of its own, named `@<id>`.
    fn pseudo(&mut self) -> Result<(), String> {
        for ci in 0..self.cells.len() {
            let c = &self.cells[ci];
            let ids: BTreeSet<String> = c
                .paths_named(false)
                .iter()
                .filter(|p| p.starts_with('@'))
                .map(|p| by_id(p).map_or(p.clone(), |x| x.0.to_string()))
                .collect();
            let mut pseudo = Vec::new();
            for iid in &ids {
                let x = c.by_id.get(iid).map(|j| &c.inputs[*j]);
                let Some(x) = x else {
                    return Err(format!("no input with that id: {} {iid}", c.id));
                };
                let [Src::Place(n, p)] = x.from.as_slice() else {
                    return Err(format!("an input read by id has one source, a place: {} {iid}", c.id));
                };
                let cands: Vec<usize> = if Self::is_pointer(n) {
                    self.static_points(ci, n).into_iter().collect()
                } else {
                    self.at.get(n).copied().into_iter().collect()
                };
                let mut fs = Vec::new();
                for m in cands {
                    if self.out_key(m, p).is_some() {
                        match resolve(&self.cells[m].out, p)?.1 {
                            Resolved::Field(f) => fs.push(f),
                            Resolved::Disc(_) => return Err(format!("an input read by id is a field: {} {iid}", c.id)),
                        }
                    }
                }
                let Some(first) = fs.first() else {
                    return Err(format!(
                        "the input has no field of that name on any cell it can come from: {} {iid}",
                        c.id
                    ));
                };
                ensure!(
                    fs.iter().all(|f| f == first),
                    "the cells an input can come from give it different shapes: {} {iid}",
                    c.id
                );
                let mut f = first.clone();
                f.name = format!("@{iid}");
                f.optional = f.optional || x.maybe;
                pseudo.push(f);
            }
            let reads_all = c.kind != CellKind::Engine || !c.out.is_empty();
            ensure!(
                reads_all || c.paths_named(false).iter().all(|p| p.starts_with('@')),
                "an engine cell with no output can only read its inputs: {}",
                c.id
            );
            let mut sw = c.out.clone();
            sw.extend(pseudo);
            let paths = c.paths_named(false);
            let c = &mut self.cells[ci];
            c.sw = sw;
            c.reads_all = reads_all;
            for p in paths {
                let k = resolve(&c.sw, &p).map_err(|e| format!("{e} (at {})", c.id))?.0;
                self.akeys.insert((ci, p), k);
            }
        }
        Ok(())
    }
}

/// Whether a field holds, at any depth, a union whose branches are `what`.
fn asks_for(shapes: &Shapes, x: &RawField, what: &str) -> Result<bool, String> {
    let x = shapes.merged(x)?;
    if matches!(&x.branches, Some(crate::format::Branches::Named(b)) if b == what) {
        return Ok(true);
    }
    for y in x.fields.iter().flatten().chain(x.item.as_deref()) {
        if asks_for(shapes, y, what)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// A gate as the agent's cell it is: one question, whose own answer goes on to one cell or by its rows.
fn from_gate(n: &Node, g: &crate::format::Gate, ask_kinds: &[(String, String)]) -> Result<Node, String> {
    let id = &n.id;
    let about = n.about.clone().unwrap_or_default();
    ensure!(
        ask_kinds.iter().any(|(k, _)| *k == about),
        "a gate asks about no such kind: {id} {about}"
    );
    ensure!(
        n.to.is_some() != n.rows.is_some(),
        "a gate goes on to one cell or by rows: {id}"
    );
    ensure!(
        !(n.rows.is_some() && n.sets.is_some()),
        "a gate with rows records what each row does: {id}"
    );
    let na = n.na.as_ref().map(|_| &g.na);
    ensure!(
        !(na.is_some() && n.rows.is_some()),
        "a gate that may not apply goes on to one cell: {id}"
    );
    let own = n.step.is_some() || n.line.is_some() || n.tools.is_some() || n.rules.is_some() || n.prompt.is_some();
    let own = own || n.prompt_by.is_some() || n.out.is_some() || n.cases.is_some() || n.checks.is_some();
    ensure!(
        !own && n.handoff.is_none() && n.asks.is_none(),
        "a gate takes its line, tools, prompt and output from the flow's gate: {id}"
    );
    let question = n
        .question
        .clone()
        .ok_or_else(|| format!("a gate with no question: {id}"))?;
    let rows = match &n.rows {
        Some(rows) => rows
            .iter()
            .map(|r| {
                let mut r = r.clone();
                r.conditions.insert(0, Atom::Own);
                r
            })
            .collect(),
        None => {
            let row = na.map(|x| &x.row).unwrap_or(&g.row);
            vec![RawRow {
                name: row.name.clone(),
                conditions: vec![Atom::Own],
                to: n.to.clone().unwrap_or_default(),
                sets: n.sets.clone(),
                actions: None,
                counts: None,
                waits: None,
                interject: None,
                main: Some(row.main),
                note: None,
            }]
        }
    };
    let prompt = na.map(|x| &x.prompt).unwrap_or(&g.prompt);
    let prompt = prompt
        .replace("{about}", &about)
        .replace("{na}", n.na.as_deref().unwrap_or(""));
    let mut inputs = n.inputs.clone();
    inputs.extend(g.inputs.iter().cloned());
    Ok(Node {
        id: id.clone(),
        kind: NodeKind::Agent,
        title: n.title.clone(),
        step: None,
        line: Some(g.line.clone()),
        tools: Some(g.tools.clone()),
        inputs,
        rules: None,
        prompt: Some(vec![format!("{question} {prompt}")]),
        prompt_by: None,
        out: Some(na.map(|x| vec![x.out.clone()]).unwrap_or_default()),
        cases: Some(rows),
        always: n.always.clone(),
        checks: None,
        handoff: None,
        shown: n.shown.clone(),
        inenums: n.inenums.clone(),
        asks: None,
        about: n.about.clone(),
        needs: n.needs.clone(),
        question: None,
        na: None,
        to: None,
        sets: None,
        rows: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heads_of_conditions() {
        let mut out = BTreeSet::new();
        heads(
            &[Atom::Or(vec![
                Atom::Has("@a.b".into()),
                Atom::Is("c[x].d".into(), BTreeSet::new()),
            ])],
            &mut out,
        );
        assert_eq!(out, BTreeSet::from(["@a".to_string(), "c".to_string()]));
    }
}
