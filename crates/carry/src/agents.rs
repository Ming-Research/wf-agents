//! The agents' rounds, run with the `claude` command line: each round one run of the program, in the work's worktree,
//! in the session of the cell's line, told its message on its standard input, with the form of return of its line,
//! made of the flow and the files beside the flows and so the same for every round of every cell of the line
//! ([`flow::form`]), and the same system prompt for every round of a line ([`system`](crate::system)), made from the
//! files beside the flows, which says what the line may run as its permissions give it to the program
//! (`permissions`). The system prompt and the form are made as a session begins and kept with it, alike, so that every
//! round of the session is given those it began with, whatever the work's branch or the flow installed has changed
//! since; a round whose system prompt or form cannot be made, as for a file of the project's that is not in the
//! worktree, fails, saying why. Where a new session of a line would begin with other ones now, or with ones that cannot
//! be made, the policy the agents give says so, and the line begins a new session at its next round (decision
//! `a-changed-system-prompt-or-form-begins-a-new-session`). What came back is read against the cell the round was
//! entered for ([`returns`](crate::returns)). How many run at once is the engine's to bound: it gives at most as many
//! rounds at once as the settings give slots, and holds the others back in its own order.
//!
//! A round is run with the project of its work's item ([`crate::projects`]): its declaration, the settings its texts
//! name filled from it, and main's head as its host last gave it. A round of a work whose item has no project the
//! settings name, or whose project is held for its declaration, fails, saying so.
//!
//! A round fails, handing in nothing, where the worktree is not there, the program cannot be started, or its stream
//! holds no return of the form: neither one the command line took, nor, where it gave up on the return, its last
//! attempt at one, rebuilt from the stream (`cli::Stream`); a return the stream holds is read however the program
//! exited. The kind of its cause is told apart from what the program reported ([`failure`](crate::failure)): a
//! program that cannot be started cannot run, a return not of the form could not be read, and a round that failed
//! before its program ran failed for a cause no kind tells apart. It is not tried again here: the engine handles it as
//! its kind says. A round that fails takes what the result the program printed says it cost, as any round does, and
//! is held to the budget as any round is, its work paused where it went over; where the program printed nothing that
//! reads as a result, what the round took is not known, and is recorded as nothing. Stopped while its program runs in
//! this process, the program's process group is sent TERM, and KILL after three seconds, and the round ends with
//! nothing handed in; what it spent is not known, and is recorded as nothing.
//!
//! The program is given an environment of its own, not the engine's (`cli::environment`), and in it the program's
//! directory, the engine's (`account::profile`), and the token of the account the round was handed out with, as what
//! keeps the tokens gives it as the round begins; a token it cannot give, or none where the round was handed out with
//! no account, fails the round as a command line that cannot run, in its words, nothing being run (decision
//! no-default-credential). A keep-warm ping runs under the account it is handed out with, which its session's last call
//! ran under, and a fork under its session's account, or under the one in use where that one was removed or the session
//! ran under none, which it says. What the program's stream says of the usage of the account a round, a ping or a fork
//! runs under is said to the loop under that account, a round's as each rate limit event comes. Which of its line's
//! sessions a round runs in, and whether it begins it, is counted from the work's records (`cli::in_session`); what it
//! is sent is what the context manager fills into that session (crate `context`): what the session lacks, or, where the
//! program begins the session under its id, everything from the top, and what each try's message fills is said to the
//! loop before the program is given it, so that it is recorded. A session the program no longer has is begun again
//! under its id, and that is said to the loop at once, so that it is recorded with why. Where the program compacts the
//! session during the round, a hook it is given marks the compaction, which the round counts among what it took and
//! says to the loop as the session reopened, and gives the session the round's whole message again, as a session that
//! holds nothing is sent it. After each use of a tool another hook gives the session what the owner said to the round
//! while it runs, where the runner put it in the round's files (`cli::Words`).
//!
//! A round given again after the process of the engine that gave it ended while a try of it ran goes on from that
//! try, where its program said it began, reached the service or printed its result: what the try kept of its stream is
//! read as the end of a try that ended by itself, a return there taken as any, one past the run guard's lines failed
//! for it, and any other a crash, which fails the round in passing, saying the engine ended before it saw the program
//! end; what the records do not hold of the try, its measures, its compactions and that the service answered it, is
//! told once, as any try's is, and nothing is given the session again, taken in from main or read of the branch as the
//! round begins, what the try committed being the round's. The try is read before anything that can end the round
//! here, so that a round that now fails before its program runs, as for a level the settings no longer have, takes
//! what the try took. Only a try whose program said none of these, as one the engine ended before it read its message,
//! is given again, told the same. A round stopped after that process ended takes what such a try took, as its stream
//! says (its result's spend where it printed one, else its calls at their prices), and tells what the records do not
//! hold of it, but takes no return from it.
//!
//! As a round of a line that may change the work ends with a return, the engine looks at what the work's branch changes
//! under the paths the project protects, and asks the owner by itself where it is a change not asked of yet, beside
//! the return, each file with why the work says it is needed; where a file of it has no reason, it asks nothing, and a
//! return that does the cell's work is found wanting for each such file, and so sent back
//! ([`protected`](crate::protected)). As a round of a line that changes only the project's tests ends with a return of
//! either kind, what the work itself changed from the version of its head the run began on to its head as the round
//! ended is held to the tests, and the return is found wanting for each file that is none of them; a look that cannot
//! be made, a read of the branch that failed among them, fails the round ([`tests_only`](crate::tests_only)). As a
//! round of a line that may change the work ends with a return that asks for no call, the return is found wanting where
//! the worktree holds changes to files git tracks that are not committed, or a merge begun and not committed; a file
//! git does not track is none. As a round ends whose return writes a block handed in only with the owner's answers in
//! the design tree, the return is found wanting for each question marked `tree` that has the owner's answer, while the
//! work's change against main changes nothing in the tree; a look that cannot be made fails the round
//! ([`tree`](crate::tree)).
//!
//! Before the first round of each run of a cell that takes main in, the work's branch takes in main's head as the host
//! last gave it ([`catch_up`](crate::catch_up)), and what came of it is kept beside the run's rounds before the round
//! is told it; a round of the run given again, or another of its rounds given its task, is told what was kept, and
//! nothing is taken in again for the run. The branch's data, its design tree and the work's head, are read after that,
//! in the round that took main in, and after the round's program ended, where they may have changed
//! ([`branch`](crate::branch)): what is read as main is taken in is the outside's, put into the round; what is read as
//! the round ends is what it committed, handed in with its return, where its line may change the work and the return
//! does the cell's work (a round that asks for calls hands in none of it, and its commit is read by the round that ends
//! the cell's work), and the outside's where it only reads, but while a cell that may change the work waits for the
//! results of calls no read takes the head or the design tree, whichever round or act reads, but the one that ends that
//! cell's work.
//!
//! Each round runs on the model of its item's level, or of a level above it where its cell's line says so, at the
//! effort its cell sets, under the account it was handed out with (`Settings::ran_on`); a round whose item's level the
//! settings no longer allow fails, saying so. What each try runs on is said to the loop as it begins, and recorded in
//! the work's records: the account by its name, the one its token is given of, so that what is recorded and what is
//! given are one.
//!
//! What each round was told and the program's stream are kept for reading, in `rounds/<work>/<run>-<round>` in the
//! data directory: `.message.md` and `.stream.jsonl` for its first try, `.message.2.md` and `.stream.2.jsonl` for the
//! next, and so on, the stream written as it comes, and a message told again, as to a try after one whose program took
//! nothing in, kept as a second name of the file an earlier try of the round keeps it in, so that it is stored once;
//! `.entered.json`, `.entered.2.json` and on what each try runs on and fills into its session, kept before its message;
//! `.compacted` holds a byte for each compaction of the session during the round, and `.again.md`, while the program
//! runs and never past the round's tries, what the hook gives again; `.words.json` what the owner said that waits for
//! the hook, `.given.1.json` and on what the hook gave, and `.said.json` the versions of the owner's words each says.
//! The program's process group is stopped as the process of the engine ends, however it ends ([`process`]), and is
//! written in `.pgid` there while the program runs; a group left named there all the same by a process of the engine
//! that ended is stopped before the round is run again, or before the round's stop ends it, so that a session has one
//! program at a time and a stopped round's program does not run on. What a program that ends by itself leaves running
//! in its group is stopped as it ends, and counted with its try.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use decide::run::{Failure, Unmet};
use decide::{BlockId, CellId, Pauser, RunId, Seq};
use driver::{Going, Policy};
use engine::Came;
use flow::agents::Declaration;
use serde::{Deserialize, Serialize};
use service::{
    Clock, Doings, Fork, ForkFailed, Forked, Mark, Ping, PingReply, Round, RoundReply, Rounds, SystemClock, Teller,
    Telling, Token, Tokens, Told,
};
use store::WorkNumber;
use tokio::process::Command;
use tokio::sync::oneshot;
use uuid::Uuid;
use work::{Back, Fill, Measures, RanOn, Spent, Why, Work};

use crate::catch_up::{self, CaughtUp};
use crate::cli::{self, Begin, Calls, Compacted, InSession, Words};
use crate::guard::{Crossing, Guard};
use crate::held::{Failing, Held, Stop};
use crate::permissions::{self, Permissions};
use crate::process::{self, Ran};
use crate::settings::{Project, Settings};
use crate::{account, acts, branch, failure, git, kept, protected, returns, session, system, tests_only, tools};

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
    projects: BTreeMap<String, Result<Of, String>>,
    /// What is said of a result of the agents' program that cannot be read, in the words beside the flows: the same for
    /// every project, which a declaration does not change.
    program: flow::agents::ProgramWords,
    held: Held<(WorkNumber, RunId, usize), RoundReply>,
    /// The clock a usage limit's reset is read by, where the command line gives it as a time of day.
    clock: Arc<dyn Clock>,
    /// A lock for each line of a work, which is each session's program one at a time: a keep-warm ping of a line never
    /// runs beside a round of it, a round waits for a ping of its line to end, and a ping that finds a round of its line
    /// under way is given up; a fork of a session is the same. The rounds and pings of the work's other lines hold
    /// locks of their own.
    in_work: std::sync::Mutex<Locks>,
    /// What the owner said to each round given and not yet ended that its hook is to give it, by the round; one
    /// telling of a round, or its end, at a time, so that what waits for its hook is known.
    said: std::sync::Mutex<Tellings>,
    /// What keeps the tokens of the accounts the rounds run under.
    tokens: Arc<dyn Tokens>,
}

/// The lock of each line of each work, by the work and the line's name.
type Locks = BTreeMap<(WorkNumber, String), Arc<tokio::sync::Mutex<()>>>;

/// What the owner said to each round, by the round, each behind a lock of its own.
type Tellings = BTreeMap<(WorkNumber, RunId, usize), Arc<tokio::sync::Mutex<Said>>>;

/// Whether the round's program has ended, which hears nothing more: what the owner said to it, and what its hook gave
/// it, is in the round's files of words ([`Words`]).
#[derive(Default)]
struct Said {
    ended: bool,
}

/// Why a block whose lines name entries of the design tree (`names_tree_entries`) can be neither checked nor given:
/// the project's declaration says nothing of how an entry is found.
fn no_tree_format() -> String {
    format!(
        "the project's declaration, {}, has no [tree_format], which a block whose file says names_tree_entries needs",
        flow::agents::DECLARATION
    )
}

/// What the round of this number of the run, of this cell, is given in place of the latest version of each block it
/// is given whose lines name entries of the design tree (`names_tree_entries`): the entries whole, as the tree in the
/// worktree at `dir` has them at the commit of the tree's latest version as the round began, by the block and the
/// version's place. None of a cell given no such block; each name said to be held by no tree where the tree has no
/// version.
async fn entries_given(
    flow: &flow::Compiled,
    work: &Work,
    run: RunId,
    number: usize,
    cell: &flow::CellSpec,
    dir: &Path,
    of: &Of,
) -> Result<BTreeMap<(BlockId, Seq), String>, String> {
    let mut given = BTreeMap::new();
    let naming: Vec<&BlockId> = (cell.given.iter().map(|g| &g.block))
        .filter(|block| flow.blocks.get(*block).is_some_and(|b| b.names_tree_entries))
        .collect();
    if naming.is_empty() {
        return Ok(given);
    }
    let format = of.declaration.tree_format.as_ref().ok_or_else(no_tree_format)?;
    let began = work::sessions::began_at(work, run, number).unwrap_or_else(|| work.tick());
    let versions = decide::versions(&work.records.before(began));
    let latest = |block: &BlockId| {
        let at = versions.get(block).and_then(|all| all.last()).map(|v| v.at)?;
        Some((at, work.contents.get(&(block.clone(), at))?.as_str()))
    };
    let tree = (flow.blocks.iter()).find(|(_, b)| b.writers.outside.as_deref() == Some(crate::tree::TREE));
    let commit = tree
        .and_then(|(block, _)| latest(block))
        .and_then(|(_, content)| crate::tree::commit_of(content));
    let words = &of.beside.given.entries;
    for block in naming {
        let Some((at, names)) = latest(block) else {
            continue;
        };
        let text = crate::tree::entries(dir, &of.project.tree, format, commit.as_deref(), names, words).await?;
        given.insert((block.clone(), at), text);
    }
    Ok(given)
}

/// What the rounds of the works of a project's items are run with.
pub struct Of {
    /// The project's settings.
    project: Project,
    /// What is written beside the flows, which the form and the system prompts are made from.
    beside: Beside,
    /// The project's declaration, `wf-agents.toml` on its main, which the system prompts and the permissions are made
    /// from.
    declaration: Declaration,
    /// What the engine puts into the texts its agents are sent: the settings they name, with their values, and the
    /// places it fills alike for every line.
    filling: system::Filling,
    /// The reference that holds the head of the project's main as its host last gave it, which a run that takes main
    /// in takes in.
    main: String,
}

impl Of {
    /// What the rounds of a project's works are run with, of these settings: the project's own, what is written beside
    /// the flows, its declaration, and the reference of its repository that holds main's head as its host last gave
    /// it.
    pub fn new(settings: &Settings, project: Project, beside: Beside, declaration: Declaration, main: String) -> Of {
        let filling = system::filling(settings, &beside, &declaration);
        Of {
            project,
            beside,
            declaration,
            filling,
            main,
        }
    }

    /// What a new session of the cell's line begins with: its system prompt, which tells the line what `permissions`
    /// let it do and holds the project's files as the worktree at `dir` has them, or its form of return; or why it
    /// cannot be made. A round makes it as its session begins, and the policy makes it to say whether a session would
    /// begin with another one now.
    pub(crate) fn begins_with(
        &self,
        flow: &flow::Compiled,
        cell: &flow::CellSpec,
        permissions: &Permissions,
        dir: &Path,
        what: system::Begun,
    ) -> Result<String, String> {
        let line = (cell.line.as_deref()).ok_or_else(|| format!("the cell {} names no line", cell.id))?;
        match what {
            system::Begun::System => {
                let read = |path: &str| system::read_within(dir, path);
                let tests = cell.tests_only.then(|| tests_only::paths(&self.declaration));
                let your_line = permissions.told(&self.beside.your_line, &self.declaration.protected.paths, tests);
                system::of_line(
                    &self.beside,
                    &self.declaration,
                    &self.filling,
                    flow,
                    cell,
                    &your_line,
                    read,
                )
            }
            system::Begun::Form => flow::form::form(flow, &self.beside, line),
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
    /// The agents of these settings, with this data directory, which is an absolute path, the installation's id, what
    /// the rounds of each project's works are run with, by the project's name, and what keeps the tokens of the
    /// accounts they run under.
    pub fn new(
        settings: Settings,
        data: PathBuf,
        installation: Uuid,
        projects: BTreeMap<String, Result<Of, String>>,
        tokens: Arc<dyn Tokens>,
    ) -> Self {
        Self::with_clock(settings, data, installation, projects, tokens, Arc::new(SystemClock))
    }

    /// The agents as [`Agents::new`] gives them, reading the time by this clock.
    pub fn with_clock(
        settings: Settings,
        data: PathBuf,
        installation: Uuid,
        projects: BTreeMap<String, Result<Of, String>>,
        tokens: Arc<dyn Tokens>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        // The words are beside the flows, the same for every project: those of any project that is carried. A held
        // project runs no program.
        let program = (projects.values().find_map(|of| of.as_ref().ok())).map(|of| of.beside.cost.program.clone());
        Agents {
            inner: Arc::new(Inner {
                settings,
                data,
                installation,
                program: program.unwrap_or_default(),
                projects,
                held: Held::new(),
                clock,
                in_work: std::sync::Mutex::new(BTreeMap::new()),
                said: std::sync::Mutex::new(BTreeMap::new()),
                tokens,
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
        (self.inner.said.lock().expect("the tellings' lock")).insert(key, Arc::default());
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let carried = inner.carry(&round, &teller, &mut stop).await;
            // What the owner said that the round's hook gave it is said before the round's end, so that the round
            // consumes it and its session is not given it again.
            inner.heard(&round, &teller).await;
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
                        // Told by what the program reported where the round was carried, as its kind is.
                        let resets = (failure == Failure::UsageLimit)
                            .then_some(said.resets)
                            .flatten()
                            .map(work::Time);
                        match inner.to_wait(&round, &spent) {
                            Some(over) => {
                                // The two reasons are joined in the project's words, as two reasons to wait are, the
                                // failure with its cause after a colon.
                                let (between, failed) = (crate::projects::of(&inner.projects, &round.item))
                                    .map(|of| (of.beside.cost.work.between.clone(), of.beside.cost.work.failed.clone()))
                                    .unwrap_or_default();
                                let cause = flow::agents::sentence(&why);
                                let pause =
                                    format!("{}{between}{over}", flow::agents::fill(&failed, &[("why", &cause)]));
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
        if let Some((reply, here)) = self.inner.held.stopped(key, reply) {
            // Nothing here carries the round out; a program a process of the engine that ended left running for it
            // is stopped before the round ends, which takes what its last try took, as its stream kept it says: the
            // try that process left, its calls and compactions told as any try's, or, for a round given here whose
            // carrying has ended and told them, that try's spend alone.
            let group = self.inner.kept(&round).with_extension("pgid");
            let dir = git::place(&self.inner.data, &git::tag(&self.inner.installation), round.work).dir;
            let inner = self.inner.clone();
            tokio::spawn(async move {
                process::stop_left(&group, &dir).await;
                let teller = reply.teller();
                let spent = inner.left_spent(&round, (!here).then_some(&teller)).await;
                // What the hook gave a try a process that ended left is the round's, as for a round given again; a
                // round given here had it said as its carrying ended.
                if !here {
                    inner.hook_gave(&round, &teller).await;
                }
                reply.ended(Came::hands_in(Vec::new()), spent);
            });
        }
    }

    fn tell(&self, round: Round, told: Telling) {
        let inner = self.inner.clone();
        tokio::spawn(async move { inner.tell(&round, told).await });
    }

    fn told(&self, work: WorkNumber, run: RunId, round: usize) -> Result<Told, String> {
        kept::told(&self.inner.data, work, run, round, &self.inner.program)
    }

    fn system(&self, work: WorkNumber, line: &str, number: u32) -> Result<Option<String>, String> {
        let session = session::session(self.inner.installation, work, line, number);
        let at = system::kept_at(&self.inner.data, session, system::Begun::System);
        match std::fs::read_to_string(&at) {
            Ok(system) => Ok(Some(system)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!(
                "{}: {e}",
                at.strip_prefix(&self.inner.data).unwrap_or(&at).display()
            )),
        }
    }

    fn doing(&self, work: WorkNumber, run: RunId, round: usize, from: Mark) -> Result<Doings, String> {
        kept::doing(&self.inner.data, work, run, round, from)
    }

    fn policy(&self) -> Option<Policy> {
        let mut policy = self.inner.settings.policy();
        let inner = self.inner.clone();
        let changed = move |flow: &flow::Compiled, work, item: &store::Item, cell: &flow::CellSpec, session| {
            inner.changed(flow, work, item, cell, session)
        };
        policy.changed = Some(Box::new(changed));
        Some(policy)
    }

    fn fork(
        &self,
        fork: Fork,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Forked, ForkFailed>> + Send + '_>> {
        // Whoever asks stops the fork by giving the request up: its program ends with it.
        Box::pin(async move { self.inner.fork(&fork).await })
    }

    fn ping(&self, ping: Ping, reply: PingReply) {
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let mut usage = None;
            let mut left_running = 0;
            let pinged = inner.ping(&ping, &mut usage, &mut left_running).await;
            // Said before the ping's end, under the account the ping was handed out with.
            if let (Some(account), Some(said)) = (ping.account.clone(), usage) {
                reply.usage(account, said);
            }
            if left_running > 0 {
                reply.left_behind(left_running);
            }
            match pinged {
                Ok(spent) => reply.ended(spent),
                Err((why, spent)) => reply.failed(why, spent),
            }
        });
    }
}

impl Inner {
    /// What the program is let run for a round of the cell, of the project.
    fn permissions(&self, of: &Of, cell: &flow::CellSpec) -> Permissions {
        let refused = permissions::refused_commands(&of.beside, &of.declaration, &self.settings.agents.command);
        Permissions::of(cell.reads_only, &of.beside.allowed.may, &std::env::temp_dir(), &refused)
    }

    /// Whether the line of the cell, in the work, for the item, would begin a new session of it with another system
    /// prompt or form of return now than its session of this number began with, or with one that cannot be made
    /// ([`system::changed`]). No where the round would fail before either is made, saying why: where the item has no
    /// project of the settings, or the work has no worktree, which the cell that starts the work makes.
    fn changed(
        &self,
        flow: &flow::Compiled,
        work: WorkNumber,
        item: &store::Item,
        cell: &flow::CellSpec,
        number: u32,
    ) -> bool {
        let Some(line) = cell.line.as_deref() else {
            return false;
        };
        let Ok(of) = crate::projects::of(&self.projects, item) else {
            return false;
        };
        let place = git::place(&self.data, &git::tag(&self.installation), work);
        let Ok(dir) = git::worktree(&place) else {
            return false;
        };
        let permissions = self.permissions(of, cell);
        let session = session::session(self.installation, work, line, number);
        system::changed(&self.data, session, |what| {
            of.begins_with(flow, cell, &permissions, &dir, what)
        })
    }

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
        // A round given again after the process of the engine that gave it ended while a try of it ran goes on from
        // that try, where its program said it began, reached the service or printed its result: what the try kept of
        // its stream says what came of it. It is read before anything that can end the round here, so that what the try
        // took is the round's however it ends. Nothing is taken in or read of the branch for it as it begins: what that
        // try committed is the round's, read as it ends.
        let left = match self.left(round, Some(teller)).await? {
            Some(Left {
                guarded: Some(failed), ..
            }) => return Ok(failed),
            left => left,
        };
        // The project of the work's item, whose repository and declaration the round is run with.
        let of = match crate::projects::of(&self.projects, &round.item) {
            Ok(of) => of,
            Err(why) => return failed_before(left, why),
        };
        // A round given anew runs on what the settings give its item's level and its cell, and is given the token of
        // the account it was handed out with, which every try of it is given, taken before anything changes the
        // worktree or the branch: one that cannot be given, as with no account in use, fails the round as a command line
        // that cannot run, nothing changed and nothing sent, in the words of what keeps the tokens or beside the flows.
        // A try left by an engine that ended goes on with neither: it ran on what it recorded, and its return is read
        // from what it kept, so a level the settings no longer have does not fail its round.
        let begun = match left {
            Some(left) => Begun::Left(Box::new(left)),
            None => {
                let profile = account::profile(&self.data);
                let on = self.settings.ran_on(
                    round.item.level.as_deref(),
                    &round.cell,
                    &profile,
                    round.account.as_deref(),
                )?;
                match account::token_of(&self.tokens, round.account.as_deref(), &of.beside.cost.check.no_account).await
                {
                    Ok(token) => Begun::Anew(token, round.account.clone().unwrap_or_default(), on),
                    Err(why) => {
                        return Ok(Carried::Failed(
                            why,
                            Failure::CannotRun,
                            failure::Said::default(),
                            Spent::default(),
                        ));
                    }
                }
            }
        };
        let dir = match git::worktree(&place) {
            Ok(dir) => dir,
            Err(why) => return failed_before(begun.left(), why),
        };
        let main = if round.cell.takes_main && matches!(begun, Begun::Anew(..)) {
            self.caught_up(round, &place, of).await?
        } else {
            None
        };
        // The branch may have new data where the round took main in, which only the run's first round does: a later
        // round of the run would find what an earlier one committed, which is that run's.
        let read_with = branch::Reading {
            tree: of.project.tree.clone(),
            main: of.main.clone(),
        };
        let mut looking = branch::Looking::of(flow, work, read_with.clone(), Some(round.run));
        if main.is_some() && round.number == 1 {
            looking.tell(&place, teller, true).await;
        }
        if let Some(parent) = base.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let permissions = self.permissions(of, &round.cell);
        // One program at a time in a line's session: a keep-warm ping of the line under way ends first, so that what its
        // session holds and cost is worked out with the ping in.
        let lock = self.lock(round.work, line);
        let _one = lock.lock().await;
        let InSession { number, mut begin } = cli::in_session(flow, work, line, round.run, round.number);
        let session = session::session(self.installation, round.work, line, number);
        // What the round is judged by while it runs: the prompt its session last had, and whether the owner let it run
        // past the guard.
        let last = driver::history::last_prompt(flow, work, line, round.run, round.number);
        let returned = driver::history::last_return(flow, work, line, round.run, round.number).map_or(0, chars_of);
        let waived = let_run(work, round.run, round.number, Failure::Guarded);
        // The guard stopped the run's round before this one, which this one is tried again for.
        let guarded_before = stopped_before(work, round.run, round.number, Failure::Guarded);
        // The system prompt and the form of return are the session's: made from the worktree and the flow as it begins,
        // and kept, so that every try of every round of it is given those it began with.
        let begun_with = || -> Result<(String, String), String> {
            let of_session = |what: system::Begun| {
                let made = || of.begins_with(flow, &round.cell, &permissions, &dir, what);
                system::of_session(&system::kept_at(&self.data, session, what), made)
            };
            Ok((of_session(system::Begun::System)?, of_session(system::Begun::Form)?))
        };
        // A compaction within the round is marked, and the round's whole message, as a session that holds nothing is
        // sent it, is given again.
        let compacted = Compacted {
            marks: base.with_extension("compacted"),
            again: base.with_extension("again.md"),
        };
        // The session the program lost is begun again under its id, and that is recorded once for the round.
        let recorded = lost_in(work, line, round);
        let (status, stream, stderr, compactions) = match begun {
            // The try the engine that ended left, read as of a try that ended by itself, but for how its program ended,
            // which no process of the engine saw ([`Inner::left`]). Nothing is given the session again.
            Begun::Left(left) => {
                let left = *left;
                (None, left.stream, String::new(), left.compactions)
            }
            Begun::Anew(token, under, on) => {
                // What the round is sent in the session as it stands, and in one that holds nothing: one begun under
                // the round, or compacted during it.
                let (written_work, written_item) = (round.work.to_string(), round.item.number.to_string());
                let about = context::For {
                    work: &written_work,
                    item: &written_item,
                    title: &round.item.title,
                };
                // What changed in the branch's data, which git compares, from each earlier version to the latest: the
                // files of the design tree, and the command that shows what the work itself changed.
                let changes = branch::changes(flow, work, round.run, round.number, &dir, &read_with).await?;
                // The task as the message sends it, each setting it names with its value beside it; and a result of the
                // tool `query` longer than its file says a message gives, cut as the tool cuts it, the records holding
                // it whole and a file in the temporary directory, which the program is let read, holding it whole for
                // the agent.
                let files = tools::files_dir(&std::env::temp_dir(), &git::tag(&self.installation), round.work);
                if let Some(root) = files.parent() {
                    tools::sweep(root, std::time::SystemTime::now()).await;
                }
                tools::keep_wholes(work, &of.beside.tools, &files).await?;
                let results = tools::given(work, &of.beside.tools, &files);
                let told = main.as_ref().and_then(CaughtUp::told);
                let compared = branch::compared(flow);
                // What the round is given in place of the names a block's lines write, where they name entries of the
                // design tree: the entries whole, as the tree's latest version has them.
                let given_as = entries_given(flow, work, round.run, round.number, &round.cell, &dir, of).await?;
                let carried = context::Carried {
                    about,
                    main: told.as_ref(),
                    compared: &compared,
                    changes: &changes,
                    given_as: &given_as,
                    named: &of.filling.named,
                    results: &results,
                };
                let enter = |anew| context::enter(flow, &of.beside, work, round.run, round.number, &carried, anew);
                let (going_on, anew) = (enter(false)?, enter(true)?);
                let again = format!("{}\n\n{}", of.beside.message.compacted, anew.text);
                write(&compacted.again, again.as_bytes()).await?;
                // It is the hook's while the program runs, and is not kept past the round's tries, however they end.
                let again = Removed(compacted.again.clone());
                // What the owner says to the round while it runs waits beside it, for the hook after the next use of a
                // tool.
                let words = words(&base);
                let mut tried = false;
                let ran = loop {
                    // A session begun under its id holds nothing: it is sent everything from the top.
                    let entered = if begin == Begin::New { &anew } else { &going_on };
                    // The round takes what its last try took, so it counts that try's compactions alone: an earlier
                    // try's, or a try's before the engine stopped, are not its.
                    let _ = tokio::fs::remove_file(&compacted.marks).await;
                    let (n, told_at, stream_at) = attempt(&base).await;
                    // What the try runs on is said before its message is written, which makes the try one that was
                    // kept, and what its message fills, before the program is given it. Both are kept beside the try
                    // first, so that what the loop has not recorded when its process ends is told from there.
                    let kept_entered = Entered {
                        on: on.clone(),
                        fills: entered.fills.clone(),
                    };
                    keep_entered(&kept::entered_at(&base, n), &kept_entered).await?;
                    teller.ran_on(n, on.clone());
                    teller.filled(n, entered.fills.clone());
                    let told = &entered.text;
                    keep_told(&base, n, &told_at, told).await?;
                    let (system, form) = begun_with()?;
                    let before = before_messages(&on, round.account.as_deref(), &system, &form);
                    let command = self.command(
                        &dir,
                        &of.declaration.environment,
                        &on,
                        &system,
                        &permissions,
                        &form,
                        &compacted,
                        &words,
                        (session, begin),
                        &token,
                        false,
                    );
                    // The stream is kept as it comes, so that what the agent is doing can be read while it does it; and
                    // the run guard judges each call as the stream tells it, halting the program at a signal past its
                    // lines.
                    let mut kept = Keeping::to(&stream_at)?;
                    // What the engine gave the round in its session: its message, and the return before it, which the
                    // session has held only since its last call, so that a short message after a long return is no
                    // jump.
                    let given = told.chars().count() as u64 + returned;
                    let mut guard = Guard::new(round.lines.clone(), last, given, waived);
                    let (halt, halted) = oneshot::channel();
                    let mut halt = Some(halt);
                    let mut crossed: Option<Crossing> = None;
                    let mut told_answered = false;
                    let mut told_usage = 0;
                    // A try begins with nothing told of its calls, whatever the try before it came to.
                    teller.going(round.number, Going::default());
                    let mut told_calls = 0;
                    let ran = {
                        let mut each = |line: &[u8]| {
                            kept.line(line);
                            // The service serves the round: a wait every work shares need not wait for its end.
                            if !told_answered && kept.stream.answered() {
                                told_answered = true;
                                teller.answered();
                            }
                            // What the service says of the usage of the account the round was handed out with, as it
                            // says it, whichever account is in use now.
                            if kept.stream.usages() > told_usage {
                                told_usage = kept.stream.usages();
                                if let Some(said) = kept.stream.usage() {
                                    teller.usage(under.clone(), said.clone());
                                }
                            }
                            // What the calls have come to so far, against the lines the guard judges the round by, is
                            // told for each call: no record holds it, and the owner reads it while the round runs.
                            if kept.calls().count != told_calls {
                                let calls = kept.calls();
                                told_calls = calls.count;
                                let going =
                                    Going::of(&round.lines, calls.count, calls.last, calls.added, kept.stream.cost().0);
                                teller.going(round.number, going);
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
                        // A program that cannot be started printed no result and never began: it is the command line
                        // failing to run, which holds every work, whatever its words say of why.
                        let stdin = Some(cli::stdin(told));
                        process::streamed(command, stdin, Some(&group), stop, Some(halted), &mut each).await
                    };
                    // Each compaction of the try is a reopening of the session, told while the round is still under
                    // way, however the try ended; and so is what the calls of a try whose program ran came to. A round
                    // the guard stopped printed no result: what it took is its calls at their models' prices. So is
                    // what a try took whose program ended of itself with no result to say it, as one that crashed.
                    let compactions = marked(&compacted.marks).await;
                    let halted = crossed.is_some() && guard.stops();
                    let ended = matches!(&ran, Ok(Ran::Ended { .. }));
                    let priced = halted || (ended && kept.stream.estimated());
                    let measures = kept.measures(given, before, crossed.is_some(), priced);
                    tell_try(teller, line, n, compactions, measures);
                    // What the program left running in its group, stopped as it ended, is counted with the try, before
                    // its round ends.
                    if let Ok(Ran::Ended { left, .. }) = &ran
                        && *left > 0
                    {
                        teller.left_behind(round.number, n, *left);
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
                            let words = &of.beside.cost.guard;
                            return Ok(guarded(&crossing, words, &kept.stream, compactions, guarded_before));
                        }
                        return Ok(Carried::Stopped);
                    };
                    let stream = kept.done()?;
                    let stderr = String::from_utf8_lossy(&stderr).into_owned();
                    // A session begun before its first round was cut off, or one the program no longer has, is tried
                    // the other way, once.
                    if !status.success() && !tried && stream.refused(begin, &stderr) {
                        if begin == Begin::Resume && !recorded {
                            teller.session_began(line.to_string(), Why::Lost);
                        }
                        begin = begin.other();
                        tried = true;
                        continue;
                    }
                    break (Some(status), stream, stderr, compactions);
                };
                drop(again);
                ran
            }
        };
        // What the round committed is handed in with its return, where its line may change the work and the return asks
        // for no call: the run decides that as it takes the return ([`decide::run::next`]), the head being read here
        // whatever the return asks for, since the check of a line that changes only tests reads it. A change found
        // at the end of a round that only reads is none of its own, and the outside's. A round that fails hands in
        // nothing: what it committed is read at the end of the round that takes it up. The version of the work's head
        // the run began on is the latest the records keep, what an earlier round of the run committed being handed in
        // by none. While another cell that may change the work waits for the results of its calls, the branch is read
        // by none of the round's ends: what it holds is that cell's, handed in by the run entered for its results.
        let began = looking.head().map(str::to_string);
        let committed = if round.cell.reads_only {
            looking.tell(&place, teller, false).await;
            Ok(Vec::new())
        } else {
            looking.handed_in(&place).await
        };
        // The work's head as the round ended, where the branch could be read.
        let ended = (committed.as_ref())
            .map(|_| looking.head().map(str::to_string))
            .map_err(String::clone);
        let committed = committed.unwrap_or_else(|why| {
            eprintln!("the work's branch could not be read: {why}");
            Vec::new()
        });
        let had_result = stream.has_result();
        let read = stream.read(&of.beside.cost.program).map(|mut read| {
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
        let failed = |why: String, report: &str, said: failure::Said, spent: Option<Spent>| {
            Ok(failed_round(why, report, said, spent, &*self.clock))
        };
        let command = &self.settings.agents.command;
        let read = match (read, status) {
            (Ok(read), _) => read,
            (Err(no), Some(status)) if status.success() => {
                return failed(no.why, &no.report, no.said, no.spent);
            }
            (Err(no), Some(status)) => {
                let (why, report) = match had_result {
                    true => (no.why, no.report),
                    false => {
                        let tail = process::tail(stderr.trim().as_bytes());
                        (tail.clone(), tail)
                    }
                };
                let ended = process::ended(&of.beside.cost.program, command, status, &why);
                return failed(ended, &report, no.said, no.spent);
            }
            // A try the engine that ended left, which printed no return to take: what its result said, where it printed
            // one, or that its engine ended before it saw it end, in the words beside the flows, which fails as a crash
            // does.
            (Err(no), None) => {
                let (why, report) = match had_result {
                    true => (no.why, no.report),
                    false => {
                        let cut_off = &of.beside.cost.program.cut_off;
                        (
                            flow::agents::fill(cut_off, &[("command", command.as_str())]),
                            String::new(),
                        )
                    }
                };
                return failed(why, &report, no.said, no.spent);
            }
        };
        match returns::came(&read.structured, flow, &round.cell, &of.beside) {
            Ok(reading) => {
                let mut came = reading.came;
                came.read = committed;
                // What the returns of the run sent back before this one, which it completes, said and was not told was
                // wrong, which the look at the protected paths and the tree check hold what it says to as well as what
                // it says itself (`decide::run::carried`). A return that asks for calls completes nothing.
                let carried = if came.asks.is_empty() {
                    engine::carried(flow, work, round.run)
                } else {
                    Default::default()
                };
                let mut said_why = reading.protected.clone();
                if let Some(said) = said_why.as_mut() {
                    for (path, why) in &carried.protected {
                        said.entry(path.clone()).or_insert_with(|| why.clone());
                    }
                }
                let looked = self.protected(round, &place, of, said_why.as_ref()).await;
                came.engine.extend(looked.asks);
                let unexplained = looked.unexplained.into_iter().map(|path| Unmet::Unexplained { path });
                came.found.extend(unexplained);
                // A return that writes a block handed in only with the owner's answers in the design tree is held to
                // each question marked `tree` that has an answer, while the work's change leaves the tree as main has
                // it; so is one completed by a return before it that wrote it. A look that cannot be made fails the
                // round, as the look at the tests does, rather than take the change as holding them.
                let tree_block =
                    |block: &decide::BlockId| flow.blocks.get(block).is_some_and(|b| b.answers_in_the_tree);
                let with_the_tree = (came.writes.iter()).any(|h| tree_block(&h.block))
                    || (carried.writes.iter()).any(|w| tree_block(&w.block));
                if with_the_tree {
                    let (project, tree) = (&of.project, of.project.tree.as_str());
                    let rewritten: Vec<String> = (came.said.questions.iter())
                        .map(|q| q.name().to_string())
                        .chain(carried.questions.iter().map(|q| q.name.clone()))
                        .collect();
                    match crate::tree::unwritten(project, &place, &of.main, tree, &round.kept, &rewritten).await {
                        Ok(names) => (came.found).extend(names.into_iter().map(|name| Unmet::TreeUnchanged { name })),
                        Err(why) => {
                            return Ok(Carried::Failed(
                                why,
                                Failure::Other,
                                failure::Said::default(),
                                read.spent,
                            ));
                        }
                    }
                }
                // A return that writes a block whose lines name entries of the design tree is held to lines that each
                // name an entry the tree holds at the head of the work's branch, as the project's format of the tree
                // finds it. A look that cannot be made, a declaration that gives no format among it, fails the round.
                let naming: Vec<String> = (came.writes.iter())
                    .filter(|h| (flow.blocks.get(&h.block)).is_some_and(|b| b.names_tree_entries))
                    .map(|h| h.content.clone())
                    .collect();
                for names in &naming {
                    let looked = match &of.declaration.tree_format {
                        Some(format) => {
                            let (tree, head) = (of.project.tree.as_str(), place.reference());
                            crate::tree::unknown(&place.dir, tree, format, &head, names).await
                        }
                        None => Err(no_tree_format()),
                    };
                    match looked {
                        Ok(lines) => (came.found).extend(lines.into_iter().map(|line| Unmet::NotAnEntry { line })),
                        Err(why) => {
                            return Ok(Carried::Failed(
                                why,
                                Failure::Other,
                                failure::Said::default(),
                                read.spent,
                            ));
                        }
                    }
                }
                // Every return of a line that changes only the project's tests is held to what its run changed, one
                // that asks for calls as well: taken, it ends the run, and the run entered for the results begins on
                // the head it left.
                if round.cell.tests_only {
                    let tests = tests_only::paths(&of.declaration);
                    let outside = match &ended {
                        Ok(now) => {
                            let (began, now) = (began.as_deref(), now.as_deref());
                            tests_only::outside(&of.project, &place, &of.main, began, now, tests).await
                        }
                        Err(why) => Err(format!("the work's branch could not be read: {why}")),
                    };
                    match outside {
                        Ok(outside) => came
                            .found
                            .extend(outside.into_iter().map(|path| Unmet::NotATest { path })),
                        // Nothing after it holds the line to the tests: the round fails, and the work waits.
                        Err(why) => {
                            return Ok(Carried::Failed(
                                why,
                                Failure::Other,
                                failure::Said::default(),
                                read.spent,
                            ));
                        }
                    }
                }
                // A return that does the cell's work, of a line that may change the work, leaves nothing of the work
                // uncommitted: its agent is here to put it right, which nothing later can, since nothing runs a cell
                // for the worktree. A return that asks for calls is held to nothing: the run entered for the results
                // goes on in the same worktree. A look that cannot be made fails the round, as the look at the tests does.
                if !round.cell.reads_only && came.asks.is_empty() {
                    match uncommitted_in(&place).await {
                        Ok(found) => came.found.extend(found),
                        Err(why) => {
                            return Ok(Carried::Failed(
                                why,
                                Failure::Other,
                                failure::Said::default(),
                                read.spent,
                            ));
                        }
                    }
                }
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

    /// What the engine finds of the work's change under the paths the project protects, as the round ends, where its
    /// line may change the work, the return saying why the work changes each file where it does the cell's work
    /// (`given`) ([`protected`]): what it asks the owner, the early notice of what the check before the merge holds the
    /// work to, and each file the return is sent back for, as nothing says why. A look that fails is said on the
    /// standard error, asks nothing and sends nothing back, the check being what holds the merge.
    async fn protected(
        &self,
        round: &Round,
        place: &git::Place,
        of: &Of,
        given: Option<&BTreeMap<String, String>>,
    ) -> protected::Looked {
        if round.cell.reads_only {
            return protected::Looked::default();
        }
        let (paths, words) = (&of.declaration.protected.paths, &of.beside.asks.protected);
        match protected::look(&of.project, place, &of.main, paths, words, &round.kept, given).await {
            Ok(looked) => looked,
            Err(why) => {
                eprintln!(
                    "{}: the change under the protected paths could not be looked at: {why}",
                    round.work
                );
                protected::Looked::default()
            }
        }
    }

    /// The program's command line for a round on what it runs on, with its line's system prompt, of a line with these
    /// permissions, given the form of return of its flow, in the session, begun or gone on in, in the worktree, which
    /// the program compacts at the installation's `sessions.compact_at`, given the program's directory and the token of
    /// the account it runs under and the variables its project's declaration names; where `refusing` says so, with every use of a tool refused but the return's
    /// ([`cli::refusing`]), as a keep-warm ping runs.
    #[allow(clippy::too_many_arguments)]
    fn command(
        &self,
        dir: &Path,
        declared: &BTreeMap<String, String>,
        on: &RanOn,
        system: &str,
        permissions: &Permissions,
        form: &str,
        compacted: &Compacted,
        words: &Words,
        (session, begin): (Uuid, Begin),
        token: &Token,
        refusing: bool,
    ) -> Command {
        let agents = &self.settings.agents;
        let mut command = crate::programs::command(&agents.command);
        let compact = self.settings.sessions.compact_at;
        let args = cli::arguments(on, system, form, permissions, compacted, words, compact, session, begin);
        let args = if refusing { cli::refusing(args) } else { args };
        let profile = account::profile(&self.data);
        command
            .args(args)
            .env_clear()
            .envs(cli::environment(
                agents,
                &profile,
                crate::programs::engine().iter().cloned(),
                declared,
                token,
            ))
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
            eprintln!("main could not be taken into the branch of {}: {why}", round.work);
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

    /// Of a round given again, or stopped, after the process of the engine that gave it ended while a try of it ran:
    /// that try, as what it kept of its stream says, where its program said it began, reached the service or printed
    /// its result; none where the round kept no such try. What came of it is said to the loop, where `teller` is given,
    /// as any try's is, but for what the records hold of it already, told by the process that ended: each compaction of
    /// it as the session reopened, what its calls came to, where the stream kept the start of each, and that the
    /// service answered it. It is read whatever the settings now say of the round's item, its project or its level, and
    /// whether or not its worktree is there: what came before the session's messages that they no longer say is left
    /// out of its measures, and a try past the run guard's lines with no project to word why is read as any other.
    async fn left(&self, round: &Round, teller: Option<&Teller>) -> Result<Option<Left>, String> {
        let (flow, work) = (&*round.flow, &*round.kept);
        let base = self.kept(round);
        let Some((n, stream)) = left_try(&base).await else {
            return Ok(None);
        };
        let line = (round.cell.line.as_deref()).ok_or_else(|| format!("the cell {} names no line", round.cell.id))?;
        let of = crate::projects::of(&self.projects, &round.item).ok();
        // What the try ran on, as the records say it began on it or else as the try kept it, and the account it ran
        // under; and what its message filled, which the records lack where the process of the engine that began it
        // ended before it recorded it.
        let ran = (work.ran.iter()).rfind(|r| r.run == round.run && r.round == round.number && r.attempt == n);
        let entered = entered_of(&kept::entered_at(&base, n)).await;
        let ran_on = ran.map(|ran| &ran.on).or(entered.as_ref().map(|e| &e.on));
        let account = match ran_on {
            Some(on) => on.account.clone(),
            None => round.account.clone(),
        };
        let profile = account::profile(&self.data);
        let on = ran_on.cloned().or_else(|| {
            (self.settings)
                .ran_on(round.item.level.as_deref(), &round.cell, &profile, account.as_deref())
                .ok()
        });
        // What came before the session's messages, as the session kept it when it began.
        let InSession { number, .. } = cli::in_session(flow, work, line, round.run, round.number);
        let session = session::session(self.installation, round.work, line, number);
        let begun = |what| std::fs::read_to_string(system::kept_at(&self.data, session, what)).ok();
        let (system, form) = (begun(system::Begun::System), begun(system::Begun::Form));
        let mut before = match &on {
            Some(on) => before_messages(
                on,
                account.as_deref(),
                system.as_deref().unwrap_or_default(),
                form.as_deref().unwrap_or_default(),
            ),
            None => BTreeMap::new(),
        };
        for (part, there) in [("system", system.is_some()), ("schema", form.is_some())] {
            if !there {
                before.remove(part);
            }
        }
        // What the try was given, and how the run guard judges its calls.
        let told = tokio::fs::read_to_string(kept::at(&base, n).0)
            .await
            .unwrap_or_default();
        let returned = driver::history::last_return(flow, work, line, round.run, round.number).map_or(0, chars_of);
        let given = told.chars().count() as u64 + returned;
        let last = driver::history::last_prompt(flow, work, line, round.run, round.number);
        let waived = let_run(work, round.run, round.number, Failure::Guarded);
        let mut guard = Guard::new(round.lines.clone(), last, given, waived);
        let crossing = guard.judge(&stream.own);
        // What the records hold of it already: the compactions told since it began, and its measures.
        let since = ran
            .map(|r| r.at)
            .or_else(|| work::sessions::began_at(work, round.run, round.number));
        let compacted = (work::sessions::of_line(work, line))
            .filter(|s| s.why == Why::Compacted && since.is_some_and(|at| at < s.at))
            .count();
        let compactions = marked(&base.with_extension("compacted")).await;
        let more = compactions.saturating_sub(u32::try_from(compacted).unwrap_or(u32::MAX));
        let measured = (work.measured.iter()).any(|m| m.run == round.run && m.round == round.number && m.attempt == n);
        // A stream kept before the start of each call was kept with it tells the calls' cost and not their prompts:
        // its measures are not known, and none are said, which no record would tell from a try that made no call.
        let starts = stream.own.count > 0 || stream.took().is_none();
        if !starts {
            eprintln!(
                "{}: the try {n} of the round {} of run {} kept no start of its calls: what they came to is not \
                 recorded",
                round.work, round.number, round.run.0
            );
        }
        let priced = stream.estimated();
        let measures = (!measured && starts)
            .then(|| measures(&stream, given, before, crossing.is_some(), priced))
            .flatten();
        let filled = (work.filled.iter()).any(|f| f.run == round.run && f.round == round.number && f.attempt == n);
        // The service served the round: a wait every work shares need not wait for its end.
        if let Some(teller) = teller {
            if let Some(entered) = entered {
                if ran.is_none() {
                    teller.ran_on(n, entered.on);
                }
                if !filled {
                    teller.filled(n, entered.fills);
                }
            }
            tell_try(teller, line, n, more, measures);
            if stream.answered() {
                teller.answered();
            }
        }
        let _ = tokio::fs::remove_file(base.with_extension("again.md")).await;
        // A try past a signal the run guard stops a round on, which printed no result, failed for it, as the engine
        // that ended would have stopped it.
        let guarded_before = stopped_before(work, round.run, round.number, Failure::Guarded);
        let halted = crossing.is_some() && guard.stops() && !stream.has_result();
        let guarded = (crossing.filter(|_| halted).zip(of))
            .map(|(crossing, of)| guarded(&crossing, &of.beside.cost.guard, &stream, compactions, guarded_before));
        Ok(Some(Left {
            stream,
            compactions,
            guarded,
        }))
    }

    /// What a round stopped with nothing carrying it out took: what its last try took, as what it kept of its stream
    /// says ([`Inner::left`]); nothing where it kept none. Where `teller` is given, as for a round this process never
    /// gave, what came of the try is told as well; a round this process gave had it told by its carrying, which has
    /// ended. What cannot be read is said on the standard error, and taken as nothing.
    async fn left_spent(&self, round: &Round, teller: Option<&Teller>) -> Spent {
        match self.left(round, teller).await {
            Ok(Some(left)) => spent_of(left),
            Ok(None) => Spent::default(),
            Err(why) => {
                eprintln!("{}: what the stopped round's try took was not read: {why}", round.work);
                Spent::default()
            }
        }
    }

    /// Where what the round was told and returned are kept, without the extension.
    fn kept(&self, round: &Round) -> PathBuf {
        kept::base(&self.data, round.work, round.run, round.number)
    }

    /// Puts what the owner said where the round's hook gives it to the round's session after its program's next use
    /// of a tool, in the words of `said` in the message's file; and keeps which versions wait there, and which the
    /// hook gave. Once the round's program has ended, nothing is put: the words stay new to the cell, which runs again
    /// for them. What cannot be put is said on the standard error, and the words stay new to the cell all the same.
    async fn tell(&self, round: &Round, told: Telling) {
        let failed = |why: String| eprintln!("{}: what the owner said was not handed to its round: {why}", round.work);
        let said = match crate::projects::of(&self.projects, &round.item) {
            Ok(of) => said(&of.beside, &told),
            Err(why) => return failed(why),
        };
        let key = (round.work, round.run, round.number);
        let Some(state) = (self.said.lock().expect("the tellings' lock")).get(&key).cloned() else {
            return;
        };
        let state = state.lock().await;
        if state.ended {
            return;
        }
        if let Err(why) = words(&self.kept(round)).put(&said, &told.versions).await {
            failed(why);
        }
    }

    /// As the round's program has ended, takes back what waits for its hook, and says what the hook gave the round, as
    /// its files of words keep it: those versions are put into it. What waited and was never given stays new to the
    /// cell. The files are read the same whichever process of the engine put the words, so that words the hook gave a
    /// try that a process that ended left are the round's too, and are not given again.
    async fn heard(&self, round: &Round, teller: &Teller) {
        let key = (round.work, round.run, round.number);
        let Some(state) = (self.said.lock().expect("the tellings' lock")).remove(&key) else {
            return;
        };
        let mut state = state.lock().await;
        state.ended = true;
        self.hook_gave(round, teller).await;
    }

    /// Takes back what waits for the round's hook, whose program has ended, and says what the hook gave the round, as
    /// its files of words keep it: those versions are put into it, and what waited and was never given stays new to the
    /// cell. It reads only the files, so that a round this process gave and one a process that ended left, given again
    /// or stopped, are read alike.
    async fn hook_gave(&self, round: &Round, teller: &Teller) {
        let words = words(&self.kept(round));
        let unread = |why: String| {
            eprintln!(
                "{}: what the round's hook was given could not be read: {why}",
                round.work
            );
        };
        if let Err(why) = words.take_back().await {
            unread(why);
        }
        let (given, unknown) = match words.given().await {
            Ok(given) => given,
            Err(why) => return unread(why),
        };
        for at in unknown {
            eprintln!(
                "{}: {} keeps no versions of the owner's words it gave, which stay new to the cell",
                round.work,
                at.display()
            );
        }
        if !given.is_empty() {
            teller.said_to_round(given);
        }
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
        usd += (work.spent_beside_rounds().iter())
            .filter(|call| ended(call.at))
            .map(|call| call.spent.usd_micros)
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

    /// Why the work is to wait for the owner, where the round took it over what it may spend: everything its rounds
    /// have spent, this one with them, is more than the bar its budget sets it, which each letting-on after a pause
    /// for the budget raises (`driver::budget`); in the project's words, which a work whose project the settings no
    /// longer have, and whose rounds do not run, has none of. The words say the bar, which is what the work may spend
    /// and what the owner reads beside what it has spent, and what letting the work on allows it beyond the bar, which
    /// is what the owner's button buys.
    fn over_budget(&self, round: &Round, spent: &Spent) -> Option<String> {
        let budget = self.settings.budget()?;
        let bar = budget.bar(&round.kept);
        // No other round of the work ended while this one was under way: a work has one round under way at a time. The
        // pings of its other lines that ended meanwhile are not in what the round was given, and count at the next
        // round's judgment: at most each other session's `warm.most` pings.
        let beside = (round.kept.spent_beside_rounds().iter())
            .map(|call| call.spent.usd_micros)
            .sum::<u64>();
        let total = round.kept.spent.values().map(|s| s.usd_micros).sum::<u64>() + spent.usd_micros + beside;
        let dollars = |micros: u64| format!("{:.2}", micros as f64 / 1_000_000.0);
        let of = crate::projects::of(&self.projects, &round.item).ok()?;
        (total > bar).then(|| {
            let (value, bar, more) = (dollars(total), dollars(bar), dollars(budget.let_on));
            let places = [
                ("value", value.as_str()),
                ("budget", bar.as_str()),
                ("more", more.as_str()),
            ];
            flow::agents::fill(&of.beside.cost.work.budget, &places)
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

/// What comes before a session's messages in the prompt of a call on this, under this account, with this system
/// prompt and form of return, which the cache reads only where it is what the call before had: the model, the
/// account, whose cache is its own, by its name, left out where there is none, and a digest each of the system
/// prompt and the schema ([`work::Measures::before`]). What the program says it gave the model, its version, tools
/// and agents, is added from its stream ([`measures`]); the effort is no part of it, a change of it alone having
/// missed no cache (research/cache-miss-causes.md).
fn before_messages(on: &RanOn, account: Option<&str>, system: &str, form: &str) -> BTreeMap<String, String> {
    let mut before = BTreeMap::from([
        ("model".to_string(), on.model.clone()),
        ("system".to_string(), work::digest(system)),
        ("schema".to_string(), work::digest(form)),
    ]);
    if let Some(account) = account {
        before.insert("account".to_string(), account.to_string());
    }
    before
}

impl Inner {
    /// The lock of the line of the work, made where it has none.
    fn lock(&self, work: WorkNumber, line: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.in_work.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        locks.entry((work, line.to_string())).or_default().clone()
    }

    /// Sends the session of the ping its keep-warm message, with the command line of the session's last round, which
    /// the cache keeps the session under, and gives what it took; or why it failed, with what it took. A session whose
    /// line has a round under way is not pinged, and a ping that has not ended by the time the cache lets the session
    /// go is stopped, since after that it would write the session again for nothing. The program is let use no tool, so
    /// that it changes nothing in the worktree beside the rounds of the work's other lines. What the service said of
    /// the usage of the account it ran under is put in `usage`, and how many processes its program left running in
    /// its group as it ended, which were stopped, in `left_running`.
    async fn ping(
        &self,
        ping: &Ping,
        usage: &mut Option<driver::Said>,
        left_running: &mut usize,
    ) -> Result<Spent, (String, Spent)> {
        let failed = |why: String| (why, Spent::default());
        let of = crate::projects::of(&self.projects, &ping.item).map_err(failed)?;
        let said = &of.beside.cost.ping;
        let fill = flow::agents::fill;
        let (flow, work, line) = (&*ping.flow, &*ping.kept, ping.line.as_str());
        let lock = self.lock(ping.work, line);
        let Ok(_one) = lock.try_lock() else {
            return Err(failed(said.under_way.clone()));
        };
        // A work that waits is on the version installed, whose flow places its cells.
        let line_of = |cell: &CellId| (flow.cells.iter().find(|c| c.id == *cell)).and_then(|c| c.line.clone());
        let reads_only = flow.lines.get(line).is_some_and(|l| l.reads_only);
        let made = self
            .session_line(&line_of, reads_only, work, ping.work, line, ping.session, of)
            .map_err(|no| match no {
                NoLine::Round => failed(fill(&said.no_round, &[("line", line)])),
                NoLine::System(why) => failed(fill(&said.no_system, &[("why", &why)])),
            })?;
        let SessionLine {
            session,
            run,
            number,
            on,
            system,
            form,
            permissions,
        } = made;
        // The account it was handed out with, which its session's last call ran under, whose cache holds the session.
        let token = account::token_of(&self.tokens, ping.account.as_deref(), &of.beside.cost.check.no_account)
            .await
            .map_err(failed)?;
        let place = git::place(&self.data, &git::tag(&self.installation), ping.work);
        let dir = git::worktree(&place).map_err(failed)?;
        let base =
            kept::base(&self.data, ping.work, run, number).with_file_name(format!("ping-{line}-{}", ping.session));
        let compacted = Compacted {
            marks: base.with_extension("compacted"),
            again: base.with_extension("again.md"),
        };
        let words = Words {
            waiting: base.with_extension("words.json"),
            given: base.with_extension("given"),
        };
        let _ = tokio::fs::remove_file(&compacted.marks).await;
        // A ping's compaction gives the session nothing again: its next round is filled from the top.
        write(&compacted.again, b"").await.map_err(failed)?;
        let command = self.command(
            &dir,
            &of.declaration.environment,
            &on,
            &system,
            &permissions,
            &form,
            &compacted,
            &words,
            (session, Begin::Resume),
            &token,
            true,
        );
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
        *usage = stream.usage().cloned();
        let marks = marked(&compacted.marks).await;
        let _ = tokio::fs::remove_file(&compacted.marks).await;
        let _ = tokio::fs::remove_file(&compacted.again).await;
        let priced = || Spent {
            compactions: marks,
            ..stream.took().unwrap_or_default()
        };
        let Ran::Ended {
            status,
            stderr,
            left: stopped,
            ..
        } = ran.map_err(failed)?
        else {
            let minutes = (left / 60_000).to_string();
            return Err((fill(&said.stopped, &[("minutes", &minutes)]), priced()));
        };
        *left_running = stopped;
        let spent = cli::took(&lines).unwrap_or_else(priced);
        let spent = Spent {
            compactions: marks,
            ..spent
        };
        let spent_any = spent.uncached_tokens + spent.cached_tokens + spent.output_tokens > 0;
        if status.success() || spent_any {
            return Ok(spent);
        }
        let end = process::tail(String::from_utf8_lossy(&stderr).trim().as_bytes());
        let why = process::ended(&of.beside.cost.program, &self.settings.agents.command, status, &end);
        Err((why, spent))
    }
}

/// The command line of a session's last round that ran, which the cache keeps the session under: what a keep-warm ping
/// and a fork of the session are sent with. The round, what it ran on, what the session began with as its rounds are
/// given it, and the permissions of the round's line.
struct SessionLine {
    session: Uuid,
    run: RunId,
    number: usize,
    on: RanOn,
    system: String,
    form: String,
    permissions: Permissions,
}

/// Why a session's command line cannot be made: no round of it ran, or what it began with was not kept, with why.
enum NoLine {
    Round,
    System(String),
}

impl Inner {
    /// The command line of the session of this number of the line in the work, as [`SessionLine`] says: the line of
    /// each run's cell as `line_of` tells it, of the version the work is on, and whether the line's agents only read.
    #[allow(clippy::too_many_arguments)]
    fn session_line(
        &self,
        line_of: &dyn Fn(&CellId) -> Option<String>,
        reads_only: bool,
        work: &Work,
        number: WorkNumber,
        line: &str,
        of_session: u32,
        of: &Of,
    ) -> Result<SessionLine, NoLine> {
        let session = session::session(self.installation, number, line, of_session);
        let line_of_run = |run: RunId| {
            let cell = work.records.started.iter().find(|s| s.run == run).map(|s| &s.cell);
            cell.and_then(line_of)
        };
        let in_session = |r: &&work::Ran| {
            let at = work::sessions::start(work, r.run, r.round);
            line_of_run(r.run).as_deref() == Some(line) && work::sessions::number_at(work, line, at) == of_session
        };
        let last = (work.ran.iter().rev()).find(in_session).ok_or(NoLine::Round)?;
        let began = |what| {
            let at = system::kept_at(&self.data, session, what);
            std::fs::read_to_string(&at)
                .map_err(|e| NoLine::System(format!("{}: {e}", at.file_name().unwrap_or_default().to_string_lossy())))
        };
        let refused = permissions::refused_commands(&of.beside, &of.declaration, &self.settings.agents.command);
        Ok(SessionLine {
            session,
            run: last.run,
            number: last.round,
            on: last.on.clone(),
            system: began(system::Begun::System)?,
            form: began(system::Begun::Form)?,
            permissions: Permissions::of(reads_only, &of.beside.allowed.may, &std::env::temp_dir(), &refused),
        })
    }

    /// Asks a fork of the session the fork's round ran in its question ([`Rounds::fork`]): on the command line of that
    /// session's last round, forked ([`cli::forked`]), begun at the round's end where the session went on past it, or
    /// refused where it cannot be; in the work's worktree, or, where the work has ended and its worktree is gone, a
    /// checkout of its kept branch made at the worktree's place and taken away after; holding the session's line, so
    /// that no round or ping of the line runs beside the fork, which refuses every tool but the return and so leaves
    /// what the work's other lines run in the worktree as it is. What the fork was told, its stream and its answer are
    /// kept beside the round's, its hooks' files are its own and taken away after.
    async fn fork(&self, fork: &Fork) -> Result<Forked, ForkFailed> {
        let of = crate::projects::of(&self.projects, &fork.item)?;
        let (outline, work) = (&*fork.outline, &*fork.kept);
        // The work's rounds are placed in their sessions as the version it is on has its cells, whichever is installed.
        let line_of = |cell: &CellId| outline.cell(cell).and_then(|c| c.line.clone());
        let started = (work.records.started.iter()).find(|s| s.run == fork.run);
        let line = (started.and_then(|s| line_of(&s.cell))).ok_or("The round's cell runs in no session.")?;
        let Ok(one) = self.lock(fork.work, &line).try_lock_owned() else {
            return Err(format!(
                "A round or a ping of the line {line} of {} is under way: ask the fork once it has ended.",
                fork.work
            )
            .into());
        };
        // Held until the fork has ended, and what it stood in is taken away.
        let mut holding = Some(one);
        let reads_only = (outline.lines.iter())
            .find(|l| l.name == line)
            .and_then(|l| l.reads_only)
            .ok_or("The version of the flow the work is on does not say whether the line's agents only read.")?;
        let of_session = work::sessions::number_at(work, &line, work::sessions::start(work, fork.run, fork.round));
        let made = self
            .session_line(&line_of, reads_only, work, fork.work, &line, of_session, of)
            .map_err(|no| match no {
                NoLine::Round => "No round of the session ran.".to_string(),
                NoLine::System(why) => format!("What the session began with was not kept: {why}."),
            })?;
        // The later rounds of the session, which the fork leaves out, beginning at the end of the round asked of.
        let asked_at = work::sessions::start(work, fork.run, fork.round);
        let mut later: Vec<(RunId, usize)> = Vec::new();
        for r in work.ran.iter() {
            let at = work::sessions::start(work, r.run, r.round);
            let of_line = (work.records.started.iter())
                .find(|s| s.run == r.run)
                .and_then(|s| line_of(&s.cell))
                .is_some_and(|of| of == line);
            let same = of_line && work::sessions::number_at(work, &line, at) == of_session;
            if same && at > asked_at && !later.contains(&(r.run, r.round)) {
                later.push((r.run, r.round));
            }
        }
        // Where the first of them made its first call, which read what the round left from the cache.
        let until = (later.iter())
            .map(|(run, round)| work::sessions::first_try_at(work, *run, *round))
            .min();
        let end_of = |run: RunId, number: usize| {
            (work.records.returned.iter())
                .filter(|r| r.run == run)
                .nth(number.saturating_sub(1))
                .map(|e| e.at)
        };
        let asked_end = end_of(fork.run, fork.round);
        // Where the session went on past the round, the fork begins at the round's last message; what keeps it from
        // that is found before anything is written.
        let begins = match until {
            None => None,
            Some(_) => {
                let after = asked_end.unwrap_or(asked_at);
                let reopened = (work::sessions::of_line(work, &line))
                    .filter(|s| matches!(s.why, Why::Compacted | Why::Lost) && s.at > after)
                    .find(|s| work::sessions::number_at(work, &line, s.at) == of_session);
                if let Some(reopened) = reopened {
                    let (run, round) = (fork.run.0, fork.round);
                    let how = match reopened.why {
                        Why::Lost => {
                            format!("begun again after run {run} round {round}, the agents' program having lost it")
                        }
                        _ => format!(
                            "compacted after run {run} round {round}, and holds only a summary of what came before"
                        ),
                    };
                    return Err(format!("The session was {how}: no fork can begin at that round's end.").into());
                }
                let base = kept::base(&self.data, fork.work, fork.run, fork.round);
                Some(last_message_of(&base).await?.ok_or(
                    "The session went on past the round, and no stream of the round's tries names a message of it: no \
                     fork can begin at its end.",
                )?)
            }
        };
        // The account the session's last try ran under, whose cache holds it; else the one in use, which it says: that
        // one was removed, or the try ran under none. With none in use, no token can be given and nothing is run.
        let (account, instead) = fork.accounts.of_session(made.on.account.as_deref());
        let token = account::token_of(&self.tokens, account.as_deref(), &of.beside.cost.check.no_account).await?;
        let place = git::place(&self.data, &git::tag(&self.installation), fork.work);
        if !place.dir.exists() && !fork.ended {
            return Err(format!("{} has not ended, and its worktree is not there.", fork.work).into());
        }
        // What stands in for an ended work's worktree, which is let go, is a checkout
        // with no branch, taken away after: one made now, or one an earlier fork could not take away. The work's own
        // worktree, on its branch, is left to the work's end.
        let standing = if fork.ended {
            git::configuration(&of.project, &place).await?;
            !place.dir.exists() || !git::said(&place.dir, &["symbolic-ref", "--quiet", "HEAD"]).await?.ok
        } else {
            false
        };
        let mut stand_in = standing.then(|| StandIn {
            project: of.project.clone(),
            place: place.clone(),
            work: fork.work,
            holding: holding.take(),
            gone: false,
        });
        if !place.dir.exists() {
            // A worktree whose directory was taken away is named by the repository until it is pruned.
            git::git(&of.project.repo, &["worktree", "prune"]).await?;
            let dir = place.dir.to_string_lossy().to_string();
            let added = git::git(
                &of.project.repo,
                &["worktree", "add", "--detach", &dir, &place.reference()],
            )
            .await;
            added.map_err(|why| format!("The work's worktree is gone, and its branch could not stand in: {why}"))?;
        }
        let asked = self.forked(fork, &made, begins.as_deref(), (&place.dir, &token)).await;
        if let Some(stand_in) = stand_in.as_mut() {
            stand_in.take_away().await;
        }
        let (answer, text, spent, read, kept, usage, left) = asked?;
        let last = match until {
            Some(_) => (fork.run, fork.round),
            None => (made.run, made.number),
        };
        let held = (work.spent.get(&last)).and_then(|s| s.context_tokens);
        // The last call that read from the cache what the fork begins with, which the cache keeps for its life after.
        // At the session's end, its last round's end or a keep-warm ping since. At an earlier round's end, the first
        // call after that round, a ping or the next round's start, which read what the round left and wrote the cache
        // on past it, so that no later call read that again; where that call came after the cache had let it go, the
        // round's end.
        let time = |at: Seq| work.time_of(at).map(|t| t.0);
        let life = self.settings.cache_ms();
        let pings: Vec<Seq> = (work.pinged.iter())
            .filter(|p| p.line == line && p.session == of_session && p.failed.is_none())
            .map(|p| p.at)
            .collect();
        let when = match until {
            None => end_of(made.run, made.number)
                .max(pings.iter().copied().max())
                .and_then(time),
            Some(until) => {
                let next = (pings.iter().copied())
                    .filter(|at| asked_end.is_some_and(|end| *at > end) && *at < until)
                    .min()
                    .unwrap_or(until);
                let (ended, next) = (asked_end.and_then(time), time(next));
                match (ended, next, life) {
                    (Some(ended), Some(next), Some(life)) if next.saturating_sub(ended) <= life => Some(next),
                    _ => ended,
                }
            }
        };
        let now = self.clock.now().ok().map(|t| t.0);
        let expired = match (when, now, life) {
            (Some(when), Some(now), Some(life)) if now.saturating_sub(when) > life => {
                Some((now - when - life) / 60_000)
            }
            _ => None,
        };
        Ok(Forked {
            answer,
            text,
            spent,
            read,
            held,
            expired,
            round: (fork.run, fork.round),
            later,
            kept: kept.display().to_string(),
            account: account.unwrap_or_default(),
            instead,
            usage,
            left,
        })
    }

    /// Runs the fork's program in this directory on the session's command line, forked, begun at the message `begins`
    /// names where it names one, given the token of the account it runs under, keeping what it was told, its stream and
    /// its answer beside the round's; gives its return, what its program said at its end, what it cost, what its first
    /// call read from the cache, the file its answer is kept in, and what its stream said of the usage.
    async fn forked(
        &self,
        fork: &Fork,
        made: &SessionLine,
        begins: Option<&str>,
        (dir, token): (&Path, &Token),
    ) -> Result<Forking, ForkFailed> {
        let base = kept::base(&self.data, fork.work, fork.run, fork.round);
        let name = base.file_name().unwrap_or_default().to_string_lossy().to_string();
        let n = (1..)
            .find(|n| !base.with_file_name(format!("{name}.fork.{n}.message.md")).exists())
            .unwrap_or(1);
        let at = |what: &str| base.with_file_name(format!("{name}.fork.{n}.{what}"));
        if let Some(parent) = base.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let compacted = Compacted {
            marks: at("compacted"),
            again: at("again.md"),
        };
        let words = Words {
            waiting: at("words.json"),
            given: at("given"),
        };
        // A fork's compaction gives the session nothing again.
        write(&compacted.again, b"").await?;
        write(&at("message.md"), fork.question.as_bytes()).await?;
        let agents = &self.settings.agents;
        let round = cli::arguments(
            &made.on,
            &made.system,
            &made.form,
            &made.permissions,
            &compacted,
            &words,
            self.settings.sessions.compact_at,
            made.session,
            Begin::Resume,
        );
        let of = crate::projects::of(&self.projects, &fork.item)?;
        let mut command = crate::programs::command(&agents.command);
        let profile = account::profile(&self.data);
        command
            .args(cli::forked(round, begins))
            .env_clear()
            .envs(cli::environment(
                agents,
                &profile,
                crate::programs::engine().iter().cloned(),
                &of.declaration.environment,
                token,
            ))
            .current_dir(dir);
        let mut stream = cli::Stream::default();
        let mut lines = Vec::new();
        let mut each = |line: &[u8]| {
            stream.line(line);
            lines.extend_from_slice(line);
            lines.push(b'\n');
        };
        let mut never = crate::held::Stop::never();
        let ran = process::streamed(
            command,
            Some(cli::fork_stdin(&of.beside.cost.fork.first, &fork.question)),
            None,
            &mut never,
            None,
            &mut each,
        )
        .await;
        for file in [&compacted.marks, &compacted.again, &words.waiting] {
            let _ = tokio::fs::remove_file(file).await;
        }
        write(&at("stream.jsonl"), &lines).await?;
        let Ran::Ended {
            status, stderr, left, ..
        } = ran?
        else {
            return Err(ForkFailed {
                why: "The fork's program was stopped before it ended.".to_string(),
                spent: stream.took(),
                left: 0,
            });
        };
        // The question is the second message: a stream that holds one result holds only the first message's return,
        // which is no answer to it.
        if stream.results() < 2 {
            let mut why = format!(
                "The fork's question was not answered: its program ended {} of the fork's 2 messages",
                stream.results()
            );
            if !status.success() {
                let end = process::tail(String::from_utf8_lossy(&stderr).trim().as_bytes());
                let ended = process::ended(&of.beside.cost.program, &self.settings.agents.command, status, &end);
                why = format!("{why}; {ended}");
            }
            return Err(ForkFailed {
                why: format!("{why}."),
                spent: stream.took(),
                left,
            });
        }
        let read = stream.own.first.map(|(read, _)| read);
        let text = (String::from_utf8_lossy(&lines).lines())
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .rev()
            .find(|v| v["type"] == "result")
            .and_then(|v| v["result"].as_str().map(str::to_string))
            .unwrap_or_default();
        let of = crate::projects::of(&self.projects, &fork.item)?;
        let (answer, spent) = match cli::read(&lines, &of.beside.cost.program) {
            Ok(read) => (Some(read.structured.to_string()), read.spent),
            // Neither a return nor words: why is what the stream and the program's end say.
            Err(no) if text.is_empty() => {
                // What the stream says is a sentence of the words beside the flows, which ends in a stop: the program's
                // end is joined to it after a semicolon and the whole is made a sentence once.
                let mut why = no.why.trim_end_matches('.').to_string();
                if !status.success() {
                    let end = process::tail(String::from_utf8_lossy(&stderr).trim().as_bytes());
                    let ended = process::ended(&of.beside.cost.program, &self.settings.agents.command, status, &end);
                    why = format!("{why}; {ended}");
                }
                return Err(ForkFailed {
                    why: flow::agents::sentence(&format!("The fork wrote no answer: {why}")),
                    spent: no.spent.or_else(|| stream.took()),
                    left,
                });
            }
            Err(no) => (None, no.spent.unwrap_or_else(|| stream.took().unwrap_or_default())),
        };
        let kept = answer.clone().unwrap_or_else(|| text.clone());
        write(&at("answer"), kept.as_bytes()).await?;
        Ok((answer, text, spent, read, at("answer"), stream.usage().cloned(), left))
    }
}

/// What a fork's program came to: its return, what it said at its end, what it cost, what its first call read from the
/// cache, the file its answer is kept in, what its stream said of the usage, where it said anything, and how many
/// processes the program left running in its group as it ended, which were stopped.
type Forking = (
    Option<String>,
    String,
    Spent,
    Option<u64>,
    PathBuf,
    Option<driver::Said>,
    usize,
);

/// A checkout at an ended work's worktree's place that a fork ran in: taken away once the fork is done with it, or,
/// where the fork is given up before, as it is dropped; the work held until it is.
struct StandIn {
    project: Project,
    place: git::Place,
    work: WorkNumber,
    holding: Option<tokio::sync::OwnedMutexGuard<()>>,
    gone: bool,
}

impl StandIn {
    /// Takes the checkout away, then lets the work go; given up halfway, its drop takes it away again.
    async fn take_away(&mut self) {
        removed(&self.project, &self.place, self.work).await;
        self.gone = true;
        self.holding = None;
    }
}

impl Drop for StandIn {
    fn drop(&mut self) {
        if self.gone {
            return;
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let (project, place, work, holding) =
            (self.project.clone(), self.place.clone(), self.work, self.holding.take());
        runtime.spawn(async move {
            removed(&project, &place, work).await;
            drop(holding);
        });
    }
}

/// What of the work the worktree holds uncommitted as a round of a line that may change the work ends: a merge begun
/// and not committed, with what git says of it, or else the files git tracks that are changed and not committed. A
/// file git does not track is no change of the work: it is in no commit, whoever left it, and holds nothing back.
async fn uncommitted_in(place: &git::Place) -> Result<Vec<Unmet>, String> {
    let dir = git::worktree(place)?;
    let changes = git::uncommitted(&dir).await?;
    if git::merging(&dir).await? {
        return Ok(vec![Unmet::Merging { changes }]);
    }
    if changes.is_empty() {
        return Ok(Vec::new());
    }
    Ok(vec![Unmet::Uncommitted { changes }])
}

/// Takes away the checkout at the place a fork of the work ran in, saying where it cannot; one already gone is passed.
async fn removed(project: &Project, place: &git::Place, work: WorkNumber) {
    if let Err(why) = git::remove(project, place).await {
        eprintln!("The checkout {work} was forked in could not be taken away: {why}");
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
fn resets(failure: Failure, report: &str, said: &failure::Said, clock: &dyn Clock) -> Option<work::Time> {
    if failure != Failure::UsageLimit {
        return None;
    }
    let now = clock.now().ok()?;
    failure::resets(report, said, now.0, &failure::zone_offset).map(work::Time)
}

/// A round that failed, with the reason in the engine's words, `why`, and what the program itself reported of it,
/// `report`: the kind of the failure, and where that is the usage limit when it resets, are told by the report and by
/// what the program's stream said in its fields, and by none of the engine's words. The time is kept in what the stream
/// said (`resets`), as the failure's own.
fn failed_round(
    why: String,
    report: &str,
    mut said: failure::Said,
    spent: Option<Spent>,
    clock: &dyn Clock,
) -> Carried {
    let kind = failure::of(report, &said, spent.as_ref());
    if kind == Failure::UsageLimit {
        said.resets = resets(kind, report, &said, clock).map(|time| time.0);
    }
    Carried::Failed(why, kind, said, spent.unwrap_or_default())
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

/// What a try runs on and what its message fills into its session, as it keeps them beside its message before its
/// program is begun ([`kept::entered_at`]).
#[derive(Serialize, Deserialize)]
struct Entered {
    on: RanOn,
    fills: Vec<Fill>,
}

/// Keeps what the try runs on and fills at `at`, whole or not at all, so that a stop in the middle leaves no file a
/// try's reading takes for what it entered.
async fn keep_entered(at: &Path, entered: &Entered) -> Result<(), String> {
    let kept = serde_json::to_vec(entered).map_err(|e| format!("{}: {e}", at.display()))?;
    let writing = at.with_extension("json.part");
    write(&writing, &kept).await?;
    tokio::fs::rename(&writing, at)
        .await
        .map_err(|e| format!("{}: {e}", at.display()))
}

/// What the try whose file this is kept of what it ran on and filled; none where it kept none, as a try kept before
/// tries kept it. A file that cannot be read is said on the standard error, and taken as none.
async fn entered_of(at: &Path) -> Option<Entered> {
    let bytes = match tokio::fs::read(at).await {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            eprintln!("{} cannot be read: {e}", at.display());
            return None;
        }
    };
    match serde_json::from_slice(&bytes) {
        Ok(entered) => Some(entered),
        Err(e) => {
            eprintln!("{} is not what a try entered: {e}", at.display());
            None
        }
    }
}

/// Keeps what the try of this number of the round is told, at `at`: where an earlier try of the round was told the
/// same, as a second name of the file that try keeps it in, so that a message given again is stored once and found
/// as every try's is; otherwise, and where the file system cannot give a file a second name, in a file of its own.
async fn keep_told(base: &Path, n: usize, at: &Path, told: &str) -> Result<(), String> {
    for k in (1..n).rev() {
        let earlier = kept::at(base, k).0;
        let same = match tokio::fs::metadata(&earlier).await {
            Ok(meta) if meta.len() == told.len() as u64 => {
                (tokio::fs::read(&earlier).await).is_ok_and(|kept| kept == told.as_bytes())
            }
            _ => false,
        };
        if !same {
            continue;
        }
        match tokio::fs::hard_link(&earlier, at).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                eprintln!(
                    "{} is kept whole, as it could not be a second name of {}: {e}",
                    at.display(),
                    earlier.display()
                );
                break;
            }
        }
    }
    write(at, told.as_bytes()).await
}

/// A file that is removed as this is dropped, however the scope that holds it is left.
struct Removed(PathBuf);

impl Drop for Removed {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
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

    /// What the try's calls came to ([`measures`]).
    fn measures(&self, given: u64, before: BTreeMap<String, String>, crossed: bool, priced: bool) -> Option<Measures> {
        measures(&self.stream, given, before, crossed, priced)
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

/// What a try's calls came to, as its stream told them, given a message of this many characters, with what came before
/// the session's messages and the program's version, tools and agents as its stream told them, whether it went past the
/// run guard's lines, and whether what it took is priced from its calls; none where its program printed nothing.
fn measures(
    stream: &cli::Stream,
    given: u64,
    mut before: BTreeMap<String, String>,
    crossed: bool,
    priced: bool,
) -> Option<Measures> {
    if stream.is_empty() {
        return None;
    }
    if let Some(program) = &stream.program {
        before.insert("program".to_string(), program.clone());
    }
    // What the program says it gave the model, which it may change with the engine's arguments the same: the cache
    // reads the tools before anything else in the prompt, in the order given.
    for (part, names) in [
        ("program_tools", &stream.program_tools),
        ("program_agents", &stream.program_agents),
    ] {
        if let Some(names) = names {
            before.insert(part.to_string(), work::digest(&names.join("\n")));
        }
    }
    let calls = &stream.own;
    // The program's own count where it printed its result; what the stream told of the agent's calls where not, as of
    // a try that was stopped.
    let (fresh, read, written, thinking, turns) =
        (stream.printed()).unwrap_or((calls.fresh, calls.read, calls.written, None, u64::from(calls.count)));
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
        received: stream.began(),
        crossed,
        priced,
    })
}

/// How a round's program is begun: going on from the try a process of the engine that ended left, or anew, given the
/// token of the account the round was handed out with, by its name.
enum Begun {
    Left(Box<Left>),
    Anew(Token, String, RanOn),
}

impl Begun {
    /// The try left, where the round goes on from one.
    fn left(self) -> Option<Left> {
        match self {
            Begun::Left(left) => Some(*left),
            Begun::Anew(..) => None,
        }
    }
}

/// A try of a round that the process of the engine that gave it left as it ended ([`Inner::left`]).
struct Left {
    /// Its program's stream, read from what the try kept of it.
    stream: cli::Stream,
    /// How many times the program compacted the session during it.
    compactions: u32,
    /// Where it went past a signal the run guard stops a round on and printed no result, the round's failure for it.
    guarded: Option<Carried>,
}

/// Says to the loop what came of the try of this number: each of these compactions of the session during it as the
/// session reopened, while the round is still under way, and what its calls came to, where they are said.
fn tell_try(teller: &Teller, line: &str, n: usize, compactions: u32, measures: Option<Measures>) {
    for _ in 0..compactions {
        teller.session_began(line.to_string(), Why::Compacted);
    }
    if let Some(measures) = measures {
        teller.measured(n, measures);
    }
}

/// Why a try the run guard stopped failed, in the words beside the flows, with the last tools it used, `again` where
/// the guard stopped the run's round before it too; and what it took: its calls at their models' prices, with its
/// compactions.
fn guarded(
    crossing: &Crossing,
    words: &flow::agents::GuardWords,
    stream: &cli::Stream,
    compactions: u32,
    again: bool,
) -> Carried {
    let why = crossing.words(words, &stream.tools, again);
    let spent = Spent {
        compactions,
        ..stream.took().unwrap_or_default()
    };
    Carried::Failed(why, Failure::Guarded, failure::Said::default(), spent)
}

/// What a try the process of the engine that gave it left took, as what it kept of its stream says: what its result
/// says, where it printed one, or else its calls at their models' prices; with its compactions.
fn spent_of(left: Left) -> Spent {
    let spent = left.stream.what_it_took().unwrap_or_default();
    Spent {
        compactions: left.compactions,
        ..spent
    }
}

/// A round that fails, for this reason, before its program could be run: as any such round, where it has no try a
/// process of the engine that ended left; else as a failure of the kind any such round's is, having taken what that
/// try took, which is the round's.
fn failed_before(left: Option<Left>, why: String) -> Result<Carried, String> {
    match left {
        None => Err(why),
        Some(left) => Ok(Carried::Failed(
            why,
            Failure::Other,
            failure::Said::default(),
            spent_of(left),
        )),
    }
}

/// Whether a try of the round found its line's session lost, and began it again under its id: as the records say, once
/// for the round.
fn lost_in(work: &Work, line: &str, round: &Round) -> bool {
    let began = work::sessions::began_at(work, round.run, round.number);
    (work::sessions::of_line(work, line)).any(|s| s.why == Why::Lost && began.is_some_and(|at| at < s.at))
}

/// Of a round given again after the process of the engine that gave it ended while a try of it ran: that try's number
/// and its program's stream, read from what the try kept of it, where the program said it began, reached the service
/// or printed its result. None where the round has kept no try, or its last try's program said none of these, as one
/// the engine ended before it read its message, or one the command line refused to begin or to go on in, on its
/// standard error or in a result that is all its stream holds, which is given again.
async fn left_try(base: &Path) -> Option<(usize, cli::Stream)> {
    let (next, ..) = attempt(base).await;
    let last = next.checked_sub(1).filter(|n| *n > 0)?;
    let printed = tokio::fs::read(kept::stream_at(base, last)).await.ok()?;
    let mut stream = cli::Stream::default();
    for line in printed.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
        stream.line(line);
    }
    let said = stream.began() || stream.took().is_some() || stream.has_result();
    (said && stream.refusal().is_none()).then_some((last, stream))
}

/// The last message of the round of this base in its session, as its tries' streams name it ([`cli::last_message`]):
/// the latest try's that names one, since a try that ended naming none, as one that found its session lost or in use,
/// left the session as the try before it did. None where no try's stream names one; why, where one cannot be read.
async fn last_message_of(base: &Path) -> Result<Option<String>, String> {
    let (next, ..) = attempt(base).await;
    for n in (1..next).rev() {
        let at = kept::stream_at(base, n);
        match tokio::fs::read(&at).await {
            Ok(stream) => {
                if let Some(id) = cli::last_message(&stream) {
                    return Ok(Some(id));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                let name = at.file_name().unwrap_or_default().to_string_lossy().to_string();
                return Err(format!("The round's stream {name} cannot be read: {e}."));
            }
        }
    }
    Ok(None)
}

/// Where what the owner says to the round of this base waits for its hook, and where the hook moves what it gave: the
/// one place the round's command line is given and the telling puts the words.
fn words(base: &Path) -> Words {
    Words {
        waiting: base.with_extension("words.json"),
        given: base.with_extension("given"),
    }
}

/// What the owner said to a round under way, as its session is given it: in the words of `said` in the message's
/// file, with what the cell does when its words are new as the part of what is new says it; or, for a round told to
/// hand in what it has for the owner turning to a cell before its own, in the words of `hurried`.
fn said(beside: &Beside, told: &Telling) -> String {
    let message = &beside.message;
    let part = message.parts.iter().find(|p| p.part == flow::agents::Part::New);
    let does = match (&told.does, part.and_then(|p| p.does.as_deref())) {
        (Some(does), Some(text)) => flow::agents::fill(text, &[("do", does.trim())]),
        _ => String::new(),
    };
    let number = told.number.to_string();
    let places = [
        ("entry", told.entry.as_str()),
        ("block", told.block.as_str()),
        ("number", number.as_str()),
        ("do", does.as_str()),
    ];
    let text = if told.hurry { &message.hurried } else { &message.said };
    flow::agents::fill(text, &places)
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
    use driver::Instead;
    use store::AccountAct;
    use work::Back;

    use super::*;

    fn pause(at: u64, paused: bool, by: Pauser) -> Pause {
        Pause {
            paused,
            at: Seq(at),
            by,
            why: None,
            waits_for: Vec::new(),
        }
    }

    // A message a later try of the round is told again is stored once, under a second name of the file an earlier try
    // keeps it in, while each try is counted and read back with its message as before; one told otherwise is kept in
    // a file of its own.
    #[tokio::test]
    async fn a_message_told_again_to_a_later_try_is_stored_once_and_each_try_reads_back_with_its_message() {
        let dir = tempfile::tempdir().unwrap();
        let (work, run, number) = (WorkNumber(3), RunId(7), 2);
        let base = kept::base(dir.path(), work, run, number);
        std::fs::create_dir_all(base.parent().unwrap()).unwrap();
        let told = [
            "Do the task.\n",
            "Do the task.\n",
            "Only what changed.\n",
            "Do the task.\n",
        ];
        for said in told {
            let (n, at, _) = attempt(&base).await;
            keep_told(&base, n, &at, said).await.unwrap();
        }
        assert_eq!(attempt(&base).await.0, 5, "each try is counted");
        let read = kept::told(dir.path(), work, run, number, &Default::default()).unwrap();
        let messages: Vec<&str> = read.tries.iter().map(|t| t.message.as_str()).collect();
        assert_eq!(messages, told);
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let file = |n: usize| std::fs::metadata(kept::at(&base, n).0).unwrap();
            assert_eq!(file(2).ino(), file(1).ino());
            assert_eq!(file(4).ino(), file(1).ino());
            assert_ne!(file(3).ino(), file(1).ino());
            assert_eq!(file(1).nlink(), 3, "the message told three times is stored once");
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

    // What a round under way is told is in the words of the message's file: the owner's own words to its cell ask for
    // its answer, and the owner turning to a cell before its own asks it to hand in what it has.
    #[test]
    fn a_round_is_told_in_the_words_of_its_telling_which_ask_for_an_answer_or_for_what_it_has() {
        let (_, beside) = engineer();
        let telling = |hurry: bool| Telling {
            entry: "The owner: Shorter, please.".to_string(),
            block: "d1_words".into(),
            number: 2,
            does: None,
            versions: vec![("d1_words".into(), Seq(5))],
            hurry,
        };
        let heading = "## `d1_words`, version 2, said while this round is running\n\nThe owner: Shorter, please.";
        let own = said(&beside, &telling(false));
        assert!(own.starts_with(heading), "{own}");
        assert!(own.contains("hand in your answer now"), "{own}");
        let hurried = said(&beside, &telling(true));
        assert!(hurried.starts_with(heading), "{hurried}");
        assert!(hurried.contains("hand in what you have now"), "{hurried}");
        assert!(!hurried.contains("your answer"), "{hurried}");
    }

    /// The engineer's flow as the repository defines it, and what is written beside it.
    fn engineer() -> (flow::Compiled, Beside) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
        let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
        let has = flow::Has {
            sources: names(&[
                "answer",
                "asked",
                "dependencies",
                "handed_in",
                "main",
                "start",
                "head",
                "test_times",
                "tree",
                "words",
            ]),
            acts: names(&["start", "check", "merge", "deliver"]),
            tools: names(&["query", "run_tests"]),
            ..Default::default()
        };
        let source = flow::read(&root.join("engineer")).expect("the flow is read");
        let flow = flow::compile(&source, &has).expect("the flow compiles");
        (
            flow,
            flow::agents::read(&root).expect("the files beside the flows are read"),
        )
    }

    /// The repository's own declaration, `wf-agents.toml` at its root, read as the engine reads a project's.
    fn declared(beside: &Beside) -> Declaration {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let text = std::fs::read_to_string(root.join(flow::agents::DECLARATION)).expect("the declaration is read");
        Declaration::read(&text, flow::agents::DECLARATION, beside).expect("the repository's declaration is one")
    }

    /// A work whose run 7, of R2, ran its first round, its try on these at 3, and whose round ended at 4.
    fn ran_once(on: RanOn) -> Work {
        let mut work = Work::default();
        work.records.started.push(decide::Started {
            run: RunId(7),
            cell: "R2".into(),
            at: Seq(1),
            because: Vec::new(),
            basis: Default::default(),
        });
        work.records.rounds.push(Began {
            run: RunId(7),
            at: Seq(2),
        });
        work.records.returned.push(Returned {
            run: RunId(7),
            at: Seq(4),
        });
        work.ran.push(work::Ran {
            at: Seq(3),
            run: RunId(7),
            round: 1,
            attempt: 1,
            on,
        });
        work
    }

    /// What the rounds of the fork tests ran on.
    fn sonnet() -> RanOn {
        RanOn {
            level: "2".to_string(),
            model: "claude-sonnet-5-5".to_string(),
            effort: Some("high".to_string()),
            directory: None,
            account: None,
        }
    }

    /// What the stand-in program of the fork tests prints for the two messages a fork is given, the first answered with a
    /// return and the second with the answer: the start, the week's usage, the first message's call, read mostly from
    /// the cache, and its result, then the question's call and its result; the first message spent $0.20 and the question $0.30, which the session's total, $0.50, is the sum of.
    const FORK_STREAM: [&str; 6] = [
        r#"{"type":"system","subtype":"init","claude_code_version":"9.9.9"}"#,
        r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","unifiedWindows":{"seven_day":{"utilization":0.67,"resetsAt":1795129200,"status":"allowed"}}}}"#,
        r#"{"type":"stream_event","event":{"type":"message_start","message":{"usage":{"input_tokens":100,"cache_creation_input_tokens":0,"cache_read_input_tokens":9000}}},"parent_tool_use_id":null}"#,
        r#"{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.2,"model_usage_since_last_result":{"m":{"costUSD":0.2}},"usage":{"input_tokens":100,"cache_creation_input_tokens":0,"cache_read_input_tokens":9000,"output_tokens":10},"structured_output":{"outcome":{"observations":["Waiting."]}}}"#,
        r#"{"type":"stream_event","event":{"type":"message_start","message":{"usage":{"input_tokens":20,"cache_creation_input_tokens":0,"cache_read_input_tokens":9100}}},"parent_tool_use_id":null}"#,
        r#"{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.5,"model_usage_since_last_result":{"m":{"costUSD":0.3}},"usage":{"input_tokens":20,"cache_creation_input_tokens":0,"cache_read_input_tokens":9100,"output_tokens":20},"structured_output":{"outcome":{"observations":["The task reads two ways."]}}}"#,
    ];

    /// The stand-in program of the fork tests in this directory: it keeps its arguments, where it ran and what it found
    /// there, the token and the directory it was given, and what it was sent on its input, and prints `stream`.
    fn write_program(data: &Path, stream: &[&str]) -> PathBuf {
        let program = data.join("program.sh");
        let printed: String = stream.iter().map(|line| format!("printf '%s\\n' '{line}'\n")).collect();
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{0}/args'\npwd > '{0}/cwd'\nls > '{0}/listed'\n\
             printf '%s' \"${{CLAUDE_CODE_OAUTH_TOKEN:-none}}\" > '{0}/token'\n\
             printf '%s' \"$CLAUDE_CONFIG_DIR\" > '{0}/directory'\ncat > '{0}/input'\n\
             if [ -f '{0}/leave' ]; then sleep 30 >/dev/null 2>&1 & echo $! > '{0}/left'; fi\n{printed}",
            data.display()
        );
        std::fs::write(&program, script).expect("written");
        std::fs::set_permissions(&program, std::os::unix::fs::PermissionsExt::from_mode(0o755)).expect("made runnable");
        program
    }

    /// The carrier a fork of work 3's first round of run 7 is asked of, its data in this directory, and that fork, of
    /// the work as [`ran_once`] keeps it: the program a stand-in ([`write_program`]) that prints [`FORK_STREAM`].
    /// The engineer's line's first session began as kept, the work's worktree is there, its project's repository
    /// beside, and the time now is ten thousand seconds in; the cache keeps a session an hour after its last call. The
    /// accounts `work` and `home` are added, and `work` is in use.
    fn forking(data: &Path) -> (Agents, git::tests::Repo, Fork) {
        let program = write_program(data, &FORK_STREAM);
        let mut settings = Settings::read(include_str!("../install.sample.toml")).expect("the sample reads");
        settings.agents.command = program.display().to_string();
        let repo = git::tests::Repo::new("true");
        let project = repo.project.clone();
        let (flow, beside) = engineer();
        let declaration = declared(&beside);
        let of = Of::new(&settings, project, beside, declaration, "refs/heads/main".to_string());
        // The tokens of the accounts `work` and `home`.
        let tokens = service::KeptTokens::new();
        for (name, secret) in [("work", "sk-ant-oat01-work"), ("home", "sk-ant-oat01-home")] {
            service::Tokens::keep(&tokens, name, &Token::new(secret.to_string())).expect("kept");
        }
        let agents = Agents::with_clock(
            settings,
            data.to_path_buf(),
            Uuid::nil(),
            [("example".to_string(), Ok(of))].into(),
            Arc::new(tokens),
            Arc::new(service::TokioClock::new(work::Time(10_000_000))),
        );
        let session = session::session(Uuid::nil(), WorkNumber(3), "engineer", 0);
        let keep = |what, text: &str| {
            let at = system::kept_at(data, session, what);
            std::fs::create_dir_all(at.parent().expect("a directory")).expect("made");
            std::fs::write(at, text).expect("kept");
        };
        keep(system::Begun::System, "THE SYSTEM PROMPT");
        keep(system::Begun::Form, "{\"type\":\"object\"}");
        let place = git::place(data, &git::tag(&Uuid::nil()), WorkNumber(3));
        std::fs::create_dir_all(&place.dir).expect("the worktree");
        let first = store::First {
            title: "Greet the user".to_string(),
            level: None,
            project: Some("example".to_string()),
            depends_on: Vec::new(),
            tags: Vec::new(),
            continues: None,
        };
        let fork = Fork {
            work: WorkNumber(3),
            outline: Arc::new(flow::Outline::of(&flow)),
            item: store::Item::new(store::ItemNumber(1), work::Time(0), first, Vec::new()),
            kept: Arc::new(Work {
                moments: vec![
                    work::Moment {
                        from: Seq(1),
                        at: work::Time(1_000_000),
                    },
                    work::Moment {
                        from: Seq(5),
                        at: work::Time(6_000_000),
                    },
                ],
                ..ran_once(sonnet())
            }),
            run: RunId(7),
            round: 1,
            question: "Which words can be read two ways?".to_string(),
            ended: false,
            accounts: driver::Accounts::new(accounts(&[
                ("work", AccountAct::Added),
                ("home", AccountAct::Added),
                ("work", AccountAct::Taken),
            ])),
        };
        (agents, repo, fork)
    }

    /// The records of what was done to the accounts, in order, a millisecond apart.
    fn accounts(done: &[(&str, store::AccountAct)]) -> Vec<store::AccountRecord> {
        (1..)
            .zip(done)
            .map(|(at, (name, what))| store::AccountRecord {
                name: name.to_string(),
                what: *what,
                at: work::Time(at),
                taker: store::AccountTaker::Owner,
                left_resets: None,
            })
            .collect()
    }

    // A fork runs under the account its session's last try ran under, whose cache holds the session, given its token
    // and the engine's directory; where that account was removed, or the try ran under none, under the account in use,
    // and it says why; with none in use, such a fork runs nothing and fails, saying none is. What its stream says of
    // the usage of the account it ran under comes with it. Three forks of a stand-in program: a few tens of
    // milliseconds.
    #[tokio::test]
    async fn a_fork_runs_under_its_sessions_account_or_the_one_in_use_saying_why_and_none_with_none_in_use() {
        let dir = tempfile::tempdir().expect("a directory");
        let data = std::path::absolute(dir.path()).expect("absolute");
        let (agents, _repo, fork) = forking(&data);
        let given = || std::fs::read_to_string(data.join("token")).expect("what it was given");
        // The session's try ran under no account: under the one in use.
        let forked = agents.inner.fork(&fork).await.expect("the fork is asked");
        assert_eq!(
            (forked.account.as_str(), forked.instead, given()),
            ("work", Some(Instead::NoAccount), "sk-ant-oat01-work".to_string())
        );
        let profile = std::fs::read_to_string(data.join("directory")).expect("its directory");
        assert_eq!(profile, data.join("profile").display().to_string());
        assert_eq!(
            forked.usage,
            Some(driver::Said::Windows(vec![driver::Window {
                name: "seven_day".to_string(),
                utilization: Some(0.67),
                resets: Some(work::Time(1_795_129_200_000)),
                allowed: true,
            }]))
        );
        let mut kept = (*fork.kept).clone();
        kept.ran[0].on.account = Some("home".to_string());
        let under = |done: &[(&str, AccountAct)]| Fork {
            kept: Arc::new(kept.clone()),
            accounts: driver::Accounts::new(accounts(done)),
            ..fork.clone()
        };
        let mut done = vec![
            ("work", AccountAct::Added),
            ("home", AccountAct::Added),
            ("work", AccountAct::Taken),
        ];
        let forked = agents.inner.fork(&under(&done)).await.expect("the fork is asked");
        assert_eq!(
            (forked.account.as_str(), forked.instead, given()),
            ("home", None, "sk-ant-oat01-home".to_string())
        );
        done.push(("home", AccountAct::Removed));
        let forked = agents.inner.fork(&under(&done)).await.expect("the fork is asked");
        assert_eq!(
            (forked.account.as_str(), forked.instead, given()),
            (
                "work",
                Some(Instead::Removed("home".to_string())),
                "sk-ant-oat01-work".to_string()
            )
        );
        // With none in use, a session last run under none has no account to run under.
        std::fs::remove_file(data.join("token")).expect("taken away");
        let none = Fork {
            accounts: driver::Accounts::new(accounts(&[("work", AccountAct::Added)])),
            ..fork.clone()
        };
        let refused = agents.inner.fork(&none).await.expect_err("no account is in use");
        assert_eq!(refused.why, "No account is in use: choose one on the Settings view.");
        assert_eq!(refused.spent, None, "nothing was run, and nothing cost");
        assert!(!data.join("token").exists(), "nothing was run");
    }

    #[tokio::test]
    async fn a_fork_runs_on_its_sessions_round_command_line_in_the_worktree_and_keeps_its_answer() {
        let dir = tempfile::tempdir().expect("a directory");
        let data = std::path::absolute(dir.path()).expect("absolute");
        let (agents, repo, fork) = forking(&data);
        let session = session::session(Uuid::nil(), WorkNumber(3), "engineer", 0);
        let place = git::place(&data, &git::tag(&Uuid::nil()), WorkNumber(3));
        let forked = agents.inner.fork(&fork).await.expect("the fork is asked");
        let answer: serde_json::Value =
            serde_json::from_str(forked.answer.as_deref().expect("a return")).expect("JSON");
        assert_eq!(answer["outcome"]["observations"][0], "The task reads two ways.");
        assert_eq!(forked.read, Some(9000));
        assert_eq!(forked.spent.usd_micros, 500_000);
        // It was sent two messages, the first the same for every fork and the second its question, and it took the
        // tokens of both.
        let input = std::fs::read_to_string(data.join("input")).expect("what it was sent");
        let said: Vec<String> = (input.lines())
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("a message"))
            .map(|line| {
                line["message"]["content"][0]["text"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string()
            })
            .collect();
        let carried = agents.inner.projects["example"]
            .as_ref()
            .expect("the project is carried");
        let first = &carried.beside.cost.fork.first;
        assert!(!first.is_empty(), "the flow's words give the first message");
        assert_eq!(said, [first.clone(), fork.question.clone()]);
        assert_eq!(
            (
                forked.spent.uncached_tokens,
                forked.spent.cached_tokens,
                forked.spent.output_tokens
            ),
            (100 + 20, 9000 + 9100, 10 + 20)
        );
        // Its round ended at a thousand seconds: the cache let the session go 5,400 seconds ago.
        assert_eq!(forked.expired, Some(90));
        assert!(std::path::Path::new(&forked.kept).is_file(), "{}", forked.kept);
        // Given the session's own command line: what it began with, its model and effort, resumed and forked.
        let args = std::fs::read_to_string(data.join("args")).expect("its arguments");
        let args: Vec<&str> = args.lines().collect();
        let after = |flag: &str| args.iter().position(|a| *a == flag).map(|i| args[i + 1]);
        assert_eq!(after("--append-system-prompt"), Some("THE SYSTEM PROMPT"));
        assert_eq!(after("--json-schema"), Some("{\"type\":\"object\"}"));
        assert_eq!(after("--model"), Some("claude-sonnet-5-5"));
        assert_eq!(after("--effort"), Some("high"));
        assert_eq!(after("--resume"), Some(session.to_string().as_str()));
        assert_eq!(args.last(), Some(&"--fork-session"));
        let cwd = std::fs::read_to_string(data.join("cwd")).expect("where it ran");
        let worktree = std::fs::canonicalize(&place.dir).expect("the worktree");
        assert_eq!(std::fs::canonicalize(cwd.trim()).expect("a directory"), worktree);
        // What it was asked and answered is kept beside the round's; its hooks' files are taken away.
        let kept = |n: usize, what: &str| {
            let base = kept::base(&data, WorkNumber(3), RunId(7), 1);
            let name = base.file_name().expect("a name").to_string_lossy().to_string();
            base.with_file_name(format!("{name}.fork.{n}.{what}"))
        };
        assert_eq!(
            std::fs::read_to_string(kept(1, "message.md")).expect("kept"),
            fork.question
        );
        assert!(
            std::fs::read_to_string(kept(1, "answer"))
                .expect("kept")
                .contains("two ways")
        );
        for gone in ["compacted", "again.md", "words.json"] {
            assert!(!kept(1, gone).exists(), "{gone}");
        }
        // What the session cost before, its rounds and its pings, does not change what the fork cost: the program's
        // results say what each message spent; it held what its last round's prompt held. The ping, at six thousand
        // seconds, was the session's last call.
        let mut paid = (*fork.kept).clone();
        paid.sessions.push(work::Session {
            at: Seq(6),
            line: "engineer".to_string(),
            why: Why::Owner,
        });
        let spent = |usd_micros, context_tokens| Spent {
            usd_micros,
            context_tokens,
            ..Spent::default()
        };
        paid.spent.insert((RunId(7), 1), spent(100_000, Some(10_000)));
        paid.pinged.push(work::Pinged {
            at: Seq(5),
            line: "engineer".to_string(),
            session: 0,
            spent: spent(50_000, None),
            failed: None,
        });
        let again = Fork {
            kept: Arc::new(paid),
            ..fork.clone()
        };
        let forked = agents.inner.fork(&again).await.expect("asked again");
        assert_eq!(forked.spent.usd_micros, 500_000);
        assert_eq!(forked.held, Some(10_000));
        assert_eq!(forked.expired, Some(6));
        assert!(forked.later.is_empty());
        assert!(kept(2, "answer").is_file());
        // A work that has not ended, its worktree gone, is not forked in a directory made for it.
        std::fs::remove_dir_all(&place.dir).expect("removed");
        let refused = agents.inner.fork(&fork).await.expect_err("refused");
        assert_eq!(refused.why, "W3 has not ended, and its worktree is not there.");
        assert!(!place.dir.exists());
        // A work that has ended, its worktree gone, is forked in a checkout of its kept branch at the worktree's place,
        // taken away after; the branch stays.
        git::tests::run(&repo.project.repo, &["branch", &place.branch]);
        let ended = Fork { ended: true, ..fork };
        agents.inner.fork(&ended).await.expect("asked in a checkout");
        let cwd = std::fs::read_to_string(data.join("cwd")).expect("where it ran");
        let at = std::fs::canonicalize(&data)
            .expect("the data")
            .join(place.dir.strip_prefix(&data).expect("in the data"));
        assert_eq!(Path::new(cwd.trim()), at);
        let listed = std::fs::read_to_string(data.join("listed")).expect("what it held");
        assert!(listed.lines().any(|f| f == "greeting.txt"), "{listed}");
        assert!(!place.dir.exists());
        let worktrees = git::tests::run(&repo.project.repo, &["worktree", "list", "--porcelain"]);
        assert_eq!(worktrees.lines().filter(|l| l.starts_with("worktree ")).count(), 1);
        git::tests::run(&repo.project.repo, &["rev-parse", "--verify", &place.reference()]);
        // The work's own worktree, on its branch, is left to the work's end; a checkout with no branch that an earlier
        // fork left there is taken away.
        let dir = place.dir.display().to_string();
        git::tests::run(&repo.project.repo, &["worktree", "add", &dir, &place.branch]);
        agents.inner.fork(&ended).await.expect("asked in the work's worktree");
        assert!(place.dir.join("greeting.txt").is_file());
        git::tests::run(&repo.project.repo, &["worktree", "remove", "--force", &dir]);
        git::tests::run(
            &repo.project.repo,
            &["worktree", "add", "--detach", &dir, &place.branch],
        );
        agents.inner.fork(&ended).await.expect("asked in what was left");
        assert!(!place.dir.exists());
    }

    /// A call of the model's, as the stand-in program's stream says it of the fork's first message and of its question.
    const FIRST_CALL: &str = r#"{"type":"assistant","message":{"id":"m1","model":"claude-sonnet-5-5","usage":{"input_tokens":100,"cache_creation_input_tokens":0,"cache_read_input_tokens":9000,"output_tokens":10}},"parent_tool_use_id":null}"#;
    const SECOND_CALL: &str = r#"{"type":"assistant","message":{"id":"m2","model":"claude-sonnet-5-5","usage":{"input_tokens":20,"cache_creation_input_tokens":0,"cache_read_input_tokens":9100,"output_tokens":20}},"parent_tool_use_id":null}"#;

    // A fork whose program ended with the first message answered and the question not is no answer, which the first
    // message's return is not taken for, and it says what the call it made cost, and how many processes its program
    // left running, which were stopped, for the work to keep. The stand-in program, one fork: a few tens of
    // milliseconds, the leftover's ending on TERM a few more.
    #[tokio::test]
    async fn a_fork_whose_program_ended_one_message_is_not_answered_and_gives_what_it_cost() {
        let dir = tempfile::tempdir().expect("a directory");
        let data = std::path::absolute(dir.path()).expect("absolute");
        let (agents, _repo, fork) = forking(&data);
        write_program(&data, &[FORK_STREAM[0], FIRST_CALL, FORK_STREAM[3]]);
        std::fs::write(data.join("leave"), "").expect("written");
        let failed = agents
            .inner
            .fork(&fork)
            .await
            .expect_err("the question was not answered");
        assert_eq!(
            failed.why,
            "The fork's question was not answered: its program ended 1 of the fork's 2 messages."
        );
        let spent = failed.spent.expect("what its call cost");
        assert_eq!(
            (spent.uncached_tokens, spent.cached_tokens, spent.output_tokens),
            (100, 9000, 10)
        );
        assert!(spent.usd_micros > 0);
        assert!(failed.to_string().contains(" It cost $"), "{failed}");
        assert_eq!(failed.left, 1, "what the fork's program left is counted");
    }

    // A fork that ran and made no return says what it cost: what its two results say they spent, and the tokens of both
    // messages; and how many processes its program left running, which were stopped. One fork of a stand-in program: a
    // few tens of milliseconds, the leftover's ending on TERM a few more.
    #[tokio::test]
    async fn a_fork_that_ran_and_gave_no_answer_gives_what_it_cost() {
        let dir = tempfile::tempdir().expect("a directory");
        let data = std::path::absolute(dir.path()).expect("absolute");
        let (agents, _repo, fork) = forking(&data);
        let error = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"total_cost_usd":0.4,"model_usage_since_last_result":{"m":{"costUSD":0.2}},"usage":{"input_tokens":20,"cache_creation_input_tokens":0,"cache_read_input_tokens":9100,"output_tokens":20}}"#;
        write_program(
            &data,
            &[
                FORK_STREAM[0],
                FIRST_CALL,
                FORK_STREAM[3],
                FORK_STREAM[4],
                SECOND_CALL,
                error,
            ],
        );
        std::fs::write(data.join("leave"), "").expect("written");
        let failed = agents.inner.fork(&fork).await.expect_err("no answer");
        assert!(failed.why.starts_with("The fork wrote no answer: "), "{}", failed.why);
        assert_eq!(
            failed.spent,
            Some(Spent {
                usd_micros: 400_000,
                uncached_tokens: 120,
                cache_written_tokens: Some(0),
                cached_tokens: 18_100,
                output_tokens: 30,
                context_tokens: None,
                compactions: 0,
            })
        );
        assert_eq!(failed.left, 1, "what the fork's program left is counted");
    }

    // A fork whose question's result holds no return and no words says why with the sentence of the words beside the
    // flows, which ends in a stop: the reason is one sentence with one stop, not the sentence's stop and the fork's own.
    // One fork of a stand-in program: a few tens of milliseconds.
    #[tokio::test]
    async fn a_fork_that_wrote_no_answer_says_why_in_one_sentence_with_one_stop() {
        let dir = tempfile::tempdir().expect("a directory");
        let data = std::path::absolute(dir.path()).expect("absolute");
        let (agents, _repo, fork) = forking(&data);
        let no_return = r#"{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.5,"usage":{"input_tokens":20,"cache_creation_input_tokens":0,"cache_read_input_tokens":9100,"output_tokens":20}}"#;
        write_program(
            &data,
            &[
                FORK_STREAM[0],
                FIRST_CALL,
                FORK_STREAM[3],
                FORK_STREAM[4],
                SECOND_CALL,
                no_return,
            ],
        );
        let failed = agents.inner.fork(&fork).await.expect_err("no answer");
        assert!(failed.why.starts_with("The fork wrote no answer: "), "{}", failed.why);
        assert!(failed.why.ends_with("structured return."), "{}", failed.why);
        assert!(!failed.why.contains(".."), "{}", failed.why);
    }

    #[tokio::test]
    async fn a_fork_of_a_round_its_session_went_on_past_begins_at_that_rounds_last_message() {
        let dir = tempfile::tempdir().expect("a directory");
        let data = std::path::absolute(dir.path()).expect("absolute");
        let (agents, _repo, fork) = forking(&data);
        // Run 7's round, asked of, ran in two tries, the first finding the session lost, and ended at 6, at a thousand
        // seconds; two keep-warm pings followed, at 7 and 8, then run 8's round, on the same line in the same session,
        // from 11, at four thousand seconds, to 12, at nine thousand.
        let mut work = Work::default();
        for (run, at) in [(7, 1), (8, 9)] {
            work.records.started.push(decide::Started {
                run: RunId(run),
                cell: "R2".into(),
                at: Seq(at),
                because: Vec::new(),
                basis: Default::default(),
            });
            work.records.rounds.push(Began {
                run: RunId(run),
                at: Seq(at + 1),
            });
        }
        let ran = |at, run, attempt| work::Ran {
            at: Seq(at),
            run: RunId(run),
            round: 1,
            attempt,
            on: sonnet(),
        };
        work.ran.extend([ran(3, 7, 1), ran(5, 7, 2), ran(11, 8, 1)]);
        let reopened = |at, why| work::Session {
            at: Seq(at),
            line: "engineer".to_string(),
            why,
        };
        work.sessions.push(reopened(4, Why::Lost));
        let ended = |run, at| Returned {
            run: RunId(run),
            at: Seq(at),
        };
        work.records.returned.extend([ended(7, 6), ended(8, 12)]);
        let spent = |usd_micros, context_tokens| Spent {
            usd_micros,
            context_tokens,
            ..Spent::default()
        };
        work.spent.insert((RunId(7), 1), spent(100_000, Some(4_000)));
        work.spent.insert((RunId(8), 1), spent(200_000, Some(9_000)));
        for at in [7, 8] {
            work.pinged.push(work::Pinged {
                at: Seq(at),
                line: "engineer".to_string(),
                session: 0,
                spent: spent(10_000, None),
                failed: None,
            });
        }
        let moment = |from, at| work::Moment {
            from: Seq(from),
            at: work::Time(at),
        };
        work.moments = [
            (1, 1_000_000),
            (7, 2_000_000),
            (8, 3_000_000),
            (11, 4_000_000),
            (12, 9_000_000),
        ]
        .into_iter()
        .map(|(from, at)| moment(from, at))
        .collect();
        // The first try's stream is empty; the second's names the session's messages, the last of its own a tool's
        // result, and a subagent's after it.
        let base = kept::base(&data, WorkNumber(3), RunId(7), 1);
        std::fs::create_dir_all(base.parent().expect("a directory")).expect("made");
        let named = [
            r#"{"type":"system","subtype":"init","claude_code_version":"9.9.9"}"#,
            r#"{"type":"assistant","uuid":"a-1","parent_tool_use_id":null,"message":{}}"#,
            r#"{"type":"user","uuid":"u-1","parent_tool_use_id":null,"message":{}}"#,
            r#"{"type":"assistant","uuid":"sub-1","parent_tool_use_id":"toolu_9","message":{}}"#,
            r#"{"type":"result","subtype":"success","uuid":"r-1"}"#,
        ]
        .join("\n");
        for (n, stream) in [(1, ""), (2, named.as_str())] {
            std::fs::write(kept::at(&base, n).0, "Do the task.\n").expect("told");
            std::fs::write(kept::stream_at(&base, n), stream).expect("streamed");
        }
        let asked = |work: &Work| Fork {
            kept: Arc::new(work.clone()),
            ..fork.clone()
        };
        let forked = agents.inner.fork(&asked(&work)).await.expect("the fork is asked");
        let args = std::fs::read_to_string(data.join("args")).expect("its arguments");
        let args: Vec<&str> = args.lines().collect();
        assert_eq!(args[args.len() - 3..], ["--fork-session", "--resume-session-at", "u-1"]);
        // It held what the round asked of ended with, and is told the round and the later rounds it left out; it cost
        // what its two messages' results say, whatever the later round and the pings cost.
        assert_eq!(forked.held, Some(4_000));
        assert_eq!(forked.round, (RunId(7), 1));
        assert_eq!(forked.later, [(RunId(8), 1)]);
        assert_eq!(forked.spent.usd_micros, 500_000);
        // The first ping, at two thousand seconds, read what the round left from the cache, and the cache kept it an
        // hour after; the second read the first's.
        assert_eq!(forked.expired, Some(73));
        // With no ping, the next round's start read it, at four thousand seconds.
        let mut unpinged = work.clone();
        unpinged.pinged.clear();
        let forked = agents.inner.fork(&asked(&unpinged)).await.expect("asked with no ping");
        assert_eq!(forked.expired, Some(40));
        // A next round's start after the cache let it go read nothing of it: it was let go an hour after the round's
        // end.
        unpinged.moments[3].at = work::Time(6_000_000);
        let forked = agents
            .inner
            .fork(&asked(&unpinged))
            .await
            .expect("asked with a later round late");
        assert_eq!(forked.expired, Some(90));
        // A fork that cannot begin at the round's end is refused before anything is written, and takes no number.
        let kept = |n: usize, what: &str| {
            let name = base.file_name().expect("a name").to_string_lossy().to_string();
            base.with_file_name(format!("{name}.fork.{n}.{what}"))
        };
        assert!(kept(3, "message.md").is_file());
        let mut refused = Vec::new();
        for why in [Why::Compacted, Why::Lost] {
            let mut reopened_after = work.clone();
            reopened_after.sessions.push(reopened(11, why));
            refused.push(
                agents
                    .inner
                    .fork(&asked(&reopened_after))
                    .await
                    .expect_err("refused")
                    .why,
            );
        }
        std::fs::write(kept::stream_at(&base, 2), "{\"type\":\"result\"}\n").expect("named nothing");
        refused.push(agents.inner.fork(&asked(&work)).await.expect_err("refused").why);
        assert_eq!(
            refused,
            [
                "The session was compacted after run 7 round 1, and holds only a summary of what came before: no fork \
                 can begin at that round's end.",
                "The session was begun again after run 7 round 1, the agents' program having lost it: no fork can begin \
                 at that round's end.",
                "The session went on past the round, and no stream of the round's tries names a message of it: no fork \
                 can begin at its end.",
            ]
        );
        std::fs::remove_file(kept::stream_at(&base, 2)).expect("removed");
        std::fs::create_dir(kept::stream_at(&base, 2)).expect("made unreadable");
        let unreadable = agents.inner.fork(&asked(&work)).await.expect_err("refused").why;
        assert!(
            unreadable.starts_with("The round's stream 7-1.stream.2.jsonl cannot be read: "),
            "{unreadable}"
        );
        for what in ["message.md", "again.md"] {
            assert!(!kept(4, what).exists(), "{what}");
        }
    }

    /// The carrier of the engineer's flow's rounds, its data in this directory, and R2's first round of run 7 of work
    /// 3, as its `give` keeps it, the work as kept being this one and the round judged by these lines.
    fn given_round_of(dir: &Path, kept: Work, lines: driver::history::Lines) -> (Agents, Round) {
        let settings = Settings::read(include_str!("../install.sample.toml")).expect("the sample reads");
        let project = settings.projects["example"].clone();
        let (flow, beside) = engineer();
        let declaration = declared(&beside);
        let of = Of::new(&settings, project, beside, declaration, "refs/heads/main".to_string());
        let projects = [("example".to_string(), Ok(of))].into();
        let agents = Agents::new(
            settings,
            dir.to_path_buf(),
            Uuid::nil(),
            projects,
            Arc::new(service::KeptTokens::new()),
        );
        let first = store::First {
            title: "Greet the user".to_string(),
            level: None,
            project: Some("example".to_string()),
            depends_on: Vec::new(),
            tags: Vec::new(),
            continues: None,
        };
        let r2 = (flow.cells.iter())
            .find(|c| c.id.as_str() == "R2")
            .expect("the flow has R2")
            .clone();
        let round = Round {
            work: WorkNumber(3),
            cell: r2,
            flow: Arc::new(flow),
            run: RunId(7),
            number: 1,
            given: work::Given::Task,
            item: store::Item::new(store::ItemNumber(1), work::Time(0), first, Vec::new()),
            kept: Arc::new(kept),
            lines,
            account: None,
        };
        let key = (round.work, round.run, round.number);
        (agents.inner.said.lock().unwrap()).insert(key, Arc::default());
        (agents, round)
    }

    // A keep-warm ping runs under the account it is handed out with, the one in use, which its session's last call ran
    // under: its program is given that account's token and the engine's directory, and what its stream says of the
    // account's usage is heard; what its program leaves running is stopped and counted, as a fork's is; handed out with
    // none, or under an account whose token cannot be given, it fails, saying so, its program not run. Three runs of a
    // stand-in program, two leaving a process that ends on TERM: a few tens of milliseconds; no other test holds a ping
    // or a fork whose program leaves something running.
    #[tokio::test]
    async fn a_ping_runs_under_the_account_it_is_handed_out_with_given_its_token() {
        let dir = tempfile::tempdir().expect("a directory");
        let data = std::path::absolute(dir.path()).expect("absolute");
        let (agents, _repo, fork) = forking(&data);
        let ping = |account: Option<&str>| Ping {
            work: fork.work,
            flow: Arc::new(engineer().0),
            line: "engineer".to_string(),
            session: 0,
            item: fork.item.clone(),
            kept: fork.kept.clone(),
            account: account.map(str::to_string),
        };
        let given = || std::fs::read_to_string(data.join("token")).ok();
        let mut usage = None;
        agents
            .inner
            .ping(&ping(Some("work")), &mut usage, &mut 0)
            .await
            .expect("the ping ends");
        assert_eq!(given().as_deref(), Some("sk-ant-oat01-work"));
        let profile = std::fs::read_to_string(data.join("directory")).expect("its directory");
        assert_eq!(profile, data.join("profile").display().to_string());
        assert!(
            matches!(&usage, Some(driver::Said::Windows(windows)) if windows.len() == 1),
            "the week's window: {usage:?}"
        );
        let args = std::fs::read_to_string(data.join("args")).expect("its arguments");
        let session = session::session(Uuid::nil(), WorkNumber(3), "engineer", 0).to_string();
        assert!(args.lines().any(|a| a == session), "it goes on in the session: {args}");
        assert!(!args.contains("sk-ant"), "{args}");
        // It goes on in the session itself, and is let use no tool but the return: a hook refuses every other, so that
        // beside the rounds of the work's other lines it changes nothing in their worktree.
        assert!(
            !args.lines().any(|a| a == "--fork-session"),
            "it forks the session: {args}"
        );
        let settings = (args.lines().skip_while(|a| *a != "--settings").nth(1)).expect("its settings");
        let hooks: serde_json::Value = serde_json::from_str(settings).expect("settings are JSON");
        assert_eq!(hooks["hooks"]["PreToolUse"][0]["matcher"], "*", "{settings}");
        assert_eq!(
            hooks["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            cli::refusal(),
            "{settings}"
        );
        // A program that leaves a process running in its group as it ends has it stopped, and counted for the ping's
        // reply to say; and a fork's, for its answer, which the service keeps with the work. The leftover's ending on
        // TERM adds a few milliseconds.
        std::fs::write(data.join("leave"), "").expect("written");
        let (to, mut said) = tokio::sync::mpsc::unbounded_channel();
        let reply = service::PingReply::new(fork.work, "engineer".to_string(), 0, to);
        Rounds::ping(&agents, ping(Some("work")), reply);
        let mut events = Vec::new();
        while let Some(message) = said.recv().await {
            if let service::Message::Happened { event, .. } = message {
                let ended = matches!(event, engine::Event::Pinged { .. });
                events.push(event);
                if ended {
                    break;
                }
            }
        }
        let of = work::Left::Ping {
            line: "engineer".to_string(),
            session: 0,
        };
        let ended = |event: &engine::Event| matches!(event, engine::Event::Pinged { failed: None, .. });
        assert!(
            matches!(&events[..], [engine::Event::LeftBehind { of: told, processes: 1 }, end] if *told == of && ended(end)),
            "what the ping's program left is said before its end: {events:?}"
        );
        let left = std::fs::read_to_string(data.join("left")).expect("what it left");
        let running = std::process::Command::new("kill")
            .args(["-0", left.trim()])
            .stderr(std::process::Stdio::null())
            .status()
            .expect("kill runs")
            .success();
        let _ = std::process::Command::new("kill")
            .args(["-s", "KILL", left.trim()])
            .status();
        assert!(!running, "what the ping's program left runs on");
        let forked = agents.inner.fork(&fork).await.expect("the fork ends");
        assert_eq!(forked.left, 1, "what the fork's program left is counted");
        std::fs::remove_file(data.join("leave")).expect("taken away");
        std::fs::remove_file(data.join("token")).expect("taken away");
        let (why, _) = (agents.inner.ping(&ping(None), &mut None, &mut 0).await).expect_err("no account");
        assert_eq!(why, "No account is in use: choose one on the Settings view.");
        let (why, _) = (agents.inner.ping(&ping(Some("gone")), &mut None, &mut 0).await).expect_err("no token");
        assert!(
            why.starts_with("the token of the account gone could not be read: "),
            "{why}"
        );
        assert_eq!(given(), None, "its program is not run");
    }

    // The program of a session runs one at a time: a ping of a line, and a fork of its session, are refused while a round
    // or a ping of that line holds its lock, and a ping or a fork of a line beside it is not, for its session is
    // another's. One run of a stand-in program for each that goes on: a few tens of milliseconds.
    #[tokio::test]
    async fn a_ping_and_a_fork_are_refused_beside_their_own_lines_program_and_not_another_lines() {
        let dir = tempfile::tempdir().expect("a directory");
        let data = std::path::absolute(dir.path()).expect("absolute");
        let (agents, _repo, fork) = forking(&data);
        let ping = |line: &str| Ping {
            work: fork.work,
            flow: Arc::new(engineer().0),
            line: line.to_string(),
            session: 0,
            item: fork.item.clone(),
            kept: fork.kept.clone(),
            account: Some("work".to_string()),
        };
        // A round of another line of the work is under way: the engineer's session is pinged and forked beside it.
        let other = agents.inner.lock(fork.work, "review");
        let _beside = other.lock().await;
        agents
            .inner
            .ping(&ping("engineer"), &mut None, &mut 0)
            .await
            .expect("a ping beside another line's round");
        agents
            .inner
            .fork(&fork)
            .await
            .expect("a fork beside another line's round");
        // A round of the engineer's own line is under way: both are refused, the fork naming the line.
        let own = agents.inner.lock(fork.work, "engineer");
        let _round = own.lock().await;
        let (why, _) =
            (agents.inner.ping(&ping("engineer"), &mut None, &mut 0).await).expect_err("a ping beside its round");
        assert!(why.contains("under way"), "{why}");
        let refused = agents.inner.fork(&fork).await.expect_err("a fork beside its round").why;
        assert!(refused.contains("line engineer"), "{refused}");
        assert!(refused.contains("under way"), "{refused}");
    }

    // What comes before a session's messages names the account a call runs under, so that a miss after another is taken
    // says it, and leaves it out where none is in use. It holds neither the effort, which caused no miss, nor a digest
    // of the engine's own arguments, the program's own report of its tools being kept in its place.
    #[test]
    fn what_comes_before_the_messages_names_the_account_and_leaves_it_out_where_there_is_none() {
        let under = before_messages(&sonnet(), Some("work"), "system", "form");
        assert_eq!(under.get("account").map(String::as_str), Some("work"));
        let none = before_messages(&sonnet(), None, "system", "form");
        assert!(!none.contains_key("account"));
        assert_eq!(under.len(), none.len() + 1);
        for part in ["effort", "tools"] {
            assert!(!under.contains_key(part), "{part} is no part of what the engine says");
        }
    }

    // What the program says it gave the model, from its first line, is kept in the try's measures under names of their
    // own, in the order it gave it, so that a tool taken out of the list changes the digest, and an equal list does not.
    // The shape of work 23 run 15: the program's list lost WebFetch with the engine's arguments unchanged.
    #[test]
    fn what_the_program_says_it_gave_the_model_is_kept_in_the_measures_before_the_messages() {
        // The program's first partial message of a call of its own agent, which tells the call's prompt.
        const CALL: &str = r#"{"type":"stream_event","event":{"type":"message_start","message":{"usage":{"input_tokens":2,"cache_read_input_tokens":90,"cache_creation_input_tokens":8}}},"parent_tool_use_id":null}"#;
        let before_of = |init: serde_json::Value| {
            let mut stream = cli::Stream::default();
            stream.line(init.to_string().as_bytes());
            stream.line(CALL.as_bytes());
            measures(&stream, 10, BTreeMap::new(), false, false)
                .expect("the stream holds a call")
                .before
        };
        let init = |tools: &[&str], agents: &[&str]| {
            serde_json::json!({
                "type": "system", "subtype": "init", "claude_code_version": "2.1.327", "tools": tools, "agents": agents,
            })
        };
        let with = before_of(init(&["Bash", "WebFetch", "Write"], &["claude", "Plan"]));
        let same = before_of(init(&["Bash", "WebFetch", "Write"], &["claude", "Plan"]));
        let without = before_of(init(&["Bash", "Write"], &["claude", "Plan", "web-fetch"]));
        assert_eq!(with, same);
        assert_ne!(with["program_tools"], without["program_tools"]);
        assert_ne!(with["program_agents"], without["program_agents"]);
        assert_eq!(with["program"], "2.1.327");
        let reordered = before_of(init(&["WebFetch", "Bash", "Write"], &["claude", "Plan"]));
        assert_ne!(
            with["program_tools"], reordered["program_tools"],
            "the order is part of the prompt"
        );
        // A first line that names no tools says none: the parts are left out, not made of an empty list.
        let bare = before_of(serde_json::json!({
            "type": "system", "subtype": "init", "claude_code_version": "2.1.327",
        }));
        assert!(!bare.contains_key("program_tools") && !bare.contains_key("program_agents"));
    }

    // What a work spent on forks counts against its budget as a keep-warm ping's spend does: a round that took a dollar
    // ends a work over the sample's five-dollar budget where its forks cost six, and not where it has none. Two checks on
    // a carrier with no program run: a few milliseconds.
    #[test]
    fn the_budget_counts_what_the_work_spent_on_forks() {
        let dir = tempfile::tempdir().unwrap();
        let (agents, round) = given_round_of(dir.path(), Work::default(), Default::default());
        let spent = Spent {
            usd_micros: 1_000_000,
            ..Spent::default()
        };
        assert_eq!(agents.inner.over_budget(&round, &spent), None);
        let mut kept = Work::default();
        kept.forked.push(work::Forked {
            at: Seq(3),
            run: RunId(7),
            round: 1,
            usd_micros: 6_000_000,
            uncached_tokens: 0,
            cache_written_tokens: None,
            cached_tokens: 0,
            output_tokens: 0,
        });
        let round = Round {
            kept: Arc::new(kept),
            ..round
        };
        let said = agents
            .inner
            .over_budget(&round, &spent)
            .expect("the work is over its budget");
        assert!(said.contains("7.00"), "{said}");
    }

    // What the work spent on forks since the engine last paused it and the owner let it on counts against the line of
    // what the flow's other works spent, as a keep-warm ping's spend does, and what it spent on forks before that does
    // not. One round of $1 against a line of $2.50: alone it is under it; with $3 of forks after the letting-on it is
    // over, and with them before it is not. Three checks on a carrier with no program run: a few milliseconds.
    #[test]
    fn what_the_work_spent_on_forks_since_it_was_let_on_counts_against_the_line_of_its_flows_works() {
        let line = driver::history::Line {
            value: 2_500_000.0,
            samples: 3,
            scope: driver::history::Scope::Works,
        };
        let lines = driver::history::Lines {
            work_spent: Some(line),
            ..Default::default()
        };
        let past = |forks: &[u64]| {
            let dir = tempfile::tempdir().unwrap();
            let mut kept = Work::default();
            kept.records.pauses = vec![pause(4, true, Pauser::Engine), pause(5, false, Pauser::Owner)];
            for at in forks {
                kept.forked.push(work::Forked {
                    at: Seq(*at),
                    run: RunId(7),
                    round: 1,
                    usd_micros: 3_000_000,
                    uncached_tokens: 0,
                    cache_written_tokens: None,
                    cached_tokens: 0,
                    output_tokens: 0,
                });
            }
            let (agents, round) = given_round_of(dir.path(), kept, lines.clone());
            let spent = Spent {
                usd_micros: 1_000_000,
                ..Spent::default()
            };
            agents.inner.past_history(&round, &spent)
        };
        assert_eq!(past(&[]), [None, None]);
        assert_eq!(past(&[3]), [None, None], "a fork before the letting-on is not since it");
        let over = past(&[8]);
        assert!(over[0].as_deref().is_some_and(|said| said.contains("4.00")), "{over:?}");
        assert_eq!(over[1], None);
    }

    /// A line of a stream: the program's first, which says it took its message in.
    const INIT: &str = r#"{"type":"system","subtype":"init","claude_code_version":"9.9.9"}"#;

    /// A line of a stream: the start of a call of the session's own agent, whose prompt was sent this fresh.
    fn start(fresh: u64) -> String {
        format!(
            r#"{{"type":"stream_event","event":{{"type":"message_start","message":{{"usage":{{"input_tokens":{fresh},"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}}}}},"parent_tool_use_id":null}}"#
        )
    }

    /// A line of a stream: the whole message of a call of the session's own agent to Opus 5.5, of 100 tokens sent,
    /// 1,000 read from the cache and 10 back.
    fn called(id: u32) -> String {
        format!(
            r#"{{"type":"assistant","parent_tool_use_id":null,"message":{{"id":"msg_{id}","model":"claude-opus-5-5","usage":{{"input_tokens":100,"cache_creation_input_tokens":0,"cache_read_input_tokens":1000,"output_tokens":10}}}}}}"#
        )
    }

    /// A line of a stream: the program's result, which spent this much, in dollars, the session having cost $9.99 in all.
    fn result(spent: &str) -> String {
        format!(
            r#"{{"type":"result","subtype":"success","is_error":false,"total_cost_usd":9.99,"model_usage_since_last_result":{{"m":{{"costUSD":{spent}}}}},"usage":{{"input_tokens":10,"cache_creation_input_tokens":5,"cache_read_input_tokens":100,"output_tokens":20}},"structured_output":{{"outcome":{{}}}}}}"#
        )
    }

    /// A work whose run 7, of R2, began its first round at 5 and ran its first try at 6, after an earlier run of R2, 6,
    /// had ended a round that cost a cent.
    fn left_work() -> Work {
        let mut work = Work::default();
        let started = |run: u64, at: u64| decide::Started {
            run: RunId(run),
            cell: "R2".into(),
            at: Seq(at),
            because: Vec::new(),
            basis: BTreeMap::new(),
        };
        work.records.started.extend([started(6, 1), started(7, 4)]);
        work.records.rounds.extend([
            Began {
                at: Seq(2),
                run: RunId(6),
            },
            Began {
                at: Seq(5),
                run: RunId(7),
            },
        ]);
        work.records.returned.push(Returned {
            at: Seq(3),
            run: RunId(6),
        });
        let spent = Spent {
            usd_micros: 10_000,
            uncached_tokens: 100,
            ..Spent::default()
        };
        work.spent.insert((RunId(6), 1), spent);
        work.ran.push(work::Ran {
            at: Seq(6),
            run: RunId(7),
            round: 1,
            attempt: 1,
            on: RanOn {
                level: "1".to_string(),
                model: "claude-opus-5-5".to_string(),
                effort: None,
                directory: None,
                account: None,
            },
        });
        work
    }

    /// The round's first try, as the process of the engine that gave it left it: what it was told, its stream as it
    /// kept it, and so many compactions marked; and the work's place.
    fn left_by_an_engine(agents: &Agents, round: &Round, stream: &[String], compactions: usize) {
        let base = agents.inner.kept(round);
        std::fs::create_dir_all(base.parent().expect("a directory")).unwrap();
        std::fs::write(kept::at(&base, 1).0, "# Is there a better way\n").unwrap();
        std::fs::write(kept::stream_at(&base, 1), format!("{}\n", stream.join("\n"))).unwrap();
        if compactions > 0 {
            std::fs::write(base.with_extension("compacted"), vec![b'.'; compactions]).unwrap();
        }
        let place = git::place(&agents.inner.data, &git::tag(&agents.inner.installation), round.work);
        std::fs::create_dir_all(place.dir).unwrap();
    }

    /// What the loop was told, by kind: the measures of each try, the compactions, and whether the service answered.
    fn told(told: &mut tokio::sync::mpsc::UnboundedReceiver<service::Message>) -> (Vec<usize>, usize, bool) {
        let (mut measured, mut compacted, mut answered) = (Vec::new(), 0, false);
        for message in std::iter::from_fn(|| told.try_recv().ok()) {
            match message {
                service::Message::Happened {
                    event: engine::Event::Measured { attempt, .. },
                    ..
                } => measured.push(attempt),
                service::Message::Happened {
                    event: engine::Event::Anew {
                        why: Why::Compacted, ..
                    },
                    ..
                } => compacted += 1,
                service::Message::Answered { .. } => answered = true,
                _ => {}
            }
        }
        (measured, compacted, answered)
    }

    /// The round carried out, with its teller's channel.
    async fn carried(
        agents: &Agents,
        round: &Round,
    ) -> (Carried, tokio::sync::mpsc::UnboundedReceiver<service::Message>) {
        let (to, told) = tokio::sync::mpsc::unbounded_channel();
        let teller = RoundReply::new(round.work, round.run, to).teller();
        let carried = (agents.inner)
            .carry(round, &teller, &mut Stop::never())
            .await
            .expect("the round is carried");
        (carried, told)
    }

    // A try whose engine told its compactions and measures, and ended before the round's end was recorded, is told only
    // what the records do not hold: here one of its two compactions and none of its measures; with none told, all of
    // it; and that the service answered it. Well under a second alone, nothing run; no other test holds what is told
    // of a left try once.
    #[tokio::test]
    async fn a_left_try_is_told_once_what_the_records_do_not_hold_of_it() {
        let stream = [INIT.to_string(), start(1_000), called(1)];
        let dir = tempfile::tempdir().unwrap();
        let mut kept = left_work();
        kept.sessions.push(work::Session {
            at: Seq(8),
            line: "engineer".to_string(),
            why: Why::Compacted,
        });
        kept.measured.push(work::Measured {
            at: Seq(9),
            run: RunId(7),
            round: 1,
            attempt: 1,
            measures: Measures::default(),
        });
        let (agents, round) = given_round_of(dir.path(), kept, Default::default());
        left_by_an_engine(&agents, &round, &stream, 2);
        let (_, mut heard) = carried(&agents, &round).await;
        assert_eq!(told(&mut heard), (Vec::new(), 1, true));

        let dir = tempfile::tempdir().unwrap();
        let (agents, round) = given_round_of(dir.path(), left_work(), Default::default());
        left_by_an_engine(&agents, &round, &stream, 2);
        let (carried, mut heard) = carried(&agents, &round).await;
        assert_eq!(told(&mut heard), (vec![1], 2, true));
        let Carried::Failed(_, failure, _, spent) = carried else {
            panic!("the round did not fail");
        };
        // Opus 5.5: 100 × 4 + 1,000 × 0.2 + 10 × 20 millionths of a dollar.
        assert_eq!(
            (failure, spent.usd_micros, spent.compactions),
            (Failure::Passing, 800, 2)
        );
    }

    // A left try past the run guard's lines, which printed no result, fails for the guard, as its engine would have
    // stopped it, so that its line begins a new session. Well under a second alone; no other test holds the guard of a
    // left try.
    #[tokio::test]
    async fn a_left_try_past_the_guard_s_lines_fails_for_the_guard() {
        let line = |value: f64| driver::history::Line {
            value,
            samples: 3,
            scope: driver::history::Scope::Cell,
        };
        let lines = driver::history::Lines {
            added: Some(line(10.0)),
            rate: Some(line(5.0)),
            ..Default::default()
        };
        let dir = tempfile::tempdir().unwrap();
        let (agents, round) = given_round_of(dir.path(), left_work(), lines);
        let stream = [INIT.to_string(), start(1_000), called(1), start(5_000), called(2)];
        left_by_an_engine(&agents, &round, &stream, 0);
        let (carried, _) = carried(&agents, &round).await;
        let Carried::Failed(why, failure, _, _) = carried else {
            panic!("the round did not fail");
        };
        assert_eq!(failure, Failure::Guarded, "{why}");
    }

    // A left try whose stream is only a result saying that its session is not found, as version 2.1.327 prints it, was
    // refused, and is none: its round is given again and tries the session the other way. A result that is an error for
    // another reason is a left try. Well under a second alone; no other test holds a left try that was refused.
    #[tokio::test]
    async fn a_left_try_whose_result_says_only_that_its_session_is_not_found_is_given_again() {
        let failed = |said: &str| {
            serde_json::json!({ "type": "result", "subtype": "error_during_execution", "is_error": true,
                "num_turns": 0, "total_cost_usd": 0, "errors": [said] })
            .to_string()
        };
        let lost = failed("No conversation found with session ID: 00000000-0000-5000-8000-000000000001");
        let dir = tempfile::tempdir().unwrap();
        let (agents, round) = given_round_of(dir.path(), left_work(), Default::default());
        left_by_an_engine(&agents, &round, &[lost], 0);
        assert!(agents.inner.left(&round, None).await.expect("it is read").is_none());

        let dir = tempfile::tempdir().unwrap();
        let (agents, round) = given_round_of(dir.path(), left_work(), Default::default());
        left_by_an_engine(&agents, &round, &[failed("Prompt is too long")], 0);
        assert!(agents.inner.left(&round, None).await.expect("it is read").is_some());
    }

    // A stream kept before the start of each call was kept with it says what the calls cost and not their prompts: no
    // measures are said, rather than measures of no call, and the round still takes what the calls cost. Well under a
    // second alone; no other test holds a stream kept so.
    #[tokio::test]
    async fn a_left_stream_that_kept_no_start_of_its_calls_says_no_measures() {
        let dir = tempfile::tempdir().unwrap();
        let (agents, round) = given_round_of(dir.path(), left_work(), Default::default());
        left_by_an_engine(&agents, &round, &[INIT.to_string(), called(1)], 0);
        let (carried, mut heard) = carried(&agents, &round).await;
        assert_eq!(told(&mut heard).0, Vec::<usize>::new());
        let Carried::Failed(_, _, _, spent) = carried else {
            panic!("the round did not fail");
        };
        assert_eq!(spent.usd_micros, 800);
    }

    // A round stopped after the process of the engine that gave it ended takes what the try that process left took,
    // and tells its measures, so that the session is known to hold its message. Well under a second alone; no other
    // test holds the stop of a left try.
    #[tokio::test]
    async fn a_round_stopped_after_its_engine_ended_takes_what_its_left_try_took() {
        let dir = tempfile::tempdir().unwrap();
        let (agents, round) = given_round_of(dir.path(), left_work(), Default::default());
        left_by_an_engine(&agents, &round, &[INIT.to_string(), start(1_000), called(1)], 0);
        let (to, mut heard) = tokio::sync::mpsc::unbounded_channel();
        let reply = RoundReply::new(round.work, round.run, to);
        agents.stop(round, reply);
        let mut measured = false;
        let spent = loop {
            let message = tokio::time::timeout(std::time::Duration::from_secs(10), heard.recv())
                .await
                .expect("the round ends")
                .expect("the channel is open");
            match message {
                service::Message::Happened {
                    event: engine::Event::Measured { .. },
                    ..
                } => measured = true,
                service::Message::Happened {
                    event: engine::Event::Returned { spent, .. },
                    ..
                } => break spent,
                _ => {}
            }
        };
        assert!(measured, "its measures are told");
        assert_eq!(spent.usd_micros, 800);
    }

    // A round this process gave, whose carrying ended and told what came of its try while its reply still waits, is
    // stopped taking that try's spend alone: what came of it is not told again. Well under a second alone; no other
    // test holds a stop that meets a round so.
    #[tokio::test]
    async fn a_round_given_here_and_stopped_as_its_carrying_ends_tells_nothing_of_its_try_again() {
        let dir = tempfile::tempdir().unwrap();
        let (agents, round) = given_round_of(dir.path(), left_work(), Default::default());
        left_by_an_engine(&agents, &round, &[INIT.to_string(), start(1_000), called(1)], 1);
        let (to, mut heard) = tokio::sync::mpsc::unbounded_channel();
        let key = (round.work, round.run, round.number);
        let held = agents.inner.held.hold(key, RoundReply::new(round.work, round.run, to));
        // Its carrying has ended: nothing waits for the stop any more.
        drop(held.expect("the round is held"));
        let (other, _) = tokio::sync::mpsc::unbounded_channel();
        agents.stop(round.clone(), RoundReply::new(round.work, round.run, other));
        let mut told = 0;
        let spent = loop {
            let message = tokio::time::timeout(std::time::Duration::from_secs(10), heard.recv())
                .await
                .expect("the round ends")
                .expect("the channel is open");
            match message {
                service::Message::Happened {
                    event: engine::Event::Returned { spent, .. },
                    ..
                } => break spent,
                service::Message::Happened {
                    event: engine::Event::Measured { .. } | engine::Event::Anew { .. },
                    ..
                } => told += 1,
                _ => {}
            }
        };
        assert_eq!(told, 0, "what came of its try was told again");
        assert_eq!((spent.usd_micros, spent.compactions), (800, 1));
    }

    /// What the loop is told of the round, until it is told one of these.
    async fn heard_until<T>(
        heard: &mut tokio::sync::mpsc::UnboundedReceiver<service::Message>,
        mut wanted: impl FnMut(service::Message) -> Option<T>,
    ) -> T {
        loop {
            let message = tokio::time::timeout(std::time::Duration::from_secs(10), heard.recv())
                .await
                .expect("the round ends")
                .expect("the channel is open");
            if let Some(found) = wanted(message) {
                return found;
            }
        }
    }

    // A round stopped after the process of the engine that gave it ended, whose left try printed its result, takes
    // what the result says it cost, not its calls at their prices. Well under a second alone; no other test holds the
    // stop of a left try that printed its result.
    #[tokio::test]
    async fn a_round_stopped_after_its_engine_ended_takes_what_its_left_try_s_result_says() {
        let dir = tempfile::tempdir().unwrap();
        let (agents, round) = given_round_of(dir.path(), left_work(), Default::default());
        let stream = [INIT.to_string(), start(1_000), called(1), result("0.03")];
        left_by_an_engine(&agents, &round, &stream, 0);
        let (to, mut heard) = tokio::sync::mpsc::unbounded_channel();
        agents.stop(round.clone(), RoundReply::new(round.work, round.run, to));
        let spent = heard_until(&mut heard, |message| match message {
            service::Message::Happened {
                event: engine::Event::Returned { spent, .. },
                ..
            } => Some(spent),
            _ => None,
        })
        .await;
        // What the result says it spent, $0.03, whatever the session cost in all; its call at its price is 800.
        assert_eq!(spent.usd_micros, 30_000);
    }

    // The kind of a failed round and when its usage limit resets are told by what the program reported, and no word of
    // the engine's is read for either: a reason that says nothing of the limit gives both from the report alone, and a
    // report that says no limit is no limit. Well under a millisecond; no other test holds the time a report gives.
    #[test]
    fn the_kind_of_a_failed_round_and_when_its_limit_resets_are_told_by_the_report_and_not_by_the_reason() {
        struct At;
        impl Clock for At {
            fn now(&self) -> Result<work::Time, String> {
                Ok(work::Time(1_792_958_400_000))
            }
        }
        let reason = "The command line's result is an error.".to_string();
        let said = failure::Said::default();
        let limit = failed_round(
            reason.clone(),
            "Claude AI usage limit reached|1792962000",
            said,
            None,
            &At,
        );
        let Carried::Failed(why, kind, said, _) = limit else {
            panic!("a failed round")
        };
        assert_eq!((why.as_str(), kind), (reason.as_str(), Failure::UsageLimit));
        assert_eq!(said.resets, Some(1_792_962_000_000));
        let other = failed_round(
            reason,
            "API Error: 529 Overloaded resets 5pm",
            failure::Said::default(),
            None,
            &At,
        );
        let Carried::Failed(_, kind, ..) = other else {
            panic!("a failed round")
        };
        assert_ne!(kind, Failure::UsageLimit);
    }

    // A left try whose result says the account's usage limit refused it fails of that kind, which holds every work's
    // rounds as one wait (decision a-shared-cause-is-one-wait), and not as a crash in passing. Well under a second
    // alone; no other test holds a limit a left try met.
    #[tokio::test]
    async fn a_left_try_that_met_the_usage_limit_fails_of_that_kind() {
        let refused = serde_json::json!({ "type": "assistant", "parent_tool_use_id": null, "error": "rate_limit",
            "api_error": "usage_limit_reached", "api_error_status": 429,
            "message": { "content": [{ "type": "text", "text": "You've hit your weekly limit · resets 11pm" }] } });
        let limit = serde_json::json!({ "type": "result", "subtype": "success", "is_error": true,
            "api_error_status": 429, "api_error": "usage_limit_reached", "terminal_reason": "api_error",
            "total_cost_usd": 0.0, "result": "You've hit your weekly limit · resets 11pm" });
        let dir = tempfile::tempdir().unwrap();
        let (agents, round) = given_round_of(dir.path(), left_work(), Default::default());
        left_by_an_engine(
            &agents,
            &round,
            &[INIT.to_string(), refused.to_string(), limit.to_string()],
            0,
        );
        let (to, mut heard) = tokio::sync::mpsc::unbounded_channel();
        agents.give(round.clone(), RoundReply::new(round.work, round.run, to));
        let failed = heard_until(&mut heard, |message| match message {
            service::Message::Happened {
                event: engine::Event::Failed { failure, why, .. },
                ..
            } => Some((failure, why)),
            _ => None,
        })
        .await;
        assert_eq!(failed.0, Failure::UsageLimit, "{}", failed.1);
    }
}
