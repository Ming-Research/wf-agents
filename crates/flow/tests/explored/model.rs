//! The flow as the exploration reads it, every name taken from the compiled definition: what each cell may return,
//! what the outside may write, and how far a write reaches. Only the data of the engine's (`flow/data/`), the triggers
//! beside the flows (`flow/triggers/`) and the acts of the engine's are named here, as the files beside the flows name
//! them.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use decide::run::{Return, Told};
use decide::{BlockId, CallId, Cell, CellId, Condition, TestKind, Write};
use flow::{Compiled, Kind};

/// The data of the engine's that people write: what the item asks, what the owner says to the work, and the answers.
pub const BACKGROUND: &str = "background";
pub const DISCUSSION: &str = "discussion";
pub const ANSWERS: &str = "answers";

/// The triggers of a cell's own that the owner fires: speaking to it, and answering one of its questions.
const SPOKEN: &str = "on_spoken";
const ANSWERED: &str = "on_answer";

/// The sources outside the work: the start, the items the work depends on, main, the sessions of the lines, each line
/// writing a block of its own, and the design tree.
pub const START: &str = "start";
pub const DEPENDENCIES: &str = "dependencies";
pub const MAIN: &str = "main";
pub const SESSION: &str = "session";
pub const TREE: &str = "tree";

/// The engine's act that checks the work before the merge.
const CHECK: &str = "check";

/// What a round of a cell returns.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Variant {
    /// What a cell content with its work returns: it deals with all that ran it; each verdict it writes is no
    /// refusal, each choice a precondition waits on has the value waited for, and every other choice is as it was, or
    /// its default or last value where it was never written; it writes each other block of its that only cells after
    /// it run, and its reply where the owner spoke to it; an act of the engine's writes each block it writes, the
    /// check only its choice where the work passed.
    Content,
    /// The content, with these choices written with these values, each another than the content's.
    Choices(BTreeMap<BlockId, String>),
    /// The content's choices, and no other block.
    Bare,
    /// The content, and each block of its that a cell before it runs: its opinions for those cells.
    Opinions,
    /// The content, and this one of its opinions.
    Opinion(BlockId),
    /// A judge's refusal with its reasons: each verdict it writes as the refusal, and its opinions.
    Refuses,
    /// A return that does not meet what the flow requires, which is sent back.
    SentBack,
    /// The content with these choices, each another than the content's, and a question that waits for the owner's
    /// answer.
    Asks(BTreeMap<BlockId, String>),
    /// The check's act, the work not passing: each choice a precondition waits on with another value, and every
    /// other block it writes.
    Fails(&'static str),
}

/// The check's act on a red check, and on a branch that does not merge into main.
pub const RED: &str = "a red check";
pub const CONFLICT: &str = "a merge conflict";

impl fmt::Display for Variant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Variant::Content => write!(f, "its content"),
            Variant::Choices(values) => {
                let values: Vec<String> = values.iter().map(|(b, v)| format!("{b} = {v}")).collect();
                write!(f, "its content with {}", values.join(", "))
            }
            Variant::Bare => write!(f, "its choices alone"),
            Variant::Opinions => write!(f, "its content and its opinions for the cells before it"),
            Variant::Opinion(block) => write!(f, "its content and its opinion in {block}"),
            Variant::Refuses => write!(f, "a refusal with its reasons"),
            Variant::SentBack => write!(f, "what does not meet the flow"),
            Variant::Asks(values) => {
                let values: Vec<String> = values.iter().map(|(b, v)| format!(" with {b} = {v}")).collect();
                write!(f, "its content{}, and a question it waits on", values.concat())
            }
            Variant::Fails(why) => write!(f, "{why}"),
        }
    }
}

/// What the outside writes, or the owner does, beside the cells.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Event {
    /// The owner speaks to the cell: its block of `on_spoken`, and `discussion`.
    Speaks(CellId),
    /// The owner writes the background again.
    Background,
    /// The owner answers the call: a question's, with `answers` and the asking cell's block of `on_answer` where it
    /// has one; or the owner's word at a run's limit of returns, letting the run go on.
    Answers(CallId),
    /// The owner answers a question of the cell's with another option than its own: `answers`, and its block of
    /// `on_answer`.
    OtherOption(CellId),
    /// The owner lets the cell go on at its limit.
    LetsGo(CellId),
    /// The source outside the work writes its block: main moves, the tree changes.
    Source(&'static str),
    /// The line begins a new session, which writes its own block of the source `session`.
    Session(String),
    /// The items the work depends on have all merged.
    Dependencies,
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Event::Speaks(cell) => write!(f, "the owner speaks to {cell}"),
            Event::Background => write!(f, "the owner changes the background"),
            Event::Answers(call) => write!(f, "the owner answers the call {}", call.0),
            Event::OtherOption(cell) => write!(f, "the owner takes another option for a question of {cell}"),
            Event::LetsGo(cell) => write!(f, "the owner lets {cell} go on at its limit"),
            Event::Source(MAIN) => write!(f, "main moves"),
            Event::Source(source) => write!(f, "the outside writes {source}"),
            Event::Session(line) => write!(f, "a session of {line} is begun anew"),
            Event::Dependencies => write!(f, "the items the work depends on merge"),
        }
    }
}

/// The compiled flow, with what the exploration works out of it once.
pub struct Model {
    pub compiled: Compiled,
    /// The values of each choice.
    pub choices: BTreeMap<BlockId, Vec<String>>,
    /// Each verdict, a choice a test of what stands reads, with its refusal.
    pub refusals: BTreeMap<BlockId, String>,
    /// Each choice a precondition waits on, with the value it waits for.
    pub passing: BTreeMap<BlockId, String>,
    /// Of each cell by its place, the blocks it writes that are no choice: those only cells after it run, those a
    /// cell before it runs, and those that run no cell. A cell whose condition wants another value of a block than
    /// the writer's does is no reader, unless the writer writes that block itself: the two never run one after the
    /// other.
    pub forward: Vec<Vec<BlockId>>,
    pub backward: Vec<Vec<BlockId>>,
    pub silent: Vec<Vec<BlockId>>,
    /// Of each cell by its place, the trigger each block that runs it stands for, by the trigger's name.
    pub triggers: Vec<BTreeMap<BlockId, String>>,
    /// Of each cell by its place, its groups' triggers, each with its blocks.
    pub groups: Vec<BTreeMap<String, Vec<BlockId>>>,
    /// Of each cell by its place, its block of `on_spoken` and of `on_answer`, where it has them.
    pub spoken: Vec<Option<BlockId>>,
    pub answered: Vec<Option<BlockId>>,
    /// Each pair of cells whose conditions want another value of a block, by their places, with the block.
    pub contradicting: Vec<(usize, usize, BlockId)>,
    /// The block each source outside the work but `session` writes.
    pub sources: BTreeMap<&'static str, BlockId>,
    /// The block of the source `session` of each line whose cells read one, by the line.
    pub sessions: BTreeMap<String, BlockId>,
    /// The place of the check's cell.
    pub check: usize,
    /// Of each cell by its place, a block that is no choice and that it does not write: what a return sent back
    /// writes.
    stray: Vec<BlockId>,
}

impl Model {
    pub fn new(compiled: Compiled) -> Self {
        let flow = &compiled.flow;
        let cells = flow.cells();
        let choices: BTreeMap<BlockId, Vec<String>> = (compiled.blocks.iter())
            .filter_map(|(b, declared)| declared.values.clone().map(|values| (b.clone(), values)))
            .collect();
        let mut refusals = BTreeMap::new();
        for test in flow.tests().values().filter(|t| t.kind == TestKind::NoneStanding) {
            for b in &test.reads {
                refusals.insert(b.clone(), test.is.clone());
            }
        }
        let mut passing = BTreeMap::new();
        for key in cells.iter().flat_map(|c| &c.requires) {
            match key {
                Condition::Is { block, value } => {
                    passing.insert(block.clone(), value.clone());
                }
                Condition::Test(name) if flow.tests()[name].kind == TestKind::All => {
                    for b in &flow.tests()[name].reads {
                        passing.insert(b.clone(), flow.tests()[name].is.clone());
                    }
                }
                Condition::Test(_) => {}
            }
        }
        let wants = |c: &Cell| -> Vec<(BlockId, String)> {
            (c.applies.iter())
                .filter_map(|key| match key {
                    Condition::Is { block, value } => Some((block.clone(), value.clone())),
                    Condition::Test(_) => None,
                })
                .collect()
        };
        let mut contradicting = Vec::new();
        for i in 0..cells.len() {
            for j in i + 1..cells.len() {
                for (b, v) in wants(&cells[i]) {
                    if wants(&cells[j]).iter().any(|(b2, v2)| *b2 == b && *v2 != v) {
                        contradicting.push((i, j, b.clone()));
                    }
                }
            }
        }
        let apart = |i: usize, j: usize| {
            (contradicting.iter())
                .any(|(a, b, x)| ((*a == i && *b == j) || (*a == j && *b == i)) && !cells[i].outputs.contains(x))
        };
        let (mut forward, mut backward, mut silent) = (Vec::new(), Vec::new(), Vec::new());
        for (i, cell) in cells.iter().enumerate() {
            let (mut f, mut b, mut s) = (Vec::new(), Vec::new(), Vec::new());
            for block in cell.outputs.iter().filter(|b| !choices.contains_key(*b)) {
                let readers: Vec<usize> = (cells.iter().enumerate())
                    .filter(|(j, c)| *j != i && c.runs_on.contains(block) && !apart(i, *j))
                    .map(|(j, _)| j)
                    .collect();
                if readers.is_empty() {
                    s.push(block.clone());
                } else if readers.iter().any(|j| *j < i) {
                    b.push(block.clone());
                } else {
                    f.push(block.clone());
                }
            }
            forward.push(f);
            backward.push(b);
            silent.push(s);
        }
        let mut triggers = Vec::new();
        let mut groups = Vec::new();
        let mut spoken = Vec::new();
        let mut answered = Vec::new();
        for spec in &compiled.cells {
            let mut of_cell = BTreeMap::new();
            let mut of_groups = BTreeMap::new();
            for t in &spec.triggers {
                if t.group.is_empty() {
                    of_cell.entry(t.block.clone()).or_insert_with(|| t.name.clone());
                } else {
                    of_groups.insert(t.name.clone(), t.group.clone());
                    for member in &t.group {
                        of_cell.entry(member.clone()).or_insert_with(|| t.name.clone());
                    }
                }
            }
            triggers.push(of_cell);
            groups.push(of_groups);
            let own = |name: &str| (spec.triggers.iter()).find(|t| t.name == name).map(|t| t.block.clone());
            spoken.push(own(SPOKEN));
            answered.push(own(ANSWERED));
        }
        let sources = [START, DEPENDENCIES, MAIN, TREE]
            .into_iter()
            .filter_map(|source| {
                (compiled.blocks.iter())
                    .find(|(_, b)| b.writers.outside.as_deref() == Some(source))
                    .map(|(block, _)| (source, block.clone()))
            })
            .collect();
        let line_of = |cell: &CellId| {
            (compiled.cells.iter())
                .find(|c| &c.id == cell)
                .and_then(|c| c.line.clone())
        };
        let sessions = (compiled.blocks.iter())
            .filter(|(_, b)| b.writers.outside.as_deref() == Some(SESSION))
            .flat_map(|(block, b)| {
                (b.readers.cells.iter()).filter_map(move |cell| line_of(cell).map(|line| (line, block.clone())))
            })
            .collect();
        let check = (compiled.cells.iter())
            .position(|c| c.act.as_deref() == Some(CHECK))
            .expect("the flow has a check");
        let stray = (cells.iter())
            .map(|cell| {
                (compiled.blocks.keys())
                    .find(|b| !choices.contains_key(*b) && !cell.outputs.contains(b))
                    .expect("a block the cell does not write")
                    .clone()
            })
            .collect();
        Self {
            compiled,
            choices,
            refusals,
            passing,
            forward,
            backward,
            silent,
            triggers,
            groups,
            spoken,
            answered,
            contradicting,
            sources,
            sessions,
            check,
            stray,
        }
    }

    pub fn cells(&self) -> &[Cell] {
        self.compiled.flow.cells()
    }

    pub fn place(&self, cell: &CellId) -> usize {
        self.compiled.flow.place(cell).expect("a cell of the flow")
    }

    /// Whether an agent carries the cell at this place out.
    pub fn agent(&self, i: usize) -> bool {
        self.compiled.cells[i].kind == Kind::Agent
    }

    /// The blocks a key of a condition or of preconditions reads.
    pub fn key_reads(&self, key: &Condition) -> Vec<BlockId> {
        match key {
            Condition::Is { block, .. } => vec![block.clone()],
            Condition::Test(name) => self.compiled.flow.tests()[name].reads.clone(),
        }
    }

    /// The cells, by their places, a write of these blocks can run again: each that one of them runs, or whose
    /// condition or preconditions read one, and so on through what each of those writes.
    pub fn reach(&self, from: &[BlockId]) -> BTreeSet<usize> {
        let cells = self.cells();
        let mut blocks: Vec<BlockId> = from.to_vec();
        let mut seen = BTreeSet::new();
        let mut reached = BTreeSet::new();
        while let Some(block) = blocks.pop() {
            if !seen.insert(block.clone()) {
                continue;
            }
            for (j, cell) in cells.iter().enumerate() {
                let reads = cell.runs_on.contains(&block)
                    || (cell.applies.iter().chain(&cell.requires)).any(|k| self.key_reads(k).contains(&block));
                if reads && reached.insert(j) {
                    blocks.extend(cell.outputs.iter().cloned());
                }
            }
        }
        reached
    }

    /// The value the content of a cell writes into the choice, which has this value now.
    fn content_value(&self, block: &BlockId, now: Option<&String>) -> String {
        let values = &self.choices[block];
        if let Some(refusal) = self.refusals.get(block) {
            return (values.iter())
                .find(|v| *v != refusal)
                .cloned()
                .expect("a value other than the refusal");
        }
        if let Some(pass) = self.passing.get(block) {
            return pass.clone();
        }
        (now.cloned())
            .or_else(|| self.compiled.flow.default_of(block).map(str::to_string))
            .or_else(|| values.last().cloned())
            .expect("a choice has values")
    }

    /// The returns of the cell at this place other than its content, with whether a cell may give the same at every
    /// run from then on: a return that sends work round, which only the flow's limits can stop. Of its choices, each
    /// set of values but the content's, whose values these are now.
    pub fn deviations(&self, i: usize, told: &Told) -> Vec<(Variant, bool)> {
        let spec = &self.compiled.cells[i];
        if spec.kind == Kind::Engine {
            if i == self.check {
                return vec![(Variant::Fails(RED), true), (Variant::Fails(CONFLICT), false)];
            }
            return Vec::new();
        }
        let cell = &self.cells()[i];
        let mut out = Vec::new();
        // Every set of values of its choices, each set given by what differs from the content's.
        let mut sets: Vec<BTreeMap<BlockId, String>> = vec![BTreeMap::new()];
        for block in cell.outputs.iter().filter(|b| self.choices.contains_key(*b)) {
            let now = told.outputs.get(block).and_then(|s| s.value.as_ref());
            let content = self.content_value(block, now);
            let mut more = Vec::new();
            for set in &sets {
                for value in &self.choices[block] {
                    let mut set = set.clone();
                    if *value != content {
                        set.insert(block.clone(), value.clone());
                    }
                    more.push(set);
                }
            }
            sets = more;
        }
        // A question with its content's choices, or with choices that skip the cell itself: one it leaves behind.
        let own: Vec<BlockId> = cell.applies.iter().flat_map(|k| self.key_reads(k)).collect();
        for set in sets.iter().filter(|set| set.keys().all(|b| own.contains(b))) {
            out.push((Variant::Asks(set.clone()), false));
        }
        for set in sets.into_iter().filter(|set| !set.is_empty()) {
            out.push((Variant::Choices(set), false));
        }
        if !self.forward[i].is_empty() {
            out.push((Variant::Bare, false));
        }
        if !self.backward[i].is_empty() {
            out.push((Variant::Opinions, true));
        }
        if self.backward[i].len() > 1 {
            for block in &self.backward[i] {
                out.push((Variant::Opinion(block.clone()), false));
            }
        }
        if cell.outputs.iter().any(|b| self.refusals.contains_key(b)) {
            out.push((Variant::Refuses, true));
        }
        out.push((Variant::SentBack, true));
        out
    }

    /// What the return of the round of the cell at this place deals with, with what the round was told: of an agent's,
    /// each trigger the round was told ran it, named as its agent names it and read as the agents' rounds read a return
    /// (`CellSpec::dealt_with`); of an act of the engine's, which names none, everything that ran the round.
    pub fn dealt_with(&self, i: usize, told: &Told) -> Vec<BlockId> {
        let spec = &self.compiled.cells[i];
        if !self.agent(i) {
            return told.ran_by.clone();
        }
        let names: Vec<String> = spec.ran(&told.ran_by).map(|t| t.name.clone()).collect();
        let (dealt_with, none) = spec.dealt_with(&names);
        assert!(none.is_empty(), "{} names no trigger {none:?}", spec.id);
        dealt_with
    }

    /// What the round of the cell at this place returns as this variant, with what it was told: the return as the
    /// engine's check of it reads it, and whether a question of it waits for the owner.
    pub fn hand_in(&self, i: usize, variant: &Variant, told: &Told) -> (Return, bool) {
        let cell = &self.cells()[i];
        let ran_by = told.ran_by.clone();
        let dealt_with = self.dealt_with(i, told);
        if *variant == Variant::SentBack {
            let stray = Write {
                block: self.stray[i].clone(),
                meaning_unchanged: false,
                value: None,
            };
            let ret = Return::Came {
                dealt_with,
                writes: vec![stray],
                calls: 0,
                found: Vec::new(),
                waits: false,
                questions: Vec::new(),
            };
            return (ret, false);
        }
        let mut values: BTreeMap<BlockId, String> = (cell.outputs.iter())
            .filter(|b| self.choices.contains_key(*b))
            .map(|b| {
                let now = told.outputs.get(b).and_then(|s| s.value.as_ref());
                (b.clone(), self.content_value(b, now))
            })
            .collect();
        let mut prose: Vec<BlockId> = Vec::new();
        if self.agent(i) {
            prose.extend(self.forward[i].iter().cloned());
            let spoken = self.spoken[i].as_ref().is_some_and(|b| ran_by.contains(b));
            if spoken {
                prose.extend(self.silent[i].iter().cloned());
            }
            match variant {
                Variant::Choices(set) | Variant::Asks(set) => {
                    values.extend(set.iter().map(|(b, v)| (b.clone(), v.clone())))
                }
                Variant::Bare => prose.clear(),
                Variant::Opinions => prose.extend(self.backward[i].iter().cloned()),
                Variant::Opinion(block) => prose.push(block.clone()),
                Variant::Refuses => {
                    for (block, value) in values.iter_mut() {
                        if let Some(refusal) = self.refusals.get(block) {
                            *value = refusal.clone();
                        }
                    }
                    prose.extend(self.backward[i].iter().cloned());
                }
                Variant::Content | Variant::SentBack | Variant::Fails(_) => {}
            }
        } else {
            let fails = matches!(variant, Variant::Fails(_));
            for (block, value) in values.iter_mut() {
                if let (Some(pass), true) = (self.passing.get(block), fails) {
                    let other = self.choices[block].iter().find(|v| *v != pass);
                    *value = other.expect("a choice has another value").clone();
                }
            }
            if i != self.check || fails {
                prose.extend(
                    (cell.outputs.iter())
                        .filter(|b| !self.choices.contains_key(*b))
                        .cloned(),
                );
            }
        }
        let writes = (cell.outputs.iter())
            .filter_map(|block| {
                let value = values.get(block).cloned();
                (value.is_some() || prose.contains(block)).then(|| Write {
                    block: block.clone(),
                    meaning_unchanged: false,
                    value,
                })
            })
            .collect();
        let asks = matches!(variant, Variant::Asks(_));
        let ret = Return::Came {
            dealt_with,
            writes,
            calls: 0,
            found: Vec::new(),
            waits: asks,
            questions: Vec::new(),
        };
        (ret, asks)
    }

    /// Each item of coverage the exploration could reach on this flow: the cells, the triggers, the ways back, the
    /// conditions, the gates, the reply loops and the returns of each cell, the acts of the engine's and the stops.
    pub fn universe(&self) -> BTreeSet<String> {
        let cells = self.cells();
        let mut all = BTreeSet::new();
        for (i, cell) in cells.iter().enumerate() {
            all.insert(ran(&cell.id));
            for (block, trigger) in &self.triggers[i] {
                all.insert(ran_on(&cell.id, trigger));
                let later = (cells.iter().enumerate()).any(|(j, c)| j > i && c.outputs.contains(block));
                if later {
                    all.insert(ran_back_on(&cell.id, trigger));
                }
            }
            if !cell.applies.is_empty() {
                all.insert(skipped(&cell.id));
            }
            for key in &cell.requires {
                all.insert(waited_at_gate(&cell.id, key));
            }
        }
        for (o, r) in self.loops() {
            all.insert(reply_loop(&o, &r));
        }
        all
    }

    /// Each pair of an opinion and its reply: a block a cell writes that runs a cell before it, which writes a block
    /// that runs the first again.
    pub fn loops(&self) -> BTreeSet<(BlockId, BlockId)> {
        let cells = self.cells();
        let mut pairs = BTreeSet::new();
        for (a, writer) in cells.iter().enumerate() {
            for o in &self.backward[a] {
                for (b, reader) in cells.iter().enumerate().take(a) {
                    if !reader.runs_on.contains(o) {
                        continue;
                    }
                    // A reply runs the writer by a trigger of its own, not as one of a group, and is no choice.
                    let own = |r: &BlockId| self.groups[a].values().all(|members| !members.contains(r));
                    let replies = (reader.outputs.iter())
                        .filter(|r| writer.runs_on.contains(*r) && *r != o && !self.choices.contains_key(*r) && own(r));
                    for r in replies {
                        if self.backward[b].contains(r) {
                            continue;
                        }
                        // A reply is a block written only to answer: none of the writer's other cells runs on it.
                        let only = (cells.iter().enumerate()).all(|(j, c)| j == a || j == b || !c.runs_on.contains(r));
                        if only {
                            pairs.insert((o.clone(), r.clone()));
                        }
                    }
                }
            }
        }
        pairs
    }
}

pub fn ran(cell: &CellId) -> String {
    format!("{cell} ran")
}

pub fn ran_on(cell: &CellId, trigger: &str) -> String {
    format!("{cell} ran on {trigger}")
}

pub fn ran_back_on(cell: &CellId, trigger: &str) -> String {
    format!("{cell} ran back on {trigger}, written by a cell after it")
}

pub fn skipped(cell: &CellId) -> String {
    format!("{cell} was skipped by its condition with something new to it")
}

pub fn waited_at_gate(cell: &CellId, key: &Condition) -> String {
    format!("the work waited at {cell} on {}", show_key(key))
}

pub fn reply_loop(opinion: &BlockId, reply: &BlockId) -> String {
    format!("the loop of {opinion} and {reply} went round")
}

pub fn show_key(key: &Condition) -> String {
    match key {
        Condition::Is { block, value } => format!("{block} = {value}"),
        Condition::Test(name) => format!("the test {name}"),
    }
}
