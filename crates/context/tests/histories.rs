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
//! The situations the tool writes (`wf-agents flow situations`) are the rounds of works that a search of the context
//! crate carried out, and these histories choose what they do apart from that search: its set of actions is its own,
//! and only the simulation of a work is shared. Each situation written is held to be a real round (its script, run on a
//! fresh work, carries out a round in the same situation, and what the page shows for it is what the engine sent), and
//! every line a round of these histories stood in ([`context::situations::stood`]: a real round is a product of every
//! datum's own state, and a written situation is one cut of them) is one a situation written stands in, on the small flow
//! in the test of the rounds below, and on the engineer's flow, which these histories carry with the owner's words,
//! questions and answers, in a test of its own that names the lines the search left out, by cell. The directed tests of
//! the engineer's flow bring about, by what its cells hand in and what the owner and the outside write between two
//! rounds of one session, what random steps reach too seldom; each says where the work stands if it does not come to
//! the round it is for.

mod support;

use std::collections::{BTreeMap, BTreeSet};

use context::message::{Values, kept};
use context::situations::stood;
use context::text::as_written;
use context::witness;
use context::{Entered, Form, Piece, Stands};
use decide::run::Unmet;
use decide::{BlockId, CallId, CellId, Decision, Outsider, RunId, Seq, Waited, Writer, decide};
use engine::spoken::{self, To};
use engine::{Ask, Back, Call, Came, Event, Fill, Given, Handed, Why};
use flow::agents::Part;
use flow::{CellSpec, InputKind, Listed};
use support::{Given as Round, Sim, fits};
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
    /// The rounds of the last scripted history.
    trace: Vec<String>,
    /// The directed tests that did not bring the work where they say, and why.
    stale: Vec<String>,
}

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
            prefer: std::mem::replace(&mut self.prefer, mode.prefer),
            nothing_dealt: std::mem::replace(&mut self.nothing_dealt, mode.nothing_dealt),
        }
    }

    fn new(seed: u64) -> History {
        let mut sim = support::sim_of(support::limited(), support::words());
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
            forward: 0,
            steady: false,
            says: BTreeMap::new(),
            nothing_dealt: BTreeSet::new(),
            prefer: BTreeMap::new(),
            waits: BTreeSet::new(),
            compact_next: false,
            force: BTreeMap::new(),
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
        self.scene.passes = Some(100);
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
            let drawn = match &said {
                Some(_) => listed.is_some(),
                None => !only_must && (all || self.rng.chance(percent)),
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
    // What the tool writes is held to real rounds: the search carries works of the flow out and keeps, for each line a
    // round stood in, a script that carries such a round out, and each situation written is one of them, which its script,
    // run again, carries out. The histories above choose what they do apart from the search, so that every line a round
    // of theirs stood in is one the search found, or the search leaves out what a work can reach.
    let sim = support::sim_of(support::limited(), support::words());
    let found = witness::search(&sim.flow, &sim.words, &sim.compared, None, &witness::BOUNDS);
    let written = support::written(&sim, &found);
    let gaps = missing(&reached.real, &written.lines);
    assert!(
        found.faults.is_empty()
            && written.wrong.is_empty()
            && written.without.is_empty()
            && reached.real.is_subset(&written.lines),
        "what the search met: {:?}\n\nwhat is wrong with a situation written: {:?}\n\nthe cells that none is written \
         for: {:?}\n\nrounds stood in what the search did not find, {gaps}\n\nof the close: {:?}\n\nthe last scripted \
         history: {:?}",
        found.faults,
        written.wrong,
        written.without,
        (reached.examples.iter())
            .filter(|(line, _)| line.starts_with("close: the session holds"))
            .collect::<Vec<_>>(),
        reached.trace.iter().rev().take(30).rev().collect::<Vec<_>>()
    );
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

/// How many histories of the engineer's flow the test runs apart from the search, how many steps each takes, and how
/// many of the first steps of each only carry what the work gives with everything handed in, so that the history goes
/// into the flow.
const ENGINEER_HISTORIES: u64 = 3;
const ENGINEER_STEPS: usize = 400;
const ENGINEER_QUIET: usize = 120;

/// The histories of the engineer's flow that choose what they do apart from the search, each in a mode of its own for
/// what returns hand in, deal with and ask for, and the directed tests: what the rounds of them stood in.
fn the_engineers_histories() -> Reached {
    let mut reached = Reached::default();
    for n in 0..ENGINEER_HISTORIES {
        let sim = support::engineer_sim();
        let scene = Scene::engineer(&sim);
        let mut history = History::of(sim, scene, 1_000 + n);
        let mode = n as usize;
        history.quiet = ENGINEER_QUIET;
        history.forward = 40;
        history.writes = [25, 70, 100][mode];
        history.asks = [8, 40, 90][mode];
        history.unmatters = [0, 20, 50][mode];
        history.partial = [0, 25, 60][mode];
        history.calls = [15, 40, 70][mode];
        for _ in 0..ENGINEER_STEPS {
            if history.ended() {
                break;
            }
            history.step(&mut reached);
        }
    }
    let (passed, deep) = (first_pass(&mut reached, false), first_pass(&mut reached, true));
    for line in directed_tests(&mut reached, &passed, &deep) {
        eprintln!("{line}");
    }
    reached
}

// The tool writes, for the engineer's flow, situations that are rounds of real works, and every line a round stood in
// under histories that choose what they do apart from the search is one of them. One test, because the search runs once
// and each test of nextest has a process of its own.
#[test]
fn every_situation_written_for_the_engineers_flow_is_a_round_of_a_work_and_covers_what_the_histories_stood_in() {
    let begun = std::time::Instant::now();
    let sim = support::engineer_sim();
    let found = witness::search(&sim.flow, &sim.words, &sim.compared, None, &witness::BOUNDS);
    eprintln!(
        "the search: {:?}, {} rounds, {} states kept, {} lines found",
        begun.elapsed(),
        found.rounds,
        found.kept,
        found.lines.len()
    );
    let written = support::written(&sim, &found);
    eprintln!(
        "{} situations written, {} of a session reopened after another cell was read in it, {:?}",
        written.situations,
        written.after_read,
        begun.elapsed()
    );
    let reached = the_engineers_histories();
    eprintln!(
        "the histories: {} lines, {} rounds, {:?}",
        reached.real.len(),
        reached.rounds,
        begun.elapsed()
    );
    let gaps: BTreeSet<String> = reached.real.difference(&written.lines).cloned().collect();
    assert!(
        found.faults.is_empty() && written.wrong.is_empty() && written.without.is_empty(),
        "what the search met: {:?}\n\nwhat is wrong with a situation written: {:?}\n\nthe cells that none is written \
         for: {:?}",
        found.faults,
        written.wrong,
        written.without
    );
    assert!(
        written.after_read > 0,
        "no situation of a session begun anew since the cell ran in which another cell of its line was entered first"
    );
    assert!(
        reached.stale.is_empty(),
        "directed tests that did not bring the work where they say:\n{}",
        reached.stale.join("\n")
    );
    assert!(
        gaps.is_empty(),
        "rounds stood in {} lines that the search did not find:\n{}",
        gaps.len(),
        by_cell(&gaps)
    );
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

#[test]
fn a_place_fits_any_text_and_what_is_written_only_itself() {
    let text = |t: &str| Piece::Text { text: t.into() };
    let slot = Piece::Slot { name: "x".into() };
    assert!(fits(&[text("a "), slot.clone(), text(" b")], "a anything b"));
    assert!(fits(&[text("a "), slot.clone()], "a "));
    assert!(!fits(&[text("a "), slot.clone(), text(" b")], "a anything c"));
    assert!(!fits(&[text("a")], "ab"));
}
