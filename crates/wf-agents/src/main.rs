//! The `wf-agents` command. Each part of the system adds its subcommands here.
//!
//! - `running`: `wf-agents run`, the engine as a process.
//! - `talk`: the commands that talk to a running engine.
//! - `situations`: `wf-agents flow situations`, what each agent's cell is sent in every situation it can be in.

mod running;
mod situations;
mod talk;

use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use flow::{Fault, FaultKind, Has, Source};
use web::works::{Act, DependencyChange, EngineAct};

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
    /// Puts an item at another level, among those the settings name and at most the highest an item may be at: its
    /// work's rounds run on it from their next.
    SetLevel {
        item: u64,
        level: String,
        #[command(flatten)]
        at: At,
    },
    /// Gives an item another title, which its merge will be named by. The title it had is kept, as every change of an
    /// item is: `wf-agents show` says each.
    SetTitle {
        item: u64,
        title: String,
        #[command(flatten)]
        at: At,
    },
    /// Makes an item depend on another, while its work has started no run: its work then waits for that one to merge as
    /// well. Refused where that one depends on it already, by way of any items.
    AddDependency {
        item: u64,
        /// The item it comes to depend on.
        on: u64,
        #[command(flatten)]
        at: At,
    },
    /// Makes an item no longer depend on another, while its work has started no run.
    RemoveDependency {
        item: u64,
        /// The item it no longer depends on.
        on: u64,
        #[command(flatten)]
        at: At,
    },
    /// Says whether everything is paused and how many slots are taken, and lists the items, each with what its work
    /// does now.
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
    /// Cancels a work's item, saying why: nothing of the work goes on.
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
    /// Gives a call that waits for a person its result.
    Answer {
        work: u64,
        call: u64,
        result: String,
        #[command(flatten)]
        at: At,
    },
    /// Waits for what needs whoever plans the work and says it, a line each, most pressing first: a work that ended,
    /// one that rests for a person (a cell's question, a ruling only a person can give, a limit, a pause the engine
    /// made with its reason), one that waits for items none of which is going on. It answers as soon as there is
    /// something since the mark, and says the mark to give next time; nothing that happens between two calls is lost.
    /// With no mark, it gives what stands now, the works that ended being taken as read.
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
    /// Has a line of a work begin a new session at its next round.
    NewSession {
        work: u64,
        line: String,
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
            texts,
            files,
            choices,
            at,
        } => match talk::new_item(title, level, project, flow, depends_on, &texts, &files, &choices) {
            Ok(item) => talk::ask(at.port, Asked::Add(item)),
            Err(why) => {
                eprintln!("{why}");
                ExitCode::FAILURE
            }
        },
        Command::SetLevel { item, level, at } => talk::ask(at.port, Asked::SetLevel(item, level)),
        Command::SetTitle { item, title, at } => talk::ask(at.port, Asked::SetTitle(item, title)),
        Command::AddDependency { item, on, at } => {
            talk::ask(at.port, Asked::Depend(item, DependencyChange::Add { item: on }))
        }
        Command::RemoveDependency { item, on, at } => {
            talk::ask(at.port, Asked::Depend(item, DependencyChange::Remove { item: on }))
        }
        Command::List { at } => talk::ask(at.port, Asked::List),
        Command::Show { work, at } => talk::ask(at.port, Asked::Show(work)),
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
        Command::Answer { work, call, result, at } => {
            talk::ask(at.port, Asked::Act(work, Act::Answer { call, result }))
        }
        Command::Wait { since, longest, at } => talk::ask(at.port, Asked::Wait { since, longest }),
        Command::LetGo { work, cell, at } => talk::ask(at.port, Asked::Act(work, Act::LetGo { cell })),
        Command::NewSession { work, line, at } => talk::ask(at.port, Asked::Act(work, Act::NewSession { line })),
    }
}

/// The names of the sources outside a work that write its blocks, which every definition is compiled against:
/// `dependencies`, `session` and `start`, written by the engine's driver, the last as the owner starts the work;
/// `main`, written by the acts' watch of main on the host; and `tree`, written by the carriers as they look at the
/// design tree on a work's branch.
const SOURCES: [&str; 5] = ["dependencies", "main", "session", "start", "tree"];

/// The names of the acts of the engine's own that carry out an engine cell: `start` makes the place a work is done
/// in, `check` checks the work before its merge, `merge` merges it, and `deliver` says that a deliverable item, made
/// of the items it depends on, is delivered.
const ACTS: [&str; 4] = ["start", "check", "merge", "deliver"];

/// The names of the tools an agent may ask the engine for in its return: `query`, one read-only query on the engine's
/// records, and `run_tests`, which runs named tests and checks, or all of them, where the host checks the work. Neither
/// is built yet: a cell may name them, and the system prompt of its line tells of those its flow names, with their
/// words beside the flows (`agents/tools/`), while the form of the return does not hold them yet.
const TOOLS: [&str; 2] = ["query", "run_tests"];

/// The names of the fields of a return that are the engine's, as the design of data and context settles them: what a
/// round has dealt with, the blocks it hands in with their meaning unchanged, its observations, its questions and its
/// answer to the owner. A datum may be written through `questions` or `reply`.
const FIELDS: [&str; 5] = ["dealt_with", "meaning_unchanged", "observations", "questions", "reply"];

/// The names of the places the engine fills in a run with what happened, which a trigger may say it happened by:
/// `files`, the files of the design tree that changed. No trigger of the flows says it now: a cell is given the tree
/// as a datum.
const DETAILS: [&str; 1] = ["files"];

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
        fields: names(&FIELDS),
        details: names(&DETAILS),
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
        F::Twice { name } => format!("{name} is named a second time in the same list"),
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
        F::ValuesEmpty => "a choice has no values to choose from".to_string(),
        F::NoteNotByPeople { block } => format!("{block} is a note, but people do not write it"),
        F::NoteIsAChoice { block } => format!("{block} is a note, but it is a choice, and a note is words to a cell"),
        F::NoSuchBlock { block } => format!("the block {block} is not declared in flow.toml"),
        F::NotAnOutput { block } => {
            format!("the cell must write {block} every time, but {block} is not among the blocks it may write")
        }
        F::DescribedNotRunOn { block } => {
            format!("the cell says what the work is by {block}, but {block} does not run it")
        }
        F::TakenNotRead { block } => {
            format!("the cell says how it takes {block}, but {block} is not among the blocks it reads")
        }
        F::TwoForms => "name them one way: inputs, runs_on and given, or triggered_by and context".to_string(),
        F::NoSuchData { name } => {
            format!("the flow names {name} among the engine's own data, but no file of ../data/ defines {name}")
        }
        F::DefinedTwice { name, also } => format!("{name} is defined here and again in {also}"),
        F::NoSuchTrigger { trigger } => {
            format!("the cell names the trigger {trigger}, which nothing in ../triggers/ defines")
        }
        F::NoSuchDatum { trigger, datum } => {
            format!("the cell names the trigger {trigger}, but the flow has no datum {datum}")
        }
        F::ReadNotInContext { trigger, block } => {
            format!("the trigger {trigger} has the agent read {block}, but {block} is not in the cell's context")
        }
        F::NotATrigger { trigger } => {
            format!("the cell says what it does for {trigger}, but {trigger} is not among its triggers")
        }
        F::TriggerOnly { block } => {
            format!("{block} stands only for a trigger and has no content: no cell is given it or writes it")
        }
        F::NotLaidOut => {
            "the cell names triggered_by and context, which only a flow laid out one thing to a file has".to_string()
        }
        F::MissingPlaceholder { placeholder } => format!("this must hold {placeholder}"),
        F::UnknownReturnField { field } => format!("the engine's return has no field named \"{field}\""),
        F::UnknownDetail { detail } => format!("the engine fills no place named \"{detail}\""),
        F::EngineIsGiven { block } => {
            format!("an engine's cell says how it is given {block}, but it has no session")
        }
        F::RunByNothing => "nothing runs this agent's cell, so it would never be entered".to_string(),
        F::NoKind { block } => format!("the cell is given {block}, but does not say its kind"),
        F::SaysNothing { block } => format!("the table for {block} says nothing"),
        F::NotRunOn { block } => format!("the cell says why {block} runs it, but {block} is not among what runs it"),
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
