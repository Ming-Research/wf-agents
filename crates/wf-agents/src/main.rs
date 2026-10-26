//! The `wf-agents` command. Each part of the system adds its subcommands here.

mod owner;

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
    /// Runs the engine: starts works on the queue's items and takes each through the flow, until stopped.
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
    Card {
        /// The card's number.
        id: i64,
    },
    /// Answers a question card, or rules on a design card.
    Answer {
        /// The card's number.
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

#[derive(Subcommand)]
enum ItemCommand {
    /// Creates an item.
    Add {
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
    },
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
    e.run(&|| false, Duration::from_secs(10))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use clap::{Command, CommandFactory};

    use super::Cli;

    mod adversarial;

    const README: &str = include_str!("../../../README.md");

    // What the README's data directory section names. This list is a copy: it keeps the README from losing an entry,
    // and does not notice a path the engine starts to write.
    const DATA_DIRECTORY: [&str; 9] = [
        "install.toml",
        "shared/",
        "projects/",
        "rules/",
        "db/",
        "logs/",
        "tool-output/",
        "notifications.log",
        "worktrees/",
    ];

    /// The command tree as clap builds it to parse: with its own `--help` and `help`, and with the global `--data`
    /// passed on to every subcommand.
    fn tree() -> Command {
        let mut cmd = Cli::command();
        cmd.build();
        cmd
    }

    /// Every command that has no subcommands, with the words that run it, such as `wf-agents item add`.
    fn leaves<'a>(cmd: &'a Command, words: &str, out: &mut Vec<(String, &'a Command)>) {
        for sub in cmd.get_subcommands().filter(|s| s.get_name() != "help") {
            let path = format!("{words} {}", sub.get_name());
            if sub.has_subcommands() {
                leaves(sub, &path, out);
            } else {
                out.push((path, sub));
            }
        }
    }

    /// The command lines a text writes in code: the inline spans, and the lines of fenced blocks, that start with
    /// `wf-agents `. A command line has to be on one line of the text.
    fn snippets(text: &str) -> Vec<&str> {
        let mut out = Vec::new();
        let mut fenced = false;
        for line in text.lines() {
            if line.trim_start().starts_with("```") {
                fenced = !fenced;
            } else if fenced {
                out.push(line.trim());
            } else {
                out.extend(line.split('`').skip(1).step_by(2).map(str::trim));
            }
        }
        out.retain(|s| s.starts_with("wf-agents "));
        out
    }

    /// What a command line comes to: the words that name its command, and what is wrong with it. Subcommands are
    /// looked for up to the first option; the words after it are option values or positional arguments, and are not
    /// checked, nor are short options.
    fn read_line(root: &Command, line: &str) -> (String, Vec<String>) {
        let mut cmd = root;
        let mut path = String::from("wf-agents");
        let mut wrong = Vec::new();
        let mut options = false;
        for word in line.split_whitespace().skip(1) {
            if word.starts_with('-') {
                options = true;
                if let Some(long) = word.strip_prefix("--") {
                    let name = long.split('=').next().unwrap_or_default();
                    if !cmd.get_arguments().any(|a| a.get_long() == Some(name)) {
                        wrong.push(format!("{line}: no option --{name}"));
                    }
                }
            } else if options || !cmd.has_subcommands() {
                // An option's value or a positional argument: not checked.
            } else if word.starts_with('<') {
                // A placeholder for the command: which one is not known, so nothing after it is checked.
                break;
            } else if let Some(sub) = cmd.find_subcommand(word) {
                cmd = sub;
                path = format!("{path} {word}");
            } else {
                wrong.push(format!("{line}: no subcommand {word}"));
                break;
            }
        }
        (path, wrong)
    }

    /// What is wrong with a text against the commands: a command it never writes, and a subcommand or a long option
    /// that one of its command lines names and the command does not have.
    fn problems(text: &str, root: &Command) -> Vec<String> {
        let mut written = BTreeSet::new();
        let mut out = Vec::new();
        for line in snippets(text) {
            let (path, wrong) = read_line(root, line);
            written.insert(path);
            out.extend(wrong);
        }
        let mut all = Vec::new();
        leaves(root, "wf-agents", &mut all);
        for (path, _) in all {
            if !written.contains(&path) {
                out.push(format!("{path}: never written"));
            }
        }
        out
    }

    // The README writes every command, and names only subcommands and long options that exist. One command from each
    // depth is named, so that a walk which finds no commands cannot pass.
    #[test]
    fn the_readme_matches_the_commands() {
        let root = tree();
        let mut all = Vec::new();
        leaves(&root, "wf-agents", &mut all);
        let paths: Vec<&str> = all.iter().map(|(path, _)| path.as_str()).collect();
        for sample in ["wf-agents run", "wf-agents flow check", "wf-agents item add"] {
            assert!(paths.contains(&sample), "{sample} is not among {paths:?}");
        }
        let wrong = problems(README, &root);
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    // Each way a README can be wrong is reported once, as what it is; a README that is right reports nothing.
    #[test]
    fn a_wrong_readme_is_reported_as_what_is_wrong_with_it() {
        let root = tree();
        let mut all = Vec::new();
        leaves(&root, "wf-agents", &mut all);
        // The text that writes every command, less the commands in `without` and with the command lines in `extra`.
        let text = |without: &[&str], extra: &[&str]| {
            let mut lines: Vec<String> = Vec::new();
            for (path, _) in &all {
                if !without.contains(&path.as_str()) {
                    lines.push(format!("`{path}`"));
                }
            }
            lines.extend(extra.iter().map(|line| format!("`{line}`")));
            lines.join("\n")
        };
        let right = problems(&text(&[], &[]), &root);
        assert!(right.is_empty(), "{right:#?}");
        let missing = text(&["wf-agents queue put"], &[]);
        let expected = ["wf-agents queue put: never written"];
        assert_eq!(problems(&missing, &root), expected);
        // `wf-agents cards` stays in the text, and is not `wf-agents card`.
        let confused = text(&["wf-agents card"], &[]);
        let expected = ["wf-agents card: never written"];
        assert_eq!(problems(&confused, &root), expected);
        let no_such_subcommand = text(&[], &["wf-agents queue add"]);
        let expected = ["wf-agents queue add: no subcommand add"];
        assert_eq!(problems(&no_such_subcommand, &root), expected);
        let no_such_option = text(&[], &["wf-agents run --twice"]);
        let expected = ["wf-agents run --twice: no option --twice"];
        assert_eq!(problems(&no_such_option, &root), expected);
    }

    // Every argument of every command has a help text, so `--help` describes all of them.
    #[test]
    fn every_argument_has_a_help_text() {
        let root = tree();
        let mut all = Vec::new();
        leaves(&root, "wf-agents", &mut all);
        let seen: usize = all.iter().map(|(_, cmd)| cmd.get_arguments().count()).sum();
        assert!(seen > 0, "no arguments were found");
        let mut bare = Vec::new();
        for (path, cmd) in &all {
            for arg in cmd.get_arguments() {
                if arg.get_help().is_none() {
                    bare.push(format!("{path}: {}", arg.get_id()));
                }
            }
        }
        assert!(bare.is_empty(), "{bare:#?}");
    }

    // The README names what is in the data directory.
    #[test]
    fn the_readme_names_what_is_in_the_data_directory() {
        for entry in DATA_DIRECTORY {
            let code = format!("`{entry}`");
            assert!(README.contains(&code), "the README does not name {code}");
        }
    }
}
