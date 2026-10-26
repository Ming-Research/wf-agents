//! The `wf-agents` command. Each part of the system adds its subcommands here.

use clap::Parser;

/// Runs coding agents through a defined flow.
#[derive(Parser)]
#[command(version)]
struct Cli {}

fn main() {
    Cli::parse();
}
