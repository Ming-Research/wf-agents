//! Random histories of a work through the engine's loop, saved after every step and, at random steps, dropped and
//! loaded back from a store opened anew: each must end in the same work as the same history kept in memory alone.
//!
//! What a step is, roughly. In the first 0 to 59 steps people act at a step with a chance of one in four; otherwise,
//! and always once they stop, one of the things the engine gave to be done is done, each with an equal chance: a round
//! under way ends, handing in blocks (some its cell may not write, some choices with a wrong value), asking for one to
//! three calls or asking for nothing; a call has its result (one for a person only while people act); a call to be
//! withdrawn is said to be; a round to be stopped ends. A person's act is a write by a person or the outside, with a
//! value or without, one the loop refuses among them; letting a cell go on; a pause or a resume; the owner's word at
//! the merge, a choice the merge's named test reads, written the other way; or, in one act in fifty, a cancel. After every step the work is saved, and with
//! a chance of three in ten it is dropped and loaded back.
//!
//! A failing case prints its seed and the steps it took; `one_history(seed, ..)` replays it.

use std::panic::{self, AssertUnwindSafe};

use engine::Call;
use work::Measures;

use super::*;

/// How many histories are run.
const SEEDS: u64 = 40;

/// How many steps, at most, people act in.
const PEOPLE: usize = 60;

/// How many steps a work may take after people stop before it must be at rest.
const REST: usize = 5000;

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

/// Where a history's work is saved after every step, and the draws, apart from the history's own, of when it is
/// dropped and loaded back.
struct Kept {
    db: Db,
    store: Store,
    work: WorkNumber,
    draws: Rng,
}

struct History {
    flow: Compiled,
    /// The owner's word at the merge, as last written: yes, or no where it was never written.
    go: bool,
    work: Work,
    rng: Rng,
    clock: u64,
    /// None for the history kept in memory alone.
    kept: Option<Kept>,
    reloads: usize,
    log: Vec<String>,
}

impl History {
    fn new(seed: u64, kept: bool) -> Self {
        let kept = kept.then(|| {
            let db = Db::new();
            let mut store = db.open();
            let work = first_work(&mut store);
            let draws = Rng::new(seed ^ 0x5DEE_CE66_D1CE_F00D);
            Kept { db, store, work, draws }
        });
        Self {
            flow: judged(),
            go: false,
            work: Work::default(),
            rng: Rng::new(seed),
            clock: START,
            kept,
            reloads: 0,
            log: Vec::new(),
        }
    }

    /// Hands the event to the loop; then saves the work and, at random, drops it and loads it back.
    fn take(&mut self, event: Event) {
        self.log.push(format!("event: {event:?}"));
        self.clock += 1000;
        let now = Time(self.clock);
        match handle(&self.flow, &mut self.work, now, event) {
            Ok(effects) => self.log.push(format!("effects: {effects:?}")),
            Err(why) => self.log.push(format!("refused: {why:?}")),
        }
        if let Some(k) = &mut self.kept {
            k.store.save(k.work, &self.work).expect("the work is saved");
            if k.draws.chance(30) {
                let loaded = k.db.open().load(k.work).expect("the work loads");
                assert_eq!(loaded, self.work, "the work loaded back is not the work saved");
                self.work = loaded;
                self.reloads += 1;
                self.log.push("dropped and loaded back".into());
            }
        }
    }

    /// A round under way ends as its agent returns.
    fn end_round(&mut self, run: RunId, cell: &str) {
        let place = self.flow.flow.place(&cell.into()).expect("the cell is in the flow");
        let outputs = self.flow.flow.cells()[place].outputs.clone();
        let came = match self.rng.below(10) {
            0..=4 => {
                let mut blocks: Vec<decide::BlockId> = outputs.into_iter().filter(|_| self.rng.chance(70)).collect();
                if self.rng.chance(10) {
                    blocks.push(["request", "merged", "opinions", "verdict"][self.rng.below(4)].into());
                }
                let handed = (blocks.into_iter())
                    .map(|block| {
                        let choice = self.flow.blocks.get(&block).is_some_and(|b| b.values.is_some());
                        let value = match (choice, self.rng.below(10)) {
                            (true, 0..=7) => Some(if self.rng.chance(50) { "yes" } else { "no" }.to_string()),
                            (true, 8) => Some("maybe".to_string()),
                            (false, 0) => Some("yes".to_string()),
                            _ => None,
                        };
                        Handed {
                            content: format!("{block} by {cell} at {}", self.clock),
                            block,
                            matters: self.rng.chance(50),
                            value,
                        }
                    })
                    .collect();
                Came::hands_in(handed)
            }
            5..=7 => {
                let n = 1 + self.rng.below(3);
                let asks = (0..n)
                    .map(|i| {
                        if self.rng.chance(70) {
                            tool(&format!("tool {i}"))
                        } else {
                            question(if self.rng.chance(50) {
                                Who::Owner
                            } else {
                                Who::Architect
                            })
                        }
                    })
                    .collect();
                Came::asks(asks)
            }
            _ => Came::asks(vec![]),
        };
        // What the round's calls came to is told before its end, now and then.
        if self.rng.chance(50) {
            let measures = Measures {
                input_tokens: self.clock % 7,
                cache_read_tokens: self.clock % 997,
                cache_written_tokens: self.clock % 991,
                thinking_tokens: self.rng.chance(50).then_some(self.clock % 13),
                turns: 1 + self.rng.below(5) as u64,
                calls: 1 + self.rng.below(5) as u32,
                first_read: Some(self.clock % 997),
                first_context: Some(self.clock % 991 + 1000),
                added: self.clock % 89,
                given: self.clock % 83,
                before: [("model".to_string(), format!("m{}", self.rng.below(2)))].into(),
                received: self.rng.chance(50),
                crossed: self.rng.chance(10),
                priced: self.rng.chance(10),
            };
            self.take(Event::Measured {
                run,
                attempt: 1,
                measures,
            });
        }
        let spent = self.spent(run);
        self.take(Event::Returned { run, came, spent });
    }

    /// What a round of the run took: told apart by the run and the clock.
    fn spent(&self, run: RunId) -> Spent {
        Spent {
            usd_micros: run.0,
            input_tokens: self.clock % 997,
            cached_tokens: self.clock % 991,
            output_tokens: 1,
            ..Default::default()
        }
    }

    /// A person or the outside acts.
    fn person_acts(&mut self) {
        let value = |rng: &mut Rng, a: &str, b: &str| Some(if rng.chance(50) { a } else { b }.to_string());
        let event = match self.rng.below(100) {
            0..=1 => Event::Cancelled { why: None },
            2..=21 => {
                let block = ["request", "opinions", "verdict"][self.rng.below(3)];
                let value = if block == "verdict" {
                    value(&mut self.rng, "yes", "maybe")
                } else {
                    None
                };
                wrote(block, Outsider::Person, self.rng.chance(50), value)
            }
            22..=41 => {
                if self.rng.chance(50) {
                    wrote("main", Outsider::Outside, self.rng.chance(50), None)
                } else {
                    let value = value(&mut self.rng, "pass", "fail");
                    wrote("checks", Outsider::Outside, true, value)
                }
            }
            42..=47 => {
                // A keep-warm ping of a line's session ended, or failed.
                let failed = self.rng.chance(30).then(|| "the network".to_string());
                Event::Pinged {
                    line: ["main", "other"][self.rng.below(2)].to_string(),
                    session: self.rng.below(2) as u32,
                    spent: self.spent(RunId(self.clock)),
                    failed,
                }
            }
            48..=55 => {
                let cells = self.flow.flow.cells();
                Event::LetGo {
                    cell: cells[self.rng.below(cells.len())].id.clone(),
                }
            }
            56..=77 => {
                if decide::paused(&self.work.records) {
                    Event::Resumed
                } else {
                    Event::Paused {
                        by: Pauser::Owner,
                        why: None,
                    }
                }
            }
            _ => {
                self.go = !self.go;
                let word = if self.go { "yes" } else { "no" };
                wrote("go", Outsider::Person, true, Some(word.to_string()))
            }
        };
        self.take(event);
    }

    /// One step. Says whether anything was done: once people stop, nothing is when nothing they need not do is left.
    fn step(&mut self, people: bool) -> bool {
        if people && self.rng.chance(25) {
            self.person_acts();
            return true;
        }
        let doable: Vec<Effect> = (pending(&self.flow, &self.work).into_iter())
            .filter(|e| match e {
                Effect::Call { what, .. } => people || matches!(what, Call::Asked(Ask::Tool { .. })),
                _ => true,
            })
            .collect();
        if doable.is_empty() {
            if people {
                self.person_acts();
                return true;
            }
            return false;
        }
        match doable[self.rng.below(doable.len())].clone() {
            Effect::Round { run, cell, .. } => self.end_round(run, cell.as_str()),
            Effect::Call { call, .. } => {
                let result = format!("the result of {}", call.0);
                self.take(Event::Done { call, result });
            }
            Effect::Withdraw { call } => {
                let result = "withdrawn".to_string();
                self.take(Event::Done { call, result });
            }
            Effect::Stop { run } => {
                let spent = self.spent(run);
                let came = Came::asks(vec![]);
                self.take(Event::Returned { run, came, spent });
            }
            Effect::Release { .. } => {
                self.take(Event::Released);
            }
            Effect::Ping { .. } => unreachable!("the work's loop gives no ping"),
        }
        true
    }
}

fn wrote(block: &str, by: Outsider, matters: bool, value: Option<String>) -> Event {
    Event::Wrote {
        block: block.into(),
        by,
        content: format!("{block} by {by:?}"),
        matters,
        value,
    }
}

/// One history, kept in a store or in memory alone: the work it ends with and how often it was loaded back.
fn one_history(seed: u64, kept: bool, log: &mut Vec<String>) -> (Work, usize) {
    let mut h = History::new(seed, kept);
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        h.take(Event::Look);
        let people = h.rng.below(PEOPLE);
        for _ in 0..people {
            h.step(true);
        }
        h.log.push("people stop".into());
        let mut steps = 0;
        while h.step(false) {
            steps += 1;
            assert!(steps <= REST, "the work has not come to rest within {REST} steps");
        }
        if let Some(k) = &h.kept {
            assert_eq!(
                k.db.open().load(k.work).as_ref(),
                Ok(&h.work),
                "the store does not hold the work"
            );
        }
    }));
    *log = std::mem::take(&mut h.log);
    if let Err(e) = result {
        panic::resume_unwind(e);
    }
    (h.work, h.reloads)
}

#[test]
fn a_work_saved_at_every_step_and_loaded_back_at_random_ends_as_the_one_kept_in_memory() {
    let mut failures = Vec::new();
    let mut reloads = 0;
    for seed in 0..SEEDS {
        let mut log = Vec::new();
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
            let (kept, n) = one_history(seed, true, &mut log);
            let mut quiet = Vec::new();
            let (straight, _) = one_history(seed, false, &mut quiet);
            assert_eq!(
                kept, straight,
                "the history loaded back ended otherwise than the one in memory"
            );
            n
        }));
        match outcome {
            Ok(n) => reloads += n,
            Err(e) => {
                let message = (e.downcast_ref::<String>().cloned())
                    .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_string()))
                    .unwrap_or_default();
                failures.push((seed, message, log));
            }
        }
    }
    if let Some((seed, message, log)) = failures.first() {
        let seeds: Vec<_> = failures.iter().map(|(s, m, _)| format!("{s}: {m}")).collect();
        panic!(
            "{} of {SEEDS} histories failed:\n{}\n\nthe steps of seed {seed} ({message}):\n{}",
            failures.len(),
            seeds.join("\n"),
            log.join("\n")
        );
    }
    assert!(reloads > SEEDS as usize, "works were too seldom loaded back: {reloads}");
}
