//! The items and the works as the pages read them: their own types, named for what the owner reads, made from what
//! the driver holds, and read back by the command line. Nothing here is decided: what a work does now is the deciding
//! code's own account of it ([`decide::view`]), so that what is shown and what is decided cannot differ (property
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
//! - [`SystemPage`]: the system prompt a line's session began with, as its carrier kept it.
//! - [`DoingPage`]: what the agent of a round was last seen doing, as its carrier keeps it as it comes.
//! - [`Act`], [`NewItem`]: what the owner does on the pages.
//! - [`Engine`], [`EngineAct`]: the engine as a whole, its slots and the pause of everything, and what the owner
//!   does to it.
//!
//! A work whose records name what it does not hold, which the engine never writes, is not shown as something it is
//! not: making its view gives [`Broken`], saying what is missing.

use std::collections::BTreeMap;

use decide::run::{Failure as FailureOf, Limit as LimitOf, Unmet as UnmetOf};
use decide::{
    BlockId, CallId, Cause as CauseOf, CellId, Condition as Applies, Decision, End, Holder, Maker as MakerOf,
    Pauser as PauserOf, Records, RunId, Seq, Unrun as UnrunOf, View, Waited as WaitedOf, Who, Writer as WriterOf,
};
use driver::{Along, Driver, Kept, Under, Until as UntilOf};
use flow::Kind;
use serde::{Deserialize, Serialize};
use service::{Levels, Projects};
use store::{ItemNumber, WorkNumber};
use work::{Ask as AskOf, Back as BackOf, Call as CallOf, Fill as FillOf, Given as GivenOf, Time, Why, Work, sessions};

use crate::view::Flow;

/// Every item, in the order of their numbers, each with its work.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Items {
    pub items: Vec<Item>,
    /// A wait every work shares, where one stands: no agent's round of any work begins.
    pub outage: Option<Outage>,
}

/// A wait every work shares: a round of an agent failed for a cause outside every work, and no agent's round of any
/// work begins while it holds; the engine checks the outside by itself meanwhile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outage {
    /// The service whose rounds it holds: the agents', or the host's the acts reach.
    pub service: Service,
    /// The kind of the cause the latest round that failed so met: the usage limit, a command line that cannot run, no
    /// network, or a service that does not serve.
    pub failure: Failure,
    /// Why that round failed, as what carried it out said.
    pub why: String,
    /// That round: of this work, this run, this number.
    pub work: u64,
    pub run: u64,
    pub round: usize,
    /// Since when what the rounds need is down: when the first round that failed so in the wait ended.
    pub since: u64,
    /// The latest check of the outside in the wait, where one was taken.
    pub checked: Option<Checked>,
    /// When the rounds it holds may begin again.
    pub until: Until,
    /// Whether it holds rounds now: once a check found the outside well, it stands only until a round that began after
    /// the latest that failed so comes back, and holds nothing meanwhile.
    pub holds: bool,
}

/// The latest check of the outside of the agents' rounds in a wait: when, how many were taken in the wait, and what it
/// found.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Checked {
    pub at: u64,
    pub taken: u32,
    pub found: Found,
}

/// What a check of the outside found: everything well, or what is down, as a failed round's kind of cause, and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Found {
    Well,
    Down { failure: Failure, why: String },
}

/// A service the works reach, each with its wait.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Service {
    /// The agents' service.
    Agents,
    /// The host of the project's repository, as the engine's acts reach it.
    Host,
}

/// When the rounds a wait holds may begin again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Until {
    /// Once a check of the outside of the agents' rounds, which the engine takes at this time, finds it well: at a
    /// usage limit whose reset was said, at the reset.
    Checked { at: u64 },
    /// Now: the check taken at this time found the outside well, and every work goes on.
    Well { at: u64 },
    /// Every act, once a look at the host, taken every half minute from this time, reaches it.
    Looked { at: u64 },
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
    /// The run guard stopped it, its context growing past what its cell's history explains: tried again in a new
    /// session of its line.
    Guarded,
    /// The account's usage limit.
    UsageLimit,
    /// The agents' command line cannot run.
    CannotRun,
    /// The network cannot be reached.
    Offline,
    /// The agents' service is down, overloaded, throttling the account's requests or not answering in time.
    Unavailable,
    /// The prompt is too long for the model.
    PromptTooLong,
    /// The service has no model of the name the round's level gives.
    NoModel,
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
    /// The project it belongs to, by the name the installation's settings give it, whose repository its works run in;
    /// none for an item added before items had projects.
    pub project: Option<String>,
    /// When it was added, in milliseconds since the start of 1970, as every time here is.
    pub added: u64,
    /// The items it depends on, in the order of their numbers.
    pub depends_on: Vec<u64>,
    /// The tags it carries, in the order of their names.
    #[serde(default)]
    pub tags: Vec<String>,
    /// What it was as it was added, and each change made to it since: what it said before, and when it changed.
    pub history: ItemHistory,
    /// The one work done for it, which was added with it.
    pub work: Summary,
}

/// An item as it was added, and each change made to it since, in the order they were made: its title, level and
/// dependencies as they stand are those it was added with, with each change made in turn.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemHistory {
    pub first: FirstItem,
    pub changes: Vec<ItemChange>,
}

/// An item as it was added.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirstItem {
    pub title: String,
    /// None for an item added before items had levels.
    pub level: Option<String>,
    pub depends_on: Vec<u64>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// A change made to an item, at this time, and why, as whoever made it said; none for a change kept before changes
/// said why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemChange {
    pub time: u64,
    #[serde(default)]
    pub why: Option<String>,
    pub what: ItemChanged,
}

/// What a change made to an item did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ItemChanged {
    /// It was given this title.
    Title { title: String },
    /// It was put at this level.
    Level { level: String },
    /// It came to depend on this item.
    DependsOn { item: u64 },
    /// It no longer depends on this item.
    NoLongerDependsOn { item: u64 },
    /// It came to carry this tag.
    Tagged { tag: String },
    /// It no longer carries this tag.
    Untagged { tag: String },
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
    /// Who paused the work, where it is paused: no round of it begins but those of a run that answers a note, a
    /// person's words to its cell. A round under way goes on, and what it hands in is taken.
    pub paused: Option<Pauser>,
    /// What the work does now.
    pub now: Now,
    /// Each cell of its flow as it stands for this work, in the order of the walk.
    pub cells: Vec<CellNow>,
    /// What all its rounds that ended took, and its keep-warm pings, which count in what it spent as its rounds do:
    /// `rounds` counts its rounds alone.
    pub spent: Spent,
    /// What the keep-warm pings of its sessions took, each ping counted as a round, none of them under way for any
    /// time the page counts.
    #[serde(default)]
    pub pinged: Spent,
    /// When something last happened to it; none where nothing has.
    pub last: Option<u64>,
    /// When its first round began; none before it has one.
    #[serde(default)]
    pub from: Option<u64>,
    /// Where its time went, from its first round's begin to the end of the last round or tool's call (one under way
    /// counted to now), cut into [`BAR_CELLS`] equal cells, one character each in order: `m` where an agent's rounds
    /// ran most of the cell, `t` where an engine cell's rounds or the tools' calls did; where less than half of it was
    /// run by either, what the work waited for at the cell's middle, on the records as they stood then: `o` where it
    /// waited on the owner, as the one definition of that says ([`driver::owner`]), `p` where a pause held it, its own
    /// or the owner's pause of everything, `w` where it waited for the items its item depends on, and `.` anything
    /// else, a stalled work among it; empty for a work with no round or a span with no length.
    #[serde(default)]
    pub bar: String,
    /// How many of its rounds were given back what the round before handed in, sent back as it did not do.
    #[serde(default)]
    pub sent_back: usize,
    /// Everything the work waits on the owner for now, in the order it waits for them, as the one definition of it
    /// says ([`driver::owner`]): what the count, the marks, the cards and the notifications of the pages read.
    #[serde(default)]
    pub waits: Vec<Wait>,
}

/// One thing a work waits on the owner for: what names it within its work, which a notification's address and the
/// card that shows it carry; why, in a sentence, a question's own words for a question; and what the owner does that
/// ends it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wait {
    pub key: String,
    pub why: String,
    pub ends: Ends,
    /// For a question a cell's precondition waits for, which the work went on past at the cell that asked it, why the
    /// work waits for it there, in a sentence, which the question's card alone says under what it asks; none else.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub under: Option<String>,
}

/// What the owner does that ends a wait on them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Ends {
    /// Answers this version of the work's question of this name, by one of its options.
    Answer { question: String, version: u64 },
    /// Answers in words the question this call puts to them, as one work's old records hold one.
    Reply { call: u64 },
    /// Lets the run that reached its limit go on, as the result of this call.
    GoOn { call: u64 },
    /// Lets the cell go on past its limit.
    LetGo { cell: String },
    /// Writes the block with this value, which makes the precondition hold; none where the block's values do not make
    /// that plain.
    Write { block: String, value: Option<String> },
    /// Lets the work go on, lifting the engine's pause.
    Resume {},
    /// Lets every work go on, lifting the pause of everything the engine set.
    ResumeEverything {},
    /// Does what this says, which no control of the page does.
    Outside { what: String },
}

impl Wait {
    pub(crate) fn of(waiting: driver::owner::Waiting) -> Wait {
        use driver::owner::Ends as EndsOf;
        Wait {
            key: waiting.key,
            why: waiting.why,
            ends: match waiting.ends {
                EndsOf::Answer { question, version } => Ends::Answer {
                    question,
                    version: version.0,
                },
                EndsOf::Reply { call } => Ends::Reply { call: call.0 },
                EndsOf::GoOn { call } => Ends::GoOn { call: call.0 },
                EndsOf::LetGo { cell } => Ends::LetGo { cell: cell.to_string() },
                EndsOf::Write { block, value } => Ends::Write {
                    block: block.to_string(),
                    value,
                },
                EndsOf::Resume => Ends::Resume {},
                EndsOf::ResumeEverything => Ends::ResumeEverything {},
                EndsOf::Outside { what } => Ends::Outside { what },
            },
            under: waiting.under,
        }
    }
}

/// How many equal cells a work's time bar is cut into.
const BAR_CELLS: u64 = 96;

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
    /// A round of this run of this cell is under way, since this time; where it is an engine cell's act that runs as
    /// a CI run, the run's link, as the act says as it goes.
    Round {
        cell: String,
        run: u64,
        round: usize,
        since: u64,
        #[serde(default)]
        at: Option<String>,
    },
    /// It is paused, and what it would do next is begin a round: by the owner, or by the engine itself for this
    /// reason, as when a round failed.
    Paused { by: Pauser, why: Option<String> },
    /// The pause of everything holds it back, and what it would do next is begin a round; its own pause does not hold
    /// it. `by` is who set it: the owner, or the engine as it started with works going on, which holds every work until
    /// the owner lifts it.
    Everything { by: Pauser },
    /// It waits at this cell, for each of these.
    Waits { at: String, on: Vec<Waited> },
    /// It is about to do something at this cell. A work is never left so between two things that happen to it.
    Next { cell: String },
    /// It can go on by itself no further at this cell, and nothing pauses it: each key of the cell's `requires` that
    /// does not hold, none where it stands at its last cell for want of anything new to run it; and each cell that
    /// writes what it waits for, none of which runs, with why; and all that in a sentence, in the words beside the
    /// flows (`stalled`), which every reader says. It goes on once the owner, or a source outside the work, writes what
    /// runs a cell.
    Stalled {
        at: String,
        on: Vec<CellKey>,
        by: Vec<Stalling>,
        #[serde(default)]
        why: String,
    },
}

/// A key of a cell's `when` or `requires`: the named test, or the block's key as the cell's file writes it
/// (`checked = yes`); and the words shown for it, the test's or the same key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellKey {
    pub test: String,
    pub words: String,
}

/// A cell that writes what a stalled work waits for, and why it does not run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stalling {
    pub cell: String,
    pub why: Unrun,
}

/// Why a cell that writes what a stalled work waits for does not run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Unrun {
    /// Its condition does not hold: these keys of its `when`.
    Skipped { on: Vec<CellKey> },
    /// Nothing new runs it.
    NothingNew {},
    /// It stands at or after the cell the work stands at, whose `requires` hold it.
    Held {},
}

/// What a work waits for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Waited {
    /// The result of a call a run rests at: from this person, or from the engine where there is none; made at this
    /// time, and, for a tool that runs as a CI run, the run's link, as the tool says as it goes.
    Call {
        call: u64,
        who: Option<Person>,
        what: Call,
        #[serde(default)]
        made: u64,
        #[serde(default)]
        at: Option<String>,
    },
    /// A key of the cell's `requires` does not hold: the named test, or the block's key as the cell's file writes it
    /// (`dependencies = merged`); the words shown for it, the test's or the same key; and each who can make it hold, by
    /// writing a block that keeps it from holding.
    Precondition {
        test: String,
        words: String,
        by: Vec<Maker>,
    },
    /// This many of the cell's runs since its counts last began anew were each for what this cell after it wrote, and
    /// so would the next be: the owner lets it go on or cancels the work.
    Limit { by: String, runs: u32 },
    /// The result of what the cell asked for in this record, which these cells give, those the record runs: the work
    /// runs them and no other cell meanwhile.
    Result { record: String, by: Vec<String> },
    /// A free slot: as many rounds of agents are under way as the engine has slots, and the engine gives it the first
    /// that comes free, in its order.
    Slot {},
    /// The wait every work shares: the cell's agent's round does not begin until it is over.
    Agents { outage: Outage },
    /// The owner's answer to the latest version of this question, which the cell asked and waits for.
    Answer { question: String },
    /// A source outside the work, as main moving: only this cell can make a precondition hold, and what its latest run
    /// wrote, `why` in the run's own words, waits for that source before the cell runs again.
    Outside { source: String, cell: String, why: String },
    /// The project's turn to check and merge, which this work holds: the cell's act, its round under way since this
    /// time, waits for it, as the act says. The works of a project check and merge one at a time.
    Turn { holder: u64, since: u64 },
    /// The items the work's item depends on that have not merged, by their numbers: a precondition only they make
    /// hold, as they merge; and those of them that were cancelled, which never will, so that the work waits for good.
    Items {
        items: Vec<u64>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        cancelled: Vec<u64>,
    },
}

/// A person who can give what a work waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Person {
    Owner,
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
    /// A round of its run, an agent's, is under way.
    Running,
    /// The engine carries out something for its run that takes its time: the act of an engine cell, its round under
    /// way, or a tool's call its agent asked for, which the work waits for at the cell.
    Calling,
    /// Its run is at rest: for a call's result, or only for its next round; or, for an engine cell whose act waits for
    /// the project's turn to check and merge, for that turn.
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
    /// The project of its item, as [`Item`] has it.
    pub project: Option<String>,
    /// The tags of its item, as [`Item`] has them.
    #[serde(default)]
    pub tags: Vec<String>,
    /// What its item was as it was added, and each change made to it since, as [`Item`] has them.
    pub history: ItemHistory,
    /// The flow it runs on, as the flow page reads a flow.
    pub flow: Flow,
    /// Each cell now, with why it would run and what it reads, in the order of the walk.
    pub cells: Vec<CellDetail>,
    /// Every block that has a version, by name, with its versions in the order they were written.
    pub blocks: Vec<Block>,
    /// Everything that happened to the work, in the order it happened; a run stands where it started.
    pub steps: Vec<Step>,
    /// Each question put to the owner, as it stands, in the order of their names.
    #[serde(default)]
    pub questions: Vec<QuestionPage>,
    /// Where the owner's words would go now, as the engine chooses it: to each line of the flow, in the order of the
    /// names of the lines, then about each question that stands, in the order of their names.
    #[serde(default)]
    pub speaking: Vec<Speaking>,
}

/// What the owner's words said to a line, or about a question, would reach now, as the engine chooses it: the cell,
/// or why no cell can take them, when they are not sent (crate `engine`, module `spoken`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Speaking {
    pub to: SpokenTo,
    /// The cell the words would reach; none where no cell can take them.
    pub cell: Option<String>,
    /// Why no cell can take them, in words; none where one can.
    pub refused: Option<String>,
}

/// What the owner's words are said to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SpokenTo {
    /// A line of the work's flow.
    Line { line: String },
    /// A question of the work, by its name: the words are about it.
    Question { name: String },
}

impl SpokenTo {
    fn taken(self) -> engine::spoken::To {
        match self {
            SpokenTo::Line { line } => engine::spoken::To::Line(line),
            SpokenTo::Question { name } => engine::spoken::To::Question(name),
        }
    }
}

impl Speaking {
    /// Where the owner's words would go now, to each line of the work's flow and about each question that stands.
    fn all(kept: &Kept) -> Vec<Speaking> {
        let lines = (kept.flow.lines.keys()).map(|line| SpokenTo::Line { line: line.clone() });
        let standing = decide::questions::questions(&kept.work.records);
        let questions = (standing.into_iter())
            .filter(|(_, q)| q.stands())
            .map(|(name, _)| SpokenTo::Question { name: name.to_string() });
        (lines.chain(questions))
            .map(|to| {
                let chosen = engine::spoken::receiver(kept.flow, kept.work, &to.clone().taken());
                Speaking {
                    to,
                    cell: chosen.as_ref().ok().map(|cell| cell.to_string()),
                    refused: chosen.err().map(|why| service::no_one_words(&why)),
                }
            })
            .collect()
    }
}

/// A question put to the owner, as it stands: its latest version, with the keys a return writes it with, and the
/// owner's answer to that version, where there is one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionPage {
    pub name: String,
    /// The cell it belongs to.
    pub cell: String,
    /// The version an answer names: the place of the latest.
    pub version: u64,
    /// The engine itself asked it, and not the cell's agent.
    pub engine: bool,
    pub waits: bool,
    /// The latest version withdraws it, for this reason; none while it stands.
    pub withdrawn: Option<String>,
    /// What it asks, its options and the one it recommends, counted from one; of a question withdrawn, none.
    pub ask: Option<String>,
    pub options: Vec<WayPage>,
    pub recommends: Option<u64>,
    /// The owner's answer to the latest version.
    pub answer: Option<AnswerPage>,
}

/// An option of a question.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WayPage {
    pub does: String,
    pub follows: String,
}

/// The owner's answer to a version of a question: the option taken, counted from one, when, and why where said.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnswerPage {
    pub option: u64,
    pub at: u64,
    pub why: Option<String>,
}

impl QuestionPage {
    /// Each question of the work as it stands.
    fn all(work: &Work) -> Vec<QuestionPage> {
        (decide::questions::questions(&work.records).into_iter())
            .map(|(name, q)| {
                let (ask, options, recommends, withdrawn) = match work.question(q.latest) {
                    Some(work::Question::Asked {
                        ask,
                        options,
                        recommends,
                        ..
                    }) => {
                        let options = (options.iter())
                            .map(|o| WayPage {
                                does: o.does.clone(),
                                follows: o.follows.clone(),
                            })
                            .collect();
                        (Some(ask.clone()), options, *recommends, None)
                    }
                    Some(work::Question::Withdrawn { why, .. }) => (None, Vec::new(), None, Some(why.clone())),
                    None => (None, Vec::new(), None, None),
                };
                QuestionPage {
                    name: name.to_string(),
                    cell: q.latest.cell.to_string(),
                    version: q.latest.at.0,
                    engine: q.latest.engine,
                    waits: q.latest.waits,
                    withdrawn: withdrawn.or_else(|| q.latest.withdrawn.then(String::new)),
                    ask,
                    options,
                    recommends,
                    answer: q.answer.map(|a| AnswerPage {
                        option: a.option,
                        at: a.at.0,
                        why: a.why.clone(),
                    }),
                }
            })
            .collect()
    }
}

/// A cell now: why it would run, and the blocks it reads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellDetail {
    pub cell: String,
    /// What is new to the cell: why it would run were the walk to reach it with nothing holding it; empty where
    /// nothing is.
    pub needs: Vec<Cause>,
    /// The blocks that run it, in the order it lists them.
    pub inputs: Vec<Input>,
}

/// Why a cell runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Cause {
    /// It had no record, as a run started before a cell ran only for what is new says.
    NoRecord,
    /// The block is new to the cell. `was` is the version the cell consumed; `behind` says every new version was
    /// written by a cell after this one.
    Changed {
        block: String,
        was: Option<u64>,
        behind: bool,
    },
    /// The results of the calls its earlier runs asked for are all in.
    Results { calls: Vec<u64> },
}

/// A block that runs a cell, as the cell consumed it and as it is now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub block: String,
    /// The block's current version; none where it has none.
    pub current: Option<u64>,
    /// The version the cell consumed; none where it consumed none.
    pub known: Option<u64>,
    /// Whether the block is new to the cell.
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
    /// The cell it is said to, where it is said to one, as the owner's entry of the record of what is said is.
    pub said_to: Option<String>,
    /// How long its content is, in characters.
    pub chars: usize,
    /// Whether it holds nothing, as the engine recorded it and a test of content reads it.
    pub empty: bool,
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
    /// The work's item was cancelled, for this reason where whoever cancelled it gave one.
    Cancelled { at: u64, time: u64, why: Option<String> },
    /// The work's item, cancelled, was reopened, for this reason: the work is paused until it is let on.
    Reopened { at: u64, time: u64, why: String },
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
    /// The context of the line's session, as its last round ended, was past the limit of its line, in tokens.
    Limit { context: u64, limit: u64 },
    /// The line's rounds run on another model than this one, which the session was begun on.
    Model { was: String },
    /// The line's rounds run under another account than the session was begun under.
    Account,
    /// The owner asked for one.
    Owner,
    /// The service's safety check flagged the line's last round.
    Flagged,
    /// The run guard stopped the line's last round, its context having grown past what its cell's history explains.
    Guarded,
    /// The line's last round failed for a prompt too long for the model.
    TooLong,
    /// The agents' program no longer had the session the line was in.
    Lost,
    /// The agents' program compacted the line's session during a round.
    Compacted,
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
            Why::Guarded => SessionWhy::Guarded,
            Why::TooLong => SessionWhy::TooLong,
            Why::Lost => SessionWhy::Lost,
            Why::Compacted => SessionWhy::Compacted,
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
    /// The session of its line it runs in; none for a round of a cell that is not on a line, as an engine's act is.
    /// Worked out for a work's page only, as are its tries.
    pub session: Option<RoundSession>,
    /// Each try of it the records hold, in order; none for a try that began before tries were recorded.
    pub tries: Vec<RoundTry>,
    /// How it missed its session's prompt cache, where it did. Worked out for a work's page only, as are its tries.
    pub missed: Option<Missed>,
    /// Whether what it took was worked out from its calls at the models' prices, its program having printed no result
    /// to say it, as of a round the run guard stopped: an estimate. Worked out for a work's page only.
    #[serde(default)]
    pub estimated: bool,
    /// What it cost and how long it took, or has taken so far while it is under way, against the lines of their
    /// history. Worked out for a work's page only.
    #[serde(default)]
    pub judged: Judged,
}

/// What a round cost and how long it took against the lines of their history, as they stood when it began: worked out
/// whenever it is read, and stopping nothing (principle `judged-by-own-history`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Judged {
    /// What it cost, in millionths of a dollar, against what rounds of its kind on its model cost: none while it is
    /// under way, for a round of an act, which costs nothing, and where too few rounds tell the line.
    pub cost: Option<Against>,
    /// How long it took, or has taken so far while it is under way, in milliseconds: none where too few rounds tell
    /// the line.
    pub time: Option<Against>,
}

/// A measure against the line of its history.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Against {
    /// The measure: millionths of a dollar for a cost, milliseconds for a time.
    pub value: u64,
    /// Its line, in the same units.
    pub line: Line,
    /// Whether the measure is past its line.
    pub past: bool,
}

/// The line of a measure's history, which stands past the largest of the values of its kind by as much as that is
/// past their median.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    /// Where it stands, in the measure's units, to the nearest whole one.
    pub value: u64,
    /// How many values gave it.
    pub samples: usize,
    /// What gave it.
    pub scope: Scope,
}

impl Line {
    pub(crate) fn of(line: &driver::history::Line) -> Line {
        Line {
            value: line.value.round() as u64,
            samples: line.samples,
            scope: Scope::of(line.scope),
        }
    }
}

/// What gave a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    /// The rounds of the cell.
    Cell,
    /// Every agent's round of the flow.
    Flow,
    /// The calls of the tool.
    Tool,
    /// The flow's other works.
    Works,
    /// The rounds of the line.
    Line,
}

impl Scope {
    pub(crate) fn of(scope: driver::history::Scope) -> Scope {
        match scope {
            driver::history::Scope::Cell => Scope::Cell,
            driver::history::Scope::Flow => Scope::Flow,
            driver::history::Scope::Tool => Scope::Tool,
            driver::history::Scope::Works => Scope::Works,
            driver::history::Scope::Line => Scope::Line,
        }
    }
}

impl Against {
    pub(crate) fn of(against: &driver::judged::Against) -> Against {
        Against {
            value: against.value.round() as u64,
            line: Line::of(&against.line),
            past: against.past(),
        }
    }
}

impl Judged {
    pub(crate) fn of(round: &driver::judged::Round) -> Judged {
        Judged {
            cost: round.cost.as_ref().map(Against::of),
            time: round.time.as_ref().map(Against::of),
        }
    }
}

/// How a round missed its session's prompt cache: it went on in its session within the cache's lifetime, and its first
/// call read from the cache less than half of what the session held.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Missed {
    /// The tokens its first call read from the cache.
    pub read: u64,
    /// The tokens its session held.
    pub held: u64,
    /// What comes before the session's messages in the prompt and differs from the call before, by name: the cause by
    /// design; none where nothing does, and the miss is a fault of wf-agents.
    pub differs: Vec<String>,
}

/// The session of its line a round runs in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundSession {
    pub line: String,
    /// Which of the line's sessions it is, as the records count them: the line's first is none.
    pub number: u32,
}

/// A try of a round, as the records hold it: what it ran on, and what was filled into its session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundTry {
    /// Its number in its round, the first being one.
    pub attempt: usize,
    pub ran: RanOn,
    /// Each time something was filled into its session, in order, what was: what the session lacked of what the
    /// round's cell is given, as the flow's files name it. A try is filled again where its session is begun anew under
    /// it.
    pub filled: Vec<Vec<Fill>>,
}

/// One thing filled into a session, as the flow's files name it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Fill {
    /// A part of the message that the session keeps, by its `part` in the flow's message.
    Part { part: String },
    /// A version of a datum of the cell's context, whole.
    Whole { block: String, version: u64 },
    /// A version of a datum, as what changed in it since an earlier version, which the session held.
    Change { block: String, version: u64, since: u64 },
    /// The results of these calls.
    Results { calls: Vec<u64> },
    /// Why what the round of this number of the same run returned was sent back.
    SentBack { round: usize },
    /// That a datum of the cell's context has no version yet.
    Unwritten { block: String },
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
    /// The block is named as dealt with, and it did not run the round.
    NotRunBy { block: String },
    /// The block is named twice as dealt with.
    DealtWithTwice { block: String },
    /// Something ran the round, and the return deals with none of it and asks for no call.
    DealtWithNothing,
    /// The block got this version from another writer since the round was told of the one it writes over.
    WrittenOver { block: String, version: u64 },
    /// The block is a choice, written as changing nothing in meaning with another value than it has.
    ChoiceChanged { block: String },
    /// The return asks for calls and also does the cell's work.
    AsksAndWorks,
    /// The return names as dealt with a name that is none of its cell's triggers.
    NotATrigger { name: String },
    /// The return asks for a tool its cell may not ask for.
    NotATool { name: String },
    /// The return names the block as written with its meaning unchanged, and does not write it.
    UnchangedNotWritten { block: String },
    /// The return names a record it adds an entry to as written with its meaning unchanged: each entry is new.
    EntryUnchanged { block: String },
    /// It writes a block of a shape its file gives as something else.
    NotOfItsShape { block: String },
    /// A question it writes is not its cell's own: another cell's, or one the engine asked.
    QuestionTaken { name: String },
    /// It withdraws a question of which none of its cell's stands.
    NoSuchQuestion { name: String },
    /// It writes a question twice.
    QuestionTwice { name: String },
    /// A question it asks recommends none of its options.
    NotAnOption { name: String },
    /// It asks a question from a cell that no answer runs.
    NotAnswerable { name: String },
    /// It deals with the owner's words, by this trigger, and writes no answer in the record of what is said.
    NotAnswered { name: String },
    /// The work's branch changes this file the project protects, and nothing says why.
    Unexplained { path: String },
    /// The run of a line that changes only the project's tests changed this file, which is none of them.
    NotATest { path: String },
    /// It deals with what an act after its cell wrote, and leaves nothing new that runs the act and no question.
    Unanswered { block: String },
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
    /// How long a call of a tool took, or has taken so far while it has no result, in milliseconds, against the line
    /// of the calls of its tool as it stood when it was made. Worked out for a work's page only.
    #[serde(default)]
    pub time: Option<Against>,
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

/// What a try of an agent's round ran on, as the work's records say it was when the try began, or, for a try kept
/// before tries were recorded, as its carrier kept it beside the try.
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

/// The projects the installation names, by their names in order: those an item may be added to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectsPage {
    pub names: Vec<String>,
}

impl ProjectsPage {
    /// The page of these projects.
    pub fn of(projects: &Projects) -> ProjectsPage {
        ProjectsPage {
            names: projects.names().to_vec(),
        }
    }
}

/// A level, with what work it is for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelPage {
    pub name: String,
    pub desc: String,
}

/// Changes of items whoever plans the work asks for together, for one reason: made in the order given, each judged
/// against what those before it make, all of them or none.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ItemChanges {
    /// Why they are made, in a sentence: kept with each.
    pub why: String,
    pub changes: Vec<ChangeAsked>,
}

/// A change of an item, as whoever plans the work asks for it. Each kind is a struct, as an [`Act`] is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ChangeAsked {
    /// The item is given this title, in the owner's language; its merge is named by a line its work writes.
    Title { item: u64, title: String },
    /// The item is put at this level, among those the settings name and at most the highest an item may be at.
    Level { item: u64, level: String },
    /// The item comes to depend on this one, while its work has started no run.
    DependsOn { item: u64, on: u64 },
    /// The item no longer depends on this one, while its work has started no run.
    NoLongerDependsOn { item: u64, on: u64 },
    /// The item comes to carry this tag.
    Tag { item: u64, tag: String },
    /// The item no longer carries this tag.
    Untag { item: u64, tag: String },
    /// The item is cancelled: nothing of its work goes on.
    Cancel { item: u64 },
    /// The item, cancelled, is reopened: its work is paused, and goes on from its records once it is let on.
    Reopen { item: u64 },
}

/// Whether each change of those asked together changed its item, in the order asked: it was so already where not, as
/// when the item was at that level already.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemsChanged {
    pub changed: Vec<bool>,
}

impl ChangeAsked {
    /// The item it changes.
    pub fn item(&self) -> u64 {
        match self {
            ChangeAsked::Title { item, .. }
            | ChangeAsked::Level { item, .. }
            | ChangeAsked::DependsOn { item, .. }
            | ChangeAsked::NoLongerDependsOn { item, .. }
            | ChangeAsked::Tag { item, .. }
            | ChangeAsked::Untag { item, .. }
            | ChangeAsked::Cancel { item }
            | ChangeAsked::Reopen { item } => *item,
        }
    }

    /// The change as the loop takes it, a level being one of these, or why that level is no item's.
    pub fn taken(self, levels: &Levels) -> Result<(ItemNumber, driver::Asked), String> {
        use driver::Asked;
        use store::ItemChange as C;
        let item = ItemNumber(self.item());
        let asked = match self {
            ChangeAsked::Title { title, .. } => {
                levels
                    .title_fits(&title)
                    .map_err(|why| format!("the item's title: {why}"))?;
                Asked::Item(C::Title(title))
            }
            ChangeAsked::Level { level, .. } => Asked::Item(C::Level(
                levels
                    .of_item(Some(&level))
                    .map_err(|why| format!("the item's level: {why}"))?,
            )),
            ChangeAsked::DependsOn { on, .. } => Asked::Item(C::DependsOn(ItemNumber(on))),
            ChangeAsked::NoLongerDependsOn { on, .. } => Asked::Item(C::NoLongerDependsOn(ItemNumber(on))),
            ChangeAsked::Tag { tag, .. } => Asked::Item(C::Tagged(tag)),
            ChangeAsked::Untag { tag, .. } => Asked::Item(C::Untagged(tag)),
            ChangeAsked::Cancel { .. } => Asked::Cancel,
            ChangeAsked::Reopen { .. } => Asked::Reopen,
        };
        Ok((item, asked))
    }
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
        /// The tokens it wrote to the prompt cache, as the program printed them; the records count them among those
        /// not read from the cache.
        cache_creation_input_tokens: Option<u64>,
    },
    /// The program said the try failed, for this reason.
    Failed { why: String },
    /// What was kept of it could not be read, for this reason.
    Unreadable { why: String },
}

/// The system prompt a line's session began with, as it was kept with the session: every round of the session is
/// given it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemPage {
    pub line: String,
    /// Which of the line's sessions it is, as the records count them: the line's first is none.
    pub session: u32,
    pub text: String,
}

/// What the agent of a round was last seen doing, as its program said it as it went: the last few things its latest
/// try did, the latest last.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoingPage {
    pub doing: Vec<Doing>,
}

/// A thing an agent was seen doing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Doing {
    /// It used the tool of this name, on this where its input names what: a command, a path, a pattern.
    Tool { name: String, on: Option<String> },
    /// It said this; the start of it where it is long.
    Said { text: String },
    /// Its session was being compacted.
    Compacting,
}

impl Fill {
    fn of(fill: &FillOf) -> Fill {
        match fill {
            FillOf::Part { part, .. } => Fill::Part { part: part.clone() },
            FillOf::Whole { block, version } => Fill::Whole {
                block: block.to_string(),
                version: version.0,
            },
            FillOf::Change { block, version, since } => Fill::Change {
                block: block.to_string(),
                version: version.0,
                since: since.0,
            },
            FillOf::Results(calls) => Fill::Results {
                calls: calls.iter().map(|call| call.0).collect(),
            },
            FillOf::SentBack { round } => Fill::SentBack { round: *round },
            FillOf::Unwritten { block } => Fill::Unwritten {
                block: block.to_string(),
            },
        }
    }
}

impl Doing {
    fn of(doing: service::Doing) -> Doing {
        match doing {
            service::Doing::Tool { name, on } => Doing::Tool { name, on },
            service::Doing::Said(text) => Doing::Said { text },
            service::Doing::Compacting => Doing::Compacting,
        }
    }
}

impl DoingPage {
    pub fn of(doing: Vec<service::Doing>) -> DoingPage {
        DoingPage {
            doing: doing.into_iter().map(Doing::of).collect(),
        }
    }
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
    /// Cancels the work's item, saying why where a reason is given, such as the item it was joined into.
    Cancel {
        #[serde(default)]
        why: Option<String>,
    },
    /// Gives a call that waits for a person its result: the owner's word that a run at its limit goes on.
    Result { call: u64, result: String },
    /// Answers a version of a question, the one the page shows, taking an option, counted from one, and saying why
    /// where the owner does.
    Answer {
        question: String,
        version: u64,
        option: u64,
        #[serde(default)]
        why: Option<String>,
    },
    /// Has the line begin a new session at its next round.
    NewSession { line: String },
    /// Says words to a line of the work's flow, or about a question of the work: to the cell the engine chooses, which
    /// the work's page shows (`speaking`), and which the act answers with ([`Said`]).
    Say { to: SpokenTo, words: String },
}

/// What the act that says the owner's words answers with: the cell the engine gave them to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Said {
    pub reached: String,
}

/// An item the owner adds, with its work, which is added paused: it begins no round until the owner lets it on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewItem {
    pub title: String,
    /// The level it is given; with none, the level the installation adds an item at where it is given none.
    #[serde(default)]
    pub level: Option<String>,
    /// The project it belongs to, one the installation names.
    pub project: String,
    /// The items it depends on, of its own project or another.
    pub depends_on: Vec<u64>,
    /// The tags it carries; none where none is given.
    #[serde(default)]
    pub tags: Vec<String>,
    /// The name of the flow its work runs on.
    pub flow: String,
    /// What is written for it as it is added, each a block people write.
    pub writes: Vec<NewWrite>,
}

/// The engine as a whole: its slots, and the pause of everything.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Engine {
    /// How many rounds of agents may go on at once.
    pub slots: u64,
    /// How many are under way; more than there are slots where the slots were fewer when the engine started again.
    pub taken: u64,
    /// The pause of everything, while it holds; none while works go on.
    pub paused: Option<EverythingPaused>,
}

/// The pause of everything: when it began, and who set it, the owner, or the engine as it started with works going on,
/// which holds everything until the owner has seen how things stand and lifts it as they lift their own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EverythingPaused {
    pub since: u64,
    pub by: Pauser,
}

/// What the owner does to the engine as a whole on the pages, each kind a struct as an [`Act`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum EngineAct {
    /// Pauses everything: no work begins a round but those of a run that answers a note, until it is lifted.
    Pause {},
    /// Lifts the pause of everything, the owner's or the engine's: every work goes on that its own pause does not hold.
    Resume {},
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

/// Something happened: these works grew, an item was added where `items`, and the copies of the database changed where
/// `copies`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Changed {
    pub works: Vec<u64>,
    pub items: bool,
    pub copies: bool,
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
        let outages = Outages::of(driver);
        let outage = outages.agents.clone();
        for kept in driver.works() {
            let view = viewed(driver, kept.head.number)?;
            items.push(Item {
                number: kept.item.number.0,
                title: kept.item.title.clone(),
                level: kept.item.level.clone(),
                project: kept.item.project.clone(),
                added: kept.item.added.0,
                depends_on: kept.item.depends_on.iter().map(|on| on.0).collect(),
                tags: kept.item.tags.clone(),
                history: ItemHistory::of(kept.item),
                work: Summary::of(kept, &view, driver, &outages, now)?,
            });
        }
        items.sort_by_key(|item| item.number);
        Ok(Items { items, outage })
    }
}

impl ItemHistory {
    /// What the item was as it was added, and each change made to it since.
    pub fn of(item: &store::Item) -> ItemHistory {
        let first = &item.first;
        ItemHistory {
            first: FirstItem {
                title: first.title.clone(),
                level: first.level.clone(),
                depends_on: first.depends_on.iter().map(|on| on.0).collect(),
                tags: first.tags.clone(),
            },
            changes: (item.changes.iter())
                .map(|change| ItemChange {
                    time: change.at.0,
                    why: change.why.clone(),
                    what: ItemChanged::of(&change.what),
                })
                .collect(),
        }
    }
}

impl ItemChanged {
    /// What a change the store keeps did, as the pages read it.
    pub fn of(what: &store::ItemChange) -> ItemChanged {
        match what {
            store::ItemChange::Title(title) => ItemChanged::Title { title: title.clone() },
            store::ItemChange::Level(level) => ItemChanged::Level { level: level.clone() },
            store::ItemChange::DependsOn(on) => ItemChanged::DependsOn { item: on.0 },
            store::ItemChange::NoLongerDependsOn(on) => ItemChanged::NoLongerDependsOn { item: on.0 },
            store::ItemChange::Tagged(tag) => ItemChanged::Tagged { tag: tag.clone() },
            store::ItemChange::Untagged(tag) => ItemChanged::Untagged { tag: tag.clone() },
        }
    }
}

impl Summary {
    /// The work in brief, from what the driver holds of it, the deciding code's view of it, the waits every work
    /// shares as the driver works them out, and what the driver says of the rest: what holds back the round it would
    /// begin next, where each round or call under way stands, as what carries it out says, and what it waits on the
    /// owner for; at the time `when` by the loop's clock.
    pub fn of(kept: Kept<'_>, view: &View, driver: &Driver, outages: &Outages, when: Time) -> Result<Summary, Broken> {
        let work = kept.work;
        let number = kept.head.number;
        let held = driver.held(number);
        let along = &driver.along_of(number);
        let runs = runs(kept)?;
        let engine = |cell: &CellId| (kept.flow.cells.iter()).any(|spec| spec.id == *cell && spec.kind == Kind::Engine);
        // The act of an engine cell that waits for the project's turn runs nothing yet.
        let turn = |run: RunId| {
            (along.iter()).any(|(under, said)| {
                matches!(under, Under::Round { run: of, .. } if *of == run) && matches!(said, Along::Turn { .. })
            })
        };
        let now = Now::of(kept, &view.decision, held, outages, along, &driver.unmerged(number))?;
        // The cell the work waits at for a tool's call the engine runs.
        let calling = match &now {
            Now::Waits { at, on } => (on.iter())
                .any(|waited| {
                    matches!(
                        waited,
                        Waited::Call {
                            who: None,
                            what: Call::Tool { .. },
                            ..
                        }
                    )
                })
                .then_some(at.as_str()),
            _ => None,
        };
        let cells = (view.cells.iter())
            .map(|cell| CellNow {
                cell: cell.cell.to_string(),
                state: match (&cell.open, cell.applies, cell.record) {
                    (Some(open), ..) if open.under_way && engine(&cell.cell) && turn(open.run) => CellState::Resting,
                    (Some(open), ..) if open.under_way && engine(&cell.cell) => CellState::Calling,
                    (Some(open), ..) if open.under_way => CellState::Running,
                    _ if calling == Some(cell.cell.as_str()) => CellState::Calling,
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
            paused: decide::pause(&work.records).map(|pause| Pauser::of(pause.by)),
            waits: (driver.waits_on_owner(number).into_iter()).map(Wait::of).collect(),
            now,
            cells,
            spent: (runs.iter().fold(Spent::default(), |sum, run| sum.with(run.spent)))
                .with(pinged(work, &|_| true).as_no_round()),
            pinged: pinged(work, &|_| true),
            last: work.moments.last().map(|moment| moment.at.0),
            from: (runs.iter().flat_map(|run| &run.rounds)).map(|round| round.began).min(),
            bar: bar(kept, &runs, &HeldAll::of(driver), when),
            sent_back: (runs.iter().flat_map(|run| &run.rounds))
                .filter(|round| matches!(round.given, Given::SentBack { .. }))
                .count(),
        })
    }
}

/// What holds every work at once, as the bar reads it at a moment: each stretch the pause of everything held, from the
/// store's record of every pause set and lifted; and the agents' wait where their command line cannot run, from when,
/// with what it said. Whether that wait held at a moment before is not kept: the engine's checks of the outside, which
/// end it, are no records, so only the one that stands now is read, from when it began.
struct HeldAll {
    everything: Vec<driver::Span>,
    cannot_run: Option<(Time, String)>,
}

impl HeldAll {
    fn of(driver: &Driver) -> HeldAll {
        let outage = driver.outage().filter(driver::Outage::holds);
        HeldAll {
            everything: driver.everything_spans().to_vec(),
            cannot_run: outage.and_then(|outage| {
                let (failure, why) = outage.down();
                (failure == FailureOf::CannotRun).then(|| (outage.since, why.to_string()))
            }),
        }
    }

    /// Who set the pause of everything that held at this time, where one did.
    fn everything_at(&self, at: u64) -> Option<PauserOf> {
        (self.everything.iter())
            .find(|span| span.from.0 <= at && span.until.is_none_or(|until| at < until.0))
            .map(|span| span.by)
    }
}

/// The work's time bar ([`Summary::bar`]): its span from its first round's begin to the end of its last round or
/// tool's call cut into [`BAR_CELLS`] equal cells, each the character of what ran most of it. A round of an agent's
/// cell is a model's; a round of an engine cell, and a tool's call from when it was made to when it was done, are the
/// tools'. A call to a person waits rather than runs. A cell less than half of which ran is the character of what the
/// work waited for at its middle ([`waited`]). What is under way is counted to `now`, by the loop's clock.
fn bar(kept: Kept<'_>, runs: &[Run], all: &HeldAll, now: Time) -> String {
    let now = now.0;
    let agent = |cell: &str| {
        (kept.flow.cells.iter())
            .find(|spec| spec.id.as_str() == cell)
            .is_some_and(|spec| spec.act.is_none())
    };
    // What ran: each round, and each tool's call, from its begin to its end, one under way to now; and whether a
    // model ran it.
    let mut stretches: Vec<(u64, u64, bool)> = Vec::new();
    for run in runs {
        let model = agent(&run.cell);
        for round in &run.rounds {
            stretches.push((round.began, round.ended.unwrap_or(now).max(round.began), model));
        }
        for call in &run.calls {
            if matches!(call.what, Call::Tool { .. }) {
                stretches.push((call.made, call.done.unwrap_or(now).max(call.made), false));
            }
        }
    }
    let rounds = || runs.iter().flat_map(|run| &run.rounds);
    let (Some(from), Some(until)) = (rounds().map(|r| r.began).min(), stretches.iter().map(|s| s.1).max()) else {
        return String::new();
    };
    let span = until.saturating_sub(from);
    if span == 0 {
        return String::new();
    }
    // Times are counted from `from` and multiplied by `BAR_CELLS`, so that cell i is the stretch from `span * i` up to
    // `span * (i + 1)` and every cell is `span` long, with no edge of a cell divided out for each stretch.
    let mut in_cell = vec![(0_u64, 0_u64); BAR_CELLS as usize];
    for (start, stop, model) in stretches {
        let (start, stop) = (start.clamp(from, until), stop.clamp(from, until));
        if stop <= start {
            continue;
        }
        let (s, e) = ((start - from) * BAR_CELLS, (stop - from) * BAR_CELLS);
        let first = (s / span).min(BAR_CELLS - 1);
        let last = (e.saturating_sub(BAR_CELLS) / span).min(BAR_CELLS - 1);
        for i in first..=last {
            let lo = span * i;
            let overlap = e.min(lo + span).saturating_sub(s.max(lo));
            let cell = &mut in_cell[i as usize];
            if model {
                cell.0 += overlap;
            } else {
                cell.1 += overlap;
            }
        }
    }
    // What the work waited for in a cell, by the place of the first entry after the cell's middle: the records as they
    // stood then are those before it; and by whether what holds every work at once held then. The idle cells of one
    // wait share these, and each is decided once.
    // Who set the pause of everything is part of what is decided once, as whether it was the engine's.
    let mut decided: BTreeMap<(Option<Seq>, Option<bool>, bool), char> = BTreeMap::new();
    let mut waited_in = |i: u64| {
        let middle = from + (span * (2 * i + 1)) / (2 * BAR_CELLS);
        let place = (kept.work.moments.iter()).find(|m| m.at.0 > middle).map(|m| m.from);
        let everything = all.everything_at(middle);
        let cannot_run = (all.cannot_run.as_ref()).filter(|(since, _)| since.0 <= middle);
        let key = (place, everything.map(|by| by == PauserOf::Engine), cannot_run.is_some());
        *decided.entry(key).or_insert_with(|| {
            let then = driver::owner::Shared {
                everything,
                cannot_run: cannot_run.map(|(_, why)| why.as_str()),
            };
            match place {
                Some(place) => waited(kept, &kept.work.records.before(place), then),
                None => waited(kept, &kept.work.records, then),
            }
        })
    };
    (in_cell.iter().zip(0..))
        .map(|(&(model, tool), i)| {
            if (model + tool) * 2 < span {
                waited_in(i)
            } else if model >= tool {
                'm'
            } else {
                't'
            }
        })
        .collect()
}

/// What a work on these records waited for, as the character of its bar ([`Summary::bar`]), with what held every work
/// at once then: `o` where it waited on the owner, as the one definition of that says ([`driver::owner`]); `p` where a
/// pause held it, its own or the owner's pause of everything; `w` where it waited for the items its item depends on;
/// `.` anything else, a stalled work among it; by what the deciding code decides on them.
fn waited(kept: Kept<'_>, records: &Records, then: driver::owner::Shared<'_>) -> char {
    let decision = decide::decide(&kept.flow.flow, records);
    let paused = decide::paused(records);
    let begins = matches!(decision, Decision::Run { .. } | Decision::GoOn { .. });
    // The pause of everything holds a work that would begin a round, where its own pause does not.
    let held = (then.everything.is_some() && begins && !paused).then_some(Holder::Everything);
    if !driver::owner::waits(&kept, records, &decision, held, then).is_empty() {
        return 'o';
    }
    match decision {
        Decision::Paused => 'p',
        Decision::Stalled { .. } | Decision::Run { .. } | Decision::GoOn { .. } if paused => 'p',
        _ if held.is_some() => 'p',
        Decision::Wait { on, .. } if on.iter().any(on_items) => 'w',
        _ => '.',
    }
}

/// Whether this is a wait for the items the work's item depends on: a precondition only they make hold, as they merge.
fn on_items(waited: &WaitedOf) -> bool {
    matches!(waited, WaitedOf::Precondition { by, .. }
        if !by.is_empty()
            && by.iter().all(|maker| matches!(maker, MakerOf::Outside { source } if source == driver::DEPENDENCIES)))
}

impl Now {
    /// What the work does now. Of what holds it back, its own pause is said first, then the owner's pause of
    /// everything, then the wait every work shares, then a slot. A round under way whose act waits for the project's
    /// turn, as the act says, waits for it.
    pub(crate) fn of(
        kept: Kept<'_>,
        decision: &Decision,
        held: Option<Holder>,
        outages: &Outages,
        along: &[(Under, &Along)],
        unmerged: &[driver::Unmerged],
    ) -> Result<Now, Broken> {
        let work = kept.work;
        // Held by the pause of everything, which holds while the driver says it holds the work.
        let everything = || -> Result<Now, Broken> {
            let by = (outages.everything).ok_or_else(|| Broken("the pause of everything holds no work".to_string()))?;
            Ok(Now::Everything { by })
        };
        // The wait that holds the rounds of a cell now: the agents' for an agent's, the host's for an act's.
        let holding = |cell: &CellId| {
            let agent = (kept.flow.cells.iter()).any(|spec| spec.id == *cell && spec.kind == Kind::Agent);
            let outage = if agent { &outages.agents } else { &outages.host };
            outage.as_ref().filter(|o| o.holds)
        };
        let shared = |cell: &CellId| holding(cell).is_some();
        let waits_shared = |cell: &CellId| Now::Waits {
            at: cell.to_string(),
            on: holding(cell)
                .into_iter()
                .map(|outage| Waited::Agents { outage: outage.clone() })
                .collect(),
        };
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
                let (round, since) = (began.len(), time(work, *last)?);
                let said = (along.iter())
                    .find(|(under, _)| *under == Under::Round { run: *run, round })
                    .map(|(_, said)| *said);
                match said {
                    Some(Along::Turn { holder }) => Now::Waits {
                        at: cell.to_string(),
                        on: vec![Waited::Turn {
                            holder: holder.0,
                            since,
                        }],
                    },
                    _ => Now::Round {
                        cell: cell.to_string(),
                        run: run.0,
                        round,
                        since,
                        at: runs_at(said),
                    },
                }
            }
            Decision::Paused if held == Some(Holder::Everything) => everything()?,
            Decision::Paused => Now::paused(work)?,
            // A run at rest that goes on holds its next round while something holds it back: the engine leaves a work
            // so only then, as when the run's last round failed and the work was paused.
            Decision::GoOn { .. } if held == Some(Holder::Pause) => Now::paused(work)?,
            Decision::GoOn { .. } if held == Some(Holder::Everything) => everything()?,
            // A cell that would run or go on, and does not, is held by the wait every work shares, where it holds now
            // and holds the cell's rounds: the engine leaves a work so only then. It is said before a slot.
            Decision::Run { cell, .. } | Decision::GoOn { cell, .. } if shared(cell) => waits_shared(cell),
            Decision::Wait { at, on } if on == &[WaitedOf::Slot] && shared(at) => waits_shared(at),
            Decision::GoOn { cell, .. } if held == Some(Holder::Slot) => Now::Waits {
                at: cell.to_string(),
                on: vec![Waited::Slot {}],
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
                                made: made(work, *call)?,
                                at: runs_at(
                                    (along.iter())
                                        .find(|(under, _)| *under == Under::Call(*call))
                                        .map(|(_, said)| *said),
                                ),
                            },
                            // The work waits for those of the items its item depends on that have not merged.
                            waited if on_items(waited) => Waited::Items {
                                items: unmerged.iter().map(|on| on.item.0).collect(),
                                cancelled: (unmerged.iter())
                                    .filter(|on| on.cancelled)
                                    .map(|on| on.item.0)
                                    .collect(),
                            },
                            WaitedOf::Precondition { on, by } => {
                                let CellKey { test, words } = key_of(&kept, on)?;
                                Waited::Precondition {
                                    test,
                                    words,
                                    by: by.iter().map(Maker::of).collect(),
                                }
                            }
                            WaitedOf::Limit { by, runs } => Waited::Limit {
                                by: by.to_string(),
                                runs: *runs,
                            },
                            WaitedOf::Result { record, by, .. } => Waited::Result {
                                record: record.to_string(),
                                by: by.iter().map(|cell| cell.to_string()).collect(),
                            },
                            WaitedOf::Slot => Waited::Slot {},
                            WaitedOf::Answer { question } => Waited::Answer {
                                question: question.clone(),
                            },
                            WaitedOf::Outside { source, run, block } => outside(work, source, *run, block)?,
                        })
                    })
                    .collect::<Result<_, Broken>>()?,
            },
            Decision::Run { cell, .. } | Decision::GoOn { cell, .. } | Decision::Abandon { cell, .. } => {
                Now::Next { cell: cell.to_string() }
            }
            // A stalled work that its own pause holds is shown paused: it is the pauser's to let on.
            Decision::Stalled { .. } if decide::paused(&work.records) => Now::paused(work)?,
            Decision::Stalled { at, on, by } => {
                let on: Vec<CellKey> = (on.iter())
                    .map(|key| key_of(&kept, key))
                    .collect::<Result<_, Broken>>()?;
                let by: Vec<Stalling> = (by.iter())
                    .map(|stalling| {
                        let why = match &stalling.why {
                            UnrunOf::Skipped { on } => Unrun::Skipped {
                                on: (on.iter())
                                    .map(|key| key_of(&kept, key))
                                    .collect::<Result<_, Broken>>()?,
                            },
                            UnrunOf::NothingNew => Unrun::NothingNew {},
                            UnrunOf::Held => Unrun::Held {},
                        };
                        Ok(Stalling {
                            cell: stalling.cell.to_string(),
                            why,
                        })
                    })
                    .collect::<Result<_, Broken>>()?;
                let why = stalled_words(&kept.flow.waits.stalled, &on, &by);
                Now::Stalled {
                    at: at.to_string(),
                    on,
                    by,
                    why,
                }
            }
        })
    }

    /// The work's pause, with who paused it and why.
    pub(crate) fn paused(work: &Work) -> Result<Now, Broken> {
        let pause = decide::pause(&work.records).ok_or_else(|| Broken("the work has no pause".to_string()))?;
        Ok(Now::Paused {
            by: Pauser::of(pause.by),
            why: pause.why.clone(),
        })
    }
}

/// Why a stalled work rests, in one sentence in these words: what the cell it stands at waits for, `on`, and which cells
/// could make it hold, or that nothing new runs it, the last cell; then why each of the cells, `by`, does not run. The
/// cell it stands at is said beside it by every reader, and the sentence does not name it.
fn stalled_words(words: &flow::agents::StalledWords, on: &[CellKey], by: &[Stalling]) -> String {
    use flow::agents::fill;
    let name = |name: &str| fill(&words.name, &[("name", name)]);
    let keys = |keys: &[CellKey]| {
        keys.iter()
            .map(|key| name(&key.words))
            .collect::<Vec<_>>()
            .join(&words.and)
    };
    let writers = match by.is_empty() {
        true => words.none.clone(),
        false => {
            let cells: Vec<String> = by.iter().map(|stalling| name(&stalling.cell)).collect();
            fill(&words.only, &[("cells", &cells.join(&words.or))])
        }
    };
    let waits = match on.is_empty() {
        false => fill(&words.waits, &[("keys", &keys(on)), ("writers", &writers)]),
        true => fill(&words.last, &[("writers", &writers)]),
    };
    let unrun = (by.iter()).map(|stalling| {
        let why = match &stalling.why {
            Unrun::Skipped { on } => fill(&words.skipped, &[("keys", &keys(on))]),
            Unrun::NothingNew {} => words.nothing_new.clone(),
            Unrun::Held {} => words.held.clone(),
        };
        fill(&words.unrun, &[("cell", &name(&stalling.cell)), ("why", &why)])
    });
    let parts: Vec<String> = std::iter::once(waits).chain(unrun).collect();
    flow::agents::sentence(&parts.join(&words.then))
}

/// A key of a cell's `when` or `requires` as the page shows it: a named test with its words, or a key on a block's value
/// as the cell's file writes it.
fn key_of(kept: &Kept<'_>, key: &Applies) -> Result<CellKey, Broken> {
    Ok(match key {
        Applies::Test(test) => CellKey {
            test: test.to_string(),
            words: (kept.flow.tests.get(test).cloned())
                .ok_or_else(|| Broken(format!("the flow has no test {test}")))?,
        },
        Applies::Is { block, value } => CellKey {
            test: format!("{block} = {value}"),
            words: format!("{block} = {value}"),
        },
    })
}

impl Pauser {
    pub(crate) fn of(by: PauserOf) -> Pauser {
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
    pub(crate) fn with(self, other: Spent) -> Spent {
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

impl Spent {
    /// What this took, counted in a spend beside rounds as no round of it: what keep-warm pings took, in what a work
    /// or a span spent.
    pub(crate) fn as_no_round(self) -> Spent {
        Spent {
            working_ms: 0,
            rounds: 0,
            ..self
        }
    }
}

/// What the keep-warm pings of the work that ended at a time `within` takes took, each counted as a round.
pub(crate) fn pinged(work: &work::Work, within: &dyn Fn(u64) -> bool) -> Spent {
    (work.pinged.iter())
        .filter(|p| work.time_of(p.at).is_some_and(|at| within(at.0)))
        .fold(Spent::default(), |sum, p| {
            sum.with(Spent {
                usd_micros: p.spent.usd_micros,
                input_tokens: p.spent.input_tokens,
                cached_tokens: p.spent.cached_tokens,
                output_tokens: p.spent.output_tokens,
                working_ms: 0,
                rounds: 1,
            })
        })
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
                        said_to: said_to(work, block, version.at),
                        chars: chars(work, block, version.at)?,
                        empty: version.empty,
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
                inputs: (cell.runs_on.iter())
                    .map(|input| Input {
                        block: input.block.to_string(),
                        current: input.current.map(|at| at.0),
                        known: input.consumed.map(|at| at.0),
                        changed: input.new,
                    })
                    .collect(),
            })
            .collect();
        let misses = driver.misses(number);
        let judged = driver.judged(Some(number), now);
        let mut runs = runs(kept)?;
        in_detail(kept, &mut runs, &misses, &judged);
        let mut steps: Vec<Step> = runs.into_iter().map(Step::Run).collect();
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
        for at in &work.records.cancels {
            steps.push(Step::Cancelled {
                at: at.0,
                time: time(work, *at)?,
                why: work.why_cancelled.get(at).cloned(),
            });
        }
        for at in &work.records.reopens {
            steps.push(Step::Reopened {
                at: at.0,
                time: time(work, *at)?,
                why: work.why_reopened.get(at).cloned().unwrap_or_default(),
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
            summary: Summary::of(kept, &view, driver, &Outages::of(driver), now)?,
            title: kept.item.title.clone(),
            level: kept.item.level.clone(),
            project: kept.item.project.clone(),
            tags: kept.item.tags.clone(),
            history: ItemHistory::of(kept.item),
            flow: Flow::of(kept.flow),
            cells,
            blocks,
            steps,
            questions: QuestionPage::all(work),
            speaking: Speaking::all(&kept),
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
            | Step::Reopened { at, .. }
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
            CauseOf::Results { calls } => Cause::Results {
                calls: calls.iter().map(|call| call.0).collect(),
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
            Some(BackOf::HandedIn(handed) | BackOf::Returned { handed, .. }) => (handed.iter())
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
                        cache_creation_input_tokens: r.cache_creation_input_tokens,
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
            Act::Cancel { why } => service::Act::Cancel { why },
            Act::Result { call, result } => service::Act::Result {
                call: CallId(call),
                result,
            },
            Act::Answer {
                question,
                version,
                option,
                why,
            } => service::Act::Answer {
                question,
                version: Seq(version),
                option,
                why,
            },
            Act::NewSession { line } => service::Act::NewSession { line },
            Act::Say { to, words } => service::Act::Say { to: to.taken(), words },
        }
    }
}

impl NewItem {
    /// The item as the driver adds it, at the level it asks for or else at the default of these levels, of the project
    /// it asks for among these; or why it cannot be, in words.
    pub fn taken(self, levels: &Levels, projects: &Projects) -> Result<driver::NewItem, String> {
        levels
            .title_fits(&self.title)
            .map_err(|why| format!("The item's title: {why}"))?;
        let level = levels
            .of_item(self.level.as_deref())
            .map_err(|why| format!("The item's level: {why}"))?;
        let project = projects
            .of_item(&self.project)
            .map_err(|why| format!("The item's project: {why}"))?;
        Ok(driver::NewItem {
            title: self.title,
            level,
            project,
            depends_on: self.depends_on.into_iter().map(ItemNumber).collect(),
            tags: self.tags,
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

impl Engine {
    /// The engine as the driver holds it.
    pub fn of(driver: &Driver) -> Engine {
        let slots = driver.slots();
        Engine {
            slots: slots.slots as u64,
            taken: slots.taken as u64,
            paused: driver.everything_paused().map(|paused| EverythingPaused {
                since: paused.since.0,
                by: Pauser::of(paused.by),
            }),
        }
    }
}

impl EngineAct {
    /// Whether the act sets the pause of everything, rather than lifting it.
    pub fn pauses(self) -> bool {
        matches!(self, EngineAct::Pause {})
    }
}

impl Changed {
    pub fn of(changed: &service::Changed) -> Changed {
        Changed {
            works: changed.works.iter().map(|work| work.0).collect(),
            items: changed.items,
            copies: changed.copies,
        }
    }
}

/// The deciding code's view of the work, which the driver holds.
fn viewed(driver: &Driver, number: WorkNumber) -> Result<View, Broken> {
    driver
        .view(number)
        .ok_or_else(|| Broken(format!("the driver shows work {} and has no view of it", number.0)))
}

/// The link of the CI run a round or a call runs as, where what carries it out says it does.
fn runs_at(said: Option<&Along>) -> Option<String> {
    match said {
        Some(Along::Runs { at }) => Some(at.clone()),
        _ => None,
    }
}

/// When the call was made.
fn made(work: &Work, call: CallId) -> Result<u64, Broken> {
    let at = (work.records.called.iter())
        .find(|c| c.call == call)
        .map(|c| c.at)
        .ok_or_else(|| Broken(format!("call {} was not made", call.0)))?;
    time(work, at)
}

/// When the entry at this place was added. Every entry the engine's loop adds has its time.
pub(crate) fn time(work: &Work, at: Seq) -> Result<u64, Broken> {
    (work.time_of(at).map(|time| time.0)).ok_or_else(|| Broken(format!("the entry at {} has no time", at.0)))
}

/// How long the content of the version of the block written at this place is, in characters.
fn chars(work: &Work, block: &BlockId, at: Seq) -> Result<usize, Broken> {
    (work.contents.get(&(block.clone(), at)))
        .map(|content| content.chars().count())
        .ok_or_else(|| Broken(format!("the version of {block} at {} has no content", at.0)))
}

/// The cell the version of the block written at this place is said to, where it is said to one.
fn said_to(work: &Work, block: &BlockId, at: Seq) -> Option<String> {
    (work.records.written.iter())
        .find(|w| w.at == at && w.block == *block)
        .and_then(|w| w.said_to.as_ref().map(|c| c.0.clone()))
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

/// What a work waits for from a source outside it, as the run that said so gives it: its cell, and what it wrote in the
/// block that keeps the work, in its own words.
fn outside(work: &Work, source: &str, run: RunId, block: &BlockId) -> Result<Waited, Broken> {
    let finished = (work.records.finished.iter()).find(|f| f.run == run);
    let finished = finished.ok_or_else(|| Broken(format!("run {} never finished", run.0)))?;
    let why = (work.contents.get(&(block.clone(), finished.at)))
        .ok_or_else(|| Broken(format!("run {} wrote no {block}", run.0)))?;
    Ok(Waited::Outside {
        source: source.to_string(),
        cell: cell_of(work, run)?,
        why: why.clone(),
    })
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
            AskOf::Tool(work::Tool { name, input }) => Ask::Tool {
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
            UnmetOf::NotRunBy { block } => Unmet::NotRunBy {
                block: block.to_string(),
            },
            UnmetOf::DealtWithTwice { block } => Unmet::DealtWithTwice {
                block: block.to_string(),
            },
            UnmetOf::DealtWithNothing => Unmet::DealtWithNothing,
            UnmetOf::WrittenOver { block, version } => Unmet::WrittenOver {
                block: block.to_string(),
                version: version.0,
            },
            UnmetOf::ChoiceChanged { block } => Unmet::ChoiceChanged {
                block: block.to_string(),
            },
            UnmetOf::AsksAndWorks => Unmet::AsksAndWorks,
            UnmetOf::NotATrigger { name } => Unmet::NotATrigger { name: name.clone() },
            UnmetOf::NotATool { name } => Unmet::NotATool { name: name.clone() },
            UnmetOf::UnchangedNotWritten { block } => Unmet::UnchangedNotWritten {
                block: block.to_string(),
            },
            UnmetOf::EntryUnchanged { block } => Unmet::EntryUnchanged {
                block: block.to_string(),
            },
            UnmetOf::NotOfItsShape { block } => Unmet::NotOfItsShape {
                block: block.to_string(),
            },
            UnmetOf::QuestionTaken { name } => Unmet::QuestionTaken { name: name.clone() },
            UnmetOf::NoSuchQuestion { name } => Unmet::NoSuchQuestion { name: name.clone() },
            UnmetOf::QuestionTwice { name } => Unmet::QuestionTwice { name: name.clone() },
            UnmetOf::NotAnOption { name } => Unmet::NotAnOption { name: name.clone() },
            UnmetOf::NotAnswerable { name } => Unmet::NotAnswerable { name: name.clone() },
            UnmetOf::NotAnswered { name } => Unmet::NotAnswered { name: name.clone() },
            UnmetOf::Unexplained { path } => Unmet::Unexplained { path: path.clone() },
            UnmetOf::NotATest { path } => Unmet::NotATest { path: path.clone() },
            UnmetOf::Unanswered { block } => Unmet::Unanswered {
                block: block.to_string(),
            },
        }
    }
}

/// Gives each round of the runs the session of its line it runs in, by the records' count, each try of it the records
/// hold, with what was filled into its session, how it missed its session's cache, of these misses, and how it went
/// against the lines of its history, as each call of a tool did, as judged: what a work's page shows of a round, and
/// nothing else needs, since working out a round's session counts the line's rounds before it.
fn in_detail(kept: Kept<'_>, runs: &mut [Run], misses: &[driver::cache::Miss], judged: &driver::judged::Judged) {
    let of_work = kept.head.number;
    let work = kept.work;
    let records = &work.records;
    // The line of each run's cell, where the flow puts it on one.
    let line_of = |run: RunId| {
        let cell = records.started.iter().find(|s| s.run == run).map(|s| &s.cell)?;
        let spec = kept.flow.cells.iter().find(|spec| spec.id == *cell)?;
        spec.line.as_deref()
    };
    for r in runs {
        let run = RunId(r.run);
        let line = line_of(run);
        for call in &mut r.calls {
            call.time = judged.calls.get(&(of_work, CallId(call.call))).map(Against::of);
        }
        for round in &mut r.rounds {
            let number = round.number;
            round.judged = (judged.rounds.get(&(of_work, run, number))).map_or_else(Judged::default, Judged::of);
            round.estimated = work::measured::of_round(&work.measured, run, number).is_some_and(|m| m.priced);
            round.missed = (misses.iter())
                .find(|m| m.run == run && m.round == number)
                .map(|m| Missed {
                    read: m.read,
                    held: m.held,
                    differs: m.differs.clone(),
                });
            round.session = line.map(|line| {
                let on_line = |other: RunId| line_of(other) == Some(line);
                RoundSession {
                    line: line.to_string(),
                    number: sessions::of_round(work, line, on_line, run, number).0,
                }
            });
            round.tries = (work.ran.iter())
                .filter(|ran| ran.run == run && ran.round == number)
                .map(|ran| RoundTry {
                    attempt: ran.attempt,
                    ran: RanOn {
                        level: ran.on.level.clone(),
                        model: ran.on.model.clone(),
                        effort: ran.on.effort.clone(),
                    },
                    filled: (work.filled.iter())
                        .filter(|f| f.run == run && f.round == number && f.attempt == ran.attempt)
                        .map(|f| f.fills.iter().map(Fill::of).collect())
                        .collect(),
                })
                .collect();
        }
    }
}

/// Every run of the work, in the order they started, each with its rounds, its calls and how it ended; a round's
/// session and tries are left out, for [`in_detail`] to give where they are shown.
pub(crate) fn runs(kept: Kept<'_>) -> Result<Vec<Run>, Broken> {
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
            // A return asks for calls or does the cell's work, and is shown as one that asked or one that handed in.
            let back = work.back.get(&(run, number)).map(|back| match back {
                BackOf::Asked(asks) => Back::Asked {
                    asks: asks.iter().map(Ask::of).collect(),
                },
                BackOf::Returned { asks, .. } if !asks.is_empty() => Back::Asked {
                    asks: asks.iter().map(Ask::of).collect(),
                },
                BackOf::HandedIn(handed) | BackOf::Returned { handed, .. } => Back::HandedIn {
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
                session: None,
                tries: Vec::new(),
                missed: None,
                estimated: false,
                judged: Judged::default(),
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
                time: None,
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
                            matters: !write.meaning_unchanged,
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

/// The waits every work shares, as they stand: the agents' service's, and the host's the acts reach.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outages {
    pub agents: Option<Outage>,
    pub host: Option<Outage>,
    /// Who set the pause of everything, where it holds; none where it does not.
    pub everything: Option<Pauser>,
}

impl Outages {
    /// The waits as the driver works them out, each saying whether it holds its rounds now.
    pub(crate) fn of(driver: &Driver) -> Outages {
        let gate = driver.gate();
        let (agents, host) = (gate.holds_agents(), gate.holds_acts());
        Outages {
            agents: driver.outage().map(|outage| Outage::of(&outage, agents)),
            host: driver.host_outage().map(|outage| Outage::of(&outage, host)),
            everything: driver.everything_paused().map(|paused| Pauser::of(paused.by)),
        }
    }
}

impl Outage {
    pub(crate) fn of(outage: &driver::Outage, holds: bool) -> Outage {
        Outage {
            service: match outage.service {
                driver::Service::Agents => Service::Agents,
                driver::Service::Host => Service::Host,
            },
            failure: Failure::of(outage.failure),
            why: outage.why.clone(),
            work: outage.work.0,
            run: outage.run.0,
            round: outage.round,
            since: outage.since.0,
            checked: (outage.checked.as_ref()).map(|checked| Checked {
                at: checked.at.0,
                taken: checked.taken,
                found: match &checked.found {
                    driver::Found::Well => Found::Well,
                    driver::Found::Down { failure, why, .. } => Found::Down {
                        failure: Failure::of(*failure),
                        why: why.clone(),
                    },
                },
            }),
            until: match outage.until() {
                UntilOf::Checked { at } => Until::Checked { at: at.0 },
                UntilOf::Well { at } => Until::Well { at: at.0 },
                UntilOf::Looked { at } => Until::Looked { at: at.0 },
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
            FailureOf::Guarded => Failure::Guarded,
            FailureOf::UsageLimit => Failure::UsageLimit,
            FailureOf::CannotRun => Failure::CannotRun,
            FailureOf::Offline => Failure::Offline,
            FailureOf::Unavailable => Failure::Unavailable,
            FailureOf::PromptTooLong => Failure::PromptTooLong,
            FailureOf::NoModel => Failure::NoModel,
            FailureOf::Other => Failure::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    //! What a stretch of a work's bar no round ran in says it waited for, on the records as they stood then, with what
    //! held every work at once then.

    use decide::{Outsider, Pause, Written};
    use flow::{Has, Source, compile};
    use store::{First, Item, ItemNumber, WorkHead, WorkNumber};

    use super::*;

    /// A writer the owner's request runs, and the merge, with the repository's words beside it.
    fn flow() -> flow::Compiled {
        let text = r#"
            name = "small"
            order = ["write", "merge"]
            [limits]
            again = 3
            [lines.main]
            [blocks.request]
            people = true
            [blocks.text]
            [blocks.merged]
            shown = true
        "#;
        let write = "title = \"Write\"\nkind = \"agent\"\neffort = \"high\"\nline = \"main\"\ntask = \"Write.\"\n\
                     inputs = [\"request\"]\noutputs = [\"text\"]\n";
        let merge =
            "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\ninputs = [\"text\"]\noutputs = [\"merged\"]\n";
        let source = Source {
            flow: text.to_string(),
            cells: [("write", write), ("merge", merge)]
                .iter()
                .map(|(id, text)| (id.to_string(), text.to_string()))
                .collect(),
            files: [(
                flow::waits_file(),
                include_str!("../../../flow/agents/waits.toml").to_string(),
            )]
            .into(),
            ..Source::default()
        };
        let has = Has {
            acts: ["merge".to_string()].into(),
            ..Has::default()
        };
        compile(&source, &has).expect("the definition compiles")
    }

    /// What this gives of the work on the flow, kept as the driver keeps it.
    fn of_kept<T>(work: &Work, give: impl FnOnce(Kept<'_>) -> T) -> T {
        let flow = flow();
        let head = WorkHead {
            number: WorkNumber(1),
            item: ItemNumber(1),
            flow: "small".to_string(),
            added: Time(0),
        };
        let first = First {
            title: "Greet".to_string(),
            level: None,
            project: None,
            depends_on: Vec::new(),
            tags: Vec::new(),
        };
        let item = Item::new(ItemNumber(1), Time(0), first, Vec::new());
        let kept = Kept {
            head: &head,
            item: &item,
            flow: &flow,
            work,
        };
        give(kept)
    }

    /// The character the bar gives a work on these records, with what held every work at once then.
    fn waited_on(work: &Work, everything: Option<PauserOf>) -> char {
        let then = driver::owner::Shared {
            everything,
            cannot_run: None,
        };
        of_kept(work, |kept| waited(kept, &work.records, then))
    }

    /// A work the owner's request runs the writer of, paused by this pauser where one is given.
    fn requested(paused: Option<PauserOf>) -> Work {
        let mut work = Work::default();
        work.records.written.push(Written {
            block: "request".into(),
            by: Outsider::Person,
            matters: true,
            value: None,
            empty: false,
            said_to: None,
            at: Seq(1),
        });
        if let Some(by) = paused {
            work.records.pauses.push(Pause {
                paused: true,
                at: Seq(2),
                by,
                why: Some("it spent more than it may".to_string()),
            });
        }
        work
    }

    /// A work whose writer asked the owner a question by a call, as one work's old records hold it, whose return asked
    /// it; with the call, which has no answer yet.
    fn asked_the_owner() -> (Work, CallId) {
        let flow = flow();
        let mut work = Work::default();
        let request = engine::Event::Wrote {
            block: "request".into(),
            by: Outsider::Person,
            content: "Greet the user.".to_string(),
            matters: true,
            value: None,
        };
        engine::handle(&flow, &mut work, Time(1000), request).expect("the write is taken");
        let run = work.records.started[0].run;
        let tool = work::Tool {
            name: "read".to_string(),
            input: String::new(),
        };
        let came = engine::Came::asks(vec![tool]);
        let spent = engine::Spent::default();
        let returned = engine::Event::Returned { run, came, spent };
        engine::handle(&flow, &mut work, Time(2000), returned).expect("the return is taken");
        let question = AskOf::Person {
            who: Who::Owner,
            question: "Which greeting?".to_string(),
        };
        let called = work.records.called.last_mut().expect("the call is made");
        called.on = Some(Who::Owner);
        let call = called.call;
        work.calls.insert(call, CallOf::Asked(question.clone()));
        let Some(BackOf::Returned { asks, .. }) = work.back.get_mut(&(run, 1)) else {
            panic!("the round's return is not kept");
        };
        *asks = vec![question];
        (work, call)
    }

    // A question put to the owner by a call, as one work's old records hold it (decision
    // a-return-asks-no-person-by-a-call), is read in the history as it was recorded: the round's return asked it, and
    // the call on the owner had its answer. Milliseconds alone; no other test reads one from records.
    #[test]
    fn a_question_put_to_the_owner_by_a_call_in_old_records_is_read_as_it_was_recorded() {
        let (mut work, call) = asked_the_owner();
        let answer = "The short one.".to_string();
        let done = engine::Event::Done { call, result: answer };
        engine::handle(&flow(), &mut work, Time(3000), done).expect("the answer is taken");

        let runs = of_kept(&work, runs).expect("the history is read");
        let asked = Ask::Question {
            who: Person::Owner,
            question: "Which greeting?".to_string(),
        };
        assert_eq!(runs[0].rounds[0].back, Some(Back::Asked { asks: vec![asked] }));
        let row = &runs[0].calls[0];
        let what = Call::Question {
            who: Person::Owner,
            question: "Which greeting?".to_string(),
        };
        assert_eq!((&row.what, row.chars), (&what, Some(14)));
        assert!(row.done.is_some(), "{row:?}");
        // The writer is entered again, given the answer.
        assert_eq!(runs[1].rounds[0].given, Given::Results { calls: vec![call.0] });
    }

    // While such a call had no answer, the work waited on the owner, and its bar says so as it did.
    #[test]
    fn a_question_put_to_the_owner_by_a_call_in_old_records_waits_on_the_owner_until_answered() {
        assert_eq!(waited_on(&asked_the_owner().0, None), 'o');
    }

    #[test]
    fn a_pause_the_engine_made_is_the_owners_and_a_stalled_work_is_nobodys() {
        assert_eq!(waited_on(&requested(Some(PauserOf::Engine)), None), 'o');
        assert_eq!(waited_on(&requested(Some(PauserOf::Owner)), None), 'p');
        // Nothing runs a cell of a work nothing was written to: it is stalled, which waits on no one.
        assert_eq!(waited_on(&Work::default(), None), '.');
    }

    #[test]
    fn the_pause_of_every_work_the_engine_set_is_the_owners_and_the_owners_own_is_a_pause() {
        assert_eq!(waited_on(&requested(None), Some(PauserOf::Engine)), 'o');
        assert_eq!(waited_on(&requested(None), Some(PauserOf::Owner)), 'p');
        assert_eq!(waited_on(&requested(None), None), '.');
    }

    #[test]
    fn the_pause_of_everything_holds_the_stretches_it_held_as_the_store_keeps_them() {
        let span = |from: u64, until: Option<u64>, by: PauserOf| driver::Span {
            from: Time(from),
            until: until.map(Time),
            by,
        };
        let all = HeldAll {
            everything: vec![span(10, Some(20), PauserOf::Engine), span(30, None, PauserOf::Owner)],
            cannot_run: None,
        };
        assert_eq!(all.everything_at(5), None);
        assert_eq!(all.everything_at(15), Some(PauserOf::Engine));
        assert_eq!(all.everything_at(20), None);
        assert_eq!(all.everything_at(35), Some(PauserOf::Owner));
    }

    #[test]
    fn a_stalled_work_is_said_in_one_sentence_in_the_words_beside_the_flows_its_names_marked() {
        let flow = flow();
        let words = &flow.waits.stalled;
        let key = |words: &str| CellKey {
            test: words.to_string(),
            words: words.to_string(),
        };
        let stalling = |cell: &str, why: Unrun| Stalling {
            cell: cell.to_string(),
            why,
        };
        assert_eq!(
            stalled_words(words, &[key("checked = yes")], &[stalling("M1", Unrun::NothingNew {})]),
            "It waits for `checked = yes`, which only `M1` can make hold, and `M1` does not run, as nothing new runs it."
        );
        // The cell it stands at, which every reader says beside it, it does not name.
        assert_eq!(
            stalled_words(words, &[], &[]),
            "Nothing new runs this last cell, and no cell can write what runs it."
        );
        let by = [
            stalling(
                "review",
                Unrun::Skipped {
                    on: vec![key("big = yes")],
                },
            ),
            stalling("check", Unrun::Held {}),
        ];
        assert_eq!(
            stalled_words(words, &[key("ready"), key("done")], &by),
            "It waits for `ready` and `done`, which only `review` or `check` can make hold, and `review` does not run, \
             as it is skipped, `big = yes` not holding, and `check` does not run, as the cell the work stands at holds it."
        );
    }
}
