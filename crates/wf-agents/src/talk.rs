//! The commands that talk to a running engine over the owner's interface, for the owner and their assistant to manage
//! items from a terminal: each sends one request on the loopback address and says, in plain text a person and an
//! agent can both read, what came of it. A refusal is said on the error stream in the engine's own words, and the
//! command fails.

use std::net::{Ipv4Addr, SocketAddr};
use std::process::ExitCode;

use driver::attention::Mark;
use flow::agents::sentence;
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::{Method, Request, StatusCode, header};
use hyper_util::rt::TokioIo;
use serde::Serialize;
use serde::de::DeserializeOwned;
use web::attention::{Attention, Line, Stuck};
use web::observations::{Observation, ObservationsPage};
use web::plan::{Candidates, Field, Graph, Hits, Node, PlanChange, PlanChanges, Row, Standing, Tags};
use web::system::{Past, System, UnderWay};
use web::works::{
    Act, Added, Against, Back, Call, CellState, ChangeAsked, Checked, Engine, EngineAct, Failure, ForkAsked,
    ForkInstead, ForkPage, Found, ItemChanged, ItemChanges, ItemHistory, Items, ItemsChanged, Limit, Maker, NewItem,
    NewWrite, Now, Outage, Pauser, Person, QuestionPage, Refusal, Run, Said, SessionWhy, Spent, SpokenTo, Step, Until,
    VersionPage, Waited, WorkPage, Writer,
};

/// What a command asks of the engine.
pub enum Asked {
    Add(NewItem),
    /// The items a listing gives.
    List(Listing),
    /// A work; or one part of it alone, from the entry at this place, the first being one.
    Show {
        work: u64,
        part: Option<Part>,
        from: Option<usize>,
    },
    /// How the system ran over this many days until now, at most this many of each kind of thing listed.
    System {
        days: u64,
        limit: usize,
    },
    Act(u64, Act),
    /// A fork of the session of a round of a run asked a question: the run's last round that ran where none is named.
    Fork {
        work: u64,
        run: u64,
        round: Option<usize>,
        question: String,
    },
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
    /// The observations, the newest first, from this number down, at most this many.
    Observations {
        from: Option<u64>,
        limit: usize,
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

/// A day, in milliseconds.
const DAY: u64 = 24 * 60 * 60 * 1000;

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
                for (name, version) in texts_of(&page, block.as_deref())? {
                    let path = format!("/api/v1/works/{work}/versions/{}/{version}", encoded(&name));
                    let version: VersionPage = send(address, Method::GET, &path, None::<&()>).await?;
                    texts.push((name, version.content));
                }
                Ok(text_said(item, &page, &texts, whole, block.is_some()))
            }
            Asked::Observations { from, limit } => {
                let page: ObservationsPage = send(address, Method::GET, "/api/v1/observations", None::<&()>).await?;
                Ok(observations_said(&page, from, limit))
            }
            Asked::Everything(act) => {
                send::<(), _>(address, Method::POST, "/api/v1/engine/acts", Some(&act)).await?;
                let engine: Engine = send(address, Method::GET, "/api/v1/engine", None::<&()>).await?;
                Ok(standing(&engine))
            }
            Asked::Show { work, part, from } => {
                let page: WorkPage = send(address, Method::GET, &format!("/api/v1/works/{work}"), None::<&()>).await?;
                Ok(match part {
                    None => show(&page),
                    Some(part) => show_part(&page, part, from.unwrap_or(1)),
                })
            }
            Asked::System { days, limit } => {
                let from = clock().saturating_sub(days.saturating_mul(DAY));
                let path = format!("/api/v1/system?weeks={from}");
                let system: System = send(address, Method::GET, &path, None::<&()>).await?;
                Ok(system_said(&system, days, limit))
            }
            Asked::Fork {
                work,
                run,
                round,
                question,
            } => {
                let path = format!("/api/v1/works/{work}/runs/{run}/forks");
                let asked = ForkAsked { round, question };
                let page: ForkPage = send(address, Method::POST, &path, Some(&asked)).await?;
                Ok(forked_said(&page))
            }
            Asked::Act(work, act) => {
                let resumed = act == Act::Resume {};
                let path = format!("/api/v1/works/{work}/acts");
                // Words said answer with the cell the engine gave them to; every other act with nothing.
                let said: Option<Said> = send(address, Method::POST, &path, Some(&act)).await?;
                let done = done(work, &act, said.as_ref());
                if !resumed {
                    return Ok(done);
                }
                // A work let on that the pause of everything holds back is said to be held, not to go on.
                let page: WorkPage = send(address, Method::GET, &format!("/api/v1/works/{work}"), None::<&()>).await?;
                Ok(match page.summary.now {
                    Now::Everything { .. } => format!(
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
                    now => Err(format!("Work {work} is not cancelled: it {}", sentence(&self::now(&now)))),
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

/// A question as it stands, in lines: its name, cell and version, by its number, the first line of what it asks, its
/// options and the owner's answer; of one withdrawn, why.
fn question(q: &QuestionPage) -> Vec<String> {
    let by = if q.engine { ", asked by the engine" } else { "" };
    let waits = if q.waits { ", waits" } else { "" };
    let version = q.version.map_or_else(|| "none".to_string(), |n| n.to_string());
    let head = format!("  {} (of {}, version {version}{by}{waits})", q.name, q.cell);
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
        None => match q.version {
            Some(number) => format!("    unanswered: wf-agents answer <work> {} {number} <option>", q.name),
            None => "    unanswered, and no version of its cell's questions names it to answer by".to_string(),
        },
    });
    lines
}

/// What a command that acts says once it is done, with what the engine answered: of words said, the cell it gave them
/// to.
fn done(work: u64, act: &Act, said: Option<&Said>) -> String {
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
        Act::Say { to, .. } => {
            let cell = said.map_or("the cell the engine chose", |s| s.reached.as_str());
            match to {
                SpokenTo::Line { line } => format!("Said to {cell} of work {work}, where the line {line} stands."),
                SpokenTo::Cell { .. } => format!("Said to {cell} of work {work}."),
            }
        }
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
const STARTED_HELD: &str = "The engine started with works going on and holds every work until the owner has looked: \
                            none begins a round but to answer a note, until `wf-agents resume --all`";

/// The most items `list` gives in one answer, and the most entries of a part of a work `show` gives alone.
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
/// how to see them, is said, and how many ended are left out. An item that waits for good for a cancelled item says
/// so in what its work does now, as the engine gives it.
fn list(items: &Items, listing: &Listing) -> String {
    if items.items.is_empty() {
        return "No items.".to_string();
    }
    let (tag, from) = (listing.tag.as_deref(), listing.from);
    let mut lines: Vec<String> = (items.outage.iter())
        .filter(|o| o.holds)
        .map(|o| format!("No agent's round of any work begins. {}", outage(o)))
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
        // What it does now says which items it waits for, those that were cancelled among them, as the engine says.
        let work = &item.work;
        lines.push(format!(
            "#{} \"{}\" at {}, {}, W{} on {}{after}{tags}: {}",
            item.number,
            item.title,
            level(item.level.as_deref()),
            project(item.project.as_deref()),
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

/// The most entries of each part of a work that grows with it, what happened to its item, its questions, its runs and
/// its sessions, that `show` gives with the rest of the work.
const SHOWN: usize = 10;

/// A part of a work that grows with it, which `show` gives alone, a page at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Part {
    /// What happened to the work's item since it was added: each change made to it, each cancel and each reopen.
    History,
    /// Each question put to the owner, as it stands, in the order of their names.
    Questions,
    /// Each run of a cell, in the order they started.
    Runs,
    /// Each new session of a line, in the order they began.
    Sessions,
}

impl Part {
    /// Its name, as the command is given it.
    fn name(self) -> String {
        let value = clap::ValueEnum::to_possible_value(&self).expect("every part is given by its name");
        value.get_name().to_string()
    }

    /// What it is of the work, in words.
    fn said(self) -> &'static str {
        match self {
            Part::History => "what happened to its item",
            Part::Questions => "its questions",
            Part::Runs => "its runs",
            Part::Sessions => "its sessions",
        }
    }
}

/// One work: what it does now, its cells and blocks and what it took; and of each part that grows with it, at most
/// [`SHOWN`] entries, the latest, or of the questions those not answered first, with how many more there are and the
/// command that gives them.
fn show(page: &WorkPage) -> String {
    let summary = &page.summary;
    let work = summary.number;
    let happened = entries(page, Part::History);
    let mut lines = vec![
        format!(
            "Work {} of item {} \"{}\" at {}, {}, on the flow {}.",
            summary.number,
            summary.item,
            page.title,
            level(page.level.as_deref()),
            project(page.project.as_deref()),
            summary.flow
        ),
        format!("Now: {}", sentence(&now(&summary.now))),
        format!(
            "Paused: {}.",
            match summary.paused {
                Some(Pauser::Owner) => "by the owner",
                Some(Pauser::Engine) => "by the engine",
                None => "no",
            }
        ),
        format!(
            "Tags: {}.",
            if page.tags.is_empty() {
                "none".to_string()
            } else {
                page.tags.join(", ")
            }
        ),
        format!("Spent: {}.", spent(&summary.spent)),
        format!("Item {} {}", summary.item, added(&page.history, happened.is_empty())),
    ];
    latest(&mut lines, &happened, work, Part::History);
    lines.push("Cells:".to_string());
    for cell in &summary.cells {
        let run = |run: Option<u64>| run.map_or_else(String::new, |run| format!(" in run {run}"));
        let failure = |open: Option<u64>| {
            let run = open.and_then(|open| runs(page).find(|run| run.run == open));
            run.and_then(failure).map_or_else(String::new, |why| format!(": {why}"))
        };
        let state = match cell.state {
            CellState::Running => format!("a round under way{}", run(cell.open)),
            CellState::Calling => format!("a call under way{}", run(cell.open)),
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
            CellState::NotRun => "did not run".to_string(),
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
        // Those not answered first, which may hold the work for the owner.
        let (waiting, other): (Vec<&QuestionPage>, Vec<&QuestionPage>) = page.questions.iter().partition(|q| open(q));
        let order: Vec<&QuestionPage> = waiting.iter().chain(&other).copied().collect();
        lines.extend(order.iter().take(SHOWN).flat_map(|q| question(q)));
        let more = order.len().saturating_sub(SHOWN);
        if more > 0 {
            let of = match waiting.len().saturating_sub(SHOWN) {
                0 => String::new(),
                n => format!(", {n} of them not answered"),
            };
            lines.push(format!("  And {more} more{of}: wf-agents show {work} questions"));
        }
    }
    lines.push("Runs:".to_string());
    latest(&mut lines, &entries(page, Part::Runs), work, Part::Runs);
    let sessions = entries(page, Part::Sessions);
    if !sessions.is_empty() {
        lines.push("Sessions:".to_string());
        latest(&mut lines, &sessions, work, Part::Sessions);
    }
    lines.join("\n")
}

/// One part of a work alone: at most [`LISTED`] of its entries in their order, from the one at this place, the first
/// being one, with how many more there are and the command that gives them.
fn show_part(page: &WorkPage, part: Part, from: usize) -> String {
    let (work, said) = (page.summary.number, part.said());
    let all = entries(page, part);
    let from = from.max(1);
    if all.is_empty() {
        return format!("Work {work}, {said}: none.");
    }
    if from > all.len() {
        return format!("Work {work}, {said}: {}, fewer than {from}.", all.len());
    }
    let to = all.len().min(from - 1 + LISTED);
    let mut lines = vec![format!("Work {work}, {said}, {from} to {to} of {}:", all.len())];
    lines.extend(all[from - 1..to].iter().flatten().cloned());
    if to < all.len() {
        lines.push(format!(
            "And {} more: wf-agents show {work} {} --from {}",
            all.len() - to,
            part.name(),
            to + 1
        ));
    }
    lines.join("\n")
}

/// The last [`SHOWN`] entries of a part, each its lines, after how many came before them, where any did, with the
/// command that gives them.
fn latest(lines: &mut Vec<String>, entries: &[Vec<String>], work: u64, part: Part) {
    let before = entries.len().saturating_sub(SHOWN);
    if before > 0 {
        lines.push(format!("  And {before} earlier: wf-agents show {work} {}", part.name()));
    }
    lines.extend(entries[before..].iter().flatten().cloned());
}

/// Whether a question is neither answered nor withdrawn.
fn open(q: &QuestionPage) -> bool {
    q.answer.is_none() && q.withdrawn.is_none()
}

/// The runs of a work, in the order they started.
fn runs(page: &WorkPage) -> impl Iterator<Item = &Run> {
    page.steps.iter().filter_map(|step| match step {
        Step::Run(run) => Some(run),
        _ => None,
    })
}

/// The entries of a part of a work, in their order, each its lines.
fn entries(page: &WorkPage, part: Part) -> Vec<Vec<String>> {
    match part {
        Part::History => {
            // What was changed of the item, and each cancel and reopen of it, each at its time.
            let mut happened: Vec<(u64, String)> = (page.history.changes.iter())
                .map(|change| {
                    let why = (change.why.as_deref()).map_or_else(String::new, |why| format!(", because \"{why}\""));
                    let line = format!("  {}: {}{why}", utc(change.time), item_changed(&change.what));
                    (change.time, line)
                })
                .collect();
            for step in &page.steps {
                match step {
                    Step::Cancelled { time, why, .. } => {
                        let why = (why.as_deref()).map_or_else(
                            || ", no reason being given".to_string(),
                            |why| format!(", because \"{why}\""),
                        );
                        happened.push((*time, format!("  {}: cancelled{why}", utc(*time))));
                    }
                    Step::Reopened { time, why, .. } => {
                        happened.push((*time, format!("  {}: reopened, because \"{why}\"", utc(*time))));
                    }
                    _ => {}
                }
            }
            happened.sort_by_key(|(time, _)| *time);
            happened.into_iter().map(|(_, line)| vec![line]).collect()
        }
        Part::Questions => page.questions.iter().map(question).collect(),
        Part::Runs => runs(page).map(run_said).collect(),
        Part::Sessions => (page.steps.iter())
            .filter_map(|step| match step {
                Step::Session { at, line, why, .. } => Some(vec![format!("  {line} at {at}: {}", session_why(why))]),
                _ => None,
            })
            .collect(),
    }
}

/// A run, in lines: its rounds and calls, the context its last round ended with, its session's compactions and how it
/// ended; then each of its rounds that missed its session's prompt cache, with what differed before the messages, or
/// that nothing did, which is a fault of wf-agents; then what of it went past the lines of its history.
fn run_said(run: &Run) -> Vec<String> {
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
    let mut lines = vec![format!(
        "  run {} of {}: {rounds}{calls}{context}{compacted}, {end}",
        run.run, run.cell
    )];
    for round in &run.rounds {
        let Some(missed) = &round.missed else {
            continue;
        };
        let cause = match missed.differs.is_empty() {
            true => "nothing before the messages differed, a fault of wf-agents".to_string(),
            false => format!("{} differed from the call before", missed.differs.join(", ")),
        };
        lines.push(format!(
            "    round {} missed its session's cache: read {} of the {} tokens it held; {cause}",
            round.number, missed.read, missed.held
        ));
    }
    lines.extend(past_of(run));
    lines
}

/// What an item was as it was added, as the end of a sentence about it, and whether anything happened to it since.
fn added(history: &ItemHistory, unchanged: bool) -> String {
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
    format!(
        "was added as \"{}\" at {}{after}{tags}{}",
        first.title,
        level(first.level.as_deref()),
        if unchanged {
            ", and has not changed."
        } else {
            "; changed since:"
        }
    )
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

/// An item's project, in words: the name the installation's settings give it.
fn project(project: Option<&str>) -> String {
    match project {
        Some(project) => format!("project {project}"),
        None => "no project".to_string(),
    }
}

/// Why a session of a line began, as a clause.
fn session_why(why: &SessionWhy) -> String {
    match why {
        SessionWhy::Limit { context, limit } => {
            format!("a new one, its context having been {context} tokens, past the limit of {limit}")
        }
        SessionWhy::Model { was } => format!("a new one, its rounds running on another model than {was}"),
        SessionWhy::Changed { context } => {
            let changed =
                "a new one, the rules or the form of return its agents are given having changed since it began";
            match context {
                Some(context) => format!("{changed}, the session it left holding {context} tokens of context"),
                None => changed.to_string(),
            }
        }
        SessionWhy::Account => "a new one, its rounds running under another account".to_string(),
        SessionWhy::Owner => "a new one, as the owner asked".to_string(),
        SessionWhy::Flagged => "a new one, the service's safety check having flagged its last round".to_string(),
        SessionWhy::Guarded => "a new one, the run guard having stopped its last round".to_string(),
        SessionWhy::TooLong => "a new one, its last round's prompt having been too long for the model".to_string(),
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

/// The blocks whose text is to be read, with the number of the version of each: the one named, or every block people
/// write, as the flow declares them, whose latest version the owner wrote, which is the item's own text. The owner's
/// words to a cell and answers are the owner's too, and no text of the item's.
fn texts_of(page: &WorkPage, block: Option<&str>) -> Result<Vec<(String, u64)>, String> {
    let latest = |b: &web::works::Block| b.versions.last().map(|v| (b.name.clone(), v.number));
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
        None => {
            // As the flow declares them; a work that ended is given no flow, and its item's text is what the card of
            // the version it ran on shows (decision a-works-past-is-drawn-from-its-outline).
            let people = |name: &str| match &page.flow {
                Some(flow) => (flow.blocks.iter()).any(|b| b.name == name && b.written_by.people),
                None => page.outline.card.iter().any(|shown| shown == name),
            };
            Ok((page.blocks.iter())
                .filter(|b| people(&b.name) && b.versions.last().is_some_and(|v| v.by == Writer::Owner))
                .filter_map(latest)
                .collect())
        }
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
                "Nothing for the owner since the mark."
            } else {
                "Nothing waits for the owner."
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
        // The shared wait, and where the agents' command line cannot run, what the owner does, said here alone.
        Line::Agents { since, outage: o, mend } => {
            let mend = mend
                .as_deref()
                .map_or_else(String::new, |mend| format!(" {}", sentence(mend)));
            return format!(
                "No agent's round of any work begins, for {}. {}{mend}",
                duration(now.saturating_sub(*since)),
                outage(o),
            );
        }
        // How long first, as what a stall says ends as a sentence.
        Line::Fault { of, since, now: is } | Line::Stalled { of, since, now: is } => (
            of,
            format!("for {}, {}", duration(now.saturating_sub(*since)), self::now(is)),
        ),
        // What it waits on the owner for, at the cell it stands at, each by why, a question by the first line of what
        // it asks, each a sentence. What the owner does about the agents' command line is said on the line of the wait
        // every work shares.
        Line::ForAPerson {
            of,
            since,
            waits,
            now: is,
        } => {
            let at = match is {
                Now::Waits { at, .. }
                | Now::Stalled { at, .. }
                | Now::Paused { at, .. }
                | Now::Everything { at, .. } => format!(" at {at}"),
                Now::Round { cell, .. } | Now::Next { cell } => format!(" at {cell}"),
                _ => String::new(),
            };
            let why: Vec<String> = waits.iter().map(|wait| sentence(&first_line(&wait.why))).collect();
            let waited = duration(now.saturating_sub(*since));
            (of, format!("waits for the owner{at} for {waited}: {}", why.join(" ")))
        }
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
            let why: Vec<String> = (items.iter())
                .map(|idle| format!("#{} {}", idle.item, stuck(idle.why)))
                .collect();
            let none = match items.len() {
                1 => "which is not going on",
                _ => "none of which is going on",
            };
            let items = numbers(&items.iter().map(|idle| idle.item).collect::<Vec<_>>());
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
    format!(
        "Item {} \"{}\", work {}: {}",
        of.item,
        cut_title(&of.title),
        of.work,
        sentence(&said)
    )
}

/// A title, cut to what a line of `wait` or `observations` carries.
fn cut_title(title: &str) -> String {
    if title.chars().count() > TITLE {
        format!("{}…", title.chars().take(TITLE - 1).collect::<String>())
    } else {
        title.to_string()
    }
}

/// The observations from the number given down, the newest first, each as a line saying where it came from and whether
/// the owner read it, then its whole text indented; and how many more there are, with the command that gives them.
fn observations_said(page: &ObservationsPage, from: Option<u64>, limit: usize) -> String {
    if page.observations.is_empty() {
        return "There are no observations.".to_string();
    }
    let down: Vec<&Observation> = (page.observations.iter())
        .filter(|o| from.is_none_or(|from| o.number <= from))
        .collect();
    if down.is_empty() {
        return format!("No observation is numbered O{} or lower.", from.unwrap_or_default());
    }
    let mut lines = Vec::new();
    for o in down.iter().take(limit) {
        let read = match o.read {
            Some(at) => format!("read {}", utc(at)),
            None => "unread".to_string(),
        };
        lines.push(format!(
            "O{}, {read}, kept {}: item #{} \"{}\", W{}, cell {}, run {}, round {}",
            o.number,
            utc(o.kept),
            o.item,
            cut_title(&o.title),
            o.work,
            o.cell,
            o.run,
            o.round
        ));
        lines.extend(o.text.trim().lines().map(|line| format!("  {line}")));
    }
    if let Some(next) = down.get(limit) {
        lines.push(format!(
            "And {} more, older: wf-agents observations --from {}",
            down.len() - limit,
            next.number
        ));
    }
    lines.join("\n")
}

/// The first line of a text, cut to a paragraph's length.
fn first_line(text: &str) -> String {
    let line = text.trim().lines().next().unwrap_or_default();
    if line.chars().count() > PARAGRAPH {
        format!("{}…", line.chars().take(PARAGRAPH).collect::<String>())
    } else {
        line.to_string()
    }
}

/// Why an item waited for is not going on, after its number.
fn stuck(why: Stuck) -> &'static str {
    match why {
        Stuck::Paused => "paused",
        Stuck::ForAPerson => "waiting for the owner",
        Stuck::Stalled => "stalled",
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
        Now::Paused {
            at, by: Pauser::Owner, ..
        } => format!("paused at {at}"),
        Now::Paused {
            at,
            by: Pauser::Engine,
            why: Some(why),
        } => format!("paused at {at} by the engine. {}", sentence(why)),
        Now::Paused {
            at,
            by: Pauser::Engine,
            why: None,
        } => format!("paused at {at} by the engine, which gave no reason"),
        Now::Everything { at, .. } => format!("held back at {at} by the pause of everything"),
        Now::Waits { at, on } => {
            let on: Vec<String> = on.iter().map(waited).collect();
            format!("waits at {at} for {}", on.join("; and for "))
        }
        Now::Next { cell } => format!("about to go on at {cell}"),
        Now::Stalled { at, why, .. } => format!("stalled at {at}. {why}"),
    }
}

/// What a work waits for, as a clause.
fn waited(waited: &Waited) -> String {
    match waited {
        Waited::Call {
            call,
            who: Some(who),
            what,
            ..
        } => format!("the {} to answer call {call}: {}", person(*who), called(what)),
        Waited::Call {
            call,
            who: None,
            what,
            at,
            ..
        } => match at {
            Some(at) => format!("call {call}, {}, running at {at}", called(what)),
            None => format!("call {call}, {}", called(what)),
        },
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
        Waited::Turn { holder, .. } => format!("the project's turn to check and merge, which work {holder} holds"),
        Waited::Agents { outage: o } => format!("the agents' rounds to begin again, as {}", cause(o)),
        Waited::Outside { source, cell, why } => format!("the outside ({source}), before {cell} runs again: {why}"),
        // Where one was cancelled the work waits for it for good, which is said in the page's words.
        Waited::Items { items, cancelled } => match cancelled.is_empty() {
            true => format!("the items it depends on to merge: {}", numbers(items)),
            false => format!(
                "the items it depends on to merge: {}; it waits for good on {}, cancelled",
                numbers(items),
                numbers(cancelled)
            ),
        },
    }
}

/// Items by their numbers, each as an item's number is written: #5, #5 and #7, #5, #6 and #7.
fn numbers(items: &[u64]) -> String {
    let items: Vec<String> = items.iter().map(|item| format!("#{item}")).collect();
    match items.as_slice() {
        [] => "none".to_string(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// What a wait every work shares has as its cause, as the latest check of the outside found it or else as the round
/// that met it did, as a clause.
fn cause(outage: &Outage) -> &'static str {
    let failure = match &outage.checked {
        Some(Checked {
            found: Found::Down { failure, .. },
            ..
        }) if outage.holds => Some(*failure),
        _ => None,
    };
    match failure.unwrap_or(outage.failure) {
        Failure::UsageLimit => "the account's usage limit was reached",
        Failure::CannotRun => "the agents' command line cannot run",
        Failure::Offline => "the network cannot be reached",
        Failure::Unavailable => "the agents' service is down, overloaded or not answering",
        _ if failure.is_some() => "the engine's check of the outside failed",
        _ => "a round failed for a cause every work shares",
    }
}

/// A wait every work shares, in sentences: what is down and since when; what the engine does next; what the latest
/// check of the outside found, where it found it down; and the round that met it, with what it said.
fn outage(outage: &Outage) -> String {
    let what = capital(cause(outage));
    let until = match outage.until {
        Until::Checked { at } => format!(
            "The engine checks the outside itself at {}, touching no work's session, and every work goes on once it is \
             well.",
            utc(at)
        ),
        Until::Well { at } => format!(
            "The engine's check at {} found it well, and every work goes on.",
            utc(at)
        ),
        Until::Looked { at } => format!(
            "The host is looked at from {} every half minute, and every act goes on once it is reached.",
            utc(at)
        ),
    };
    let checked = match &outage.checked {
        Some(
            checked @ Checked {
                found: Found::Down { why, .. },
                ..
            },
        ) if outage.holds => format!(
            " The check at {}, the {} in the wait, found it down. {}",
            utc(checked.at),
            ordinal(checked.taken),
            sentence(why)
        ),
        _ => String::new(),
    };
    format!(
        "{what} since {}. {until}{checked} Round {} of run {} of W{} met it. {}",
        utc(outage.since),
        outage.round,
        outage.run,
        outage.work,
        sentence(&outage.why)
    )
}

/// A text with its first letter a capital, to begin a sentence.
fn capital(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}

/// A count as a place in order: 1st, 2nd, 3rd, 4th, 11th, 21st.
fn ordinal(n: u32) -> String {
    let end = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{end}")
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
        Writer::Owner => "the owner".to_string(),
        Writer::Outside => "the outside".to_string(),
        Writer::Run { run, cell } => format!("{cell} in run {run}"),
    }
}

/// What of the run went past the lines of its history, a line each: a round whose cost or time did, one under way
/// that has taken longer so far than its line, and a call of a tool that did, or has waited longer for its result.
fn past_of(run: &Run) -> Vec<String> {
    let mut lines = Vec::new();
    for round in &run.rounds {
        let mut said = Vec::new();
        if let Some(cost) = round.judged.cost.filter(|cost| cost.past) {
            said.push(cost_past(&cost));
        }
        if let Some(time) = round.judged.time.filter(|time| time.past) {
            let took = match round.ended {
                Some(_) => "took",
                None => "has been under way for",
            };
            said.push(time_past(took, &time));
        }
        if !said.is_empty() {
            lines.push(format!("    round {} {}", round.number, said.join("; ")));
        }
    }
    for call in &run.calls {
        let (Call::Tool { name, .. }, Some(time)) = (&call.what, call.time.filter(|time| time.past)) else {
            continue;
        };
        let took = match call.done {
            Some(_) => "took",
            None => "has waited for its result for",
        };
        lines.push(format!(
            "    call {} of the tool {name} {}",
            call.call,
            time_past(took, &time)
        ));
    }
    lines
}

/// A cost past its line, as a clause.
fn cost_past(cost: &Against) -> String {
    format!(
        "cost {}, past the line of {} that {}",
        dollars(cost.value),
        dollars(cost.line.value),
        given_by(&cost.line)
    )
}

/// A time past its line, as a clause, after what it did: took, or has been under way for.
fn time_past(took: &str, time: &Against) -> String {
    format!(
        "{took} {}, past the line of {} that {}",
        duration(time.value),
        duration(time.line.value),
        given_by(&time.line)
    )
}

/// What gave a line, as the end of a sentence.
fn given_by(line: &web::works::Line) -> String {
    let of = match line.scope {
        web::works::Scope::Cell => "rounds of its cell",
        web::works::Scope::Flow => "agents' rounds of its flow",
        web::works::Scope::Tool => "calls of its tool",
        web::works::Scope::Works => "other works of its flow",
        web::works::Scope::Line => "rounds of its line",
    };
    format!("{} {of} give", line.samples)
}

/// Millionths of a dollar, in dollars and cents.
fn dollars(micros: u64) -> String {
    format!("${:.2}", micros as f64 / 1_000_000.0)
}

/// How the system ran over the last `days`: what its rounds took, what ended past the lines of its history, the latest
/// first, and what is under way against its own, the oldest first, at most `limit` of each, with how many more there
/// are and how to see them.
fn system_said(system: &System, days: u64, limit: usize) -> String {
    let mut lines = vec![format!(
        "Over the last {days} {}: {}.",
        if days == 1 { "day" } else { "days" },
        spent(&system.spent)
    )];
    let more = |left: usize| {
        format!(
            "  And {left} more: wf-agents system --days {days} --limit {}",
            limit + left
        )
    };
    if system.past.is_empty() {
        lines.push("Nothing ended past the lines of its history.".to_string());
    } else {
        lines.push("Past the lines of their history, the latest first:".to_string());
    }
    for past in system.past.iter().take(limit) {
        lines.push(match past {
            Past::Round {
                work,
                item,
                cell,
                run,
                round,
                ended,
                judged,
            } => {
                let mut said = Vec::new();
                if let Some(cost) = judged.cost.filter(|cost| cost.past) {
                    said.push(cost_past(&cost));
                }
                if let Some(time) = judged.time.filter(|time| time.past) {
                    said.push(time_past("took", &time));
                }
                format!(
                    "  W{work} of #{item}, round {round} of run {run} of {cell}, ended {}: {}",
                    utc(*ended),
                    said.join("; ")
                )
            }
            Past::Call {
                work,
                item,
                cell,
                run,
                call,
                tool,
                done,
                time,
            } => format!(
                "  W{work} of #{item}, call {call} of the tool {tool} by run {run} of {cell}, done {}: {}",
                utc(*done),
                time_past("took", time)
            ),
        });
    }
    if system.past.len() > limit {
        lines.push(more(system.past.len() - limit));
    }
    if system.under_way.is_empty() {
        lines.push("Nothing is under way.".to_string());
    } else {
        lines.push("Under way, the oldest first:".to_string());
    }
    let so_far = |time: &Option<Against>, what: &str| match time {
        Some(time) => format!(
            "{} so far, {} the line of {} that {}",
            duration(time.value),
            if time.past { "past" } else { "within" },
            duration(time.line.value),
            given_by(&time.line)
        ),
        None => format!("no line yet, too few {what} having ended"),
    };
    for under_way in system.under_way.iter().take(limit) {
        lines.push(match under_way {
            UnderWay::Round {
                work,
                item,
                cell,
                run,
                round,
                began,
                time,
                bounds,
            } => {
                let bounds = bounds.map_or_else(String::new, |bounds| {
                    let tokens = |line: Option<web::works::Line>| {
                        line.map_or_else(|| "none yet".to_string(), |line| format!("{} tokens", line.value))
                    };
                    let cost = bounds
                        .cost
                        .map_or_else(|| "none yet".to_string(), |line| dollars(line.value));
                    format!(
                        "; its context's lines {} in all and {} a call, its cost's {cost}",
                        tokens(bounds.added),
                        tokens(bounds.rate)
                    )
                });
                format!(
                    "  W{work} of #{item}, round {round} of run {run} of {cell}, since {}: {}{bounds}",
                    utc(*began),
                    so_far(time, "rounds of its cell")
                )
            }
            UnderWay::Call {
                work,
                item,
                cell,
                run,
                call,
                tool,
                made,
                time,
            } => format!(
                "  W{work} of #{item}, call {call} of the tool {tool} by run {run} of {cell}, since {}: {}",
                utc(*made),
                so_far(time, "calls of its tool")
            ),
        });
    }
    if system.under_way.len() > limit {
        lines.push(more(system.under_way.len() - limit));
    }
    lines.join("\n")
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
                delivers: false,
                added: 0,
                paused: None,
                now: if merged {
                    Now::Merged
                } else {
                    Now::Stalled {
                        at: "merge".to_string(),
                        on: Vec::new(),
                        by: Vec::new(),
                        why: "Nothing new runs this last cell, and no cell can write what runs it.".to_string(),
                    }
                },
                cells: Vec::new(),
                spent: Spent::default(),
                pinged: Spent::default(),
                last: None,
                from: None,
                bar: String::new(),
                sent_back: 0,
                waits: Vec::new(),
                level: Some("1".to_string()),
                cell_level: None,
                raise: None,
                pull: None,
                changed: None,
            },
        }
    }

    fn items(items: Vec<Item>) -> Items {
        Items { items, outage: None }
    }

    /// An observation of the plan's first round of this work, kept at this time.
    fn observation(number: u64, text: &str, read: Option<u64>) -> Observation {
        Observation {
            number,
            text: text.to_string(),
            work: 3,
            item: 2,
            title: "Greet the user".to_string(),
            cell: "plan".to_string(),
            run: 4,
            round: 1,
            kept: 1_800_000_000_000,
            read,
        }
    }

    #[test]
    fn observations_are_said_whole_a_bounded_few_at_a_time_with_the_number_to_ask_for_the_rest() {
        let page = ObservationsPage {
            observations: vec![
                observation(3, "Third.\nIn two lines.", None),
                observation(2, "Second.", Some(1_800_000_060_000)),
                observation(1, "First.", None),
            ],
        };
        assert_eq!(
            observations_said(&page, None, 2),
            "O3, unread, kept 2027-01-15 08:00 UTC: item #2 \"Greet the user\", W3, cell plan, run 4, round 1\n  Third.\n  \
             In two lines.\nO2, read 2027-01-15 08:01 UTC, kept 2027-01-15 08:00 UTC: item #2 \"Greet the user\", W3, \
             cell plan, run 4, round 1\n  Second.\nAnd 1 more, older: wf-agents observations --from 1"
        );
        let down = observations_said(&page, Some(2), 20);
        assert!(down.starts_with("O2, read ") && down.ends_with("  First."), "{down}");
        assert!(!down.contains("O3") && !down.contains("more"), "{down}");
        assert_eq!(
            observations_said(&page, Some(0), 20),
            "No observation is numbered O0 or lower."
        );
        let none = ObservationsPage {
            observations: Vec::new(),
        };
        assert_eq!(observations_said(&none, None, 20), "There are no observations.");
    }

    // The list says what is down while the wait holds, as the latest check found it, since when, and when the engine
    // checks next; once a check found the outside well it holds nothing, and the list says nothing of it.
    #[test]
    fn a_list_says_what_is_down_since_when_and_when_the_engine_checks_next() {
        let hour = 3_600_000;
        let mut waiting = items(vec![item(1, false, &[], &[])]);
        waiting.outage = Some(Outage {
            service: web::works::Service::Agents,
            failure: Failure::Unavailable,
            why: "API Error: 529 Overloaded".to_string(),
            work: 1,
            run: 2,
            round: 1,
            since: hour,
            checked: Some(Checked {
                at: 2 * hour,
                taken: 2,
                found: Found::Down {
                    failure: Failure::CannotRun,
                    why: "No account is in use: choose one on the Settings view.".to_string(),
                },
            }),
            until: Until::Checked { at: 3 * hour },
            holds: true,
        });
        let said = list(&waiting, &Listing::default());
        let first = said.lines().next().unwrap_or_default();
        assert_eq!(
            first,
            "No agent's round of any work begins. The agents' command line cannot run since 1970-01-01 01:00 UTC. The \
             engine checks the outside itself at 1970-01-01 03:00 UTC, touching no work's session, and every work goes \
             on once it is well. The check at 1970-01-01 02:00 UTC, the 2nd in the wait, found it down. No account is \
             in use: choose one on the Settings view. Round 1 of run 2 of W1 met it. API Error: 529 Overloaded."
        );
        let outage = waiting.outage.as_mut().unwrap();
        outage.until = Until::Well { at: 2 * hour };
        outage.holds = false;
        assert!(list(&waiting, &Listing::default()).starts_with("#1 "));
    }

    // `wait` says what the owner does about the agents' command line once, on the line of the wait every work shares,
    // in the words beside the flows; a work's own line says its reasons alone.
    #[test]
    fn wait_says_how_to_mend_the_command_line_once_on_the_line_of_the_wait_every_work_shares() {
        let hour = 3_600_000;
        let mend = "Choose an account on the Settings view, add its token again, or install the command line. The \
                    engine lets the work go on once its check finds that the command line runs.";
        let outage = Outage {
            service: web::works::Service::Agents,
            failure: Failure::CannotRun,
            why: "Not logged in".to_string(),
            work: 1,
            run: 2,
            round: 1,
            since: hour,
            checked: None,
            until: Until::Checked { at: 3 * hour },
            holds: true,
        };
        let line = Line::Agents {
            since: hour,
            outage,
            mend: Some(mend.to_string()),
        };
        let said = heed(&line, 2 * hour);
        assert!(
            said.starts_with(
                "No agent's round of any work begins, for 1h 00m. The agents' command line cannot run since "
            ),
            "{said}"
        );
        assert!(
            said.ends_with(&format!("Round 1 of run 2 of W1 met it. Not logged in. {mend}")),
            "{said}"
        );
        assert_eq!(said.matches("Choose an account").count(), 1, "{said}");
    }

    #[test]
    fn wait_says_a_stalled_work_by_the_engines_sentence_after_how_long_it_stood_so() {
        let hour = 3_600_000;
        let stalled = item(2, false, &[], &[]);
        let line = Line::Stalled {
            of: web::attention::Of {
                work: 2,
                item: 2,
                title: "item 2".to_string(),
            },
            since: hour,
            now: stalled.work.now,
        };
        assert_eq!(
            heed(&line, 2 * hour),
            "Item 2 \"item 2\", work 2: for 1h 00m, stalled at merge. Nothing new runs this last cell, and no cell can \
             write what runs it."
        );
    }

    #[test]
    fn a_work_waiting_for_an_item_that_was_cancelled_is_said_to_wait_for_good() {
        let items = |cancelled: Vec<u64>| Waited::Items {
            items: vec![5, 7],
            cancelled,
        };
        assert_eq!(
            waited(&items(Vec::new())),
            "the items it depends on to merge: #5 and #7"
        );
        assert_eq!(
            waited(&items(vec![7])),
            "the items it depends on to merge: #5 and #7; it waits for good on #7, cancelled"
        );
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
        // The engine says the item it waits for was cancelled; the list says what the engine says.
        let mut waiting = item(2, false, &[1], &["page"]);
        waiting.work.now = Now::Waits {
            at: "S1".to_string(),
            on: vec![Waited::Items {
                items: vec![1],
                cancelled: vec![1],
            }],
        };
        let some = items(vec![cancelled, waiting, item(3, true, &[], &["page"])]);
        let page = |all: bool| Listing {
            tag: Some("page".to_string()),
            from: None,
            all,
        };
        // What ended is left out, and counted, unless all are asked for.
        assert_eq!(
            list(&some, &page(false)),
            "#2 \"item 2\" at level 1, project script, W2 on trial, after #1, tagged page: waits at S1 for the items \
             it depends on to merge: #1; it waits for good on #1, cancelled\nAnd 1 item that ended, merged or \
             cancelled: wf-agents list --tag page --all"
        );
        assert_eq!(
            list(&some, &page(true)),
            "#2 \"item 2\" at level 1, project script, W2 on trial, after #1, tagged page: waits at S1 for the items \
             it depends on to merge: #1; it waits for good on #1, cancelled\n#3 \"item 3\" at level 1, project \
             script, W3 on trial, tagged page: merged"
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
    fn a_list_says_each_items_project_and_an_item_kept_before_projects_as_none() {
        let mut other = item(1, false, &[], &[]);
        other.project = Some("wf-agents".to_string());
        let mut kept = item(2, false, &[], &[]);
        kept.project = None;
        let said = list(&items(vec![other, kept]), &Listing::default());
        let lines: Vec<&str> = said.lines().collect();
        assert!(
            lines[0].starts_with("#1 \"item 1\" at level 1, project wf-agents, W1 on trial: "),
            "{said}"
        );
        assert!(
            lines[1].starts_with("#2 \"item 2\" at level 1, no project, W2 on trial: "),
            "{said}"
        );
    }

    /// A work of the trial flow with this many runs, new sessions and changes of its item, and this many questions, the
    /// first `open` of them not answered.
    fn work(runs: u64, sessions: u64, changes: u64, questions: u64, open: u64) -> WorkPage {
        let item = item(1, false, &[], &[]);
        let mut history = item.history;
        history.changes = (1..=changes)
            .map(|n| web::works::ItemChange {
                time: n * 10,
                why: None,
                what: ItemChanged::Tagged { tag: format!("t{n}") },
            })
            .collect();
        let mut steps: Vec<Step> = (1..=runs)
            .map(|n| {
                Step::Run(Run {
                    run: n,
                    cell: "plan".to_string(),
                    at: n,
                    time: n,
                    because: Vec::new(),
                    basis: Vec::new(),
                    rounds: Vec::new(),
                    calls: Vec::new(),
                    end: None,
                    spent: Spent::default(),
                })
            })
            .collect();
        steps.extend((1..=sessions).map(|n| Step::Session {
            at: n,
            time: n,
            line: "engineer".to_string(),
            why: SessionWhy::Owner,
        }));
        let questions = (1..=questions)
            .map(|n| QuestionPage {
                name: format!("q{n:03}"),
                cell: "plan".to_string(),
                version: Some(n),
                engine: false,
                waits: true,
                tree: false,
                withdrawn: None,
                ask: Some(format!("Which way, {n}?")),
                options: Vec::new(),
                recommends: None,
                answer: (n > open).then_some(web::works::AnswerPage {
                    option: 1,
                    at: n,
                    why: None,
                }),
            })
            .collect();
        WorkPage {
            summary: item.work,
            title: item.title,
            level: item.level,
            project: item.project,
            tags: Vec::new(),
            history,
            outline: web::view::OutlineView {
                kept: true,
                cells: Vec::new(),
                lines: Vec::new(),
                groups: Vec::new(),
                card: Vec::new(),
            },
            flow: Some(web::view::Flow {
                name: "trial".to_string(),
                limits: web::view::Limits {
                    runs: 3,
                    returns: 3,
                    calls: 3,
                },
                lines: Vec::new(),
                tests: Vec::new(),
                tools: Vec::new(),
                groups: Vec::new(),
                card: Vec::new(),
                every_agent: web::view::EveryAgent::default(),
                cells: Vec::new(),
                blocks: Vec::new(),
            }),
            cells: Vec::new(),
            blocks: Vec::new(),
            steps,
            questions,
            speaking: Vec::new(),
        }
    }

    // What a work grows by, its item's history, its questions, its runs and its sessions, is shown at most ten of each,
    // with how many more there are and the command that gives them, so that a work a thousand runs long shows in as
    // many lines as one of twenty.
    // A session begun because what a new one would begin with changed says so, and says the tokens of context of the
    // session it left only where one was told. Milliseconds; the one test of these words.
    #[test]
    fn a_session_begun_for_a_changed_prompt_or_form_says_the_context_it_left_only_where_one_was_told() {
        let changed = "a new one, the rules or the form of return its agents are given having changed since it began";
        assert_eq!(
            session_why(&SessionWhy::Changed { context: Some(86_953) }),
            format!("{changed}, the session it left holding 86953 tokens of context")
        );
        assert_eq!(session_why(&SessionWhy::Changed { context: None }), changed);
    }

    #[test]
    fn a_work_is_shown_in_as_many_lines_however_long_it_grows_and_says_what_it_left_out_and_how_to_see_it() {
        let shown = show(&work(25, 13, 12, 14, 12));
        let lines: Vec<&str> = shown.lines().collect();
        let runs: Vec<&str> = lines.iter().copied().filter(|l| l.starts_with("  run ")).collect();
        assert_eq!(runs.len(), SHOWN, "{shown}");
        assert!(runs[0].starts_with("  run 16 of plan: ") && runs[9].starts_with("  run 25 of plan: "));
        assert!(lines.contains(&"  And 15 earlier: wf-agents show 1 runs"), "{shown}");
        // The latest of the item's history: the third change on.
        let history = format!(
            "; changed since:\n  And 2 earlier: wf-agents show 1 history\n  {}: tagged t3\n",
            utc(30)
        );
        assert!(shown.contains(&history), "{shown}");
        let sessions = lines.iter().filter(|l| l.starts_with("  engineer at ")).count();
        assert_eq!(sessions, SHOWN, "{shown}");
        assert!(lines.contains(&"  And 3 earlier: wf-agents show 1 sessions"), "{shown}");
        // The questions not answered come first: of the twelve, ten are shown, and the four left out are said with how
        // many of them are not answered.
        let asked = lines.iter().filter(|l| l.contains(": Which way, ")).count();
        assert_eq!(asked, SHOWN, "{shown}");
        assert!(
            lines.contains(&"  q001 (of plan, version 1, waits): Which way, 1?"),
            "{shown}"
        );
        assert!(
            !shown.contains("Which way, 13?"),
            "an answered one is left out first: {shown}"
        );
        assert!(
            lines.contains(&"  And 4 more, 2 of them not answered: wf-agents show 1 questions"),
            "{shown}"
        );
        let long = show(&work(1000, 400, 300, 200, 150));
        assert_eq!(long.lines().count(), lines.len(), "{long}");
        assert!(long.contains("  And 990 earlier: wf-agents show 1 runs\n"), "{long}");
        assert!(long.contains("  And 390 earlier: wf-agents show 1 sessions"), "{long}");
        assert!(long.contains("  And 190 more, 140 of them not answered: wf-agents show 1 questions"));
        // Fewer than the bound are each shown, and nothing is said of more.
        let few = show(&work(3, 3, 0, 1, 1));
        assert_eq!(
            few.lines().filter(|l| l.starts_with("  engineer at ")).count(),
            3,
            "{few}"
        );
        assert_eq!(few.lines().filter(|l| l.starts_with("  run ")).count(), 3, "{few}");
        assert!(!few.contains(" earlier: ") && !few.contains(" more"), "{few}");
    }

    // A part given alone comes a hundred entries at a time, from the place asked, each answer saying how to ask for the
    // next.
    #[test]
    fn a_part_of_a_work_is_given_alone_a_hundred_at_a_time_from_the_place_asked() {
        let page = work(250, 0, 0, 0, 0);
        let first = show_part(&page, Part::Runs, 1);
        let lines: Vec<&str> = first.lines().collect();
        assert_eq!(lines[0], "Work 1, its runs, 1 to 100 of 250:");
        assert_eq!(lines.len(), LISTED + 2);
        assert!(lines[1].starts_with("  run 1 of plan: ") && lines[LISTED].starts_with("  run 100 of plan: "));
        assert_eq!(lines[LISTED + 1], "And 150 more: wf-agents show 1 runs --from 101");
        let last = show_part(&page, Part::Runs, 201);
        assert!(
            last.starts_with("Work 1, its runs, 201 to 250 of 250:\n  run 201 of plan: "),
            "{last}"
        );
        assert_eq!(last.lines().count(), 51, "the last answer says nothing more: {last}");
        assert_eq!(
            show_part(&page, Part::Runs, 251),
            "Work 1, its runs: 250, fewer than 251."
        );
        assert_eq!(show_part(&page, Part::Sessions, 1), "Work 1, its sessions: none.");
    }

    // What happened to an item is one history in the order it happened: the changes made to it, and the cancels and
    // reopens of its work.
    #[test]
    fn an_items_history_holds_its_changes_cancels_and_reopens_in_the_order_they_happened() {
        let mut page = work(0, 0, 2, 0, 0);
        page.steps.push(Step::Cancelled {
            at: 1,
            time: 15,
            why: None,
        });
        page.steps.push(Step::Reopened {
            at: 2,
            time: 25,
            why: "Wanted after all.".to_string(),
        });
        let said = show_part(&page, Part::History, 1);
        let lines = [
            "Work 1, what happened to its item, 1 to 4 of 4:".to_string(),
            format!("  {}: tagged t1", utc(10)),
            format!("  {}: cancelled, no reason being given", utc(15)),
            format!("  {}: tagged t2", utc(20)),
            format!("  {}: reopened, because \"Wanted after all.\"", utc(25)),
        ];
        assert_eq!(said, lines.join("\n"));
        assert!(show(&page).contains("; changed since:\n"));
        assert!(show(&work(0, 0, 0, 0, 0)).contains(", and has not changed.\nCells:\n"));
    }

    // The item's own text is what the owner wrote in a block people write, as the flow declares them: never the owner's
    // words to a cell or an answer, which the owner wrote as well, nor what a cell or the outside wrote.
    #[test]
    fn the_item_s_own_text_is_what_the_owner_wrote_in_the_blocks_people_write() {
        use web::view::{Block as Declared, ReadBy, WrittenBy};
        use web::works::{Block, Version};

        let declared = |name: &str, people: bool, outside: Option<&str>| Declared {
            name: name.to_string(),
            values: None,
            default: None,
            what: None,
            writes: Vec::new(),
            written_by: WrittenBy {
                cells: Vec::new(),
                people,
                note: false,
                outside: outside.map(str::to_string),
                changed_by: None,
            },
            read_by: ReadBy {
                cells: Vec::new(),
                conditions: Vec::new(),
                tests: Vec::new(),
                shown: false,
            },
        };
        let written = |name: &str, at: u64, by: Writer| Block {
            name: name.to_string(),
            versions: vec![Version {
                at,
                number: 1,
                time: at,
                by,
                meaning: Some(1),
                value: None,
                said_to: None,
                chars: 5,
                empty: false,
            }],
        };
        let mut page = work(0, 0, 0, 0, 0);
        page.flow.as_mut().expect("a work going on has its flow").blocks = vec![
            declared("request", true, None),
            declared("plan_words", false, Some("words")),
            declared("plan_questions", false, Some("answer")),
            declared("main", false, Some("main")),
        ];
        page.blocks = vec![
            written("request", 1, Writer::Owner),
            written("plan_words", 2, Writer::Owner),
            written("plan_questions", 3, Writer::Owner),
            written("main", 4, Writer::Outside),
        ];
        assert_eq!(texts_of(&page, None), Ok(vec![("request".to_string(), 1)]));
        // A work that ended is given no flow: its item's text is what the card of its version shows.
        page.flow = None;
        page.outline.card = vec!["request".to_string()];
        assert_eq!(texts_of(&page, None), Ok(vec![("request".to_string(), 1)]));
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
    fn the_system_lists_at_most_its_limit_of_what_went_past_its_lines_and_says_how_to_see_the_rest() {
        let line = web::works::Line {
            value: 25_000,
            samples: 19,
            scope: web::works::Scope::Cell,
        };
        let time = Against {
            value: 100_000,
            line,
            past: true,
        };
        let past = |n: u64| Past::Round {
            work: n,
            item: n,
            cell: "I1".to_string(),
            run: 1,
            round: 1,
            ended: 0,
            judged: web::works::Judged {
                cost: None,
                time: Some(time),
            },
        };
        let system = System {
            now: 0,
            from: Some(0),
            spent: Spent::default(),
            pinged: Spent::default(),
            weeks: Vec::new(),
            by_cell: Vec::new(),
            by_model: Vec::new(),
            by_work: Vec::new(),
            merged: Vec::new(),
            cancelled: Vec::new(),
            added: Vec::new(),
            sent_back: Vec::new(),
            failed: Vec::new(),
            past: (1..=12).map(past).collect(),
            under_way: Vec::new(),
        };
        let said = system_said(&system, 7, 10);
        let lines: Vec<&str> = said.lines().collect();
        assert_eq!(lines.len(), 14, "{said}");
        assert_eq!(lines[1], "Past the lines of their history, the latest first:");
        assert_eq!(
            lines[2],
            "  W1 of #1, round 1 of run 1 of I1, ended 1970-01-01 00:00 UTC: took 1m 40s, past the line of 25s that 19 \
             rounds of its cell give"
        );
        assert_eq!(lines[12], "  And 2 more: wf-agents system --days 7 --limit 12");
        assert_eq!(lines[13], "Nothing is under way.");
    }

    #[test]
    fn a_text_in_a_query_is_written_so_that_no_byte_of_it_ends_the_query() {
        assert_eq!(encoded("a b&c=d/é"), "a%20b%26c%3Dd%2F%C3%A9");
        assert_eq!(encoded("3,2.1.0"), "3%2C2.1.0");
    }
}

/// The most of each field of a fork's return that is said, in characters: the whole is kept on the engine's machine, in
/// the file the words name.
const FORK_FIELD: usize = 4_000;

/// The share of what the session held below which a fork is said not to have been read from the cache.
const READ_ENOUGH: u64 = 80;

/// What a fork answered, in words: each field of its return that holds something, by its key, each cut at
/// [`FORK_FIELD`] characters, saying where it is kept whole; else what its program said; then what it cost, the share
/// of the session its first request read from the cache, said plainly where under [`READ_ENOUGH`] percent; where the
/// session went on past the round asked of, that the fork began at that round's end; and where the account the session
/// ran under was removed, or it ran under none, that the fork ran under the account in use instead.
pub fn forked_said(page: &ForkPage) -> String {
    let mut out = String::new();
    let cut = |text: &str| -> String {
        match text.chars().count() > FORK_FIELD {
            true => format!(
                "{}… (cut; whole in {})",
                text.chars().take(FORK_FIELD).collect::<String>(),
                page.kept
            ),
            false => text.to_string(),
        }
    };
    let answer: Option<serde_json::Value> = page.answer.as_deref().and_then(|a| serde_json::from_str(a).ok());
    let mut fields = Vec::new();
    if let Some(answer) = &answer {
        gather("", answer, &mut fields);
    }
    match fields.is_empty() {
        true => out.push_str(&format!("The fork said:\n{}\n", cut(page.text.trim()))),
        false => {
            for (key, text) in fields {
                out.push_str(&format!("{key}:\n{}\n", cut(&text)));
            }
        }
    }
    out.push_str(&format!("\nIt cost ${:.4}.\n", page.usd_micros as f64 / 1e6));
    let share = match (page.read, page.held) {
        (Some(read), Some(held)) if held > 0 => Some((read.min(held) * 100 / held, read, held)),
        _ => None,
    };
    match (share, page.expired) {
        (Some((share, read, held)), _) if share >= READ_ENOUGH => out.push_str(&format!(
            "Its first request read {share}% of the session from the cache ({read} of the {held} tokens it held).\n"
        )),
        (Some((share, read, held)), Some(minutes)) => out.push_str(&format!(
            "Its first request read only {share}% of the session from the cache ({read} of the {held} tokens it held): the \
             session's cache had expired {minutes} minutes before.\n"
        )),
        (Some((share, read, held)), None) => out.push_str(&format!(
            "Its first request read only {share}% of the session from the cache ({read} of the {held} tokens it held), \
             under {READ_ENOUGH}%: the fork did not begin as the session did, and was not read from the cache.\n"
        )),
        (None, _) => out.push_str("What its first request read from the cache, or what the session held, is not known.\n"),
    }
    if !page.later.is_empty() {
        let later: Vec<String> = page
            .later
            .iter()
            .map(|(run, round)| format!("run {run} round {round}"))
            .collect();
        let (run, round) = page.round;
        out.push_str(&format!(
            "It began at the end of run {run} round {round}, the session's later rounds left out: {}.\n",
            later.join(", ")
        ));
    }
    let ran = &page.account;
    match &page.instead {
        Some(ForkInstead::Removed { account }) => out.push_str(&format!(
            "The session ran under the account {account}, which was removed: the fork ran under the account in use, \
             {ran}, whose cache holds nothing of the session.\n"
        )),
        Some(ForkInstead::NoAccount) => out.push_str(&format!(
            "The session ran under no account: the fork ran under the account in use, {ran}, whose cache holds \
             nothing of the session.\n"
        )),
        None => {}
    }
    out.push_str(&format!("The answer is kept in {}.\n", page.kept));
    out
}

/// Each field of a return that holds text, by its key, its path through the return joined by dots.
fn gather(at: &str, value: &serde_json::Value, out: &mut Vec<(String, String)>) {
    let key = |k: &str| {
        if at.is_empty() {
            k.to_string()
        } else {
            format!("{at}.{k}")
        }
    };
    match value {
        serde_json::Value::Object(map) => map.iter().for_each(|(k, v)| gather(&key(k), v, out)),
        serde_json::Value::Array(items) => {
            let texts: Vec<String> = items.iter().filter_map(|i| i.as_str().map(str::to_string)).collect();
            if texts.len() == items.len() && !texts.is_empty() {
                out.push((at.to_string(), texts.join("\n")));
            } else {
                items
                    .iter()
                    .enumerate()
                    .for_each(|(n, v)| gather(&key(&(n + 1).to_string()), v, out));
            }
        }
        serde_json::Value::String(text) if !text.trim().is_empty() => out.push((at.to_string(), text.clone())),
        serde_json::Value::Bool(_) | serde_json::Value::Number(_) => out.push((at.to_string(), value.to_string())),
        _ => {}
    }
}

#[cfg(test)]
mod fork_tests {
    use super::*;

    fn page() -> ForkPage {
        ForkPage {
            answer: Some(r#"{"outcome":{"dealt_with":[],"observations":["Read two ways."],"outputs":{}}}"#.to_string()),
            text: String::new(),
            usd_micros: 41_000,
            read: Some(9_000),
            held: Some(10_000),
            expired: None,
            round: (7, 1),
            later: Vec::new(),
            kept: "/k/7-1.fork.1.answer".to_string(),
            account: "work".to_string(),
            instead: None,
        }
    }

    #[test]
    fn a_forks_answer_is_said_field_by_field_with_its_cost_and_the_share_of_the_session_read_from_the_cache() {
        assert_eq!(
            forked_said(&page()),
            "outcome.observations:\nRead two ways.\n\nIt cost $0.0410.\nIts first request read 90% of the session from \
             the cache (9000 of the 10000 tokens it held).\nThe answer is kept in /k/7-1.fork.1.answer.\n"
        );
        let under = ForkPage {
            read: Some(3_000),
            ..page()
        };
        assert!(forked_said(&under).contains(
            "read only 30% of the session from the cache (3000 of the 10000 tokens it held), under 80%: the fork did \
             not begin as the session did, and was not read from the cache."
        ));
        let expired = ForkPage {
            read: Some(0),
            expired: Some(42),
            ..page()
        };
        assert!(forked_said(&expired).contains("the session's cache had expired 42 minutes before."));
        let unknown = ForkPage { held: None, ..page() };
        assert!(
            forked_said(&unknown)
                .contains("What its first request read from the cache, or what the session held, is not known.")
        );
        let removed = ForkPage {
            instead: Some(ForkInstead::Removed {
                account: "home".to_string(),
            }),
            ..page()
        };
        assert!(forked_said(&removed).contains(
            "The session ran under the account home, which was removed: the fork ran under the account in use, work"
        ));
        let none = ForkPage {
            instead: Some(ForkInstead::NoAccount),
            ..page()
        };
        assert!(
            forked_said(&none)
                .contains("The session ran under no account: the fork ran under the account in use, work, whose cache")
        );
        assert!(!forked_said(&page()).contains("the fork ran under the account in use"));
    }

    #[test]
    fn a_long_field_is_cut_saying_where_it_is_whole_and_the_later_rounds_the_fork_left_out_are_named() {
        let long = ForkPage {
            answer: Some(serde_json::json!({ "outcome": { "observations": ["x".repeat(5_000)] } }).to_string()),
            later: vec![(8, 1), (8, 2)],
            ..page()
        };
        let said = forked_said(&long);
        assert!(
            said.contains(&format!(
                "{}… (cut; whole in /k/7-1.fork.1.answer)",
                "x".repeat(FORK_FIELD)
            )),
            "{said}"
        );
        assert!(
            said.contains(
                "It began at the end of run 7 round 1, the session's later rounds left out: run 8 round 1, run 8 \
                 round 2."
            ),
            "{said}"
        );
        // No return: what the program said is said.
        let text = ForkPage {
            answer: None,
            text: "It said this.".to_string(),
            ..page()
        };
        assert!(forked_said(&text).starts_with("The fork said:\nIt said this.\n"));
    }
}
