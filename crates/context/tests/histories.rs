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

mod support;

use std::collections::{BTreeMap, BTreeSet};

use context::message::{Values, kept};
use context::text::as_written;
use context::{Entered, Form, Piece, Situation, Stands};
use decide::run::Unmet;
use decide::{BlockId, CallId, CellId, Decision, Outsider, RunId, Seq, Waited, Who, Writer, decide};
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

/// How many histories of the engineer's flow are run, and how many steps each takes.
const ENGINEER_SEEDS: u64 = 12;
const ENGINEER_STEPS: usize = 250;

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
}

/// What a history of a flow does beside carrying its rounds.
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
    /// How often, in a hundred, a cell that writes a choice with the value `yes` writes it, so that the work goes on to
    /// its later cells; none where the choice is any of its values alike.
    passes: Option<u64>,
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
            passes: None,
        }
    }

    /// People and the outside write what the work reads from them: the background, and the branch's tree and head. The
    /// items it depends on have merged as it starts.
    fn engineer(sim: &Sim) -> Scene {
        let lines = sim.flow.lines.keys().cloned().collect();
        Scene {
            writes: vec!["background", "tree", "tree", "head", "head"],
            lines,
            models: false,
            talks: true,
            puts: 8,
            passes: Some(70),
        }
    }
}

/// One history, and the test's account beside it.
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
}

/// What a scripted return does.
enum Forced {
    Ask(Ask),
    Wrong,
    /// Hands in every block the cell writes.
    All,
}

impl History {
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

    fn step(&mut self, reached: &mut Reached) {
        if self.quiet > 0 {
            self.quiet -= 1;
            if let Some(round) = self.sim.round() {
                return self.carry(&round, reached);
            }
            if let Some(call) = self.sim.calls().first() {
                return self.sim.done(*call, "the rows");
            }
            if self.lets_go() {
                return;
            }
        }
        let roll = self.rng.below(100);
        let puts = self.scene.puts;
        if roll < puts {
            let block = self.scene.writes[self.rng.below(self.scene.writes.len())];
            self.put(block);
        } else if roll < puts + 4 {
            let line = self.scene.lines[self.rng.below(self.scene.lines.len())].clone();
            self.sim.anew(&line, Why::Owner);
            self.lines.insert(line, Held::default());
            reached.reopened += 1;
        } else if self.scene.talks && roll < puts + 10 && self.speaks() {
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

    /// The owner says a few words to a cell that takes them, or answers a question that stands unanswered. Says whether
    /// either was possible.
    fn speaks(&mut self) -> bool {
        if self.rng.chance(50) && self.answers() {
            return true;
        }
        let cells: Vec<CellId> = (self.sim.flow.cells.iter())
            .filter(|c| spoken::words_of(&self.sim.flow, &c.id).is_some())
            .map(|c| c.id.clone())
            .collect();
        let to = To::Cell(cells[self.rng.below(cells.len())].clone());
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
        let question = Forced::Ask(Ask::Person {
            who: Who::Owner,
            question: "Which way?".to_string(),
        });
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
        // The close, run by the finish, reads the verdict, and its run ends the work: it asks for a tool run on
        // something and on nothing, and a question, is sent back twice, so that it reaches the limit of its returns,
        // and the owner lets it go on; its rounds are given each result.
        let close = vec![tool("x"), tool(""), question, Forced::Wrong, Forced::Wrong, tool("y")];
        self.forced = Some(close);
        self.put("verdict");
        self.put("finish");
        self.under_way("close");
        let round = self.sim.round().expect("the close's round");
        self.carry(&round, reached);
        // While it rests at the call, the request is written: the plan and the build, which hand in what they write, are
        // run before its next round, which the session holds as the build's hand-in last.
        let close = self.forced.replace(vec![Forced::All, Forced::All]);
        self.put("request");
        for _ in 0..20 {
            match self.sim.round() {
                Some(round) if round.cell.as_str() != "close" => self.carry(&round, reached),
                _ => break,
            }
        }
        self.forced = close;
        self.drain(reached);
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
        let begins = self.sim.begins(round);
        let held = if begins {
            Held::default()
        } else {
            self.lines.get(&line).cloned().unwrap_or_default()
        };
        let going_on = self.sim.enter(round, false);
        let anew = self.sim.enter(round, true);
        self.check(round, &going_on, &held, reached);
        self.check(round, &anew, &Held::default(), reached);
        let lost = !begins && self.rng.chance(10);
        let used = if lost {
            self.sim.tries(round, 1, going_on.fills.clone());
            self.sim.anew(&line, Why::Lost);
            self.lines.insert(line.clone(), Held::default());
            self.sim.tries(round, 2, anew.fills.clone());
            reached.lost += 1;
            anew.clone()
        } else {
            let used = if begins { anew.clone() } else { going_on.clone() };
            self.sim.tries(round, 1, used.fills.clone());
            used
        };
        assert_eq!(self.sim.enter(round, false), going_on, "entered again after its try");
        assert_eq!(self.sim.enter(round, true), anew, "entered again anew after its try");
        // A message gives a new version whole though it says what one the session holds said.
        for fill in &going_on.fills {
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
        if self.rng.chance(8) {
            // A round that failed after its program had its message, as at the usage limit, put it into its session;
            // one that failed before its program ran put nothing there.
            if self.rng.chance(50) {
                self.lines.entry(line.clone()).or_default().fill(round.run, &used.fills);
                self.sim.fails_as(round, decide::run::Failure::UsageLimit, true);
                reached.wrote += 1;
            } else {
                self.sim.fails(round);
            }
            return;
        }
        self.lines.entry(line.clone()).or_default().fill(round.run, &used.fills);
        let compactions = u32::from(self.rng.chance(10));
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
        let (mut wrong, mut all) = (false, false);
        match self.forced.as_mut().filter(|f| !f.is_empty()).map(|f| f.remove(0)) {
            Some(Forced::Ask(ask)) => return Came::asks(vec![ask]),
            Some(Forced::Wrong) => wrong = true,
            Some(Forced::All) => all = true,
            None => {}
        }
        if !scripted && self.rng.chance(15) {
            // A tool run on something, a tool run on nothing, or a question for the owner.
            let ask = match self.rng.below(4) {
                0 | 1 => Ask::Tool {
                    name: "query".to_string(),
                    input: format!("{{\"n\": {}}}", self.count),
                },
                2 => Ask::Tool {
                    name: "query".to_string(),
                    input: String::new(),
                },
                _ => Ask::Person {
                    who: Who::Owner,
                    question: "Which way?".to_string(),
                },
            };
            return Came::asks(vec![ask]);
        }
        let cell = self.sim.cell(round).clone();
        let place = self.sim.flow.flow.place(&cell.id).expect("the cell's place");
        let outputs = self.sim.flow.flow.cells()[place].outputs.clone();
        let leaves_out = !cell.must_write.is_empty() && !all && self.rng.chance(14);
        let mut writes = Vec::new();
        for block in outputs {
            // A question is asked in the return's own field, and what answers it is the owner's.
            if self.sim.flow.blocks[&block].writers.outside.as_deref() == Some(flow::ANSWER) {
                continue;
            }
            let must = cell.must_write.contains(&block);
            if (must && !leaves_out) || (!must && (all || self.rng.chance(70))) {
                let passes = self.scene.passes.is_some_and(|percent| self.rng.chance(percent));
                let value = self.sim.flow.blocks[&block].values.clone().map(|v| match passes {
                    true if v.iter().any(|x| x == "yes") => "yes".to_string(),
                    _ => v[self.rng.below(v.len())].clone(),
                });
                let content = value.clone().unwrap_or_else(|| self.content(block.as_str()));
                writes.push(Handed {
                    block,
                    content,
                    matters: true,
                    value,
                });
            }
        }
        // A block the cell may not write is handed in beside the rest in one return of twenty-five, which is sent back,
        // and in two of five of a return that was sent back, so that a run reaches the limit of its returns.
        let again = matches!(
            self.sim.work.given.get(&(round.run, round.number)),
            Some(Given::SentBack(_))
        );
        if wrong || (!scripted && self.rng.chance(if again { 40 } else { 4 })) {
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
        let mut came = Came::hands_in(writes);
        came.said.questions = self.questions(&cell);
        came
    }

    /// A question of the cell's own in one return of twelve, where its cell takes the owner's answers: it waits for the
    /// answer in one of two.
    fn questions(&mut self, cell: &CellSpec) -> Vec<Question> {
        let compiled = &self.sim.flow;
        let answered = (cell.watches.iter()).any(|t| t.of_source(compiled, flow::ANSWER).is_some());
        if !self.scene.talks || !answered || !self.rng.chance(8) {
            return Vec::new();
        }
        let name = format!("{}_{}", cell.id, self.sim.work.records.questions.len());
        let way = |does: &str, follows: &str| Way {
            does: does.to_string(),
            follows: follows.to_string(),
        };
        vec![Question::Asked {
            ask: format!("Which way for {name}?"),
            name,
            waits: self.rng.chance(50),
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
            let because = (sim.work.records.started.iter())
                .find(|s| s.run == round.run)
                .map(|s| format!("{:?}", s.because));
            for line in stood(cell, &entered.situation) {
                let example = format!("{because:?}, round {}, {:?}", round.number, entered.situation);
                reached.examples.entry(line.clone()).or_insert(example);
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

/// What a situation of the cell says, as lines each of one thing the tool writes a situation for, apart: how the session
/// stands with each datum, and whether the datum ran the cell; what the session holds as the entry begins; what else is
/// given. A real round and a written situation are held to the same lines, so that the state of a datum is compared by
/// the cell and whether it ran it, and a state of one datum is not asked of another. A situation does not say its entry:
/// two entries that stand alike are one.
fn stood(cell: &CellSpec, s: &Situation) -> Vec<String> {
    let id = cell.id.as_str();
    // A datum ran the cell where it is watched by itself and new; a member of a group stands by its own state alone, the
    // text of a datum being the builder's from its kind and its state, and the group that ran the cell is a line of its own.
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

/// What the tool writes for every agent's cell of the flow, as the lines of [`stood`].
fn written(sim: &Sim) -> BTreeSet<String> {
    let mut lines = BTreeSet::new();
    for cell in sim.flow.cells.iter().filter(|c| c.kind == flow::Kind::Agent) {
        let all = context::situations::situations(&sim.flow, &sim.words, cell, REOPENED, &sim.compared);
        for named in all {
            lines.extend(stood(cell, &named.situation));
        }
    }
    lines
}

/// The source that writes a block of a line as its session begins anew, and none of the small flow's blocks.
const REOPENED: &str = "session";

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
    let written = written(&Sim::new());
    let (gaps, nothing) = (missing(&reached.real, &written), missing(&written, &reached.real));
    assert!(
        reached.real.is_subset(&written) && written.is_subset(&reached.real),
        "rounds stood in what the tool does not write, {gaps}\n\nthe tool writes what no round stood in, {nothing}\n\nof the close: {:?}",
        (reached.examples.iter())
            .filter(|(line, _)| line.starts_with("close: the session holds"))
            .collect::<Vec<_>>()
    );
}

/// How many of the lines are of each cell.
fn per_cell(lines: &BTreeSet<String>) -> String {
    let mut cells: BTreeMap<&str, usize> = BTreeMap::new();
    for line in lines {
        *cells.entry(line.split(':').next().unwrap_or("")).or_default() += 1;
    }
    format!("{cells:?}")
}

#[test]
fn the_rounds_of_works_of_the_engineer_s_flow_stand_in_what_the_tool_writes_for_its_cells() {
    let mut reached = Reached::default();
    let began = std::time::Instant::now();
    for seed in 0..ENGINEER_SEEDS {
        let sim = support::engineer_sim();
        let scene = Scene::engineer(&sim);
        let mut history = History::of(sim, scene, seed);
        // Each goes some way into the flow before anything else happens, the next further than the last.
        history.quiet = 12 * (seed as usize % 12);
        history.scene.passes = Some(60 + 10 * (seed % 5));
        let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for _ in 0..ENGINEER_STEPS {
                if history.ended() {
                    break;
                }
                history.step(&mut reached);
            }
        }));
        if let Err(e) = ran {
            eprintln!("the history of seed {seed} failed");
            std::panic::resume_unwind(e);
        }
    }
    let written = written(&support::engineer_sim());
    let (gaps, nothing) = (missing(&reached.real, &written), missing(&written, &reached.real));
    let gaps = format!(
        "{gaps}\n{}",
        (reached.real.difference(&written))
            .map(|line| format!("{line}\n    {}", reached.examples[line]))
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(
        reached.real.is_subset(&written) && written.is_subset(&reached.real),
        "rounds stood in what the tool does not write, {gaps}\n\nthe tool writes what no round stood in, {nothing}\n\nof the \
         rounds, by cell: {}\nof the tool, by cell: {}\nthe histories carried {} rounds in {:?}",
        per_cell(&reached.real),
        per_cell(&written),
        reached.rounds,
        began.elapsed()
    );
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
