//! The commands that talk to a running engine over the owner's interface, for the owner and their assistant to manage
//! items from a terminal: each sends one request on the loopback address and says, in plain text a person and an
//! agent can both read, what came of it. A refusal is said on the error stream in the engine's own words, and the
//! command fails.

use std::net::{Ipv4Addr, SocketAddr};
use std::process::ExitCode;

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::{Method, Request, StatusCode, header};
use hyper_util::rt::TokioIo;
use serde::Serialize;
use serde::de::DeserializeOwned;
use web::works::{
    Act, Added, Back, Call, CellState, Items, Limit, Maker, NewItem, NewWrite, Now, Pauser, Person, Refusal, Run,
    Spent, Step, Waited, WorkPage, Writer,
};

/// What a command asks of the engine.
pub enum Asked {
    Add(NewItem),
    List,
    Show(u64),
    Act(u64, Act),
    /// A cancel, which says afterwards what the work is: cancelled, or ended otherwise before it could be.
    Cancel(u64),
}

/// Asks the engine on the loopback address at this port, and prints what came of it.
pub fn ask(port: u16, asked: Asked) -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("the command cannot start: {e}");
            return ExitCode::FAILURE;
        }
    };
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let said: Result<String, String> = runtime.block_on(async move {
        match asked {
            Asked::Add(item) => {
                let added: Added = send(address, Method::POST, "/api/v1/items", Some(&item)).await?;
                Ok(format!(
                    "Added item {} with its work {}, paused: let it on with `wf-agents resume {}`.",
                    added.item, added.work, added.work
                ))
            }
            Asked::List => {
                let items: Items = send(address, Method::GET, "/api/v1/items", None::<&()>).await?;
                Ok(list(&items))
            }
            Asked::Show(work) => {
                let page: WorkPage = send(address, Method::GET, &format!("/api/v1/works/{work}"), None::<&()>).await?;
                Ok(show(&page))
            }
            Asked::Act(work, act) => {
                let done = done(work, &act);
                let path = format!("/api/v1/works/{work}/acts");
                send::<(), _>(address, Method::POST, &path, Some(&act)).await?;
                Ok(done)
            }
            Asked::Cancel(work) => {
                let page = format!("/api/v1/works/{work}");
                let before: WorkPage = send(address, Method::GET, &page, None::<&()>).await?;
                if !matches!(before.summary.now, Now::Merged | Now::Cancelled) {
                    let path = format!("/api/v1/works/{work}/acts");
                    send::<(), _>(address, Method::POST, &path, Some(&Act::Cancel {})).await?;
                }
                let after: WorkPage = send(address, Method::GET, &page, None::<&()>).await?;
                match after.summary.now {
                    Now::Cancelled => Ok(format!("Work {work} is cancelled.")),
                    Now::Merged => Err(format!("Work {work} is not cancelled: it has merged.")),
                    now => Err(format!("Work {work} is not cancelled: it {}.", self::now(&now))),
                }
            }
        }
    });
    match said {
        Ok(said) => {
            println!("{said}");
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}

/// The new item, with what is written for it: each text, then each file's content, then each choice, in the order
/// given. Each is `BLOCK=WHAT`.
pub fn new_item(
    title: String,
    level: Option<String>,
    flow: String,
    depends_on: Vec<u64>,
    texts: &[String],
    files: &[String],
    choices: &[String],
) -> Result<NewItem, String> {
    let mut writes = Vec::new();
    for text in texts {
        let (block, content) = split(text, "--write")?;
        writes.push(NewWrite {
            block,
            content,
            value: None,
        });
    }
    for file in files {
        let (block, path) = split(file, "--write-file")?;
        let content = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
        writes.push(NewWrite {
            block,
            content,
            value: None,
        });
    }
    for choice in choices {
        let (block, value) = split(choice, "--choose")?;
        writes.push(NewWrite {
            block,
            content: String::new(),
            value: Some(value),
        });
    }
    Ok(NewItem {
        title,
        level,
        depends_on,
        flow,
        writes,
    })
}

/// `BLOCK=WHAT` as its block and what is written of it.
fn split(given: &str, flag: &str) -> Result<(String, String), String> {
    match given.split_once('=') {
        Some((block, what)) if !block.is_empty() => Ok((block.to_string(), what.to_string())),
        _ => Err(format!("{flag} takes BLOCK=WHAT, and was given {given:?}")),
    }
}

/// Sends a request, and gives what the engine answered as `T`, or its refusal's words.
async fn send<T: DeserializeOwned, B: Serialize>(
    address: SocketAddr,
    method: Method,
    path: &str,
    body: Option<&B>,
) -> Result<T, String> {
    let unreachable =
        |e: &dyn std::fmt::Display| format!("No engine answers at {address}: {e}. One starts with `wf-agents run`.");
    // Once the request is sent, the engine may have taken it before the answer was lost.
    let lost = |e: &dyn std::fmt::Display| {
        format!(
            "The engine at {address} stopped answering after it was asked: {e}. What was asked may have been taken: \
             look with `wf-agents list` or `wf-agents show` before asking it again."
        )
    };
    let stream = tokio::net::TcpStream::connect(address)
        .await
        .map_err(|e| unreachable(&e))?;
    let (mut sender, connection) =
        (hyper::client::conn::http1::handshake(TokioIo::new(stream)).await).map_err(|e| unreachable(&e))?;
    tokio::spawn(connection);
    let body = match body {
        Some(body) => serde_json::to_vec(body).map_err(|e| format!("the request cannot be written: {e}"))?,
        None => Vec::new(),
    };
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, address.to_string())
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(body)))
        .map_err(|e| format!("the request cannot be made: {e}"))?;
    let response = sender.send_request(request).await.map_err(|e| lost(&e))?;
    let status = response.status();
    let body = (response.into_body().collect().await).map_err(|e| lost(&e))?.to_bytes();
    if !status.is_success() {
        return Err(match serde_json::from_slice::<Refusal>(&body) {
            Ok(refusal) => refusal.error,
            Err(_) => format!("The engine answered {status}."),
        });
    }
    // An answer with no content is read as JSON's null, which is what `()` reads from.
    let body: &[u8] = if status == StatusCode::NO_CONTENT {
        b"null"
    } else {
        &body
    };
    serde_json::from_slice(body).map_err(|e| format!("The engine's answer cannot be read: {e}."))
}

/// What a command that acts says once it is done.
fn done(work: u64, act: &Act) -> String {
    match act {
        Act::Write { block, .. } => format!("Wrote {block} for work {work}."),
        Act::LetGo { cell } => format!("Cell {cell} of work {work} goes on."),
        Act::Pause {} => format!("Work {work} is paused."),
        Act::Resume {} => format!("Work {work} goes on."),
        Act::Cancel {} => format!("Work {work} is cancelled."),
        Act::Answer { call, .. } => format!("Call {call} of work {work} has its result."),
    }
}

/// Every item, a line each, with what its work does now.
fn list(items: &Items) -> String {
    if items.items.is_empty() {
        return "No items.".to_string();
    }
    let lines: Vec<String> = (items.items.iter())
        .map(|item| {
            let after = if item.depends_on.is_empty() {
                String::new()
            } else {
                let on: Vec<String> = item.depends_on.iter().map(u64::to_string).collect();
                format!(", after {}", on.join(", "))
            };
            let work = &item.work;
            format!(
                "Item {} \"{}\" at {}, work {} on {}{after}: {}",
                item.number,
                item.title,
                level(item.level.as_deref()),
                work.number,
                work.flow,
                now(&work.now)
            )
        })
        .collect();
    lines.join("\n")
}

/// One work: what it does now, its cells and blocks, what it took, and its runs.
fn show(page: &WorkPage) -> String {
    let summary = &page.summary;
    let mut lines = vec![
        format!(
            "Work {} of item {} \"{}\" at {}, on the flow {}.",
            summary.number,
            summary.item,
            page.title,
            level(page.level.as_deref()),
            summary.flow
        ),
        format!("Now: {}.", now(&summary.now)),
        format!("Paused: {}.", if summary.paused { "yes" } else { "no" }),
        format!("Spent: {}.", spent(&summary.spent)),
        "Cells:".to_string(),
    ];
    let runs: Vec<&Run> = (page.steps.iter())
        .filter_map(|step| match step {
            Step::Run(run) => Some(run),
            _ => None,
        })
        .collect();
    for cell in &summary.cells {
        let run = |run: Option<u64>| run.map_or_else(String::new, |run| format!(" in run {run}"));
        let failure = |open: Option<u64>| {
            let run = open.and_then(|open| runs.iter().find(|run| run.run == open));
            run.and_then(|run| failure(run))
                .map_or_else(String::new, |why| format!(": {why}"))
        };
        let state = match cell.state {
            CellState::Running => format!("a round under way{}", run(cell.open)),
            CellState::Resting => format!("resting{}", run(cell.open)),
            CellState::Failed => format!(
                "its last round failed{}, and is given again once the work is let on{}",
                run(cell.open),
                failure(cell.open)
            ),
            CellState::Skipped => "skipped".to_string(),
            CellState::NotYet => "not yet".to_string(),
            CellState::Again => format!("runs again, its record being run {}", run_or_none(cell.record)),
            CellState::Done => format!("done, its record being run {}", run_or_none(cell.record)),
        };
        lines.push(format!("  {}: {state}", cell.cell));
    }
    lines.push("Blocks:".to_string());
    for block in &page.blocks {
        let Some(latest) = block.versions.last() else {
            continue;
        };
        let count = block.versions.len();
        let what = match &latest.value {
            Some(value) => value.clone(),
            None => format!("{} characters", latest.chars),
        };
        let versions = if count == 1 {
            "1 version".to_string()
        } else {
            format!("{count} versions")
        };
        lines.push(format!(
            "  {}: {versions}, the latest {what}, by {}",
            block.name,
            writer(&latest.by)
        ));
    }
    lines.push("Runs:".to_string());
    for run in runs {
        let rounds = if run.rounds.len() == 1 {
            "1 round".to_string()
        } else {
            format!("{} rounds", run.rounds.len())
        };
        let calls: Vec<String> = run.calls.iter().map(|call| call.call.to_string()).collect();
        let calls = if calls.is_empty() {
            String::new()
        } else {
            format!(", calls {}", calls.join(", "))
        };
        let end = match (&run.end, failure(run)) {
            (None, Some(why)) => format!("open, its last round failed: {why}"),
            (None, None) => "open".to_string(),
            (Some(web::works::RunEnd::Finished { .. }), _) => "finished".to_string(),
            (Some(web::works::RunEnd::Abandoned { .. }), _) => "abandoned".to_string(),
        };
        lines.push(format!("  run {} of {}: {rounds}{calls}, {end}", run.run, run.cell));
    }
    lines.join("\n")
}

/// An item's level, in words.
fn level(level: Option<&str>) -> String {
    match level {
        Some(level) => format!("level {level}"),
        None => "no level, added before items had levels".to_string(),
    }
}

/// Why the run's last round failed, where it did: nothing came back of it.
fn failure(run: &Run) -> Option<&str> {
    match &run.rounds.last()?.back {
        Some(Back::Failed { why }) => Some(why),
        _ => None,
    }
}

fn run_or_none(run: Option<u64>) -> String {
    run.map_or_else(|| "none".to_string(), |run| run.to_string())
}

/// What the work does now, as a clause.
fn now(now: &Now) -> String {
    match now {
        Now::Fault { words } => format!("it cannot go on, for its records are not those of a work: {words}"),
        Now::Merged => "merged".to_string(),
        Now::Cancelled => "cancelled".to_string(),
        Now::Round { cell, run, round, .. } => format!("a round of {cell} is under way, round {round} of run {run}"),
        Now::Paused { by: Pauser::Owner, .. } => "paused".to_string(),
        Now::Paused {
            by: Pauser::Engine,
            why: Some(why),
        } => format!("paused by the engine: {why}"),
        Now::Paused {
            by: Pauser::Engine,
            why: None,
        } => "paused by the engine, which gave no reason".to_string(),
        Now::Waits { at, on } => {
            let on: Vec<String> = on.iter().map(waited).collect();
            format!("waits at {at} for {}", on.join("; and for "))
        }
        Now::Next { cell } => format!("about to go on at {cell}"),
    }
}

/// What a work waits for, as a clause.
fn waited(waited: &Waited) -> String {
    match waited {
        Waited::Call {
            call,
            who: Some(who),
            what,
        } => format!("the {} to answer call {call}: {}", person(*who), called(what)),
        Waited::Call { call, who: None, what } => format!("call {call}, {}", called(what)),
        Waited::Precondition { words, by, .. } => {
            let by: Vec<String> = by.iter().map(maker).collect();
            format!("{}: {words}", by.join(" or "))
        }
        Waited::Limit { runs } => {
            format!("the owner to let it go on, its last {runs} runs having each been for what a cell after it wrote")
        }
    }
}

/// What a call is, as a clause.
fn called(call: &Call) -> String {
    match call {
        Call::Tool { name, input } => format!("the tool {name} on {input:?}"),
        Call::Question { question, .. } => format!("{question:?}"),
        Call::AtLimit {
            limit: Limit::Returns { sent_back },
        } => format!("its run reached its limit, {sent_back} returns having been sent back"),
        Call::AtLimit {
            limit: Limit::Calls { made },
        } => format!("its run reached its limit, {made} calls having been made"),
    }
}

/// Who can make a precondition hold, as a noun.
fn maker(maker: &Maker) -> String {
    match maker {
        Maker::Owner => "the owner".to_string(),
        Maker::Outside { source } => format!("the outside ({source})"),
        Maker::Cell { cell } => format!("cell {cell}"),
    }
}

fn person(person: Person) -> &'static str {
    match person {
        Person::Owner => "owner",
        Person::Architect => "architect",
    }
}

fn writer(writer: &Writer) -> String {
    match writer {
        Writer::Person => "a person".to_string(),
        Writer::Outside => "the outside".to_string(),
        Writer::Run { run, cell } => format!("{cell} in run {run}"),
    }
}

/// What rounds took, as a clause.
fn spent(spent: &Spent) -> String {
    let rounds = if spent.rounds == 1 {
        "1 round".to_string()
    } else {
        format!("{} rounds", spent.rounds)
    };
    let dollars = spent.usd_micros / 1_000_000;
    let micros = spent.usd_micros % 1_000_000;
    let seconds = spent.working_ms / 1000;
    let tenths = spent.working_ms % 1000 / 100;
    format!("${dollars}.{micros:06} in {rounds}, under way for {seconds}.{tenths} s")
}
