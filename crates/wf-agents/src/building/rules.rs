//! The cases a build's rules give, from the engine's own kinds, for what the exploration of a flow does not model
//! (design document `testing`, 18): a round failing by each kind of failure, on the first agent's cell of the flow,
//! with what its handling makes of it; the work paused by the owner while a round is under way and while it waits;
//! the engine's own question of a change of a protected path, at each agent's cell of a line that may change the work;
//! never let on; every slot taken, a work waiting for one; the pause of everything; and the work cancelled from each
//! kind of state the others stand at. Each is made from a case the exploration gave, the nearest it kept of a round
//! under way at the cell, its state's end taken off and the rule's put on. Each rule names no cell, block or trigger:
//! it reads them of the flow and of the cases.

use decide::run::Failure;
use decide::{End, Pauser};
use flow::Kind;
use flow::agents::Agents;
use service::script::items::{Step, Wait};
use toml::{Table, Value};
use work::Question;

use super::cases::{Case, LONG, Store, failure_name, kind_of, step_value, text, took};

/// Every kind of failure of a round, in the order the engine tells them apart.
const FAILURES: [Failure; 11] = [
    Failure::Passing,
    Failure::Unread,
    Failure::Flagged,
    Failure::Guarded,
    Failure::UsageLimit,
    Failure::CannotRun,
    Failure::Offline,
    Failure::Unavailable,
    Failure::PromptTooLong,
    Failure::NoModel,
    Failure::Other,
];

/// When a usage limit a build's round reached resets, in milliseconds after the build's start: a year, which no real
/// account gives, so that the wait stands while the store is served.
pub const YEAR: u64 = 365 * 86_400_000;

/// A file of a change under the protected paths, as the engine's own question names it: made up, as the content of
/// every block is.
const PROTECTED: (char, &str, &str) = ('M', "protected/file.md", "+ A line the work adds to a protected file.");

/// The kind of state of a work waiting for the engine's own question of its change of a protected path.
pub const ENGINE_QUESTION: &str = "waits for the engine's own question";

/// The kind of state of a work never let on.
pub const NEVER_LET_ON: &str = "never let on";

/// The cases the rules give on this flow, from the cases its exploration gave; and each the rules could not make,
/// with why.
pub fn ruled(flow: &flow::Compiled, words: &Agents, cases: &[Case]) -> (Vec<Case>, Vec<(String, String)>) {
    let mut ruled = Vec::new();
    let mut not = Vec::new();
    let under_way_at = |cell: &decide::CellId| {
        (cases.iter()).find(|c| c.store == Store::Main && c.under_way.as_ref().is_some_and(|(at, ..)| at == cell))
    };
    let first = flow.cells.iter().find(|c| c.kind == Kind::Agent).map(|c| c.id.clone());
    let base = first.as_ref().and_then(under_way_at);
    match base {
        None => not.push((
            format!("the rules' states on the flow {}", flow.name),
            "no case of the exploration has a round of the flow's first agent's cell under way".to_string(),
        )),
        Some(base) => {
            for failure in FAILURES {
                ruled.push(failed(base, failure));
            }
            ruled.push(paused_under_way(base));
            ruled.push(slot_taken(base));
            ruled.push(slot_waited(base));
            ruled.push(everything(base));
        }
    }
    match (cases.iter()).find(|c| {
        c.store == Store::Main
            && matches!(
                c.end,
                Step::Waits {
                    on: Wait::Answer(_),
                    ..
                }
            )
    }) {
        Some(waiting) => ruled.push(paused_at_rest(waiting)),
        None => not.push((
            format!(
                "{{ paused = \"owner\" }} while the work waits, on the flow {}",
                flow.name
            ),
            "no case of the exploration waits for an answer".to_string(),
        )),
    }
    let changing = (flow.cells.iter()).filter(|c| c.kind == Kind::Agent && !c.reads_only);
    for cell in changing {
        match under_way_at(&cell.id) {
            Some(base) => match engine_question(base, words) {
                Some(case) => ruled.push(case),
                None => not.push((
                    format!("{ENGINE_QUESTION} at {}", cell.id),
                    "the words beside the flows make no question of a change".to_string(),
                )),
            },
            None => not.push((
                format!("{ENGINE_QUESTION} at {}", cell.id),
                "no case of the exploration has a round of the cell under way".to_string(),
            )),
        }
    }
    ruled.push(never_let_on(&flow.name));
    // Cancelled from each kind of state a case of the main store stands at, but an end.
    let mut kinds: Vec<String> = Vec::new();
    for case in cases.iter().chain(ruled.clone().iter()) {
        let ends = matches!(case.end, Step::Ends(_)) || case.held || case.kind == NEVER_LET_ON;
        // A round failed is cancelled from what its handling made of it.
        let kind = match case.kind.starts_with("fails ") {
            true => kind_of(&case.end, None),
            false => case.kind.clone(),
        };
        if case.store != Store::Main || ends || kinds.contains(&kind) {
            continue;
        }
        kinds.push(kind);
        ruled.push(cancelled(case));
    }
    (ruled, not)
}

/// The case with its end taken off: the round left under way, and the path's last step.
fn without_end(case: &Case) -> Case {
    let mut c = case.clone();
    if c.under_way.is_some() {
        c.rounds.pop();
        c.under_way = None;
        c.holds_slot = false;
    }
    c.path.pop();
    c
}

/// A round of the cell left under way: this round of its nth run, taking longer than a play's bound.
fn long(cell: &decide::CellId, run: usize, round: usize) -> Table {
    let mut t = Table::new();
    t.insert("cell".to_string(), text(cell.as_str()));
    t.insert("run".to_string(), Value::Integer(run as i64));
    t.insert("round".to_string(), Value::Integer(round as i64));
    t.insert("took".to_string(), took(LONG));
    t
}

/// The state, as the list names it: its end, after what brought it about.
fn named(end: &Step, after: &str) -> String {
    format!("{end} {after}")
}

/// The base's round under way failing of this kind, and what its handling makes of it: tried again after the engine's
/// wait, the round then under way; sent back, or tried again in a new session, the round then under way; for a cause
/// that holds every work, every agent's round held, in a store of its own; else the work paused by the engine.
fn failed(base: &Case, failure: Failure) -> Case {
    let (cell, run, round) = base.under_way.clone().expect("a round under way");
    let mut c = without_end(base);
    let name = failure_name(failure);
    let mut fails = Table::new();
    fails.insert("cell".to_string(), text(cell.as_str()));
    fails.insert("run".to_string(), Value::Integer(run as i64));
    fails.insert("round".to_string(), Value::Integer(round as i64));
    fails.insert(
        "fails".to_string(),
        text(&format!("The round failed: {}.", name.replace('_', " "))),
    );
    fails.insert("failure".to_string(), text(&name));
    if failure == Failure::UsageLimit {
        fails.insert("resets".to_string(), Value::Integer(YEAR as i64));
    }
    c.rounds.push(fails);
    let failing = Step::Fails {
        cell: cell.clone(),
        failure: Some(failure),
    };
    c.path.push(failing.clone());
    let again = |c: &mut Case| {
        c.rounds.push(long(&cell, run, round + 1));
        c.under_way = Some((cell.clone(), run, round + 1));
        c.holds_slot = true;
        Step::UnderWay {
            cell: cell.clone(),
            run: None,
            round: None,
        }
    };
    let end = match failure {
        Failure::Passing => {
            c.path.push(Step::Waits {
                cell: cell.clone(),
                on: Wait::Retry,
            });
            again(&mut c)
        }
        Failure::Unread | Failure::Flagged | Failure::Guarded => again(&mut c),
        f if f.shared() => {
            c.store = Store::Failed(f);
            Step::Waits {
                cell: cell.clone(),
                on: Wait::Shared,
            }
        }
        Failure::NoModel => {
            c.store = Store::Failed(Failure::NoModel);
            Step::Paused(Pauser::Engine)
        }
        _ => Step::Paused(Pauser::Engine),
    };
    c.path.push(end.clone());
    c.state = named(&end, &format!("after {failing}"));
    c.kind = format!("fails {name}");
    c.end = end;
    c
}

/// The base paused by the owner while its round is under way, which goes on.
fn paused_under_way(base: &Case) -> Case {
    let (cell, run, round) = base.under_way.clone().expect("a round under way");
    let mut c = base.clone();
    let at = Step::UnderWay {
        cell: cell.clone(),
        run: Some(run),
        round: Some(round),
    };
    c.acts.push(act(&at, "pause"));
    c.path.pop();
    c.path.push(Step::Paused(Pauser::Owner));
    c.state = named(&Step::Paused(Pauser::Owner), &format!("while {}", general(&at)));
    c.kind = kind_of(&Step::Paused(Pauser::Owner), None);
    c.end = Step::Paused(Pauser::Owner);
    c
}

/// A work paused by the owner while it waits.
fn paused_at_rest(waiting: &Case) -> Case {
    let mut c = waiting.clone();
    c.acts.push(act(&waiting.end, "pause"));
    c.path.push(Step::Paused(Pauser::Owner));
    c.state = named(&Step::Paused(Pauser::Owner), &format!("while {}", waiting.state));
    c.kind = kind_of(&Step::Paused(Pauser::Owner), None);
    c.end = Step::Paused(Pauser::Owner);
    c
}

/// The base under way, in the store whose every slot such rounds take.
fn slot_taken(base: &Case) -> Case {
    let mut c = base.clone();
    c.store = Store::Slots;
    c
}

/// The base's round waiting for a slot, every one taken.
fn slot_waited(base: &Case) -> Case {
    let (cell, ..) = base.under_way.clone().expect("a round under way");
    let mut c = without_end(base);
    let end = Step::Waits {
        cell: cell.clone(),
        on: Wait::Slot,
    };
    c.path.push(end.clone());
    c.store = Store::Slots;
    c.state = end.to_string();
    c.kind = kind_of(&end, None);
    c.end = end;
    c
}

/// The base pausing everything while its round is under way, in a store of its own.
fn everything(base: &Case) -> Case {
    let (cell, run, round) = base.under_way.clone().expect("a round under way");
    let mut c = base.clone();
    let at = Step::UnderWay {
        cell,
        run: Some(run),
        round: Some(round),
    };
    c.acts.push(act(&at, "pause_everything"));
    c.path.pop();
    let end = Step::Everything(Pauser::Owner);
    c.path.push(end.clone());
    c.store = Store::Everything;
    c.state = named(&end, &format!("while {}", general(&at)));
    c.kind = kind_of(&end, None);
    c.end = end;
    c
}

/// The base's round under way ending with the engine's own question of a change of a protected path, worded as the
/// engine words it, which the work waits for; none where the words beside the flows make none.
fn engine_question(base: &Case, words: &Agents) -> Option<Case> {
    let (cell, run, round) = base.under_way.clone()?;
    let (status, path, diff) = PROTECTED;
    let asked = carry::question_of_change(&words.asks.protected, &[(status, path.to_string(), diff.to_string())])?;
    let Question::Asked {
        name,
        waits,
        ask,
        options,
        about,
        reasons,
        ..
    } = asked
    else {
        return None;
    };
    let mut question = Table::new();
    question.insert("waits".to_string(), Value::Boolean(waits));
    question.insert("ask".to_string(), text(&ask));
    let options = (options.iter())
        .map(|o| {
            let mut way = Table::new();
            way.insert("does".to_string(), text(&o.does));
            way.insert("follows".to_string(), text(&o.follows));
            Value::Table(way)
        })
        .collect();
    question.insert("options".to_string(), Value::Array(options));
    if let Some(about) = about {
        question.insert("about".to_string(), text(&about));
    }
    let reasons: Table = reasons.into_iter().map(|(k, v)| (k, text(&v))).collect();
    question.insert("reasons".to_string(), Value::Table(reasons));
    let mut c = without_end(base);
    let mut r = Table::new();
    r.insert("cell".to_string(), text(cell.as_str()));
    r.insert("run".to_string(), Value::Integer(run as i64));
    r.insert("round".to_string(), Value::Integer(round as i64));
    r.insert("engine".to_string(), Value::Array(vec![Value::Table(question)]));
    c.rounds.push(r);
    let end = Step::Waits {
        cell,
        on: Wait::Answer(Some(name)),
    };
    c.path.push(end.clone());
    c.state = end.to_string();
    c.kind = ENGINE_QUESTION.to_string();
    c.end = end;
    Some(c)
}

/// An item never let on: its work stays paused as it was added.
pub fn never_let_on(flow: &str) -> Case {
    Case {
        store: Store::Main,
        flow: flow.to_string(),
        state: NEVER_LET_ON.to_string(),
        kind: NEVER_LET_ON.to_string(),
        rounds: Vec::new(),
        acts: Vec::new(),
        path: vec![Step::Paused(Pauser::Owner)],
        end: Step::Paused(Pauser::Owner),
        held: true,
        unmerged: false,
        mains: 0,
        holds_slot: false,
        under_way: None,
    }
}

/// The case cancelled from the state it stands at as its path ends.
fn cancelled(case: &Case) -> Case {
    let mut c = case.clone();
    c.acts.push({
        let mut cancel = act(&case.end, "cancel");
        cancel.insert("why".to_string(), text("Cancelled from the state it stood at."));
        cancel
    });
    // An end only the engine's view shows is no step of the records, and the cancel comes after the last that is.
    if c.path.last().is_some_and(Step::ends_only) {
        c.path.pop();
    }
    let end = Step::Ends(End::Cancelled);
    c.path.push(end.clone());
    c.under_way = None;
    c.holds_slot = false;
    c.state = named(&end, &format!("from {}", case.state));
    c.kind = kind_of(&end, None);
    c.end = end;
    c
}

/// An act of a script's item, done once its work stands at this state.
fn act(at: &Step, does: &str) -> Table {
    let mut act = Table::new();
    act.insert("at".to_string(), step_value(at));
    act.insert("do".to_string(), text(does));
    act
}

/// A round under way, without which run and round.
fn general(step: &Step) -> Step {
    match step {
        Step::UnderWay { cell, .. } => Step::UnderWay {
            cell: cell.clone(),
            run: None,
            round: None,
        },
        step => step.clone(),
    }
}
