//! The cases of a build of the page's states (design document `testing`, 18): each a state of the page a work can be
//! brought to, with the rounds, the acts and the path one item's script takes to bring its work there.
//!
//! The states of a work's routing come from the exploration of its flow (crate `explore`): every state it keeps is
//! named by what the page shows of the work there, as the judge reads the engine's view (`service::judge::states`):
//! under way at a cell, waiting at a cell for each thing it waits for, stalled, ended; a work merged is told apart as
//! well by its ways back and the cells it went without, so that each way through the flow is a state of its own. Of
//! each name the nearest path is taken, and played again on a work added as a play adds an item: paused as it is
//! added, its block of dependencies written, what people write for it written, then let on. What the path gave the
//! engine's loop becomes the item's script: each round's return as the exploration's model made it, never left to the
//! script's default round; each write by a person, each of the owner's words and answers and each cell let go, an act
//! at the state the engine showed the work in as it came; main moving, the act that moves main; a session begun anew,
//! the act that begins one; and what the engine gives the loop itself, a call withdrawn, a round stopped, what an
//! ended work held let go, nothing. The steps the records of that play hold are the item's path, the state it was kept
//! for its end; a round or a call left under way there takes longer than a play's bound of clock time, so that the
//! play ends while it is under way. A path that writes what a script writes only as the round that reads it does (the
//! tree, the head), or that a play cannot add an item for, is no case: it is listed with why.
//!
//! Nothing here names a cell, a block or a trigger of a flow: each is read of the flow, or of the source the engine
//! writes it by.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use decide::run::Failure;
use decide::{BlockId, CallId, CellId, Decision, End, Outsider, Pauser, Records, Room, RunId};
use engine::spoken::To;
use engine::{Came, Effect, Event as Given, Time, Work};
use explore::model::Model;
use flow::{Compiled, Kind};
use service::Shown;
use service::judge::{self, Added};
use service::script::items::{Step, Wait};
use toml::{Table, Value};
use work::{Ask, Call, Question};

/// How long a round or a call meant to stay under way takes, in milliseconds: longer than a play's bound of clock time
/// (crate module `playing`), so that the play ends while it is under way.
pub const LONG: u64 = 2 * 86_400_000;

/// The time the work is added at in the replay of a path, in milliseconds; what it says of the path does not rest on
/// it.
const ADDED: u64 = 1_000_000;

/// The stores of a build, each its own data directory, which holds what the others cannot: the plan's table (design
/// document `testing`, 18).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Store {
    /// Every state of a work but those below; main never moves in it.
    Main,
    /// Every slot taken, and a work waiting for one.
    Slots,
    /// A round failed for a cause that holds every work, of this kind, the cause left standing.
    Failed(Failure),
    /// The pause of everything.
    Everything,
    /// The works whose paths move main once, each to the state its path moves it at, main then moved once.
    MainMoved,
    /// A work whose path moves main more than once, alone: the nth such.
    Moves(usize),
    /// A stall, with the other states of the flow that reaches one.
    Stalled,
}

impl Store {
    /// The name of its directory, beside the others'.
    pub fn dir(&self) -> String {
        match self {
            Store::Main => "main".to_string(),
            Store::Slots => "slots".to_string(),
            Store::Failed(failure) => format!("failed-{}", failure_name(*failure).replace('_', "-")),
            Store::Everything => "everything-paused".to_string(),
            Store::MainMoved => "main-moved".to_string(),
            Store::Moves(n) => format!("main-moved-{n}"),
            Store::Stalled => "stalled".to_string(),
        }
    }
}

/// The kind of failure as a script and a round's entry name it.
pub fn failure_name(failure: Failure) -> String {
    (Value::try_from(failure).ok())
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// One state of the page, and how one item's work is brought to it.
#[derive(Clone, Debug)]
pub struct Case {
    /// The store it is made in.
    pub store: Store,
    /// The flow its work runs on.
    pub flow: String,
    /// The state, as a path's steps say it: what the page shows of the work at its end.
    pub state: String,
    /// The kind of state it is (`kind_of`).
    pub kind: String,
    /// The rounds its script says, each a table of the script's rounds but for its work, which its store fills in.
    pub rounds: Vec<Table>,
    /// Its acts, each a table of a script item's acts.
    pub acts: Vec<Table>,
    pub path: Vec<Step>,
    /// The state its work stands at as its path ends, as the engine shows it, as an act would wait for it.
    pub end: Step,
    /// Whether it is never let on.
    pub held: bool,
    /// Whether it depends on an item left unmerged, which its store adds, never let on.
    pub unmerged: bool,
    /// How often main moves on its path.
    pub mains: usize,
    /// Whether an agent's round of it is left under way, holding a slot.
    pub holds_slot: bool,
    /// The round left under way at its end, by its cell, run and round, where one is: what a rule changes it from.
    pub under_way: Option<(CellId, usize, usize)>,
}

/// The name of the state the engine shows a work at, as the page shows it: each state the judge reads of the engine's
/// view, without the run and the round of a round under way and the name of a question waited for, joined; a work
/// merged told apart by its ways back and the cells it went without. None where the engine shows it at none.
pub fn state_of(flow: &Compiled, work: &Work, decision: &Decision) -> Option<String> {
    let shown = shown_as(decision.clone());
    let states: Vec<String> = (judge::states(work, &shown).into_iter())
        .map(|step| general(step).to_string())
        .collect();
    if states.is_empty() {
        return None;
    }
    let mut name = states.join(", ");
    if matches!(decision, Decision::Ended(End::Merged)) {
        name.push_str(&ways(flow, &work.records));
    }
    Some(name)
}

/// What the engine shows of a work with this decision, nothing outside its records holding it back.
fn shown_as(decision: Decision) -> Shown {
    Shown {
        decision,
        turn: false,
        shared: false,
        everything: None,
    }
}

/// The state with what the page shows of every work alike left out: the run and round of a round under way, and which
/// question is waited for.
fn general(step: Step) -> Step {
    match step {
        Step::UnderWay { cell, .. } => Step::UnderWay {
            cell,
            run: None,
            round: None,
        },
        Step::Waits {
            cell,
            on: Wait::Answer(_),
        } => Step::Waits {
            cell,
            on: Wait::Answer(None),
        },
        step => step,
    }
}

/// The ways a work went back and the cells it went without, as words after its end: each run of a cell that came
/// after a run of a cell later in the flow is a way back, from that cell to it.
fn ways(flow: &Compiled, records: &Records) -> String {
    let mut started: Vec<&decide::Started> = records.started.iter().collect();
    started.sort_by_key(|s| s.at);
    let place = |cell: &CellId| flow.flow.place(cell).unwrap_or(usize::MAX);
    let mut back: BTreeSet<(usize, usize)> = BTreeSet::new();
    for pair in started.windows(2) {
        let (from, to) = (place(&pair[0].cell), place(&pair[1].cell));
        if to < from {
            back.insert((from, to));
        }
    }
    let ran: BTreeSet<&CellId> = started.iter().map(|s| &s.cell).collect();
    let cells = flow.flow.cells();
    let mut said = String::new();
    if !back.is_empty() {
        let back: Vec<String> = (back.into_iter())
            .map(|(from, to)| format!("{} to {}", cells[from].id, cells[to].id))
            .collect();
        said.push_str(&format!(", back from {}", back.join(", from ")));
    }
    let without: Vec<&str> = (cells.iter())
        .filter(|c| !ran.contains(&c.id))
        .map(|c| c.id.as_str())
        .collect();
    if !without.is_empty() {
        said.push_str(&format!(", without {}", without.join(", ")));
    }
    said
}

/// The kind of state a work stands at as its path ends there, which the build counts its states by.
pub fn kind_of(end: &Step, dependencies: Option<&BlockId>) -> String {
    match end {
        Step::Runs(_) | Step::UnderWay { .. } => "under way".to_string(),
        Step::SentBack(_) => "sent back".to_string(),
        Step::Fails { .. } => "failed".to_string(),
        Step::Waits {
            on: Wait::Precondition(Some(key)),
            ..
        } if dependencies.is_some_and(|block| block.as_str() == key) => "waits for an item it depends on".to_string(),
        Step::Waits { on, .. } => format!("waits for {}", wait_kind(on)),
        Step::Paused(Pauser::Owner) => "paused by the owner".to_string(),
        Step::Paused(Pauser::Engine) => "paused by the engine".to_string(),
        Step::LetOn => "let on".to_string(),
        Step::LetGo(_) => "let go".to_string(),
        Step::Reopened => "reopened".to_string(),
        Step::Stalled(_) => "stalled".to_string(),
        Step::Ends(End::Merged) => "merged".to_string(),
        Step::Ends(End::Cancelled) => "cancelled".to_string(),
        Step::Everything(_) => "everything paused".to_string(),
    }
}

/// What a wait is for, as a path's step says it.
fn wait_kind(on: &Wait) -> &'static str {
    match on {
        Wait::Tool => "a tool",
        Wait::Retry => "the engine's retry",
        Wait::Answer(_) => "an answer",
        Wait::Outside(_) => "the outside",
        Wait::Limit(_) => "the owner at a limit",
        Wait::Precondition(_) => "a precondition",
        Wait::Result => "a result",
        Wait::Slot => "a slot",
        Wait::Turn => "the turn",
        Wait::Shared => "what every work shares",
    }
}

/// How many things the exploration gives the loop as it starts a work, before anything happens to it: the start, what
/// the owner writes of the work, the items it depends on not yet merged and main, each where the flow has a block of
/// it (`explore`'s first state).
fn starting(model: &Model) -> usize {
    let has = |source: &str| usize::from(model.sources.contains_key(source));
    has(driver::START) + model.people.len() + has(driver::DEPENDENCIES) + has(driver::MAIN)
}

/// The source outside the work that writes the block, where one does.
fn source<'f>(flow: &'f Compiled, block: &BlockId) -> Option<&'f str> {
    flow.blocks.get(block).and_then(|b| b.writers.outside.as_deref())
}

/// The case of the state of this name, which the nearest path the exploration kept to it reaches; or why it is none.
pub fn case_of(model: &Model, state: &str, path: &explore::Path) -> Result<Case, String> {
    let flow = &model.compiled;
    let first = starting(model);
    let given = &path.given;
    let begins = given.len() >= first && given[..first].iter().all(|g| matches!(g, Given::Wrote { .. }));
    if !begins {
        return Err("its path does not begin as the exploration begins a work".to_string());
    }
    let dependencies = model.sources.get(driver::DEPENDENCIES);
    let merges = |g: &Given| match g {
        Given::Wrote {
            block, value: Some(v), ..
        } => Some(block) == dependencies && v == driver::MERGED,
        _ => false,
    };
    let unmerged = dependencies.is_some() && !given[first..].iter().any(merges);
    let mut replay = Replay::added(model, unmerged)?;
    let mut rounds: Vec<Table> = Vec::new();
    let mut acts: Vec<Table> = Vec::new();
    let mut mains = 0;
    // What the engine gives the loop itself, after what asked for it, as the exploration does: in a play, the engine.
    let mut own: VecDeque<Given> = VecDeque::new();
    // Of each call of a tool a round asked for, the round's place among the rounds and the ask's among its asks.
    let mut asked: BTreeMap<CallId, (usize, usize)> = BTreeMap::new();
    // Of each call on the owner at a run's limit, the run's cell.
    let mut limits: BTreeMap<CallId, CellId> = BTreeMap::new();
    for g in &given[first..] {
        let g = &replay.translated(g, &path.work.records)?;
        if own.front() == Some(g) {
            own.pop_front();
            let effects = replay.give(g.clone())?;
            follow(&effects, &mut own);
            continue;
        }
        let at = replay.at()?;
        let act = |does: &str| {
            let mut act = Table::new();
            act.insert("at".to_string(), step_value(&at));
            act.insert("do".to_string(), text(does));
            act
        };
        let mut asking: Option<usize> = None;
        match g {
            Given::Look => {}
            Given::Wrote {
                block,
                by: Outsider::Outside,
                content,
                ..
            } => match source(flow, block) {
                Some(s) if s == driver::DEPENDENCIES => continue,
                Some(s) if s == driver::MAIN => {
                    let mut main = act("main");
                    main.insert("content".to_string(), text(content));
                    acts.push(main);
                    mains += 1;
                }
                Some(_) if model.sessions.values().any(|b| b == block) => {
                    let line = (model.sessions.iter())
                        .find(|(_, b)| *b == block)
                        .map(|(line, _)| line.clone());
                    let mut anew = act("new_session");
                    anew.insert("line".to_string(), text(&line.unwrap_or_default()));
                    acts.push(anew);
                }
                Some(s) => {
                    return Err(format!(
                        "its path has {block} written by the source {s}, which a script writes only as the round \
                         that reads it does: not built"
                    ));
                }
                None => {
                    return Err(format!(
                        "its path has {block} written from outside, which no source writes"
                    ));
                }
            },
            Given::Wrote {
                block,
                by: Outsider::Person,
                content,
                value,
                ..
            } => {
                let mut write = act("write");
                write.insert("block".to_string(), text(block.as_str()));
                write.insert("content".to_string(), text(content));
                if let Some(value) = value {
                    write.insert("value".to_string(), text(value));
                }
                acts.push(write);
            }
            Given::Said { to, words } => {
                let mut say = act("say");
                match to {
                    To::Line(line) => say.insert("line".to_string(), text(line)),
                    To::Question(question) => say.insert("question".to_string(), text(question)),
                };
                say.insert("words".to_string(), text(words));
                acts.push(say);
            }
            Given::Answer {
                question, option, why, ..
            } => {
                let mut answer = act("answer");
                answer.insert("question".to_string(), text(question));
                answer.insert("option".to_string(), Value::Integer(*option as i64));
                if let Some(why) = why {
                    answer.insert("why".to_string(), text(why));
                }
                acts.push(answer);
            }
            Given::LetGo { cell } => {
                let mut go = act("let_go");
                go.insert("cell".to_string(), text(cell.as_str()));
                acts.push(go);
            }
            Given::Done { call, result } => {
                if let Some((k, j)) = asked.get(call).copied() {
                    let asks = rounds[k].get_mut("asks").and_then(Value::as_array_mut);
                    if let Some(ask) = asks.and_then(|asks| asks.get_mut(j)).and_then(Value::as_table_mut) {
                        ask.insert("answer".to_string(), text(result));
                    }
                } else if let Some(cell) = limits.get(call) {
                    let mut go = act("let_go");
                    go.insert("cell".to_string(), text(cell.as_str()));
                    acts.push(go);
                } else {
                    return Err("its path gives a call a result that no round of it asked for".to_string());
                }
            }
            Given::Returned { run, came, .. } => {
                let (cell, nth, number) = replay.round_of(*run)?;
                rounds.push(round_of(flow, &cell, nth, number, came)?);
                if !came.asks.is_empty() {
                    asking = Some(rounds.len() - 1);
                }
            }
            other => return Err(format!("its path gives the loop what a script cannot: {other:?}")),
        }
        let effects = replay.give(g.clone())?;
        let mut j = 0;
        for effect in &effects {
            if let Effect::Call { call, run, what } = effect {
                match what {
                    Call::Asked(Ask::Tool { .. }) => {
                        if let Some(k) = asking {
                            asked.insert(*call, (k, j));
                            j += 1;
                        }
                    }
                    Call::AtLimit(_) => {
                        if let Some(cell) = replay.cell_of(*run) {
                            limits.insert(*call, cell);
                        }
                    }
                    _ => {}
                }
            }
        }
        follow(&effects, &mut own);
    }
    let decision = decide::decide(&flow.flow, &replay.work.records);
    let reached = state_of(flow, &replay.work, &decision);
    if reached.as_deref() != Some(state) {
        return Err(format!(
            "as a play adds its item, its path ends at {}",
            reached.unwrap_or_else(|| "nothing the engine shows".to_string())
        ));
    }
    let end = replay.at()?;
    let added = Added {
        writes: model.people.len(),
        held: false,
    };
    let mut steps: Vec<Step> = (judge::seen(flow, &replay.work, added).into_iter())
        .map(|seen| seen.step)
        .collect();
    let mut holds_slot = false;
    let mut under_way = None;
    match &decision {
        Decision::Busy { cell, run } => {
            let (cell, nth, number) = replay.round_now(cell, *run);
            let mut long = Table::new();
            long.insert("cell".to_string(), text(cell.as_str()));
            long.insert("run".to_string(), Value::Integer(nth as i64));
            long.insert("round".to_string(), Value::Integer(number as i64));
            long.insert("took".to_string(), took(LONG));
            rounds.push(long);
            holds_slot = (flow.cells.iter()).any(|c| c.id == cell && c.kind == Kind::Agent);
            steps.push(Step::UnderWay {
                cell: cell.clone(),
                run: None,
                round: None,
            });
            under_way = Some((cell, nth, number));
        }
        Decision::Stalled { at, .. } => steps.push(Step::Stalled(at.clone())),
        _ => {}
    }
    // A call of a tool left waiting at the end takes longer than the play's bound.
    for (call, (k, j)) in &asked {
        if !replay.work.records.done.iter().any(|d| d.call == *call) {
            let asks = rounds[*k].get_mut("asks").and_then(Value::as_array_mut);
            if let Some(ask) = asks.and_then(|asks| asks.get_mut(*j)).and_then(Value::as_table_mut) {
                ask.insert("ms".to_string(), Value::Integer(LONG as i64));
            }
            holds_slot = true;
        }
    }
    Ok(Case {
        store: Store::Main,
        flow: flow.name.clone(),
        state: state.to_string(),
        kind: kind_of(&end, dependencies),
        rounds,
        acts,
        path: steps,
        end,
        held: false,
        unmerged,
        mains,
        holds_slot,
        under_way,
    })
}

/// What the engine gives the loop itself for these effects, as the exploration gives it at once after what asked for
/// it, before anything else: a call withdrawn its result, a round stopped its end with nothing, what an ended work
/// held let go. Put first, in their order, so that what they ask for in turn comes before what came after them.
fn follow(effects: &[Effect], own: &mut VecDeque<Given>) {
    let follows: Vec<Given> = (effects.iter())
        .filter_map(|effect| match effect {
            Effect::Withdraw { call } => Some(Given::Done {
                call: *call,
                result: explore::WITHDRAWN.to_string(),
            }),
            Effect::Stop { run } => Some(Given::Returned {
                run: *run,
                came: Came::asks(Vec::new()),
                spent: Default::default(),
            }),
            Effect::Release { .. } => Some(Given::Released),
            _ => None,
        })
        .collect();
    for given in follows.into_iter().rev() {
        own.push_front(given);
    }
}

/// The round entry of a script for this round of the cell's nth run, as its return came: what it hands in, or the
/// tools it asks for; what it dealt with; what what it wrote waits for; its questions; what it read of the branch.
fn round_of(flow: &Compiled, cell: &CellId, nth: usize, number: usize, came: &Came) -> Result<Table, String> {
    let mut round = Table::new();
    round.insert("cell".to_string(), text(cell.as_str()));
    round.insert("run".to_string(), Value::Integer(nth as i64));
    round.insert("round".to_string(), Value::Integer(number as i64));
    let spec = flow.cells.iter().find(|c| c.id == *cell);
    if !came.engine.is_empty() {
        return Err("its path has a round end with the engine's own question, which a script says by a rule".into());
    }
    if !came.asks.is_empty() {
        if !came.writes.is_empty() {
            return Err("its path has a round that both hands in and asks".to_string());
        }
        let mut asks = Vec::new();
        for ask in &came.asks {
            let Ask::Tool { name, input } = ask else {
                return Err("its path has a round ask a person, which no carrier but a script's does".to_string());
            };
            let mut t = Table::new();
            t.insert("tool".to_string(), text(name));
            t.insert("input".to_string(), text(input));
            asks.push(Value::Table(t));
        }
        round.insert("asks".to_string(), Value::Array(asks));
        return Ok(round);
    }
    // The act that runs the tests hands in the result of each entry it gives as a script's act does by itself.
    let results: BTreeSet<&BlockId> = (flow.flow.cells().iter())
        .flat_map(|c| c.asks.iter().map(|a| &a.result))
        .collect();
    let runs_tests = spec.is_some_and(|c| c.act.as_deref() == Some(flow::RUNS_TESTS));
    let handed: Vec<Value> = (came.writes.iter())
        .filter(|h| !(runs_tests && results.contains(&h.block)))
        .map(|h| {
            let mut t = Table::new();
            t.insert("block".to_string(), text(h.block.as_str()));
            t.insert("content".to_string(), text(&h.content));
            if let Some(value) = &h.value {
                t.insert("value".to_string(), text(value));
            }
            t.insert("matters".to_string(), Value::Boolean(h.matters));
            Value::Table(t)
        })
        .collect();
    round.insert("hands_in".to_string(), Value::Array(handed));
    if let Some(dealt_with) = &came.dealt_with {
        let blocks = dealt_with.iter().map(|b| text(b.as_str())).collect();
        round.insert("dealt_with".to_string(), Value::Array(blocks));
    }
    if let Some(block) = &came.waits_for {
        round.insert("waits_for".to_string(), text(block.as_str()));
    }
    if !came.said.questions.is_empty() {
        let mut questions = Vec::new();
        for question in &came.said.questions {
            questions.push(Value::Table(question_of(question)?));
        }
        round.insert("questions".to_string(), Value::Array(questions));
    }
    if !came.said.observations.is_empty() {
        let observations = came.said.observations.iter().map(|o| text(o)).collect();
        round.insert("observations".to_string(), Value::Array(observations));
    }
    if !came.read.is_empty() {
        let reads = (came.read.iter())
            .map(|r| {
                let mut t = Table::new();
                t.insert("block".to_string(), text(r.block.as_str()));
                t.insert("content".to_string(), text(&r.content));
                Value::Table(t)
            })
            .collect();
        round.insert("reads".to_string(), Value::Array(reads));
    }
    Ok(round)
}

/// A question a return asks, as a script's round writes it.
fn question_of(question: &Question) -> Result<Table, String> {
    let mut t = Table::new();
    match question {
        Question::Asked {
            name,
            waits,
            recommends,
            ask,
            options,
            about,
            reasons,
        } => {
            if about.is_some() || !reasons.is_empty() {
                return Err("its path has a question of a change, which only the engine asks".to_string());
            }
            t.insert("name".to_string(), text(name));
            t.insert("waits".to_string(), Value::Boolean(*waits));
            if let Some(recommends) = recommends {
                t.insert("recommends".to_string(), Value::Integer(*recommends as i64));
            }
            t.insert("ask".to_string(), text(ask));
            let options = (options.iter())
                .map(|o| {
                    let mut way = Table::new();
                    way.insert("does".to_string(), text(&o.does));
                    way.insert("follows".to_string(), text(&o.follows));
                    Value::Table(way)
                })
                .collect();
            t.insert("options".to_string(), Value::Array(options));
        }
        Question::Withdrawn { name, why } => {
            t.insert("name".to_string(), text(name));
            t.insert("withdrawn".to_string(), text(why));
        }
    }
    Ok(t)
}

/// A text, as a script's value.
pub fn text(s: &str) -> Value {
    Value::String(s.to_string())
}

/// What a round takes, as a script's round says it: this many milliseconds.
pub fn took(ms: u64) -> Value {
    let mut took = Table::new();
    took.insert("ms".to_string(), Value::Integer(ms as i64));
    Value::Table(took)
}

/// A step, or a state an act waits for, as a script's table: the step as a path writes it, read back.
pub fn step_value(step: &Step) -> Value {
    let read: Table = toml::from_str(&format!("step = {step}")).expect("a step is written as a table of a script");
    read.get("step").cloned().expect("the step read back")
}

/// The play of a path again, on a work added as a play adds an item.
struct Replay<'m> {
    flow: &'m Compiled,
    work: Work,
    clock: u64,
}

impl<'m> Replay<'m> {
    /// A work added as the driver adds an item's, and let on as a play lets it on: paused, its block of dependencies
    /// written, merged unless it is to wait for them, and what people write of it written, each in a room that begins
    /// nothing; looked at; then let on, and its block of the start written as the driver writes it then.
    fn added(model: &'m Model, unmerged: bool) -> Result<Replay<'m>, String> {
        let mut replay = Replay {
            flow: &model.compiled,
            work: Work::default(),
            clock: ADDED,
        };
        let mut first = vec![Given::Paused {
            by: Pauser::Owner,
            why: None,
        }];
        if let Some(block) = model.sources.get(driver::DEPENDENCIES) {
            let value = if unmerged { driver::WAITING } else { driver::MERGED };
            first.push(Given::Wrote {
                block: block.clone(),
                by: Outsider::Outside,
                content: format!("The items it depends on: {value}."),
                matters: true,
                value: Some(value.to_string()),
            });
        }
        for block in &model.people {
            first.push(Given::Wrote {
                block: block.clone(),
                by: Outsider::Person,
                content: format!("The {block} as a person wrote it."),
                matters: true,
                value: None,
            });
        }
        first.push(Given::Look);
        let taking = Room {
            paused: true,
            slot: false,
        };
        for given in first {
            replay.clock += 1000;
            engine::handle_in(replay.flow, &mut replay.work, Time(replay.clock), given, taking)
                .map_err(|why| format!("the work could not be added as a play adds it: {why:?}"))?;
        }
        replay.give(Given::Resumed)?;
        // Let on with nothing of it started, the work is started: the driver writes its block of the start.
        if let Some(block) = model.sources.get(driver::START) {
            let declared = model.compiled.blocks.get(block);
            let content = declared.and_then(|b| b.writes.get("text")).unwrap_or_default();
            replay.give(Given::Wrote {
                block: block.clone(),
                by: Outsider::Outside,
                content: content.to_string(),
                matters: true,
                value: None,
            })?;
        }
        Ok(replay)
    }

    /// Gives the loop this, as the exploration did, with a second more on the clock.
    fn give(&mut self, given: Given) -> Result<Vec<Effect>, String> {
        self.clock += 1000;
        let shown = format!("{given:?}");
        engine::handle(self.flow, &mut self.work, Time(self.clock), given).map_err(|why| {
            format!(
                "played on a work added as a play adds an item, its path is refused at {}: {why:?}",
                shown.chars().take(120).collect::<String>()
            )
        })
    }

    /// The state the work stands at now, as the engine shows it, which an act given now waits for.
    fn at(&self) -> Result<Step, String> {
        let shown = shown_as(decide::decide(&self.flow.flow, &self.work.records));
        (judge::states(&self.work, &shown).into_iter().next())
            .ok_or_else(|| "its path gives an act where the engine shows the work at nothing".to_string())
    }

    /// What the exploration gave its loop, with the names it gave its runs and calls and the version of each question
    /// it answered, as this work names them: the same, run for run and call for call in the order each began, where
    /// the play goes the way the exploration went.
    fn translated(&self, given: &Given, explored: &Records) -> Result<Given, String> {
        let mut runs: Vec<(decide::Seq, RunId)> = explored.started.iter().map(|s| (s.at, s.run)).collect();
        runs.sort();
        let mut ours: Vec<(decide::Seq, RunId)> = self.work.records.started.iter().map(|s| (s.at, s.run)).collect();
        ours.sort();
        let run = |run: RunId| {
            let k = runs.iter().position(|(_, r)| *r == run);
            k.and_then(|k| ours.get(k)).map(|(_, r)| *r).ok_or_else(|| {
                format!(
                    "its path gives the run {} of the exploration a round, which played as a play adds its item has \
                     not started: it started {:?}, standing at {:?}",
                    run.0,
                    (ours.iter()).map(|(_, r)| r.0).collect::<Vec<_>>(),
                    decide::decide(&self.flow.flow, &self.work.records)
                )
            })
        };
        let mut calls: Vec<(decide::Seq, CallId)> = explored.called.iter().map(|c| (c.at, c.call)).collect();
        calls.sort();
        let mut our_calls: Vec<(decide::Seq, CallId)> =
            self.work.records.called.iter().map(|c| (c.at, c.call)).collect();
        our_calls.sort();
        let call = |call: CallId| {
            let k = calls.iter().position(|(_, c)| *c == call);
            (k.and_then(|k| our_calls.get(k)).map(|(_, c)| *c))
                .ok_or_else(|| "its path gives a result to a call that played so was not made".to_string())
        };
        Ok(match given {
            Given::Returned { run: r, came, spent } => Given::Returned {
                run: run(*r)?,
                came: came.clone(),
                spent: *spent,
            },
            Given::Done { call: c, result } => Given::Done {
                call: call(*c)?,
                result: result.clone(),
            },
            Given::Answer {
                question, option, why, ..
            } => {
                let latest = (self.work.records.questions.iter())
                    .filter(|q| q.name == *question)
                    .map(|q| q.at)
                    .max()
                    .ok_or_else(|| format!("its path answers {question}, which played so was not asked"))?;
                Given::Answer {
                    question: question.clone(),
                    version: latest,
                    option: *option,
                    why: why.clone(),
                }
            }
            other => other.clone(),
        })
    }

    /// The cell of the run.
    fn cell_of(&self, run: RunId) -> Option<CellId> {
        (self.work.records.started.iter())
            .find(|s| s.run == run)
            .map(|s| s.cell.clone())
    }

    /// The round of the run under way, by its cell, the cell's nth run and the run's nth round.
    fn round_of(&self, run: RunId) -> Result<(CellId, usize, usize), String> {
        let cell = self
            .cell_of(run)
            .ok_or("its path returns a round of a run that did not start")?;
        Ok(self.round_now(&cell, run))
    }

    /// The round of this run of the cell that began last, by the cell, the cell's nth run and the run's nth round.
    fn round_now(&self, cell: &CellId, run: RunId) -> (CellId, usize, usize) {
        let records = &self.work.records;
        let nth = (records.started.iter())
            .filter(|s| s.cell == *cell && s.run <= run)
            .count();
        let number = records.rounds.iter().filter(|r| r.run == run).count();
        (cell.clone(), nth, number)
    }
}
