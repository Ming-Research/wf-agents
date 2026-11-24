//! Simulated histories of works of the small flow (design document `testing`, 4), each round filled by the context
//! manager as a carrier fills it: people write, the lines' sessions are reopened by the owner, lost by the agents'
//! program or compacted during a round, rounds fail before they reach the model, return sent back, ask for a tool,
//! and hand in. Beside the work, the test keeps its own account of what each line's session holds, from what each
//! round that reached the model was filled with and handed in, and of the version of each datum each cell last
//! worked from.
//!
//! What is checked of every round, as it is entered in its session and in one that holds nothing:
//! - it is filled with exactly what its cell is given and its session lacks, each datum by its kind, in the order of
//!   the `context` part, by where the cell's definition gives it, then the latest of each block the cell writes that
//!   nothing else gives it, then what the session keeps that it lacks, the results of calls and why what was returned
//!   was sent back;
//! - nothing is filled that the cell is not given, and a datum with no version is never sent; the tree, whose versions
//!   the carrier compares, is told unread where the session holds none of it;
//! - a session that holds nothing is filled from the top, and the task is filled once a session;
//! - what the last return in the session handed in is what the message says it is kept as;
//! - entered again, before its try or after it, the round is sent the same bytes;
//! - a change sent of a datum given `last_and_latest` rebuilds its latest exactly from the version the session holds,
//!   and of the tree, whose versions the carrier compares, is what the carrier says changed, from the version the
//!   session read where the cell worked from none;
//! - what the page shows for the situation the round is in, its places open, has the parts, the forms and the words
//!   of what the engine sends in it, each place standing for what fills it.
//!
//! The histories must reach each of these: a change sent, one of the tree, the tree unread, an output the session
//! lacked, several versions of a datum given `all` sent at once, a session reopened, lost and compacted, results given
//! and a return sent back; each is counted, and a test of no history reaching one fails. A failing case prints its
//! seed.
//!
//! The situations the tool writes (`wf-agents flow situations`) are held to the rounds the histories carry, datum by
//! datum ([`stood`]: a real round is a product of every datum's own state, a written situation one cut of them): on the
//! small flow in both directions, so that every state a datum stood in a round is one the tool writes, and every state
//! it writes was stood in by a round of the histories, which is why the scripted history brings about what the random
//! steps reach too seldom; and on the engineer's flow, which these histories carry with the owner's words, questions
//! and answers, in both directions too, by a search that keeps the state of a history whenever it reached a state of a
//! datum none had and goes on from it, a group of the flow's cells to a test. Each run is steered at a state not yet
//! reached: it begins from a kept state of the target's cell, of one before it or of one after it, forces the choices
//! the target cell runs on, has the owner write and speak to the cells that write the datum, and hands in blocks that
//! change nothing in meaning and returns that deal with part of what ran the round, as real returns do. The search ends
//! where the last runs reached nothing. A state it does not reach is read: it is given a directed test, or it is
//! deleted from the rules as one no round can stand in, by an argument from the flow's files and the deciding code. The
//! check holds each state of each datum, not the joint situations written, so that a datum stood in alone in each of
//! its states may not be stood in together with the others as a written situation says.

mod support;

use std::collections::{BTreeMap, BTreeSet};

use context::message::{Values, kept};
use context::text::as_written;
use context::{Entered, Form, Piece, Situation, Stands};
use decide::run::Unmet;
use decide::{BlockId, CallId, CellId, Condition, Decision, Outsider, RunId, Seq, Waited, Writer, decide};
use engine::spoken::{self, To};
use engine::{Ask, Back, Call, Came, Event, Fill, Given, Handed, Why};
use flow::agents::Part;
use flow::{CellSpec, InputKind, Listed};
use support::{Given as Round, Sim};
use work::{Question, Way};

/// How many histories are run.
const SEEDS: u64 = 90;

/// How many scripted histories are run, each by the seed after those of the random ones.
const SCRIPTED: u64 = 6;

/// How many steps each takes.
const STEPS: usize = 120;

#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(
            seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)
                .wrapping_add(0x2545_F491_4F6C_DD1D)
                | 1,
        )
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
        (self.next() % n as u64) as usize
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

/// What a line's session holds, as the test keeps it.
#[derive(Clone, Debug, Default)]
struct Held {
    versions: BTreeMap<BlockId, BTreeSet<Seq>>,
    parts: Vec<Fill>,
    results: BTreeSet<CallId>,
    sent_back: BTreeSet<(RunId, usize)>,
    /// What the last return in it handed in.
    taken: Vec<(BlockId, Seq)>,
    /// How often each task was filled into it.
    tasks: BTreeMap<String, u32>,
}

impl Held {
    fn holds(&self, block: &BlockId, at: Seq) -> bool {
        self.versions.get(block).is_some_and(|v| v.contains(&at))
    }

    /// What a round that reached the model filled into it.
    fn fill(&mut self, run: RunId, fills: &[Fill]) {
        for fill in fills {
            match fill {
                Fill::Whole { block, version } | Fill::Change { block, version, .. } => {
                    self.versions.entry(block.clone()).or_default().insert(*version);
                }
                Fill::Part { part, digest } => {
                    self.parts.push(fill.clone());
                    if part == Part::Task.name() {
                        let n = self.tasks.entry(digest.clone()).or_default();
                        *n += 1;
                        assert_eq!(*n, 1, "a task filled twice in a session");
                    }
                }
                Fill::Results(calls) => self.results.extend(calls.iter().copied()),
                Fill::SentBack { round } => {
                    self.sent_back.insert((run, *round));
                }
            }
        }
    }
}

/// What has been reached across the histories.
#[derive(Default)]
struct Reached {
    changes: usize,
    files: usize,
    held: usize,
    held_new: usize,
    lacks_last: usize,
    put_in: usize,
    written_over: usize,
    main: usize,
    versions: usize,
    reopened: usize,
    lost: usize,
    compacted: usize,
    results: usize,
    sent_back: usize,
    /// Rounds that failed after their program had the message.
    wrote: usize,
    /// New versions given whole that said what one the session held said.
    said_again: usize,
    /// The tree sent to a session that held none of it.
    unread: usize,
    /// A block the cell writes, given by nothing else, sent as the session lacked its latest.
    outputs: usize,
    /// The owner's word to a run at its limit.
    at_limit: usize,
    /// What the entries of real rounds stood in, by [`stood`].
    real: BTreeSet<String>,
    /// For a line of `real`, the first round that stood in it: why its run began, its number, and the situation.
    examples: BTreeMap<String, String>,
    /// Rounds carried to a session of an agent's cell.
    rounds: usize,
    /// Of those, how many of each cell.
    of_cell: BTreeMap<String, usize>,
    /// The rounds of the last scripted history.
    trace: Vec<String>,
    /// The directed tests that did not bring the work where they say, and why.
    stale: Vec<String>,
    /// For each agent's cell, the states of a history at the entry of a round of it, one for each situation it stood
    /// in.
    archive: BTreeMap<String, Vec<Kept>>,
}

/// A state of a history kept at the entry of a round, with the number of the situation the round stood in and how each
/// datum stood in it.
type Kept = (u64, BTreeMap<String, Stands>, History);

/// What a history of a flow does beside carrying its rounds.
#[derive(Clone)]
struct Scene {
    /// The blocks people or the outside write now and then, a block named twice written twice as often.
    writes: Vec<&'static str>,
    /// The lines of the flow, whose sessions the owner reopens now and then.
    lines: Vec<String>,
    /// Whether what each round is sent is held to the test's own account of the design, which the small flow's blocks
    /// are all the kinds of; the engineer's flow is held to what the tool writes alone.
    models: bool,
    /// Whether the owner speaks to the cells that take words and answers the questions cells ask.
    talks: bool,
    /// How many steps in a hundred a person or the outside writes.
    puts: usize,
    /// How many steps in a hundred, after those, the owner reopens a line's session, and then speaks or answers.
    reopens: usize,
    speaks: usize,
    /// How many steps in a hundred, after those, a run of the search changes what the work holds at its target
    /// ([`History::perturb`]).
    nudges: usize,
    /// How often, in a hundred, a cell that writes a choice with the value `yes` writes it, so that the work goes on to
    /// its later cells; none where the choice is any of its values alike.
    passes: Option<u64>,
    /// The choices the passes are of, which let the work go on.
    passing: Vec<&'static str>,
}

impl Scene {
    fn small() -> Scene {
        Scene {
            writes: vec![
                "request", "notes", "tree", "plan", "opinions", "finish", "finish", "verdict", "verdict",
            ],
            lines: vec!["engineer".to_string(), "reviewer".to_string()],
            models: true,
            talks: false,
            puts: 22,
            reopens: 4,
            speaks: 6,
            nudges: 0,
            passes: None,
            passing: Vec::new(),
        }
    }

    /// People and the outside write what the work reads from them: the background, the branch's tree and head, and the
    /// times of the project's tests. The items it depends on have merged as it starts.
    fn engineer(sim: &Sim) -> Scene {
        let lines = sim.flow.lines.keys().cloned().collect();
        Scene {
            writes: vec![
                "background",
                "tree",
                "tree",
                "head",
                "head",
                "test_times",
                "plan_ok",
                "tree_ok",
                "review_ok",
                "test_ok",
                "design_ok",
                "testing_ok",
            ],
            lines,
            models: false,
            talks: true,
            puts: 8,
            reopens: 4,
            speaks: 6,
            nudges: 0,
            passes: Some(70),
            passing: vec![
                "plan_ok",
                "tree_ok",
                "review_ok",
                "test_ok",
                "design_ok",
                "testing_ok",
                "checked",
                "needs_testing",
            ],
        }
    }
}

/// One history, and the test's account beside it.
#[derive(Clone)]
struct History {
    sim: Sim,
    scene: Scene,
    rng: Rng,
    /// What each line's session holds.
    lines: BTreeMap<String, Held>,
    /// The version of each datum each cell last worked from.
    worked: BTreeMap<(CellId, BlockId), Seq>,
    /// The last content written of each block.
    last: BTreeMap<String, String>,
    /// Every content written of each block, so that one may be written again.
    written: BTreeMap<String, Vec<String>>,
    count: u64,
    /// What the next returns do, in turn, where a history is scripted: ask for this call, or hand in a block the cell
    /// may not write beside the rest. A scripted history's other returns ask for nothing and are never sent back.
    forced: Option<Vec<Forced>>,
    /// How many of the first steps only carry what the work gives and let it go on, so that the history goes as far
    /// into the flow as it can before anything else happens.
    quiet: usize,
    /// The rounds carried, each by its cell, its run and its number among the run's.
    trace: Vec<String>,
    /// How often, in a hundred, a return hands in a block its cell need not write, and how often a return of a cell
    /// that takes the owner's answers asks a question; and, of some blocks, how often the return hands in that block
    /// alone.
    writes: u64,
    asks: u64,
    /// How often, in a hundred, a block a return hands in that was written before is handed in as changing nothing in
    /// meaning.
    unmatters: u64,
    /// How often, in a hundred, a return that hands in names only some of what ran its round as dealt with.
    partial: u64,
    /// How often, in a hundred, a return asks for a tool, so that its run goes on to a round after the first.
    calls: u64,
    /// Whether the datum the run is steered at is handed in, mostly, as changing nothing in meaning.
    unmatter_datum: bool,
    /// Whether the returns hand in, beside what their cell must, only the blocks near the target, so that the cells
    /// that watch the rest are not run.
    focus: bool,
    /// How many of the first steps, which only carry the work's rounds, hand in everything and let every choice pass,
    /// so that the work goes as far into the flow as it can.
    forward: usize,
    /// Whether nothing happens by chance: no round is lost, fails or is compacted, no return asks for a tool or leaves
    /// out what its cell must write, and no round is also entered anew. A directed test is run so.
    steady: bool,
    /// What the next returns of each cell hand in, in turn, in place of what the draw gives.
    says: BTreeMap<String, Vec<Say>>,
    /// The cells whose returns deal with nothing, so that a cell that runs until a run of it has dealt with something
    /// is run again.
    nothing_dealt: BTreeSet<String>,
    prefer: BTreeMap<String, u64>,
    /// The cells whose returns ask a question their cell waits for the answer to, where the cell takes answers.
    waits: BTreeSet<String>,
    /// Whether the next round to return compacts its session.
    compact_next: bool,
    /// The value a return that writes a choice gives it, of some choices.
    force: BTreeMap<String, String>,
    /// What a run of the search is steered at.
    target: Option<Target>,
}

/// What the return of a cell hands in, exactly: each block it hands in beside what its cell must write every time, and
/// whether it changes its meaning.
#[derive(Clone)]
struct Say {
    writes: Vec<(String, bool)>,
    /// Whether it asks the owner a question, which its cell does not wait for.
    asks: bool,
}

impl Say {
    /// Hands in these blocks, each as changing its meaning.
    fn all(blocks: &[&str]) -> Say {
        Say {
            writes: blocks.iter().map(|b| (b.to_string(), true)).collect(),
            asks: false,
        }
    }

    /// The same, asking a question.
    fn asking(mut self) -> Say {
        self.asks = true;
        self
    }

    /// Hands in these blocks, each as changing its meaning, and these, each as changing nothing in it.
    fn with(changed: &[&str], unchanged: &[&str]) -> Say {
        let mut writes: Vec<(String, bool)> = changed.iter().map(|b| (b.to_string(), true)).collect();
        writes.extend(unchanged.iter().map(|b| (b.to_string(), false)));
        Say { writes, asks: false }
    }
}

/// What a scripted return does.
#[derive(Clone)]
enum Forced {
    Ask(Ask),
    Wrong,
    /// Hands in every block the cell writes.
    All,
    /// Hands in the blocks the cell must write every time and no other.
    Must,
    /// The round that comes next fails after its program had its message.
    Fail,
}

/// How a history's returns are drawn, which a run of the search sets and the first steps of a work that goes forward
/// set aside.
struct Mode {
    writes: u64,
    asks: u64,
    calls: u64,
    partial: u64,
    unmatters: u64,
    passes: Option<u64>,
    focus: bool,
    unmatter_datum: bool,
    prefer: BTreeMap<String, u64>,
    nothing_dealt: BTreeSet<String>,
}

impl Mode {
    /// Every block handed in, every choice passing, no tool asked for, no question asked, nothing handed in as changing
    /// nothing in meaning, and nothing steered but the cells whose returns deal with nothing.
    fn forward(nothing_dealt: BTreeSet<String>) -> Mode {
        Mode {
            writes: 100,
            asks: 0,
            calls: 0,
            partial: 0,
            unmatters: 0,
            passes: Some(100),
            focus: false,
            unmatter_datum: false,
            prefer: BTreeMap::new(),
            nothing_dealt,
        }
    }
}

impl History {
    /// Sets the mode and gives the one it replaces.
    fn swap_mode(&mut self, mode: Mode) -> Mode {
        Mode {
            writes: std::mem::replace(&mut self.writes, mode.writes),
            asks: std::mem::replace(&mut self.asks, mode.asks),
            calls: std::mem::replace(&mut self.calls, mode.calls),
            partial: std::mem::replace(&mut self.partial, mode.partial),
            unmatters: std::mem::replace(&mut self.unmatters, mode.unmatters),
            passes: std::mem::replace(&mut self.scene.passes, mode.passes),
            focus: std::mem::replace(&mut self.focus, mode.focus),
            unmatter_datum: std::mem::replace(&mut self.unmatter_datum, mode.unmatter_datum),
            prefer: std::mem::replace(&mut self.prefer, mode.prefer),
            nothing_dealt: std::mem::replace(&mut self.nothing_dealt, mode.nothing_dealt),
        }
    }

    fn new(seed: u64) -> History {
        let mut sim = Sim::of(support::limited(), support::words());
        sim.takes_main = true;
        History::of(sim, Scene::small(), seed)
    }

    fn of(sim: Sim, scene: Scene, seed: u64) -> History {
        let mut history = History {
            sim,
            scene,
            rng: Rng::new(seed),
            lines: BTreeMap::new(),
            worked: BTreeMap::new(),
            last: BTreeMap::new(),
            written: BTreeMap::new(),
            count: 0,
            forced: None,
            quiet: 0,
            trace: Vec::new(),
            writes: 70,
            asks: 8,
            unmatters: 0,
            partial: 0,
            calls: 15,
            unmatter_datum: false,
            focus: false,
            forward: 0,
            steady: false,
            says: BTreeMap::new(),
            nothing_dealt: BTreeSet::new(),
            prefer: BTreeMap::new(),
            waits: BTreeSet::new(),
            compact_next: false,
            force: BTreeMap::new(),
            target: None,
        };
        history.start();
        history
    }

    /// The owner starts the work by writing the first of what people write that runs the flow's first agent's cell, as
    /// the tool says a work starts, the items it depends on having merged where the flow waits for them.
    fn start(&mut self) {
        let flow = &self.sim.flow;
        if flow.blocks.contains_key(&BlockId::from("dependencies")) {
            self.sim.wrote("dependencies", "", Some("merged"), Outsider::Outside);
        }
        let flow = &self.sim.flow;
        let first = (flow.flow.cells().iter())
            .filter_map(|c| flow.cells.iter().find(|s| s.id == c.id))
            .find(|c| c.kind == flow::Kind::Agent)
            .expect("an agent's cell");
        let blocks: Vec<String> = (first.watches.iter())
            .find(|t| !t.is_group() && flow.blocks[&BlockId::from(t.name.as_str())].writers.people)
            .map(|t| t.name.clone())
            .into_iter()
            .collect();
        for block in blocks {
            let content = self.content(&block);
            self.sim.writes(&block, &content);
        }
    }

    /// A new content of the block: now and then one it had before, as the same said twice or a change taken back;
    /// otherwise the last one with a line changed or added, or a first one of many lines.
    fn content(&mut self, block: &str) -> String {
        self.count += 1;
        let before = self.written.get(block).cloned().unwrap_or_default();
        if !before.is_empty() && self.rng.chance(15) {
            let again = before[self.rng.below(before.len())].clone();
            self.last.insert(block.to_string(), again.clone());
            return again;
        }
        let mut lines: Vec<String> = match self.last.get(block) {
            Some(last) => last.split('\n').map(str::to_string).collect(),
            None => (1..=12).map(|n| format!("{block}, line {n}")).collect(),
        };
        let at = self.rng.below(lines.len());
        if self.rng.chance(70) {
            lines[at] = format!("{block}, changed {}", self.count);
        } else {
            lines.insert(at, format!("{block}, added {}", self.count));
        }
        let text = lines.join("\n");
        self.last.insert(block.to_string(), text.clone());
        self.written.entry(block.to_string()).or_default().push(text.clone());
        text
    }

    /// Whether the work has ended.
    fn ended(&self) -> bool {
        matches!(decide(&self.sim.flow.flow, &self.sim.work.records), Decision::Ended(_))
    }

    /// One of the first steps, which only carry what the work gives and let it go on. Says whether the work gave any.
    fn quiet_step(&mut self, reached: &mut Reached) -> bool {
        if let Some(round) = self.sim.round() {
            self.carry(&round, reached);
            return true;
        }
        if let Some(call) = self.sim.calls().first() {
            self.sim.done(*call, "the rows");
            return true;
        }
        self.lets_go()
    }

    fn step(&mut self, reached: &mut Reached) {
        if self.quiet > 0 {
            self.quiet -= 1;
            // The work goes forward: every block its cells write is handed in, every choice passes, no tool is asked
            // for and nobody asks a question, as a work that goes well does.
            let kept = (self.forward > 0).then(|| {
                self.forward -= 1;
                self.swap_mode(Mode::forward(self.nothing_dealt.clone()))
            });
            let acted = self.quiet_step(reached);
            if let Some(kept) = kept {
                self.swap_mode(kept);
            }
            if acted {
                return;
            }
        }
        let roll = self.rng.below(100);
        let (puts, reopens, speaks) = (self.scene.puts, self.scene.reopens, self.scene.speaks);
        if roll < puts {
            let block = self.scene.writes[self.rng.below(self.scene.writes.len())];
            self.put(block);
        } else if roll < puts + reopens {
            let line = self.scene.lines[self.rng.below(self.scene.lines.len())].clone();
            self.reopen(&line, reached);
        } else if self.scene.talks && roll < puts + reopens + speaks && self.speaks() {
            // The owner spoke or answered.
        } else if roll < puts + reopens + speaks + self.scene.nudges && self.target.is_some() {
            let target = self.target.clone().expect("a target");
            self.perturb(reached, &target);
        } else if let Some(round) = self.sim.round() {
            self.carry(&round, reached);
        } else if let Some(call) = self.sim.calls().first() {
            self.sim.done(*call, "the rows");
        } else if !self.lets_go() {
            let block = self.scene.writes[self.rng.below(self.scene.writes.len())];
            self.put(block);
        }
    }

    /// A person or the outside writes the block anew: a choice with one of its values.
    fn put(&mut self, block: &str) {
        let content = self.content(block);
        let declared = &self.sim.flow.blocks[&BlockId::from(block)];
        let by = if declared.writers.people {
            Outsider::Person
        } else {
            Outsider::Outside
        };
        match declared.values.clone() {
            Some(values) => {
                let value = &values[self.rng.below(values.len())];
                self.sim.wrote(block, value, Some(value.as_str()), by);
            }
            None => self.sim.wrote(block, &content, None, by),
        }
    }

    /// A person or the outside writes the choice with this value.
    fn put_value(&mut self, block: &str, value: &str) {
        let by = match self.sim.flow.blocks[&BlockId::from(block)].writers.people {
            true => Outsider::Person,
            false => Outsider::Outside,
        };
        self.sim.wrote(block, value, Some(value), by);
    }

    /// What the work holds changes before its next round, as it does among the random steps: a person or the outside
    /// writes, the owner reopens a line's session or speaks to a cell, a cell is made to ask a question, or the blocks
    /// the next returns hand in are chosen. Each is done nearer the target than the random steps would, where it can
    /// be.
    fn perturb(&mut self, reached: &mut Reached, target: &Target) {
        match self.rng.below(9) {
            0 | 1 => {
                let mine: Vec<&'static str> = (self.scene.writes.iter().copied())
                    .filter(|w| target.blocks.iter().any(|b| b == w))
                    .collect();
                let block = match mine.is_empty() || !self.rng.chance(70) {
                    true => self.scene.writes[self.rng.below(self.scene.writes.len())],
                    false => mine[self.rng.below(mine.len())],
                };
                self.put(block);
            }
            2 => {
                let own = (self.sim.flow.cells.iter())
                    .find(|c| c.id.as_str() == target.cell)
                    .and_then(|c| c.line.clone());
                let line = match own {
                    Some(line) if self.rng.chance(70) => line,
                    _ => self.scene.lines[self.rng.below(self.scene.lines.len())].clone(),
                };
                self.reopen(&line, reached);
            }
            3 | 4 => {
                if self.scene.talks {
                    self.speaks_near(&target.blocks);
                }
            }
            5 => self.asks = [8, 40, 90][self.rng.below(3)],
            6 | 7 => {
                // A choice a cell writes is given one of its values, or left to the draw.
                let choices: Vec<&BlockId> = (self.sim.flow.blocks.iter())
                    .filter(|(_, d)| d.values.is_some() && !d.writers.cells.is_empty())
                    .map(|(b, _)| b)
                    .collect();
                let block = choices[self.rng.below(choices.len())].clone();
                let values = self.sim.flow.blocks[&block].values.clone().unwrap_or_default();
                match self.rng.chance(25) {
                    true => {
                        self.force.remove(block.as_str());
                    }
                    false => {
                        let value = values[self.rng.below(values.len())].clone();
                        self.force.insert(block.as_str().to_string(), value);
                    }
                }
            }
            _ => {
                if !target.blocks.is_empty() {
                    let block = target.blocks[self.rng.below(target.blocks.len())].clone();
                    self.prefer.insert(block, [0, 100][self.rng.below(2)]);
                }
            }
        }
    }

    /// The owner has the line begin a new session. A round of the line that has begun goes on in the session it began
    /// in, and the line's next round runs in the new one: which round begins a session is what the records say
    /// ([`Sim::begins`]), where the test's account of what the session holds is begun anew ([`History::carry`]).
    fn reopen(&mut self, line: &str, reached: &mut Reached) {
        self.sim.anew(line, Why::Owner);
        reached.reopened += 1;
    }

    /// The owner says a few words to a cell that takes them, or answers a question that stands unanswered. Says whether
    /// either was possible.
    fn speaks(&mut self) -> bool {
        self.speaks_near(&[])
    }

    /// As [`History::speaks`], where the words go, more often than not, to a cell whose words are one of these blocks.
    fn speaks_near(&mut self, near: &[String]) -> bool {
        // Where a cell is made to ask a question it waits for, the owner speaks to it first, which enters it by the
        // words alone, and answers later.
        let answering = if self.waits.is_empty() { 50 } else { 10 };
        if self.rng.chance(answering) && self.answers() {
            return true;
        }
        let cells: Vec<CellId> = (self.sim.flow.cells.iter())
            .filter(|c| spoken::words_of(&self.sim.flow, &c.id).is_some())
            .map(|c| c.id.clone())
            .collect();
        let wanted: Vec<CellId> = (cells.iter())
            .filter(|c| (spoken::words_of(&self.sim.flow, c)).is_some_and(|b| near.iter().any(|n| n == b.as_str())))
            .cloned()
            .collect();
        let to = match wanted.is_empty() || !self.rng.chance(80) {
            true => To::Cell(cells[self.rng.below(cells.len())].clone()),
            false => To::Cell(wanted[self.rng.below(wanted.len())].clone()),
        };
        if spoken::receiver(&self.sim.flow, &self.sim.work, &to).is_err() {
            return false;
        }
        let words = format!("Do it again, {}", self.count);
        self.sim.take(Event::Said { to, words });
        true
    }

    /// The owner answers a question that stands with no answer, by either option. Says whether there was one.
    fn answers(&mut self) -> bool {
        let records = &self.sim.work.records;
        let unanswered: Vec<(String, u64)> = (decide::questions::questions(records).into_iter())
            .filter(|(_, q)| q.unanswered())
            .filter_map(|(name, q)| {
                let number = engine::asked::number_of(&self.sim.flow, records, &q.latest.cell, q.latest.at);
                Some((name.to_string(), number?))
            })
            .collect();
        if unanswered.is_empty() {
            return false;
        }
        let (question, version) = unanswered[self.rng.below(unanswered.len())].clone();
        let option = 1 + self.rng.below(2) as u64;
        self.sim.take(Event::Answer {
            question,
            version,
            option,
        });
        true
    }

    /// The owner does what the work waits for: lets a cell at its limit go on, or answers a call of a run that reached
    /// the limit of its returns. Says whether the work waited for one.
    fn lets_go(&mut self) -> bool {
        let Decision::Wait { at, on } = decide(&self.sim.flow.flow, &self.sim.work.records) else {
            return false;
        };
        for waited in on {
            match waited {
                Waited::Limit { .. } | Waited::Calls { .. } => {
                    self.sim.take(Event::LetGo { cell: at });
                    return true;
                }
                Waited::Call { call, on: Some(_) } => {
                    self.sim.done(call, "Go on.");
                    return true;
                }
                Waited::Answer { .. } if self.answers() => return true,
                _ => {}
            }
        }
        false
    }

    /// Carries every round the work gives, answers every call and lets every cell at its limit go on, until it rests.
    fn drain(&mut self, reached: &mut Reached) {
        for _ in 0..200 {
            if let Some(round) = self.sim.round() {
                self.carry(&round, reached);
            } else if let Some(call) = self.sim.calls().first() {
                self.sim.done(*call, "the rows");
            } else if !self.lets_go() {
                return;
            }
        }
    }

    /// A history for a directed test of the engineer's flow, in which nothing happens by chance: each cell hands in
    /// what it must write and what the test says, every choice passes, and nobody asks a question or a tool.
    fn directed(seed: u64) -> History {
        let sim = support::engineer_sim();
        let scene = Scene::engineer(&sim);
        let mut history = History::of(sim, scene, seed);
        history.steady_mode();
        history
    }

    /// Nothing happens by chance from here: each cell hands in what it must write and what the test says, every choice
    /// passes, and nobody asks a question or a tool, and what a run was steered at is let go.
    fn steady_mode(&mut self) {
        self.steady = true;
        self.writes = 0;
        self.asks = 0;
        self.calls = 0;
        self.partial = 0;
        self.unmatters = 0;
        self.quiet = 0;
        self.forward = 0;
        self.focus = false;
        self.unmatter_datum = false;
        self.scene.passes = Some(100);
        self.target = None;
        self.forced = None;
        self.says.clear();
        self.nothing_dealt.clear();
        self.prefer.clear();
        self.waits.clear();
        self.compact_next = false;
    }

    /// Carries what the work gives until the round next is of this cell, which it leaves to be carried. Says where the
    /// work stood where it does not come to one: the directed test is stale.
    fn until(&mut self, reached: &mut Reached, cell: &str) -> Result<(), String> {
        if self.until_within(reached, cell, 300) {
            return Ok(());
        }
        // What the last return sent back was sent back for, where a run goes on sending its returns back.
        let last_run = (self.sim.work.given.iter().rev())
            .find(|(_, given)| matches!(given, Given::SentBack(_)))
            .map(|((run, _), _)| *run);
        let mut sent_back: Vec<String> = (self.sim.work.given.iter())
            .filter(|((run, _), _)| Some(*run) == last_run)
            .filter_map(|(_, given)| match given {
                Given::SentBack(unmet) => Some(format!("{unmet:?}")),
                _ => None,
            })
            .collect();
        sent_back.dedup();
        let sent_back = sent_back.join(" | ");
        Err(format!(
            "no round of {cell} came: the work stands at {:.300}; the last return sent back was sent back for {sent_back:.400}; the last rounds carried: {:?}",
            format!("{:?}", decide(&self.sim.flow.flow, &self.sim.work.records)),
            self.trace.iter().rev().take(4).collect::<Vec<_>>()
        ))
    }

    /// Carries what the work gives, in at most this many steps, until the round next is of this cell, which it leaves
    /// to be carried. Says whether it came to one.
    fn until_within(&mut self, reached: &mut Reached, cell: &str, steps: usize) -> bool {
        for _ in 0..steps {
            match self.sim.round() {
                Some(round) if round.cell.as_str() == cell => return true,
                Some(round) => self.carry(&round, reached),
                None => match self.sim.calls().first() {
                    Some(call) => self.sim.done(*call, "the rows"),
                    None if self.lets_go() => {}
                    None => return false,
                },
            }
        }
        false
    }

    /// Has the cell run: the owner says some words to it, or, where it has none of its own, one block it watches by
    /// itself is written, by a person or the outside, or by the cell that writes it, which is run in the same way, to a
    /// depth.
    fn wake(&mut self, reached: &mut Reached, cell: &str, depth: u8) {
        if self.say_to(cell) || depth >= 2 {
            return;
        }
        let Some(spec) = self.sim.flow.cells.iter().find(|c| c.id.as_str() == cell) else {
            return;
        };
        let watched: Vec<String> = (spec.watches.iter().filter(|t| !t.is_group()))
            .flat_map(|t| t.blocks.iter().map(|b| b.as_str().to_string()))
            .collect();
        for block in watched {
            if self.scene.writes.iter().any(|w| *w == block) {
                self.put(&block);
                return;
            }
            let writers = (self.sim.flow.blocks.get(&BlockId::from(block.as_str())))
                .map(|d| d.writers.cells.clone())
                .unwrap_or_default();
            if let Some(writer) = writers.first() {
                Between::Writes(writer.as_str().to_string(), block).done(self, reached, depth + 1);
                return;
            }
        }
    }

    /// Carries the round under way.
    fn carry_round(&mut self, reached: &mut Reached) {
        if let Some(round) = self.sim.round() {
            self.carry(&round, reached);
        }
    }

    /// Carries the round of this cell that comes next.
    fn carry_next(&mut self, reached: &mut Reached, cell: &str) -> Result<(), String> {
        self.until(reached, cell)?;
        let round = self.sim.round().ok_or("the round of the cell is gone")?;
        self.carry(&round, reached);
        Ok(())
    }

    /// Brings the work to a round of this cell whose run took main in, which the number of the run decides; where it
    /// did not, the round is carried, and what runs the cell is done again, which `wake` does, a few times.
    fn until_with_main(&mut self, reached: &mut Reached, cell: &str, wake: fn(&mut History)) -> Result<(), String> {
        for _ in 0..8 {
            wake(self);
            self.until(reached, cell)?;
            let round = self.sim.round().ok_or("the round of the cell is gone")?;
            if self.sim.main(&round).is_some() {
                return Ok(());
            }
            self.carry(&round, reached);
        }
        Err(format!("no run of {cell} took main in"))
    }

    /// The owner says some words to the cell. Says whether the work takes them: a cell has words of its own or not, and
    /// takes none while its condition does not hold.
    fn say_to(&mut self, cell: &str) -> bool {
        let id = (self.sim.flow.cells.iter())
            .find(|c| c.id.as_str() == cell)
            .map(|c| c.id.clone())
            .expect("a cell of the flow");
        let to = To::Cell(id);
        if spoken::receiver(&self.sim.flow, &self.sim.work, &to).is_err() {
            return false;
        }
        self.count += 1;
        let words = format!("Do it again, {}", self.count);
        self.sim.take(Event::Said { to, words });
        true
    }

    /// The owner answers each question that stands unanswered.
    fn answers_all(&mut self) {
        while self.answers() {}
    }

    /// The scripted history: each state of a datum that the random steps reach too seldom to count on, in turn, each
    /// brought about by what happens in the engine's own order. A datum the cell is given may be written after a run
    /// began and before its round is entered, and a return sent back is followed by a round of the same run, which
    /// holds what the first was sent.
    fn script(&mut self, reached: &mut Reached) {
        let tool = |input: &str| {
            Forced::Ask(Ask::Tool {
                name: "query".to_string(),
                input: input.to_string(),
            })
        };
        self.forced = Some(Vec::new());
        // The plan reads the notes and the tree, and the request, with the opinions, runs it.
        self.put("notes");
        self.put("tree");
        // The opinions are written while the first run is under way, and its first return is sent back.
        self.put("opinions");
        self.forced = Some(vec![Forced::Wrong]);
        self.drain(reached);
        self.put("opinions");
        self.drain(reached);
        // A run by the request, in which the opinions, the notes and the tree are written, its return sent back; and
        // the same by the opinions, in which the request is written.
        for (ran, written) in [
            ("request", ["opinions", "notes", "tree"]),
            ("opinions", ["request", "notes", "tree"]),
        ] {
            self.put(ran);
            self.under_way("plan");
            for block in written {
                self.put(block);
            }
            // Its first return is sent back, and so is the second, at the limit of its returns, which the owner lets
            // pass: the round after the second holds what the first round, which began before they were written, never
            // was sent.
            self.forced = Some(vec![Forced::Wrong, Forced::Wrong]);
            self.drain(reached);
        }
        // The build is run by a plan a person writes, and the request is written while its run is under way.
        self.put("plan");
        self.drain(reached);
        self.put("plan");
        self.under_way("build");
        self.put("request");
        self.forced = Some(vec![Forced::Wrong]);
        self.drain(reached);
        // A build asks for a tool run on something and on nothing, and for another, is sent back twice, so that it
        // reaches the limit of its returns, and the owner lets it go on; its rounds are given each result.
        self.put("plan");
        self.under_way("build");
        self.forced = Some(vec![
            tool("x"),
            tool(""),
            tool("z"),
            Forced::Wrong,
            Forced::Wrong,
            tool("y"),
        ]);
        self.drain(reached);
        // The notes are written after the review's last run, and its next run reads them in its first round, which
        // fails after its program had the message: its second round holds them as the latest, which the review never
        // worked from as they are. The first of the two runs gives the review a last run to have worked from.
        for _ in 0..2 {
            self.put("notes");
            self.put("plan");
            self.under_way("build");
            self.forced = Some(vec![Forced::All, Forced::Fail]);
            self.drain(reached);
        }
        // The close, run by the finish, reads the verdict, and its run ends the work once it hands in or asks for a
        // call: its first two returns are sent back, which is the limit of its returns.
        self.forced = Some(vec![Forced::Wrong, Forced::Wrong]);
        self.put("verdict");
        self.put("finish");
        self.under_way("close");
        for _ in 0..2 {
            let round = self.sim.round().expect("the close's round");
            self.carry(&round, reached);
        }
        // While the owner is to let it go on, the request is written: the plan and the build, which hand in what they
        // write, are run before its next round, which the session holds as the build's hand-in last.
        let close = self.forced.replace(vec![Forced::All, Forced::All, Forced::Must]);
        self.put("request");
        self.note("after the request");
        for _ in 0..20 {
            match self.sim.round() {
                Some(round) if round.cell.as_str() != "close" => self.carry(&round, reached),
                _ => break,
            }
        }
        self.note("before the close's next round");
        self.forced = close;
        self.drain(reached);
        self.note("after the drain");
    }

    /// Keeps what the work does next, as the trace's own line.
    fn note(&mut self, what: &str) {
        let next = format!("{:?}", decide(&self.sim.flow.flow, &self.sim.work.records));
        self.trace.push(format!("{what}: {}", &next[..next.len().min(200)]));
    }

    /// The round under way is this cell's.
    fn under_way(&self, cell: &str) {
        let round = self.sim.round().expect("a round is under way");
        assert_eq!(round.cell.as_str(), cell, "the round under way is another cell's");
    }

    /// Carries the round out, checking what it is sent.
    fn carry(&mut self, round: &Round, reached: &mut Reached) {
        // A cell of the engine's own is sent nothing: it hands in what it writes.
        if self.sim.cell(round).line.is_none() {
            let came = self.came(round);
            self.sim.returns(round, came, 0);
            return;
        }
        let line = self.sim.line(round);
        self.trace
            .push(format!("{}#{} round {}", round.cell, round.run.0, round.number));
        // A round that begins its line's session is the first to put anything into it, whatever the account held of the
        // session before.
        let begins = self.sim.begins(round);
        if begins {
            self.lines.insert(line.clone(), Held::default());
        }
        let held = self.lines.get(&line).cloned().unwrap_or_default();
        let going_on = self.sim.enter(round, false);
        let lost = !begins && !self.steady && self.rng.chance(10);
        // Entered in a session that holds nothing: on the small flow every round; on the engineer's where the round
        // begins one or is lost, and in two of five of the others, which each take as long as the walk of the work to
        // enter.
        let anew = (self.scene.models || begins || lost || (!self.steady && self.rng.chance(40)))
            .then(|| self.sim.enter(round, true));
        self.check(round, &going_on, &held, reached);
        if let Some(anew) = &anew {
            self.check(round, anew, &Held::default(), reached);
        }
        if !self.scene.models {
            self.keep(round.cell.as_str(), &going_on.situation, reached);
        }
        let used = if lost {
            let anew = anew.as_ref().expect("a round that is lost is entered anew");
            self.sim.tries(round, 1, going_on.fills.clone());
            self.sim.anew(&line, Why::Lost);
            self.lines.insert(line.clone(), Held::default());
            self.sim.tries(round, 2, anew.fills.clone());
            reached.lost += 1;
            anew.clone()
        } else {
            let used = match &anew {
                Some(anew) if begins => anew.clone(),
                _ => going_on.clone(),
            };
            self.sim.tries(round, 1, used.fills.clone());
            used
        };
        // What a round is sent is checked in full on the small flow; on the engineer's flow only the situations its
        // rounds stood in are kept, so that the search over them is not made slow by what the small flow already holds.
        if self.scene.models {
            assert_eq!(self.sim.enter(round, false), going_on, "entered again after its try");
            assert_eq!(
                Some(self.sim.enter(round, true)),
                anew,
                "entered again anew after its try"
            );
        }
        // A message gives a new version whole though it says what one the session holds said.
        for fill in going_on.fills.iter().filter(|_| self.scene.models) {
            let Fill::Whole { block, version } = fill else {
                continue;
            };
            let text = self.sim.content(block.as_str(), *version);
            let had = held.versions.get(block).into_iter().flatten();
            if had
                .into_iter()
                .any(|h| h != version && self.sim.content(block.as_str(), *h) == text)
            {
                reached.said_again += 1;
            }
        }
        let scripted = matches!(self.forced.as_ref().and_then(|f| f.first()), Some(Forced::Fail));
        if let Some(forced) = self.forced.as_mut().filter(|_| scripted) {
            forced.remove(0);
        }
        if scripted || (!self.steady && self.rng.chance(8)) {
            // A round that failed after its program had its message, as at the usage limit, put it into its session;
            // one that failed before its program ran put nothing there.
            if scripted || self.rng.chance(50) {
                self.lines.entry(line.clone()).or_default().fill(round.run, &used.fills);
                self.sim.fails_as(round, decide::run::Failure::UsageLimit, true);
                reached.wrote += 1;
            } else {
                self.sim.fails(round);
            }
            return;
        }
        self.lines.entry(line.clone()).or_default().fill(round.run, &used.fills);
        let compacts = std::mem::take(&mut self.compact_next);
        let compactions = u32::from(compacts || (!self.steady && self.rng.chance(10)));
        let came = self.came(round);
        self.sim.returns(round, came, compactions);
        if compactions > 0 {
            self.lines.insert(line.clone(), Held::default());
            reached.compacted += 1;
        }
        // What the round handed in, where its run finished at it.
        let records = &self.sim.work.records;
        let finished = records.finished.iter().find(|f| f.run == round.run).cloned();
        let held = self.lines.entry(line).or_default();
        held.taken = Vec::new();
        if let Some(f) = &finished {
            for w in &f.writes {
                held.versions.entry(w.block.clone()).or_default().insert(f.at);
                held.taken.push((w.block.clone(), f.at));
            }
            if self.scene.models {
                self.worked_from(round, f);
            }
        }
    }

    /// What the round returns: a tool asked for in one in ten, the review's verdict left out in one in seven, and
    /// otherwise each block the cell writes with a chance of seven in ten, those it must write every time.
    fn came(&mut self, round: &Round) -> Came {
        let scripted = self.forced.is_some();
        let (mut wrong, mut all, mut only_must) = (false, false, false);
        match self.forced.as_mut().filter(|f| !f.is_empty()).map(|f| f.remove(0)) {
            Some(Forced::Ask(ask)) => return Came::asks(vec![ask]),
            Some(Forced::Wrong) => wrong = true,
            Some(Forced::All) => all = true,
            Some(Forced::Must) => only_must = true,
            Some(Forced::Fail) => {}
            None => {}
        }
        let said = (self.says.get_mut(round.cell.as_str())).and_then(|says| (!says.is_empty()).then(|| says.remove(0)));
        if !scripted && !self.steady && self.rng.chance(self.calls) {
            // A tool run on something, or a tool run on nothing.
            let ask = match self.rng.below(3) {
                0 | 1 => Ask::Tool {
                    name: "query".to_string(),
                    input: format!("{{\"n\": {}}}", self.count),
                },
                _ => Ask::Tool {
                    name: "query".to_string(),
                    input: String::new(),
                },
            };
            return Came::asks(vec![ask]);
        }
        let cell = self.sim.cell(round).clone();
        let place = self.sim.flow.flow.place(&cell.id).expect("the cell's place");
        let outputs = self.sim.flow.flow.cells()[place].outputs.clone();
        let leaves_out = !cell.must_write.is_empty() && !all && !self.steady && self.rng.chance(14);
        let mut writes = Vec::new();
        for block in outputs {
            // A question is asked in the return's own field, and what answers it is the owner's.
            if self.sim.flow.blocks[&block].writers.outside.as_deref() == Some(flow::ANSWER) {
                continue;
            }
            let must = cell.must_write.contains(&block);
            let percent = self.prefer.get(block.as_str()).copied().unwrap_or(self.writes);
            let listed = said
                .as_ref()
                .and_then(|s| s.writes.iter().find(|(b, _)| b == block.as_str()).map(|(_, m)| *m));
            let near = !self.focus || {
                let named = (self.target.as_ref()).is_none_or(|t| t.near.iter().any(|n| n == block.as_str()));
                named || self.rng.chance(8)
            };
            let drawn = match &said {
                Some(_) => listed.is_some(),
                None => !only_must && near && (all || self.rng.chance(percent)),
            };
            if (must && !leaves_out) || (!must && drawn) {
                let passes = self.scene.passing.contains(&block.as_str())
                    && self.scene.passes.is_some_and(|percent| self.rng.chance(percent));
                let value = self.sim.flow.blocks[&block].values.clone().map(|v| match passes {
                    _ if self.force.contains_key(block.as_str()) => self.force[block.as_str()].clone(),
                    true if v.iter().any(|x| x == "yes") => "yes".to_string(),
                    _ => v[self.rng.below(v.len())].clone(),
                });
                let content = value.clone().unwrap_or_else(|| self.content(block.as_str()));
                let matters = match listed {
                    Some(matters) => matters || value.is_some(),
                    None => self.matters(&block, value.as_deref()),
                };
                writes.push(Handed {
                    block,
                    content,
                    matters,
                    value,
                });
            }
        }
        // A choice that says what a return holds nothing else but, with the value it is given, is handed in with just
        // that.
        for kept in writes.clone() {
            let Some(value) = &kept.value else {
                continue;
            };
            if let Some(only) = self.sim.flow.blocks[&kept.block].with_nothing_but.get(value).cloned() {
                writes.retain(|w| w.block == kept.block || only.contains(&w.block));
                for block in &only {
                    // What the return may hold beside it is held now and then, as the draw of its own blocks gives it.
                    let percent = self.prefer.get(block.as_str()).copied().unwrap_or(self.writes);
                    if !writes.iter().any(|w| w.block == *block) && self.rng.chance(percent) {
                        let content = self.content(block.as_str());
                        let matters = self.matters(block, None);
                        writes.push(Handed {
                            block: block.clone(),
                            content,
                            matters,
                            value: None,
                        });
                    }
                }
            }
        }
        // A block the cell may not write is handed in beside the rest in one return of twenty-five, which is sent back,
        // and in two of five of a return that was sent back, so that a run reaches the limit of its returns.
        let again = matches!(
            self.sim.work.given.get(&(round.run, round.number)),
            Some(Given::SentBack(_))
        );
        if wrong || (!scripted && !self.steady && self.rng.chance(if again { 40 } else { 4 })) {
            let others: Vec<BlockId> = (self.sim.flow.blocks.keys())
                .filter(|b| !self.sim.flow.flow.cells()[place].outputs.contains(b))
                .cloned()
                .collect();
            let block = others[self.rng.below(others.len())].clone();
            let value = self.sim.flow.blocks[&block].values.as_ref().map(|v| v[0].clone());
            let content = value.clone().unwrap_or_else(|| self.content(block.as_str()));
            writes.push(Handed {
                block,
                content,
                matters: true,
                value,
            });
        }
        // Where nothing happens by chance, a run the owner's words ran answers them, as a cell that is told something
        // does.
        if self.steady {
            let ran_by = (decide::told(&self.sim.flow.flow, &self.sim.work.records, round.run))
                .map(|told| told.ran_by)
                .unwrap_or_default();
            if let Some(words) = spoken::words_of(&self.sim.flow, &cell.id).cloned()
                && ran_by.contains(&words)
                && !writes.iter().any(|w| w.block == words)
            {
                let content = self.content(words.as_str());
                writes.push(Handed {
                    block: words,
                    content,
                    matters: true,
                    value: None,
                });
            }
        }
        let mut came = Came::hands_in(writes);
        if self.nothing_dealt.contains(cell.id.as_str()) && self.rng.chance(80) {
            came.dealt_with = Some(Vec::new());
        } else if self.partial > 0 && cell.line.is_some() && self.rng.chance(self.partial) {
            let ran_by = (decide::told(&self.sim.flow.flow, &self.sim.work.records, round.run))
                .map(|told| told.ran_by)
                .unwrap_or_default();
            came.dealt_with = Some(ran_by.into_iter().filter(|_| self.rng.chance(50)).collect());
        }
        let takes_answers = (cell.watches.iter()).any(|t| t.of_source(&self.sim.flow, flow::ANSWER).is_some());
        came.said.questions = match said.as_ref().is_some_and(|s| s.asks) {
            true => self.question(&cell, false),
            false if takes_answers && self.waits.contains(cell.id.as_str()) && self.rng.chance(85) => {
                self.question(&cell, true)
            }
            false => self.questions(&cell),
        };
        came
    }

    /// Whether a block a return hands in changes its meaning: a block that is no choice, written before, is handed in
    /// as changing nothing in meaning as often as the history says, and the datum the run is steered at, where the
    /// history says so, as often as not, a first version included, which says nothing and is new to no cell.
    fn matters(&mut self, block: &BlockId, value: Option<&str>) -> bool {
        if value.is_some() {
            return true;
        }
        let sought = self.target.as_ref().and_then(|t| t.datum.as_deref()) == Some(block.as_str());
        if self.unmatter_datum && sought {
            return !self.rng.chance(80);
        }
        if self.unmatters == 0 {
            return true;
        }
        let written = decide::versions(&self.sim.work.records)
            .get(block)
            .is_some_and(|versions| !versions.is_empty());
        !(written && self.rng.chance(self.unmatters))
    }

    /// A question of the cell's own in one return of twelve, where its cell takes the owner's answers: it waits for the
    /// answer in one of two.
    fn questions(&mut self, cell: &CellSpec) -> Vec<Question> {
        let compiled = &self.sim.flow;
        let answered = (cell.watches.iter()).any(|t| t.of_source(compiled, flow::ANSWER).is_some());
        if !self.scene.talks || !answered || !self.rng.chance(self.asks) {
            return Vec::new();
        }
        let waits = self.rng.chance(50);
        self.question(cell, waits)
    }

    /// A question of the cell's own, which it waits for the answer to or not.
    fn question(&mut self, cell: &CellSpec, waits: bool) -> Vec<Question> {
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

    /// The versions the run that finished worked from, by the cell's account: of a datum that runs the cell, the one
    /// its run began from where it dealt with it; of another, the latest before its last round began; and what it
    /// wrote.
    fn worked_from(&mut self, round: &Round, finished: &decide::Finished) {
        let records = &self.sim.work.records;
        let cell = self.sim.cell(round);
        let place = self.sim.flow.flow.place(&cell.id).expect("the cell's place");
        let runs_on = &self.sim.flow.flow.cells()[place].runs_on;
        let started = records.started.iter().find(|s| s.run == round.run).expect("the run");
        let began = (records.rounds.iter())
            .filter(|r| r.run == round.run)
            .map(|r| r.at)
            .max();
        let versions = decide::versions(records);
        for given in &cell.given {
            let block = &given.block;
            let told = if runs_on.contains(block) {
                let dealt = finished.dealt_with.as_ref().is_none_or(|d| d.contains(block));
                started.basis.get(block).copied().flatten().filter(|_| dealt)
            } else {
                let all = versions.get(block).map_or(&[][..], Vec::as_slice);
                began.and_then(|b| all.iter().map(|v| v.at).filter(|at| *at < b).max())
            };
            if let Some(told) = told {
                let worked = self.worked.entry((cell.id.clone(), block.clone())).or_insert(told);
                *worked = (*worked).max(told);
            }
        }
        for w in &finished.writes {
            let worked = self
                .worked
                .entry((cell.id.clone(), w.block.clone()))
                .or_insert(finished.at);
            *worked = (*worked).max(finished.at);
        }
    }

    /// How the session should stand with a datum the round's cell is given, by the test's account of it, what it should
    /// be filled with of it, and the version the cell last worked from.
    fn datum(&self, round: &Round, held: &Held, given: &Datum) -> (Stands, Vec<Fill>, Option<Seq>) {
        let (block, kind) = (&given.block, given.kind);
        let sim = &self.sim;
        let cell = sim.cell(round);
        let began = (sim.work.records.rounds.iter())
            .filter(|r| r.run == round.run)
            .nth(round.number - 1)
            .expect("the round began")
            .at;
        let records = sim.work.records.before(began);
        let versions = decide::versions(&records);
        let Some(all) = versions.get(block).filter(|v| !v.is_empty()) else {
            return (Stands::Unwritten, Vec::new(), None);
        };
        let latest = all.last().expect("a version").at;
        let last = self.worked.get(&(cell.id.clone(), block.clone())).copied();
        let own = |w: &Writer| match w {
            Writer::Run(r) => records.started.iter().any(|s| s.run == *r && s.cell == cell.id),
            _ => false,
        };
        let new = all
            .iter()
            .any(|v| Some(v.at) > last && v.meaning == Some(v.at) && !own(&v.by));
        let whole = |at: Seq| Fill::Whole {
            block: block.clone(),
            version: at,
        };
        let lacking: Vec<Seq> = all.iter().map(|v| v.at).filter(|at| !held.holds(block, *at)).collect();
        // What the cell writes and is given by nothing else is sent, its latest, only where the session lacks it.
        if given.output {
            return match held.holds(block, latest) {
                true => (Stands::Holds, Vec::new(), last),
                false => (Stands::Lacks, vec![whole(latest)], last),
            };
        }
        // The carrier says what changed in the tree, from the version the session read where the cell worked from none,
        // and a session that holds none of it is told so.
        let compared = block.as_str() == "tree";
        if kind == InputKind::All && !lacking.is_empty() {
            let stands = if held.holds(block, all[0].at) {
                Stands::HoldsLast
            } else {
                Stands::Lacks
            };
            return (stands, lacking.into_iter().map(whole).collect(), last);
        }
        if held.holds(block, latest) {
            let stands = match last {
                None => Stands::HoldsNew,
                Some(_) if new => Stands::HoldsLatest,
                Some(_) => Stands::Holds,
            };
            return (stands, Vec::new(), last);
        }
        let since = last.filter(|l| held.holds(block, *l) && *l < latest);
        let read = || {
            all.iter()
                .rev()
                .map(|v| v.at)
                .find(|at| *at < latest && held.holds(block, *at))
        };
        let since = if compared && since.is_none() { read() } else { since };
        let change = match (kind, since) {
            (InputKind::LastAndLatest, Some(since)) if compared => Some(since),
            (InputKind::LastAndLatest, Some(since)) => {
                let (a, b) = (sim.content(block.as_str(), since), sim.content(block.as_str(), latest));
                context::diff::diff(a, b).filter(|d| d.len() <= b.len()).map(|_| since)
            }
            _ => None,
        };
        match change {
            Some(since) => {
                let fill = Fill::Change {
                    block: block.clone(),
                    version: latest,
                    since,
                };
                (Stands::HoldsLast, vec![fill], last)
            }
            None if kind == InputKind::LastAndLatest && compared && lacking.len() == all.len() => {
                (Stands::Unread, vec![whole(latest)], last)
            }
            None if kind == InputKind::LastAndLatest => (Stands::Lacks, vec![whole(latest)], last),
            None if since.is_some() => (Stands::HoldsLast, vec![whole(latest)], last),
            None => (Stands::Lacks, vec![whole(latest)], last),
        }
    }

    /// What the round entered so should be filled with, by the test's account of its session.
    fn expected(&self, round: &Round, entered: &Entered, held: &Held) -> Vec<Fill> {
        let sim = &self.sim;
        let cell = sim.cell(round);
        let began = (sim.work.records.rounds.iter())
            .filter(|r| r.run == round.run)
            .nth(round.number - 1)
            .expect("the round began")
            .at;
        let records = sim.work.records.before(began);
        let versions = decide::versions(&records);
        let mut fills = Vec::new();
        for part in &sim.words.message.parts {
            match part.part {
                Part::Context => {
                    for given in given(sim, cell) {
                        fills.extend(self.datum(round, held, &given).1);
                    }
                }
                Part::Results => {
                    let calls: Vec<CallId> = (1..=round.number)
                        .filter_map(|k| match sim.work.given.get(&(round.run, k)) {
                            Some(Given::Results(calls)) => Some(calls.clone()),
                            _ => None,
                        })
                        .flatten()
                        .filter(|c| !held.results.contains(c))
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect();
                    if !calls.is_empty() {
                        fills.push(Fill::Results(calls));
                    }
                }
                Part::SentBack => {
                    for k in 1..=round.number {
                        let failed = matches!(sim.work.back.get(&(round.run, k)), Some(Back::Failed { .. }));
                        let sent_back = matches!(sim.work.given.get(&(round.run, k)), Some(Given::SentBack(_)));
                        if sent_back && (k == round.number || (!failed && !held.sent_back.contains(&(round.run, k)))) {
                            fills.push(Fill::SentBack { round: k });
                        }
                    }
                    // The version the round's own return would have written over, of a block the cell is not given.
                    if let Some(Given::SentBack(unmet)) = sim.work.given.get(&(round.run, round.number)) {
                        for u in unmet {
                            let Unmet::WrittenOver { block, version } = u else {
                                continue;
                            };
                            // Given by the cell's definition, or by the context already as the latest it writes.
                            let in_context = fills.contains(&Fill::Whole {
                                block: block.clone(),
                                version: *version,
                            });
                            let given = in_context || cell.given.iter().any(|g| &g.block == block);
                            let written = versions
                                .get(block)
                                .is_some_and(|vs| vs.iter().any(|v| v.at == *version));
                            if !given && !held.holds(block, *version) && written {
                                fills.push(Fill::Whole {
                                    block: block.clone(),
                                    version: *version,
                                });
                            }
                        }
                    }
                }
                _ if part.kind.is_some() => {
                    let texts = kept(&sim.flow, &sim.words, cell, Values::Run(&entered.run));
                    let text = texts.iter().find(|(name, _)| name == part.part.name());
                    if let Some((name, text)) = text {
                        let fill = Fill::part(name, text);
                        if !held.parts.contains(&fill) {
                            fills.push(fill);
                        }
                    }
                }
                _ => {}
            }
        }
        fills
    }

    /// Keeps the history as it stands where a round of the cell is given and not yet carried, for the search to go on
    /// from, where no state kept stood in the same situation: what a datum stands in at a round depends on what was
    /// written and what the session held before it, so that the states worth going on from are those that differ. Where
    /// the cell has kept enough, the new one takes the place of one by the situation's own number.
    fn keep(&self, cell: &str, situation: &Situation, reached: &mut Reached) {
        use std::hash::{DefaultHasher, Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        format!("{situation:?}").hash(&mut hasher);
        let number = hasher.finish();
        let kept = reached.archive.entry(cell.to_string()).or_default();
        if kept.iter().any(|(n, ..)| *n == number) {
            return;
        }
        let state = (number, situation.data.clone(), self.clone());
        match kept.len() < ARCHIVE {
            true => kept.push(state),
            false => {
                let at = (number % kept.len() as u64) as usize;
                kept[at] = state;
            }
        }
    }

    fn check(&self, round: &Round, entered: &Entered, held: &Held, reached: &mut Reached) {
        let sim = &self.sim;
        let cell = sim.cell(round);
        if !self.scene.models {
            for line in stood(cell, &entered.situation) {
                if reached.real.contains(&line) {
                    continue;
                }
                let because = (sim.work.records.started.iter())
                    .find(|s| s.run == round.run)
                    .map(|s| format!("{:?}", s.because));
                let example = format!("{because:?}, round {}, {:?}", round.number, entered.situation);
                reached.examples.insert(line.clone(), example);
                reached.real.insert(line);
            }
            reached.rounds += 1;
            *reached.of_cell.entry(cell.id.to_string()).or_default() += 1;
            return;
        }
        assert_eq!(
            entered.fills,
            self.expected(round, entered, held),
            "what is filled: {}",
            entered.text
        );
        let data = given(sim, cell);
        let given: Vec<&BlockId> = (data.iter().map(|g| &g.block))
            .chain(entered.run.written_over.iter().map(|(b, _)| b))
            .collect();
        // A version the round's return would have written over is given to it, with why it was sent back or, as the
        // latest the cell writes, in its context.
        if let Some(Given::SentBack(unmet)) = sim.work.given.get(&(round.run, round.number)) {
            for u in unmet {
                if let Unmet::WrittenOver { block, version } = u {
                    let whole = Fill::Whole {
                        block: block.clone(),
                        version: *version,
                    };
                    reached.written_over += usize::from(entered.fills.contains(&whole));
                }
            }
        }
        // How the session stands with each datum, and the line of one it holds that is new to the cell.
        for g in &data {
            let (stands, _, last) = self.datum(round, held, g);
            let name = g.block.as_str();
            assert_eq!(
                entered.situation.data.get(name),
                Some(&stands),
                "how the session stands with {name}"
            );
            let sent = entered.sent.iter().find(|s| s.block.as_deref() == Some(name));
            match stands {
                Stands::HoldsLatest => {
                    let sent = sent.expect("the line of a datum held that is new");
                    assert_eq!(sent.form, Form::Held);
                    let worked = last.expect("a version worked from");
                    let said = format!("last worked from {}", sim.number_at(name, worked));
                    assert!(
                        as_written(&sent.pieces).ends_with(&said),
                        "{}",
                        as_written(&sent.pieces)
                    );
                    reached.held += 1;
                    reached.lacks_last += usize::from(!held.holds(&g.block, worked));
                }
                Stands::HoldsNew => {
                    let sent = sent.expect("the line of a datum held that the cell never worked from");
                    assert_eq!(sent.form, Form::Held);
                    // Its latest, which the session holds, by its number among the datum's versions.
                    let latest = held.versions.get(&g.block).and_then(|v| v.iter().max()).copied();
                    let latest = latest.expect("the session holds the latest");
                    let version = format!("version {}, ", sim.number_at(name, latest));
                    assert!(
                        as_written(&sent.pieces).contains(&version),
                        "{}",
                        as_written(&sent.pieces)
                    );
                    reached.held_new += 1;
                }
                Stands::Holds | Stands::Unwritten => assert!(sent.is_none(), "{name} is sent"),
                Stands::Lacks | Stands::HoldsLast => {
                    assert!(sent.is_some(), "{name} is not sent");
                    reached.outputs += usize::from(g.output);
                }
                Stands::Unread => {
                    let sent = sent.expect("the tree the session has read none of");
                    let text = as_written(&sent.pieces);
                    assert!(text.contains("this session has read none of it"), "{text}");
                    reached.unread += 1;
                }
            }
        }
        assert_eq!(
            entered.situation.main.as_deref(),
            sim.main(round).as_ref().map(|m| m.key()),
            "main taken in"
        );
        reached.main += usize::from(entered.situation.main.is_some());
        let versions = decide::versions(&sim.work.records);
        for fill in &entered.fills {
            match fill {
                Fill::Whole { block, .. } | Fill::Change { block, .. } => {
                    assert!(given.contains(&block), "{block} is filled and not given to {}", cell.id);
                }
                Fill::Results(_) => reached.results += 1,
                Fill::SentBack { .. } => reached.sent_back += 1,
                Fill::Part { .. } => {}
            }
            if let Fill::Change { block, version, since } = fill {
                let diff = entered.run.data[block].diff.as_deref().expect("the change");
                if block.as_str() == "tree" {
                    reached.files += 1;
                    assert_eq!(
                        diff,
                        support::changed(*since, *version),
                        "what the carrier says changed"
                    );
                    continue;
                }
                reached.changes += 1;
                let rebuilt = context::diff::apply(sim.content(block.as_str(), *since), diff);
                assert_eq!(
                    rebuilt.as_deref(),
                    Some(sim.content(block.as_str(), *version)),
                    "{block}"
                );
            }
        }
        let all =
            (entered.fills.iter()).filter(|f| matches!(f, Fill::Whole { block, .. } if block.as_str() == "notes"));
        if all.count() > 1 {
            reached.versions += 1;
        }
        for sent in &entered.sent {
            if let Some(block) = &sent.block {
                let written = versions
                    .get(&BlockId::from(block.as_str()))
                    .is_some_and(|v| !v.is_empty());
                assert!(written, "{block} is sent with no version");
            }
        }
        // A session that holds nothing is filled with every datum written and the task.
        if held.versions.is_empty() && held.parts.is_empty() {
            for g in &data {
                let written = entered.run.data.contains_key(&g.block);
                let filled = (entered.fills.iter())
                    .any(|f| matches!(f, Fill::Whole { block, .. } | Fill::Change { block, .. } if *block == g.block));
                assert_eq!(written, filled, "{} from the top", g.block);
            }
            assert!(
                entered
                    .fills
                    .iter()
                    .any(|f| matches!(f, Fill::Part { part, .. } if part == "task"))
            );
        }
        // Each by the number of the version it is kept as among its block's versions.
        let taken: Vec<(BlockId, u64)> = (held.taken.iter())
            .map(|(block, at)| (block.clone(), sim.number_at(block.as_str(), *at)))
            .collect();
        assert_eq!(entered.run.taken, taken, "what the last return handed in");
        assert_eq!(entered.situation.after_hand_in, !held.taken.is_empty());
        for line in stood(cell, &entered.situation) {
            let example = format!("round {}, {:?}", round.number, entered.situation);
            reached.examples.entry(line.clone()).or_insert(example);
            reached.real.insert(line);
        }
        // What the page shows for the situation, its places open, is what the engine sends in it.
        let open = context::message(
            &sim.flow,
            &sim.words,
            cell,
            &entered.situation,
            Values::Slots(&sim.named),
        );
        assert_eq!(open.len(), entered.sent.len(), "{}", entered.text);
        for (open, sent) in open.iter().zip(&entered.sent) {
            assert_eq!(
                (&open.part, &open.block, open.form),
                (&sent.part, &sent.block, sent.form)
            );
            let text = as_written(&sent.pieces);
            assert!(fits(&open.pieces, &text), "{:?} does not fit {text:?}", open.pieces);
        }
    }
}

/// What a situation of the cell says, as lines each of one thing the tool writes a situation for, apart: how the
/// session stands with each datum, and whether the datum ran the cell; what the session holds as the entry begins; what
/// else is given. A real round and a written situation are held to the same lines, so that the state of a datum is
/// compared by the cell and whether it ran it, and a state of one datum is not asked of another. A situation does not
/// say its entry: two entries that stand alike are one.
fn stood(cell: &CellSpec, s: &Situation) -> Vec<String> {
    let id = cell.id.as_str();
    // A datum ran the cell where it is watched by itself and new; a member of a group stands by its own state alone,
    // the text of a datum being the builder's from its kind and its state, and the group that ran the cell is a line of
    // its own.
    let ran = |block: &str| (cell.watches.iter()).any(|t| !t.is_group() && t.name == block && s.new.contains(&t.name));
    let mut lines: Vec<String> = (s.data.iter())
        .map(|(block, stands)| {
            let ran = if ran(block) { "ran it" } else { "did not run it" };
            format!("{id}: {block} {} and {ran}", stands.name())
        })
        .collect();
    lines.extend(s.new.iter().map(|name| format!("{id}: run by {name}")));
    lines.push(format!(
        "{id}: the session holds {:?}, after a hand-in {}",
        s.holds, s.after_hand_in
    ));
    lines.extend(
        s.results
            .iter()
            .map(|key| format!("{id}: given the result of a call, {key}")),
    );
    lines.extend(s.sent_back.then(|| format!("{id}: given why a return was sent back")));
    lines.extend(
        s.main
            .iter()
            .map(|key| format!("{id}: given what came of taking main in, {key}")),
    );
    lines
}

/// What the tool writes for every agent's cell of the flow, as the lines of [`stood`], and how many of its situations
/// are of a session begun anew in which another cell of its line was read first. The project's guide is in the system
/// prompt of every session, and no situation holds it or lacks it as a datum.
fn written(sim: &Sim) -> (BTreeSet<String>, usize) {
    let mut lines = BTreeSet::new();
    let mut after_read = 0;
    for cell in sim.flow.cells.iter().filter(|c| c.kind == flow::Kind::Agent) {
        let all = context::situations::situations(&sim.flow, &sim.words, cell, &sim.compared);
        for named in all {
            assert!(
                !named.situation.data.contains_key("guide") && !named.situation.holds.iter().any(|h| h == "guide"),
                "{}: {} gives the guide",
                cell.id,
                named.name
            );
            after_read += usize::from(named.name.starts_with("reopened_after_read-"));
            lines.extend(stood(cell, &named.situation));
        }
    }
    (lines, after_read)
}

/// The lines in the first set and not in the second, cut to how many a failure says.
fn missing(a: &BTreeSet<String>, b: &BTreeSet<String>) -> String {
    let all: Vec<&String> = a.difference(b).collect();
    let stride = all.len().div_ceil(120).max(1);
    let shown: Vec<&str> = all.iter().step_by(stride).map(|s| s.as_str()).collect();
    format!("{} of them:\n{}", all.len(), shown.join("\n"))
}

/// A datum a round of the cell is given, by the test's account: its kind, and whether it is given only as what the
/// cell writes.
struct Datum {
    block: BlockId,
    kind: InputKind,
    output: bool,
}

/// What a round of the cell is given, by the test's account of the design (design document `data-and-context`, 14, 15,
/// 21): each datum its definition gives it, by where it gives it, in the order the `context` part names; and under
/// `outputs`, each block it writes that nothing else gives it, as `latest`.
fn given(sim: &Sim, cell: &CellSpec) -> Vec<Datum> {
    let context = sim.words.message.parts.iter().find(|p| p.part == Part::Context);
    let order = context.and_then(|p| p.order.clone()).expect("the context part's order");
    let place = sim.flow.flow.place(&cell.id).expect("the cell's place");
    let outputs = &sim.flow.flow.cells()[place].outputs;
    let mut data = Vec::new();
    for list in order {
        if list == Listed::Outputs {
            for block in outputs.iter().filter(|b| !cell.given.iter().any(|g| &g.block == *b)) {
                data.push(Datum {
                    block: block.clone(),
                    kind: InputKind::Latest,
                    output: true,
                });
            }
        }
        for g in cell.given.iter().filter(|g| g.list == list) {
            data.push(Datum {
                block: g.block.clone(),
                kind: g.kind,
                output: false,
            });
        }
    }
    data
}

/// Whether the text is the pieces with each place filled by some text: a place, and the places right after it, stand
/// for what comes before a place where the text after them is found.
fn fits(pieces: &[Piece], text: &str) -> bool {
    match pieces.split_first() {
        None => text.is_empty(),
        Some((Piece::Text { text: t }, rest)) => text.strip_prefix(t.as_str()).is_some_and(|after| fits(rest, after)),
        Some((Piece::Slot { .. }, rest)) => match rest.iter().position(|p| matches!(p, Piece::Text { .. })) {
            None => true,
            Some(next) => {
                let Piece::Text { text: t } = &rest[next] else {
                    return false;
                };
                text.match_indices(t.as_str())
                    .any(|(at, _)| fits(&rest[next..], &text[at..]))
            }
        },
    }
}

#[test]
fn every_round_is_filled_with_what_its_session_lacks_by_the_kinds_of_its_cell() {
    let mut reached = Reached::default();
    for seed in 0..SEEDS + SCRIPTED {
        let mut history = History::new(seed);
        let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if seed < SEEDS {
                for _ in 0..STEPS {
                    history.step(&mut reached);
                }
            } else {
                history.script(&mut reached);
            }
            if seed >= SEEDS {
                reached.trace = history.trace.clone();
            }
            reached.put_in += history.sim.work.records.put_in.len();
            reached.at_limit += (history.sim.work.calls.values())
                .filter(|c| matches!(c, Call::AtLimit(_)))
                .count();
        }));
        if let Err(e) = ran {
            eprintln!("the history of seed {seed} failed");
            std::panic::resume_unwind(e);
        }
    }
    eprintln!(
        "reached: {} changes, {} of the tree, {} rounds given several versions at once, {} sessions reopened, {} lost, {} \
         compacted, {} results, {} sent back",
        reached.changes,
        reached.files,
        reached.versions,
        reached.reopened,
        reached.lost,
        reached.compacted,
        reached.results,
        reached.sent_back
    );
    for (what, n) in [
        ("a change sent", reached.changes),
        ("the line of a datum held that is new", reached.held),
        (
            "the line of a datum held that the cell never worked from",
            reached.held_new,
        ),
        (
            "the latest held referred to where the session lacks the version last worked from",
            reached.lacks_last,
        ),
        ("a version put into a round", reached.put_in),
        ("a version written over sent back", reached.written_over),
        ("main taken in", reached.main),
        ("what the carrier says changed sent", reached.files),
        ("several versions sent at once", reached.versions),
        ("a session reopened", reached.reopened),
        ("a session lost", reached.lost),
        ("a session compacted", reached.compacted),
        ("results given", reached.results),
        ("a return sent back", reached.sent_back),
        ("a round that failed after its program had its message", reached.wrote),
        ("a new version given that said what one held said", reached.said_again),
        ("the tree sent to a session that held none of it", reached.unread),
        ("an output the session lacked sent", reached.outputs),
        ("a run at the limit of its returns", reached.at_limit),
    ] {
        assert!(n > 0, "no history reached {what}");
    }
    // What the tool writes and what the rounds reached are the same, line by line: a line of a round that the tool does
    // not write is a gap in its rules, and a line it writes that no round reached is one it writes for nothing.
    let (written, _) = written(&Sim::new());
    let (gaps, nothing) = (missing(&reached.real, &written), missing(&written, &reached.real));
    assert!(
        reached.real.is_subset(&written) && written.is_subset(&reached.real),
        "rounds stood in what the tool does not write, {gaps}\n\nthe tool writes what no round stood in, {nothing}\n\nof the close: {:?}\n\nthe last scripted history: {:?}",
        (reached.examples.iter())
            .filter(|(line, _)| line.starts_with("close: the session holds"))
            .collect::<Vec<_>>(),
        reached.trace.iter().rev().take(30).rev().collect::<Vec<_>>()
    );
}

/// How many runs the search makes at most, how many steps each takes after its quiet ones, how many of the first steps
/// of a run begun from nothing only carry the work's rounds (up to), and how many states of the work at the entry of a
/// round of a cell it keeps, each of a situation the round stood in that no other kept did.
const RUNS: usize = 300;

/// How many runs without a line reached end the search.
const PLATEAU: usize = 40;
const RUN_STEPS: usize = 30;
const QUIET_STEPS: usize = 150;
const ARCHIVE: usize = 50;

/// What a run of the search is steered at: a cell, the datum and the state of it a line not yet reached says, and the
/// blocks near them: those that run the cell, and the verdict that the owner's ruling on the datum is written in.
#[derive(Clone)]
struct Target {
    cell: String,
    datum: Option<String>,
    /// The state of the datum the line says.
    state: Option<String>,
    unwritten: bool,
    blocks: Vec<String>,
    /// The value of each choice the cell's own condition names, which the cell runs on.
    runs_on: Vec<(String, String)>,
    /// Whether the cell runs only until a run of it has dealt with something: its condition names a datum the engine's
    /// source writes once a run that dealt with something has finished.
    one_shot: bool,
    /// Whether the line says the datum ran the cell, and whether the owner can speak to the cell.
    ran_it: bool,
    wordy: bool,
    /// The blocks a return hands in, beside what its cell must, where the run is focused: the datum, what runs the
    /// writers of it and what the owner's ruling is written in, and each block the cell watches by itself, not as one
    /// of a group, which is what runs the cell without running the cells before it that watch the group.
    near: Vec<String>,
}

impl Target {
    /// What a line of [`stood`] not yet reached is of.
    fn of(sim: &Sim, line: &str) -> Target {
        let (cell, rest) = line.split_once(": ").unwrap_or(("", line));
        let datum = match rest.strip_prefix("run by ") {
            Some(name) => Some(name),
            None if rest.starts_with("the session holds") || rest.starts_with("given ") => None,
            None => rest.split(' ').next(),
        };
        let mut blocks: Vec<String> = datum.into_iter().map(str::to_string).collect();
        // What the owner says to a cell that writes the datum runs that cell, which writes it again.
        let writers: Vec<CellId> = (datum.and_then(|d| sim.flow.blocks.get(&BlockId::from(d))))
            .map(|declared| declared.writers.cells.clone())
            .unwrap_or_default();
        for writer in &writers {
            if let Some(words) = spoken::words_of(&sim.flow, writer) {
                blocks.push(words.as_str().to_string());
            }
        }
        // The owner rules on a dialogue by writing the verdict the flow pairs with it, which makes the judge's points
        // before it new to no cell.
        for judge in sim.flow.flow.cells() {
            for (verdict, of) in &judge.verdicts {
                if datum.is_some_and(|d| of.dialogue.as_str() == d) {
                    blocks.push(verdict.as_str().to_string());
                }
            }
        }
        if let Some(spec) = sim.flow.cells.iter().find(|c| c.id.as_str() == cell) {
            for watched in &spec.watches {
                blocks.extend(watched.blocks.iter().map(|b| b.as_str().to_string()));
            }
        }
        // What wakes the cells that write the datum and the blocks the cell watches by itself, besides what the datum
        // is and what the owner's words and rulings on it are written in: the blocks a chain of runs from a writer of
        // the datum to the cell passes, which the returns hand in where a run is focused. A block watched as one of a
        // group wakes every cell that watches the group, so it is left out unless it is the datum.
        let alone = |cell: &str| -> Vec<String> {
            let spec = sim.flow.cells.iter().find(|c| c.id.as_str() == cell);
            (spec.into_iter())
                .flat_map(|c| c.watches.iter().filter(|t| !t.is_group()))
                .flat_map(|t| t.blocks.iter().map(|b| b.as_str().to_string()))
                .collect()
        };
        let grouped: Vec<String> = (sim.flow.cells.iter().find(|c| c.id.as_str() == cell).into_iter())
            .flat_map(|c| c.watches.iter().filter(|t| t.is_group()))
            .flat_map(|t| t.blocks.iter().map(|b| b.as_str().to_string()))
            .collect();
        let mut near: Vec<String> = blocks.iter().filter(|b| !grouped.contains(b)).cloned().collect();
        near.extend(datum.map(str::to_string));
        near.extend(alone(cell));
        for block in near.clone() {
            let writers = (sim.flow.blocks.get(&BlockId::from(block.as_str()))).map(|d| d.writers.cells.clone());
            for writer in writers.unwrap_or_default() {
                near.extend(alone(writer.as_str()));
            }
        }
        near.sort();
        near.dedup();
        let state = datum.and_then(|_| rest.split(' ').nth(1)).map(str::to_string);
        let runs_on = (sim.flow.flow.cells().iter())
            .find(|c| c.id.as_str() == cell)
            .map(|c| {
                (c.applies.iter())
                    .filter_map(|key| match key {
                        Condition::Is { block, value } => Some((block.as_str().to_string(), value.clone())),
                        Condition::Test(_) => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let one_shot = (sim.flow.flow.cells().iter())
            .find(|c| c.id.as_str() == cell)
            .is_some_and(|c| {
                (c.applies.iter()).any(|key| match key {
                    Condition::Is { block, .. } => (sim.flow.blocks.get(block))
                        .is_some_and(|d| d.writers.outside.as_deref() == Some(flow::HANDED_IN)),
                    Condition::Test(_) => false,
                })
            });
        Target {
            cell: cell.to_string(),
            datum: datum.map(str::to_string),
            state,
            unwritten: rest.contains(" unwritten and "),
            blocks,
            runs_on,
            one_shot,
            ran_it: rest.ends_with("and ran it"),
            wordy: spoken::words_of(&sim.flow, &CellId::from(cell)).is_some(),
            near,
        }
    }

    /// How near a state of a history, as it stands in its situation, is to what the line says, nought where it is none:
    /// of a datum to be unwritten, it is unwritten; of one to be held, it is held in some way; of one to be lacking,
    /// it is unwritten or lacking.
    fn nearness(&self, data: &BTreeMap<String, Stands>) -> u32 {
        let stands = self.datum.as_ref().and_then(|datum| data.get(datum));
        match (self.state.as_deref(), stands) {
            (Some("unwritten"), Some(Stands::Unwritten)) => 8,
            (Some("unwritten"), _) => 0,
            (Some("lacks"), Some(Stands::Unwritten | Stands::Lacks)) => 4,
            (Some(state), Some(Stands::Holds | Stands::HoldsLast | Stands::HoldsLatest | Stands::HoldsNew))
                if state.starts_with("holds") =>
            {
                4
            }
            _ => 1,
        }
    }

    /// Whether the datum is one of the owner's talk with a cell, which the owner not talking leaves unwritten.
    fn talk(&self) -> bool {
        (self.datum.as_ref()).is_some_and(|d| d.ends_with("_questions") || d.ends_with("_words"))
    }
}

/// A directed test of the engineer's flow: from the work as it stands after the first pass, it says what happens.
type Directed = fn(&mut Reached, History) -> Result<(), String>;

/// The blocks the implementation hands in as changing nothing in meaning, so that no review that watches the change is
/// run.
const UNCHANGED: [&str; 5] = ["code", "title", "decisions", "evidence", "acceptance"];

/// The owner and the outside write, the owner speaks to the cells that wrote the plan and the change and answers what
/// they asked, and the plan is written anew, which the implementation then answers, in what it hands in with `changed`
/// as changing its meaning and the rest of the change as changing nothing in it. `head`, a block of the change, is not
/// written: it runs every review.
fn after_a_pass(history: &mut History, changed: &[&'static str]) {
    for block in ["background", "tree", "test_times"] {
        history.put(block);
    }
    history.say_to("R1");
    history.say_to("I1");
    history.answers_all();
    // Each answers the owner's words, which a return that deals with them must.
    history.says.insert(
        "R1".to_string(),
        vec![Say::all(&["plan_owner", "plan_agent", "r1_words"])],
    );
    let mut answered = changed.to_vec();
    answered.push("i1_words");
    history
        .says
        .insert("I1".to_string(), vec![Say::with(&answered, &UNCHANGED)]);
}

/// The implementation answers one review alone, so that the review's own dialogue runs it with its session lacking what
/// the cells before it wrote since; and the outside writes the head while its round is under way.
fn one_review_answered(reached: &mut Reached, passed: History) -> Result<(), String> {
    for (cell, dialogue) in [("T1", "review"), ("T2", "test_review"), ("T3", "design_review")] {
        let mut history = passed.clone();
        after_a_pass(&mut history, &[dialogue]);
        history.until(reached, cell)?;
        history.put("head");
        history.carry_next(reached, cell)?;
    }
    Ok(())
}

/// The implementation answers all three at once: the first fills the session with what the others then stand with as
/// the latest.
fn all_three_answered(reached: &mut Reached, mut history: History) -> Result<(), String> {
    after_a_pass(&mut history, &["review", "test_review", "design_review"]);
    for cell in ["T1", "T2", "T3"] {
        history.carry_next(reached, cell)?;
    }
    Ok(())
}

/// The owner rules on the plan before it is written, and the implementation hands in the code and nothing else: the
/// reviews are run with the plan and the rest of the change unwritten.
fn no_plan_written(reached: &mut Reached, _: History) -> Result<(), String> {
    let mut history = History::directed(1);
    history.force.insert("deep_needed".to_string(), "no".to_string());
    history
        .says
        .insert("I1".to_string(), vec![Say::all(&["code", "i1_words"])]);
    for block in ["plan_ok", "tree_ok"] {
        history.put_value(block, "yes");
    }
    history.say_to("I1");
    for cell in ["T1", "T2", "T3"] {
        history.carry_next(reached, cell)?;
    }
    Ok(())
}

/// The line of the reviews begins a new session after the tester wrote the tests, and the implementation answers one
/// review.
fn reopened_after_the_tests(reached: &mut Reached, mut history: History) -> Result<(), String> {
    history.reopen("code_reviewer", reached);
    after_a_pass(&mut history, &["design_review"]);
    history.carry_next(reached, "T3")
}

/// The tests: the tester writes them as changing nothing in meaning and raises a point; the implementation answers it,
/// which the tester's own dialogue runs it with; then each of the reviews is run by the implementation's answer to its
/// own dialogue alone, with the session lacking the tests the tester wrote since. The test review's choice of whether
/// the change needs the tester goes to no and back to yes, so that the tester is run by a choice whose versions between
/// its last and its latest it was never told.
fn the_tests_written_between(reached: &mut Reached, mut history: History) -> Result<(), String> {
    // The reviews are run again, and the tester is not run: the change needs no tester.
    history.force.insert("needs_testing".to_string(), "no".to_string());
    history.put("head");
    for cell in ["T1", "T2", "T3"] {
        history.carry_next(reached, cell)?;
    }
    // And again, with the tester needed: it is run by a version of the choice that follows one it was never told.
    history.force.insert("needs_testing".to_string(), "yes".to_string());
    history
        .says
        .insert("T4".to_string(), vec![Say::with(&["testing"], &["tests"])]);
    history.put("head");
    for cell in ["T1", "T2", "T3", "T4"] {
        history.carry_next(reached, cell)?;
    }
    // Each review in turn is run by the implementation's answer alone after the tester wrote tests the review's session
    // lacks: the implementation answers the tester's point and the review, the review is run, and the tester is run
    // again.
    for (cell, dialogue) in [("T1", "review"), ("T2", "test_review"), ("T3", "design_review")] {
        history.say_to("I1");
        history.says.insert(
            "I1".to_string(),
            vec![Say::with(
                &["acceptance", "testing", "i1_words"],
                &["code", "title", "decisions", "evidence"],
            )],
        );
        history
            .says
            .insert("T4".to_string(), vec![Say::with(&["testing"], &["tests"])]);
        history.carry_next(reached, "T4")?;
        history.says.insert(
            "I1".to_string(),
            vec![Say::with(
                &["testing", dialogue],
                &["code", "title", "decisions", "evidence", "acceptance"],
            )],
        );
        history.carry_next(reached, cell)?;
    }
    Ok(())
}

/// The check before the merge fails, the implementation answers, and the check fails again, and then once more as
/// changing nothing in meaning while the owner's words run the implementation: its session holds the check's earlier
/// version and lacks the later, as the implementation's own, and not.
fn the_check_failing_twice(reached: &mut Reached, mut history: History) -> Result<(), String> {
    history.force.insert("checked".to_string(), "no".to_string());
    // What was asked is answered, so that the check may run, and the cells it runs are carried.
    history.answers_all();
    history.drain(reached);
    let answered = || vec![Say::with(&["i1_words"], &["code"])];
    for _ in 0..2 {
        history
            .says
            .insert("M1".to_string(), vec![Say::all(&["check", "checked"])]);
        history.says.insert("I1".to_string(), vec![Say::with(&[], &["code"])]);
        history.put("head");
        history.carry_next(reached, "I1")?;
    }
    history
        .says
        .insert("M1".to_string(), vec![Say::with(&["checked"], &["check"])]);
    history.put("head");
    history.say_to("I1");
    history.says.insert("I1".to_string(), answered());
    history.carry_next(reached, "I1")
}

/// The line of the implementation begins a new session in which a cell that takes main in was read and the other is
/// then run, by the owner's words or the background, the first cell's return having handed in or having asked for a
/// tool.
fn main_taken_in(reached: &mut Reached, passed: History) -> Result<(), String> {
    let tool = || {
        Forced::Ask(Ask::Tool {
            name: "query".to_string(),
            input: "x".to_string(),
        })
    };
    for asks in [false, true] {
        // The implementation, then the first cell of its line.
        let mut history = passed.clone();
        history.says.insert("I1".to_string(), vec![Say::all(&["i1_words"]); 8]);
        history.until_with_main(reached, "I1", |h| {
            h.sim.anew("engineer", Why::Owner);
            h.say_to("I1");
        })?;
        if asks {
            history.forced = Some(vec![tool()]);
        }
        let round = history.sim.round().ok_or("the round of I1 is gone")?;
        history.carry(&round, reached);
        history.put("background");
        history.carry_next(reached, "R0")?;
        // The first cell, then the implementation.
        let mut history = passed.clone();
        history.says.insert("I1".to_string(), vec![Say::all(&["i1_words"]); 8]);
        history.until_with_main(reached, "R0", |h| {
            h.sim.anew("engineer", Why::Owner);
            h.put("background");
        })?;
        if asks {
            history.forced = Some(vec![tool()]);
        }
        let round = history.sim.round().ok_or("the round of R0 is gone")?;
        history.carry(&round, reached);
        history.say_to("I1");
        history.carry_next(reached, "I1")?;
    }
    Ok(())
}

/// The last round of the line of the reviews ends with its session compacted, and the first of the reviews is run
/// again: its session holds what that return handed in and none of the message. The same of the tester, run again by
/// the implementation's answer to its point.
fn compacted_after_a_return(reached: &mut Reached, passed: History) -> Result<(), String> {
    let mut history = passed.clone();
    history.put("head");
    history.until(reached, "T2")?;
    history.compact_next = true;
    history.carry_next(reached, "T2")?;
    history.put("head");
    history.carry_next(reached, "T1")?;
    let mut history = passed;
    // The choice of whether the tester is needed goes to no and back to yes, so that the tester is run by a new
    // version.
    history.force.insert("needs_testing".to_string(), "no".to_string());
    history.put("head");
    for cell in ["T1", "T2", "T3"] {
        history.carry_next(reached, cell)?;
    }
    history.force.insert("needs_testing".to_string(), "yes".to_string());
    history
        .says
        .insert("T4".to_string(), vec![Say::with(&["testing"], &["tests"]); 2]);
    history
        .says
        .insert("I1".to_string(), vec![Say::with(&["testing", "i1_words"], &UNCHANGED)]);
    history.put("head");
    for cell in ["T1", "T2", "T3"] {
        history.carry_next(reached, cell)?;
    }
    history.until(reached, "T4")?;
    history.compact_next = true;
    history.carry_next(reached, "T4")?;
    history.say_to("I1");
    history.carry_next(reached, "I1")?;
    history.carry_next(reached, "T4")
}

/// The work as it stands once the plan is written, the change is made, every review has been run, the tester has raised
/// a point and the implementation has answered it, and no other point has been raised; with the design cells run too
/// where `deep` says the plan needs them.
fn first_pass(reached: &mut Reached, deep: bool) -> History {
    let mut passed = History::directed(u64::from(deep));
    let needed = if deep { "yes" } else { "no" };
    passed.force.insert("deep_needed".to_string(), needed.to_string());
    // A return that says the plan needs deep research holds nothing else but a question, so the plan is the design
    // cell's.
    let plan = ["plan_owner", "plan_agent", "acceptance"];
    if deep {
        passed
            .says
            .insert("R1".to_string(), vec![Say::all(&["deep_needed"]).asking()]);
        passed.says.insert("D1".to_string(), vec![Say::all(&plan).asking()]);
    } else {
        passed.says.insert("R1".to_string(), vec![Say::all(&plan).asking()]);
    }
    passed.says.insert(
        "I1".to_string(),
        vec![
            Say::all(&["code", "title", "decisions", "evidence"]).asking(),
            Say::all(&["testing"]),
        ],
    );
    passed
        .says
        .insert("T4".to_string(), vec![Say::with(&["testing"], &["tests"])]);
    passed.drain(reached);
    passed
}

/// The directed tests of the engineer's flow, each a history that is brought to a round by what its cells hand in and
/// what the owner and the outside write between two rounds of one line's session, which the random steps reach only by
/// chance. Each begins from the work as it stands after [`first_pass`]. Says, of each, how many lines it reached that
/// no other reached before it.
fn directed_tests(reached: &mut Reached, passed: &History, deep: &History) -> Vec<String> {
    let scripts: [(&str, Directed, &History); 9] = [
        ("one review answered", one_review_answered, passed),
        ("all three answered", all_three_answered, passed),
        ("no plan written", no_plan_written, passed),
        ("reopened after the tests", reopened_after_the_tests, passed),
        ("the tests written between", the_tests_written_between, passed),
        ("the check failing twice", the_check_failing_twice, passed),
        ("main taken in", main_taken_in, passed),
        ("main taken in, the plan needing deep research", main_taken_in, deep),
        ("compacted after a return", compacted_after_a_return, passed),
    ];
    let mut log = Vec::new();
    for (name, script, base) in scripts {
        let (lines, rounds) = (reached.real.len(), reached.rounds);
        let result = script(reached, base.clone());
        log.push(format!(
            "directed test {name}: {} lines, {} rounds{}",
            reached.real.len() - lines,
            reached.rounds - rounds,
            if result.is_err() { ", stale" } else { "" }
        ));
        if let Err(why) = result {
            reached.stale.push(format!("{name}: {why}"));
        }
    }
    log
}

/// What the owner and the outside do between two rounds of one line's session, to a datum.
#[derive(Clone)]
enum Between {
    /// A person or the outside writes the block.
    Put(String),
    /// The owner says some words to the cell, which are the datum.
    Words(String),
    /// The cell, run by the owner's words, hands in the block.
    Writes(String, String),
    /// The cell, run by the owner's words, asks a question, and the owner answers it: the datum is its questions.
    Asks(String),
}

impl Between {
    /// What is done between two rounds, as it needs the work: says whether the work was brought to it.
    fn done(&self, history: &mut History, reached: &mut Reached, depth: u8) -> bool {
        match self {
            Between::Put(block) => history.put(block),
            Between::Words(cell) => return history.say_to(cell),
            Between::Writes(cell, block) => {
                history.says.insert(cell.clone(), vec![Say::all(&[block.as_str()])]);
                history.wake(reached, cell, depth);
                if !history.until_within(reached, cell, LADDER_STEPS) {
                    return false;
                }
                history.carry_round(reached);
            }
            Between::Asks(cell) => {
                history.says.insert(cell.clone(), vec![Say::all(&[]).asking()]);
                history.wake(reached, cell, depth);
                if !history.until_within(reached, cell, LADDER_STEPS) {
                    return false;
                }
                history.carry_round(reached);
                history.answers_all();
            }
        }
        true
    }

    /// Whether it needs a round of the cell it writes through, which the work cannot then wait at.
    fn needs_a_round(&self) -> bool {
        matches!(self, Between::Writes(..) | Between::Asks(_))
    }
}

/// How many steps a directed attempt waits for a round of a cell, how many attempts are made at a line, and how many
/// rounds the attempts at the lines still open may carry together.
const LADDER_STEPS: usize = 30;
const ATTEMPTS: usize = 150;
const LADDER_ROUNDS: usize = 3000;

/// One attempt of [`between_rounds`].
struct Attempt {
    /// The base the work begins from, by its place among them.
    base: usize,
    /// Whether the target's round asks for a tool, so that a round of its run goes on.
    again: bool,
    /// Where the datum is written: before the first round, between the two, or after the target's own run began.
    position: u8,
    /// Whether the line's session is begun anew first.
    reopen: bool,
    /// The cell of the line that runs first, by its place among the line's.
    first: usize,
    /// How the datum is written, by its place among the ways.
    way: usize,
    /// Whether the target is run by the owner's words or what it watches, or by the datum alone.
    wake: bool,
}

/// Tries to bring the work to a round that stands in the state of a datum this line says, by what the owner and the
/// outside do to the datum between two rounds of one line's session: from one of the bases, the line's session is
/// reopened or not, a cell of the line runs, and the datum is written before it, after it, or after the target's own
/// run began, as the owner's words, a person's or the outside's hand, or the cell that writes it; and the target's
/// round asks for a tool, or does not, so that a round of its run goes on in what the first was sent. A cell the owner
/// cannot speak to is run by writing what it watches ([`History::wake`]). The attempts are taken in an order that does
/// not depend on where the work stands, as many as [`ATTEMPTS`] and as the budget of rounds lets. Every attempt carries
/// its rounds through the history's own checks, so that all the lines they stand in are reached.
fn between_rounds(bases: &[&History], line: &str, reached: &mut Reached, budget: usize) {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let Some(first_base) = bases.first() else {
        return;
    };
    let target = Target::of(&first_base.sim, line);
    let Some(datum) = target.datum.clone() else {
        return;
    };
    let flow = &first_base.sim.flow;
    let Some(own) = (flow.cells.iter().find(|c| c.id.as_str() == target.cell)).and_then(|c| c.line.clone()) else {
        return;
    };
    let mut firsts = vec![target.cell.clone()];
    firsts.extend(
        (flow.cells.iter())
            .filter(|c| c.line.as_ref() == Some(&own) && c.id.as_str() != target.cell)
            .map(|c| c.id.as_str().to_string()),
    );
    let mut ways: Vec<Between> = Vec::new();
    if first_base.scene.writes.iter().any(|w| *w == datum) {
        ways.push(Between::Put(datum.clone()));
    }
    for cell in &flow.cells {
        if spoken::words_of(flow, &cell.id).is_some_and(|w| w.as_str() == datum) {
            ways.push(Between::Words(cell.id.as_str().to_string()));
        }
        if format!("{}_questions", cell.id.as_str().to_lowercase()) == datum {
            ways.push(Between::Asks(cell.id.as_str().to_string()));
        }
    }
    if let Some(declared) = flow.blocks.get(&BlockId::from(datum.as_str())) {
        for writer in &declared.writers.cells {
            ways.push(Between::Writes(writer.as_str().to_string(), datum.clone()));
        }
    }
    let watched = (flow.cells.iter().find(|c| c.id.as_str() == target.cell))
        .is_some_and(|c| c.watches.iter().any(|t| t.blocks.iter().any(|b| b.as_str() == datum)));
    let mut attempts = Vec::new();
    for base in 0..bases.len() {
        for again in [false, true] {
            for position in 0..3u8 {
                for reopen in [false, true] {
                    for first in 0..firsts.len() {
                        for (way, how) in ways.iter().enumerate() {
                            // A datum written through a cell's round cannot be written while the target's round is
                            // awaited.
                            if position == 2 && how.needs_a_round() {
                                continue;
                            }
                            for wake in [true, false] {
                                // A datum the cell does not watch runs it by nothing: something else wakes it.
                                if !wake && (!watched || position == 2) {
                                    continue;
                                }
                                attempts.push(Attempt {
                                    base,
                                    again,
                                    position,
                                    reopen,
                                    first,
                                    way,
                                    wake,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    let mut hasher = DefaultHasher::new();
    line.hash(&mut hasher);
    let mut rng = Rng::new(hasher.finish());
    for i in (1..attempts.len()).rev() {
        attempts.swap(i, rng.below(i + 1));
    }
    let started = reached.rounds;
    for attempt in attempts.into_iter().take(ATTEMPTS) {
        if reached.rounds - started > budget {
            return;
        }
        let mut history = bases[attempt.base].clone();
        if attempt.reopen {
            history.sim.anew(&own, Why::Owner);
        }
        let (first, way) = (&firsts[attempt.first], &ways[attempt.way]);
        let brought = (|| {
            if attempt.position == 0 && !way.done(&mut history, reached, 0) {
                return false;
            }
            history.wake(reached, first, 0);
            if !history.until_within(reached, first, LADDER_STEPS) {
                return false;
            }
            history.carry_round(reached);
            if attempt.position == 1 && !way.done(&mut history, reached, 0) {
                return false;
            }
            if attempt.wake {
                history.wake(reached, &target.cell, 0);
            }
            if attempt.position == 2 && !way.done(&mut history, reached, 0) {
                return false;
            }
            if !history.until_within(reached, &target.cell, LADDER_STEPS) {
                return false;
            }
            if attempt.again {
                history.forced = Some(vec![Forced::Ask(Ask::Tool {
                    name: "query".to_string(),
                    input: "x".to_string(),
                })]);
                history.carry_round(reached);
                history.forced = None;
                return history.until_within(reached, &target.cell, LADDER_STEPS);
            }
            true
        })();
        if brought {
            history.carry_round(reached);
        }
        if reached.real.contains(line) {
            return;
        }
    }
}

/// Searches the engineer's flow for rounds that stand in the lines wanted. Each run is steered at a line not yet
/// reached, at random: it begins from a state of the work kept at the entry of a round of the line's cell, or of the
/// nearest cell before it that has one, or from nothing, where none is kept and now and then; sets how the histories'
/// random steps are weighed, as a mode of its own (how often a return hands in what it need not, how often the owner
/// speaks, asks and writes, how often a choice passes), and which of the blocks near the target the returns hand in or
/// leave out; changes what the work holds a few times, as the owner and the outside do; and goes on with the driver's
/// own steps. The rounds it carries keep the states they were entered in, one for each situation ([`History::keep`]).
/// It goes on until every line wanted was reached or the runs are spent. Says what the rounds reached, and, every tenth
/// run, how many lines were still open.
fn search(wanted: &BTreeSet<String>, seed: u64) -> (Reached, Vec<String>) {
    let mut rng = Rng::new(seed);
    let mut reached = Reached::default();
    let mut log = Vec::new();
    let begun = std::time::Instant::now();
    let (passed, deep) = (first_pass(&mut reached, false), first_pass(&mut reached, true));
    log.push(format!(
        "after the first passes, {:?}: {} lines open, {} rounds {:?}",
        begun.elapsed(),
        wanted.difference(&reached.real).count(),
        reached.rounds,
        reached.of_cell
    ));
    log.extend(directed_tests(&mut reached, &passed, &deep));
    log.push(format!(
        "after the directed tests, {:?}: {} lines open",
        begun.elapsed(),
        wanted.difference(&reached.real).count()
    ));
    let flow_of = support::engineer_sim();
    let order: Vec<String> = (flow_of.flow.flow.cells().iter())
        .map(|c| c.id.as_str().to_string())
        .collect();
    let run = |history: &mut History, steps: usize, reached: &mut Reached| {
        let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for _ in 0..steps {
                if history.ended() {
                    break;
                }
                history.step(reached);
            }
        }));
        if let Err(e) = ran {
            eprintln!("a history of the search of seed {seed} failed");
            std::panic::resume_unwind(e);
        }
    };
    let mut tried: BTreeMap<&String, usize> = BTreeMap::new();
    // The fewest lines open so far, and the run at which that was first so.
    let mut best = (usize::MAX, 0);
    for n in 0..RUNS {
        let open: Vec<&String> = wanted.iter().filter(|line| !reached.real.contains(*line)).collect();
        if open.is_empty() {
            break;
        }
        if open.len() < best.0 {
            best = (open.len(), n);
        }
        // No line was reached in the last runs: what is open is not found by more of them.
        if n - best.1 >= PLATEAU {
            break;
        }
        // A line the fewest runs were steered at is the next one's, so that none is left to chance.
        let fewest = open.iter().map(|line| tried.get(*line).copied().unwrap_or(0)).min();
        let next: Vec<&&String> = (open.iter())
            .filter(|line| Some(tried.get(**line).copied().unwrap_or(0)) == fewest)
            .collect();
        let line = *next[rng.below(next.len())];
        *tried.entry(line).or_default() += 1;
        let target = Target::of(&flow_of, line);
        // Where the run begins: a state kept at a round of the target's cell, or of the nearest cell before it that has
        // one, chosen by how near its situation is to the line, which the run then carries on to the cell; from
        // nothing, where none is kept and now and then.
        let at = order.iter().position(|c| *c == target.cell).unwrap_or(0);
        let near = |cell: &str, reached: &Reached| -> Vec<(usize, u32)> {
            (reached.archive.get(cell).into_iter().flatten().enumerate())
                .map(|(i, (_, data, _))| (i, target.nearness(data)))
                .filter(|(_, n)| *n > 0)
                .collect()
        };
        let first = if rng.chance(90) { at } else { at.saturating_sub(1) };
        let source = (0..=first)
            .rev()
            .map(|i| order[i].as_str())
            .map(|cell| (cell, near(cell, &reached)))
            .find(|(_, states)| !states.is_empty());
        let from = source.filter(|_| rng.chance(92)).map(|(cell, states)| {
            let total: u32 = states.iter().map(|(_, n)| n).sum();
            let mut pick = rng.below(total as usize) as u32;
            let mut chosen = states[0].0;
            for (i, n) in &states {
                if pick < *n {
                    chosen = *i;
                    break;
                }
                pick -= n;
            }
            (reached.archive[cell][chosen].2.clone(), cell == target.cell)
        });
        // Now and then it begins from a state kept at a round of a cell after the target's instead, and has the owner
        // write what runs the cell again: a session holds what the cells after it read, which only a cell run again can
        // be sent as the session stands.
        let later: Option<History> = (at + 1 < order.len() && rng.chance(20))
            .then(|| {
                let cells: Vec<&String> = (order[at + 1..].iter())
                    .filter(|cell| reached.archive.get(*cell).is_some_and(|kept| !kept.is_empty()))
                    .collect();
                (!cells.is_empty()).then(|| {
                    let kept = &reached.archive[cells[rng.below(cells.len())]];
                    kept[rng.below(kept.len())].2.clone()
                })
            })
            .flatten();
        let from_later = later.is_some();
        let mut history = match (later, from) {
            (Some(mut history), _) => {
                history.quiet = 0;
                history
            }
            (None, Some((mut history, at_target))) => {
                history.quiet = if at_target { 0 } else { rng.below(60) };
                history
            }
            (None, None) => {
                let sim = support::engineer_sim();
                let scene = Scene::engineer(&sim);
                let mut history = History::of(sim, scene, seed.wrapping_mul(7_919).wrapping_add(n as u64));
                history.quiet = 10 + rng.below(QUIET_STEPS);
                history
            }
        };
        history.rng = Rng::new(seed.wrapping_mul(104_729).wrapping_add(n as u64));
        // The mode of the run.
        history.writes = [25, 70, 100][rng.below(3)];
        history.asks = [8, 40, 90][rng.below(3)];
        history.unmatters = [0, 20, 50][rng.below(3)];
        history.partial = [0, 25, 60][rng.below(3)];
        history.calls = [15, 40, 70][rng.below(3)];
        history.forward = if rng.chance(60) { rng.below(60) } else { 0 };
        history.focus = rng.chance(25);
        history.unmatter_datum = rng.chance(50);
        history.nothing_dealt.clear();
        history.waits.clear();
        if target.one_shot && rng.chance(85) {
            history.nothing_dealt.insert(target.cell.clone());
        }
        // A version of a datum that the cell never dealt with stands held in its session, new to the cell, where its
        // run deals with nothing; and, where the owner can turn to the cell while a question it waits for stands
        // unanswered, the datum need not run it.
        let held_line = target.state.as_deref().is_some_and(|s| s.starts_with("holds"));
        if held_line && rng.chance(50) {
            history.nothing_dealt.insert(target.cell.clone());
        }
        if held_line && target.wordy && !target.ran_it && rng.chance(70) {
            history.nothing_dealt.insert(target.cell.clone());
            history.waits.insert(target.cell.clone());
        }
        history.scene.passes = Some([60, 85, 100][rng.below(3)]);
        history.scene.puts = [4, 8, 16][rng.below(3)];
        history.scene.reopens = [2, 4, 10][rng.below(3)];
        history.scene.speaks = [6, 20, 40][rng.below(3)];
        if !history.waits.is_empty() {
            history.scene.speaks = 40;
            history.asks = 90;
        }
        history.scene.nudges = [0, 5, 12][rng.below(3)];
        history.target = Some(target.clone());
        history.prefer.clear();
        history.force.clear();
        // The cell runs where the choices its condition names have those values.
        if rng.chance(85) {
            history.force.extend(target.runs_on.iter().cloned());
        }
        // A state of a datum unwritten is brought about by no return handing it in, and of the owner's talk with a
        // cell, by the owner not talking nor a cell asking.
        let quiet_owner = target.unwritten && target.talk() && rng.chance(85);
        if let Some(datum) = target.datum.as_ref().filter(|_| target.unwritten && rng.chance(85)) {
            history.prefer.insert(datum.clone(), 0);
        }
        if quiet_owner {
            history.asks = 0;
            history.scene.speaks = 0;
        }
        if from_later {
            let mut mine: Vec<&'static str> = (history.scene.writes.iter().copied())
                .filter(|w| target.blocks.iter().any(|b| b == w) || *w == "background")
                .collect();
            mine.dedup();
            let block = mine[rng.below(mine.len())];
            history.put(block);
        }
        // What the work holds at the round changes some, before it is carried.
        for _ in 0..1 + rng.below(3) {
            if !quiet_owner {
                history.perturb(&mut reached, &target);
            }
        }
        let steps = history.quiet + RUN_STEPS;
        run(&mut history, steps, &mut reached);
        if n % 10 == 9 {
            log.push(format!(
                "after {} runs, {:?}: {} lines open, {} rounds {:?}, {} states kept",
                n + 1,
                begun.elapsed(),
                wanted.difference(&reached.real).count(),
                reached.rounds,
                reached.of_cell,
                reached.archive.values().map(Vec::len).sum::<usize>()
            ));
        }
    }
    // What is open is tried by what the owner and the outside do between two rounds of one session.
    let open: Vec<&String> = wanted.iter().filter(|line| !reached.real.contains(*line)).collect();
    if !open.is_empty() {
        let each = (LADDER_ROUNDS / open.len()).clamp(100, 500);
        let started = reached.rounds;
        for line in &open {
            if reached.real.contains(*line) {
                continue;
            }
            // From the work after the first passes, and from states the search kept at the entry of a round of the
            // target's cell or of one of the two cells before it, with nothing left to chance.
            let at = (line.split_once(": ")).and_then(|(cell, _)| order.iter().position(|c| c == cell));
            let mut kept: Vec<History> = Vec::new();
            for cell in order[..=at.unwrap_or(0)].iter().rev().take(3) {
                let states = reached.archive.get(cell).map(Vec::as_slice).unwrap_or_default();
                for i in [0, states.len() / 2] {
                    if let Some(state) = states.get(i) {
                        let mut history = state.2.clone();
                        history.steady_mode();
                        kept.push(history);
                    }
                }
            }
            let bases: Vec<&History> = [&passed, &deep].into_iter().chain(kept.iter()).collect();
            let left = LADDER_ROUNDS.saturating_sub(reached.rounds - started);
            between_rounds(&bases, line, &mut reached, each.min(left));
        }
        log.push(format!(
            "the attempts between two rounds of a session, {:?}: {} of {} open lines reached, {} rounds",
            begun.elapsed(),
            open.iter().filter(|line| reached.real.contains(**line)).count(),
            open.len(),
            reached.rounds - started
        ));
    }
    (reached, log)
}

/// The lines, a cell on each, with what each says after the cell's name.
fn by_cell(lines: &BTreeSet<String>) -> String {
    let mut cells: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for line in lines {
        let (cell, rest) = line.split_once(": ").unwrap_or(("", line.as_str()));
        cells.entry(cell).or_default().push(rest);
    }
    (cells.iter())
        .map(|(cell, rest)| format!("{cell} ({}): {}", rest.len(), rest.join("; ")))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Searches the engineer's flow for rounds of these cells that stand in each state of a datum the tool writes for them,
/// and holds the tool to what the rounds the search carried stood in, in both directions, as the test over the small
/// flow does: every state of a datum that a round stood in is one the tool writes, and every one it writes for these
/// cells was stood in by a round.
fn every_state_written_for(cells: &[&str], seed: u64) {
    let (written, after_read) = written(&support::engineer_sim());
    assert!(
        after_read > 0,
        "no situation of a session begun anew and another cell read in it"
    );
    let of_these = |line: &String| cells.iter().any(|c| line.starts_with(&format!("{c}: ")));
    let wanted: BTreeSet<String> = written.iter().filter(|line| of_these(line)).cloned().collect();
    let (reached, log) = search(&wanted, seed);
    let open: BTreeSet<String> = wanted.difference(&reached.real).cloned().collect();
    let gaps: Vec<String> = (reached.real.difference(&written))
        .map(|line| format!("{line}\n    {}", reached.examples[line]))
        .collect();
    // The states of a datum that ran the cell and was held that rounds stood in, which the open lines are read against.
    let held: Vec<&String> = (reached.real.iter())
        .filter(|line| of_these(line) && line.contains(" holds") && line.ends_with("and ran it"))
        .collect();
    // What the session held, as rounds of the cells with a line of it open stood in, which the open lines are read
    // against.
    let frames: Vec<&String> = (reached.real.iter())
        .filter(|line| {
            let cell = line.split_once(": ").map(|(cell, _)| cell);
            line.contains(": the session holds")
                && open
                    .iter()
                    .any(|o| o.contains(": the session holds") && o.split_once(": ").map(|(c, _)| c) == cell)
        })
        .collect();
    assert!(
        open.is_empty() && gaps.is_empty() && reached.stale.is_empty(),
        "{} states of a datum that rounds stood in, and the tool does not write:\n{}\n\n{} of the {} states the tool writes \
         for {cells:?} were stood in by no round:\n{}\n\ndirected tests that did not bring the work where they say:\n{}\n\nthe search: {}\n\nwhat the session held in the rounds of the cells with an open line of it:\n{}\n\nheld states the rounds stood in, and what ran the cell then:\n{}",
        gaps.len(),
        gaps.join("\n"),
        open.len(),
        wanted.len(),
        by_cell(&open),
        reached.stale.join("\n"),
        log.join("\n"),
        frames.iter().map(|line| line.as_str()).collect::<Vec<_>>().join("\n"),
        (held.iter())
            .map(|line| {
                let example = &reached.examples[*line];
                let why = example.split(", round").next().unwrap_or(example);
                format!("{line} <- {}", why.chars().take(150).collect::<String>())
            })
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn every_state_of_a_datum_the_tool_writes_for_the_reading_cells_is_one_a_round_stood_in() {
    every_state_written_for(&["R0", "R1", "R2"], 0);
}

#[test]
fn every_state_of_a_datum_the_tool_writes_for_the_design_cells_is_one_a_round_stood_in() {
    every_state_written_for(&["D1", "D2"], 1);
}

#[test]
fn every_state_of_a_datum_the_tool_writes_for_the_plan_cells_is_one_a_round_stood_in() {
    every_state_written_for(&["P1", "P2"], 2);
}

#[test]
fn every_state_of_a_datum_the_tool_writes_for_the_implementation_is_one_a_round_stood_in() {
    every_state_written_for(&["I1"], 3);
}

#[test]
fn every_state_of_a_datum_the_tool_writes_for_the_first_tests_is_one_a_round_stood_in() {
    every_state_written_for(&["T1", "T2"], 4);
}

#[test]
fn every_state_of_a_datum_the_tool_writes_for_the_last_tests_is_one_a_round_stood_in() {
    every_state_written_for(&["T3", "T4"], 5);
}

#[test]
fn a_place_fits_any_text_and_what_is_written_only_itself() {
    let text = |t: &str| Piece::Text { text: t.into() };
    let slot = Piece::Slot { name: "x".into() };
    assert!(fits(&[text("a "), slot.clone(), text(" b")], "a anything b"));
    assert!(fits(&[text("a "), slot.clone()], "a "));
    assert!(!fits(&[text("a "), slot.clone(), text(" b")], "a anything c"));
    assert!(!fits(&[text("a")], "ab"));
}
