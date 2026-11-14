//! Simulated histories of works whose cells put questions to the owner (principle simulated-histories; design
//! document `data-and-context`, 32 to 36): agents ask, ask anew and withdraw questions at any step, waiting for them
//! or not, and now and then ask for a tool; the engine asks one of its own beside a round now and then, and the owner answers at any step, a version
//! that is the latest or an earlier one, with the option the work goes by or another, or with no such option. The
//! driver is `support`'s; at any step the engine may stop.
//!
//! What is checked after every step, beside what the driver checks:
//! - a cell is never entered while a question of its own that it waits for stands with no answer to its latest version;
//! - the merge never begins while `asked` is `waiting`, nor while any question stands unanswered;
//! - `asked` says `waiting` exactly while a question stands unanswered, and is otherwise `answered` or never written;
//! - an answer runs the asking cell again, by its block of an answer, exactly where it answers the question's latest
//!   version with another option than the one the work went by and the cell has a trigger an answer runs; an answer
//!   to an earlier version writes nothing.
//!
//! Once people stop, the work comes to rest within a bound, whatever its agents asked for: a cell entered for an answer
//! that asks for a tool first is not entered again for the same answer before the tool's result.
//!
//! The histories must reach answers that run the cell, answers that run nothing, answers to an earlier version, a
//! question asked anew and one withdrawn, a question of the engine's, a waiting cell let go on by its answer, and the
//! merge begun after `asked` was `waiting`: each is counted, and a test of no history reaching one fails.
//!
//! A failing case prints its seed and the steps it took.

// The driver's counts of the engine's own histories are not read here.
#[allow(dead_code)]
mod support;

use std::io::Write as _;
use std::panic::{self, AssertUnwindSafe};

use decide::questions::questions;
use decide::{Cause, Outsider, Records, RunId, Seq};
use engine::{Ask, Came, Event, Handed, Question, Way, Work};
use flow::{Compiled, Has, Source, compile};
use support::{Driver, Part, Rng};

/// How many histories are run.
const SEEDS: u64 = 120;

/// How many steps people act in.
const PEOPLE: usize = 120;

/// The most steps a history takes once people stop.
const AFTER: usize = 400;

/// A research that what people ask for and the owner's answers to it run, writing the plan; an implementation that the
/// plan and the answers to it run, writing the code; a review of the code; and the merge, which requires every
/// question to be answered.
fn asking() -> Compiled {
    let flow = r#"
        name = "asking"
        order = ["research", "implement", "review", "merge"]
        [limits]
        runs = 3
        returns = 2
        calls = 3
        [lines.main]
        [blocks.request]
        people = true
        [blocks.research_answered]
        outside = "answer"
        [blocks.implement_answered]
        outside = "answer"
        [blocks.asked]
        values = ["waiting", "answered"]
        default = "answered"
        outside = "asked"
        shown = true
        [blocks.asked.writes]
        answered = "Every question has the owner's answer."
        waiting = "Waits for the owner's answer to {questions}."
        between = ", "
        [blocks.plan]
        [blocks.code]
        [blocks.verdict]
        [blocks.merged]
        shown = true
    "#;
    let agent = |inputs: &str, outputs: &str| {
        format!(
            "title = \"A cell\"\nkind = \"agent\"\neffort = \"high\"\nline = \"main\"\ntask = \"Do the cell's part.\"\n\
             inputs = [{inputs}]\noutputs = [{outputs}]\n"
        )
    };
    let merge = "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\nrequires = { asked = \"answered\" }\n\
                 inputs = [\"verdict\"]\noutputs = [\"merged\"]\n";
    let cells = [
        ("research", agent("\"request\", \"research_answered\"", "\"plan\"")),
        ("implement", agent("\"plan\", \"implement_answered\"", "\"code\"")),
        ("review", agent("\"code\"", "\"verdict\"")),
        ("merge", merge.to_string()),
    ];
    let source = Source {
        flow: flow.to_string(),
        cells: cells.into_iter().map(|(id, text)| (id.to_string(), text)).collect(),
        ..Default::default()
    };
    let has = Has {
        sources: ["answer".to_string(), "asked".to_string()].into(),
        acts: ["merge".to_string()].into(),
        ..Default::default()
    };
    compile(&source, &has).expect("the definition compiles")
}

/// The question the engine itself asks in these histories.
const ENGINES: &str = "protected_files";

/// The questions each cell's agent may ask.
fn names(cell: &str) -> &'static [&'static str] {
    match cell {
        "research" => &["which", "where"],
        "implement" => &["how"],
        _ => &[],
    }
}

/// A question asked with two options, its words told apart by `n` so that asking it anew may change them or not.
fn asked(name: &str, waits: bool, recommends: Option<u64>, n: usize) -> Question {
    let way = |does: &str| Way {
        does: does.to_string(),
        follows: format!("what follows from {does}"),
    };
    Question::Asked {
        name: name.to_string(),
        waits,
        recommends,
        ask: format!("Which way for {name}, the {n}th time?"),
        options: vec![way("the short way"), way("the long way")],
    }
}

/// What the histories reached.
#[derive(Default, Debug)]
struct Reached {
    ran: usize,
    ran_nothing: usize,
    earlier: usize,
    asked_anew: usize,
    withdrawn: usize,
    engines: usize,
    let_go: usize,
    merged_after_waiting: usize,
}

/// The agents ask, and the owner answers.
struct Asking;

impl Asking {
    /// The questions an agent of the cell writes now: none, one or two, each asked, asked anew or withdrawn.
    fn questions(&self, d: &mut Driver, cell: &str) -> Vec<Question> {
        let names = names(cell);
        let mut out = Vec::new();
        for name in names {
            if !d.rng.chance(35) {
                continue;
            }
            if d.rng.chance(25) {
                out.push(Question::Withdrawn {
                    name: name.to_string(),
                    why: "Settled by the request.".to_string(),
                });
            } else {
                let waits = d.rng.chance(50);
                let recommends = Some(1 + d.rng.below(2) as u64);
                out.push(asked(name, waits, recommends, d.rng.below(3)));
            }
        }
        out
    }
}

impl Part for Asking {
    fn fails(&mut self, d: &mut Driver, _: bool, people: bool) -> bool {
        people && d.rng.chance(4)
    }

    fn returns(&mut self, d: &mut Driver, _: RunId, cell: &str) -> Came {
        // Now and then an agent that may ask asks for a tool first, as one entered for an answer may.
        if !names(cell).is_empty() && d.rng.chance(15) {
            return Came::asks(vec![Ask::Tool {
                name: "look".to_string(),
                input: format!("what {cell} looks up"),
            }]);
        }
        let block = match cell {
            "research" => "plan",
            "implement" => "code",
            "review" => "verdict",
            _ => "merged",
        };
        let mut came = Came::hands_in(vec![Handed {
            block: block.into(),
            content: format!("the {block} at {}", d.clock),
            matters: d.rng.chance(80),
            value: None,
        }]);
        came.said.questions = self.questions(d, cell);
        // Now and then the engine asks its own beside the round of a cell whose agent may change the work, or
        // withdraws it.
        if cell != "merge" && d.rng.chance(8) {
            let question = if d.rng.chance(25) {
                Question::Withdrawn {
                    name: ENGINES.to_string(),
                    why: "The work no longer changes a protected file.".to_string(),
                }
            } else {
                asked(ENGINES, true, None, d.rng.below(2))
            };
            came.engine = vec![question];
        }
        came
    }

    fn acts(&mut self, d: &mut Driver) -> Event {
        let r = &d.work.records;
        let versions: Vec<(String, Seq)> = (r.questions.iter())
            .filter(|q| !q.withdrawn)
            .map(|q| (q.name.clone(), q.at))
            .collect();
        match d.rng.below(100) {
            0..=49 if !versions.is_empty() => {
                // The latest version of a question most of the time, an earlier one at times.
                let (name, mut version) = versions[d.rng.below(versions.len())].clone();
                if d.rng.chance(70)
                    && let Some(q) = questions(r).get(name.as_str())
                    && !q.latest.withdrawn
                {
                    version = q.latest.at;
                }
                let option = 1 + d.rng.below(3) as u64;
                let why = d.rng.chance(50).then(|| "Because.".to_string());
                Event::Answer {
                    question: name,
                    version,
                    option,
                    why,
                }
            }
            50..=69 => Event::Wrote {
                block: "request".into(),
                by: Outsider::Person,
                content: format!("the request at {}", d.clock),
                matters: true,
                value: None,
            },
            70..=74 if d.paused => Event::Resumed,
            70..=74 => Event::Paused {
                by: decide::Pauser::Owner,
                why: None,
            },
            75 => Event::Cancelled { why: None },
            _ => Event::Look,
        }
    }
}

/// The option the work went by on the question as the records stood: the one last taken on its latest version, else
/// the one it recommends; none while its cell waits for the answer.
fn goes_by(work: &Work, records: &Records, name: &str) -> Option<u64> {
    let standing = questions(records);
    let q = standing.get(name)?;
    if let Some(answer) = q.answer {
        return Some(answer.option);
    }
    if q.latest.waits {
        return None;
    }
    match work.question(q.latest) {
        Some(Question::Asked { recommends, .. }) => *recommends,
        _ => None,
    }
}

/// The value `asked` has as the records stand: its latest version's, or none where it was never written.
fn asked_value(records: &Records) -> Option<&str> {
    (records.written.iter())
        .filter(|w| w.block.as_str() == "asked")
        .max_by_key(|w| w.at)
        .and_then(|w| w.value.as_deref())
}

/// What must hold of the questions after every step.
fn check(work: &Work) {
    let r = &work.records;
    for started in &r.started {
        let before = r.before(started.at);
        let standing = questions(&before);
        for (name, q) in &standing {
            assert!(
                !(q.latest.cell == started.cell && q.holds()),
                "{} is entered at {:?} while its question {name} waits for its answer",
                started.cell,
                started.at
            );
        }
        if started.cell.as_str() == "merge" {
            assert_ne!(
                asked_value(&before),
                Some("waiting"),
                "the merge begins while asked is waiting"
            );
            let unanswered: Vec<&&str> = standing
                .iter()
                .filter(|(_, q)| q.unanswered())
                .map(|(n, _)| n)
                .collect();
            assert!(
                unanswered.is_empty(),
                "the merge begins while {unanswered:?} stand unanswered"
            );
        }
    }
    let unanswered = questions(r).values().any(|q| q.unanswered());
    let value = asked_value(r);
    assert_eq!(
        value == Some("waiting"),
        unanswered,
        "asked is {value:?} while a question stands unanswered is {unanswered}"
    );
    for answer in &r.answers {
        let before = r.before(answer.at);
        let standing = questions(&before);
        let asking = standing.get(answer.question.as_str());
        let latest = asking.is_some_and(|q| q.latest.at == answer.version);
        // Only the research and the implementation have a trigger an answer runs; the review's question, which the
        // engine asks, holds the work at the review until it is answered, and its answer runs nothing.
        let answerable = asking.is_some_and(|q| matches!(q.latest.cell.as_str(), "research" | "implement"));
        let went_by = goes_by(work, &before, &answer.question);
        let next = Seq(answer.at.0 + 1);
        // What the answer wrote is at the place after it, in the same thing that happened: no later one begins there.
        let same = !work.moments.iter().any(|m| m.from == next);
        let turned = same
            && (r.written.iter())
                .any(|w| w.at == next && w.by == Outsider::Outside && w.block.as_str().ends_with("_answered"));
        assert_eq!(
            turned,
            latest && answerable && went_by != Some(answer.option),
            "the answer at {:?} to version {:?} of {}, option {}, the work going by {went_by:?}",
            answer.at,
            answer.version,
            answer.question,
            answer.option
        );
        if !latest {
            assert!(
                !(same && r.written.iter().any(|w| w.at == next)),
                "the answer at {:?} to an earlier version wrote something",
                answer.at
            );
        }
    }
}

/// One history of the seed, with the engine stopping or not.
fn one_history(seed: u64, stops: bool, log: &mut Vec<String>) -> (Work, Reached) {
    let mut d = Driver::new(asking(), Rng::new(seed), seed, stops);
    let mut part = Asking;
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        d.take(Event::Wrote {
            block: "request".into(),
            by: Outsider::Person,
            content: "the first request".to_string(),
            matters: true,
            value: None,
        });
        d.check();
        check(&d.work);
        for _ in 0..PEOPLE {
            d.step(&mut part, true);
            d.check();
            check(&d.work);
        }
        if d.paused {
            d.take(Event::Resumed);
        }
        let mut rested = false;
        for _ in 0..AFTER {
            if !d.step(&mut part, false) {
                rested = true;
                break;
            }
            d.check();
            check(&d.work);
        }
        assert!(
            rested,
            "the work has not come to rest within {AFTER} steps of people stopping"
        );
    }));
    *log = std::mem::take(&mut d.log);
    if let Err(e) = result {
        panic::resume_unwind(e);
    }
    let reached = reached(&d.work);
    (d.work, reached)
}

/// What the history reached, read from its records.
fn reached(work: &Work) -> Reached {
    let r = &work.records;
    let mut reached = Reached::default();
    for answer in &r.answers {
        let before = r.before(answer.at);
        let latest = questions(&before)
            .get(answer.question.as_str())
            .is_some_and(|q| q.latest.at == answer.version);
        let next = Seq(answer.at.0 + 1);
        let same = !work.moments.iter().any(|m| m.from == next);
        let turned = same && (r.written.iter()).any(|w| w.at == next && w.block.as_str().ends_with("_answered"));
        match (latest, turned) {
            (false, _) => reached.earlier += 1,
            (true, true) => reached.ran += 1,
            (true, false) => reached.ran_nothing += 1,
        }
    }
    for q in &r.questions {
        let earlier = (r.questions.iter()).any(|e| e.name == q.name && e.at < q.at && !e.withdrawn);
        if earlier && !q.withdrawn {
            reached.asked_anew += 1;
        }
        reached.withdrawn += usize::from(q.withdrawn);
        reached.engines += usize::from(q.engine);
    }
    for started in &r.started {
        let before = r.before(started.at);
        let waited = (before.questions.iter()).any(|q| q.cell == started.cell && q.waits);
        let for_answer = (started.because.iter())
            .any(|c| matches!(c, Cause::Changed { block, .. } if block.as_str().ends_with("_answered")));
        reached.let_go += usize::from(waited && for_answer);
        let was_waiting =
            (before.written.iter()).any(|w| w.block.as_str() == "asked" && w.value.as_deref() == Some("waiting"));
        reached.merged_after_waiting += usize::from(started.cell.as_str() == "merge" && was_waiting);
    }
    reached
}

#[test]
fn questions_and_answers_at_any_step_keep_the_waits_and_the_merge_gate() {
    // The hook is the whole process's: this test must stay alone in its binary, or it hides the panics of the others.
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let mut failures = Vec::new();
    let mut seen = Reached::default();
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
                seen.ran += s.ran;
                seen.ran_nothing += s.ran_nothing;
                seen.earlier += s.earlier;
                seen.asked_anew += s.asked_anew;
                seen.withdrawn += s.withdrawn;
                seen.engines += s.engines;
                seen.let_go += s.let_go;
                seen.merged_after_waiting += s.merged_after_waiting;
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
    let _ = writeln!(std::io::stderr(), "the histories of questions reached: {seen:?}");
    let every = seen.ran > 0
        && seen.ran_nothing > 0
        && seen.earlier > 0
        && seen.asked_anew > 0
        && seen.withdrawn > 0
        && seen.engines > 0
        && seen.let_go > 0
        && seen.merged_after_waiting > 0;
    assert!(every, "a class of step never occurred: {seen:?}");
}
