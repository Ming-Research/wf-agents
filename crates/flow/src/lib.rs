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
//! - every input is there on every way to its cell unless marked maybe, and missing on some way when it is; and
//!   whether an agent's session already holds each input and rule it is given.

mod bits;
mod check;
mod compiled;
mod field;
pub mod format;
mod graph;
mod reads;
mod returns;
mod run;
mod walk;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub use compiled::{
    COMPILED, Compiled, CompiledCell, CompiledGraph, CompiledInput, CompiledInterject, CompiledRow, Held, RuleUse,
    Stats, ToolUse, found_at, valid,
};
pub use run::Position;

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
    let source: Source = serde_json::from_str(text).map_err(|e| fail(format!("flow.json: {e}")))?;
    if source.format != FORMAT {
        return Err(fail(format!(
            "flow.json is not in the format {FORMAT}: {}",
            source.format
        )));
    }
    let mut ids = BTreeSet::new();
    for f in &source.flows {
        for n in &f.nodes {
            if !ids.insert(n.id.as_str()) {
                return Err(fail(format!("two cells with one id: {}", n.id)));
            }
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
        for c in &g.cells {
            steps.extend(c.step.clone());
            needs.extend(c.needs.clone());
            for r in &c.rows {
                actions.extend(r.actions.iter().map(Do::name).map(str::to_string));
            }
            for x in &c.always {
                match (&x.view, &x.action) {
                    (Some(v), None) => relations.push((v.clone(), None, format!("{} always at {}", c.id, x.at))),
                    (None, Some(d)) => {
                        actions.insert(d.clone());
                    }
                    _ => problems.push(format!("{}: always at {} names one view or one action", c.id, x.at)),
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
