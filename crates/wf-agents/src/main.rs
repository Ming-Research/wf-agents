//! The `wf-agents` command. Each part of the system adds its subcommands here.

use std::collections::BTreeSet;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use flow::{Fault, FaultKind, Source};

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
}

// The engine has no sources outside a work yet, so each command here takes every source a definition's blocks name in
// `flow.toml` as one the engine has, and says so in its help. When the engine has its sources, the commands take those
// instead.
#[derive(Subcommand)]
enum FlowCommand {
    /// Compiles the definition in a directory: prints each fault with its file and field and fails, or prints a line
    /// saying what it compiled to. The engine has no sources outside a work yet, so every source the definition names
    /// is taken as one the engine has.
    Check {
        /// The definition's directory.
        dir: PathBuf,
    },
    /// Serves the owner's flow page for the definition in a directory, on the loopback address: the flow as a chart,
    /// or its faults when it does not compile, and what changed from another definition when one is given. Both
    /// directories are read again for every request, so an edit shows when the page is loaded again. The engine has no
    /// sources outside a work yet, so every source each definition names is taken as one the engine has.
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
    }
}

/// The sources outside a work that a definition's blocks name, standing for the sources the engine has until it has
/// its own. A `flow.toml` that cannot be read names none; compiling it then says why it cannot be read.
fn named_sources(source: &Source) -> BTreeSet<String> {
    let Ok(flow) = source.flow.parse::<toml::Table>() else {
        return BTreeSet::new();
    };
    let Some(toml::Value::Table(blocks)) = flow.get("blocks") else {
        return BTreeSet::new();
    };
    (blocks.values())
        .filter_map(|block| block.get("outside")?.as_str().map(str::to_string))
        .collect()
}

fn check(dir: &Path) -> ExitCode {
    let source = match flow::read(dir) {
        Ok(source) => source,
        Err(e) => {
            eprintln!("{}: {e}", dir.display());
            return ExitCode::FAILURE;
        }
    };
    match flow::compile(&source, &named_sources(&source)) {
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
        F::ConditionOnOwnOutput { block } => {
            format!("the condition reads {block}, which the cell itself writes: the cell could never come to run")
        }
        F::ConditionNotWrittenBefore { block } => {
            format!("the condition reads {block}, which neither a cell before this one nor people write")
        }
        F::LastHasCondition => {
            "the last cell has a condition, but a work ends when its last cell has a record, so it always runs"
                .to_string()
        }
        F::UndeclaredTest { test } => format!("the test {test} is not declared in flow.toml"),
        F::ReadsEmpty => "the test reads no block".to_string(),
        F::UnusedTest { test } => format!("no cell uses the test {test}"),
        F::TestOnOwnOutput { test, block } => format!(
            "the test {test} reads {block}, which the cell itself writes: the cell could never come to run, or never \
             stop waiting"
        ),
        F::TestNotWrittenBefore { test, block } => {
            format!("the test {test} reads {block}, which neither a cell before this one, people nor the outside write")
        }
        F::Flow(fault) => format!("the deciding code refuses the cells as a flow: {fault:?}"),
    }
}

fn page(dir: PathBuf, against: Option<PathBuf>, port: u16) -> ExitCode {
    let shown = dir.display().to_string();
    let compared = against.as_ref().map(|a| a.display().to_string());
    let flows = web::Flows::dirs(dir, against, Box::new(named_sources));
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
        println!("The flow page of {shown}{against} is at http://{address}/");
        match web::serve(listener, flows).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("the server stopped: {e}");
                ExitCode::FAILURE
            }
        }
    })
}
