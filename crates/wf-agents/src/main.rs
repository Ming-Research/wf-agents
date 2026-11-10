//! The `wf-agents` command. Each part of the system adds its subcommands here.
//!
//! - `running`: `wf-agents run`, the engine as a process.
//! - `talk`: the commands that talk to a running engine.

mod running;
mod talk;

use std::collections::BTreeSet;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use flow::{Fault, FaultKind, Source};
use web::works::Act;

use crate::talk::Asked;

/// The port the engine serves the owner's pages on, on the loopback address, unless another is given.
const PORT: u16 = 7470;

/// Runs coding agents through a defined flow.
#[derive(Parser)]
#[command(version)]
struct Cli {
    // Required: with none, clap fails with the usage and the names of the subcommands.
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// A flow's definition: a directory holding `flow.toml` and one file for each cell.
    #[command(subcommand)]
    Flow(FlowCommand),
    /// Runs the engine: drives every work of the store in the data directory on the flows given, with agents, engine
    /// acts and tools that follow a script, and serves the owner's pages on the loopback address. It stops when the
    /// engine stops, saying why; started again, it goes on from the store.
    Run {
        /// The data directory, which holds the store.
        #[arg(long)]
        data: PathBuf,
        /// The directory of a flow's definition; given once for each flow.
        #[arg(long = "flow", required = true)]
        flows: Vec<PathBuf>,
        /// The script the agents, the engine's acts and the tools follow.
        #[arg(long)]
        script: PathBuf,
        /// The port of the pages; 0 for any free one.
        #[arg(long, default_value_t = PORT)]
        port: u16,
    },
    /// Adds an item with its work, which is added paused.
    Add {
        #[arg(long)]
        title: String,
        /// The name of the flow its work runs on.
        #[arg(long)]
        flow: String,
        /// An item it depends on, by number; given once for each.
        #[arg(long = "depends-on", value_name = "ITEM")]
        depends_on: Vec<u64>,
        /// A block written for it, with this text.
        #[arg(long = "write", value_name = "BLOCK=TEXT")]
        texts: Vec<String>,
        /// A block written for it, with the content of this file.
        #[arg(long = "write-file", value_name = "BLOCK=FILE")]
        files: Vec<String>,
        /// A block that is a choice, written for it with this value.
        #[arg(long = "choose", value_name = "BLOCK=VALUE")]
        choices: Vec<String>,
        #[command(flatten)]
        at: At,
    },
    /// Lists the items, each with what its work does now.
    List {
        #[command(flatten)]
        at: At,
    },
    /// Shows a work: what it does now, its cells, its blocks and its runs.
    Show {
        work: u64,
        #[command(flatten)]
        at: At,
    },
    /// Lets a work on: its rounds begin again.
    Resume {
        work: u64,
        #[command(flatten)]
        at: At,
    },
    /// Pauses a work: no round of it begins, and the one under way ends.
    Pause {
        work: u64,
        #[command(flatten)]
        at: At,
    },
    /// Cancels a work's item: nothing of the work goes on.
    Cancel {
        work: u64,
        #[command(flatten)]
        at: At,
    },
    /// Writes a block of a work that people write: with a text, the content of a file, or nothing but a value.
    Write {
        work: u64,
        block: String,
        #[arg(long, conflicts_with = "file")]
        text: Option<String>,
        #[arg(long)]
        file: Option<PathBuf>,
        /// The block's value, where it is a choice.
        #[arg(long)]
        value: Option<String>,
        #[command(flatten)]
        at: At,
    },
    /// Gives a call that waits for a person its result.
    Answer {
        work: u64,
        call: u64,
        result: String,
        #[command(flatten)]
        at: At,
    },
    /// Lets a cell that reached its limit go on.
    LetGo {
        work: u64,
        cell: String,
        #[command(flatten)]
        at: At,
    },
}

/// Where the running engine is.
#[derive(Args)]
struct At {
    /// The port the engine serves on, on the loopback address.
    #[arg(long, default_value_t = PORT)]
    port: u16,
}

#[derive(Subcommand)]
enum FlowCommand {
    /// Compiles the definition in a directory: prints each fault with its file and field and fails, or prints a line
    /// saying what it compiled to.
    Check {
        /// The definition's directory.
        dir: PathBuf,
    },
    /// Serves the owner's flow page for the definition in a directory, on the loopback address: the flow as a chart,
    /// or its faults when it does not compile, and what changed from another definition when one is given. Both
    /// directories are read again for every request, so an edit shows when the page is loaded again.
    Page {
        /// The definition's directory.
        dir: PathBuf,
        /// The directory of a definition to compare against: the page shows what changed from it to the one shown.
        #[arg(long)]
        against: Option<PathBuf>,
        /// The port of the page on the loopback address.
        #[arg(long, default_value_t = 7490)]
        port: u16,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Flow(FlowCommand::Check { dir }) => check(&dir),
        Command::Flow(FlowCommand::Page { dir, against, port }) => page(dir, against, port),
        Command::Run {
            data,
            flows,
            script,
            port,
        } => running::run(&data, &flows, &script, port),
        Command::Add {
            title,
            flow,
            depends_on,
            texts,
            files,
            choices,
            at,
        } => match talk::new_item(title, flow, depends_on, &texts, &files, &choices) {
            Ok(item) => talk::ask(at.port, Asked::Add(item)),
            Err(why) => {
                eprintln!("{why}");
                ExitCode::FAILURE
            }
        },
        Command::List { at } => talk::ask(at.port, Asked::List),
        Command::Show { work, at } => talk::ask(at.port, Asked::Show(work)),
        Command::Resume { work, at } => talk::ask(at.port, Asked::Act(work, Act::Resume {})),
        Command::Pause { work, at } => talk::ask(at.port, Asked::Act(work, Act::Pause {})),
        Command::Cancel { work, at } => talk::ask(at.port, Asked::Cancel(work)),
        Command::Write {
            work,
            block,
            text,
            file,
            value,
            at,
        } => {
            let content = match (text, file) {
                (Some(text), _) => text,
                (None, Some(file)) => match std::fs::read_to_string(&file) {
                    Ok(content) => content,
                    Err(e) => {
                        eprintln!("{}: {e}", file.display());
                        return ExitCode::FAILURE;
                    }
                },
                (None, None) if value.is_some() => String::new(),
                (None, None) => {
                    eprintln!("Nothing is given to write: write takes --text, --file or --value.");
                    return ExitCode::FAILURE;
                }
            };
            talk::ask(at.port, Asked::Act(work, Act::Write { block, content, value }))
        }
        Command::Answer { work, call, result, at } => {
            talk::ask(at.port, Asked::Act(work, Act::Answer { call, result }))
        }
        Command::LetGo { work, cell, at } => talk::ask(at.port, Asked::Act(work, Act::LetGo { cell })),
    }
}

/// The names of the sources outside a work that write its blocks, which every definition is compiled against:
/// `dependencies`, written by the engine's driver; and `main`, `session` and `tree`, written by the host's adapters,
/// which are not built yet.
const SOURCES: [&str; 4] = ["dependencies", "main", "session", "tree"];

/// The sources outside a work, by name, for any definition.
fn sources(_: &Source) -> BTreeSet<String> {
    SOURCES.into_iter().map(str::to_string).collect()
}

fn check(dir: &Path) -> ExitCode {
    let source = match flow::read(dir) {
        Ok(source) => source,
        Err(e) => {
            eprintln!("{}: {e}", dir.display());
            return ExitCode::FAILURE;
        }
    };
    match flow::compile(&source, &sources(&source)) {
        Ok(compiled) => {
            let count = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
            println!(
                "{} compiles: {}, {}, {}.",
                compiled.name,
                count(compiled.cells.len(), "cell", "cells"),
                count(compiled.blocks.len(), "block", "blocks"),
                count(compiled.tests.len(), "named test", "named tests"),
            );
            ExitCode::SUCCESS
        }
        Err(faults) => {
            for fault in &faults {
                eprintln!("{}", line(fault));
            }
            ExitCode::FAILURE
        }
    }
}

/// A fault as one line: its file, its field, and what is wrong there.
fn line(fault: &Fault) -> String {
    let at = if fault.field.is_empty() {
        fault.file.clone()
    } else {
        format!("{} {}", fault.file, fault.field)
    };
    format!("{at}: {}", words(&fault.what))
}

/// What is wrong, in words.
fn words(what: &FaultKind) -> String {
    use FaultKind as F;
    match what {
        F::Unreadable { message } => format!("cannot be read: {}", message.trim_end()),
        F::OrderEmpty => "the order names no cell".to_string(),
        F::NoCellFile { cell } => format!("the order names {cell}, which has no file"),
        F::NotInOrder { cell } => format!("the cell {cell} has a file but is not named in the order"),
        F::Twice { name } => format!("{name} is named a second time in the same list"),
        F::LimitOutOfRange { value } => {
            format!("the limit {value} is not a whole number from 1 to the largest count the engine keeps")
        }
        F::NoLine => "an agent's cell names no line to run in".to_string(),
        F::EngineHasLine { line } => format!("an engine's cell runs in no session, but names the line {line}"),
        F::NoSuchLine { line } => format!("the line {line} is not declared in flow.toml"),
        F::ValuesEmpty => "a choice has no values to choose from".to_string(),
        F::NoSuchBlock { block } => format!("the block {block} is not declared in flow.toml"),
        F::NoWriter { block } => format!("nothing writes {block}: no cell of the order, no person and nothing outside"),
        F::NoReader { block } => {
            format!("nothing reads {block}: no cell, no condition, no test, and the page does not show it")
        }
        F::UnknownSource { block, source } => {
            format!("{block} is written by the source {source}, which the engine does not have")
        }
        F::NotAChoice { block } => format!("a condition or a test reads {block}, which is not a choice"),
        F::NotAValue { block, is } => {
            format!("a condition or a test waits for {block} to be {is}, which is not one of its values")
        }
        F::ConditionOnOwnOutput { block } => format!(
            "the condition reads {block}, which the cell itself writes: the cell would decide by what it writes itself \
             whether it runs"
        ),
        F::ConditionNotWrittenBefore { block } => {
            format!("the condition reads {block}, which neither a cell before this one, people nor the outside write")
        }
        F::LastHasCondition => {
            "the last cell has a condition, but a work ends when its last cell has a record, so it always runs"
                .to_string()
        }
        F::UndeclaredTest { test } => format!("the test {test} is not declared in flow.toml"),
        F::ReadsEmpty { test } => format!("the test {test} reads no block"),
        F::UnusedTest { test } => format!("no cell uses the test {test}"),
        F::TestOnOwnOutput { test, block } => format!(
            "the test {test} reads {block}, which the cell itself writes: the cell would decide by what it writes itself \
             whether it runs, or whether the work waits before it"
        ),
        F::TestNotWrittenBefore { test, block } => {
            format!("the test {test} reads {block}, which neither a cell before this one, people nor the outside write")
        }
        F::Flow(fault) => format!("the deciding code refuses the cells as a flow: {fault:?}"),
    }
}

/// The address the flow page is printed at: the page shows the view its address's hash names, and this server serves
/// the flow alone.
fn flow_page(address: SocketAddr) -> String {
    format!("http://{address}/#/flow")
}

#[test]
fn the_flow_page_is_printed_at_the_flow_view() {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, 7490));
    assert_eq!(flow_page(address), "http://127.0.0.1:7490/#/flow");
}

fn page(dir: PathBuf, against: Option<PathBuf>, port: u16) -> ExitCode {
    let shown = dir.display().to_string();
    let compared = against.as_ref().map(|a| a.display().to_string());
    let flows = web::Flows::dirs(dir, against, Box::new(sources));
    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("the server cannot start: {e}");
            return ExitCode::FAILURE;
        }
    };
    runtime.block_on(async move {
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        let listener = match tokio::net::TcpListener::bind(address).await {
            Ok(listener) => listener,
            Err(e) => {
                eprintln!("{address}: {e}");
                return ExitCode::FAILURE;
            }
        };
        let against = compared.map(|c| format!(", compared against {c}")).unwrap_or_default();
        println!("The flow page of {shown}{against} is at {}", flow_page(address));
        match web::serve(listener, web::router(flows)).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("the server stopped: {e}");
                ExitCode::FAILURE
            }
        }
    })
}
