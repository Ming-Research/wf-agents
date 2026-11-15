//! The cases a build's rules give, from the engine's own kinds, for what the exploration of a flow does not model
//! (design document `testing`, 18): a round failing by each kind of failure, on the first agent's cell of the flow,
//! with what its handling makes of it, a cause that holds every work holding another work of its store too; the work
//! paused by the owner while a round is under way and while it waits; the engine's own question of a change of a
//! protected path, at each agent's cell of a line that may change the work; the check waiting for the project's turn,
//! which another work holds; never let on; every slot taken, a work waiting for one; the pause of everything while a
//! round is under way, with a work it holds and one cancelled while it holds; and the work cancelled from each kind of
//! state the others stand at. Each is made from a case the exploration gave, the nearest it kept of a round under way
//! at the cell, its state's end taken off and the rule's put on. Each rule names no cell, block or trigger: it reads
//! them of the flow and of the cases.

use decide::run::Failure;
use decide::{CellId, End, Pauser};
use flow::agents::Agents;
use flow::{Compiled, Kind};
use service::script::items::{ActScript, Does, Step, Wait};
use service::script::{CHECK, EngineAsked, EngineQuestion, RoundScript, WayScript};
use work::Question;

use super::cases::{Case, LONG, Store, failure_name, general, kind_of};

/// Why no case rests at the engine's wait before a round failed in passing is tried again: the clock answers the wait,
/// whatever holds the work, at most `decide::run::LONGEST_WAIT` after the round that failed, and a store is served on
/// the real clock, after the times of its build.
pub const RETRY_PASSES: &str = "the engine's wait before a round is tried again is answered by the clock, whatever \
                                holds the work, within five minutes of the round that failed: served after its \
                                build, the store has it answered as it opens, the round tried again then waiting for \
                                the hold";

/// Why no case holds another work of its store paused as a round finds no model its level runs on: the engine pauses
/// every other work that runs a round on that model, which only the levels name, and a script runs without them.
pub const NO_MODEL_HOLDS: &str = "a script runs without the levels, which name the model a work's rounds run on: the \
                                  engine pauses another work as a round finds no model only where its rounds run on \
                                  that model";

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

/// How long the first round of a work that another's failure, or the pause of everything, holds takes, in
/// milliseconds: long enough that the other's round has failed, or everything was paused, before its next round would
/// begin. As long the round of the work cancelled while everything is paused takes, which ends in the pause.
const LATER: u64 = 5_000;

/// How long the first round of the work that pauses everything takes, in milliseconds: longer than a round of the
/// build, so that the round of the work cancelled while everything is paused has begun before the pause, which holds
/// every round from beginning; and short of [`LATER`].
const PAUSES_AFTER: u64 = 2_000;

/// A file of a change under the protected paths, as the engine's own question names it: made up, as the content of
/// every block is.
const PROTECTED: (char, &str, &str) = ('M', "protected/file.md", "+ A line the work adds to a protected file.");

/// The kind of state of a work waiting for the engine's own question of its change of a protected path.
pub const ENGINE_QUESTION: &str = "waits for the engine's own question";

/// The kind of state of a work never let on.
pub const NEVER_LET_ON: &str = "never let on";

/// The cases the rules give on this flow, from the cases its exploration gave; and each the rules could not make,
/// with why.
pub fn ruled(flow: &Compiled, words: &Agents, cases: &[Case]) -> (Vec<Case>, Vec<(String, String)>) {
    let mut ruled = Vec::new();
    let mut not = Vec::new();
    let under_way_at = |cell: &CellId| {
        (cases.iter()).find(|c| c.store == Store::Main && c.under_way.as_ref().is_some_and(|(at, ..)| at == cell))
    };
    let first = flow.cells.iter().find(|c| c.kind == Kind::Agent).map(|c| c.id.clone());
    match first.as_ref().map(|cell| (cell, under_way_at(cell))) {
        None => not.push((
            format!("the rules' states on the flow {}", flow.name),
            "the flow has no agent's cell, whose round the rules' states start from".to_string(),
        )),
        Some((cell, None)) => not.push((
            format!("the rules' states on the flow {}", flow.name),
            format!("no case of the exploration has a round of the flow's first agent's cell, {cell}, under way"),
        )),
        Some((cell, Some(base))) => {
            let retry = Step::Waits {
                cell: cell.clone(),
                on: Wait::Retry,
            };
            not.push((format!("{retry} resting there"), RETRY_PASSES.to_string()));
            for failure in FAILURES {
                ruled.push(failed(base, failure));
                if failure.shared() {
                    ruled.push(held_by(base, Store::Failed(failure), Wait::Shared));
                }
            }
            not.push((
                format!(
                    "{} of a work another holds there, in the store {}",
                    Step::Paused(Pauser::Engine),
                    Store::Failed(Failure::NoModel).dir()
                ),
                NO_MODEL_HOLDS.to_string(),
            ));
            ruled.push(paused_under_way(base));
            ruled.push(slot_taken(base));
            ruled.push(slot_waited(base));
            ruled.push(cancelled_in_pause(base));
            ruled.push(everything(base));
            ruled.push(held_by(base, Store::Everything, Wait::Shared));
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
            Some(base) => match engine_question(flow, base, cases, words) {
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
    let checks = (flow.cells.iter()).filter(|c| c.act.as_deref() == Some(CHECK));
    for check in checks {
        match under_way_at(&check.id) {
            Some(base) => ruled.push(turn(base)),
            None => not.push((
                format!("{} waiting for the turn", check.id),
                "no case of the exploration has the check's round under way".to_string(),
            )),
        }
    }
    ruled.push(never_let_on(&flow.name));
    (ruled, not)
}

/// The cases cancelled from each kind of state the cases stand at, each in its case's store, but an end; and each
/// kind no cancel can be given at, with why.
pub fn cancels(cases: &[Case]) -> (Vec<Case>, Vec<(String, String)>) {
    let mut cancelled = Vec::new();
    let mut not = Vec::new();
    let mut kinds: Vec<String> = Vec::new();
    for case in cases {
        // A round failed is cancelled from what its handling made of it.
        let kind = match case.kind.starts_with("fails ") {
            true => kind_of(&case.end, None),
            false => case.kind.clone(),
        };
        // The pause of everything is cancelled from by its rule ([`ruled`]).
        if matches!(case.end, Step::Ends(_) | Step::Everything(_)) || kinds.contains(&kind) {
            continue;
        }
        kinds.push(kind.clone());
        match &case.end {
            // The records keep neither the wait every work shares nor the turn: by them the work's run waits to begin
            // a round, or its check's round is under way (design document `testing`, 18).
            Step::Waits {
                on: Wait::Shared | Wait::Turn,
                ..
            } => not.push((
                format!("{{ ends = \"cancelled\" }} from {}", case.state),
                "the records keep no such wait, so a cancel there shows nothing a cancel at the same cell does not"
                    .to_string(),
            )),
            _ => cancelled.push(cancel(case)),
        }
    }
    (cancelled, not)
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
fn long(cell: &CellId, run: usize, round: usize) -> RoundScript {
    let mut r = RoundScript::of(cell.as_str(), run, round);
    r.took.ms = Some(LONG);
    r
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
    let mut fails = RoundScript::of(cell.as_str(), run, round);
    fails.fails = Some(format!("The round failed: {}.", name.replace('_', " ")));
    fails.failure = Some(failure);
    if failure == Failure::UsageLimit {
        fails.resets = Some(YEAR);
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

/// A work of this store whose first round takes long enough that what holds every work there, another's round
/// failing for a cause that holds every work or the pause of everything, holds its next: its first agent's round does
/// not begin.
fn held_by(base: &Case, store: Store, wait: Wait) -> Case {
    let (cell, ..) = base.under_way.clone().expect("a round under way");
    let mut c = without_end(base);
    // Its run of the cell does not start either: no round of it could begin.
    if c.path.last() == Some(&Step::Runs(cell.clone())) {
        c.path.pop();
    }
    if let Some(first) = c.rounds.first_mut() {
        first.took.ms = Some(LATER);
    }
    let end = match store {
        Store::Everything => Step::Everything(Pauser::Owner),
        _ => Step::Waits { cell, on: wait },
    };
    c.path.push(end.clone());
    c.state = named(&end, "of a work another holds there");
    c.kind = kind_of(&end, None);
    c.store = store;
    c.end = end;
    c
}

/// The base paused by the owner while its round is under way, which goes on.
fn paused_under_way(base: &Case) -> Case {
    let (cell, run, round) = base.under_way.clone().expect("a round under way");
    let mut c = base.clone();
    let at = Step::UnderWay {
        cell,
        run: Some(run),
        round: Some(round),
    };
    c.acts.push(ActScript::at(&at, Does::Pause));
    c.path.pop();
    c.path.push(Step::Paused(Pauser::Owner));
    c.state = named(&Step::Paused(Pauser::Owner), &format!("while {}", general(at)));
    c.kind = kind_of(&Step::Paused(Pauser::Owner), None);
    c.end = Step::Paused(Pauser::Owner);
    c
}

/// A work paused by the owner while it waits.
fn paused_at_rest(waiting: &Case) -> Case {
    let mut c = waiting.clone();
    c.acts.push(ActScript::at(&waiting.end, Does::Pause));
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
    // Its run does not start: no slot is free for its round.
    if c.path.last() == Some(&Step::Runs(cell.clone())) {
        c.path.pop();
    }
    let end = Step::Waits { cell, on: Wait::Slot };
    c.path.push(end.clone());
    c.store = Store::Slots;
    c.state = end.to_string();
    c.kind = kind_of(&end, None);
    c.end = end;
    c
}

/// A work of the store of the pause of everything whose round, begun before the pause, fails in it for a cause that
/// pauses the work, which is then cancelled, while everything is paused: the pause holds every round from beginning,
/// so only a round that ends in it brings a work to a state a cancel can wait for there.
fn cancelled_in_pause(base: &Case) -> Case {
    let mut c = failed(base, Failure::Other);
    if let Some(fails) = c.rounds.last_mut() {
        fails.took.ms = Some(LATER);
    }
    c.store = Store::Everything;
    c.state = named(&c.end, &format!("while {}", Step::Everything(Pauser::Owner)));
    cancel(&c)
}

/// The base pausing everything while its round is under way, which goes on in the pause, in a store of its own; its
/// first round takes long enough that the round of the work cancelled while everything is paused has begun.
fn everything(base: &Case) -> Case {
    let (cell, run, round) = base.under_way.clone().expect("a round under way");
    let mut c = base.clone();
    let before = c.rounds.len().saturating_sub(1);
    if let Some(first) = c.rounds[..before].first_mut() {
        first.took.ms = Some(PAUSES_AFTER);
    }
    let at = Step::UnderWay {
        cell,
        run: Some(run),
        round: Some(round),
    };
    c.acts.push(ActScript::at(&at, Does::PauseEverything));
    c.path.pop();
    let end = Step::Everything(Pauser::Owner);
    c.path.push(end.clone());
    c.store = Store::Everything;
    c.state = named(&end, &format!("while {}", general(at)));
    c.kind = kind_of(&end, None);
    c.end = end;
    c
}

/// The base's check waiting for the project's turn to check and merge, which another work of its store holds, for
/// the first half of its round's time, longer than a play's bound.
fn turn(base: &Case) -> Case {
    let (cell, ..) = base.under_way.clone().expect("a round under way");
    let mut c = base.clone();
    c.turn = true;
    c.holds_slot = false;
    c.path.pop();
    let end = Step::Waits { cell, on: Wait::Turn };
    c.path.push(end.clone());
    c.state = end.to_string();
    c.kind = kind_of(&end, None);
    c.end = end;
    c
}

/// The base's round under way ending with the engine's own question of a change of a protected path, worded as the
/// engine words it, which the work waits for, the round handing in what that round of the cell hands in on another way
/// the exploration kept that leaves the work as it was, where one does, and else nothing; none where the words beside
/// the flows make no question.
fn engine_question(flow: &Compiled, base: &Case, cases: &[Case], words: &Agents) -> Option<Case> {
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
    let question = EngineQuestion::Asked(EngineAsked {
        waits,
        ask,
        options: (options.iter())
            .map(|o| WayScript {
                does: o.does.clone(),
                follows: o.follows.clone(),
            })
            .collect(),
        about,
        reasons,
    });
    let same = |r: &&RoundScript| {
        r.cell == cell.as_str() && r.run == run && r.round == round && r.hands_in.is_some() && r.questions.is_none()
    };
    // Of those, one whose every choice keeps the value its block starts with, which leaves the work as it was.
    let kept = |r: &&RoundScript| {
        (r.hands_in.iter().flatten()).all(|h| {
            let default = flow
                .blocks
                .get(&decide::BlockId::from(h.block.as_str()))
                .and_then(|b| b.default.as_deref());
            default.is_none_or(|default| h.value.as_deref() == Some(default))
        })
    };
    let rounds: Vec<&RoundScript> = (cases.iter()).flat_map(|c| &c.rounds).filter(same).collect();
    let done = rounds.iter().find(|r| kept(r)).or(rounds.first()).copied();
    let mut c = without_end(base);
    let mut r = RoundScript::of(cell.as_str(), run, round);
    r.hands_in = Some(done.and_then(|d| d.hands_in.clone()).unwrap_or_default());
    r.dealt_with = done.and_then(|d| d.dealt_with.clone());
    r.reads = done.and_then(|d| d.reads.clone());
    r.engine = Some(vec![question]);
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
        turn: false,
    }
}

/// The case cancelled from the state it stands at as its path ends: where it pauses everything, right after; where it
/// is never let on, from the pause it was added with, which is no step of its records.
fn cancel(case: &Case) -> Case {
    let mut c = case.clone();
    let at = match &case.end {
        Step::Everything(_) => (case.acts.iter().rev())
            .find(|a| a.does == Does::PauseEverything)
            .and_then(|a| a.at.step().ok())
            .unwrap_or_else(|| case.end.clone()),
        end => end.clone(),
    };
    let mut cancel = ActScript::at(&at, Does::Cancel);
    cancel.why = Some("Cancelled from the state it stood at.".to_string());
    c.acts.push(cancel);
    // An end only the engine's view shows is no step of the records, nor the pause a held item was added with; the
    // cancel comes after the last that is.
    if c.held || c.path.last().is_some_and(Step::ends_only) {
        c.path.pop();
    }
    let end = Step::Ends(End::Cancelled);
    c.path.push(end.clone());
    c.under_way = None;
    c.holds_slot = false;
    c.turn = false;
    c.state = named(&end, &format!("from {}", case.state));
    c.kind = kind_of(&end, None);
    c.end = end;
    c
}
