//! The exploration of a flow: from the work as the owner starts it, every way it can go within a bound, breadth first,
//! the work driven by the engine's own loop ([`engine::handle`]), which keeps its records. What the exploration gives
//! the loop is what happens outside it: what a round under way returns ([`model::Variant`]), what the owner or the
//! outside writes ([`model::Event`]), and what the engine asked to be done and is its own, a call withdrawn, a round
//! stopped, what an ended work held let go. A state the search keeps is one where something outside happens next: a
//! round under way returns, or the work waits or rests and only the outside can write.
//!
//! The properties are checked as each run starts, on the records as they were just before, and at each stop, by the
//! records read as the design tree says, apart from the deciding code (crate `reading`), never by the walk's own
//! reading. A breach is kept under a name that says what went wrong without the path, the first path to it beside it;
//! the search is breadth first, so that path is among the shortest. Each item the exploration reaches is kept with the
//! nearest path to it: what was given the loop, in order, which given again to a new work gives the same records
//! ([`replay`]).

pub mod model;

use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;
use std::time::{Duration, Instant};

use decide::{
    BlockId, Cause, CellId, Condition, Decision, End, Maker, Outsider, Records, RunId, Seq, Stalling, Unrun, Waited,
    Who, decide, told,
};
use engine::spoken::To;
use engine::{Came, Effect, Event as Given, Handed, Refused, Spent, Time, Work};
use reading::{Reading, Reason, Writer};

use model::{
    DEPENDENCIES, Event, HEAD, MAIN, Model, START, TREE, Variant, ran, ran_back_on, ran_on, reply_loop, show_key,
    skipped, waited_at_gate,
};

/// The properties, by their numbers in the owner's test plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Property {
    /// 1. A cell runs only on a version of one of its own triggers that it has not consumed and did not write.
    NewToIt = 1,
    /// 2. Cells whose conditions want another value of a block never both run off the same version of it.
    Contradicting = 2,
    /// 3. Every state has a next step, or is a stop on purpose: the owner or the outside is waited for, or the work
    ///    ended.
    NextStep = 3,
    /// 4. With nothing written from outside, the walk comes to a stop within a bound, never coming back to a state it
    ///    was in.
    ComesToRest = 4,
    /// 5. A group runs a cell only for a change of one of its members, and the cell is given every member.
    Groups = 5,
    /// 6. A cell runs only while each of its preconditions holds, and the check only once every question put to the
    ///    owner is answered.
    Gates = 6,
    /// 7. A write from outside runs again only the cells its change reaches, and reaches each cell it runs.
    Incremental = 7,
    /// 8. Every stall is shown as waiting for the owner, with where it stands, what it waits for and why each cell that
    ///    writes that does not run, as the reading finds them.
    Shown = 8,
    /// What the exploration itself could not do as the engine does: a step the engine's own loop refused, or left
    /// half done.
    Engine = 0,
}

/// The bounds of the exploration.
#[derive(Clone, Copy)]
pub struct Bounds {
    /// How many returns other than the content, and writes from outside, a path may take.
    pub spend: u8,
    /// The most a path may have spent before the outside writes while the work waits or rests.
    pub outside_after: u8,
    /// The most things given the loop in a row with nothing written from outside.
    pub steps: u32,
    /// The most states the search keeps: past it, the search stops, and the test fails.
    pub states: u64,
}

/// What the exploration found.
#[derive(Default)]
pub struct Explored {
    /// Each breach, by its property and its name, with the first path to it and what was found there.
    pub breaches: BTreeMap<(Property, String), Breach>,
    /// What it reached, each with how often.
    pub reached: BTreeMap<String, u64>,
    /// Of each item reached, the nearest path to it.
    pub paths: BTreeMap<String, Path>,
    /// Of each property, how many cases it was checked on: a property checked on none judged nothing.
    pub checked: BTreeMap<Property, u64>,
    /// Of each write from outside, by what it was, the cells it ran again.
    pub reruns: BTreeMap<String, BTreeSet<CellId>>,
    pub stats: Stats,
}

#[derive(Default)]
pub struct Stats {
    /// The states kept: rounds under way, waits and rests; and of them, by what their paths spent, those before and
    /// past a write from outside.
    pub states: u64,
    pub by_spent: BTreeMap<(u8, bool), u64>,
    /// The things given the engine's loop, and the calls of the engine's reading of what a round was told.
    pub decisions: u64,
    pub told: u64,
    /// The runs started.
    pub runs: u64,
    /// The most things given the loop in a row with nothing written from outside.
    pub longest: u32,
    pub took: Duration,
}

pub struct Breach {
    pub path: Vec<String>,
    pub what: String,
    rank: (u8, u32),
}

impl Breach {
    pub fn show(&self) -> String {
        let mut s = String::new();
        for (n, step) in self.path.iter().enumerate() {
            s.push_str(&format!("  {:>2}. {step}\n", n + 1));
        }
        s.push_str(&format!("  => {}\n", self.what));
        s
    }
}

/// The nearest path to an item reached: its steps as words, what was given the engine's loop in order, and the work's
/// records as they stood when the item was reached.
pub struct Path {
    pub steps: Vec<String>,
    pub given: Vec<Given>,
    pub records: Records,
    rank: (u8, u32),
}

/// The steps of a path, shared between the states that follow it: each said in words, or given the loop.
#[derive(Clone, Default)]
struct Trace(Option<Rc<Link>>);

struct Link {
    step: Step,
    before: Trace,
    /// What the path had spent at this step, and its steps up to it.
    spent: u8,
    len: u32,
}

enum Step {
    Said(String),
    Given(Box<Given>),
}

impl Trace {
    fn link(&self, spent: u8, step: Step) -> Trace {
        let len = self.0.as_ref().map_or(0, |l| l.len) + 1;
        Trace(Some(Rc::new(Link {
            step,
            before: self.clone(),
            spent,
            len,
        })))
    }

    fn then(&self, spent: u8, said: String) -> Trace {
        self.link(spent, Step::Said(said))
    }

    fn gave(&self, spent: u8, given: Given) -> Trace {
        self.link(spent, Step::Given(Box::new(given)))
    }

    /// How far the path is from the start: what it spent, then its steps. Of two paths to one breach, the nearer is
    /// kept.
    fn rank(&self) -> (u8, u32) {
        self.0.as_ref().map_or((0, 0), |l| (l.spent, l.len))
    }

    fn links(&self) -> Vec<&Link> {
        let mut links = Vec::new();
        let mut at = self.0.as_deref();
        while let Some(link) = at {
            links.push(link);
            at = link.before.0.as_deref();
        }
        links.reverse();
        links
    }

    fn steps(&self) -> Vec<String> {
        (self.links().into_iter())
            .filter_map(|l| match &l.step {
                Step::Said(said) => Some(said.clone()),
                Step::Given(_) => None,
            })
            .collect()
    }

    fn given(&self) -> Vec<Given> {
        (self.links().into_iter())
            .filter_map(|l| match &l.step {
                Step::Given(given) => Some((**given).clone()),
                Step::Said(_) => None,
            })
            .collect()
    }
}

/// What reached on from a write from outside: what it was, the blocks it wrote, the cells its change can reach, and the
/// versions it, an answer it gave and the runs it ran wrote.
#[derive(Clone)]
struct Taint {
    what: String,
    blocks: Vec<BlockId>,
    reach: BTreeSet<usize>,
    versions: BTreeSet<Seq>,
    runs: BTreeSet<RunId>,
}

#[derive(Clone)]
struct State {
    /// The work, its records kept by the engine's loop.
    work: Work,
    /// The clock the loop is given: a second more for each thing it takes.
    clock: u64,
    /// The return each cell gives at every round from now on, where one was chosen for all.
    always: BTreeMap<CellId, Variant>,
    spent: u8,
    /// The outside wrote on this path, and what that reached.
    outside: Option<Taint>,
    /// The things given the loop since the outside last wrote, and the states kept since then.
    since: u32,
    ancestors: Vec<u64>,
    trace: Trace,
}

pub struct Explorer<'m> {
    model: &'m Model,
    bounds: Bounds,
    out: Explored,
}

/// A place after every entry: the records as they are now.
const NOW: Seq = Seq(u64::MAX);

/// The time the loop is first given, in milliseconds.
const CLOCK: u64 = 1_800_000_000_000;

/// The result a call withdrawn is given.
const WITHDRAWN: &str = "Withdrawn: its run was abandoned before it had its result.";

pub fn explore(model: &Model, bounds: Bounds) -> Explored {
    let began = Instant::now();
    let mut e = Explorer {
        model,
        bounds,
        out: Explored::default(),
    };
    e.static_checks();
    let mut seen: HashMap<u64, u8> = HashMap::new();
    let mut queue = VecDeque::new();
    if let Some(first) = e.first() {
        queue.push_back(first);
    }
    while let Some(mut state) = queue.pop_front() {
        if e.out.stats.states >= bounds.states {
            let name = format!("the search kept {} states, its most, and stopped", bounds.states);
            e.breach(Property::Engine, name, &state.trace, String::new());
            break;
        }
        let decision = decide(e.flow(), &state.work.records);
        if !e.at_rest(&state, &decision) {
            continue;
        }
        let key = e.key(&state);
        if state.ancestors.contains(&key) {
            e.breach(
                Property::ComesToRest,
                "the walk came back to a state it was in, with nothing written from outside".to_string(),
                &state.trace,
                format!("{decision:?}"),
            );
            continue;
        }
        match seen.entry(key) {
            Entry::Occupied(spent) if *spent.get() <= state.spent => continue,
            Entry::Occupied(mut spent) => *spent.get_mut() = state.spent,
            Entry::Vacant(v) => {
                v.insert(state.spent);
            }
        }
        state.ancestors.push(key);
        e.out.stats.states += 1;
        *e.out
            .stats
            .by_spent
            .entry((state.spent, state.outside.is_some()))
            .or_default() += 1;
        for child in e.children(&state, &decision) {
            queue.push_back(child);
        }
    }
    e.out.stats.took = began.elapsed();
    e.out
}

/// Gives the engine's loop, for a new work on the model's flow, what a path gave it, in order and with the same
/// clock: the work as the loop leaves it, or the first thing it refused.
pub fn replay(model: &Model, given: &[Given]) -> Result<Work, Refused> {
    let mut work = Work::default();
    let mut clock = CLOCK;
    for g in given {
        clock += 1000;
        engine::handle(&model.compiled, &mut work, Time(clock), g.clone())?;
    }
    Ok(work)
}

/// A write of a block from outside the work, with this value where it is a choice.
fn from_outside(block: &BlockId, value: Option<&str>, clock: u64) -> Given {
    Given::Wrote {
        block: block.clone(),
        by: Outsider::Outside,
        content: match value {
            Some(_) => String::new(),
            None => format!("The {block} as the outside wrote it at {clock}."),
        },
        matters: true,
        value: value.map(str::to_string),
    }
}

/// A person's write of a block.
fn by_person(block: &BlockId, clock: u64) -> Given {
    Given::Wrote {
        block: block.clone(),
        by: Outsider::Person,
        content: format!("The {block} as the owner wrote it at {clock}."),
        matters: true,
        value: None,
    }
}

/// The latest version of each question asked, by its name: the cell that asked it, the place of that version, whether
/// it is withdrawn, and whether the owner answered it.
fn questions(records: &Records) -> BTreeMap<String, (CellId, Seq, bool, bool)> {
    let mut latest: BTreeMap<String, (CellId, Seq, bool, bool)> = BTreeMap::new();
    for q in &records.questions {
        latest.insert(q.name.clone(), (q.cell.clone(), q.at, q.withdrawn, false));
    }
    for (name, (_, at, _, answered)) in latest.iter_mut() {
        *answered = records.answers.iter().any(|a| &a.question == name && a.version == *at);
    }
    latest
}

impl Explorer<'_> {
    fn flow(&self) -> &decide::Flow {
        &self.model.compiled.flow
    }

    fn count(&mut self, property: Property) {
        *self.out.checked.entry(property).or_default() += 1;
    }

    fn reach(&mut self, what: String, trace: &Trace, records: &Records) {
        *self.out.reached.entry(what.clone()).or_default() += 1;
        let rank = trace.rank();
        let nearer = (self.out.paths.get(&what)).is_none_or(|p| rank < p.rank);
        if nearer {
            let path = Path {
                steps: trace.steps(),
                given: trace.given(),
                records: records.clone(),
                rank,
            };
            self.out.paths.insert(what, path);
        }
    }

    fn breach(&mut self, property: Property, name: String, trace: &Trace, what: String) {
        let rank = trace.rank();
        let found = (self.out.breaches.get(&(property, name.clone()))).is_some_and(|b| b.rank <= rank);
        if !found {
            let breach = Breach {
                path: trace.steps(),
                what,
                rank,
            };
            self.out.breaches.insert((property, name), breach);
        }
    }

    /// What holds of the definition whatever a work does: each cell a group runs is given each of its members.
    fn static_checks(&mut self) {
        let m = self.model;
        for (i, spec) in m.compiled.cells.iter().enumerate() {
            for (trigger, members) in &m.groups[i] {
                for member in members {
                    self.count(Property::Groups);
                    let runs = m.cells()[i].runs_on.contains(member);
                    let given = spec.given.iter().any(|g| &g.block == member);
                    if !runs || !given {
                        let name = format!(
                            "{} has {trigger}, and {member} of it does not run it or is not given",
                            spec.id
                        );
                        self.breach(Property::Groups, name, &Trace::default(), String::new());
                    }
                }
            }
        }
    }

    /// The work as the owner starts it: the start written, what the owner writes of the work written, the items the
    /// work depends on not yet merged, and main written, as the watch of main writes it into every work that has not
    /// ended; each where the flow has a block of it.
    fn first(&mut self) -> Option<State> {
        let m = self.model;
        let mut s = State {
            work: Work::default(),
            clock: CLOCK,
            always: BTreeMap::new(),
            spent: 0,
            outside: None,
            since: 0,
            ancestors: Vec::new(),
            trace: Trace::default().then(0, "the owner starts the work: the background is written".to_string()),
        };
        let mut given = Vec::new();
        if let Some(block) = m.sources.get(START) {
            given.push(from_outside(block, None, CLOCK));
        }
        for block in &m.people {
            given.push(by_person(block, CLOCK));
        }
        if let Some(block) = m.sources.get(DEPENDENCIES) {
            given.push(from_outside(block, Some("waiting"), CLOCK));
        }
        if let Some(block) = m.sources.get(MAIN) {
            given.push(from_outside(block, None, CLOCK));
        }
        for g in given {
            if !self.give(&mut s, g) {
                return None;
            }
        }
        Some(s)
    }

    /// Gives the engine's loop this, and does what it then asks that is its own to have done: a call withdrawn is given
    /// its result, a round stopped ends with nothing, what an ended work held is let go. Says whether the loop took it;
    /// where it refused, that is a breach, but for the owner's words no cell can take, which only make no path.
    fn give(&mut self, s: &mut State, given: Given) -> bool {
        match self.take(s, given) {
            Ok(()) => true,
            Err(Refused::NoOneSpokenTo(_)) => false,
            Err(why) => {
                self.breach(
                    Property::Engine,
                    format!("the engine's loop refused {}", refused_kind(&why)),
                    &s.trace,
                    format!("{why:?}"),
                );
                false
            }
        }
    }

    fn take(&mut self, s: &mut State, given: Given) -> Result<(), Refused> {
        let before = s.work.records.clone();
        s.clock += 1000;
        s.since += 1;
        self.out.stats.decisions += 1;
        self.out.stats.longest = self.out.stats.longest.max(s.since);
        s.trace = s.trace.gave(s.spent, given.clone());
        let effects = engine::handle(&self.model.compiled, &mut s.work, Time(s.clock), given)?;
        self.after(s, &before);
        for effect in effects {
            let follow = match effect {
                Effect::Withdraw { call } => Some(Given::Done {
                    call,
                    result: WITHDRAWN.to_string(),
                }),
                Effect::Stop { run } => Some(Given::Returned {
                    run,
                    came: Came::asks(Vec::new()),
                    spent: Spent::default(),
                }),
                Effect::Release { .. } => Some(Given::Released),
                Effect::Round { .. } | Effect::Call { .. } | Effect::Tell { .. } | Effect::Ping { .. } => None,
            };
            if let Some(follow) = follow {
                self.take(s, follow)?;
            }
        }
        Ok(())
    }

    /// What the loop did as it took one thing, read from the records it added, in the order it added them: each run
    /// it started, checked on the records as they were just before; each run it finished, sent back, abandoned or
    /// asked the owner of at its limit; and each question asked.
    fn after(&mut self, s: &mut State, before: &Records) {
        let m = self.model;
        let r = s.work.records.clone();
        let cell_of = |run: RunId| (r.started.iter()).find(|st| st.run == run).map(|st| st.cell.clone());
        let mut said: Vec<(Seq, String)> = Vec::new();
        for f in &r.finished[before.finished.len()..] {
            let shown: Vec<String> = (f.writes.iter())
                .map(|w| match &w.value {
                    Some(v) => format!("{} = {v}", w.block),
                    None => w.block.to_string(),
                })
                .collect();
            let writes = if shown.is_empty() {
                "nothing".to_string()
            } else {
                shown.join(", ")
            };
            let cell = cell_of(f.run).map_or(String::new(), |c| c.to_string());
            said.push((f.at, format!("the engine takes it: {cell} writes {writes}")));
            if let Some(t) = s.outside.as_mut()
                && t.runs.contains(&f.run)
            {
                t.versions.insert(f.at);
            }
        }
        for a in &r.abandoned[before.abandoned.len()..] {
            let cell = cell_of(a.run).map_or(String::new(), |c| c.to_string());
            said.push((a.at, format!("the engine abandons the run of {cell}")));
        }
        let sent_back: Vec<&decide::Round> = (r.rounds[before.rounds.len()..].iter())
            .filter(|e| before.started.iter().any(|st| st.run == e.run))
            .filter(|e| !r.abandoned.iter().any(|a| a.run == e.run))
            .collect();
        for e in sent_back {
            let cell = cell_of(e.run).map_or(String::new(), |c| c.to_string());
            said.push((e.at, format!("the engine sends {cell}'s return back")));
            let trace = s.trace.clone();
            self.reach("a return was sent back".to_string(), &trace, &r);
            if r.put_in[before.put_in.len()..].iter().any(|p| p.run == e.run) {
                self.reach(WRITTEN_OVER.to_string(), &trace, &r);
            }
        }
        for c in &r.called[before.called.len()..] {
            if c.on == Some(Who::Owner) {
                let cell = cell_of(c.run).map_or(String::new(), |c| c.to_string());
                said.push((c.at, format!("{cell}'s run is at its limit: the owner is asked")));
                let trace = s.trace.clone();
                self.reach("a run reached its limit of returns sent back".to_string(), &trace, &r);
            }
        }
        for q in &r.questions[before.questions.len()..] {
            let waits = if q.waits { "waits on it" } else { "goes on past it" };
            said.push((q.at, format!("{} asks the owner {}, and {waits}", q.cell, q.name)));
        }
        let starts: Vec<&decide::Started> = r.started[before.started.len()..].iter().collect();
        for st in &starts {
            let i = m.place(&st.cell);
            let names: BTreeSet<String> = (st.because.iter())
                .map(|c| match c {
                    Cause::Changed { block, .. } => m.triggers[i].get(block).cloned().unwrap_or(block.0.clone()),
                    Cause::Results { .. } => "the answers it waited for".to_string(),
                    Cause::NoRecord => "no record".to_string(),
                })
                .collect();
            said.push((
                st.at,
                format!(
                    "{} starts, for {}",
                    st.cell,
                    names.into_iter().collect::<Vec<_>>().join(", ")
                ),
            ));
        }
        said.sort_by_key(|(at, _)| *at);
        for (_, line) in said {
            s.trace = s.trace.then(s.spent, line);
        }
        for st in starts {
            self.out.stats.runs += 1;
            let at_start = r.before(st.at);
            let i = m.place(&st.cell);
            let tainted = s
                .outside
                .as_ref()
                .is_some_and(|t| self.tainted(&at_start, i, &st.because, t));
            self.check_run(s, &at_start, &st.cell, &st.because, tainted);
            if tainted && let Some(t) = s.outside.as_mut() {
                t.runs.insert(st.run);
            }
        }
    }

    /// Whether the state is at rest, the loop having done all that is its own: a round under way, or a wait, a
    /// stall, a pause or an end, each stop checked; and not past the bound of steps.
    fn at_rest(&mut self, s: &State, d: &Decision) -> bool {
        if s.since > self.bounds.steps {
            let name = format!(
                "no stop within {} things given the loop with nothing written from outside",
                self.bounds.steps
            );
            self.breach(Property::ComesToRest, name, &s.trace, format!("{d:?}"));
            return false;
        }
        self.count(Property::ComesToRest);
        match d {
            Decision::Busy { .. } | Decision::Paused => true,
            Decision::Wait { .. } | Decision::Stalled { .. } | Decision::Ended(_) => {
                self.check_stop(s, d);
                true
            }
            Decision::Fault(fault) => {
                self.breach(
                    Property::Engine,
                    format!("the records are faulty: {fault:?}"),
                    &s.trace,
                    String::new(),
                );
                false
            }
            d => {
                let name = "the engine's loop left the work at a step of its own".to_string();
                self.breach(Property::Engine, name, &s.trace, format!("{d:?}"));
                false
            }
        }
    }

    /// Whether a run of the cell at this place for these reasons, on these records, runs for what the write from
    /// outside reached.
    fn tainted(&self, records: &Records, i: usize, because: &[Cause], taint: &Taint) -> bool {
        let r = Reading::of_flow(self.flow(), records);
        let cell = &self.model.cells()[i].id;
        because.iter().any(|cause| match cause {
            Cause::Changed { block, .. } => (r.new_to(cell, block, NOW).iter()).any(|v| taint.versions.contains(&v.at)),
            Cause::Results { calls } => {
                (records.done.iter()).any(|d| calls.contains(&d.call) && taint.versions.contains(&d.at))
            }
            Cause::NoRecord => false,
        })
    }

    /// Whether a run of the cell finished on the path writing a record it asks for tests in.
    fn asked_before(&self, s: &State, cell: &CellId) -> bool {
        let m = self.model;
        let records = &m.asking[m.place(cell)];
        let r = &s.work.records;
        (r.finished.iter())
            .filter(|f| r.started.iter().any(|st| st.run == f.run && &st.cell == cell))
            .any(|f| f.writes.iter().any(|w| records.contains(&w.block)))
    }

    /// What can follow a state the search keeps.
    fn children(&mut self, s: &State, d: &Decision) -> Vec<State> {
        let m = self.model;
        let mut out = Vec::new();
        let may_spend = s.spent < self.bounds.spend && s.outside.is_none();
        match d {
            Decision::Busy { cell, run } => {
                let i = m.place(cell);
                match s.always.get(cell) {
                    Some(v) => out.extend(self.returns(s, cell, *run, v.clone(), false, false)),
                    None => {
                        out.extend(self.returns(s, cell, *run, Variant::Content, false, false));
                        if may_spend {
                            self.out.stats.told += 1;
                            let told = told(self.flow(), &s.work.records, *run).unwrap_or_default();
                            for (v, may_always) in m.deviations(i, &told) {
                                // A return sent back takes nothing in and is the same wherever it comes: a path
                                // that spent nothing tries it.
                                if v == Variant::SentBack && s.spent > 0 {
                                    continue;
                                }
                                // How often a cell asks for tests is judged by the history of asking beside the walk:
                                // once a path is what the walk itself reads.
                                if v == Variant::Tests && self.asked_before(s, cell) {
                                    continue;
                                }
                                out.extend(self.returns(s, cell, *run, v.clone(), true, false));
                                if may_always {
                                    out.extend(self.returns(s, cell, *run, v, true, true));
                                }
                            }
                        }
                    }
                }
                // While a round is under way, the owner speaks to its line; on a path that spent nothing, the outside
                // also writes the background or what runs the cell.
                if s.outside.is_none() && s.spent <= self.bounds.outside_after && s.spent < self.bounds.spend {
                    let cells = m.cells();
                    let mut events = Vec::new();
                    if m.spoken[i].is_some()
                        && let Some(line) = &m.compiled.cells[i].line
                    {
                        events.push(Event::Speaks(line.clone()));
                    }
                    if s.spent == 0 {
                        if !m.people.is_empty() {
                            events.push(Event::Background);
                        }
                        for (source, block) in &m.sources {
                            if (*source == MAIN || *source == HEAD) && cells[i].runs_on.contains(block) {
                                events.push(Event::Source(source));
                            }
                        }
                        for (line, block) in &m.sessions {
                            if cells[i].runs_on.contains(block) {
                                events.push(Event::Session(line.clone()));
                            }
                        }
                    }
                    for e in events {
                        out.extend(self.event(s, d, e));
                    }
                }
            }
            Decision::Wait { .. } | Decision::Stalled { .. } => {
                let waiting = (m.sources.get(DEPENDENCIES)).is_some_and(|deps| {
                    (s.work.records.written.iter())
                        .rev()
                        .find(|w| &w.block == deps)
                        .is_some_and(|w| w.value.as_deref() != Some("merged"))
                });
                if waiting && let Some(mut child) = self.event(s, d, Event::Dependencies) {
                    child.outside = None;
                    child.spent = s.spent;
                    child.since = 0;
                    child.ancestors.clear();
                    out.push(child);
                }
                if s.outside.is_none() && s.spent <= self.bounds.outside_after && s.spent < self.bounds.spend {
                    for e in self.events(s, d) {
                        out.extend(self.event(s, d, e));
                    }
                } else if s.outside.is_none()
                    && let Decision::Wait { on, .. } = d
                {
                    // The owner answers what the work waits on, whatever the path spent.
                    for w in on {
                        let e = match w {
                            Waited::Answer { question } => Event::Answer {
                                question: question.clone(),
                                option: 1,
                            },
                            Waited::Call {
                                call,
                                on: Some(Who::Owner),
                            } => Event::Answers(*call),
                            _ => continue,
                        };
                        out.extend(self.event(s, d, e));
                    }
                }
            }
            _ => {}
        }
        out
    }

    /// The round under way of the run returns this; `cost` where the path spends on it, `always` where the cell gives
    /// it at every round from now on. None where the loop refused it.
    fn returns(&mut self, s: &State, cell: &CellId, run: RunId, v: Variant, cost: bool, always: bool) -> Option<State> {
        let m = self.model;
        let mut c = s.clone();
        self.reach(format!("{cell} returned {v}"), &s.trace, &s.work.records);
        let mut said = format!("{cell} returns {v}");
        if always {
            said.push_str(", and will at every round from now on");
            c.always.insert(cell.clone(), v.clone());
        }
        if cost {
            c.spent += 1;
        }
        c.trace = c.trace.then(c.spent, said);
        let i = m.place(cell);
        self.out.stats.told += 1;
        let told = told(self.flow(), &c.work.records, run).unwrap_or_default();
        let round = (c.work.records.rounds.iter()).filter(|e| e.run == run).count();
        let mut came = m.came(i, &v, &told, run, round);
        // The act that runs the tests the cells ask for hands in the result of each entry it gives, as the asking
        // cell's own block, and nothing else.
        if m.tests == Some(i) {
            came.writes = (decide::asked_of(self.flow(), &c.work.records, cell).into_iter())
                .map(|entry| Handed {
                    block: entry.result,
                    content: format!("The tests {} asked for passed.", entry.cell),
                    matters: true,
                    value: None,
                })
                .collect();
            let (trace, records) = (c.trace.clone(), c.work.records.clone());
            self.reach(GAVE_RESULTS.to_string(), &trace, &records);
        }
        let returned = Given::Returned {
            run,
            came,
            spent: Spent::default(),
        };
        self.give(&mut c, returned).then_some(c)
    }

    /// The writes from outside that may follow a wait or a rest.
    fn events(&self, s: &State, d: &Decision) -> Vec<Event> {
        let m = self.model;
        let r = &s.work.records;
        let reading = Reading::of_flow(self.flow(), r);
        let applies = |i: usize| reading.applies(&m.compiled.flow, i);
        let mut events = Vec::new();
        if !m.people.is_empty() {
            events.push(Event::Background);
        }
        let mut lines = BTreeSet::new();
        for (i, spec) in m.compiled.cells.iter().enumerate() {
            if m.spoken[i].is_some()
                && applies(i)
                && let Some(line) = &spec.line
                && lines.insert(line.clone())
            {
                events.push(Event::Speaks(line.clone()));
            }
        }
        if m.sources.contains_key(MAIN) {
            events.push(Event::Source(MAIN));
        }
        events.extend(m.sessions.keys().map(|line| Event::Session(line.clone())));
        // The tree runs no cell: once is enough to show it. A commit no round made moves the work's head, as a
        // person's, once.
        if s.spent == 0 {
            for source in [TREE, HEAD] {
                if m.sources.contains_key(source) {
                    events.push(Event::Source(source));
                }
            }
        }
        for c in r.called.iter().filter(|c| c.on == Some(Who::Owner)) {
            if !r.done.iter().any(|d| d.call == c.call) {
                events.push(Event::Answers(c.call));
            }
        }
        // The owner answers each question that stands as it recommends, or takes another option.
        for (name, (_, _, withdrawn, answered)) in questions(r) {
            if !withdrawn && !answered {
                for option in [1, 2] {
                    events.push(Event::Answer {
                        question: name.clone(),
                        option,
                    });
                }
            }
        }
        if let Decision::Wait { at, on } = d
            && on.iter().any(|w| matches!(w, Waited::Limit { .. }))
        {
            events.push(Event::LetsGo(at.clone()));
        }
        events
    }

    /// The outside writes, while the work is as the decision says; none where the loop refused it.
    fn event(&mut self, s: &State, d: &Decision, e: Event) -> Option<State> {
        let m = self.model;
        let mut c = s.clone();
        let label = e.to_string();
        let kind = kind_of(&e);
        self.reach(format!("outside: {kind}"), &s.trace, &s.work.records);
        if let (Event::Source(MAIN), Decision::Wait { at, .. }) = (&e, d)
            && Some(m.place(at)) == m.check
        {
            self.reach(MAIN_WHILE_THE_CHECK_WAITS.to_string(), &s.trace, &s.work.records);
        }
        c.spent += 1;
        c.trace = c.trace.then(c.spent, label);
        let before = c.work.records.clone();
        // A cell an answer enters, which no block it writes may run.
        let mut entered: Option<usize> = None;
        let clock = c.clock + 1000;
        let given: Vec<Given> = match &e {
            Event::Speaks(line) => vec![Given::Said {
                to: To::Line(line.clone()),
                words: format!("What the owner says to {line} at {clock}."),
            }],
            Event::Background => m.people.iter().map(|b| by_person(b, clock)).collect(),
            Event::Answers(call) => vec![Given::Done {
                call: *call,
                result: "Go on.".to_string(),
            }],
            Event::Answer { question, option } => {
                let latest = questions(&c.work.records).get(question).cloned();
                let (asker, version, _, _) = latest.expect("a question asked");
                entered = Some(m.place(&asker));
                vec![Given::Answer {
                    question: question.clone(),
                    version,
                    option: *option,
                    why: None,
                }]
            }
            Event::LetsGo(cell) => vec![Given::LetGo { cell: cell.clone() }],
            Event::Source(source) => vec![from_outside(&m.sources[source], None, clock)],
            Event::Session(line) => vec![from_outside(&m.sessions[line], None, clock)],
            Event::Dependencies => {
                let deps = m.sources.get(DEPENDENCIES).expect("the flow has the items' block");
                vec![from_outside(deps, Some("merged"), clock)]
            }
        };
        for g in given {
            if !self.give(&mut c, g) {
                return None;
            }
        }
        c.since = 0;
        c.ancestors.clear();
        let r = &c.work.records;
        let mut written: Vec<(BlockId, Seq)> = (r.written[before.written.len()..].iter())
            .map(|w| (w.block.clone(), w.at))
            .collect();
        written.extend((r.answers[before.answers.len()..].iter()).map(|a| (BlockId::from(""), a.at)));
        written.extend((r.done[before.done.len()..].iter()).map(|d| (BlockId::from(""), d.at)));
        let blocks: Vec<BlockId> = (written.iter())
            .filter(|(b, _)| !b.as_str().is_empty())
            .map(|(b, _)| b.clone())
            .collect();
        let mut reach = m.reach(&blocks);
        if let Some(i) = entered {
            reach.insert(i);
            reach.extend(m.reach(&m.cells()[i].outputs));
        }
        let mut taint = Taint {
            what: kind,
            blocks,
            reach,
            versions: written.iter().map(|(_, at)| *at).collect(),
            runs: BTreeSet::new(),
        };
        // The runs the loop started as it took the write, in order, run for it where something new to them is of it,
        // or of what a run for it wrote; each is checked as any run for it is.
        let mut steps: Vec<(Seq, Option<&decide::Started>, RunId)> = (r.started[before.started.len()..].iter())
            .map(|st| (st.at, Some(st), st.run))
            .chain((r.finished[before.finished.len()..].iter()).map(|f| (f.at, None, f.run)))
            .collect();
        steps.sort_by_key(|(at, _, _)| *at);
        let records = r.clone();
        for (at, started, run) in steps {
            match started {
                Some(st) => {
                    let i = m.place(&st.cell);
                    if self.tainted(&records.before(st.at), i, &st.because, &taint) {
                        taint.runs.insert(run);
                        self.rerun(&c.trace, &taint, i, &st.cell);
                    }
                }
                None if taint.runs.contains(&run) => {
                    taint.versions.insert(at);
                }
                None => {}
            }
        }
        c.outside = Some(taint);
        Some(c)
    }

    /// What the exploration keeps of a state to know it again: whatever the walk's next decisions, and the
    /// properties, read of it. Two states with the same key go on alike.
    fn key(&self, s: &State) -> u64 {
        let m = self.model;
        let flow = self.flow();
        let records = &s.work.records;
        let r = Reading::of_flow(flow, records);
        let mut h = DefaultHasher::new();
        for block in m.choices.keys() {
            match r.current(block) {
                Some(v) => {
                    v.value.hash(&mut h);
                    match &v.by {
                        Writer::Cell(cell, _) => cell.hash(&mut h),
                        Writer::Person => 1u8.hash(&mut h),
                        Writer::Outside => 2u8.hash(&mut h),
                    }
                }
                None => 0u8.hash(&mut h),
            }
        }
        for (i, cell) in flow.cells().iter().enumerate() {
            for block in &cell.runs_on {
                let new = r.new_to(&cell.id, block, NOW);
                let code: u8 = match (new.is_empty(), r.behind(flow, i, block, NOW)) {
                    (true, _) => 0,
                    (false, true) => 1,
                    (false, false) => 2,
                };
                code.hash(&mut h);
            }
            for call in r.owed(&cell.id, NOW) {
                r.done(call, NOW).hash(&mut h);
            }
            streaks(flow, records, &cell.id).hash(&mut h);
            r.pending(cell, NOW).hash(&mut h);
            let finished = (r.started.values()).any(|st| st.cell == cell.id && r.finished.contains_key(&st.run));
            finished.hash(&mut h);
            (records.let_go.iter())
                .filter(|l| l.cell == cell.id)
                .count()
                .hash(&mut h);
        }
        for st in records.started.iter().filter(|st| !r.finished.contains_key(&st.run)) {
            if records.abandoned.iter().any(|a| a.run == st.run) {
                continue;
            }
            st.cell.hash(&mut h);
            let began = records.rounds.iter().filter(|e| e.run == st.run).count();
            let ended = records.returned.iter().filter(|e| e.run == st.run).count();
            (began, ended).hash(&mut h);
            for cause in &st.because {
                match cause {
                    Cause::Changed { block, .. } => block.hash(&mut h),
                    Cause::Results { calls } => calls.len().hash(&mut h),
                    Cause::NoRecord => {}
                }
            }
            let cell = &flow.cells()[m.place(&st.cell)];
            for block in cell.runs_on.iter().chain(&cell.outputs) {
                let told = r.told(st.run, block);
                let newer = (r.of(block).iter()).any(|v| {
                    Some(v.at) > told && v.changed_meaning() && !matches!(&v.by, Writer::Cell(c, _) if *c == st.cell)
                });
                newer.hash(&mut h);
            }
            for c in records.called.iter().filter(|c| c.run == st.run) {
                r.done(c.call, NOW).hash(&mut h);
            }
        }
        for (name, (cell, _, withdrawn, answered)) in questions(records) {
            (name, cell, withdrawn, answered).hash(&mut h);
        }
        let last = &flow.cells()[flow.cells().len() - 1].id;
        (r.started.values())
            .any(|st| &st.cell == last && r.finished.contains_key(&st.run))
            .hash(&mut h);
        s.always.hash(&mut h);
        // Past a write from outside, what it reached: what it was, how far it can reach, which of what is new to each
        // cell it wrote, and which open runs run for it.
        if let Some(t) = &s.outside {
            (&t.what, &t.reach).hash(&mut h);
            for cell in flow.cells() {
                for block in &cell.runs_on {
                    let new = r.new_to(&cell.id, block, NOW);
                    new.iter().any(|v| t.versions.contains(&v.at)).hash(&mut h);
                }
            }
            for st in records.started.iter().filter(|st| !r.finished.contains_key(&st.run)) {
                t.runs.contains(&st.run).hash(&mut h);
            }
        }
        h.finish()
    }

    /// The properties of a run the loop started, on the records as they were just before it started.
    fn check_run(&mut self, s: &State, records: &Records, cell: &CellId, because: &[Cause], tainted: bool) {
        let m = self.model;
        let flow = &m.compiled.flow;
        let r = Reading::of_flow(flow, records);
        let i = m.place(cell);
        let def = &m.cells()[i];
        let trace = &s.trace;
        self.reach(ran(cell), trace, &s.work.records);
        self.count(Property::NewToIt);
        // 1: what runs it is its own trigger, new to it, and not its own write; nothing new to it is left out.
        for cause in because {
            match cause {
                Cause::NoRecord => {
                    self.breach(
                        Property::NewToIt,
                        format!("{cell} ran with no record as its reason"),
                        trace,
                        String::new(),
                    );
                }
                Cause::Changed { block, was, behind } => {
                    let trigger = m.triggers[i].get(block).cloned();
                    match &trigger {
                        Some(t) => {
                            self.reach(ran_on(cell, t), trace, &s.work.records);
                            if *behind {
                                self.reach(ran_back_on(cell, t), trace, &s.work.records);
                            }
                        }
                        None => {
                            let name = format!("{cell} ran on {block}, which is none of its triggers");
                            self.breach(Property::NewToIt, name, trace, String::new());
                        }
                    }
                    let new = r.new_to(cell, block, NOW);
                    // The cell that gives results is entered for an entry it put away as well, its cell applying again.
                    let taken_up = new.is_empty() && r.taken_up(flow, def, NOW).contains(block);
                    if taken_up {
                        self.reach(TAKEN_UP.to_string(), trace, &s.work.records);
                    } else if new.is_empty() {
                        let own = r
                            .of(block)
                            .last()
                            .is_some_and(|v| matches!(&v.by, Writer::Cell(c, _) if c == cell));
                        let name = if own {
                            format!("{cell} ran again on its own write of {block}")
                        } else {
                            format!("{cell} ran on {block}, which it had consumed")
                        };
                        self.breach(
                            Property::NewToIt,
                            name,
                            trace,
                            format!("consumed {:?}", r.consumed(cell, block, NOW)),
                        );
                    }
                    if *was != r.consumed(cell, block, NOW) || *behind != r.behind(flow, i, block, NOW) {
                        let name = format!("the walk read what {cell} consumed of {block} otherwise");
                        let what = format!(
                            "walk: was {was:?}, behind {behind}; reading: was {:?}, behind {}",
                            r.consumed(cell, block, NOW),
                            r.behind(flow, i, block, NOW)
                        );
                        self.breach(Property::NewToIt, name, trace, what);
                    }
                    self.check_group(s, i, block);
                }
                Cause::Results { calls } => {
                    self.reach(
                        format!("{cell} was entered with the answers it waited for"),
                        trace,
                        records,
                    );
                    let owed = r.owed(cell, NOW);
                    if owed != *calls || calls.iter().any(|c| !r.done(*c, NOW)) {
                        let name = format!("{cell} was entered for answers it was not owed or that are not in");
                        self.breach(Property::NewToIt, name, trace, format!("{calls:?}, owed {owed:?}"));
                    }
                }
            }
        }
        for reason in r.entering(flow, def, NOW) {
            if let Reason::Changed { block, .. } = reason
                && !because
                    .iter()
                    .any(|c| matches!(c, Cause::Changed { block: b, .. } if *b == block))
            {
                let name = format!("{cell} ran leaving out {block}, new to it");
                self.breach(Property::NewToIt, name, trace, String::new());
            }
        }
        // The cell entered is the first that something new runs: none before it that applies has something new, or a
        // precondition that does not hold. A cell that waits for the result of what it asked for holds the work: only
        // a cell that gives the result runs after it, whatever is new to the cells between.
        let mut held = false;
        let gated =
            !def.requires.is_empty() || Some(i) == m.check || m.cells()[..i].iter().any(|c| !c.requires.is_empty());
        if gated {
            self.count(Property::Gates);
        }
        for (j, before) in m.cells().iter().enumerate().take(i) {
            if !r.applies(flow, j) {
                if !r.reasons(before, NOW).is_empty() {
                    self.reach(skipped(&before.id), trace, &s.work.records);
                }
                continue;
            }
            if !held && !r.entering(flow, before, NOW).is_empty() {
                let name = format!("{cell} ran while {} before it had something new", before.id);
                self.breach(Property::NewToIt, name, trace, String::new());
            }
            if r.owed(&before.id, NOW).iter().any(|c| !r.done(*c, NOW)) {
                let name = format!("{cell} ran while {} before it waited for an answer", before.id);
                self.breach(Property::NewToIt, name, trace, String::new());
            }
            if r.pending(before, NOW) {
                if (before.asks.iter()).any(|asks| def.runs_on.contains(&asks.record)) {
                    held = true;
                    self.reach(RAN_FOR_A_WAIT.to_string(), trace, &s.work.records);
                } else {
                    let name = format!(
                        "{cell} ran while {} before it waited for the result of its tests",
                        before.id
                    );
                    self.breach(Property::NewToIt, name, trace, String::new());
                }
            }
            for key in &before.requires {
                if !r.key_holds(flow, key, j) {
                    let name = format!(
                        "{cell} ran while the gate {} of {} before it did not hold",
                        show_key(key),
                        before.id
                    );
                    self.breach(Property::Gates, name, trace, String::new());
                }
            }
        }
        // 2: its condition holds, and no cell whose condition wants another value of a block ran off the same version.
        if !def.applies.is_empty() || m.contradicting.iter().any(|(a, b, _)| *a == i || *b == i) {
            self.count(Property::Contradicting);
        }
        if !r.applies(flow, i) {
            self.breach(
                Property::Contradicting,
                format!("{cell} ran while its condition did not hold"),
                trace,
                String::new(),
            );
        }
        for (a, b, block) in &m.contradicting {
            let other = if *a == i {
                *b
            } else if *b == i {
                *a
            } else {
                continue;
            };
            let now = r.current(block).map(|v| v.at);
            let other_id = &m.cells()[other].id;
            let same = (r.started.values())
                .filter(|st| &st.cell == other_id)
                .any(|st| r.current_before(block, st.at).map(|v| v.at) == now);
            if same {
                let name = format!("{cell} and {other_id} both ran off the same version of {block}");
                self.breach(Property::Contradicting, name, trace, format!("version {now:?}"));
            }
        }
        // 6: each precondition holds; the check, only once every question put to the owner is answered.
        for key in &def.requires {
            if !r.key_holds(flow, key, i) {
                let name = format!("{cell} ran though its gate {} did not hold", show_key(key));
                self.breach(Property::Gates, name, trace, String::new());
            }
        }
        // The check runs only once the review of the change said yes: each judge that runs on a block the check
        // describes the work by has, as the current version of its verdict, another value than the refusal. Main is
        // written from the start, as the watch writes it, so that the check could run at any step.
        if Some(i) == m.check {
            let describes = &m.compiled.cells[i].describes;
            for judge in m
                .cells()
                .iter()
                .filter(|c| c.runs_on.iter().any(|b| describes.contains(b)))
            {
                for (verdict, refusal) in m.refusals.iter().filter(|(b, _)| judge.outputs.contains(b)) {
                    let said = r.current(verdict).and_then(|v| v.value.clone());
                    if said.is_none() || said.as_ref() == Some(refusal) {
                        let name = format!("{cell} ran before {} said yes to the change in {verdict}", judge.id);
                        self.breach(Property::Gates, name, trace, String::new());
                    }
                }
            }
            for (name, (asker, _, withdrawn, answered)) in questions(records) {
                if !withdrawn && !answered {
                    let applies = r.applies(flow, m.place(&asker));
                    let breach = format!(
                        "{cell} ran while the question {name} of {asker} waited for the owner, {asker} being {}",
                        if applies { "on" } else { "skipped" }
                    );
                    self.breach(Property::Gates, breach, trace, String::new());
                }
            }
        }
        // The loops of an opinion and its reply.
        for (o, reply) in m.loops() {
            let replies = because
                .iter()
                .any(|c| matches!(c, Cause::Changed { block, .. } if *block == reply));
            let answered = (r.started.values()).any(|st| {
                m.place(&st.cell) < i
                    && st
                        .because
                        .iter()
                        .any(|c| matches!(c, Cause::Changed { block, .. } if *block == o))
                    && r.finished
                        .get(&st.run)
                        .is_some_and(|f| f.writes.iter().any(|w| w.block == reply))
            });
            if replies && answered {
                self.reach(reply_loop(&o, &reply), trace, &s.work.records);
            }
        }
        // 7: what a write from outside reaches.
        if let Some(t) = &s.outside
            && tainted
        {
            self.rerun(trace, t, i, cell);
        }
    }

    /// 7, of a run for what a write from outside reached: its change reaches the cell.
    fn rerun(&mut self, trace: &Trace, t: &Taint, i: usize, cell: &CellId) {
        self.count(Property::Incremental);
        let what = t.what.clone();
        self.out.reruns.entry(what.clone()).or_default().insert(cell.clone());
        if !t.reach.contains(&i) {
            let name = format!("{what} ran {cell} again, which its change does not reach");
            self.breach(Property::Incremental, name, trace, String::new());
        }
    }

    /// 5, of a run: a block of a group that runs the cell is one of the group's.
    fn check_group(&mut self, s: &State, i: usize, block: &BlockId) {
        let m = self.model;
        if let Some(trigger) = m.triggers[i].get(block)
            && let Some(members) = m.groups[i].get(trigger)
        {
            self.count(Property::Groups);
            if !members.contains(block) {
                let name = format!("{} ran on {trigger} for {block}, none of its members", m.cells()[i].id);
                self.breach(Property::Groups, name, &s.trace, String::new());
            }
        }
    }

    /// What is wrong with how a stall is shown, by the reading: its keys are those of where it stands that do not hold,
    /// or, with none, it stands at its last cell; nothing before where it stands, or anywhere where it stands at its
    /// last cell, has anything new; and each cell it names writes what a key reads, or what runs its last cell, and
    /// does not run for the reason it gives.
    fn shown_wrongly(&self, s: &State, at: &CellId, on: &[Condition], by: &[Stalling]) -> Vec<String> {
        let m = self.model;
        let flow = &m.compiled.flow;
        let cells = m.cells();
        let r = Reading::of_flow(flow, &s.work.records);
        let i = m.place(at);
        let mut wrong = Vec::new();
        let unmet: Vec<&Condition> = (cells[i].requires.iter())
            .filter(|key| !r.key_holds(flow, key, i))
            .collect();
        if unmet.len() != on.len() || !on.iter().all(|key| unmet.contains(&key)) {
            wrong.push(format!("the keys that do not hold are {unmet:?}"));
        }
        if on.is_empty() && i != cells.len() - 1 {
            wrong.push("it waits on nothing short of its last cell".to_string());
        }
        let upto = if on.is_empty() { cells.len() } else { i };
        for (j, cell) in cells.iter().enumerate().take(upto) {
            if r.applies(flow, j) && !r.entering(flow, cell, NOW).is_empty() {
                wrong.push(format!("{} has something new", cell.id));
            }
        }
        let read: Vec<BlockId> = if on.is_empty() {
            cells[i].runs_on.clone()
        } else {
            (on.iter())
                .flat_map(|key| match key {
                    Condition::Test(test) => flow.tests()[test].reads.clone(),
                    Condition::Is { block, .. } => vec![block.clone()],
                })
                .collect()
        };
        for stalling in by {
            let j = m.place(&stalling.cell);
            let why = if j >= i {
                Unrun::Held
            } else if !r.applies(flow, j) {
                let on = (cells[j].applies.iter())
                    .filter(|key| !r.key_holds(flow, key, j))
                    .cloned()
                    .collect();
                Unrun::Skipped { on }
            } else {
                Unrun::NothingNew
            };
            if stalling.why != why || !cells[j].outputs.iter().any(|b| read.contains(b)) {
                wrong.push(format!("{} is said {:?}, and is {why:?}", stalling.cell, stalling.why));
            }
        }
        wrong
    }

    /// 3, and 7 where the outside wrote: the work waits, is stalled, which is shown as waiting for the owner, or has
    /// ended.
    fn check_stop(&mut self, s: &State, d: &Decision) {
        let m = self.model;
        let flow = &m.compiled.flow;
        let trace = &s.trace;
        let records = &s.work.records;
        self.count(Property::NextStep);
        let (stop, on_purpose) = match d {
            Decision::Ended(End::Merged) => ("the work merged".to_string(), true),
            Decision::Ended(End::Cancelled) => ("the work was cancelled".to_string(), true),
            Decision::Stalled { at, on, by } => {
                for key in on {
                    self.reach(waited_at_gate(at, key), trace, records);
                }
                self.reach(STALLED_SHOWN.to_string(), trace, records);
                self.count(Property::Shown);
                let stop = format!(
                    "the work is stalled at {at} on {:?}, its writers {by:?}",
                    on.iter().map(show_key).collect::<Vec<_>>()
                );
                let wrong = self.shown_wrongly(s, at, on, by);
                if !wrong.is_empty() {
                    self.breach(Property::Shown, stop.clone(), trace, wrong.join("; "));
                }
                (stop, true)
            }
            Decision::Wait { at, on } => {
                for w in on {
                    if let Waited::Precondition { on: key, .. } = w {
                        self.reach(waited_at_gate(at, key), trace, records);
                    }
                }
                let shown: Vec<String> = on.iter().map(show_waited).collect();
                let on_purpose = on.iter().any(|w| match w {
                    Waited::Call { on, .. } => on.is_some(),
                    Waited::Limit { .. } | Waited::Answer { .. } => true,
                    // The cells that give a result run while their asking cell waits: a work that waits for one
                    // none of them gives has no next step.
                    Waited::Result { .. } => false,
                    Waited::Precondition { by, .. } => by
                        .iter()
                        .any(|maker| matches!(maker, Maker::Owner | Maker::Outside { .. })),
                    Waited::Outside { .. } => true,
                    Waited::Slot => false,
                });
                (
                    format!("the work waits at {at} for {}", shown.join(" and ")),
                    on_purpose,
                )
            }
            _ => return,
        };
        self.reach(format!("stop: {stop}"), trace, records);
        if on_purpose {
            if !matches!(d, Decision::Ended(_)) {
                self.reach(WAITED_FOR_THE_OWNER.to_string(), trace, records);
            }
        } else {
            self.breach(Property::NextStep, stop, trace, String::new());
        }
        if let Decision::Wait { on, .. } = d
            && on.iter().any(|w| matches!(w, Waited::Answer { .. }))
        {
            self.reach(A_QUESTION_WAITED.to_string(), trace, records);
        }
        // 7: the write reached each cell it runs before where the work stops.
        let Some(t) = &s.outside else {
            return;
        };
        // A wait for an answer or a result holds its cell and the work (decision a-wait-holds-its-cell): what the
        // write is new to runs once the wait ends, and is checked at the stop after it.
        if let Decision::Wait { on, .. } = d
            && on
                .iter()
                .any(|w| matches!(w, Waited::Answer { .. } | Waited::Result { .. }))
        {
            return;
        }
        self.count(Property::Incremental);
        let r = Reading::of_flow(flow, records);
        let upto = match d {
            Decision::Wait { at, .. } => m.place(at),
            Decision::Stalled { at, on, .. } if !on.is_empty() => m.place(at),
            _ => m.cells().len(),
        };
        let mut missed = Vec::new();
        for (j, cell) in m.cells().iter().enumerate().take(upto) {
            if !r.applies(flow, j) {
                continue;
            }
            for block in t.blocks.iter().filter(|b| cell.runs_on.contains(*b)) {
                if !r.new_to(&cell.id, block, NOW).is_empty() {
                    missed.push(format!(
                        "{} did not reach {}, which it runs, before the work stopped",
                        t.what, cell.id
                    ));
                }
            }
        }
        for name in missed {
            self.breach(Property::Incremental, name, trace, String::new());
        }
    }
}

/// Of the cell's runs that finished since its counts last began anew and dealt with something, how many each block
/// ran: what the walk's limit counts, by the cell whose writes sent each (decision the-same-sender-again).
fn streaks(flow: &decide::Flow, records: &Records, cell: &CellId) -> BTreeMap<BlockId, usize> {
    let from = decide::again_from(flow, records, cell);
    let mut counts: BTreeMap<BlockId, usize> = BTreeMap::new();
    for st in records
        .started
        .iter()
        .filter(|st| &st.cell == cell && from.is_none_or(|from| st.at > from))
    {
        let Some(f) = records.finished.iter().find(|f| f.run == st.run) else {
            continue;
        };
        if f.dealt_with.as_ref().is_some_and(Vec::is_empty) {
            continue;
        }
        for cause in &st.because {
            if let Cause::Changed { block, .. } = cause {
                *counts.entry(block.clone()).or_default() += 1;
            }
        }
    }
    counts
}

fn show_waited(w: &Waited) -> String {
    match w {
        Waited::Call { on: Some(who), .. } => format!("an answer from the {}", format!("{who:?}").to_lowercase()),
        Waited::Call { on: None, .. } => "the result of a tool".to_string(),
        Waited::Precondition { on, by } => {
            let by: Vec<String> = (by.iter())
                .map(|maker| match maker {
                    Maker::Owner => "the owner".to_string(),
                    Maker::Outside { source } => format!("the outside ({source})"),
                    Maker::Cell { cell } => cell.to_string(),
                })
                .collect();
            format!("{}, which {} can make hold", show_key(on), by.join(" or "))
        }
        Waited::Limit { by, runs } => format!("the owner, at its limit of {runs} runs sent by {by}"),
        Waited::Result { record, .. } => format!("the result of what it asked for in {record}"),
        Waited::Answer { question } => format!("the owner's answer to {question}"),
        Waited::Outside { source, block, .. } => format!("the outside ({source}), of which {block} waits"),
        Waited::Slot => "a slot".to_string(),
    }
}

/// What an outside write is, apart from the call or the question it answers.
fn kind_of(e: &Event) -> String {
    match e {
        Event::Answers(_) | Event::Answer { option: 1, .. } => "the owner answers".to_string(),
        Event::Answer { .. } => "the owner takes another option".to_string(),
        e => e.to_string(),
    }
}

/// What the loop refused, apart from what it was about.
fn refused_kind(why: &Refused) -> String {
    let said = format!("{why:?}");
    said.split(['(', ' ', '{']).next().unwrap_or_default().to_string()
}

/// The items of coverage that are no cell's.
pub const MAIN_WHILE_THE_CHECK_WAITS: &str = "main moved while the check waited at its gate";
pub const WAITED_FOR_THE_OWNER: &str = "the work waited for the owner or the outside";
pub const STALLED_SHOWN: &str = "a stalled work was shown as waiting for the owner";
pub const A_QUESTION_WAITED: &str = "a question waited for the owner";
pub const GAVE_RESULTS: &str = "the act that runs tests gave the results of what the cells asked for";
pub const RAN_FOR_A_WAIT: &str = "a cell ran to give the result a cell before it waited for";
pub const TAKEN_UP: &str = "the cell that gives results ran for an entry put away while its cell was skipped";
pub const WRITTEN_OVER: &str = "a return was sent back as it would write over what its round was not told of";
