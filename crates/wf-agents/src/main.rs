//! The `wf-agents` command. Each part of the system adds its subcommands here.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// Runs coding agents through a defined flow.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// The data directory: the configuration, the rules and the database.
    #[arg(long, global = true, env = "WF_AGENTS_DATA")]
    data: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// The flow the engine runs.
    #[command(subcommand)]
    Flow(FlowCommand),
    /// The configuration in the data directory.
    #[command(subcommand)]
    Config(ConfigCommand),
}

#[derive(Subcommand)]
enum FlowCommand {
    /// Checks a flow against its format, the database and every state the engine can be in, and prints what it
    /// comes to and the digest of the compiled flow.
    Check {
        /// The flow to check.
        #[arg(default_value = "flow/flow.json")]
        flow: PathBuf,
        /// The database definition the flow's inputs are read from; by default, the one built into the command.
        #[arg(long)]
        schema: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum ConfigCommand {
    /// Checks the configuration as the data directory's files are now, committed or not, against the flow this
    /// command runs.
    Check,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        None => Ok(()),
        Some(Command::Flow(FlowCommand::Check { flow, schema })) => flow_check(&flow, schema.as_deref()),
        Some(Command::Config(ConfigCommand::Check)) => data(cli.data.as_deref()).and_then(config_check),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn read(p: &Path) -> Result<String, String> {
    std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))
}

fn flow_check(flow: &Path, schema: Option<&Path>) -> Result<(), String> {
    let text = read(flow)?;
    let schema = match schema {
        Some(p) => read(p)?,
        None => db::SCHEMA.to_string(),
    };
    let relations = db::relations(&schema).map_err(|e| format!("the database definition: {e}"))?;
    let compiled = flow::compile(&text, &flow::Context { relations }).map_err(|e| e.to_string())?;
    for s in &compiled.stats {
        println!(
            "{}: {} cells, {} rows, {} states, {} output places",
            s.graph, s.cells, s.rows, s.states, s.places
        );
    }
    println!("digest {}", compiled.digest());
    Ok(())
}

fn data(dir: Option<&Path>) -> Result<&Path, String> {
    dir.ok_or_else(|| "no data directory: pass --data or set WF_AGENTS_DATA".to_string())
}

/// The flow this command runs, compiled against the database it creates.
fn compiled() -> Result<flow::Compiled, String> {
    let relations = db::relations(db::SCHEMA).map_err(|e| format!("the database definition: {e}"))?;
    flow::compile(flow::FLOW, &flow::Context { relations }).map_err(|e| e.to_string())
}

fn config_check(dir: &Path) -> Result<(), String> {
    let c = config::working(dir).map_err(|e| e.to_string())?;
    let problems = c.check(&compiled()?);
    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    println!(
        "{}: {} projects ({}), {} rules",
        dir.display(),
        c.projects.len(),
        c.projects.keys().cloned().collect::<Vec<_>>().join(", "),
        c.rules.len()
    );
    Ok(())
}
