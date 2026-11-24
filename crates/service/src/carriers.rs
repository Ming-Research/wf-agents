//! What carries out what the engine gives to be done, as the loop reaches it.
//!
//! A carrier is given what it needs to do its thing without reaching into the driver: the work's number, its flow,
//! the cell's specification, the run, the round and what it is given, the item the work is done for, and the work as
//! it is kept when the todo is handed out. It does the thing on its own time, and says once what came of it through
//! the reply it was handed with it, which takes that into the loop's channel. A carrier gives no answer to the loop
//! otherwise: what it does is known only by what it says came of it.
//!
//! What was to be done and is not known as done is handed out again when the process starts again, with the same
//! work, run, round and call: a carrier recognises by those what it began before the stop (principle
//! `effects-known-as-done`), and answers it once.
//!
//! A round a carrier cannot carry out it says failed, with why, and with the kind of cause where it tells one apart:
//! the engine handles the failure as the kind says, pausing the work with the reason where that is the kind's handling,
//! and the run's next round is given the same as the failed one, a new round that the carrier carries out anew. The
//! carrier keeps nothing of a failed round, and tries nothing again itself: when a round is tried again is the engine's
//! to say, as its records show, and whatever made the round fail is shown on the page.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};

use decide::run::Failure;
use decide::{BlockId, CallId, End, Outsider, Pauser, RunId, Seq};
use driver::history::Lines;
use driver::{Accounts, Along, Found, Instead, Policy, Said, Under};
use engine::{Came, Event};
use flow::{CellSpec, Compiled, Kind};
use store::{Item, WorkNumber};
use tokio::sync::mpsc;
use work::{Fill, Given, Left, Measures, RanOn, Spent, Time, Why, Work};

use crate::Message;

pub use engine::spoken::Telling;

/// What carries out the rounds of cells of one kind: an agent's rounds, or the engine's own acts.
///
/// Both methods are called inside the loop, between two things it takes: each returns at once, and does its work
/// elsewhere, on a task or a thread of its own, so that the loop is held up by no round.
pub trait Rounds: Send + Sync {
    /// Gives the round: an agent is given it in the session of the cell's line, or the act is done. The round ends when
    /// the reply says what came back and what it took, or that it failed and why. Given again after the process started
    /// again, it is the same round, to be gone on with where what was done of it is known, and done again where it is
    /// not.
    fn give(&self, round: Round, reply: RoundReply);

    /// Stops the round, which is under way: its work was cancelled. The round ends as it always does, by a reply
    /// saying what it took; what came back is dropped. A round has one end: where the reply it was given with is
    /// still held, that one is used and this one dropped, and where the round has ended already this one is dropped.
    fn stop(&self, round: Round, reply: RoundReply);

    /// Tells the round, which is under way, what the owner said to it, to be handed to its session after its program's
    /// next use of one of its own tools; once it was, the carrier says so by its teller
    /// ([`Teller::said_to_round`]), and words that reached the round before it ended are said before its end. Rounds
    /// carried out in no session are told nothing, as by default, and their cells take the words in their next round.
    fn tell(&self, round: Round, told: Telling) {
        let _ = (round, told);
    }

    /// What the round of this number of the run was told and what came back of it, each try in order, as far as
    /// this carrier keeps them; no try where it keeps nothing, as by default. Unlike the others, this is called off
    /// the loop, on a thread that may wait on the disk; an error says why what was kept could not be read.
    fn told(&self, work: WorkNumber, run: RunId, round: usize) -> Result<Told, String> {
        let _ = (work, run, round);
        Ok(Told::default())
    }

    /// The system prompt the line's session of this number, the line's first being none, began with, as this carrier
    /// kept it for the work; none where it kept none, as by default and for rounds carried out in no session. It is
    /// called off the loop, as [`Rounds::told`] is.
    fn system(&self, work: WorkNumber, line: &str, session: u32) -> Result<Option<String>, String> {
        let _ = (work, line, session);
        Ok(None)
    }

    /// What the agent of the round of this number of the run did from the mark on, as this carrier keeps it as it
    /// comes: each thing whole in the order done, every try in order, and the mark the next read goes on from; nothing
    /// where it keeps none, as by default. It is called off the loop, as [`Rounds::told`] is.
    fn doing(&self, work: WorkNumber, run: RunId, round: usize, from: Mark) -> Result<Doings, String> {
        let _ = (work, run, round);
        Ok(Doings {
            done: Vec::new(),
            next: from,
        })
    }

    /// What the repository of the work's project says of the work beside its records, as this carrier works it out
    /// with git: what the work changed, and, of a work that merged with no pull request kept, the pull request its
    /// merge names; nothing, as by default, for rounds that keep no repository. It is called off the loop, as
    /// [`Rounds::fork`] is.
    fn seen(&self, of: OfWork) -> Pin<Box<dyn Future<Output = Result<Seen, String>> + Send + '_>> {
        let _ = of;
        Box::pin(async { Ok(Seen::default()) })
    }

    /// Each file the work changed, as [`Rounds::seen`] counts them; none, as by default, where nothing tells what it
    /// changed. It is called off the loop.
    fn changes(&self, of: OfWork) -> Pin<Box<dyn Future<Output = Result<Option<Changes>, String>> + Send + '_>> {
        let _ = of;
        Box::pin(async { Ok(None) })
    }

    /// The unified diff of one file the work changed, between the base and the head [`Rounds::changes`] gives; none,
    /// as by default, where the work changed no file of that path. It is called off the loop.
    fn diff(
        &self,
        of: OfWork,
        path: String,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Diff>, String>> + Send + '_>> {
        let _ = (of, path);
        Box::pin(async { Ok(None) })
    }

    /// The pull requests open on the host of this project's repository, as the carrier that keeps the repository asks
    /// the host; none, as by default, where the host has no pull requests, as the local repository; or why the host
    /// could not say. It is called off the loop.
    fn open_pulls(&self, project: &str) -> Pin<Box<dyn Future<Output = PullsSaid> + Send + '_>> {
        let _ = project;
        Box::pin(async { Ok(None) })
    }

    /// When a line begins a new session, for rounds carried out in sessions: what they run on, and whether a new session
    /// of a line would begin with another system prompt or form of return than its session began with. None, as by
    /// default, for rounds carried out in no session, whose lines the loop begins no session for itself.
    fn policy(&self) -> Option<Policy> {
        None
    }

    /// Asks a fork of the session the round ran in a question (design document `testing`, 27), as the planner asks
    /// it: what the fork answered, or why it could not be asked. It is called off the loop, as [`Rounds::told`] is,
    /// and takes no slot. Rounds carried out in no session have none to fork, as by default.
    fn fork(&self, fork: Fork) -> Pin<Box<dyn Future<Output = Result<Forked, ForkFailed>> + Send + '_>> {
        let _ = fork;
        Box::pin(async {
            Err(ForkFailed::from(
                "these rounds are carried out in no session, which a fork could be asked of",
            ))
        })
    }

    /// Sends the session a keep-warm ping, which only keeps its prompt cache from letting it go while its line waits:
    /// the reply says once what it took, or why it failed. Rounds carried out in no session are given none; by default
    /// a ping fails, saying so.
    fn ping(&self, ping: Ping, reply: PingReply) {
        let _ = ping;
        reply.failed(
            "these rounds are carried out in no session".to_string(),
            Spent::default(),
        );
    }

    /// Lets go of what the rounds of this kind made for the work outside the engine, the work having ended and nothing
    /// of it being under way; the reply says once that it is let go. Given again after the process started again, it is
    /// the same letting go, and what was let go already is found let go. Rounds that make nothing outside the engine
    /// let go of nothing, as by default.
    fn release(&self, release: Release, reply: ReleaseReply) {
        let _ = release;
        reply.released();
    }
}

/// A fork of a round's session, asked a question: the work as it is kept, the outline of the version of its flow it is
/// on, by which its rounds are placed in their sessions whichever version is installed, its item, the run and the round
/// of it whose session is forked, the question, whether the work has ended, and the accounts as they stand, by which
/// the fork runs under its session's account, or the one in use where that one was removed or the session ran under
/// none ([`Accounts::of_session`]).
#[derive(Clone, Debug)]
pub struct Fork {
    pub work: WorkNumber,
    pub outline: Arc<flow::Outline>,
    pub item: Item,
    pub kept: Arc<Work>,
    pub run: RunId,
    pub round: usize,
    pub question: String,
    pub ended: bool,
    pub accounts: Accounts,
}

/// Why a fork could not be asked or gave no answer, what its program cost where it ran, and how many processes it
/// left running in its group as it ended, which were stopped: a fork that ran spent money whether or not it answered,
/// and that is recorded with the work, as what it left is. A fork refused before its program ran cost nothing and has
/// none, and left nothing.
#[derive(Clone, Debug, PartialEq)]
pub struct ForkFailed {
    pub why: String,
    pub spent: Option<Spent>,
    pub left: usize,
}

impl From<String> for ForkFailed {
    fn from(why: String) -> Self {
        ForkFailed {
            why,
            spent: None,
            left: 0,
        }
    }
}

impl From<&str> for ForkFailed {
    fn from(why: &str) -> Self {
        ForkFailed::from(why.to_string())
    }
}

impl std::fmt::Display for ForkFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.why)?;
        match &self.spent {
            Some(spent) => write!(f, " It cost ${:.4}.", spent.usd_micros as f64 / 1e6),
            None => Ok(()),
        }
    }
}

/// What a fork answered: its return, as JSON, where it wrote one, and what its program said at its end; what the fork
/// cost and its tokens; what its first call read from the cache, and what the session held as the fork
/// began, the context the round it begins at ended with, in tokens, where each is known; how many minutes before the
/// cache had let go what the fork begins with, where it had: the last call that read it was more than the cache's life
/// before; the round asked of, by run and number; the later rounds of the session, which the fork leaves out, beginning
/// at that round's end; the file the answer is kept in; the account the fork ran under, and why that is the account in
/// use and not the session's, where it is; and what the service said of the usage of that account, none where
/// it named none.
#[derive(Clone, Debug, PartialEq)]
pub struct Forked {
    pub answer: Option<String>,
    pub text: String,
    pub spent: Spent,
    pub read: Option<u64>,
    pub held: Option<u64>,
    pub expired: Option<u64>,
    pub round: (RunId, usize),
    pub later: Vec<(RunId, usize)>,
    pub kept: String,
    pub account: String,
    pub instead: Option<Instead>,
    pub usage: Option<Said>,
    /// How many processes its program left running in its group as it ended, which were stopped: kept with the work
    /// and said with the answer, as what it cost is.
    pub left: usize,
}

/// A keep-warm ping of the session of this number of a line of the work.
#[derive(Clone, Debug)]
pub struct Ping {
    pub work: WorkNumber,
    /// The flow the work runs on.
    pub flow: Arc<Compiled>,
    pub line: String,
    /// Which of the line's sessions, counted from none.
    pub session: u32,
    /// The item the work is done for.
    pub item: Item,
    /// The work as it was kept when the ping was handed out.
    pub kept: Arc<Work>,
    /// The account the ping runs under: the one in use, which the session's last call ran under, by its name. None is
    /// handed out with none in use (decision no-default-credential); one handed out with none fails, its token not
    /// given.
    pub account: Option<String>,
}

/// The way to say once what came of a keep-warm ping.
#[derive(Debug)]
pub struct PingReply {
    work: WorkNumber,
    line: String,
    session: u32,
    to: mpsc::UnboundedSender<Message>,
}

impl PingReply {
    /// The reply that says what came of the ping down this channel, as the loop makes it; whoever tests a carrier
    /// makes one to read what the carrier says.
    pub fn new(work: WorkNumber, line: String, session: u32, to: mpsc::UnboundedSender<Message>) -> Self {
        PingReply {
            work,
            line,
            session,
            to,
        }
    }

    /// The ping ended, having taken this.
    pub fn ended(self, spent: Spent) {
        self.send(spent, None);
    }

    /// The ping failed, for this reason, having taken this.
    pub fn failed(self, why: String, spent: Spent) {
        self.send(spent, Some(why));
    }

    /// The agents' service said this of the usage of the account of this name, which the ping ran under.
    pub fn usage(&self, account: String, said: Said) {
        let _ = self.to.send(Message::Usage { account, said });
    }

    /// The ping's program ended by itself and left these many processes running in its group, which the carrier
    /// stopped: said before the ping's end.
    pub fn left_behind(&self, processes: usize) {
        let of = Left::Ping {
            line: self.line.clone(),
            session: self.session,
        };
        let event = Event::LeftBehind { of, processes };
        // Where the loop has ended, the process is stopping: what the ping left is not counted.
        let _ = self.to.send(Message::Happened { work: self.work, event });
    }

    fn send(self, spent: Spent, failed: Option<String>) {
        let event = Event::Pinged {
            line: self.line,
            session: self.session,
            spent,
            failed,
        };
        // Where the loop has ended, the process is stopping: a ping still due is given again when it starts again.
        let _ = self.to.send(Message::Happened { work: self.work, event });
    }
}

/// A work that has ended, whose rounds' carriers let go of what they made for it outside the engine.
#[derive(Clone, Debug)]
pub struct Release {
    pub work: WorkNumber,
    /// How it ended.
    pub end: End,
    /// The flow the work runs on.
    pub flow: Arc<Compiled>,
    /// The item the work is done for.
    pub item: Item,
    /// The work as it was kept when its release was handed out.
    pub kept: Arc<Work>,
}

/// The way to say once that what a work held is let go.
#[derive(Debug)]
pub struct ReleaseReply {
    work: WorkNumber,
    to: mpsc::UnboundedSender<Message>,
}

impl ReleaseReply {
    /// The reply that says what came of it down this channel, as the loop makes it; whoever tests a carrier makes one
    /// to read what the carrier says.
    pub fn new(work: WorkNumber, to: mpsc::UnboundedSender<Message>) -> Self {
        ReleaseReply { work, to }
    }

    /// What the work held could not be let go, for this reason.
    pub fn failed(self, why: String) {
        // Where the loop has ended, the process is stopping: the release is handed out again when it starts again.
        let _ = self.to.send(Message::Happened {
            work: self.work,
            event: Event::NotReleased { why },
        });
    }

    /// What the work held is let go.
    pub fn released(self) {
        // Where the loop has ended, the process is stopping: the release is handed out again when it starts again.
        let _ = self.to.send(Message::Happened {
            work: self.work,
            event: Event::Released,
        });
    }
}

/// What a round was told and what came back of it, as its carrier kept them: each try of it, in order. A round is
/// tried more than once where its carrier ran its program again for it. What is read of a round is bounded, and each
/// cut is said: a long message is cut, and tries past what is read of a round are left out and counted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Told {
    pub tries: Vec<Try>,
    /// How many later tries were left out, those before them being as much as is read of a round.
    pub left_out: usize,
}

/// One try of a round.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Try {
    /// The message it was told, as it was written, up to where it was cut.
    pub message: String,
    /// The message's whole length in bytes, where it was cut before its end.
    pub cut: Option<u64>,
    pub back: Back,
    /// What it ran on, as the work's records say its carrier chose it as the try began, or, for a try that began
    /// before tries were recorded, as its carrier kept it beside the try; none where neither says, as of a try kept
    /// before either, or of an act of the engine's own.
    pub ran: Option<RanOn>,
}

/// What came back of a try.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Back {
    /// Nothing that was kept: the try is under way, or it was stopped, never got as far as its program, or its program
    /// printed no result.
    Nothing,
    Returned(Returned),
    /// The program said the try failed, for this reason.
    Failed(String),
    /// What was kept of it could not be read, for this reason.
    Unreadable(String),
}

/// What came back of a try, as far as the page is given it: never the whole of what the program printed, which holds
/// what is not for the page, such as ids of sessions and of credentials.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Returned {
    /// The agent's closing words; none where they are the structured return itself.
    pub text: Option<String>,
    /// The structured return, as pretty-printed JSON.
    pub structured: Option<String>,
    pub turns: Option<u64>,
    pub duration_ms: Option<u64>,
    /// The models it was answered by, by name.
    pub models: Vec<String>,
    /// The tokens it wrote to the prompt cache, as the program printed them: a try's own, where a round's are in what
    /// it took.
    pub cache_creation_input_tokens: Option<u64>,
}

/// A thing an agent was seen doing during its round, as its program said it as it went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Doing {
    /// It used a tool by this name, on this where the tool's input names what: a command, a path, a pattern.
    Tool { name: String, on: Option<String> },
    /// It said this; the start of it where it is long.
    Said(String),
    /// Its session was being compacted.
    Compacting,
}

/// Where a read of what the agent of a round did goes on from: the try, the first being one, and how far into what
/// that try kept of its program's stream, in bytes. A round is read from its start at [`Mark::default`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mark {
    pub attempt: usize,
    pub at: u64,
}

impl Default for Mark {
    fn default() -> Self {
        Mark { attempt: 1, at: 0 }
    }
}

/// What the agent of a round did from a mark on: each thing with the try it was done in, the first being one, in the
/// order done; and the mark a later read goes on from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Doings {
    pub done: Vec<(usize, Doing)>,
    pub next: Mark,
}

/// A work as the carrier of its project's repository is asked of it, off the loop: its number; its item, whose
/// project says which repository; where it merged and no pull request of it is kept, what its merge wrote, which
/// names the commit its merge made; and how far its records had come, with whether a round of it or a tool run for
/// it was under way, which say how long what git says of it holds.
#[derive(Clone, Debug)]
pub struct OfWork {
    pub work: WorkNumber,
    pub item: Item,
    pub merged: Option<String>,
    /// The place the work's next record takes ([`work::Work::tick`]).
    pub place: Seq,
    /// Whether a round of the work or a tool run for it is under way ([`work::Work::under_way`]): only these change
    /// what its branch and worktree hold, and each is recorded as it begins and ends.
    pub under_way: bool,
}

impl OfWork {
    /// The work as the driver keeps it, with what its merge wrote where that is given.
    pub fn of(kept: &driver::Kept<'_>, merged: Option<String>) -> OfWork {
        OfWork {
            work: kept.head.number,
            item: kept.item.clone(),
            merged,
            place: kept.work.tick(),
            under_way: kept.work.under_way(),
        }
    }
}

/// What the repository of a work's project says of the work beside its records.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Seen {
    /// What the work changed; none where nothing of it is there to tell, as before its branch is made.
    pub changed: Option<Diffstat>,
    /// The pull request the commit its merge made names, where it was asked for one and the commit names one.
    pub pull: Option<Pull>,
}

/// What a work changed, between where its branch left main and its latest head, together with what its worktree holds
/// that is not committed while the worktree is there: how many files, and how many lines added and removed in them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Diffstat {
    pub files: u64,
    pub added: u64,
    pub removed: u64,
}

/// A pull request on the host of a work's repository: its number, and its address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pull {
    pub number: u64,
    pub url: String,
}

/// A pull request open on the host of a project's repository, as a person lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenPull {
    pub number: u64,
    pub title: String,
    pub url: String,
    /// Whether it is a draft, which is not yet for anyone to review or merge.
    pub draft: bool,
    /// When the host last updated it, in milliseconds since 1970.
    pub updated: u64,
}

/// What the host of a project's repository says of its open pull requests: them, none where the host has no pull
/// requests, as the local repository, or why it could not say.
pub type PullsSaid = Result<Option<Vec<OpenPull>>, String>;

/// Each file a work changed, as [`Diffstat`] counts them, in the order of their paths; the commit where its branch left
/// main and its latest head they are between; and whether what its worktree holds was counted, the worktree being
/// there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Changes {
    pub base: String,
    pub head: String,
    pub worktree: bool,
    pub files: Vec<FileChanged>,
}

/// A file a work changed: its path; the lines added and removed, none of a binary file; and whether the work's head
/// holds a change of it, or only its worktree does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileChanged {
    pub path: String,
    pub added: u64,
    pub removed: u64,
    pub binary: bool,
    pub committed: bool,
}

/// The unified diff of one file a work changed, up to where it was cut, with its whole length in bytes where it was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diff {
    pub path: String,
    pub text: String,
    pub cut: Option<u64>,
}

/// What runs the tools that agents ask for.
///
/// Both methods are called inside the loop, between two things it takes: each returns at once, and does its work
/// elsewhere, on a task or a thread of its own, so that the loop is held up by no tool.
pub trait Tools: Send + Sync {
    /// Runs the tool. The call has its result when the reply gives it.
    fn call(&self, call: ToolCall, reply: CallReply);

    /// Withdraws the call, whose run was abandoned: the tool is stopped where it runs, and the call has the result of
    /// a withdrawn call ([`Carriers::calls`]) for its result, or the result that came before it could be stopped. A
    /// call has one result: where the reply it was made with is still held, that one is used and this one dropped,
    /// and where the call's result was sent already this one is dropped and nothing more is sent.
    fn withdraw(&self, call: ToolCall, reply: CallReply);
}

/// What looks at the network while the rounds of a service wait for it to be reached: whether the service, the agents'
/// or the acts' host, can be reached from here, as far as a look that costs no round and writes nothing can tell.
///
/// It is called off the loop, on a thread that may wait on the network for a few seconds.
pub trait Network: Send + Sync {
    /// Whether the network reaches the service now.
    fn reached(&self) -> bool;
}

/// What checks the outside the agents' rounds need while they wait for it, by itself and with no round: an account to
/// run under, the network to their service and a short request the service answers, each in turn, so that what is down
/// is said by the first that is. It touches no work's session, and costs at most that short request, which also says
/// the account's usage.
///
/// It is called off the loop, and may take as long as the short request may.
pub trait Outside: Send + Sync {
    /// What the outside is now, under the account of this name: the one in use, or one the owner's look at the
    /// accounts is taken under. With none, no account is in use, which is down with nothing run.
    fn check(&self, account: Option<String>) -> Pin<Box<dyn Future<Output = Heard> + Send + '_>>;

    /// What the outside is under this token of an account not yet kept: the check a token is taken on before it is
    /// kept, which finds a token the service refuses as a command line that cannot run.
    fn check_token(&self, token: Token) -> Pin<Box<dyn Future<Output = Heard> + Send + '_>>;
}

/// What a check of the outside heard: what it found, in words that hold no token, and what the service said of the
/// usage of the account or of the token it was taken under, none where it said nothing; a short request that met the
/// usage limit with no word of a window says the account is at its limit.
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    pub found: Found,
    pub usage: Option<Said>,
}

impl From<Found> for Heard {
    /// What was found, with no word of usage.
    fn from(found: Found) -> Heard {
        Heard { found, usage: None }
    }
}

/// The token of an account the agents' program may run under: a secret, given to the program alone, in a variable of
/// its environment, and to what keeps it outside the engine. It says nothing of itself in a debug print, and has no
/// other print and no form the store or a page could take it in.
#[derive(Clone, PartialEq, Eq)]
pub struct Token(String);

impl Token {
    pub fn new(secret: String) -> Token {
        Token(secret)
    }

    /// The secret, for the program's environment and for what keeps it.
    pub fn secret(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(..)")
    }
}

/// What keeps the tokens of the accounts the agents' program may run under, outside the engine, each under its
/// account's name; and what the program says of a token: the command that makes one, and how long one lasts. Each
/// method may wait on what keeps the tokens, and is called off the loop.
pub trait Tokens: Send + Sync {
    /// Keeps the token under the name, in place of any kept under it before.
    fn keep(&self, name: &str, token: &Token) -> Result<(), String>;

    /// The token kept under the name; or why it cannot be given, in the words of what keeps it.
    fn token(&self, name: &str) -> Result<Token, String>;

    /// Lets go of the token kept under the name; nothing where none is.
    fn let_go(&self, name: &str) -> Result<(), String>;

    /// Why no token can be kept or given now, in words, as where what keeps them is missing; none where one can. While
    /// it says so, no account is added or taken, and no agent runs.
    fn closed(&self) -> Option<String>;

    /// The command the owner runs, signed in to an account in the browser, to make a token of it.
    fn making(&self) -> String;

    /// How long a token lasts from when it is made, in milliseconds.
    fn life_ms(&self) -> u64;
}

/// Tokens kept in this process's memory alone, for tests and the scripted carriers.
#[derive(Default)]
pub struct KeptTokens {
    kept: Mutex<BTreeMap<String, Token>>,
    /// Why no token can be kept or given, where a test says so.
    refusing: Mutex<Option<String>>,
    /// Why no token can be let go of, where a test says so.
    holding: Mutex<Option<String>>,
}

/// A year, in milliseconds: how long a token lasts, as the agents' program says of the tokens it makes.
pub const YEAR_MS: u64 = 365 * 24 * 60 * 60 * 1000;

/// What a lock holds, whole whatever panicked while it was held: each change is one insert, take or set.
fn locked<T>(lock: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    lock.lock().unwrap_or_else(PoisonError::into_inner)
}

impl KeptTokens {
    /// Tokens kept in memory, none refused.
    pub fn new() -> KeptTokens {
        KeptTokens::default()
    }

    /// Tokens kept in memory, none kept or given for this reason.
    pub fn refusing(why: &str) -> KeptTokens {
        let tokens = KeptTokens::default();
        tokens.refuse(Some(why));
        tokens
    }

    /// From now on no token is kept or given, for this reason, as [`Tokens::closed`] says it; or, with none, every one
    /// is.
    pub fn refuse(&self, why: Option<&str>) {
        *locked(&self.refusing) = why.map(str::to_string);
    }

    /// From now on no token can be let go of, for this reason, or, with none, every one can.
    pub fn hold(&self, why: Option<&str>) {
        *locked(&self.holding) = why.map(str::to_string);
    }

    /// The names tokens are kept under.
    pub fn names(&self) -> Vec<String> {
        locked(&self.kept).keys().cloned().collect()
    }
}

impl Tokens for KeptTokens {
    fn keep(&self, name: &str, token: &Token) -> Result<(), String> {
        locked(&self.kept).insert(name.to_string(), token.clone());
        Ok(())
    }

    fn token(&self, name: &str) -> Result<Token, String> {
        (locked(&self.kept).get(name).cloned()).ok_or_else(|| format!("no token is kept under the name {name}"))
    }

    fn let_go(&self, name: &str) -> Result<(), String> {
        if let Some(why) = locked(&self.holding).clone() {
            return Err(why);
        }
        locked(&self.kept).remove(name);
        Ok(())
    }

    fn closed(&self) -> Option<String> {
        locked(&self.refusing).clone()
    }

    fn making(&self) -> String {
        "claude setup-token".to_string()
    }

    fn life_ms(&self) -> u64 {
        YEAR_MS
    }
}

/// What carries out each kind of todo.
#[derive(Clone)]
pub struct Carriers {
    /// The rounds of the cells an agent carries out.
    pub agents: Arc<dyn Rounds>,
    /// The rounds of the cells that are the engine's own acts.
    pub acts: Arc<dyn Rounds>,
    pub tools: Arc<dyn Tools>,
    /// How many rounds of agents may go on at once as the engine starts: it gives the agents no more than this, until the
    /// owner sets other slots, and the others wait for a slot in the engine's order. The engine's own acts take none.
    pub slots: usize,
    /// The check of the outside the agents' rounds need, while they wait for it.
    pub outside: Arc<dyn Outside>,
    /// The look at the network to the host the acts reach; none where no network reaches it, as the local repository.
    pub host: Option<Arc<dyn Network>>,
    /// What keeps the tokens of the accounts the agents' program may run under.
    pub tokens: Arc<dyn Tokens>,
    /// The result a call has where the engine ends it itself, in the words beside the flows: the loop gives it to a
    /// run at its limit and to the engine's wait, and the tools give it to a tool's call they stop.
    pub calls: flow::agents::Calls,
}

impl Carriers {
    /// What carries out the rounds of cells of this kind.
    pub fn rounds(&self, kind: Kind) -> &Arc<dyn Rounds> {
        match kind {
            Kind::Agent => &self.agents,
            Kind::Engine => &self.acts,
        }
    }

    /// What the round was told and returned, as the carrier of its cell's kind kept them; for a cell of no kind
    /// known, one the flow no longer has, as the first carrier that kept anything of it. It waits on the disk.
    pub fn told(&self, kind: Option<Kind>, work: WorkNumber, run: RunId, round: usize) -> Result<Told, String> {
        let carriers = match kind {
            Some(kind) => vec![self.rounds(kind)],
            None => vec![&self.agents, &self.acts],
        };
        let mut told = Told::default();
        for rounds in carriers {
            told = rounds.told(work, run, round)?;
            if !told.tries.is_empty() {
                break;
            }
        }
        Ok(told)
    }

    /// The system prompt the line's session of this number, the line's first being none, began with, as the agents'
    /// carrier kept it. It waits on the disk.
    pub fn system(&self, work: WorkNumber, line: &str, session: u32) -> Result<Option<String>, String> {
        self.agents.system(work, line, session)
    }

    /// What the agent of the round did from the mark on, as the carrier of its cell's kind keeps it, or, for a cell of
    /// no kind known, as the first carrier that keeps anything of it from there. It waits on the disk.
    pub fn doing(
        &self,
        kind: Option<Kind>,
        (work, run, round): (WorkNumber, RunId, usize),
        from: Mark,
    ) -> Result<Doings, String> {
        let carriers = match kind {
            Some(kind) => vec![self.rounds(kind)],
            None => vec![&self.agents, &self.acts],
        };
        let mut doings = Doings {
            done: Vec::new(),
            next: from,
        };
        for rounds in carriers {
            doings = rounds.doing(work, run, round, from)?;
            if !doings.done.is_empty() || doings.next != from {
                break;
            }
        }
        Ok(doings)
    }

    /// What the repository of the work's project says of it beside its records, as the carrier of the engine's acts,
    /// which keeps that repository, works it out.
    pub async fn seen(&self, of: OfWork) -> Result<Seen, String> {
        self.acts.seen(of).await
    }

    /// Each file the work changed, as the carrier of the engine's acts works it out.
    pub async fn changes(&self, of: OfWork) -> Result<Option<Changes>, String> {
        self.acts.changes(of).await
    }

    /// The unified diff of one file the work changed, as the carrier of the engine's acts works it out.
    pub async fn diff(&self, of: OfWork, path: String) -> Result<Option<Diff>, String> {
        self.acts.diff(of, path).await
    }

    /// The pull requests open on the host of this project's repository, as the carrier of the engine's acts asks the
    /// host.
    pub async fn open_pulls(&self, project: &str) -> PullsSaid {
        self.acts.open_pulls(project).await
    }
}

/// A round of a run, to give or to stop.
#[derive(Clone, Debug)]
pub struct Round {
    pub work: WorkNumber,
    /// The flow the work runs on.
    pub flow: Arc<Compiled>,
    pub cell: CellSpec,
    pub run: RunId,
    /// Which round of the run it is, the first being one.
    pub number: usize,
    pub given: Given,
    /// The item the work is done for: its title and the items it depends on, which do not change once it is added.
    pub item: Item,
    /// The work as it was kept when the round was handed out.
    pub kept: Arc<Work>,
    /// What the round is judged by while it runs: the lines the history of its cell and its flow give, in every work
    /// on the flow (crate `driver`, module `history`).
    pub lines: Lines,
    /// The account the round runs under, by its name: the one in use as it was handed out, every try of it running
    /// under it. Where none was, the round fails as its token cannot be given (decision no-default-credential).
    pub account: Option<String>,
}

/// A call of a run that a tool answers, to make or to withdraw.
#[derive(Clone, Debug)]
pub struct ToolCall {
    pub work: WorkNumber,
    pub flow: Arc<Compiled>,
    pub run: RunId,
    pub call: CallId,
    /// The tool.
    pub name: String,
    /// What it is run on.
    pub input: String,
    /// The item the work is done for, as a round is given it.
    pub item: Item,
    /// The work as it was kept when the call was handed out.
    pub kept: Arc<Work>,
    /// For a call of the tool that runs the project's tests: how long one test may run before it is stopped, in
    /// milliseconds, as the history of the project's merged checks says it (crate `driver`, module `timed`).
    pub test_time: Option<f64>,
}

/// The way to say once that a round ended, or that it failed.
#[derive(Debug)]
pub struct RoundReply {
    work: WorkNumber,
    run: RunId,
    to: mpsc::UnboundedSender<Message>,
}

/// The way to give a call its result, once.
#[derive(Debug)]
pub struct CallReply {
    work: WorkNumber,
    call: CallId,
    to: mpsc::UnboundedSender<Message>,
}

impl RoundReply {
    /// The reply that says what came of it down this channel, as the loop makes it; whoever tests a carrier makes one
    /// to read what the carrier says.
    pub fn new(work: WorkNumber, run: RunId, to: mpsc::UnboundedSender<Message>) -> Self {
        RoundReply { work, run, to }
    }

    /// The way to say, while the round is under way, what each of its tries runs on and which session of its line it
    /// began for a reason of its own.
    pub fn teller(&self) -> Teller {
        Teller {
            work: self.work,
            run: self.run,
            to: self.to.clone(),
        }
    }

    /// The way to say where the round of this number of the run, the first being one, stands as it goes, while it is
    /// under way.
    pub fn saying(&self, round: usize) -> Saying {
        Saying {
            work: self.work,
            under: Under::Round { run: self.run, round },
            to: self.to.clone(),
        }
    }

    /// The way to say what the host found of the work while the round is under way.
    pub fn hosting(&self) -> Hosting {
        Hosting {
            work: self.work,
            to: self.to.clone(),
        }
    }

    /// The round ended: this came back, and it took this.
    pub fn ended(self, came: Came, spent: Spent) {
        let event = Event::Returned {
            run: self.run,
            came,
            spent,
        };
        // Where the loop has ended, the process is stopping: the round is handed out again when it starts again.
        let _ = self.to.send(Message::Happened { work: self.work, event });
    }

    /// The round ended as [`RoundReply::ended`] says, and its work is to wait for the owner, for this reason, which
    /// the owner reads: the carrier found that the work should not go on before the owner has looked, as when it has
    /// spent more than it may. The engine's pause and then the end are sent down the loop's one channel in that order,
    /// so that the loop takes the pause while the round is still under way: what came back is taken as from any
    /// round, and no next round begins. The owner lets the work on again as from any pause.
    pub fn ended_and_paused(self, came: Came, spent: Spent, why: String) {
        let paused = Event::Paused {
            by: Pauser::Engine,
            why: Some(why),
            waits_for: Vec::new(),
        };
        // Where the loop has ended, the process is stopping: the round is handed out again when it starts again.
        let _ = self.to.send(Message::Happened {
            work: self.work,
            event: paused,
        });
        self.ended(came, spent);
    }

    /// The round failed, for this reason, having taken this: nothing came back, for a cause no kind tells apart. The
    /// engine pauses the work with the reason as it takes this; once the owner lets the work on, the run's next round
    /// is given what this one was.
    pub fn failed(self, why: String, spent: Spent) {
        self.failed_as(Failure::Other, None, why, spent);
    }

    /// The round failed as [`RoundReply::failed_as`] says, and its work is to wait for the owner, for this reason, as
    /// when the round took it over its budget: the pause is sent first, as [`RoundReply::ended_and_paused`] sends it.
    pub fn failed_and_paused(self, failure: Failure, resets: Option<Time>, why: String, spent: Spent, pause: String) {
        let paused = Event::Paused {
            by: Pauser::Engine,
            why: Some(pause),
            waits_for: Vec::new(),
        };
        // Where the loop has ended, the process is stopping: the round is handed out again when it starts again.
        let _ = self.to.send(Message::Happened {
            work: self.work,
            event: paused,
        });
        self.failed_as(failure, resets, why, spent);
    }

    /// The round failed, for this reason, having taken this, for a cause of this kind, as the carrier told it apart;
    /// `resets` says when the account's usage limit resets, where that is the cause and its report says when. The
    /// engine handles the failure as its kind says.
    pub fn failed_as(self, failure: Failure, resets: Option<Time>, why: String, spent: Spent) {
        let event = Event::Failed {
            run: self.run,
            why,
            failure,
            resets,
            spent,
        };
        // Where the loop has ended, the process is stopping: the round is handed out again when it starts again.
        let _ = self.to.send(Message::Happened { work: self.work, event });
    }
}

/// The way to say what happens while a round is under way, as often as it happens: each is taken while the round is
/// still under way, before its end.
#[derive(Clone, Debug)]
pub struct Teller {
    work: WorkNumber,
    run: RunId,
    to: mpsc::UnboundedSender<Message>,
}

/// The way to say where a round or a call stands as it goes, which no record holds: the engine keeps the latest while
/// it is under way, and the page shows it (crate `driver`, module `along`). Said again and again, it costs nothing
/// where nothing changed.
#[derive(Clone, Debug)]
pub struct Saying {
    work: WorkNumber,
    under: Under,
    to: mpsc::UnboundedSender<Message>,
}

/// The way to say what the host of a work's repository found of the work while a round or a call of it is carried
/// out, which is kept with the work: the pull request the work is checked through.
#[derive(Clone, Debug)]
pub struct Hosting {
    work: WorkNumber,
    to: mpsc::UnboundedSender<Message>,
}

impl Hosting {
    /// The work is checked through the pull request of this number, at this address; said again at each check, it is
    /// kept once.
    pub fn pulled(&self, number: u64, url: String) {
        let event = Event::Pulled { number, url };
        // Where the loop has ended, the process is stopping: what is given again finds the pull request again.
        let _ = self.to.send(Message::Happened { work: self.work, event });
    }

    /// The host read a run of the project's CI, which this call or round asked for, and says how long its tests and
    /// jobs took, or that it gives no time; said again for the same run, it is kept once.
    pub fn timed(&self, reading: work::Reading) {
        let event = Event::Timed { reading };
        // Where the loop has ended, the process is stopping: what is given again reads the run again.
        let _ = self.to.send(Message::Happened { work: self.work, event });
    }

    /// The project's check the host ran for the work, for a round of an act or a call, ended by itself and left these
    /// many processes running in its group, which the host stopped: said before the round's end or the call's result.
    pub fn left_behind(&self, of: Left, processes: usize) {
        let event = Event::LeftBehind { of, processes };
        // Where the loop has ended, the process is stopping: what the check left is not counted.
        let _ = self.to.send(Message::Happened { work: self.work, event });
    }
}

impl Saying {
    /// Where it stands now; none where it no longer stands anywhere it says, as once the CI run it ran as has ended.
    pub fn say(&self, along: Option<Along>) {
        // Where the loop has ended, the process is stopping: what is given again says it again.
        let _ = self.to.send(Message::Along {
            work: self.work,
            under: self.under,
            along,
        });
    }
}

impl Teller {
    /// The try of this number of the round, the first being one, begins on this.
    pub fn ran_on(&self, attempt: usize, on: RanOn) {
        self.tell(Event::Ran {
            run: self.run,
            attempt,
            on,
        });
    }

    /// The message of the try of this number of the round, the first being one, fills this into its session: said
    /// after the try's beginning and before its message is given, so that what a session holds is in the records.
    pub fn filled(&self, attempt: usize, fills: Vec<Fill>) {
        self.tell(Event::Filled {
            run: self.run,
            attempt,
            fills,
        });
    }

    /// What the calls to the model of the try of this number of the round came to, as the carrier read them: said
    /// before the round's end, so that it is in the records with the round.
    pub fn measured(&self, attempt: usize, measures: Measures) {
        self.tell(Event::Measured {
            run: self.run,
            attempt,
            measures,
        });
    }

    /// The program of the try of this number of the round of this number ended by itself and left these many
    /// processes running in its group, which the carrier stopped: said before the round's end, so that it is in the
    /// records with the round.
    pub fn left_behind(&self, round: usize, attempt: usize, processes: usize) {
        let of = Left::Try {
            run: self.run,
            round,
            attempt,
        };
        self.tell(Event::LeftBehind { of, processes });
    }

    /// The line's session was reopened during the round, for this reason: the agents' program had lost the one the
    /// round was to go on in, or compacted it.
    pub fn session_began(&self, line: String, why: Why) {
        self.tell(Event::Anew { line, why });
    }

    /// The source outside the work that writes the block says this of it now: the carrier read it of what the round
    /// leaves, as the design tree on the work's branch after a round that committed to it. It is written as a new
    /// version of the block, while the round is under way.
    pub fn outside_wrote(&self, block: BlockId, content: String) {
        self.tell(Event::Wrote {
            block,
            by: Outsider::Outside,
            content,
            matters: true,
            value: None,
        });
    }

    /// As [`Teller::outside_wrote`], for the round: the version is put into it, so that the round is told of it and its
    /// run consumes it; where the block says it already, the version that says it is put in, and none is written. The
    /// carrier does it for what it did or read itself for the round, as taking main in.
    pub fn outside_wrote_for_round(&self, block: BlockId, content: String) {
        self.tell(Event::WroteForRound {
            run: self.run,
            block,
            content,
        });
    }

    /// The round was given what the owner said to it, which the engine gave to be told it ([`Rounds::tell`]): these
    /// versions are put into it. Said before the round's end.
    pub fn said_to_round(&self, versions: Vec<(BlockId, decide::Seq)>) {
        self.tell(Event::SaidToRound {
            run: self.run,
            versions,
        });
    }

    /// The agents' service said this of the usage of the account of this name, which the round runs under.
    pub fn usage(&self, account: String, said: Said) {
        let _ = self.to.send(Message::Usage { account, said });
    }

    /// The service of the agents answered the round with a message that is no error: it serves the round, which a wait
    /// every work shares for it may be over by.
    pub fn answered(&self) {
        let _ = self.to.send(Message::Answered {
            work: self.work,
            run: self.run,
        });
    }

    fn tell(&self, event: Event) {
        // Where the loop has ended, the process is stopping: the round is handed out again when it starts again.
        let _ = self.to.send(Message::Happened { work: self.work, event });
    }
}

impl CallReply {
    /// The reply that says what came of it down this channel, as the loop makes it; whoever tests a carrier makes one
    /// to read what the carrier says.
    pub fn new(work: WorkNumber, call: CallId, to: mpsc::UnboundedSender<Message>) -> Self {
        CallReply { work, call, to }
    }

    /// The way to say where the call stands as it goes, while it has no result.
    pub fn saying(&self) -> Saying {
        Saying {
            work: self.work,
            under: Under::Call(self.call),
            to: self.to.clone(),
        }
    }

    /// The way to say what the host found of the work while the call has no result.
    pub fn hosting(&self) -> Hosting {
        Hosting {
            work: self.work,
            to: self.to.clone(),
        }
    }

    /// The call has this result.
    pub fn done(self, result: String) {
        let event = Event::Done {
            call: self.call,
            result,
        };
        // Where the loop has ended, the process is stopping: the call is handed out again when it starts again.
        let _ = self.to.send(Message::Happened { work: self.work, event });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rounds that keep, of every round, one try with this message, or nothing.
    struct Keeps(Option<&'static str>);

    impl Rounds for Keeps {
        fn give(&self, _: Round, _: RoundReply) {}

        fn stop(&self, _: Round, _: RoundReply) {}

        fn told(&self, _: WorkNumber, _: RunId, _: usize) -> Result<Told, String> {
            let tries = (self.0.iter())
                .map(|message| Try {
                    message: message.to_string(),
                    cut: None,
                    back: Back::Nothing,
                    ran: None,
                })
                .collect();
            Ok(Told { tries, left_out: 0 })
        }
    }

    struct NoTools;

    impl Tools for NoTools {
        fn call(&self, _: ToolCall, _: CallReply) {}

        fn withdraw(&self, _: ToolCall, _: CallReply) {}
    }

    struct Well;

    impl Outside for Well {
        fn check(&self, _: Option<String>) -> Pin<Box<dyn Future<Output = Heard> + Send + '_>> {
            Box::pin(async { Found::Well.into() })
        }

        fn check_token(&self, _: Token) -> Pin<Box<dyn Future<Output = Heard> + Send + '_>> {
            Box::pin(async { Found::Well.into() })
        }
    }

    fn carriers(agents: Option<&'static str>, acts: Option<&'static str>) -> Carriers {
        Carriers {
            agents: Arc::new(Keeps(agents)),
            acts: Arc::new(Keeps(acts)),
            tools: Arc::new(NoTools),
            slots: 1,
            outside: Arc::new(Well),
            host: None,
            tokens: Arc::new(KeptTokens::new()),
            calls: crate::Scripted::calls(),
        }
    }

    #[test]
    fn a_token_says_nothing_of_itself_in_a_debug_print() {
        let token = Token::new("sk-ant-oat01-secret".to_string());
        assert_eq!(format!("{token:?}"), "Token(..)");
        assert_eq!(format!("{:?}", Some(token.clone())), "Some(Token(..))");
        assert_eq!(token.secret(), "sk-ant-oat01-secret");
    }

    fn messages(told: Told) -> Vec<String> {
        told.tries.into_iter().map(|t| t.message).collect()
    }

    #[test]
    fn a_round_is_told_by_the_carrier_of_its_cells_kind() {
        let carriers = carriers(Some("an agent's"), Some("an act's"));
        let told = |kind| messages(carriers.told(Some(kind), WorkNumber(1), RunId(1), 1).unwrap());
        assert_eq!(told(Kind::Agent), ["an agent's"]);
        assert_eq!(told(Kind::Engine), ["an act's"]);
    }

    #[test]
    fn a_round_of_a_cell_the_flow_no_longer_has_is_told_by_the_first_carrier_that_kept_it() {
        let told = |carriers: Carriers| messages(carriers.told(None, WorkNumber(1), RunId(1), 1).unwrap());
        assert_eq!(told(carriers(Some("an agent's"), None)), ["an agent's"]);
        assert_eq!(told(carriers(None, Some("an act's"))), ["an act's"]);
        assert!(told(carriers(None, None)).is_empty());
    }
}
