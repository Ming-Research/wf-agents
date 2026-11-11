//! The agents' rounds, run with the `claude` command line: each round one run of the program, in the work's worktree,
//! in the session of the cell's line, told its message on its standard input, with the same form of return for every
//! round and the same system prompt for every round of a line, which says what the line may run as its permissions
//! give it to the program (`permissions`). At most as many rounds run at once as the settings give slots; the others
//! wait their turn.
//!
//! A round fails, handing in nothing, where the worktree is not there, the program cannot be started, it exits with
//! an error or prints a result that is one, or what it prints is no result with a structured return of the form. The
//! kind of its cause is told apart from what the program reported ([`failure`](crate::failure)): a program that cannot
//! be started cannot run, a return not of the form could not be read, and a round that failed before its program ran
//! failed for a cause no kind tells apart. It is not tried again here: the engine handles it as its kind says. A round
//! that fails takes what the result the program printed says it cost, as any round does, and is held to the budget as
//! any round is, its work paused where it went over; where the program printed nothing that reads as a result, what
//! the round took is not known, and is recorded as nothing. Stopped, the program's process group is sent TERM, and
//! KILL after three seconds, and the round ends with nothing handed in; what it spent is not known, and is recorded
//! as nothing.
//!
//! The program is given an environment of its own, not the engine's (`cli::environment`). What a round is told depends
//! on whether it begins its line's session, and is built again where the program is tried the other way.
//!
//! What each round was told and what the program printed are kept for reading, in `rounds/<work>/<run>-<round>` in
//! the data directory: `.message.md` and `.result.json` for its first try, `.message.2.md` and `.result.2.json` for
//! the next, and so on. While the program runs, its process group is written in `.pgid` there; a group left named
//! there by a process of the engine that ended is stopped before the round is run again, or before the round's stop
//! ends it, so that a session has one program at a time and a stopped round's program does not run on.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use decide::RunId;
use decide::run::Failure;
use engine::Came;
use service::{Round, RoundReply, Rounds, Told};
use store::WorkNumber;
use tokio::process::Command;
use tokio::sync::Semaphore;
use uuid::Uuid;
use work::Spent;

use crate::cli::{self, Begin};
use crate::held::{Failing, Held, Stop};
use crate::permissions::Permissions;
use crate::process::{self, Ran};
use crate::settings::Settings;
use crate::{acts, failure, git, kept, message, returns, session};

/// The system prompt every round is given, before the owner's language and what the tool `check` does are put in.
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
    /// The system prompt, the same for every round of a line once what the line may do is put in.
    protocol: String,
    slots: Semaphore,
    held: Held<(WorkNumber, RunId, usize), RoundReply>,
}

/// What came of running a round's program.
enum Carried {
    Came(Came, Spent),
    /// It failed, for this reason, of this kind of cause, having taken this, as the result it printed says, or
    /// nothing where it printed none.
    Failed(String, Failure, Spent),
    Stopped,
}

impl Agents {
    /// The agents of these settings, with this data directory, which is an absolute path, and the installation's id.
    pub fn new(settings: Settings, data: PathBuf, installation: Uuid) -> Self {
        let protocol = (PROTOCOL.replace("{owner_language}", &settings.agents.owner_language))
            .replace("{check}", crate::tools::told(&settings));
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
                Ok(Carried::Came(came, spent)) => {
                    let Some(reply) = inner.held.take(key) else {
                        return;
                    };
                    match inner.over_budget(&round, &spent) {
                        Some(why) => reply.ended_and_paused(came, spent, why),
                        None => reply.ended(came, spent),
                    }
                }
                Ok(Carried::Stopped) => {
                    if let Some(reply) = inner.held.take(key) {
                        reply.ended(Came::HandsIn(Vec::new()), Spent::default());
                    }
                }
                Ok(Carried::Failed(why, failure, spent)) => match inner.held.fail(key) {
                    // Where the round took the work over its budget, the work is paused for that whatever the kind.
                    Some(Failing::Fail(reply)) => {
                        let resets = resets(failure, &why);
                        match inner.over_budget(&round, &spent) {
                            Some(over) => {
                                let pause = format!("{why}; and {over}");
                                reply.failed_and_paused(failure, resets, why, spent, pause);
                            }
                            None => reply.failed_as(failure, resets, why, spent),
                        }
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

    fn told(&self, work: WorkNumber, run: RunId, round: usize) -> Result<Told, String> {
        kept::told(&self.inner.data, work, run, round)
    }
}

impl Inner {
    /// Runs the round's program, and gives what came back and what it took; or that the round failed, why, of what
    /// kind and what it took; or, where it could not be run for a cause before its program, only why.
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
        let (args, added) = (&self.settings.agents.args, &self.settings.agents.permissions);
        let permissions = Permissions::of(round.cell.reads_only, args, added);
        let session = session::session(self.installation, round.work, line, cli::anew(flow, work, line));
        let mut begin = cli::begin(flow, work, line);
        let before = cli::spent_before(flow, work, line);
        let mut tried = false;
        let (status, stdout, stderr) = loop {
            let begins = begin == Begin::New;
            let told = message::message(flow, work, &round.cell, round.run, round.number, &round.given, begins);
            let (told_at, result_at) = attempt(&base).await;
            write(&told_at, told.as_bytes()).await?;
            let command = self.command(&dir, &permissions, session, begin);
            // A program that cannot be started cannot run, however the system words it.
            let ran = match process::run(command, Some(told.into_bytes()), Some(&group), stop).await {
                Ok(ran) => ran,
                Err(why) => return Ok(Carried::Failed(why, Failure::CannotRun, Spent::default())),
            };
            let Ran::Ended { status, stdout, stderr } = ran else {
                return Ok(Carried::Stopped);
            };
            write(&result_at, &stdout).await?;
            let stderr = String::from_utf8_lossy(&stderr).into_owned();
            // A session begun before its first round was cut off, or one the program no longer has, is tried the
            // other way, once.
            if !status.success() && stdout.is_empty() && !tried && begin.refused(&stderr) {
                begin = begin.other();
                tried = true;
                continue;
            }
            break (status, stdout, stderr);
        };
        let read = cli::read(&stdout, begin, before);
        // A failure whose result was read took what the result says; one with none, what is not known. Its kind is
        // told by what the program reported.
        let failed = |why: String, spent: Option<Spent>| {
            let kind = failure::kind(&why);
            Ok(Carried::Failed(why, kind, spent.unwrap_or_default()))
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
            Ok(came) => Ok(Carried::Came(came, read.spent)),
            Err(why) => Ok(Carried::Failed(why, Failure::Unread, read.spent)),
        }
    }

    /// The program's command line for a round of a line with these permissions, in the session, begun or gone on in,
    /// in the worktree. The system prompt says what the line may do.
    fn command(&self, dir: &Path, permissions: &Permissions, session: Uuid, begin: Begin) -> Command {
        let agents = &self.settings.agents;
        let protocol = self.protocol.replace("{your_line}", &permissions.told());
        let mut command = Command::new(&agents.command);
        command
            .args(cli::arguments(
                agents,
                &protocol,
                returns::SCHEMA,
                permissions,
                session,
                begin,
            ))
            .env_clear()
            .envs(cli::environment(agents, std::env::vars()))
            .current_dir(dir);
        command
    }

    /// Where what the round was told and returned are kept, without the extension.
    fn kept(&self, round: &Round) -> PathBuf {
        kept::base(&self.data, round.work, round.run, round.number)
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

/// When the account's usage limit resets, where the failure is that limit and the program's report says when.
fn resets(failure: Failure, why: &str) -> Option<work::Time> {
    if failure != Failure::UsageLimit {
        return None;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    let now = u64::try_from(now.as_millis()).ok()?;
    failure::resets_at(why, now, &failure::zone_offset).map(work::Time)
}

/// Where the next try of the round keeps what it is told and what it returns: the first that has kept nothing yet.
async fn attempt(base: &Path) -> (PathBuf, PathBuf) {
    let mut n = 1;
    while tokio::fs::try_exists(&kept::at(base, n).0).await.unwrap_or(false) {
        n += 1;
    }
    kept::at(base, n)
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
