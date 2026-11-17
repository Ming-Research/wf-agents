//! Each case checked against the compiled flow, and its names resolved: the history as steps, each round's blocks with
//! their content. Every fault of every file is said at once (principle `report-all-at-once`), each with its file and
//! case, and a case with a fault is not given.
//!
//! What is checked is what the flow's definition can say alone: that a round's cell is a cell of the flow and each block
//! it hands in one of its outputs, a choice with one of its values and no other with one, a shaped block with its shape;
//! that what the owner says goes to a cell that has words of its own; that what the case says is new to its cell is
//! among what the cell watches, and what it owes is among what the cell writes. Whether the history leads to the cell
//! entered as the case says is the engine's to tell, when the case is built.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use decide::{BlockId, CellId};
use flow::{CellSpec, Compiled, Kind};

use crate::read::{COMMON, Read};
use crate::written::{self, Item, Owes, Rules, Tests, Value};

/// What is wrong with a file of cases, or with one of its cases.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
    pub file: String,
    pub case: Option<String>,
    pub what: String,
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.case {
            Some(case) => write!(f, "{}: {case}: {}", self.file, self.what),
            None => write!(f, "{}: {}", self.file, self.what),
        }
    }
}

/// A flow's cases, checked: the made items, the rules by name, and each cell's cases.
#[derive(Clone, Debug, Default)]
pub struct Cases {
    /// The made project the cases run in, by name.
    pub project: String,
    pub items: BTreeMap<u64, Item>,
    /// How the made project's tests are found, where any case names rules.
    pub tests: Option<Tests>,
    pub rules: BTreeMap<String, Rules>,
    pub cells: Vec<Of>,
}

/// The cases of a cell, from its file.
#[derive(Clone, Debug)]
pub struct Of {
    pub cell: CellId,
    pub file: String,
    pub cases: Vec<Case>,
}

/// A case, checked.
#[derive(Clone, Debug)]
pub struct Case {
    pub id: String,
    pub cell: CellId,
    pub item: u64,
    pub history: Vec<Step>,
    pub ran_before: bool,
    pub new: Vec<String>,
    pub results: Option<String>,
    pub owes: Owes,
    pub judge: Option<String>,
}

/// Something that happened in the work before the case's cell is entered.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Round(Round),
    /// The owner wrote the item's background anew.
    Background(String),
    /// The owner said these words to the cell.
    Words {
        to: CellId,
        text: String,
    },
    /// The owner answered the question of this name, taking the option, counted from one.
    Answer {
        question: String,
        option: u64,
    },
    /// Main moved to this commit of the made project, by a name of it.
    Main(String),
}

/// A round of a cell, which hands these in and, where it commits, leaves the work's branch at this commit of the made
/// project.
#[derive(Clone, Debug, PartialEq)]
pub struct Round {
    pub cell: CellId,
    pub hands_in: Vec<Hand>,
    pub commit: Option<String>,
}

/// A block a round hands in.
#[derive(Clone, Debug, PartialEq)]
pub struct Hand {
    pub block: BlockId,
    pub content: String,
    pub value: Option<String>,
    /// Handed in as changing nothing in meaning.
    pub unchanged: bool,
    /// Of a block whose form is a shape, the shaped value, of which `content` is nothing.
    pub shaped: Option<toml::Value>,
}

/// The ways a question may be owed.
const QUESTION: [&str; 3] = ["none", "waits", "any"];
/// What `writes` of what is owed says of a block that is no choice, or of a choice written with any value.
pub const WRITTEN: &str = "written";

/// Checks every case against the compiled flow; every fault is said.
pub fn check(flow: &Compiled, read: &Read) -> Result<Cases, Vec<Fault>> {
    let mut faults = Vec::new();
    let mut items = BTreeMap::new();
    for (number, item) in &read.common.items {
        match number.parse::<u64>() {
            Ok(n) => {
                items.insert(n, item.clone());
            }
            Err(_) => faults.push(Fault {
                file: COMMON.to_string(),
                case: None,
                what: format!("items.{number}: an item is named by its number"),
            }),
        }
    }
    let project = read.common.project.clone().unwrap_or_default();
    if project.is_empty() && !read.cells.is_empty() {
        faults.push(fault(
            COMMON,
            None,
            "project: the made project the cases run in is named".into(),
        ));
    }
    let mut cases = Cases {
        project,
        items,
        tests: read.common.tests.clone(),
        ..Cases::default()
    };
    let mut ids: BTreeSet<String> = BTreeSet::new();
    let mut rules = read.common.results.clone();
    for (file, cell) in &read.cells {
        for (name, own) in &cell.results {
            if rules.insert(name.clone(), own.clone()).is_some() {
                faults.push(fault(
                    file,
                    None,
                    format!("results.{name}: named in another file as well"),
                ));
            }
        }
    }
    let found = read.common.tests.as_ref();
    let empty = found.is_none_or(|t| t.mark.is_empty() || t.name_after.is_empty() || t.under.is_empty());
    if !rules.is_empty() && empty {
        faults.push(fault(
            COMMON,
            None,
            "tests: the made project's tests are found by a mark, a name after it, and the directories under which"
                .into(),
        ));
    }
    for (name, rule) in &rules {
        if rule.fails.iter().any(|f| f.names.is_empty() || f.said.is_empty()) {
            faults.push(fault(
                COMMON,
                None,
                format!("results.{name}: a test that fails is named, and says something"),
            ));
        }
    }
    for (file, written) in &read.cells {
        let Some(spec) = agent(flow, &written.cell) else {
            faults.push(fault(
                file,
                None,
                format!("{} is no agent's cell of the flow", written.cell),
            ));
            continue;
        };
        let names = Names {
            texts: joined(&read.common.texts, &written.texts),
            sets: joined(&read.common.sets, &written.sets),
        };
        let mut of = Of {
            cell: spec.id.clone(),
            file: file.clone(),
            cases: Vec::new(),
        };
        for case in &written.cases {
            let mut wrong = Vec::new();
            if !ids.insert(case.id.clone()) {
                wrong.push("another case has its id".to_string());
            }
            if !cases.items.contains_key(&case.item) {
                wrong.push(format!("item {} is no made item of {COMMON}", case.item));
            }
            let mut history = Vec::new();
            for (n, entry) in case.history.iter().enumerate() {
                match step(flow, &names, entry) {
                    Ok(step) => history.push(step),
                    Err(why) => wrong.extend(why.into_iter().map(|w| format!("history {}: {w}", n + 1))),
                }
            }
            for name in &case.new {
                if !spec.watches.iter().any(|w| &w.name == name) {
                    wrong.push(format!("new: {name} is none of what {} watches", spec.id));
                }
            }
            if case.new.is_empty() {
                wrong.push("new: nothing is new to the cell, and nothing runs it".to_string());
            }
            if let Some(name) = &case.results
                && !rules.contains_key(name)
            {
                wrong.push(format!("results: no rules are named {name}"));
            }
            wrong.extend(owed(flow, spec, &case.owes));
            if wrong.is_empty() {
                of.cases.push(Case {
                    id: case.id.clone(),
                    cell: spec.id.clone(),
                    item: case.item,
                    history,
                    ran_before: case.ran_before,
                    new: case.new.clone(),
                    results: case.results.clone(),
                    owes: case.owes.clone(),
                    judge: case.judge.clone(),
                });
            }
            faults.extend(wrong.into_iter().map(|w| fault(file, Some(&case.id), w)));
        }
        cases.cells.push(of);
    }
    cases.rules = rules;
    match faults.is_empty() {
        true => Ok(cases),
        false => Err(faults),
    }
}

fn fault(file: &str, case: Option<&str>, what: String) -> Fault {
    Fault {
        file: file.to_string(),
        case: case.map(str::to_string),
        what,
    }
}

/// The names a file's cases may use: its own and those of `common.toml`.
struct Names {
    texts: BTreeMap<String, String>,
    sets: BTreeMap<String, BTreeMap<String, Value>>,
}

fn joined<T: Clone>(common: &BTreeMap<String, T>, own: &BTreeMap<String, T>) -> BTreeMap<String, T> {
    let mut all = common.clone();
    all.extend(own.iter().map(|(k, v)| (k.clone(), v.clone())));
    all
}

/// The agent's cell of this id.
fn agent<'f>(flow: &'f Compiled, id: &str) -> Option<&'f CellSpec> {
    (flow.cells.iter()).find(|c| c.id.as_str() == id && c.kind == Kind::Agent)
}

/// An entry of a history as a step, or every reason it is not one.
fn step(flow: &Compiled, names: &Names, entry: &written::Entry) -> Result<Step, Vec<String>> {
    let kinds = [entry.cell.is_some(), entry.owner.is_some(), entry.main.is_some()];
    if kinds.iter().filter(|k| **k).count() != 1 {
        return Err(vec![
            "an entry is one of a round of a `cell`, an act of the `owner` and `main` moving".into(),
        ]);
    }
    let text = || -> Result<String, String> {
        match (&entry.content, &entry.text) {
            (Some(content), None) => Ok(content.clone()),
            (None, Some(name)) => (names.texts.get(name).cloned()).ok_or_else(|| format!("no text is named {name}")),
            _ => Err("it gives its words as `content` or as a named `text`, one of them".into()),
        }
    };
    if let Some(cell) = &entry.cell {
        return round(flow, names, cell, entry).map(Step::Round);
    }
    if let Some(main) = &entry.main {
        return Ok(Step::Main(main.clone()));
    }
    match entry.owner.as_deref() {
        Some("background") => text().map(Step::Background).map_err(|w| vec![w]),
        Some("words") => {
            let to = entry.to.as_deref().unwrap_or_default();
            let words =
                (agent(flow, to)).and_then(|spec| spec.watches.iter().find_map(|w| w.of_source(flow, flow::WORDS)));
            match (words, text()) {
                (Some(_), Ok(text)) => Ok(Step::Words {
                    to: CellId::from(to),
                    text,
                }),
                (None, _) => Err(vec![format!(
                    "words go to an agent's cell that has words of its own, not {to:?}"
                )]),
                (_, Err(why)) => Err(vec![why]),
            }
        }
        Some("answer") => match (&entry.question, entry.option) {
            (Some(question), Some(option)) if option > 0 => Ok(Step::Answer {
                question: question.clone(),
                option,
            }),
            _ => Err(vec![
                "an answer names its `question` and the `option` taken, counted from one".into(),
            ]),
        },
        other => Err(vec![format!(
            "the owner's act is background, words or answer, not {other:?}"
        )]),
    }
}

/// A round of the cell, its blocks resolved and checked against the cell's outputs.
fn round(flow: &Compiled, names: &Names, cell: &str, entry: &written::Entry) -> Result<Round, Vec<String>> {
    let Some(spec) = flow.cells.iter().find(|c| c.id.as_str() == cell) else {
        return Err(vec![format!("{cell} is no cell of the flow")]);
    };
    let mut wrong = Vec::new();
    let mut blocks: Vec<(String, Value)> = Vec::new();
    if let Some(set) = &entry.set {
        match names.sets.get(set) {
            Some(of) => blocks.extend(of.iter().map(|(b, v)| (b.clone(), v.clone()))),
            None => wrong.push(format!("no set is named {set}")),
        }
    }
    for (block, value) in &entry.blocks {
        if blocks.iter().any(|(b, _)| b == block) {
            wrong.push(format!("{block} is handed in twice"));
        }
        blocks.push((block.clone(), value.clone()));
    }
    let outputs = (flow.flow.place(&spec.id)).map_or(&[][..], |p| &flow.flow.cells()[p].outputs[..]);
    let mut hands_in = Vec::new();
    for (block, value) in blocks {
        let id = BlockId::from(block.as_str());
        if !outputs.contains(&id) {
            wrong.push(format!("{block} is none of what {cell} writes"));
            continue;
        }
        match hand(flow, names, &id, value) {
            Ok(hand) => hands_in.push(hand),
            Err(why) => wrong.push(format!("{block}: {why}")),
        }
    }
    match wrong.is_empty() {
        true => Ok(Round {
            cell: spec.id.clone(),
            hands_in,
            commit: entry.commit.clone(),
        }),
        false => Err(wrong),
    }
}

/// A block as handed in, its content resolved, or why it cannot be.
fn hand(flow: &Compiled, names: &Names, block: &BlockId, value: Value) -> Result<Hand, String> {
    let version = match value {
        Value::Content(content) => written::Version {
            content: Some(content),
            ..written::Version::default()
        },
        Value::Table(version) => version,
    };
    let def = flow.blocks.get(block).ok_or("no block of the flow has this name")?;
    let content = match (&version.content, &version.text) {
        (Some(content), None) => content.clone(),
        (None, Some(name)) => (names.texts.get(name).cloned()).ok_or_else(|| format!("no text is named {name}"))?,
        (None, None) => String::new(),
        (Some(_), Some(_)) => return Err("it gives `content` or a named `text`, not both".into()),
    };
    match (&def.values, &version.value) {
        (Some(values), Some(value)) if !values.contains(value) => {
            return Err(format!("{value} is none of its values ({})", values.join(", ")));
        }
        (Some(_), None) => return Err("it is a choice, handed in with a value".into()),
        (None, Some(_)) => return Err("it is no choice, and takes no value".into()),
        _ => {}
    }
    match (&def.shape, &version.shaped) {
        (Some(_), None) => return Err("its form is a shape, handed in as `shaped`".into()),
        (None, Some(_)) => return Err("its form is no shape, and takes no `shaped`".into()),
        (Some(_), Some(_)) if !content.is_empty() => return Err("a shaped block has no content beside".into()),
        _ => {}
    }
    if def.values.is_none() && def.shape.is_none() && content.trim().is_empty() && !version.unchanged {
        return Err("it is handed in with nothing in it".into());
    }
    Ok(Hand {
        block: block.clone(),
        content,
        value: version.value.clone(),
        unchanged: version.unchanged,
        shaped: version.shaped.clone(),
    })
}

/// Every reason what the case says its cell owes is none the cell can owe.
fn owed(flow: &Compiled, spec: &CellSpec, owes: &Owes) -> Vec<String> {
    let outputs = (flow.flow.place(&spec.id)).map_or(&[][..], |p| &flow.flow.cells()[p].outputs[..]);
    let writes = |name: &str| outputs.iter().any(|b| b.as_str() == name);
    // A group whose blocks the cell writes, every one of them.
    let group = |name: &str| (flow.groups.get(name)).is_some_and(|blocks| blocks.iter().all(|b| writes(b)));
    let mut wrong = Vec::new();
    for (block, value) in &owes.writes {
        let choice = flow
            .blocks
            .get(&BlockId::from(block.as_str()))
            .and_then(|b| b.values.as_ref());
        if !writes(block) {
            wrong.push(format!("owes: writes {block}, none of what {} writes", spec.id));
        } else if value != WRITTEN && !choice.is_some_and(|values| values.contains(value)) {
            wrong.push(format!(
                "owes: writes {block} = {value}: `{WRITTEN}`, or one of a choice's values"
            ));
        }
    }
    for block in &owes.not_writes {
        if !writes(block) {
            wrong.push(format!("owes: not_writes {block}, none of what {} writes", spec.id));
        }
    }
    for name in owes.changes.iter().chain(&owes.keeps) {
        if !writes(name) && !group(name) {
            wrong.push(format!("owes: {name} is no datum or group {} writes", spec.id));
        }
    }
    for name in &owes.answers {
        let by = flow.blocks.get(&BlockId::from(name.as_str())).map(|b| &b.writers);
        let others = by.is_some_and(|w| w.cells.iter().any(|c| *c != spec.id) || w.outside.is_some());
        if !writes(name) || !others {
            wrong.push(format!(
                "owes: answers {name}, which is no dialogue {} writes in",
                spec.id
            ));
        }
    }
    if let Some(question) = &owes.question
        && !QUESTION.contains(&question.as_str())
    {
        wrong.push(format!(
            "owes: question is one of {}, not {question}",
            QUESTION.join(", ")
        ));
    }
    wrong
}
