//! Simulated histories of whole works through `handle` only (principle simulated-histories). A driver holds what the
//! engine gave to be done and not yet done (rounds to end, calls to answer, rounds to stop, calls to withdraw) and at
//! each step does one of those things, or a person or the outside acts; at any step the engine may stop, and the
//! driver then forgets what it held and takes `pending` instead. The driver is `support`'s, which the histories of the
//! engineer's flow share; what an agent returns and what a person does on the shapes is here.
//!
//! What is checked after every step:
//! - `pending` equals, as a set, what the driver holds (entries engine-holds-no-state, a-stop-changes-nothing);
//! - at most one round is under way, and the deciding code never says the records are faulty;
//! - every effect held names a run and a call the records hold, and no round is given twice;
//! - the times of the moments never go back, the driver's clock never going back, and each moment's entries begin
//!   after the last's;
//! - an event the loop refuses leaves the work equal to what it was.
//!
//! And once people stop: the work rests within the bound stated on `History::bound` (entry a-work-comes-to-rest), at
//! rest the walk waits, is paused or has ended, and `pending` holds only calls for a person; a cancelled work ends with
//! nothing pending, and a look at it gives nothing. The same history ends with the same work run twice, and with its
//! stops as without them.
//!
//! What a step is, roughly. In the first 0 to 59 steps people act at a step with a chance of one in four; otherwise,
//! and always once they stop, the driver does one of the things it holds, each with an equal chance:
//! - a round under way ends: in half of them the agent hands in each block its cell may write with a chance of seven
//!   in ten and, in one in ten, a block it may not; a choice with one of its values in four of five, a wrong value or
//!   none otherwise; prose with a value in one in twenty; each mattering or not as a coin says. In three in ten it asks
//!   for one to three calls, a tool in seven of ten and a question for the owner or the architect otherwise; in two in
//!   ten it asks for nothing.
//! - a call has its result: always for a tool; for a person or the owner's call at a limit only while people act.
//! - a round given to be stopped ends, what it returns being dropped; a call given to be withdrawn is said to have its
//!   result, that it was withdrawn.
//!
//! A person's act is, in about equal shares of a tenth: a write of a block people may write (a choice with one of its
//! values); a write that must be refused (of a block people may not write, of a choice with a wrong value, of a block
//! the flow does not have); the outside writing main, mattering or not; letting a random cell go on; pausing or
//! resuming; the owner's word at the merge, a choice the merge's named test reads, written the other way; a result of a
//! call that has one already or of no call, or the return of a run that has no round under way; and, in one act in
//! fifty, cancelling the work. The owner's word is yes from the start in two histories of five. The engine stops at
//! any step with a chance of one in twelve, and with one in two at a step when the driver holds a stop or a withdrawal
//! not yet done.
//!
//! A failing case prints its seed and the steps it took; `one_history(seed, ..)` replays it.

mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::panic::{self, AssertUnwindSafe};

use decide::{CallId, Decision, Outsider, RunId, Who, decide};
use engine::{Ask, Came, Effect, Event, Handed, Spent, Work, pending};
use flow::{Compiled, Has, Source, compile};
use support::{Driver, Part, Rng, Seen};

/// How many histories are run.
const SEEDS: u64 = 200;

/// How many steps, at most, people act in.
const PEOPLE: usize = 60;

/// The flow's file for a shape: its order, and the blocks its cells use, each declared with who else may write it.
fn flow_file(order: &[&str], blocks: &BTreeSet<&str>) -> String {
    let order = order.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ");
    let declared: String = (blocks.iter())
        .map(|b| {
            let who = match *b {
                "request" | "opinions" => "people = true\n",
                "choice" => "values = [\"yes\", \"no\"]\npeople = true\n",
                "main" => "outside = \"main\"\n",
                _ => "",
            };
            format!("[blocks.{b}]\n{who}shown = true\n")
        })
        .collect();
    format!(
        "name = \"shape\"\norder = [{order}]\n[limits]\nruns = 2\nreturns = 2\ncalls = 3\n[lines.main]\n\
         [tests.may_merge]\nsays = \"The owner let the work merge\"\nkind = \"all\"\nreads = [\"go\"]\nis = \"yes\"\n\
         [blocks.go]\nvalues = [\"yes\", \"no\"]\npeople = true\n{declared}"
    )
}

fn agent(inputs: &[&str], outputs: &[&str], when: Option<&str>) -> (String, Vec<&'static str>) {
    let list = |l: &[&str]| l.iter().map(|b| format!("\"{b}\"")).collect::<Vec<_>>().join(", ");
    let when = when.map_or(String::new(), |w| format!("when = {w}\n"));
    let file = format!(
        "title = \"A cell\"\nkind = \"agent\"\nline = \"main\"\ntask = \"Do the cell's part.\"\n{when}\
         inputs = [{}]\noutputs = [{}]\n",
        list(inputs),
        list(outputs)
    );
    let used = inputs.iter().chain(outputs).map(|b| block_name(b)).collect();
    (file, used)
}

/// The block's name as a constant, so that the blocks a shape uses can be gathered.
fn block_name(b: &str) -> &'static str {
    ["request", "main", "opinions", "plan", "notes", "choice", "merged"]
        .into_iter()
        .find(|n| *n == b)
        .expect("a block of the histories")
}

/// The merge, the engine's own cell, after the cell that last writes the plan.
fn merge() -> String {
    "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\nrequires = [\"may_merge\"]\n\
     inputs = [\"plan\"]\noutputs = [\"merged\"]\n"
        .to_string()
}

/// One of the shapes of flow, each ending in the merge.
fn shape(n: usize) -> Compiled {
    let choice = Some(r#"{ block = "choice", is = "yes" }"#);
    let cells: Vec<(&str, (String, Vec<&'static str>))> = match n {
        // A chain.
        0 => vec![
            ("draft", agent(&["request", "main"], &["notes"], None)),
            ("plan", agent(&["notes"], &["plan"], None)),
        ],
        // A writer with a judge that writes opinions for it.
        1 => vec![
            ("write", agent(&["request", "opinions"], &["plan"], None)),
            ("judge", agent(&["plan"], &["opinions"], None)),
        ],
        // A cell with a condition on a choice an earlier cell makes.
        2 => vec![
            ("ask", agent(&["request"], &["choice", "plan"], None)),
            ("extra", agent(&["plan", "main"], &["notes"], choice)),
        ],
        // A cell reading and writing one block.
        _ => vec![
            ("draft", agent(&["request"], &["plan"], None)),
            ("refine", agent(&["plan", "opinions"], &["plan"], None)),
        ],
    };
    let mut order: Vec<&str> = cells.iter().map(|(id, _)| *id).collect();
    order.push("merge");
    let mut blocks: BTreeSet<&str> = cells.iter().flat_map(|(_, (_, used))| used.iter().copied()).collect();
    blocks.extend(["plan", "merged"]);
    let mut files: BTreeMap<String, String> = cells.iter().map(|(id, (f, _))| (id.to_string(), f.clone())).collect();
    files.insert("merge".to_string(), merge());
    let source = Source {
        flow: flow_file(&order, &blocks),
        cells: files,
    };
    let has = Has {
        sources: ["main".to_string()].into(),
        acts: ["merge".to_string()].into(),
    };
    compile(&source, &has).expect("the shape compiles")
}

/// What the histories make of the steps that are their own, on the shapes: what an agent returns, and what a person
/// or the outside does.
struct Shape {
    /// The owner's word at the merge, as last written: yes, or no where it was never written.
    go: bool,
    paused: bool,
}

impl Part for Shape {
    /// A round under way ends as its agent returns.
    fn returns(&mut self, d: &mut Driver, _: RunId, cell: &str) -> Came {
        let place = d.flow.flow.place(&cell.into()).expect("the cell is in the flow");
        let outputs = d.flow.flow.cells()[place].outputs.clone();
        match d.rng.below(10) {
            0..=4 => {
                let mut blocks = Vec::new();
                for b in &outputs {
                    if d.rng.chance(70) {
                        blocks.push(b.clone());
                    }
                }
                if d.rng.chance(10) {
                    blocks.push(["request", "merged", "opinions", "choice"][d.rng.below(4)].into());
                }
                let handed = (blocks.into_iter())
                    .map(|block| {
                        let value = if d.flow.blocks.get(&block).is_some_and(|b| b.values.is_some()) {
                            match d.rng.below(10) {
                                0..=7 => Some(if d.rng.chance(50) { "yes" } else { "no" }.to_string()),
                                8 => Some("maybe".to_string()),
                                _ => None,
                            }
                        } else if d.rng.chance(5) {
                            Some("yes".to_string())
                        } else {
                            None
                        };
                        Handed {
                            content: format!("{block} by {cell}"),
                            block,
                            matters: d.rng.chance(50),
                            value,
                        }
                    })
                    .collect();
                Came::HandsIn(handed)
            }
            5..=7 => {
                let n = 1 + d.rng.below(3);
                let asks = (0..n)
                    .map(|i| {
                        if d.rng.chance(70) {
                            Ask::Tool {
                                name: format!("tool {i}"),
                                input: "x".to_string(),
                            }
                        } else {
                            let who = if d.rng.chance(50) { Who::Owner } else { Who::Architect };
                            Ask::Person {
                                who,
                                question: "which?".to_string(),
                            }
                        }
                    })
                    .collect();
                Came::Asks(asks)
            }
            _ => Came::Asks(vec![]),
        }
    }

    /// A person or the outside acts.
    fn acts(&mut self, d: &mut Driver) -> Event {
        match d.rng.below(100) {
            0..=1 => Event::Cancelled,
            2..=13 => {
                let block = ["request", "opinions", "choice"][d.rng.below(3)];
                let value = (block == "choice").then(|| if d.rng.chance(50) { "yes" } else { "no" }.to_string());
                let matters = d.rng.chance(50);
                wrote(block, Outsider::Person, matters, value)
            }
            14..=25 => match d.rng.below(3) {
                0 => wrote("plan", Outsider::Person, true, None),
                1 => wrote("choice", Outsider::Person, true, Some("maybe".to_string())),
                _ => wrote("nowhere", Outsider::Person, true, None),
            },
            26..=37 => wrote("main", Outsider::Outside, d.rng.chance(50), None),
            38..=49 => {
                let cells = d.flow.flow.cells();
                Event::LetGo {
                    cell: cells[d.rng.below(cells.len())].id.clone(),
                }
            }
            50..=61 => {
                self.paused = !self.paused;
                if self.paused { Event::Paused } else { Event::Resumed }
            }
            62..=73 => self.owner_says(!self.go),
            74..=86 => {
                // A result of a call that has none to be given: one that has one already, or no call at all.
                let call = if !d.closed.is_empty() && d.rng.chance(70) {
                    d.closed[d.rng.below(d.closed.len())]
                } else {
                    CallId(10_000)
                };
                Event::Done {
                    call,
                    result: "late".to_string(),
                }
            }
            _ => Event::Returned {
                run: RunId(10_000),
                came: Came::Asks(vec![]),
                spent: Spent::default(),
            },
        }
    }
}

impl Shape {
    /// The owner's word at the merge, written as the choice the merge's named test reads.
    fn owner_says(&mut self, go: bool) -> Event {
        self.go = go;
        wrote(
            "go",
            Outsider::Person,
            true,
            Some(if go { "yes" } else { "no" }.to_string()),
        )
    }
}

/// How many steps, once people stop, the work may take before it rests (entry a-work-comes-to-rest).
///
/// Once people stop, the driver ends rounds and gives tools their results, and nothing else. A run's rounds are at
/// most `returns + 1` that are sent back or hand in what finishes it, and `calls + 1` that ask for calls, each of which
/// but the last makes at least one: past either limit the run waits for the owner, who no longer answers. A run takes
/// at most four steps a round, the round's end and the results of up to three calls, and four to be undone, its round
/// stopped and up to three calls withdrawn. A run of a cell is a way round unless one of its reasons is having no
/// record or a change that people, the outside or a cell before it made; nobody lets a cell go on any more, so between
/// two that are not a cell runs at most its limit `L` of ways round. Those that are not number at most one first run,
/// one for each input (the last write of it by people or the outside), and one for each run of a cell before it, which
/// may also abandon it once. So the cell at place `i` starts at most `S_i = (L + 2) * (3 + n_i + S_0 + ... + S_(i-1))`
/// runs, `n_i` its inputs.
fn bound(flow: &Compiled) -> u64 {
    let rounds = u64::from(flow.limits.returns + flow.limits.calls + 2);
    let mut before: u64 = 0;
    for c in flow.flow.cells() {
        let s = (u64::from(c.limit) + 2).saturating_mul(3 + c.inputs.len() as u64 + before);
        before = before.saturating_add(s);
    }
    (before + 1).saturating_mul(4 * rounds + 4)
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

/// One history, with or without stops: the work it ends with, what it saw and its steps.
fn one_history(seed: u64, stops: bool, log: &mut Vec<String>) -> (Work, Seen) {
    let mut rng = Rng::new(seed);
    let flow = shape(rng.below(4));
    let yes_at_first = rng.chance(40);
    let mut d = Driver::new(flow, rng, seed, stops);
    let mut shape = Shape {
        go: false,
        paused: false,
    };
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let first = if yes_at_first {
            shape.owner_says(true)
        } else {
            Event::Look
        };
        d.take(first);
        d.check();
        let people = d.rng.below(PEOPLE);
        for _ in 0..people {
            d.step(&mut shape, true);
            d.check();
        }
        d.note("people stop".into());
        let bound = bound(&d.flow);
        let mut steps = 0;
        while d.step(&mut shape, false) {
            d.check();
            steps += 1;
            assert!(steps <= bound, "the work has not come to rest within {bound} steps");
        }
        let decision = decide(&d.flow.flow, &d.work.records);
        let paused = decide::paused(&d.work.records);
        assert!(
            matches!(decision, Decision::Wait { .. } | Decision::Paused | Decision::Ended(_))
                || (paused && matches!(decision, Decision::GoOn { .. })),
            "the work rests deciding {decision:?}"
        );
        for effect in pending(&d.work) {
            let Effect::Call { call, .. } = &effect else {
                panic!("at rest something other than a call is pending: {effect:?}");
            };
            let called = d.work.records.called.iter().find(|c| c.call == *call).expect("a call");
            assert!(called.on.is_some(), "at rest a call waits for nobody: {effect:?}");
        }
        // A work cancelled after it merged has merged.
        if d.work.records.cancelled.is_some() && decision != Decision::Ended(decide::End::Merged) {
            assert_eq!(
                decision,
                Decision::Ended(decide::End::Cancelled),
                "a cancelled work goes on"
            );
            assert_eq!(pending(&d.work), vec![], "a cancelled work has something pending");
            d.take(Event::Look);
            d.check();
            assert_eq!(d.held, vec![], "a cancelled work was given more");
            d.seen.cancels += 1;
        }
    }));
    d.seen.abandons = d.work.records.abandoned.len();
    *log = std::mem::take(&mut d.log);
    if let Err(e) = result {
        panic::resume_unwind(e);
    }
    (d.work, d.seen)
}

#[test]
fn whole_works_keep_the_engines_properties_with_and_without_stops() {
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
            let (again, _) = one_history(seed, true, &mut quiet);
            assert_eq!(again, stopped, "the same history ended otherwise the second time");
            s
        }));
        match outcome {
            Ok(s) => {
                seen.limits += s.limits;
                seen.abandons += s.abandons;
                seen.withdrawn += s.withdrawn;
                seen.refused += s.refused;
                seen.stops += s.stops;
                seen.stops_undoing += s.stops_undoing;
                seen.stopped += s.stopped;
                seen.cancels += s.cancels;
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
    let every = seen.limits > 0
        && seen.abandons > 0
        && seen.withdrawn > 0
        && seen.refused > 0
        && seen.stops > 0
        && seen.stops_undoing > 0
        && seen.stopped > 0
        && seen.cancels > 0;
    assert!(every, "a class of step never occurred: {seen:?}");
}
