//! The exploration: from the work as the owner starts it, every way it can go within the bound, breadth first. A state
//! is the work's records, kept by the helper the deciding code's tests share, with the rounds of each run as the
//! engine's check of a return reads them, the questions put to the owner and what was spent. Between two states the
//! engine does all that is its own to do (it starts the run the walk says to, takes a return and goes on with a run,
//! abandons one), so that a state the search keeps is one where something else happens next: a round under way
//! returns, the outside writes, or the work waits or rests and only the outside can write.
//!
//! The properties are checked at each decision of the walk, by the histories' own reading of the records, never by
//! the walk's. A breach is kept under a name that says what went wrong without the path, the first path to it beside
//! it; the search is breadth first, so that path is among the shortest.

use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;
use std::time::{Duration, Instant};

use decide::run::{self, Hold, Requires, Step};
use decide::{
    Abandoned, BlockId, CallId, Called, Cause, CellId, Decision, Done, End, Finished, LetGo, Maker, PutIn, Records,
    Returned, Round, RunId, Seq, Started, Waited, Who, decide, told,
};

use crate::model::{
    ANSWERS, BACKGROUND, DEPENDENCIES, DISCUSSION, Event, MAIN, Model, SESSION, START, TREE, Variant, ran, ran_back_on,
    ran_on, reply_loop, show_key, skipped, waited_at_gate,
};
use crate::reading::{Reading, Reason, Writer};
use crate::support::World;

/// The properties, by their numbers in the owner's test plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Property {
    /// 1. A cell runs only on a version of one of its own triggers that it has not consumed and did not write.
    NewToIt = 1,
    /// 2. Cells whose conditions want another value of a block never both run off the same version of it.
    Contradicting = 2,
    /// 3. Every state has a next step, or is a stop on purpose: the owner or the outside is waited for, or the work
    ///    merged.
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
    /// What the exploration itself could not do as the engine does: a step the engine's own code refused.
    Engine = 0,
}

/// The bounds of the exploration.
#[derive(Clone, Copy)]
pub struct Bounds {
    /// How many returns other than the content, and writes from outside, a path may take.
    pub spend: u8,
    /// The most a path may have spent before the outside writes while the work waits or rests.
    pub outside_after: u8,
    /// The most decisions of the walk in a row with nothing written from outside.
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
    /// The decisions of the walk taken, and the calls of the engine's reading of what a round was told.
    pub decisions: u64,
    pub told: u64,
    /// The runs started.
    pub runs: u64,
    /// The most decisions in a row with nothing written from outside.
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

/// The steps of a path, shared between the states that follow it.
#[derive(Clone, Default)]
struct Trace(Option<Rc<Link>>);

struct Link {
    step: String,
    before: Trace,
    /// What the path had spent at this step, and its steps up to it.
    spent: u8,
    len: u32,
}

impl Trace {
    fn then(&self, spent: u8, step: String) -> Trace {
        let len = self.0.as_ref().map_or(0, |l| l.len) + 1;
        Trace(Some(Rc::new(Link {
            step,
            before: self.clone(),
            spent,
            len,
        })))
    }

    /// How far the path is from the start: what it spent, then its steps. Of two paths to one breach, the nearer is
    /// kept.
    fn rank(&self) -> (u8, u32) {
        self.0.as_ref().map_or((0, 0), |l| (l.spent, l.len))
    }

    fn steps(&self) -> Vec<String> {
        let mut steps = Vec::new();
        let mut at = self.0.clone();
        while let Some(link) = at {
            steps.push(link.step.clone());
            at = link.before.0.clone();
        }
        steps.reverse();
        steps
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
    world: World,
    /// The rounds of each run, as the engine's check of a return reads them.
    logs: BTreeMap<RunId, run::Log>,
    /// What the round of each run that has ended and was not yet taken returned.
    returning: BTreeMap<RunId, Variant>,
    /// The cell whose question each call on the owner asks; a call on the owner not here is the owner's word at a
    /// run's limit of returns.
    questions: BTreeMap<CallId, CellId>,
    /// The return each cell gives at every round from now on, where one was chosen for all.
    always: BTreeMap<CellId, Variant>,
    spent: u8,
    /// The outside wrote on this path, and what that reached.
    outside: Option<Taint>,
    /// The decisions since the outside last wrote, and the states kept since then.
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

pub fn explore(model: &Model, bounds: Bounds) -> Explored {
    let began = Instant::now();
    let mut e = Explorer {
        model,
        bounds,
        out: Explored::default(),
    };
    e.static_checks();
    let mut seen: HashMap<u64, u8> = HashMap::new();
    let mut queue = VecDeque::from([e.first()]);
    while let Some(state) = queue.pop_front() {
        if e.out.stats.states >= bounds.states {
            let name = format!("the search kept {} states, its most, and stopped", bounds.states);
            e.breach(Property::Engine, name, &state.trace, String::new());
            break;
        }
        let Some((mut state, decision)) = e.settle(state) else {
            continue;
        };
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

impl Explorer<'_> {
    fn flow(&self) -> &decide::Flow {
        &self.model.compiled.flow
    }

    fn reach(&mut self, what: String) {
        *self.out.reached.entry(what).or_default() += 1;
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

    /// The work as the owner starts it: the start written, the background written, and the items the work depends on
    /// not yet merged.
    fn first(&self) -> State {
        let m = self.model;
        let mut world = World::of(m.compiled.flow.clone());
        world.outside_writes(m.sources[&START].as_str());
        world.person_writes(BACKGROUND);
        world.outside_sets(m.sources[&DEPENDENCIES].as_str(), "waiting");
        State {
            world,
            logs: BTreeMap::new(),
            returning: BTreeMap::new(),
            questions: BTreeMap::new(),
            always: BTreeMap::new(),
            spent: 0,
            outside: None,
            since: 0,
            ancestors: Vec::new(),
            trace: Trace::default().then(0, "the owner starts the work: the background is written".to_string()),
        }
    }

    /// Does what is the engine's own to do, deciding again after each, until a round is under way, or the work waits,
    /// rests or has ended; none where the path breaks the bound of steps.
    fn settle(&mut self, mut s: State) -> Option<(State, Decision)> {
        loop {
            let d = decide(self.flow(), &s.world.records);
            self.out.stats.decisions += 1;
            s.since += 1;
            self.out.stats.longest = self.out.stats.longest.max(s.since);
            if s.since > self.bounds.steps {
                let name = format!(
                    "no stop within {} decisions with nothing written from outside",
                    self.bounds.steps
                );
                self.breach(Property::ComesToRest, name, &s.trace, format!("{d:?}"));
                return None;
            }
            self.check(&s, &d);
            match d {
                Decision::Run { cell, because, basis } => {
                    let run = self.start(&mut s, cell.clone(), because, basis);
                    return Some((s, Decision::Busy { cell, run }));
                }
                Decision::GoOn { cell, run } => {
                    if !self.go_on(&mut s, &cell, run) {
                        return None;
                    }
                }
                Decision::Abandon { cell, run, because } => {
                    s.trace = s
                        .trace
                        .then(s.spent, format!("the engine abandons the run of {cell}: {because:?}"));
                    let r = &s.world.records;
                    let began = r.rounds.iter().filter(|e| e.run == run).count();
                    let ended = r.returned.iter().filter(|e| e.run == run).count();
                    if began > ended {
                        let at = s.world.tick();
                        s.world.records.returned.push(Returned { run, at });
                    }
                    let at = s.world.tick();
                    s.world.records.abandoned.push(Abandoned { run, at });
                    s.returning.remove(&run);
                }
                Decision::Fault(fault) => {
                    self.breach(
                        Property::Engine,
                        format!("the records are faulty: {fault:?}"),
                        &s.trace,
                        String::new(),
                    );
                    return None;
                }
                d => return Some((s, d)),
            }
        }
    }

    /// Starts the run the walk says to, with exactly what the decision gives, and begins its first round.
    fn start(
        &mut self,
        s: &mut State,
        cell: CellId,
        because: Vec<Cause>,
        basis: BTreeMap<BlockId, Option<Seq>>,
    ) -> RunId {
        self.out.stats.runs += 1;
        let i = self.model.place(&cell);
        let names: Vec<String> = (because.iter())
            .map(|c| match c {
                Cause::Changed { block, .. } => self.model.triggers[i].get(block).cloned().unwrap_or(block.0.clone()),
                Cause::Results { .. } => "the answers it waited for".to_string(),
                Cause::NoRecord => "no record".to_string(),
            })
            .collect();
        let mut names: Vec<String> = names.into_iter().collect::<BTreeSet<_>>().into_iter().collect();
        names.sort();
        s.trace = s
            .trace
            .then(s.spent, format!("{cell} starts, for {}", names.join(", ")));
        let run = RunId(s.world.id());
        let tainted = s.outside.as_ref().is_some_and(|t| self.tainted(s, i, &because, t));
        let at = s.world.tick();
        s.world.records.started.push(Started {
            run,
            cell,
            at,
            because,
            basis,
        });
        let at = s.world.tick();
        s.world.records.rounds.push(Round { run, at });
        s.logs.insert(
            run,
            run::Log {
                rounds: vec![run::Round { returned: None }],
                let_go: 0,
                let_on: 0,
            },
        );
        if tainted && let Some(t) = s.outside.as_mut() {
            t.runs.insert(run);
        }
        run
    }

    /// Whether a run of the cell at this place for these reasons runs for what the write from outside reached.
    fn tainted(&self, s: &State, i: usize, because: &[Cause], taint: &Taint) -> bool {
        let r = Reading::new(&s.world.records);
        let cell = &self.model.cells()[i].id;
        because.iter().any(|cause| match cause {
            Cause::Changed { block, .. } => (r.new_to(cell, block, NOW).iter()).any(|v| taint.versions.contains(&v.at)),
            Cause::Results { calls } => {
                (s.world.records.done.iter()).any(|d| calls.contains(&d.call) && taint.versions.contains(&d.at))
            }
            Cause::NoRecord => false,
        })
    }

    /// Goes on with the run the walk says to: takes what its last round returned as the engine's check of a return
    /// says, finishing the run, sending the return back or asking the owner at the limit of returns.
    fn go_on(&mut self, s: &mut State, cell: &CellId, run: RunId) -> bool {
        let m = self.model;
        let i = m.place(cell);
        self.out.stats.told += 1;
        let Some(told) = told(self.flow(), &s.world.records, run) else {
            self.breach(
                Property::Engine,
                format!("the walk goes on with a run of {cell} it cannot read"),
                &s.trace,
                String::new(),
            );
            return false;
        };
        let Some(log) = s.logs.get_mut(&run) else {
            return false;
        };
        let mut asks = false;
        let mut variant = None;
        if let Some(last) = log.rounds.last_mut()
            && last.returned.is_none()
        {
            let Some(v) = s.returning.remove(&run) else {
                self.breach(
                    Property::Engine,
                    format!("the walk goes on with {cell}, whose round is under way"),
                    &s.trace,
                    String::new(),
                );
                return false;
            };
            let (ret, a) = m.hand_in(i, &v, &told);
            last.returned = Some(ret);
            asks = a;
            variant = Some(v);
        }
        let spec = &m.compiled.cells[i];
        let requires = Requires {
            outputs: &m.cells()[i].outputs,
            must_write: &spec.must_write,
            choices: &m.choices,
            returns: spec.returns,
            told: &told,
        };
        let step = run::next(log, requires, Hold::Nothing);
        let sent_back = variant.as_ref() == Some(&Variant::SentBack);
        match step {
            Step::Finish { dealt_with, writes } if !sent_back => {
                let shown: Vec<String> = (writes.iter())
                    .map(|w| match &w.value {
                        Some(v) => format!("{} = {v}", w.block),
                        None => w.block.to_string(),
                    })
                    .collect();
                let mut said = format!(
                    "the engine takes it: {cell} writes {}",
                    if shown.is_empty() {
                        "nothing".to_string()
                    } else {
                        shown.join(", ")
                    }
                );
                if asks {
                    let call = CallId(s.world.id());
                    let at = s.world.tick();
                    s.world.records.called.push(Called {
                        call,
                        run,
                        on: Some(Who::Owner),
                        at,
                    });
                    s.questions.insert(call, cell.clone());
                    said.push_str(&format!(", and the owner is asked, the call {}", call.0));
                }
                s.trace = s.trace.then(s.spent, said);
                let at = s.world.tick();
                s.world.records.finished.push(Finished {
                    run,
                    at,
                    writes,
                    dealt_with: Some(dealt_with),
                });
                if let Some(t) = s.outside.as_mut()
                    && t.runs.contains(&run)
                {
                    t.versions.insert(at);
                }
            }
            // A return is sent back where it would write over a version its round was not told of: a cell before it
            // ran while the return waited to be taken, and wrote what this one writes.
            Step::SendBack { unmet }
                if variant.is_none()
                    || sent_back
                    || unmet.iter().all(|u| matches!(u, run::Unmet::WrittenOver { .. })) =>
            {
                self.reach("a return was sent back".to_string());
                if !sent_back && variant.is_some() {
                    self.reach(WRITTEN_OVER.to_string());
                }
                s.trace = s
                    .trace
                    .then(s.spent, format!("the engine sends {cell}'s return back: {unmet:?}"));
                let at = s.world.tick();
                s.world.records.rounds.push(Round { run, at });
                for u in &unmet {
                    if let run::Unmet::WrittenOver { block, version } = u {
                        let at = s.world.tick();
                        s.world.records.put_in.push(PutIn {
                            run,
                            block: block.clone(),
                            version: *version,
                            at,
                        });
                    }
                }
                log.rounds.push(run::Round { returned: None });
            }
            Step::AtLimit(limit) => {
                self.reach("a run reached its limit of returns sent back".to_string());
                s.trace = s.trace.then(
                    s.spent,
                    format!("{cell}'s run is at its limit {limit:?}: the owner is asked"),
                );
                let call = CallId(s.world.id());
                let at = s.world.tick();
                s.world.records.called.push(Called {
                    call,
                    run,
                    on: Some(Who::Owner),
                    at,
                });
            }
            step => {
                let name = format!(
                    "the engine's check of a return of {cell} judged {} otherwise",
                    variant.map_or("a return".to_string(), |v| v.to_string())
                );
                self.breach(Property::Engine, name, &s.trace, format!("{step:?}"));
                return false;
            }
        }
        true
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
                    Some(v) => out.push(self.returns(s, cell, *run, v.clone(), false, false)),
                    None => {
                        out.push(self.returns(s, cell, *run, Variant::Content, false, false));
                        if may_spend {
                            self.out.stats.told += 1;
                            let told = told(self.flow(), &s.world.records, *run).unwrap_or_default();
                            for (v, may_always) in m.deviations(i, &told) {
                                out.push(self.returns(s, cell, *run, v.clone(), true, false));
                                if may_always {
                                    out.push(self.returns(s, cell, *run, v, true, true));
                                }
                            }
                        }
                    }
                }
                // While a round is under way, the outside writes what runs its cell, or the background.
                if s.spent == 0 && s.outside.is_none() {
                    let cells = m.cells();
                    let mut events = vec![Event::Background];
                    if m.spoken[i].is_some() {
                        events.push(Event::Speaks(cell.clone()));
                    }
                    for (source, block) in &m.sources {
                        if [MAIN, SESSION].contains(source) && cells[i].runs_on.contains(block) {
                            events.push(Event::Source(source));
                        }
                    }
                    for e in events {
                        out.push(self.event(s, d, e));
                    }
                }
            }
            Decision::Wait { .. } | Decision::Idle => {
                let waiting = (s.world.records.written.iter())
                    .rev()
                    .find(|w| w.block == m.sources[&DEPENDENCIES])
                    .is_some_and(|w| w.value.as_deref() != Some("merged"));
                if waiting {
                    let mut child = self.event(s, d, Event::Dependencies);
                    child.outside = None;
                    child.spent = s.spent;
                    child.since = 0;
                    child.ancestors.clear();
                    out.push(child);
                }
                if s.outside.is_none() && s.spent <= self.bounds.outside_after && s.spent < self.bounds.spend {
                    for e in self.events(s, d) {
                        out.push(self.event(s, d, e));
                    }
                } else if s.outside.is_none()
                    && let Decision::Wait { on, .. } = d
                {
                    // The owner answers a question the work waits on, whatever the path spent.
                    for w in on {
                        if let Waited::Call {
                            call,
                            on: Some(Who::Owner),
                        } = w
                            && s.questions.contains_key(call)
                        {
                            out.push(self.event(s, d, Event::Answers(*call)));
                        }
                    }
                }
            }
            _ => {}
        }
        out
    }

    /// The round under way of the run returns this; `cost` where the path spends on it, `always` where the cell gives
    /// it at every round from now on.
    fn returns(&mut self, s: &State, cell: &CellId, run: RunId, v: Variant, cost: bool, always: bool) -> State {
        let mut c = s.clone();
        self.reach(format!("{cell} returned {v}"));
        let mut said = format!("{cell} returns {v}");
        if always {
            said.push_str(", and will at every round from now on");
            c.always.insert(cell.clone(), v.clone());
        }
        if cost {
            c.spent += 1;
        }
        c.trace = c.trace.then(c.spent, said);
        let at = c.world.tick();
        c.world.records.returned.push(Returned { run, at });
        c.returning.insert(run, v);
        c
    }

    /// The writes from outside that may follow a wait or a rest.
    fn events(&self, s: &State, d: &Decision) -> Vec<Event> {
        let m = self.model;
        let r = &s.world.records;
        let reading = Reading::new(r);
        let applies = |i: usize| reading.applies(&m.compiled.flow, i);
        let mut events = vec![Event::Background];
        for (i, cell) in m.cells().iter().enumerate() {
            if m.spoken[i].is_some() && applies(i) {
                events.push(Event::Speaks(cell.id.clone()));
            }
        }
        events.extend([MAIN, SESSION].map(Event::Source));
        // The tree runs no cell: once is enough to show it.
        if s.spent == 0 {
            events.push(Event::Source(TREE));
        }
        for c in r.called.iter().filter(|c| c.on == Some(Who::Owner)) {
            if !r.done.iter().any(|d| d.call == c.call) {
                events.push(Event::Answers(c.call));
            }
        }
        for (i, cell) in m.cells().iter().enumerate() {
            let finished =
                (r.started.iter()).any(|st| st.cell == cell.id && r.finished.iter().any(|f| f.run == st.run));
            if m.answered[i].is_some() && finished && applies(i) {
                events.push(Event::OtherOption(cell.id.clone()));
            }
        }
        if let Decision::Wait { at, on } = d
            && on
                .iter()
                .any(|w| matches!(w, Waited::Limit { .. } | Waited::Calls { .. }))
        {
            events.push(Event::LetsGo(at.clone()));
        }
        events
    }

    /// The outside writes, while the work is as the decision says.
    fn event(&mut self, s: &State, d: &Decision, e: Event) -> State {
        let m = self.model;
        let mut c = s.clone();
        let label = e.to_string();
        self.reach(format!("outside: {}", kind_of(&e)));
        if let (Event::Source(MAIN), Decision::Wait { at, .. }) = (&e, d)
            && m.place(at) == m.check
        {
            self.reach(MAIN_WHILE_THE_CHECK_WAITS.to_string());
        }
        let w = &mut c.world;
        let mut written: Vec<(BlockId, Seq)> = Vec::new();
        // A cell an answer enters, which no block it writes may run.
        let mut entered: Option<usize> = None;
        let mut answer: Option<Seq> = None;
        let person = |w: &mut World, block: &BlockId, written: &mut Vec<(BlockId, Seq)>| {
            let at = w.person_writes(block.as_str());
            written.push((block.clone(), at));
        };
        match &e {
            Event::Speaks(cell) => {
                let i = m.place(cell);
                let block = m.spoken[i].clone().expect("the cell is spoken to");
                person(w, &block, &mut written);
                person(w, &DISCUSSION.into(), &mut written);
            }
            Event::Background => person(w, &BACKGROUND.into(), &mut written),
            Event::Answers(call) => {
                let at = w.tick();
                w.records.done.push(Done { call: *call, at });
                answer = Some(at);
                match c.questions.get(call).cloned() {
                    Some(asker) => {
                        entered = Some(m.place(&asker));
                        person(w, &ANSWERS.into(), &mut written);
                        if let Some(block) = m.answered[m.place(&asker)].clone() {
                            person(w, &block, &mut written);
                        }
                    }
                    None => {
                        // The owner's word at a run's limit: the run goes on, the counts starting anew.
                        let run = w.records.called.iter().find(|x| x.call == *call).map(|x| x.run);
                        if let Some(log) = run.and_then(|run| c.logs.get_mut(&run)) {
                            log.let_go = log.rounds.len();
                        }
                    }
                }
            }
            Event::OtherOption(cell) => {
                person(w, &ANSWERS.into(), &mut written);
                let block = m.answered[m.place(cell)].clone().expect("the cell is answered");
                person(w, &block, &mut written);
            }
            Event::LetsGo(cell) => {
                let at = w.tick();
                w.records.let_go.push(LetGo { cell: cell.clone(), at });
            }
            Event::Source(source) => {
                let block = m.sources[source].clone();
                let at = w.outside_writes(block.as_str());
                written.push((block, at));
            }
            Event::Dependencies => {
                let block = m.sources[&DEPENDENCIES].clone();
                let at = w.outside_sets(block.as_str(), "merged");
                written.push((block, at));
            }
        }
        c.spent += 1;
        c.trace = c.trace.then(c.spent, label.clone());
        c.since = 0;
        c.ancestors.clear();
        let blocks: Vec<BlockId> = written.iter().map(|(b, _)| b.clone()).collect();
        let mut reach = m.reach(&blocks);
        if let Some(i) = entered {
            reach.insert(i);
            reach.extend(m.reach(&m.cells()[i].outputs));
        }
        c.outside = Some(Taint {
            what: kind_of(&e),
            blocks,
            reach,
            versions: written.iter().map(|(_, at)| *at).chain(answer).collect(),
            runs: BTreeSet::new(),
        });
        c
    }

    /// What the exploration keeps of a state to know it again: whatever the walk's next decisions, and the
    /// properties, read of it. Two states with the same key go on alike.
    fn key(&self, s: &State) -> u64 {
        let m = self.model;
        let flow = self.flow();
        let records = &s.world.records;
        let r = Reading::new(records);
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
                (r.done(call, NOW), s.questions.contains_key(&call)).hash(&mut h);
            }
            streaks(records, &cell.id).hash(&mut h);
            let finished = (r.started.values()).any(|st| st.cell == cell.id && r.finished.contains_key(&st.run));
            finished.hash(&mut h);
        }
        for st in records.started.iter().filter(|st| !r.finished.contains_key(&st.run)) {
            if records.abandoned.iter().any(|a| a.run == st.run) {
                continue;
            }
            st.cell.hash(&mut h);
            let began = records.rounds.iter().filter(|e| e.run == st.run).count();
            let ended = records.returned.iter().filter(|e| e.run == st.run).count();
            (began, ended).hash(&mut h);
            if let Some(log) = s.logs.get(&st.run) {
                (log.rounds.len(), log.let_go).hash(&mut h);
            }
            s.returning.get(&st.run).hash(&mut h);
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

    /// The properties, at a decision the walk took on the state.
    fn check(&mut self, s: &State, d: &Decision) {
        match d {
            Decision::Run { cell, because, .. } => self.check_run(s, cell, because),
            Decision::Wait { .. } | Decision::Idle | Decision::Ended(_) => self.check_stop(s, d),
            _ => {}
        }
    }

    fn check_run(&mut self, s: &State, cell: &CellId, because: &[Cause]) {
        let m = self.model;
        let flow = &m.compiled.flow;
        let r = Reading::new(&s.world.records);
        let i = m.place(cell);
        let def = &m.cells()[i];
        let trace = &s.trace;
        self.reach(ran(cell));
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
                            self.reach(ran_on(cell, t));
                            if *behind {
                                self.reach(ran_back_on(cell, t));
                            }
                        }
                        None => {
                            let name = format!("{cell} ran on {block}, which is none of its triggers");
                            self.breach(Property::NewToIt, name, trace, String::new());
                        }
                    }
                    let new = r.new_to(cell, block, NOW);
                    if new.is_empty() {
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
                    self.reach(format!("{cell} was entered with the answers it waited for"));
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
        // precondition that does not hold.
        for (j, before) in m.cells().iter().enumerate().take(i) {
            if !r.applies(flow, j) {
                if !r.reasons(before, NOW).is_empty() {
                    self.reach(skipped(&before.id));
                }
                continue;
            }
            if !r.entering(flow, before, NOW).is_empty() {
                let name = format!("{cell} ran while {} before it had something new", before.id);
                self.breach(Property::NewToIt, name, trace, String::new());
            }
            if r.owed(&before.id, NOW).iter().any(|c| !r.done(*c, NOW)) {
                let name = format!("{cell} ran while {} before it waited for an answer", before.id);
                self.breach(Property::NewToIt, name, trace, String::new());
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
        if i == m.check {
            for (call, asker) in &s.questions {
                if !r.done(*call, NOW) {
                    let applies = r.applies(flow, m.place(asker));
                    let name = format!(
                        "{cell} ran while a question of {asker} waited for the owner, {asker} being {}",
                        if applies { "on" } else { "skipped" }
                    );
                    self.breach(Property::Gates, name, trace, String::new());
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
                self.reach(reply_loop(&o, &reply));
            }
        }
        // 7: what a write from outside reaches.
        if let Some(t) = &s.outside
            && self.tainted(s, i, because, t)
        {
            let what = t.what.clone();
            self.out.reruns.entry(what.clone()).or_default().insert(cell.clone());
            if !t.reach.contains(&i) {
                let name = format!("{what} ran {cell} again, which its change does not reach");
                self.breach(Property::Incremental, name, trace, String::new());
            }
        }
    }

    /// 5, of a run: a block of a group that runs the cell is one of the group's.
    fn check_group(&mut self, s: &State, i: usize, block: &BlockId) {
        let m = self.model;
        if let Some(trigger) = m.triggers[i].get(block)
            && let Some(members) = m.groups[i].get(trigger)
            && !members.contains(block)
        {
            let name = format!("{} ran on {trigger} for {block}, none of its members", m.cells()[i].id);
            self.breach(Property::Groups, name, &s.trace, String::new());
        }
    }

    /// 3, and 7 where the outside wrote: the work waits, rests or has ended.
    fn check_stop(&mut self, s: &State, d: &Decision) {
        let m = self.model;
        let flow = &m.compiled.flow;
        let trace = &s.trace;
        let (stop, on_purpose) = match d {
            Decision::Ended(End::Merged) => ("the work merged".to_string(), true),
            Decision::Ended(End::Cancelled) => ("the work was cancelled".to_string(), true),
            Decision::Idle => (
                "the work rests with nothing new to any cell, not merged".to_string(),
                false,
            ),
            Decision::Wait { at, on } => {
                for w in on {
                    if let Waited::Precondition { on: key, .. } = w {
                        self.reach(waited_at_gate(at, key));
                    }
                }
                let shown: Vec<String> = on.iter().map(show_waited).collect();
                let on_purpose = on.iter().any(|w| match w {
                    Waited::Call { on, .. } => on.is_some(),
                    Waited::Limit { .. } | Waited::Calls { .. } => true,
                    Waited::Precondition { by, .. } => by
                        .iter()
                        .any(|maker| matches!(maker, Maker::Owner | Maker::Outside { .. })),
                    Waited::Slot => false,
                });
                (
                    format!("the work waits at {at} for {}", shown.join(" and ")),
                    on_purpose,
                )
            }
            _ => return,
        };
        self.reach(format!("stop: {stop}"));
        if on_purpose {
            if !matches!(d, Decision::Ended(_)) {
                self.reach(WAITED_FOR_THE_OWNER.to_string());
            }
        } else {
            self.breach(Property::NextStep, stop, trace, String::new());
        }
        if let Decision::Wait { on, .. } = d
            && on.iter().any(|w| {
                matches!(
                    w,
                    Waited::Call {
                        on: Some(Who::Owner),
                        ..
                    }
                )
            })
            && s.questions
                .keys()
                .any(|c| on.iter().any(|w| matches!(w, Waited::Call { call, .. } if call == c)))
        {
            self.reach(A_QUESTION_WAITED.to_string());
        }
        // 7: the write reached each cell it runs before where the work stops.
        let Some(t) = &s.outside else {
            return;
        };
        let r = Reading::new(&s.world.records);
        let upto = match d {
            Decision::Wait { at, .. } => m.place(at),
            _ => m.cells().len(),
        };
        for (j, cell) in m.cells().iter().enumerate().take(upto) {
            if !r.applies(flow, j) {
                continue;
            }
            for block in t.blocks.iter().filter(|b| cell.runs_on.contains(*b)) {
                if !r.new_to(&cell.id, block, NOW).is_empty() {
                    let name = format!(
                        "{} did not reach {}, which it runs, before the work stopped",
                        t.what, cell.id
                    );
                    self.breach(Property::Incremental, name, trace, String::new());
                }
            }
        }
    }
}

/// The runs in a row of the cell, since the owner last let it go on, that were each a way round, and that each asked
/// for calls and dealt with nothing: what the walk's limits count.
fn streaks(records: &Records, cell: &CellId) -> (usize, usize) {
    let let_go = (records.let_go.iter()).filter(|l| &l.cell == cell).map(|l| l.at).max();
    let mut finished: Vec<(&Finished, &Started)> = (records.started.iter())
        .filter(|st| &st.cell == cell && let_go.is_none_or(|at| st.at > at))
        .filter_map(|st| records.finished.iter().find(|f| f.run == st.run).map(|f| (f, st)))
        .collect();
    finished.sort_by_key(|(f, _)| f.at);
    let dealt = |f: &Finished| f.dealt_with.as_ref().is_none_or(|d| !d.is_empty());
    let way_round = |because: &[Cause]| {
        because.iter().any(|c| matches!(c, Cause::Changed { .. })) && !because.iter().any(Cause::forward)
    };
    let ways = (finished.iter().rev())
        .take_while(|(_, st)| !st.because.iter().any(Cause::forward))
        .filter(|(f, st)| way_round(&st.because) && dealt(f))
        .count();
    let asked = |st: &Started, f: &Finished| {
        let ended = (records.returned.iter())
            .filter(|e| e.run == st.run)
            .map(|e| e.at)
            .max();
        (records.called.iter()).any(|c| c.run == st.run && ended.is_none_or(|e| c.at > e) && c.at < f.at)
    };
    let calls = (finished.iter().rev())
        .take_while(|(f, st)| f.dealt_with.as_ref().is_some_and(Vec::is_empty) && asked(st, f))
        .count();
    (ways, calls)
}

fn show_waited(w: &Waited) -> String {
    match w {
        Waited::Call { call, on } => format!("the call {} on {on:?}", call.0),
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
        Waited::Limit { runs } => format!("the owner, at its limit of {runs} runs"),
        Waited::Calls { runs } => format!("the owner, at its limit of {runs} runs that asked"),
        Waited::Slot => "a slot".to_string(),
    }
}

/// What an outside write is, apart from the call it answers.
fn kind_of(e: &Event) -> String {
    match e {
        Event::Answers(_) => "the owner answers".to_string(),
        e => e.to_string(),
    }
}

/// The items of coverage that are no cell's.
pub const MAIN_WHILE_THE_CHECK_WAITS: &str = "main moved while the check waited at its gate";
pub const WAITED_FOR_THE_OWNER: &str = "the work waited for the owner or the outside";
pub const A_QUESTION_WAITED: &str = "a question waited for the owner";
pub const WRITTEN_OVER: &str = "a return was sent back as it would write over what its round was not told of";
