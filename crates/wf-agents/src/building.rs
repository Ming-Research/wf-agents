//! `wf-agents play --states`: the build of every state of the page (design document `testing`, 16 to 21). It takes the
//! states of each flow given from the flow's exploration and the engine's own kinds (module [`cases`], module
//! [`rules`]), gives each to an item of the installation's data in turn (principle harness-only), and plays each store
//! of them as `wf-agents play` plays a script (crate module `playing`), on the engine `wf-agents run` makes, on a clock
//! of its own that starts at the time given, or at a play's bound before now. Beside each store it writes the script
//! it played, its list of states, each with the item that shows it and each it could not bring about with why
//! (principle nothing-passes-for-normal), its serving script, under which every round and every call takes six hours,
//! and how it is served, in words and as `serving.toml`; and beside the stores, the list of every state, counted by
//! kind, with how many ways to a merge and points of a cancel the exploration reached. It fails where a state it played
//! was not reached.
//!
//! The stores are the plan's table: the main store, which holds every state but those below, main moving in it only as
//! the works start; every slot taken; each kind of failure of a round that holds every work; the pause of everything;
//! the works whose paths move main once, and each whose path moves it more often, alone; and a stall, with the other
//! states of the flow that reaches one, on which the rules make nothing but cancels. Each is served by `wf-agents run
//! --script` with its serving script on a fresh copy of its data directory: a store with a round left under way starts
//! with every work held by the engine, the hold after a restart, and is then let on, but the pause of everything, which
//! lifting would undo; a store with none starts as it was left.
//!
//! The items file is TOML: each `[[items]]` a `title`, its `background`, the `flow` its work runs on, and the items it
//! depends on (`depends_on`), by their places in the file counted from one. A state is given to the next item not yet
//! given one, of its flow, and of none where its flow has none left: one that waits for an item it depends on to an
//! item that depends on others, and the item it depends on is added never let on; every other to one that depends on
//! none. The project the items are added in is named as the file is, without its extension.

mod cases;
mod rules;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use decide::BlockId;
use decide::run::Failure;
use explore::model::Model;
use flow::Compiled;
use flow::agents::Agents;
use serde::{Deserialize, Serialize};
use service::script::items::{ItemScript, Step, StepScript, WriteScript};
use service::script::{Down, FlowScript, OutsideScript, Took, ToolScript};
use service::{Clock, Script, SystemClock};
use work::Time;

use crate::playing::{self, BOUND};
use crate::running;
use cases::{Case, Store};

/// The bound of each flow's exploration: as the flow's own tests explore it. A smaller one would build other states
/// than the page can show: the deep research's runs past its first, and every other return of a way that spent one.
const EXPLORED: explore::Bounds = explore::Bounds {
    spend: 2,
    outside_after: 1,
    steps: 200,
    states: 200_000,
};

/// How long a round of a store's build takes, in milliseconds, but for one left under way.
const ROUND_MS: u64 = 1000;

/// How long every round and every call takes under a store's serving script: six hours, so that a work let on, answered
/// or left under way stays under way while it is looked at.
const SERVED_MS: u64 = 6 * 3_600_000;

/// What the check of the agents' outside says it finds, in a store of a failure that holds every work.
const DOWN: &str = "The outside the agents' rounds need is down, as the build of the page's states left it.";

/// The file beside a store that says how it is served, for whatever serves it.
pub const SERVING: &str = "serving.toml";

/// The items file.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Items {
    items: Vec<FileItem>,
}

/// An item of the installation's data, which a build gives a state.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileItem {
    title: String,
    #[serde(default)]
    background: String,
    flow: String,
    #[serde(default)]
    depends_on: Vec<usize>,
}

/// How a store is served: with its serving script on a fresh copy of its data directory, on the flows in these
/// directories; whether it starts with every work held by the engine, a round having been left under way; and whether
/// it is then let on.
#[derive(Serialize)]
pub struct Serving {
    pub data: PathBuf,
    pub flows: Vec<PathBuf>,
    pub script: PathBuf,
    pub held: bool,
    pub let_on: bool,
}

/// A flow given, with where it is and what is written beside it.
struct Given {
    dir: PathBuf,
    compiled: Compiled,
    /// The directory that holds it and what its flows share.
    root: PathBuf,
}

/// A state as a store's list names it: made, by its item and work, or not, with why.
struct Listed {
    kind: String,
    state: String,
    flow: String,
    made: Result<(usize, String), String>,
}

/// What a flow's exploration reached, for the owner's count of 18 (design document `testing`): the states it kept that
/// stand merged, each a way to a merge, and those that have not ended, each a point a cancel can come at.
#[derive(Default)]
struct Counted {
    merged: u64,
    points: u64,
}

/// Builds every state of the page from the flows in these directories, with the items of this file, into this data
/// directory, which holds nothing yet, its clock starting at `start`, or at a play's bound before now.
pub fn build(data: &Path, dirs: &[PathBuf], items: &Path, start: Option<u64>) -> ExitCode {
    match built(data, dirs, items, start) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}

/// The build; whether every state it played was reached, or why it could not be made.
fn built(data: &Path, dirs: &[PathBuf], items: &Path, start: Option<u64>) -> Result<bool, String> {
    let holds = std::fs::read_dir(data)
        .map_err(|e| format!("{}: {e}", data.display()))?
        .next()
        .is_some();
    if holds {
        return Err(format!(
            "{}: the data directory holds files already: a build writes its stores into an empty one",
            data.display()
        ));
    }
    let given = flows(dirs)?;
    let mut words: BTreeMap<PathBuf, Agents> = BTreeMap::new();
    for root in given.iter().map(|g| g.root.clone()) {
        if words.contains_key(&root) {
            continue;
        }
        let of: Vec<&Given> = given.iter().filter(|g| g.root == root).collect();
        let dirs: Vec<PathBuf> = of.iter().map(|g| g.dir.clone()).collect();
        let compiled: Vec<Compiled> = of.iter().map(|g| g.compiled.clone()).collect();
        words.insert(root, running::beside(&dirs, &compiled)?);
    }
    let text = std::fs::read_to_string(items).map_err(|e| format!("{}: {e}", items.display()))?;
    let file: Items = toml::from_str(&text).map_err(|e| format!("{}: {e}", items.display()))?;
    let project = running::script_project(items);
    let start = match start {
        Some(start) => start,
        None => SystemClock
            .now()
            .map_err(|e| format!("the clock cannot be read: {e}"))?
            .0
            .saturating_sub(BOUND.ms),
    };
    // The states of each flow, each with its store, and those that could not be made, with why.
    let mut all: Vec<Case> = Vec::new();
    let mut not: Vec<Listed> = Vec::new();
    let mut people: BTreeMap<String, Vec<BlockId>> = BTreeMap::new();
    let mut counted = Counted::default();
    let mut moves = 0;
    for g in &given {
        let model = Model::new(g.compiled.clone());
        people.insert(g.compiled.name.clone(), model.people.clone());
        let began = std::time::Instant::now();
        let (of_flow, explored, unmade) = explored(&model, &words[&g.root], EXPLORED);
        println!(
            "{}: {} states explored, {} of them shown apart, in {:.1?}",
            g.compiled.name,
            explored.stats.states,
            explored.grouped.len(),
            began.elapsed()
        );
        for (state, count) in &explored.grouped {
            match state.starts_with(&Step::Ends(decide::End::Merged).to_string()) {
                true => counted.merged += count,
                false if !state.starts_with("{ ends") => counted.points += count,
                false => {}
            }
        }
        not.extend(unmade.into_iter().map(|(state, why)| Listed {
            kind: "the exploration's".to_string(),
            state,
            flow: g.compiled.name.clone(),
            made: Err(why),
        }));
        let mut of_flow = of_flow;
        let stalls = of_flow
            .iter()
            .any(|c| matches!(c.end, service::script::items::Step::Stalled(_)));
        for case in &mut of_flow {
            case.store = match (stalls, case.mains) {
                (true, _) => Store::Stalled,
                (false, 0) => Store::Main,
                (false, 1) => Store::MainMoved,
                (false, _) => {
                    moves += 1;
                    Store::Moves(moves)
                }
            };
        }
        // A flow that reaches a stall is built in the store of the stall alone: the rules' states but its cancels are
        // made on the others.
        if !stalls {
            let (ruled, unmade) = rules::ruled(&g.compiled, &words[&g.root], &of_flow);
            of_flow.extend(ruled);
            not.extend(unmade.into_iter().map(|(state, why)| Listed {
                kind: "the rules'".to_string(),
                state,
                flow: g.compiled.name.clone(),
                made: Err(why),
            }));
        }
        let (cancelled, unmade) = rules::cancels(&of_flow);
        of_flow.extend(cancelled);
        not.extend(unmade.into_iter().map(|(state, why)| Listed {
            kind: "cancelled".to_string(),
            state,
            flow: g.compiled.name.clone(),
            made: Err(why),
        }));
        all.extend(of_flow);
    }
    // The stores, in the order of the plan's table.
    let mut stores: Vec<(Store, Vec<Case>)> = Vec::new();
    for case in all {
        match stores.iter().position(|(s, _)| *s == case.store) {
            Some(i) => stores[i].1.push(case),
            None => stores.push((case.store.clone(), vec![case])),
        }
    }
    let mut pool = Pool {
        used: vec![false; file.items.len()],
        items: file.items,
    };
    let mut listed: Vec<(String, Vec<Listed>, String)> = Vec::new();
    let mut reached_all = true;
    for (store, cases) in &stores {
        let making = Making {
            data,
            given: &given,
            words: &words,
            project: &project,
            start,
            people: &people,
            store,
            cases,
        };
        let began = std::time::Instant::now();
        let made = make(&making, &mut pool)?;
        reached_all &= made.reached_all;
        println!(
            "{}: {} states made, {} not, in {} things the play took, in {:.1?}",
            store.dir(),
            made.list.iter().filter(|l| l.made.is_ok()).count(),
            made.list.iter().filter(|l| l.made.is_err()).count(),
            made.steps,
            began.elapsed()
        );
        listed.push((store.dir(), made.list, made.serving));
    }
    write_list(data, items, start, &listed, &not, &counted)?;
    println!("Every state's list: {}", data.join("states.md").display());
    Ok(reached_all)
}

/// The cases of a flow's exploration at this bound: of each state the page shows apart, the nearest path a play can
/// be given made a case; with the exploration, and each state no path to which a play can be given, with why the
/// nearest could not.
fn explored(
    model: &Model,
    words: &Agents,
    bounds: explore::Bounds,
) -> (Vec<Case>, explore::Explored, Vec<(String, String)>) {
    let made: RefCell<BTreeMap<String, Case>> = RefCell::new(BTreeMap::new());
    let why: RefCell<BTreeMap<String, String>> = RefCell::new(BTreeMap::new());
    let group = |work: &work::Work, decision: &decide::Decision| cases::state_of(&model.compiled, work, decision);
    let take = |state: &str, path: &explore::Path| match cases::case_of(model, state, path, words) {
        Ok(case) => {
            made.borrow_mut().insert(state.to_string(), case);
            true
        }
        Err(not) => {
            why.borrow_mut().entry(state.to_string()).or_insert(not);
            false
        }
    };
    let explored = explore::explore_grouped(model, bounds, &group, &take);
    let made = made.into_inner();
    let unmade = (why.into_inner().into_iter())
        .filter(|(state, _)| !made.contains_key(state))
        .collect();
    (made.into_values().collect(), explored, unmade)
}

/// The flows in these directories, compiled.
fn flows(dirs: &[PathBuf]) -> Result<Vec<Given>, String> {
    let mut given = Vec::new();
    for dir in dirs {
        let source = flow::read(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let compiled = flow::compile(&source, &crate::has(dir)).map_err(|faults| {
            let faults: Vec<String> = faults
                .iter()
                .map(|f| format!("{}/{}", dir.display(), crate::line(f)))
                .collect();
            faults.join("\n")
        })?;
        let full = std::fs::canonicalize(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let root = full.parent().map(Path::to_path_buf).unwrap_or_default();
        given.push(Given {
            dir: dir.clone(),
            compiled,
            root,
        });
    }
    Ok(given)
}

/// The items of the file, and which of them were given a state.
struct Pool {
    items: Vec<FileItem>,
    used: Vec<bool>,
}

impl Pool {
    /// The next item not yet given a state: of the flow, depending on others where `depending` and on none otherwise;
    /// else any of the flow; else any.
    fn take(&mut self, flow: &str, depending: bool) -> Option<usize> {
        let free = |k: &usize| !self.used[*k];
        let all = 0..self.items.len();
        let found = (all.clone())
            .filter(free)
            .find(|k| self.items[*k].flow == flow && self.items[*k].depends_on.is_empty() != depending)
            .or_else(|| all.clone().filter(free).find(|k| self.items[*k].flow == flow))
            .or_else(|| all.clone().find(free));
        if let Some(k) = found {
            self.used[k] = true;
        }
        found
    }

    /// An item the one of this place depends on, not yet given a state; else the next not yet given one.
    fn depended_on(&mut self, k: usize) -> Option<usize> {
        let on = (self.items[k].depends_on.iter().copied())
            .filter_map(|place| place.checked_sub(1))
            .find(|j| *j < self.items.len() && !self.used[*j]);
        match on {
            Some(j) => {
                self.used[j] = true;
                Some(j)
            }
            None => {
                let flow = self.items[k].flow.clone();
                self.take(&flow, false)
            }
        }
    }
}

/// What a store is made of.
struct Making<'a> {
    data: &'a Path,
    given: &'a [Given],
    words: &'a BTreeMap<PathBuf, Agents>,
    project: &'a str,
    start: u64,
    people: &'a BTreeMap<String, Vec<BlockId>>,
    store: &'a Store,
    cases: &'a [Case],
}

/// A store made: its list, how it is served, and whether every state it played was reached.
struct Made {
    list: Vec<Listed>,
    serving: String,
    reached_all: bool,
    /// How many things the store's play took.
    steps: usize,
}

/// Makes the store: gives each of its states an item of the pool, writes its script, plays it, and writes beside it the
/// script, the serving script and how it is served.
fn make(m: &Making<'_>, pool: &mut Pool) -> Result<Made, String> {
    let dir = m.data.join(m.store.dir());
    let names: Vec<&str> = m.cases.iter().map(|c| c.flow.as_str()).collect();
    let of_store: Vec<&Given> = (m.given.iter())
        .filter(|g| names.contains(&g.compiled.name.as_str()))
        .collect();
    let roots: Vec<&PathBuf> = of_store.iter().map(|g| &g.root).collect();
    let Some(root) = roots.first() else {
        return Err(format!("{}: a store of no flow", m.store.dir()));
    };
    if roots.iter().any(|r| r != root) {
        let names: Vec<&str> = of_store.iter().map(|g| g.compiled.name.as_str()).collect();
        return Err(format!(
            "{}: its flows, {}, are in more than one directory, and a store is served with one",
            m.store.dir(),
            names.join(", ")
        ));
    }
    let words = &m.words[*root];
    let flows: Vec<Compiled> = of_store.iter().map(|g| g.compiled.clone()).collect();
    let dirs: Vec<PathBuf> = of_store.iter().map(|g| g.dir.clone()).collect();
    let mut list: Vec<Listed> = Vec::new();
    // Each item of the script, with the state its work is to show.
    let mut items: Vec<ItemScript> = Vec::new();
    let mut shown: Vec<(usize, String, String, String)> = Vec::new();
    let mut rounds: BTreeMap<String, Vec<service::script::RoundScript>> = BTreeMap::new();
    let mut holding = 0;
    for case in m.cases {
        let Some(k) = pool.take(&case.flow, case.unmerged) else {
            list.push(Listed {
                kind: case.kind.clone(),
                state: case.state.clone(),
                flow: case.flow.clone(),
                made: Err("no item of the items file is left for it".to_string()),
            });
            continue;
        };
        let mut depends_on = Vec::new();
        if case.unmerged {
            let Some(j) = pool.depended_on(k) else {
                list.push(Listed {
                    kind: case.kind.clone(),
                    state: case.state.clone(),
                    flow: case.flow.clone(),
                    made: Err("no item of the items file is left for the item it depends on".to_string()),
                });
                continue;
            };
            let held = rules::never_let_on(&case.flow);
            items.push(item(m, &pool.items[j], &held, Vec::new()));
            shown.push((items.len(), held.kind.clone(), held.state.clone(), held.flow.clone()));
            depends_on.push(items.len());
        }
        items.push(item(m, &pool.items[k], case, depends_on));
        let place = items.len();
        shown.push((place, case.kind.clone(), case.state.clone(), case.flow.clone()));
        let last = case.rounds.len().saturating_sub(1);
        for (n, round) in case.rounds.iter().enumerate() {
            let mut round = round.clone();
            round.work = Some(place as u64);
            // The turn its check waits for is another work's: the first's, or the second's where it is the first.
            if case.turn && n == last {
                round.turn = Some(if place == 1 { 2 } else { 1 });
            }
            rounds.entry(case.flow.clone()).or_default().push(round);
        }
        holding += usize::from(case.holds_slot);
    }
    // Every work's first round may go on at once, so that each work stands at its first state as main moves.
    let slots = match m.store {
        Store::Slots => holding.max(1),
        _ => items.len() + holding + 2,
    };
    let outside = outside(m.store);
    let script = Script {
        start: Some(m.start),
        slots: Some(slots),
        took: Took {
            ms: Some(ROUND_MS),
            ..Took::default()
        },
        tools: BTreeMap::new(),
        outside: outside.clone(),
        flows: (rounds.into_iter())
            .map(|(flow, rounds)| {
                let script = FlowScript {
                    values: BTreeMap::new(),
                    rounds,
                };
                (flow, script)
            })
            .collect(),
        items,
    };
    let written = toml::to_string(&script).map_err(|e| format!("{}: its script: {e}", m.store.dir()))?;
    let build = dir.join("build");
    let store_data = dir.join("data");
    let serve = dir.join("serve");
    for d in [&build, &store_data, &serve] {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let script_file = build.join(format!("{}.toml", m.project));
    std::fs::write(&script_file, &written).map_err(|e| format!("{}: {e}", script_file.display()))?;
    let played = playing::played(&store_data, &dirs, &script_file, Time(m.start)).ok_or_else(|| {
        format!(
            "{}: its script could not be played, as said above",
            script_file.display()
        )
    })?;
    let mut reached_all = played.fault.is_none();
    for (k, (place, kind, state, flow)) in shown.into_iter().enumerate() {
        let made = match played.items.get(k) {
            Some(item) if item.took() => {
                let title = (script.items.get(place - 1))
                    .map(|i| i.title.clone())
                    .unwrap_or_default();
                Ok((place, title))
            }
            Some(item) => {
                reached_all = false;
                Err(format!("its play: {}", item.line(place - 1)))
            }
            None => {
                reached_all = false;
                Err("its play has no such item".to_string())
            }
        };
        list.push(Listed {
            kind,
            state,
            flow,
            made,
        });
    }
    if let Some(stopped) = played.stopped() {
        reached_all = false;
        list.push(Listed {
            kind: "the play".to_string(),
            state: "the play of the store".to_string(),
            flow: String::new(),
            made: Err(stopped),
        });
    }
    // The serving script, under which every round and call takes six hours, and how the store is served.
    let tools = (flows.iter())
        .flat_map(|flow| flow.tools.iter().chain(flow.cells.iter().flat_map(|c| &c.tools)))
        .map(|tool| {
            let script = ToolScript {
                answer: None,
                ms: Some(SERVED_MS),
                at: None,
            };
            (tool.clone(), script)
        })
        .collect();
    let serving = Script {
        start: Some(m.start),
        slots: Some(slots),
        took: Took {
            ms: Some(SERVED_MS),
            ..Took::default()
        },
        tools,
        outside,
        flows: BTreeMap::new(),
        items: Vec::new(),
    };
    let serving_text = toml::to_string(&serving).map_err(|e| format!("{}: its serving script: {e}", m.store.dir()))?;
    let serving_file = serve.join(format!("{}.toml", m.project));
    let faults = Script::read(&serving_text)
        .map(|read| read.check(&flows, words))
        .map_err(|e| format!("{}: {e}", serving_file.display()))?;
    if !faults.is_empty() {
        return Err(faults
            .iter()
            .map(|f| format!("{}: {f}", serving_file.display()))
            .collect::<Vec<_>>()
            .join("\n"));
    }
    std::fs::write(&serving_file, &serving_text).map_err(|e| format!("{}: {e}", serving_file.display()))?;
    // A store with a work going on, one that has not ended and that its own pause does not hold, starts with every
    // work held by the engine, unless the owner's pause of everything holds them (decision
    // a-start-pauses-everything-only-over-works-going-on); one with none starts as it was left.
    let going_on = |c: &&Case| !c.held && !matches!(c.end, Step::Ends(_) | Step::Paused(_));
    let held = *m.store != Store::Everything && m.cases.iter().any(|c| going_on(&c));
    let let_on = held;
    let how = Serving {
        data: store_data.clone(),
        flows: dirs.clone(),
        script: serving_file.clone(),
        held,
        let_on,
    };
    let how_text = toml::to_string(&how).map_err(|e| format!("{}: how it is served: {e}", m.store.dir()))?;
    std::fs::write(dir.join(SERVING), how_text).map_err(|e| format!("{}: {e}", dir.join(SERVING).display()))?;
    let flow_args: Vec<String> = dirs.iter().map(|d| format!("--flow {}", d.display())).collect();
    let run = format!(
        "`wf-agents run --data <a fresh copy of {}> {} --script {} --port <port>`",
        store_data.display(),
        flow_args.join(" "),
        serving_file.display()
    );
    let serving = match (held, m.store) {
        (true, _) => format!(
            "Served by {run}. A work being on its way, it starts with every work held by the engine, the hold after a \
             restart; `wf-agents resume --all --port <port>` lets it on, and every other state shows."
        ),
        (false, Store::Everything) => format!(
            "Served by {run}. It starts with everything paused by the owner, as it was left, and is not let on: \
             lifting the pause of everything would undo the state it shows."
        ),
        (false, _) => format!(
            "Served by {run}. No work of it being on its way, each having ended or being paused, it starts as it was \
             left, holding nothing, and shows its states as it starts."
        ),
    };
    write_store(&dir, m.store, &list, &serving, &played.untaken)?;
    Ok(Made {
        list,
        serving,
        reached_all,
        steps: played.steps,
    })
}

/// What the check of the agents' outside finds in a store of a failure that holds every work: the failure, standing,
/// and for a usage limit a reset a year after the start.
fn outside(store: &Store) -> OutsideScript {
    let finds = match store {
        Store::Failed(failure) if failure.shared() => Some(Down {
            failure: *failure,
            why: DOWN.to_string(),
            resets: (*failure == Failure::UsageLimit).then_some(rules::YEAR),
        }),
        _ => None,
    };
    OutsideScript { finds }
}

/// The item of a script that shows the case's state: the file's item's title, what people write of it its background,
/// and the case's acts and path.
fn item(m: &Making<'_>, file: &FileItem, case: &Case, depends_on: Vec<usize>) -> ItemScript {
    let background = match file.background.trim().is_empty() {
        true => file.title.clone(),
        false => file.background.clone(),
    };
    let writes = (m.people.get(&case.flow).into_iter().flatten())
        .map(|block| WriteScript {
            block: block.to_string(),
            content: background.clone(),
            value: None,
        })
        .collect();
    ItemScript {
        title: file.title.clone(),
        flow: case.flow.clone(),
        depends_on,
        writes,
        held: case.held,
        acts: case.acts.clone(),
        path: case.path.iter().map(StepScript::of).collect(),
    }
}

/// Writes the store's list of states, each with the item that shows it, and each not brought about with why; how it is
/// served; and the script's round entries no round took.
fn write_store(dir: &Path, store: &Store, list: &[Listed], serving: &str, untaken: &[String]) -> Result<(), String> {
    let mut s = format!("# The store {}\n\n{serving}\n\n## Its states\n\n", store.dir());
    for l in list.iter().filter(|l| l.made.is_ok()) {
        if let Ok((place, title)) = &l.made {
            s.push_str(&format!(
                "- {} ({}, {}): item {place}, \"{title}\"\n",
                l.state, l.flow, l.kind
            ));
        }
    }
    let not: Vec<&Listed> = list.iter().filter(|l| l.made.is_err()).collect();
    if !not.is_empty() {
        s.push_str("\n## Not brought about\n\n");
        for l in not {
            if let Err(why) = &l.made {
                s.push_str(&format!("- {} ({}, {}): {why}\n", l.state, l.flow, l.kind));
            }
        }
    }
    if !untaken.is_empty() {
        s.push_str("\n## Round entries of its script no round took\n\n");
        for u in untaken {
            s.push_str(&format!("- {u}\n"));
        }
    }
    let file = dir.join("states.md");
    std::fs::write(&file, s).map_err(|e| format!("{}: {e}", file.display()))
}

/// Writes the list of every state beside the stores: each kind with how many states of it were made and how many not,
/// how many ways to a merge and points of a cancel the exploration reached, each store with its count, every state
/// made with its store and item, and each state not brought about with why.
fn write_list(
    data: &Path,
    items: &Path,
    start: u64,
    stores: &[(String, Vec<Listed>, String)],
    not: &[Listed],
    counted: &Counted,
) -> Result<(), String> {
    let mut kinds: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for l in stores.iter().flat_map(|(_, list, _)| list).chain(not) {
        let count = kinds.entry(l.kind.as_str()).or_default();
        match l.made {
            Ok(_) => count.0 += 1,
            Err(_) => count.1 += 1,
        }
    }
    let mut s = format!(
        "# Every state of the page\n\nBuilt with the items of {}, its clock starting at {start}.\n\n## By kind\n\n",
        items.display()
    );
    for (kind, (made, unmade)) in &kinds {
        s.push_str(&format!("- {kind}: {made} made, {unmade} not\n"));
    }
    s.push_str(&format!(
        "\n## What the explorations reached\n\n- states merged, each a way to a merge: {}\n- states not ended, each a \
         point a cancel can come at: {}\n",
        counted.merged, counted.points
    ));
    s.push_str("\n## The stores\n\n");
    for (dir, list, serving) in stores {
        let made = list.iter().filter(|l| l.made.is_ok()).count();
        s.push_str(&format!(
            "- {dir}: {made} states, listed in {dir}/states.md. {serving}\n"
        ));
    }
    s.push_str("\n## Every state made, by its store\n\n");
    for (dir, list, _) in stores {
        for l in list {
            if let Ok((place, title)) = &l.made {
                s.push_str(&format!(
                    "- {} ({}, {}): {dir}, item {place}, \"{title}\"\n",
                    l.state, l.flow, l.kind
                ));
            }
        }
    }
    let unmade: Vec<&Listed> = (stores.iter().flat_map(|(_, list, _)| list).chain(not))
        .filter(|l| l.made.is_err())
        .collect();
    if !unmade.is_empty() {
        s.push_str("\n## Not brought about\n\n");
        for l in unmade {
            if let Err(why) = &l.made {
                s.push_str(&format!("- {} ({}, {}): {why}\n", l.state, l.flow, l.kind));
            }
        }
    }
    let file = data.join("states.md");
    std::fs::write(&file, s).map_err(|e| format!("{}: {e}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bound the flow and the flow renamed are explored at: the exploration's own renaming property's.
    const SMALL: explore::Bounds = explore::Bounds {
        spend: 1,
        outside_after: 1,
        steps: 200,
        states: 200_000,
    };

    /// Each state the build would make on the flow, in words: its kind and state, its path, and its acts and rounds.
    fn states(compiled: &Compiled, words: &Agents) -> Vec<String> {
        let model = Model::new(compiled.clone());
        let (mut of_flow, _, unmade) = explored(&model, words, SMALL);
        let mut said: Vec<String> = unmade
            .into_iter()
            .map(|(state, why)| format!("not {state}: {why}"))
            .collect();
        let (ruled, not) = rules::ruled(compiled, words, &of_flow);
        of_flow.extend(ruled);
        said.extend(not.into_iter().map(|(state, why)| format!("not {state}: {why}")));
        let (cancelled, not) = rules::cancels(&of_flow);
        of_flow.extend(cancelled);
        said.extend(not.into_iter().map(|(state, why)| format!("not {state}: {why}")));
        for case in of_flow {
            let path: Vec<String> = case.path.iter().map(ToString::to_string).collect();
            said.push(format!(
                "{} | {} | {} | {:?} | {:?}",
                case.kind,
                case.state,
                path.join(", "),
                case.acts,
                case.rounds
            ));
        }
        said.sort();
        said
    }

    /// The text with each word a renamed name gives back as the name it replaced, as it is or in small letters, and so
    /// a word that begins with one of them and `_`, as the exploration names a cell's question by its cell.
    fn back(text: &str, names: &BTreeMap<String, String>) -> String {
        let mut to: BTreeMap<String, String> = BTreeMap::new();
        for (was, is) in names {
            to.insert(is.clone(), was.clone());
            to.insert(is.to_lowercase(), was.to_lowercase());
        }
        let given = |word: &str| -> String {
            if let Some(was) = to.get(word) {
                return was.clone();
            }
            let prefix = (to.iter())
                .filter(|(is, _)| word.starts_with(&format!("{is}_")))
                .max_by_key(|(is, _)| is.len());
            match prefix {
                Some((is, was)) => format!("{was}{}", &word[is.len()..]),
                None => word.to_string(),
            }
        };
        let mut out = String::with_capacity(text.len());
        let mut word = String::new();
        for c in text.chars().chain(std::iter::once(' ')) {
            if c.is_ascii_alphanumeric() || c == '_' {
                word.push(c);
                continue;
            }
            out.push_str(&given(&word));
            word.clear();
            out.push(c);
        }
        out.pop();
        out
    }

    // The build names no cell, block or trigger: the engineer's flow with every one of them renamed alike gives the
    // same states, their paths, acts and rounds the same, their names following. Ten seconds or so alone, at the bound
    // the exploration's own renaming property explores at.
    #[test]
    fn the_flow_renamed_gives_the_same_states_its_names_following() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../flow/engineer");
        let source = flow::read(&dir).expect("the flow is read");
        let has = crate::has(&dir);
        let compiled = flow::compile(&source, &has).expect("the flow compiles");
        let (names, renamed) = explore::renaming::renamed(&source);
        let renamed = flow::compile(&renamed, &has).expect("the flow renamed compiles");
        let words = running::beside(std::slice::from_ref(&dir), std::slice::from_ref(&compiled)).expect("the words");
        let were = states(&compiled, &words);
        let mut are: Vec<String> = states(&renamed, &words).iter().map(|s| back(s, &names)).collect();
        are.sort();
        assert!(were.len() > 10, "{were:#?}");
        let only =
            |a: &[String], b: &[String]| a.iter().filter(|s| !b.contains(s)).take(3).cloned().collect::<Vec<_>>();
        assert!(
            are == were,
            "renamed, {} states against {}; only the flow's: {:#?}; only the flow renamed's: {:#?}",
            are.len(),
            were.len(),
            only(&were, &are),
            only(&are, &were)
        );
    }
}
