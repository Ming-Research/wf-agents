//! The checks of a return, as properties over returns drawn at random from all the form allows, with faults put in now
//! and then: a return the form and its cell allow is never sent back, and its run finishes with what it said, a choice
//! with its line of why as its content, its calls made, its questions kept as versions of their own and the work held
//! at the cell for the owner's answer to one it waits for; and one with faults in it is sent back at once, naming each
//! of them once and nothing else (principle `report-all-at-once`). Each return is read as the agents' rounds read it
//! and handed to the engine's loop, which checks it against the records, so that what the reading finds and what the
//! run's check finds are seen together, as an agent is told them.

use decide::{Outsider, Pauser, RunId, Write};
use engine::{Effect, Event, handle};
use work::{Given, Spent, Time, Work};

use super::*;

/// How many returns are drawn of each kind.
const DRAWS: usize = 400;

/// A small random number generator, the same draws in every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
}

/// A return drawn, with what must come of it.
struct Drawn {
    r: Value,
    /// What the checks must find wrong with it, in any order: none where the form and its cell allow it.
    faults: Vec<Unmet>,
    /// Of a return with nothing wrong: what it dealt with, and each block it writes, in the order of the cell's
    /// outputs, with its content, its value and whether it is written with its meaning unchanged.
    dealt_with: Vec<BlockId>,
    writes: Vec<(BlockId, String, Option<String>, bool)>,
    /// How many calls it asks for.
    asks: usize,
    /// The names of the questions it asks, and whether one of them waits for the owner's answer.
    questions: Vec<&'static str>,
    waits: bool,
}

/// The triggers of `write` that run its round: what people asked for, and the owner's words where the owner spoke.
fn ran(spoken: bool) -> Vec<(&'static str, &'static str)> {
    let mut ran = vec![("on_request", "request")];
    if spoken {
        ran.push(("on_spoken", "write_spoken_to"));
    }
    ran
}

/// A return drawn from all the form allows; where `clean`, one the form and the cell allow.
fn draw(rng: &mut Rng, spoken: bool, clean: bool) -> Drawn {
    let mut faults: Vec<Unmet> = Vec::new();
    let mut r = Map::new();
    let (with_calls, with_outcome) = if clean {
        let calls = rng.chance(15);
        (calls, !calls)
    } else {
        match rng.below(20) {
            0 | 1 => (true, false),
            2 => (true, true),
            _ => (false, true),
        }
    };
    let mut asks = 0;
    if with_calls {
        let mut calls = Map::new();
        let n = if clean { 1 + rng.below(2) } else { rng.below(3) } as usize;
        if n > 0 || rng.chance(50) {
            calls.insert("query".into(), json!(vec![json!({"sql": "select 1"}); n]));
        }
        asks += n;
        if !clean && rng.chance(20) {
            calls.insert("browse".into(), json!([{"url": "x"}]));
            faults.push(Unmet::NotATool {
                name: "browse".to_string(),
            });
            asks += 1;
        }
        r.insert("calls".into(), Value::Object(calls));
    }
    let mut dealt_with: Vec<BlockId> = Vec::new();
    let mut writes = Vec::new();
    let mut handed: Vec<&str> = Vec::new();
    let mut waits = false;
    let mut misshapen = false;
    let mut asked: Vec<&'static str> = Vec::new();
    if with_outcome {
        let mut outcome = Map::new();
        let mut names: Vec<&str> = Vec::new();
        for (name, block) in ran(spoken) {
            if rng.chance(75) {
                names.push(name);
                dealt_with.push(block.into());
            }
        }
        let mut questions = Vec::new();
        if rng.chance(20) {
            waits = rng.chance(50);
            questions.push(
                json!({"name": "which", "waits": waits, "recommends": 1, "ask": "Which name?",
                "options": [{"does": "the first name", "follows": "a warmer greeting"}]}),
            );
        }
        if !questions.is_empty() {
            asked.push("which");
        }
        if !clean {
            let option = json!([{"does": "blue", "follows": "a calm page"}]);
            if rng.chance(5) {
                // A question is withdrawn only where one of the cell's of its name stands.
                questions.push(json!({"name": "old", "withdrawn": "The request settles it."}));
                faults.push(Unmet::NoSuchQuestion {
                    name: "old".to_string(),
                });
            }
            if rng.chance(5) {
                let twice = json!({"name": "twice", "waits": false, "recommends": 1, "ask": "Which colour?",
                    "options": option});
                questions.extend([twice.clone(), twice]);
                faults.push(Unmet::QuestionTwice {
                    name: "twice".to_string(),
                });
            }
            if rng.chance(5) {
                questions.push(
                    json!({"name": "none", "waits": false, "recommends": 0, "ask": "Which colour?",
                    "options": option}),
                );
                faults.push(Unmet::NotAnOption {
                    name: "none".to_string(),
                });
            }
        }
        if clean && dealt_with.is_empty() && !waits {
            names.push("on_request");
            dealt_with.push("request".into());
        }
        if !clean {
            if rng.chance(10) {
                names.push("on_nothing");
                faults.push(Unmet::NotATrigger {
                    name: "on_nothing".to_string(),
                });
            }
            if !spoken && rng.chance(10) {
                names.push("on_spoken");
                dealt_with.push("write_spoken_to".into());
                faults.push(Unmet::NotRunBy {
                    block: "write_spoken_to".into(),
                });
            }
            let first = ran(spoken).into_iter().find(|(name, _)| names.contains(name));
            if let Some((name, block)) = first
                && rng.chance(8)
            {
                names.push(name);
                dealt_with.push(block.into());
                faults.push(Unmet::DealtWithTwice { block: block.into() });
            }
        }
        outcome.insert("dealt_with".into(), json!(names));
        let mut outputs = Map::new();
        let mut unchanged: Vec<&str> = Vec::new();
        // The plan, a block that is no choice.
        match rng.below(100) {
            0..55 => {
                outputs.insert("plan".into(), json!("Greet by name."));
                handed.push("plan");
                let same = rng.chance(30);
                if same {
                    unchanged.push("plan");
                }
                writes.push(("plan".into(), "Greet by name.".to_string(), None, same));
            }
            55..65 if !clean => {
                outputs.insert("plan".into(), json!({"value": "yes", "why": "It is."}));
                handed.push("plan");
                faults.push(Unmet::NotAChoice { block: "plan".into() });
            }
            65..70 if !clean => {
                outputs.insert("plan".into(), json!(3));
                faults.push(Unmet::NotAChoice { block: "plan".into() });
            }
            _ => {}
        }
        // The choice, which the cell hands in every time.
        let n = rng.below(100);
        if n < 60 || (clean && !waits) {
            let value = if rng.chance(50) { "yes" } else { "no" };
            outputs.insert("ok".into(), json!({"value": value, "why": "It says how."}));
            handed.push("ok");
            writes.push(("ok".into(), "It says how.".to_string(), Some(value.to_string()), false));
            if !clean && rng.chance(15) {
                // A choice is unchanged in meaning only with the value it has, and it has none yet.
                unchanged.push("ok");
                faults.push(Unmet::ChoiceChanged { block: "ok".into() });
            }
        } else if !clean {
            match n {
                60..70 => {
                    outputs.insert("ok".into(), json!("yes"));
                    handed.push("ok");
                    faults.push(Unmet::NoValue { block: "ok".into() });
                }
                70..80 => {
                    outputs.insert("ok".into(), json!({"value": "maybe", "why": "Unsure."}));
                    handed.push("ok");
                    faults.push(Unmet::NotAValue {
                        block: "ok".into(),
                        value: "maybe".to_string(),
                    });
                }
                80..85 => {
                    // Written in a shape it does not take, it is said to be wrong once, for its shape.
                    outputs.insert("ok".into(), json!({"value": "yes"}));
                    misshapen = true;
                    faults.push(Unmet::NoValue { block: "ok".into() });
                }
                _ => {}
            }
        }
        if !clean && rng.chance(8) {
            // What the cell writes through its field `reply` is not written in `outputs`.
            outputs.insert("discussion".into(), json!("Hello."));
            faults.push(Unmet::NotAnOutput {
                block: "discussion".into(),
            });
        }
        if !clean && rng.chance(5) {
            outputs.insert("nowhere".into(), json!("x"));
            faults.push(Unmet::NotAnOutput {
                block: "nowhere".into(),
            });
        }
        if !clean && rng.chance(8) {
            unchanged.push("ghost");
            faults.push(Unmet::UnchangedNotWritten { block: "ghost".into() });
        }
        if !clean && !outputs.contains_key("plan") && rng.chance(8) {
            unchanged.push("plan");
            faults.push(Unmet::UnchangedNotWritten { block: "plan".into() });
        }
        outcome.insert("outputs".into(), Value::Object(outputs));
        if !unchanged.is_empty() {
            outcome.insert("meaning_unchanged".into(), json!(unchanged));
        }
        if !questions.is_empty() {
            outcome.insert("questions".into(), Value::Array(questions));
        }
        if rng.chance(20) {
            outcome.insert("observations".into(), json!(["The greeting has no test."]));
        }
        // The owner's words, dealt with, are answered in `reply`, which the record of what is said is written through.
        let answers = names.contains(&"on_spoken");
        if rng.chance(20) || (clean && answers) {
            outcome.insert("reply".into(), json!("By first name."));
            writes.push(("discussion".into(), "By first name.".to_string(), None, false));
        } else if answers {
            faults.push(Unmet::NotAnswered {
                name: "on_spoken".to_string(),
                field: "reply".to_string(),
            });
        }
        r.insert("outcome".into(), Value::Object(outcome));
    }
    // A return is of one kind, never both, whatever each holds.
    if with_calls && with_outcome {
        faults.push(Unmet::AsksAndWorks);
    }
    // What the run's check finds beside, from the records.
    if asks == 0 && dealt_with.is_empty() && !waits {
        faults.push(Unmet::DealtWithNothing);
    }
    if asks == 0 && !waits && !handed.contains(&"ok") && !misshapen {
        faults.push(Unmet::NotHandedIn { block: "ok".into() });
    }
    Drawn {
        r: Value::Object(r),
        faults,
        dealt_with,
        writes,
        asks,
        questions: asked,
        waits,
    }
}

/// A work of the small flow whose cell `write` is entered, run by what people asked for and, where `spoken`, by the
/// owner's words: they are written while the work is paused, so that both run its first run. Gives the work and the
/// run.
fn entered(flow: &Compiled, spoken: bool) -> (Work, RunId) {
    let mut work = Work::default();
    let written = |block: &str| Event::Wrote {
        block: block.into(),
        by: Outsider::Person,
        content: "Greet the user.".to_string(),
        matters: true,
        value: None,
    };
    let mut events = vec![
        Event::Paused {
            by: Pauser::Owner,
            why: None,
        },
        written("request"),
    ];
    if spoken {
        events.push(Event::Said {
            line: "engineer".to_string(),
            words: "Greet them by name.".to_string(),
        });
    }
    events.push(Event::Resumed);
    let mut effects = Vec::new();
    for event in events {
        effects.extend(handle(flow, &mut work, Time(0), event).expect("the loop takes it"));
    }
    let run = (effects.iter())
        .find_map(|e| match e {
            Effect::Round { run, number: 1, .. } => Some(*run),
            _ => None,
        })
        .expect("the cell is entered");
    (work, run)
}

/// What comes of a return drawn: it is read against the cell, and the loop takes it.
fn returned(flow: &Compiled, work: &mut Work, run: RunId, drawn: &Drawn) -> Vec<Effect> {
    let words = super::words();
    let came = came(&drawn.r, flow, cell(flow), &words).unwrap_or_else(|why| panic!("{} is not read: {why}", drawn.r));
    let event = Event::Returned {
        run,
        came,
        spent: Spent::default(),
    };
    handle(flow, work, Time(0), event).expect("the loop takes it")
}

/// Each thing, in words, in an order of their own.
fn sorted(unmet: &[Unmet]) -> Vec<String> {
    let mut words: Vec<String> = unmet.iter().map(|u| format!("{u:?}")).collect();
    words.sort();
    words
}

#[test]
fn a_return_the_form_and_its_cell_allow_is_never_sent_back_and_its_run_finishes_with_what_it_said() {
    let flow = flow();
    let mut rng = Rng(0x5EED_0001);
    for i in 0..DRAWS {
        let spoken = rng.chance(50);
        let drawn = draw(&mut rng, spoken, true);
        let (mut work, run) = entered(&flow, spoken);
        let effects = returned(&flow, &mut work, run, &drawn);
        let sent_back = effects
            .iter()
            .any(|e| matches!(e, Effect::Round { run: r, given: Given::SentBack(_), .. } if *r == run));
        assert!(!sent_back, "draw {i}: {} is sent back: {effects:?}", drawn.r);
        let finished = (work.records.finished.iter())
            .find(|f| f.run == run)
            .unwrap_or_else(|| panic!("draw {i}: the run of {} has not finished", drawn.r));
        assert_eq!(
            finished.dealt_with.as_ref(),
            Some(&drawn.dealt_with),
            "draw {i}: {}",
            drawn.r
        );
        let writes: Vec<Write> = (drawn.writes.iter())
            .map(|(block, _, value, unchanged)| Write {
                block: block.clone(),
                meaning_unchanged: *unchanged,
                value: value.clone(),
            })
            .collect();
        assert_eq!(finished.writes, writes, "draw {i}: {}", drawn.r);
        for (block, content, ..) in &drawn.writes {
            assert_eq!(
                work.contents.get(&(block.clone(), finished.at)),
                Some(content),
                "draw {i}: {}",
                drawn.r
            );
        }
        let calls = effects
            .iter()
            .filter(|e| matches!(e, Effect::Call { run: r, .. } if *r == run))
            .count();
        assert_eq!(calls, drawn.asks, "draw {i}: {}", drawn.r);
        // Its questions are versions of their own, of its cell, and no call: the work waits at the cell for the
        // owner's answer to one it waits for.
        let asked: Vec<&str> = (work.records.questions.iter())
            .filter(|q| q.run == run && q.cell.as_str() == "write" && !q.engine)
            .map(|q| q.name.as_str())
            .collect();
        assert_eq!(asked, drawn.questions, "draw {i}: {}", drawn.r);
        let decision = decide::decide(&flow.flow, &work.records);
        let held = matches!(&decision, decide::Decision::Wait { at, on }
            if at.as_str() == "write" && on.contains(&decide::Waited::Answer { question: "which".to_string() }));
        assert_eq!(held, drawn.waits, "draw {i}: {decision:?}");
    }
}

#[test]
fn a_return_with_faults_is_sent_back_at_once_naming_each_of_them_and_nothing_else() {
    let flow = flow();
    let mut rng = Rng(0x5EED_0002);
    let mut sent_back = 0;
    for i in 0..DRAWS {
        let spoken = rng.chance(50);
        let drawn = draw(&mut rng, spoken, false);
        let (mut work, run) = entered(&flow, spoken);
        let effects = returned(&flow, &mut work, run, &drawn);
        let unmet = effects.iter().find_map(|e| match e {
            Effect::Round {
                run: r,
                given: Given::SentBack(unmet),
                ..
            } if *r == run => Some(unmet.clone()),
            _ => None,
        });
        match unmet {
            Some(unmet) => {
                sent_back += 1;
                assert_eq!(sorted(&unmet), sorted(&drawn.faults), "draw {i}: {}", drawn.r);
                assert!(!work.records.finished.iter().any(|f| f.run == run), "draw {i}");
            }
            None => {
                assert!(
                    drawn.faults.is_empty(),
                    "draw {i}: {} is not sent back: {:?}",
                    drawn.r,
                    drawn.faults
                );
                assert!(work.records.finished.iter().any(|f| f.run == run), "draw {i}");
            }
        }
    }
    // Most of the returns drawn so have something wrong with them.
    assert!(sent_back > DRAWS / 2, "{sent_back} of {DRAWS} sent back");
}
