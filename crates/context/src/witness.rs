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

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

use decide::{BlockId, CellId, Condition, Decision, Outsider, RunId, Seq, Waited, decide};
use engine::spoken::{self, To};
use engine::{Ask, Came, Event, Handed, Question, Way, Why};
use flow::agents::Agents;
use flow::{CellSpec, Compiled, Kind};

use crate::manager::Entered;
use crate::message::Situation;
use crate::sim::{Given, Sim};
use crate::situations::stood;

thread_local! {
    /// What the search's time on this thread went to, by what it did.
    static SPENT: RefCell<BTreeMap<&'static str, Duration>> = const { RefCell::new(BTreeMap::new()) };
}

/// Does this, and adds the time it took to what the search's time went to under this name.
fn timed<T>(what: &'static str, then: impl FnOnce() -> T) -> T {
    let begun = Instant::now();
    let out = then();
    SPENT.with(|spent| *spent.borrow_mut().entry(what).or_default() += begun.elapsed());
    out
}

/// What the time of the searches on this thread went to, by what they did, the most first.
pub fn spent() -> Vec<(&'static str, Duration)> {
    let mut all: Vec<_> = SPENT.with(|spent| spent.borrow().iter().map(|(what, took)| (*what, *took)).collect());
    all.sort_by_key(|(_, took)| std::cmp::Reverse(*took));
    all
}

/// What a state that no round is under way in is called, in its key.
const AT_REST: &str = "at rest";

/// How a round enters its cell's session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    /// What the cell must write and, of the blocks of its outputs it need not, each at the place of a set bit.
    Mask(u32),
    /// Of the blocks of its outputs, those it must write and then those it need not, each at the place of a set bit:
    /// what a cell must write may be left out, which the engine takes beside a question the cell waits for the answer
    /// to and sends back otherwise.
    Part(u32),
}

/// Which of what ran the round a return names as dealt with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dealt {
    Everything,
    Nothing,
    /// Each of what ran the round at the place of a set bit.
    Mask(u32),
}

/// Whether the bit of this place is set in the mask; a place past the mask's width is not.
fn set(mask: u32, place: usize) -> bool {
    u32::try_from(place)
        .ok()
        .and_then(|p| mask.checked_shr(p))
        .is_some_and(|m| m & 1 == 1)
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
    /// Which value a block of the return that is a choice is given: its last where the bit of its place among the blocks
    /// handed in is set, its first where not.
    pub picks: u32,
    pub dealt: Dealt,
    pub asks: Asks,
    pub fate: Fate,
    /// Whether a block handed in that was written before is handed in as changing nothing in meaning: each at the place
    /// of a set bit among the blocks handed in.
    pub unmatters: u32,
    /// Whether a block the cell may not write is handed in beside the rest, which sends the return back.
    pub wrong: bool,
}

impl Carry {
    /// The return of a cell that hands in what it must write, and deals with all that ran it.
    pub const PLAIN: Carry = Carry {
        hand: Hand::Must,
        picks: 0,
        dealt: Dealt::Everything,
        asks: Asks::Nothing,
        fate: Fate::Reaches,
        unmatters: 0,
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
    /// How many rounds it carried out that entered their cell each way.
    pub entries: BTreeMap<Entry, usize>,
    /// How many rounds it carried out of each cell.
    pub cells: BTreeMap<String, usize>,
    /// Why histories it tried were dropped, by the words of the engine's or the search's refusal, and how many; and how
    /// many it made that reached a state already kept.
    pub dropped: BTreeMap<String, usize>,
    pub repeated: usize,
    /// For each cell a state was kept pending at: how many states it kept there, how many times it chose one to go on from,
    /// and how many of the kept states it ended with.
    pub pending: BTreeMap<String, (usize, usize, usize)>,
    /// How many lines it had found after each thousand rounds, by the rounds it had carried out.
    pub curve: Vec<(usize, usize)>,
    /// Why it ended: the rounds were spent, no new line came for the stated rounds, or there was nothing left to try.
    pub ended: &'static str,
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
    rounds: 16_000,
    plateau: 4_000,
    archive: 1_000,
    draws: 8,
};

/// The seed of the draws.
const SEED: u64 = 0x5EED_0F5E_A2C4;

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
        timed("engine", || self.sim.try_take(event))
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
        // Where no round is under way, there is nothing to carry out.
        let Some(round) = self.sim.round() else {
            return Ok(());
        };
        // A cell of the engine's own is sent nothing: it hands in what it writes.
        if self.sim.cell(&round).line.is_none() {
            let came = self.came(&round, how);
            self.sim.returns(&round, came, 0);
            return Ok(());
        }
        let line = self.sim.line(&round);
        let begins = self.sim.begins(&round);
        let entry = timed("entry", || self.entry(&round, begins));
        let going_on = (!begins).then(|| timed("enter", || self.sim.enter(&round, false)));
        let lost = !begins && how.fate == Fate::Lost;
        let anew = (begins || lost).then(|| timed("enter", || self.sim.enter(&round, true)));
        // The round is sent what its first try fills: the session as it stands, or one that holds nothing where it
        // begins its session; and, where its session is lost, a second try in one that holds nothing.
        let first = match (&anew, &going_on) {
            (Some(anew), None) => anew,
            (_, Some(going_on)) => going_on,
            (None, None) => return Err("a round that begins its session is entered in none".to_string()),
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
                self.sim.tries(&round, 1, first.fills.clone());
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
            Hand::Mask(mask) => {
                let more = optional.iter().enumerate().filter(|(place, _)| set(mask, *place));
                chosen.extend(more.map(|(_, block)| block.clone()));
            }
            Hand::Part(mask) => {
                let all = chosen.iter().chain(optional.iter()).enumerate();
                chosen = all
                    .filter(|(place, _)| set(mask, *place))
                    .map(|(_, b)| b.clone())
                    .collect();
            }
        }
        let mut writes = Vec::new();
        for (place, block) in chosen.into_iter().enumerate() {
            let values = self.sim.flow.blocks[&block].values.clone();
            let value = values.map(|v| match set(how.picks, place) {
                false => v[0].clone(),
                true => v[v.len() - 1].clone(),
            });
            let written = !self.sim.versions(block.as_str()).is_empty();
            let content = match &value {
                Some(value) => value.clone(),
                None => self.content(block.as_str()),
            };
            let matters = value.is_some() || !(set(how.unmatters, place) && written);
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
            Dealt::Mask(mask) => {
                let part = (ran_by.into_iter().enumerate()).filter(|(place, _)| set(mask, *place));
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
        timed("key", || self.key_of())
    }

    fn key_of(&self) -> (String, u64) {
        let mut hasher = DefaultHasher::new();
        let cell = match self.sim.round() {
            Some(round) => {
                let spec = self.sim.cell(&round);
                if spec.line.is_some() {
                    let begins = self.sim.begins(&round);
                    let entered = self.sim.situation(&round, begins);
                    format!("{:?}", entered.situation).hash(&mut hasher);
                    // How it enters its session, which what comes after it depends on.
                    self.entry(&round, begins).hash(&mut hasher);
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
                AT_REST.to_string()
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

/// Everything people, the outside or the owner may do before a round, whether or not each may at a state: each write,
/// each line's session begun anew, a word to each cell that has words of its own, and an answer to a question.
fn things_before(flow: &Compiled, writes: &[Action]) -> Vec<Action> {
    let mut all: Vec<Action> = writes.to_vec();
    all.extend(flow.lines.keys().map(|line| Action::Reopen { line: line.clone() }));
    all.extend(
        (flow.cells.iter())
            .filter(|cell| spoken::words_of(flow, &cell.id).is_some())
            .map(|cell| Action::Say {
                cell: cell.id.as_str().to_string(),
            }),
    );
    all.extend([1, 2].map(|option| Action::Answer { option }));
    all
}

/// Whether the thing may be done at the state: a word goes to a cell that takes words, an answer to a question that
/// stands unanswered; the rest, always.
fn may_do(node: &Node, action: &Action) -> bool {
    match action {
        Action::Say { cell } => {
            let to = To::Cell(CellId::from(cell.as_str()));
            spoken::receiver(&node.sim.flow, &node.sim.work, &to).is_ok()
        }
        Action::Answer { .. } => {
            (decide::questions::questions(&node.sim.work.records).into_iter()).any(|(_, q)| q.unanswered())
        }
        _ => true,
    }
}

/// One of the things before a round, drawn from those the state allows, in a few draws; none where they found none.
fn draw_before(node: &Node, all: &[Action], rng: &mut Rng) -> Option<Action> {
    if all.is_empty() {
        return None;
    }
    (0..4)
        .map(|_| &all[rng.below(all.len())])
        .find(|action| may_do(node, action))
        .cloned()
}

/// The first of the things before a round the state allows, from this place among them on.
fn systematic_before(node: &Node, all: &[Action], from: usize) -> Option<Action> {
    (0..all.len())
        .map(|k| &all[(from + k) % all.len()])
        .find(|action| may_do(node, action))
        .cloned()
}

/// Each way a round of the cell under way returns, one thing different in each from the plain return, the plain one
/// first and then those that reach most of what a state leads to.
fn carries(cell: &CellSpec, must: usize, optional: usize) -> Vec<Carry> {
    let plain = Carry::PLAIN;
    let all_of = Carry {
        hand: Hand::All,
        ..plain
    };
    let every_other = Dealt::Mask(0b0101_0101);
    let mut all = vec![
        plain,
        all_of,
        Carry {
            picks: u32::MAX,
            ..all_of
        },
        Carry {
            picks: u32::MAX,
            ..plain
        },
        Carry {
            dealt: Dealt::Nothing,
            ..all_of
        },
        Carry {
            dealt: every_other,
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
    all.extend((0..optional.min(32)).map(|k| Carry {
        hand: Hand::Mask(1 << k),
        ..plain
    }));
    // What the cell must write left out beside a question it waits for the answer to: each block alone, all but each of
    // those it must write, and what it need not write alone.
    if cell.line.is_some() && must > 0 {
        let outputs = (must + optional).min(32);
        let every = u32::try_from((1u64 << outputs) - 1).unwrap_or(u32::MAX);
        let musts = u32::try_from((1u64 << must.min(32)) - 1).unwrap_or(u32::MAX);
        let parts = (0..outputs)
            .map(|k| 1u32 << k)
            .chain((0..must.min(32)).map(|k| every & !(1u32 << k)))
            .chain([every & !musts]);
        all.extend(parts.map(|mask| Carry {
            hand: Hand::Part(mask),
            asks: Asks::Waits,
            ..plain
        }));
    }
    // Every block handed in that was written before as changing nothing in meaning, and each of the first few alone.
    all.extend(
        [u32::MAX, 0b1, 0b10, 0b100, 0b1000, 0b10000]
            .into_iter()
            .map(|unmatters| Carry { unmatters, ..all_of }),
    );
    all.extend([
        Carry {
            dealt: Dealt::Nothing,
            ..plain
        },
        Carry {
            dealt: every_other,
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
fn drawn(rng: &mut Rng, line: bool) -> Carry {
    let hand = match rng.below(12) {
        0..=1 => Hand::Must,
        2..=3 => Hand::All,
        4 if line => Hand::Part(rng.next() as u32),
        _ => Hand::Mask(rng.next() as u32),
    };
    let dealt = match rng.below(10) {
        0..=5 => Dealt::Everything,
        6..=7 => Dealt::Nothing,
        _ => Dealt::Mask(rng.next() as u32),
    };
    let asks = match (line, rng.below(20)) {
        (true, _) if matches!(hand, Hand::Part(_)) && rng.chance(80) => Asks::Waits,
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
        picks: rng.next() as u32,
        dealt,
        asks,
        fate,
        unmatters: if rng.chance(30) { rng.next() as u32 } else { 0 },
        wrong: rng.chance(4),
    }
}

/// A state the search keeps, with how many times it has gone on from it and the order the states were kept in.
struct Kept {
    node: Node,
    key: (String, u64),
    turns: usize,
    born: usize,
}

/// The search's state: what it found, and the states it goes on from.
struct Search<'a> {
    flow: &'a Compiled,
    focus: Option<&'a CellId>,
    bounds: Bounds,
    /// Everything people, the outside and the owner may do before a round, which the state of a work allows or not.
    statics: Vec<Action>,
    found: Found,
    archive: Vec<Kept>,
    keys: BTreeSet<(String, u64)>,
    since: usize,
    /// How many histories it tried, which end it too where they carry out no round of an agent's cell.
    tried: usize,
    /// How many states it kept in all, and how many times it chose one to go on from.
    born: usize,
    picks: usize,
    /// What `Found::pending` says, as the search goes: states kept and states chosen, by the cell they are pending at.
    kept_at: BTreeMap<String, usize>,
    chosen_at: BTreeMap<String, usize>,
}

impl Search<'_> {
    /// Tries the actions on a copy of the node: where all of them were done, the copy and what it carried out.
    fn child(&mut self, from: &Node, actions: &[Action]) -> Option<(Node, Vec<Sight>)> {
        let mut node = timed("clone", || from.clone());
        let mut sights = Vec::new();
        let done = catch_unwind(AssertUnwindSafe(|| {
            for action in actions {
                node.apply(action, false, &mut sights)?;
            }
            Ok::<(), String>(())
        }));
        match done {
            Ok(Ok(())) => Some((node, sights)),
            Ok(Err(why)) => {
                *self.found.dropped.entry(why.chars().take(70).collect()).or_default() += 1;
                None
            }
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
            *self.found.entries.entry(sight.entry).or_default() += 1;
            *self.found.cells.entry(sight.cell.as_str().to_string()).or_default() += 1;
            if self.found.rounds.is_multiple_of(1000) {
                self.found.curve.push((self.found.rounds, self.found.lines.len()));
            }
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

    /// Keeps the node where no state alike was kept. Where the archive is full, a state of the highest rank, one of them
    /// by its key, makes room: what ranks least is what the search goes on from next.
    fn keep(&mut self, node: &Node) {
        let key = node.key();
        if !self.keys.insert(key.clone()) {
            self.found.repeated += 1;
            return;
        }
        self.born += 1;
        *self.kept_at.entry(key.0.clone()).or_default() += 1;
        let kept = Kept {
            node: timed("clone", || node.clone()),
            key,
            turns: 0,
            born: self.born,
        };
        if self.archive.len() < self.bounds.archive {
            self.archive.push(kept);
        } else {
            let ranks: Vec<usize> = self.archive.iter().map(|k| self.rank(k)).collect();
            let most = ranks.iter().copied().max().unwrap_or(0);
            let worn: Vec<usize> = (0..self.archive.len()).filter(|&i| ranks[i] == most).collect();
            let at = worn[(key_number(&kept.key) % worn.len() as u64) as usize];
            self.keys.remove(&self.archive[at].key);
            self.archive[at] = kept;
        }
        self.found.kept += 1;
    }

    /// Does the actions on a copy of the node, notes what its rounds stood in, keeps the copy, and gives it.
    fn try_out(&mut self, from: &Node, actions: &[Action]) -> Option<Node> {
        self.tried += 1;
        let (node, sights) = self.child(from, actions)?;
        timed("note", || self.note(&node, &sights));
        self.keep(&node);
        Some(node)
    }

    /// Why the search ends, where it does.
    fn ended(&self) -> Option<&'static str> {
        if self.found.rounds >= self.bounds.rounds {
            Some("the rounds were spent")
        } else if self.since >= self.bounds.plateau {
            Some("no new line came for the stated rounds")
        } else if self.tried >= self.bounds.rounds * 10 {
            Some("the histories tried were spent")
        } else {
            None
        }
    }

    fn done(&self) -> bool {
        self.ended().is_some()
    }

    /// How much the search has spent on a kept state's pending cell and on the state itself: the less, the sooner the
    /// search goes on from it, so that each cell, one of the engine's own that no round of an agent is carried out of
    /// included, gets its share of the times a state is gone on from.
    fn rank(&self, kept: &Kept) -> usize {
        self.rank_of(&kept.key.0, kept.turns)
    }

    /// The same for a state of this pending cell that was gone on from so many times: a time one is gone on from is
    /// weighed as the draws it makes, many to one turn of a state.
    fn rank_of(&self, cell: &str, turns: usize) -> usize {
        let chosen = self.chosen_at.get(cell).copied().unwrap_or(0);
        chosen * self.bounds.draws * 4 + turns
    }

    /// The kept state gone on from soonest by its rank: of those of the least, the newest on one turn and the oldest on
    /// the next, so that the search goes deep into what it has just reached and comes back to what it reached first.
    fn next(&mut self) -> Option<usize> {
        self.picks += 1;
        let ranks: Vec<usize> = self.archive.iter().map(|k| self.rank(k)).collect();
        let least = *ranks.iter().min()?;
        let among = (0..self.archive.len()).filter(|&i| ranks[i] == least);
        let born = |i: &usize| self.archive[*i].born;
        match self.picks % 2 {
            0 => among.min_by_key(born),
            _ => among.max_by_key(born),
        }
    }

    /// Goes on from a kept state, a few histories each time it is gone on from: a return of the cell under way and a
    /// thing that may happen before a round, each a different one for each state and each turn, so that the states
    /// between them try each; then draws, some going forward only, every block handed in, so that a work goes deep into
    /// its flow, and the others of a few things before a round and a return, some going on from what the draw before it
    /// reached, so that what is reached takes more than one thing to come to.
    fn go_on(&mut self, at: usize) {
        let turn = self.archive[at].turns;
        let born = self.archive[at].born;
        *self.chosen_at.entry(self.archive[at].key.0.clone()).or_default() += 1;
        self.archive[at].turns += 1;
        let node = self.archive[at].node.clone();
        let mut rng = Rng::new(SEED ^ key_number(&self.archive[at].key) ^ turn as u64);
        let how = match node.sim.round() {
            Some(round) => {
                let cell = node.sim.cell(&round).clone();
                let (must, optional) = outputs_of(&node.sim.flow, &cell);
                carries(&cell, must.len(), optional.len())
            }
            None => Vec::new(),
        };
        let mut cursor: Option<Node> = None;
        for place in 0..self.bounds.draws {
            if self.done() {
                return;
            }
            let onward = place >= 2 && rng.chance(60);
            let from = match (&cursor, onward) {
                (Some(cursor), true) => cursor,
                _ => &node,
            };
            let (line, has_round) = facts(from);
            let actions = match place {
                0 if !how.is_empty() => vec![Action::Carry(how[(born + turn) % how.len()])],
                1 => match timed("befores", || systematic_before(&node, &self.statics, born + turn)) {
                    Some(before) => vec![before, Action::Carry(Carry::PLAIN)],
                    None => continue,
                },
                _ if has_round && rng.chance(25) => vec![Action::Carry(forward(&mut rng))],
                _ => {
                    let mut actions = Vec::new();
                    for _ in 0..rng.below(BEFORE + 1) {
                        if let Some(before) = timed("befores", || draw_before(from, &self.statics, &mut rng)) {
                            actions.push(before);
                        }
                    }
                    actions.push(Action::Carry(drawn(&mut rng, line)));
                    actions
                }
            };
            if let Some(next) = self.try_out(from, &actions) {
                cursor = Some(next);
            }
        }
    }
}

/// What the round under way of a state is like: whether its cell is an agent's, and whether there is one.
fn facts(node: &Node) -> (bool, bool) {
    match node.sim.round() {
        Some(round) => (node.sim.cell(&round).line.is_some(), true),
        None => (true, false),
    }
}

/// A return that hands in every block its cell may write, and deals with all that ran it, with a value for a choice
/// drawn.
fn forward(rng: &mut Rng) -> Carry {
    Carry {
        hand: Hand::All,
        picks: rng.next() as u32,
        ..Carry::PLAIN
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
    let statics = things_before(flow, &writes);
    let mut search = Search {
        flow,
        focus,
        bounds: *bounds,
        statics,
        found: Found::default(),
        archive: Vec::new(),
        keys: BTreeSet::new(),
        since: 0,
        tried: 0,
        born: 0,
        picks: 0,
        kept_at: BTreeMap::new(),
        chosen_at: BTreeMap::new(),
    };
    search.found.faults = faults;
    for root in &roots {
        search.keep(root);
    }
    while !search.done() {
        let Some(at) = search.next() else {
            break;
        };
        search.go_on(at);
    }
    let mut pending: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();
    for (cell, kept) in &search.kept_at {
        pending.entry(cell.clone()).or_default().0 = *kept;
    }
    for (cell, chosen) in &search.chosen_at {
        pending.entry(cell.clone()).or_default().1 = *chosen;
    }
    for kept in &search.archive {
        pending.entry(kept.key.0.clone()).or_default().2 += 1;
    }
    search.found.pending = pending;
    search.found.ended = search.ended().unwrap_or("nothing was left to try");
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
