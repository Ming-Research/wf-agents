//! `wf-agents run`: the engine as a process. It drives every work the store in the data directory holds, on the flows
//! it is given, with agents, engine acts and tools that follow a script, and serves the owner's pages over it. It stops
//! when the engine's loop stops, saying why; started again, it goes on from the store.

use std::collections::BTreeSet;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use flow::{Compiled, Source};
use service::{Script, Scripted, Service, SystemClock};
use store::Store;

/// The store's file in the data directory.
const STORE: &str = "store.sqlite";

/// The sources outside a work that this process writes blocks of: the driver's, whether the items an item depends on
/// have merged. A flow that names another has works that would wait for it without end, and does not compile here.
fn sources(_: &Source) -> BTreeSet<String> {
    BTreeSet::from([driver::DEPENDENCIES.to_string()])
}

/// The address the works page is printed at.
pub fn works_page(address: SocketAddr) -> String {
    format!("http://{address}/#/works")
}

pub fn run(data: &Path, dirs: &[PathBuf], script: &Path, port: u16) -> ExitCode {
    // The flow page shows the first flow given.
    let Some(shown) = dirs.first() else {
        eprintln!("no flow is given: the engine runs on at least one");
        return ExitCode::FAILURE;
    };
    let Some((flows, script)) = read(data, dirs, script) else {
        return ExitCode::FAILURE;
    };
    let store = match Store::open(data.join(STORE)) {
        Ok(store) => store,
        Err(fault) => {
            let words = service::store_fault_words(&fault);
            eprintln!("{}: the store cannot be opened: {words}", data.join(STORE).display());
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
        let carriers = Scripted::new(script).carriers();
        let (service, inbox) = match Service::open(Box::new(store), flows, carriers, Arc::new(SystemClock)) {
            Ok(opened) => opened,
            Err(fault) => {
                eprintln!("The engine cannot start: {fault}.");
                return ExitCode::FAILURE;
            }
        };
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
        let router = web::with_works(
            web::Flows::dirs(shown.clone(), None, Box::new(sources)),
            service.handle(),
        );
        println!("The owner's pages are at {}", works_page(address));
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

/// The flows compiled and the script, checked against them; or none, having said everything wrong with them.
fn read(data: &Path, dirs: &[PathBuf], script: &Path) -> Option<(Vec<Compiled>, Script)> {
    let mut wrong = false;
    if !data.is_dir() {
        eprintln!(
            "{}: no such directory: the data directory is made before the engine runs in it",
            data.display()
        );
        wrong = true;
    }
    let mut flows = Vec::new();
    for dir in dirs {
        let source = match flow::read(dir) {
            Ok(source) => source,
            Err(e) => {
                eprintln!("{}: {e}", dir.display());
                wrong = true;
                continue;
            }
        };
        match flow::compile(&source, &sources(&source)) {
            Ok(compiled) => flows.push(compiled),
            Err(faults) => {
                for fault in &faults {
                    eprintln!("{}/{}", dir.display(), crate::line(fault));
                }
                wrong = true;
            }
        }
    }
    let script = match std::fs::read_to_string(script)
        .map_err(|e| e.to_string())
        .and_then(|t| Script::read(&t))
    {
        Ok(read) => {
            for fault in read.check(&flows) {
                eprintln!("{}: {fault}", script.display());
                wrong = true;
            }
            Some(read)
        }
        Err(e) => {
            eprintln!("{}: {e}", script.display());
            None
        }
    };
    match script {
        Some(script) if !wrong => Some((flows, script)),
        _ => None,
    }
}
