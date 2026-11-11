//! The items and the works as the pages read them: their own types, named for what the owner reads, made from what
//! the driver holds, and read back by the command line. Nothing here is decided: what a work does now is the deciding
//! code's own account of it ([`decide::view`]), so that what is shown and what is decided cannot differ (entry
//! `records-give-the-same-walk`). `client/src/works-api.ts` gives the same types to the pages, field for field.
//!
//! The pages are given a work's history without the content of what was written: each write says how long its
//! content is, and the content of a version, of what a round handed in and of a call's result is asked for by itself,
//! when the owner opens it.
//!
//! - [`Items`]: every item with its work in brief, for the item graph and the list of works.
//! - [`WorkPage`]: one work, as it is now and step by step from its start.
//! - [`RoundPage`], [`VersionPage`], [`CallPage`]: the content the history leaves out.
//! - [`ToldPage`]: what a round was told and what came back of it, as its carrier kept them.
//! - [`Act`], [`NewItem`]: what the owner does on the pages.
//!
//! A work whose records name what it does not hold, which the engine never writes, is not shown as something it is
//! not: making its view gives [`Broken`], saying what is missing.

use decide::run::{Failure as FailureOf, Limit as LimitOf, Unmet as UnmetOf};
use decide::{
    BlockId, CallId, Cause as CauseOf, CellId, Decision, End, Maker as MakerOf, Pauser as PauserOf, RunId, Seq, View,
    Waited as WaitedOf, Who, Writer as WriterOf,
};
use driver::{Driver, Kept, Until as UntilOf};
use flow::Kind;
use serde::{Deserialize, Serialize};
use service::Levels;
use store::{ItemNumber, WorkNumber};
use work::{Ask as AskOf, Back as BackOf, Call as CallOf, Given as GivenOf, Time, Why, Work};

use crate::view::Flow;

/// Every item, in the order of their numbers, each with its work.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Items {
    pub items: Vec<Item>,
    /// A wait every work shares, where one stands: no agent's round of any work begins.
    pub outage: Option<Outage>,
}

/// A wait every work shares: a round of an agent failed for a cause that is no one work's, and no agent's round of any
/// work begins while it stands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outage {
    /// The kind of the cause: the usage limit, a command line that cannot run, or no network.
    pub failure: Failure,
    /// Why the round failed, as what carried it out said.
    pub why: String,
    /// The round that failed so, the latest that did: of this work, this run, this number.
    pub work: u64,
    pub run: u64,
    pub round: usize,
    /// When it ended.
    pub since: u64,
    /// When the agents' rounds may begin again.
    pub until: Until,
    /// Whether it holds rounds now: once its time has come, it stands only until a round that began after it comes
    /// back, and holds nothing meanwhile.
    pub holds: bool,
}

/// When the agents' rounds may begin again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Until {
    /// At this time, every work's: as the usage limit resets.
    At { at: u64 },
    /// One round, of whichever work comes to one first, tries at this time; where `look`, the network is looked at
    /// every half minute from then, and the round tries once a look reaches it, at the latest at `latest`.
    Trial { at: u64, look: bool, latest: u64 },
    /// Once the round that tries now has ended.
    Tried,
}

/// The kind of cause of a round that failed, as what carried it out told it apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Failure {
    /// In passing: tried again after a wait.
    Passing,
    /// What came back could not be read: sent back.
    Unread,
    /// The service's safety check flagged it: tried again in a new session of its line.
    Flagged,
    /// The account's usage limit.
    UsageLimit,
    /// The agents' command line cannot run.
    CannotRun,
    /// The network cannot be reached.
    Offline,
    /// The prompt is too long for the model.
    PromptTooLong,
    /// A cause no kind tells apart.
    Other,
}

/// An item: a thing to be done.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub number: u64,
    pub title: String,
    /// The level it was given as it was added, which chooses what its agents' rounds run on; none for an item added
    /// before items had levels, whose rounds run at the default level.
    pub level: Option<String>,
    /// When it was added, in milliseconds since the start of 1970, as every time here is.
    pub added: u64,
    /// The items it depends on, in the order of their numbers.
    pub depends_on: Vec<u64>,
    /// The one work done for it, which was added with it.
    pub work: Summary,
}

/// A work in brief.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub number: u64,
    /// The item it is done for.
    pub item: u64,
    /// The name of the flow it runs on.
    pub flow: String,
    pub added: u64,
    /// Whether the work is paused: no round of it begins but those of a run that answers a note, a person's words to
    /// its cell. A round under way goes on, and what it hands in is taken.
    pub paused: bool,
    /// What the work does now.
    pub now: Now,
    /// Each cell of its flow as it stands for this work, in the order of the walk.
    pub cells: Vec<CellNow>,
    /// What all its rounds that ended took.
    pub spent: Spent,
    /// When something last happened to it; none where nothing has.
    pub last: Option<u64>,
}

/// What a work does now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Now {
    /// Its records cannot be those of a work, and nothing is decided on them; in words.
    Fault { words: String },
    /// It has ended: its last cell, the merge, has a record.
    Merged,
    /// It has ended: its item was cancelled.
    Cancelled,
    /// A round of this run of this cell is under way, since this time.
    Round {
        cell: String,
        run: u64,
        round: usize,
        since: u64,
    },
    /// It is paused, and what it would do next is begin a round: by the owner, or by the engine itself for this
    /// reason, as when a round failed.
    Paused { by: Pauser, why: Option<String> },
    /// It waits at this cell, for each of these.
    Waits { at: String, on: Vec<Waited> },
    /// It is about to do something at this cell. A work is never left so between two things that happen to it.
    Next { cell: String },
}

/// What a work waits for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Waited {
    /// The result of a call a run rests at: from this person, or from the engine where there is none.
    Call { call: u64, who: Option<Person>, what: Call },
    /// A precondition of the cell does not hold: the named test, the words shown for it, and each who can make it
    /// hold, by writing a block that keeps it from holding.
    Precondition {
        test: String,
        words: String,
        by: Vec<Maker>,
    },
    /// The cell's last runs, this many in a row, were each for what a cell after it wrote, and so would the next be:
    /// the owner lets it go on or cancels the work.
    Limit { runs: u32 },
    /// The wait every work shares: the cell's agent's round does not begin until it is over.
    Agents { outage: Outage },
}

/// A person who can give what a work waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Person {
    Owner,
    Architect,
}

/// Who can make a precondition hold, by writing a block its test reads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Maker {
    /// The owner, as people write the block.
    Owner,
    /// This source outside the work, such as the items the work depends on as they merge.
    Outside { source: String },
    /// The cell, by what its next run writes.
    Cell { cell: String },
}

/// Who paused a work: the owner, or the engine itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Pauser {
    Owner,
    Engine,
}

/// What a call is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Call {
    /// A tool the engine runs, with what it is run on.
    Tool { name: String, input: String },
    /// A question for a person.
    Question { who: Person, question: String },
    /// The run reached a limit: the owner lets it go on or cancels the work.
    AtLimit { limit: Limit },
    /// The engine waits until this time before the run's last round, which failed in passing, the `failed`th time in
    /// a row, is tried again.
    Wait { until: u64, failed: u32 },
}

/// The limit a run reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "of", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Limit {
    /// This many returns were sent back.
    Returns { sent_back: u32 },
    /// This many calls were made.
    Calls { made: u32 },
}

/// A cell of the flow as it stands for a work.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellNow {
    pub cell: String,
    pub state: CellState,
    /// How many of its runs finished.
    pub finished: usize,
    /// The cell's record: its latest finished run.
    pub record: Option<u64>,
    /// Its run that has neither finished nor been abandoned.
    pub open: Option<u64>,
}

/// How a cell stands for a work, the first of these that is so.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CellState {
    /// A round of its run is under way.
    Running,
    /// Its run is at rest: for a call's result, or only for its next round.
    Resting,
    /// Its run's last round failed: the run is at rest, and its next round, once the work is let on, is that round
    /// again.
    Failed,
    /// Its condition is not met, so it is skipped this time; what it wrote before still counts.
    Skipped,
    /// It has no record: it never finished a run.
    NotYet,
    /// It has a record, and a block it reads has changed in meaning since: it runs again when the walk reaches it.
    Again,
    /// Its record knows the current meaning of every block it reads.
    Done,
}

/// What rounds took.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spent {
    /// The cost, in millionths of a dollar.
    pub usd_micros: u64,
    /// The tokens sent that were not read from the cache.
    pub input_tokens: u64,
    /// The tokens sent that were read from the cache.
    pub cached_tokens: u64,
    /// The tokens that came back.
    pub output_tokens: u64,
    /// How long the rounds were under way, in milliseconds.
    pub working_ms: u64,
    /// How many rounds ended.
    pub rounds: usize,
}

/// One work: as it is now, and step by step from its start.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkPage {
    pub summary: Summary,
    /// The title of its item.
    pub title: String,
    /// The level of its item, as [`Item`] has it.
    pub level: Option<String>,
    /// The flow it runs on, as the flow page reads a flow.
    pub flow: Flow,
    /// Each cell now, with why it would run and what it reads, in the order of the walk.
    pub cells: Vec<CellDetail>,
    /// Every block that has a version, by name, with its versions in the order they were written.
    pub blocks: Vec<Block>,
    /// Everything that happened to the work, in the order it happened; a run stands where it started.
    pub steps: Vec<Step>,
}

/// A cell now: why it would run, and the blocks it reads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellDetail {
    pub cell: String,
    /// Why the cell would run were the walk to reach it with nothing holding it; empty where it would not.
    pub needs: Vec<Cause>,
    /// The blocks it reads, in the order it lists them.
    pub inputs: Vec<Input>,
}

/// Why a cell runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Cause {
    /// It has no record.
    NoRecord,
    /// The block has another meaning than the one its record knows. `was` is the version the record knows; `behind`
    /// says a cell after this one gave the block the meaning it has now.
    Changed {
        block: String,
        was: Option<u64>,
        behind: bool,
    },
}

/// A block a cell reads, as the cell's record knows it and as it is now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub block: String,
    /// The block's current version; none where it has none.
    pub current: Option<u64>,
    /// The version the cell's record knows; none where the cell has no record, or the block had no version then.
    pub known: Option<u64>,
    /// Whether the block now has another meaning than the one the cell's record knows.
    pub changed: bool,
}

/// A block with its versions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub name: String,
    pub versions: Vec<Version>,
}

/// A version of a block: one write of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    /// The place of the entry that wrote it, which names it.
    pub at: u64,
    pub time: u64,
    pub by: Writer,
    /// The version at which the meaning it has last changed: itself, or an earlier one.
    pub meaning: u64,
    /// The block's value, where it is a choice.
    pub value: Option<String>,
    /// How long its content is, in characters.
    pub chars: usize,
}

/// Who wrote a version of a block.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Writer {
    Person,
    Outside,
    /// A cell, by this run of it.
    Run {
        run: u64,
        cell: String,
    },
}

/// One thing that happened to a work.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Step {
    /// A cell ran.
    Run(Run),
    /// A person or the outside wrote a block.
    Wrote {
        at: u64,
        time: u64,
        block: String,
        by: Writer,
        /// Whether the write changed the block's meaning.
        changed: bool,
        value: Option<String>,
        chars: usize,
    },
    /// The owner let the cell go on at its limit.
    LetGo { at: u64, time: u64, cell: String },
    /// The work was paused: by the owner, or by the engine itself for this reason.
    Paused {
        at: u64,
        time: u64,
        by: Pauser,
        why: Option<String>,
    },
    /// The work was let on again.
    Resumed { at: u64, time: u64 },
    /// The work's item was cancelled.
    Cancelled { at: u64, time: u64 },
    /// A new session of the line began, or begins with the line's next round, for this reason.
    Session {
        at: u64,
        time: u64,
        line: String,
        why: SessionWhy,
    },
}

/// Why a line began a new session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SessionWhy {
    /// The context of the line's session, as its last round ended, was past the limit, in tokens.
    Limit { context: u64, limit: u64 },
    /// The line's rounds run on another model than this one, which the session was begun on.
    Model { was: String },
    /// The line's rounds run under another account than the session was begun under.
    Account,
    /// The owner asked for one.
    Owner,
    /// The service's safety check flagged the line's last round.
    Flagged,
    /// The agents' program no longer had the session the line was in.
    Lost,
}

impl SessionWhy {
    fn of(why: &Why) -> SessionWhy {
        match why {
            Why::Limit { context, limit } => SessionWhy::Limit {
                context: *context,
                limit: *limit,
            },
            Why::Model { was } => SessionWhy::Model { was: was.clone() },
            Why::Account { .. } => SessionWhy::Account,
            Why::Owner => SessionWhy::Owner,
            Why::Flagged => SessionWhy::Flagged,
            Why::Lost => SessionWhy::Lost,
        }
    }
}

/// One run of a cell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub run: u64,
    pub cell: String,
    /// The place of the entry that started it.
    pub at: u64,
    pub time: u64,
    /// Why the cell ran.
    pub because: Vec<Cause>,
    /// The version of each block it reads or may write that it started from; none where the block had no version.
    pub basis: Vec<Basis>,
    /// Its rounds, in order.
    pub rounds: Vec<Round>,
    /// Its calls, in the order they were made.
    pub calls: Vec<CallRow>,
    /// How it ended; none while it is open.
    pub end: Option<RunEnd>,
    /// What its rounds that ended took.
    pub spent: Spent,
}

/// A block a run started from, at this version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Basis {
    pub block: String,
    pub version: Option<u64>,
}

/// One round of a run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Round {
    /// Which round of the run it is, the first being one.
    pub number: usize,
    pub began: u64,
    /// When it ended; none while it is under way.
    pub ended: Option<u64>,
    /// What it was given.
    pub given: Given,
    /// What came back; none while it is under way, and for a round that ended after its run was abandoned, when
    /// what came back was dropped.
    pub back: Option<Back>,
    /// What it took; none while it is under way.
    pub spent: Option<Spent>,
    /// The context it ended with, in tokens; none while it is under way and where it is not known.
    pub context: Option<u64>,
    /// How often its session was compacted during it.
    pub compactions: u32,
}

/// What a round is given.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Given {
    /// The first round: the cell's task.
    Task,
    /// The results of the calls the round before asked for.
    Results { calls: Vec<u64> },
    /// Why what the round before handed in was sent back.
    SentBack { unmet: Vec<Unmet> },
}

/// Why what came back does not meet what the flow requires.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Unmet {
    /// The round asked for calls, and for none.
    AsksNothing,
    /// The block is not one the cell may write.
    NotAnOutput { block: String },
    /// The block was written before in the same return.
    WrittenTwice { block: String },
    /// The block is a choice, and was written with no value.
    NoValue { block: String },
    /// The block is a choice, and this is not one of its values.
    NotAValue { block: String, value: String },
    /// The block is no choice, and was written with a value.
    NotAChoice { block: String },
    /// The block is one the cell must hand in each time, and was not handed in.
    NotHandedIn { block: String },
    /// What came back could not be read as a return, for this reason.
    Unreadable { why: String },
}

/// What came back at the end of a round.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Back {
    /// Calls were asked for.
    Asked { asks: Vec<Ask> },
    /// These blocks were handed in.
    HandedIn { blocks: Vec<Handed> },
    /// Nothing came back: the round failed, for this reason, of this kind of cause; `resets` says when the usage limit
    /// it reached resets, where the round's report said.
    Failed {
        why: String,
        failure: Failure,
        resets: Option<u64>,
    },
}

/// What an agent asks for at the end of a round.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Ask {
    Tool { name: String, input: String },
    Question { who: Person, question: String },
}

/// A block as a round hands it in, without its content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Handed {
    pub block: String,
    /// Whether the run says the change matters to the cells that read the block.
    pub matters: bool,
    pub value: Option<String>,
    pub chars: usize,
}

/// A call of a run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallRow {
    pub call: u64,
    pub what: Call,
    /// When it was made.
    pub made: u64,
    /// When it had its result; none while it has none.
    pub done: Option<u64>,
    /// How long its result is, in characters.
    pub chars: Option<usize>,
}

/// How a run ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RunEnd {
    /// It finished, and wrote these blocks: each is the version of its block named by `at`.
    Finished { at: u64, time: u64, writes: Vec<Handed> },
    /// It was abandoned, and wrote nothing.
    Abandoned { at: u64, time: u64 },
}

/// The content of a version of a block.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionPage {
    pub block: String,
    pub at: u64,
    pub content: String,
}

/// One round in full: the results it was given and the content of each block it handed in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundPage {
    pub run: u64,
    pub number: usize,
    /// The results it was given, where it was given results: each call with its result.
    pub results: Vec<CallPage>,
    /// The content of each block it handed in, where it handed blocks in, in the order it gave them.
    pub handed: Vec<HandedContent>,
}

/// The content of a block a round handed in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandedContent {
    pub block: String,
    pub content: String,
}

/// What a round was told and what came back of it, each try in order, as its carrier kept them; no try where it kept
/// nothing, as of a round of the engine's own acts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToldPage {
    pub tries: Vec<TryPage>,
    /// How many later tries were left out, those before them being as much as is read of a round.
    pub left_out: usize,
}

/// One try of a round.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TryPage {
    /// The message it was told, as it was written, up to where it was cut.
    pub message: String,
    /// The message's whole length in bytes, where it was cut before its end.
    pub cut: Option<u64>,
    pub back: TryBack,
    /// What it ran on as it began; none where the records say nothing of it, as of an act of the engine's own.
    pub ran: Option<RanOn>,
}

/// What a try of an agent's round ran on, as the work's records say it was when the try began.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RanOn {
    pub level: String,
    pub model: String,
    /// The reasoning effort its cell sets.
    pub effort: Option<String>,
}

/// The levels an item may be given, lowest first, with the one it is given where it is given none, and the highest
/// a level a line runs above its item's may reach.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelsPage {
    /// Every level of the installation, those above the highest an item may be at among them.
    pub levels: Vec<LevelPage>,
    pub default: String,
    pub highest: String,
}

/// A level, with what work it is for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelPage {
    pub name: String,
    pub desc: String,
}

/// The level the owner puts an item at.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewLevel {
    pub level: String,
}

/// Whether putting an item at a level changed it: it was at that level already where not.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelSet {
    pub changed: bool,
}

impl LevelsPage {
    pub fn of(levels: &Levels) -> LevelsPage {
        LevelsPage {
            levels: (levels.levels().iter())
                .map(|level| LevelPage {
                    name: level.name.clone(),
                    desc: level.desc.clone(),
                })
                .collect(),
            default: levels.default().to_string(),
            highest: levels.highest().to_string(),
        }
    }
}

/// What came back of a try: only what the page shows of what the program printed, never the whole of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TryBack {
    /// Nothing that was kept: the try is under way, or it was stopped, never got as far as its program, or its program
    /// printed no result.
    Nothing,
    Returned {
        /// The agent's closing words; none where they are the structured return itself.
        text: Option<String>,
        /// The structured return, as pretty-printed JSON.
        structured: Option<String>,
        turns: Option<u64>,
        duration_ms: Option<u64>,
        /// The models it was answered by, by name.
        models: Vec<String>,
    },
    /// The program said the try failed, for this reason.
    Failed { why: String },
    /// What was kept of it could not be read, for this reason.
    Unreadable { why: String },
}

/// A call with its result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallPage {
    pub call: u64,
    pub what: Call,
    /// Its result; none while it has none.
    pub result: Option<String>,
}

/// What the owner does to a work on the pages. Each kind is a struct, those with no field too, so that a field it
/// does not know is refused: a kind with no field at all would take any.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Act {
    /// Writes a block people write: what the item is about, an opinion for a cell, a choice.
    Write {
        block: String,
        content: String,
        /// The block's value, where it is a choice.
        value: Option<String>,
    },
    /// Lets the cell go on at its limit.
    LetGo { cell: String },
    /// Pauses the work.
    Pause {},
    /// Lets the work on again.
    Resume {},
    /// Cancels the work's item.
    Cancel {},
    /// Gives a call that waits for a person its result: the answer to a question, or that a run at its limit goes on.
    Answer { call: u64, result: String },
    /// Has the line begin a new session at its next round.
    NewSession { line: String },
}

/// An item the owner adds, with its work, which is added paused and begins no round until it is let on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewItem {
    pub title: String,
    /// The level it is given; with none, the level the installation adds an item at where it is given none.
    #[serde(default)]
    pub level: Option<String>,
    pub depends_on: Vec<u64>,
    /// The name of the flow its work runs on.
    pub flow: String,
    /// What is written for it as it is added, each a block people write.
    pub writes: Vec<NewWrite>,
}

/// A block written as an item is added.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewWrite {
    pub block: String,
    pub content: String,
    pub value: Option<String>,
}

/// The item and the work that were added.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Added {
    pub item: u64,
    pub work: u64,
}

/// Something happened: these works grew, and an item was added where `items`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Changed {
    pub works: Vec<u64>,
    pub items: bool,
}

/// Why something asked of the interface was refused, in words.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Refusal {
    pub error: String,
    /// Where the engine stopped as it was asked: whether what was asked was kept before it stopped, and so is not to
    /// be asked again. Left out of every other refusal, where nothing was kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kept: Option<bool>,
}

/// What a work's records name and the work does not hold, in words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Broken(pub String);

impl Items {
    /// Every item the driver holds, each with its work, as they stand at this time.
    pub fn of(driver: &Driver, now: Time) -> Result<Items, Broken> {
        let mut items = Vec::new();
        let outage = driver.outage().map(|outage| Outage::of(&outage, driver.holds(now)));
        for kept in driver.works() {
            let view = viewed(driver, kept.head.number)?;
            items.push(Item {
                number: kept.item.number.0,
                title: kept.item.title.clone(),
                level: kept.item.level.clone(),
                added: kept.item.added.0,
                depends_on: kept.item.depends_on.iter().map(|on| on.0).collect(),
                work: Summary::of(kept, &view, outage.as_ref())?,
            });
        }
        items.sort_by_key(|item| item.number);
        Ok(Items { items, outage })
    }
}

impl Summary {
    /// The work in brief, from what the driver holds of it, the deciding code's view of it, and the wait every work
    /// shares, where one stands.
    pub fn of(kept: Kept<'_>, view: &View, outage: Option<&Outage>) -> Result<Summary, Broken> {
        let work = kept.work;
        let runs = runs(kept)?;
        let cells = (view.cells.iter())
            .map(|cell| CellNow {
                cell: cell.cell.to_string(),
                state: match (&cell.open, cell.applies, cell.record) {
                    (Some(open), ..) if open.under_way => CellState::Running,
                    (Some(open), ..) if failed(work, open.run) => CellState::Failed,
                    (Some(_), ..) => CellState::Resting,
                    (None, false, _) => CellState::Skipped,
                    (None, true, None) => CellState::NotYet,
                    (None, true, Some(_)) if !cell.needs.is_empty() => CellState::Again,
                    (None, true, Some(_)) => CellState::Done,
                },
                finished: (runs.iter())
                    .filter(|run| run.cell == cell.cell.as_str() && matches!(run.end, Some(RunEnd::Finished { .. })))
                    .count(),
                record: cell.record.map(|run| run.0),
                open: cell.open.as_ref().map(|open| open.run.0),
            })
            .collect();
        Ok(Summary {
            number: kept.head.number.0,
            item: kept.head.item.0,
            flow: kept.head.flow.clone(),
            added: kept.head.added.0,
            paused: decide::paused(&work.records),
            now: Now::of(kept, &view.decision, outage)?,
            cells,
            spent: runs.iter().fold(Spent::default(), |sum, run| sum.with(run.spent)),
            last: work.moments.last().map(|moment| moment.at.0),
        })
    }
}

impl Now {
    fn of(kept: Kept<'_>, decision: &Decision, outage: Option<&Outage>) -> Result<Now, Broken> {
        let work = kept.work;
        let agents = |cell: &CellId| (kept.flow.cells.iter()).any(|spec| spec.id == *cell && spec.kind == Kind::Agent);
        // The rounds of a cell the wait holds now: an agent's, and without the network every cell's.
        let held =
            |cell: &CellId| (outage.filter(|o| o.holds)).is_some_and(|o| o.failure == Failure::Offline || agents(cell));
        Ok(match decision {
            Decision::Fault(fault) => Now::Fault {
                words: service::record_fault_words(fault),
            },
            Decision::Ended(End::Merged) => Now::Merged,
            Decision::Ended(End::Cancelled) => Now::Cancelled,
            Decision::Busy { cell, run } => {
                let began: Vec<Seq> = (work.records.rounds.iter())
                    .filter(|r| r.run == *run)
                    .map(|r| r.at)
                    .collect();
                let last = began
                    .last()
                    .ok_or_else(|| Broken(format!("run {} is busy with no round", run.0)))?;
                Now::Round {
                    cell: cell.to_string(),
                    run: run.0,
                    round: began.len(),
                    since: time(work, *last)?,
                }
            }
            Decision::Paused => Now::paused(work)?,
            // A run at rest that goes on holds its next round while the work is paused: the engine leaves a work so
            // only when it is paused, as when the run's last round failed.
            Decision::GoOn { .. } if decide::paused(&work.records) => Now::paused(work)?,
            // A cell that would run or go on, and does not, is held by the wait every work shares, where it holds now
            // and holds the cell's rounds: the engine leaves a work so only then.
            Decision::Run { cell, .. } | Decision::GoOn { cell, .. } if held(cell) => Now::Waits {
                at: cell.to_string(),
                on: outage
                    .into_iter()
                    .map(|outage| Waited::Agents { outage: outage.clone() })
                    .collect(),
            },
            Decision::Wait { at, on } => Now::Waits {
                at: at.to_string(),
                on: (on.iter())
                    .map(|waited| {
                        Ok(match waited {
                            WaitedOf::Call { call, on } => Waited::Call {
                                call: call.0,
                                who: on.as_ref().map(Person::of),
                                what: call_of(work, *call)?,
                            },
                            WaitedOf::Precondition { test, by } => Waited::Precondition {
                                test: test.to_string(),
                                words: (kept.flow.tests.get(test).cloned())
                                    .ok_or_else(|| Broken(format!("the flow has no test {test}")))?,
                                by: by.iter().map(Maker::of).collect(),
                            },
                            WaitedOf::Limit { runs } => Waited::Limit { runs: *runs },
                        })
                    })
                    .collect::<Result<_, Broken>>()?,
            },
            Decision::Run { cell, .. } | Decision::GoOn { cell, .. } | Decision::Abandon { cell, .. } => {
                Now::Next { cell: cell.to_string() }
            }
        })
    }

    /// The work's pause, with who paused it and why.
    fn paused(work: &Work) -> Result<Now, Broken> {
        let pause = decide::pause(&work.records).ok_or_else(|| Broken("the work has no pause".to_string()))?;
        Ok(Now::Paused {
            by: Pauser::of(pause.by),
            why: pause.why.clone(),
        })
    }
}

impl Pauser {
    fn of(by: PauserOf) -> Pauser {
        match by {
            PauserOf::Owner => Pauser::Owner,
            PauserOf::Engine => Pauser::Engine,
        }
    }
}

impl Person {
    fn of(who: &Who) -> Person {
        match who {
            Who::Owner => Person::Owner,
            Who::Architect => Person::Architect,
        }
    }
}

impl Maker {
    fn of(maker: &MakerOf) -> Maker {
        match maker {
            MakerOf::Owner => Maker::Owner,
            MakerOf::Outside { source } => Maker::Outside { source: source.clone() },
            MakerOf::Cell { cell } => Maker::Cell { cell: cell.to_string() },
        }
    }
}

impl Spent {
    fn with(self, other: Spent) -> Spent {
        Spent {
            usd_micros: self.usd_micros + other.usd_micros,
            input_tokens: self.input_tokens + other.input_tokens,
            cached_tokens: self.cached_tokens + other.cached_tokens,
            output_tokens: self.output_tokens + other.output_tokens,
            working_ms: self.working_ms + other.working_ms,
            rounds: self.rounds + other.rounds,
        }
    }
}

impl WorkPage {
    /// The work of this number, as the driver holds it now; none where the driver has no such work.
    pub fn of(driver: &Driver, number: WorkNumber, now: Time) -> Result<Option<WorkPage>, Broken> {
        let Some(kept) = driver.work(number) else {
            return Ok(None);
        };
        let view = viewed(driver, number)?;
        let work = kept.work;
        let writer = |by: &WriterOf| -> Result<Writer, Broken> {
            Ok(match by {
                WriterOf::Person => Writer::Person,
                WriterOf::Outside => Writer::Outside,
                WriterOf::Run(run) => Writer::Run {
                    run: run.0,
                    cell: cell_of(work, *run)?,
                },
            })
        };
        let mut blocks = Vec::new();
        for (block, versions) in &view.blocks {
            let versions = (versions.iter())
                .map(|version| {
                    Ok(Version {
                        at: version.at.0,
                        time: time(work, version.at)?,
                        by: writer(&version.by)?,
                        meaning: version.meaning.0,
                        value: version.value.clone(),
                        chars: chars(work, block, version.at)?,
                    })
                })
                .collect::<Result<_, Broken>>()?;
            blocks.push(Block {
                name: block.to_string(),
                versions,
            });
        }
        let cells = (view.cells.iter())
            .map(|cell| CellDetail {
                cell: cell.cell.to_string(),
                needs: cell.needs.iter().map(Cause::of).collect(),
                inputs: (cell.inputs.iter())
                    .map(|input| Input {
                        block: input.block.to_string(),
                        current: input.current.map(|at| at.0),
                        known: input.known.map(|at| at.0),
                        changed: input.changed,
                    })
                    .collect(),
            })
            .collect();
        let mut steps: Vec<Step> = runs(kept)?.into_iter().map(Step::Run).collect();
        for written in &work.records.written {
            let version = (view.blocks.get(&written.block))
                .and_then(|versions| versions.iter().find(|v| v.at == written.at))
                .ok_or_else(|| {
                    Broken(format!(
                        "the write of {} at {} is no version",
                        written.block, written.at.0
                    ))
                })?;
            steps.push(Step::Wrote {
                at: written.at.0,
                time: time(work, written.at)?,
                block: written.block.to_string(),
                by: writer(&version.by)?,
                changed: version.meaning == version.at,
                value: written.value.clone(),
                chars: chars(work, &written.block, written.at)?,
            });
        }
        for let_go in &work.records.let_go {
            steps.push(Step::LetGo {
                at: let_go.at.0,
                time: time(work, let_go.at)?,
                cell: let_go.cell.to_string(),
            });
        }
        for pause in &work.records.pauses {
            let (at, time) = (pause.at.0, time(work, pause.at)?);
            steps.push(if pause.paused {
                Step::Paused {
                    at,
                    time,
                    by: Pauser::of(pause.by),
                    why: pause.why.clone(),
                }
            } else {
                Step::Resumed { at, time }
            });
        }
        if let Some(at) = work.records.cancelled {
            steps.push(Step::Cancelled {
                at: at.0,
                time: time(work, at)?,
            });
        }
        for session in &work.sessions {
            steps.push(Step::Session {
                at: session.at.0,
                time: time(work, session.at)?,
                line: session.line.clone(),
                why: SessionWhy::of(&session.why),
            });
        }
        steps.sort_by_key(Step::at);
        Ok(Some(WorkPage {
            summary: Summary::of(
                kept,
                &view,
                driver
                    .outage()
                    .map(|outage| Outage::of(&outage, driver.holds(now)))
                    .as_ref(),
            )?,
            title: kept.item.title.clone(),
            level: kept.item.level.clone(),
            flow: Flow::of(kept.flow),
            cells,
            blocks,
            steps,
        }))
    }
}

impl Step {
    /// The place of the entry the step stands at.
    fn at(&self) -> u64 {
        match self {
            Step::Run(run) => run.at,
            Step::Wrote { at, .. }
            | Step::LetGo { at, .. }
            | Step::Paused { at, .. }
            | Step::Resumed { at, .. }
            | Step::Cancelled { at, .. }
            | Step::Session { at, .. } => *at,
        }
    }
}

impl Cause {
    fn of(cause: &CauseOf) -> Cause {
        match cause {
            CauseOf::NoRecord => Cause::NoRecord,
            CauseOf::Changed { block, was, behind } => Cause::Changed {
                block: block.to_string(),
                was: was.map(|at| at.0),
                behind: *behind,
            },
        }
    }
}

impl VersionPage {
    /// The content of the version of the block written at this place; none where the work has no such version.
    pub fn of(driver: &Driver, number: WorkNumber, block: &str, at: u64) -> Option<VersionPage> {
        let work = driver.work(number)?.work;
        let content = work.contents.get(&(BlockId::from(block), Seq(at)))?;
        Some(VersionPage {
            block: block.to_string(),
            at,
            content: content.clone(),
        })
    }
}

impl RoundPage {
    /// The round of this number of the run, in full; none where the work has no such round.
    pub fn of(driver: &Driver, work: WorkNumber, run: u64, number: usize) -> Result<Option<RoundPage>, Broken> {
        let Some(kept) = driver.work(work) else {
            return Ok(None);
        };
        let work = kept.work;
        let run = RunId(run);
        let Some(given) = work.given.get(&(run, number)) else {
            return Ok(None);
        };
        let results = match given {
            GivenOf::Results(calls) => (calls.iter())
                .map(|call| CallPage::made(work, *call))
                .collect::<Result<_, Broken>>()?,
            GivenOf::Task | GivenOf::SentBack(_) => Vec::new(),
        };
        let handed = match work.back.get(&(run, number)) {
            Some(BackOf::HandedIn(handed)) => (handed.iter())
                .map(|h| HandedContent {
                    block: h.block.to_string(),
                    content: h.content.clone(),
                })
                .collect(),
            Some(BackOf::Asked(_) | BackOf::Failed { .. }) | None => Vec::new(),
        };
        Ok(Some(RoundPage {
            run: run.0,
            number,
            results,
            handed,
        }))
    }
}

impl ToldPage {
    pub fn of(told: service::Told) -> ToldPage {
        let tries = (told.tries.into_iter())
            .map(|t| TryPage {
                message: t.message,
                cut: t.cut,
                back: match t.back {
                    service::Back::Nothing => TryBack::Nothing,
                    service::Back::Returned(r) => TryBack::Returned {
                        text: r.text,
                        structured: r.structured,
                        turns: r.turns,
                        duration_ms: r.duration_ms,
                        models: r.models,
                    },
                    service::Back::Failed(why) => TryBack::Failed { why },
                    service::Back::Unreadable(why) => TryBack::Unreadable { why },
                },
                ran: t.ran.map(|on| RanOn {
                    level: on.level,
                    model: on.model,
                    effort: on.effort,
                }),
            })
            .collect();
        ToldPage {
            tries,
            left_out: told.left_out,
        }
    }
}

impl CallPage {
    /// The call of this number with its result; none where the work has no such call.
    pub fn of(driver: &Driver, work: WorkNumber, call: u64) -> Result<Option<CallPage>, Broken> {
        let Some(kept) = driver.work(work) else {
            return Ok(None);
        };
        if !kept.work.calls.contains_key(&CallId(call)) {
            return Ok(None);
        }
        CallPage::made(kept.work, CallId(call)).map(Some)
    }

    fn made(work: &Work, call: CallId) -> Result<CallPage, Broken> {
        Ok(CallPage {
            call: call.0,
            what: call_of(work, call)?,
            result: work.results.get(&call).cloned(),
        })
    }
}

impl Act {
    /// The act as the loop takes it.
    pub fn taken(self) -> service::Act {
        match self {
            Act::Write { block, content, value } => service::Act::Write {
                block: BlockId(block),
                content,
                value,
            },
            Act::LetGo { cell } => service::Act::LetGo { cell: CellId(cell) },
            Act::Pause {} => service::Act::Pause,
            Act::Resume {} => service::Act::Resume,
            Act::Cancel {} => service::Act::Cancel,
            Act::Answer { call, result } => service::Act::Answer {
                call: CallId(call),
                result,
            },
            Act::NewSession { line } => service::Act::NewSession { line },
        }
    }
}

impl NewItem {
    /// The item as the driver adds it, at the level it asks for or else at the default of these levels; or why it
    /// cannot be at the level it asks for, in words.
    pub fn taken(self, levels: &Levels) -> Result<driver::NewItem, String> {
        let level = levels.of_item(self.level.as_deref())?;
        Ok(driver::NewItem {
            title: self.title,
            level,
            depends_on: self.depends_on.into_iter().map(ItemNumber).collect(),
            flow: self.flow,
            writes: (self.writes.into_iter())
                .map(|write| driver::FirstWrite {
                    block: BlockId(write.block),
                    content: write.content,
                    value: write.value,
                })
                .collect(),
        })
    }
}

impl Changed {
    pub fn of(changed: &service::Changed) -> Changed {
        Changed {
            works: changed.works.iter().map(|work| work.0).collect(),
            items: changed.items,
        }
    }
}

/// The deciding code's view of the work, which the driver holds.
fn viewed(driver: &Driver, number: WorkNumber) -> Result<View, Broken> {
    driver
        .view(number)
        .ok_or_else(|| Broken(format!("the driver shows work {} and has no view of it", number.0)))
}

/// When the entry at this place was added. Every entry the engine's loop adds has its time.
fn time(work: &Work, at: Seq) -> Result<u64, Broken> {
    (work.time_of(at).map(|time| time.0)).ok_or_else(|| Broken(format!("the entry at {} has no time", at.0)))
}

/// How long the content of the version of the block written at this place is, in characters.
fn chars(work: &Work, block: &BlockId, at: Seq) -> Result<usize, Broken> {
    (work.contents.get(&(block.clone(), at)))
        .map(|content| content.chars().count())
        .ok_or_else(|| Broken(format!("the version of {block} at {} has no content", at.0)))
}

/// Whether the run's last round that ended failed.
fn failed(work: &Work, run: RunId) -> bool {
    let ended = work.records.returned.iter().filter(|r| r.run == run).count();
    matches!(work.back.get(&(run, ended)), Some(BackOf::Failed { .. }))
}

/// The cell the run is of.
fn cell_of(work: &Work, run: RunId) -> Result<String, Broken> {
    (work.records.started.iter())
        .find(|started| started.run == run)
        .map(|started| started.cell.to_string())
        .ok_or_else(|| Broken(format!("run {} never started", run.0)))
}

/// What the call is.
fn call_of(work: &Work, call: CallId) -> Result<Call, Broken> {
    Ok(match work.calls.get(&call) {
        Some(CallOf::Asked(ask)) => match Ask::of(ask) {
            Ask::Tool { name, input } => Call::Tool { name, input },
            Ask::Question { who, question } => Call::Question { who, question },
        },
        Some(CallOf::AtLimit(limit)) => Call::AtLimit {
            limit: match limit {
                LimitOf::Returns { sent_back } => Limit::Returns { sent_back: *sent_back },
                LimitOf::Calls { made } => Limit::Calls { made: *made },
            },
        },
        Some(CallOf::Wait { until, failed }) => Call::Wait {
            until: until.0,
            failed: *failed,
        },
        None => return Err(Broken(format!("call {} is not held", call.0))),
    })
}

impl Ask {
    fn of(ask: &AskOf) -> Ask {
        match ask {
            AskOf::Tool { name, input } => Ask::Tool {
                name: name.clone(),
                input: input.clone(),
            },
            AskOf::Person { who, question } => Ask::Question {
                who: Person::of(who),
                question: question.clone(),
            },
        }
    }
}

impl Unmet {
    fn of(unmet: &UnmetOf) -> Unmet {
        match unmet {
            UnmetOf::AsksNothing => Unmet::AsksNothing,
            UnmetOf::NotAnOutput { block } => Unmet::NotAnOutput {
                block: block.to_string(),
            },
            UnmetOf::WrittenTwice { block } => Unmet::WrittenTwice {
                block: block.to_string(),
            },
            UnmetOf::NoValue { block } => Unmet::NoValue {
                block: block.to_string(),
            },
            UnmetOf::NotAValue { block, value } => Unmet::NotAValue {
                block: block.to_string(),
                value: value.clone(),
            },
            UnmetOf::NotAChoice { block } => Unmet::NotAChoice {
                block: block.to_string(),
            },
            UnmetOf::Unreadable { why } => Unmet::Unreadable { why: why.clone() },
            UnmetOf::NotHandedIn { block } => Unmet::NotHandedIn {
                block: block.to_string(),
            },
        }
    }
}

/// Every run of the work, in the order they started, each with its rounds, its calls and how it ended.
fn runs(kept: Kept<'_>) -> Result<Vec<Run>, Broken> {
    let work = kept.work;
    let records = &work.records;
    let mut runs = Vec::new();
    for started in &records.started {
        let run = started.run;
        let began: Vec<Seq> = (records.rounds.iter()).filter(|r| r.run == run).map(|r| r.at).collect();
        let ended: Vec<Seq> = (records.returned.iter())
            .filter(|r| r.run == run)
            .map(|r| r.at)
            .collect();
        let mut rounds = Vec::new();
        for (i, &at) in began.iter().enumerate() {
            let number = i + 1;
            let began = time(work, at)?;
            let ended = ended.get(i).map(|&at| time(work, at)).transpose()?;
            let given = match work.given.get(&(run, number)) {
                Some(GivenOf::Task) => Given::Task,
                Some(GivenOf::Results(calls)) => Given::Results {
                    calls: calls.iter().map(|call| call.0).collect(),
                },
                Some(GivenOf::SentBack(unmet)) => Given::SentBack {
                    unmet: unmet.iter().map(Unmet::of).collect(),
                },
                None => return Err(Broken(format!("round {number} of run {} was given nothing", run.0))),
            };
            let back = work.back.get(&(run, number)).map(|back| match back {
                BackOf::Asked(asks) => Back::Asked {
                    asks: asks.iter().map(Ask::of).collect(),
                },
                BackOf::HandedIn(handed) => Back::HandedIn {
                    blocks: (handed.iter())
                        .map(|h| Handed {
                            block: h.block.to_string(),
                            matters: h.matters,
                            value: h.value.clone(),
                            chars: h.content.chars().count(),
                        })
                        .collect(),
                },
                BackOf::Failed { why, failure, resets } => Back::Failed {
                    why: why.clone(),
                    failure: Failure::of(*failure),
                    resets: resets.map(|at| at.0),
                },
            });
            let took = ended.map(|_| work.spent.get(&(run, number)));
            let spent = match ended {
                None => None,
                Some(ended) => {
                    let spent = (work.spent.get(&(run, number)))
                        .ok_or_else(|| Broken(format!("round {number} of run {} ended and took nothing", run.0)))?;
                    Some(Spent {
                        usd_micros: spent.usd_micros,
                        input_tokens: spent.input_tokens,
                        cached_tokens: spent.cached_tokens,
                        output_tokens: spent.output_tokens,
                        working_ms: ended.saturating_sub(began),
                        rounds: 1,
                    })
                }
            };
            let took = took.flatten();
            rounds.push(Round {
                number,
                began,
                ended,
                given,
                back,
                spent,
                context: took.and_then(|t| t.context_tokens),
                compactions: took.map_or(0, |t| t.compactions),
            });
        }
        let mut calls = Vec::new();
        for called in records.called.iter().filter(|called| called.run == run) {
            let done = records.done.iter().find(|done| done.call == called.call);
            calls.push(CallRow {
                call: called.call.0,
                what: call_of(work, called.call)?,
                made: time(work, called.at)?,
                done: done.map(|done| time(work, done.at)).transpose()?,
                chars: work.results.get(&called.call).map(|result| result.chars().count()),
            });
        }
        let finished = records.finished.iter().find(|finished| finished.run == run);
        let abandoned = records.abandoned.iter().find(|abandoned| abandoned.run == run);
        let end = match (finished, abandoned) {
            (Some(finished), _) => Some(RunEnd::Finished {
                at: finished.at.0,
                time: time(work, finished.at)?,
                writes: (finished.writes.iter())
                    .map(|write| {
                        Ok(Handed {
                            block: write.block.to_string(),
                            matters: write.matters,
                            value: write.value.clone(),
                            chars: chars(work, &write.block, finished.at)?,
                        })
                    })
                    .collect::<Result<_, Broken>>()?,
            }),
            (None, Some(abandoned)) => Some(RunEnd::Abandoned {
                at: abandoned.at.0,
                time: time(work, abandoned.at)?,
            }),
            (None, None) => None,
        };
        runs.push(Run {
            run: run.0,
            cell: started.cell.to_string(),
            at: started.at.0,
            time: time(work, started.at)?,
            because: started.because.iter().map(Cause::of).collect(),
            basis: (started.basis.iter())
                .map(|(block, version)| Basis {
                    block: block.to_string(),
                    version: version.map(|at| at.0),
                })
                .collect(),
            spent: (rounds.iter())
                .filter_map(|round| round.spent)
                .fold(Spent::default(), Spent::with),
            rounds,
            calls,
            end,
        });
    }
    Ok(runs)
}

impl Outage {
    fn of(outage: &driver::Outage, holds: bool) -> Outage {
        Outage {
            failure: Failure::of(outage.failure),
            why: outage.why.clone(),
            work: outage.work.0,
            run: outage.run.0,
            round: outage.round,
            since: outage.since.0,
            until: match outage.until() {
                UntilOf::At(at) => Until::At { at: at.0 },
                UntilOf::Trial { at, look, latest } => Until::Trial {
                    at: at.0,
                    look,
                    latest: latest.0,
                },
                UntilOf::Tried => Until::Tried,
            },
            holds,
        }
    }
}

impl Failure {
    fn of(failure: FailureOf) -> Failure {
        match failure {
            FailureOf::Passing => Failure::Passing,
            FailureOf::Unread => Failure::Unread,
            FailureOf::Flagged => Failure::Flagged,
            FailureOf::UsageLimit => Failure::UsageLimit,
            FailureOf::CannotRun => Failure::CannotRun,
            FailureOf::Offline => Failure::Offline,
            FailureOf::PromptTooLong => Failure::PromptTooLong,
            FailureOf::Other => Failure::Other,
        }
    }
}
