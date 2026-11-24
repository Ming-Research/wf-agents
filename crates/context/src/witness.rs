//! A search for the situations a work of a flow can reach, which finds, for each line of what a round stood in
//! ([`crate::situations::stood`]), a history of a work that reaches it, and keeps the history as a script that a fresh
//! simulation ([`crate::sim::Sim`]) runs again to the same situation.
//!
//! It runs works through the engine's own loop with the context manager entering each round as a carrier does, and
//! asks of a work nothing the engine would refuse: people and the outside write the blocks they write, the owner
//! reopens a line's session, speaks to a cell and answers a question, a round is carried out and returns in one of a
//! fixed set of ways (what it hands in, what it deals with, what it asks for, whether it reaches the model, is lost,
//! fails or is compacted), and the owner lets go a cell at its limit. It names no cell, block, line or value of any
//! flow: what it may write, ask and say it reads from the compiled flow and the engine's own rules of who writes what.
//!
//! The search keeps an archive of states of works, each at the entry of a round, one for each state of what the round
//! is sent and of the choices the work holds, and goes on from each in turn: first each way a round returns, then each
//! thing the owner or the outside may do before it, then draws of both, seeded and in a fixed order, so that the
//! same flow gives the same finds. It ends where its rounds are spent or no new line has been found for a stated number
//! of rounds. It finds only what it reaches; where it reaches less than a work can, a line is missing from what is
//! written, and the search is made better in what it does for every flow, never in what it knows of one.
//!
//! The simulation here is the one the tests of the manager run works with, and the search's own set of actions is
//! its own: the tests' histories choose theirs apart from it, so that they can show what the search leaves out.

use std::collections::{BTreeMap, BTreeSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::panic::{AssertUnwindSafe, catch_unwind};

use decide::{BlockId, CellId, Condition, Decision, Outsider, RunId, Seq, Waited, decide};
use engine::spoken::{self, To};
use engine::{Ask, Came, Event, Handed, Question, Way, Why};
use flow::agents::Agents;
use flow::{CellSpec, Compiled, Kind};

use crate::manager::Entered;
use crate::message::Situation;
use crate::sim::{Given, Sim};
use crate::situations::stood;

/// How a round enters its cell's session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    /// The cell has not been entered before.
    First,
    /// The cell was entered before, in the session it is entered in now.
    Later,
    /// The cell was entered before, and its session was begun anew since: the round begins the new one.
    Reopened,
    /// The cell was entered before, and the session was compacted since.
    Compacted,
    /// The cell was entered before, its session was begun anew since, and another cell of its line was entered in the
    /// new one first.
    ReopenedAfterRead,
}

impl Entry {
    /// Its name, in the words the files use.
    pub fn name(self) -> &'static str {
        match self {
            Entry::First => "first",
            Entry::Later => "later",
            Entry::Reopened => "reopened",
            Entry::Compacted => "compacted",
            Entry::ReopenedAfterRead => "reopened_after_read",
        }
    }
}

/// Which of what a return may hand in it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hand {
    /// Only what the cell must write every time.
    Must,
    /// Every block the cell may write.
    All,
    /// What the cell must write and the block of its outputs at this place among those it need not.
    Only(usize),
}

/// Which value a return that writes a choice gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    First,
    Last,
}

/// Which of what ran the round a return names as dealt with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dealt {
    Everything,
    Nothing,
    /// Every other one of what ran the round.
    Part,
}

/// What a return asks for, beside its blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asks {
    Nothing,
    /// A tool the cell may ask for, run on something.
    Tool,
    /// The same, run on nothing.
    ToolOnNothing,
    /// A question to the owner that the cell does not wait for the answer to.
    Question,
    /// A question to the owner that the cell waits for the answer to.
    Waits,
}

/// What becomes of the round as it is carried out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fate {
    Reaches,
    /// Its session is compacted as it ends.
    Compacts,
    /// Its session is lost to the agents' program after its first try, and a second try begins a new one.
    Lost,
    /// It fails before its program has its message.
    FailsBefore,
    /// It fails after its program had its message, as at the usage limit.
    FailsAfter,
}

/// How a round under way is carried out and returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Carry {
    pub hand: Hand,
    pub pick: Pick,
    pub dealt: Dealt,
    pub asks: Asks,
    pub fate: Fate,
    /// Whether a block handed in that was written before is handed in as changing nothing in meaning.
    pub unmatters: bool,
    /// Whether a block the cell may not write is handed in beside the rest, which sends the return back.
    pub wrong: bool,
}

impl Carry {
    /// The return of a cell that hands in what it must write, and deals with all that ran it.
    pub const PLAIN: Carry = Carry {
        hand: Hand::Must,
        pick: Pick::First,
        dealt: Dealt::Everything,
        asks: Asks::Nothing,
        fate: Fate::Reaches,
        unmatters: false,
        wrong: false,
    };
}

/// One thing that happens to a work, as a script holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// A person or the outside writes the block, a choice with one of its values.
    Write { block: String, value: Option<String> },
    /// The owner has the line begin a new session.
    Reopen { line: String },
    /// The owner says some words to the cell.
    Say { cell: String },
    /// The owner answers a question that stands with no answer, taking this option.
    Answer { option: u64 },
    /// The round under way is carried out.
    Carry(Carry),
}

/// A round that was carried out, as it was entered.
#[derive(Clone, Debug)]
pub struct Sight {
    pub cell: CellId,
    pub entry: Entry,
    pub situation: Situation,
    /// What the round was sent, where the script was run to keep it.
    pub entered: Option<Entered>,
}

/// A history that reaches lines, as a script and which of the rounds it carries out is the one that does.
#[derive(Clone, Debug)]
pub struct Witness {
    pub cell: CellId,
    pub entry: Entry,
    pub situation: Situation,
    pub script: Vec<Action>,
    /// Which of the rounds the script carries out, counting from none, stands in the situation.
    pub nth: usize,
}

/// The rounds a script carries out, or why it does not run.
pub type Ran = Result<Vec<Sight>, String>;

/// What a search found.
#[derive(Clone, Debug, Default)]
pub struct Found {
    /// The first history to reach each line.
    pub witnesses: Vec<Witness>,
    /// For each line, the witness that reaches it, by its place.
    pub lines: BTreeMap<String, usize>,
    /// How many rounds the search carried out.
    pub rounds: usize,
    /// How many states of works it kept.
    pub kept: usize,
    /// What went wrong in a history it tried: the engine refused what the search did, or something in the simulation
    /// failed. The history was dropped.
    pub faults: Vec<String>,
}

/// What bounds a search: how many rounds it carries out in all, how many rounds in a row without a new line end it,
/// how many states of works it keeps, and how many histories it makes from a state at each turn.
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub rounds: usize,
    pub plateau: usize,
    pub archive: usize,
    pub draws: usize,
}

/// The bounds of the command: how many rounds the search carries out at most, how many in a row without a new line end
/// it (a quarter of them), how many states of works it keeps, and how many histories it makes from a state at each turn.
/// What the engineer's flow needs of them and what they cost is what `measure/situations-search` times.
pub const BOUNDS: Bounds = Bounds {
    rounds: 6_000,
    plateau: 1_500,
    archive: 400,
    draws: 8,
};

/// The seed of the draws.
const SEED: u64 = 0x5EED_0F_5EA2C4;

/// How many things a draw does before a round is carried out, at most.
const BEFORE: usize = 2;

/// How many steps the work is let on by itself after what happened to it, at most: the engine's own calls, and a cell
/// at its limit.
const GO_ON: usize = 40;

/// What the search keeps of a work: a state, the script that makes it from a fresh work, and how many rounds the
/// script carries out.
#[derive(Clone)]
struct Node {
    sim: Sim,
    script: Vec<Action>,
    count: u64,
    seen: usize,
}

/// The work of a flow, the way the carrier takes works: main taken in as runs of cells that take it in start.
fn fresh(flow: &Compiled, words: &Agents, compared: &BTreeSet<BlockId>) -> Node {
    let mut sim = Sim::of(flow.clone(), words.clone());
    sim.takes_main = true;
    sim.compared = compared.clone();
    Node {
        sim,
        script: Vec::new(),
        count: 0,
        seen: 0,
    }
}

/// The blocks agents' cells are given or watch, and those any cell's condition or a named test of one names: what a
/// person or the outside may write to change what an agent's cell is sent or whether it runs.
fn used(flow: &Compiled) -> BTreeSet<BlockId> {
    let mut used = BTreeSet::new();
    for spec in flow.cells.iter().filter(|c| c.kind == Kind::Agent) {
        used.extend(spec.given.iter().map(|g| g.block.clone()));
        used.extend(spec.watches.iter().flat_map(|w| w.blocks.iter().cloned()));
    }
    for cell in flow.flow.cells() {
        for key in cell.applies.iter().chain(&cell.requires) {
            match key {
                Condition::Is { block, .. } => {
                    used.insert(block.clone());
                }
                Condition::Test(test) => {
                    used.extend(
                        flow.flow
                            .tests()
                            .get(test)
                            .into_iter()
                            .flat_map(|t| t.reads.iter().cloned()),
                    );
                }
            }
        }
    }
    used
}

/// What people and the outside may write: each block they write that matters to an agent's cell, a choice once for each
/// of its values.
fn writes(flow: &Compiled) -> Vec<Action> {
    let used = used(flow);
    let mut out = Vec::new();
    for (block, declared) in &flow.blocks {
        if !(declared.writers.people || engine::handle::outside_writes(declared)) || !used.contains(block) {
            continue;
        }
        match &declared.values {
            Some(values) => out.extend(values.iter().map(|v| Action::Write {
                block: block.as_str().to_string(),
                value: Some(v.clone()),
            })),
            None => out.push(Action::Write {
                block: block.as_str().to_string(),
                value: None,
            }),
        }
    }
    out
}

/// A random generator, seeded.
#[derive(Clone)]
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(0x2545_F491_4F6C_DD1D)
            | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

impl Node {
    fn take(&mut self, event: Event) -> Result<(), String> {
        self.sim
            .try_take(event)
            .map(|_| ())
            .map_err(|refused| format!("{refused:?}"))
    }

    /// A new content of the block: the latest one with a line changed or added, or a first one of many lines, so that
    /// what the carrier says changed between two versions is small beside the whole.
    fn content(&mut self, block: &str) -> String {
        self.count += 1;
        let latest = self.sim.versions(block).last().copied();
        let before = latest.map(|at| self.sim.content(block, at).to_string());
        let mut lines: Vec<String> = match before {
            Some(text) if !text.is_empty() => text.split('\n').map(str::to_string).collect(),
            _ => (1..=12).map(|n| format!("{block}, line {n}")).collect(),
        };
        let at = self.count as usize % lines.len();
        match self.count % 4 {
            0 => lines.insert(at, format!("{block}, added {}", self.count)),
            _ => lines[at] = format!("{block}, changed {}", self.count),
        }
        lines.join("\n")
    }

    /// Does what the action says, and then lets the work go on by itself as far as it does.
    fn apply(&mut self, action: &Action, full: bool, sights: &mut Vec<Sight>) -> Result<(), String> {
        self.script.push(action.clone());
        match action {
            Action::Write { block, value } => {
                let people = (self.sim.flow.blocks.get(&BlockId::from(block.as_str())))
                    .ok_or_else(|| format!("no block {block}"))?
                    .writers
                    .people;
                let by = if people { Outsider::Person } else { Outsider::Outside };
                let content = match value {
                    Some(value) => value.clone(),
                    None => self.content(block),
                };
                self.take(Event::Wrote {
                    block: block.as_str().into(),
                    by,
                    content,
                    matters: true,
                    value: value.clone(),
                })?;
            }
            Action::Reopen { line } => {
                self.take(Event::Anew {
                    line: line.clone(),
                    why: Why::Changed { context: None },
                })?;
            }
            Action::Say { cell } => {
                let to = To::Cell(CellId::from(cell.as_str()));
                spoken::receiver(&self.sim.flow, &self.sim.work, &to).map_err(|e| format!("{e:?}"))?;
                self.count += 1;
                let words = format!("Do it again, {}", self.count);
                self.take(Event::Said { to, words })?;
            }
            Action::Answer { option } => {
                if !self.answers(*option)? {
                    return Err("no question stands unanswered".to_string());
                }
            }
            Action::Carry(how) => self.carry(how, full, sights)?,
        }
        self.advance()
    }

    /// The owner answers the first question that stands with no answer. Says whether there was one.
    fn answers(&mut self, option: u64) -> Result<bool, String> {
        let records = &self.sim.work.records;
        let unanswered = (decide::questions::questions(records).into_iter())
            .filter(|(_, q)| q.unanswered())
            .find_map(|(name, q)| {
                let number = engine::asked::number_of(&self.sim.flow, records, &q.latest.cell, q.latest.at)?;
                Some((name.to_string(), number))
            });
        let Some((question, version)) = unanswered else {
            return Ok(false);
        };
        self.take(Event::Answer {
            question,
            version,
            option,
        })?;
        Ok(true)
    }

    /// Lets the work go on as far as the engine does by itself: the calls it makes are answered, and a cell at its limit
    /// is let go on.
    fn advance(&mut self) -> Result<(), String> {
        for _ in 0..GO_ON {
            if self.sim.round().is_some() {
                return Ok(());
            }
            if let Some(call) = self.sim.calls().first().copied() {
                self.take(Event::Done {
                    call,
                    result: "the rows".to_string(),
                })?;
                continue;
            }
            if !self.lets_go()? {
                return Ok(());
            }
        }
        Ok(())
    }

    /// The owner does what the work waits for from them and what no choice is theirs in: lets a cell at its limit go on,
    /// or answers a call of a run that reached the limit of its returns. Says whether the work waited for one.
    fn lets_go(&mut self) -> Result<bool, String> {
        let Decision::Wait { at, on } = decide(&self.sim.flow.flow, &self.sim.work.records) else {
            return Ok(false);
        };
        for waited in on {
            match waited {
                Waited::Limit { .. } | Waited::Calls { .. } => {
                    self.take(Event::LetGo { cell: at })?;
                    return Ok(true);
                }
                Waited::Call { call, on: Some(_) } => {
                    self.take(Event::Done {
                        call,
                        result: "Go on.".to_string(),
                    })?;
                    return Ok(true);
                }
                _ => {}
            }
        }
        Ok(false)
    }

    /// How the round enters its cell, by what the records say of the cell's earlier rounds and its line's sessions.
    fn entry(&self, round: &Given, begins: bool) -> Entry {
        let (kept, records) = (&self.sim.work, &self.sim.work.records);
        let line = self.sim.line(round);
        let this = work::sessions::start(kept, round.run, round.number);
        let mut counted: BTreeMap<RunId, usize> = BTreeMap::new();
        let mut before: Option<Seq> = None;
        for r in &records.rounds {
            let number = counted.entry(r.run).or_default();
            *number += 1;
            if r.run == round.run && *number == round.number {
                break;
            }
            if records.started.iter().any(|s| s.run == r.run && s.cell == round.cell) {
                before = Some(r.at);
            }
        }
        let Some(before) = before else {
            return Entry::First;
        };
        if work::sessions::number_at(kept, &line, before) != work::sessions::number_at(kept, &line, this) {
            return if begins {
                Entry::Reopened
            } else {
                Entry::ReopenedAfterRead
            };
        }
        let compacted = kept
            .sessions
            .iter()
            .any(|s| s.line == line && s.why == Why::Compacted && s.at > before && s.at < this);
        if compacted { Entry::Compacted } else { Entry::Later }
    }

    /// Carries the round under way out as the carrier does, entering it in its session as it stands, or in one that
    /// holds nothing where it begins its session or is lost, and making what it returns.
    fn carry(&mut self, how: &Carry, full: bool, sights: &mut Vec<Sight>) -> Result<(), String> {
        let round = self.sim.round().ok_or_else(|| "no round is under way".to_string())?;
        // A cell of the engine's own is sent nothing: it hands in what it writes.
        if self.sim.cell(&round).line.is_none() {
            let came = self.came(&round, how);
            self.sim.returns(&round, came, 0);
            return Ok(());
        }
        let line = self.sim.line(&round);
        let begins = self.sim.begins(&round);
        let entry = self.entry(&round, begins);
        let going_on = self.sim.enter(&round, false);
        let lost = !begins && how.fate == Fate::Lost;
        let anew = (begins || lost).then(|| self.sim.enter(&round, true));
        // The round is sent what its first try fills: the session as it stands, or one that holds nothing where it
        // begins its session; and, where its session is lost, a second try in one that holds nothing.
        let first = match (&anew, begins) {
            (Some(anew), true) => anew,
            _ => &going_on,
        };
        let mut sight = |entered: &Entered| {
            sights.push(Sight {
                cell: round.cell.clone(),
                entry,
                situation: entered.situation.clone(),
                entered: full.then(|| entered.clone()),
            });
            self.seen += 1;
        };
        sight(first);
        if let (true, Some(anew)) = (lost, &anew) {
            sight(anew);
        }
        match (&anew, lost) {
            (Some(anew), true) => {
                self.sim.tries(&round, 1, going_on.fills.clone());
                self.sim.anew(&line, Why::Lost);
                self.sim.tries(&round, 2, anew.fills.clone());
            }
            _ => self.sim.tries(&round, 1, first.fills.clone()),
        }
        match how.fate {
            Fate::FailsBefore => self.sim.fails(&round),
            Fate::FailsAfter => self.sim.fails_as(&round, decide::run::Failure::UsageLimit, true),
            fate => {
                let came = self.came(&round, how);
                self.sim.returns(&round, came, u32::from(fate == Fate::Compacts));
            }
        }
        Ok(())
    }

    /// What the round returns, as the carry says.
    fn came(&mut self, round: &Given, how: &Carry) -> Came {
        let cell = self.sim.cell(round).clone();
        let tool = cell.tools.first().cloned();
        match (how.asks, tool) {
            (Asks::Tool, Some(name)) => {
                let input = format!("{{\"n\": {}}}", self.count);
                return Came::asks(vec![Ask::Tool { name, input }]);
            }
            (Asks::ToolOnNothing, Some(name)) => {
                return Came::asks(vec![Ask::Tool {
                    name,
                    input: String::new(),
                }]);
            }
            _ => {}
        }
        let (mut chosen, optional) = outputs_of(&self.sim.flow, &cell);
        match how.hand {
            Hand::Must => {}
            Hand::All => chosen.extend(optional.iter().cloned()),
            Hand::Only(k) => chosen.extend(optional.get(k).cloned()),
        }
        let mut writes = Vec::new();
        for block in chosen {
            let values = self.sim.flow.blocks[&block].values.clone();
            let value = values.map(|v| match how.pick {
                Pick::First => v[0].clone(),
                Pick::Last => v[v.len() - 1].clone(),
            });
            let written = !self.sim.versions(block.as_str()).is_empty();
            let content = match &value {
                Some(value) => value.clone(),
                None => self.content(block.as_str()),
            };
            let matters = value.is_some() || !(how.unmatters && written);
            writes.push(Handed {
                block,
                content,
                matters,
                value,
            });
        }
        // A choice that says what a return holds nothing else but, with the value it is given, is handed in with just
        // that.
        for kept in writes.clone() {
            let Some(value) = &kept.value else {
                continue;
            };
            if let Some(only) = self.sim.flow.blocks[&kept.block].with_nothing_but.get(value).cloned() {
                writes.retain(|w| w.block == kept.block || only.contains(&w.block));
            }
        }
        // A block the cell may not write is handed in beside the rest, which sends the return back.
        if how.wrong {
            let place = self.sim.flow.flow.place(&cell.id).expect("the cell's place");
            let outputs = &self.sim.flow.flow.cells()[place].outputs;
            let others = (self.sim.flow.blocks.keys()).find(|b| !outputs.contains(b)).cloned();
            if let Some(block) = others {
                let value = self.sim.flow.blocks[&block].values.as_ref().map(|v| v[0].clone());
                let content = match &value {
                    Some(value) => value.clone(),
                    None => self.content(block.as_str()),
                };
                writes.push(Handed {
                    block,
                    content,
                    matters: true,
                    value,
                });
            }
        }
        let mut came = Came::hands_in(writes);
        let ran_by = (decide::told(&self.sim.flow.flow, &self.sim.work.records, round.run))
            .map(|told| told.ran_by)
            .unwrap_or_default();
        match how.dealt {
            Dealt::Everything => {}
            Dealt::Nothing => came.dealt_with = Some(Vec::new()),
            Dealt::Part => {
                let part = (ran_by.into_iter().enumerate()).filter(|(i, _)| i % 2 == 0);
                came.dealt_with = Some(part.map(|(_, b)| b).collect());
            }
        }
        let takes_answers = (cell.watches.iter()).any(|t| t.of_source(&self.sim.flow, flow::ANSWER).is_some());
        if takes_answers && matches!(how.asks, Asks::Question | Asks::Waits) {
            came.said.questions = self.question(&cell, how.asks == Asks::Waits);
        }
        came
    }

    /// A question of the cell's own, which it waits for the answer to or not.
    fn question(&self, cell: &CellSpec, waits: bool) -> Vec<Question> {
        let name = format!("{}_{}", cell.id, self.sim.work.records.questions.len());
        let way = |does: &str, follows: &str| Way {
            does: does.to_string(),
            follows: follows.to_string(),
        };
        vec![Question::Asked {
            ask: format!("Which way for {name}?"),
            name,
            waits,
            tree: None,
            recommends: Some(1),
            options: vec![
                way("the short way", "less to read"),
                way("the long way", "more to read"),
            ],
            about: None,
            reasons: Default::default(),
        }]
    }

    /// What the next round is sent and what the work holds of its choices, as a key: states with the same key are
    /// alike for the search.
    fn key(&self) -> (String, u64) {
        let mut hasher = DefaultHasher::new();
        let cell = match self.sim.round() {
            Some(round) => {
                let spec = self.sim.cell(&round);
                if spec.line.is_some() {
                    let begins = self.sim.begins(&round);
                    let entered = self.sim.enter(&round, begins);
                    format!("{:?}", entered.situation).hash(&mut hasher);
                    // The cell of the line that ran last, which decides what the session holds before this entry.
                    let line = spec.line.clone();
                    let last = (self.sim.work.records.started.iter().rev())
                        .filter(|s| s.run != round.run)
                        .find(|s| {
                            (self.sim.flow.cells.iter()).any(|c| c.id == s.cell && c.line.is_some() && c.line == line)
                        })
                        .map(|s| s.cell.clone());
                    format!("{last:?}").hash(&mut hasher);
                }
                round.cell.as_str().to_string()
            }
            None => {
                let at = format!("{:?}", decide(&self.sim.flow.flow, &self.sim.work.records));
                at.chars().take(160).collect::<String>().hash(&mut hasher);
                "rest".to_string()
            }
        };
        let versions = decide::versions(&self.sim.work.records);
        for (block, declared) in &self.sim.flow.blocks {
            if declared.values.is_none() {
                continue;
            }
            let latest = versions.get(block).and_then(|v| v.last()).map(|v| v.at);
            let value = latest.map(|at| self.sim.content(block.as_str(), at));
            (block.as_str(), value).hash(&mut hasher);
        }
        (cell, hasher.finish())
    }
}

/// The blocks among a cell's outputs that it must write every time and those it may: a question is asked in the
/// return's own field, and what answers it is the owner's, so the blocks of that source are neither.
fn outputs_of(flow: &Compiled, cell: &CellSpec) -> (Vec<BlockId>, Vec<BlockId>) {
    let place = flow.flow.place(&cell.id).expect("the cell's place");
    let outputs = &flow.flow.cells()[place].outputs;
    let answers = |b: &&BlockId| flow.blocks[*b].writers.outside.as_deref() == Some(flow::ANSWER);
    let (must, may): (Vec<&BlockId>, Vec<&BlockId>) = (outputs.iter())
        .filter(|b| !answers(b))
        .partition(|b| cell.must_write.contains(b));
    (must.into_iter().cloned().collect(), may.into_iter().cloned().collect())
}

/// What the search does at a state before a round is carried out: each thing people, the outside or the owner may do.
fn befores(node: &Node, writes: &[Action]) -> Vec<Action> {
    let mut all: Vec<Action> = writes.to_vec();
    all.extend(
        node.sim
            .flow
            .lines
            .keys()
            .map(|line| Action::Reopen { line: line.clone() }),
    );
    for cell in &node.sim.flow.cells {
        if spoken::words_of(&node.sim.flow, &cell.id).is_some() {
            let to = To::Cell(cell.id.clone());
            if spoken::receiver(&node.sim.flow, &node.sim.work, &to).is_ok() {
                all.push(Action::Say {
                    cell: cell.id.as_str().to_string(),
                });
            }
        }
    }
    let unanswered = (decide::questions::questions(&node.sim.work.records).into_iter()).any(|(_, q)| q.unanswered());
    if unanswered {
        all.extend([1, 2].map(|option| Action::Answer { option }));
    }
    all
}

/// Each way a round of the cell under way returns, one thing different in each from the plain return, the plain one
/// first and then those that reach most of what a state leads to.
fn carries(cell: &CellSpec, optional: usize) -> Vec<Carry> {
    let plain = Carry::PLAIN;
    let all_of = Carry {
        hand: Hand::All,
        ..plain
    };
    let mut all = vec![
        plain,
        all_of,
        Carry {
            pick: Pick::Last,
            ..all_of
        },
        Carry {
            pick: Pick::Last,
            ..plain
        },
        Carry {
            dealt: Dealt::Nothing,
            ..all_of
        },
        Carry {
            dealt: Dealt::Part,
            ..all_of
        },
    ];
    if cell.line.is_some() {
        all.extend([
            Carry {
                asks: Asks::Waits,
                ..all_of
            },
            Carry {
                asks: Asks::Waits,
                dealt: Dealt::Nothing,
                ..plain
            },
            Carry {
                fate: Fate::Compacts,
                ..all_of
            },
            Carry {
                fate: Fate::Lost,
                ..all_of
            },
        ]);
    }
    all.extend((0..optional).map(|k| Carry {
        hand: Hand::Only(k),
        ..plain
    }));
    all.extend([
        Carry {
            unmatters: true,
            ..all_of
        },
        Carry {
            dealt: Dealt::Nothing,
            ..plain
        },
        Carry {
            dealt: Dealt::Part,
            ..plain
        },
        Carry { wrong: true, ..plain },
    ]);
    if cell.line.is_some() {
        all.extend([
            Carry {
                asks: Asks::Tool,
                ..plain
            },
            Carry {
                asks: Asks::ToolOnNothing,
                ..plain
            },
            Carry {
                asks: Asks::Question,
                ..all_of
            },
            Carry {
                fate: Fate::Compacts,
                ..plain
            },
            Carry {
                fate: Fate::FailsBefore,
                ..plain
            },
            Carry {
                fate: Fate::FailsAfter,
                ..plain
            },
        ]);
    }
    all
}

/// A return drawn at random.
fn drawn(rng: &mut Rng, optional: usize, line: bool) -> Carry {
    let hand = match rng.below(10) {
        0..=1 => Hand::Must,
        2..=4 => Hand::All,
        _ => Hand::Only(rng.below(optional.max(1))),
    };
    let dealt = match rng.below(10) {
        0..=5 => Dealt::Everything,
        6..=7 => Dealt::Nothing,
        _ => Dealt::Part,
    };
    let asks = match (line, rng.below(20)) {
        (false, _) => Asks::Nothing,
        (_, 0..=11) => Asks::Nothing,
        (_, 12) => Asks::Tool,
        (_, 13) => Asks::ToolOnNothing,
        (_, 14..=16) => Asks::Question,
        _ => Asks::Waits,
    };
    let fate = match (line, rng.below(20)) {
        (false, _) => Fate::Reaches,
        (_, 0..=15) => Fate::Reaches,
        (_, 16) => Fate::Compacts,
        (_, 17) => Fate::Lost,
        (_, 18) => Fate::FailsBefore,
        _ => Fate::FailsAfter,
    };
    Carry {
        hand,
        pick: if rng.chance(50) { Pick::First } else { Pick::Last },
        dealt,
        asks,
        fate,
        unmatters: rng.chance(20),
        wrong: rng.chance(4),
    }
}

/// A state the search keeps, with how many times it has gone on from it.
struct Kept {
    node: Node,
    key: (String, u64),
    turns: usize,
}

/// The search's state: what it found, and the states it goes on from.
struct Search<'a> {
    flow: &'a Compiled,
    focus: Option<&'a CellId>,
    bounds: Bounds,
    writes: Vec<Action>,
    found: Found,
    archive: Vec<Kept>,
    keys: BTreeSet<(String, u64)>,
    since: usize,
    /// How many histories it tried, which end it too where they carry out no round of an agent's cell.
    tried: usize,
}

impl Search<'_> {
    /// Tries the actions on a copy of the node: where all of them were done, the copy and what it carried out.
    fn child(&mut self, from: &Node, actions: &[Action]) -> Option<(Node, Vec<Sight>)> {
        let mut node = from.clone();
        let mut sights = Vec::new();
        let done = catch_unwind(AssertUnwindSafe(|| {
            for action in actions {
                node.apply(action, false, &mut sights)?;
            }
            Ok::<(), String>(())
        }));
        match done {
            Ok(Ok(())) => Some((node, sights)),
            Ok(Err(_)) => None,
            Err(panic) => {
                let said = (panic.downcast_ref::<String>().cloned())
                    .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "a panic".to_string());
                if self.found.faults.len() < 20 {
                    self.found.faults.push(format!("{actions:?}: {said}"));
                }
                None
            }
        }
    }

    /// Notes the lines the rounds a child carried out stood in, with the script of the child where one is new.
    fn note(&mut self, node: &Node, sights: &[Sight]) {
        let first = node.seen - sights.len();
        for (n, sight) in sights.iter().enumerate() {
            let Some(spec) = self.flow.cells.iter().find(|c| c.id == sight.cell) else {
                continue;
            };
            let focused = |line: &str| self.focus.is_none_or(|id| line.starts_with(&format!("{id}: ")));
            let lines: Vec<String> = stood(spec, &sight.situation)
                .into_iter()
                .filter(|l| focused(l))
                .collect();
            let new: Vec<&String> = lines.iter().filter(|l| !self.found.lines.contains_key(*l)).collect();
            self.found.rounds += 1;
            self.since += 1;
            if new.is_empty() {
                continue;
            }
            let at = self.found.witnesses.len();
            for line in new {
                self.found.lines.insert(line.clone(), at);
            }
            self.found.witnesses.push(Witness {
                cell: sight.cell.clone(),
                entry: sight.entry,
                situation: sight.situation.clone(),
                script: node.script.clone(),
                nth: first + n,
            });
            self.since = 0;
        }
    }

    /// Keeps the node where no state alike was kept.
    fn keep(&mut self, node: Node) {
        let key = node.key();
        if !self.keys.insert(key.clone()) {
            return;
        }
        let kept = Kept { node, key, turns: 0 };
        match self.archive.len() < self.bounds.archive {
            true => self.archive.push(kept),
            false => {
                let at = (key_number(&kept.key) % self.archive.len() as u64) as usize;
                self.keys.remove(&self.archive[at].key);
                self.archive[at] = kept;
            }
        }
        self.found.kept += 1;
    }

    /// Does the actions on a copy of the node, notes what its rounds stood in, and keeps the copy.
    fn try_out(&mut self, from: &Node, actions: &[Action]) {
        self.tried += 1;
        if let Some((node, sights)) = self.child(from, actions) {
            self.note(&node, &sights);
            self.keep(node);
        }
    }

    fn done(&self) -> bool {
        self.found.rounds >= self.bounds.rounds
            || self.since >= self.bounds.plateau
            || self.tried >= self.bounds.rounds * 4
    }

    /// The kept state gone on from least often, the first of them.
    fn next(&self) -> Option<usize> {
        (0..self.archive.len()).min_by_key(|&i| (self.archive[i].turns, i))
    }

    /// Goes on from a kept state, a few histories each time it is gone on from: first each way a round returns, then each
    /// thing that may happen before a round and the plain return after it, and when those are done, draws of both.
    fn go_on(&mut self, at: usize) {
        let turn = self.archive[at].turns;
        self.archive[at].turns += 1;
        let node = self.archive[at].node.clone();
        let round = node.sim.round();
        let mut items: Vec<Vec<Action>> = Vec::new();
        let mut optional = 0;
        let mut line = true;
        if let Some(round) = &round {
            let cell = node.sim.cell(round).clone();
            optional = outputs_of(&node.sim.flow, &cell).1.len();
            line = cell.line.is_some();
            items.extend(carries(&cell, optional).into_iter().map(|how| vec![Action::Carry(how)]));
        }
        let before = befores(&node, &self.writes);
        items.extend(
            before
                .iter()
                .map(|action| vec![action.clone(), Action::Carry(Carry::PLAIN)]),
        );
        let mut rng = Rng::new(SEED ^ key_number(&self.archive[at].key) ^ turn as u64);
        for place in turn * self.bounds.draws..(turn + 1) * self.bounds.draws {
            if self.done() {
                return;
            }
            let actions = match items.get(place) {
                Some(actions) => actions.clone(),
                None => {
                    let mut actions = Vec::new();
                    for _ in 0..rng.below(BEFORE + 1) {
                        if !before.is_empty() {
                            actions.push(before[rng.below(before.len())].clone());
                        }
                    }
                    actions.push(Action::Carry(drawn(&mut rng, optional, line)));
                    actions
                }
            };
            self.try_out(&node, &actions);
        }
    }
}

/// A key's number, for a draw and a place in the archive.
fn key_number(key: &(String, u64)) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

/// The works the search begins from: the least number of writes, in each order only once, that give a round to carry
/// out.
fn roots(from: &Node, writes: &[Action], faults: &mut Vec<String>) -> Vec<Node> {
    let mut level = vec![from.clone()];
    let mut seen: BTreeSet<Vec<String>> = BTreeSet::new();
    for _ in 0..3 {
        let mut next = Vec::new();
        let mut roots = Vec::new();
        for node in &level {
            for write in writes {
                let mut child = node.clone();
                let mut none = Vec::new();
                match catch_unwind(AssertUnwindSafe(|| child.apply(write, false, &mut none))) {
                    Ok(Ok(())) => {}
                    Ok(Err(_)) => continue,
                    Err(_) => {
                        faults.push(format!("{write:?} could not begin a work"));
                        continue;
                    }
                }
                let mut written: Vec<String> = child.script.iter().map(|a| format!("{a:?}")).collect();
                written.sort();
                if !seen.insert(written) {
                    continue;
                }
                if child.sim.round().is_some() {
                    roots.push(child);
                } else {
                    next.push(child);
                }
            }
        }
        if !roots.is_empty() {
            return roots;
        }
        level = next;
    }
    Vec::new()
}

/// Searches for histories of works of the flow that reach each line a round stands in, as the module says: all the
/// flow's cells, or those lines of the one cell named alone.
pub fn search(
    flow: &Compiled,
    words: &Agents,
    compared: &BTreeSet<BlockId>,
    focus: Option<&CellId>,
    bounds: &Bounds,
) -> Found {
    let writes = writes(flow);
    let mut faults = Vec::new();
    let roots = roots(&fresh(flow, words, compared), &writes, &mut faults);
    let mut search = Search {
        flow,
        focus,
        bounds: *bounds,
        writes,
        found: Found::default(),
        archive: Vec::new(),
        keys: BTreeSet::new(),
        since: 0,
        tried: 0,
    };
    search.found.faults = faults;
    for root in roots {
        search.keep(root);
    }
    while !search.done() {
        let Some(at) = search.next() else {
            break;
        };
        search.go_on(at);
    }
    search.found
}

/// Runs a script on a fresh work of the flow and gives the rounds it carries out, each as it was entered.
pub fn replay(
    flow: &Compiled,
    words: &Agents,
    compared: &BTreeSet<BlockId>,
    script: &[Action],
) -> Result<Vec<Sight>, String> {
    let mut node = fresh(flow, words, compared);
    let mut sights = Vec::new();
    for action in script {
        node.apply(action, true, &mut sights)?;
    }
    Ok(sights)
}

/// Runs each of the scripts on a fresh work of the flow, as [`replay`] does, and gives the rounds each carries out: the
/// scripts that begin alike share the work they begin with.
pub fn replay_all(flow: &Compiled, words: &Agents, compared: &BTreeSet<BlockId>, scripts: &[&[Action]]) -> Vec<Ran> {
    let mut out: Vec<Option<Ran>> = (0..scripts.len()).map(|_| None).collect();
    let all: Vec<usize> = (0..scripts.len()).collect();
    walk(fresh(flow, words, compared), Vec::new(), 0, &all, scripts, &mut out);
    (out.into_iter())
        .map(|ran| ran.unwrap_or_else(|| Err("the script was not run".to_string())))
        .collect()
}

/// The scripts of the group all begin with the first `depth` actions the node has done, and the rounds they carried
/// out are `sights`: those that end here are done, and the others go on in groups that do the same next.
fn walk(node: Node, sights: Vec<Sight>, depth: usize, group: &[usize], scripts: &[&[Action]], out: &mut [Option<Ran>]) {
    let (done, mut left): (Vec<usize>, Vec<usize>) = group.iter().copied().partition(|&i| scripts[i].len() == depth);
    for i in done {
        out[i] = Some(Ok(sights.clone()));
    }
    while let Some(&first) = left.first() {
        let action = &scripts[first][depth];
        let (same, other): (Vec<usize>, Vec<usize>) = left.iter().copied().partition(|&i| scripts[i][depth] == *action);
        left = other;
        let mut child = node.clone();
        let mut seen = sights.clone();
        match child.apply(action, true, &mut seen) {
            Ok(()) => walk(child, seen, depth + 1, &same, scripts, out),
            Err(why) => {
                for i in same {
                    out[i] = Some(Err(why.clone()));
                }
            }
        }
    }
}
