//! The agents' rounds, run with the `claude` command line: each round one run of the program, in the work's worktree,
//! in the session of the cell's line, told its message on its standard input, with the same system prompt and the
//! same form of return for every round. At most as many rounds run at once as the settings give slots; the others
//! wait their turn.
//!
//! A round fails, handing in nothing, where the worktree is not there, the program cannot be started, it exits with
//! an error or prints a result that is one, or what it prints is no result with a structured return. It is not tried
//! again here: the work is paused for the owner to see why. A round that fails takes what the result the program
//! printed says it cost, as any round does, and is held to the budget as any round is; where the program printed
//! nothing that reads as a result, what the round took is not known, and is recorded as nothing. Stopped, the program's process group is sent TERM, and
//! KILL after three seconds, and the round ends with nothing handed in; what it spent is not known, and is recorded
//! as nothing.
//!
//! The program is given an environment of its own, not the engine's (`cli::environment`). What a round is told depends
//! on whether it begins its line's session, and is built again where the program is tried the other way.
//!
//! What each round was told and what the program printed are kept for reading, in `rounds/<work>/<run>-<round>` in
//! the data directory: `.message.md`, `.result.jsonl` (the whole stream) and `.result.json` (its result line alone,
//! empty where it has none) for its first try, `.message.2.md`, `.result.2.jsonl` and `.result.2.json` for the next,
//! and so on. While the program runs, its process group is written in `.pgid` there; a group left named there by a
//! process of the engine that ended is stopped before the round is run again, or before the round's stop ends it, so
//! that a session has one program at a time and a stopped round's program does not run on.
//!
//! A round that made a request has its first request judged against what its session's previous round left in the
//! prompt cache, and the numbers and the judgement are kept in `.cache.json` there ([`cache`]). Where it missed, the
//! engine's standard error says so, and, unless the settings turn the check off, the work waits for the owner with
//! the reason, as it does over its budget, what the round handed in being taken.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use decide::RunId;
use engine::Came;
use service::{Clock, Round, RoundReply, Rounds, SystemClock};
use store::WorkNumber;
use tokio::process::Command;
use tokio::sync::Semaphore;
use uuid::Uuid;
use work::{Spent, Time};

use crate::cache::{self, Before, Judged, Kept};
use crate::cli::{self, Begin};
use crate::held::{Failing, Held, Stop};
use crate::process::{self, Ran};
use crate::settings::Settings;
use crate::{acts, git, message, returns, session};

/// The system prompt every round is given, before the owner's language is put in.
pub const PROTOCOL: &str = include_str!("protocol.md");

/// The directory in the data directory where what each round was told and what it returned are kept.
pub const ROUNDS: &str = "rounds";

/// The agents' rounds.
#[derive(Clone)]
pub struct Agents {
    inner: Arc<Inner>,
}

struct Inner {
    settings: Settings,
    data: PathBuf,
    installation: Uuid,
    /// The system prompt, the same for every round.
    protocol: String,
    slots: Semaphore,
    held: Held<(WorkNumber, RunId, usize), RoundReply>,
}

/// What came of running a round's program.
enum Carried {
    /// This came back, having taken this; and the work is to wait for the owner for this reason, where the round
    /// missed the cache.
    Came(Came, Spent, Option<String>),
    /// It failed, for this reason, having taken this, as the result it printed says.
    Failed(String, Spent),
    Stopped,
}

impl Agents {
    /// The agents of these settings, with this data directory, which is an absolute path, and the installation's id.
    pub fn new(settings: Settings, data: PathBuf, installation: Uuid) -> Self {
        let protocol = PROTOCOL.replace("{owner_language}", &settings.agents.owner_language);
        Agents {
            inner: Arc::new(Inner {
                slots: Semaphore::new(settings.agents.slots),
                settings,
                data,
                installation,
                protocol,
                held: Held::new(),
            }),
        }
    }
}

impl Rounds for Agents {
    fn give(&self, round: Round, reply: RoundReply) {
        acts::prune(&self.inner.held, &round);
        let key = (round.work, round.run, round.number);
        let Some(mut stop) = self.inner.held.hold(key, reply) else {
            return;
        };
        let inner = self.inner.clone();
        tokio::spawn(async move {
            // A round stopped while it waits its turn is not run at all.
            let permit = tokio::select! {
                permit = inner.slots.acquire() => permit.ok(),
                () = stop.told() => None,
            };
            let carried = match permit {
                Some(_permit) => inner.carry(&round, &mut stop).await,
                None => Ok(Carried::Stopped),
            };
            match carried {
                Ok(Carried::Came(came, spent, missed)) => {
                    let Some(reply) = inner.held.take(key) else {
                        return;
                    };
                    // Where the round both missed the cache and took the work over its budget, one reason says both.
                    let why = [missed, inner.over_budget(&round, &spent)].into_iter().flatten();
                    match why.reduce(|missed, over| format!("{missed}; and {over}")) {
                        Some(why) => reply.ended_and_paused(came, spent, why),
                        None => reply.ended(came, spent),
                    }
                }
                Ok(Carried::Stopped) => {
                    if let Some(reply) = inner.held.take(key) {
                        reply.ended(Came::HandsIn(Vec::new()), Spent::default());
                    }
                }
                Ok(Carried::Failed(why, spent)) => match inner.held.fail(key) {
                    // The work is paused for the failure; where the round took it over its budget, the reason says
                    // that too.
                    Some(Failing::Fail(reply)) => {
                        let why = match inner.over_budget(&round, &spent) {
                            Some(over) => format!("{why}; and {over}"),
                            None => why,
                        };
                        reply.failed(why, spent);
                    }
                    Some(Failing::End(reply)) => reply.ended(Came::HandsIn(Vec::new()), spent),
                    None => {}
                },
                // What a round that failed before its program printed a result took is not known: it is kept as
                // nothing.
                Err(why) => match inner.held.fail(key) {
                    Some(Failing::Fail(reply)) => reply.failed(why, Spent::default()),
                    Some(Failing::End(reply)) => reply.ended(Came::HandsIn(Vec::new()), Spent::default()),
                    None => {}
                },
            }
        });
    }

    fn stop(&self, round: Round, reply: RoundReply) {
        acts::prune(&self.inner.held, &round);
        let key = (round.work, round.run, round.number);
        if let Some(reply) = self.inner.held.stop(key, reply) {
            // Nothing here carries the round out; a program a process of the engine that ended left running for it
            // is stopped before the round ends.
            let group = self.inner.kept(&round).with_extension("pgid");
            let dir = git::place(&self.inner.data, &git::tag(&self.inner.installation), round.work).dir;
            tokio::spawn(async move {
                process::stop_left(&group, &dir).await;
                reply.ended(Came::HandsIn(Vec::new()), Spent::default());
            });
        }
    }
}

impl Inner {
    /// Runs the round's program, and gives what came back and what it took; or that the round failed, why and what it
    /// took, where the program printed a result; or, where it printed none, only why.
    async fn carry(&self, round: &Round, stop: &mut Stop) -> Result<Carried, String> {
        let (flow, work) = (&*round.flow, &*round.kept);
        let line = (round.cell.line.as_deref()).ok_or_else(|| format!("the cell {} names no line", round.cell.id))?;
        let place = git::place(&self.data, &git::tag(&self.installation), round.work);
        let dir = git::worktree(&place)?;
        let base = self.kept(round);
        if let Some(parent) = base.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let group = base.with_extension("pgid");
        process::stop_left(&group, &dir).await;
        let session = session::session(self.installation, round.work, line);
        let mut begin = cli::begin(flow, work, line);
        let before = cli::spent_before(flow, work, line);
        let mut tried = false;
        let (status, stdout, stderr, began, requests) = loop {
            let begins = begin == Begin::New;
            let told = message::message(flow, work, &round.cell, round.run, round.number, &round.given, begins);
            let at = attempt(&base).await;
            write(&at.told, told.as_bytes()).await?;
            let command = self.command(&dir, session, begin);
            let began = SystemClock.now();
            let ran = process::run(command, Some(told.into_bytes()), Some(&group), stop).await?;
            let Ran::Ended { status, stdout, stderr } = ran else {
                return Ok(Carried::Stopped);
            };
            write(&at.stream, &stdout).await?;
            write(&at.result, cli::result(&stdout).unwrap_or_default()).await?;
            let stderr = String::from_utf8_lossy(&stderr).into_owned();
            let requests = cli::requests(&stdout);
            // A session begun before its first round was cut off, or one the program no longer has, is tried the
            // other way, once: the program then made no request.
            if !status.success() && requests.is_empty() && !tried && begin.refused(&stderr) {
                begin = begin.other();
                tried = true;
                continue;
            }
            break (status, stdout, stderr, began, requests);
        };
        let missed = self.judge(round, line, begin, began, &requests, &base).await;
        // Where the round failed, the reason says that it missed the cache too.
        let and_missed = |why: String| match &missed {
            Some(missed) => format!("{why}; and {missed}"),
            None => why,
        };
        let read = cli::read(&stdout, begin, before);
        // A failure whose result was read took what the result says; one with none, what is not known.
        let failed = |why: String, spent: Option<Spent>| match spent {
            Some(spent) => Ok(Carried::Failed(and_missed(why), spent)),
            None => Err(and_missed(why)),
        };
        if !status.success() {
            let tail = || process::tail(stderr.trim().as_bytes());
            let (why, spent) = match read {
                Err(no) if !stdout.is_empty() => (no.why, no.spent),
                Err(no) => (tail(), no.spent),
                Ok(read) => (tail(), Some(read.spent)),
            };
            return failed(
                format!("{} exited with {status}: {why}", self.settings.agents.command),
                spent,
            );
        }
        let read = match read {
            Ok(read) => read,
            Err(no) => return failed(no.why, no.spent),
        };
        match returns::came(&read.structured) {
            Ok(came) => Ok(Carried::Came(came, read.spent, missed)),
            Err(why) => failed(why, Some(read.spent)),
        }
    }

    /// Judges the round's first request against what the session's previous round left in the prompt cache, the
    /// round's program having been started at `began`, and keeps the numbers and the judgement beside the round's
    /// files; and gives why the work is to wait for the owner, where the round missed and the check is on. A round
    /// that made no request is not judged, and keeps nothing.
    async fn judge(
        &self,
        round: &Round,
        line: &str,
        begin: Begin,
        began: Result<Time, String>,
        requests: &[cache::Request],
        base: &Path,
    ) -> Option<String> {
        let (first, last) = (requests.first()?, requests.last()?);
        let at = base.with_extension(cache::EXTENSION);
        let times = began.and_then(|began| Ok((began, SystemClock.now()?)));
        let (began, ended) = match times {
            Ok(times) => times,
            Err(why) => {
                eprintln!("{}: not kept: {why}", at.display());
                return None;
            }
        };
        let before = match begin {
            Begin::New => Before::Nothing,
            Begin::Resume => self.before(round, line).await,
        };
        let judged = cache::judge(&before, began, first, &self.settings.cache());
        let kept = Kept {
            began,
            ended,
            first: first.clone(),
            last: last.clone(),
            judged: judged.clone(),
        };
        // A file not kept leaves the session's next round not judged, which is said; the round is not failed for it.
        let text = serde_json::to_vec_pretty(&kept).unwrap_or_default();
        if let Err(why) = write(&at, &text).await {
            eprintln!("{why}");
        }
        let (Judged::Miss { expected, read }, Before::Round(previous)) = (judged, before) else {
            return None;
        };
        let minutes = began.0.saturating_sub(previous.ended.0) / 60_000;
        let why = cache::missed(round.run.0, round.number, expected, read, minutes);
        eprintln!("Work {}: {why}.", round.work.0);
        self.settings.agents.cache_check.then_some(why)
    }

    /// What came before the round in its line's session, which it goes on in: the latest round of the line that
    /// ended having made a request, as its kept numbers say, or as what it took says where it kept none. A round
    /// that took nothing and kept nothing made no request, and is passed over.
    async fn before(&self, round: &Round, line: &str) -> Before {
        for (run, number, spent) in cli::line_ended(&round.flow, &round.kept, line).into_iter().rev() {
            let at = self.kept_at(round.work, run, number).with_extension(cache::EXTENSION);
            match tokio::fs::read(&at).await {
                Ok(text) => {
                    return serde_json::from_slice(&text).map_or(Before::Unknown, Before::Round);
                }
                Err(_) if spent.input_tokens + spent.cached_tokens + spent.output_tokens > 0 => {
                    return Before::Unknown;
                }
                Err(_) => {}
            }
        }
        Before::Unknown
    }

    /// The program's command line for a round in the session, begun or gone on in, in the worktree.
    fn command(&self, dir: &Path, session: Uuid, begin: Begin) -> Command {
        let agents = &self.settings.agents;
        let mut command = Command::new(&agents.command);
        command
            .args(cli::arguments(agents, &self.protocol, returns::SCHEMA, session, begin))
            .env_clear()
            .envs(cli::environment(agents, std::env::vars()))
            .current_dir(dir);
        command
    }

    /// Where what the round was told and returned are kept, without the extension.
    fn kept(&self, round: &Round) -> PathBuf {
        self.kept_at(round.work, round.run, round.number)
    }

    /// Where what the round of this number of the run of the work was told and returned are kept, without the
    /// extension.
    fn kept_at(&self, work: WorkNumber, run: RunId, number: usize) -> PathBuf {
        let dir = self.data.join(ROUNDS).join(work.0.to_string());
        dir.join(format!("{run}-{number}", run = run.0))
    }

    /// Why the work is to wait for the owner, where the round took it over its budget: everything its rounds have
    /// spent, this one with them, is more than the settings let a work spend.
    fn over_budget(&self, round: &Round, spent: &Spent) -> Option<String> {
        let budget = self.settings.budget_micros()?;
        // No other round of the work ended while this one was under way: a work has one round under way at a time.
        let total = round.kept.spent.values().map(|s| s.usd_micros).sum::<u64>() + spent.usd_micros;
        let dollars = |micros: u64| micros as f64 / 1_000_000.0;
        (total > budget).then(|| {
            format!(
                "work {} has spent ${:.2}, over its budget of ${:.2}",
                round.work.0,
                dollars(total),
                dollars(budget)
            )
        })
    }
}

/// Where a try of a round keeps what it is told and what the program printed.
struct Attempt {
    /// What it is told.
    told: PathBuf,
    /// The whole stream the program printed.
    stream: PathBuf,
    /// The stream's result line alone.
    result: PathBuf,
}

/// Where the next try of the round keeps what it is told and what it returns: the first that has kept nothing yet.
async fn attempt(base: &Path) -> Attempt {
    let at = |n: usize| {
        let n = if n == 1 { String::new() } else { format!(".{n}") };
        Attempt {
            told: base.with_extension(format!("message{n}.md")),
            stream: base.with_extension(format!("result{n}.jsonl")),
            result: base.with_extension(format!("result{n}.json")),
        }
    };
    let mut n = 1;
    while tokio::fs::try_exists(&at(n).told).await.unwrap_or(false) {
        n += 1;
    }
    at(n)
}

/// Writes the file, making its directory where there is none.
async fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    tokio::fs::write(path, bytes)
        .await
        .map_err(|e| format!("{}: {e}", path.display()))
}
