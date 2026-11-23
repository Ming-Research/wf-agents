//! What a run does next, from what it has done so far.
//!
//! A run is one entering of its cell (design document `data-and-context`, 7). It is made of rounds: the first, each one
//! after a return was sent back, and each one after a round failed. At the end of a round its agent, or the engine's
//! own act, returns, and a return is of one of two kinds (design document `data-and-context`, 30): it asks for calls
//! and for nothing else, dealing with nothing; or it does the cell's work, saying which of what ran the round it has
//! dealt with and writing its blocks. A return that does not meet what the flow requires, one that mixes the two among
//! it, is sent back: the next round is given why. One that meets it is taken, and ends the run: the calls it asks for
//! are made, or the run writes its blocks and what it dealt with is consumed. A cell whose run asked for calls is
//! entered again as a new run once their results are in, which the walk says. How often a return is sent back has a
//! limit, at which the owner lets the run go on or cancels the work.
//!
//! A round may also fail: nothing came back, as when whatever carries it out could not start or gave nothing usable.
//! Whatever carried it out tells the cause apart ([`Failure`]), and each kind is handled its own way. A failure in
//! passing is tried again, after a wait that grows each time, as a call of the run that the engine makes; so many in a
//! row and the next pauses the work ([`pausing`]). A return that could not be read is sent back, as any return that
//! does not meet what the flow requires. A round the service's safety check flagged, and one the run guard stopped for a
//! context its cell's history does not explain, is tried again in a new session of its line, and the second of the same
//! kind in a row pauses the work. A cause outside every work, the same for all (the account's usage limit, a command
//! line that cannot run, no network, a service that is down), holds the agents' rounds of every work, which is no
//! matter of one run's: the run only has its next round once they may begin again, and so many failures of the run in
//! a row for such a cause, each after the engine found the outside well, and the next pauses the work. Any other
//! failure pauses the work as the round fails, so that the owner sees the cause first. Once the work is let on, the run has its next round, given what the failed round was
//! given, and the failures counted in a row start anew. A round that failed, or a return that was sent back, deals with
//! nothing.
//!
//! A pause of the work holds back the rounds of a run, unless the run started for a note a person wrote: such a run has
//! every round it needs while the work is paused, but the round after one that failed and paused the work, which still
//! waits for the work to be let on.
//!
//! The walk says which cell runs and which run goes on; this says what going on is.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::flow::{self, BlockId};
use crate::records::{CallId, Seq, Write};

/// What a run has done so far: its rounds, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Log {
    pub rounds: Vec<Round>,
    /// How many of the rounds had ended when the owner last let the run go on at a limit; none at first. The counts
    /// that have a limit start anew from there.
    pub let_go: usize,
    /// How many of the rounds had ended when the owner last let the work on after a pause; none at first. The failures
    /// counted in a row start anew from there.
    pub let_on: usize,
}

/// How many failures in passing in a row are tried again, and as many for a cause outside every work: the next one
/// pauses the work, running again not having helped.
pub const PASSING: u32 = 8;

/// How long the engine waits after the first failure in passing in a row before the round is tried again, in
/// milliseconds; each later one in the row waits twice as long as the one before, up to [`LONGEST_WAIT`].
pub const FIRST_WAIT: u64 = 10_000;

/// The longest the engine waits before a round that failed in passing is tried again, in milliseconds.
pub const LONGEST_WAIT: u64 = 300_000;

/// How long the engine waits after the round that failed in passing, the `failed`th in a row, before it is tried
/// again, in milliseconds.
pub fn wait(failed: u32) -> u64 {
    let doublings = failed.saturating_sub(1).min(16);
    FIRST_WAIT.saturating_mul(1 << doublings).min(LONGEST_WAIT)
}

/// Why a round failed, as whatever carried it out told it apart. Each kind is handled its own way (the module's
/// documentation).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Failure {
    /// In passing, the round's own: the command line crashed after it reached the service, the service's answer to
    /// it was cut off, or the model gave what could not be taken. The round is tried again after a wait.
    Passing,
    /// What the agent returned could not be read: the command line gave up on it, or it is not of the form of a
    /// return. It is sent back.
    Unread,
    /// The service's safety check flagged the round, stopping it or refusing its message. It is tried again in a new
    /// session of its line, which does not give the check the same context again.
    Flagged,
    /// The run guard stopped the round: its context grew past what its cell's history explains. It is tried again in a
    /// new session of its line, since the same session would give the next round the same context.
    Guarded,
    /// The account's usage limit was reached: no agent round of any work begins until it resets.
    UsageLimit,
    /// The agents' command line cannot run at all: no account is in use, the account's token cannot be given or was
    /// refused, or the program is missing or ended before it began, having reached nothing. No agent round of any work
    /// begins until it runs.
    CannotRun,
    /// The network cannot be reached: no agent round of any work begins until it can be.
    Offline,
    /// The agents' service did not serve the round: it failed, was overloaded, throttled the account's requests or did
    /// not answer in time. No agent round of any work begins until it serves again.
    Unavailable,
    /// The prompt is too long for the model: running it again cannot help, and the work is paused.
    PromptTooLong,
    /// The service has no model of the name the round's level gives: no round on it can run until the installation
    /// names another, so the work is paused, and so is every other work that runs a round on that model.
    NoModel,
    /// A cause no other kind holds: a request the service refused, an error the command line reported of no kind its
    /// fields or its words tell, a refusal of the engine's act by its host or what only a person mends there, or a
    /// round that could not be carried out at all. Running again meets it again, and the work is paused. Rounds that
    /// failed before kinds were told apart read so, since each paused its work.
    #[default]
    Other,
}

impl Failure {
    /// Whether the cause is outside every work and the same for all, so that it holds the agents' rounds of every work
    /// rather than one run.
    pub fn shared(self) -> bool {
        matches!(
            self,
            Failure::UsageLimit | Failure::CannotRun | Failure::Offline | Failure::Unavailable
        )
    }
}

/// Why the engine pauses the work as a round of its run fails.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pausing {
    /// Running the round again cannot help, by its kind.
    Failed(Failure),
    /// The round failed in passing this many times in a row, and running it again has not helped.
    Passing { failed: u32 },
    /// The round failed this many times in a row for a cause outside every work, each try begun once the engine found
    /// the outside well: what the rounds meet is not what the engine's check finds.
    Outside { failed: u32 },
    /// The round was flagged once more, in the new session its line was given for the time before.
    FlaggedAgain,
    /// The run guard stopped the round once more, in the new session its line was given for the time before.
    GuardedAgain,
}

/// A round of a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Round {
    /// What came back at the end of the round; `None` while the round is under way.
    pub returned: Option<Return>,
    /// What the return was sent back for, as the round after it was told: none where no round after it began, or it
    /// was not sent back. The round after it is told only what is wrong, and the rest of what was returned stands
    /// ([`together`]).
    pub sent_back: Vec<Unmet>,
}

/// What came back at the end of a round.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Return {
    /// The return was read: what of what ran the round it dealt with, in the order it named them, each datum by its
    /// block or, of a group, by the group's name ([`flow::stands_on`]); the blocks it writes,
    /// and how many calls it asks for; what was found wrong with it as it was read, which only its reading could see;
    /// and whether a question it asks the owner waits for the answer.
    Came {
        dealt_with: Vec<BlockId>,
        writes: Vec<Write>,
        calls: usize,
        /// What was found wrong with the return as it was read against its cell, in the order found: what it named
        /// that is none of its cell's, and what it wrote in a shape its cell does not take ([`Unmet::NotWatched`],
        /// [`Unmet::NotATool`], [`Unmet::UnchangedNotWritten`], and others of a block the reading could not hand in).
        found: Vec<Unmet>,
        /// A question the return asks the owner waits for the answer: the work stops at the cell, and the return is
        /// held to no block its cell must write.
        waits: bool,
        /// The questions it asks, asks anew or withdraws, in the order it wrote them.
        questions: Vec<Asking>,
        /// The blocks of the work's branch the carrier read new as the round ended, as what the round committed: the
        /// run writes each as it finishes, beside the blocks it writes itself, unless the round asks for calls or its
        /// cell still waits (see [`next`]).
        read: Vec<BlockId>,
        /// Why it says the work changes each file the project protects, by the file's path.
        protected: BTreeMap<String, String>,
    },
    /// The round failed: nothing came back, for this kind of cause, said so.
    Failed {
        failure: Failure,
        why: String,
        /// The waits the engine made as calls after it, before it is tried again: none until the engine makes one.
        waits: Vec<Call>,
    },
}

/// A question a return writes, by its name: asked or asked anew, or withdrawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asking {
    pub name: String,
    pub withdrawn: bool,
    /// Whether the question waits for the owner's answer.
    pub waits: bool,
}

/// A question of the work as a round of a cell is told of it: whether it is the cell's own, which the cell's agent
/// asked, and so may write anew or withdraw; whether it stands; and whether it stands with no answer to its latest
/// version, an answer being still to come.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Asked {
    pub own: bool,
    pub stands: bool,
    pub unanswered: bool,
}

/// A call made for a round.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    pub call: CallId,
    /// Whether the call has its result.
    pub done: bool,
}

/// What the run's round was told, and how the blocks its cell may write stand now, as the walk reads them from the
/// records ([`crate::told`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Told {
    /// What ran the round: each block new to the cell as the run started, in the order the cell names them, then each
    /// block put in to the round since that was not among them. Its return may name as dealt with each datum or group
    /// the cell watches that stands for one of them ([`flow::ran`]).
    pub ran_by: Vec<BlockId>,
    /// Of what ran the round, each dialogue its cell holds as the maker in which a point of the other side was new as
    /// the run began, while the dialogue was open: the maker owes the next word in it (design document
    /// `data-and-context`, 41).
    pub points: Vec<BlockId>,
    /// Of what ran the round, each dialogue its cell judges in in which it was woken as the run began, by a version of
    /// the dialogue new to it, an answer, or by a change of what it judges: the judge owes its verdict in it, a fail
    /// with its points (design document `data-and-context`, 41).
    pub woken_to_judge: Vec<BlockId>,
    /// Of each block the cell may write, the version the round was told of and the latest.
    pub outputs: BTreeMap<BlockId, Standing>,
    /// Whether the round's cell still waits for the results of calls its runs asked for, or for the answer to a question
    /// of its asked before the run began: the round was entered for the owner turning to the cell meanwhile, answers
    /// and goes on waiting, and so is held to no block its cell must write (design document `data-and-context`, 36).
    pub waits: bool,
    /// Whether the round was given the telling to hand in what it has now, for the owner turning to a cell before its
    /// own, after it began: it may hand in none of what its cell must write, and nothing as dealt with (design document
    /// `data-and-context`, 35). Whatever it does hand in is judged as any return is.
    pub hurried: bool,
    /// Each question of the work, by its name, as the round's cell stands to it.
    pub questions: BTreeMap<String, Asked>,
}

/// A block a cell may write: the version its run's round was told of, and the latest, with its value.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Standing {
    /// The latest version the round was told of: the one the run started from, or one put in to it since.
    pub told: Option<Seq>,
    /// The block's latest version.
    pub latest: Option<Seq>,
    /// The latest version's value, where the block is a choice.
    pub value: Option<String>,
    /// The latest version holds nothing: no value, and its content is empty or only whitespace.
    pub empty: bool,
    /// A version written of it is an entry added to a record ([`crate::Flow::adds_entries`]): it replaces no version
    /// before it, and so writes over none the round was not told of.
    pub added: bool,
}

/// What the flow requires of what a cell's run returns.
#[derive(Clone, Copy, Debug)]
pub struct Requires<'a> {
    /// The blocks the cell may write.
    pub outputs: &'a [BlockId],
    /// The groups of blocks the cell watches, by the group's name, with the group's blocks
    /// ([`crate::Cell::groups`]).
    pub groups: &'a BTreeMap<BlockId, Vec<BlockId>>,
    /// The blocks among those that every return that asks for no call includes, but one with a question that waits
    /// for the owner's answer, one of a round whose cell still waits for the results of calls ([`Told::waits`]), and
    /// one that deals with the cell's own words alone, writes nothing but its answer in them and asks nothing
    /// ([`Requires::words`]).
    pub must_write: &'a [BlockId],
    /// The values of each block that is a choice.
    pub choices: &'a BTreeMap<BlockId, Vec<String>>,
    /// Of each choice, by a value of it, the other blocks a return that writes it with that value may hold beside it,
    /// and no more of what its cell may write: no other block, and no question it asks or withdraws, but where these
    /// name the cell's questions (`with_nothing_but` of the choice's file).
    pub nothing_but: &'a BTreeMap<BlockId, BTreeMap<String, Vec<BlockId>>>,
    /// The cell's own questions, where it may ask: the block a return writes the questions it asks or withdraws in.
    pub questions: Option<&'a BlockId>,
    /// The cell's own words, its dialogue with the owner, where it has them. A return that deals with them alone,
    /// writes nothing but its answer in them and asks nothing answers the owner first, and goes on with the cell's work
    /// in later rounds (design document `data-and-context`, 35): it owes no block its cell must write.
    pub words: Option<&'a BlockId>,
    /// How often what the run returns may be sent back.
    pub returns: u32,
    /// What the round was told, and how the blocks it may write stand.
    pub told: &'a Told,
    /// Of each block that runs the cell and that an act of the engine's after it writes, what runs that act again.
    pub answered_by: &'a BTreeMap<BlockId, Answer>,
    /// Of each dialogue the cell holds as the maker, the blocks the other side watches that it writes
    /// ([`crate::Cell::dialogues`]).
    pub dialogues: &'a BTreeMap<BlockId, Vec<BlockId>>,
    /// Of each verdict the cell hands in as a judge, the value that does not pass and its dialogue
    /// ([`crate::Cell::verdicts`]).
    pub verdicts: &'a BTreeMap<BlockId, crate::Verdict>,
}

/// What a run does next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// The run has its first round.
    Begin,
    /// A round of the run is under way: nothing is decided until it ends.
    UnderWay,
    /// The run would have its next round, and the pause of its work holds that round back until the work is let on.
    Held,
    /// The run rests at these calls, which have no result yet.
    Rest { calls: Vec<CallId> },
    /// The last round failed in passing, the `failed`th time in a row: the engine waits this long from its end, in
    /// milliseconds, as a call of the run that it makes itself, before the round is tried again.
    Wait { failed: u32, ms: u64 },
    /// The last round failed: the run has its next round, which is given what the failed round was given.
    Retry,
    /// What came back does not meet what the flow requires: it is sent back, the next round being given why.
    SendBack { unmet: Vec<Unmet> },
    /// What came back meets everything the flow requires: the calls it asks for are made, and the run finishes,
    /// having dealt with these blocks, each once, of what ran its round, and writing these blocks.
    Finish {
        dealt_with: Vec<BlockId>,
        writes: Vec<Write>,
    },
    /// The run has reached a limit: the owner lets it go on, the count starting anew, or cancels the work.
    AtLimit(Limit),
}

/// A limit a run has reached.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Limit {
    /// This many returns were sent back, and what the last round returned does not meet what the flow requires
    /// either.
    Returns { sent_back: u32 },
    /// This many calls were made, and with those the last round asks for the run would have made more than it may. A
    /// run reached it before a return that asks ended its run; the walk now bounds how often a cell is entered again
    /// for the results of what it asked for.
    Calls { made: u32 },
}

/// Why what came back does not meet what the flow requires.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unmet {
    /// The round asked for calls, and for none: it neither asked for anything nor handed anything in. Returns of the
    /// form before a return said what it dealt with were sent back so.
    AsksNothing,
    /// The block is not one the cell may write.
    NotAnOutput { block: BlockId },
    /// The block was written before in the same return.
    WrittenTwice { block: BlockId },
    /// The block is a choice, and was written with no value.
    NoValue { block: BlockId },
    /// The block is a choice, and this is not one of its values.
    NotAValue { block: BlockId, value: String },
    /// The block is no choice, and was written with a value.
    NotAChoice { block: BlockId },
    /// The block is one the cell must hand in each time, and was not handed in.
    NotHandedIn { block: BlockId },
    /// The return writes the choice, `block`, with `value`, with which a return holds no other block its cell may
    /// write but those the choice's file names (`with_nothing_but`), and it holds these besides, in the order it writes
    /// them, and the cell's questions last where it asks or withdraws one.
    NothingBut {
        block: BlockId,
        value: String,
        besides: Vec<BlockId>,
    },
    /// What came back could not be read as a return, for this reason, as whatever carried the round out says.
    Unreadable { why: String },
    /// The return named the block, or the group of this name, as dealt with, and it did not run the round. Returns are
    /// no longer sent back for it: it changes nothing, and is left out of what they deal with ([`standing_names`]). Kept
    /// to read the reasons of rounds sent back before.
    NotRunBy { block: BlockId },
    /// The return named the block, or the group of this name, as dealt with twice. Returns are no longer sent back for
    /// it, as for [`Unmet::NotRunBy`].
    DealtWithTwice { block: BlockId },
    /// Something ran the round, and the return deals with none of it, asks for no call and has no question that waits
    /// for the owner's answer: the cell would be entered again at once for the same.
    DealtWithNothing,
    /// The block got this version from another writer after the round was told of the one it writes over: the return
    /// would write over what the round never saw. The next round is told of the version. An entry added to a record
    /// writes over nothing.
    WrittenOver { block: BlockId, version: Seq },
    /// The block is a choice, written as changing nothing in meaning with another value than the one it has: a choice
    /// changes its meaning when its value does (decision `meaning-told-by-the-writer`).
    ChoiceChanged { block: BlockId },
    /// The return asks for calls and also does the cell's work, dealing with something or writing a block, or is of
    /// both kinds whatever each holds: a return that asks for calls asks for nothing else, and deals with nothing
    /// (design document `data-and-context`, 30).
    AsksAndWorks,
    /// The round was run by a point of the other side in a dialogue the cell holds as the maker, and the return names
    /// the dialogue as dealt with and writes no version of it, none of `watched`, what the other side watches, and no
    /// question the work waits for: the next word stays owed (design document `data-and-context`, 41, 42).
    WordOwed { name: BlockId, watched: Vec<BlockId> },
    /// The return hands in the verdict, `block`, with the value that does not pass, `value`, and no version of its
    /// dialogue, `name`: a judge's fail brings the points that still stand (design document `data-and-context`, 41,
    /// 42).
    NoPoint {
        block: BlockId,
        value: String,
        name: BlockId,
    },
    /// The return names as dealt with a name that is none of what its cell is given, as it wrote it. Kept under its
    /// earlier name too, in the reasons of rounds sent back before.
    #[serde(alias = "NotATrigger")]
    NotWatched { name: String },
    /// The return asks for a tool its cell may not ask for, as it wrote it.
    NotATool { name: String },
    /// The return names a block as written with its meaning unchanged, and that is none of the blocks its cell writes.
    /// (Named, and a block of the cell's, it changes nothing when it is not written: no return is sent back for it.)
    UnchangedNotWritten { block: BlockId },
    /// The return writes a question of this name that is not its cell's own: another cell's, or one the engine asked.
    QuestionTaken { name: String },
    /// The return withdraws a question of this name that is none of its cell's that stands.
    NoSuchQuestion { name: String },
    /// The return writes a question of this name twice.
    QuestionTwice { name: String },
    /// The question of this name recommends none of its options, or has none.
    NotAnOption { name: String },
    /// The return asks a question, of this name, of a cell that no answer runs: the answer could not reach it. Returns
    /// read before a cell could ask only where it writes the work's questions were sent back so; a question of a cell
    /// that does not write them is now a block that is not among its outputs.
    NotAnswerable { name: String },
    /// The return names as dealt with its cell's own words, of this name, and writes no answer in them (design document
    /// `data-and-context`, 35). Returns read before a cell's words were its own named so what ran them on the owner's
    /// words, whose record they wrote no answer in.
    NotAnswered { name: String },
    /// The work's branch, as the round left it, changes the file of this path, which the project protects, and neither
    /// the return nor the engine's question that stands of the file as it is says why: as the engine found it, looking
    /// at the branch as the round ended.
    Unexplained { path: String },
    /// The run's rounds changed the file of this path on the work's branch, as the work's own change, and it is none
    /// of the project's tests, which are all the cell's line changes: as the engine found it, looking at the branch as
    /// the round ended.
    NotATest { path: String },
    /// The round ended with a return that asks for no call, in a line that may change the work, and the worktree holds
    /// these changes to files git tracks that are not committed, as `git status` lists them: as the engine found it,
    /// looking at the worktree as the round ended. A file git does not track is none.
    Uncommitted { changes: String },
    /// The round ended so, and a merge is under way in the worktree, begun and not committed, with these changes: as the
    /// engine found it, looking at the worktree as the round ended.
    Merging { changes: String },
    /// The return writes a block handed in only with the owner's answers in the design tree (`answers_in_the_tree` of
    /// its file), the question of this name is marked `tree` and has the owner's answer to its latest version, and the
    /// work's change against main changes nothing in the project's design tree: as the engine found it, looking at the
    /// branch as the round ended.
    TreeUnchanged { name: String },
    /// The return writes a block whose lines name entries of the design tree (`names_tree_entries` of its file), and
    /// this line of it is not written as the project's `[tree_format]` refers to an entry, or names one that the tree
    /// at the head of the work's branch does not hold: as the engine found it, looking at the branch as the round
    /// ended.
    NotAnEntry { line: String },
    /// The return deals with what an act of the engine's after its cell wrote, the block, and leaves new nothing that
    /// runs that act, neither by what it hands in, the work's head it committed among it, nor by a question that
    /// stands for the owner's answer, and nothing that runs it is new to it already ([`Answer`]): the act reads no
    /// words, and would never run again.
    Unanswered { block: BlockId },
}

/// What runs an act of the engine's again, of a block it wrote that runs a cell before it: the blocks that run the
/// act; whether the block of the questions' standing is among them, so that a question that stands unanswered runs it
/// once it is answered; and whether one of them has a version new to the act already, as the return is taken, which
/// runs it again whatever the return leaves. A return that deals with the block is held to leaving one of them new.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Answer {
    pub runs: Vec<BlockId>,
    pub question: bool,
    pub new: bool,
}

/// What a pause of the run's work holds back of its rounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hold {
    /// Nothing: the work is not paused, or it is and the run started for a note a person wrote, and no failure of it
    /// paused the work since its last round ended.
    Nothing,
    /// Only the round after one that failed: the work is paused, the run started for a note a person wrote, and its
    /// last round's failure paused the work.
    Retry,
    /// Every round: the work is paused, and the run did not start for a note a person wrote.
    Rounds,
}

/// The names a return gives as dealt with that ran the round, each once and in its order: a datum or group the cell
/// watches that stands for a block that ran the round ([`flow::ran`]). What it names beside, as it was given to the cell
/// or was written again, changes nothing and is none of what a check reads.
fn standing_names(dealt_with: &[BlockId], requires: Requires) -> Vec<BlockId> {
    let mut names: Vec<BlockId> = Vec::new();
    for named in dealt_with {
        let ran = flow::ran(flow::stands_on(requires.groups, named), &requires.told.ran_by)
            .next()
            .is_some();
        if ran && !names.contains(named) {
            names.push(named.clone());
        }
    }
    names
}

/// What the returns of a run that came back come to taken together (decision
/// a-sent-back-return-is-completed-by-what-is-missing): the last return, and, before it, what the returns sent back
/// earlier said and the round after each was not told was wrong, each part by its name replaced by a later return's of
/// the same name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Together {
    pub dealt_with: Vec<BlockId>,
    pub writes: Vec<Write>,
    pub calls: usize,
    pub found: Vec<Unmet>,
    pub waits: bool,
    pub questions: Vec<Asking>,
    pub read: Vec<BlockId>,
    pub protected: BTreeMap<String, String>,
}

impl Unmet {
    /// The blocks of what was returned that this says are wrong: what the next return gives anew, if it means them.
    fn wrong_blocks(&self) -> Vec<&BlockId> {
        match self {
            Unmet::NotAnOutput { block }
            | Unmet::WrittenTwice { block }
            | Unmet::NoValue { block }
            | Unmet::NotAValue { block, .. }
            | Unmet::NotAChoice { block }
            | Unmet::WrittenOver { block, .. }
            | Unmet::ChoiceChanged { block } => vec![block],
            Unmet::NothingBut { block, besides, .. } => std::iter::once(block).chain(besides).collect(),
            _ => Vec::new(),
        }
    }

    /// The question of what was returned that this says is wrong.
    fn wrong_question(&self) -> Option<&str> {
        match self {
            Unmet::QuestionTaken { name }
            | Unmet::NoSuchQuestion { name }
            | Unmet::QuestionTwice { name }
            | Unmet::NotAnOption { name }
            | Unmet::NotAnswerable { name } => Some(name),
            _ => None,
        }
    }
}

impl Together {
    /// What of this the faults it was sent back for do not name: the blocks and questions they name are left out, and
    /// what it found as it was read is the return's own.
    fn carried(mut self, faults: &[Unmet]) -> Together {
        let wrong: Vec<&BlockId> = faults.iter().flat_map(Unmet::wrong_blocks).collect();
        self.writes.retain(|w| !wrong.contains(&&w.block));
        let questions: Vec<&str> = faults.iter().filter_map(Unmet::wrong_question).collect();
        self.questions.retain(|q| !questions.contains(&q.name.as_str()));
        self
    }

    /// This, carried, completed by a later return: a part of the later one replaces the one of its name.
    fn completed_by(mut self, later: Together) -> Together {
        for name in later.dealt_with {
            if !self.dealt_with.contains(&name) {
                self.dealt_with.push(name);
            }
        }
        self.writes.retain(|w| !later.writes.iter().any(|l| l.block == w.block));
        self.writes.extend(later.writes);
        self.questions
            .retain(|q| !later.questions.iter().any(|l| l.name == q.name));
        self.questions.extend(later.questions);
        self.protected.extend(later.protected);
        self.waits = self.questions.iter().any(|q| q.waits && !q.withdrawn) || later.waits;
        self.calls = later.calls;
        self.found = later.found;
        self.read = later.read;
        self
    }
}

/// What the returns of the rounds that came back come to taken together, with the faults the last of them was sent back
/// for; none where none came back. A round that failed leaves what came before it as it was. Nothing is carried out of a
/// return that asks for calls, nor into one: a return that asks for calls asks for nothing else.
fn fold(rounds: &[Round]) -> Option<(Together, &[Unmet])> {
    let mut last: Option<(Together, &[Unmet])> = None;
    for round in rounds {
        let Some(Return::Came {
            dealt_with,
            writes,
            calls,
            found,
            waits,
            questions,
            read,
            protected,
        }) = &round.returned
        else {
            continue;
        };
        let now = Together {
            dealt_with: dealt_with.clone(),
            writes: writes.clone(),
            calls: *calls,
            found: found.clone(),
            waits: *waits,
            questions: questions.clone(),
            read: read.clone(),
            protected: protected.clone(),
        };
        let now = match last.take() {
            Some((before, faults)) if before.calls == 0 && now.calls == 0 => before.carried(faults).completed_by(now),
            _ => now,
        };
        last = Some((now, &round.sent_back));
    }
    last
}

/// What the returns of the rounds that came back come to taken together; none where none did.
pub fn together(rounds: &[Round]) -> Option<Together> {
    fold(rounds).map(|(taken, _)| taken)
}

/// What the next return of the run completes: what the returns that came back come to taken together, but for what the
/// faults the last of them was sent back for name; nothing where none came back, or the last asked for calls.
pub fn carried(rounds: &[Round]) -> Together {
    match fold(rounds) {
        Some((taken, faults)) if taken.calls == 0 => taken.carried(faults),
        _ => Together::default(),
    }
}

/// The blocks a return that is taken deals with: of each datum or group it names, in its order, the blocks it stands
/// for that ran the round, each once. A group is dealt with for each of its blocks that ran the round, and so each of
/// their versions the round was told of is consumed.
fn consumed(dealt_with: &[BlockId], requires: Requires) -> Vec<BlockId> {
    let mut blocks: Vec<BlockId> = Vec::new();
    for named in dealt_with {
        for block in flow::ran(flow::stands_on(requires.groups, named), &requires.told.ran_by) {
            if !blocks.contains(block) {
                blocks.push(block.clone());
            }
        }
    }
    blocks
}

/// What the run does next. Where the run would have a round that the pause of its work holds back, it is held.
pub fn next(log: &Log, requires: Requires, hold: Hold) -> Step {
    let Some(last) = log.rounds.last() else {
        return held(Step::Begin, hold);
    };
    let Some(returned) = &last.returned else {
        return Step::UnderWay;
    };
    // The rounds before the last, from where the owner last let the run go on: what has a limit is counted there.
    let before = log.rounds.len() - 1;
    let earlier = &log.rounds[log.let_go.min(before)..before];
    // Whether the owner let the run go on after the last round ended: what that round returned is then taken up
    // whatever the counts were.
    let let_on = log.let_go > before;
    let unmet = match returned {
        // What could not be read is sent back, as a return that is not of its form.
        Return::Failed {
            failure: Failure::Unread,
            why,
            ..
        } => vec![Unmet::Unreadable { why: why.clone() }],
        // In passing, the engine waits before the round is tried again; one let on since, or that paused the work,
        // waits no more.
        Return::Failed {
            failure: Failure::Passing,
            waits,
            ..
        } => {
            let failed = passing_in_a_row(log);
            if failed == 0 || failed > PASSING {
                return held(Step::Retry, hold);
            }
            if waits.is_empty() {
                return Step::Wait {
                    failed,
                    ms: wait(failed),
                };
            }
            let open: Vec<CallId> = waits.iter().filter(|c| !c.done).map(|c| c.call).collect();
            if open.is_empty() {
                return held(Step::Retry, hold);
            }
            return Step::Rest { calls: open };
        }
        // Any other failure is no return sent back and makes no call: it counts against no limit. A flagged round, and
        // one the run guard stopped, is tried in a new session of its line, which whatever carries it out begins; one
        // whose cause every work shares has its next round once agents' rounds may begin again; one that paused the
        // work, once it is let on.
        Return::Failed { .. } => return held(Step::Retry, hold),
        Return::Came { .. } => {
            // What the run has returned, taken together: the last round's return, with what the returns sent back
            // before it did not name as wrong.
            let taken = together(&log.rounds).unwrap_or_default();
            let came = Came {
                dealt_with: &taken.dealt_with,
                writes: &taken.writes,
                calls: taken.calls,
                found: &taken.found,
                waits: taken.waits,
                questions: &taken.questions,
                read: &taken.read,
            };
            let unmet = unmet(&came, requires);
            if unmet.is_empty() {
                // What the round committed is the run's too, as the carrier read it of the branch, once the cell's work
                // is handed in: a return that asks for calls hands in none of it, and what it committed is read, with
                // what the run commits after, by the round that ends the cell's work. While a cell that may change the
                // work waits for the results of calls, the carrier reads none of the branch for any round, the waiting
                // cell's own round entered for the owner's words included, so there is nothing read to hand in; a round
                // entered while its cell waits for an answer holds nothing back (decision
                // a-version-read-at-a-rounds-end-is-that-rounds).
                let hands_in_the_commit = taken.calls == 0;
                let committed = (taken.read.iter())
                    .filter(|_| hands_in_the_commit)
                    .filter(|block| !taken.writes.iter().any(|w| &w.block == *block))
                    .map(|block| Write {
                        block: block.clone(),
                        meaning_unchanged: false,
                        value: None,
                        // What the carrier read of the branch, a commit, holds something.
                        empty: false,
                    });
                return Step::Finish {
                    dealt_with: consumed(&taken.dealt_with, requires),
                    writes: taken.writes.iter().cloned().chain(committed).collect(),
                };
            }
            unmet
        }
    };
    // Every return before this one that was read, or could not be, was sent back: the run would have finished
    // otherwise.
    let sent_back = earlier.iter().filter(|r| sent_back(r)).count() as u32;
    if !let_on && sent_back >= requires.returns {
        return Step::AtLimit(Limit::Returns { sent_back });
    }
    held(Step::SendBack { unmet }, hold)
}

/// The step, which begins a round, or its being held where the pause holds that round back.
fn held(step: Step, hold: Hold) -> Step {
    let holds = match hold {
        Hold::Nothing => false,
        Hold::Retry => step == Step::Retry,
        Hold::Rounds => true,
    };
    if holds { Step::Held } else { step }
}

/// Whether what the round returned was sent back: it is not the last round, and it was read or could not be. A round
/// that failed otherwise returned nothing to send back.
fn sent_back(round: &Round) -> bool {
    matches!(
        round.returned,
        Some(
            Return::Came { .. }
                | Return::Failed {
                    failure: Failure::Unread,
                    ..
                }
        )
    )
}

/// The failures the run's last rounds ended in, in a row, the last first: those since the owner last let the work on,
/// up to the last round that did not fail.
fn failures_in_a_row(log: &Log) -> impl Iterator<Item = Failure> + '_ {
    let since = log.let_on.min(log.rounds.len());
    (log.rounds[since..].iter().rev()).map_while(|round| match &round.returned {
        Some(Return::Failed { failure, .. }) => Some(*failure),
        _ => None,
    })
}

/// How many of the run's last rounds in a row failed in passing ([`failures_in_a_row`]).
fn passing_in_a_row(log: &Log) -> u32 {
    failures_in_a_row(log).filter(|f| *f == Failure::Passing).count() as u32
}

/// How many of the run's last rounds in a row failed for a cause outside every work ([`failures_in_a_row`]).
fn outside_in_a_row(log: &Log) -> u32 {
    failures_in_a_row(log).filter(|f| f.shared()).count() as u32
}

/// Why the engine pauses the work as the run's last round fails, if it does: the failure is of a kind running again
/// cannot help; or it is in passing, or of a cause outside every work, and more of the run's last rounds in a row failed
/// so than are tried again ([`PASSING`]); or the round was flagged, or the run guard stopped it, and another of the
/// run's last rounds in a row failed so, whatever failed between them, the first having had its line's session begun
/// anew. Any other failure the run takes up itself ([`next`]). The failures in a row start anew where the owner let the
/// work on.
///
/// A round of the run begins after one that failed for a cause outside every work only once the engine has found the
/// outside well, so that a row of them says the rounds meet what the engine's check does not find, as a limit of the
/// model the rounds run on alone; each wrote its message into its session, and the row is bounded as one in passing is.
pub fn pausing(log: &Log) -> Option<Pausing> {
    let mut row = failures_in_a_row(log);
    match row.next()? {
        failure @ (Failure::PromptTooLong | Failure::NoModel | Failure::Other) => Some(Pausing::Failed(failure)),
        Failure::Passing => {
            let failed = passing_in_a_row(log);
            (failed > PASSING).then_some(Pausing::Passing { failed })
        }
        Failure::Flagged => row.any(|f| f == Failure::Flagged).then_some(Pausing::FlaggedAgain),
        Failure::Guarded => row.any(|f| f == Failure::Guarded).then_some(Pausing::GuardedAgain),
        Failure::UsageLimit | Failure::CannotRun | Failure::Offline | Failure::Unavailable => {
            let failed = outside_in_a_row(log);
            (failed > PASSING).then_some(Pausing::Outside { failed })
        }
        Failure::Unread => None,
    }
}

/// Each way in which what came back, taken together with what the returns sent back before it did not name as wrong
/// ([`together`]), does not meet what the flow requires, in one pass (principle `report-all-at-once`): first what was
/// found as it was read, in the order found; then whether it names the cell's own words as dealt with and writes no
/// answer in them, what it names that did not run the round being left out ([`standing_names`]); then one
/// for each write, in the order of the writes; then, of each choice it writes with a value that stands with nothing but
/// some blocks, what else it holds; then whether it asks for calls beside doing the cell's work; then whether it deals
/// with nothing while something ran its round, asks for no call and has no question that waits for the owner's answer;
/// then each block the cell must hand in and did not, in the order the cell lists them, where the return asks for no
/// call, has no question that waits, its cell waits for no result and it does more than answer the owner in the cell's
/// own words, but one found written in a shape it does not take; then each question it writes that it may not, in the
/// order it wrote them; then each block an act wrote that it deals with and leaves unanswered, in the order it dealt
/// with them.
fn unmet(came: &Came<'_>, requires: Requires) -> Vec<Unmet> {
    let Came {
        dealt_with,
        writes,
        calls,
        found,
        waits,
        questions,
        read,
    } = *came;
    let told = requires.told;
    let mut unmet = found.to_vec();
    // What the return names as dealt with that ran the round, each once: a name that did not, or that is written again,
    // changes nothing and is left out of every check that follows.
    let named = standing_names(dealt_with, requires);
    let dealt_with: &[BlockId] = &named;
    // A return that names the cell's own words as dealt with writes an answer in them.
    if let Some(own) = requires.words
        && dealt_with.contains(own)
        && !writes.iter().any(|w| w.block == *own && !w.empty)
    {
        unmet.push(Unmet::NotAnswered { name: own.to_string() });
    }
    for (i, write) in writes.iter().enumerate() {
        let block = || write.block.clone();
        if writes[..i].iter().any(|w| w.block == write.block) {
            unmet.push(Unmet::WrittenTwice { block: block() });
            continue;
        }
        if !requires.outputs.contains(&write.block) {
            unmet.push(Unmet::NotAnOutput { block: block() });
            continue;
        }
        match (requires.choices.get(&write.block), &write.value) {
            (Some(_), None) => unmet.push(Unmet::NoValue { block: block() }),
            (Some(values), Some(value)) if !values.contains(value) => unmet.push(Unmet::NotAValue {
                block: block(),
                value: value.clone(),
            }),
            (None, Some(_)) => unmet.push(Unmet::NotAChoice { block: block() }),
            _ => {}
        }
        let standing = told.outputs.get(&write.block);
        if let Some(Standing {
            told,
            latest: Some(latest),
            added: false,
            ..
        }) = standing
            && *told != Some(*latest)
        {
            unmet.push(Unmet::WrittenOver {
                block: block(),
                version: *latest,
            });
        }
        let choice = requires.choices.contains_key(&write.block);
        let value = standing.and_then(|s| s.value.as_ref());
        if choice && write.meaning_unchanged && value != write.value.as_ref() {
            unmet.push(Unmet::ChoiceChanged { block: block() });
        }
    }
    // A choice written with a value that stands with nothing but some blocks: the return holds no other block its cell
    // may write, and asks and withdraws no question, but what the choice's file names. A block the cell may not write is
    // said to be that alone.
    for (i, write) in writes.iter().enumerate() {
        let value = write.value.as_ref();
        let only = value.and_then(|v| requires.nothing_but.get(&write.block)?.get(v));
        let (Some(value), Some(only)) = (value, only) else {
            continue;
        };
        if writes[..i].iter().any(|w| w.block == write.block) || !requires.outputs.contains(&write.block) {
            continue;
        }
        let mut besides: Vec<BlockId> = Vec::new();
        for other in writes.iter().map(|w| &w.block) {
            let held = *other != write.block && !only.contains(other) && requires.outputs.contains(other);
            if held && !besides.contains(other) {
                besides.push(other.clone());
            }
        }
        let asks = waits || !questions.is_empty();
        if let Some(own) = requires.questions.filter(|q| asks && !only.contains(*q)) {
            besides.push(own.clone());
        }
        if !besides.is_empty() {
            unmet.push(Unmet::NothingBut {
                block: write.block.clone(),
                value: value.clone(),
                besides,
            });
        }
    }
    // A return of both kinds was found so as it was read, and is said so once.
    if calls > 0 && !(dealt_with.is_empty() && writes.is_empty()) && !found.contains(&Unmet::AsksAndWorks) {
        unmet.push(Unmet::AsksAndWorks);
    }
    // A question that waits stops the work at the cell, which may leave all that ran it for after the answer.
    if calls == 0 && dealt_with.is_empty() && !told.ran_by.is_empty() && !waits && !told.hurried {
        unmet.push(Unmet::DealtWithNothing);
    }
    // The one woken in a dialogue owes the next word (design document `data-and-context`, 41, 42): a maker woken by a
    // point of the other side, an answer in the dialogue, a change of what the other side watches, the work's head
    // among it as the round committed it, or a question the work waits for; a judge woken by an answer or a change of
    // what it judges, a fail with its points. One woken by anything else owes nothing in the dialogue, but a judge's
    // fail that is new brings its points: one handed in as it stands changes nothing and is taken (22). A version that changes nothing in
    // meaning, as the walk reads it, is new to no cell (9): it answers nothing, changes nothing the other side watches
    // and brings no point. The round saw what it writes over where it was told of the latest; one that did not is sent
    // back for it, whatever it says.
    if calls == 0 && !told.waits {
        let changes = |w: &Write| {
            let standing = told.outputs.get(&w.block).filter(|s| s.latest.is_some());
            let over = standing.map(|s| s.value.as_deref());
            let saw = standing.is_none_or(|s| s.added || s.told == s.latest);
            let nothing =
                crate::walk::says_nothing(w.empty, standing.map(|s| s.empty), standing.is_some_and(|s| s.added));
            crate::walk::gives_meaning(w.value.as_deref(), over, !w.meaning_unchanged, saw, nothing)
        };
        let said = || (writes.iter()).filter(|w| changes(w)).map(|w| &w.block);
        for (dialogue, watched) in requires.dialogues {
            let woken = told.points.contains(dialogue) && dealt_with.contains(dialogue);
            let answered = (said().chain(read)).any(|b| b == dialogue || watched.contains(b));
            if woken && !answered && !waits {
                unmet.push(Unmet::WordOwed {
                    name: dialogue.clone(),
                    watched: watched.clone(),
                });
            }
        }
        for write in writes {
            let Some(verdict) = requires.verdicts.get(&write.block) else {
                continue;
            };
            let fails = write.value.as_deref() == Some(verdict.fails.as_str());
            // A fail that is new is a judgment the maker must be able to answer, whatever woke the judge.
            let owes = told.woken_to_judge.contains(&verdict.dialogue) || changes(write);
            if owes && fails && !said().any(|b| *b == verdict.dialogue) {
                unmet.push(Unmet::NoPoint {
                    block: write.block.clone(),
                    value: verdict.fails.clone(),
                    name: verdict.dialogue.clone(),
                });
            }
        }
    }
    // A block written in a shape it does not take was handed in, and is said to be wrong only for its shape.
    let misshapen = |block: &BlockId| {
        (found.iter()).any(|u| matches!(u, Unmet::NoValue { block: b } | Unmet::NotAChoice { block: b } if b == block))
    };
    // A return that deals with the cell's own words alone, writes nothing but its answer in them and asks nothing
    // answers the owner first, and goes on with the cell's work in later rounds (design document `data-and-context`,
    // 35).
    let answers_alone = requires.words.is_some_and(|own| {
        dealt_with == std::slice::from_ref(own) && writes.iter().all(|w| w.block == *own) && questions.is_empty()
    });
    if calls == 0 && !waits && !told.waits && !told.hurried && !answers_alone {
        for block in requires.must_write {
            if !writes.iter().any(|w| w.block == *block) && !misshapen(block) {
                unmet.push(Unmet::NotHandedIn { block: block.clone() });
            }
        }
    }
    // A question is one cell's: its agent alone writes it anew or withdraws it, under a name no other question has.
    for (i, q) in questions.iter().enumerate() {
        let name = || q.name.clone();
        let taken = Unmet::QuestionTaken { name: name() };
        // A name the reading found taken is said so once, and nothing more of it.
        if found.contains(&taken) {
            continue;
        }
        let asked = told.questions.get(&q.name);
        if questions[..i].iter().any(|before| before.name == q.name) {
            unmet.push(Unmet::QuestionTwice { name: name() });
        } else if asked.is_some_and(|a| !a.own) {
            unmet.push(taken);
        } else if q.withdrawn && !asked.is_some_and(|a| a.stands) {
            unmet.push(Unmet::NoSuchQuestion { name: name() });
        }
    }
    // What an act of the engine's after the cell said to it is answered by what runs the act: what the return hands
    // in, the work's head it committed among it, what is new to the act already, or an answer to come once the return
    // is taken, to a question it asks or to one that stands with no answer to its latest version. The act reads no
    // words, and an answered question has no answer to come: a work answered with none of these would never run it
    // again.
    if calls == 0 && !told.waits {
        let withdrawn = |name: &str| questions.iter().any(|q| q.withdrawn && q.name == name);
        let to_come = waits
            || questions.iter().any(|q| !q.withdrawn)
            || (told.questions.iter()).any(|(name, q)| q.unanswered && !withdrawn(name));
        for block in consumed(dealt_with, requires) {
            let Some(answer) = requires.answered_by.get(&block) else {
                continue;
            };
            let written = (writes.iter().map(|w| &w.block))
                .chain(read)
                .any(|b| answer.runs.contains(b));
            if !written && !answer.new && !(answer.question && to_come) {
                unmet.push(Unmet::Unanswered { block });
            }
        }
    }
    unmet
}

/// What came back of a round as a return, as [`unmet`] reads it.
#[derive(Clone, Copy)]
struct Came<'r> {
    dealt_with: &'r [BlockId],
    writes: &'r [Write],
    calls: usize,
    found: &'r [Unmet],
    waits: bool,
    questions: &'r [Asking],
    read: &'r [BlockId],
}
