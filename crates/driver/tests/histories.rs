//! Simulated histories of several works through the driver (principle simulated-histories). Items are added, some
//! depending on others; what the driver gives to be done is answered in a random order and in random ways; people act
//! at random; and the process stops, or the store fails, at random calls, the driver being opened anew each time on the
//! same file.
//!
//! What is checked after every step is what the harness in `support` checks after every call: the store holds what
//! the driver shows; no work has two rounds under way; what was given and not yet answered is exactly what is pending,
//! and nothing is given twice unless it was pending across an opening; the stored records walk to the decision shown;
//! every dependencies block agrees with which of the items its item depends on have merged; no work whose
//! dependencies have not all merged has started a run; and a work never let on has started none. When the driver
//! stops, what it says was kept is what the store holds.
//!
//! Once people stop, the works left paused are let on as their last act, and the rounds, the tools' calls, the
//! withdrawals and the stops are answered until nothing more is given: within a bound each work then waits, saying for
//! what and for whom, or has ended, and what is pending is only what people are to answer (property
//! a-work-comes-to-rest). A history with stops and failures, the owner lifting at once the pause of everything the
//! driver makes as it opens on works going on, ends as the same history with none, each entry added at the same time.
//!
//! What a step is, roughly. In the first 0 to 79 steps people act at a step with a chance of one in four; otherwise,
//! and always once they stop, one of the things given to be done is answered, each with an equal chance:
//! - a round ends: an engine's act hands in its outputs, or, one in seven, asks for nothing; an agent hands in what its
//!   cell may write in half of them (each output with a chance of six in ten, a choice with one of its values), hands
//!   in what does not meet the flow in three in twenty (a block it may not write, or a choice with a wrong value), asks
//!   for one to three tools in three in twenty, asks the owner or the architect in three in twenty, and asks for
//!   nothing in one in twenty;
//! - a call has its result: always for a tool, and for a person or at a limit only while people act;
//! - a call withdrawn is said to be; a round stopped ends.
//!
//! A person's act is, about: a write of a block people may write, or a look where they may write none; a pause or a
//! letting on; a cancel; letting a cell go on, at its limit where a work waits at one; the result of a question or of
//! a limit; adding an item, depending on some of those there; or something the driver must refuse, which must change
//! nothing. An item is added on one of the small flows of the tests or on the deliver flow as the repository defines
//! it, whose items are made of the items they depend on and wait for them at their one cell, so that items depending on
//! a deliverable wait for it in turn. One item in five is a deliverable, drawn by a generator of its own, so that the
//! other draws keep their order.
//!
//! The process stops before a step with a chance of one in sixteen; with one in twenty-five the store is set to fail
//! at one of its next three calls, and with one in fifty the process to stop right after one of them.
//!
//! The driver has two slots. Every item is added paused; one in three is let on by the owner at once as it is added,
//! drawn by a generator of its own, and the others are let on later, as before; and, drawn by another, the owner sets
//! or lifts the pause of everything before a step with a chance of one in twenty while people act, and lifts it as
//! people stop.
//!
//! A failing case prints its seed and the steps it took; `one_history(seed, ..)` replays it.

#[allow(dead_code)]
#[path = "support/mod.rs"]
mod support;

use std::collections::BTreeSet;
use std::panic::{self, AssertUnwindSafe};

use decide::{CallId, Decision, End, Outsider, Pauser, RunId, Waited, Who};
use driver::{Asked, FirstWrite, NewItem, Todo};
use engine::{Ask, Call, Came, Effect, Event, Handed, Spent, Time, Work};
use store::{ItemChange, ItemNumber, WorkHead, WorkNumber};
use support::*;

/// How many histories are run: enough that each class of step is reached. With a deliverable among every five items,
/// twenty-four histories reached no round stopped in one draw and no stop that kept what it was asked in another; the
/// histories take some fifteen seconds of CI for every twenty-four.
const SEEDS: u64 = 48;

/// How many steps, at most, people act in.
const PEOPLE: usize = 80;

/// How many items a history holds at most.
const ITEMS: usize = 7;

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

/// What the histories did, so that each property is known to have been put to the test.
#[derive(Default, Debug)]
struct Seen {
    stops: Stops,
    /// Openings anew between two steps.
    reopens: usize,
    /// Acts the driver refused, each leaving everything as it was.
    refused: usize,
    /// Items added while people acted.
    added: usize,
    cancels: usize,
    /// Calls given at a limit.
    limits: usize,
    /// Calls withdrawn and rounds stopped.
    withdrawn: usize,
    stopped: usize,
    /// Calls in which a work merged and another work's first round was given.
    started_at_merge: usize,
    /// Steps after which a work let on waited for its dependencies.
    waited: usize,
    /// Steps after which a work had never been let on.
    never_let_on: usize,
    /// Items let on as they were added, steps after which a work waited for a slot, and steps after which one was held
    /// back by the pause of everything.
    let_on_as_added: usize,
    waited_for_a_slot: usize,
    held_by_everything: usize,
    /// Works found waiting at rest, and works found ended.
    rested_waiting: usize,
    rested_ended: usize,
    /// Rounds given again by an opening because they were pending across it.
    given_again: usize,
    /// Changes of an item the driver made, and those it refused or that changed nothing.
    changed: usize,
    change_refused: usize,
    /// Works reopened, their items cancelled before.
    reopened: usize,
    /// Openings on works going on, at which the driver paused everything until the owner lifted it.
    held_at_start: usize,
}

/// A history: the harness, the draws of the history and, apart, of its stops.
struct History {
    h: H,
    rng: Rng,
    /// Draws the stops and failures alone, so that a history with them makes the same choices as one without.
    faults: Option<Rng>,
    /// Draws which items are deliverables alone, so that the other draws keep the order they had before the deliver
    /// flow was among the flows.
    delivers: Rng,
    /// Draws which items are added going on, and when the owner sets or lifts the pause of everything, alone, so
    /// that the other draws keep the order they had before.
    room: Rng,
    /// Draws when an item is changed, and how, alone, so that the other draws keep the order they had before.
    changes: Rng,
    step: usize,
    /// The calls that have their result, each with its work.
    done: Vec<(WorkNumber, CallId)>,
    log: Vec<String>,
    seen: Seen,
}

/// How a history ended: every work with its head and all that is kept of it, what each does next, and every round and
/// call given and answered.
#[derive(Debug, PartialEq)]
struct Ending {
    works: Vec<(WorkHead, Work, Decision)>,
    ever: BTreeSet<String>,
    undone: BTreeSet<String>,
    answered: BTreeSet<String>,
}

/// The flows the histories add items on: those of the tests, and the deliver flow as the repository defines it, whose
/// items are made of the items they depend on.
fn every_flow() -> Vec<flow::Compiled> {
    let mut flows = flows();
    flows.push(deliver());
    flows
}

impl History {
    fn new(seed: u64, faults: bool) -> Self {
        Self {
            h: H::with_slots(every_flow(), 2),
            rng: Rng::new(seed),
            faults: faults.then(|| Rng::new(seed ^ 0x5DEE_CE66_D1CE_F00D)),
            delivers: Rng::new(seed ^ 0xDE11_7E2A_B1E5_0F00),
            room: Rng::new(seed ^ 0x0000_5107_5A0F_A11D),
            changes: Rng::new(seed ^ 0x00C4_A93E_D17E_0000),
            step: 0,
            done: Vec::new(),
            log: Vec::new(),
            seen: Seen::default(),
        }
    }

    /// The time of the step: a second after the one before, the same however often the step is told.
    fn time(&self) -> Time {
        Time(START + 1_000_000 + self.step as u64 * 1000)
    }

    /// Does the act, opening the driver anew where it or the process stopped.
    fn act(&mut self, act: Act) -> Option<Vec<Todo>> {
        self.log.push(format!("{}: {act:?}", self.step));
        let now = self.time();
        let todos = perform(&mut self.h, now, &act, &mut self.seen.stops);
        // What was told is kept by now, told again where the driver said it was not.
        if let Act::On(work, Event::Done { call, .. }) = &act {
            self.done.push((*work, *call));
        }
        if let Some(todos) = &todos {
            self.note(&act, todos);
        }
        todos
    }

    /// Counts what was given.
    fn note(&mut self, act: &Act, todos: &[Todo]) {
        for todo in todos {
            match &todo.effect {
                Effect::Call {
                    what: Call::AtLimit(_), ..
                } => self.seen.limits += 1,
                Effect::Withdraw { .. } => self.seen.withdrawn += 1,
                Effect::Stop { .. } => self.seen.stopped += 1,
                Effect::Round { number: 1, .. } => {
                    if let Act::On(work, _) = act
                        && *work != todo.work
                    {
                        self.seen.started_at_merge += 1;
                    }
                }
                _ => {}
            }
        }
    }

    /// The works that have not ended.
    fn going(&self) -> Vec<WorkNumber> {
        (self.h.d().works())
            .map(|k| k.head.number)
            .filter(|n| !matches!(self.h.decision(*n), Decision::Ended(_)))
            .collect()
    }

    fn pick<T: Clone>(&mut self, of: &[T]) -> T {
        of[self.rng.below(of.len())].clone()
    }

    /// Adds an item, depending on some of those there that are not cancelled, on a random flow, with a first write or
    /// none.
    fn add(&mut self, title: &str) {
        let numbers: Vec<ItemNumber> = (self.h.d().works())
            .filter(|k| self.h.decision(k.head.number) != Decision::Ended(End::Cancelled))
            .map(|k| k.head.item)
            .collect();
        let depends_on: Vec<ItemNumber> = numbers.into_iter().filter(|_| self.rng.chance(35)).collect();
        let flow = ["judged", "short", "plain"][self.rng.below(3)];
        let flow = if self.delivers.chance(20) { "deliver" } else { flow };
        let has_request = (self.h.d().flows()).any(|f| f.name == flow && f.blocks.contains_key(&block("request")));
        let writes: Vec<FirstWrite> = if self.rng.chance(50) && has_request {
            vec![first_write("request", None)]
        } else {
            Vec::new()
        };
        let new = NewItem {
            title: title.to_string(),
            level: "1".to_string(),
            project: "one".to_string(),
            depends_on,
            tags: Vec::new(),
            flow: flow.to_string(),
            writes,
        };
        let let_on = self.room.chance(33);
        self.act(Act::Add(new));
        if let_on {
            let work = (self.h.d().works())
                .map(|kept| kept.head.number)
                .max()
                .expect("the work just added");
            self.seen.let_on_as_added += 1;
            self.act(Act::On(work, Event::Resumed));
        }
    }

    /// Whoever plans the work changes items, one or two together: a title or a level, what one depends on, a cancel
    /// or the reopening of a cancelled one, which may be refused.
    fn change(&mut self) {
        let items: Vec<ItemNumber> = self.h.d().works().map(|k| k.head.item).collect();
        if items.is_empty() {
            return;
        }
        let cancelled: Vec<ItemNumber> = (self.h.d().works())
            .filter(|k| self.h.decision(k.head.number) == Decision::Ended(End::Cancelled))
            .map(|k| k.head.item)
            .collect();
        let mut asked = Vec::new();
        for _ in 0..1 + usize::from(self.changes.chance(25)) {
            let item = items[self.changes.below(items.len())];
            let on = items[self.changes.below(items.len())];
            let depends_on = (self.h.d().works())
                .find(|k| k.head.item == item)
                .map(|k| k.item.depends_on.clone())
                .unwrap_or_default();
            let change = match self.changes.below(12) {
                0..=2 => Asked::Item(ItemChange::Title(format!("retitled at step {}", self.step))),
                3 => Asked::Item(ItemChange::Level(["1", "2"][self.changes.below(2)].to_string())),
                4..=6 => Asked::Item(ItemChange::DependsOn(on)),
                7 | 8 => Asked::Item(ItemChange::NoLongerDependsOn(match depends_on.as_slice() {
                    [] => on,
                    some => some[self.changes.below(some.len())],
                })),
                9 => Asked::Cancel,
                _ => match cancelled.as_slice() {
                    [] => Asked::Reopen,
                    some => {
                        asked.push((some[self.changes.below(some.len())], Asked::Reopen));
                        continue;
                    }
                },
            };
            asked.push((item, change));
        }
        let counts = |h: &H| -> (usize, usize) {
            (h.d().works()).fold((0, 0), |(changes, reopens), k| {
                (changes + k.item.changes.len(), reopens + k.work.records.reopens.len())
            })
        };
        let before = counts(&self.h);
        if self.act(Act::Change(asked)).is_some() {
            let after = counts(&self.h);
            if after.1 > before.1 {
                self.seen.reopened += after.1 - before.1;
            }
            if after.0 > before.0 {
                self.seen.changed += 1;
            } else {
                self.seen.change_refused += 1;
            }
        }
    }

    /// Before a step the process may stop, or the store be set to fail.
    fn maybe_fault(&mut self) {
        let Some(faults) = &mut self.faults else {
            return;
        };
        if faults.chance(6) {
            self.log.push("the process stops between two steps".into());
            self.h.clock = self.time().0;
            let todos = self.h.reopen();
            self.h.check();
            self.seen.reopens += 1;
            self.seen.given_again += todos.len();
            if self.h.d().everything_paused().is_some_and(|p| p.by == Pauser::Engine) {
                self.log
                    .push("the owner lifts the pause of everything the driver made as it opened".into());
                self.seen.held_at_start += 1;
                self.h.lift_the_starts_pause();
            }
        } else if faults.chance(4) {
            let plan = Plan::Fail {
                op: None,
                n: 1 + faults.below(3),
                on: faults.chance(30),
            };
            self.log.push(format!("the store is set: {plan:?}"));
            self.h.arm(plan);
        } else if faults.chance(2) {
            let plan = Plan::DieAfter { n: 1 + faults.below(3) };
            self.log.push(format!("the store is set: {plan:?}"));
            self.h.arm(plan);
        }
    }

    /// One step. Says whether anything was done: once people stop, nothing is when nothing given can be answered.
    fn step(&mut self, people: bool) -> bool {
        self.maybe_fault();
        if people && self.room.chance(5) {
            let paused = self.h.d().everything_paused().is_none();
            self.act(Act::Everything(paused));
        }
        if people && self.changes.chance(8) {
            self.change();
        }
        let done = if people && self.rng.chance(25) {
            self.person_acts();
            true
        } else if self.answer(people) {
            true
        } else if people {
            self.person_acts();
            true
        } else {
            false
        };
        if done {
            self.after();
        }
        self.step += 1;
        done
    }

    /// Counts what the step left.
    fn after(&mut self) {
        let d = self.h.d();
        for kept in d.works() {
            let number = kept.head.number;
            if matches!(self.h.decision(number), Decision::Ended(_)) {
                continue;
            }
            match d.held(number) {
                Some(decide::Holder::Slot) => self.seen.waited_for_a_slot += 1,
                Some(decide::Holder::Everything) => self.seen.held_by_everything += 1,
                _ => {}
            }
            if !self.h.let_on.contains(&number) {
                self.seen.never_let_on += 1;
            } else if dependencies_value(kept.flow, kept.work).as_deref() == Some(driver::WAITING) {
                self.seen.waited += 1;
            }
        }
    }

    /// Answers one of the things given to be done that can be answered now; says whether there was one.
    fn answer(&mut self, people: bool) -> bool {
        let doable: Vec<Todo> = (sorted(&self.h.outstanding).into_iter())
            .filter(|t| match &t.effect {
                Effect::Call { what, .. } => people || matches!(what, Call::Asked(Ask::Tool { .. })),
                _ => true,
            })
            .collect();
        if doable.is_empty() {
            return false;
        }
        let todo = self.pick(&doable);
        let event = match todo.effect {
            Effect::Round { run, cell, .. } => {
                let came = self.came(todo.work, cell.as_str());
                Event::Returned {
                    run,
                    came,
                    spent: self.spent(run),
                }
            }
            Effect::Call { call, .. } => Event::Done {
                call,
                result: format!("the result of {}", call.0),
            },
            Effect::Withdraw { call } => Event::Done {
                call,
                result: "withdrawn".to_string(),
            },
            Effect::Stop { run } => Event::Returned {
                run,
                came: Came::asks(vec![]),
                spent: self.spent(run),
            },
            Effect::Release { .. } => Event::Released,
        };
        self.act(Act::On(todo.work, event));
        true
    }

    /// What a round of the cell comes back with.
    fn came(&mut self, work: WorkNumber, cell: &str) -> Came {
        let kept = self.h.d().work(work).expect("the work is there");
        let flow = kept.flow.clone();
        if engine_cell(&flow, cell) {
            return if self.rng.chance(14) {
                Came::asks(vec![])
            } else {
                proper(&flow, cell)
            };
        }
        let place = flow.flow.place(&cell.into()).expect("the cell is in the flow");
        let outputs = flow.flow.cells()[place].outputs.clone();
        match self.rng.below(20) {
            0..=9 => {
                let mut handed = Vec::new();
                for block in outputs {
                    if !self.rng.chance(60) {
                        continue;
                    }
                    let value = (flow.blocks.get(&block))
                        .and_then(|b| b.values.clone())
                        .map(|values| values[self.rng.below(values.len())].clone());
                    handed.push(Handed {
                        content: format!("{block} by {cell} at {}", self.step),
                        block,
                        matters: self.rng.chance(70),
                        value,
                    });
                }
                Came::hands_in(handed)
            }
            10..=12 => {
                if self.rng.chance(50) {
                    hands_in(&[("request", None)])
                } else {
                    let first = outputs[0].as_str().to_string();
                    let value = flow.blocks.get(&outputs[0]).and_then(|b| b.values.as_ref());
                    // A choice with a wrong value, or a prose block with one.
                    let wrong = if value.is_some() { "maybe" } else { "yes" };
                    hands_in(&[(first.as_str(), Some(wrong))])
                }
            }
            13..=15 => {
                let n = 1 + self.rng.below(3);
                Came::asks((0..n).map(|i| tool(&format!("tool {i}"))).collect())
            }
            16..=18 => {
                let who = if self.rng.chance(50) {
                    Who::Owner
                } else {
                    Who::Architect
                };
                Came::asks(vec![question(who)])
            }
            _ => Came::asks(vec![]),
        }
    }

    /// What a round of the run took, as its agent reports it: told apart by the run and the step.
    fn spent(&self, run: RunId) -> Spent {
        Spent {
            usd_micros: run.0,
            input_tokens: self.step as u64,
            cached_tokens: 0,
            output_tokens: 1,
            ..Default::default()
        }
    }

    /// A person acts.
    fn person_acts(&mut self) {
        let going = self.going();
        let r = self.rng.below(100);
        if going.is_empty() || (55..63).contains(&r) {
            if self.h.d().works().count() < ITEMS {
                let title = format!("item {}", self.step);
                self.add(&title);
                self.seen.added += 1;
            } else {
                self.refuse();
            }
            return;
        }
        let work = self.pick(&going);
        let flow = self.h.d().work(work).expect("the work is there").flow.clone();
        let event = match r {
            0..=17 => {
                let people: Vec<String> = (flow.blocks.iter())
                    .filter(|(_, b)| b.writers.people)
                    .map(|(id, _)| id.0.clone())
                    .collect();
                // A flow people write nothing of, as the deliver flow, is only looked at.
                if people.is_empty() {
                    self.act(Act::On(work, Event::Look));
                    return;
                }
                let block = self.pick(&people);
                let value = (flow.blocks.get(&block.as_str().into()))
                    .and_then(|b| b.values.clone())
                    .map(|values| values[self.rng.below(values.len())].clone());
                wrote(&block, Outsider::Person, value.as_deref())
            }
            18..=29 => {
                let kept = self.h.d().work(work).expect("the work is there");
                if decide::paused(&kept.work.records) {
                    Event::Resumed
                } else {
                    Event::Paused {
                        by: Pauser::Owner,
                        why: None,
                    }
                }
            }
            30..=32 => {
                self.seen.cancels += 1;
                Event::Cancelled { why: None }
            }
            33..=41 => match self.h.decision(work) {
                Decision::Wait { at, on } if on.iter().any(|w| matches!(w, Waited::Limit { .. })) => {
                    Event::LetGo { cell: at }
                }
                _ => {
                    let cells = flow.flow.cells();
                    Event::LetGo {
                        cell: cells[self.rng.below(cells.len())].id.clone(),
                    }
                }
            },
            42..=54 => {
                let asked: Vec<Todo> = (sorted(&self.h.outstanding).into_iter())
                    .filter(|t| matches!(&t.effect, Effect::Call { what, .. } if !matches!(what, Call::Asked(Ask::Tool { .. }))))
                    .collect();
                if asked.is_empty() {
                    Event::Look
                } else {
                    let todo = self.pick(&asked);
                    let Effect::Call { call, .. } = todo.effect else {
                        unreachable!("a call was picked");
                    };
                    self.act(Act::On(
                        todo.work,
                        Event::Done {
                            call,
                            result: "the answer".to_string(),
                        },
                    ));
                    return;
                }
            }
            _ => {
                self.refuse();
                return;
            }
        };
        self.act(Act::On(work, event));
    }

    /// Something the driver must refuse, leaving the driver and the store as they were.
    fn refuse(&mut self) {
        let works: Vec<WorkNumber> = self.h.d().works().map(|k| k.head.number).collect();
        if works.is_empty() {
            return;
        }
        let pick = if self.done.is_empty() {
            self.rng.below(4)
        } else {
            self.rng.below(5)
        };
        let work = self.pick(&works);
        let (of, call) = if self.done.is_empty() {
            (work, CallId(10_000))
        } else {
            self.pick(&self.done.clone())
        };
        self.log
            .push(format!("{}: a refused act ({pick}) of work {}", self.step, work.0));
        self.h.refused(|d, now| match pick {
            0 => d
                .happened(now, work, wrote("nowhere", Outsider::Person, None))
                .map(drop),
            1 => {
                let late = Event::Done {
                    call: CallId(10_000),
                    result: "late".into(),
                };
                d.happened(now, work, late).map(drop)
            }
            2 => {
                let returned = Event::Returned {
                    run: RunId(10_000),
                    came: Came::asks(vec![]),
                    spent: Spent::default(),
                };
                d.happened(now, work, returned).map(drop)
            }
            3 => d.happened(now, WorkNumber(1000), Event::Look).map(drop),
            // The result of a call that has its result already.
            _ => {
                let again = Event::Done {
                    call,
                    result: "again".into(),
                };
                d.happened(now, of, again).map(drop)
            }
        });
        self.seen.refused += 1;
    }

    /// How many steps, once people stop, the works may take before they rest (property a-work-comes-to-rest): for each
    /// work, the bound the engine's own histories state for a work alone, from its flow's limits and cells.
    fn bound(&self) -> u64 {
        (self.h.d().works())
            .map(|k| {
                let flow = k.flow;
                let rounds = u64::from(flow.limits.returns + flow.limits.calls + 2);
                let mut before: u64 = 0;
                for c in flow.flow.cells() {
                    let s = (u64::from(c.limit) + u64::from(c.calls) + 2)
                        .saturating_mul(3 + c.runs_on.len() as u64 + before);
                    before = before.saturating_add(s);
                }
                (before + 1).saturating_mul(4 * rounds + 4)
            })
            .sum::<u64>()
            + 10
    }

    fn ending(&self) -> Ending {
        let works = (self.h.d().works())
            .map(|k| (k.head.clone(), by_time(k.work), self.h.decision(k.head.number)))
            .collect();
        Ending {
            works,
            ever: self.h.ever.clone(),
            undone: self.h.undone.clone(),
            answered: self.h.answered.clone(),
        }
    }
}

/// One history, with or without stops and failures: how it ended and what it saw.
fn one_history(seed: u64, faults: bool, log: &mut Vec<String>) -> (Ending, Seen) {
    let mut x = History::new(seed, faults);
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let first = 2 + x.rng.below(3);
        for i in 0..first {
            x.add(&format!("first {i}"));
        }
        let works: Vec<WorkNumber> = x.h.d().works().map(|k| k.head.number).collect();
        for work in works {
            if x.rng.chance(70) {
                x.act(Act::On(work, Event::Resumed));
            }
        }
        let people = x.rng.below(PEOPLE);
        for _ in 0..people {
            x.step(true);
        }
        // The last acts of people: the pause of everything is lifted, and every work left paused is let on.
        x.log.push("people stop".into());
        if x.h.d().everything_paused().is_some() {
            x.act(Act::Everything(false));
        }
        for work in x.going() {
            let kept = x.h.d().work(work).expect("the work is there");
            if decide::paused(&kept.work.records) {
                x.act(Act::On(work, Event::Resumed));
            }
        }
        let bound = x.bound();
        let mut steps = 0;
        while x.step(false) {
            steps += 1;
            assert!(steps <= bound, "the works have not come to rest within {bound} steps");
        }
        x.h.check();
        for kept in x.h.d().works() {
            let number = kept.head.number;
            match x.h.decision(number) {
                Decision::Ended(_) => x.seen.rested_ended += 1,
                Decision::Wait { on, .. } => {
                    assert!(!on.is_empty(), "work {} waits for nothing", number.0);
                    for waited in &on {
                        if let Waited::Call { on: None, .. } = waited {
                            panic!("work {} waits at rest for the engine: {on:?}", number.0);
                        }
                    }
                    x.seen.rested_waiting += 1;
                }
                // It can go on by itself no further, and is shown as waiting for the owner.
                Decision::Stalled { .. } => x.seen.rested_waiting += 1,
                other => panic!("work {} rests deciding {other:?}", number.0),
            }
        }
        for todo in x.h.d().pending() {
            let Effect::Call { what, .. } = &todo.effect else {
                panic!("at rest something other than a call is pending: {todo:?}");
            };
            assert!(
                !matches!(what, Call::Asked(Ask::Tool { .. })),
                "at rest a tool's call is pending: {todo:?}"
            );
        }
    }));
    *log = std::mem::take(&mut x.log);
    if let Err(e) = result {
        panic::resume_unwind(e);
    }
    (x.ending(), x.seen)
}

#[test]
fn many_works_keep_the_drivers_properties_with_and_without_stops_and_failures() {
    // The hook is the whole process's: this test must stay alone in its binary, or it hides the panics of the others.
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let mut failures = Vec::new();
    let mut seen = Seen::default();
    for seed in 0..SEEDS {
        let mut log = Vec::new();
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
            let (stopped, s) = one_history(seed, true, &mut log);
            let mut quiet = Vec::new();
            let (straight, _) = one_history(seed, false, &mut quiet);
            assert_eq!(stopped, straight, "the history with stops ended otherwise than without");
            s
        }));
        match outcome {
            Ok(s) => {
                seen.stops.kept += s.stops.kept;
                seen.stops.not_kept += s.stops.not_kept;
                seen.stops.deaths += s.stops.deaths;
                seen.reopens += s.reopens;
                seen.refused += s.refused;
                seen.added += s.added;
                seen.cancels += s.cancels;
                seen.limits += s.limits;
                seen.withdrawn += s.withdrawn;
                seen.stopped += s.stopped;
                seen.started_at_merge += s.started_at_merge;
                seen.waited += s.waited;
                seen.never_let_on += s.never_let_on;
                seen.let_on_as_added += s.let_on_as_added;
                seen.waited_for_a_slot += s.waited_for_a_slot;
                seen.held_by_everything += s.held_by_everything;
                seen.rested_waiting += s.rested_waiting;
                seen.rested_ended += s.rested_ended;
                seen.given_again += s.given_again;
                seen.changed += s.changed;
                seen.change_refused += s.change_refused;
                seen.reopened += s.reopened;
                seen.held_at_start += s.held_at_start;
            }
            Err(e) => {
                let message = (e.downcast_ref::<String>().cloned())
                    .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_string()))
                    .unwrap_or_default();
                failures.push((seed, message, log));
            }
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
    let counts = [
        ("kept as the driver stopped", seen.stops.kept),
        ("not kept as the driver stopped", seen.stops.not_kept),
        ("the process stopped after a call", seen.stops.deaths),
        ("opened anew between steps", seen.reopens),
        ("given again by an opening", seen.given_again),
        ("openings that paused everything for works going on", seen.held_at_start),
        ("refused", seen.refused),
        ("added while people acted", seen.added),
        ("cancelled", seen.cancels),
        ("calls at a limit", seen.limits),
        // A call is withdrawn only as a run at rest at it is abandoned or its work ends with a cell waiting for it,
        // which these histories all but never reach; the engine's own histories put it to the test.
        ("rounds stopped", seen.stopped),
        ("first rounds given by another work's merge", seen.started_at_merge),
        ("works waiting for their dependencies", seen.waited),
        ("works never let on", seen.never_let_on),
        ("items let on as they were added", seen.let_on_as_added),
        ("works waiting for a slot", seen.waited_for_a_slot),
        ("works held back by the pause of everything", seen.held_by_everything),
        ("works waiting at rest", seen.rested_waiting),
        ("works ended at rest", seen.rested_ended),
        ("items changed", seen.changed),
        ("items reopened", seen.reopened),
        ("changes of items refused or changing nothing", seen.change_refused),
    ];
    let never: Vec<&str> = counts.iter().filter(|(_, n)| *n == 0).map(|(what, _)| *what).collect();
    assert!(never.is_empty(), "never put to the test: {never:?} ({seen:?})");
}
