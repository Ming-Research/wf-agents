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
//! A round is run with the project of its work's item ([`crate::projects`]): its declaration in place of the one beside
//! the flows, the settings its texts name filled from it, and main's head as its host last gave it. A round of a work
//! whose item has no project the settings name fails, saying so.
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
//! a round runs in, and whether it begins it, is counted from the work's records (`cli::in_session`); what it is sent
//! is what the context manager fills into that session (crate `context`): what the session lacks, or, where the
//! program begins the session under its id, everything from the top, and what each try's message fills is said to the
//! loop before the program is given it, so that it is recorded. A session the program no longer has is begun again
//! under its id, and that is said to the loop at once, so that it is recorded with why. Where the program compacts the
//! session during the round, a hook it is given marks the compaction, which the round counts among what it took and
//! says to the loop as the session reopened, and gives the session the round's whole message again, as a session that
//! holds nothing is sent it. After each use of a tool another hook gives the session what the owner said to the
//! round while it runs, where the runner put it in the round's files (`cli::Words`).
//!
//! As a round of a line that may change the work ends with a return, the engine looks at what the work's branch changes
//! under the paths the project protects, and asks the owner by itself where it is a change not asked of yet, beside
//! the return ([`protected`](crate::protected)).
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

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use decide::run::Failure;
use decide::{Pauser, RunId};
use driver::Policy;
use engine::Came;
use service::{Clock, Doing, Ping, PingReply, Round, RoundReply, Rounds, SystemClock, Teller, Told};
use store::WorkNumber;
use tokio::process::Command;
use tokio::sync::oneshot;
use uuid::Uuid;
use work::{Back, Measures, RanOn, Spent, Why, Work};

use crate::catch_up::{self, CaughtUp};
use crate::cli::{self, Begin, Calls, Compacted, InSession, Words};
use crate::guard::{Crossing, Guard};
use crate::held::{Failing, Held, Stop};
use crate::permissions::Permissions;
use crate::process::{self, Ran};
use crate::settings::{Project, Settings};
use crate::{acts, failure, git, kept, protected, returns, session, system, tools, tree};

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
    /// Each project's, by its name, for the rounds of the works of its items.
    projects: BTreeMap<String, Of>,
    held: Held<(WorkNumber, RunId, usize), RoundReply>,
    /// The clock a usage limit's reset is read by, where the command line gives it as a time of day.
    clock: Arc<dyn Clock>,
    /// A lock for each work, so that a keep-warm ping of a work never runs beside a round of it: a round waits for a
    /// ping of its work to end, and a ping that finds a round of its work under way is given up.
    in_work: std::sync::Mutex<BTreeMap<WorkNumber, Arc<tokio::sync::Mutex<()>>>>,
}

/// What the rounds of the works of a project's items are run with.
pub struct Of {
    /// The project's settings.
    project: Project,
    /// What is written beside the flows, with the project's declaration in place of the one beside them, which the
    /// form and the system prompts are made from.
    beside: Beside,
    /// What the engine puts into the texts its agents are sent: the settings they name, with their values, and the
    /// places it fills alike for every line.
    filling: system::Filling,
    /// The reference that holds the head of the project's main as its host last gave it, which a run that takes main
    /// in takes in.
    main: String,
}

impl Of {
    /// What the rounds of a project's works are run with, of these settings: the project's own, what is written beside
    /// the flows with its declaration, and the reference of its repository that holds main's head as its host last
    /// gave it.
    pub fn new(settings: &Settings, project: Project, beside: Beside, main: String) -> Of {
        let filling = system::filling(settings, &beside);
        Of {
            project,
            beside,
            filling,
            main,
        }
    }
}

/// What came of running a round's program.
enum Carried {
    Came(Came, Spent),
    /// It failed, for this reason, of this kind of cause, with what its stream said of the service's errors, having
    /// taken this, as the result it printed says, or nothing where it printed none.
    Failed(String, Failure, failure::Said, Spent),
    Stopped,
}

impl Agents {
    /// The agents of these settings, with this data directory, which is an absolute path, the installation's id, and
    /// what the rounds of each project's works are run with, by the project's name.
    pub fn new(settings: Settings, data: PathBuf, installation: Uuid, projects: BTreeMap<String, Of>) -> Self {
        Self::with_clock(settings, data, installation, projects, Arc::new(SystemClock))
    }

    /// The agents as [`Agents::new`] gives them, reading the time by this clock.
    pub fn with_clock(
        settings: Settings,
        data: PathBuf,
        installation: Uuid,
        projects: BTreeMap<String, Of>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Agents {
            inner: Arc::new(Inner {
                settings,
                data,
                installation,
                projects,
                held: Held::new(),
                clock,
                in_work: std::sync::Mutex::new(BTreeMap::new()),
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
                    match inner.to_wait(&round, &spent) {
                        Some(why) => reply.ended_and_paused(came, spent, why),
                        None => reply.ended(came, spent),
                    }
                }
                Ok(Carried::Stopped) => {
                    if let Some(reply) = inner.held.take(key) {
                        reply.ended(Came::hands_in(Vec::new()), Spent::default());
                    }
                }
                Ok(Carried::Failed(why, failure, said, spent)) => match inner.held.fail(key) {
                    // Where the round took the work over its budget, the work is paused for that whatever the kind.
                    Some(Failing::Fail(reply)) => {
                        let resets = resets(failure, &why, &said, &*inner.clock);
                        match inner.to_wait(&round, &spent) {
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

    fn system(&self, work: WorkNumber, line: &str, number: u32) -> Result<Option<String>, String> {
        let session = session::session(self.inner.installation, work, line, number);
        let at = system::kept_at(&self.inner.data, session);
        match std::fs::read_to_string(&at) {
            Ok(system) => Ok(Some(system)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!(
                "{}: {e}",
                at.strip_prefix(&self.inner.data).unwrap_or(&at).display()
            )),
        }
    }

    fn doing(&self, work: WorkNumber, run: RunId, round: usize) -> Result<Vec<Doing>, String> {
        kept::doing(&self.inner.data, work, run, round)
    }

    fn policy(&self) -> Option<Policy> {
        Some(self.inner.settings.policy())
    }

    fn ping(&self, ping: Ping, reply: PingReply) {
        let inner = self.inner.clone();
        tokio::spawn(async move {
            match inner.ping(&ping).await {
                Ok(spent) => reply.ended(spent),
                Err((why, spent)) => reply.failed(why, spent),
            }
        });
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
        // The project of the work's item, whose repository and declaration the round is run with.
        let of = crate::projects::of(&self.projects, &round.item)?;
        // The form of the flow, the same bytes for every round of every cell of it.
        let form = flow::form::form(flow, &of.beside)?;
        let on = self.settings.ran_on(round.item.level.as_deref(), &round.cell)?;
        let dir = git::worktree(&place)?;
        let main = if round.cell.takes_main {
            self.caught_up(round, &place, of).await?
        } else {
            None
        };
        // The branch may have a new design tree where it took main in.
        let tree_dir = &of.project.tree;
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
        // One program at a time in a work: a keep-warm ping of it under way ends first, so that what its session holds
        // and cost is worked out with the ping in.
        let lock = self.lock(round.work);
        let _one = lock.lock().await;
        let InSession {
            number,
            mut begin,
            before,
        } = cli::in_session(flow, work, line, round.run, round.number);
        let session = session::session(self.installation, round.work, line, number);
        // What the round is judged by while it runs: the prompt its session last had, and whether the owner let it run
        // past the guard.
        let last = driver::history::last_prompt(flow, work, line, round.run, round.number);
        let returned = driver::history::last_return(flow, work, line, round.run, round.number).map_or(0, chars_of);
        let waived = let_run(work, round.run, round.number, Failure::Guarded);
        // The guard stopped the run's round before this one, which this one is tried again for.
        let guarded_before = stopped_before(work, round.run, round.number, Failure::Guarded);
        // What the round is sent in the session as it stands, and in one that holds nothing: one begun under the round,
        // or compacted during it.
        let about = context::For {
            work: round.work.0,
            item: round.item.number.0,
            title: &round.item.title,
        };
        // What changed in the design tree, which git compares: the files, from each earlier version to the latest.
        let changes = tree::changes(flow, work, round.run, round.number, &dir, tree_dir).await?;
        // The task as the message sends it, each setting it names with its value beside it; and a result of the tool
        // `query` longer than its file says a message gives, cut as the tool cuts it, the records holding it whole.
        let task = system::task(&of.filling.named, &round.cell)?;
        let results = tools::given(work, &of.beside);
        let told = main.as_ref().and_then(CaughtUp::told);
        let carried = context::Carried {
            about,
            main: told.as_ref(),
            changes: &changes,
            task: task.as_deref(),
            results: &results,
        };
        let enter = |anew| context::enter(flow, &of.beside, work, round.run, round.number, &carried, anew);
        let (going_on, anew) = (enter(false)?, enter(true)?);
        // A compaction within the round is marked, and the round's whole message, as a session that holds nothing is
        // sent it, is given again.
        let compacted = Compacted {
            marks: base.with_extension("compacted"),
            again: base.with_extension("again.md"),
        };
        let again = format!("{}\n\n{}", of.beside.message.compacted, anew.text);
        write(&compacted.again, again.as_bytes()).await?;
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
        let (status, stream, stderr, compactions) = loop {
            // A session begun under its id holds nothing: it is sent everything from the top.
            let entered = if begin == Begin::New { &anew } else { &going_on };
            // The round takes what its last try took, so it counts that try's compactions alone: an earlier try's, or
            // a try's before the engine stopped, are not its.
            let _ = tokio::fs::remove_file(&compacted.marks).await;
            let (n, told_at, stream_at) = attempt(&base).await;
            // What the try runs on is said before its message is written, which makes the try one that was kept, and
            // what its message fills, before the program is given it.
            teller.ran_on(n, on.clone());
            teller.filled(n, entered.fills.clone());
            let told = &entered.text;
            write(&told_at, told.as_bytes()).await?;
            // The system prompt is the one the session began with: made from the worktree as it begins, and kept.
            let made = || {
                let read = |path: &str| system::read_within(&dir, path);
                let your_line = permissions.told(&of.beside.your_line);
                system::of_line(&of.beside, &of.filling, flow, &round.cell, &your_line, read)
            };
            let kept_at = system::kept_at(&self.data, session);
            let system = system::of_session(&kept_at, made)?;
            let before = self.before_messages(&on, &system, &form, &permissions);
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
            // The stream is kept as it comes, so that what the agent is doing can be read while it does it; and the run
            // guard judges each call as the stream tells it, halting the program at a signal past its lines.
            let mut kept = Keeping::to(&stream_at)?;
            // What the engine gave the round in its session: its message, and the return before it, which the session
            // has held only since its last call, so that a short message after a long return is no jump.
            let given = told.chars().count() as u64 + returned;
            let mut guard = Guard::new(round.lines.clone(), last, given, waived);
            let (halt, halted) = oneshot::channel();
            let mut halt = Some(halt);
            let mut crossed: Option<Crossing> = None;
            let mut told_answered = false;
            let ran = {
                let mut each = |line: &[u8]| {
                    kept.line(line);
                    // The service serves the round: a wait every work shares need not wait for its end.
                    if !told_answered && kept.stream.answered() {
                        told_answered = true;
                        teller.answered();
                    }
                    if crossed.is_none()
                        && let Some(crossing) = guard.judge(kept.calls())
                    {
                        crossed = Some(crossing);
                        if guard.stops()
                            && let Some(halt) = halt.take()
                        {
                            let _ = halt.send(());
                        }
                    }
                };
                // A program that cannot be started printed no result: its words say whether it is missing, which holds
                // every work, and any other cause is tried again as a failure in passing.
                let stdin = Some(cli::stdin(told));
                process::streamed(command, stdin, Some(&group), stop, Some(halted), &mut each).await
            };
            // Each compaction of the try is a reopening of the session, told while the round is still under way,
            // however the try ended.
            let compactions = marked(&compacted.marks).await;
            for _ in 0..compactions {
                teller.session_began(line.to_string(), Why::Compacted);
            }
            // A round the guard stopped printed no result: what it took is its calls at their models' prices. So is
            // what a try took whose program ended of itself with no result to say it, as one that crashed.
            let halted = crossed.is_some() && guard.stops();
            let ended = matches!(&ran, Ok(Ran::Ended { .. }));
            let priced = halted || (ended && !kept.stream.has_result() && kept.stream.priced().is_some());
            // What the calls of a try whose program ran came to is told before the round ends, however it ended.
            if let Some(measures) = kept.measures(given, before, crossed.is_some(), priced) {
                teller.measured(n, measures);
            }
            let ran = match ran {
                Ok(ran) => ran,
                Err(why) => {
                    let said = failure::Said::default();
                    let kind = failure::of(&why, &said, None);
                    return Ok(Carried::Failed(why, kind, said, Spent::default()));
                }
            };
            let Ran::Ended { status, stderr, .. } = ran else {
                // Stopped by the guard, the round failed for it, having taken what its calls cost.
                if let Some(crossing) = crossed.filter(|_| halted) {
                    let why = crossing.words(&of.beside.cost.guard, kept.tools(), guarded_before);
                    let spent = Spent {
                        compactions,
                        ..kept.stream.priced().unwrap_or_default()
                    };
                    return Ok(Carried::Failed(why, Failure::Guarded, failure::Said::default(), spent));
                }
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
        // told by what the program's stream said of the service's errors, what it reported and what it took. A return
        // the stream holds is read however the program exited: the one the command line took, or the last attempt of
        // one it gave up on, after which it exits with an error.
        let failed = |why: String, said: failure::Said, spent: Option<Spent>| {
            let kind = failure::of(&why, &said, spent.as_ref());
            Ok(Carried::Failed(why, kind, said, spent.unwrap_or_default()))
        };
        let read = match read {
            Ok(read) => read,
            Err(no) if status.success() => return failed(no.why, no.said, no.spent),
            Err(no) => {
                let why = match had_result {
                    true => no.why,
                    false => process::tail(stderr.trim().as_bytes()),
                };
                let command = &self.settings.agents.command;
                return failed(format!("{command} exited with {status}: {why}"), no.said, no.spent);
            }
        };
        match returns::came(&read.structured, flow, &round.cell, &of.beside) {
            Ok(mut came) => {
                came.engine.extend(self.protected(round, &place, of).await);
                Ok(Carried::Came(came, read.spent))
            }
            Err(why) => Ok(Carried::Failed(
                why,
                Failure::Unread,
                failure::Said::default(),
                read.spent,
            )),
        }
    }

    /// What the engine asks the owner of the work's change under the paths the project protects, as the round ends,
    /// where its line may change the work ([`protected`]): the early notice of what the check before the merge holds
    /// the work to. A look that fails is said on the standard error and asks nothing, the check being what holds the
    /// merge.
    async fn protected(&self, round: &Round, place: &git::Place, of: &Of) -> Option<work::Question> {
        if round.cell.reads_only {
            return None;
        }
        let (paths, words) = (&of.beside.project.protected.paths, &of.beside.asks.protected);
        match protected::look(&of.project, place, &of.main, paths, words, &round.kept).await {
            Ok(question) => question,
            Err(why) => {
                eprintln!(
                    "work {}: the change under the protected paths could not be looked at: {why}",
                    round.work.0
                );
                None
            }
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
    async fn caught_up(&self, round: &Round, place: &git::Place, of: &Of) -> Result<Option<CaughtUp>, String> {
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
        let caught = catch_up::take_in(&of.project, place, &of.main, round.run).await;
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

    /// Why the work is to wait for the owner as the round ends, where it does: the round took it over its budget, or
    /// its spend or time since it last waited for the owner is past what the flow's other works explain; both where
    /// both hold.
    fn to_wait(&self, round: &Round, spent: &Spent) -> Option<String> {
        let reasons: Vec<String> = [self.over_budget(round, spent)]
            .into_iter()
            .chain(self.past_history(round, spent))
            .flatten()
            .collect();
        // Two reasons are joined in the project's words, which the second, of the history, is said in.
        let between = (crate::projects::of(&self.projects, &round.item)).map(|of| of.beside.cost.work.between.clone());
        (!reasons.is_empty()).then(|| reasons.join(&between.unwrap_or_default()))
    }

    /// Why the work is to wait for the owner, where what it has spent, or the time its rounds have taken, since it
    /// started or was last let on after the engine paused it, this round with them, is past the line the flow's other
    /// works give (principle `judged-by-own-history`): each where it is.
    fn past_history(&self, round: &Round, spent: &Spent) -> Vec<Option<String>> {
        let work = &round.kept;
        let since = stretch(work);
        let ended = |at: decide::Seq| at > since;
        let mut usd = spent.usd_micros;
        let mut ms = 0u64;
        let mut counted: Vec<RunId> = Vec::new();
        for r in &work.records.rounds {
            counted.push(r.run);
            let k = counted.iter().filter(|x| **x == r.run).count();
            let end = (work.records.returned.iter()).filter(|e| e.run == r.run).nth(k - 1);
            let Some(end) = end.filter(|end| ended(end.at)) else {
                continue;
            };
            usd += work.spent.get(&(r.run, k)).map_or(0, |s| s.usd_micros);
            if let (Some(from), Some(to)) = (work.time_of(r.at), work.time_of(end.at)) {
                ms += to.0.saturating_sub(from.0);
            }
        }
        usd += (work.pinged.iter())
            .filter(|p| ended(p.at))
            .map(|p| p.spent.usd_micros)
            .sum::<u64>();
        // The round under way, from its beginning to now.
        let began = work::sessions::began_at(work, round.run, round.number).and_then(|at| work.time_of(at));
        if let (Some(began), Ok(now)) = (began, self.clock.now()) {
            ms += now.0.saturating_sub(began.0);
        }
        // The words are the project's; a work whose project the settings no longer have says nothing of its history.
        let Ok(of) = crate::projects::of(&self.projects, &round.item) else {
            return Vec::new();
        };
        let words = &of.beside.cost.work;
        let since_words = match since {
            decide::Seq(0) => &words.started,
            _ => &words.let_on,
        };
        let told = |said: &str, value: f64, line: &driver::history::Line, unit: Unit| {
            flow::agents::fill(
                said,
                &[
                    ("work", &round.work.0.to_string()),
                    ("value", &unit.say(value)),
                    ("line", &unit.say(line.value)),
                    ("works", &line.samples.to_string()),
                    ("since", since_words),
                ],
            )
        };
        let lines = &round.lines;
        let spent_past = (lines.work_spent.as_ref())
            .filter(|line| usd as f64 > line.value)
            .map(|line| told(&words.spent, usd as f64, line, Unit::Dollars));
        let time_past = (lines.work_time.as_ref())
            .filter(|line| ms as f64 > line.value)
            .map(|line| told(&words.time, ms as f64, line, Unit::Minutes));
        vec![spent_past, time_past]
    }

    /// Why the work is to wait for the owner, where the round took it over its budget: everything its rounds have
    /// spent, this one with them, is more than the settings let a work spend.
    fn over_budget(&self, round: &Round, spent: &Spent) -> Option<String> {
        let budget = self.settings.budget_micros()?;
        // No other round of the work ended while this one was under way: a work has one round under way at a time.
        let pinged = round.kept.pinged.iter().map(|p| p.spent.usd_micros).sum::<u64>();
        let total = round.kept.spent.values().map(|s| s.usd_micros).sum::<u64>() + spent.usd_micros + pinged;
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

/// How many characters a round came back with, as its return writes them: what it dealt with, handed in, asked for and
/// said; nothing for a round that failed.
fn chars_of(back: &Back) -> u64 {
    let written = match back {
        Back::Returned {
            dealt_with,
            handed,
            asks,
            said,
            ..
        } => serde_json::to_string(&(dealt_with, handed, asks, said)),
        Back::HandedIn(handed) => serde_json::to_string(handed),
        Back::Asked(asks) => serde_json::to_string(asks),
        Back::Failed { .. } => return 0,
    };
    written.map_or(0, |text| text.chars().count() as u64)
}

impl Inner {
    /// The lock of the work, made where it has none.
    fn lock(&self, work: WorkNumber) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.in_work.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        locks.entry(work).or_default().clone()
    }

    /// What comes before a session's messages in the prompt of a call on this, with this system prompt, form of return
    /// and permissions, which the cache reads only where it is what the call before had: the model, the effort, and a
    /// digest each of the system prompt, the schema and the tools the program is given ([`work::Measures::before`]).
    fn before_messages(
        &self,
        on: &RanOn,
        system: &str,
        form: &str,
        permissions: &Permissions,
    ) -> BTreeMap<String, String> {
        let tools: Vec<String> = (self.settings.agents.args.iter().cloned())
            .chain(permissions.arguments())
            .collect();
        let mut before = BTreeMap::from([
            ("model".to_string(), on.model.clone()),
            ("system".to_string(), work::digest(system)),
            ("schema".to_string(), work::digest(form)),
            ("tools".to_string(), work::digest(&tools.join("\n"))),
        ]);
        if let Some(effort) = &on.effort {
            before.insert("effort".to_string(), effort.clone());
        }
        before
    }

    /// Sends the session of the ping its keep-warm message, with the command line of the session's last round, which
    /// the cache keeps the session under, and gives what it took; or why it failed, with what it took. A work a round
    /// of which is under way is not pinged, and a ping that has not ended by the time the cache lets the session go is
    /// stopped, since after that it would write the session again for nothing.
    async fn ping(&self, ping: &Ping) -> Result<Spent, (String, Spent)> {
        let failed = |why: String| (why, Spent::default());
        let of = crate::projects::of(&self.projects, &ping.item).map_err(failed)?;
        let said = &of.beside.cost.ping;
        let fill = flow::agents::fill;
        let (flow, work, line) = (&*ping.flow, &*ping.kept, ping.line.as_str());
        let lock = self.lock(ping.work);
        let Ok(_one) = lock.try_lock() else {
            return Err(failed(said.under_way.clone()));
        };
        let session = session::session(self.installation, ping.work, line, ping.session);
        // The session's last round that ran, whose command line it is sent with.
        let last = (work.ran.iter().rev()).find(|r| {
            let cell = work.records.started.iter().find(|s| s.run == r.run).map(|s| &s.cell);
            let spec = cell.and_then(|cell| flow.cells.iter().find(|c| c.id == *cell));
            spec.and_then(|c| c.line.as_deref()) == Some(line)
        });
        let no_round = || failed(fill(&said.no_round, &[("line", line)]));
        let (run, number, on) = last.map(|r| (r.run, r.round, r.on.clone())).ok_or_else(no_round)?;
        let started = (work.records.started.iter())
            .find(|s| s.run == run)
            .ok_or_else(no_round)?;
        let cell = (flow.cells.iter())
            .find(|c| c.id == started.cell)
            .ok_or_else(no_round)?;
        let place = git::place(&self.data, &git::tag(&self.installation), ping.work);
        let dir = git::worktree(&place).map_err(failed)?;
        let system = std::fs::read_to_string(system::kept_at(&self.data, session))
            .map_err(|e| failed(fill(&said.no_system, &[("why", &e.to_string())])))?;
        let form = flow::form::form(flow, &of.beside).map_err(failed)?;
        let (args, added) = (&self.settings.agents.args, &self.settings.agents.permissions);
        let permissions = Permissions::of(cell.reads_only, args, added);
        let base =
            kept::base(&self.data, ping.work, run, number).with_file_name(format!("ping-{line}-{}", ping.session));
        let compacted = Compacted {
            marks: base.with_extension("compacted"),
            again: base.with_extension("again.md"),
        };
        let words = Words {
            waiting: base.with_extension("words.json"),
            delivered: base.with_extension("delivered"),
        };
        let _ = tokio::fs::remove_file(&compacted.marks).await;
        // A ping's compaction gives the session nothing again: its next round is filled from the top.
        write(&compacted.again, b"").await.map_err(failed)?;
        let command = self.command(
            &dir,
            &on,
            &system,
            &permissions,
            &form,
            &compacted,
            &words,
            session,
            Begin::Resume,
        );
        // What the session cost before the ping: its rounds' and its earlier pings'.
        let before = cli::in_session(flow, work, line, run, number + 1).before;
        let message = cli::stdin(&said.message);
        // The ping is given what is left of the cache's life after it was due.
        let left = (self.settings.cache_ms())
            .map(|life| life.saturating_sub(self.settings.sessions.warm_minutes.saturating_mul(60_000)))
            .unwrap_or(0);
        let (halt, halted) = oneshot::channel();
        let timer = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(left)).await;
            let _ = halt.send(());
        });
        let mut stream = cli::Stream::default();
        let mut lines = Vec::new();
        let mut each = |line: &[u8]| {
            stream.line(line);
            lines.extend_from_slice(line);
            lines.push(b'\n');
        };
        let mut never = crate::held::Stop::never();
        let ran = process::streamed(command, Some(message), None, &mut never, Some(halted), &mut each).await;
        timer.abort();
        let marks = marked(&compacted.marks).await;
        let _ = tokio::fs::remove_file(&compacted.marks).await;
        let _ = tokio::fs::remove_file(&compacted.again).await;
        let priced = || Spent {
            compactions: marks,
            ..stream.priced().unwrap_or_default()
        };
        let Ran::Ended { status, stderr, .. } = ran.map_err(failed)? else {
            let minutes = (left / 60_000).to_string();
            return Err((fill(&said.stopped, &[("minutes", &minutes)]), priced()));
        };
        let spent = match cli::read(&lines, Begin::Resume, before) {
            Ok(read) => read.spent,
            Err(no) => no.spent.unwrap_or_else(priced),
        };
        let spent = Spent {
            compactions: marks,
            ..spent
        };
        let spent_any = spent.input_tokens + spent.cached_tokens + spent.output_tokens > 0;
        if status.success() || spent_any {
            return Ok(spent);
        }
        let end = process::tail(String::from_utf8_lossy(&stderr).trim().as_bytes());
        let why = fill(
            &said.exited,
            &[
                ("command", &self.settings.agents.command),
                ("status", &status.to_string()),
                ("said", &end),
            ],
        );
        Err((why, spent))
    }
}

/// Where the work's spend and time are counted from, as its history judges them: the place where it was last let on
/// after the engine paused it, or its start.
fn stretch(work: &Work) -> decide::Seq {
    let pauses = &work.records.pauses;
    (pauses.iter().enumerate())
        .filter(|(i, p)| {
            !p.paused
                && pauses[..*i]
                    .iter()
                    .rev()
                    .find(|q| q.paused)
                    .is_some_and(|q| q.by == Pauser::Engine)
        })
        .map(|(_, p)| p.at)
        .max()
        .unwrap_or(decide::Seq(0))
}

/// The unit a work's spend or time is said in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unit {
    /// Dollars, of millionths of a dollar.
    Dollars,
    /// Minutes, of milliseconds.
    Minutes,
}

impl Unit {
    /// A value as the words say it in the unit.
    fn say(self, value: f64) -> String {
        match self {
            Unit::Dollars => format!("{:.2}", value / 1_000_000.0),
            Unit::Minutes => format!("{:.0}", value / 60_000.0),
        }
    }
}

/// The rounds of the run before the one of this number that ran to an end of their own, newest first: those that
/// failed for another cause than `kind`, or were cut off, passed over.
fn ended_before(work: &Work, run: RunId, number: usize, kind: Failure) -> Option<(usize, &Back)> {
    (1..number).rev().find_map(|k| match work.back.get(&(run, k)) {
        Some(back @ Back::Failed { failure, .. }) if *failure == kind => Some((k, back)),
        Some(Back::Failed { .. }) | None => None,
        Some(back) => Some((k, back)),
    })
}

/// Whether the round of the run before this one, passing over those that failed otherwise, failed of this kind.
fn stopped_before(work: &Work, run: RunId, number: usize, kind: Failure) -> bool {
    matches!(ended_before(work, run, number, kind), Some((_, Back::Failed { failure, .. })) if *failure == kind)
}

/// Whether the owner let the round of this number of the run run past what stopped the run's round before it, of this
/// kind: that round, passing over those that failed for another cause or were cut off, failed of it, and the work was
/// let on since it ended.
fn let_run(work: &Work, run: RunId, number: usize, kind: Failure) -> bool {
    let Some((k, Back::Failed { failure, .. })) = ended_before(work, run, number, kind) else {
        return false;
    };
    let ended = (work.records.returned.iter()).filter(|e| e.run == run).nth(k - 1);
    let let_on = |at: decide::Seq| work.records.pauses.iter().any(|p| !p.paused && p.at > at);
    *failure == kind && ended.is_some_and(|ended| let_on(ended.at))
}

/// When the account's usage limit resets, where the failure is that limit and the program's stream or its report says
/// when, the time now read by the clock.
fn resets(failure: Failure, why: &str, said: &failure::Said, clock: &dyn Clock) -> Option<work::Time> {
    if failure != Failure::UsageLimit {
        return None;
    }
    let now = clock.now().ok()?;
    failure::resets(why, said, now.0, &failure::zone_offset).map(work::Time)
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

    /// The calls of the session's own agent the stream has told so far.
    fn calls(&self) -> &Calls {
        &self.stream.own
    }

    /// The uses of tools of the session's own agent the stream has told so far.
    fn tools(&self) -> &[(String, String, String)] {
        &self.stream.tools
    }

    /// What the try's calls came to, given a message of this many characters, with what came before the session's
    /// messages and the program's version as its stream told it, whether it went past the run guard's lines, and
    /// whether what it took is priced from its calls; none where its program printed nothing.
    fn measures(
        &self,
        given: u64,
        mut before: BTreeMap<String, String>,
        crossed: bool,
        priced: bool,
    ) -> Option<Measures> {
        if self.stream.is_empty() {
            return None;
        }
        if let Some(program) = &self.stream.program {
            before.insert("program".to_string(), program.clone());
        }
        let calls = &self.stream.own;
        // The program's own count where it printed its result; what the stream told of the agent's calls where not, as
        // of a try that was stopped.
        let (fresh, read, written, thinking, turns) =
            (self.stream.printed()).unwrap_or((calls.fresh, calls.read, calls.written, None, u64::from(calls.count)));
        Some(Measures {
            input_tokens: fresh,
            cache_read_tokens: read,
            cache_written_tokens: written,
            thinking_tokens: thinking,
            turns,
            calls: calls.count,
            first_read: calls.first.map(|(read, _)| read),
            first_context: calls.first.map(|(_, whole)| whole),
            added: calls.added,
            given,
            before,
            received: self.stream.began(),
            crossed,
            priced,
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

#[cfg(test)]
mod tests {
    use decide::{Pause, Returned, Round as Began, Seq};
    use work::Back;

    use super::*;

    fn pause(at: u64, paused: bool, by: Pauser) -> Pause {
        Pause {
            paused,
            at: Seq(at),
            by,
            why: None,
        }
    }

    #[test]
    fn a_works_spend_is_counted_from_where_it_was_last_let_on_after_the_engine_paused_it() {
        let mut work = Work::default();
        assert_eq!(stretch(&work), Seq(0), "from its start");
        work.records.pauses = vec![pause(3, true, Pauser::Owner), pause(4, false, Pauser::Owner)];
        assert_eq!(stretch(&work), Seq(0), "the owner's own pause starts nothing anew");
        work.records
            .pauses
            .extend([pause(6, true, Pauser::Engine), pause(8, false, Pauser::Owner)]);
        assert_eq!(stretch(&work), Seq(8));
    }

    /// A work whose one run's rounds ended so, each a round of its own, the work paused and let on as the pauses say.
    fn failed(backs: &[Option<Failure>], pauses: Vec<Pause>) -> Work {
        let mut work = Work::default();
        let run = RunId(1);
        for (k, back) in backs.iter().enumerate() {
            let at = 10 * k as u64 + 1;
            work.records.rounds.push(Began { run, at: Seq(at) });
            work.records.returned.push(Returned { run, at: Seq(at + 1) });
            let back = match back {
                Some(failure) => Back::Failed {
                    why: "it failed".to_string(),
                    failure: *failure,
                    resets: None,
                },
                None => Back::HandedIn(Vec::new()),
            };
            work.back.insert((run, k + 1), back);
        }
        // The round the questions are about, under way.
        work.records.rounds.push(Began { run, at: Seq(1000) });
        work.records.pauses = pauses;
        work
    }

    #[test]
    fn a_round_after_the_guard_stopped_its_run_and_the_owner_let_the_work_on_runs_past_the_guard() {
        let (run, guard) = (RunId(1), Failure::Guarded);
        let let_on = || vec![pause(50, true, Pauser::Engine), pause(60, false, Pauser::Owner)];
        let stopped = failed(&[Some(guard), Some(guard)], Vec::new());
        assert!(!let_run(&stopped, run, 3, guard), "not let on since");
        let stopped = failed(&[Some(guard), Some(guard)], let_on());
        assert!(let_run(&stopped, run, 3, guard));
        assert!(stopped_before(&stopped, run, 3, guard));
        assert!(!let_run(&stopped, run, 1, guard), "a first round");
        // A failure of another cause between, as in passing after the owner's Start, does not undo the release.
        let between = failed(&[Some(guard), Some(Failure::Passing)], let_on());
        assert!(let_run(&between, run, 3, guard));
        // A round that ended of itself since does.
        let ended = failed(&[Some(guard), None], let_on());
        assert!(!let_run(&ended, run, 3, guard));
        let flagged = failed(&[Some(Failure::Flagged)], let_on());
        assert!(!let_run(&flagged, run, 2, guard), "stopped for another cause");
    }

    #[test]
    fn a_line_is_said_in_its_unit() {
        assert_eq!(Unit::Dollars.say(2_500_000.0), "2.50");
        assert_eq!(Unit::Minutes.say(180_000.0), "3");
    }

    #[test]
    fn a_return_counts_the_characters_it_was_written_in_and_a_failure_none() {
        let handed = vec![work::Handed {
            block: "plan".into(),
            content: "Greet by name.".to_string(),
            matters: true,
            value: None,
        }];
        let back = Back::HandedIn(handed.clone());
        assert_eq!(
            chars_of(&back),
            serde_json::to_string(&handed).unwrap().chars().count() as u64
        );
        assert!(chars_of(&back) > "Greet by name.".len() as u64);
        let failed = Back::Failed {
            why: "the program crashed".to_string(),
            failure: Failure::Passing,
            resets: None,
        };
        assert_eq!(chars_of(&failed), 0);
    }
}
