//! Simulated histories of works of the small flow (principle simulated-histories), each round filled by the context
//! manager as a carrier fills it: people write, the lines' sessions are reopened by the owner, lost by the agents'
//! program or compacted during a round, rounds fail before they reach the model, return sent back, ask for a tool,
//! and hand in. Beside the work, the test keeps its own account of what each line's session holds, from what each
//! round that reached the model was filled with and handed in, and of the version of each datum each cell last
//! worked from.
//!
//! What is checked of every round, as it is entered in its session and in one that holds nothing:
//! - it is filled with exactly what its cell is given and its session lacks, each datum by its kind, in the cell's
//!   order, then what the session keeps that it lacks, the results of calls and why what was returned was sent back;
//! - nothing is filled that the cell is not given, its own outputs among them, and a datum with no version is never
//!   sent;
//! - a session that holds nothing is filled from the top, and the task is filled once a session;
//! - what the last return in the session handed in is what the message says it is kept as;
//! - entered again, before its try or after it, the round is sent the same bytes;
//! - a change sent of a datum given `last_and_latest` rebuilds its latest exactly from the version the session holds,
//!   and of the tree, whose versions the carrier compares, is what the carrier says changed, from the version the
//!   session read where the cell worked from none;
//! - what the page shows for the situation the round is in, its places open, has the parts, the forms and the words
//!   of what the engine sends in it, each place standing for what fills it.
//!
//! The histories must reach each of these: a change sent, one of the tree, several versions of a datum given `all` sent at once, a
//! session reopened, lost and compacted, results given and a return sent back; each is counted, and a test of no
//! history reaching one fails. A failing case prints its seed.

mod support;

use std::collections::{BTreeMap, BTreeSet};

use context::message::{Values, kept};
use context::text::as_written;
use context::{Entered, Form, Piece, Stands};
use decide::run::Unmet;
use decide::{BlockId, CallId, CellId, RunId, Seq, Writer};
use engine::{Ask, Back, Came, Fill, Given, Handed, Why};
use flow::InputKind;
use flow::agents::Part;
use support::{Given as Round, Sim};

/// How many histories are run.
const SEEDS: u64 = 60;

/// How many steps each takes.
const STEPS: usize = 120;

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
}

/// One history, and the test's account beside it.
struct History {
    sim: Sim,
    rng: Rng,
    /// What each line's session holds.
    lines: BTreeMap<String, Held>,
    /// The version of each datum each cell last worked from.
    worked: BTreeMap<(CellId, BlockId), Seq>,
    /// The last content written of each block.
    last: BTreeMap<String, String>,
    count: u64,
}

impl History {
    fn new(seed: u64) -> History {
        let mut sim = Sim::new();
        sim.takes_main = true;
        History {
            sim,
            rng: Rng::new(seed),
            lines: BTreeMap::new(),
            worked: BTreeMap::new(),
            last: BTreeMap::new(),
            count: 0,
        }
    }

    /// A new content of the block: the last one with a line changed or added, or a first one of many lines.
    fn content(&mut self, block: &str) -> String {
        self.count += 1;
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
        text
    }

    fn step(&mut self, reached: &mut Reached) {
        let roll = self.rng.below(100);
        if roll < 14 {
            let block = ["request", "notes", "tree", "plan", "opinions"][self.rng.below(5)];
            let content = self.content(block);
            self.sim.writes(block, &content);
        } else if roll < 18 {
            let line = if self.rng.chance(50) { "engineer" } else { "reviewer" };
            self.sim.anew(line, Why::Owner);
            self.lines.insert(line.to_string(), Held::default());
            reached.reopened += 1;
        } else if let Some(round) = self.sim.round() {
            self.carry(&round, reached);
        } else if let Some(call) = self.sim.calls().first() {
            self.sim.done(*call, "the rows");
        } else {
            let content = self.content("request");
            self.sim.writes("request", &content);
        }
    }

    /// Carries the round out, checking what it is sent.
    fn carry(&mut self, round: &Round, reached: &mut Reached) {
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
        if self.rng.chance(8) {
            self.sim.fails(round);
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
            self.worked_from(round, f);
        }
    }

    /// What the round returns: a tool asked for in one in ten, the review's verdict left out in one in seven, and
    /// otherwise each block the cell writes with a chance of seven in ten, those it must write every time.
    fn came(&mut self, round: &Round) -> Came {
        if self.rng.chance(10) {
            let ask = Ask::Tool {
                name: "query".to_string(),
                input: format!("{{\"n\": {}}}", self.count),
            };
            return Came::asks(vec![ask]);
        }
        let cell = self.sim.cell(round).clone();
        let place = self.sim.flow.flow.place(&cell.id).expect("the cell's place");
        let outputs = self.sim.flow.flow.cells()[place].outputs.clone();
        let leaves_out = !cell.must_write.is_empty() && self.rng.chance(14);
        let mut writes = Vec::new();
        for block in outputs {
            let must = cell.must_write.contains(&block);
            if (must && !leaves_out) || (!must && self.rng.chance(70)) {
                let value = self.sim.flow.blocks[&block]
                    .values
                    .as_ref()
                    .map(|v| v[self.rng.below(v.len())].clone());
                let content = value.clone().unwrap_or_else(|| self.content(block.as_str()));
                writes.push(Handed {
                    block,
                    content,
                    matters: true,
                    value,
                });
            }
        }
        Came::hands_in(writes)
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
    fn datum(&self, round: &Round, held: &Held, block: &BlockId, kind: InputKind) -> (Stands, Vec<Fill>, Option<Seq>) {
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
            .any(|v| Some(v.at) > last && v.meaning == v.at && !own(&v.by));
        let whole = |at: Seq| Fill::Whole {
            block: block.clone(),
            version: at,
        };
        let lacking: Vec<Seq> = all.iter().map(|v| v.at).filter(|at| !held.holds(block, *at)).collect();
        // The carrier says what changed in the tree, from the version the session read where the cell worked from none.
        let compared = block.as_str() == "tree";
        let lacks_last = new && last.is_some_and(|l| l < latest && !held.holds(block, l));
        if kind == InputKind::All && !lacking.is_empty() {
            let stands = if held.holds(block, all[0].at) {
                Stands::HoldsLast
            } else {
                Stands::Lacks
            };
            return (stands, lacking.into_iter().map(whole).collect(), last);
        }
        if held.holds(block, latest) && lacks_last && kind == InputKind::LastAndLatest && !compared {
            return (Stands::Lacks, vec![whole(latest)], last);
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
                    for given in &cell.given {
                        fills.extend(self.datum(round, held, &given.block, given.kind).1);
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
                            let given = cell.given.iter().any(|g| &g.block == block);
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
        assert_eq!(
            entered.fills,
            self.expected(round, entered, held),
            "what is filled: {}",
            entered.text
        );
        let given: Vec<&BlockId> = (cell.given.iter().map(|g| &g.block))
            .chain(entered.run.written_over.iter().map(|(b, _)| b))
            .collect();
        reached.written_over += entered.run.written_over.len();
        // How the session stands with each datum, and the line of one it holds that is new to the cell.
        for g in &cell.given {
            let (stands, _, last) = self.datum(round, held, &g.block, g.kind);
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
                    let last = format!("last worked from {}", last.expect("a version worked from").0);
                    assert!(
                        as_written(&sent.pieces).ends_with(&last),
                        "{}",
                        as_written(&sent.pieces)
                    );
                    reached.held += 1;
                }
                Stands::HoldsNew => {
                    let sent = sent.expect("the line of a datum held that the cell never worked from");
                    assert_eq!(sent.form, Form::Held);
                    let version = format!("version {}, ", entered.run.data[&g.block].latest.number.0);
                    assert!(
                        as_written(&sent.pieces).contains(&version),
                        "{}",
                        as_written(&sent.pieces)
                    );
                    reached.held_new += 1;
                }
                Stands::Holds | Stands::Unwritten => assert!(sent.is_none(), "{name} is sent"),
                Stands::Lacks if held.holds(&g.block, entered.run.data[&g.block].latest.number) => {
                    reached.lacks_last += 1;
                }
                Stands::Lacks | Stands::HoldsLast => assert!(sent.is_some(), "{name} is not sent"),
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
            for g in &cell.given {
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
        assert_eq!(entered.run.taken, held.taken, "what the last return handed in");
        assert_eq!(entered.situation.after_hand_in, !held.taken.is_empty());
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
    for seed in 0..SEEDS {
        let mut history = History::new(seed);
        let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for _ in 0..STEPS {
                history.step(&mut reached);
            }
            reached.put_in += history.sim.work.records.put_in.len();
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
            "the latest sent whole where the session holds it and not the version last worked from",
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
    ] {
        assert!(n > 0, "no history reached {what}");
    }
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
