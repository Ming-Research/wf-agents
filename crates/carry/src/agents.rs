//! The agents' rounds, run with the `claude` command line: each round one run of the program, in the work's worktree,
//! in the session of the cell's line, told its message on its standard input, with the form of return of its flow,
//! made of the flow and the files beside the flows and so the same for every round of every cell of the flow
//! ([`flow::form`]), and the same system prompt for every round of a line ([`system`](crate::system)), made from the
//! files beside the flows, which says what the line may run as its permissions give it to the program
//! (`permissions`). The system prompt is made as a session begins and kept with it, so that every round of the session
//! is given the one it began with, whatever the work's branch has changed since; a round whose system prompt cannot be
//! made, as for a file of the project's that is not in the worktree, fails, saying why. What came back is read against
//! the cell the round was entered for ([`returns`](crate::returns)). How many run at once is the engine's to bound: it
//! gives at most as many rounds at once as the settings give slots, and holds the others back in its own order.
//!
//! A round fails, handing in nothing, where the worktree is not there, the program cannot be started, or its stream
//! holds no return of the form: neither one the command line took, nor, where it gave up on the return, its last
//! attempt at one, rebuilt from the stream (`cli::Stream`); a return the stream holds is read however the program
//! exited. The kind of its cause is told apart from what the program reported ([`failure`](crate::failure)): a
//! program that cannot be started cannot run, a return not of the form could not be read, and a round that failed
//! before its program ran failed for a cause no kind tells apart. It is not tried again here: the engine handles it as
//! its kind says. A round that fails takes what the result the program printed says it cost, as any round does, and
//! is held to the budget as any round is, its work paused where it went over; where the program printed nothing that
//! reads as a result, what the round took is not known, and is recorded as nothing. Stopped, the program's process
//! group is sent TERM, and KILL after three seconds, and the round ends with nothing handed in; what it spent is not
//! known, and is recorded as nothing.
//!
//! The program is given an environment of its own, not the engine's (`cli::environment`). Which of its line's sessions
//! a round runs in, and whether it begins it, is counted from the work's records (`cli::in_session`); what it is told
//! depends on whether it begins the session, or goes on in one that never saw its run's earlier rounds, which is told
//! the same, and is built again where the program is tried the other way. A session
//! the program no longer has is begun again under its id, and that is said to the loop at once, so that it is
//! recorded with why. Where the program compacts the session during the round, a hook it is given marks the
//! compaction, which the round counts among what it took and says to the loop as the session reopened, and gives the
//! session the round's whole message again, as a session that begins is told it. After each use of a tool another hook gives the session what the owner said to the
//! round while it runs, where the runner put it in the round's files (`cli::Words`).
//!
//! Before the first round of each run of a cell that takes main in, the work's branch takes in main's head as the host
//! last gave it ([`catch_up`](crate::catch_up)), and what came of it is kept beside the run's rounds before the round
//! is told it; a round of the run given again, or another of its rounds given its task, is told what was kept, and
//! nothing is taken in again for the run. The branch's design tree is looked at after that, and after the round's
//! program ended ([`tree`]), where it may have changed.
//!
//! Each round runs on the model of its item's level, or of a level above it where its cell's line says so, and at the
//! effort its cell sets (`Settings::ran_on`); a round whose item's level the settings no longer allow fails, saying so.
//! What each try runs on is said to the loop as it begins, and recorded in the work's records.
//!
//! What each round was told and the program's stream are kept for reading, in `rounds/<work>/<run>-<round>` in the
//! data directory: `.message.md` and `.stream.jsonl` for its first try, `.message.2.md` and `.stream.2.jsonl` for the
//! next, and so on, the stream written as it comes; `.compacted` holds a byte for each compaction of the session
//! during the round, and `.again.md`, while the program runs, what the hook gives again; `.words.json` what the owner
//! said that waits for the hook, and `.delivered.1.json` and on what the hook gave. While the program runs, its
//! process group is written in `.pgid` there; a group left named there by a process of the engine that ended is
//! stopped before the round is run again, or before the round's stop ends it, so that a session has one program at a
//! time and a stopped round's program does not run on.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use decide::RunId;
use decide::run::Failure;
use driver::Policy;
use engine::Came;
use service::{Clock, Round, RoundReply, Rounds, SystemClock, Teller, Told};
use store::WorkNumber;
use tokio::process::Command;
use uuid::Uuid;
use work::{RanOn, Spent, Why};

use crate::catch_up::{self, CaughtUp};
use crate::cli::{self, Begin, Compacted, InSession, Words};
use crate::held::{Failing, Held, Stop};
use crate::permissions::Permissions;
use crate::process::{self, Ran};
use crate::settings::Settings;
use crate::{acts, failure, git, kept, message, returns, session, system, tree};

/// What is written beside the flows, which every flow shares: the words the agents are sent, and the definitions the
/// form of a return is made of.
type Beside = flow::agents::Agents;

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
    /// What is written beside the flows, which the form and the system prompts are made from.
    beside: Beside,
    /// What the engine puts into the texts its agents are sent: the settings they name, with their values, and the
    /// places it fills alike for every line.
    filling: system::Filling,
    held: Held<(WorkNumber, RunId, usize), RoundReply>,
    /// The clock a usage limit's reset is read by, where the command line gives it as a time of day.
    clock: Arc<dyn Clock>,
    /// The reference that holds main's head as the host last gave it, which a run that takes main in takes in.
    main: String,
}

/// What came of running a round's program.
enum Carried {
    Came(Came, Spent),
    /// It failed, for this reason, of this kind of cause, having taken this, as the result it printed says, or
    /// nothing where it printed none.
    Failed(String, Failure, Spent),
    Stopped,
}

/// What a session that was compacted during a round is told, before the round's whole message is given again.
const COMPACTED: &str = "Your session was compacted during this round. What the round was told, again, whole, as to a \
                         session that begins; carry on with the round's work.";

impl Agents {
    /// The agents of these settings, with this data directory, which is an absolute path, the installation's id, the
    /// reference of the project's repository that holds main's head as the host last gave it, and what is written
    /// beside the flows, which their system prompts are made from.
    pub fn new(settings: Settings, data: PathBuf, installation: Uuid, main: String, beside: Beside) -> Self {
        Self::with_clock(settings, data, installation, main, beside, Arc::new(SystemClock))
    }

    /// The agents as [`Agents::new`] gives them, reading the time by this clock.
    pub fn with_clock(
        settings: Settings,
        data: PathBuf,
        installation: Uuid,
        main: String,
        beside: Beside,
        clock: Arc<dyn Clock>,
    ) -> Self {
        let filling = system::filling(&settings, &beside);
        Agents {
            inner: Arc::new(Inner {
                settings,
                data,
                installation,
                beside,
                filling,
                held: Held::new(),
                clock,
                main,
            }),
        }
    }
}

impl Rounds for Agents {
    fn give(&self, round: Round, reply: RoundReply) {
        acts::prune(&self.inner.held, &round);
        let key = (round.work, round.run, round.number);
        let teller = reply.teller();
        let Some(mut stop) = self.inner.held.hold(key, reply) else {
            return;
        };
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let carried = inner.carry(&round, &teller, &mut stop).await;
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
                        reply.ended(Came::hands_in(Vec::new()), Spent::default());
                    }
                }
                Ok(Carried::Failed(why, failure, spent)) => match inner.held.fail(key) {
                    // Where the round took the work over its budget, the work is paused for that whatever the kind.
                    Some(Failing::Fail(reply)) => {
                        let resets = resets(failure, &why, &*inner.clock);
                        match inner.over_budget(&round, &spent) {
                            Some(over) => {
                                let pause = format!("{why}; and {over}");
                                reply.failed_and_paused(failure, resets, why, spent, pause);
                            }
                            None => reply.failed_as(failure, resets, why, spent),
                        }
                    }
                    Some(Failing::End(reply)) => reply.ended(Came::hands_in(Vec::new()), spent),
                    None => {}
                },
                // What a round that failed before its program printed a result took is not known: it is kept as
                // nothing.
                Err(why) => match inner.held.fail(key) {
                    Some(Failing::Fail(reply)) => reply.failed(why, Spent::default()),
                    Some(Failing::End(reply)) => reply.ended(Came::hands_in(Vec::new()), Spent::default()),
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
                reply.ended(Came::hands_in(Vec::new()), Spent::default());
            });
        }
    }

    fn told(&self, work: WorkNumber, run: RunId, round: usize) -> Result<Told, String> {
        kept::told(&self.inner.data, work, run, round)
    }

    fn policy(&self) -> Option<Policy> {
        Some(self.inner.settings.policy())
    }
}

impl Inner {
    /// Runs the round's program, and gives what came back and what it took; or that the round failed, why, of what
    /// kind and what it took; or, where it could not be run for a cause before its program, only why.
    async fn carry(&self, round: &Round, teller: &Teller, stop: &mut Stop) -> Result<Carried, String> {
        let (flow, work) = (&*round.flow, &*round.kept);
        let place = git::place(&self.data, &git::tag(&self.installation), round.work);
        let base = self.kept(round);
        let group = base.with_extension("pgid");
        // A program a process of the engine that ended left running for the round is stopped before anything that can
        // end the round here, so that a round that now fails, as for a level the settings no longer have, leaves no
        // program running unseen.
        process::stop_left(&group, &place.dir).await;
        let line = (round.cell.line.as_deref()).ok_or_else(|| format!("the cell {} names no line", round.cell.id))?;
        // The form of the flow, the same bytes for every round of every cell of it.
        let form = flow::form::form(flow, &self.beside)?;
        let on = self.settings.ran_on(round.item.level.as_deref(), &round.cell)?;
        let dir = git::worktree(&place)?;
        let main = if round.cell.takes_main {
            self.caught_up(round, &place).await?
        } else {
            None
        };
        // The branch may have a new design tree where it took main in.
        let tree_dir = &self.settings.project.tree;
        let mut looking = tree::Looking::of(flow, work, tree_dir);
        if let (Some(looking), Some(_)) = (&mut looking, &main) {
            looking.tell(&place, teller, true).await;
        }
        if let Some(parent) = base.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let (args, added) = (&self.settings.agents.args, &self.settings.agents.permissions);
        let permissions = Permissions::of(round.cell.reads_only, args, added);
        let InSession {
            number,
            mut begin,
            before,
            moved,
        } = cli::in_session(flow, work, line, round.run, round.number);
        let session = session::session(self.installation, round.work, line, number);
        // A compaction within the round is marked, and the round's whole message, as a session that begins is told
        // it, is given again.
        let compacted = Compacted {
            marks: base.with_extension("compacted"),
            again: base.with_extension("again.md"),
        };
        // What the owner says to the round while it runs waits beside it, for the hook after the next use of a tool.
        let words = Words {
            waiting: base.with_extension("words.json"),
            delivered: base.with_extension("delivered"),
        };
        let mut tried = false;
        // The session the program lost is begun again under its id, and that is recorded once for the round.
        let at = work::sessions::began_at(work, round.run, round.number);
        let recorded =
            (work::sessions::of_line(work, line)).any(|s| s.why == Why::Lost && at.is_some_and(|a| a < s.at));
        // The task as the message sends it, each setting it names with its value beside it.
        let task = system::task(&self.filling.named, &round.cell)?;
        let (status, stream, stderr, compactions) = loop {
            // A round whose run ran in another session before is told what a session that begins is told: the
            // session it goes on in never saw the run.
            let opening = message::Opening {
                begins: begin == Begin::New || moved,
                main: main.as_ref(),
                task: task.as_deref(),
            };
            let (cell, beside) = (&round.cell, &self.beside);
            let told = message::message(
                flow,
                work,
                cell,
                round.run,
                round.number,
                &round.given,
                &opening,
                beside,
            );
            let whole = message::Opening {
                begins: true,
                ..opening
            };
            let again = message::message(flow, work, cell, round.run, round.number, &round.given, &whole, beside);
            write(&compacted.again, format!("{COMPACTED}\n\n{again}").as_bytes()).await?;
            // The round takes what its last try took, so it counts that try's compactions alone: an earlier try's, or
            // a try's before the engine stopped, are not its.
            let _ = tokio::fs::remove_file(&compacted.marks).await;
            let (n, told_at, stream_at) = attempt(&base).await;
            // What the try runs on is said before its message is written, which makes the try one that was kept.
            teller.ran_on(n, on.clone());
            write(&told_at, told.as_bytes()).await?;
            // The system prompt is the one the session began with: made from the worktree as it begins, and kept.
            let made = || {
                let read = |path: &str| system::read_within(&dir, path);
                let your_line = permissions.told(&self.beside.your_line);
                system::of_line(&self.beside, &self.filling, flow, &round.cell, &your_line, read)
            };
            let kept_at = system::kept_at(&self.data, session);
            let system = system::of_session(&kept_at, made)?;
            let command = self.command(
                &dir,
                &on,
                &system,
                &permissions,
                &form,
                &compacted,
                &words,
                session,
                begin,
            );
            // The stream is kept as it comes, so that what the agent is doing can be read while it does it.
            let mut kept = Keeping::to(&stream_at)?;
            let mut each = |line: &[u8]| kept.line(line);
            // A program that cannot be started printed no result: its words say whether it is missing, which holds
            // every work, and any other cause pauses the work.
            let ran = process::streamed(command, Some(cli::stdin(&told)), Some(&group), stop, &mut each).await;
            // Each compaction of the try is a reopening of the session, told while the round is still under way,
            // however the try ended.
            let compactions = marked(&compacted.marks).await;
            for _ in 0..compactions {
                teller.session_began(line.to_string(), Why::Compacted);
            }
            let ran = match ran {
                Ok(ran) => ran,
                Err(why) => {
                    let kind = failure::of(&why, None);
                    return Ok(Carried::Failed(why, kind, Spent::default()));
                }
            };
            let Ran::Ended { status, stderr, .. } = ran else {
                return Ok(Carried::Stopped);
            };
            let stream = kept.done()?;
            let stderr = String::from_utf8_lossy(&stderr).into_owned();
            // A session begun before its first round was cut off, or one the program no longer has, is tried the
            // other way, once.
            if !status.success() && stream.is_empty() && !tried && begin.refused(&stderr) {
                if begin == Begin::Resume && !recorded {
                    teller.session_began(line.to_string(), Why::Lost);
                }
                begin = begin.other();
                tried = true;
                continue;
            }
            break (status, stream, stderr, compactions);
        };
        let _ = tokio::fs::remove_file(&compacted.again).await;
        // What the round committed may have changed the design tree.
        if let Some(looking) = &mut looking {
            looking.tell(&place, teller, false).await;
        }
        let had_result = stream.has_result();
        let read = stream.read(begin, before).map(|mut read| {
            read.spent.compactions = compactions;
            read
        });
        let read = read.map_err(|mut no| {
            if let Some(spent) = &mut no.spent {
                spent.compactions = compactions;
            }
            no
        });
        // A failure whose result was read took what the result says; one with none, what is not known. Its kind is
        // told by what the program reported and what it took. A return the stream holds is read however the program
        // exited: the one the command line took, or the last attempt of one it gave up on, after which it exits with
        // an error.
        let failed = |why: String, spent: Option<Spent>| {
            let kind = failure::of(&why, spent.as_ref());
            Ok(Carried::Failed(why, kind, spent.unwrap_or_default()))
        };
        let read = match read {
            Ok(read) => read,
            Err(no) if status.success() => return failed(no.why, no.spent),
            Err(no) => {
                let why = match had_result {
                    true => no.why,
                    false => process::tail(stderr.trim().as_bytes()),
                };
                let command = &self.settings.agents.command;
                return failed(format!("{command} exited with {status}: {why}"), no.spent);
            }
        };
        match returns::came(&read.structured, flow, &round.cell, &self.beside) {
            Ok(came) => Ok(Carried::Came(came, read.spent)),
            Err(why) => Ok(Carried::Failed(why, Failure::Unread, read.spent)),
        }
    }

    /// The program's command line for a round on what it runs on, with its line's system prompt, of a line with these
    /// permissions, given the form of return of its flow, in the session, begun or gone on in, in the worktree.
    #[allow(clippy::too_many_arguments)]
    fn command(
        &self,
        dir: &Path,
        on: &RanOn,
        system: &str,
        permissions: &Permissions,
        form: &str,
        compacted: &Compacted,
        words: &Words,
        session: Uuid,
        begin: Begin,
    ) -> Command {
        let agents = &self.settings.agents;
        let mut command = Command::new(&agents.command);
        let args = cli::arguments(agents, on, system, form, permissions, compacted, words, session, begin);
        command
            .args(args)
            .env_clear()
            .envs(cli::environment(agents, std::env::vars()))
            .current_dir(dir);
        command
    }

    /// What came of taking main in as the round's run started, as it was kept; taken in and kept first at the run's
    /// first round. None for a later round of a run that kept nothing, which started before its cell took main in.
    async fn caught_up(&self, round: &Round, place: &git::Place) -> Result<Option<CaughtUp>, String> {
        let at = catch_up::kept_at(&self.data, round.work, round.run);
        let named = || at.display().to_string();
        match tokio::fs::read(&at).await {
            Ok(kept) => {
                let caught = serde_json::from_slice(&kept).map_err(|e| format!("{}: {e}", named()))?;
                return Ok(Some(caught));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{}: {e}", named())),
        }
        if round.number != 1 {
            return Ok(None);
        }
        let caught = catch_up::take_in(&self.settings.project, place, &self.main, round.run).await;
        // The round is told only that main was not taken in; why is for whoever runs the engine.
        if let catch_up::Outcome::Failed { why } = &caught.outcome {
            eprintln!(
                "main could not be taken into the branch of work {}: {why}",
                round.work.0
            );
        }
        let kept = serde_json::to_vec_pretty(&caught).map_err(|e| format!("{}: {e}", named()))?;
        // Written whole or not at all, so that a stop in the middle leaves no file the run's rounds cannot read.
        let writing = at.with_extension("json.part");
        write(&writing, &kept).await?;
        tokio::fs::rename(&writing, &at)
            .await
            .map_err(|e| format!("{}: {e}", named()))?;
        Ok(Some(caught))
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

/// When the account's usage limit resets, where the failure is that limit and the program's report says when, the time
/// now read by the clock.
fn resets(failure: Failure, why: &str, clock: &dyn Clock) -> Option<work::Time> {
    if failure != Failure::UsageLimit {
        return None;
    }
    let now = clock.now().ok()?;
    failure::resets_at(why, now.0, &failure::zone_offset).map(work::Time)
}

/// How many compactions of the session the file of marks holds, a byte for each.
async fn marked(marks: &Path) -> u32 {
    match tokio::fs::metadata(marks).await {
        Ok(marks) => u32::try_from(marks.len()).unwrap_or(u32::MAX),
        Err(_) => 0,
    }
}

/// The number of the next try of the round, and where it keeps what it is told and its stream: the first that has
/// kept nothing yet.
async fn attempt(base: &Path) -> (usize, PathBuf, PathBuf) {
    let mut n = 1;
    while tokio::fs::try_exists(&kept::at(base, n).0).await.unwrap_or(false) {
        n += 1;
    }
    (n, kept::at(base, n).0, kept::stream_at(base, n))
}

/// The stream of a try as it comes: read, and each line kept that is kept with the round ([`cli::Stream::line`]),
/// written to its file as it comes; the first fault of the writing is kept, and said once the try has ended.
struct Keeping {
    stream: cli::Stream,
    file: std::fs::File,
    path: PathBuf,
    fault: Option<std::io::Error>,
}

impl Keeping {
    /// Keeping a stream in the file, which is made anew.
    fn to(path: &Path) -> Result<Keeping, String> {
        let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Keeping {
            stream: cli::Stream::default(),
            file,
            path: path.to_path_buf(),
            fault: None,
        })
    }

    /// Takes a line of the stream, without its newline.
    fn line(&mut self, line: &[u8]) {
        use std::io::Write;
        if self.stream.line(line) && self.fault.is_none() {
            let written = (self.file.write_all(line)).and_then(|()| self.file.write_all(b"\n"));
            self.fault = written.err();
        }
    }

    /// The stream as it was read; or why it could not be kept.
    fn done(self) -> Result<cli::Stream, String> {
        match self.fault {
            Some(e) => Err(format!("{}: {e}", self.path.display())),
            None => Ok(self.stream),
        }
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
