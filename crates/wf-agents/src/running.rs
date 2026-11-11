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
use service::{Carriers, Levels, Script, Scripted, Service, SystemClock};
use store::Store;

/// The store's file in the data directory.
const STORE: &str = "store.sqlite";

/// The file in the data directory whose lock says that an engine runs on it.
const HELD: &str = "engine.lock";

/// What a flow run here may name: what every definition is compiled against. Of the sources, this process writes the
/// driver's, whether the items an item depends on have merged, and, with the real carriers, `main`, through the
/// acts' watch of main; nothing writes `session` or `tree` yet, so a block of either stays unwritten, and a cell that
/// reads one starts from no version of it.
fn has(source: &Source) -> Has {
    crate::has(source)
}

/// What carries the works out.
pub enum With {
    /// Agents, engine acts and tools that follow the script in this file.
    Script(PathBuf),
    /// The real carriers, as the settings in this file say.
    Settings(PathBuf),
}

/// What carries the works out, read and checked: a script, or the settings.
enum Read {
    Script(Script),
    Settings(Box<carry::Settings>),
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
        let store = match Store::open(data.join(STORE)) {
            Ok(store) => store,
            Err(fault) => {
                let words = service::store_fault_words(&fault);
                eprintln!("{}: the store cannot be opened: {words}", data.join(STORE).display());
                return ExitCode::FAILURE;
            }
        };
        let levels = match &with {
            Read::Script(_) => script_levels(),
            Read::Settings(settings) => match settings.levels() {
                Ok(levels) => levels,
                Err(why) => {
                    eprintln!("{why}");
                    return ExitCode::FAILURE;
                }
            },
        };
        let (carriers, watch): (Carriers, Option<carry::Watch>) = match with {
            Read::Script(script) => (Scripted::new(script).carriers(), None),
            Read::Settings(settings) => match carry::carry(*settings, data) {
                Ok((carriers, watch)) => (carriers, Some(watch)),
                Err(why) => {
                    eprintln!("The carriers cannot be made: {why}");
                    return ExitCode::FAILURE;
                }
            },
        };
        let (service, inbox) = match Service::open(Box::new(store), flows, carriers, Arc::new(SystemClock)) {
            Ok(opened) => opened,
            Err(fault) => {
                eprintln!("The engine cannot start: {fault}.");
                return ExitCode::FAILURE;
            }
        };
        // The watch of main writes, through the loop, the block of main of each work: the check before a merge then
        // runs again when main has moved since it passed.
        if let Some(watch) = watch {
            let handle = service.handle();
            tokio::spawn(watch.run(move |head| handle.outside(carry::MAIN, &head)));
        }
        let flows = web::Flows::dirs(shown.clone(), None, Box::new(has));
        let router = web::with_works(flows, service.handle(), levels);
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
        match flow::compile(&source, &has(&source)) {
            Ok(compiled) => flows.push(compiled),
            Err(faults) => {
                for fault in &faults {
                    eprintln!("{}/{}", dir.display(), crate::line(fault));
                }
                compiled_all = false;
            }
        }
    }
    let read = match with {
        With::Script(script) => read_script(script, &flows, compiled_all, &mut wrong).map(Read::Script),
        With::Settings(settings) => match carry::Settings::load(settings) {
            Ok(settings) => {
                // What the real acts need of a flow, checked once the flows compiled.
                for fault in flows.iter().flat_map(carry::faults) {
                    eprintln!("{fault}");
                    wrong = true;
                }
                for warning in settings.warnings() {
                    eprintln!("{warning}");
                }
                Some(Read::Settings(Box::new(settings)))
            }
            Err(why) => {
                eprintln!("{why}");
                None
            }
        },
    };
    match read {
        Some(read) if !wrong && compiled_all => Some((flows, read)),
        _ => None,
    }
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
