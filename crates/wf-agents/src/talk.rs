//! The commands that talk to a running engine over the owner's interface, for the owner and their assistant to manage
//! items from a terminal: each sends one request on the loopback address and says, in plain text a person and an
//! agent can both read, what came of it. A refusal is said on the error stream in the engine's own words, and the
//! command fails.

use std::net::{Ipv4Addr, SocketAddr};
use std::process::ExitCode;

use flow::agents::sentence;
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::{Method, Request, StatusCode, header};
use hyper_util::rt::TokioIo;
use serde::Serialize;
use serde::de::DeserializeOwned;
use web::attention::{Asked as AskedAhead, Attention, Line, Mark, Of, Stuck};
use web::observations::{
    NewObservation, Observation, ObservationAdded, ObservationChangeAsked, ObservationChanges, ObservationsChanged,
    ObservationsPage, TagsPage,
};
use web::plan::{Candidates, Field, Graph, Hits, Node, PlanChange, PlanChanges, Row, Standing, Tags};
use web::pulls::{OpenPull, Pulls};
use web::system::{Figure, Going, LeftOf, Past, RedTest, Rest, System, UnderWay, WaitReason, Waited};
use web::works::{
    Act, Added, Against, Back, Call, CellState, ChangeAsked, Checked, Continues, Engine, EngineAct, Failure, ForkAsked,
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
    /// The engine stopped: once what is under way has ended, or, where `now`, at once, which cuts that off and is done
    /// only where `yes` confirms it, having said without it what it would cut off.
    Stop {
        now: bool,
        yes: bool,
    },
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
    /// The pull requests open on the hosts of the projects' repositories: of this project alone, where one is named,
    /// and numbered this or more, where a number is.
    Pulls {
        project: Option<String>,
        from: Option<u64>,
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
    /// The observations that are shown, the newest first, from this number down, at most this many.
    Observations {
        from: Option<u64>,
        limit: usize,
        showing: Showing,
    },
    /// An observation an architect adds.
    AddObservation(NewObservation),
    /// Changes of observations made together, for this reason.
    ChangeObservations {
        why: String,
        changes: Vec<ObservationChangeAsked>,
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
                let (item, work) = (item_number(added.item), work_number(added.work));
                Ok(format!(
                    "Added item {item} with its work {work}, paused: let it on with `wf-agents resume {}`.",
                    added.work
                ))
            }
            Asked::List(listing) => {
                let engine: Engine = send(address, Method::GET, "/api/v1/engine", None::<&()>).await?;
                let items: Items = send(address, Method::GET, "/api/v1/items", None::<&()>).await?;
                Ok(format!("{}\n{}", standing(&engine), list(&items, &listing, clock())))
            }
            Asked::Change { why, changes } => {
                let asked = ItemChanges {
                    why,
                    changes: changes.clone(),
                };
                let done: ItemsChanged = send(address, Method::POST, "/api/v1/items/changes", Some(&asked)).await?;
                // A reopen says how to let the work on, and a dependency whether the item depended on is cancelled:
                // each needs the items as they now are.
                let looked = (changes.iter())
                    .any(|c| matches!(c, ChangeAsked::Reopen { .. } | ChangeAsked::DependsOn { .. }));
                let works = match looked {
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
            Asked::Pulls { project, from } => {
                let path = match &project {
                    Some(project) => format!("/api/v1/pulls?project={}", encoded(project)),
                    None => "/api/v1/pulls".to_string(),
                };
                let pulls: Pulls = send(address, Method::GET, &path, None::<&()>).await?;
                Ok(pulls_said(&pulls, from, clock()))
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
                let observed: TagsPage = send(address, Method::GET, "/api/v1/observations/tags", None::<&()>).await?;
                Ok(tags_said(&tags, &observed))
            }
            Asked::Text { item, block, whole } => {
                let items: Items = send(address, Method::GET, "/api/v1/items", None::<&()>).await?;
                let Some(work) = items.items.iter().find(|i| i.number == item).map(|i| i.work.number) else {
                    return Err(format!("There is no item {item}.", item = item_number(item)));
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
            Asked::Observations { from, limit, showing } => {
                let page: ObservationsPage = send(address, Method::GET, "/api/v1/observations", None::<&()>).await?;
                Ok(observations_said(&page, &showing, from, limit))
            }
            Asked::AddObservation(observation) => {
                let added: ObservationAdded =
                    send(address, Method::POST, "/api/v1/observations", Some(&observation)).await?;
                Ok(format!("Added observation O{}.", added.number))
            }
            Asked::ChangeObservations { why, changes } => {
                let asked = ObservationChanges {
                    why,
                    changes: changes.clone(),
                };
                let done: ObservationsChanged =
                    send(address, Method::POST, "/api/v1/observations/changes", Some(&asked)).await?;
                Ok(observations_changed(&changes, &done.changed))
            }
            Asked::Everything(act) => {
                send::<(), _>(address, Method::POST, "/api/v1/engine/acts", Some(&act)).await?;
                let engine: Engine = send(address, Method::GET, "/api/v1/engine", None::<&()>).await?;
                Ok(standing(&engine))
            }
            Asked::Stop { now, yes } => {
                // What is under way is read first: what a stop at once would cut off, and what one waits for.
                let path = format!("/api/v1/system?weeks={}", clock());
                let system: System = send(address, Method::GET, &path, None::<&()>).await?;
                if now && !yes {
                    return Ok(stop_said(&system, Stopped::NotYet));
                }
                let act = EngineAct::Stop { now };
                if let Err(why) = send::<(), _>(address, Method::POST, "/api/v1/engine/acts", Some(&act)).await {
                    // An engine that ends on the stop may end before its answer is sent: one that no longer answers
                    // has stopped.
                    if send::<Engine, ()>(address, Method::GET, "/api/v1/engine", None).await.is_ok() {
                        return Err(why);
                    }
                }
                Ok(stop_said(&system, if now { Stopped::AtOnce } else { Stopped::Once }))
            }
            Asked::Show { work, part, from } => {
                let page: WorkPage = send(address, Method::GET, &format!("/api/v1/works/{work}"), None::<&()>).await?;
                Ok(match part {
                    None => show(&page, clock()),
                    Some(part) => show_part(&page, part, from.unwrap_or(1)),
                })
            }
            Asked::System { days, limit } => {
                let from = clock().saturating_sub(days.saturating_mul(DAY));
                let path = format!("/api/v1/system?weeks={from}");
                let system: System = send(address, Method::GET, &path, None::<&()>).await?;
                let going: Vec<Going> = send(address, Method::GET, "/api/v1/system/under-way", None::<&()>).await?;
                Ok(system_said(&system, &going, days, limit))
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
                    Now::Everything { .. } => {
                        let work = work_number(work);
                        format!("Work {work} is let on, and held back by the pause of everything until `wf-agents resume --all`.")
                    }
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
                let work = work_number(work);
                match after.summary.now {
                    Now::Cancelled => Ok(format!("Work {work} is cancelled.")),
                    Now::Merged => Err(format!("Work {work} is not cancelled: it has merged.")),
                    now => Err(format!("Work {work} is not cancelled: it {}", sentence(&self::now(&now)))),
                }
            }
            Asked::Wait { since, longest } => {
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
                        break Ok(attention(&answer, clock()));
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
    /// Why the item is added paused, and what its pause waits for.
    pub paused: Paused,
}

/// Why a work is paused, and the items its pause waits for, as the owner says them as they pause it or add it paused.
pub struct Paused {
    pub why: Option<String>,
    pub waits_for: Vec<u64>,
}

/// The new item, with what is written for it: each text, then each file's content, then each choice, in the order
/// given. It is added paused, for the reason given where there is one.
pub fn new_item(
    title: String,
    level: Option<String>,
    project: String,
    flow: String,
    depends_on: Vec<u64>,
    tags: Vec<String>,
    written: Written<'_>,
) -> Result<NewItem, String> {
    let Written {
        texts,
        files,
        choices,
        paused,
    } = written;
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
        continues: None,
        flow,
        writes,
        why: paused.why,
        waits_for: paused.waits_for,
    })
}

/// What `--continue-branch` and `--continue-pull` ask an item to continue; none where neither is given. The command
/// line refuses both together.
pub fn continues(branch: Option<String>, pull: Option<u64>) -> Option<Continues> {
    match (branch, pull) {
        (Some(name), _) => Some(Continues::Branch { name }),
        (None, Some(number)) => Some(Continues::Pull { number }),
        (None, None) => None,
    }
}

/// What an item continues, in words: `the branch <name>` or `pull request <number>`.
fn continues_words(continues: &Continues) -> String {
    match continues {
        Continues::Branch { name } => format!("the branch {name}"),
        Continues::Pull { number } => format!("pull request {number}"),
    }
}

/// The changes `wf-agents change` is given, each `ITEM:KIND` or `ITEM:KIND=VALUE`, in the order given.
pub fn changes(given: &[String]) -> Result<Vec<ChangeAsked>, String> {
    let usage = |given: &str, why: &str| {
        format!(
            "{given:?} is no change: {why}. A change is ITEM:KIND or ITEM:KIND=VALUE, the kind one of title=TEXT, \
             level=LEVEL, depends-on=ITEM, no-longer-depends-on=ITEM, tag=TAG, untag=TAG, continue-branch=BRANCH, \
             continue-pull=PULL, no-longer-continues, cancel and reopen"
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
            "continue-branch" => ChangeAsked::Continues {
                item,
                continues: Some(Continues::Branch { name: text(value)? }),
            },
            "continue-pull" => ChangeAsked::Continues {
                item,
                continues: Some(Continues::Pull {
                    number: (value.as_deref().map(|v| v.trim().parse::<u64>()))
                        .and_then(Result::ok)
                        .ok_or_else(|| usage(change, "it needs a pull request's number after ="))?,
                }),
            },
            "no-longer-continues" => {
                none(&value)?;
                ChangeAsked::Continues { item, continues: None }
            }
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

/// A work's number as the engine writes it in words, everywhere: `W5`.
fn work_number(number: u64) -> String {
    store::WorkNumber(number).to_string()
}

/// An item's number as the engine writes it in words, everywhere: `#5`.
fn item_number(number: u64) -> String {
    store::ItemNumber(number).to_string()
}

/// What a command that acts says once it is done, with what the engine answered: of words said, the cell it gave them
/// to.
fn done(work: u64, act: &Act, said: Option<&Said>) -> String {
    let work = work_number(work);
    match act {
        Act::Write { block, .. } => format!("Wrote {block} for work {work}."),
        Act::LetGo { cell } => format!("Cell {cell} of work {work} goes on."),
        Act::GoOn { call } => format!("The run of work {work} whose call {call} was at its limit goes on."),
        Act::Pause { .. } => format!("Work {work} is paused."),
        Act::Resume {} => format!("Work {work} goes on."),
        Act::Cancel { .. } => format!("Work {work} is cancelled."),
        Act::Result { call, .. } => format!("Call {call} of work {work} has its result."),
        Act::Answer {
            question,
            version,
            option,
            ..
        } => format!("Version {version} of the question {question} of work {work} is answered with option {option}."),
        Act::Read { versions } => format!("{} versions of work {work} are read.", versions.len()),
        Act::Say { to, .. } => {
            let cell = said.map_or("the cell the engine chose", |s| s.reached.as_str());
            match to {
                SpokenTo::Line { line } => format!("Said to {cell} of work {work}, where the line {line} stands."),
                SpokenTo::Cell { .. } => format!("Said to {cell} of work {work}."),
            }
        }
    }
}

/// How a stop was asked for: at once and not yet confirmed, at once, or once what is under way has ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stopped {
    NotYet,
    AtOnce,
    Once,
}

/// The most rounds under way a stop lists.
const STOP_LISTED: usize = 20;

/// What a stop does to what is under way, as `system` read it just before: each round under way, with how long it has
/// taken and how much longer it is expected to take; for a stop at once not yet confirmed, that nothing was stopped.
fn stop_said(system: &System, stopped: Stopped) -> String {
    let rounds: Vec<&UnderWay> = (system.under_way.iter())
        .filter(|u| matches!(u, UnderWay::Round { .. }))
        .collect();
    let latest = web::system::rest_of_rounds(&system.under_way);
    let mut listed = Vec::new();
    for under_way in &rounds {
        let UnderWay::Round {
            work,
            item,
            cell,
            began,
            rest,
            ..
        } = under_way
        else {
            continue;
        };
        let rest = rest.unwrap_or(Rest::Longer {});
        let to_come = match rest {
            Rest::Expected { ms } => format!("about {} more", duration(ms)),
            Rest::Longer {} => "longer than any round of its history".to_string(),
        };
        listed.push(format!(
            "  {work} of {item} at {cell}: {} so far, {to_come}",
            duration(system.now.saturating_sub(*began)),
            work = work_number(*work),
            item = item_number(*item)
        ));
    }
    if listed.len() > STOP_LISTED {
        let left = listed.len() - STOP_LISTED;
        listed.truncate(STOP_LISTED);
        listed.push(format!("  And {left} more."));
    }
    let n = rounds.len();
    let these = if n == 1 {
        "this round".to_string()
    } else {
        format!("these {n} rounds")
    };
    let head = match stopped {
        Stopped::NotYet if n == 0 => {
            return "Nothing is stopped: no round is under way, and `wf-agents stop --now --yes` stops the engine at \
                    once."
                .to_string();
        }
        Stopped::NotYet => format!(
            "Nothing is stopped: `wf-agents stop --now --yes` stops the engine at once and cuts off {these}, which \
             `wf-agents stop` waits for:"
        ),
        Stopped::AtOnce if n == 0 => return "The engine stopped at once; no round was under way.".to_string(),
        Stopped::AtOnce => format!("The engine stopped at once, cutting off {these}:"),
        Stopped::Once if n == 0 => return "The engine stopped; no round was under way.".to_string(),
        Stopped::Once => {
            let ends = match latest {
                Rest::Expected { ms } => format!("expected in about {}", duration(ms)),
                Rest::Longer {} => "once a round that has taken longer than any before has ended".to_string(),
            };
            format!("The engine is stopping: no round begins, and it ends once {these} have ended, {ends}:")
        }
    };
    std::iter::once(head).chain(listed).collect::<Vec<_>>().join("\n")
}

/// The engine as a whole, in a line: whether it is stopping, whether everything is paused and by whom, and its slots.
fn standing(engine: &Engine) -> String {
    if let Some(stop) = engine.stopping {
        let how = if stop.at_once {
            "at once"
        } else {
            "once what is under way has ended"
        };
        return format!(
            "The engine is stopping {how}: no round begins; {} of {} slots taken.",
            engine.taken, engine.slots
        );
    }
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

/// The most items `list` gives in one answer, the most pull requests `pulls` gives in one, and the most entries of a part
/// of a work `show` gives alone.
const LISTED: usize = 100;

/// Which observations a list gives: those carrying the tag alone, where one is given; those of the project alone,
/// where one is given; and those an item took in that is not cancelled, only where `all`.
#[derive(Clone, Debug, Default)]
pub struct Showing {
    pub tag: Option<String>,
    pub project: Option<String>,
    pub all: bool,
}

/// The observation an architect adds: its text, from the words given or from a file, its title, its tags, the project
/// it is of and the project that reported it.
pub fn new_observation(
    project: String,
    reported_by: String,
    title: Option<String>,
    tags: Vec<String>,
    text: Option<String>,
    file: Option<&std::path::Path>,
) -> Result<NewObservation, String> {
    let text = match (text, file) {
        (Some(text), None) => text,
        (None, Some(path)) => std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?,
        _ => return Err("An observation is given its text by --text or by --file, one of them.".to_string()),
    };
    Ok(NewObservation {
        text,
        title,
        tags,
        project,
        reported_by,
    })
}

/// The changes `wf-agents change-observations` is given, each `OBSERVATION:KIND` or `OBSERVATION:KIND=VALUE`, in the
/// order given.
pub fn observation_changes(given: &[String]) -> Result<Vec<ObservationChangeAsked>, String> {
    let usage = |given: &str, why: &str| {
        format!(
            "{given:?} is no change: {why}. A change is OBSERVATION:KIND or OBSERVATION:KIND=VALUE, the observation \
             its number with or without the O, the kind one of title=TEXT, tag=TAG, untag=TAG, taken-in-by=ITEM and \
             delete"
        )
    };
    let mut changes = Vec::new();
    for change in given {
        let Some((observation, rest)) = change.split_once(':') else {
            return Err(usage(change, "it names no observation before a colon"));
        };
        let observation: u64 = (observation.trim().trim_start_matches('O').parse())
            .map_err(|_| usage(change, "what is before the colon is no observation's number"))?;
        let (kind, value) = match rest.split_once('=') {
            Some((kind, value)) => (kind, Some(value.to_string())),
            None => (rest, None),
        };
        let text = |value: Option<String>| value.ok_or_else(|| usage(change, "it needs a value after ="));
        changes.push(match kind {
            "title" => ObservationChangeAsked::Title {
                observation,
                title: text(value)?,
            },
            "tag" => ObservationChangeAsked::Tag {
                observation,
                tag: text(value)?,
            },
            "untag" => ObservationChangeAsked::Untag {
                observation,
                tag: text(value)?,
            },
            "taken-in-by" => {
                let item = (value
                    .as_deref()
                    .map(|v| v.trim().trim_start_matches('#').parse::<u64>()))
                .and_then(Result::ok)
                .ok_or_else(|| usage(change, "it needs an item's number after ="))?;
                ObservationChangeAsked::TakenIn { observation, item }
            }
            "delete" => {
                if value.is_some() {
                    return Err(usage(change, "it takes no value"));
                }
                ObservationChangeAsked::Delete { observation }
            }
            _ => return Err(usage(change, "its kind is none of these")),
        });
    }
    Ok(changes)
}

/// What came of each change of observations, a line each, in the order asked.
fn observations_changed(changes: &[ObservationChangeAsked], done: &[bool]) -> String {
    let lines: Vec<String> = (changes.iter().zip(done))
        .map(|(change, done)| match (change, *done) {
            (ObservationChangeAsked::Title { observation, title }, true) => {
                format!("O{observation} is titled \"{title}\".")
            }
            (ObservationChangeAsked::Title { observation, title }, false) => {
                format!("O{observation} was titled \"{title}\" already.")
            }
            (ObservationChangeAsked::Tag { observation, tag }, true) => format!("O{observation} is tagged {tag}."),
            (ObservationChangeAsked::Tag { observation, tag }, false) => {
                format!("O{observation} was tagged {tag} already.")
            }
            (ObservationChangeAsked::Untag { observation, tag }, true) => {
                format!("O{observation} is no longer tagged {tag}.")
            }
            (ObservationChangeAsked::Untag { observation, tag }, false) => {
                format!("O{observation} was not tagged {tag}.")
            }
            (ObservationChangeAsked::TakenIn { observation, item }, true) => {
                format!("O{observation} is taken in by {}.", item_number(*item))
            }
            (ObservationChangeAsked::TakenIn { observation, item }, false) => {
                format!("O{observation} was taken in by {} already.", item_number(*item))
            }
            (ObservationChangeAsked::Delete { observation }, _) => {
                format!("O{observation} is deleted: its text and why stay in the records.")
            }
        })
        .collect();
    lines.join("\n")
}

/// Which items a list gives: those carrying the tag alone, where one is given; from the item given; and those that
/// ended, merged or cancelled, only where `all`.
#[derive(Clone, Debug, Default)]
pub struct Listing {
    pub tag: Option<String>,
    pub from: Option<u64>,
    pub all: bool,
}

/// Every item a list gives, a line each, with what its work does now and when something last happened to it, at most
/// [`LISTED`]: how many more there are, and how to see them, is said, and how many ended are left out. `now` is the
/// time the list is made, to say how long ago that was.
fn list(items: &Items, listing: &Listing, now: u64) -> String {
    if items.items.is_empty() {
        return "No items.".to_string();
    }
    let (tag, from) = (listing.tag.as_deref(), listing.from);
    let mut lines: Vec<String> = (items.outage.iter())
        .filter(|o| o.holds)
        .map(|o| shared_wait(o, None, None))
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
            let on: Vec<String> = item.depends_on.iter().map(|on| item_number(*on)).collect();
            format!(", after {}", on.join(", "))
        };
        let tags = match item.tags.is_empty() {
            true => String::new(),
            false => format!(", tagged {}", item.tags.join(", ")),
        };
        let tags = match &item.continues {
            Some(continues) => format!("{tags}, continuing {}", continues_words(continues)),
            None => tags,
        };
        // What it does now says which items it waits for, as the engine says.
        let work = &item.work;
        lines.push(format!(
            "{} \"{}\" at {}, {} on {}{after}{tags}: {} ({})",
            item_ref(item.number, item.project.as_deref()),
            item.title,
            level(item.level.as_deref()),
            work_number(work.number),
            work.flow,
            self::now(&work.now),
            last_said(work.last, now)
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

/// The open pull requests, a line each, at most [`LISTED`]: each says its project, number, title, whether it is a draft,
/// when it was last updated, its address and the work that keeps it, what that work does now and when something last
/// happened to it, or that no work keeps it. A project whose host could not say, and one whose host has no pull
/// requests, are each said on a line of their own first. What is left out is said by project, with the command that
/// gives it. `now` is the time it is said, to say how long ago things were.
fn pulls_said(pulls: &Pulls, from: Option<u64>, now: u64) -> String {
    let mut lines: Vec<String> = (pulls.failed.iter())
        .map(|unsaid| {
            format!(
                "Project {}: its host could not say which pull requests are open. {}",
                unsaid.project,
                sentence(&unsaid.why)
            )
        })
        .collect();
    lines.extend((pulls.without.iter()).map(|project| format!("Project {project}: its host has no pull requests.")));
    let chosen: Vec<&OpenPull> = (pulls.pulls.iter())
        .filter(|pull| from.is_none_or(|from| pull.number >= from))
        .collect();
    if chosen.is_empty() && lines.is_empty() {
        return format!(
            "No pull request is open{}.",
            from.map_or_else(String::new, |from| format!(" numbered {from} or more"))
        );
    }
    lines.extend(chosen.iter().take(LISTED).map(|pull| pull_line(pull, now)));
    let rest = chosen.get(LISTED..).unwrap_or_default();
    let mut said: Vec<&str> = Vec::new();
    for pull in rest {
        if said.contains(&pull.project.as_str()) {
            continue;
        }
        said.push(&pull.project);
        let of: Vec<&&OpenPull> = rest.iter().filter(|other| other.project == pull.project).collect();
        lines.push(format!(
            "And {} more of {}: wf-agents pulls --project {} --from {}",
            of.len(),
            pull.project,
            pull.project,
            pull.number
        ));
    }
    lines.join("\n")
}

/// An open pull request, as a line of the list of them.
fn pull_line(pull: &OpenPull, now: u64) -> String {
    let draft = if pull.draft { " (draft)" } else { "" };
    let keeper = match &pull.work {
        Some(kept) => {
            let (work, item) = (work_number(kept.work), item_number(kept.item));
            format!(
                "{work} of item {item}: {} ({})",
                self::now(&kept.now),
                last_said(kept.last, now)
            )
        }
        None => "no work keeps it.".to_string(),
    };
    format!(
        "{} pull request {} \"{}\"{draft}, updated {} ago, {}: {keeper}",
        pull.project,
        pull.number,
        pull.title,
        duration(now.saturating_sub(pull.updated)),
        pull.url
    )
}

/// From where a list begins, as the end of a sentence.
fn from_words(from: Option<u64>) -> String {
    from.map_or_else(String::new, |from| format!(" from {} on", item_number(from)))
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

/// One work: what it does now, when something last happened to it, its pull request, its cells and blocks and what it
/// took; and of each part that grows with it, at most [`SHOWN`] entries, the latest, or of the questions those not
/// answered first, with how many more there are and the command that gives them. `now` is the time it is shown, to say
/// how long ago that was.
fn show(page: &WorkPage, now: u64) -> String {
    let summary = &page.summary;
    let work = summary.number;
    let happened = entries(page, Part::History);
    let (named_work, named_item) = (work_number(work), item_number(summary.item));
    let mut lines = vec![
        format!(
            "Work {named_work} of item {named_item} \"{}\" at {}, {}, on the flow {}.",
            page.title,
            level(page.level.as_deref()),
            project(page.project.as_deref()),
            summary.flow
        ),
        format!("Now: {}", sentence(&self::now(&summary.now))),
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
        format!(
            "Last happened: {}.",
            match summary.last {
                Some(at) => format!("{}, {} ago", utc(at), duration(now.saturating_sub(at))),
                None => "nothing yet".to_string(),
            }
        ),
        format!(
            "Pull request: {}.",
            match &summary.pull {
                Some(pull) => format!("{}, {}", pull.number, pull.url),
                None => "none".to_string(),
            }
        ),
        format!("Item {named_item} {}", added(&page.history, happened.is_empty())),
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
    let (number, said) = (page.summary.number, part.said());
    let work = work_number(number);
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
            "And {} more: wf-agents show {number} {} --from {}",
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

/// How a round missed its session's cache and why: what differed from the call before, that nothing did, which is a
/// fault of wf-agents, or that the records do not say.
fn missed_said(number: usize, missed: &web::works::Missed) -> String {
    let cause = match (missed.differs.is_empty(), missed.unknown) {
        (false, _) => format!("{} differed from the call before", missed.differs.join(", ")),
        (true, true) => "the cause is not known: the records do not say what the call before was given, or the \
            prompt grew shorter with no compaction recorded"
            .to_string(),
        (true, false) => "nothing before the messages differed, a fault of wf-agents".to_string(),
    };
    format!(
        "    round {number} missed its session's cache: read {} of the {} tokens it held; {cause}",
        missed.read, missed.held
    )
}

/// A run, in lines: its rounds and calls, the context its last round ended with, its session's compactions and how it
/// ended; then each of its rounds that missed its session's prompt cache, with what differed before the messages, or
/// that nothing did, which is a fault of wf-agents, or that the records do not say; then what of it went past the
/// lines of its history.
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
        lines.push(missed_said(round.number, missed));
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
        let on: Vec<String> = first.depends_on.iter().map(|on| item_number(*on)).collect();
        format!(", after {}", on.join(", "))
    };
    let tags = match first.tags.is_empty() {
        true => String::new(),
        false => format!(", tagged {}", first.tags.join(", ")),
    };
    let tags = match &first.continues {
        Some(continues) => format!("{tags}, continuing {}", continues_words(continues)),
        None => tags,
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
        ItemChanged::DependsOn { item } => format!("came to depend on {}", item_number(*item)),
        ItemChanged::NoLongerDependsOn { item } => format!("no longer depends on {}", item_number(*item)),
        ItemChanged::Tagged { tag } => format!("tagged {tag}"),
        ItemChanged::Untagged { tag } => format!("no longer tagged {tag}"),
        ItemChanged::Continues {
            continues: Some(continues),
        } => {
            format!("came to continue {}", continues_words(continues))
        }
        ItemChanged::Continues { continues: None } => "no longer continues anything".to_string(),
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

/// An item as a line of a command that lists items names it: its number and its project, so that items of several
/// projects are told apart where they are listed together. Every such line begins with it.
fn item_ref(number: u64, project: Option<&str>) -> String {
    format!("{} ({})", item_number(number), self::project(project))
}

/// When something last happened to a work, how long ago, as a clause: none where nothing has.
fn last_said(last: Option<u64>, now: u64) -> String {
    match last {
        Some(at) => format!("last happened {} ago", duration(now.saturating_sub(at))),
        None => "nothing has happened yet".to_string(),
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
/// A reopened item says how to let its work on, and an item that came to depend on a cancelled one says that its work
/// does not wait for it.
fn changed(changes: &[ChangeAsked], done: &[bool], items: Option<&Items>) -> String {
    let of = |item: u64| items.and_then(|items| items.items.iter().find(|i| i.number == item));
    let work_of = |item: u64| of(item).map(|i| i.work.number);
    let cancelled = |item: u64| of(item).is_some_and(|i| matches!(i.work.now, Now::Cancelled));
    let lines: Vec<String> = (changes.iter().zip(done))
        .map(|(change, done)| match (change, *done) {
            (ChangeAsked::Title { item, title }, true) => {
                format!(
                    "{item} is titled \"{title}\"; the title it had is kept in its history.",
                    item = item_number(*item)
                )
            }
            (ChangeAsked::Title { item, title }, false) => {
                format!("{item} was titled \"{title}\" already.", item = item_number(*item))
            }
            (ChangeAsked::Level { item, level }, true) => {
                format!(
                    "{item} is at level {level}: its work's rounds run on it from now on.",
                    item = item_number(*item)
                )
            }
            (ChangeAsked::Level { item, level }, false) => {
                format!("{item} was at level {level} already.", item = item_number(*item))
            }
            (ChangeAsked::DependsOn { item, on }, true) if cancelled(*on) => {
                format!(
                    "{item} depends on {on}, which is cancelled: its work does not wait for it.",
                    item = item_number(*item),
                    on = item_number(*on)
                )
            }
            (ChangeAsked::DependsOn { item, on }, true) => {
                format!(
                    "{item} depends on {on}: its work waits for it to merge.",
                    item = item_number(*item),
                    on = item_number(*on)
                )
            }
            (ChangeAsked::DependsOn { item, on }, false) => format!(
                "{item} depended on {on} already.",
                item = item_number(*item),
                on = item_number(*on)
            ),
            (ChangeAsked::NoLongerDependsOn { item, on }, _) => format!(
                "{item} no longer depends on {on}.",
                item = item_number(*item),
                on = item_number(*on)
            ),
            (ChangeAsked::Tag { item, tag }, true) => format!("{item} is tagged {tag}.", item = item_number(*item)),
            (ChangeAsked::Tag { item, tag }, false) => {
                format!("{item} was tagged {tag} already.", item = item_number(*item))
            }
            (ChangeAsked::Untag { item, tag }, _) => {
                format!("{item} is no longer tagged {tag}.", item = item_number(*item))
            }
            (
                ChangeAsked::Continues {
                    item,
                    continues: Some(continues),
                },
                true,
            ) => format!(
                "{item} continues {}: its work starts from the head of that, until its branch is made.",
                continues_words(continues),
                item = item_number(*item)
            ),
            (
                ChangeAsked::Continues {
                    item,
                    continues: Some(continues),
                },
                false,
            ) => format!(
                "{item} continued {} already.",
                continues_words(continues),
                item = item_number(*item)
            ),
            (ChangeAsked::Continues { item, continues: None }, true) => {
                format!(
                    "{item} continues nothing: its work starts from the head of main.",
                    item = item_number(*item)
                )
            }
            (ChangeAsked::Continues { item, continues: None }, false) => {
                format!("{item} continued nothing already.", item = item_number(*item))
            }
            (ChangeAsked::Cancel { item }, _) => format!("{item} is cancelled.", item = item_number(*item)),
            (ChangeAsked::Reopen { item }, _) => match work_of(*item) {
                Some(work) => format!(
                    "{item} is reopened, paused: let it on with `wf-agents resume {work}`.",
                    item = item_number(*item)
                ),
                None => format!("{item} is reopened, paused.", item = item_number(*item)),
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
                        let on: Vec<String> = depends_on.iter().map(|on| item_number(*on)).collect();
                        format!(", after {}", on.join(", "))
                    }
                };
                let tags = match tags.is_empty() {
                    true => String::new(),
                    false => format!(", tagged {}", tags.join(", ")),
                };
                format!(
                    "{} added as \"{}\" at {}{after}{tags}",
                    item_ref(line.item, line.project.as_deref()),
                    line.title,
                    level(at.as_deref())
                )
            }
            PlanChange::Changed { change } => format!(
                "{} {}",
                item_ref(line.item, line.project.as_deref()),
                item_changed(change)
            ),
            PlanChange::Cancelled => format!("{} cancelled", item_ref(line.item, line.project.as_deref())),
            PlanChange::Reopened => format!("{} reopened", item_ref(line.item, line.project.as_deref())),
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
    format!(
        "{} \"{}\" at {}{tags}, {}",
        item_ref(row.item, row.project.as_deref()),
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
    if candidates.waiting > 0 {
        let (items, waits) = match candidates.waiting {
            1 => ("item is", "its pause waits"),
            _ => ("items are", "their pauses wait"),
        };
        lines.push(format!(
            "{} paused {items} not among them: {waits} for items that have neither merged nor been cancelled \
             (`wf-agents list` says which).",
            candidates.waiting
        ));
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
            "{} \"{}\", {field}: {}{again}",
            item_ref(hit.item, hit.project.as_deref()),
            hit.title,
            hit.snippet
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
fn tags_said(tags: &Tags, observed: &TagsPage) -> String {
    let mut names: Vec<&str> = (tags.tags.iter().map(|t| t.tag.as_str()))
        .chain(observed.tags.iter().map(|t| t.tag.as_str()))
        .collect();
    names.sort_unstable();
    names.dedup();
    if names.is_empty() {
        return "No item or observation carries a tag.".to_string();
    }
    let lines: Vec<String> = (names.into_iter())
        .map(|name| {
            let mut parts = Vec::new();
            if let Some(tag) = tags.tags.iter().find(|t| t.tag == name) {
                parts.push(format!("{} open of {}", tag.open, tag.items));
            }
            if let Some(tag) = observed.tags.iter().find(|t| t.tag == name) {
                parts.push(format!(
                    "{} not handled of {} observations",
                    tag.unhandled, tag.carrying
                ));
            }
            format!("{name}: {}", parts.join("; "))
        })
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
                    "{} has no version of {name}: the blocks it has are {}.",
                    item_number(page.summary.item),
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
    let mut lines = vec![format!("{} \"{}\":", item_number(item), page.title)];
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

/// What needs the planner, a line each, then how many more there are, and how to ask again from here. An answer that
/// gives what stands whole says so first, and when it gives it again.
fn attention(answer: &Attention, now: u64) -> String {
    let mut lines = Vec::new();
    let again = duration(answer.again);
    match (answer.whole, answer.lines.is_empty()) {
        (true, true) => lines.push(format!(
            "Nothing stands to be handled; what comes is given as it comes, and what stands every {again}."
        )),
        (false, true) => lines.push("Nothing new since the mark.".to_string()),
        (true, false) => lines.push(format!(
            "Everything that stands to be handled, most pressing first, given again every {again} until it is gone:"
        )),
        (false, false) => {}
    }
    for line in &answer.lines {
        lines.push(heed(line, now));
    }
    if answer.left_out > 0 {
        lines.push(format!("And {} more, given at once from this mark.", answer.left_out));
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
        Line::MainRed {
            since,
            project,
            tests,
            works,
        } => {
            return main_red(project, tests, works, duration(now.saturating_sub(*since)));
        }
        // The shared wait, and where the agents' command line cannot run, what the owner does, said here alone.
        Line::Agents { since, outage: o, mend } => {
            let held = duration(now.saturating_sub(*since));
            return shared_wait(o, Some(&held), mend.as_deref());
        }
        // How long first, as what a stall says ends as a sentence.
        Line::Fault { of, since, now: is }
        | Line::Broken { of, since, now: is }
        | Line::Stalled { of, since, now: is } => (
            of,
            format!("for {}, {}", duration(now.saturating_sub(*since)), self::now(is)),
        ),
        Line::Turn {
            since,
            project,
            holder,
            now: is,
            waiting,
        } => {
            let project = project
                .as_deref()
                .map_or_else(String::new, |project| format!(" of {project}"));
            let waiting: Vec<String> = waiting.iter().map(|of| work_number(of.work)).collect();
            return format!(
                "The turn{project} to check and merge is held by item {item} \"{title}\", work {work}, whose round or \
                 call has been under way for {held}; now {is}. Waiting for the turn: {waiting}.",
                item = item_number(holder.item),
                title = cut_title(&holder.title),
                work = work_number(holder.work),
                held = duration(now.saturating_sub(*since)),
                is = self::now(is),
                waiting = waiting.join(", ")
            );
        }
        Line::Queued {
            of,
            since,
            runner,
            now: is,
        } => (
            of,
            format!(
                "has waited for the runner {runner} for {}: {}",
                duration(now.saturating_sub(*since)),
                self::now(is)
            ),
        ),
        Line::Machine {
            of,
            since,
            machine,
            behind,
            now: is,
        } => {
            let behind = match behind {
                Some(holder) => format!(
                    "behind item {item} \"{title}\", work {work}",
                    item = item_number(holder.item),
                    title = cut_title(&holder.title),
                    work = work_number(holder.work)
                ),
                None => "behind a run there the engine's turns do not hold".to_string(),
            };
            (
                of,
                format!(
                    "its measurement has waited for its turn on the machine {machine} for {}, {behind}: {}",
                    duration(now.saturating_sub(*since)),
                    self::now(is)
                ),
            )
        }
        Line::Ahead { of, questions, .. } => {
            let asked: Vec<String> = (questions.iter())
                .map(|AskedAhead { name, since }| format!("{name} ({})", duration(now.saturating_sub(*since))))
                .collect();
            let them = if questions.len() == 1 {
                "a question"
            } else {
                "questions"
            };
            (
                of,
                format!("has {them} the owner may answer ahead: {}", asked.join(", ")),
            )
        }
        Line::Unexplained { of, since, now: is } => (
            of,
            format!(
                "the owner paused it {} ago and said no reason: {}",
                duration(now.saturating_sub(*since)),
                self::now(is)
            ),
        ),
        Line::Slots {
            since,
            slots,
            free,
            items,
        } => {
            let items = match *items {
                1 => "1 item could".to_string(),
                n => format!("{n} items could"),
            };
            return format!(
                "{free} of {slots} slots are free while {items} be let on, the first of them for {} \
                 (`wf-agents candidates` lists them).",
                duration(now.saturating_sub(*since))
            );
        }
        Line::Root { of, since } => (
            of,
            format!(
                "stands at a root and is no deliverable: no item that has not ended depends on it, for {}",
                duration(now.saturating_sub(*since))
            ),
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
                .map(|idle| format!("{} {}", item_number(idle.item), stuck(idle.why)))
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
        Line::PauseOver { of, since, items } => {
            let has = if items.len() == 1 {
                "which has"
            } else {
                "each of which has"
            };
            (
                of,
                format!(
                    "its pause waited for {}, {has} merged or been cancelled, {} ago: it is paused still, and only \
                     `wf-agents resume {}` lets it on",
                    numbers(items),
                    duration(now.saturating_sub(*since)),
                    of.work
                ),
            )
        }
        Line::Ended { of, since, now: is } => (
            of,
            format!("{}, {} ago", self::now(is), duration(now.saturating_sub(*since))),
        ),
    };
    let (item, work) = (item_number(of.item), work_number(of.work));
    format!(
        "Item {item} \"{}\", work {work}: {}",
        cut_title(&of.title),
        sentence(&said)
    )
}

/// How many of the tests that are red on main a line of `wait` names.
const RED_TESTS: usize = 5;

/// That the main branch of a project fails these tests on its head, since when, and which works wait for it to pass.
fn main_red(project: &str, tests: &[RedTest], works: &[Of], red_for: String) -> String {
    let named: Vec<String> = (tests.iter().take(RED_TESTS))
        .map(|test| format!("`{}`", test.name))
        .collect();
    let more = match tests.len().saturating_sub(RED_TESTS) {
        0 => String::new(),
        left => format!(" and {left} more"),
    };
    let mut heads: Vec<&str> = tests.iter().map(|test| test.head.as_str()).collect();
    heads.sort_unstable();
    heads.dedup();
    let heads: Vec<String> = heads.iter().map(|head| head.chars().take(12).collect()).collect();
    let waiting = if works.is_empty() {
        "No work waits for it.".to_string()
    } else {
        let waiting: Vec<String> = (works.iter())
            .map(|of| {
                let (work, item) = (work_number(of.work), item_number(of.item));
                format!("{work} for item {item}")
            })
            .collect();
        format!("Waiting for it to pass: {}.", waiting.join(", "))
    };
    format!(
        "The main branch of {project} is red: {}{more} fail{} on its head {}, for {red_for}. {waiting}",
        named.join(", "),
        if tests.len() == 1 { "s" } else { "" },
        heads.join(", ")
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

/// The observations shown, from the number given down, the newest first, each as a line saying where it came from and
/// whether the owner read it, a line of what it is of, its title, tags and the item that took it in where there is
/// anything to say, then its whole text indented; and how many more there are, with the command that gives them. An
/// observation an item that is not cancelled took in is left out unless `all`.
fn observations_said(page: &ObservationsPage, showing: &Showing, from: Option<u64>, limit: usize) -> String {
    if page.observations.is_empty() {
        return "There are no observations.".to_string();
    }
    let shown: Vec<&Observation> = (page.observations.iter())
        .filter(|o| showing.all || !o.handled)
        .filter(|o| showing.tag.as_ref().is_none_or(|tag| o.tags.contains(tag)))
        .filter(|o| {
            showing
                .project
                .as_ref()
                .is_none_or(|project| o.project.as_ref() == Some(project))
        })
        .collect();
    if shown.is_empty() {
        return "No observation is shown: those taken in by an item are left out unless --all is given.".to_string();
    }
    let down: Vec<&Observation> = (shown.into_iter())
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
        let came = match &o.round {
            Some(r) => format!(
                "item {item} \"{}\", {}, cell {}, run {}, round {}",
                cut_title(&r.item_title),
                work_number(r.work),
                r.cell,
                r.run,
                r.round,
                item = item_number(r.item),
            ),
            None => format!("reported by {}", o.reported_by.as_deref().unwrap_or("no project")),
        };
        lines.push(format!("O{}, {read}, kept {}: {came}", o.number, utc(o.kept)));
        let mut facts = Vec::new();
        if o.round.is_none() {
            facts.push(format!("of {}", o.project.as_deref().unwrap_or("no project")));
        }
        if let Some(title) = &o.title {
            facts.push(format!("title \"{}\"", cut_title(title)));
        }
        if !o.tags.is_empty() {
            facts.push(format!("tags {}", o.tags.join(", ")));
        }
        if let Some(item) = o.taken_in_by {
            let cancelled = if o.handled { "" } else { ", which is cancelled" };
            facts.push(format!("taken in by {}{cancelled}", item_number(item)));
        }
        if !facts.is_empty() {
            lines.push(format!("  {}", facts.join("; ")));
        }
        lines.extend(o.text.trim().lines().map(|line| format!("  {line}")));
    }
    if let Some(next) = down.get(limit) {
        let mut more = format!("wf-agents observations --from {}", next.number);
        if let Some(tag) = &showing.tag {
            more.push_str(&format!(" --tag {tag}"));
        }
        if let Some(project) = &showing.project {
            more.push_str(&format!(" --project {project}"));
        }
        if showing.all {
            more.push_str(" --all");
        }
        lines.push(format!("And {} more, older: {more}", down.len() - limit));
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
        Stuck::Fault => "unable to go on",
    }
}

/// A duration in its two largest units, as the owner's page writes one: 2d 03h, 2h 05m, 3m 07s, 41s.
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
        Now::Broken { words } => format!("its state cannot be worked out: {words}"),
        Now::Merged => "merged".to_string(),
        Now::Cancelled => "cancelled".to_string(),
        Now::Round {
            cell,
            run,
            round,
            queued_for: Some(runner),
            ..
        } => format!("a round of {cell} is under way, queued for {runner}, round {round} of run {run}"),
        Now::Round { cell, run, round, .. } => format!("a round of {cell} is under way, round {round} of run {run}"),
        Now::Paused {
            at,
            by: Pauser::Owner,
            why,
            waits_for,
            open,
        } => owner_paused(at, why.as_deref(), waits_for, open),
        Now::Paused {
            at,
            by: Pauser::Engine,
            why: Some(why),
            ..
        } => format!("paused at {at} by the engine. {}", sentence(why)),
        Now::Paused {
            at,
            by: Pauser::Engine,
            why: None,
            ..
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

/// The owner's pause, as a clause: where it is, until which items have merged or been cancelled where it waits for
/// items, or that what it waited for is over, and why where the owner said.
fn owner_paused(at: &str, why: Option<&str>, waits_for: &[u64], open: &[u64]) -> String {
    let each = |items: &[u64]| match items {
        [_] => numbers(items),
        _ => format!("each of {}", numbers(items)),
    };
    let waits = match (waits_for.is_empty(), open.is_empty()) {
        (true, _) => String::new(),
        (false, false) => format!(" until {} merges or is cancelled", each(open)),
        (false, true) => format!(
            ", and what it waited for is over: {} merged or was cancelled",
            each(waits_for)
        ),
    };
    match why {
        Some(why) if !why.trim().is_empty() => format!("paused at {at}{waits}: {}", first_line(why)),
        _ => format!("paused at {at}{waits}"),
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
            held: Some(held),
            ..
        } => format!(
            "call {call}, {}, held until {} ({} in a row): {}",
            called(what),
            utc(held.until),
            held.failed,
            obstacle(held.obstacle),
        ),
        Waited::Call {
            call,
            who: None,
            what,
            machine: Some(machine),
            ..
        } => format!(
            "call {call}, {}, waiting since {} for its turn on the machine {}, where one measurement runs at a time, \
             {}",
            called(what),
            utc(machine.since),
            machine.machine,
            match machine.behind {
                Some(work) => format!("behind work {work}", work = work_number(work)),
                None => "behind a run there the engine's turns do not hold".to_string(),
            },
        ),
        Waited::Call {
            call,
            who: None,
            what,
            at,
            queued_for,
            ..
        } => match (at, queued_for) {
            (Some(at), Some(runner)) => format!("call {call}, {}, queued for {runner} at {at}", called(what)),
            (Some(at), None) => format!("call {call}, {}, running at {at}", called(what)),
            (None, _) => format!("call {call}, {}", called(what)),
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
        Waited::Turn { holder, .. } => {
            let holder = work_number(*holder);
            format!("the project's turn to check and merge, which work {holder} has")
        }
        Waited::Agents { outage: o } => match o.first {
            Some(first) => {
                let what = if o.service == web::works::Service::Host {
                    "act"
                } else {
                    "round"
                };
                let first = work_number(first);
                format!("{first}'s {what} to come back: it is well again, and that {what} goes first")
            }
            None => format!("the agents' rounds to begin again, as {}", cause(o)),
        },
        Waited::Stopping { since } => format!(
            "the engine to start again, as it was asked to stop {} ago",
            duration(clock().saturating_sub(*since))
        ),
        Waited::Outside { source, cell, why } => format!("the outside ({source}), before {cell} runs again: {why}"),
        Waited::Items { items } => format!("the items it depends on to merge: {}", numbers(items)),
    }
}

/// Items by their numbers, each as an item's number is written: #5, #5 and #7, #5, #6 and #7.
fn numbers(items: &[u64]) -> String {
    let items: Vec<String> = items.iter().map(|item| item_number(*item)).collect();
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

/// A wait every work shares, in sentences: what is down and since when; what the engine does next, moving to another
/// account included where the owner lets it; what the latest check of the outside found, where it found it down; and the
/// round that met it, with what it said.
fn outage(outage: &Outage) -> String {
    let what = capital(cause(outage));
    let until = match (outage.until, outage.first) {
        (Until::Checked { at }, _) => format!(
            "The engine checks the outside itself at {}, touching no work's session, and one round goes first once it \
             is well.",
            utc(at)
        ),
        (Until::Well { at }, Some(first)) => format!(
            "The engine's check at {} found it well: {first}'s round goes first, and every work goes on once it comes \
             back.",
            utc(at),
            first = work_number(first)
        ),
        (Until::Well { at }, None) => format!(
            "The engine's check at {} found it well, and one round goes first.",
            utc(at)
        ),
        (Until::Looked { .. }, Some(first)) => {
            let first = work_number(first);
            format!("A look reached the host: {first}'s act goes first, and every act goes on once it comes back.")
        }
        (Until::Looked { at }, None) => format!(
            "The host is looked at from {} every half minute, and one act goes first once it is reached.",
            utc(at)
        ),
    };
    // Where the owner lets the engine move at a usage limit and another account may yet be moved to, the wait ends
    // by that as well as by the limit's reset.
    let moves = if outage.may_move && outage.holds {
        " The engine also checks the other accounts and moves every work to the first one it finds well."
    } else {
        ""
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
        "{what} since {}. {until}{moves}{checked} Round {} of run {} of {} met it. {}",
        utc(outage.since),
        outage.round,
        outage.run,
        work_number(outage.work),
        sentence(&outage.why)
    )
}

/// The wait every work shares, as one line, for `list` and `wait` alike. While it holds rounds, it says that none begins
/// (for how long, where that is known), what is down and what the owner does about it (`mend`, where there is one).
/// Once the engine's check found the outside well and it holds nothing, it says in the past what was down and that it
/// was found well, and gives nothing to mend.
fn shared_wait(o: &Outage, held: Option<&str>, mend: Option<&str>) -> String {
    if !o.holds {
        return outage_over(o);
    }
    let held = held.map_or_else(String::new, |held| format!(", for {held}"));
    let mend = mend.map_or_else(String::new, |mend| format!(" {}", sentence(mend)));
    format!("No agent's round of any work begins{held}. {}{mend}", outage(o))
}

/// A wait every work shares that holds nothing any more, in the past: what was down, since when, that the engine's
/// check found it well where it did, and the round that met it, with what it said.
fn outage_over(outage: &Outage) -> String {
    let what = capital(match outage.failure {
        Failure::UsageLimit => "the account's usage limit was reached",
        Failure::CannotRun => "the agents' command line could not run",
        Failure::Offline => "the network could not be reached",
        Failure::Unavailable => "the agents' service was down, overloaded or not answering",
        _ => "a round failed for a cause every work shares",
    });
    let well = match outage.until {
        Until::Well { at } => format!(" The engine's check at {} found it well.", utc(at)),
        Until::Checked { .. } | Until::Looked { .. } => String::new(),
    };
    format!(
        "{what} from {}.{well} Round {} of run {} of {} met it. {}",
        utc(outage.since),
        outage.round,
        outage.run,
        work_number(outage.work),
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

/// What kept the host from carrying a call out for now, as a clause.
fn obstacle(obstacle: web::works::Obstacle) -> &'static str {
    match obstacle {
        web::works::Obstacle::Limit => "GitHub's limit on its requests",
        web::works::Obstacle::Fault => "GitHub failed",
        web::works::Obstacle::NoRunner => "no machine took the run",
        web::works::Obstacle::Network => "the network failed",
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
        Call::Wait {
            until,
            failed,
            obstacle: None,
        } => format!(
            "the engine's wait until {}, before the round that failed in passing ({failed} in a row) is tried again",
            utc(*until)
        ),
        Call::Wait {
            until,
            failed,
            obstacle: Some(why),
        } => format!(
            "the engine's wait until {}, before the round the host kept from running is tried again: {} ({failed} in a row)",
            utc(*until),
            obstacle(*why)
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
/// that has taken longer so far than its line, and a call of a tool whose time did, or whose result's size did, or that
/// has waited longer for its result.
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
        let Call::Tool { name, .. } = &call.what else {
            continue;
        };
        let said = call_past(call.done.is_some(), call.time, call.size);
        if !said.is_empty() {
            lines.push(format!("    call {} of the tool {name} {}", call.call, said.join("; ")));
        }
    }
    lines
}

/// What of a call of a tool went past its line, a clause each: its time, as it took, or has waited for its result so
/// far, and the size of its result.
fn call_past(done: bool, time: Option<Against>, size: Option<Against>) -> Vec<String> {
    let mut said = Vec::new();
    if let Some(time) = time.filter(|time| time.past) {
        let took = if done { "took" } else { "has waited for its result for" };
        said.push(time_past(took, &time));
    }
    if let Some(size) = size.filter(|size| size.past) {
        said.push(size_past(&size));
    }
    said
}

/// The size of a result past its line, as a clause.
fn size_past(size: &Against) -> String {
    format!(
        "returned {} bytes, past the line of {} bytes that {}",
        size.value,
        size.line.value,
        given_by(&size.line)
    )
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

/// A measure of a round under way, in words: what it is, and whether it is past the line it is judged by, with what
/// gives the line, where it has one.
fn figure_said(what: &str, figure: &Figure, unit: impl Fn(u64) -> String) -> String {
    match &figure.line {
        Some(line) => format!(
            "{what} {}, {} the line of {} that {}",
            unit(figure.value),
            if figure.past { "past" } else { "within" },
            unit(line.value),
            given_by(line)
        ),
        None => format!("{what} {}, no line yet", unit(figure.value)),
    }
}

/// Millionths of a dollar, in dollars and cents.
fn dollars(micros: u64) -> String {
    format!("${:.2}", micros as f64 / 1_000_000.0)
}

/// What the works waited for over the span, in at most nine lines whatever there is: for each cause how many stops ended
/// and how long they took, with the median and the longest; how long the calls and rounds that waited for CI took and
/// how long its jobs waited for a runner; how many works held a slot and ran; and what is not known. Nothing is judged:
/// the lines of each history are the judgments' to say.
fn waited_said(waited: &Waited) -> Vec<String> {
    if waited.by.is_empty() {
        return Vec::new();
    }
    let ends = |stopped: &web::system::Stopped| match (stopped.median_ms, stopped.longest_ms) {
        (Some(median), Some(longest)) => format!("median {}, longest {}", duration(median), duration(longest)),
        _ => "none ended".to_string(),
    };
    let mut lines =
        vec!["Waited (stops that ended, the time they took, and the median and longest of them):".to_string()];
    for by in &waited.by {
        let cause = match by.reason {
            WaitReason::Owner => "the owner",
            WaitReason::Items => "the items its item depends on",
            WaitReason::Slot => "a slot",
            WaitReason::Turn => "the turn to check and merge",
            WaitReason::Paused => "a pause",
        };
        lines.push(format!(
            "  for {cause}: {} stops, {} in all; {}",
            by.stopped.stops,
            duration(by.stopped.total_ms),
            ends(&by.stopped)
        ));
    }
    let ci = &waited.ci;
    let queued = match (ci.queued_median_ms, ci.queued_longest_ms) {
        (Some(median), Some(longest)) => format!(
            "; {} jobs said how long they waited for a runner, median {}, longest {}",
            ci.jobs,
            duration(median),
            duration(longest)
        ),
        _ => "; no job said how long it waited for a runner".to_string(),
    };
    lines.push(format!(
        "  for CI: {} calls and rounds, {} in all; {}{queued}",
        ci.waited.stops,
        duration(ci.waited.total_ms),
        ends(&ci.waited)
    ));
    let slots = &waited.occupancy;
    let of = match (slots.slots, slots.recorded) {
        (Some(most), true) => format!(", of {most} slots"),
        (Some(most), false) => format!(", of {most} slots (the slots were kept only from part of the span)"),
        (None, _) => ", the slots not having been kept".to_string(),
    };
    let thousandths = |n: u64| format!("{}.{:01}", n / 1000, n % 1000 / 100);
    lines.push(format!(
        "  slots: on average {} works held one, at most {}{of}; on average {} ran, at most {}",
        thousandths(slots.taking_avg),
        slots.taking_peak,
        thousandths(slots.running_avg),
        slots.running_peak
    ));
    if waited.unseparated.total_ms > 0 {
        lines.push(format!(
            "  {} in all, of works that ended before a wait for a slot was told apart, is in none of the causes above",
            duration(waited.unseparated.total_ms)
        ));
    }
    if waited.uncounted > 0 {
        lines.push(format!(
            "  {} works are not counted: their flow's version was not kept",
            waited.uncounted
        ));
    }
    lines
}

/// How the system ran over the last `days`: what its rounds took, what ended past the lines of its history, the latest
/// first, and what is under way against its own, the oldest first, each agent's round with what its calls have come to
/// so far as `going` says it, at most `limit` of each, with how many more there are and how to see them.
fn system_said(system: &System, going: &[Going], days: u64, limit: usize) -> String {
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
    lines.extend(waited_said(&system.waited));
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
                    "  {work} of {item}, round {round} of run {run} of {cell}, ended {}: {}",
                    utc(*ended),
                    said.join("; "),
                    work = work_number(*work),
                    item = item_number(*item)
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
                size,
            } => format!(
                "  {work} of {item}, call {call} of the tool {tool} by run {run} of {cell}, done {}: {}",
                utc(*done),
                call_past(true, *time, *size).join("; "),
                work = work_number(*work),
                item = item_number(*item)
            ),
        });
    }
    if system.past.len() > limit {
        lines.push(more(system.past.len() - limit));
    }
    if let Some(stop) = &system.stopping {
        let how = if stop.at_once {
            "at once"
        } else {
            "once what is under way has ended"
        };
        let ends = match stop.rest {
            Rest::Expected { ms } => format!("expected to end in about {}", duration(ms)),
            Rest::Longer {} => "to end once a round that has taken longer than any before has ended".to_string(),
        };
        lines.push(format!(
            "The engine was asked {} to stop {how}: no round begins, and it is {ends}.",
            utc(stop.since)
        ));
    }
    if system.under_way.is_empty() {
        lines.push("Nothing is under way.".to_string());
    } else {
        lines.push("Under way, the oldest first:".to_string());
    }
    let so_far = |time: &Option<Against>, since: u64, what: &str| match time {
        Some(time) => format!(
            "{} so far, {} the line of {} that {}",
            duration(time.value),
            if time.past { "past" } else { "within" },
            duration(time.line.value),
            given_by(&time.line)
        ),
        None => format!(
            "{} so far, no line yet, too few {what} having ended",
            duration(system.now.saturating_sub(since))
        ),
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
                rest,
            } => {
                let rest = match rest {
                    Some(Rest::Expected { ms }) => format!("; about {} more", duration(*ms)),
                    Some(Rest::Longer {}) => "; longer than any round of its history".to_string(),
                    None => String::new(),
                };
                // What its calls have come to so far, against the lines the run guard judges it by; nothing before its
                // carrier says any.
                let calls = going
                    .iter()
                    .find(|g| g.work == *work && g.run == *run && g.round == *round)
                    .map_or_else(String::new, |g| {
                        let said = [
                            g.context.map(|context| format!("its context is {context} tokens")),
                            g.added
                                .map(|f| figure_said("its context has grown by", &f, |n| format!("{n} tokens"))),
                            g.per_call
                                .map(|f| figure_said("and by", &f, |n| format!("{n} tokens a call after the first"))),
                            g.cost.map(|f| figure_said("its calls cost", &f, dollars)),
                        ];
                        let said: Vec<String> = said.into_iter().flatten().collect();
                        if said.is_empty() {
                            String::new()
                        } else {
                            format!("; {}", said.join(", "))
                        }
                    });
                format!(
                    "  {work} of {item}, round {round} of run {run} of {cell}, since {}: {}{rest}{calls}",
                    utc(*began),
                    so_far(time, *began, "rounds of its cell"),
                    work = work_number(*work),
                    item = item_number(*item)
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
                "  {work} of {item}, call {call} of the tool {tool} by run {run} of {cell}, since {}: {}",
                utc(*made),
                so_far(time, *made, "calls of its tool"),
                work = work_number(*work),
                item = item_number(*item)
            ),
        });
    }
    if system.under_way.len() > limit {
        lines.push(more(system.under_way.len() - limit));
    }
    if !system.left_behind.is_empty() {
        lines.push("Left running by a program as it ended, and stopped, the latest first:".to_string());
    }
    for left in system.left_behind.iter().take(limit) {
        let of = match &left.of {
            LeftOf::Try {
                cell,
                run,
                round,
                attempt,
            } => format!("try {attempt} of round {round} of run {run} of {cell}"),
            LeftOf::Act { cell, run, round } => format!("the check of round {round} of run {run} of {cell}"),
            LeftOf::Call { cell, run, call, tool } => {
                format!("the check of call {call} of the tool {tool} by run {run} of {cell}")
            }
            LeftOf::Ping { line, session } => format!("a keep-warm ping of session {session} of the line {line}"),
            LeftOf::Fork { cell, run, round } => format!("a fork of round {round} of run {run} of {cell}"),
        };
        lines.push(format!(
            "  {} of {}, {of}, ended {}: {} {}",
            work_number(left.work),
            item_number(left.item),
            utc(left.at),
            left.processes,
            if left.processes == 1 { "process" } else { "processes" }
        ));
    }
    if system.left_behind.len() > limit {
        lines.push(more(system.left_behind.len() - limit));
    }
    if !system.short_of_time.is_empty() {
        lines.push(
            "Calls of run_tests the fast check's budget kept tests of from their end, by day, the latest first:"
                .to_string(),
        );
    }
    for day in system.short_of_time.iter().take(limit) {
        lines.push(format!(
            "  {} on the day from {}: {} {}; tests stopped {}, not started {}, refused {}",
            day.project.as_deref().unwrap_or("no project"),
            utc(day.day),
            day.calls,
            if day.calls == 1 { "call" } else { "calls" },
            day.stopped,
            day.not_started,
            day.refused
        ));
    }
    if system.short_of_time.len() > limit {
        lines.push(more(system.short_of_time.len() - limit));
    }
    for tests in &system.tests {
        lines.extend(tests_said(tests, days, limit));
    }
    lines.join("\n")
}

/// A time of a test or a job, in milliseconds or seconds.
fn millis_said(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms} ms")
    } else {
        format!("{:.1} s", ms as f64 / 1000.0)
    }
}

/// What a project's merged checks say of how long its tests and jobs took, each list at most `limit` long, with how
/// many more there are and how to see them.
fn tests_said(tests: &web::system::TestTimes, days: u64, limit: usize) -> Vec<String> {
    let project = tests.project.as_deref().unwrap_or("a project with no name");
    let mut lines = vec![format!(
        "Times of the tests and jobs of {project}, from its {} merged checks:",
        tests.checks
    )];
    let more = |left: usize| {
        format!(
            "  And {left} more: wf-agents system --days {days} --limit {}",
            limit + left
        )
    };
    if tests.slowed.is_empty() {
        lines.push("  No test took longer than its own history says in two merged checks in a row.".to_string());
    } else {
        lines.push("  Tests that took longer than their own history says in two merged checks in a row:".to_string());
    }
    for t in tests.slowed.iter().take(limit) {
        lines.push(format!(
            "    {} in {}: {} at {}, against a median of {} and a line of {}",
            t.name,
            t.unit,
            millis_said(t.millis),
            work_number(t.work),
            millis_said(t.median),
            millis_said(t.line)
        ));
    }
    if tests.slowed.len() > limit {
        lines.push(more(tests.slowed.len() - limit));
    }
    for g in &tests.grown {
        lines.push(format!(
            "  The CI grew: {} had a median of {} over the {} merged checks before the latest {} and {} over them ({} of \
             them above the earlier median, {} needed); it stepped at the merge of {}.",
            if g.of == driver::timed::TESTS { "the sum of the tests' times".to_string() } else { format!("the job {}", g.of) },
            millis_said(g.before),
            g.n,
            g.n,
            millis_said(g.after),
            g.above,
            g.needed,
            work_number(g.work)
        ));
    }
    lines.push("  The latest merges, the latest last, with what each check took and the tests it added:".to_string());
    let skipped = tests.merges.len().saturating_sub(limit);
    for m in tests.merges.iter().skip(skipped) {
        let jobs: Vec<String> = (m.jobs.iter())
            .map(|j| format!("{} {}", j.name, millis_said(j.millis)))
            .collect();
        lines.push(format!(
            "    {}: jobs {}; the tests {}; {} tests added, {}",
            work_number(m.work),
            if jobs.is_empty() {
                "none read".to_string()
            } else {
                jobs.join(", ")
            },
            millis_said(m.tests_millis),
            m.added,
            millis_said(m.added_millis)
        ));
    }
    if skipped > 0 {
        lines.push(more(skipped));
    }
    lines
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
    // Where some of it is not known, what is said is a part of it, and said so.
    let unknown = match spent.unpriced {
        0 => String::new(),
        n => format!("; {n} of its rounds, pings and forks cost more than that, by how much is not known"),
    };
    format!("${dollars}.{micros:06} in {rounds}, under way for {seconds}.{tenths} s{unknown}")
}

#[cfg(test)]
mod tests {
    use web::works::{FirstItem, Fresh, Item, ItemHistory, Summary};

    use super::*;

    // What rounds took is said with the part of it that is not known, never as if what is known were all of it.
    #[test]
    fn a_spend_some_of_whose_cost_is_not_known_says_so() {
        let known = Spent {
            usd_micros: 1_500_000,
            working_ms: 3_200,
            rounds: 2,
            ..Spent::default()
        };
        assert_eq!(spent(&known), "$1.500000 in 2 rounds, under way for 3.2 s");
        let part = Spent { unpriced: 1, ..known };
        assert_eq!(
            spent(&part),
            "$1.500000 in 2 rounds, under way for 3.2 s; 1 of its rounds, pings and forks cost more than that, by how \
             much is not known"
        );
    }

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
            continues: None,
            history: ItemHistory {
                first: FirstItem {
                    title: format!("item {number}"),
                    level: Some("1".to_string()),
                    depends_on: depends_on.to_vec(),
                    tags: Vec::new(),
                    continues: None,
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
                forked: Spent::default(),
                last: None,
                from: None,
                bar: String::new(),
                sent_back: 0,
                waits: Vec::new(),
                ahead: 0,
                level: Some("1".to_string()),
                cell_level: None,
                raise: None,
                held: None,
                pull: None,
                changed: None,
                unread: false,
                fresh: Fresh::None,
                replies_unread: 0,
            },
        }
    }

    fn items(items: Vec<Item>) -> Items {
        Items {
            items,
            outage: None,
            waits: Vec::new(),
        }
    }

    /// An observation of the plan's first round of this work, kept at this time.
    fn observation(number: u64, text: &str, read: Option<u64>) -> Observation {
        Observation {
            number,
            text: text.to_string(),
            title: None,
            tags: Vec::new(),
            project: Some("wf-agents".to_string()),
            reported_by: Some("wf-agents".to_string()),
            round: Some(web::observations::Round {
                work: 3,
                item: 2,
                item_title: "Greet the user".to_string(),
                cell: "plan".to_string(),
                run: 4,
                round: 1,
            }),
            kept: 1_800_000_000_000,
            read,
            taken_in_by: None,
            handled: false,
        }
    }

    /// An observation an architect added, of this project and reported by that one.
    fn an_added_observation(number: u64, text: &str, project: &str, reported_by: &str) -> Observation {
        Observation {
            round: None,
            project: Some(project.to_string()),
            reported_by: Some(reported_by.to_string()),
            ..observation(number, text, None)
        }
    }

    /// An open pull request of a project, a draft where its number is 2, updated an hour into the time, kept by this work.
    fn pull(project: &str, number: u64, work: Option<web::pulls::PullWork>) -> OpenPull {
        OpenPull {
            project: project.to_string(),
            number,
            title: format!("Pull {number}"),
            url: format!("https://host.example/pull/{number}"),
            draft: number == 2,
            updated: 3_600_000,
            work,
        }
    }

    // The open pull requests are a line each, with the work that keeps one, what it does and when something last happened
    // to it, or that no work keeps it; a project whose host could not say, and one whose host has no pull requests, are
    // said by themselves first, and the first is never said as having none.
    #[test]
    fn the_open_pull_requests_are_a_line_each_and_a_project_whose_host_could_not_say_is_said_by_itself() {
        let hour = 3_600_000;
        let kept = web::pulls::PullWork {
            work: 5,
            item: 40,
            now: Now::Cancelled,
            last: Some(2 * hour),
        };
        let pulls = Pulls {
            pulls: vec![pull("wf", 1, Some(kept)), pull("wf", 2, None)],
            failed: vec![web::pulls::Unsaid {
                project: "halo".to_string(),
                why: "gh api GET repos/o/r/pulls failed: HTTP 403: rate limit".to_string(),
            }],
            without: vec!["local".to_string()],
        };
        assert_eq!(
            pulls_said(&pulls, None, 3 * hour),
            "Project halo: its host could not say which pull requests are open. gh api GET repos/o/r/pulls failed: \
             HTTP 403: rate limit.\n\
             Project local: its host has no pull requests.\n\
             wf pull request 1 \"Pull 1\", updated 2h 00m ago, https://host.example/pull/1: W5 of item #40: cancelled \
             (last happened 1h 00m ago)\n\
             wf pull request 2 \"Pull 2\" (draft), updated 2h 00m ago, https://host.example/pull/2: no work keeps it."
        );
        let nothing = Pulls {
            pulls: Vec::new(),
            failed: Vec::new(),
            without: Vec::new(),
        };
        assert_eq!(pulls_said(&nothing, None, hour), "No pull request is open.");
        assert_eq!(
            pulls_said(&nothing, Some(30), hour),
            "No pull request is open numbered 30 or more."
        );
        // From a number on, those numbered less are left out.
        let from = pulls_said(&pulls, Some(2), 3 * hour);
        assert!(
            from.contains(" pull request 2 ") && !from.contains(" pull request 1 "),
            "{from}"
        );
    }

    // At most a hundred are said, and what is left out is said by project, with the command that gives it from its first.
    #[test]
    fn the_open_pull_requests_past_the_most_said_are_counted_by_project_with_the_command_that_gives_them() {
        let many = |project: &str, from: u64, to: u64| (from..=to).map(|n| pull(project, n, None)).collect::<Vec<_>>();
        let pulls = Pulls {
            pulls: [many("a", 1, 60), many("b", 1, 60)].concat(),
            failed: Vec::new(),
            without: Vec::new(),
        };
        let said = pulls_said(&pulls, None, 3_600_000);
        let lines: Vec<&str> = said.lines().collect();
        assert_eq!(lines.len(), LISTED + 1, "{said}");
        assert_eq!(
            lines[LISTED], "And 20 more of b: wf-agents pulls --project b --from 41",
            "{said}"
        );
        assert!(lines[LISTED - 1].starts_with("b pull request 40 "), "{said}");
    }

    // A work whose state the engine could not work out is said as that, with the engine's words, on its own line of the list,
    // and the lines of the other items are as they are. Fails where the list says nothing of it or says it as a fault.
    #[test]
    fn a_list_says_a_state_that_cannot_be_worked_out_on_that_items_line_alone() {
        let mut broken = item(2, false, &[], &[]);
        broken.work.now = Now::Broken {
            words: "the driver says neither whose turn the work waits for nor since when".to_string(),
        };
        broken.work.unread = true;
        let others = [item(1, false, &[], &[]), item(3, true, &[], &[])];
        let said = list(
            &items(vec![others[0].clone(), broken, others[1].clone()]),
            &Listing::default(),
            0,
        );
        let alone = list(&items(others.to_vec()), &Listing::default(), 0);
        let lines: Vec<&str> = said.lines().collect();
        assert!(
            lines[1].contains(
                "its state cannot be worked out: the driver says neither whose turn the work waits for nor since when"
            ),
            "{said}"
        );
        assert!(!lines[1].contains("cannot go on"), "{said}");
        assert_eq!(lines[0], alone.lines().next().unwrap(), "{said}");
    }

    // The owner's pause is said with why, and with the items it waits for until each has merged or been cancelled, and that
    // what it waited for is over once each has; one with neither is said as it always was, and the engine's keeps its words.
    #[test]
    fn the_owners_pause_is_said_with_its_reason_and_the_items_it_waits_for_until_they_end() {
        let paused = |by, why: Option<&str>, waits_for: &[u64], open: &[u64]| {
            now(&Now::Paused {
                at: "plan".to_string(),
                by,
                why: why.map(str::to_string),
                waits_for: waits_for.to_vec(),
                open: open.to_vec(),
            })
        };
        let owner = Pauser::Owner;
        assert_eq!(paused(owner, None, &[], &[]), "paused at plan");
        assert_eq!(
            paused(owner, Some("After the base."), &[], &[]),
            "paused at plan: After the base."
        );
        assert_eq!(
            paused(owner, None, &[12], &[12]),
            "paused at plan until #12 merges or is cancelled"
        );
        assert_eq!(
            paused(owner, Some("After both."), &[12, 14], &[12, 14]),
            "paused at plan until each of #12 and #14 merges or is cancelled: After both."
        );
        assert_eq!(
            paused(owner, Some("After both."), &[12, 14], &[14]),
            "paused at plan until #14 merges or is cancelled: After both."
        );
        assert_eq!(
            paused(owner, None, &[12, 14], &[]),
            "paused at plan, and what it waited for is over: each of #12 and #14 merged or was cancelled"
        );
        assert_eq!(
            paused(Pauser::Engine, Some("A round failed"), &[], &[]),
            "paused at plan by the engine. A round failed."
        );
    }

    // A pause whose items have all ended is a line for the planner, which says whom it waited for, since when, and how to
    // let the work on; the candidates say how many paused items are none yet because their pause still waits.
    #[test]
    fn a_pause_that_is_over_is_a_line_and_the_candidates_count_the_pauses_that_still_wait() {
        let hour = 3_600_000;
        let line = Line::PauseOver {
            of: web::attention::Of {
                work: 5,
                item: 40,
                title: "Greet".to_string(),
            },
            since: hour,
            items: vec![12, 14],
        };
        assert_eq!(
            heed(&line, 3 * hour),
            "Item #40 \"Greet\", work W5: its pause waited for #12 and #14, each of which has merged or been cancelled, \
             2h 00m ago: it is paused still, and only `wf-agents resume 5` lets it on."
        );
        let none = Candidates {
            slots: 3,
            taken: 1,
            total: 0,
            waiting: 2,
            rows: Vec::new(),
        };
        assert_eq!(
            candidates_said(&none),
            "1 of 3 slots taken; 0 items could be let on now.\n2 paused items are not among them: their pauses wait \
             for items that have neither merged nor been cancelled (`wf-agents list` says which)."
        );
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
            observations_said(&page, &Showing::default(), None, 2),
            "O3, unread, kept 2027-01-15 08:00 UTC: item #2 \"Greet the user\", W3, cell plan, run 4, round 1\n  Third.\n  \
             In two lines.\nO2, read 2027-01-15 08:01 UTC, kept 2027-01-15 08:00 UTC: item #2 \"Greet the user\", W3, \
             cell plan, run 4, round 1\n  Second.\nAnd 1 more, older: wf-agents observations --from 1"
        );
        let down = observations_said(&page, &Showing::default(), Some(2), 20);
        assert!(down.starts_with("O2, read ") && down.ends_with("  First."), "{down}");
        assert!(!down.contains("O3") && !down.contains("more"), "{down}");
        assert_eq!(
            observations_said(&page, &Showing::default(), Some(0), 20),
            "No observation is numbered O0 or lower."
        );
        let none = ObservationsPage {
            observations: Vec::new(),
        };
        assert_eq!(
            observations_said(&none, &Showing::default(), None, 20),
            "There are no observations."
        );
    }

    #[test]
    fn the_line_of_an_added_observation_says_who_reported_it_what_it_is_of_and_its_title_tags_and_taker() {
        let mut one = an_added_observation(5, "The merge cell names no tests.", "wf-agents", "compiler");
        one.title = Some("Name the tests".to_string());
        one.tags = vec!["flow".to_string(), "tests".to_string()];
        let page = ObservationsPage {
            observations: vec![one.clone()],
        };
        assert_eq!(
            observations_said(&page, &Showing::default(), None, 20),
            "O5, unread, kept 2027-01-15 08:00 UTC: reported by compiler\n  of wf-agents; title \"Name the tests\"; tags \
             flow, tests\n  The merge cell names no tests."
        );
        // An item that is cancelled took it in: it is said, and it is shown without --all.
        one.taken_in_by = Some(12);
        let page = ObservationsPage {
            observations: vec![one],
        };
        assert!(
            observations_said(&page, &Showing::default(), None, 20)
                .contains("tags flow, tests; taken in by #12, which is cancelled\n"),
        );
    }

    #[test]
    fn observations_are_filtered_by_tag_and_project_and_the_handled_are_left_out_unless_all() {
        let mut tagged = an_added_observation(4, "Tagged.", "wf-agents", "wf-agents");
        tagged.tags = vec!["flow".to_string()];
        let mut handled = an_added_observation(3, "Handled.", "wf-agents", "wf-agents");
        handled.tags = vec!["flow".to_string()];
        handled.taken_in_by = Some(7);
        handled.handled = true;
        let page = ObservationsPage {
            observations: vec![
                tagged,
                handled,
                an_added_observation(2, "Another project's.", "compiler", "compiler"),
                an_added_observation(1, "Untagged.", "wf-agents", "wf-agents"),
            ],
        };
        let numbers = |showing: &Showing| -> Vec<String> {
            let said = observations_said(&page, showing, None, 20);
            said.lines()
                .filter(|l| l.starts_with('O'))
                .map(|l| l[..2].to_string())
                .collect()
        };
        let by_tag = Showing {
            tag: Some("flow".to_string()),
            ..Showing::default()
        };
        assert_eq!(numbers(&by_tag), ["O4"]);
        assert_eq!(numbers(&Showing { all: true, ..by_tag }), ["O4", "O3"]);
        let by_project = Showing {
            project: Some("compiler".to_string()),
            ..Showing::default()
        };
        assert_eq!(numbers(&by_project), ["O2"]);
        assert_eq!(numbers(&Showing::default()), ["O4", "O2", "O1"]);
        // What is left to give is asked for with the same filters.
        let limited = observations_said(
            &page,
            &Showing {
                all: true,
                ..Showing::default()
            },
            None,
            1,
        );
        assert!(
            limited.ends_with("And 3 more, older: wf-agents observations --from 3 --all"),
            "{limited}"
        );
        let none = Showing {
            tag: Some("nothing".to_string()),
            ..Showing::default()
        };
        assert!(observations_said(&page, &none, None, 20).starts_with("No observation is shown"));
    }

    #[test]
    fn tags_lists_a_tag_only_observations_carry_beside_the_items_tags() {
        let items = Tags {
            tags: vec![web::plan::TagCount {
                tag: "page".to_string(),
                items: 3,
                open: 2,
            }],
        };
        let observed = TagsPage {
            tags: vec![
                web::observations::TagCount {
                    tag: "flow".to_string(),
                    carrying: 4,
                    unhandled: 1,
                },
                web::observations::TagCount {
                    tag: "page".to_string(),
                    carrying: 1,
                    unhandled: 1,
                },
            ],
        };
        assert_eq!(
            tags_said(&items, &observed),
            "flow: 1 not handled of 4 observations\npage: 2 open of 3; 1 not handled of 1 observations"
        );
        assert_eq!(tags_said(&items, &TagsPage { tags: Vec::new() }), "page: 2 open of 3");
        assert_eq!(
            tags_said(&Tags { tags: Vec::new() }, &TagsPage { tags: Vec::new() }),
            "No item or observation carries a tag."
        );
    }

    #[test]
    fn changes_of_observations_are_read_by_kind_and_a_wrong_one_is_refused() {
        let given = |changes: &[&str]| observation_changes(&changes.iter().map(|c| c.to_string()).collect::<Vec<_>>());
        assert_eq!(
            given(&[
                "O31:taken-in-by=#12",
                "32:title=Fix the test",
                "33:tag=flow",
                "O34:untag=flow",
                "35:delete"
            ]),
            Ok(vec![
                ObservationChangeAsked::TakenIn {
                    observation: 31,
                    item: 12
                },
                ObservationChangeAsked::Title {
                    observation: 32,
                    title: "Fix the test".to_string()
                },
                ObservationChangeAsked::Tag {
                    observation: 33,
                    tag: "flow".to_string()
                },
                ObservationChangeAsked::Untag {
                    observation: 34,
                    tag: "flow".to_string()
                },
                ObservationChangeAsked::Delete { observation: 35 },
            ])
        );
        for wrong in [
            "31",
            "x:delete",
            "31:merge=12",
            "31:delete=1",
            "31:taken-in-by",
            "31:taken-in-by=x",
            "31:tag",
        ] {
            assert!(given(&[wrong]).is_err(), "{wrong} was read");
        }
    }

    #[test]
    fn an_observation_is_given_its_text_by_words_or_by_a_file_and_never_by_neither_or_both() {
        let new = |text: Option<&str>, file: Option<&str>| {
            let file = file.map(std::path::Path::new);
            new_observation(
                "wf-agents".to_string(),
                "compiler".to_string(),
                None,
                vec!["flow".to_string()],
                text.map(str::to_string),
                file,
            )
        };
        let made = new(Some("Words."), None).expect("words are a text");
        assert_eq!(
            (made.text.as_str(), made.project.as_str(), made.reported_by.as_str()),
            ("Words.", "wf-agents", "compiler")
        );
        assert!(new(None, None).is_err());
        assert!(new(Some("Words."), Some("a-file")).is_err());
        assert!(new(None, Some("/no/such/file")).is_err());
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
            first: None,
            may_move: false,
        });
        let said = list(&waiting, &Listing::default(), 0);
        let first = said.lines().next().unwrap_or_default();
        assert_eq!(
            first,
            "No agent's round of any work begins. The agents' command line cannot run since 1970-01-01 01:00 UTC. The \
             engine checks the outside itself at 1970-01-01 03:00 UTC, touching no work's session, and one round goes \
             first once it is well. The check at 1970-01-01 02:00 UTC, the 2nd in the wait, found it down. No account is \
             in use: choose one on the Settings view. Round 1 of run 2 of W1 met it. API Error: 529 Overloaded."
        );
        let outage = waiting.outage.as_mut().unwrap();
        outage.until = Until::Well { at: 2 * hour };
        outage.holds = false;
        assert!(list(&waiting, &Listing::default(), 0).starts_with("#1 "));
    }

    // The system view says, by project and day, the latest first and bounded as every list is, how many calls of
    // run_tests the fast check's budget kept tests of from their end, and how many tests it stopped, did not start and
    // refused. Microseconds.
    #[test]
    fn the_system_says_by_project_and_day_how_often_the_fast_budget_kept_tests_from_their_end() {
        let day = |day: u64, calls: usize| web::system::ShortDay {
            project: Some("Firn-wf".to_string()),
            day,
            calls,
            stopped: 2,
            not_started: 3,
            refused: 0,
        };
        let system = System {
            now: 0,
            from: None,
            spent: Spent::default(),
            pinged: Spent::default(),
            weeks: Vec::new(),
            by_project: Vec::new(),
            by_cell: Vec::new(),
            by_model: Vec::new(),
            by_work: Vec::new(),
            merged: Vec::new(),
            cancelled: Vec::new(),
            added: Vec::new(),
            sent_back: Vec::new(),
            failed: Vec::new(),
            past: Vec::new(),
            under_way: Vec::new(),
            tests: Vec::new(),
            left_behind: Vec::new(),
            short_of_time: vec![day(2 * 86_400_000, 1), day(86_400_000, 4), day(0, 2)],
            stopping: None,
            red_main: Vec::new(),
            waited: Waited::default(),
        };
        let said = system_said(&system, &[], 7, 2);
        assert!(
            said.ends_with(
                "Calls of run_tests the fast check's budget kept tests of from their end, by day, the latest first:\n  \
                 Firn-wf on the day from 1970-01-03 00:00 UTC: 1 call; tests stopped 2, not started 3, refused 0\n  \
                 Firn-wf on the day from 1970-01-02 00:00 UTC: 4 calls; tests stopped 2, not started 3, refused 0\n  \
                 And 1 more: wf-agents system --days 7 --limit 3"
            ),
            "{said}"
        );
    }

    // The system view lists what each program run for a work left running and had stopped, the latest first, saying
    // which program it was: a try of a round, the check of an act's round or of a call, a keep-warm ping or a fork.
    // Microseconds.
    #[test]
    fn the_system_says_which_program_left_processes_running_and_how_many() {
        let left = |at: u64, of: LeftOf, processes: usize| web::system::LeftBehind {
            work: 4,
            item: 9,
            of,
            at,
            processes,
        };
        let system = System {
            now: 600_000,
            from: None,
            spent: Spent::default(),
            pinged: Spent::default(),
            weeks: Vec::new(),
            by_project: Vec::new(),
            by_cell: Vec::new(),
            by_model: Vec::new(),
            by_work: Vec::new(),
            merged: Vec::new(),
            cancelled: Vec::new(),
            added: Vec::new(),
            sent_back: Vec::new(),
            failed: Vec::new(),
            past: Vec::new(),
            under_way: Vec::new(),
            tests: Vec::new(),
            left_behind: vec![
                left(
                    300_000,
                    LeftOf::Fork {
                        cell: "I1".to_string(),
                        run: 7,
                        round: 1,
                    },
                    2,
                ),
                left(
                    240_000,
                    LeftOf::Ping {
                        line: "engineer".to_string(),
                        session: 2,
                    },
                    1,
                ),
                left(
                    180_000,
                    LeftOf::Call {
                        cell: "I1".to_string(),
                        run: 7,
                        call: 3,
                        tool: "run_tests".to_string(),
                    },
                    2,
                ),
                left(
                    120_000,
                    LeftOf::Act {
                        cell: "M1".to_string(),
                        run: 8,
                        round: 1,
                    },
                    3,
                ),
                left(
                    60_000,
                    LeftOf::Try {
                        cell: "I1".to_string(),
                        run: 7,
                        round: 2,
                        attempt: 1,
                    },
                    1,
                ),
            ],
            short_of_time: Vec::new(),
            stopping: None,
            red_main: Vec::new(),
            waited: Waited::default(),
        };
        let said = system_said(&system, &[], 7, 10);
        assert!(
            said.ends_with(
                "Left running by a program as it ended, and stopped, the latest first:\n  W4 of #9, a fork of round 1 \
                 of run 7 of I1, ended 1970-01-01 00:05 UTC: 2 processes\n  W4 of #9, a keep-warm ping of session 2 of \
                 the line engineer, ended 1970-01-01 00:04 UTC: 1 process\n  W4 of #9, the check of call 3 \
                 of the tool run_tests by run 7 of I1, ended 1970-01-01 00:03 UTC: 2 processes\n  W4 of #9, the check of \
                 round 1 of run 8 of M1, ended 1970-01-01 00:02 UTC: 3 processes\n  W4 of #9, try 1 of round 2 of run 7 \
                 of I1, ended 1970-01-01 00:01 UTC: 1 process"
            ),
            "{said}"
        );
    }

    // A work the wait every work shares holds while one round goes first says it waits for that round, not for the
    // cause the check found gone, and for the act that goes first where the wait is the acts'. Microseconds.
    #[test]
    fn a_work_held_while_one_round_goes_first_says_it_waits_for_that_one() {
        let mut outage = Outage {
            service: web::works::Service::Agents,
            failure: Failure::Offline,
            why: "Unable to connect to API".to_string(),
            work: 1,
            run: 2,
            round: 1,
            since: 0,
            checked: Some(Checked {
                at: 30_000,
                taken: 1,
                found: Found::Well,
            }),
            until: Until::Well { at: 30_000 },
            holds: true,
            first: Some(3),
            may_move: false,
        };
        assert_eq!(
            waited(&Waited::Agents { outage: outage.clone() }),
            "W3's round to come back: it is well again, and that round goes first"
        );
        outage.service = web::works::Service::Host;
        assert_eq!(
            waited(&Waited::Agents { outage: outage.clone() }),
            "W3's act to come back: it is well again, and that act goes first"
        );
        outage.first = None;
        assert!(
            waited(&Waited::Agents { outage }).starts_with("the agents' rounds to begin again, as "),
            "a wait no round goes first in says its cause"
        );
    }

    // Where the owner lets the engine move at a usage limit and another account may yet be moved to, the planner is told
    // the wait ends by the engine's move as well; where it may not, or the wait holds nothing, the sentence is not there.
    #[test]
    fn a_wait_at_a_usage_limit_says_the_engine_may_move_to_another_account_only_where_it_may() {
        let hour = 3_600_000;
        let mut waiting = items(vec![item(1, false, &[], &[])]);
        waiting.outage = Some(Outage {
            service: web::works::Service::Agents,
            failure: Failure::UsageLimit,
            why: "You've hit your limit".to_string(),
            work: 1,
            run: 2,
            round: 1,
            since: hour,
            checked: None,
            until: Until::Checked { at: 3 * hour },
            holds: true,
            first: None,
            may_move: true,
        });
        let moves = "The engine also checks the other accounts and moves every work to the first one it finds well.";
        let said = list(&waiting, &Listing::default(), 0);
        let first = said.lines().next().unwrap_or_default();
        assert_eq!(first.matches(moves).count(), 1, "{first}");
        assert!(first.contains("once it is well. The engine also checks"), "{first}");
        waiting.outage.as_mut().unwrap().may_move = false;
        let said = list(&waiting, &Listing::default(), 0);
        assert!(!said.contains(moves), "{said}");
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
            first: None,
            may_move: false,
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
    fn wait_says_a_red_main_with_its_tests_the_head_and_the_works_that_wait_for_it() {
        let hour = 3_600_000;
        let test = |name: &str, head: &str| RedTest {
            unit: "carry".to_string(),
            name: name.to_string(),
            head: head.to_string(),
        };
        let of = |work, item| Of {
            work,
            item,
            title: "Greet".to_string(),
        };
        let line = |tests: Vec<RedTest>, works: Vec<Of>| Line::MainRed {
            since: hour,
            project: "Firn-wf".to_string(),
            tests,
            works,
        };
        assert_eq!(
            heed(
                &line(
                    vec![test("tools::a", "0123456789abcdef0123")],
                    vec![of(184, 90), of(185, 91)]
                ),
                3 * hour
            ),
            "The main branch of Firn-wf is red: `tools::a` fails on its head 0123456789ab, for 2h 00m. Waiting for it \
             to pass: W184 for item #90, W185 for item #91."
        );
        // Many tests are named by the first few and counted; no work waiting is said so.
        let many: Vec<RedTest> = (1..=7).map(|n| test(&format!("t{n}"), "abc")).collect();
        assert_eq!(
            heed(&line(many, vec![]), 2 * hour),
            "The main branch of Firn-wf is red: `t1`, `t2`, `t3`, `t4`, `t5` and 2 more fail on its head abc, for 1h \
             00m. No work waits for it."
        );
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
            "Item #2 \"item 2\", work W2: for 1h 00m, stalled at merge. Nothing new runs this last cell, and no cell can \
             write what runs it."
        );
    }

    // What holds work back with no one asked, what the owner may answer ahead, a pause with no reason and an item at a
    // root are each said with how long they have stood; an answer that gives what stands whole says so first, with
    // when it gives it again.
    #[test]
    fn wait_says_each_thing_that_stands_with_how_long_it_has_stood() {
        let (minute, hour) = (60_000, 3_600_000);
        let of = |work| web::attention::Of {
            work,
            item: work,
            title: format!("item {work}"),
        };
        let round = Now::Round {
            cell: "merge".to_string(),
            run: 3,
            round: 1,
            since: hour,
            at: Some("https://ci.example/runs/9".to_string()),
            queued_for: Some("14900k".to_string()),
        };
        let turn = Line::Turn {
            since: hour,
            project: Some("Whitefoot".to_string()),
            holder: of(7),
            now: round.clone(),
            waiting: vec![of(8), of(9)],
        };
        assert_eq!(
            heed(&turn, 2 * hour),
            "The turn of Whitefoot to check and merge is held by item #7 \"item 7\", work W7, whose round or call has \
             been under way for 1h 00m; now a round of merge is under way, queued for 14900k, round 1 of run 3. Waiting \
             for the turn: W8, W9."
        );
        let queued = Line::Queued {
            of: of(7),
            since: hour,
            runner: "14900k".to_string(),
            now: round,
        };
        assert_eq!(
            heed(&queued, hour + 20 * minute),
            "Item #7 \"item 7\", work W7: has waited for the runner 14900k for 20m 00s: a round of merge is under way, \
             queued for 14900k, round 1 of run 3."
        );
        let machine = Line::Machine {
            of: of(4),
            since: hour,
            machine: "14900k".to_string(),
            behind: Some(of(7)),
            now: Now::Merged,
        };
        assert_eq!(
            heed(&machine, 2 * hour + 5 * minute),
            "Item #4 \"item 4\", work W4: its measurement has waited for its turn on the machine 14900k for 1h 05m, \
             behind item #7 \"item 7\", work W7: merged."
        );
        let ahead = Line::Ahead {
            of: of(3),
            since: hour,
            questions: vec![
                AskedAhead {
                    name: "scope".to_string(),
                    since: hour,
                },
                AskedAhead {
                    name: "bench".to_string(),
                    since: hour + 50 * minute,
                },
            ],
        };
        assert_eq!(
            heed(&ahead, 2 * hour),
            "Item #3 \"item 3\", work W3: has questions the owner may answer ahead: scope (1h 00m), bench (10m 00s)."
        );
        let paused = item(4, false, &[], &[]).work.now;
        let unexplained = Line::Unexplained {
            of: of(4),
            since: hour,
            now: paused,
        };
        assert!(
            heed(&unexplained, 3 * hour)
                .starts_with("Item #4 \"item 4\", work W4: the owner paused it 2h 00m ago and said no reason: "),
            "{}",
            heed(&unexplained, 3 * hour)
        );
        let slots = Line::Slots {
            since: hour,
            slots: 16,
            free: 11,
            items: 121,
        };
        assert_eq!(
            heed(&slots, 2 * hour),
            "11 of 16 slots are free while 121 items could be let on, the first of them for 1h 00m (`wf-agents \
             candidates` lists them)."
        );
        let root = Line::Root { of: of(5), since: hour };
        assert_eq!(
            heed(&root, 26 * hour),
            "Item #5 \"item 5\", work W5: stands at a root and is no deliverable: no item that has not ended depends on \
             it, for 1d 01h."
        );
        let answer = Attention {
            whole: true,
            lines: vec![root],
            left_out: 2,
            mark: "1.2.1".to_string(),
            again: 5 * minute,
        };
        let said = attention(&answer, 26 * hour);
        assert_eq!(
            said.lines().next(),
            Some(
                "Everything that stands to be handled, most pressing first, given again every 5m 00s until it is gone:"
            )
        );
        assert!(
            said.ends_with("And 2 more, given at once from this mark.\nNext: wf-agents wait --since 1.2.1"),
            "{said}"
        );
    }

    #[test]
    fn a_work_waiting_for_items_is_said_to_wait_for_them_to_merge() {
        let items = Waited::Items { items: vec![5, 7] };
        assert_eq!(waited(&items), "the items it depends on to merge: #5 and #7");
    }

    // The engine's wait before a round is tried again is said with the time it is until and how many times in a row, and,
    // where the host only kept the round's act from running for now, what kept it.
    #[test]
    fn the_engines_wait_before_a_round_is_said_with_what_kept_the_host_where_the_host_did() {
        use web::works::Obstacle;
        let wait = |obstacle| {
            called(&Call::Wait {
                until: 1_700_000_000_000,
                failed: 3,
                obstacle,
            })
        };
        let until = utc(1_700_000_000_000);
        assert_eq!(
            wait(None),
            format!(
                "the engine's wait until {until}, before the round that failed in passing (3 in a row) is tried again"
            )
        );
        for (obstacle, why) in [
            (Obstacle::Limit, "GitHub's limit on its requests"),
            (Obstacle::Fault, "GitHub failed"),
            (Obstacle::NoRunner, "no machine took the run"),
            (Obstacle::Network, "the network failed"),
        ] {
            assert_eq!(
                wait(Some(obstacle)),
                format!(
                    "the engine's wait until {until}, before the round the host kept from running is tried again: {why} (3 in a row)"
                )
            );
        }
    }

    // A call the host could not carry out for now is said with the time its wait is until, how many times in a row it
    // was held and what kept the host, and a call not held as it was.
    #[test]
    fn a_call_held_is_said_with_the_time_of_its_wait_and_what_kept_the_host() {
        let call = |held| Waited::Call {
            call: 2,
            who: None,
            what: Call::Tool {
                name: "run_tests".to_string(),
                input: "x".to_string(),
            },
            made: 1,
            at: None,
            queued_for: None,
            machine: None,
            held,
        };
        let held = |obstacle| {
            Some(web::works::HeldCall {
                until: 1_700_000_000_000,
                failed: 2,
                obstacle,
            })
        };
        let said = |obstacle| waited(&call(held(obstacle)));
        let until = utc(1_700_000_000_000);
        assert_eq!(
            said(web::works::Obstacle::Limit),
            format!(
                "call 2, the tool run_tests on \"x\", held until {until} (2 in a row): GitHub's limit on its requests"
            )
        );
        assert!(said(web::works::Obstacle::Fault).ends_with(": GitHub failed"));
        assert!(said(web::works::Obstacle::NoRunner).ends_with(": no machine took the run"));
        assert!(said(web::works::Obstacle::Network).ends_with(": the network failed"));
        assert_eq!(waited(&call(None)), "call 2, the tool run_tests on \"x\"");
        // A call that waits for its turn on a machine says which and since when.
        let waits = Waited::Call {
            call: 2,
            who: None,
            what: Call::Tool {
                name: "run_tests".to_string(),
                input: "x".to_string(),
            },
            made: 1,
            at: None,
            queued_for: None,
            machine: Some(web::works::MachineWait {
                machine: "14900k".to_string(),
                since: 1_700_000_000_000,
                behind: Some(33),
            }),
            held: None,
        };
        assert_eq!(
            waited(&waits),
            format!(
                "call 2, the tool run_tests on \"x\", waiting since {until} for its turn on the machine 14900k, where \
                 one measurement runs at a time, behind work W33"
            )
        );
    }

    // A dependency on a cancelled item holds nothing, and the words for it say so; one on an item that has not ended
    // is waited for.
    #[test]
    fn an_item_that_comes_to_depend_on_a_cancelled_one_is_said_not_to_wait_for_it() {
        let mut cancelled = item(1, false, &[], &[]);
        cancelled.work.now = Now::Cancelled;
        let some = items(vec![cancelled, item(2, false, &[], &[]), item(3, false, &[1, 2], &[])]);
        let asked = [
            ChangeAsked::DependsOn { item: 3, on: 1 },
            ChangeAsked::DependsOn { item: 3, on: 2 },
        ];
        assert_eq!(
            changed(&asked, &[true, true], Some(&some)),
            "#3 depends on #1, which is cancelled: its work does not wait for it.\n#3 depends on #2: its work waits \
             for it to merge."
        );
    }

    #[test]
    fn a_list_gives_at_most_a_hundred_items_and_says_how_to_see_the_rest() {
        let many = items((1..=105).map(|n| item(n, false, &[], &[])).collect());
        let said = list(&many, &Listing::default(), 0);
        let lines: Vec<&str> = said.lines().collect();
        assert_eq!(lines.len(), LISTED + 1);
        assert_eq!(lines[LISTED], "And 5 more: wf-agents list --from 101");
        let from = Listing {
            from: Some(101),
            ..Listing::default()
        };
        let rest = list(&many, &from, 0);
        assert_eq!(rest.lines().count(), 5);
        assert!(rest.starts_with("#101 "), "{rest}");
    }

    #[test]
    fn a_list_by_tag_gives_the_items_that_carry_it_and_says_what_one_waits_for() {
        // The engine says which items the work waits for; the list says what the engine says.
        let mut waiting = item(2, false, &[1], &["page"]);
        waiting.work.now = Now::Waits {
            at: "S1".to_string(),
            on: vec![Waited::Items { items: vec![1] }],
        };
        let some = items(vec![item(1, false, &[], &[]), waiting, item(3, true, &[], &["page"])]);
        let page = |all: bool| Listing {
            tag: Some("page".to_string()),
            from: None,
            all,
        };
        // What ended is left out, and counted, unless all are asked for.
        assert_eq!(
            list(&some, &page(false), 0),
            "#2 (project script) \"item 2\" at level 1, W2 on trial, after #1, tagged page: waits at S1 for the items \
             it depends on to merge: #1 (nothing has happened yet)\nAnd 1 item that ended, merged or cancelled: \
             wf-agents list --tag page --all"
        );
        assert_eq!(
            list(&some, &page(true), 0),
            "#2 (project script) \"item 2\" at level 1, W2 on trial, after #1, tagged page: waits at S1 for the items \
             it depends on to merge: #1 (nothing has happened yet)\n#3 (project script) \"item 3\" at level 1, W3 on \
             trial, tagged page: merged (nothing has happened yet)"
        );
        let none = Listing {
            tag: Some("none".to_string()),
            ..Listing::default()
        };
        assert_eq!(
            list(&some, &none, 0),
            "No item that has not ended carries the tag none."
        );
        // A list of none that has not ended still says how many ended.
        let ended = items(vec![item(3, true, &[], &[])]);
        assert_eq!(
            list(&ended, &Listing::default(), 0),
            "No item that has not ended.\nAnd 1 item that ended, merged or cancelled: wf-agents list --all"
        );
    }

    #[test]
    fn a_list_says_each_items_project_and_an_item_kept_before_projects_as_none() {
        let mut other = item(1, false, &[], &[]);
        other.project = Some("wf-agents".to_string());
        let mut kept = item(2, false, &[], &[]);
        kept.project = None;
        let said = list(&items(vec![other, kept]), &Listing::default(), 0);
        let lines: Vec<&str> = said.lines().collect();
        assert!(
            lines[0].starts_with("#1 (project wf-agents) \"item 1\" at level 1, W1 on trial: "),
            "{said}"
        );
        assert!(
            lines[1].starts_with("#2 (no project) \"item 2\" at level 1, W2 on trial: "),
            "{said}"
        );
    }

    // Every command that lists items begins each line of an item with its number and its project, as one function
    // writes them, so that items of several projects are told apart in a list that holds them together.
    #[test]
    fn every_command_that_lists_items_says_the_project_of_each_item() {
        use web::plan::{Candidate, Hit, PlanLine};
        let row = |item: u64, project: Option<&str>| Row {
            item,
            work: item,
            title: format!("item {item}"),
            level: Some("1".to_string()),
            project: project.map(str::to_string),
            tags: Vec::new(),
            standing: Standing::Paused,
        };
        let projects = [Some("alpha"), None];
        let mut said: Vec<(&str, String)> = Vec::new();

        let mut listed = vec![item(1, false, &[], &[]), item(2, false, &[], &[])];
        listed[0].project = Some("alpha".to_string());
        listed[1].project = None;
        said.push(("list", list(&items(listed), &Listing::default(), 0)));

        let candidates = Candidates {
            slots: 3,
            taken: 1,
            total: 2,
            waiting: 0,
            rows: (1..=2)
                .map(|n| Candidate {
                    row: row(n, projects[n as usize - 1]),
                    started: false,
                    waited_by: 0,
                })
                .collect(),
        };
        said.push(("candidates", candidates_said(&candidates)));

        let hits = Hits {
            hits: (1..=2)
                .map(|n| Hit {
                    item: n,
                    title: format!("item {n}"),
                    project: projects[n as usize - 1].map(str::to_string),
                    field: Field::Title,
                    snippet: "item".to_string(),
                    again: 0,
                })
                .collect(),
            more: 0,
        };
        said.push(("search", hits_said(&hits, "item")));

        // Each change at a time of its own, so that each is a line of its own.
        let line = |n: u64, time: u64, what: PlanChange| PlanLine {
            item: n,
            title: format!("item {n}"),
            project: projects[n as usize - 1].map(str::to_string),
            time,
            why: None,
            what,
        };
        let changes = PlanChanges {
            lines: vec![
                line(
                    1,
                    1,
                    PlanChange::Added {
                        level: Some("1".to_string()),
                        depends_on: Vec::new(),
                        tags: Vec::new(),
                    },
                ),
                line(
                    2,
                    2,
                    PlanChange::Changed {
                        change: ItemChanged::Tagged {
                            tag: "page".to_string(),
                        },
                    },
                ),
                line(1, 3, PlanChange::Cancelled),
                line(2, 4, PlanChange::Reopened),
            ],
            left_out: 0,
            mark: "2".to_string(),
        };
        said.push(("changes", plan_changes(&changes, false)));

        let graph = Graph {
            item: row(1, projects[0]),
            up: Vec::new(),
            down: vec![Node {
                row: row(2, projects[1]),
                again: false,
                next: Vec::new(),
                beyond: 0,
            }],
            left_out: 0,
        };
        said.push(("graph", graph_said(&graph, 3, None)));

        for (command, said) in said {
            let items: Vec<&str> = said
                .lines()
                .map(str::trim_start)
                .filter(|l| l.starts_with('#') || l.contains(": #"))
                .collect();
            assert!(!items.is_empty(), "{command} said no item: {said}");
            for line in items {
                let has = line.contains("(project alpha)") || line.contains("(no project)");
                assert!(has, "{command} left the project out of {line:?}");
            }
        }
        // Each is written as `#n (project p)`, the project of an item kept before projects as none.
        assert_eq!(item_ref(7, Some("alpha")), "#7 (project alpha)");
        assert_eq!(item_ref(7, None), "#7 (no project)");
    }

    #[test]
    fn a_list_and_a_show_say_when_something_last_happened_to_the_work_and_a_show_says_its_pull_request() {
        let hour = 3_600_000;
        let mut stood = item(1, false, &[], &[]);
        stood.work.last = Some(hour);
        let said = list(
            &items(vec![stood, item(2, false, &[], &[])]),
            &Listing::default(),
            3 * hour,
        );
        let lines: Vec<&str> = said.lines().collect();
        assert!(lines[0].ends_with(" (last happened 2h 00m ago)"), "{said}");
        assert!(lines[1].ends_with(" (nothing has happened yet)"), "{said}");

        let mut page = work(0, 0, 0, 0, 0);
        page.summary.last = Some(hour);
        page.summary.pull = Some(web::works::Pull {
            number: 7,
            url: "https://example.test/pull/7".to_string(),
        });
        let shown = show(&page, 3 * hour);
        let lines: Vec<&str> = shown.lines().collect();
        assert!(
            lines.contains(&"Last happened: 1970-01-01 01:00 UTC, 2h 00m ago."),
            "{shown}"
        );
        assert!(
            lines.contains(&"Pull request: 7, https://example.test/pull/7."),
            "{shown}"
        );
        let bare = show(&work(0, 0, 0, 0, 0), 3 * hour);
        assert!(bare.lines().any(|l| l == "Last happened: nothing yet."), "{bare}");
        assert!(bare.lines().any(|l| l == "Pull request: none."), "{bare}");
    }

    // A wait every work shares that holds nothing any more is said whole in the past, as the engine's check found it
    // well, with nothing to mend: no part of it says the command line cannot run, or that the owner is to do anything.
    #[test]
    fn a_wait_every_work_shares_that_holds_nothing_any_more_is_said_whole_in_the_past() {
        let hour = 3_600_000;
        let outage = Outage {
            service: web::works::Service::Agents,
            failure: Failure::CannotRun,
            why: "Not logged in".to_string(),
            work: 1,
            run: 2,
            round: 1,
            since: hour,
            checked: Some(Checked {
                at: 2 * hour,
                taken: 1,
                found: Found::Well,
            }),
            until: Until::Well { at: 2 * hour },
            holds: false,
            first: None,
            may_move: false,
        };
        let line = Line::Agents {
            since: hour,
            outage,
            mend: Some("Choose an account on the Settings view.".to_string()),
        };
        assert_eq!(
            heed(&line, 3 * hour),
            "The agents' command line could not run from 1970-01-01 01:00 UTC. The engine's check at 1970-01-01 02:00 \
             UTC found it well. Round 1 of run 2 of W1 met it. Not logged in."
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
                skipped: None,
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
        let shown = show(&work(25, 13, 12, 14, 12), 0);
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
        let long = show(&work(1000, 400, 300, 200, 150), 0);
        assert_eq!(long.lines().count(), lines.len(), "{long}");
        assert!(long.contains("  And 990 earlier: wf-agents show 1 runs\n"), "{long}");
        assert!(long.contains("  And 390 earlier: wf-agents show 1 sessions"), "{long}");
        assert!(long.contains("  And 190 more, 140 of them not answered: wf-agents show 1 questions"));
        // Fewer than the bound are each shown, and nothing is said of more.
        let few = show(&work(3, 3, 0, 1, 1), 0);
        assert_eq!(
            few.lines().filter(|l| l.starts_with("  engineer at ")).count(),
            3,
            "{few}"
        );
        assert_eq!(few.lines().filter(|l| l.starts_with("  run ")).count(), 3, "{few}");
        assert!(!few.contains(" earlier: ") && !few.contains(" more"), "{few}");
    }

    // A round's miss of its session's cache says why: what differed, that the records do not say, or that nothing
    // before the messages differed, which is a fault of wf-agents.
    #[test]
    fn a_round_that_missed_its_cache_says_why_by_what_differed_or_that_it_is_not_known_or_a_fault() {
        let missed = |differs: &[&str], unknown: bool| web::works::Missed {
            read: 1_200,
            held: 34_340,
            differs: differs.iter().map(|part| part.to_string()).collect(),
            unknown,
        };
        assert_eq!(
            missed_said(2, &missed(&["model", "program_tools"], false)),
            "    round 2 missed its session's cache: read 1200 of the 34340 tokens it held; model, program_tools differed from the call before"
        );
        let unknown = missed_said(2, &missed(&[], true));
        assert!(
            unknown.contains("the cause is not known") && !unknown.contains("fault"),
            "{unknown}"
        );
        let fault = missed_said(2, &missed(&[], false));
        assert!(
            fault.ends_with("nothing before the messages differed, a fault of wf-agents"),
            "{fault}"
        );
    }

    // A part given alone comes a hundred entries at a time, from the place asked, each answer saying how to ask for the
    // next.
    #[test]
    fn a_part_of_a_work_is_given_alone_a_hundred_at_a_time_from_the_place_asked() {
        let page = work(250, 0, 0, 0, 0);
        let first = show_part(&page, Part::Runs, 1);
        let lines: Vec<&str> = first.lines().collect();
        assert_eq!(lines[0], "Work W1, its runs, 1 to 100 of 250:");
        assert_eq!(lines.len(), LISTED + 2);
        assert!(lines[1].starts_with("  run 1 of plan: ") && lines[LISTED].starts_with("  run 100 of plan: "));
        assert_eq!(lines[LISTED + 1], "And 150 more: wf-agents show 1 runs --from 101");
        let last = show_part(&page, Part::Runs, 201);
        assert!(
            last.starts_with("Work W1, its runs, 201 to 250 of 250:\n  run 201 of plan: "),
            "{last}"
        );
        assert_eq!(last.lines().count(), 51, "the last answer says nothing more: {last}");
        assert_eq!(
            show_part(&page, Part::Runs, 251),
            "Work W1, its runs: 250, fewer than 251."
        );
        assert_eq!(show_part(&page, Part::Sessions, 1), "Work W1, its sessions: none.");
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
            "Work W1, what happened to its item, 1 to 4 of 4:".to_string(),
            format!("  {}: tagged t1", utc(10)),
            format!("  {}: cancelled, no reason being given", utc(15)),
            format!("  {}: tagged t2", utc(20)),
            format!("  {}: reopened, because \"Wanted after all.\"", utc(25)),
        ];
        assert_eq!(said, lines.join("\n"));
        assert!(show(&page, 0).contains("; changed since:\n"));
        assert!(show(&work(0, 0, 0, 0, 0), 0).contains(", and has not changed.\nCells:\n"));
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
                reply: false,
                read: false,
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
    fn what_an_item_continues_is_given_by_two_flags_and_changed_by_three_kinds() {
        assert_eq!(continues(None, None), None);
        assert_eq!(
            continues(Some("old".to_string()), None),
            Some(Continues::Branch {
                name: "old".to_string()
            })
        );
        assert_eq!(continues(None, Some(226)), Some(Continues::Pull { number: 226 }));
        let given: Vec<String> = [
            "3:continue-branch=work-226-0a1b2c3d",
            "#4:continue-pull=226",
            "5:no-longer-continues",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(
            changes(&given),
            Ok(vec![
                ChangeAsked::Continues {
                    item: 3,
                    continues: Some(Continues::Branch {
                        name: "work-226-0a1b2c3d".to_string()
                    })
                },
                ChangeAsked::Continues {
                    item: 4,
                    continues: Some(Continues::Pull { number: 226 })
                },
                ChangeAsked::Continues {
                    item: 5,
                    continues: None
                },
            ])
        );
        for wrong in [
            "3:continue-branch",
            "3:continue-pull=x",
            "3:continue-pull",
            "3:no-longer-continues=x",
        ] {
            let refused = changes(&[wrong.to_string()]).expect_err(wrong);
            assert!(
                refused.contains("A change is ITEM:KIND or ITEM:KIND=VALUE"),
                "{refused}"
            );
        }
    }

    #[test]
    fn an_item_that_continues_something_says_so_where_it_is_listed_and_where_its_history_is_told() {
        let continuing = Some(Continues::Pull { number: 226 });
        let mut history = item(1, false, &[], &[]).history;
        assert!(!added(&history, true).contains("continuing"));
        history.first.continues = continuing.clone();
        assert!(
            added(&history, true).contains(", continuing pull request 226"),
            "{}",
            added(&history, true)
        );
        assert_eq!(
            item_changed(&ItemChanged::Continues {
                continues: Some(Continues::Branch {
                    name: "old".to_string()
                })
            }),
            "came to continue the branch old"
        );
        assert_eq!(
            item_changed(&ItemChanged::Continues { continues: None }),
            "no longer continues anything"
        );
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
            by_project: Vec::new(),
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
            tests: Vec::new(),
            left_behind: Vec::new(),
            short_of_time: Vec::new(),
            stopping: None,
            red_main: Vec::new(),
            waited: Waited::default(),
        };
        let said = system_said(&system, &[], 7, 10);
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

    // What a project's merged checks say of time is bounded as every list of the report is: the slowed tests and the
    // latest merges each at most the limit, with how many more there are and how to see them; a series that grew is said
    // with the merge it stepped at.
    #[test]
    fn the_times_of_a_projects_merged_checks_are_said_bounded_with_how_many_more_there_are() {
        use web::system::{Grown, JobTime, MergeTimes, SlowedTest, TestTimes};
        let tests = TestTimes {
            project: Some("wf-agents".to_string()),
            checks: 40,
            slowed: (1..=7)
                .map(|n| SlowedTest {
                    work: n,
                    unit: "carry".to_string(),
                    name: format!("t{n}"),
                    millis: 9_000,
                    median: 3_000,
                    line: 5_000,
                })
                .collect(),
            grown: vec![Grown {
                of: "tests".to_string(),
                work: 33,
                before: 100_000,
                after: 130_000,
                above: 18,
                n: 19,
                needed: 17,
            }],
            merges: (1..=8)
                .map(|n| MergeTimes {
                    work: n,
                    jobs: vec![JobTime {
                        name: "lint".to_string(),
                        millis: 7_000,
                    }],
                    tests_millis: 100_000 + n * 10,
                    added: 2,
                    added_millis: 300,
                })
                .collect(),
        };
        let said = tests_said(&tests, 7, 3);
        assert_eq!(said.len(), 12, "{said:#?}");
        assert_eq!(
            said[0],
            "Times of the tests and jobs of wf-agents, from its 40 merged checks:"
        );
        assert_eq!(
            said[2],
            "    t1 in carry: 9.0 s at W1, against a median of 3.0 s and a line of 5.0 s"
        );
        assert_eq!(said[5], "  And 4 more: wf-agents system --days 7 --limit 7");
        assert!(
            said[6].starts_with("  The CI grew: the sum of the tests' times had a median of 100.0 s")
                && said[6].ends_with("it stepped at the merge of W33."),
            "{}",
            said[6]
        );
        assert_eq!(
            said[8],
            "    W6: jobs lint 7.0 s; the tests 100.1 s; 2 tests added, 300 ms"
        );
        assert_eq!(
            said[10],
            "    W8: jobs lint 7.0 s; the tests 100.1 s; 2 tests added, 300 ms"
        );
        assert_eq!(said[11], "  And 5 more: wf-agents system --days 7 --limit 8");
        let none = TestTimes {
            slowed: Vec::new(),
            grown: Vec::new(),
            merges: Vec::new(),
            ..tests
        };
        let said = tests_said(&none, 7, 3);
        assert_eq!(said.len(), 3, "{said:#?}");
        assert!(
            said[1].starts_with("  No test took longer than its own history says"),
            "{}",
            said[1]
        );
    }

    // A round under way is said with what its calls have come to so far, each against the line the run guard judges it
    // by and whether it is past it, and one whose carrier has said nothing yet is said as before. Fails where a figure is
    // left out, joined to another round's, or its line or its being past it is misread. Microseconds.
    #[test]
    fn a_round_under_way_is_said_with_what_its_calls_have_come_to_against_their_lines() {
        let round = |work: u64| UnderWay::Round {
            work,
            item: work + 10,
            cell: "I1".to_string(),
            run: 2,
            round: 1,
            began: 0,
            time: None,
            rest: None,
        };
        let line = |value: u64| web::works::Line {
            value,
            samples: 5,
            scope: web::works::Scope::Cell,
        };
        let figure = |value: u64, line: Option<web::works::Line>, past: bool| Figure { value, line, past };
        let system = System {
            now: 600_000,
            from: Some(0),
            spent: Spent::default(),
            pinged: Spent::default(),
            weeks: Vec::new(),
            by_project: Vec::new(),
            by_cell: Vec::new(),
            by_model: Vec::new(),
            by_work: Vec::new(),
            merged: Vec::new(),
            cancelled: Vec::new(),
            added: Vec::new(),
            sent_back: Vec::new(),
            failed: Vec::new(),
            past: Vec::new(),
            under_way: vec![round(3), round(4)],
            tests: Vec::new(),
            left_behind: Vec::new(),
            short_of_time: Vec::new(),
            stopping: None,
            red_main: Vec::new(),
            waited: Waited::default(),
        };
        let going = [Going {
            work: 3,
            run: 2,
            round: 1,
            context: Some(12_000),
            added: Some(figure(4_000, Some(line(3_000)), true)),
            per_call: Some(figure(2_000, Some(line(2_500)), false)),
            cost: Some(figure(250_000, None, false)),
        }];
        let said = system_said(&system, &going, 7, 10);
        let lines: Vec<&str> = said.lines().filter(|l| l.starts_with("  W")).collect();
        assert_eq!(lines.len(), 2, "{said}");
        assert!(
            lines[0].ends_with(
                "; its context is 12000 tokens, its context has grown by 4000 tokens, past the line of 3000 tokens \
                 that 5 rounds of its cell give, and by 2000 tokens a call after the first, within the line of 2500 \
                 tokens a call after the first that 5 rounds of its cell give, its calls cost $0.25, no line yet"
            ),
            "{}",
            lines[0]
        );
        assert!(
            !lines[1].contains("context"),
            "a round its carrier has said nothing of has no figures: {}",
            lines[1]
        );
    }

    // A stop says each round under way it waits for, or would cut off, with how long it has taken and how much longer
    // it is expected to take, and when the engine is expected to end, the latest of them; the system view says the
    // stop that stands and each round's rest. Fails where a round is left out, its rest misread, or the end not the
    // latest. Microseconds.
    #[test]
    fn a_stop_says_the_rounds_it_waits_for_or_would_cut_off_with_how_long_each_has_to_go() {
        let round = |work: u64, began: u64, rest| UnderWay::Round {
            work,
            item: work + 10,
            cell: "I1".to_string(),
            run: 2,
            round: 1,
            began,
            time: None,
            rest,
        };
        let mut system = System {
            now: 600_000,
            from: Some(0),
            spent: Spent::default(),
            pinged: Spent::default(),
            weeks: Vec::new(),
            by_project: Vec::new(),
            by_cell: Vec::new(),
            by_model: Vec::new(),
            by_work: Vec::new(),
            merged: Vec::new(),
            cancelled: Vec::new(),
            added: Vec::new(),
            sent_back: Vec::new(),
            failed: Vec::new(),
            past: Vec::new(),
            under_way: vec![
                round(1, 0, Some(Rest::Expected { ms: 120_000 })),
                round(2, 540_000, Some(Rest::Expected { ms: 300_000 })),
            ],
            tests: Vec::new(),
            left_behind: Vec::new(),
            short_of_time: Vec::new(),
            stopping: None,
            red_main: Vec::new(),
            waited: Waited::default(),
        };
        let listed =
            "  W1 of #11 at I1: 10m 00s so far, about 2m 00s more\n  W2 of #12 at I1: 1m 00s so far, about 5m 00s more";
        assert_eq!(
            stop_said(&system, Stopped::NotYet),
            format!(
                "Nothing is stopped: `wf-agents stop --now --yes` stops the engine at once and cuts off these 2 rounds, \
                 which `wf-agents stop` waits for:\n{listed}"
            )
        );
        assert_eq!(
            stop_said(&system, Stopped::Once),
            format!(
                "The engine is stopping: no round begins, and it ends once these 2 rounds have ended, expected in about \
                 5m 00s:\n{listed}"
            )
        );
        system.under_way[0] = round(1, 0, Some(Rest::Longer {}));
        assert!(
            stop_said(&system, Stopped::Once).starts_with(
                "The engine is stopping: no round begins, and it ends once these 2 rounds have ended, once a round \
                 that has taken longer than any before has ended:\n  W1 of #11 at I1: 10m 00s so far, longer than any \
                 round of its history\n"
            ),
            "{}",
            stop_said(&system, Stopped::Once)
        );
        system.stopping = Some(web::system::Stop {
            since: 0,
            at_once: false,
            rest: Rest::Longer {},
        });
        let said = system_said(&system, &[], 7, 10);
        assert!(
            said.contains(
                "The engine was asked 1970-01-01 00:00 UTC to stop once what is under way has ended: no round begins, \
                 and it is to end once a round that has taken longer than any before has ended.\nUnder way"
            ),
            "{said}"
        );
        assert!(
            said.contains(
                "Under way, the oldest first:\n  W1 of #11, round 1 of run 2 of I1, since 1970-01-01 00:00 UTC: 10m 00s \
                 so far, no line yet, too few rounds of its cell having ended; longer than any round of its history\n  \
                 W2 of #12, round 1 of run 2 of I1, since 1970-01-01 00:09 UTC: 1m 00s so far, no line yet, too few \
                 rounds of its cell having ended; about 5m 00s more"
            ),
            "{said}"
        );
    }

    #[test]
    fn a_call_past_its_line_by_the_size_of_its_result_is_said_with_its_bytes_beside_its_time() {
        let line = |value| web::works::Line {
            value,
            samples: 2,
            scope: web::works::Scope::Tool,
        };
        let against = |value, line_at, past| Against {
            value,
            line: line(line_at),
            past,
        };
        let call = |time, size| Past::Call {
            work: 7,
            item: 3,
            cell: "I1".to_string(),
            run: 2,
            call: 9,
            tool: "run_tests".to_string(),
            done: 0,
            time,
            size,
        };
        let system = System {
            now: 0,
            from: Some(0),
            spent: Spent::default(),
            pinged: Spent::default(),
            weeks: Vec::new(),
            by_project: Vec::new(),
            by_cell: Vec::new(),
            by_model: Vec::new(),
            by_work: Vec::new(),
            merged: Vec::new(),
            cancelled: Vec::new(),
            added: Vec::new(),
            sent_back: Vec::new(),
            failed: Vec::new(),
            past: vec![
                call(Some(against(2_000, 2_000, false)), Some(against(60_000, 12_500, true))),
                call(
                    Some(against(100_000, 25_000, true)),
                    Some(against(60_000, 12_500, true)),
                ),
            ],
            under_way: Vec::new(),
            tests: Vec::new(),
            left_behind: Vec::new(),
            short_of_time: Vec::new(),
            stopping: None,
            red_main: Vec::new(),
            waited: Waited::default(),
        };
        let said = system_said(&system, &[], 7, 10);
        let lines: Vec<&str> = said.lines().collect();
        let head = "  W7 of #3, call 9 of the tool run_tests by run 2 of I1, done 1970-01-01 00:00 UTC: ";
        let size = "returned 60000 bytes, past the line of 12500 bytes that 2 calls of its tool give";
        assert_eq!(lines[2], format!("{head}{size}"), "{said}");
        assert_eq!(
            lines[3],
            format!("{head}took 1m 40s, past the line of 25s that 2 calls of its tool give; {size}"),
            "{said}"
        );
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

/// What a fork answered, in words: each field of its return that holds something, by its key, each cut at
/// [`FORK_FIELD`] characters, saying where it is kept whole; else what its program said; then what it cost, the share
/// of the session its first request read from the cache, said plainly where it missed it ([`driver::cache::missed`]); where the
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
    if page.left > 0 {
        let processes = if page.left == 1 { "process" } else { "processes" };
        out.push_str(&format!(
            "Its program left {} {processes} running in its group as it ended, which were stopped.\n",
            page.left
        ));
    }
    let share = match (page.read, page.held) {
        (Some(read), Some(held)) if held > 0 => Some((read.min(held) * 100 / held, read, held)),
        _ => None,
    };
    match (share, page.expired) {
        (Some((share, read, held)), _) if !driver::cache::missed(read, held) => out.push_str(&format!(
            "Its first request read {share}% of the session from the cache ({read} of the {held} tokens it held).\n"
        )),
        (Some((share, read, held)), Some(minutes)) => out.push_str(&format!(
            "Its first request read only {share}% of the session from the cache ({read} of the {held} tokens it held): the \
             session's cache had expired {minutes} minutes before.\n"
        )),
        (Some((share, read, held)), None) => out.push_str(&format!(
            "Its first request read only {share}% of the session from the cache ({read} of the {held} tokens it held), \
             under half: the fork did not begin as the session did, and was not read from the cache.\n"
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
        Some(ForkInstead::AtLimit { account }) => out.push_str(&format!(
            "The session ran under the account {account}, which is at its usage limit: the fork ran under the account \
             in use, {ran}, whose cache holds nothing of the session.\n"
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
            left: 0,
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
            "read only 30% of the session from the cache (3000 of the 10000 tokens it held), under half: the fork did \
             not begin as the session did, and was not read from the cache."
        ));
        // The one test of a miss, a round's and a fork's: under half. A fork that read 60% read the session, and one
        // that read 40% did not.
        let said = |read| {
            forked_said(&ForkPage {
                read: Some(read),
                ..page()
            })
        };
        assert!(said(6_000).contains("Its first request read 60% of the session from the cache"));
        assert!(said(4_000).contains("Its first request read only 40% of the session from the cache"));
        assert!(
            said(5_000).contains("Its first request read 50% of the session from the cache"),
            "half is not under"
        );
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
        let limited = ForkPage {
            instead: Some(ForkInstead::AtLimit {
                account: "home".to_string(),
            }),
            ..page()
        };
        assert!(forked_said(&limited).contains(
            "The session ran under the account home, which is at its usage limit: the fork ran under the account in \
             use, work"
        ));
        assert!(!forked_said(&page()).contains("the fork ran under the account in use"));
        // What its program left running, stopped as it ended, is said with its cost, and nothing where it left none.
        let left = ForkPage { left: 2, ..page() };
        assert!(forked_said(&left).contains(
            "It cost $0.0410.\nIts program left 2 processes running in its group as it ended, which were stopped.\n"
        ));
        assert!(!forked_said(&page()).contains("running in its group"));
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
