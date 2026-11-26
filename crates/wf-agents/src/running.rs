//! `wf-agents run`: the engine as a process. It drives every work the store in the data directory holds, on the flows
//! it is given, and serves the owner's pages over it. What carries the works out is the real carriers, as the
//! installation's settings say (the crate `carry`), or, given a script, agents, engine acts and tools that follow it.
//! It stops when the engine's loop stops, saying why; started again, it goes on from the store, every work held where
//! works were going on, until the owner lets them on (crate `driver`). An item is added at one
//! of the levels the settings name; with a script, at the one level a script's engine has.
//!
//! Before it touches the store it holds the data directory and has its port, so that an engine that cannot serve
//! hands nothing out, and two engines never drive the same works: each would give every round under way to its own
//! carriers, and both would carry it out.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions, TryLockError};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use flow::{Compiled, Has, Source};
use service::{Carriers, Levels, Projects, Script, Scripted, Service, SystemClock};
use store::Store;

use crate::carried;

/// The file in the data directory whose lock says that an engine runs on it.
const HELD: &str = "engine.lock";

/// What a flow run here may name: what every definition is compiled against. Of the sources, this process writes the
/// driver's, whether the items an item depends on have merged or were cancelled, and `session`, as a line begins a new
/// session or its session is compacted; a carrier writes `tree`, as it reads the design tree on the work's branch after
/// a round that committed to it, and, with the real carriers, `main`, through the acts' watch of main. A block of a
/// source that nothing has written yet stays unwritten, and a cell that reads one starts from no version of it.
fn has(dir: &Path) -> Has {
    crate::has(dir)
}

/// The flows the engine runs.
pub enum Flows {
    /// The flows in these directories, the first being the one the flow page shows.
    Directories(Vec<PathBuf>),
    /// These flows of the ones the program carries, by name, the first being the one the flow page shows: the engine
    /// takes the files they are carried in out into the data directory as it starts.
    Program(Vec<String>),
}

/// What carries the works out.
pub enum With {
    /// Agents, engine acts and tools that follow the script in this file.
    Script(PathBuf),
    /// The real carriers, as the settings in this file say.
    Settings(PathBuf),
}

/// What carries the works out, read and checked: a script, or the settings with what is written beside the flows.
enum Read {
    Script(Script),
    /// The settings, and what is written beside the flows: what the acts write and the agents' rounds are told, and
    /// what the form of a return and the system prompts are made of.
    Settings(Box<carry::Settings>, Box<flow::agents::Agents>),
}

/// The address the works page is printed at.
pub fn works_page(address: SocketAddr) -> String {
    format!("http://{address}/#/works")
}

/// Runs the engine on the data directory. `new` says that the directory is to get a store of its own: a directory that
/// holds a store is run in as it is, and one that holds none is run in only with it, so that a directory that is no data
/// directory is never made into one by being named. That is asked before anything is made in the directory, the hold
/// and the program's flows included.
pub fn run(data: &Path, flows: &Flows, with: With, port: u16, new: bool) -> ExitCode {
    if let Some(why) = directory_refusal(data, new) {
        eprintln!("{why}");
        return ExitCode::FAILURE;
    }
    // The program's own flows are taken out only once the data directory is held, so that a second engine that is
    // refused changes nothing of the files the first runs on.
    let (mut held, dirs) = match flows {
        Flows::Directories(dirs) => (None, dirs.clone()),
        Flows::Program(names) => match take_program_flows(data, carried::FLOWS, names) {
            Ok((held, dirs)) => (Some(held), dirs),
            Err(why) => {
                eprintln!("{why}");
                return ExitCode::FAILURE;
            }
        },
    };
    let dirs = &dirs[..];
    // The flow page shows the first flow given.
    let Some(shown) = dirs.first() else {
        eprintln!("no flow is given: the engine runs on at least one");
        return ExitCode::FAILURE;
    };
    // The settings' file, which the page shows and the owner changes there.
    let settings_file = match &with {
        With::Settings(file) => Some(file.clone()),
        With::Script(_) => None,
    };
    // The one project of a script's items, named before what carries the works out is read.
    let scripted = script_projects(&with);
    let Some((flows, with)) = read(dirs, &with) else {
        return ExitCode::FAILURE;
    };
    // Held until the process ends.
    let _held = match held.take() {
        Some(held) => held,
        None => match hold(data) {
            Ok(held) => held,
            Err(why) => {
                eprintln!("{why}");
                return ExitCode::FAILURE;
            }
        },
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("the engine cannot start: {e}");
            return ExitCode::FAILURE;
        }
    };
    // The loop runs on this thread, and the server of the pages on the runtime's other threads ([`loop_and_server`]),
    // so that what the loop does with the store holds up none of the threads that serve the pages or carry out the
    // rounds.
    runtime.block_on(async move {
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        let listener = match tokio::net::TcpListener::bind(address).await {
            Ok(listener) => listener,
            Err(e) => {
                eprintln!("{address}: {e}");
                return ExitCode::FAILURE;
            }
        };
        let address = match listener.local_addr() {
            Ok(address) => address,
            Err(e) => {
                eprintln!("{address}: {e}");
                return ExitCode::FAILURE;
            }
        };
        let store = match Store::open(data.join(store::FILE)) {
            Ok(store) => store,
            Err(fault) => {
                let words = service::store_fault_words(&fault);
                eprintln!(
                    "{}: the store cannot be opened: {words}",
                    data.join(store::FILE).display()
                );
                return ExitCode::FAILURE;
            }
        };
        let levels = match &with {
            Read::Script(_) => script_levels(),
            Read::Settings(settings, _) => match settings.levels() {
                Ok(levels) => levels,
                Err(why) => {
                    eprintln!("{why}");
                    return ExitCode::FAILURE;
                }
            },
        };
        // The projects an item may be added to: the settings'; a script's one, named as its file is.
        let projects = match &with {
            Read::Script(_) => scripted,
            Read::Settings(settings, _) => Projects::new(settings.projects.keys().cloned()),
        };
        // The checkout of the project whose repository holds the flows shown, for what the page shows an agent is sent,
        // its declaration (`wf-agents.toml`) and its files being read there; none where no project's does, as with a
        // script.
        let project = match &with {
            Read::Script(_) => None,
            Read::Settings(settings, _) => holding(settings, shown),
        };
        // What the block of main says as a watch reads its head, in the words its definition gives: the same in every
        // flow, whose engine's data are shared. Filled with the name of the project's main branch, which the settings give.
        let main_says = (flows.iter().flat_map(|flow| flow.blocks.values()))
            .find(|block| block.writers.outside.as_deref() == Some(carry::MAIN))
            .and_then(|block| block.writes.get("text").map(str::to_string))
            .unwrap_or_default();
        // The copies of the database are kept where the real carriers are, as the settings say; a script keeps none.
        let mut copies: Option<Keeping> = None;
        // The settings as the page shows them, beside those the engine runs with; a script has none.
        let mut install: Option<web::settings::Install> = None;
        let (carriers, watches): (Carriers, Vec<carry::ProjectWatch>) = match with {
            Read::Script(script) => (Scripted::new(script).carriers(), Vec::new()),
            Read::Settings(settings, beside) => {
                if let Some(file) = settings_file {
                    match installed(file, &settings) {
                        Ok(made) => install = Some(made),
                        Err(why) => {
                            eprintln!("{why}");
                            return ExitCode::FAILURE;
                        }
                    }
                }
                match keeping(data, settings.backup.as_ref()) {
                    Ok(keeping) => copies = Some(keeping),
                    Err(why) => {
                        eprintln!("{why}");
                        return ExitCode::FAILURE;
                    }
                }
                match carry::carry(*settings, data, *beside, &flows) {
                    Ok((carriers, watches, declarations)) => {
                        // The page shows each project's declaration as the carriers read it, or why it is held.
                        if let Some(install) = install.as_mut() {
                            install.declarations = declarations_shown(declarations);
                        }
                        (carriers, watches)
                    }
                    Err(why) => {
                        eprintln!("The carriers cannot be made: {why}");
                        return ExitCode::FAILURE;
                    }
                }
            }
        };
        // The version of the flow shown that the engine starts with, which the page says beside the files it reads.
        let installed = web::installed_version(&flows);
        let (service, inbox) = match Service::open(Box::new(store), flows, carriers, Arc::new(SystemClock)) {
            Ok(opened) => opened,
            Err(fault) => {
                eprintln!("The engine cannot start: {fault}.");
                return ExitCode::FAILURE;
            }
        };
        // The watch of each project's main writes, through the loop, the block of main of each work of its items: the
        // check before a merge then runs again when main has moved since it passed.
        for carry::ProjectWatch { project, main, watch } in watches {
            let (handle, main_says) = (service.handle(), main_says.clone());
            tokio::spawn(watch.run(move |head| {
                let says = flow::agents::fill(&main_says, &[("main", &main), ("head", &head)]);
                handle.outside(&project, carry::MAIN, &says);
            }));
        }
        // The page lays out what an agent is sent as a cell is entered, read from beside the flow shown, with the
        // project's own files as its checkout holds them.
        let in_dir = shown.clone();
        let mut flows_shown = web::Flows::dirs(shown.clone(), Box::new(move |_: &Source| has(&in_dir)));
        flows_shown.alike.project = project;
        flows_shown.installed = installed;
        // One task keeps the copies, a step at a time, with the time the loop reads from its clock; an engine that
        // keeps none says so on the route of the copies.
        let handle = service.handle();
        let about = copies.map(|keeping| {
            let about = keeping.about();
            keeping.run(handle.clone());
            about
        });
        let (levels_of_observations, projects_of_observations) = (levels.clone(), projects.clone());
        let router = web::with_works_and_copies(flows_shown, handle, levels, projects, about);
        // The observations are read, added to, changed and marked read through connections of their own to the store's
        // file.
        let router = router.merge(web::observations::router(
            service.handle(),
            data.join(store::FILE),
            levels_of_observations,
            projects_of_observations,
        ));
        let router = match install {
            Some(install) => router.merge(web::settings::router(install, service.handle())),
            None => router,
        };
        println!("The owner's pages are at {}", works_page(address));
        println!("wf-agents {}", crate::VERSION);
        let held = web::works::Engine::of(service.driver()).paused.map(|paused| paused.by);
        if held == Some(web::works::Pauser::Engine) {
            println!(
                "Works were going on when the engine stopped: every work is held until the owner has looked and lets \
                 them go on with `wf-agents resume --all`."
            );
        }
        match loop_and_server(service.run(inbox), web::serve(listener, router)).await {
            Ended::Loop(Ok(())) => {
                println!("The engine stopped, as it was asked to. Started again, it goes on from the store.");
                return ExitCode::SUCCESS;
            }
            Ended::Loop(Err(fault)) => {
                eprintln!("The engine stopped: {fault}. Started again, it goes on from the store.");
            }
            Ended::Server(why) => {
                eprintln!("The server of the pages stopped: {why}. Started again, the engine goes on from the store.");
            }
        }
        ExitCode::FAILURE
    })
}

/// Which of the engine's loop and the server of the pages ended first: the loop, with what it gave; or the server,
/// in words.
#[derive(Debug)]
enum Ended<T> {
    Loop(T),
    Server(String),
}

/// Runs the engine's loop on this thread and the server of the pages on the runtime's other threads, until either
/// ends. The server is spawned, not waited on together with the loop on this thread, where it would accept no
/// connection while the loop takes one thing, such as a read of every work.
async fn loop_and_server<L, S>(engine: L, server: S) -> Ended<L::Output>
where
    L: Future,
    S: Future<Output = std::io::Result<()>> + Send + 'static,
{
    let mut served = tokio::spawn(server);
    tokio::select! {
        ended = engine => Ended::Loop(ended),
        served = &mut served => Ended::Server(match served {
            Ok(Ok(())) => "it ended".to_string(),
            Ok(Err(e)) => e.to_string(),
            Err(e) => format!("it failed: {e}"),
        }),
    }
}

/// The settings as the page shows them: their file, read as the engine reads it, beside those the engine runs with. The
/// projects' declarations are given once the carriers have read them ([`declarations_shown`]).
fn installed(file: PathBuf, settings: &carry::Settings) -> Result<web::settings::Install, String> {
    let running = serde_json::to_value(settings).map_err(|e| format!("the settings cannot be shown: {e}"))?;
    Ok(web::settings::Install {
        file,
        read: Box::new(|text| {
            let settings = carry::Settings::read(text)?;
            settings.levels()?;
            serde_json::to_value(&settings).map_err(|e| e.to_string())
        }),
        running,
        declarations: BTreeMap::new(),
        git: &carry::git::SAFE,
    })
}

/// Each project's declaration as the carriers read it, or why it is held, as the page is given it.
fn declarations_shown(
    declarations: carry::projects::Declarations,
) -> BTreeMap<String, Result<web::settings::DeclarationText, String>> {
    (declarations.into_iter())
        .map(|(name, read)| {
            let read = read.map(|read| web::settings::DeclarationText {
                at: read.at,
                text: read.text,
            });
            (name, read)
        })
        .collect()
}

/// The directory in the data directory that holds the copies of the database.
const COPIES: &str = "backups";

/// What keeps the copies of the store in the data directory, as the settings say: on the machine alone where they give
/// no place off it, and else off it too. The two are different types of plan, since the place off the machine is a type
/// parameter of the step.
enum Keeping {
    Local(backup::Plan<backup::Nowhere>, backup::Disk),
    Off(backup::Plan<carry::GhOffsite>, backup::Disk),
}

/// What keeps the copies, with the engine's token for the place off the machine from its environment, and its own
/// directory for `gh`'s configuration, apart from the host's.
fn keeping(data: &Path, settings: Option<&carry::settings::Backup>) -> Result<Keeping, String> {
    let (store, dir) = (data.join(store::FILE), data.join(COPIES));
    let ground = backup::Disk::open(&store, &dir)?;
    let Some(b) = settings else {
        let plan = backup::Plan {
            store,
            dir,
            keep: carry::settings::KEEP,
            offsite: None,
        };
        return Ok(Keeping::Local(plan, ground));
    };
    let token = (std::env::var(&b.token_env).ok().filter(|token| !token.is_empty())).ok_or_else(|| {
        format!(
            "backup.token_env: the variable {} is not set: the engine needs its token for {}",
            b.token_env, b.repository
        )
    })?;
    let config = data.join("gh-backup");
    std::fs::create_dir_all(&config).map_err(|e| format!("{}: {e}", config.display()))?;
    let plan = backup::Plan {
        store,
        dir,
        keep: b.keep,
        offsite: Some(carry::GhOffsite::new(&b.gh, &b.repository, &token, &config)),
    };
    Ok(Keeping::Off(plan, ground))
}

impl Keeping {
    /// What the route of the copies is told of them.
    fn about(&self) -> web::Copies {
        match self {
            Keeping::Local(plan, _) => web::Copies {
                store: plan.store.clone(),
                keep: plan.keep,
                off_machine: plan.configured(),
            },
            Keeping::Off(plan, _) => web::Copies {
                store: plan.store.clone(),
                keep: plan.keep,
                off_machine: plan.configured(),
            },
        }
    }

    /// Starts the one task that keeps the copies. It takes the time from the loop, which alone reads the clock, and
    /// tells the pages after each step that the copies may have changed.
    fn run(self, handle: service::Handle) {
        let asks = handle.clone();
        let now = move || {
            let handle = asks.clone();
            async move { handle.now().await.map_err(|_| "the engine has stopped".to_string()) }
        };
        let said = move |out: &backup::Outcome| {
            said(out);
            handle.copies_changed();
        };
        match self {
            Keeping::Local(plan, ground) => tokio::spawn(backup::keep(plan, ground, now, said)),
            Keeping::Off(plan, ground) => tokio::spawn(backup::keep(plan, ground, now, said)),
        };
    }
}

/// Says what a step of keeping the copies did.
fn said(out: &backup::Outcome) {
    if let Some(name) = &out.taken {
        println!("Took a copy of the database: {name}");
    }
    for name in &out.uploaded {
        println!("Kept the copy {name} off this machine");
    }
    if let Some((step, why)) = &out.failed {
        eprintln!("Keeping the database's copies failed at {step}: {why}");
    }
}

/// The one project of the items of an engine whose agents follow a script, which runs in no repository: named as the
/// script's file is, without its extension.
fn script_projects(with: &With) -> Projects {
    let name = match with {
        With::Script(script) => script.file_stem().map(|stem| stem.to_string_lossy().into_owned()),
        With::Settings(_) => None,
    };
    Projects::one(&name.unwrap_or_default())
}

/// The checkout of the project whose repository holds the directory of the flow shown, where one does.
fn holding(settings: &carry::Settings, shown: &Path) -> Option<PathBuf> {
    let shown = std::fs::canonicalize(shown).ok()?;
    (settings.projects.values())
        .map(|project| &project.repo)
        .find(|repo| std::fs::canonicalize(repo).is_ok_and(|repo| shown.starts_with(repo)))
        .cloned()
}

/// The levels of the items of an engine whose agents follow a script, which run on no model: one, `1`, at which every
/// item is added.
fn script_levels() -> Levels {
    Levels::one("1", "The one level of an engine whose agents follow a script")
}

/// Holds the data directory, and only then takes the program's flows out into it: the files a running engine reads are
/// never changed by one that is refused. Gives the hold and the directory of each flow named.
fn take_program_flows(data: &Path, files: &[(&str, &[u8])], names: &[String]) -> Result<(File, Vec<PathBuf>), String> {
    let held = hold(data)?;
    let dirs = carried::take(data, files, names)?;
    Ok((held, dirs))
}

/// Holds the data directory for as long as the process runs: an exclusive lock of the system's on a file in it, taken
/// without waiting (`flock` where the system is Unix). The system lets go of it when the file is closed, which it is
/// when the process ends however it ends, killed or failed as much as stopped; a file made to say an engine runs, and
/// taken away when it stops, would outlive a process that was killed and keep the next one out for nothing. The file
/// is left in the directory: its lock, not the file, says that an engine runs.
pub(crate) fn hold(data: &Path) -> Result<File, String> {
    let path = data.join(HELD);
    let file = (OpenOptions::new().create(true).truncate(false).write(true).open(&path))
        .map_err(|e| format!("{}: the data directory cannot be held: {e}", path.display()))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(format!(
            "{}: an engine already runs on this data directory, and only one may",
            data.display()
        )),
        Err(TryLockError::Error(e)) => Err(format!("{}: the data directory cannot be held: {e}", path.display())),
    }
}

/// Why the directory cannot be run in, where it cannot: it is no directory, it holds a store and `--new` asks for one,
/// or it holds none and `--new` is not given. Nothing is made in it either way.
fn directory_refusal(data: &Path, new: bool) -> Option<String> {
    if !data.is_dir() {
        return Some(format!(
            "{}: no such directory: the data directory is made before the engine runs in it",
            data.display()
        ));
    }
    let store = data.join(store::FILE);
    match (store.exists(), new) {
        (true, true) => Some(format!(
            "{}: the data directory holds a store already: start the engine without --new",
            data.display()
        )),
        (false, false) => Some(format!(
            "{}: the directory holds no store, and no engine makes one in a directory it was not told is new: start the \
             engine with --new to make the data directory's store here",
            data.display()
        )),
        _ => None,
    }
}

/// The flows compiled and what carries the works out, checked against them; or none, having said everything wrong
/// with them. A script is checked only against flows that all compiled, so that it is not told of a flow missing for
/// that.
fn read(dirs: &[PathBuf], with: &With) -> Option<(Vec<Compiled>, Read)> {
    let mut wrong = false;
    let mut flows = Vec::new();
    let mut compiled_all = true;
    for dir in dirs {
        let source = match flow::read(dir) {
            Ok(source) => source,
            Err(e) => {
                eprintln!("{}: {e}", dir.display());
                compiled_all = false;
                continue;
            }
        };
        match flow::compile(&source, &has(dir)) {
            Ok(compiled) => flows.push(compiled),
            Err(faults) => {
                for fault in &faults {
                    eprintln!("{}/{}", dir.display(), crate::line(fault));
                }
                compiled_all = false;
            }
        }
    }
    // What the driver and the watch of main write of a flow, whatever carries its works out, checked once the flows
    // compiled: a text they fill that the flow does not give would be written empty.
    for fault in (flows.iter()).flat_map(|flow| [driver::faults(flow), carry::main_faults(flow)].concat()) {
        eprintln!("{fault}");
        wrong = true;
    }
    let read = match with {
        With::Script(script) => read_script(script, &flows, compiled_all, &mut wrong).map(Read::Script),
        With::Settings(settings) => match (carry::Settings::load(settings), beside(dirs, &flows)) {
            (Ok(settings), Ok(beside)) => {
                // What the real acts need of a flow and of their words, checked once the flows compiled; what the
                // system prompts of its lines are made of, its texts' settings with their values and its places
                // filled. A project whose declaration cannot be used is no fault of the engine's: it is held, and each
                // is said here.
                let start = start(&settings, &beside, &flows);
                for fault in start.faults {
                    eprintln!("{fault}");
                    wrong = true;
                }
                for line in start.held {
                    eprintln!("{line}");
                }
                Some(Read::Settings(Box::new(settings), Box::new(beside)))
            }
            (settings, beside) => {
                for why in [settings.err(), beside.err()].into_iter().flatten() {
                    eprintln!("{why}");
                }
                None
            }
        },
    };
    match read {
        Some(read) if !wrong && compiled_all => Some((flows, read)),
        _ => None,
    }
}

/// What the start finds wrong with what carries the works out, and what it holds back.
struct Start {
    /// What keeps the engine from starting, every fault at once.
    faults: Vec<String>,
    /// What keeps a project's works from being carried out, each said with the project's key: the project is held, its
    /// works are paused with the reason, and the other projects go on.
    held: Vec<String>,
}

/// What keeps the real carriers from carrying out the flows, every fault at once: what the acts need of a flow and of
/// their words beside the flows; what every agent is refused whatever its project, beside the flows, and the program
/// the installation names ([`carry::permissions::faults`]), and the variables the installation keeps for the agents
/// where the flows give them ([`carry::cli::kept_faults`]), the same for every project and said once; what the texts
/// sent to the agents name and hold where it is the flows', the same for every project; and two projects in one
/// repository, each said with the projects' keys. Of each project, what is wrong with its declaration on its main, with the files it names, its heavy
/// commands and the commands its host needs of it, is no fault of the engine's but holds that project alone
/// ([`carry::projects::declared`]): each held project is listed with why.
fn start(settings: &carry::Settings, beside: &flow::agents::Agents, flows: &[Compiled]) -> Start {
    let mut faults: Vec<String> = (flows.iter())
        .flat_map(|flow| carry::faults(flow, &beside.acts.check.values))
        .collect();
    let words = carry::word_faults(&beside.acts.check).into_iter();
    faults.extend(words.map(|fault| format!("{}/{fault}", beside.root.display())));
    faults.extend(carry::permissions::faults(beside, &settings.agents.command));
    faults.extend(carry::cli::kept_faults(
        &settings.agents.env,
        &beside.refused.environment,
    ));
    let (declared, wrong) = carry::projects::declared(settings, beside, flows);
    faults.extend(wrong);
    let held = (declared.iter())
        .filter_map(|(name, declared)| {
            let why = declared.as_ref().err()?;
            Some(format!(
                "projects.{name}: held, its works are paused until the engine is started with its declaration mended: \
                 {why}"
            ))
        })
        .collect();
    Start { faults, held }
}

/// What is written beside the flows, read from the directory that holds them, with the form of return of each line of
/// each flow that compiled made of it; or why it cannot be: the flows are not all in one directory, what is beside them
/// cannot be read, or a line's form cannot be made of it. Every flow is beside the one directory: what every flow shares is
/// written once, beside them all.
fn beside(dirs: &[PathBuf], flows: &[Compiled]) -> Result<flow::agents::Agents, String> {
    let holder = |dir: &PathBuf| {
        let dir = std::fs::canonicalize(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        (dir.parent().map(Path::to_path_buf)).ok_or_else(|| format!("{}: no directory holds it", dir.display()))
    };
    let mut roots = Vec::new();
    for dir in dirs {
        let root = holder(dir)?;
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    let [root] = roots.as_slice() else {
        return Err(format!(
            "the flows are in more than one directory ({}): every flow is beside the files they share",
            (roots.iter())
                .map(|r| r.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    };
    let beside = flow::agents::read(root)?;
    for compiled in flows {
        for line in compiled.lines.keys() {
            flow::form::form(compiled, &beside, line)?;
        }
    }
    Ok(beside)
}

/// The script, checked against the flows where they all compiled; or none, having said why it cannot be read.
fn read_script(script: &Path, flows: &[Compiled], compiled_all: bool, wrong: &mut bool) -> Option<Script> {
    match std::fs::read_to_string(script)
        .map_err(|e| e.to_string())
        .and_then(|t| Script::read(&t))
    {
        Ok(read) if !compiled_all => Some(read),
        Ok(read) => {
            for fault in read.check(flows) {
                eprintln!("{}: {fault}", script.display());
                *wrong = true;
            }
            Some(read)
        }
        Err(e) => {
            eprintln!("{}: {e}", script.display());
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_engine_leaves_the_flows_of_the_first_as_they_are() {
        let data = tempfile::tempdir().unwrap();
        let flows = data.path().join("flows");
        std::fs::create_dir_all(flows.join("alpha")).unwrap();
        std::fs::write(flows.join("alpha/kept.toml"), "kept").unwrap();
        let files: [(&str, &[u8]); 1] = [("alpha/flow.toml", b"new")];
        let names = ["alpha".to_string()];
        let first = hold(data.path()).unwrap();
        let said = take_program_flows(data.path(), &files, &names).unwrap_err();
        assert!(said.contains("an engine already runs on this data directory"), "{said}");
        assert!(
            flows.join("alpha/kept.toml").exists(),
            "a start that was refused took the flows out all the same"
        );
        assert_eq!(std::fs::read_to_string(flows.join("alpha/kept.toml")).unwrap(), "kept");
        assert!(!flows.join("alpha/flow.toml").exists());
        assert!(!data.path().join("flows.new").exists());
        // Once the first has gone, the same call takes the files out in place of the old.
        drop(first);
        let (_held, dirs) = take_program_flows(data.path(), &files, &names).unwrap();
        assert_eq!(dirs, [flows.join("alpha")]);
        assert_eq!(std::fs::read_to_string(flows.join("alpha/flow.toml")).unwrap(), "new");
        assert!(!flows.join("alpha/kept.toml").exists());
    }

    #[test]
    fn the_server_of_the_pages_runs_while_the_loop_holds_its_thread() {
        // As `run` builds it: on one thread, what the loop takes holds that thread until it is done.
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let (ran, heard) = std::sync::mpsc::channel();
        let ended = runtime.block_on(loop_and_server(
            // The loop takes one thing, which holds its thread until the server says it ran, at most ten seconds.
            async move { heard.recv_timeout(std::time::Duration::from_secs(10)).is_ok() },
            // Not at the first poll, which the loop's thread could give it before the loop takes its thing.
            async move {
                tokio::task::yield_now().await;
                let _ = ran.send(());
                std::future::pending::<std::io::Result<()>>().await
            },
        ));
        assert!(
            matches!(ended, Ended::Loop(true)),
            "the server did not run while the loop held its thread: {ended:?}"
        );
    }

    /// The directory that holds the repository's flows.
    fn flows() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../flow")
    }

    /// The engineer's flow as the repository defines it, compiled as `wf-agents run` compiles it.
    fn engineer() -> Compiled {
        let dir = flows().join("engineer");
        let source = flow::read(&dir).expect("the flow is read");
        flow::compile(&source, &has(&dir)).expect("the flow compiles")
    }

    /// Runs git in the directory, which must pass.
    fn git(dir: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// The repository's own declaration, `wf-agents.toml`, with the command that checks the sample's project appended,
    /// which is hosted in its local repository.
    fn declaration() -> String {
        let text = std::fs::read_to_string(flows().join("../wf-agents.toml")).expect("the declaration is read");
        format!("{text}\n[local]\ncheck = \"./test.sh\"\n")
    }

    /// A repository whose main holds the files the declaration names, and the declaration where one is given.
    fn repository(repo: &Path, declaration: Option<&str>) {
        std::fs::create_dir_all(repo.join("design")).unwrap();
        git(repo, &["init", "-q", "-b", "main"]);
        for (key, value) in [
            ("user.name", "Tester"),
            ("user.email", "tester@example.com"),
            ("commit.gpgsign", "false"),
        ] {
            git(repo, &["config", key, value]);
        }
        for path in ["AGENTS.md", "design/constitution.md"] {
            std::fs::write(repo.join(path), "Some text.\n").unwrap();
        }
        if let Some(declaration) = declaration {
            std::fs::write(repo.join(flow::agents::DECLARATION), declaration).unwrap();
        }
        git(repo, &["add", "."]);
        git(repo, &["commit", "-q", "-m", "Files"]);
    }

    /// The sample installation's settings.
    const SAMPLE: &str = include_str!("../../carry/install.sample.toml");

    /// The sample's settings, on a repository of their own whose main holds the files its declaration names, and the
    /// declaration at its root.
    fn settings(dir: &Path) -> carry::Settings {
        let mut settings = carry::Settings::read(SAMPLE).expect("it reads");
        let repo = dir.join("repo");
        repository(&repo, Some(&declaration()));
        settings.projects.get_mut("example").expect("the sample's project").repo = repo;
        settings
    }

    /// The settings with the project `name` added, hosted in its local repository or on GitHub, in a repository of its
    /// own that holds the declaration where one is given.
    fn with_project(settings: &mut carry::Settings, dir: &Path, name: &str, declaration: Option<&str>, github: bool) {
        let mut project = if github {
            let table = format!(
                "\n[projects.{name}]\nrepo = \"/the/repository\"\nmain = \"main\"\n\n[projects.{name}.github]\n\
                 repository = \"owner/{name}\"\n"
            );
            carry::Settings::read(&format!("{SAMPLE}{table}"))
                .expect("it reads")
                .projects[name]
                .clone()
        } else {
            settings.projects["example"].clone()
        };
        project.repo = dir.join(name);
        repository(&project.repo, declaration);
        // A project on GitHub branches from the main its host last gave, which is a reference of the remote's.
        if let Some(github) = &project.github {
            let kept = format!("refs/remotes/{}/{}", github.remote, project.main);
            git(&project.repo, &["update-ref", &kept, "refs/heads/main"]);
        }
        settings.projects.insert(name.to_string(), project);
    }

    /// The declaration less the tables headed by these lines, each with its keys.
    fn without(text: &str, headers: &[&str]) -> String {
        let mut kept = Vec::new();
        let mut dropping = false;
        for line in text.lines() {
            if line.starts_with('[') {
                dropping = headers.contains(&line.trim_end());
            }
            if !dropping {
                kept.push(line);
            }
        }
        kept.join("\n") + "\n"
    }

    // A variable the flows give every agent's program that the installation's settings also keep of the engine's
    // environment (`agents.env`) is a fault of the engine's, said as it starts, with the key that names it; without it
    // the engine starts with no fault. Well under a millisecond: the one test that `start` asks for these faults, which
    // the test of the function alone does not show.
    #[test]
    fn a_setting_that_keeps_a_variable_the_flows_give_every_agents_program_keeps_the_engine_from_starting() {
        let dir = tempfile::tempdir().expect("a directory");
        let mut settings = settings(dir.path());
        let beside = beside(&[flows().join("engineer")], &[]).expect("the files beside the flows");
        assert_eq!(start(&settings, &beside, &[engineer()]).faults, Vec::<String>::new());
        settings.agents.env = vec!["GIT_ALLOW_PROTOCOL".to_string()];
        let started = start(&settings, &beside, &[engineer()]);
        assert_eq!(started.faults.len(), 1, "{:?}", started.faults);
        assert!(
            started.faults[0].starts_with("agents.env: GIT_ALLOW_PROTOCOL"),
            "{:?}",
            started.faults
        );
    }

    // A project whose declaration cannot be used is held, and every one is said at once with its project's key and why,
    // whatever else is wrong: the declaration main does not hold, one that names no heavy commands, variables it names
    // for the agents' command line that the engine, the settings or the agents' program own, and time limits of its
    // workflows that are no times. None of it keeps the engine from starting, and the project that is fine is not held.
    // Two projects that name one repository are a fault of the engine's, which their declarations do not cause, and
    // keep it from starting.
    #[test]
    fn a_project_whose_declaration_cannot_be_used_is_held_and_each_is_said_with_its_key_and_why() {
        let dir = tempfile::tempdir().expect("a directory");
        let mut settings = settings(dir.path());
        let good = declaration();
        let (unheavy, _) = good
            .split_once("\n[heavy]\n")
            .expect("the declaration names its heavy commands");
        with_project(&mut settings, dir.path(), "other", None, false);
        with_project(&mut settings, dir.path(), "lacking", Some(unheavy), false);
        // The same declaration naming, for the agents' command line, a variable the engine owns, one the agents' program
        // reads itself and one the settings keep of the engine's environment.
        settings.agents.env = vec!["LANGUAGE".to_string()];
        let named = good.replace(
            "RUSTUP_AUTO_INSTALL = \"0\"",
            "PATH = \"/elsewhere\"\nANTHROPIC_BASE_URL = \"https://example.com\"\nLANGUAGE = \"en\"",
        );
        assert_ne!(named, good, "the declaration names the variable that is replaced");
        with_project(&mut settings, dir.path(), "unsafe", Some(&named), false);
        // The same declaration whose full and measure workflows' time limits are no times.
        let slow = good
            .replace("too_long = \"2h\"", "too_long = \"two hours\"")
            .replace("too_long = \"30m\"", "too_long = \"30\"");
        assert_ne!(slow, good, "the declaration names the limits that are replaced");
        with_project(&mut settings, dir.path(), "untimed", Some(&slow), false);
        let beside = beside(&[flows().join("engineer")], &[]).expect("the files beside the flows");
        let started = start(&settings, &beside, &[engineer()]);
        assert_eq!(started.faults, Vec::<String>::new(), "no fault of the engine's");
        assert_eq!(started.held.len(), 4, "{:?}", started.held);
        let held = |name: &str| {
            (started.held.iter())
                .find(|line| line.starts_with(&format!("projects.{name}: held")))
                .unwrap_or_else(|| panic!("{name} is held: {:?}", started.held))
        };
        assert!(
            held("other").contains("refs/heads/main:wf-agents.toml, the project's declaration on main where works branch from, cannot be read"),
            "{:?}",
            started.held
        );
        assert!(
            held("lacking").contains("refs/heads/main:wf-agents.toml: ")
                && held("lacking").contains("missing field `heavy`"),
            "{:?}",
            started.held
        );
        let unsafe_line = held("unsafe");
        for fault in [
            "wf-agents.toml [environment]: ANTHROPIC_BASE_URL is read by the agents' program itself, which it would \
             change in a round beside what the records name",
            "wf-agents.toml [environment]: LANGUAGE is one the installation's agents.env keeps of the engine's \
             environment, which a declared value would override",
            "wf-agents.toml [environment]: PATH is the engine's own, which it gives every round itself",
        ] {
            assert!(unsafe_line.contains(fault), "{fault}: {unsafe_line}");
        }
        let untimed = held("untimed");
        assert!(
            untimed.contains("ci.full.too_long: \"two hours\" is no time such as 90m or 2h")
                && untimed.contains("ci.measure.too_long: \"30\" is no time such as 90m or 2h"),
            "{untimed}"
        );
        // Two projects in one repository keep the engine from starting, and neither is held for it.
        let example = settings.projects["example"].clone();
        settings.projects.insert("same".to_string(), example);
        let started = start(&settings, &beside, &[engineer()]);
        assert_eq!(started.faults.len(), 1, "{:?}", started.faults);
        assert!(
            started.faults[0].starts_with("projects.example, projects.same: name one repository, "),
            "{:?}",
            started.faults
        );
        assert_eq!(started.held.len(), 4, "{:?}", started.held);
    }

    // The engine starts with its one project held, as with any: the held project's reason is the only thing said.
    #[test]
    fn the_engine_starts_with_its_only_project_held_and_says_why() {
        let dir = tempfile::tempdir().expect("a directory");
        let mut settings = carry::Settings::read(SAMPLE).expect("it reads");
        let repo = dir.path().join("repo");
        repository(&repo, None);
        settings.projects.get_mut("example").expect("the sample's project").repo = repo;
        let beside = beside(&[flows().join("engineer")], &[]).expect("the files beside the flows");
        let started = start(&settings, &beside, &[engineer()]);
        assert_eq!(started.faults, Vec::<String>::new());
        assert_eq!(started.held.len(), 1, "{:?}", started.held);
        assert!(
            started.held[0].starts_with("projects.example: held"),
            "{:?}",
            started.held
        );
        assert!(started.held[0].contains("wf-agents.toml"), "{:?}", started.held);
    }

    // What a project's host needs of its declaration is its declaration's to hold, and the declaration protects itself:
    // a local repository needs the command that checks it, GitHub how its pull requests are merged, and where the
    // declaration is not among the paths it protects, a work may change what names the commands the engine runs
    // without the owner's yes; and a fast workflow given less time than the CI is given to start. A GitHub project that
    // declares no workflows is not held, since none are needed until a test or the check runs.
    #[test]
    fn a_declaration_lacking_what_the_projects_host_needs_or_not_protecting_itself_holds_its_project() {
        let dir = tempfile::tempdir().expect("a directory");
        let mut settings = settings(dir.path());
        let good = declaration();
        let unprotected = good.replace(", \"wf-agents.toml\"]", "]");
        assert_ne!(unprotected, good, "the declaration lists itself as protected");
        with_project(
            &mut settings,
            dir.path(),
            "nolocal",
            Some(&without(&good, &["[local]"])),
            false,
        );
        with_project(
            &mut settings,
            dir.path(),
            "nopull",
            Some(&without(&good, &["[pull_request]"])),
            true,
        );
        with_project(&mut settings, dir.path(), "unprotected", Some(&unprotected), false);
        // A GitHub project that declares no workflows is carried.
        let no_ci = without(&good, &["[ci.fast]", "[ci.full]", "[ci.measure]"]);
        assert!(!no_ci.contains("[ci."), "{no_ci}");
        with_project(&mut settings, dir.path(), "noci", Some(&no_ci), true);
        // A fast workflow given less time than the CI is given to start (the settings' `github.start_minutes`, 10 for a
        // project that names none) is held; one given exactly that time is carried. The first `too_long` of the
        // declaration is the fast workflow's.
        let first = good.find("too_long = \"2h\"").expect("the declaration gives a time");
        assert!(good.find("[ci.fast]").unwrap() < first && first < good.find("[ci.full]").unwrap());
        let fast_limit = |limit: &str| good.replacen("too_long = \"2h\"", &format!("too_long = \"{limit}\""), 1);
        with_project(&mut settings, dir.path(), "short", Some(&fast_limit("5m")), true);
        with_project(&mut settings, dir.path(), "exact", Some(&fast_limit("10m")), true);
        let beside = beside(&[flows().join("engineer")], &[]).expect("the files beside the flows");
        let started = start(&settings, &beside, &[engineer()]);
        assert_eq!(started.faults, Vec::<String>::new());
        let names: Vec<&str> = started
            .held
            .iter()
            .map(|line| &line[..line.find(": ").unwrap()])
            .collect();
        assert_eq!(
            names,
            [
                "projects.nolocal",
                "projects.nopull",
                "projects.short",
                "projects.unprotected"
            ],
            "{:?}",
            started.held
        );
        assert!(started.held[0].contains("has no [local]"), "{:?}", started.held);
        assert!(started.held[1].contains("has no [pull_request]"), "{:?}", started.held);
        assert!(
            started.held[2].contains(
                "[ci.fast] too_long is less than the 10 minutes the settings give the CI to start \
                 (github.start_minutes)"
            ),
            "{:?}",
            started.held
        );
        assert!(
            started.held[3].contains("[protected] paths must hold wf-agents.toml itself"),
            "{:?}",
            started.held
        );
    }

    // The flows given are beside one directory, whose files every flow shares; flows from two are refused.
    #[test]
    fn flows_from_more_than_one_directory_are_refused() {
        let read = beside(&[flows().join("engineer"), flows().join("deliver")], &[]).expect("one directory");
        assert_eq!(read.root, std::fs::canonicalize(flows()).unwrap());
        let other = tempfile::tempdir().expect("a directory");
        std::fs::create_dir_all(other.path().join("elsewhere")).unwrap();
        let refused = beside(&[flows().join("engineer"), other.path().join("elsewhere")], &[]).expect_err("refused");
        assert!(
            refused.starts_with("the flows are in more than one directory"),
            "{refused}"
        );
    }

    // As the engine starts, the sample's installation, on the local repository with no fast CI, carries the engineer's
    // flow out with nothing wrong. What the texts sent to the agents lack is a fault of the flow's, whatever the
    // project, and keeps the engine from starting, said with what the acts need: here a place no one fills in the
    // heading of the project's files, for every line. What main lacks of the project's files is the project's: the
    // tree's rules not on main hold the project, and the flow's faults are said all the same.
    #[test]
    fn the_start_says_every_fault_of_the_prompts_and_holds_the_project_that_lacks_a_file() {
        let dir = tempfile::tempdir().expect("a directory");
        let settings = settings(dir.path());
        let mut beside = beside(&[flows().join("engineer")], &[]).expect("the files beside the flows");
        let compiled = [engineer()];
        let started = start(&settings, &beside, &compiled);
        assert_eq!(
            (started.faults, started.held),
            (Vec::<String>::new(), Vec::<String>::new())
        );
        // The program the installation names is refused as an entry is, so that one holding a `*` would refuse more than
        // it names: the start is kept from going on, and says the setting.
        let mut starred = settings.clone();
        starred.agents.command = "co*dex".to_string();
        let started = start(&starred, &beside, &compiled);
        assert_eq!(started.faults.len(), 1, "{:?}", started.faults);
        assert!(
            started.faults[0].starts_with("agents.command: "),
            "{:?}",
            started.faults
        );
        beside.session.heading = "# `{path}` {where}\n\n{text}".to_string();
        let mut lines: Vec<&str> = compiled[0].cells.iter().filter_map(|c| c.line.as_deref()).collect();
        lines.sort();
        lines.dedup();
        let started = start(&settings, &beside, &compiled);
        assert_eq!(started.faults.len(), lines.len(), "{:?}", started.faults);
        assert!(
            (started.faults.iter())
                .all(|f| f.ends_with("flow/agents/session.toml: the engine fills no place `{where}`")),
            "{:?}",
            started.faults
        );
        assert_eq!(started.held, Vec::<String>::new(), "the flow's fault holds no project");
        let repo = &settings.projects["example"].repo;
        git(repo, &["rm", "-q", "design/constitution.md"]);
        git(repo, &["commit", "-q", "-m", "No constitution"]);
        let started = start(&settings, &beside, &compiled);
        assert_eq!(started.faults.len(), lines.len(), "{:?}", started.faults);
        assert_eq!(started.held.len(), 1, "{:?}", started.held);
        assert!(
            started.held[0].starts_with("projects.example: held")
                && started.held[0].contains(
                    "wf-agents.toml: the file constitution, design/constitution.md, is not on main, \
                     refs/heads/main, where works branch from"
                ),
            "{:?}",
            started.held
        );
    }

    // The page reads a change of the file as the engine reads the file, and runs with the settings the engine read.
    #[test]
    fn the_page_reads_the_settings_as_the_engine_reads_them() {
        let sample = include_str!("../../carry/install.sample.toml");
        let settings = carry::Settings::read(sample).expect("the sample reads");
        let install = installed(PathBuf::from("install.toml"), &settings).expect("it is made");
        assert_eq!((install.read)(sample), Ok(install.running.clone()));
        assert_eq!(install.running["agents"]["slots"], 2);
        assert_eq!(
            install.running["agents"]["subagents"]["most"], 3,
            "a key left out at its default"
        );
        let misspelt = sample.replace("slots = 2", "slots = 2\nslot = 3");
        assert!((install.read)(&misspelt).expect_err("refused").contains("slot"));
        let no_level = sample.replace("highest_level = \"3\"", "highest_level = \"9\"");
        assert_eq!(
            (install.read)(&no_level),
            Err("items.highest_level: there is no level \"9\"".to_string())
        );
    }

    // A value the engine reads as the one it runs with is shown with no other beside it, however the file writes it: a
    // whole number for a decimal one, a table written in part; one changed in the file by hand since the engine started
    // is, the slots too, which the engine takes at once only as the page changes them.
    #[test]
    fn the_page_shows_beside_a_key_only_a_value_the_engine_reads_otherwise() {
        let sample = include_str!("../../carry/install.sample.toml")
            .replace("work_budget_usd = 5.0", "work_budget_usd = 5")
            .replace("# subagents = { most = 3 }", "subagents = {}");
        assert!(
            sample.contains("\nsubagents = {}"),
            "the sample's table of subagents is written in part"
        );
        let settings = carry::Settings::read(&sample).expect("it reads");
        let dir = tempfile::tempdir().expect("a directory");
        let file = dir.path().join("install.toml");
        std::fs::write(&file, &sample).unwrap();
        let install = installed(file.clone(), &settings).expect("it is made");
        let differ = |page: &web::settings::SettingsPage| -> Vec<String> {
            let projects = page.projects.iter().flat_map(|p| p.tables.iter());
            (page.tables.iter().chain(projects))
                .flat_map(|t| {
                    (t.keys.iter())
                        .filter(|k| k.written.is_some() && k.running.is_some())
                        .map(move |k| format!("{}.{} runs with {}", t.name, k.key, k.running.as_deref().unwrap_or("")))
                })
                .collect()
        };
        let page = web::settings::page_of(&install, settings.agents.slots).expect("it is shown");
        assert_eq!(page.unreadable, None);
        assert_eq!(differ(&page), Vec::<String>::new());
        std::fs::write(&file, sample.replace("slots = 2", "slots = 3")).unwrap();
        let page = web::settings::page_of(&install, settings.agents.slots).expect("it is shown");
        assert_eq!(differ(&page), ["agents.slots runs with 2"]);
    }
}
