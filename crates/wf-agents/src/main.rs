//! The `wf-agents` command. Each part of the system adds its subcommands here.

mod owner;
mod tree;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
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
    /// Makes the database in the data directory, once. It is the only command that makes one: every command that
    /// works on the database, the engine included, opens the one that is there, and fails when there is none.
    Init,
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
    /// appears in the data directory. The owner's page is served beside it, on the loopback address. Every work starts
    /// paused: nothing runs until the owner starts all works, starts one, or asks for a work's round, on the page.
    Run {
        /// Takes one pass only, without the page: carries out the outbox, starts what can start, and takes each work
        /// one step.
        #[arg(long)]
        once: bool,
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
    Ask { id: i64 },
    /// Answers an ask of the owner.
    Answer {
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
        /// For an ask of the architect's that offers options, the option chosen: for its study or its policy, one of
        /// those the ask lists.
        #[arg(long)]
        choice: Option<String>,
    },
    /// The design cards waiting for a ruling, and those ruled but not yet confirmed for the merge.
    DesignCards {
        /// Every design card, replaced ones too.
        #[arg(long)]
        all: bool,
    },
    /// One design card in full, with its rulings.
    DesignCard { id: i64 },
    /// Rules on a design card: a verdict on each of its decisions, by number, all at once. The card is confirmed when
    /// every decision is.
    Rule {
        design_card: i64,
        /// A decision confirmed, by its number; `all` for every decision but those disagreed with.
        #[arg(long)]
        confirm: Vec<String>,
        /// A decision disagreed with, and why: `<number>=<why>`.
        #[arg(long)]
        disagree: Vec<String>,
        /// Another of a decision's approaches, chosen instead of the one recommended: `<number>=<approach>`; with
        /// --disagree for the same decision to say why.
        #[arg(long)]
        choose: Vec<String>,
    },
    /// Says something in a running work's group, as the page does: to its engineer, which gets it in its next round;
    /// to its project's architect, which gets it as the owner's words, the engineer told of it; or to both.
    Say {
        work: i64,
        text: String,
        /// engineer, architect or both.
        #[arg(long, default_value = "engineer")]
        to: String,
    },
    /// A project's architect: what the owner says to it.
    #[command(subcommand)]
    Architect(ArchitectCommand),
    /// Pauses a work, as the owner: a round of it under way ends first, and it stays paused, whatever the global state,
    /// until the owner starts it again with `resume` or its Start on the page; the page's Start all does not lift it.
    Pause {
        work: i64,
        #[arg(long)]
        why: String,
    },
    /// Lifts a work's pause, as the owner, as its Start on the page does: one the architect stopped is the owner's
    /// release of it, which the architect is told. It then goes on as every work does, which while every work is
    /// paused on the page is only once the owner starts it there or lets every work go on.
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
    /// The message the work's next agent round would be sent, built as the engine builds it, with nothing run or
    /// recorded.
    Message {
        work: i64,
        /// As JSON: the message, and each conditional input it gives or refers to, as a round records them.
        #[arg(long)]
        json: bool,
    },
    /// How many times a work took the rows that count each counter.
    Counters { work: i64 },
    /// Hands in the interjections waiting for a running round's line, as the round's command line runs it after each
    /// tool use: what it prints, the command line adds to the session.
    #[command(hide = true)]
    Deliver {
        #[arg(long)]
        work: i64,
        #[arg(long)]
        round: i64,
        /// The round's line; none for a cell that takes no interjections, which is handed none.
        #[arg(long)]
        line: Option<String>,
    },
    /// Returns sent back, compactions, counters past their limits, and works paused.
    Anomalies,
    /// The time and cost of each work with a total row last, or of one work's rounds by cell.
    Cost { work: Option<i64> },
}

/// What a new item is.
#[derive(clap::Args)]
struct AddItem {
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
    /// An item it depends on, the dependency's kind and what it needs of it, as N:blocking:what (it does not start until
    /// N is merged) or N:parallel:what (it may start before).
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
        item: i64,
        on: i64,
        #[arg(long)]
        needs: String,
        /// blocking: the item does not start until the other is merged; parallel: it may start before.
        #[arg(long, value_parser = ["blocking", "parallel"])]
        kind: String,
    },
    /// Cancels an item, and ends its work.
    Cancel {
        item: i64,
        #[arg(long)]
        why: String,
    },
}

#[derive(Subcommand)]
enum ArchitectCommand {
    /// Says something to a project's architect: it comes to its queue as the owner's words, and starts a work of it
    /// when none runs and works go on.
    Say {
        text: String,
        /// The project whose architect it is for; it may be left out when the configuration has one project.
        #[arg(long)]
        project: Option<String>,
        /// The item the words are about, if any.
        #[arg(long)]
        item: Option<i64>,
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
    // Each line's sessions: where a new one starts, and how large the forms every return of the line shares are.
    for g in &compiled.graphs {
        let mut lines: Vec<String> = Vec::new();
        let mut seen: Vec<&str> = Vec::new();
        for c in g.cells.iter().filter(|c| c.kind == "agent") {
            let Some(line) = c.line.as_deref().filter(|l| !seen.contains(l)) else {
                continue;
            };
            seen.push(line);
            let cells = sessions::format_cells(g, c);
            let size = flow::formats::common_format_of(g, &cells, &[]).chars().count();
            let starts: Vec<&str> = cells.iter().filter(|x| x.new_session).map(|x| x.id.as_str()).collect();
            let at = if starts.is_empty() {
                String::new()
            } else {
                format!(", new sessions at {}", starts.join(", "))
            };
            lines.push(format!("{line} {size}{at}"));
        }
        println!(
            "{}: format:common {} characters, by line: {}",
            g.name,
            flow::formats::common_format(g, &[]).chars().count(),
            lines.join("; ")
        );
    }
    println!("digest {}", compiled.digest());
    Ok(())
}

fn data(dir: Option<&Path>) -> Result<&Path, String> {
    dir.ok_or_else(|| "no data directory: pass --data or set WF_AGENTS_DATA".to_string())
}

fn config_check(dir: &Path) -> Result<(), String> {
    let c = config::working(dir).map_err(|e| e.to_string())?;
    engine::compile_for(&c)?;
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
        Command::Init => init(d),
        Command::Deliver { work, round, line } => {
            let mut db = engine::database::open(d)?;
            let said = match &line {
                Some(l) => sessions::interject::deliver(&mut db, work, round, l)?,
                None => None,
            };
            // Once the budget of the work's stretch is used up while the round runs, it is asked to pause.
            let open: bool = db
                .conn()
                .query_row(
                    "SELECT count(*) > 0 FROM rounds WHERE work = ?1 AND n = ?2 AND ended_at IS NULL",
                    [work, round],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            let budget = match config::committed(d) {
                Ok(c) if open => engine::limits::used_up(&c.config, db.conn(), work, Some(db::now()))?,
                _ => None,
            };
            if let Some(out) = hook_output(said, budget.map(engine::limits::ask_to_pause)) {
                println!("{out}");
            }
            Ok(())
        }
        Command::Run { once, port } => run(d, once, port),
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
                let shape = || format!("--depends-on is N:blocking:what or N:parallel:what it needs: {x}");
                let mut parts = x.splitn(3, ':');
                let (Some(n), Some(kind), Some(needs)) = (parts.next(), parts.next(), parts.next()) else {
                    return Err(shape());
                };
                let kind = kind.trim();
                if !matches!(kind, "blocking" | "parallel") {
                    return Err(shape());
                }
                deps.push(owner::Dependency {
                    on: n.trim().parse::<i64>().map_err(|e| format!("{n}: {e}"))?,
                    kind: kind.to_string(),
                    needs: needs.trim().to_string(),
                });
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
        Command::Item(ItemCommand::Depend { item, on, needs, kind }) => {
            owner::depend(d, item, &owner::Dependency { on, kind, needs })
        }
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
        Command::Answer {
            ask,
            text,
            why,
            reply,
            choice,
        } => {
            let reply: Option<serde_json::Value> = match reply {
                Some(r) => Some(serde_json::from_str(&r).map_err(|e| format!("--reply: {e}"))?),
                None => None,
            };
            print(owner::answer(
                d,
                owner::Answer {
                    ask,
                    text,
                    why,
                    reply,
                    choice,
                },
            ))
        }
        Command::Architect(ArchitectCommand::Say { text, project, item }) => {
            print(owner::say_to_architect(d, project.as_deref(), item, &text))
        }
        Command::DesignCards { all } => print(owner::design_cards(d, all)),
        Command::DesignCard { id } => print(owner::design_card(d, id)),
        Command::Rule {
            design_card,
            confirm,
            disagree,
            choose,
        } => print(owner::rule(
            d,
            owner::Ruling {
                design_card,
                confirm,
                disagree,
                choose,
            },
        )),
        Command::Say { work, text, to } => print(owner::say(d, work, &to, &text)),
        Command::Pause { work, why } => print(owner::pause(d, work, &why)),
        Command::Resume { work } => print(owner::resume(d, work)),
        Command::Status => print(owner::status(d)),
        Command::History { work, full } => print(owner::history(d, work, full)),
        Command::Message { work, json } => message(d, work, json),
        Command::Counters { work } => print(owner::counters(d, work)),
        Command::Anomalies => print(owner::anomalies(d)),
        Command::Cost { work } => print(owner::cost(d, work)),
    }
}

/// What the hook after a tool use gives the session: the interjections handed in, as the session manager wrote them for
/// the command line, with the ask to pause after them; none when there is neither.
fn hook_output(said: Option<String>, ask: Option<String>) -> Option<String> {
    let Some(ask) = ask else { return said };
    let mut out: serde_json::Value = said.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_else(
        || serde_json::json!({ "hookSpecificOutput": { "hookEventName": "PostToolUse", "additionalContext": "" } }),
    );
    let context = &mut out["hookSpecificOutput"]["additionalContext"];
    let before = context.as_str().unwrap_or_default().to_string();
    *context = serde_json::Value::String(if before.is_empty() {
        ask
    } else {
        format!("{before}\n\n{ask}")
    });
    Some(out.to_string())
}

/// Makes the database of a data directory that passes the check the engine makes when it starts: a git repository
/// with a commit, whose committed configuration is valid. A place that is not one gets nothing made in it.
fn init(d: &Path) -> Result<(), String> {
    let committed = config::committed(d).map_err(|e| e.to_string())?;
    // The compile of the flow is the slow part of the check, and where `create` refuses anyway it is left out.
    if !engine::database::exists(d) {
        engine::compile_for(&committed.config)?;
    }
    let path = engine::database::create(d)?;
    println!("created {}", path.display());
    Ok(())
}

/// How the engine stands, for the page; what the owner asked and the work whose round runs are the control's, which
/// fills them in as they are when the status is told.
fn status(e: &mut engine::Engine, last: Option<&api::StepTaken>, started_at: i64) -> Result<api::Status, String> {
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
        paused: true,
        keep_warm: true,
        started: Vec::new(),
        releasing: Vec::new(),
        architects_started: Vec::new(),
        running: None,
        stepping: None,
        last_step: last.cloned(),
        slots_used: slots - e.free_slots()?,
        slots,
        flow_version: e.flow_now,
        config_commit: e.config.commit.clone(),
        repos: e
            .config
            .config
            .projects
            .iter()
            .map(|(name, p)| (name.clone(), p.repo.clone()))
            .collect(),
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

/// Runs the engine on the data directory with `claude -p` and the gh CLI, and serves the owner's page beside it. Every
/// work starts paused, whatever the owner had started before the engine stopped: the owner starts them on the page;
/// a work's own pause is kept. Between two steps the loop answers what the page asked, hands the session manager the
/// new sessions the owner asked for, lifts the pause of the works the owner started once each may hold a slot, keeps
/// waiting sessions warm unless the owner switched that off while every work is paused, tells how the engine stands,
/// and takes the round of a work the owner asked for, or else the next step of the works that go on: every work but
/// those paused by themselves, or while every work is paused, those the owner started. So a pause takes effect as
/// soon as the round under way has ended.
fn run(d: &Path, once: bool, port: u16) -> Result<(), String> {
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
    let control = Arc::new(api::Control::default());
    // The owner's switch that keeps waiting sessions warm while every work is paused holds across restarts.
    control.set_keep_warm(api::keep_warm_paused(e.db.conn())?);
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
    // So does which work's round runs: the page shows it running, and what stops once it ends.
    let (told, running) = (Arc::clone(&bus), Arc::clone(&control));
    e.busy = Box::new(move |work| {
        running.set_running(work);
        if let Some(s) = told.status() {
            told.publish(api::Event::Status {
                status: Box::new(running.status(s)),
            });
        }
    });
    let first = status(&mut e, None, started_at)?;
    bus.publish(api::Event::Status {
        status: Box::new(control.status(first)),
    });
    let port = web::start(web::Options {
        data: d.to_path_buf(),
        db: config::database(d),
        port,
        bus: Arc::clone(&bus),
        control: Arc::clone(&control),
    })?;
    (e.say)(&format!(
        "the page is at http://127.0.0.1:{port}/; every work is paused until the owner starts them there"
    ));
    // The owner, or whoever installs a new command, stops the engine between steps by creating this file.
    let stop = d.join("stop");
    let _ = std::fs::remove_file(&stop);
    let idle = Duration::from_secs(10);
    let mut last: Option<api::StepTaken> = None;
    // The work the last step took, from which the next step goes round the works.
    let mut after: Option<i64> = None;
    while !stop.exists() {
        let asked = control.asked();
        for work in control.take_new_sessions() {
            e.sessions.ask_new(work);
        }
        e.reload_config();
        match e.release_started(&asked.releasing, &asked.architects_releasing) {
            Ok((done, architects)) => control.released(&done, &architects),
            Err(x) => (e.say)(&format!("lifting the pause of the works the owner started: {x}")),
        }
        // The sessions of works that wait are kept warm between steps, never beside a round.
        let pause = asked.paused.then(|| engine::GlobalPause {
            warm: asked.keep_warm,
            runs: asked.started.clone(),
            architects: asked.architects_started.clone(),
        });
        if let Err(x) = e.keep_warm(db::now(), pause.as_ref()) {
            (e.say)(&format!("keeping waiting sessions warm: {x}"));
        }
        let now = status(&mut e, last.as_ref(), started_at)?;
        bus.publish(api::Event::Status {
            status: Box::new(control.status(now)),
        });
        match asked.next() {
            api::Next::Step(work) => {
                let s = e.step_work(work)?;
                let names: Vec<String> = s.rounds.iter().map(ToString::to_string).collect();
                let stepped = s.work.unwrap_or(work);
                (e.say)(&format!(
                    "the round of work {work}, as the owner asked: work {stepped}, rounds {}",
                    names.join(", ")
                ));
                last = Some(api::StepTaken {
                    at: db::now(),
                    asked: work,
                    work: stepped,
                    rounds: s.rounds.iter().filter(|r| r.work == stepped).map(|r| r.n).collect(),
                    cell: s.cell,
                });
                control.stepped(Some(work));
            }
            api::Next::Architect(project) => {
                let s = e.step_architect(&project)?;
                let names: Vec<String> = s.rounds.iter().map(ToString::to_string).collect();
                (e.say)(&format!(
                    "the round of the architect of {project}, as the owner asked: work {}, rounds {}",
                    s.work.map_or_else(|| "none".to_string(), |w| w.to_string()),
                    names.join(", ")
                ));
                if let Some(stepped) = s.work {
                    last = Some(api::StepTaken {
                        at: db::now(),
                        asked: stepped,
                        work: stepped,
                        rounds: s.rounds.iter().filter(|r| r.work == stepped).map(|r| r.n).collect(),
                        cell: s.cell,
                    });
                }
                control.stepped(s.work);
            }
            api::Next::Pass => {
                let s = e.step(after)?;
                if s.work.is_none() {
                    control.wait(&asked, idle);
                }
                after = s.work;
            }
            api::Next::Started => {
                let s = e.step_started(after, &asked.started, &asked.architects_started)?;
                if s.work.is_none() {
                    control.wait(&asked, idle);
                }
                after = s.work;
            }
            api::Next::Wait => control.wait(&asked, idle),
        }
    }
    let _ = std::fs::remove_file(&stop);
    eprintln!("{} stopped: {} exists", db::now() / 1000, stop.display());
    Ok(())
}
