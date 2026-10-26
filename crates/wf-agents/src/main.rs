//! The `wf-agents` command. Each part of the system adds its subcommands here.

mod owner;
mod page;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

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
    /// Runs the engine: starts works on the queue's items and takes each through the flow, until a file named stop
    /// appears in the data directory.
    Run {
        /// Takes one pass only: carries out the outbox, starts what can start, and takes each work one step.
        #[arg(long)]
        once: bool,
    },
    /// Items: what to do.
    #[command(subcommand)]
    Item(ItemCommand),
    /// The dispatch queue: which items start next.
    #[command(subcommand)]
    Queue(QueueCommand),
    /// The cards waiting for the owner: questions, and design cards to rule on.
    Cards {
        /// Every card, ruled and replaced ones too.
        #[arg(long)]
        all: bool,
    },
    /// One card in full, with its rulings.
    Card { id: i64 },
    /// Answers a question card, or rules on a design card.
    Answer {
        card: i64,
        /// The answer, or what the ruling says.
        #[arg(long)]
        text: String,
        /// For a question the architect is asked: why it was decided this way.
        #[arg(long)]
        why: Option<String>,
        /// For a design card: confirm or disagree.
        #[arg(long)]
        verdict: Option<String>,
        /// For a design card: the approach chosen instead.
        #[arg(long)]
        option: Option<String>,
        /// For a question the architect is asked: the rest of the reply as JSON, such as children, rest and waits.
        #[arg(long)]
        reply: Option<String>,
    },
    /// Says something to a running work: the line it is for gets it in its next round.
    Say {
        work: i64,
        text: String,
        #[arg(long, default_value = "engineer")]
        line: String,
    },
    /// Pauses a work.
    Pause {
        work: i64,
        #[arg(long)]
        why: String,
    },
    /// Lets a paused work go on.
    Resume { work: i64 },
    /// Where each work stands.
    Status,
    /// A work's rounds.
    History {
        work: i64,
        /// Every round in full: what it was given and what it returned.
        #[arg(long)]
        full: bool,
    },
    /// How many times a work took the rows that count each counter.
    Counters { work: i64 },
    /// Returns sent back, compactions, counters past their limits, and works paused.
    Anomalies,
    /// The time and cost of each work, or of one work's rounds by cell.
    Cost { work: Option<i64> },
}

#[derive(Subcommand)]
enum ItemCommand {
    /// Creates an item.
    Add {
        #[arg(long)]
        project: String,
        #[arg(long)]
        title: String,
        /// What cannot be done or is wrong.
        #[arg(long)]
        problem: String,
        /// Why it is worth doing.
        #[arg(long)]
        why: String,
        /// How to do it, when it is already known.
        #[arg(long)]
        approach: Option<String>,
        /// change or report.
        #[arg(long, default_value = "change")]
        deliver: String,
        /// The agent level, as the configuration names it.
        #[arg(long)]
        level: String,
        /// An item it depends on and what it needs of it, as N:what.
        #[arg(long = "depends-on")]
        depends_on: Vec<String>,
        /// Puts it last in the dispatch queue.
        #[arg(long)]
        queue: bool,
    },
    /// Every item.
    List,
    /// Makes an item depend on another.
    Depend {
        item: i64,
        on: i64,
        #[arg(long)]
        needs: String,
    },
    /// Cancels an item, and ends its work.
    Cancel {
        item: i64,
        #[arg(long)]
        why: String,
    },
}

#[derive(Subcommand)]
enum QueueCommand {
    /// The items waiting to start, in order.
    List,
    /// Puts an item in the queue: last, or first, or before or after another.
    Put {
        item: i64,
        #[arg(long)]
        first: bool,
        #[arg(long)]
        before: Option<i64>,
        #[arg(long)]
        after: Option<i64>,
    },
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
    /// Writes the page that shows a flow as a state machine: each cell's inputs, returns, rows and checks, drawn as
    /// a diagram; with --against, it also lists what changed from that flow.
    Page {
        /// The flow to show.
        #[arg(default_value = "flow/flow.json")]
        flow: PathBuf,
        /// A flow to compare it with, such as the one last approved.
        #[arg(long)]
        against: Option<PathBuf>,
        /// The database definition the flows' inputs are read from; by default, the one built into the command.
        #[arg(long)]
        schema: Option<PathBuf>,
        /// Where to write the page.
        #[arg(long, short)]
        out: PathBuf,
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
        Some(Command::Flow(FlowCommand::Page {
            flow,
            against,
            schema,
            out,
        })) => flow_page(&flow, against.as_deref(), schema.as_deref(), &out),
        Some(Command::Config(ConfigCommand::Check)) => data(cli.data.as_deref()).and_then(config_check),
        Some(c) => data(cli.data.as_deref()).and_then(|d| owners(d, c)),
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

/// A flow file compiled against a database definition, or the one built into the command.
fn compile_file(flow: &Path, schema: Option<&Path>) -> Result<flow::Compiled, String> {
    let text = read(flow)?;
    let schema = match schema {
        Some(p) => read(p)?,
        None => db::SCHEMA.to_string(),
    };
    let relations = db::relations(&schema).map_err(|e| format!("the database definition: {e}"))?;
    flow::compile(&text, &flow::Context { relations }).map_err(|e| format!("{}: {e}", flow.display()))
}

fn flow_page(flow: &Path, against: Option<&Path>, schema: Option<&Path>, out: &Path) -> Result<(), String> {
    let compiled = compile_file(flow, schema)?;
    let base = against.map(|p| compile_file(p, schema)).transpose()?;
    let html = page::page(&compiled, base.as_ref())?;
    std::fs::write(out, html).map_err(|e| format!("{}: {e}", out.display()))?;
    println!("wrote {}", out.display());
    Ok(())
}

fn flow_check(flow: &Path, schema: Option<&Path>) -> Result<(), String> {
    let compiled = compile_file(flow, schema)?;
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

fn print(r: Result<String, String>) -> Result<(), String> {
    r.map(|s| print!("{s}{}", if s.ends_with('\n') { "" } else { "\n" }))
}

/// The commands that work on a data directory's database.
fn owners(d: &Path, c: Command) -> Result<(), String> {
    match c {
        Command::Flow(_) | Command::Config(_) => Ok(()),
        Command::Run { once } => run(d, once),
        Command::Item(ItemCommand::Add {
            project,
            title,
            problem,
            why,
            approach,
            deliver,
            level,
            depends_on,
            queue,
        }) => {
            let mut deps = Vec::new();
            for x in depends_on {
                let (n, needs) = x
                    .split_once(':')
                    .ok_or_else(|| format!("--depends-on is N:what it needs: {x}"))?;
                deps.push((
                    n.trim().parse::<i64>().map_err(|e| format!("{n}: {e}"))?,
                    needs.trim().to_string(),
                ));
            }
            let id = owner::add_item(
                d,
                owner::NewItem {
                    project,
                    title,
                    problem,
                    why,
                    approach,
                    deliver,
                    level,
                    depends_on: deps,
                    queue,
                },
            )?;
            println!("item {id}");
            Ok(())
        }
        Command::Item(ItemCommand::List) => print(owner::items(d)),
        Command::Item(ItemCommand::Depend { item, on, needs }) => owner::depend(d, item, on, &needs),
        Command::Item(ItemCommand::Cancel { item, why }) => owner::cancel(d, item, &why),
        Command::Queue(QueueCommand::List) => print(owner::queue_list(d)),
        Command::Queue(QueueCommand::Put {
            item,
            first,
            before,
            after,
        }) => {
            let place = match (first, before, after) {
                (true, None, None) => owner::Place::First,
                (false, Some(b), None) => owner::Place::Before(b),
                (false, None, Some(a)) => owner::Place::After(a),
                (false, None, None) => owner::Place::Last,
                _ => return Err("give at most one of --first, --before and --after".into()),
            };
            owner::queue(d, item, place)
        }
        Command::Cards { all } => print(owner::cards(d, all)),
        Command::Card { id } => print(owner::card(d, id)),
        Command::Answer {
            card,
            text,
            why,
            verdict,
            option,
            reply,
        } => {
            let reply = match reply {
                Some(r) => Some(serde_json::from_str(&r).map_err(|e| format!("--reply: {e}"))?),
                None => None,
            };
            print(owner::answer(
                d,
                owner::Answer {
                    card,
                    text,
                    why,
                    verdict,
                    option,
                    reply,
                },
            ))
        }
        Command::Say { work, text, line } => owner::say(d, work, &line, &text).map(|id| println!("interjection {id}")),
        Command::Pause { work, why } => owner::pause(d, work, Some(&why)),
        Command::Resume { work } => owner::pause(d, work, None),
        Command::Status => print(owner::status(d)),
        Command::History { work, full } => print(owner::history(d, work, full)),
        Command::Counters { work } => print(owner::counters(d, work)),
        Command::Anomalies => print(owner::anomalies(d)),
        Command::Cost { work } => print(owner::cost(d, work)),
    }
}

/// Runs the engine on the data directory with `claude -p` and the gh CLI.
fn run(d: &Path, once: bool) -> Result<(), String> {
    let committed = config::committed(d).map_err(|e| e.to_string())?;
    let github = engine::hub::Gh::new(d.join("logs"));
    let mut e = engine::Engine::new(d, committed, Box::new(engine::agent::Claude), Box::new(github))?;
    e.say = Box::new(|s| eprintln!("{} {s}", db::now() / 1000));
    if once {
        e.tick()?;
        return Ok(());
    }
    // The owner, or whoever installs a new command, stops the engine between steps by creating this file.
    let stop = d.join("stop");
    let _ = std::fs::remove_file(&stop);
    e.run(&|| stop.exists(), Duration::from_secs(10))?;
    let _ = std::fs::remove_file(&stop);
    eprintln!("{} stopped: {} exists", db::now() / 1000, stop.display());
    Ok(())
}
