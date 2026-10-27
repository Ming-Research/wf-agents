//! The `wf-agents` command. Each part of the system adds its subcommands here.

mod owner;
mod tree;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use clap::{Parser, Subcommand, ValueEnum};

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
    /// Checks the design tree of the repository here: its nodes, depth and decisions at HEAD, and how many more than
    /// at the fork point given. A project names it as its design_check.
    TreeCheck {
        /// The directory the design tree is in.
        #[arg(long, default_value = "design")]
        root: String,
        /// The fork point from main.
        fork: String,
    },
    /// Runs the engine: starts works on the queue's items and takes each through the flow, until a file named stop
    /// appears in the data directory. The owner's page is served beside it, on the loopback address.
    Run {
        /// Takes one pass only, without the page: carries out the outbox, starts what can start, and takes each work
        /// one step.
        #[arg(long)]
        once: bool,
        /// How the engine starts dispatching; the owner can switch it on the page while it runs.
        #[arg(long, value_enum, default_value_t = RunMode::Run)]
        mode: RunMode,
        /// The port of the owner's page on the loopback address.
        #[arg(long, default_value_t = 7480)]
        port: u16,
    },
    /// Items: what to do.
    #[command(subcommand)]
    Item(ItemCommand),
    /// The dispatch queue: which items start next.
    #[command(subcommand)]
    Queue(QueueCommand),
    /// The asks of the owner waiting for an answer.
    Asks {
        /// Every ask of the owner, those answered too.
        #[arg(long)]
        all: bool,
    },
    /// One ask of the owner in full, with its answer.
    Ask {
        /// The ask's number, as `wf-agents asks` lists it.
        id: i64,
    },
    /// Answers an ask of the owner.
    Answer {
        /// The ask to answer, by its number.
        ask: i64,
        /// The answer.
        #[arg(long)]
        text: String,
        /// For what the architect is asked: why it was decided this way.
        #[arg(long)]
        why: Option<String>,
        /// For what the architect is asked: the rest of the reply as JSON, such as children, rest and waits.
        #[arg(long)]
        reply: Option<String>,
    },
    /// The design cards waiting for a ruling, and those ruled but not yet confirmed for the merge.
    DesignCards {
        /// Every design card, replaced ones too.
        #[arg(long)]
        all: bool,
    },
    /// One design card in full, with its rulings.
    DesignCard {
        /// The design card's number, as `wf-agents design-cards` lists it.
        id: i64,
    },
    /// Rules on a design card.
    Rule {
        /// The design card to rule on, by its number.
        design_card: i64,
        /// Confirm or disagree.
        #[arg(long)]
        verdict: String,
        /// What the ruling says.
        #[arg(long)]
        text: String,
        /// The approach chosen instead of the one recommended.
        #[arg(long)]
        option: Option<String>,
    },
    /// Says something to a running work: the line it is for gets it in its next round.
    Say {
        /// The work to say it to.
        work: i64,
        /// What to say.
        text: String,
        /// The line of the work it is for, as the flow names its lines.
        #[arg(long, default_value = "engineer")]
        line: String,
    },
    /// Pauses a work.
    Pause {
        /// The work to pause.
        work: i64,
        /// Why it is paused.
        #[arg(long)]
        why: String,
    },
    /// Lets a paused work go on.
    Resume {
        /// The work to let go on.
        work: i64,
    },
    /// Where each work stands.
    Status,
    /// A work's rounds.
    History {
        /// The work whose rounds to list.
        work: i64,
        /// Every round in full: what it was given and what it returned.
        #[arg(long)]
        full: bool,
    },
    /// The message the work's next agent round would be sent, built as the engine builds it, with nothing run or
    /// recorded.
    Message {
        /// The work whose next round to show.
        work: i64,
        /// As JSON: the message, and each conditional input it gives or refers to, as a round records them.
        #[arg(long)]
        json: bool,
    },
    /// How many times a work took the rows that count each counter.
    Counters {
        /// The work to count for.
        work: i64,
    },
    /// Returns sent back, compactions, counters past their limits, and works paused.
    Anomalies,
    /// The time and cost of each work, or of one work's rounds by cell.
    Cost {
        /// The work whose rounds to show by cell; without it, every work.
        work: Option<i64>,
    },
}

/// How the engine dispatches.
#[derive(Clone, Copy, ValueEnum)]
enum RunMode {
    /// Takes every work as far as it can, pass after pass.
    Run,
    /// Starts nothing by itself: no round, no tool, no outbox action, except the next round of a work the owner
    /// asks for on the work's page; the page stays up.
    Hold,
}

impl From<RunMode> for api::Mode {
    fn from(m: RunMode) -> api::Mode {
        match m {
            RunMode::Run => api::Mode::Run,
            RunMode::Hold => api::Mode::Hold,
        }
    }
}

/// What a new item is.
#[derive(clap::Args)]
struct AddItem {
    /// The project the item belongs to, as the configuration names it.
    #[arg(long)]
    project: String,
    /// What to do, as an action, within the length the configuration allows.
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
    /// For an item imported from outside this flow: where it came from, such as an item of another board.
    #[arg(long = "imported-from")]
    imported_from: Option<String>,
    /// For an item imported from outside this flow: the branch work on it had, which its first work takes over.
    #[arg(long)]
    branch: Option<String>,
    /// For an item imported from outside this flow: the pull request of that branch, which its first work keeps.
    #[arg(long, requires = "branch")]
    pr: Option<i64>,
}

#[derive(Subcommand)]
enum ItemCommand {
    /// Creates an item.
    Add(Box<AddItem>),
    /// Every item.
    List,
    /// Makes an item depend on another.
    Depend {
        /// The item that depends on the other.
        item: i64,
        /// The item it depends on.
        on: i64,
        /// What it needs of that item, in one sentence.
        #[arg(long)]
        needs: String,
    },
    /// Cancels an item, and ends its work.
    Cancel {
        /// The item to cancel.
        item: i64,
        /// Why it is cancelled.
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
        /// The item to put in the queue.
        item: i64,
        /// Puts it first.
        #[arg(long)]
        first: bool,
        /// Puts it before this item.
        #[arg(long)]
        before: Option<i64>,
        /// Puts it after this item.
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
        Some(Command::TreeCheck { root, fork }) => tree::report(Path::new("."), &fork, &root).map(|r| print!("{r}")),
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

fn flow_check(flow: &Path, schema: Option<&Path>) -> Result<(), String> {
    let text = read(flow)?;
    let schema = match schema {
        Some(p) => read(p)?,
        None => db::SCHEMA.to_string(),
    };
    let relations = db::relations(&schema).map_err(|e| format!("the database definition: {e}"))?;
    let tables = db::index_of(&schema).map_err(|e| format!("the database definition: {e}"))?;
    let compiled = flow::compile(&text, &flow::Context { relations, tables }).map_err(|e| e.to_string())?;
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
    let tables = db::index_of(db::SCHEMA).map_err(|e| format!("the database definition: {e}"))?;
    flow::compile(flow::FLOW, &flow::Context { relations, tables }).map_err(|e| e.to_string())
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
        Command::Flow(_) | Command::Config(_) | Command::TreeCheck { .. } => Ok(()),
        Command::Run { once, mode, port } => run(d, once, mode.into(), port),
        Command::Item(ItemCommand::Add(add)) => {
            let AddItem {
                project,
                title,
                problem,
                why,
                approach,
                deliver,
                level,
                depends_on,
                queue,
                imported_from,
                branch,
                pr,
            } = *add;
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
                    imported: owner::Imported {
                        from: imported_from,
                        branch,
                        pr,
                    },
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
        Command::Asks { all } => print(owner::asks(d, all)),
        Command::Ask { id } => print(owner::ask(d, id)),
        Command::Answer { ask, text, why, reply } => {
            let reply = match reply {
                Some(r) => Some(serde_json::from_str(&r).map_err(|e| format!("--reply: {e}"))?),
                None => None,
            };
            print(owner::answer(d, owner::Answer { ask, text, why, reply }))
        }
        Command::DesignCards { all } => print(owner::design_cards(d, all)),
        Command::DesignCard { id } => print(owner::design_card(d, id)),
        Command::Rule {
            design_card,
            verdict,
            text,
            option,
        } => print(owner::rule(
            d,
            owner::Ruling {
                design_card,
                verdict,
                text,
                option,
            },
        )),
        Command::Say { work, text, line } => owner::say(d, work, &line, &text).map(|id| println!("interjection {id}")),
        Command::Pause { work, why } => owner::pause(d, work, Some(&why)),
        Command::Resume { work } => print(owner::resume(d, work)),
        Command::Status => print(owner::status(d)),
        Command::History { work, full } => print(owner::history(d, work, full)),
        Command::Message { work, json } => message(d, work, json),
        Command::Counters { work } => print(owner::counters(d, work)),
        Command::Anomalies => print(owner::anomalies(d)),
        Command::Cost { work } => print(owner::cost(d, work)),
    }
}

/// How the engine stands, for the page.
fn status(
    e: &mut engine::Engine,
    asked: api::Asked,
    last: Option<&api::StepTaken>,
    started_at: i64,
) -> Result<api::Status, String> {
    let slots: i64 = e
        .config
        .config
        .install
        .machines
        .hosts
        .values()
        .map(|h| i64::from(h.slots))
        .sum();
    Ok(api::Status {
        mode: asked.mode,
        stepping: asked.step,
        last_step: last.cloned(),
        slots_used: slots - e.free_slots()?,
        slots,
        flow_version: e.flow_now,
        config_commit: e.config.commit.clone(),
        started_at,
        version: format!("{} ({})", env!("CARGO_PKG_VERSION"), engine::VERSION),
        halt: e.halt.as_ref().map(|h| api::Halt {
            kind: h.kind.clone(),
            until: h.until,
            why: h.why.clone(),
        }),
    })
}

/// Prints the message the work's next agent round would be sent.
/// Prints the message a work's next round would be sent, with nothing run or recorded: the engine is made only to
/// read, so no flow version is recorded, no work stopped and none started, beside an engine that may be running.
fn message(d: &Path, work: i64, json: bool) -> Result<(), String> {
    let committed = config::committed(d).map_err(|e| e.to_string())?;
    let github = engine::hub::Gh::new(d.join("logs"));
    let mut e = engine::Engine::reading(d, committed, Box::new(engine::agent::Claude), Box::new(github))?;
    let m = engine::next_message(&mut e, work)?;
    if json {
        let v = serde_json::json!({ "message": m.text, "conditional": m.record() });
        println!("{v}");
    } else {
        print!("{}", m.text);
    }
    Ok(())
}

/// Runs the engine on the data directory with `claude -p` and the gh CLI, and serves the owner's page beside it.
fn run(d: &Path, once: bool, mode: api::Mode, port: u16) -> Result<(), String> {
    let committed = config::committed(d).map_err(|e| e.to_string())?;
    let github = engine::hub::Gh::new(d.join("logs"));
    let mut e = engine::Engine::new(d, committed, Box::new(engine::agent::Claude), Box::new(github))?;
    e.say = Box::new(|s| eprintln!("{} {s}", db::now() / 1000));
    if once {
        e.tick()?;
        return Ok(());
    }
    let started_at = db::now();
    let bus = Arc::new(api::Bus::new(started_at));
    let events = Arc::clone(&bus);
    e.db.on_commit(Box::new(move |conn, rows| {
        for ev in api::events(conn, rows) {
            events.publish(ev);
        }
    }))
    .map_err(|x| x.to_string())?;
    let control = Arc::new(api::Control::new(mode));
    // What a running agent is doing goes to the page as it happens.
    let told = Arc::clone(&bus);
    e.activity = Box::new(move |work, round, what| {
        told.publish(api::Event::Activity {
            work,
            round,
            what: what.to_string(),
            at: db::now(),
        });
    });
    let first = status(&mut e, control.asked(), None, started_at)?;
    bus.publish(api::Event::Status {
        status: Box::new(first),
    });
    let port = web::start(web::Options {
        data: d.to_path_buf(),
        db: config::database(d),
        port,
        bus: Arc::clone(&bus),
        control: Arc::clone(&control),
    })?;
    let shown = format!("{mode:?}").to_lowercase();
    (e.say)(&format!(
        "the page is at http://127.0.0.1:{port}/; the engine starts in {shown} mode"
    ));
    // The owner, or whoever installs a new command, stops the engine between steps by creating this file.
    let stop = d.join("stop");
    let _ = std::fs::remove_file(&stop);
    let idle = Duration::from_secs(10);
    let mut last: Option<api::StepTaken> = None;
    while !stop.exists() {
        // What a work's next round would be, for the page that asked: built between steps, nothing run.
        for (work, answer) in control.take_previews() {
            let next = engine::next_round(&mut e, work).map(|n| api::NextRound {
                work,
                cell: n.cell,
                title: n.title,
                kind: n.kind,
                line: n.line,
                message: n.message,
                step: n.step,
                not_asked: n.not_asked,
                tools: n.tools,
                waits: n.waits,
                design_card: n.design_card,
                ask: n.ask,
            });
            let _ = answer.send(next);
        }
        let asked = control.asked();
        e.reload_config();
        let now = status(&mut e, asked, last.as_ref(), started_at)?;
        bus.publish(api::Event::Status { status: Box::new(now) });
        match asked.next() {
            api::Next::Pass => {
                if !e.tick()? {
                    control.wait(asked, idle);
                }
            }
            api::Next::Step(work) => {
                let s = e.step_work(work)?;
                (e.say)(&format!(
                    "the next round of work {work}, as the owner asked: rounds {:?}",
                    s.rounds
                ));
                last = Some(api::StepTaken {
                    at: db::now(),
                    work,
                    rounds: s.rounds,
                    cell: s.cell,
                });
                control.stepped();
            }
            api::Next::Wait => control.wait(asked, idle),
        }
    }
    let _ = std::fs::remove_file(&stop);
    eprintln!("{} stopped: {} exists", db::now() / 1000, stop.display());
    Ok(())
}

#[cfg(test)]
mod tests;
