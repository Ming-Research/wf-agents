//! The agents' rounds, run with the `claude` command line: each round one run of the program, in the work's worktree,
//! in the session of the cell's line, told its message on its standard input, with the same system prompt and the
//! same form of return for every round. At most as many rounds run at once as the settings give slots; the others
//! wait their turn.
//!
//! A round fails, handing in nothing, where the worktree is not there, the program cannot be started, it exits with
//! an error or prints a result that is one, or what it prints is no result with a structured return. It is not tried
//! again here: the work is paused for the owner to see why. Stopped, the program's process group is sent TERM, and
//! KILL after three seconds, and the round ends with nothing handed in and nothing spent.
//!
//! What each round was told and what the program printed are kept for reading, in `rounds/<work>/<run>-<round>` in
//! the data directory, `.message.md` and `.result.json`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use decide::RunId;
use engine::Came;
use service::{Round, RoundReply, Rounds};
use store::WorkNumber;
use tokio::process::Command;
use tokio::sync::Semaphore;
use uuid::Uuid;
use work::Spent;

use crate::cli::{self, Begin};
use crate::held::{Held, Stop};
use crate::process::{self, Ran};
use crate::settings::Settings;
use crate::{git, message, returns, session};

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
    Came(Came, Spent),
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
                Err(why) => {
                    if let Some(reply) = inner.held.forget(key) {
                        reply.failed(why);
                    }
                }
            }
        });
    }

    fn stop(&self, round: Round, reply: RoundReply) {
        let key = (round.work, round.run, round.number);
        if let Some(reply) = self.inner.held.stop(key, reply) {
            reply.ended(Came::HandsIn(Vec::new()), Spent::default());
        }
    }
}

impl Inner {
    /// Runs the round's program, and gives what came back and what it took; or why the round failed.
    async fn carry(&self, round: &Round, stop: &mut Stop) -> Result<Carried, String> {
        let (flow, work) = (&*round.flow, &*round.kept);
        let line = (round.cell.line.as_deref()).ok_or_else(|| format!("the cell {} names no line", round.cell.id))?;
        let dir = git::worktree(&self.data, round.work)?;
        let told = message::message(flow, work, &round.cell, round.run, round.number, &round.given);
        let kept = self.kept(round);
        write(&kept.with_extension("message.md"), told.as_bytes()).await?;
        let session = session::session(self.installation, round.work, line);
        let mut begin = cli::begin(flow, work, line);
        let before = cli::spent_before(flow, work, line);
        let mut tried = false;
        let (status, stdout, stderr) = loop {
            let ran = process::run(
                self.command(&dir, session, begin),
                Some(told.clone().into_bytes()),
                stop,
            )
            .await?;
            let Ran::Ended { status, stdout, stderr } = ran else {
                return Ok(Carried::Stopped);
            };
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
        write(&kept.with_extension("result.json"), &stdout).await?;
        let read = cli::read(&stdout, begin, before);
        if !status.success() {
            let why = match read {
                Err(why) if !stdout.is_empty() => why,
                _ => process::tail(stderr.trim().as_bytes()),
            };
            return Err(format!("{} exited with {status}: {why}", self.settings.agents.command));
        }
        let read = read?;
        let came = returns::came(&read.structured)?;
        Ok(Carried::Came(came, read.spent))
    }

    /// The program's command line for a round in the session, begun or gone on in, in the worktree.
    fn command(&self, dir: &Path, session: Uuid, begin: Begin) -> Command {
        let agents = &self.settings.agents;
        let mut command = Command::new(&agents.command);
        command
            .args(cli::arguments(agents, &self.protocol, returns::SCHEMA, session, begin))
            .envs(cli::environment(agents))
            .current_dir(dir);
        command
    }

    /// Where what the round was told and returned are kept, without the extension.
    fn kept(&self, round: &Round) -> PathBuf {
        let dir = self.data.join(ROUNDS).join(round.work.0.to_string());
        dir.join(format!("{}-{}", round.run.0, round.number))
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
