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

use std::fs::{File, OpenOptions, TryLockError};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use flow::{Compiled, Has, Source};
use service::{Carriers, Levels, Projects, Script, Scripted, Service, SystemClock};
use store::Store;

/// The file in the data directory whose lock says that an engine runs on it.
const HELD: &str = "engine.lock";

/// What a flow run here may name: what every definition is compiled against. Of the sources, this process writes the
/// driver's, whether the items an item depends on have merged, and, with the real carriers, `main`, through the
/// acts' watch of main; nothing writes `session` or `tree` yet, so a block of either stays unwritten, and a cell that
/// reads one starts from no version of it.
fn has(dir: &Path) -> Has {
    crate::has(dir)
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

pub fn run(data: &Path, dirs: &[PathBuf], with: With, port: u16) -> ExitCode {
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
    let Some((flows, with)) = read(data, dirs, &with) else {
        return ExitCode::FAILURE;
    };
    // Held until the process ends.
    let _held = match hold(data) {
        Ok(held) => held,
        Err(why) => {
            eprintln!("{why}");
            return ExitCode::FAILURE;
        }
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("the engine cannot start: {e}");
            return ExitCode::FAILURE;
        }
    };
    // The loop runs on this thread, so that what it does with the store holds up none of the threads that serve the
    // pages or carry out the rounds.
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
        // the declaration beside the flows being that project's; none where no project's does, as with a script.
        let project = match &with {
            Read::Script(_) => None,
            Read::Settings(settings, _) => holding(settings, shown),
        };
        // What the block of main says as a watch reads its head, in the words its definition gives: the same in every
        // flow, whose triggers are shared. Filled with the name of the project's main branch, which the settings give.
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
                match carry::carry(*settings, data, *beside) {
                    Ok((carriers, watches)) => (carriers, watches),
                    Err(why) => {
                        eprintln!("The carriers cannot be made: {why}");
                        return ExitCode::FAILURE;
                    }
                }
            }
        };
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
        let router = match copies {
            Some(keeping) => {
                // One task keeps the copies, a step at a time, with the time the loop reads from its clock.
                let handle = service.handle();
                let copies = keeping.about();
                keeping.run(handle.clone());
                web::with_works_and_copies(flows_shown, handle, levels, projects, copies)
            }
            None => web::with_works(flows_shown, service.handle(), levels, projects),
        };
        // The observations are read, and marked read, through connections of their own to the store's file.
        let router = router.merge(web::observations::router(service.handle(), data.join(store::FILE)));
        let router = match install {
            Some(install) => router.merge(web::settings::router(install)),
            None => router,
        };
        println!("The owner's pages are at {}", works_page(address));
        let held = web::works::Engine::of(service.driver()).paused.map(|paused| paused.by);
        if held == Some(web::works::Pauser::Engine) {
            println!(
                "Works were going on when the engine stopped: every work is held until the owner has looked and lets \
                 them go on with `wf-agents resume --all`."
            );
        }
        tokio::select! {
            fault = service.run(inbox) => {
                eprintln!("The engine stopped: {fault}. Started again, it goes on from the store.");
            }
            served = web::serve(listener, router) => {
                let why = served.err().map_or_else(|| "it ended".to_string(), |e| e.to_string());
                eprintln!("The server of the pages stopped: {why}. Started again, the engine goes on from the store.");
            }
        }
        ExitCode::FAILURE
    })
}

/// The settings as the page shows them: their file, read as the engine reads it, beside those the engine runs with.
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
        git: &carry::git::SAFE,
        program: web::settings::Program {
            variable: carry::account::VARIABLE,
            sign_in: carry::account::SIGN_IN,
            signed_in_as: carry::account::signed_in_as,
        },
    })
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

/// Holds the data directory for as long as the process runs: an exclusive lock of the system's on a file in it, taken
/// without waiting (`flock` where the system is Unix). The system lets go of it when the file is closed, which it is
/// when the process ends however it ends, killed or failed as much as stopped; a file made to say an engine runs, and
/// taken away when it stops, would outlive a process that was killed and keep the next one out for nothing. The file
/// is left in the directory: its lock, not the file, says that an engine runs.
fn hold(data: &Path) -> Result<File, String> {
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

/// The flows compiled and what carries the works out, checked against them; or none, having said everything wrong
/// with them. A script is checked only against flows that all compiled, so that it is not told of a flow missing for
/// that.
fn read(data: &Path, dirs: &[PathBuf], with: &With) -> Option<(Vec<Compiled>, Read)> {
    let mut wrong = false;
    if !data.is_dir() {
        eprintln!(
            "{}: no such directory: the data directory is made before the engine runs in it",
            data.display()
        );
        wrong = true;
    }
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
                // filled; and the project's files every system prompt holds.
                for fault in start_faults(&settings, &beside, &flows) {
                    eprintln!("{fault}");
                    wrong = true;
                }
                for warning in settings.warnings() {
                    eprintln!("{warning}");
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

/// What keeps the real carriers from carrying out the flows, every fault at once: what the acts need of a flow and of
/// their words beside the flows; and of each project, its declaration in its repository, what the texts sent to its
/// works' agents name and hold ([`carry::system::faults`]), and its files every system prompt holds, on its main where
/// works branch from ([`carry::system::project_files`]), each said with the project's key.
fn start_faults(settings: &carry::Settings, beside: &flow::agents::Agents, flows: &[Compiled]) -> Vec<String> {
    let mut faults: Vec<String> = (flows.iter())
        .flat_map(|flow| carry::faults(flow, &beside.acts.check.values))
        .collect();
    let words = carry::word_faults(&beside.acts.check).into_iter();
    faults.extend(words.map(|fault| format!("{}/{fault}", beside.root.display())));
    let declared = match carry::projects::declared(settings, beside) {
        Ok(declared) => declared,
        Err(why) => {
            faults.push(why);
            return faults;
        }
    };
    for (name, project) in &declared {
        let of = flows
            .iter()
            .flat_map(|flow| carry::system::faults(settings, &project.beside, flow));
        let files = carry::system::project_files(&project.settings, &project.beside);
        faults.extend(of.chain(files).map(|fault| format!("projects.{name}: {fault}")));
    }
    faults
}

/// What is written beside the flows, read from the directory that holds them, with the form of return of each flow
/// that compiled made of it; or why it cannot be: the flows are not all in one directory, what is beside them cannot be
/// read, or a flow's form cannot be made of it. Every flow is beside the one directory: what every flow shares is
/// written once, beside them all (principle one-thing-one-place).
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
        flow::form::form(compiled, &beside)?;
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

    /// The sample's settings, on a repository of their own whose main holds the files `project.toml` names.
    fn settings(dir: &Path) -> carry::Settings {
        let mut settings = carry::Settings::read(include_str!("../../carry/install.sample.toml")).expect("it reads");
        let repo = dir.join("repo");
        std::fs::create_dir_all(repo.join("design")).unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        for (key, value) in [
            ("user.name", "Tester"),
            ("user.email", "tester@example.com"),
            ("commit.gpgsign", "false"),
        ] {
            git(&repo, &["config", key, value]);
        }
        for path in ["AGENTS.md", "design/constitution.md", "design/README.md"] {
            std::fs::write(repo.join(path), "Some text.\n").unwrap();
        }
        // The project's declaration, the one beside the repository's flows.
        std::fs::create_dir_all(repo.join("flow")).unwrap();
        std::fs::copy(flows().join("project.toml"), repo.join(flow::agents::DECLARATION)).unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-q", "-m", "Files"]);
        settings.projects.get_mut("example").expect("the sample's project").repo = repo;
        settings
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
    // flow out with nothing wrong; what the texts sent to the agents lack and what main lacks of the project's files are
    // said together, with what the acts need: here a place no one fills in the heading of the project's files, for every
    // line, and the tree's rules not on main.
    #[test]
    fn the_start_says_every_fault_of_the_prompts_and_the_projects_files() {
        let dir = tempfile::tempdir().expect("a directory");
        let settings = settings(dir.path());
        let mut beside = beside(&[flows().join("engineer")], &[]).expect("the files beside the flows");
        let compiled = [engineer()];
        assert_eq!(start_faults(&settings, &beside, &compiled), Vec::<String>::new());
        beside.session.heading = "# `{path}` {where}\n\n{text}".to_string();
        let mut lines: Vec<&str> = compiled[0].cells.iter().filter_map(|c| c.line.as_deref()).collect();
        lines.sort();
        lines.dedup();
        let faults = start_faults(&settings, &beside, &compiled);
        assert_eq!(faults.len(), lines.len(), "{faults:?}");
        assert!(
            (faults.iter()).all(|f| f.ends_with("flow/agents/session.toml: the engine fills no place `{where}`")),
            "{faults:?}"
        );
        let repo = &settings.projects["example"].repo;
        git(repo, &["rm", "-q", "design/README.md"]);
        git(repo, &["commit", "-q", "-m", "No rules"]);
        let faults = start_faults(&settings, &beside, &compiled);
        assert_eq!(faults.len(), lines.len() + 1, "{faults:?}");
        assert_eq!(
            faults.last().map(String::as_str),
            Some(
                "projects.example: project.toml: the file tree_rules, design/README.md, is not on main, refs/heads/main, \
                 where works branch from"
            )
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
            install.running["sessions"]["context_limit"], 500000,
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
    // whole number for a decimal one, a table written in part; one the file changed since the engine started is.
    #[test]
    fn the_page_shows_beside_a_key_only_a_value_the_engine_reads_otherwise() {
        let sample = include_str!("../../carry/install.sample.toml")
            .replace("work_budget_usd = 5.0", "work_budget_usd = 5")
            .replace(
                "permissions = { heavy = [\"uv\"], locks = [\"uv lock\"], locks_except = [\"uv lock *--script*\"] }",
                "permissions = { heavy = [\"uv\"] }",
            );
        let settings = carry::Settings::read(&sample).expect("it reads");
        let dir = tempfile::tempdir().expect("a directory");
        let file = dir.path().join("install.toml");
        std::fs::write(&file, &sample).unwrap();
        let install = installed(file.clone(), &settings).expect("it is made");
        let differ = |page: &web::settings::SettingsPage| -> Vec<String> {
            (page.tables.iter())
                .flat_map(|t| {
                    (t.keys.iter())
                        .filter(|k| k.written.is_some() && k.running.is_some())
                        .map(move |k| format!("{}.{} runs with {}", t.name, k.key, k.running.as_deref().unwrap_or("")))
                })
                .collect()
        };
        let page = web::settings::page_of(&install).expect("it is shown");
        assert_eq!(page.unreadable, None);
        assert_eq!(differ(&page), Vec::<String>::new());
        std::fs::write(&file, sample.replace("slots = 2", "slots = 3")).unwrap();
        let page = web::settings::page_of(&install).expect("it is shown");
        assert_eq!(differ(&page), ["agents.slots runs with 2"]);
    }
}
