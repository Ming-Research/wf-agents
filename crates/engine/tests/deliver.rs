//! The deliver flow as the repository defines it (`flow/deliver`), through `handle` only: the flow of a deliverable
//! item, which has nothing to do of its own and is made of the items it depends on. Its one cell is an act of the
//! engine's own, held by a precondition until the outside (the driver) writes that those items have all merged; it then
//! runs once, and the work has ended as merged.
//!
//! Beside a few examples, simulated histories on the shared driver (`support`), which checks after every step what the
//! engine keeps for any flow (what is pending, one round at a time, no fault in the records, no finished run lacking a
//! block it must write). What they check of the deliver flow itself:
//!
//! 1. No run of the cell starts unless the dependencies block's current value, as the run starts, is the one the
//!    precondition looks for (entry conditions-and-preconditions).
//! 2. The cell finishes at most one run, and once it has, the work has ended as merged (entry work-has-no-position);
//!    every run of it starts for its having no record, so that what people write of the item, its background, runs
//!    nothing.
//! 3. Once people stop, the outside says the items have merged and the owner lets the work on and lets on any run at a
//!    limit, the work ends within `CONTENT` steps: merged, with the one finished run, unless it was cancelled (entry
//!    a-work-comes-to-rest).
//! 4. The same history ends with the same work run twice, and with its stops as without them (entry
//!    a-stop-changes-nothing).
//!
//! What a history is. It begins with the outside writing the dependencies block, waiting or merged as a coin says,
//! and, in half of them, with the owner's pause the driver adds a work with. Then, for 0 to 59 steps, the shared
//! driver's random steps, people acting at a step with a chance of one in four and the engine stopping at any step: the
//! act's round fails in one in ten while people act; otherwise it hands in what it writes, and in one in eight a block
//! it may not write beside it, which sends it back. A person's or the outside's act is: the outside writing the
//! dependencies block, merged in three of five, in three of ten acts; a person writing the dependencies block or what
//! the cell writes, which must be refused, in about one in twelve; a person writing the background, which the owner
//! reads and no cell does, in one in fifty; pausing or resuming in about one in six; letting the cell go
//! on in about one in twelve; cancelling in one in fifty; a look otherwise.
//!
//! The histories of the engine's own properties on every shape of flow are in `histories.rs`; the shape these use is
//! the one cell of an act with a precondition on an outside block, which those do not draw. The engineer's histories
//! (`engineer/`) need a check and judges, which this flow has none of, so they are not run on it.

#[allow(dead_code)]
mod support;

use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use decide::{BlockId, Cause, Decision, End, Outsider, Pauser, RunId, TestKind, Waited, Who, decide};
use engine::{Call, Came, Effect, Event, Given, Handed, Spent, Time, Work, handle};
use flow::{Compiled, Has, Kind, compile, read};
use support::{Driver, Part, Rng};

/// How many histories are run.
const SEEDS: u64 = 200;

/// How many steps, at most, people act in.
const PEOPLE: usize = 60;

/// The most steps the content continuation takes: the owner letting the work on, the outside saying the items have
/// merged, the owner letting on a run at its limit, an abandoned or sent back round undone, and the round that
/// finishes the run, each a few times over.
const CONTENT: usize = 30;

/// The deliver flow as the repository defines it, compiled against what the engine has for it.
fn deliver() -> Compiled {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/deliver");
    let source = read(&dir).expect("the deliver flow is read");
    let has = Has {
        sources: ["dependencies".to_string()].into(),
        acts: ["deliver".to_string()].into(),
    };
    compile(&source, &has).expect("the deliver flow compiles")
}

/// The block the outside writes: whether the items the work's item is made of have merged.
fn dependencies(flow: &Compiled) -> BlockId {
    let (block, _) = (flow.blocks.iter())
        .find(|(_, b)| b.writers.outside.as_deref() == Some("dependencies"))
        .expect("the flow has a dependencies block");
    block.clone()
}

/// The value the cell's precondition looks for in the dependencies block.
fn wanted(flow: &Compiled) -> String {
    let cell = &flow.flow.cells()[0];
    let test = &flow.flow.tests()[&cell.requires[0]];
    test.is.clone()
}

fn wrote(block: &BlockId, by: Outsider, value: Option<&str>) -> Event {
    Event::Wrote {
        block: block.clone(),
        by,
        content: format!("{block} by {by:?}"),
        matters: true,
        value: value.map(str::to_string),
    }
}

/// What the cell hands in: each of its outputs, as an act of the engine's writes them.
fn delivered(flow: &Compiled) -> Vec<Handed> {
    (flow.flow.cells()[0].outputs.iter())
        .map(|block| Handed {
            block: block.clone(),
            content: format!("{block}, as the act wrote it"),
            matters: true,
            value: None,
        })
        .collect()
}

#[test]
fn the_deliver_flow_is_one_act_of_the_engines_held_until_the_items_it_is_made_of_have_merged() {
    let flow = deliver();
    assert_eq!(flow.name, "deliver");
    let [cell] = flow.flow.cells() else {
        panic!("the deliver flow has more than its one cell: {:?}", flow.flow.cells());
    };
    assert_eq!(cell.id.as_str(), "V1");
    assert_eq!(cell.applies, None, "the last cell has no condition");
    let spec = &flow.cells[0];
    assert_eq!((spec.title.as_str(), spec.kind), ("Deliver", Kind::Engine));
    assert_eq!(spec.act.as_deref(), Some("deliver"));
    assert_eq!(cell.requires, ["all_delivered".into()]);
    let test = &flow.flow.tests()[&cell.requires[0]];
    assert_eq!(
        (test.kind, test.reads.clone()),
        (TestKind::All, vec![dependencies(&flow)])
    );
    assert_eq!(test.is, "merged");
    assert_eq!(flow.tests[&cell.requires[0]], "The items it is made of are merged");
    assert_eq!(cell.outputs, ["delivered".into()]);
    assert!(
        flow.blocks[&BlockId::from("delivered")].readers.shown,
        "the owner reads what was delivered"
    );
    // What the item is to deliver: people write it, the owner reads it, and no cell does, so that the cell takes no
    // input from it and an edit of it runs nothing.
    assert_eq!(cell.inputs, Vec::<BlockId>::new());
    let background = &flow.blocks[&BlockId::from("background")];
    assert!(background.writers.people && background.readers.shown);
    assert_eq!(background.values, None);
    assert_eq!(background.readers.cells, vec![]);
}

#[test]
fn a_work_waits_at_its_cell_until_its_items_have_merged_then_runs_it_once_and_has_ended() {
    let flow = deliver();
    let deps = dependencies(&flow);
    let mut work = Work::default();
    let mut at = 1_800_000_000_000;
    let mut take = |work: &mut Work, event| {
        at += 1000;
        handle(&flow, work, Time(at), event).expect("the event is taken")
    };
    assert_eq!(
        take(&mut work, wrote(&deps, Outsider::Outside, Some("waiting"))),
        vec![]
    );
    let waits = Decision::Wait {
        at: "V1".into(),
        on: vec![Waited::Precondition {
            test: "all_delivered".into(),
            who: Who::Owner,
        }],
    };
    assert_eq!(decide(&flow.flow, &work.records), waits);
    // Said again, it still waits.
    assert_eq!(
        take(&mut work, wrote(&deps, Outsider::Outside, Some("waiting"))),
        vec![]
    );
    assert_eq!(decide(&flow.flow, &work.records), waits);
    let effects = take(&mut work, wrote(&deps, Outsider::Outside, Some("merged")));
    let [
        Effect::Round {
            run,
            cell,
            number: 1,
            given: Given::Task,
        },
    ] = effects.as_slice()
    else {
        panic!("the cell's first round is not given: {effects:?}");
    };
    assert_eq!(cell.as_str(), "V1");
    let came = Came::HandsIn(delivered(&flow));
    let effects = take(
        &mut work,
        Event::Returned {
            run: *run,
            came,
            spent: Spent::default(),
        },
    );
    assert_eq!(effects, vec![]);
    assert_eq!(decide(&flow.flow, &work.records), Decision::Ended(End::Merged));
    assert_eq!(work.records.started.len(), 1);
    assert_eq!(work.records.finished.len(), 1);
    // Nothing more is decided for a work that has ended.
    assert_eq!(take(&mut work, Event::Look), vec![]);
}

/// What the histories of the deliver flow make of the steps that are their own.
struct Deliverable {
    deps: BlockId,
    /// The value the precondition looks for.
    merged: String,
    /// Whether people have stopped: the act then hands in what it writes and nothing more.
    content: bool,
}

impl Part for Deliverable {
    fn fails(&mut self, d: &mut Driver, _: bool, people: bool) -> bool {
        people && d.rng.chance(10)
    }

    fn returns(&mut self, d: &mut Driver, _: RunId, _: &str) -> Came {
        let mut handed = delivered(&d.flow);
        if !self.content && d.rng.chance(12) {
            handed.push(Handed {
                block: self.deps.clone(),
                content: "not the act's to write".to_string(),
                matters: true,
                value: Some(self.merged.clone()),
            });
        }
        Came::HandsIn(handed)
    }

    fn acts(&mut self, d: &mut Driver) -> Event {
        match d.rng.below(100) {
            0..=29 => {
                let value = if d.rng.chance(60) {
                    self.merged.as_str()
                } else {
                    "waiting"
                };
                wrote(&self.deps, Outsider::Outside, Some(value))
            }
            30..=34 => wrote(&self.deps, Outsider::Person, Some(self.merged.as_str())),
            35..=37 => wrote(&BlockId::from("delivered"), Outsider::Person, None),
            38..=39 => wrote(&BlockId::from("background"), Outsider::Person, None),
            40..=56 => {
                if d.paused {
                    Event::Resumed
                } else {
                    Event::Paused {
                        by: Pauser::Owner,
                        why: None,
                    }
                }
            }
            57..=64 => Event::LetGo { cell: "V1".into() },
            65..=66 => Event::Cancelled,
            _ => Event::Look,
        }
    }
}

/// What the histories reached, so that each property is known to have been put to the test.
#[derive(Default, Debug)]
struct Seen {
    /// Steps after which the work waited at its cell for the items to merge.
    waited: usize,
    /// Runs of the cell that started.
    started: usize,
    /// Histories in which the cell finished a run while people still acted.
    delivered_early: usize,
    /// Rounds given after the one before was sent back.
    sent_back: usize,
    /// Rounds that failed.
    failed: usize,
    stops: usize,
    cancels: usize,
    /// Histories whose continuation ended merged.
    merged: usize,
    /// Writes of the background by a person.
    background: usize,
}

/// What must hold of the deliver flow after every step: properties 1 and 2.
fn check(d: &Driver, part: &Deliverable) {
    let r = &d.work.records;
    for s in &r.started {
        assert_eq!(
            s.because,
            [Cause::NoRecord],
            "{:?} started for what was written, though the cell reads nothing",
            s.run
        );
        let value = (r.written.iter())
            .filter(|w| w.block == part.deps && w.at < s.at)
            .max_by_key(|w| w.at)
            .and_then(|w| w.value.clone());
        assert_eq!(
            value.as_deref(),
            Some(part.merged.as_str()),
            "{:?} started while the items it is made of had not all merged",
            s.run
        );
    }
    assert!(r.finished.len() <= 1, "the cell finished {} runs", r.finished.len());
    if !r.finished.is_empty() {
        assert_eq!(
            decide(&d.flow.flow, r),
            Decision::Ended(End::Merged),
            "the cell has a record and the work goes on"
        );
    }
}

/// Notes what the step reached.
fn reached(d: &Driver, seen: &mut Seen) {
    if let Decision::Wait { on, .. } = decide(&d.flow.flow, &d.work.records)
        && on.iter().any(|w| matches!(w, Waited::Precondition { .. }))
    {
        seen.waited += 1;
    }
}

/// One history, with or without stops: the work it ends with, and what it reached.
fn one_history(seed: u64, stops: bool, log: &mut Vec<String>) -> (Work, Seen) {
    let flow = deliver();
    let mut rng = Rng::new(seed);
    let merged_at_first = rng.chance(50);
    let paused_at_first = rng.chance(50);
    let mut part = Deliverable {
        deps: dependencies(&flow),
        merged: wanted(&flow),
        content: false,
    };
    let mut d = Driver::new(flow, rng, seed, stops);
    let mut seen = Seen::default();
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        if paused_at_first {
            d.take(Event::Paused {
                by: Pauser::Owner,
                why: None,
            });
        }
        let value = if merged_at_first {
            part.merged.clone()
        } else {
            "waiting".to_string()
        };
        d.take(wrote(&part.deps, Outsider::Outside, Some(&value)));
        d.check();
        check(&d, &part);
        let people = d.rng.below(PEOPLE);
        for _ in 0..people {
            d.step(&mut part, true);
            d.check();
            check(&d, &part);
            reached(&d, &mut seen);
        }
        if !d.work.records.finished.is_empty() {
            seen.delivered_early += 1;
        }
        d.note("people stop".into());
        part.content = true;
        let mut steps = 0;
        loop {
            let r = &d.work.records;
            let deps_merged = (r.written.iter())
                .filter(|w| w.block == part.deps)
                .max_by_key(|w| w.at)
                .is_some_and(|w| w.value.as_deref() == Some(part.merged.as_str()));
            let at_limit = (d.held.iter())
                .find(|e| {
                    matches!(
                        e,
                        Effect::Call {
                            what: Call::AtLimit(_),
                            ..
                        }
                    )
                })
                .cloned();
            if matches!(decide(&d.flow.flow, r), Decision::Ended(_)) {
                // What is left to undo, a round to stop or a call to withdraw, is done.
                if !d.step(&mut part, false) {
                    break;
                }
            } else if d.paused {
                d.take(Event::Resumed);
            } else if !deps_merged && r.cancelled.is_none() {
                d.take(wrote(&part.deps, Outsider::Outside, Some(&part.merged)));
            } else if let Some(effect) = at_limit {
                // The owner lets the run go on at its limit.
                d.carry_out(effect, &mut part, false);
            } else if !d.step(&mut part, false) {
                break;
            }
            d.check();
            check(&d, &part);
            steps += 1;
            assert!(steps <= CONTENT, "the work has not come to rest within {CONTENT} steps");
        }
        let r = &d.work.records;
        let decision = decide(&d.flow.flow, r);
        if r.cancelled.is_some() && r.finished.is_empty() {
            assert_eq!(decision, Decision::Ended(End::Cancelled), "a cancelled work goes on");
            seen.cancels += 1;
        } else {
            assert_eq!(
                decision,
                Decision::Ended(End::Merged),
                "the work rests without being delivered"
            );
            assert_eq!(
                r.finished.len(),
                1,
                "the work ended with other than one run of its cell"
            );
            seen.merged += 1;
        }
        assert_eq!(
            engine::pending(&d.work),
            vec![],
            "the work ended with something pending"
        );
    }));
    let r = &d.work.records;
    seen.started = r.started.len();
    seen.failed = (d.work.back.values())
        .filter(|b| matches!(b, engine::Back::Failed { .. }))
        .count();
    seen.sent_back = (d.work.given.values())
        .filter(|g| matches!(g, Given::SentBack(_)))
        .count();
    seen.stops = d.seen.stops;
    seen.background = (r.written.iter()).filter(|w| w.block.as_str() == "background").count();
    *log = std::mem::take(&mut d.log);
    if let Err(e) = result {
        panic::resume_unwind(e);
    }
    (d.work, seen)
}

#[test]
fn whole_works_on_the_deliver_flow_wait_for_their_items_deliver_once_and_come_to_rest_with_and_without_stops() {
    let mut failures = Vec::new();
    let mut seen = Seen::default();
    for seed in 0..SEEDS {
        let mut log = Vec::new();
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
            let (stopped, s) = one_history(seed, true, &mut log);
            let mut quiet = Vec::new();
            let (straight, _) = one_history(seed, false, &mut quiet);
            assert_eq!(stopped, straight, "the history with stops ended otherwise than without");
            let (again, _) = one_history(seed, true, &mut quiet);
            assert_eq!(again, stopped, "the same history ended otherwise the second time");
            s
        }));
        match outcome {
            Ok(s) => {
                seen.waited += s.waited;
                seen.started += s.started;
                seen.delivered_early += s.delivered_early;
                seen.sent_back += s.sent_back;
                seen.failed += s.failed;
                seen.stops += s.stops;
                seen.cancels += s.cancels;
                seen.merged += s.merged;
                seen.background += s.background;
            }
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
    let every = seen.waited > 0
        && seen.started > 0
        && seen.delivered_early > 0
        && seen.sent_back > 0
        && seen.failed > 0
        && seen.stops > 0
        && seen.cancels > 0
        && seen.merged > 0
        && seen.background > 0;
    assert!(every, "a class of step never occurred: {seen:?}");
}
