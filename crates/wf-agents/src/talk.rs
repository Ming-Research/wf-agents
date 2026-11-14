//! The commands that talk to a running engine over the owner's interface, for the owner and their assistant to manage
//! items from a terminal: each sends one request on the loopback address and says, in plain text a person and an
//! agent can both read, what came of it. A refusal is said on the error stream in the engine's own words, and the
//! command fails.

use std::net::{Ipv4Addr, SocketAddr};
use std::process::ExitCode;

use driver::attention::Mark;
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::{Method, Request, StatusCode, header};
use hyper_util::rt::TokioIo;
use serde::Serialize;
use serde::de::DeserializeOwned;
use web::attention::{Attention, Line, Stuck};
use web::plan::{Candidates, Field, Graph, Hits, Node, PlanChange, PlanChanges, Row, Standing, Tags};
use web::works::{
    Act, Added, Back, Call, CellKey, CellState, ChangeAsked, Engine, EngineAct, Failure, ItemChanged, ItemChanges,
    ItemHistory, Items, ItemsChanged, Limit, Maker, NewItem, NewWrite, Now, Outage, Pauser, Person, QuestionPage,
    Refusal, Run, SessionWhy, Spent, Stalling, Step, Unrun, Until, VersionPage, Waited, WorkPage, Writer,
};

/// What a command asks of the engine.
pub enum Asked {
    Add(NewItem),
    /// The items a listing gives.
    List(Listing),
    Show(u64),
    Act(u64, Act),
    /// The owner's pause of everything set, or lifted.
    Everything(EngineAct),
    /// A cancel for this reason, which says afterwards what the work is: cancelled, or ended otherwise before it could
    /// be.
    Cancel(u64, String),
    /// Changes of items made together, for this reason.
    Change {
        why: String,
        changes: Vec<ChangeAsked>,
    },
    /// What changed in the plan since the mark.
    Changes {
        since: Option<String>,
    },
    /// The items that could be let on now.
    Candidates,
    /// Where the text is mentioned, at most this many places.
    Search {
        text: String,
        limit: usize,
    },
    /// What the item waits for and what waits for it, one way where it is named, to this depth.
    Graph {
        item: u64,
        way: Option<&'static str>,
        depth: usize,
    },
    /// The tags items carry.
    Tags,
    /// The text of the item's blocks, in short, or of one block, whole where asked.
    Text {
        item: u64,
        block: Option<String>,
        whole: bool,
    },
    /// What needs whoever plans the work since the mark, waiting for it at most this many seconds; with no time, as
    /// long as it takes.
    Wait {
        since: Option<String>,
        longest: Option<u64>,
    },
}

/// How many seconds a request of `wait` with no time of its own is held open before it is asked again with the mark
/// it was answered with: long enough to cost nothing, short enough that an engine gone unanswering is found out.
const HOLD: u64 = 3600;

/// The longest title a line of `wait` carries; the rest is cut.
const TITLE: usize = 60;

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
            Asked::List(listing) => {
                let engine: Engine = send(address, Method::GET, "/api/v1/engine", None::<&()>).await?;
                let items: Items = send(address, Method::GET, "/api/v1/items", None::<&()>).await?;
                Ok(format!("{}\n{}", standing(&engine), list(&items, &listing)))
            }
            Asked::Change { why, changes } => {
                let asked = ItemChanges {
                    why,
                    changes: changes.clone(),
                };
                let done: ItemsChanged = send(address, Method::POST, "/api/v1/items/changes", Some(&asked)).await?;
                let reopened = changes.iter().any(|c| matches!(c, ChangeAsked::Reopen { .. }));
                let works = match reopened {
                    true => Some(send::<Items, ()>(address, Method::GET, "/api/v1/items", None).await?),
                    false => None,
                };
                Ok(changed(&changes, &done.changed, works.as_ref()))
            }
            Asked::Changes { since } => {
                let path = match &since {
                    Some(mark) => format!("/api/v1/plan/changes?since={}", encoded(mark)),
                    None => "/api/v1/plan/changes".to_string(),
                };
                let changes: PlanChanges = send(address, Method::GET, &path, None::<&()>).await?;
                Ok(plan_changes(&changes, since.is_some()))
            }
            Asked::Candidates => {
                let candidates: Candidates = send(address, Method::GET, "/api/v1/plan/candidates", None::<&()>).await?;
                Ok(candidates_said(&candidates))
            }
            Asked::Search { text, limit } => {
                let path = format!("/api/v1/plan/search?text={}&limit={limit}", encoded(&text));
                let hits: Hits = send(address, Method::GET, &path, None::<&()>).await?;
                Ok(hits_said(&hits, &text))
            }
            Asked::Graph { item, way, depth } => {
                let mut path = format!("/api/v1/plan/graph/{item}?depth={depth}");
                if let Some(way) = way {
                    path.push_str(&format!("&way={way}"));
                }
                let graph: Graph = send(address, Method::GET, &path, None::<&()>).await?;
                Ok(graph_said(&graph, depth, way))
            }
            Asked::Tags => {
                let tags: Tags = send(address, Method::GET, "/api/v1/plan/tags", None::<&()>).await?;
                Ok(tags_said(&tags))
            }
            Asked::Text { item, block, whole } => {
                let items: Items = send(address, Method::GET, "/api/v1/items", None::<&()>).await?;
                let Some(work) = items.items.iter().find(|i| i.number == item).map(|i| i.work.number) else {
                    return Err(format!("There is no item #{item}."));
                };
                let page: WorkPage = send(address, Method::GET, &format!("/api/v1/works/{work}"), None::<&()>).await?;
                let mut texts = Vec::new();
                for (name, at) in texts_of(&page, block.as_deref())? {
                    let path = format!("/api/v1/works/{work}/versions/{}/{at}", encoded(&name));
                    let version: VersionPage = send(address, Method::GET, &path, None::<&()>).await?;
                    texts.push((name, version.content));
                }
                Ok(text_said(item, &page, &texts, whole, block.is_some()))
            }
            Asked::Everything(act) => {
                send::<(), _>(address, Method::POST, "/api/v1/engine/acts", Some(&act)).await?;
                let engine: Engine = send(address, Method::GET, "/api/v1/engine", None::<&()>).await?;
                Ok(standing(&engine))
            }
            Asked::Show(work) => {
                let page: WorkPage = send(address, Method::GET, &format!("/api/v1/works/{work}"), None::<&()>).await?;
                Ok(show(&page))
            }
            Asked::Act(work, act) => {
                let resumed = act == Act::Resume {};
                let done = done(work, &act);
                let path = format!("/api/v1/works/{work}/acts");
                send::<(), _>(address, Method::POST, &path, Some(&act)).await?;
                if !resumed {
                    return Ok(done);
                }
                // A work let on that the pause of everything holds back is said to be held, not to go on.
                let page: WorkPage = send(address, Method::GET, &format!("/api/v1/works/{work}"), None::<&()>).await?;
                Ok(match page.summary.now {
                    Now::Everything {} => format!(
                        "Work {work} is let on, and held back by the pause of everything until `wf-agents resume --all`."
                    ),
                    _ => done,
                })
            }
            Asked::Cancel(work, why) => {
                let page = format!("/api/v1/works/{work}");
                let before: WorkPage = send(address, Method::GET, &page, None::<&()>).await?;
                if !matches!(before.summary.now, Now::Merged | Now::Cancelled) {
                    let path = format!("/api/v1/works/{work}/acts");
                    let act = Act::Cancel { why: Some(why) };
                    send::<(), _>(address, Method::POST, &path, Some(&act)).await?;
                }
                let after: WorkPage = send(address, Method::GET, &page, None::<&()>).await?;
                match after.summary.now {
                    Now::Cancelled => Ok(format!("Work {work} is cancelled.")),
                    Now::Merged => Err(format!("Work {work} is not cancelled: it has merged.")),
                    now => Err(format!("Work {work} is not cancelled: it {}.", self::now(&now))),
                }
            }
            Asked::Wait { since, longest } => {
                let marked = since.is_some();
                // Read here, so that a mark that is none is refused in its own words and never put in a path.
                let mut since = (since.as_deref().map(Mark::read).transpose())
                    .map_err(|unreadable| format!("{}.", unreadable.0))?;
                loop {
                    let mut path = format!("/api/v1/attention?wait={}", longest.unwrap_or(HOLD));
                    if let Some(mark) = &since {
                        path.push_str(&format!("&since={mark}"));
                    }
                    let answer: Attention = send(address, Method::GET, &path, None::<&()>).await?;
                    if longest.is_some() || !answer.lines.is_empty() {
                        break Ok(attention(&answer, marked, clock()));
                    }
                    since = Some(Mark::read(&answer.mark).map_err(|_| {
                        format!(
                            "The engine answered with a mark that cannot be read: {:?}.",
                            answer.mark
                        )
                    })?);
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

/// What is written for a new item, each `BLOCK=WHAT`: texts, the contents of files, and choices.
pub struct Written<'a> {
    pub texts: &'a [String],
    pub files: &'a [String],
    pub choices: &'a [String],
}

/// The new item, with what is written for it: each text, then each file's content, then each choice, in the order
/// given. It is added paused.
pub fn new_item(
    title: String,
    level: Option<String>,
    project: String,
    flow: String,
    depends_on: Vec<u64>,
    tags: Vec<String>,
    written: Written<'_>,
) -> Result<NewItem, String> {
    let Written { texts, files, choices } = written;
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
        project,
        depends_on,
        tags,
        flow,
        writes,
    })
}

/// The changes `wf-agents change` is given, each `ITEM:KIND` or `ITEM:KIND=VALUE`, in the order given.
pub fn changes(given: &[String]) -> Result<Vec<ChangeAsked>, String> {
    let usage = |given: &str, why: &str| {
        format!(
            "{given:?} is no change: {why}. A change is ITEM:KIND or ITEM:KIND=VALUE, the kind one of title=TEXT, \
             level=LEVEL, depends-on=ITEM, no-longer-depends-on=ITEM, tag=TAG, untag=TAG, cancel and reopen"
        )
    };
    let mut changes = Vec::new();
    for change in given {
        let Some((item, rest)) = change.split_once(':') else {
            return Err(usage(change, "it names no item before a colon"));
        };
        let item: u64 = (item.trim().trim_start_matches('#').parse())
            .map_err(|_| usage(change, "what is before the colon is no item's number"))?;
        let (kind, value) = match rest.split_once('=') {
            Some((kind, value)) => (kind, Some(value.to_string())),
            None => (rest, None),
        };
        let number = |value: Option<String>| -> Result<u64, String> {
            (value
                .as_deref()
                .map(|v| v.trim().trim_start_matches('#').parse::<u64>()))
            .and_then(Result::ok)
            .ok_or_else(|| usage(change, "it needs an item's number after ="))
        };
        let text = |value: Option<String>| value.ok_or_else(|| usage(change, "it needs a value after ="));
        let none = |value: &Option<String>| match value {
            Some(_) => Err(usage(change, "it takes no value")),
            None => Ok(()),
        };
        changes.push(match kind {
            "title" => ChangeAsked::Title {
                item,
                title: text(value)?,
            },
            "level" => ChangeAsked::Level {
                item,
                level: text(value)?,
            },
            "depends-on" => ChangeAsked::DependsOn {
                item,
                on: number(value)?,
            },
            "no-longer-depends-on" => ChangeAsked::NoLongerDependsOn {
                item,
                on: number(value)?,
            },
            "tag" => ChangeAsked::Tag {
                item,
                tag: text(value)?,
            },
            "untag" => ChangeAsked::Untag {
                item,
                tag: text(value)?,
            },
            "cancel" => {
                none(&value)?;
                ChangeAsked::Cancel { item }
            }
            "reopen" => {
                none(&value)?;
                ChangeAsked::Reopen { item }
            }
            _ => return Err(usage(change, "its kind is none of these")),
        });
    }
    Ok(changes)
}

/// A text as a query of a path carries it: every byte that is not a letter, a digit or one of `-._~` written as `%XX`.
fn encoded(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
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

/// A question as it stands, in lines: its name, cell and version, the first line of what it asks, its options and the
/// owner's answer; of one withdrawn, why.
fn question(q: &QuestionPage) -> Vec<String> {
    let by = if q.engine { ", asked by the engine" } else { "" };
    let waits = if q.waits { ", waits" } else { "" };
    let head = format!("  {} (of {}, version {}{by}{waits})", q.name, q.cell, q.version);
    if let Some(why) = &q.withdrawn {
        return vec![format!("{head}: withdrawn: {why}")];
    }
    let first = q
        .ask
        .as_deref()
        .and_then(|ask| ask.lines().find(|l| !l.trim().is_empty()));
    let mut lines = vec![format!("{head}: {}", first.unwrap_or_default())];
    for (i, way) in q.options.iter().enumerate() {
        let n = i as u64 + 1;
        let recommended = if q.recommends == Some(n) { " (recommended)" } else { "" };
        lines.push(format!("    {n}. {}: {}{recommended}", way.does, way.follows));
    }
    lines.push(match &q.answer {
        Some(a) => {
            let why = a.why.as_deref().map_or_else(String::new, |why| format!(": \"{why}\""));
            format!("    answered {}{why}", a.option)
        }
        None => format!(
            "    unanswered: wf-agents answer <work> {} {} <option>",
            q.name, q.version
        ),
    });
    lines
}

/// What a command that acts says once it is done.
fn done(work: u64, act: &Act) -> String {
    match act {
        Act::Write { block, .. } => format!("Wrote {block} for work {work}."),
        Act::LetGo { cell } => format!("Cell {cell} of work {work} goes on."),
        Act::Pause {} => format!("Work {work} is paused."),
        Act::Resume {} => format!("Work {work} goes on."),
        Act::Cancel { .. } => format!("Work {work} is cancelled."),
        Act::Result { call, .. } => format!("Call {call} of work {work} has its result."),
        Act::Answer {
            question,
            version,
            option,
            ..
        } => format!("Version {version} of the question {question} of work {work} is answered with option {option}."),
        Act::NewSession { line } => format!("The line {line} of work {work} begins a new session at its next round."),
    }
}

/// The engine as a whole, in a line: whether everything is paused and by whom, and its slots.
fn standing(engine: &Engine) -> String {
    let paused = match engine.paused.map(|paused| paused.by) {
        Some(Pauser::Owner) => {
            "Every work is paused: none begins a round but to answer a note, until `wf-agents resume --all`"
        }
        Some(Pauser::Engine) => STARTED_HELD,
        None => "Works go on",
    };
    format!("{paused}; {} of {} slots taken.", engine.taken, engine.slots)
}

/// What the engine's own pause of everything, as it started with works going on, is, in words.
const STARTED_HELD: &str = "The engine started with works going on and holds every work until you have looked: none \
                            begins a round but to answer a note, until `wf-agents resume --all`";

/// The most items `list` gives in one answer.
const LISTED: usize = 100;

/// Which items a list gives: those carrying the tag alone, where one is given; from the item given; and those that
/// ended, merged or cancelled, only where `all`.
#[derive(Clone, Debug, Default)]
pub struct Listing {
    pub tag: Option<String>,
    pub from: Option<u64>,
    pub all: bool,
}

/// Every item a list gives, a line each, with what its work does now, at most [`LISTED`]: how many more there are, and
/// how to see them, is said, and how many ended are left out. An item that waits for good for a cancelled item is
/// marked so.
fn list(items: &Items, listing: &Listing) -> String {
    if items.items.is_empty() {
        return "No items.".to_string();
    }
    let (tag, from) = (listing.tag.as_deref(), listing.from);
    let cancelled: Vec<u64> = (items.items.iter())
        .filter(|item| item.work.now == Now::Cancelled)
        .map(|item| item.number)
        .collect();
    let mut lines: Vec<String> = (items.outage.iter())
        .filter(|o| o.holds)
        .map(|o| format!("No agent's round of any work begins: {}.", outage(o)))
        .collect();
    let ended = |item: &&web::works::Item| matches!(item.work.now, Now::Merged | Now::Cancelled);
    let asked: Vec<_> = (items.items.iter())
        .filter(|item| tag.is_none_or(|tag| item.tags.iter().any(|t| t == tag)))
        .filter(|item| from.is_none_or(|from| item.number >= from))
        .collect();
    let chosen: Vec<_> = asked
        .iter()
        .copied()
        .filter(|item| listing.all || !ended(item))
        .collect();
    let hidden = asked.len() - chosen.len();
    if chosen.is_empty() {
        let open = if listing.all { "" } else { " that has not ended" };
        lines.push(match tag {
            Some(tag) => format!("No item{open} carries the tag {tag}{}.", from_words(from)),
            None => format!("No item{open}{}.", from_words(from)),
        });
    }
    for item in chosen.iter().take(LISTED) {
        let after = if item.depends_on.is_empty() {
            String::new()
        } else {
            let on: Vec<String> = item.depends_on.iter().map(|on| format!("#{on}")).collect();
            format!(", after {}", on.join(", "))
        };
        let tags = match item.tags.is_empty() {
            true => String::new(),
            false => format!(", tagged {}", item.tags.join(", ")),
        };
        let work = &item.work;
        let ended = matches!(work.now, Now::Merged | Now::Cancelled);
        let on: Vec<String> = (item.depends_on.iter())
            .filter(|on| !ended && cancelled.contains(on))
            .map(|on| format!("#{on}"))
            .collect();
        let good = match on.is_empty() {
            true => String::new(),
            false => format!(" It waits for good on {}, cancelled.", on.join(", ")),
        };
        lines.push(format!(
            "#{} \"{}\" at {}, W{} on {}{after}{tags}: {}{good}",
            item.number,
            item.title,
            level(item.level.as_deref()),
            work.number,
            work.flow,
            now(&work.now)
        ));
    }
    let tag = tag.map_or_else(String::new, |tag| format!(" --tag {tag}"));
    let all = if listing.all { " --all" } else { "" };
    if let Some(next) = chosen.get(LISTED) {
        lines.push(format!(
            "And {} more: wf-agents list{tag}{all} --from {}",
            chosen.len() - LISTED,
            next.number
        ));
    }
    if hidden > 0 {
        let items = if hidden == 1 {
            "item that ended, merged or cancelled"
        } else {
            "items that ended, merged or cancelled"
        };
        let from = from.map_or_else(String::new, |from| format!(" --from {from}"));
        lines.push(format!("And {hidden} {items}: wf-agents list{tag} --all{from}"));
    }
    lines.join("\n")
}

/// From where a list begins, as the end of a sentence.
fn from_words(from: Option<u64>) -> String {
    from.map_or_else(String::new, |from| format!(" from #{from} on"))
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
        format!(
            "Tags: {}.",
            if page.tags.is_empty() {
                "none".to_string()
            } else {
                page.tags.join(", ")
            }
        ),
        format!("Spent: {}.", spent(&summary.spent)),
        format!("Item {} {}", summary.item, history(&page.history)),
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
    if !page.questions.is_empty() {
        lines.push("Questions:".to_string());
    }
    for q in &page.questions {
        lines.extend(question(q));
    }
    for step in &page.steps {
        match step {
            Step::Cancelled { time, why, .. } => {
                let why = why
                    .as_deref()
                    .map_or_else(|| "no reason was given".to_string(), |why| format!("\"{why}\""));
                lines.push(format!("Cancelled at {}: {why}.", utc(*time)));
            }
            Step::Reopened { time, why, .. } => lines.push(format!("Reopened at {}: \"{why}\".", utc(*time))),
            _ => {}
        }
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
        let last = run.rounds.iter().rev().find(|round| round.ended.is_some());
        let context = last
            .and_then(|round| round.context)
            .map_or_else(String::new, |context| {
                format!(", its last round ending with a context of {context} tokens")
            });
        let compactions: u32 = run.rounds.iter().map(|round| round.compactions).sum();
        let compacted = match compactions {
            0 => String::new(),
            1 => ", its session compacted once".to_string(),
            n => format!(", its session compacted {n} times"),
        };
        lines.push(format!(
            "  run {} of {}: {rounds}{calls}{context}{compacted}, {end}",
            run.run, run.cell
        ));
    }
    let sessions: Vec<String> = (page.steps.iter())
        .filter_map(|step| match step {
            Step::Session { at, line, why, .. } => Some(format!("  {line} at {at}: {}", session_why(why))),
            _ => None,
        })
        .collect();
    if !sessions.is_empty() {
        lines.push("Sessions:".to_string());
        lines.extend(sessions);
    }
    lines.join("\n")
}

/// What an item was as it was added and each change since, as the end of a sentence about it, with a line for each
/// change.
fn history(history: &ItemHistory) -> String {
    let first = &history.first;
    let after = if first.depends_on.is_empty() {
        String::new()
    } else {
        let on: Vec<String> = first.depends_on.iter().map(|on| format!("#{on}")).collect();
        format!(", after {}", on.join(", "))
    };
    let tags = match first.tags.is_empty() {
        true => String::new(),
        false => format!(", tagged {}", first.tags.join(", ")),
    };
    let mut lines = vec![format!(
        "was added as \"{}\" at {}{after}{tags}{}",
        first.title,
        level(first.level.as_deref()),
        if history.changes.is_empty() {
            ", and has not changed."
        } else {
            "; changed since:"
        }
    )];
    for change in &history.changes {
        let why = change
            .why
            .as_deref()
            .map_or_else(String::new, |why| format!(", because \"{why}\""));
        lines.push(format!("  {}: {}{why}", utc(change.time), item_changed(&change.what)));
    }
    lines.join("\n")
}

/// What a change of an item did, as a clause.
fn item_changed(what: &ItemChanged) -> String {
    match what {
        ItemChanged::Title { title } => format!("titled \"{title}\""),
        ItemChanged::Level { level } => format!("put at level {level}"),
        ItemChanged::DependsOn { item } => format!("came to depend on #{item}"),
        ItemChanged::NoLongerDependsOn { item } => format!("no longer depends on #{item}"),
        ItemChanged::Tagged { tag } => format!("tagged {tag}"),
        ItemChanged::Untagged { tag } => format!("no longer tagged {tag}"),
    }
}

/// An item's level, in words.
fn level(level: Option<&str>) -> String {
    match level {
        Some(level) => format!("level {level}"),
        None => "no level".to_string(),
    }
}

/// Why a session of a line began, as a clause.
fn session_why(why: &SessionWhy) -> String {
    match why {
        SessionWhy::Limit { context, limit } => {
            format!("a new one, its context having been {context} tokens, past the limit of {limit}")
        }
        SessionWhy::Model { was } => format!("a new one, its rounds running on another model than {was}"),
        SessionWhy::Account => "a new one, its rounds running under another account".to_string(),
        SessionWhy::Owner => "a new one, as the owner asked".to_string(),
        SessionWhy::Flagged => "a new one, the service's safety check having flagged its last round".to_string(),
        SessionWhy::Lost => "begun again, the agents' program having lost it".to_string(),
        SessionWhy::Compacted => "compacted: it holds only a summary of what it read".to_string(),
    }
}

/// What changes made together did, a line each in the order asked: what each item is now, or that it was so already.
/// A reopened item says how to let its work on.
fn changed(changes: &[ChangeAsked], done: &[bool], items: Option<&Items>) -> String {
    let work_of =
        |item: u64| (items.and_then(|items| items.items.iter().find(|i| i.number == item))).map(|i| i.work.number);
    let lines: Vec<String> = (changes.iter().zip(done))
        .map(|(change, done)| match (change, *done) {
            (ChangeAsked::Title { item, title }, true) => {
                format!("#{item} is titled \"{title}\"; the title it had is kept in its history.")
            }
            (ChangeAsked::Title { item, title }, false) => format!("#{item} was titled \"{title}\" already."),
            (ChangeAsked::Level { item, level }, true) => {
                format!("#{item} is at level {level}: its work's rounds run on it from now on.")
            }
            (ChangeAsked::Level { item, level }, false) => format!("#{item} was at level {level} already."),
            (ChangeAsked::DependsOn { item, on }, true) => {
                format!("#{item} depends on #{on}: its work waits for it to merge.")
            }
            (ChangeAsked::DependsOn { item, on }, false) => format!("#{item} depended on #{on} already."),
            (ChangeAsked::NoLongerDependsOn { item, on }, _) => format!("#{item} no longer depends on #{on}."),
            (ChangeAsked::Tag { item, tag }, true) => format!("#{item} is tagged {tag}."),
            (ChangeAsked::Tag { item, tag }, false) => format!("#{item} was tagged {tag} already."),
            (ChangeAsked::Untag { item, tag }, _) => format!("#{item} is no longer tagged {tag}."),
            (ChangeAsked::Cancel { item }, _) => format!("#{item} is cancelled."),
            (ChangeAsked::Reopen { item }, _) => match work_of(*item) {
                Some(work) => format!("#{item} is reopened, paused: let it on with `wf-agents resume {work}`."),
                None => format!("#{item} is reopened, paused."),
            },
        })
        .collect();
    lines.join("\n")
}

/// What changed in the plan since the mark, a line for each group of changes made together, oldest first, then how
/// many more there are, and how to ask again from here.
fn plan_changes(answer: &PlanChanges, marked: bool) -> String {
    let mut lines = Vec::new();
    if answer.lines.is_empty() {
        lines.push(match marked {
            true => "Nothing changed in the plan since the mark.".to_string(),
            false => "The plan has no items.".to_string(),
        });
    }
    // Changes made together share a time and a reason, and are said on one line.
    let mut groups: Vec<(u64, Option<&str>, Vec<String>)> = Vec::new();
    for line in &answer.lines {
        let what = match &line.what {
            PlanChange::Added {
                level: at,
                depends_on,
                tags,
            } => {
                let after = match depends_on.is_empty() {
                    true => String::new(),
                    false => {
                        let on: Vec<String> = depends_on.iter().map(|on| format!("#{on}")).collect();
                        format!(", after {}", on.join(", "))
                    }
                };
                let tags = match tags.is_empty() {
                    true => String::new(),
                    false => format!(", tagged {}", tags.join(", ")),
                };
                format!(
                    "#{} added as \"{}\" at {}{after}{tags}",
                    line.item,
                    line.title,
                    level(at.as_deref())
                )
            }
            PlanChange::Changed { change } => format!("#{} {}", line.item, item_changed(change)),
            PlanChange::Cancelled => format!("#{} cancelled", line.item),
            PlanChange::Reopened => format!("#{} reopened", line.item),
        };
        match groups.last_mut() {
            Some((time, why, said)) if *time == line.time && *why == line.why.as_deref() => said.push(what),
            _ => groups.push((line.time, line.why.as_deref(), vec![what])),
        }
    }
    for (time, why, said) in groups {
        let why = why.map_or_else(String::new, |why| format!(", because \"{why}\""));
        lines.push(format!("{}: {}{why}.", utc(time), said.join("; ")));
    }
    if answer.left_out > 0 {
        lines.push(format!("And {} more, given again from this mark.", answer.left_out));
    }
    lines.push(format!("Next: wf-agents changes --since {}", answer.mark));
    lines.join("\n")
}

/// An item in a row: its number, title, level and tags, and how its work stands.
fn row(row: &Row) -> String {
    let tags = match row.tags.is_empty() {
        true => String::new(),
        false => format!(", tagged {}", row.tags.join(", ")),
    };
    let on: Vec<String> = row.on_cancelled.iter().map(|on| format!("#{on}")).collect();
    let good = match on.is_empty() {
        true => String::new(),
        false => format!(", waiting for good on {}, cancelled", on.join(", ")),
    };
    format!(
        "#{} \"{}\" at {}{tags}, {}{good}",
        row.item,
        row.title,
        level(row.level.as_deref()),
        match row.standing {
            Standing::Merged => "merged",
            Standing::Cancelled => "cancelled",
            Standing::Paused => "paused",
            Standing::Held => "held by the engine for a person",
            Standing::Going => "going on",
            Standing::Fault => "unable to go on",
        }
    )
}

/// The items that could be let on now, the first of them a line each, and how many more there are.
fn candidates_said(candidates: &Candidates) -> String {
    let mut lines = vec![format!(
        "{} of {} slots taken; {} {} could be let on now{}",
        candidates.taken,
        candidates.slots,
        candidates.total,
        if candidates.total == 1 { "item" } else { "items" },
        if candidates.total == 0 {
            "."
        } else {
            ", those more items wait for first:"
        }
    )];
    for candidate in &candidates.rows {
        let waited = match candidate.waited_by {
            0 => "nothing waits for it".to_string(),
            1 => "1 open item waits for it".to_string(),
            n => format!("{n} open items wait for it"),
        };
        let begun = match candidate.started {
            true => "goes on where it was paused",
            false => "begins",
        };
        lines.push(format!(
            "  {}; {waited}; let on, it {begun}: wf-agents resume {}",
            row(&candidate.row),
            candidate.row.work
        ));
    }
    let more = candidates.total.saturating_sub(candidates.rows.len());
    if more > 0 {
        lines.push(format!("  And {more} more, which fewer items wait for."));
    }
    lines.join("\n")
}

/// Where a text is mentioned, a line each, and how many more places there are.
fn hits_said(hits: &Hits, text: &str) -> String {
    if hits.hits.is_empty() {
        return format!("Nothing mentions {text:?}.");
    }
    let mut lines = Vec::new();
    for hit in &hits.hits {
        let field = match &hit.field {
            Field::Title => "title".to_string(),
            Field::Block { block } => block.clone(),
            Field::Why { time } => format!("why, at {}", utc(*time)),
        };
        let again = match hit.again {
            0 => String::new(),
            n => format!(" ({n} more there)"),
        };
        lines.push(format!(
            "#{} \"{}\", {field}: {}{again}",
            hit.item, hit.title, hit.snippet
        ));
    }
    if hits.more > 0 {
        lines.push(format!(
            "And {} more places: give a longer text, or --limit {}.",
            hits.more,
            hits.hits.len() + hits.more
        ));
    }
    lines.join("\n")
}

/// What an item waits for and what waits for it, as two trees of lines, indented a step for each item between.
fn graph_said(graph: &Graph, depth: usize, way: Option<&str>) -> String {
    let mut lines = vec![row(&graph.item)];
    // Up is what it depends on, what it waits for; down what depends on it, what waits for it.
    if way != Some("down") {
        match graph.up.is_empty() {
            true => lines.push("It depends on no item.".to_string()),
            false => {
                lines.push("It depends on:".to_string());
                graph_nodes(&mut lines, &graph.up, 1);
            }
        }
    }
    if way != Some("up") {
        match graph.down.is_empty() {
            true => lines.push("No item depends on it.".to_string()),
            false => {
                lines.push("What depends on it:".to_string());
                graph_nodes(&mut lines, &graph.down, 1);
            }
        }
    }
    if graph.left_out > 0 {
        lines.push(format!(
            "And {} more past the depth of {depth}: walk from an item above, or give a greater --depth.",
            graph.left_out
        ));
    }
    lines.join("\n")
}

/// The nodes of a walk under one, indented a step further.
fn graph_nodes(lines: &mut Vec<String>, nodes: &[Node], indent: usize) {
    for node in nodes {
        let pad = "  ".repeat(indent);
        let tail = match (node.again, node.beyond) {
            (true, _) => ", shown above".to_string(),
            (false, 0) => String::new(),
            (false, n) => format!(", and {n} more past it"),
        };
        lines.push(format!("{pad}{}{tail}", row(&node.row)));
        graph_nodes(lines, &node.next, indent + 1);
    }
}

/// The tags items carry, a line each.
fn tags_said(tags: &Tags) -> String {
    if tags.tags.is_empty() {
        return "No item carries a tag.".to_string();
    }
    let lines: Vec<String> = (tags.tags.iter())
        .map(|tag| format!("{}: {} open of {}", tag.tag, tag.open, tag.items))
        .collect();
    lines.join("\n")
}

/// The blocks whose text is to be read, with the version of each: the one named, or every block whose latest version a
/// person wrote, which is the item's own text.
fn texts_of(page: &WorkPage, block: Option<&str>) -> Result<Vec<(String, u64)>, String> {
    let latest = |b: &web::works::Block| b.versions.last().map(|v| (b.name.clone(), v.at));
    match block {
        Some(name) => {
            let found = page.blocks.iter().find(|b| b.name == name).and_then(latest);
            found.map(|found| vec![found]).ok_or_else(|| {
                let names: Vec<&str> = page.blocks.iter().map(|b| b.name.as_str()).collect();
                format!(
                    "#{} has no version of {name}: the blocks it has are {}.",
                    page.summary.item,
                    names.join(", ")
                )
            })
        }
        None => Ok((page.blocks.iter())
            .filter(|b| b.versions.last().is_some_and(|v| v.by == Writer::Person))
            .filter_map(latest)
            .collect()),
    }
}

/// How many characters of a section's first paragraph a short text keeps.
const PARAGRAPH: usize = 240;

/// The text of an item's blocks: each in short, of each section its heading and first paragraph, cut, with the length
/// of the whole; or one whole, where asked. Without a block named, the other blocks the work has are named after.
fn text_said(item: u64, page: &WorkPage, texts: &[(String, String)], whole: bool, named: bool) -> String {
    let mut lines = vec![format!("#{item} \"{}\":", page.title)];
    if texts.is_empty() {
        lines.push("  No person has written a block of it.".to_string());
    }
    for (name, content) in texts {
        if whole {
            lines.push(format!("{name}:"));
            lines.push(content.clone());
            continue;
        }
        let chars = content.chars().count();
        lines.push(format!("{name} ({chars} characters):"));
        let short = short(content);
        lines.extend(short.lines.iter().map(|line| format!("  {line}")));
        if short.cut {
            lines.push(format!("  (in short; whole: wf-agents text {item} {name} --whole)"));
        }
    }
    let shown: Vec<&str> = texts.iter().map(|(name, _)| name.as_str()).collect();
    let others: Vec<String> = (page.blocks.iter())
        .filter(|b| !shown.contains(&b.name.as_str()))
        .filter_map(|b| {
            b.versions
                .last()
                .map(|v| format!("{} ({} characters)", b.name, v.chars))
        })
        .collect();
    if !named && !others.is_empty() {
        lines.push(format!(
            "Its other blocks: {}; one is read with wf-agents text {item} BLOCK.",
            others.join(", ")
        ));
    }
    lines.join("\n")
}

/// A text in short, and whether anything was left out.
struct Short {
    lines: Vec<String>,
    cut: bool,
}

/// A text in short: of each section, a heading line of Markdown starting one, its heading and its first paragraph, each
/// paragraph cut to [`PARAGRAPH`] characters, a cut marked with `…`. A text with no heading is one section.
fn short(text: &str) -> Short {
    let mut lines = Vec::new();
    let mut cut = false;
    // Each section: its heading, where it has one, and its paragraphs, each its lines joined.
    let mut sections: Vec<(Option<String>, Vec<String>)> = vec![(None, Vec::new())];
    let mut paragraph = String::new();
    let end = |sections: &mut Vec<(Option<String>, Vec<String>)>, paragraph: &mut String| {
        if !paragraph.trim().is_empty()
            && let Some(section) = sections.last_mut()
        {
            section.1.push(paragraph.trim().to_string());
        }
        paragraph.clear();
    };
    for line in text.lines() {
        if line.trim_start().starts_with('#') {
            end(&mut sections, &mut paragraph);
            sections.push((Some(line.trim().to_string()), Vec::new()));
        } else if line.trim().is_empty() {
            end(&mut sections, &mut paragraph);
        } else {
            if !paragraph.is_empty() {
                paragraph.push(' ');
            }
            paragraph.push_str(line.trim());
        }
    }
    end(&mut sections, &mut paragraph);
    for (heading, paragraphs) in sections {
        if heading.is_none() && paragraphs.is_empty() {
            continue;
        }
        if let Some(heading) = heading {
            lines.push(heading);
        }
        cut |= paragraphs.len() > 1;
        if let Some(first) = paragraphs.first() {
            if first.chars().count() > PARAGRAPH {
                cut = true;
                lines.push(format!("{}…", first.chars().take(PARAGRAPH).collect::<String>()));
            } else {
                lines.push(first.clone());
            }
        }
    }
    Short { lines, cut }
}

/// What needs the planner, a line each, then how many more there are, and how to ask again from here.
fn attention(answer: &Attention, marked: bool, now: u64) -> String {
    let mut lines = Vec::new();
    if answer.lines.is_empty() {
        lines.push(
            if marked {
                "Nothing for you since the mark."
            } else {
                "Nothing waits for you."
            }
            .to_string(),
        );
    }
    for line in &answer.lines {
        lines.push(heed(line, now));
    }
    if answer.left_out > 0 {
        lines.push(format!("And {} more, given again from this mark.", answer.left_out));
    }
    lines.push(format!("Next: wf-agents wait --since {}", answer.mark));
    lines.join("\n")
}

/// One thing for the planner: the item and its work, what it does, and since when.
fn heed(line: &Line, now: u64) -> String {
    let (of, said) = match line {
        Line::Everything {
            since,
            by: Pauser::Owner,
        } => {
            return format!(
                "The owner paused everything: no work begins a round but to answer a person's words to its cell, for {}.",
                duration(now.saturating_sub(*since))
            );
        }
        Line::Everything {
            since,
            by: Pauser::Engine,
        } => {
            return format!("{STARTED_HELD}, for {}.", duration(now.saturating_sub(*since)));
        }
        Line::Agents { since, outage: o } => {
            return format!(
                "No agent's round of any work begins: {}, for {}.",
                outage(o),
                duration(now.saturating_sub(*since))
            );
        }
        Line::Fault { of, since, now: is } | Line::ForAPerson { of, since, now: is } => (
            of,
            format!("{}, for {}", self::now(is), duration(now.saturating_sub(*since))),
        ),
        Line::Idle {
            of,
            since,
            now: is,
            items,
        } => {
            let at = match is {
                Now::Waits { at, .. } => format!(" at {at}"),
                _ => String::new(),
            };
            let numbers: Vec<String> = items.iter().map(|idle| idle.item.to_string()).collect();
            let why: Vec<String> = (items.iter())
                .map(|idle| format!("{} {}", idle.item, stuck(idle.why)))
                .collect();
            let (items, none) = match numbers.as_slice() {
                [one] => (format!("item {one}"), "which is not going on"),
                [rest @ .., last] => (
                    format!("items {} and {last}", rest.join(", ")),
                    "none of which is going on",
                ),
                [] => (String::new(), ""),
            };
            let waited = duration(now.saturating_sub(*since));
            (
                of,
                format!("waits{at} for {items}, {none} ({}), for {waited}", why.join(", ")),
            )
        }
        Line::Ended { of, since, now: is } => (
            of,
            format!("{}, {} ago", self::now(is), duration(now.saturating_sub(*since))),
        ),
    };
    let title: String = if of.title.chars().count() > TITLE {
        format!("{}…", of.title.chars().take(TITLE - 1).collect::<String>())
    } else {
        of.title.clone()
    };
    format!("Item {} \"{title}\", work {}: {said}.", of.item, of.work)
}

/// Why an item waited for is not going on, after its number.
fn stuck(why: Stuck) -> &'static str {
    match why {
        Stuck::Paused => "paused",
        Stuck::ForAPerson => "resting for a person",
        Stuck::Idle => "waiting in turn for items none of which is going on",
        Stuck::Cancelled => "cancelled",
        Stuck::Fault => "unable to go on",
    }
}

/// A duration in its two largest units, as the owner's page writes one: 2d 03h, 2h 05m, 3m 07s, 41s. Brought over from
/// the reminders of the engine on `old-main` (`crates/engine/src/reminders.rs`).
fn duration(ms: u64) -> String {
    let s = ms / 1000;
    let (d, h, m, sec) = (s / 86_400, s % 86_400 / 3_600, s % 3_600 / 60, s % 60);
    if d > 0 {
        format!("{d}d {h:02}h")
    } else if h > 0 {
        format!("{h}h {m:02}m")
    } else if m > 0 {
        format!("{m}m {sec:02}s")
    } else {
        format!("{sec}s")
    }
}

/// The time now, in milliseconds since the start of 1970, to say how long ago what is given happened.
fn clock() -> u64 {
    (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH))
        .map_or(0, |since| u64::try_from(since.as_millis()).unwrap_or(u64::MAX))
}

/// Why the run's last round failed, where it did: nothing came back of it.
fn failure(run: &Run) -> Option<&str> {
    match &run.rounds.last()?.back {
        Some(Back::Failed { why, .. }) => Some(why),
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
        Now::Everything {} => "held back by the pause of everything".to_string(),
        Now::Waits { at, on } => {
            let on: Vec<String> = on.iter().map(waited).collect();
            format!("waits at {at} for {}", on.join("; and for "))
        }
        Now::Next { cell } => format!("about to go on at {cell}"),
        Now::Stalled { at, on, by } => stalled(at, on, by),
    }
}

/// What a stalled work waits for and why no cell that writes it runs, as a clause.
fn stalled(at: &str, on: &[CellKey], by: &[Stalling]) -> String {
    let keys = |keys: &[CellKey]| keys.iter().map(|k| k.words.as_str()).collect::<Vec<_>>().join(" and ");
    let cells: Vec<String> = by.iter().map(|s| format!("cell {}", s.cell)).collect();
    let writers = match cells.is_empty() {
        true => "no cell".to_string(),
        false => format!("only {}", cells.join(" or ")),
    };
    let mut said = match on.is_empty() {
        false => format!(
            "stalled at {at}, waiting for {}, which {writers} can make hold",
            keys(on)
        ),
        true => format!("stalled at {at}, its last cell, which nothing new runs; what runs it {writers} can write"),
    };
    for s in by {
        let why = match &s.why {
            Unrun::Skipped { on } => format!("it is skipped, {} not holding", keys(on)),
            Unrun::NothingNew {} => "nothing new runs it".to_string(),
            Unrun::Held {} => format!("{at} holds it"),
        };
        said.push_str(&format!("; cell {} does not run: {why}", s.cell));
    }
    said
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
        Waited::Calls { runs } => format!(
            "the owner to let it go on, its last {runs} runs having each asked for calls and dealt with nothing"
        ),
        Waited::Slot {} => "a free slot".to_string(),
        Waited::Answer { question } => format!("the owner to answer the question {question}"),
        Waited::Agents { outage: o } => format!("the agents' rounds to begin again: {}", outage(o)),
    }
}

/// A wait every work shares, as a clause: its cause, when it is over, and the round that met it.
fn outage(outage: &Outage) -> String {
    let cause = match outage.failure {
        Failure::UsageLimit => "the account's usage limit was reached",
        Failure::CannotRun => "the agents' command line cannot run; sign it in again or install it",
        Failure::Offline => "the network cannot be reached",
        _ => "a round failed for a cause every work shares",
    };
    let until = match outage.until {
        Until::At { at } => format!("until {}", utc(at)),
        Until::Trial { at, look: false, .. } => format!("one round tries at {}", utc(at)),
        Until::Trial { at, .. } => format!(
            "the network is looked at from {} every half minute, and one round tries once it is reached",
            utc(at)
        ),
        Until::Looked { at } => format!(
            "the host is looked at from {} every half minute, and every act goes on once it is reached",
            utc(at)
        ),
        Until::Tried => "one round tries now".to_string(),
    };
    format!(
        "{cause}, {until} (round {} of run {} of work {}: {})",
        outage.round, outage.run, outage.work, outage.why
    )
}

/// A time in milliseconds since the start of 1970, as the owner reads it: its day and minute in UTC.
fn utc(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // The civil date of a day count from 1970-01-01, as Howard Hinnant's days_from_civil inverts it.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02} UTC", rest / 3600, rest % 3600 / 60)
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
        Call::Wait { until, failed } => format!(
            "the engine's wait until {}, before the round that failed in passing ({failed} in a row) is tried again",
            utc(*until)
        ),
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

#[cfg(test)]
mod tests {
    use web::works::{FirstItem, Item, ItemHistory, Summary};

    use super::*;

    /// An item of the trial flow, merged where `merged`, depending on these, carrying these tags.
    fn item(number: u64, merged: bool, depends_on: &[u64], tags: &[&str]) -> Item {
        Item {
            number,
            title: format!("item {number}"),
            level: Some("1".to_string()),
            project: Some("script".to_string()),
            added: 0,
            depends_on: depends_on.to_vec(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            history: ItemHistory {
                first: FirstItem {
                    title: format!("item {number}"),
                    level: Some("1".to_string()),
                    depends_on: depends_on.to_vec(),
                    tags: Vec::new(),
                },
                changes: Vec::new(),
            },
            work: Summary {
                number,
                item: number,
                flow: "trial".to_string(),
                added: 0,
                paused: false,
                now: if merged {
                    Now::Merged
                } else {
                    Now::Stalled {
                        at: "merge".to_string(),
                        on: Vec::new(),
                        by: Vec::new(),
                    }
                },
                cells: Vec::new(),
                spent: Spent::default(),
                last: None,
                from: None,
                bar: String::new(),
                sent_back: 0,
            },
        }
    }

    fn items(items: Vec<Item>) -> Items {
        Items { items, outage: None }
    }

    #[test]
    fn a_list_gives_at_most_a_hundred_items_and_says_how_to_see_the_rest() {
        let many = items((1..=105).map(|n| item(n, false, &[], &[])).collect());
        let said = list(&many, &Listing::default());
        let lines: Vec<&str> = said.lines().collect();
        assert_eq!(lines.len(), LISTED + 1);
        assert_eq!(lines[LISTED], "And 5 more: wf-agents list --from 101");
        let from = Listing {
            from: Some(101),
            ..Listing::default()
        };
        let rest = list(&many, &from);
        assert_eq!(rest.lines().count(), 5);
        assert!(rest.starts_with("#101 "), "{rest}");
    }

    #[test]
    fn a_list_by_tag_gives_the_items_that_carry_it_and_marks_one_that_waits_for_good() {
        let mut cancelled = item(1, false, &[], &[]);
        cancelled.work.now = Now::Cancelled;
        let some = items(vec![
            cancelled,
            item(2, false, &[1], &["page"]),
            item(3, true, &[], &["page"]),
        ]);
        let page = |all: bool| Listing {
            tag: Some("page".to_string()),
            from: None,
            all,
        };
        // What ended is left out, and counted, unless all are asked for.
        assert_eq!(
            list(&some, &page(false)),
            "#2 \"item 2\" at level 1, W2 on trial, after #1, tagged page: stalled at merge, its last cell, which nothing \
             new runs; what runs it no cell can write It waits for good on #1, cancelled.\nAnd 1 item that ended, merged or \
             cancelled: wf-agents list --tag page --all"
        );
        assert_eq!(
            list(&some, &page(true)),
            "#2 \"item 2\" at level 1, W2 on trial, after #1, tagged page: stalled at merge, its last cell, which nothing \
             new runs; what runs it no cell can write It waits for good on #1, cancelled.\n#3 \"item 3\" at level 1, W3 on trial, \
             tagged page: merged"
        );
        let none = Listing {
            tag: Some("none".to_string()),
            ..Listing::default()
        };
        assert_eq!(list(&some, &none), "No item that has not ended carries the tag none.");
        // A list of none that has not ended still says how many ended.
        let ended = items(vec![item(3, true, &[], &[])]);
        assert_eq!(
            list(&ended, &Listing::default()),
            "No item that has not ended.\nAnd 1 item that ended, merged or cancelled: wf-agents list --all"
        );
    }

    #[test]
    fn a_text_in_short_is_each_sections_heading_and_first_paragraph_cut() {
        let long = "word ".repeat(100);
        let text = format!("Before any heading.\n\n# One\nFirst of one.\n\nSecond of one.\n## Two\n{long}\n");
        let said = short(&text);
        assert!(said.cut);
        assert_eq!(
            said.lines[..4],
            ["Before any heading.", "# One", "First of one.", "## Two"]
        );
        assert_eq!(said.lines[4].chars().count(), PARAGRAPH + 1);
        assert!(said.lines[4].ends_with('…'));
        let whole = short("Only this.");
        assert_eq!((whole.lines, whole.cut), (vec!["Only this.".to_string()], false));
    }

    #[test]
    fn a_change_is_read_as_item_kind_and_value_and_a_wrong_one_is_refused_saying_how() {
        let given: Vec<String> = [
            "12:title=Parse: the header",
            "#3:depends-on=#9",
            "4:cancel",
            "4:reopen",
            "5:tag=a",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(
            changes(&given),
            Ok(vec![
                ChangeAsked::Title {
                    item: 12,
                    title: "Parse: the header".to_string()
                },
                ChangeAsked::DependsOn { item: 3, on: 9 },
                ChangeAsked::Cancel { item: 4 },
                ChangeAsked::Reopen { item: 4 },
                ChangeAsked::Tag {
                    item: 5,
                    tag: "a".to_string()
                },
            ])
        );
        for wrong in [
            "title=x",
            "x:title=y",
            "3:depends-on=x",
            "3:cancel=now",
            "3:title",
            "3:frob",
        ] {
            let refused = changes(&[wrong.to_string()]).expect_err(wrong);
            assert!(
                refused.contains("A change is ITEM:KIND or ITEM:KIND=VALUE"),
                "{refused}"
            );
        }
    }

    #[test]
    fn a_text_in_a_query_is_written_so_that_no_byte_of_it_ends_the_query() {
        assert_eq!(encoded("a b&c=d/é"), "a%20b%26c%3Dd%2F%C3%A9");
        assert_eq!(encoded("3,2.1.0"), "3%2C2.1.0");
    }
}
