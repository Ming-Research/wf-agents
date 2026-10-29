//! The flow's compiler. It reads `flow.json` (its format is [`format`]) and the database's tables and views, expands
//! what cells share, and checks the whole; what it hands back is what the engine runs. A failure names the place
//! and an example; every failure is reported, not only the first.
//!
//! The checks:
//! - the format, and every name: each name the flow gives the engine is used, and each one used is given; each
//!   table or view an input reads is in the database, with the column it is read by;
//! - every state the engine can be in, walking from the start (where the work stopped, the cells that called and
//!   wait for the answer, which cell brought the work here, what rows recorded): every cell is reached, every row is
//!   taken somewhere, every recorded value a row reads was recorded on the way and every recorded value is read, a
//!   cell that waits for an answer gets it, and every loop of states passes a row that counts or waits;
//! - every return each cell's output allows, in every state: exactly one row holds, and nothing in it is dropped;
//! - every row from one block into another goes to one of that block's entries;
//! - every input is there on every way to its cell unless marked maybe, and missing on some way when it is;
//! - every cell a text of the flow names, as cell `name`, is a cell of its graph ([`cells_named`]).
//!
//! What an agent's session already holds is not worked out here: the engine decides it as it runs, from what each
//! session was given and handed back.

mod bits;
mod check;
mod compiled;
mod field;
pub mod format;
pub mod formats;
mod graph;
mod reads;
mod returns;
mod run;
mod texts;
mod walk;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub use compiled::{
    AskNeed, COMPILED, Compiled, CompiledCell, CompiledGraph, CompiledInput, CompiledInterject, CompiledRow, RuleUse,
    Stats, ToolUse, found_at, valid, without_null_options,
};
pub use run::Position;
pub use texts::cells_named;

use check::Analysis;
use format::{Do, Source, Src};
use graph::Graph;

/// The format `flow.json` is written in.
pub const FORMAT: &str = "wf-agents flow 1";

/// The flow the engine runs, as this repository's `flow/flow.json` has it.
pub const FLOW: &str = include_str!("../../../flow/flow.json");

/// What the flow is checked against besides itself.
#[derive(Debug, Clone, Default)]
pub struct Context {
    /// Every table and view of the database, with its columns.
    pub relations: BTreeMap<String, BTreeSet<String>>,
    /// Each table and view agents can read, with its columns' names and the first line of what it holds, in the order
    /// the schema defines them: the index a tool that reads the database lists in its description.
    pub tables: Vec<(String, Vec<String>, String)>,
}

/// Everything wrong with a flow.
#[derive(Debug, Clone)]
pub struct Error {
    pub problems: Vec<String>,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.problems.join("\n"))
    }
}

impl std::error::Error for Error {}

fn fail(p: impl Into<String>) -> Error {
    Error {
        problems: vec![p.into()],
    }
}

/// The columns an input from the database may be read by: the item, work or project at hand.
const BY: [&str; 3] = ["item", "work", "project"];

pub fn compile(text: &str, cx: &Context) -> Result<Compiled, Error> {
    let mut source: Source = serde_json::from_str(text).map_err(|e| fail(format!("flow.json: {e}")))?;
    // A tool that reads the database says in its description what it reads: each table and view with its columns and
    // the first line of what it holds. It comes with the return formats, once in a session.
    for t in &mut source.tools {
        if let Some(lead) = &t.tables {
            let lines: Vec<String> = cx
                .tables
                .iter()
                .map(|(n, cols, d)| format!("- {n} ({}): {d}", cols.join(", ")))
                .collect();
            t.desc = format!("{}\n\n{lead}\n{}", t.desc, lines.join("\n"));
        }
    }
    if source.format != FORMAT {
        return Err(fail(format!(
            "flow.json is not in the format {FORMAT}: {}",
            source.format
        )));
    }
    // A rule every graph shares names lines, each a line of some graph.
    let all_lines: BTreeSet<&str> = source
        .flows
        .iter()
        .flat_map(|f| f.sessions.lines.iter().map(|l| l.name.as_str()))
        .collect();
    for r in &source.rules {
        let ok = !r.lines.is_empty() && r.lines.iter().all(|l| all_lines.contains(l.as_str()));
        if !ok {
            return Err(fail(format!(
                "a rule every graph shares, given to no such line: {} {:?}",
                r.name, r.lines
            )));
        }
    }
    let mut problems = Vec::new();
    let mut graphs = Vec::new();
    let mut stats = Vec::new();
    let mut built = Vec::new();
    for f in &source.flows {
        let g = Graph::build(&source, f).map_err(|e| fail(format!("{}: {e}", f.name)))?;
        built.push(g);
    }
    for g in &built {
        let name = &g.flow.name;
        let a = Analysis::run(g).map_err(|e| fail(format!("{name}: {e}")))?;
        let lines = a.findings.lines();
        if !lines.is_empty() {
            problems.extend(lines.into_iter().map(|l| format!("{name}: {l}")));
            continue;
        }
        let (cg, st) = compiled::graph(g, &a).map_err(|e| fail(format!("{name}: {e}")))?;
        graphs.push(cg);
        stats.push(st);
    }
    let settings = names(&source, &built, cx, &mut problems);
    let raw: serde_json::Value = serde_json::from_str(text).map_err(|e| fail(format!("flow.json: {e}")))?;
    problems.extend(texts::unknown_cells(&raw));
    if !problems.is_empty() {
        return Err(Error { problems });
    }
    Ok(Compiled {
        format: COMPILED.to_string(),
        engine: source.engine.clone(),
        tools: source
            .tools
            .iter()
            .map(|t| ToolUse {
                name: t.name.clone(),
                runs: t.runs.clone(),
            })
            .collect(),
        graphs,
        settings,
        stats,
    })
}

/// Every name the flow gives the engine is used and every one used is given; every table and view an input or
/// an action reads is in the database. Hands back the configuration settings the flow reads.
fn names(source: &Source, graphs: &[Graph], cx: &Context, problems: &mut Vec<String>) -> Vec<String> {
    let e = &source.engine;
    let mut steps = BTreeSet::new();
    let mut actions = BTreeSet::new();
    let mut checks = BTreeSet::new();
    let mut inputs = BTreeSet::new();
    let mut values = BTreeSet::new();
    let mut needs = BTreeSet::new();
    let mut settings = BTreeSet::new();
    let mut relations: Vec<(String, Option<String>, String)> = Vec::new();
    for g in graphs {
        let recorded: BTreeSet<&String> = g
            .cells
            .iter()
            .flat_map(|c| c.rows.iter().flat_map(|r| r.sets.keys()))
            .collect();
        for r in &g.flow.asking.roles {
            actions.extend(r.actions.iter().flatten().map(|d| d.name().to_string()));
        }
        for grp in &g.flow.ask_kinds {
            needs.extend(grp.needs.values().cloned());
        }
        for c in &g.cells {
            steps.extend(c.step.clone());
            needs.extend(c.needs.clone());
            actions.extend(c.actions.iter().cloned());
            for r in &c.rows {
                actions.extend(r.actions.iter().map(Do::name).map(str::to_string));
            }
            // What is done every time is an action; a view would read a field without anything being done with it.
            for x in &c.always {
                match (&x.view, &x.action) {
                    (None, Some(d)) => {
                        actions.insert(d.clone());
                    }
                    _ => problems.push(format!("{}: always at {} names one action, and no view", c.id, x.at)),
                }
            }
            checks.extend(c.checks.iter().map(|x| x.check.clone()));
            for k in c.inenums.keys() {
                if !recorded.contains(k) {
                    values.insert(k.clone());
                }
            }
            for x in &c.inputs {
                for s in &x.from {
                    match s {
                        Src::Engine(n) => {
                            inputs.insert(n.clone());
                        }
                        Src::Config(n) => {
                            settings.insert(n.clone());
                        }
                        Src::Recorded(n) if !recorded.contains(n) => {
                            problems.push(format!("{}: an input reads a value no row records: {n}", c.id));
                        }
                        Src::Db { relation, by } => relations.push((relation.clone(), by.clone(), c.id.clone())),
                        _ => {}
                    }
                }
            }
        }
        for k in &recorded {
            if e.values.contains_key(*k) {
                problems.push(format!("a value rows record is also one the engine knows: {k}"));
            }
        }
    }
    fn both(what: &str, given: &BTreeMap<String, String>, used: &BTreeSet<String>, problems: &mut Vec<String>) {
        for n in used.iter().filter(|n| !given.contains_key(*n)) {
            problems.push(format!("no such engine {what}: {n}"));
        }
        for n in given.keys().filter(|n| !used.contains(*n)) {
            problems.push(format!("an engine {what} nothing uses: {n}"));
        }
    }
    both("step", &e.steps, &steps, problems);
    both("action", &e.actions, &actions, problems);
    both("check", &e.checks, &checks, problems);
    both("input", &e.inputs, &inputs, problems);
    both("value", &e.values, &values, problems);
    both("need", &e.needs, &needs, problems);
    stores(e, cx, problems);
    for (rel, by, at) in relations {
        let Some(cols) = cx.relations.get(&rel) else {
            problems.push(format!("{at}: no table or view {rel} in the database"));
            continue;
        };
        if let Some(b) = by {
            if !BY.contains(&b.as_str()) {
                problems.push(format!("{at}: {rel} is read by the item, work or project, not by {b}"));
            } else if !cols.contains(&b) {
                problems.push(format!("{at}: {rel} has no column {b}"));
            }
        }
    }
    settings.into_iter().collect()
}

/// The table of what the engine writes and reads is whole: every action names the stores it writes, every value of
/// the engine what it reads, with what the chart says of it; every store named is one of the flow's, and every table
/// or view and engine input a store names is one there is.
fn stores(e: &format::EngineNames, cx: &Context, problems: &mut Vec<String>) {
    let known = |s: &String| e.stores.contains_key(s);
    for a in e.actions.keys().filter(|a| !e.writes.contains_key(*a)) {
        problems.push(format!("an engine action that says no stores it writes: {a}"));
    }
    for (n, ss) in &e.writes {
        if !e.actions.contains_key(n) && !e.steps.contains_key(n) {
            problems.push(format!("the stores written by no such engine action or step: {n}"));
        }
        for s in ss.iter().filter(|s| !known(s)) {
            problems.push(format!("{n} writes no such store: {s}"));
        }
    }
    for v in e.values.keys() {
        if e.reads
            .get(v)
            .is_none_or(|r| r.says.as_deref().is_none_or(str::is_empty))
        {
            problems.push(format!("an engine value that does not say what it reads: {v}"));
        }
    }
    for (n, r) in &e.reads {
        if !e.values.contains_key(n) && !e.steps.contains_key(n) {
            problems.push(format!("what is read by no such engine value or step: {n}"));
        }
        for s in r.stores.iter().filter(|s| !known(s)) {
            problems.push(format!("{n} reads no such store: {s}"));
        }
    }
    for (n, s) in &e.stores {
        for rel in s.relations.iter().filter(|r| !cx.relations.contains_key(*r)) {
            problems.push(format!("store {n}: no table or view {rel} in the database"));
        }
        for i in s.inputs.iter().filter(|i| !e.inputs.contains_key(*i)) {
            problems.push(format!("store {n}: no such engine input: {i}"));
        }
    }
}
