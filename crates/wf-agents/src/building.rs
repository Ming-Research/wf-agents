//! `wf-agents play --states`: the build of every state of the page (design document `testing`, 16 to 21). It takes the
//! states of each flow given from the flow's exploration and the engine's own kinds (module [`cases`], module
//! [`rules`]), gives each to an item of the installation's data in turn (principle harness-only), and plays each store
//! of them as `wf-agents play` plays a script, on the engine `wf-agents run` makes, on a clock of its own that starts
//! at the time given, or at a play's bound before now. Beside each store it writes the script it played, its list of
//! states, each with the item that shows it and each it could not bring about with why (principle
//! nothing-passes-for-normal), its serving script, under which every round and every call takes six hours, and how it
//! is served; and beside the stores, the list of every state, counted by kind. It fails where a state it played was
//! not reached.
//!
//! The stores are the plan's table: the main store, which holds every state but those below, main never moving in it;
//! every slot taken; each kind of failure of a round that holds every work; the pause of everything; the works whose
//! paths move main once, and each whose path moves it more often, alone; and a stall, with the other states of the flow
//! that reaches one, on which the rules make nothing. Each is served by `wf-agents run --script` with its serving
//! script on a fresh copy of its data directory: the main store, and every one with a round left under way, starts with
//! every work held by the engine, the hold after a restart, and is then let on, but the pause of everything, which
//! lifting would undo.
//!
//! The items file is TOML: each `[[items]]` a `title`, its `background`, the `flow` its work runs on, and the items it
//! depends on (`depends_on`), by their places in the file counted from one. A state is given to the next item not yet
//! given one, of its flow, and of none where its flow has none left: one that waits for an item it depends on to an
//! item that depends on others, and the item it depends on is added never let on; every other to one that depends on
//! none. The project the items are added in is named as the file is, without its extension.

mod cases;
mod rules;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use decide::BlockId;
use decide::run::Failure;
use explore::model::Model;
use flow::Compiled;
use flow::agents::Agents;
use serde::Deserialize;
use service::{Clock, Script, SystemClock};
use toml::{Table, Value};
use work::Time;

use crate::playing::BOUND;
use crate::running;
use cases::{Case, Store, step_value, text, took};

/// The bound of each flow's exploration: as the flow's own tests explore it.
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
    let mut moves = 0;
    for g in &given {
        let model = Model::new(g.compiled.clone());
        people.insert(g.compiled.name.clone(), model.people.clone());
        let group = |work: &work::Work, decision: &decide::Decision| cases::state_of(&model.compiled, work, decision);
        let explored = explore::explore_grouped(&model, EXPLORED, &group);
        let mut of_flow = Vec::new();
        for (state, path) in &explored.groups {
            match cases::case_of(&model, state, path) {
                Ok(case) => of_flow.push(case),
                Err(why) => not.push(Listed {
                    kind: "the exploration's".to_string(),
                    state: state.clone(),
                    flow: g.compiled.name.clone(),
                    made: Err(why),
                }),
            }
        }
        // Whether the exploration reached a stall, made or not.
        let stalls = (explored.groups.values()).any(|p| {
            matches!(
                decide::decide(&g.compiled.flow, &p.work.records),
                decide::Decision::Stalled { .. }
            )
        });
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
        // A flow that reaches a stall is built in the store of the stall alone: the rules' states are made on the
        // others.
        if !stalls {
            let words = &words[&g.root];
            let (ruled, unmade) = rules::ruled(&g.compiled, words, &of_flow);
            of_flow.extend(ruled);
            not.extend(unmade.into_iter().map(|(state, why)| Listed {
                kind: "the rules'".to_string(),
                state,
                flow: g.compiled.name.clone(),
                made: Err(why),
            }));
        }
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
        items: file.items,
        used: Vec::new(),
    };
    pool.used = vec![false; pool.items.len()];
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
        let made = make(&making, &mut pool)?;
        reached_all &= made.reached_all;
        println!(
            "{}: {} states made, {} not",
            store.dir(),
            made.list.iter().filter(|l| l.made.is_ok()).count(),
            made.list.iter().filter(|l| l.made.is_err()).count()
        );
        listed.push((store.dir(), made.list, made.serving));
    }
    write_list(data, items, start, &listed, &not)?;
    println!("Every state's list: {}", data.join("states.md").display());
    Ok(reached_all)
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
    let mut list: Vec<Listed> = Vec::new();
    // Each item of the script, with the state its work is to show.
    let mut items: Vec<Value> = Vec::new();
    let mut shown: Vec<(usize, String, String, String)> = Vec::new();
    let mut rounds: BTreeMap<String, Vec<Value>> = BTreeMap::new();
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
        for round in &case.rounds {
            let mut round = round.clone();
            round.insert("work".to_string(), Value::Integer(place as i64));
            rounds.entry(case.flow.clone()).or_default().push(Value::Table(round));
        }
        holding += usize::from(case.holds_slot);
    }
    let slots = match m.store {
        Store::Slots => holding.max(1),
        _ => 2 * holding + 2,
    };
    let mut script = Table::new();
    script.insert("start".to_string(), Value::Integer(m.start as i64));
    script.insert("slots".to_string(), Value::Integer(slots as i64));
    script.insert("took".to_string(), took(ROUND_MS));
    if let Some(outside) = outside(m.store) {
        script.insert("outside".to_string(), outside);
    }
    let mut flows_t = Table::new();
    for (flow, rounds) in rounds {
        let mut t = Table::new();
        t.insert("rounds".to_string(), Value::Array(rounds));
        flows_t.insert(flow, Value::Table(t));
    }
    script.insert("flows".to_string(), Value::Table(flows_t));
    script.insert("items".to_string(), Value::Array(items));
    let written = toml::to_string(&script).map_err(|e| format!("{}: its script: {e}", m.store.dir()))?;
    let build = dir.join("build");
    let store_data = dir.join("data");
    let serve = dir.join("serve");
    for d in [&build, &store_data, &serve] {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let script_file = build.join(format!("{}.toml", m.project));
    std::fs::write(&script_file, &written).map_err(|e| format!("{}: {e}", script_file.display()))?;
    let read = Script::read(&written).map_err(|e| format!("{}: {e}", script_file.display()))?;
    let faults = read.check(&flows, words);
    if !faults.is_empty() {
        return Err(faults
            .iter()
            .map(|f| format!("{}: {f}", script_file.display()))
            .collect::<Vec<_>>()
            .join("\n"));
    }
    let played = play(&read, &flows, words, &store_data, m.project, m.start)?;
    let mut reached_all = played.fault.is_none();
    for (k, (place, kind, state, flow)) in shown.into_iter().enumerate() {
        let made = match played.items.get(k) {
            Some(item) if item.took() => {
                let title = (read.items.get(place - 1)).map(|i| i.title.clone()).unwrap_or_default();
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
    let mut serving = Table::new();
    serving.insert("start".to_string(), Value::Integer(m.start as i64));
    serving.insert("slots".to_string(), Value::Integer(slots as i64));
    serving.insert("took".to_string(), took(SERVED_MS));
    let mut tools = Table::new();
    for flow in &flows {
        for tool in flow.tools.iter().chain(flow.cells.iter().flat_map(|c| &c.tools)) {
            let mut t = Table::new();
            t.insert("ms".to_string(), Value::Integer(SERVED_MS as i64));
            tools.insert(tool.clone(), Value::Table(t));
        }
    }
    serving.insert("tools".to_string(), Value::Table(tools));
    if let Some(outside) = outside(m.store) {
        serving.insert("outside".to_string(), outside);
    }
    let serving_text = toml::to_string(&serving).map_err(|e| format!("{}: its serving script: {e}", m.store.dir()))?;
    let serving_file = serve.join(format!("{}.toml", m.project));
    let serving_read = Script::read(&serving_text).map_err(|e| format!("{}: {e}", serving_file.display()))?;
    let faults = serving_read.check(&flows, words);
    if !faults.is_empty() {
        return Err(faults
            .iter()
            .map(|f| format!("{}: {f}", serving_file.display()))
            .collect::<Vec<_>>()
            .join("\n"));
    }
    std::fs::write(&serving_file, &serving_text).map_err(|e| format!("{}: {e}", serving_file.display()))?;
    let flow_args: Vec<String> = of_store.iter().map(|g| format!("--flow {}", g.dir.display())).collect();
    let run = format!(
        "`wf-agents run --data <a fresh copy of {}> {} --script {} --port <port>`",
        store_data.display(),
        flow_args.join(" "),
        serving_file.display()
    );
    let serving = match m.store {
        Store::Everything => format!(
            "Served by {run}, and not let on: lifting the hold the engine starts with would lift the pause of \
             everything, which is the state it shows."
        ),
        _ => format!(
            "Served by {run}. A round having been left under way, it starts with every work held by the engine, the \
             hold after a restart; `wf-agents resume --all --port <port>` lets it on, and every other state shows."
        ),
    };
    write_store(&dir, m.store, &list, &serving, &played.untaken)?;
    Ok(Made {
        list,
        serving,
        reached_all,
    })
}

/// What the check of the agents' outside finds in a store of a failure that holds every work: the failure, standing,
/// and for a usage limit a reset a year after the start.
fn outside(store: &Store) -> Option<Value> {
    let Store::Failed(failure) = store else {
        return None;
    };
    if !failure.shared() {
        return None;
    }
    let mut finds = Table::new();
    finds.insert("failure".to_string(), text(&cases::failure_name(*failure)));
    finds.insert("why".to_string(), text(DOWN));
    if *failure == Failure::UsageLimit {
        finds.insert("resets".to_string(), Value::Integer(rules::YEAR as i64));
    }
    let mut outside = Table::new();
    outside.insert("finds".to_string(), Value::Table(finds));
    Some(Value::Table(outside))
}

/// The item of a script that shows the case's state: the file's item's title, what people write of it its background,
/// and the case's acts and path.
fn item(m: &Making<'_>, file: &FileItem, case: &Case, depends_on: Vec<usize>) -> Value {
    let mut t = Table::new();
    t.insert("title".to_string(), text(&file.title));
    t.insert("flow".to_string(), text(&case.flow));
    if !depends_on.is_empty() {
        let on = depends_on.into_iter().map(|p| Value::Integer(p as i64)).collect();
        t.insert("depends_on".to_string(), Value::Array(on));
    }
    let background = match file.background.trim().is_empty() {
        true => file.title.clone(),
        false => file.background.clone(),
    };
    let writes: Vec<Value> = (m.people.get(&case.flow).into_iter().flatten())
        .map(|block| {
            let mut w = Table::new();
            w.insert("block".to_string(), text(block.as_str()));
            w.insert("content".to_string(), text(&background));
            Value::Table(w)
        })
        .collect();
    if !writes.is_empty() {
        t.insert("writes".to_string(), Value::Array(writes));
    }
    if case.held {
        t.insert("held".to_string(), Value::Boolean(true));
    }
    if !case.acts.is_empty() {
        t.insert(
            "acts".to_string(),
            Value::Array(case.acts.iter().cloned().map(Value::Table).collect()),
        );
    }
    t.insert(
        "path".to_string(),
        Value::Array(case.path.iter().map(step_value).collect()),
    );
    Value::Table(t)
}

/// Plays the store's script over its data directory, as `wf-agents play` plays one.
fn play(
    script: &Script,
    flows: &[Compiled],
    words: &Agents,
    data: &Path,
    project: &str,
    start: u64,
) -> Result<service::Played, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .map_err(|e| format!("the engine cannot start: {e}"))?;
    let open = |clock: Arc<dyn Clock>| {
        let carriers = running::script_carriers(script.clone(), words.clone());
        running::engine(data, flows.to_vec(), carriers, clock)
    };
    runtime
        .block_on(service::play(script, flows, words, &open, project, Time(start), BOUND))
        .map_err(|faults| faults.join("\n"))
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
/// each store with its count, and each state not brought about with why.
fn write_list(
    data: &Path,
    items: &Path,
    start: u64,
    stores: &[(String, Vec<Listed>, String)],
    not: &[Listed],
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
    s.push_str("\n## The stores\n\n");
    for (dir, list, serving) in stores {
        let made = list.iter().filter(|l| l.made.is_ok()).count();
        s.push_str(&format!(
            "- {dir}: {made} states, listed in {dir}/states.md. {serving}\n"
        ));
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
        let group = |work: &work::Work, decision: &decide::Decision| cases::state_of(&model.compiled, work, decision);
        let explored = explore::explore_grouped(&model, SMALL, &group);
        let mut of_flow = Vec::new();
        let mut said = Vec::new();
        for (state, path) in &explored.groups {
            match cases::case_of(&model, state, path) {
                Ok(case) => of_flow.push(case),
                Err(why) => said.push(format!("not {state}: {why}")),
            }
        }
        let (ruled, not) = rules::ruled(compiled, words, &of_flow);
        of_flow.extend(ruled);
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

    /// The text with each word a renamed name gives back as the name it replaced.
    fn back(text: &str, names: &BTreeMap<String, String>) -> String {
        let to: BTreeMap<&str, &str> = names.iter().map(|(was, is)| (is.as_str(), was.as_str())).collect();
        let mut out = String::with_capacity(text.len());
        let mut word = String::new();
        for c in text.chars().chain(std::iter::once(' ')) {
            if c.is_ascii_alphanumeric() || c == '_' {
                word.push(c);
                continue;
            }
            out.push_str(to.get(word.as_str()).copied().unwrap_or(&word));
            word.clear();
            out.push(c);
        }
        out.pop();
        out
    }

    // The build names no cell, block or trigger: the engineer's flow with every one of them renamed alike gives the
    // same states, their paths, acts and rounds the same, their names following. A second or so alone, at the bound the
    // exploration's own renaming property explores at.
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
        assert_eq!(are, were);
    }
}
