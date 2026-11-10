//! Random histories (principle simulated-histories): many works, each on a small flow drawn at random, driven step by
//! step as an engine would drive them, agents returning at random and people acting at random in the first part of
//! each history. After every step the properties the engine must always keep are checked; once people stop, the work
//! must come to rest within a bound its limits give.
//!
//! A failing case prints its seed and the steps it took; `one_history(seed)` replays it.

// The histories use only part of the helper the rule tests share.
#[allow(dead_code)]
mod support;

use std::panic::{self, AssertUnwindSafe};

use decide::{Cell, CellId, Condition, Decision, End, Outcome, ProductId, RunId, Waited, Who, current};
use support::Says::{DoesNotMatter, Matters};
use support::{World, cell};

/// How many histories are run.
const SEEDS: u64 = 1000;

/// The product only people write.
const BRIEF: &str = "brief";

/// The products cells may write.
const PRODUCTS: [&str; 3] = ["p0", "p1", "p2"];

/// The most rounds a run has before its agent finishes: within a run, how often a return is sent back is bounded by the
/// engine, outside the deciding code.
const ROUNDS: usize = 3;

/// A small deterministic generator (xorshift), so that a seed gives the same history every time.
struct Rng(u64);

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

fn on_fork(mut c: Cell, by: &str) -> Cell {
    c.applies = Some(Condition::Answered {
        by: by.into(),
        output: "go".into(),
        is: "yes".into(),
    });
    c
}

/// The cells of a flow before its merge: a shape, cut or lengthened with cells drawn at random, each with a limit of
/// one or two.
fn draw_cells(rng: &mut Rng) -> Vec<Cell> {
    let mut cells = match rng.below(4) {
        // A chain, each cell depending on what the one before it may write.
        0 => vec![
            cell("c0", &[BRIEF], &["p0"]),
            cell("c1", &["p0"], &["p1"]),
            cell("c2", &["p1"], &["p2"]),
        ],
        // A cell on the path by the answer of the one before it.
        1 => vec![
            cell("c0", &[BRIEF], &["p0"]),
            on_fork(cell("c1", &["p0"], &["p1"]), "c0"),
            cell("c2", &["p0", "p1"], &["p2"]),
        ],
        // Two cells that may write the same product.
        2 => vec![
            cell("c0", &[BRIEF], &["p0"]),
            cell("c1", &["p0"], &["p0", "p1"]),
            cell("c2", &["p0"], &["p2"]),
        ],
        // A reviewer of a writer's product.
        _ => vec![
            cell("c0", &[BRIEF], &["p0"]),
            cell("c1", &["p0"], &["p1"]),
            cell("c2", &["p1"], &[]),
        ],
    };
    let n = 2 + rng.below(4);
    cells.truncate(n);
    while cells.len() < n {
        let i = cells.len();
        let follows: Vec<&str> = [BRIEF].into_iter().chain(PRODUCTS).filter(|_| rng.chance(35)).collect();
        let writes: Vec<&str> = PRODUCTS.into_iter().filter(|_| rng.chance(30)).collect();
        let mut c = cell(&format!("c{i}"), &follows, &writes);
        if rng.chance(25) {
            c = on_fork(c, &format!("c{}", i - 1));
        }
        cells.push(c);
    }
    // Now and then a later cell writes what an earlier one depends on. A cell sees only what was written before it, so
    // this must send nobody back.
    if rng.chance(25) {
        cells[0].follows.push("p2".into());
    }
    if rng.chance(30) {
        let last = cells.len() - 1;
        if !cells[last].writes.iter().any(|p| p.as_str() == "p0") {
            cells[last].writes.push("p0".into());
        }
    }
    for c in &mut cells {
        c.limit = 1 + rng.below(2) as u32;
    }
    cells
}

struct History {
    w: World,
    rng: Rng,
    log: Vec<String>,
    /// Whether the work is paused, as people last left it.
    paused: bool,
    /// How many refusals people made.
    person_refusals: u64,
    /// The latest decision that was not `Busy`.
    last_move: Option<Decision>,
}

impl History {
    fn new(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let cells = draw_cells(&mut rng);
        let mut w = World::with_gate(cells);
        if rng.chance(40) {
            w.set("the merge may go", Outcome::Holds);
        }
        let mut log = vec![format!("flow: {:?}", w.flow.cells())];
        log.push(format!("the merge may go: {:?}", w.named.0));
        Self {
            w,
            rng,
            log,
            paused: false,
            person_refusals: 0,
            last_move: None,
        }
    }

    fn note(&mut self, what: String) {
        self.log.push(what);
    }

    /// One step: people may act first, then the engine applies the decision. Says whether the work moved; it has
    /// not when it rests waiting for people who no longer act, is paused, or has ended.
    fn step(&mut self, people: bool) -> bool {
        if people && self.rng.chance(15) {
            self.person_acts(None, &[]);
        }
        let decision = self.w.decide();
        self.note(format!("decision: {decision:?}"));
        self.check(&decision);
        let moved = match &decision {
            Decision::Fault(fault) => panic!("the records are said to be faulty: {fault:?}"),
            Decision::Ended(End::Passed) => {
                let merge = self.w.flow.cells().last().expect("a merge").id.clone();
                match &self.last_move {
                    Some(Decision::Run { cell, .. }) if *cell == merge => {}
                    other => panic!("ended without the merge having run just before: {other:?}"),
                }
                false
            }
            Decision::Ended(End::Cancelled) => false,
            Decision::Paused => {
                if people {
                    self.toggle_pause();
                }
                people
            }
            Decision::Busy { cell, run } => {
                self.end_round(cell.clone(), *run);
                true
            }
            Decision::Run { cell, .. } => {
                self.w.start(cell.as_str());
                true
            }
            Decision::GoOn { cell, .. } => {
                self.w.go_on(cell.as_str());
                true
            }
            Decision::Abandon { .. } => {
                self.w.abandon();
                true
            }
            Decision::Wait { at, on } => {
                assert!(!on.is_empty(), "a wait names nothing waited for");
                let engine = on.iter().find_map(|o| match o {
                    Waited::Call { call, on: None } => Some(*call),
                    _ => None,
                });
                if let Some(call) = engine {
                    self.note(format!("the engine's call {call:?} is done"));
                    self.w.done(call);
                    true
                } else if people {
                    self.person_acts(Some(at), on);
                    true
                } else {
                    false
                }
            }
        };
        if !matches!(decision, Decision::Busy { .. } | Decision::Ended(_)) {
            self.last_move = Some(decision);
        }
        moved
    }

    /// What must hold after every step.
    fn check(&self, decision: &Decision) {
        let r = &self.w.records;
        let under_way = (r.started.iter())
            .filter(|s| {
                let began = r.rounds.iter().filter(|x| x.run == s.run).count();
                let ended = r.returned.iter().filter(|x| x.run == s.run).count();
                began > ended
            })
            .count();
        assert!(under_way <= 1, "{under_way} runs have a round under way");
        if under_way == 1 {
            assert!(
                matches!(decision, Decision::Busy { .. } | Decision::Ended(_)),
                "a round is under way and the walk decides {decision:?}"
            );
        }
    }

    /// The round under way of the run ends as its agent returns: it finishes, returns with nothing, or rests at a call.
    fn end_round(&mut self, cell: CellId, run: RunId) {
        assert_eq!(
            self.w.open(cell.as_str()),
            run,
            "the run under way is the cell's open run"
        );
        let spec = self.w.flow.cell(&cell).expect("the cell is in the flow").clone();
        let is_merge = self.w.flow.cells().last().is_some_and(|c| c.id == cell);
        let rounds = self.w.records.rounds.iter().filter(|r| r.run == run).count();
        let how = if is_merge || rounds >= ROUNDS {
            0
        } else {
            self.rng.below(4)
        };
        match how {
            0 | 1 => {
                let go = if self.rng.chance(50) { "yes" } else { "no" };
                let mut writes = Vec::new();
                for p in &spec.writes {
                    if self.rng.chance(60) {
                        let says = if self.rng.chance(50) { Matters } else { DoesNotMatter };
                        writes.push((p.as_str(), says));
                    }
                }
                let shown: Vec<_> = (writes.iter())
                    .map(|(p, s)| format!("{p}{}", if *s == Matters { "" } else { " (does not matter)" }))
                    .collect();
                self.note(format!("{cell} finishes: go {go}, writes {shown:?}"));
                self.w.finish(cell.as_str(), &[("go", go)], &writes);
                // A cell that depends on a product may refuse the version it was built on as it finishes.
                if self.rng.chance(25) {
                    let refusable: Vec<ProductId> = (spec.follows.iter())
                        .filter(|p| self.w.basis_of(run, p.as_str()).is_some())
                        .cloned()
                        .collect();
                    if !refusable.is_empty() {
                        let p = refusable[self.rng.below(refusable.len())].clone();
                        self.note(format!("{cell} refuses {p}"));
                        self.w.refuse(cell.as_str(), p.as_str());
                    }
                }
            }
            2 => {
                self.note(format!("{cell} returns with nothing"));
                self.w.returns(cell.as_str());
            }
            _ => {
                let on = if self.rng.chance(30) { Some(Who::Owner) } else { None };
                self.note(format!("{cell} calls on {on:?}"));
                self.w.calls(cell.as_str(), on);
            }
        }
    }

    fn toggle_pause(&mut self) {
        if self.paused {
            self.note("a person lets the work on".into());
            self.w.resume();
        } else {
            self.note("a person pauses the work".into());
            self.w.pause();
        }
        self.paused = !self.paused;
    }

    /// A person acts: on what the work waits for, where there is something, or at random.
    fn person_acts(&mut self, at: Option<&CellId>, waited: &[Waited]) {
        for o in waited {
            match o {
                Waited::Call { call, on: Some(_) } if self.rng.chance(60) => {
                    self.note(format!("a person answers the call {call:?}"));
                    self.w.done(*call);
                    return;
                }
                Waited::Limit { .. } if self.rng.chance(60) => {
                    let at = at.expect("a limit is waited for at a cell").clone();
                    self.note(format!("the owner lets {at} go on"));
                    self.w.let_go(at.as_str());
                    return;
                }
                _ => {}
            }
        }
        match self.rng.below(5) {
            0 | 1 => {
                self.note(format!("a person writes {BRIEF}"));
                self.w.person_writes(BRIEF);
            }
            2 => {
                let all: Vec<&str> = [BRIEF].into_iter().chain(PRODUCTS).collect();
                let p: ProductId = all[self.rng.below(all.len())].into();
                if current(&self.w.flow, &self.w.records, &self.w.named, &p).is_some() {
                    self.note(format!("the owner refuses {p}"));
                    self.w.owner_refuses(p.as_str());
                    self.person_refusals += 1;
                }
            }
            3 => {
                let cells = self.w.flow.cells();
                let c = cells[self.rng.below(cells.len())].id.clone();
                self.note(format!("the owner lets {c} go on"));
                self.w.let_go(c.as_str());
            }
            _ => self.toggle_pause(),
        }
    }

    /// How many steps, once people stop, the work may take before it must wait or end (entry a-work-comes-to-rest).
    ///
    /// Once people stop, a run of a cell is a way round unless it has a reason from before the cell or from a person:
    /// its cell has no record, a person's refusal made before they stopped, a person's write made before they
    /// stopped, or a change of what it depends on, which it sees only as people, the outside and the cells before it
    /// wrote it. Nobody lets a cell go on any more, so between two runs that are not ways round a cell runs at most its
    /// limit of ways round. The runs that are not ways round number at most two (the cell's first, the person's last
    /// write), two for each person's refusal (the cell answers it, and once more if it took it up again after handing
    /// it on), and one for each run of a cell before it, whose finishing changed what it sees or which cells are on
    /// the path (a condition reads only cells before its own). So the cell at place `i`, of limit `L_i`, runs at most
    /// `R_i = (L_i + 1) * (2 + 2P + R_0 + ... + R_(i-1))` times, plus the one run open when people stopped. A run takes
    /// at most `4 * ROUNDS` steps (each round's end, an engine call done, the going on, and its start or abandonment).
    fn bound(&self) -> u64 {
        let mut before: u64 = 0;
        for c in self.w.flow.cells() {
            let r = (u64::from(c.limit) + 1).saturating_mul(2 + 2 * self.person_refusals + before);
            before = before.saturating_add(r);
        }
        (before + 1).saturating_mul(4 * ROUNDS as u64)
    }
}

/// One history: people act in its first part, then the work is driven until it rests.
fn one_history(seed: u64, log: &mut Vec<String>) {
    let mut h = History::new(seed);
    let people_until = h.rng.below(40);
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        for _ in 0..people_until {
            h.step(true);
            if matches!(h.w.decide(), Decision::Ended(_)) {
                break;
            }
        }
        h.note("people stop".into());
        let bound = h.bound();
        let mut steps = 0;
        while h.step(false) {
            steps += 1;
            assert!(steps <= bound, "the work has not come to rest within {bound} steps");
        }
    }));
    *log = std::mem::take(&mut h.log);
    if let Err(e) = result {
        panic::resume_unwind(e);
    }
}

#[test]
fn random_works_keep_the_engines_properties_and_come_to_rest() {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let mut failures = Vec::new();
    for seed in 0..SEEDS {
        let mut log = Vec::new();
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| one_history(seed, &mut log)));
        if let Err(e) = outcome {
            let message = (e.downcast_ref::<String>().cloned())
                .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_string()))
                .unwrap_or_default();
            failures.push((seed, message, log));
        }
    }
    panic::set_hook(hook);
    if let Some((seed, message, log)) = failures.first() {
        let seeds: Vec<_> = failures.iter().map(|(s, m, _)| format!("{s}: {m}")).collect();
        panic!(
            "{} of {SEEDS} histories failed:\n{}\n\nthe steps of seed {seed} ({message}):\n{}",
            failures.len(),
            seeds.join("\n"),
            log.join("\n")
        );
    }
}
