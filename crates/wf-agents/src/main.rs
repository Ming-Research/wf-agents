//! The `wf-agents` command. Each part of the system adds its subcommands here.
//!
//! - `reference`: `wf-agents help`, the whole reference of the command line.
//! - `running`: `wf-agents run`, the engine as a process.
//! - `talk`: the commands that talk to a running engine.
//! - `situations`: `wf-agents flow situations`, what each agent's cell is sent in every situation it can be in.
//! - `names`, a test: no source names a cell, a line, a group or a block of a flow.

#[cfg(test)]
mod names;
mod reference;
mod running;
mod situations;
mod talk;

use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use flow::{Fault, FaultKind, Has, Source};
use web::works::{Act, ChangeAsked, EngineAct, SpokenTo};

use crate::talk::Asked;

/// The port the engine serves the owner's pages on, on the loopback address, unless another is given.
const PORT: u16 = 7470;

/// Runs coding agents through a defined flow.
#[derive(Parser)]
#[command(version, disable_help_subcommand = true)]
struct Cli {
    // Required: with none, clap fails with the usage and the names of the subcommands.
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// With no command named, prints the whole reference at once: the program's description, then every command with
    /// its long help, each under a line with its full name. With commands named, prints the long help of that one,
    /// as its `--help` does.
    Help {
        /// The command, and the command under it, to print the help of.
        command: Vec<String>,
    },
    /// A flow's definition: a directory holding `flow.toml` and one file for each cell.
    #[command(subcommand)]
    Flow(FlowCommand),
    /// Runs the engine: drives every work of the store in the data directory on the flows given, and serves the
    /// owner's pages on the loopback address. Its agents run with the command line, its acts with git in the project's
    /// repository, as the installation's settings say; or, with a script, the agents, the acts and the tools follow
    /// it. It stops when the engine stops, saying why; started again, it goes on from the store.
    Run {
        /// The data directory, which holds the store.
        #[arg(long)]
        data: PathBuf,
        /// The directory of a flow's definition; given once for each flow.
        #[arg(long = "flow", required = true)]
        flows: Vec<PathBuf>,
        /// A script the agents, the engine's acts and the tools follow, in place of the real ones.
        #[arg(long, conflicts_with = "settings")]
        script: Option<PathBuf>,
        /// The installation's settings; `install.toml` in the data directory where none is given.
        #[arg(long)]
        settings: Option<PathBuf>,
        /// The port of the pages; 0 for any free one.
        #[arg(long, default_value_t = PORT)]
        port: u16,
    },
    /// Adds an item with its work, which is added paused: it begins nothing until it is let on with `wf-agents
    /// resume`.
    Add {
        #[arg(long)]
        title: String,
        /// The level of the item, among those the settings name; the settings' default level where none is given.
        #[arg(long)]
        level: Option<String>,
        /// The project it belongs to, among those the settings name, whose repository its works run in.
        #[arg(long)]
        project: String,
        /// The name of the flow its work runs on.
        #[arg(long)]
        flow: String,
        /// An item it depends on, by number, of its own project or another; given once for each.
        #[arg(long = "depends-on", value_name = "ITEM")]
        depends_on: Vec<u64>,
        /// A tag it carries, which classifies it; given once for each.
        #[arg(long = "tag", value_name = "TAG")]
        tags: Vec<String>,
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
    /// Makes several changes of items at once, for one reason: in the order given, each judged against what those
    /// before it make, all of them or none. Each is ITEM:KIND or ITEM:KIND=VALUE, the kind one of title=TEXT,
    /// level=LEVEL, depends-on=ITEM, no-longer-depends-on=ITEM, tag=TAG, untag=TAG, cancel and reopen. Every refusal
    /// is said at once, by the change's place among them.
    Change {
        #[command(flatten)]
        why: Why,
        #[arg(required = true, value_name = "ITEM:KIND[=VALUE]")]
        changes: Vec<String>,
        #[command(flatten)]
        at: At,
    },
    /// Puts an item at another level, among those the settings name and at most the highest an item may be at: its
    /// work's rounds run on it from their next.
    SetLevel {
        item: u64,
        level: String,
        #[command(flatten)]
        why: Why,
        #[command(flatten)]
        at: At,
    },
    /// Gives an item another title, in the owner's language, as the page and the item's agents know it: its merge is
    /// named by a line its work writes in the repository's language, never by its title. The title it had is kept, as
    /// every change of an item is, with why: `wf-agents show` of its work says the latest, and with `history` each.
    SetTitle {
        item: u64,
        title: String,
        #[command(flatten)]
        why: Why,
        #[command(flatten)]
        at: At,
    },
    /// Makes an item depend on another, while its work has started no run: its work then waits for that one to merge as
    /// well, unless that one is cancelled, when it waits for nothing. Refused where that one depends on it already, by
    /// way of any items.
    AddDependency {
        item: u64,
        /// The item it comes to depend on.
        on: u64,
        #[command(flatten)]
        why: Why,
        #[command(flatten)]
        at: At,
    },
    /// Makes an item no longer depend on another, while its work has started no run.
    RemoveDependency {
        item: u64,
        /// The item it no longer depends on.
        on: u64,
        #[command(flatten)]
        why: Why,
        #[command(flatten)]
        at: At,
    },
    /// Has an item carry a tag, which classifies it: a name with no space and no comma.
    Tag {
        item: u64,
        tag: String,
        #[command(flatten)]
        why: Why,
        #[command(flatten)]
        at: At,
    },
    /// Has an item no longer carry a tag.
    Untag {
        item: u64,
        tag: String,
        #[command(flatten)]
        why: Why,
        #[command(flatten)]
        at: At,
    },
    /// Reopens a cancelled item: its work is paused, and goes on from its records once it is let on with `wf-agents
    /// resume`, making its place again on the branch it kept; the items that depend on it wait for it again. Refused
    /// while an item whose work has started and was not cancelled, merged or not, depends on it, or while its cancel
    /// still lets go of what its work held.
    Reopen {
        item: u64,
        #[command(flatten)]
        why: Why,
        #[command(flatten)]
        at: At,
    },
    /// Says whether everything is paused and how many slots are taken, and lists the items that have not ended, each
    /// with what its work does now, at most a hundred from the first given, and how many that ended it leaves out.
    List {
        /// Only the items that carry this tag.
        #[arg(long)]
        tag: Option<String>,
        /// The item to list from, the first where none is given.
        #[arg(long, value_name = "ITEM")]
        from: Option<u64>,
        /// The items that ended, merged or cancelled, as well.
        #[arg(long)]
        all: bool,
        #[command(flatten)]
        at: At,
    },
    /// What changed in the plan since the mark the last call gave: each item added, each change of an item, each cancel
    /// and reopen, with when and why, oldest first, and the mark to give next time. With no mark, from the start.
    Changes {
        /// The mark the last call gave.
        #[arg(long)]
        since: Option<String>,
        #[command(flatten)]
        at: At,
    },
    /// The items that could be let on now: paused, every item they depend on merged or cancelled. Those more open items
    /// wait for come first; the choice is yours.
    Candidates {
        #[command(flatten)]
        at: At,
    },
    /// Where a text is mentioned, ignoring case: in titles, in the blocks people wrote, and in why changes were made.
    Search {
        text: String,
        /// The most hits to give.
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[command(flatten)]
        at: At,
    },
    /// The observations agents reported beyond their works, the newest first: each with its number, whether the owner
    /// read it, when it was kept, the item and work it came from and the cell, run and round that reported it, then
    /// its whole text. At most twenty are given; the line after them says how many more there are and the number to
    /// give next time with --from.
    Observations {
        /// The number of the observation to give from, going down to the older ones; the newest where none is given.
        #[arg(long, value_name = "NUMBER")]
        from: Option<u64>,
        /// The most observations to give.
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[command(flatten)]
        at: At,
    },
    /// What an item waits for and what waits for it: up, the items it depends on, each with those it depends on in
    /// turn; down, the items that depend on it, likewise; to a depth.
    Graph {
        item: u64,
        /// Only up: what it depends on.
        #[arg(long, conflicts_with = "down")]
        up: bool,
        /// Only down: what depends on it.
        #[arg(long)]
        down: bool,
        /// How many steps to walk.
        #[arg(long, default_value_t = 3)]
        depth: usize,
        #[command(flatten)]
        at: At,
    },
    /// The tags items carry, each with how many items carry it and how many of those have not ended.
    Tags {
        #[command(flatten)]
        at: At,
    },
    /// The text of an item's blocks, as the latest version of each says it, in short: of each section, its heading and
    /// first paragraph, cut. With a block, that block alone; with --whole, that block whole.
    Text {
        item: u64,
        block: Option<String>,
        #[arg(long, requires = "block")]
        whole: bool,
        #[command(flatten)]
        at: At,
    },
    /// Shows a work: what it does now, its cells and its blocks, and the latest ten of what happened to its item, of its
    /// runs and of its sessions, and ten of its questions, those not answered first, saying how many more there are.
    /// With a part, that part alone, a hundred at a time.
    Show {
        work: u64,
        /// The part to give alone.
        part: Option<talk::Part>,
        /// The place in the part to give it from, the first being one.
        #[arg(long, requires = "part", value_name = "N")]
        from: Option<usize>,
        #[command(flatten)]
        at: At,
    },
    /// Lets a work on: its rounds begin again. With --all, lifts the pause of everything instead: every work goes on
    /// that its own pause does not hold.
    Resume {
        #[arg(required_unless_present = "all")]
        work: Option<u64>,
        #[arg(long, conflicts_with = "work")]
        all: bool,
        #[command(flatten)]
        at: At,
    },
    /// Pauses a work: no round of it begins but those of a run that answers a note, a person's words to its cell, and
    /// the one under way ends. With --all, pauses everything instead, until `wf-agents resume --all`; it holds across a
    /// stop of the engine, and a work's own pause holds after it is lifted.
    Pause {
        #[arg(required_unless_present = "all")]
        work: Option<u64>,
        #[arg(long, conflicts_with = "work")]
        all: bool,
        #[command(flatten)]
        at: At,
    },
    /// Cancels a work's item, saying why: nothing of the work goes on. The items that depend on it keep that
    /// dependency, and no longer wait for it; `wf-agents change` cancels several at once, by item.
    Cancel {
        work: u64,
        /// Why it is cancelled, in a sentence, such as the item it was joined into; kept with the cancel.
        #[arg(long)]
        why: String,
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
    /// Gives a call that waits for a person its result, such as that a run at its limit goes on.
    Result {
        work: u64,
        call: u64,
        result: String,
        #[command(flatten)]
        at: At,
    },
    /// Says something to a work: to a cell of its flow, or to a line of it. The engine chooses the cell it reaches, the
    /// cell itself or the one the line has reached, and refuses it, saying why, where no cell can take it.
    Say {
        work: u64,
        words: String,
        /// The line of the work's flow it is said to.
        #[arg(long, conflicts_with = "cell", required_unless_present = "cell")]
        line: Option<String>,
        /// The cell of the work's flow it is said to, by its id.
        #[arg(long)]
        cell: Option<String>,
        #[command(flatten)]
        at: At,
    },
    /// Asks a fork of the session a round of a work ran in a question, through the running engine: with the round's own
    /// system prompt, form of return, tools, model and effort, so that its first request is read from the cache; begun
    /// at the round's end where the session went on past it; every use of a tool refused, so that nothing of the work
    /// changes; its answer in the return's form. Prints each field of the answer, what the fork cost, and the share of
    /// the session its first request read from the cache.
    Fork {
        work: u64,
        run: u64,
        /// The round of the run whose session is forked; the run's last that ran where none is given.
        #[arg(long)]
        round: Option<usize>,
        /// The file that holds the question.
        #[arg(long)]
        question: PathBuf,
        #[command(flatten)]
        at: At,
    },
    /// Answers a version of a question of a work, by its number among the versions of its cell's questions, taking an
    /// option, counted from one.
    Answer {
        work: u64,
        question: String,
        version: u64,
        option: u64,
        #[command(flatten)]
        at: At,
    },
    /// Waits for what needs whoever plans the work and says it, a line each, most pressing first: a work that ended,
    /// one that waits on the owner, each thing it waits on them for by why, as the engine's one list of what waits on
    /// the owner says, one that waits for items none of which is going on. It answers as soon as there is something
    /// since the mark, and says the mark to give next time; nothing that happens between two calls is lost. With no
    /// mark, it gives what stands now, the works that ended being taken as read.
    Wait {
        /// The mark the last call gave.
        #[arg(long)]
        since: Option<String>,
        /// The longest time to wait, in seconds; it then says that nothing came. With none, it waits as long as it
        /// takes.
        #[arg(long = "for", value_name = "SECONDS")]
        longest: Option<u64>,
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
    /// Shows how the system ran over the last days: what its rounds took; the rounds whose cost or time, and the calls
    /// of tools whose time, went past the lines of their history, the latest first; and the rounds and calls under way,
    /// the oldest first, each against its own lines. At most so many of each are listed, and how many more there are
    /// is said.
    System {
        /// How many days the span goes back from now.
        #[arg(long, default_value_t = 7)]
        days: u64,
        /// The most listed of each.
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[command(flatten)]
        at: At,
    },
}

/// Why a change is made, kept with it.
#[derive(Args)]
struct Why {
    /// Why the change is made, in a sentence; kept with it.
    #[arg(long = "why", value_name = "SENTENCE")]
    why: String,
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
    /// or its faults when it does not compile. The directory is read again for every request, so an edit shows when
    /// the page is loaded again.
    Page {
        /// The definition's directory.
        dir: PathBuf,
        /// The port of the page on the loopback address.
        #[arg(long, default_value_t = 7490)]
        port: u16,
    },
    /// Writes what each agent's cell of the definition in a directory is sent in every situation an entry of it can be
    /// in, a file for each situation in a directory for the cell: the situation, the system prompt of its line, the
    /// message and the form of a return, what a run fills left open as `{name}`. It reads nothing of an installation
    /// or a work, and prints for each cell how many situations it wrote.
    Situations {
        /// The definition's directory.
        dir: PathBuf,
        /// The directory each cell's directory is made in; a cell's that holds files already is refused.
        #[arg(long)]
        out: PathBuf,
        /// Only this cell, by its id.
        #[arg(long)]
        cell: Option<String>,
        /// The project's checkout, whose files the system prompt holds; they are left out where none is given.
        #[arg(long)]
        project: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Help { command } => {
            let program = <Cli as clap::CommandFactory>::command();
            let said = if command.is_empty() {
                Some(reference::whole(&program))
            } else {
                reference::of(&program, &command)
            };
            match said {
                Some(said) => {
                    print!("{said}");
                    ExitCode::SUCCESS
                }
                None => {
                    eprintln!("no command `{}`", command.join(" "));
                    ExitCode::FAILURE
                }
            }
        }
        Command::Flow(FlowCommand::Check { dir }) => check(&dir),
        Command::Flow(FlowCommand::Page { dir, port }) => page(dir, port),
        Command::Flow(FlowCommand::Situations {
            dir,
            out,
            cell,
            project,
        }) => {
            let asked = situations::Asked {
                dir: &dir,
                out: &out,
                cell: cell.as_deref(),
                project: project.as_deref(),
            };
            match situations::write(&asked, &has(&dir)) {
                Ok(said) => {
                    said.iter().for_each(|line| println!("{line}"));
                    ExitCode::SUCCESS
                }
                Err(why) => {
                    eprintln!("{why}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Run {
            data,
            flows,
            script,
            settings,
            port,
        } => {
            let carriers = match script {
                Some(script) => running::With::Script(script),
                None => running::With::Settings(settings.unwrap_or_else(|| data.join(carry::settings::FILE))),
            };
            running::run(&data, &flows, carriers, port)
        }
        Command::Add {
            title,
            level,
            project,
            flow,
            depends_on,
            tags,
            texts,
            files,
            choices,
            at,
        } => {
            let written = talk::Written {
                texts: &texts,
                files: &files,
                choices: &choices,
            };
            match talk::new_item(title, level, project, flow, depends_on, tags, written) {
                Ok(item) => talk::ask(at.port, Asked::Add(item)),
                Err(why) => {
                    eprintln!("{why}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Change { why, changes, at } => match talk::changes(&changes) {
            Ok(changes) => talk::ask(at.port, Asked::Change { why: why.why, changes }),
            Err(why) => {
                eprintln!("{why}");
                ExitCode::FAILURE
            }
        },
        Command::SetLevel { item, level, why, at } => one(at, why, ChangeAsked::Level { item, level }),
        Command::SetTitle { item, title, why, at } => one(at, why, ChangeAsked::Title { item, title }),
        Command::AddDependency { item, on, why, at } => one(at, why, ChangeAsked::DependsOn { item, on }),
        Command::RemoveDependency { item, on, why, at } => one(at, why, ChangeAsked::NoLongerDependsOn { item, on }),
        Command::Tag { item, tag, why, at } => one(at, why, ChangeAsked::Tag { item, tag }),
        Command::Untag { item, tag, why, at } => one(at, why, ChangeAsked::Untag { item, tag }),
        Command::Reopen { item, why, at } => one(at, why, ChangeAsked::Reopen { item }),
        Command::List { tag, from, all, at } => talk::ask(at.port, Asked::List(talk::Listing { tag, from, all })),
        Command::Changes { since, at } => talk::ask(at.port, Asked::Changes { since }),
        Command::Candidates { at } => talk::ask(at.port, Asked::Candidates),
        Command::Search { text, limit, at } => talk::ask(at.port, Asked::Search { text, limit }),
        Command::Observations { from, limit, at } => talk::ask(at.port, Asked::Observations { from, limit }),
        Command::Graph {
            item,
            down,
            up,
            depth,
            at,
        } => {
            let way = match (down, up) {
                (true, _) => Some("down"),
                (_, true) => Some("up"),
                _ => None,
            };
            talk::ask(at.port, Asked::Graph { item, way, depth })
        }
        Command::Tags { at } => talk::ask(at.port, Asked::Tags),
        Command::Text { item, block, whole, at } => talk::ask(at.port, Asked::Text { item, block, whole }),
        Command::Show { work, part, from, at } => talk::ask(at.port, Asked::Show { work, part, from }),
        Command::Resume { work, all, at } => match (work, all) {
            (Some(work), false) => talk::ask(at.port, Asked::Act(work, Act::Resume {})),
            _ => talk::ask(at.port, Asked::Everything(EngineAct::Resume {})),
        },
        Command::Pause { work, all, at } => match (work, all) {
            (Some(work), false) => talk::ask(at.port, Asked::Act(work, Act::Pause {})),
            _ => talk::ask(at.port, Asked::Everything(EngineAct::Pause {})),
        },
        Command::Cancel { work, why, at } => talk::ask(at.port, Asked::Cancel(work, why)),
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
        Command::Result { work, call, result, at } => {
            talk::ask(at.port, Asked::Act(work, Act::Result { call, result }))
        }
        Command::Fork {
            work,
            run,
            round,
            question,
            at,
        } => match std::fs::read_to_string(&question) {
            Ok(question) => talk::ask(
                at.port,
                Asked::Fork {
                    work,
                    run,
                    round,
                    question,
                },
            ),
            Err(e) => {
                eprintln!("{}: {e}", question.display());
                ExitCode::FAILURE
            }
        },
        Command::Answer {
            work,
            question,
            version,
            option,
            at,
        } => {
            let act = Act::Answer {
                question,
                version,
                option,
            };
            talk::ask(at.port, Asked::Act(work, act))
        }
        Command::Say {
            work,
            words,
            line,
            cell,
            at,
        } => {
            let to = match (line, cell) {
                (_, Some(cell)) => SpokenTo::Cell { cell },
                (Some(line), None) => SpokenTo::Line { line },
                (None, None) => unreachable!("the command line asks for a line or a cell"),
            };
            talk::ask(at.port, Asked::Act(work, Act::Say { to, words }))
        }
        Command::Wait { since, longest, at } => talk::ask(at.port, Asked::Wait { since, longest }),
        Command::LetGo { work, cell, at } => talk::ask(at.port, Asked::Act(work, Act::LetGo { cell })),
        Command::System { days, limit, at } => talk::ask(at.port, Asked::System { days, limit }),
    }
}

/// Asks for one change of an item, for its reason.
fn one(at: At, why: Why, change: ChangeAsked) -> ExitCode {
    talk::ask(
        at.port,
        Asked::Change {
            why: why.why,
            changes: vec![change],
        },
    )
}

/// The names of the sources outside a work that write its blocks, which every definition is compiled against:
/// `dependencies`, `session` and `start`, written by the engine's driver, the last as the owner starts the work;
/// `answer`, written by the engine's loop, a cell's own questions, as a question of it is asked, asked anew or
/// withdrawn and as the owner answers one; `asked`, as the questions and their answers come to stand otherwise;
/// `handed_in`, written by the engine's loop as the first run of a cell that dealt with something finishes; `main`,
/// written by the acts' watch of main on the host; `tree` and `head`, read by the carriers of the design tree and of the
/// work's head on a work's branch, written from outside or handed in with the round that committed them; and `words`,
/// written by the engine's loop as the owner speaks to a cell, the cell's own words.
const SOURCES: [&str; 10] = [
    "answer",
    "asked",
    "dependencies",
    "handed_in",
    "head",
    "main",
    "session",
    "start",
    "tree",
    "words",
];

/// The names of the acts of the engine's own that carry out an engine cell: `start` makes the place a work is done
/// in, `check` checks the work before its merge, `merge` merges it, `deliver` says that a deliverable item, made of
/// the items it depends on, is delivered, and `conclude` says that a study is concluded.
const ACTS: [&str; 5] = ["start", "check", "merge", "deliver", "conclude"];

/// The names of the tools an agent may ask the engine for in its return: `query`, one read-only query on the engine's
/// records, and `run_tests`, which runs named tests and checks, or all of them, where the host checks the work. A cell
/// may name them; the system prompt of its line tells of those its flow names, with their words beside the flows
/// (`agents/tools/`), and the form of the return holds them.
const TOOLS: [&str; 2] = ["query", "run_tests"];

/// What the engine has, by name, for the definition in this directory. Of the parts of a line's system prompt, the
/// names of those that are not the line's own sets of rules, which no line names and no set of rules takes, are those
/// `agents/session.toml` beside the flows gives them (`flow::system::reserved`); where the files beside the flows
/// cannot be read, none, and what would read them says why.
fn has(dir: &Path) -> Has {
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
    let beside = dir.parent().map(flow::agents::read);
    let system = (beside.and_then(Result::ok)).map(|beside| flow::system::reserved(&beside));
    Has {
        sources: names(&SOURCES),
        acts: names(&ACTS),
        tools: names(&TOOLS),
        system: system.into_iter().flatten().collect(),
    }
}

fn check(dir: &Path) -> ExitCode {
    let source = match flow::read(dir) {
        Ok(source) => source,
        Err(e) => {
            eprintln!("{}: {e}", dir.display());
            return ExitCode::FAILURE;
        }
    };
    match flow::compile(&source, &has(dir)) {
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
    format!("{at}: {}", words(&fault.what, &fault.field))
}

/// What is wrong, in words. Where it is wrong is told too, since a name given twice is one of two things: a name in a
/// list that stands in it again, and, at the field `tools` itself, a cell naming tools of its own where the flow's file
/// already gives them to every agent cell.
fn words(what: &FaultKind, field: &str) -> String {
    use FaultKind as F;
    match what {
        F::Unreadable { message } => format!("cannot be read: {}", message.trim_end()),
        F::OrderEmpty => "the order names no cell".to_string(),
        F::NoCellFile { cell } => format!("the order names {cell}, which has no file"),
        F::NotInOrder { cell } => format!("the cell {cell} has a file but is not named in the order"),
        F::Twice { name } if field == "tools" => {
            format!("the cell names {name} of its own, and the flow already gives its agent cells theirs")
        }
        F::Twice { name } if field.starts_with("reads[") || field.starts_with("every_agent.reads[") => {
            format!("{name} is watched already, and is given with what is watched")
        }
        F::Twice { name } => format!("{name} is named a second time in the same list"),
        F::TakenTwice { block, first, also } => {
            format!("{block} is taken by two names, {first} and {also}, which says twice how the cell takes it")
        }
        F::LimitOutOfRange { value } => {
            format!("the limit {value} is not a whole number from 1 to the largest count the engine keeps")
        }
        F::NoLine => "an agent's cell names no line to run in".to_string(),
        F::EngineHasLine { line } => format!("an engine's cell runs in no session, but names the line {line}"),
        F::NoSuchLine { line } => format!("the line {line} is not declared in flow.toml"),
        F::NoTask => {
            "an agent's cell has no task, or one of no words: its agent would be told nothing to do".to_string()
        }
        F::NoTaskFile { file } => format!("the task is written in {file}, which the flow's directory does not hold"),
        F::TaskFileEmpty { file } => format!("the task is written in {file}, which holds no words"),
        F::TaskFileOutside { file } => format!("the task is written in {file}, which is outside the flow's directory"),
        F::EngineHasTask => "an engine's cell has a task, which no agent reads".to_string(),
        F::NoAct => "an engine's cell names no act to carry it out".to_string(),
        F::AgentHasAct { act } => format!("an agent carries the cell out, but the cell names the act \"{act}\""),
        F::EngineHasEffort => "an engine's cell names a reasoning effort, which no agent runs at".to_string(),
        F::NoEffort => "an agent's cell names no reasoning effort for its rounds to run at".to_string(),
        F::EngineHasTools => "an engine's cell names tools, which no agent asks for".to_string(),
        F::UnknownAct { act } => format!("the engine has no act named \"{act}\""),
        F::UnknownTool { tool } => format!("the engine has no tool named \"{tool}\""),
        F::NoSuchRules { name } => format!("the line names the rules {name}, but the flow has no rules/{name}.md"),
        F::EnginePart { name } => {
            format!("the engine places {name} in every system prompt itself: a line names only its own sets of rules")
        }
        F::RulesNamedAsEnginePart { name } => {
            format!("a set of rules is named {name}, which is a part the engine places in every system prompt itself")
        }
        F::UnusedRules { name } => format!("no line names the rules {name}, so no agent would be given them"),
        F::ReadsOnlyAndTestsOnly => {
            "the line says its agents change only the project's tests, and that they only read, which changes no file"
                .to_string()
        }
        F::ValuesEmpty => "a choice has no values to choose from".to_string(),
        F::NoteNotByPeople { block } => format!("{block} is a note, but people do not write it"),
        F::NoteIsAChoice { block } => format!("{block} is a note, but it is a choice, and a note is words to a cell"),
        F::ChangedByWithoutSource { block } => {
            format!("{block} says who changes what its source writes, but no source outside the work writes it")
        }
        F::NoSuchBlock { block } => format!("the block {block} is not declared in flow.toml"),
        F::NotAnOutput { block } => {
            format!("the cell must write {block} every time, but {block} is not among the blocks it may write")
        }
        F::NoSuchData { name } => {
            format!("the flow names {name} among the engine's own data, but no file of ../data/ defines {name}")
        }
        F::DefinedTwice { name, also } => format!("{name} is defined here and again in {also}"),
        F::MissingPlaceholder { placeholder } => format!("this must hold {placeholder}"),
        F::AsksWithNoAnswer { block } => {
            format!(
                "the cell writes {block}, its questions, but does not watch them: the owner's answer would run no cell"
            )
        }
        F::AsksAndCannotBeSpokenTo { block } => {
            format!(
                "the cell writes {block}, its questions, but watches no words of its own: the owner's words about its question would reach no cell"
            )
        }
        F::NotItsOwn { block } => {
            format!("the cell writes {block}, a block of the owner's answers or words that is not its own")
        }
        F::SpokenToAndAsksNothing { block } => {
            format!("the cell writes {block}, its words with the owner, but asks the owner nothing")
        }
        F::VerdictOfDialogues { verdict, dialogues } => format!(
            "the cell hands in {verdict}, its verdict, and judges in {}, so its points could go in more than one",
            dialogues.join(", ")
        ),
        F::VerdictWithoutDialogue { verdict } => {
            format!("the cell hands in {verdict}, its verdict, and judges in no dialogue its points could go in")
        }
        F::LineDataOfAnother { block, line } => {
            format!("{block} is the line {line}'s own, and the cell is not on that line")
        }
        F::CannotBeAskedIn { cell } => format!(
            "the engine cannot ask in {cell}: it must be an agent's cell, on a line that may change the work, with words of its own"
        ),
        F::NeverNew {
            watched,
            block,
            needs,
            writers,
        } => {
            let writers: Vec<String> = writers.iter().map(|(cell, when)| format!("{cell} ({when})")).collect();
            format!(
                "the cell watches {watched}, but it runs only where {needs}, and each writer of {block} runs only where it cannot: {}; {watched} can never be new to it",
                writers.join("; ")
            )
        }
        F::EngineIsGiven { block } => {
            format!("an engine's cell says how it is given {block}, but it has no session")
        }
        F::RunByNothing => "nothing runs this agent's cell, so it would never be entered".to_string(),
        F::NoKind { block } => format!("the cell is given {block}, but does not say its kind"),
        F::NotWatched { block } => format!("there is a do for {block}, but {block} is not watched"),
        F::NotGiven { block } => format!("the cell says how it is given {block}, but it is not given {block}"),
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
        F::DefaultNotAChoice { block } => {
            format!("{block} says what it counts as before it is written, and it is not a choice")
        }
        F::DefaultNotAValue { block, default } => {
            format!("{block} counts as {default} before it is written, which is not one of its values")
        }
        F::NothingButNotAValue { block, value } => {
            format!(
                "{block} says what a return with {value} holds nothing else but, and {value} is not one of its values"
            )
        }
        F::NothingButNotAnOutput { block, value, output } => {
            format!("{block} says a return with {value} may hold {output}, which no cell that writes {block} may write")
        }
        F::BlockNamedTest => {
            "a block is named test, which a cell's when and requires take for a named test".to_string()
        }
        F::EmptyCondition => "a table of no keys says nothing that must hold".to_string(),
        F::GroupNameTaken { group } => format!("the group {group} has the name of a block or a datum of the flow"),
        F::GroupEmpty { group } => format!("the group {group} names no block"),
        F::ConditionOnOwnOutput { block } => format!(
            "the condition reads {block}, which the cell itself writes and another writes too: the cell would decide \
             by what it writes itself, beside another, whether it runs"
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
        F::TestNamesNoValue { test } => format!("the test {test} names no value to look for: it has no is"),
        F::ContentTestNamesValue { test } => {
            format!("the test {test} looks for content, and names a value to look for, which it does not read")
        }
        F::UnusedTest { test } => format!("no cell uses the test {test}"),
        F::TestOnOwnOutput { test, block } => format!(
            "the test {test} reads {block}, which the cell itself writes: the cell would decide by what it writes itself \
             whether the work waits before it, or, beside another writer, whether it runs"
        ),
        F::TestNotWrittenBefore { test, block } => {
            format!("the test {test} reads {block}, which neither a cell before this one, people nor the outside write")
        }
        F::TestWrittenAfter { test, block, cell } => {
            format!("the test {test} reads {block}, which {cell}, a cell after this one, writes")
        }
        F::StandingWrittenByTwo { test, block, cell } => format!(
            "the test {test}, of what stands, reads {block}, which {cell} writes as well as a cell before it: a \
             skipped cell's write would hide the other's word"
        ),
        F::NotACellId { id } => format!("{id} is no cell's id, its stage's initials and a number"),
        F::IdGivenTwice { id } => format!("the renumbering gives {id} to two cells"),
        F::IdToItself { id } => format!("the renumbering gives {id} the id it had"),
        F::MovedAndTakenOut { id } => format!("the renumbering both moves {id} and takes it out"),
        F::IdStillHeld { id } => format!(
            "the renumbering gives {id}, which a cell of its stage still had: the same renumbering moves or takes out \
             that cell"
        ),
        F::Flow(fault) => format!("the deciding code refuses the cells as a flow: {fault:?}"),
    }
}

/// The address the flow page is printed at: the page shows the view its address's hash names, and this server serves
/// the flow alone.
fn flow_page(address: SocketAddr) -> String {
    format!("http://{address}/#/flow")
}

#[test]
fn a_name_given_twice_says_where_it_was_given_twice() {
    let twice = |name: &str, field: &str| words(&FaultKind::Twice { name: name.to_string() }, field);
    assert_eq!(
        twice("query", "tools[2]"),
        "query is named a second time in the same list"
    );
    assert_eq!(
        twice("tools", "tools"),
        "the cell names tools of its own, and the flow already gives its agent cells theirs"
    );
    assert_eq!(
        twice("brief", "reads[0]"),
        "brief is watched already, and is given with what is watched"
    );
}

#[test]
fn a_block_taken_by_two_names_says_both_names() {
    let taken = |block: &str, first: &str, also: &str| {
        let what = FaultKind::TakenTwice {
            block: block.to_string(),
            first: first.to_string(),
            also: also.to_string(),
        };
        words(&what, "reads[0]")
    };
    assert_eq!(
        taken("draft", "outline", "draft"),
        "draft is taken by two names, outline and draft, which says twice how the cell takes it"
    );
    assert_eq!(
        taken("note", "outline", "papers"),
        "note is taken by two names, outline and papers, which says twice how the cell takes it"
    );
}

#[test]
fn the_flow_page_is_printed_at_the_flow_view() {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, 7490));
    assert_eq!(flow_page(address), "http://127.0.0.1:7490/#/flow");
}

fn page(dir: PathBuf, port: u16) -> ExitCode {
    let shown = dir.display().to_string();
    let in_dir = dir.clone();
    let flows = web::Flows::dirs(dir, Box::new(move |_: &Source| has(&in_dir)));
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
        println!("The flow page of {shown} is at {}", flow_page(address));
        match web::serve(listener, web::router(flows)).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("the server stopped: {e}");
                ExitCode::FAILURE
            }
        }
    })
}
