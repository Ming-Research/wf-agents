//! The flow as the exploration reads it, every name taken from the compiled definition: what each cell may return,
//! what the outside may write, and how far a write reaches. No cell, block or trigger of a flow is named here: what the
//! owner writes, the record of what is said and the record of the questions are found by the source each block's file
//! declares, as the engine finds them (decision every-block-is-written-in-outputs); only the engine's own sources and
//! acts are named, as the engine names them.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use decide::run::{Answer, Told};
use decide::{BlockId, CallId, Cell, CellId, Condition, RunId, TestKind};
use engine::asked::ASKED;
use engine::{Ask, Came, Handed, Question, Way};
use flow::{ANSWER, Compiled, Kind, WORDS};

/// The sources outside the work the driver names: the start, the items the work depends on, main, and the sessions of
/// the lines, each line writing a block of its own.
pub use driver::{DEPENDENCIES, MAIN, SESSION, START};
/// The sources outside the work the carriers read of its branch: the design tree, and the work's head, as a commit no
/// round made moves it.
pub const TREE: &str = "tree";
pub const HEAD: &str = "head";

/// The engine's act that checks the work before the merge.
pub const CHECK: &str = "check";

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
    /// The content, and a question that does not hold the work: it goes on by the question's recommendation, and the
    /// merge waits for the answer.
    AsksGoingOn,
    /// The content, with each block of its that a test of content reads written holding nothing.
    Empty,
    /// The check's act, the work not passing: each choice a precondition waits on with another value, and every
    /// other block it writes.
    Fails(&'static str),
    /// An entry of each record its cell asks for tests in, and nothing else, dealing with nothing: the cell waits for
    /// the result. Where the cell commits, the round commits as it ends, the tests running on the commit.
    Tests,
    /// A call of the first tool the cell may ask for, and nothing else: the cell waits for its result and is entered
    /// again with it.
    Calls,
}

/// The check's act on a red check, and on a branch that does not merge into main.
pub const RED: &str = "a red check";
pub const CONFLICT: &str = "a merge conflict";
/// The check's act on a red check that main fails too: its choice alone, waiting for main to move.
pub const RED_ON_MAIN: &str = "a red check that main fails too";

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
            Variant::AsksGoingOn => write!(f, "its content, and a question it goes on past"),
            Variant::Fails(why) => write!(f, "{why}"),
            Variant::Tests => write!(f, "an entry asking for tests, whose result it waits for"),
            Variant::Calls => write!(f, "a call of a tool, whose result it waits for"),
            Variant::Empty => write!(f, "its content, with what a test of content reads holding nothing"),
        }
    }
}

/// What the outside writes, or the owner does, beside the cells.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Event {
    /// The owner speaks to the line, to the cell the engine chooses on it.
    Speaks(String),
    /// The owner writes again what the owner writes of the work, its background among it.
    Background,
    /// The owner answers the call on the owner, the owner's word at a run's limit of returns, letting the run go on.
    Answers(CallId),
    /// The owner answers the latest version of the question, taking this option, the first being the one it
    /// recommends.
    Answer { question: String, option: u64 },
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
            Event::Speaks(line) => write!(f, "the owner speaks to the line {line}"),
            Event::Background => write!(f, "the owner changes the background"),
            Event::Answers(call) => write!(f, "the owner answers the call {}", call.0),
            Event::Answer { question, option: 1 } => write!(f, "the owner answers {question} as it recommends"),
            Event::Answer { question, .. } => write!(f, "the owner takes another option for {question}"),
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
    /// Each block a test of content reads.
    pub contents: BTreeSet<BlockId>,
    /// Of each cell by its place, the blocks it writes that are no choice: those only cells after it run, those a
    /// cell before it runs, and those that run no cell. A cell whose condition wants another value of a block than
    /// the writer's does is no reader, unless the writer writes that block itself: the two never run one after the
    /// other.
    pub forward: Vec<Vec<BlockId>>,
    pub backward: Vec<Vec<BlockId>>,
    pub silent: Vec<Vec<BlockId>>,
    /// Of each cell by its place, the records it asks for tests in, which none of the three holds: a return writes one
    /// only to ask.
    pub asking: Vec<Vec<BlockId>>,
    /// The place of the engine's act that runs the tests the cells ask for, where the flow has one.
    pub tests: Option<usize>,
    /// Of each cell by its place, the trigger each block that runs it stands for, by the trigger's name.
    pub triggers: Vec<BTreeMap<BlockId, String>>,
    /// Of each cell by its place, its groups' triggers, each with its blocks.
    pub groups: Vec<BTreeMap<String, Vec<BlockId>>>,
    /// Of each cell by its place, its block of what the owner says to it and of an answer to its questions, where it
    /// has them: a trigger of its own on a block the source `words`, or `answer`, writes.
    pub spoken: Vec<Option<BlockId>>,
    pub answered: Vec<Option<BlockId>>,
    /// Of each cell by its place, whether it may ask the owner: the record of the questions, a block the source
    /// `answer` writes, is among its outputs.
    pub asks_owner: Vec<bool>,
    /// The blocks the owner writes of the work, as the work starts and again later: those people write in words, no
    /// notes to a cell and no choice, as the owner's ruling on a verdict is.
    pub people: Vec<BlockId>,
    /// Each pair of cells whose conditions want another value of a block, by their places, with the block.
    pub contradicting: Vec<(usize, usize, BlockId)>,
    /// The block each source outside the work but `session` writes.
    pub sources: BTreeMap<&'static str, BlockId>,
    /// The block of the source `session` of each line whose cells read one, by the line.
    pub sessions: BTreeMap<String, BlockId>,
    /// The place of the check's cell, where the flow has one.
    pub check: Option<usize>,
    /// Of each cell by its place, of each block that runs it and that an act of the engine's after it writes, what
    /// runs that act again, as the engine reads it of the graph.
    pub answered_by: Vec<BTreeMap<BlockId, Answer>>,
    /// Of each cell by its place, a block that is no choice, that it does not write and that no source outside the work
    /// writes, where the flow has one: what a return sent back writes.
    stray: Vec<Option<BlockId>>,
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
            let Some(is) = &test.is else { continue };
            for b in &test.reads {
                refusals.insert(b.clone(), is.clone());
            }
        }
        let contents = (flow.tests().values())
            .filter(|t| t.kind == TestKind::AnyContent)
            .flat_map(|t| t.reads.iter().cloned())
            .collect();
        let mut passing = BTreeMap::new();
        for key in cells.iter().flat_map(|c| &c.requires) {
            match key {
                Condition::Is { block, value } => {
                    passing.insert(block.clone(), value.clone());
                }
                Condition::Test(name) if flow.tests()[name].kind == TestKind::All => {
                    let Some(is) = &flow.tests()[name].is else { continue };
                    for b in &flow.tests()[name].reads {
                        passing.insert(b.clone(), is.clone());
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
        let asking: Vec<Vec<BlockId>> = (cells.iter())
            .map(|c| c.asks.iter().map(|asks| asks.record.clone()).collect())
            .collect();
        for (i, cell) in cells.iter().enumerate() {
            let (mut f, mut b, mut s) = (Vec::new(), Vec::new(), Vec::new());
            let prose = (cell.outputs.iter()).filter(|b| !choices.contains_key(*b) && !asking[i].contains(*b));
            for block in prose {
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
            let own = |source: &str| {
                (spec.triggers.iter())
                    .filter(|t| t.group.is_empty())
                    .find(|t| {
                        (compiled.blocks.get(&t.block)).is_some_and(|b| b.writers.outside.as_deref() == Some(source))
                    })
                    .map(|t| t.block.clone())
            };
            spoken.push(own(WORDS));
            answered.push(own(ANSWER));
        }
        let record =
            |b: &BlockId| (compiled.blocks.get(b)).is_some_and(|b| b.writers.outside.as_deref() == Some(ANSWER));
        let asks_owner = (cells.iter()).map(|c| c.outputs.iter().any(record)).collect();
        let people = (compiled.blocks.iter())
            .filter(|(_, b)| b.writers.people && !b.writers.note && b.values.is_none())
            .map(|(block, _)| block.clone())
            .collect();
        let sources = [START, DEPENDENCIES, MAIN, TREE, HEAD]
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
        let check = (compiled.cells.iter()).position(|c| c.act.as_deref() == Some(CHECK));
        let tests = (compiled.cells.iter()).position(|c| c.act.as_deref() == Some(flow::RUNS_TESTS));
        let asked = (compiled.blocks.iter()).find(|(_, b)| b.writers.outside.as_deref() == Some(ASKED));
        let answered_by = (0..cells.len())
            .map(|i| {
                let mut answers: BTreeMap<BlockId, Answer> = BTreeMap::new();
                for block in &cells[i].runs_on {
                    let acts = (compiled.cells.iter().zip(cells)).skip(i + 1);
                    for (_, act) in acts.filter(|(spec, act)| spec.kind == Kind::Engine && act.outputs.contains(block))
                    {
                        let answer = answers.entry(block.clone()).or_default();
                        answer.runs.extend(act.runs_on.iter().cloned());
                        answer.question |= asked.is_some_and(|(id, _)| act.runs_on.contains(id));
                    }
                }
                answers
            })
            .collect();
        let stray = (cells.iter())
            .map(|cell| {
                (compiled.blocks.iter())
                    .find(|(b, declared)| {
                        !choices.contains_key(*b) && !cell.outputs.contains(b) && declared.writers.outside.is_none()
                    })
                    .map(|(b, _)| b.clone())
            })
            .collect();
        Self {
            compiled,
            choices,
            refusals,
            passing,
            contents,
            forward,
            backward,
            silent,
            asking,
            tests,
            triggers,
            groups,
            spoken,
            answered,
            asks_owner,
            people,
            contradicting,
            sources,
            sessions,
            check,
            answered_by,
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
    /// condition or preconditions read one, and so on through what each of those writes or commits, and through the
    /// result of an entry of a record of asking, which the act the record runs writes from outside.
    pub fn reach(&self, from: &[BlockId]) -> BTreeSet<usize> {
        let cells = self.cells();
        let mut blocks: Vec<BlockId> = from.to_vec();
        let mut seen = BTreeSet::new();
        let mut reached = BTreeSet::new();
        while let Some(block) = blocks.pop() {
            if !seen.insert(block.clone()) {
                continue;
            }
            for asks in cells.iter().flat_map(|c| &c.asks).filter(|asks| asks.record == block) {
                blocks.push(asks.result.clone());
            }
            for (j, cell) in cells.iter().enumerate() {
                let reads = cell.runs_on.contains(&block)
                    || (cell.applies.iter().chain(&cell.requires)).any(|k| self.key_reads(k).contains(&block));
                if reads && reached.insert(j) {
                    blocks.extend(cell.outputs.iter().cloned());
                    blocks.extend(self.commits(j).cloned());
                }
            }
        }
        reached
    }

    /// The work's head, of the agent cell at this place that answers an act of the engine's by it: a round of it that
    /// deals with what the act wrote commits its fix, which moves the head.
    pub fn commits(&self, i: usize) -> Option<&BlockId> {
        let head = self.sources.get(HEAD)?;
        let answers = self.agent(i) && self.answered_by[i].values().any(|a| a.runs.contains(head));
        answers.then_some(head)
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
            if Some(i) == self.check {
                let mut out = vec![(Variant::Fails(RED), true), (Variant::Fails(CONFLICT), false)];
                if self.sources.contains_key(MAIN) {
                    out.push((Variant::Fails(RED_ON_MAIN), false));
                }
                return out;
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
        // A question, only of a cell that may ask the owner: one it waits on, with its content's choices or with
        // choices that skip the cell itself, which it leaves behind; and one it goes on past.
        if self.asks_owner[i] {
            let own: Vec<BlockId> = cell.applies.iter().flat_map(|k| self.key_reads(k)).collect();
            for set in sets.iter().filter(|set| set.keys().all(|b| own.contains(b))) {
                out.push((Variant::Asks(set.clone()), false));
            }
            out.push((Variant::AsksGoingOn, false));
        }
        for set in sets.into_iter().filter(|set| !set.is_empty()) {
            out.push((Variant::Choices(set), false));
        }
        if !self.forward[i].is_empty() {
            out.push((Variant::Bare, false));
        }
        if self.forward[i].iter().any(|b| self.contents.contains(b)) {
            out.push((Variant::Empty, false));
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
        // Asking at every round is judged by the history of asking, which the engine keeps beside the walk: once is
        // what the walk itself reads.
        if !self.asking[i].is_empty() {
            out.push((Variant::Tests, false));
        }
        if !self.compiled.cells[i].tools.is_empty() {
            out.push((Variant::Calls, false));
        }
        if self.stray[i].is_some() {
            out.push((Variant::SentBack, true));
        }
        out
    }

    /// What the return of the round of the cell at this place deals with, with what the round was told: of an agent's,
    /// each trigger the round was told ran it, named as its agent names it and read as the agents' rounds read a return
    /// (`CellSpec::dealt_with`).
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

    /// What the round of this number of the run of the cell at this place returns as this variant, with what it was
    /// told, as its carrier hands it to the engine: an agent's return naming what it dealt with, its questions beside
    /// its work, and the work's head where the round commits; an act of the engine's, naming nothing it dealt with.
    pub fn came(&self, i: usize, variant: &Variant, told: &Told, run: RunId, round: usize) -> Came {
        let cell = &self.cells()[i];
        let handed = |block: &BlockId, value: Option<String>, empty: bool| Handed {
            block: block.clone(),
            content: if empty {
                String::new()
            } else {
                format!("The {block} as {} wrote it in run {}, round {round}.", cell.id, run.0)
            },
            matters: true,
            value,
        };
        let head = |came: &mut Came| {
            if let Some(head) = self.commits(i) {
                let content = format!("The head as {} committed it in run {}, round {round}.", cell.id, run.0);
                came.read = vec![Handed {
                    block: head.clone(),
                    content,
                    matters: true,
                    value: None,
                }];
            }
        };
        if *variant == Variant::Tests {
            let writes = self.asking[i].iter().map(|b| handed(b, None, false)).collect();
            let mut came = Came::works(Vec::new(), writes);
            head(&mut came);
            return came;
        }
        if *variant == Variant::Calls {
            let name = self.compiled.cells[i].tools[0].clone();
            let input = format!("What {} asks {name}.", cell.id);
            return Came::asks(vec![Ask::Tool { name, input }]);
        }
        if *variant == Variant::SentBack {
            let stray = self.stray[i].as_ref().expect("a block the cell does not write");
            return Came::works(self.dealt_with(i, told), vec![handed(stray, None, false)]);
        }
        let ran_by = told.ran_by.clone();
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
                Variant::Content
                | Variant::AsksGoingOn
                | Variant::Empty
                | Variant::SentBack
                | Variant::Fails(_)
                | Variant::Calls
                | Variant::Tests => {}
            }
        } else {
            let fails = matches!(variant, Variant::Fails(_));
            for (block, value) in values.iter_mut() {
                if let (Some(pass), true) = (self.passing.get(block), fails) {
                    let other = self.choices[block].iter().find(|v| *v != pass);
                    *value = other.expect("a choice has another value").clone();
                }
            }
            let on_main = *variant == Variant::Fails(RED_ON_MAIN);
            if Some(i) != self.check || (fails && !on_main) {
                prose.extend(
                    (cell.outputs.iter())
                        .filter(|b| !self.choices.contains_key(*b))
                        .cloned(),
                );
            }
        }
        // The record of the questions is handed in as the questions it holds, never as a block of the cell's: the
        // engine writes its entries (decision every-block-is-written-in-outputs).
        let recorded =
            |b: &BlockId| (self.compiled.blocks.get(b)).is_some_and(|d| d.writers.outside.as_deref() == Some(ANSWER));
        let writes = (cell.outputs.iter())
            .filter(|block| !recorded(block))
            .filter_map(|block| {
                let value = values.get(block).cloned();
                let empty = *variant == Variant::Empty && self.contents.contains(block);
                (value.is_some() || prose.contains(block)).then(|| handed(block, value, empty))
            })
            .collect();
        if !self.agent(i) {
            let mut came = Came::hands_in(writes);
            // What failed fails on main too: the check writes its choice alone and waits for main to move (decision
            // a-red-the-change-did-not-make-is-not-sent-back).
            if *variant == Variant::Fails(RED_ON_MAIN) {
                came.waits_for = self.sources.get(MAIN).cloned();
            }
            return came;
        }
        let dealt_with = self.dealt_with(i, told);
        let waits = match variant {
            Variant::Asks(_) => Some(true),
            Variant::AsksGoingOn => Some(false),
            _ => None,
        };
        // A round that deals with what an act after its cell wrote commits its fix, which moves the work's head, read
        // as the round ends.
        let commits = (self.commits(i)).is_some_and(|head| {
            (dealt_with.iter())
                .filter_map(|block| self.answered_by[i].get(block))
                .any(|a| a.runs.contains(head))
        });
        let mut came = Came::works(dealt_with, writes);
        if let Some(waits) = waits {
            came.said.questions = vec![self.question(i, waits)];
        }
        if commits {
            head(&mut came);
        }
        came
    }

    /// The question the cell at this place asks the owner, under one name for the cell, so that a later one asks it
    /// anew: two options, the first recommended.
    fn question(&self, i: usize, waits: bool) -> Question {
        let cell = &self.cells()[i].id;
        let way = |n: u64| Way {
            does: format!("The way {n} {cell} names."),
            follows: format!("What follows from the way {n}."),
        };
        Question::Asked {
            name: question_of(cell),
            waits,
            recommends: Some(1),
            ask: format!("Which way does {cell} take?"),
            options: vec![way(1), way(2)],
            about: None,
            reasons: BTreeMap::new(),
        }
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
    /// that runs the first again. The result of what a cell asked for is no opinion: it ends a wait.
    pub fn loops(&self) -> BTreeSet<(BlockId, BlockId)> {
        let cells = self.cells();
        let mut pairs = BTreeSet::new();
        for (a, writer) in cells.iter().enumerate() {
            for o in self.backward[a].iter().filter(|o| !self.compiled.flow.is_result(o)) {
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

/// The name of the question the cell asks the owner.
pub fn question_of(cell: &CellId) -> String {
    format!("{}_way", cell.as_str().to_lowercase())
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
