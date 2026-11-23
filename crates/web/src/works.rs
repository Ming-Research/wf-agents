//! The items and the works as the pages read them: their own types, named for what the owner reads, made from what
//! the driver holds, and read back by the command line. Nothing here is decided: what a work does now is the deciding
//! code's own account of it ([`decide::view`]), so that what is shown and what is decided cannot differ (property
//! `records-give-the-same-walk`). `client/src/works-api.ts` gives the same types to the pages, field for field.
//!
//! The pages are given a work's history without the content of what was written: each write says how long its
//! content is, and the content of a version, of what a round handed in and of a call's result is asked for by itself,
//! when the owner opens it.
//!
//! A version of a datum is given and taken by its number among the datum's versions, the first being 1 (decision
//! a-version-is-numbered-within-its-datum); the place of the entry that wrote it, which the engine orders what
//! happened by, is given only to order it among the steps, and never names it.
//!
//! - [`Items`]: every item with its work in brief, for the item graph and the list of works.
//! - [`WorkPage`]: one work, as it is now and step by step from its start.
//! - [`RoundPage`], [`VersionPage`], [`CallPage`]: the content the history leaves out.
//! - [`ToldPage`]: what a round was told and what came back of it, as its carrier kept them.
//! - [`SystemPage`]: the system prompt a line's session began with, as its carrier kept it.
//! - [`DoingPage`], [`Done`]: what the agent of a round did, as its carrier keeps it as it comes.
//! - [`ChangesPage`], [`DiffPage`]: what a work changed, as git in its project's repository says.
//! - [`Act`], [`NewItem`]: what the owner does on the pages.
//! - [`Engine`], [`EngineAct`]: the engine as a whole, its slots, the pause of everything and whether the sessions of
//!   works that wait are kept warm, and what the owner does to it.
//!
//! A work whose records name what it does not hold, which the engine never writes, is not shown as something it is
//! not: making its view gives [`Broken`], saying what is missing.

use context::Author;
use decide::run::{Failure as FailureOf, Limit as LimitOf, Unmet as UnmetOf};
use decide::{
    BlockId, CallId, Cause as CauseOf, CellId, Condition as Applies, Decision, End, Holder, Maker as MakerOf,
    Pauser as PauserOf, Records, RunId, Seq, Unrun as UnrunOf, VersionView, View, Waited as WaitedOf, Who,
};
use driver::{Along, Driver, Kept, Under, Until as UntilOf};
use flow::Kind;
use serde::{Deserialize, Serialize};
use service::{Levels, Projects};
use store::{ItemNumber, WorkNumber};
use work::{
    Ask as AskOf, Back as BackOf, Call as CallOf, Fill as FillOf, Given as GivenOf, Stretch, Time, Went, Why, Work,
    sessions,
};

use crate::view::{Flow, OutlineView};

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
    /// Whether the owner has let the engine move to another account at a usage limit and some other account may yet be
    /// moved to in this wait: then the wait ends when the engine moves as well as when the limit resets.
    #[serde(default)]
    pub may_move: bool,
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
    /// Its flow has no agent's cell: it does no work of its own, and delivers what its item is made of, which the page
    /// shows as a deliverable rather than as a work.
    #[serde(default)]
    pub delivers: bool,
    pub added: u64,
    /// Who paused the work, where it is paused: no round of it begins but those of a run that answers a note, a
    /// person's words to its cell. A round under way goes on, and what it hands in is taken.
    pub paused: Option<Pauser>,
    /// What the work does now.
    pub now: Now,
    /// Each cell of its flow as it stands for this work, in the order of the walk.
    pub cells: Vec<CellNow>,
    /// What all its rounds that ended took, and its keep-warm pings and its forks, which count in what it spent as its
    /// rounds do: `rounds` counts its rounds alone.
    pub spent: Spent,
    /// What the keep-warm pings of its sessions took, each ping counted as a round, none of them under way for any
    /// time the page counts.
    #[serde(default)]
    pub pinged: Spent,
    /// What the forks of its rounds' sessions took, each fork counted as a round, none of them under way for any time
    /// the page counts.
    #[serde(default)]
    pub forked: Spent,
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
    /// How many of its questions put to the owner stand unanswered and hold the work at nothing now, so that the owner
    /// may answer them ahead, as one the merge will wait for: those the item window draws as not holding the work
    /// ([`QuestionPage::ahead`]). A question that holds it counts in `waits` alone.
    #[serde(default)]
    pub ahead: usize,
    /// The level its rounds run at, by the rule that gives a round its level ([`Levels::of_round`]): its item's, or
    /// the default where its item has none; none where that rule refuses it, the item's level being none an item may
    /// be at.
    #[serde(default)]
    pub level: Option<String>,
    /// The level the rounds of the cell it stands at run at, by the same rule: its level raised as far as the cell's
    /// line says, never above the highest an item may be at; none where it stands at no cell, as a work that ended,
    /// at an engine cell, whose acts run on no level, or where the rule refuses its level.
    #[serde(default)]
    pub cell_level: Option<String>,
    /// How many levels the line of the cell it stands at raises that cell above its level, as the line's
    /// `levels_above` says, whatever the highest level an item may be at holds the cell's level to; none where it
    /// stands at no agent's cell.
    #[serde(default)]
    pub raise: Option<u32>,
    /// The pull request the host of its repository checks it through: the latest the host found, as the work keeps
    /// it; of a work that merged before works kept theirs, the one the title of the commit its merge made names. None
    /// where it has none, as on a host with no pull requests.
    #[serde(default)]
    pub pull: Option<Pull>,
    /// What it changed: the files touched and the lines added and removed between where its branch left main and its
    /// latest head, together with what its worktree holds not yet committed while the worktree is there, as git in the
    /// project's repository says ([`Summary::saw`]). None where nothing of it is there to tell, as before its branch is
    /// made, and where git could not say, which the engine's standard error says.
    #[serde(default)]
    pub changed: Option<Diffstat>,
}

/// A pull request on the host of a work's repository: its number, and its address.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pull {
    pub number: u64,
    pub url: String,
}

/// How much a work changed: how many files, and how many lines added and removed in them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diffstat {
    pub files: u64,
    pub added: u64,
    pub removed: u64,
}

/// Each file a work changed, as its summary counts them, in the order of their paths, with the commit where its branch
/// left main and its latest head they are between, and whether what its worktree holds not yet committed was counted,
/// the worktree being there.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangesPage {
    pub base: String,
    pub head: String,
    pub worktree: bool,
    pub files: Vec<FileChange>,
}

/// A file a work changed: its path; the lines added and removed, none of a binary file; and whether its latest head
/// holds a change of it, or only its worktree does.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    pub added: u64,
    pub removed: u64,
    pub binary: bool,
    pub committed: bool,
}

/// The unified diff of one file a work changed, between where its changes are, up to where it was cut, with its whole
/// length in bytes where it was.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffPage {
    pub path: String,
    pub diff: String,
    pub cut: Option<u64>,
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
}

/// What the owner does that ends a wait on them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Ends {
    /// Answers this version of the work's question of this name, by one of its options: the version by its number
    /// among the versions of its cell's questions, none where it has none.
    Answer { question: String, version: Option<u64> },
    /// Lets the run that reached its limit go on, as the result of this call.
    GoOn { call: u64 },
    /// Lets the cell go on past its limit.
    LetGo { cell: String },
    /// Writes the block with this value, which makes the precondition hold; none where the block's values do not make
    /// that plain.
    Write { block: String, value: Option<String> },
    /// Lets the work go on, lifting the engine's pause.
    Resume {},
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
                EndsOf::Answer { question, version } => Ends::Answer { question, version },
                EndsOf::GoOn { call } => Ends::GoOn { call: call.0 },
                EndsOf::LetGo { cell } => Ends::LetGo { cell: cell.to_string() },
                EndsOf::Write { block, value } => Ends::Write {
                    block: block.to_string(),
                    value,
                },
                EndsOf::Resume => Ends::Resume {},
                EndsOf::Outside { what } => Ends::Outside { what },
            },
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
    /// It is paused at this cell, by the owner, or by the engine itself for this reason, as when a round failed: what
    /// it would do next is begin a round there, or, where it is stalled and its own pause holds it, it stands there. An
    /// engine of a build before the cell was said gives none, read as empty.
    Paused {
        #[serde(default)]
        at: String,
        by: Pauser,
        why: Option<String>,
    },
    /// The pause of everything holds it back, and what it would do next is begin a round at this cell; its own pause
    /// does not hold it. `by` is who set it: the owner, or the engine as it started with works going on, which holds
    /// every work until the owner lifts it. An engine of a build before the cell was said gives none, read as empty.
    Everything {
        #[serde(default)]
        at: String,
        by: Pauser,
    },
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
    /// The cell's last runs, this many in a row, were each for what a cell after it wrote, and so would the next be:
    /// the owner lets it go on or cancels the work.
    Limit { runs: u32 },
    /// The cell's last runs, this many in a row, each asked for calls and dealt with nothing, and it would be entered
    /// again for their results: the owner lets it go on or cancels the work.
    Calls { runs: u32 },
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
    /// The items the work's item depends on that have neither merged nor been cancelled, by their numbers: a
    /// precondition only they make hold, as each merges or is cancelled.
    Items { items: Vec<u64> },
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
    /// A question for a person, as a call recorded as one is read.
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
    /// Its condition is not met, and it never finished a run.
    Skipped,
    /// It has no record: it never finished a run.
    NotYet,
    /// It has a record, and a block it reads has changed in meaning since: it runs again when the walk reaches it.
    Again,
    /// Its record knows the current meaning of every block it reads, or its condition is not met now and what it wrote
    /// still counts; of a work that ended, a run of it finished.
    Done,
    /// Of a work that ended, no run of it finished (decision `a-works-past-is-drawn-from-its-outline`).
    NotRun,
}

/// How a cell with no run open stands: where its condition does not hold, done if it finished a run, since what it
/// wrote still counts (decision `conditions-and-preconditions`), and skipped if it never did; where its condition holds,
/// not yet run with no record, to run again where a block it reads has changed in meaning since, and else done.
fn settled(applies: bool, ran: bool, changed: bool) -> CellState {
    match (applies, ran) {
        (false, false) => CellState::Skipped,
        (false, true) => CellState::Done,
        (true, false) => CellState::NotYet,
        (true, true) if changed => CellState::Again,
        (true, true) => CellState::Done,
    }
}

/// What rounds took.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spent {
    /// The cost, in millionths of a dollar.
    pub usd_micros: u64,
    /// The tokens sent fresh, neither read from the cache nor written to it.
    pub input_tokens: u64,
    /// The tokens written to the cache.
    pub cache_written_tokens: u64,
    /// The tokens sent and not read from the cache by rounds, pings and forks whose records do not tell those sent
    /// fresh from those written to it, as they were kept before the written ones were kept apart: neither of the two
    /// above counts them.
    pub unsplit_tokens: u64,
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
    /// The version of its flow it runs on, as its outline has it: what its history, its card and its talk with the
    /// owner are drawn from, a work going on and one that ended alike (decision `a-works-past-is-drawn-from-its-outline`).
    pub outline: OutlineView,
    /// The flow it runs on, as the flow page reads a flow, for what the owner does with a work going on; none for a work
    /// that ended.
    pub flow: Option<Flow>,
    /// Each cell now, with why it would run and what it reads, in the order of the walk; none for a work that ended.
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
    /// A cell of the work's flow, by its id.
    Cell { cell: String },
}

impl SpokenTo {
    fn taken(self) -> engine::spoken::To {
        match self {
            SpokenTo::Line { line } => engine::spoken::To::Line(line),
            SpokenTo::Cell { cell } => engine::spoken::To::Cell(CellId(cell)),
        }
    }
}

impl Speaking {
    /// Where the owner's words would go now, to each line of the work's flow and to each cell that has words of its
    /// own.
    fn all(kept: &Kept) -> Vec<Speaking> {
        let lines = (kept.flow.lines.keys()).map(|line| SpokenTo::Line { line: line.clone() });
        let cells = (kept.flow.cells.iter())
            .filter(|c| engine::spoken::words_of(kept.flow, &c.id).is_some())
            .map(|c| SpokenTo::Cell { cell: c.id.to_string() });
        (lines.chain(cells))
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
    /// The version an answer names: the number of the latest among the versions of its cell's questions; none where it
    /// has none, as a question of a cell with no questions of its own.
    pub version: Option<u64>,
    /// The engine itself asked it, and not the cell's agent.
    pub engine: bool,
    pub waits: bool,
    /// Its latest version is marked `tree`: the option taken goes into the design tree.
    pub tree: bool,
    /// The latest version withdraws it, for this reason; none while it stands.
    pub withdrawn: Option<String>,
    /// What it asks, its options and the one it recommends, counted from one; of a question withdrawn, none.
    pub ask: Option<String>,
    pub options: Vec<WayPage>,
    pub recommends: Option<u64>,
    /// The owner's answer to the latest version.
    pub answer: Option<AnswerPage>,
    /// Why its cell does not run now, said as a stalled work says it, where its condition does not hold; none while it
    /// does. A question of a cell that does not run holds the work at no cell.
    #[serde(default)]
    pub skipped: Option<String>,
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
    /// How many of `questions` stand to be answered and hold the work at nothing that `waits` lists: each not withdrawn,
    /// asking something, of a version an answer names and not answered, with no wait of the key `answer-<name>`. The
    /// item window reads a question so, from the work's page and the work's waits, to draw it as holding the work or not.
    pub(crate) fn ahead(questions: &[QuestionPage], waits: &[Wait]) -> usize {
        let open =
            |q: &&QuestionPage| q.withdrawn.is_none() && q.ask.is_some() && q.version.is_some() && q.answer.is_none();
        let holds = |q: &QuestionPage| (waits.iter()).any(|w| w.key == format!("answer-{}", q.name));
        (questions.iter()).filter(open).filter(|q| !holds(q)).count()
    }

    /// Each question of the work, as it stands, numbered as the version the work is on names each cell's questions.
    fn all(kept: &Kept<'_>) -> Result<Vec<QuestionPage>, Broken> {
        let work = kept.work;
        let skipped = decide::skipped(&kept.flow.flow, &work.records);
        (decide::questions::questions(&work.records).into_iter())
            .map(|(name, q)| {
                let (ask, options, recommends, withdrawn, tree) = match work.question(q.latest) {
                    Some(work::Question::Asked {
                        ask,
                        options,
                        recommends,
                        tree,
                        ..
                    }) => {
                        let options = (options.iter())
                            .map(|o| WayPage {
                                does: o.does.clone(),
                                follows: o.follows.clone(),
                            })
                            .collect();
                        (Some(ask.clone()), options, *recommends, None, *tree == Some(true))
                    }
                    Some(work::Question::Withdrawn { why, .. }) => (None, Vec::new(), None, Some(why.clone()), false),
                    None => (None, Vec::new(), None, None, false),
                };
                // Why its cell does not run says why a question that stands holds nothing: one answered or withdrawn
                // holds nothing and has nothing to be answered, so it says none.
                let skipped = match (skipped.get(&q.latest.cell), q.unanswered()) {
                    (Some(keys), true) => {
                        let on: Vec<CellKey> = (keys.iter())
                            .map(|key| key_of(kept, key))
                            .collect::<Result<_, Broken>>()?;
                        Some(skipped_words(&kept.flow.waits.stalled, &q.latest.cell.to_string(), &on))
                    }
                    _ => None,
                };
                Ok(QuestionPage {
                    name: name.to_string(),
                    cell: q.latest.cell.to_string(),
                    version: kept.question_version(&q.latest.cell, q.latest.at),
                    engine: q.latest.engine,
                    waits: q.latest.waits,
                    tree,
                    withdrawn: withdrawn.or_else(|| q.latest.withdrawn.then(String::new)),
                    ask,
                    options,
                    recommends,
                    answer: q.answer.map(|a| AnswerPage {
                        option: a.option,
                        at: a.at.0,
                        why: a.why.clone(),
                    }),
                    skipped,
                })
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
    /// The block is new to the cell. `was` is the version the cell consumed, by its number; `behind` says every new
    /// version was written by a cell after this one.
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
    /// The block's current version, by its number; none where it has none.
    pub current: Option<u64>,
    /// The version the cell consumed, by its number; none where it consumed none.
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
    /// The place of the entry that wrote it, which orders it among the work's steps and names none of them.
    pub at: u64,
    /// Its number among the versions of its block, the first being 1, which names it.
    pub number: u64,
    pub time: u64,
    pub by: Writer,
    /// The version at which the meaning it has last changed, by its number: itself, or an earlier one; none where the
    /// block has said nothing yet, as of a first version handed in as changing nothing.
    pub meaning: Option<u64>,
    /// The block's value, where it is a choice.
    pub value: Option<String>,
    /// The cell it is said to, where a record kept before a cell's words were its own says one: as the owner's entry of
    /// the one record of what is said was.
    pub said_to: Option<String>,
    /// How long its content is, in characters.
    pub chars: usize,
    /// Whether it holds nothing, as the engine recorded it and a test of content reads it.
    pub empty: bool,
}

/// Who wrote a version of a block, as a round is told it (decision a-version-is-told-by-the-line-that-wrote-it): the
/// owner, by a person's write, by words to a cell or by an answer; the outside, or an act of the engine; or a cell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Writer {
    Owner,
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
    /// The context of the line's session, as its last round ended, was past the limit the engine held it to, in
    /// tokens: of the records kept before no session is begun for its size.
    Limit { context: u64, limit: u64 },
    /// The line's rounds run on another model than this one, which the session was begun on.
    Model { was: String },
    /// What a new session of the line would begin with, its system prompt or its form of return, is not what its
    /// session began with, or cannot be made; with the tokens of context of the session it left, where one was told.
    Changed { context: Option<u64> },
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
            Why::Changed { context } => SessionWhy::Changed { context: *context },
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

/// A block a run started from, at this version, by its number.
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
    /// The measure: millionths of a dollar for a cost, milliseconds for a time, bytes for a size.
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

/// One thing filled into a session, as the flow's files name it, each version by its number.
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
    /// It writes the choice with the value, with which a return holds no other block its cell may write but those the
    /// choice's file names, and holds these besides.
    NothingBut {
        block: String,
        value: String,
        besides: Vec<String>,
    },
    /// What came back could not be read as a return, for this reason.
    Unreadable { why: String },
    /// The block is named as dealt with, and it did not run the round.
    NotRunBy { block: String },
    /// The block is named twice as dealt with.
    DealtWithTwice { block: String },
    /// Something ran the round, and the return deals with none of it and asks for no call.
    DealtWithNothing,
    /// The block got this version, by its number, from another writer since the round was told of the one it writes
    /// over.
    WrittenOver { block: String, version: u64 },
    /// The block is a choice, written as changing nothing in meaning with another value than it has.
    ChoiceChanged { block: String },
    /// The return asks for calls and also does the cell's work.
    AsksAndWorks,
    /// The return names as dealt with a name that is none of what its cell watches.
    NotWatched { name: String },
    /// The return asks for a tool its cell may not ask for.
    NotATool { name: String },
    /// The return names the block as written with its meaning unchanged, and does not write it.
    UnchangedNotWritten { block: String },
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
    /// It deals with its cell's own words, of this name, and writes no answer in them.
    NotAnswered { name: String },
    /// The work's branch changes this file the project protects, and nothing says why.
    Unexplained { path: String },
    /// The run of a line that changes only the project's tests changed this file, which is none of them.
    NotATest { path: String },
    /// A round that asks for no call ended with changes to tracked files that are not committed.
    Uncommitted { changes: String },
    /// A round that asks for no call ended with a merge begun in the worktree and not committed.
    Merging { changes: String },
    /// It hands in the change while the question of this name, marked `tree`, has the owner's answer and the work's
    /// change changes nothing in the design tree.
    TreeUnchanged { name: String },
    /// A line of a block that names entries of the design tree names none the tree on the work's branch holds.
    NotAnEntry { line: String },
    /// It deals with what an act after its cell wrote, and leaves nothing new that runs the act and no question with an
    /// answer to come.
    Unanswered { block: String },
    /// It names as dealt with a dialogue it answers as the maker, and answers none of its points there, changes none of
    /// what the other side watches and asks no question the work waits for.
    WordOwed { name: String, watched: Vec<String> },
    /// It writes a verdict with the value that does not pass, and no version of its dialogue.
    NoPoint { block: String, value: String, name: String },
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
    /// How long the result of a call of a tool is, in bytes, as the engine keeps it and tells an agent it is cut,
    /// against the line of the results of the calls of its tool as it stood when it was made: none while it has no
    /// result. Worked out for a work's page only.
    #[serde(default)]
    pub size: Option<Against>,
}

/// How a run ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RunEnd {
    /// It finished, and wrote these blocks: each is the version of its block written at `at`.
    Finished { at: u64, time: u64, writes: Vec<Handed> },
    /// It was abandoned, and wrote nothing.
    Abandoned { at: u64, time: u64 },
}

/// The content of a version of a block, the version by its number.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionPage {
    pub block: String,
    pub number: u64,
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
        /// The tokens it wrote to the prompt cache, as the program printed them: a try's own, where a round's are in
        /// what it took.
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

/// What the agent of a round did from its start, as its program said it as it went: each thing in the order done,
/// every try in order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoingPage {
    pub doing: Vec<Done>,
}

/// A thing an agent did, with the try of its round it did it in, the first being one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Done {
    pub attempt: usize,
    #[serde(flatten)]
    pub doing: Doing,
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
    fn of(fill: &FillOf, records: &Records) -> Result<Fill, Broken> {
        Ok(match fill {
            FillOf::Part { part, .. } => Fill::Part { part: part.clone() },
            FillOf::Whole { block, version } => Fill::Whole {
                block: block.to_string(),
                version: numbered(records, block, *version)?,
            },
            FillOf::Change { block, version, since } => Fill::Change {
                block: block.to_string(),
                version: numbered(records, block, *version)?,
                since: numbered(records, block, *since)?,
            },
            FillOf::Results(calls) => Fill::Results {
                calls: calls.iter().map(|call| call.0).collect(),
            },
            FillOf::SentBack { round } => Fill::SentBack { round: *round },
        })
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

impl Done {
    pub fn of((attempt, doing): (usize, service::Doing)) -> Done {
        Done {
            attempt,
            doing: Doing::of(doing),
        }
    }
}

impl DoingPage {
    pub fn of(done: Vec<(usize, service::Doing)>) -> DoingPage {
        DoingPage {
            doing: done.into_iter().map(Done::of).collect(),
        }
    }
}

impl ChangesPage {
    pub fn of(changes: service::Changes) -> ChangesPage {
        ChangesPage {
            base: changes.base,
            head: changes.head,
            worktree: changes.worktree,
            files: (changes.files.into_iter())
                .map(|file| FileChange {
                    path: file.path,
                    added: file.added,
                    removed: file.removed,
                    binary: file.binary,
                    committed: file.committed,
                })
                .collect(),
        }
    }
}

impl DiffPage {
    pub fn of(diff: service::Diff) -> DiffPage {
        DiffPage {
            path: diff.path,
            diff: diff.text,
            cut: diff.cut,
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
    /// Gives a call that waits for a person its result, such as that a run at its limit goes on.
    Result { call: u64, result: String },
    /// Answers a version of a question, the one the page shows, by its number among the versions of its cell's
    /// questions, taking an option, counted from one, and nothing else.
    Answer {
        question: String,
        version: u64,
        option: u64,
    },
    /// Says words to a cell of the work's flow, or to a line of it: to the cell the engine chooses, which the work's
    /// page shows (`speaking`), and which the act answers with ([`Said`]).
    Say { to: SpokenTo, words: String },
}

/// A fork of a round's session asked by whoever plans the work (design document `testing`, 27): the round, the run's
/// last that ran where none is named, and the question.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ForkAsked {
    #[serde(default)]
    pub round: Option<usize>,
    pub question: String,
}

/// What a fork answered, as the engine's carrier gives it ([`service::Forked`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkPage {
    /// Its return, as JSON text, where it wrote one; else nothing.
    pub answer: Option<String>,
    /// What its program said at its end.
    pub text: String,
    /// What it cost, in millionths of a dollar.
    pub usd_micros: u64,
    /// What its first call read from the cache, and what the session held as the fork began, in tokens, where each is
    /// known.
    pub read: Option<u64>,
    pub held: Option<u64>,
    /// How many minutes before the cache had let go what the fork begins with, where it had: since the last call that
    /// read it, more than the cache's life had passed.
    pub expired: Option<u64>,
    /// The round asked of, by run and number.
    pub round: (u64, usize),
    /// The later rounds of the session, by run and number, which the fork leaves out, beginning at the end of the round
    /// asked of.
    pub later: Vec<(u64, usize)>,
    /// The file the answer is kept in, on the engine's machine.
    pub kept: String,
    /// The account the fork ran under.
    pub account: String,
    /// Why that is the account in use and not the one the session ran under, whose cache holds the session, where it
    /// is.
    pub instead: Option<ForkInstead>,
}

/// Why a fork ran under the account in use and not the one its session last ran under: that one was removed, or the
/// session ran under none.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ForkInstead {
    Removed { account: String },
    NoAccount,
}

impl ForkPage {
    pub fn of(forked: service::Forked) -> ForkPage {
        ForkPage {
            answer: forked.answer,
            text: forked.text,
            usd_micros: forked.spent.usd_micros,
            read: forked.read,
            held: forked.held,
            expired: forked.expired,
            round: (forked.round.0.0, forked.round.1),
            later: forked.later.into_iter().map(|(run, round)| (run.0, round)).collect(),
            kept: forked.kept,
            account: forked.account,
            instead: forked.instead.map(|instead| match instead {
                service::Instead::Removed(account) => ForkInstead::Removed { account },
                service::Instead::NoAccount => ForkInstead::NoAccount,
            }),
        }
    }
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
    /// Whether the sessions of works that wait are kept warm: the engine's own state, which the owner switches, on
    /// until they switch it off.
    pub keep_warm: bool,
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
    /// Keeps the sessions of works that wait warm, or keeps them so no longer, as `on` says: one act for a switch the
    /// owner sets either way, so that it says where the switch now stands and not which way it was moved.
    KeepWarm { on: bool },
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

/// Something happened: these works grew, an item was added where `items`, the copies of the database changed where
/// `copies`, and the accounts, their usage or their usage limits changed where `accounts`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Changed {
    pub works: Vec<u64>,
    pub items: bool,
    pub copies: bool,
    pub accounts: bool,
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
    /// Every item the driver holds, each with its work, as they stand at this time, its rounds at these levels.
    pub fn of(driver: &Driver, levels: &Levels, now: Time) -> Result<Items, Broken> {
        let mut items = Vec::new();
        let every = EveryWork::of(driver);
        let outage = every.outages.agents.clone();
        for kept in driver.works() {
            let view = viewed_at(driver, kept.head.number, &every.at_once)?;
            items.push(Item {
                number: kept.item.number.0,
                title: kept.item.title.clone(),
                level: kept.item.level.clone(),
                project: kept.item.project.clone(),
                added: kept.item.added.0,
                depends_on: kept.item.depends_on.iter().map(|on| on.0).collect(),
                tags: kept.item.tags.clone(),
                history: ItemHistory::of(kept.item),
                work: Summary::of(kept, &view, driver, &every, levels, now)?,
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
    /// The work in brief, from what the driver holds of it, the deciding code's view of it, what every work reads
    /// alike as the driver works it out, and what the driver says of the rest: what holds back the round it would
    /// begin next, where each round or call under way stands, as what carries it out says, and what it waits on the
    /// owner for; its rounds at these levels; at the time `when` by the loop's clock.
    pub(crate) fn of(
        kept: Kept<'_>,
        view: &View,
        driver: &Driver,
        every: &EveryWork,
        levels: &Levels,
        when: Time,
    ) -> Result<Summary, Broken> {
        let work = kept.work;
        let number = kept.head.number;
        let held = driver.held_at(number, &every.at_once);
        let along = &driver.along_of(number);
        let runs = runs(kept)?;
        let engine = |cell: &CellId| kept.cell(cell).is_some_and(|cell| !cell.agent);
        // The act of an engine cell that waits for the project's turn runs nothing yet.
        let turn = |run: RunId| {
            (along.iter()).any(|(under, said)| {
                matches!(under, Under::Round { run: of, .. } if *of == run) && matches!(said, Along::Turn { .. })
            })
        };
        let now = Now::of(
            kept,
            &view.decision,
            held,
            &every.outages,
            along,
            &driver.items_waited_for(number),
        )?;
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
        // A work that ended stands as its outline has its cells, as its page draws them: why a cell did not run is the
        // walk's reading on a whole flow, which a work on a version no longer installed is not read by.
        let ended = |id: &str| {
            let mut finished = (runs.iter())
                .filter(|run| run.cell == id && matches!(run.end, Some(RunEnd::Finished { .. })))
                .map(|run| run.run);
            CellNow {
                cell: id.to_string(),
                state: if finished.clone().next().is_some() {
                    CellState::Done
                } else {
                    CellState::NotRun
                },
                finished: finished.clone().count(),
                record: finished.next_back(),
                open: None,
            }
        };
        let cells = match &view.decision {
            Decision::Ended(_) => {
                let came_from = |id: &CellId| kept.came_from(id).cloned();
                let outline = OutlineView::of(kept.on.outline(), &kept.flow.card, &work.records, &came_from);
                outline.cells.into_iter().map(|cell| ended(&cell.id)).collect()
            }
            _ => (view.cells.iter())
                .map(|cell| CellNow {
                    cell: cell.cell.to_string(),
                    state: match (&cell.open, cell.applies, cell.record) {
                        (Some(open), ..) if open.under_way && engine(&cell.cell) && turn(open.run) => {
                            CellState::Resting
                        }
                        (Some(open), ..) if open.under_way && engine(&cell.cell) => CellState::Calling,
                        (Some(open), ..) if open.under_way => CellState::Running,
                        _ if calling == Some(cell.cell.as_str()) => CellState::Calling,
                        (Some(open), ..) if failed(work, open.run) => CellState::Failed,
                        (Some(_), ..) => CellState::Resting,
                        (None, applies, record) => settled(applies, record.is_some(), !cell.needs.is_empty()),
                    },
                    finished: (runs.iter())
                        .filter(|run| {
                            run.cell == cell.cell.as_str() && matches!(run.end, Some(RunEnd::Finished { .. }))
                        })
                        .count(),
                    record: cell.record.map(|run| run.0),
                    open: cell.open.as_ref().map(|open| open.run.0),
                })
                .collect(),
        };
        let (level, cell_level, raise) = levels_of(levels, kept.item.level.as_deref(), kept.flow, &now);
        let pull = (work.pulls.last()).map(|pulled| Pull {
            number: pulled.number,
            url: pulled.url.clone(),
        });
        let waits: Vec<Wait> = (driver.waits_on_owner_at(number, view, held, &every.at_once).into_iter())
            .map(Wait::of)
            .collect();
        let ahead = QuestionPage::ahead(&QuestionPage::all(&kept)?, &waits);
        let beside = spent_beside_rounds(work, &|_| true)?;
        Ok(Summary {
            number: kept.head.number.0,
            item: kept.head.item.0,
            flow: kept.head.flow.clone(),
            delivers: delivers(kept),
            added: kept.head.added.0,
            paused: decide::pause(&work.records).map(|pause| Pauser::of(pause.by)),
            waits,
            ahead,
            now,
            cells,
            spent: (runs.iter().fold(Spent::default(), |sum, run| sum.with(run.spent)))
                .with(beside.all().as_no_round()),
            pinged: beside.pinged,
            forked: beside.forked,
            last: work.moments.last().map(|moment| moment.at.0),
            from: (runs.iter().flat_map(|run| &run.rounds)).map(|round| round.began).min(),
            bar: bar(kept, &every.held, when),
            sent_back: (runs.iter().flat_map(|run| &run.rounds))
                .filter(|round| matches!(round.given, Given::SentBack { .. }))
                .count(),
            level,
            cell_level,
            raise,
            pull,
            // What git says of it is worked out off the loop, after this ([`Summary::saw`]).
            changed: None,
        })
    }

    /// The summary with what the repository of its work's project says of the work beside its records: what it
    /// changed, and, where the work keeps no pull request, the one its merge names.
    pub fn saw(&mut self, seen: service::Seen) {
        self.changed = seen.changed.map(|stat| Diffstat {
            files: stat.files,
            added: stat.added,
            removed: stat.removed,
        });
        if self.pull.is_none() {
            self.pull = seen.pull.map(|pull| Pull {
                number: pull.number,
                url: pull.url,
            });
        }
    }
}

/// The work of this summary as the carrier of its project's repository is asked of it for what the summary shows
/// beside the records ([`Summary::saw`]): with what its merge wrote, where it merged and keeps no pull request. None
/// where the driver has no such work.
pub fn asked(driver: &Driver, summary: &Summary) -> Option<service::OfWork> {
    let number = WorkNumber(summary.number);
    let kept = driver.work(number)?;
    let merged = match (&summary.pull, &summary.now) {
        (None, Now::Merged) => merge_wrote(&kept),
        _ => None,
    };
    Some(service::OfWork::of(&kept, merged))
}

/// What the run that merged the work wrote, of a work that merged: its latest finished run, the merge's finish being
/// the last of all, as the engine reads where a work merged (`engine::release`), whatever version the work is on; none
/// where it wrote nothing.
fn merge_wrote(kept: &Kept<'_>) -> Option<String> {
    let records = &kept.work.records;
    let merged = records.finished.iter().max_by_key(|f| f.at)?;
    let write = merged.writes.first()?;
    kept.work.contents.get(&(write.block.clone(), merged.at)).cloned()
}

/// The level a work's rounds run at, the level of the cell it stands at, and how far that cell's line raises it
/// ([`Summary::level`], [`Summary::cell_level`], [`Summary::raise`]), by the one rule that gives a round its level: of an
/// item at this level, its work on this flow doing what `now` says.
fn levels_of(
    levels: &Levels,
    item: Option<&str>,
    flow: &flow::Compiled,
    now: &Now,
) -> (Option<String>, Option<String>, Option<u32>) {
    let at = |above: u32| levels.of_round(item, above).ok();
    let cell = now
        .cell()
        .and_then(|cell| (flow.cells.iter()).find(|spec| spec.id.as_str() == cell));
    let agent = cell.filter(|spec| spec.kind == Kind::Agent);
    let cell_level = agent.and_then(|spec| at(spec.levels_above));
    (at(0), cell_level, agent.map(|spec| spec.levels_above))
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
    fn of(driver: &Driver, at_once: &driver::AtOnce) -> HeldAll {
        HeldAll {
            everything: driver.everything_spans().to_vec(),
            cannot_run: at_once.cannot_run().map(|(since, why)| (since, why.to_string())),
        }
    }
}

/// What a work's bar is drawn from (decision `a-work-keeps-what-its-time-went-to`).
enum TimeOf<'a> {
    /// What it kept as it last ended, where it has not been reopened since.
    Kept(&'a [Stretch]),
    /// For a work the flow installed decides, what it waited for as that flow reads it now, asked at each time.
    Decided(driver::time::Waits<'a>),
    /// Else what ran alone, read from its records, a round a model's where it had a try: what it waited for is not
    /// known.
    Ran(Vec<Stretch>),
}

impl<'a> TimeOf<'a> {
    fn of(kept: Kept<'a>, all: &'a HeldAll, now: Time) -> TimeOf<'a> {
        let work = kept.work;
        let kept_time = (work.kept_time.last()).filter(|k| !work.records.reopens.iter().any(|reopen| *reopen > k.at));
        if let Some(kept_time) = kept_time {
            return TimeOf::Kept(&kept_time.stretches);
        }
        if kept.on.installed() {
            let cannot_run = all.cannot_run.as_ref().map(|(since, why)| (*since, why.as_str()));
            return TimeOf::Decided(driver::time::Waits::new(kept, &all.everything, cannot_run, now));
        }
        let tried = |run: RunId| work.ran.iter().any(|ran| ran.run == run);
        let end = work.moments.last().map_or(now, |moment| moment.at);
        TimeOf::Ran(driver::time::ran(work, &tried, end))
    }

    /// Each stretch that ran: a model's, or the tools'.
    fn ran(&self) -> Vec<&Stretch> {
        let ran = |s: &&Stretch| matches!(s.went, Went::Model | Went::Tools);
        match self {
            TimeOf::Kept(stretches) => stretches.iter().filter(ran).collect(),
            TimeOf::Decided(waits) => waits.ran.iter().collect(),
            TimeOf::Ran(stretches) => stretches.iter().collect(),
        }
    }

    /// The character of what the work waited for at this time, `.` where nothing says, and `?` where it is not known.
    fn waited_at(&mut self, at: Time) -> char {
        let went = match self {
            TimeOf::Kept(stretches) => (stretches.iter())
                .find(|s| !matches!(s.went, Went::Model | Went::Tools) && s.from <= at && at < s.until)
                .map(|s| s.went),
            TimeOf::Decided(waits) => waits.at(at),
            TimeOf::Ran(_) => return '?',
        };
        went.map_or('.', mark)
    }
}

/// The character of the bar for what a stretch of the work's time went to.
fn mark(went: Went) -> char {
    match went {
        Went::Model => 'm',
        Went::Tools => 't',
        Went::Owner => 'o',
        Went::Paused => 'p',
        Went::Items => 'w',
        Went::Other => '.',
    }
}

/// The work's time bar ([`Summary::bar`]): its span from its first round's begin to the end of its last round or
/// tool's call cut into [`BAR_CELLS`] equal cells, each the character of what ran most of it: a model, or the tools. A
/// cell less than half of which ran is the character of what the work waited for at its middle, `.` where nothing
/// says, and `?` where what it waited for is not known ([`TimeOf`]). What is under way is counted to `now`, by the
/// loop's clock.
fn bar(kept: Kept<'_>, all: &HeldAll, now: Time) -> String {
    let mut went = TimeOf::of(kept, all, now);
    let stretches: Vec<(u64, u64, bool)> = (went.ran().into_iter())
        .map(|s| (s.from.0, s.until.0, s.went == Went::Model))
        .collect();
    let from = stretches.iter().map(|s| s.0).min();
    let until = stretches.iter().map(|s| s.1).max();
    let (Some(from), Some(until)) = (from, until) else {
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
    // What the work waited for in a cell: the wait that held at the cell's middle, decided there for a work going on as
    // it was decided at the start of the stretch that holds it for what a work kept as it ended.
    let mut waited_in = |i: u64| went.waited_at(Time(from + (span * (2 * i + 1)) / (2 * BAR_CELLS)));
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

impl Now {
    /// The cell the work stands at, as what it does now names it; none where it names none, as a work that ended or
    /// whose records are at fault.
    pub(crate) fn cell(&self) -> Option<&str> {
        match self {
            Now::Round { cell, .. } | Now::Next { cell } => Some(cell.as_str()),
            Now::Paused { at, .. } | Now::Everything { at, .. } | Now::Waits { at, .. } | Now::Stalled { at, .. } => {
                Some(at.as_str())
            }
            Now::Fault { .. } | Now::Merged | Now::Cancelled => None,
        }
    }

    /// What the work does now. Of what holds it back, its own pause is said first, then the owner's pause of
    /// everything, then the wait every work shares, then a slot. A round under way whose act waits for the project's
    /// turn, as the act says, waits for it.
    pub(crate) fn of(
        kept: Kept<'_>,
        decision: &Decision,
        held: Option<Holder>,
        outages: &Outages,
        along: &[(Under, &Along)],
        waited_for: &[ItemNumber],
    ) -> Result<Now, Broken> {
        let work = kept.work;
        // Held by the pause of everything, which holds while the driver says it holds the work.
        let everything = |at: &CellId| -> Result<Now, Broken> {
            let by = (outages.everything).ok_or_else(|| Broken("the pause of everything holds no work".to_string()))?;
            Ok(Now::Everything { at: at.to_string(), by })
        };
        // The wait that holds the rounds of a cell now: the agents' for an agent's, the host's for an act's.
        let holding = |cell: &CellId| {
            let agent = kept.cell(cell).is_some_and(|cell| cell.agent);
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
            Decision::Paused { cell } if held == Some(Holder::Everything) => everything(cell)?,
            Decision::Paused { cell } => Now::paused(work, cell)?,
            // A run at rest that goes on holds its next round while something holds it back: the engine leaves a work
            // so only then, as when the run's last round failed and the work was paused.
            Decision::GoOn { cell, .. } if held == Some(Holder::Pause) => Now::paused(work, cell)?,
            Decision::GoOn { cell, .. } if held == Some(Holder::Everything) => everything(cell)?,
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
                            // The work waits for those of the items its item depends on that have not ended.
                            waited if driver::attention::on_items(waited) => Waited::Items {
                                items: waited_for.iter().map(|on| on.0).collect(),
                            },
                            WaitedOf::Precondition { on, by } => {
                                let CellKey { test, words } = key_of(&kept, on)?;
                                Waited::Precondition {
                                    test,
                                    words,
                                    by: by.iter().map(Maker::of).collect(),
                                }
                            }
                            WaitedOf::Limit { runs } => Waited::Limit { runs: *runs },
                            WaitedOf::Calls { runs } => Waited::Calls { runs: *runs },
                            WaitedOf::Slot => Waited::Slot {},
                            WaitedOf::Answer { question } => Waited::Answer {
                                question: question.clone(),
                            },
                            WaitedOf::Outside { source, run, block } => outside(work, source, *run, block)?,
                        })
                    })
                    .collect::<Result<_, Broken>>()?,
            },
            Decision::Run { cell, .. }
            | Decision::GoOn { cell, .. }
            | Decision::Hurry { cell, .. }
            | Decision::Abandon { cell, .. } => Now::Next { cell: cell.to_string() },
            // A stalled work that its own pause holds is shown paused: it is the pauser's to let on.
            Decision::Stalled { at, .. } if decide::paused(&work.records) => Now::paused(work, at)?,
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

    /// The work's pause at the cell it would go on at, with who paused it and why.
    pub(crate) fn paused(work: &Work, at: &CellId) -> Result<Now, Broken> {
        let pause = decide::pause(&work.records).ok_or_else(|| Broken("the work has no pause".to_string()))?;
        Ok(Now::Paused {
            at: at.to_string(),
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
    let writers = match by.is_empty() {
        true => words.none.clone(),
        false => {
            let cells: Vec<String> = by.iter().map(|stalling| name_words(words, &stalling.cell)).collect();
            fill(&words.only, &[("cells", &cells.join(&words.or))])
        }
    };
    let waits = match on.is_empty() {
        false => fill(&words.waits, &[("keys", &keys_words(words, on)), ("writers", &writers)]),
        true => fill(&words.last, &[("writers", &writers)]),
    };
    let unrun = (by.iter()).map(|stalling| {
        let why = match &stalling.why {
            Unrun::Skipped { on } => skipped_why(words, on),
            Unrun::NothingNew {} => words.nothing_new.clone(),
            Unrun::Held {} => words.held.clone(),
        };
        unrun_words(words, &stalling.cell, &why)
    });
    let parts: Vec<String> = std::iter::once(waits).chain(unrun).collect();
    flow::agents::sentence(&parts.join(&words.then))
}

/// Why a cell does not run where its condition does not hold, in one sentence in the words of a stalled work: the cell,
/// and the keys of its `when` that keep it from running.
fn skipped_words(words: &flow::agents::StalledWords, cell: &str, on: &[CellKey]) -> String {
    flow::agents::sentence(&unrun_words(words, cell, &skipped_why(words, on)))
}

/// A cell that does not run, and why, in the words of a stalled work.
fn unrun_words(words: &flow::agents::StalledWords, cell: &str, why: &str) -> String {
    flow::agents::fill(&words.unrun, &[("cell", &name_words(words, cell)), ("why", why)])
}

/// That a cell is skipped, its keys not holding, in the words of a stalled work.
fn skipped_why(words: &flow::agents::StalledWords, on: &[CellKey]) -> String {
    flow::agents::fill(&words.skipped, &[("keys", &keys_words(words, on))])
}

/// A key or a cell, in the words of a stalled work.
fn name_words(words: &flow::agents::StalledWords, name: &str) -> String {
    flow::agents::fill(&words.name, &[("name", name)])
}

/// Keys joined as a stalled work says them.
fn keys_words(words: &flow::agents::StalledWords, keys: &[CellKey]) -> String {
    (keys.iter())
        .map(|key| name_words(words, &key.words))
        .collect::<Vec<_>>()
        .join(&words.and)
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
    /// What a round, a keep-warm ping or a fork took as its record says it, counted as one round that was under way
    /// this long. A record that says it wrote more tokens to the cache than it sent and did not read from it is
    /// broken.
    pub(crate) fn of(took: &work::Spent, working_ms: u64) -> Result<Spent, Broken> {
        let (input_tokens, cache_written_tokens, unsplit_tokens) = match took.cache_written_tokens {
            Some(written) => match took.uncached_tokens.checked_sub(written) {
                Some(fresh) => (fresh, written, 0),
                None => {
                    return Err(Broken(format!(
                        "a call took {written} tokens written to the cache of the {} it sent and did not read from it",
                        took.uncached_tokens
                    )));
                }
            },
            None => (0, 0, took.uncached_tokens),
        };
        Ok(Spent {
            usd_micros: took.usd_micros,
            input_tokens,
            cache_written_tokens,
            unsplit_tokens,
            cached_tokens: took.cached_tokens,
            output_tokens: took.output_tokens,
            working_ms,
            rounds: 1,
        })
    }

    pub(crate) fn with(self, other: Spent) -> Spent {
        Spent {
            usd_micros: self.usd_micros + other.usd_micros,
            input_tokens: self.input_tokens + other.input_tokens,
            cache_written_tokens: self.cache_written_tokens + other.cache_written_tokens,
            unsplit_tokens: self.unsplit_tokens + other.unsplit_tokens,
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

/// What the work's calls beside its rounds that ended at a time `within` took, each counted as a round: its keep-warm
/// pings and its forks, apart.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Beside {
    pub pinged: Spent,
    pub forked: Spent,
}

impl Beside {
    /// What all of them took together.
    pub(crate) fn all(&self) -> Spent {
        self.pinged.with(self.forked)
    }
}

/// What each call of the work beside its rounds that ended at a time `within` took, each counted as a round, with the
/// time of its place: the one reading of what a work spent beside its rounds that the pages share
/// ([`work::Work::spent_beside_rounds`]).
pub(crate) fn beside_rounds(
    work: &work::Work,
    within: &dyn Fn(u64) -> bool,
) -> Result<Vec<(u64, work::BesideKind, Spent)>, Broken> {
    (work.spent_beside_rounds().into_iter())
        .filter_map(|call| {
            let at = work.time_of(call.at).map(|at| at.0).filter(|at| within(*at))?;
            Some(Spent::of(&call.spent, 0).map(|one| (at, call.kind, one)))
        })
        .collect()
}

/// What the work's calls beside its rounds that ended at a time `within` took, the keep-warm pings and the forks apart.
pub(crate) fn spent_beside_rounds(work: &work::Work, within: &dyn Fn(u64) -> bool) -> Result<Beside, Broken> {
    Ok(beside_rounds(work, within)?
        .into_iter()
        .fold(Beside::default(), |sum, (_, kind, one)| match kind {
            work::BesideKind::Ping => Beside {
                pinged: sum.pinged.with(one),
                ..sum
            },
            work::BesideKind::Fork => Beside {
                forked: sum.forked.with(one),
                ..sum
            },
        }))
}

impl WorkPage {
    /// The work of this number, as the driver holds it now, its rounds at these levels; none where the driver has no
    /// such work.
    pub fn of(driver: &Driver, levels: &Levels, number: WorkNumber, now: Time) -> Result<Option<WorkPage>, Broken> {
        let Some(kept) = driver.work(number) else {
            return Ok(None);
        };
        let every = EveryWork::of(driver);
        let view = viewed_at(driver, number, &every.at_once)?;
        let work = kept.work;
        let records = &work.records;
        let mut blocks = Vec::new();
        for (block, versions) in &view.blocks {
            let versions = (versions.iter())
                .map(|version| {
                    Ok(Version {
                        at: version.at.0,
                        number: numbered(records, block, version.at)?,
                        time: time(work, version.at)?,
                        by: writer_of(kept, block, version)?,
                        meaning: (version.meaning).map(|m| numbered(records, block, m)).transpose()?,
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
            .map(|cell| {
                Ok(CellDetail {
                    cell: cell.cell.to_string(),
                    needs: (cell.needs.iter())
                        .map(|cause| Cause::of(cause, records))
                        .collect::<Result<_, Broken>>()?,
                    inputs: (cell.runs_on.iter())
                        .map(|input| {
                            let number = |at: Option<Seq>| at.map(|at| numbered(records, &input.block, at)).transpose();
                            Ok(Input {
                                block: input.block.to_string(),
                                current: number(input.current)?,
                                known: number(input.consumed)?,
                                changed: input.new,
                            })
                        })
                        .collect::<Result<_, Broken>>()?,
                })
            })
            .collect::<Result<_, Broken>>()?;
        let misses = driver.misses(number);
        let judged = driver.judged(Some(number), now);
        let mut runs = runs(kept)?;
        in_detail(kept, &mut runs, &misses, &judged)?;
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
                by: writer_of(kept, &written.block, version)?,
                changed: version.meaning == Some(version.at),
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
        // A work that ended is drawn from its outline alone, which no later flow changes; and no cell of it takes the
        // owner's words.
        let ended = matches!(view.decision, Decision::Ended(_));
        Ok(Some(WorkPage {
            summary: Summary::of(kept, &view, driver, &every, levels, now)?,
            title: kept.item.title.clone(),
            level: kept.item.level.clone(),
            project: kept.item.project.clone(),
            tags: kept.item.tags.clone(),
            history: ItemHistory::of(kept.item),
            outline: OutlineView::of(kept.on.outline(), &kept.flow.card, records, &|id| {
                kept.came_from(id).cloned()
            }),
            flow: (!ended).then(|| Flow::of(kept.flow)),
            cells: if ended { Vec::new() } else { cells },
            blocks,
            steps,
            questions: QuestionPage::all(&kept)?,
            speaking: if ended { Vec::new() } else { Speaking::all(&kept) },
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
    fn of(cause: &CauseOf, records: &Records) -> Result<Cause, Broken> {
        Ok(match cause {
            CauseOf::NoRecord => Cause::NoRecord,
            CauseOf::Changed { block, was, behind } => Cause::Changed {
                block: block.to_string(),
                was: was.map(|at| numbered(records, block, at)).transpose()?,
                behind: *behind,
            },
            CauseOf::Results { calls } => Cause::Results {
                calls: calls.iter().map(|call| call.0).collect(),
            },
        })
    }
}

impl VersionPage {
    /// The content of the block's version of this number; none where the work has no such version.
    pub fn of(driver: &Driver, work: WorkNumber, block: &str, number: u64) -> Option<VersionPage> {
        let work = driver.work(work)?.work;
        let block = BlockId::from(block);
        let at = work.records.place([&block], number)?;
        let content = work.contents.get(&(block.clone(), at))?;
        Some(VersionPage {
            block: block.to_string(),
            number,
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
            } => service::Act::Answer {
                question,
                version,
                option,
            },
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
            keep_warm: driver.keeping_warm(),
        }
    }
}

impl Changed {
    pub fn of(changed: &service::Changed) -> Changed {
        Changed {
            works: changed.works.iter().map(|work| work.0).collect(),
            items: changed.items,
            copies: changed.copies,
            accounts: changed.accounts,
        }
    }
}

/// The number of the block's version written at this place, by which the pages give and take it: its place among the
/// block's versions, counted from one ([`Records::number`]).
fn numbered(records: &Records, block: &BlockId, at: Seq) -> Result<u64, Broken> {
    (records.number([block], at)).ok_or_else(|| Broken(format!("the work has no version of {block} at {}", at.0)))
}

/// The deciding code's view of the work, which the driver holds, in the room what holds every work at once leaves it.
pub(crate) fn viewed_at(driver: &Driver, number: WorkNumber, at_once: &driver::AtOnce) -> Result<View, Broken> {
    (driver.view_at(number, at_once))
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

/// Who wrote the version of the block of the work, by the rule a round is told it by, each block written from outside
/// the work coming from where the version the work is on says.
fn writer_of(kept: Kept<'_>, block: &BlockId, version: &VersionView) -> Result<Writer, Broken> {
    let from = |block: &BlockId| kept.from(block).map(str::to_string);
    Ok(match context::author_from(&from, &kept.work.records, block, version) {
        Author::Owner => Writer::Owner,
        Author::Nobody => Writer::Outside,
        Author::Run(run) => Writer::Run {
            run: run.0,
            cell: cell_of(kept.work, run)?,
        },
    })
}

/// Whether the work's flow only delivers, no cell of it an agent's, as the version the work is on has it. Of a work on
/// none kept, whether its flow only delivers is not known, and it is taken as a work of its own, whose row is drawn:
/// taken for a deliverable, its row would be hidden.
fn delivers(kept: Kept<'_>) -> bool {
    (kept.on.outline()).is_some_and(|outline| !outline.cells.iter().any(|c| c.agent))
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
    fn of(unmet: &UnmetOf, records: &Records) -> Result<Unmet, Broken> {
        Ok(match unmet {
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
            UnmetOf::NothingBut { block, value, besides } => Unmet::NothingBut {
                block: block.to_string(),
                value: value.clone(),
                besides: besides.iter().map(BlockId::to_string).collect(),
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
                version: numbered(records, block, *version)?,
            },
            UnmetOf::ChoiceChanged { block } => Unmet::ChoiceChanged {
                block: block.to_string(),
            },
            UnmetOf::AsksAndWorks => Unmet::AsksAndWorks,
            UnmetOf::NotWatched { name } => Unmet::NotWatched { name: name.clone() },
            UnmetOf::NotATool { name } => Unmet::NotATool { name: name.clone() },
            UnmetOf::UnchangedNotWritten { block } => Unmet::UnchangedNotWritten {
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
            UnmetOf::Uncommitted { changes } => Unmet::Uncommitted {
                changes: changes.clone(),
            },
            UnmetOf::Merging { changes } => Unmet::Merging {
                changes: changes.clone(),
            },
            UnmetOf::TreeUnchanged { name } => Unmet::TreeUnchanged { name: name.clone() },
            UnmetOf::NotAnEntry { line } => Unmet::NotAnEntry { line: line.clone() },
            UnmetOf::Unanswered { block } => Unmet::Unanswered {
                block: block.to_string(),
            },
            UnmetOf::WordOwed { name, watched } => Unmet::WordOwed {
                name: name.to_string(),
                watched: watched.iter().map(BlockId::to_string).collect(),
            },
            UnmetOf::NoPoint { block, value, name } => Unmet::NoPoint {
                block: block.to_string(),
                value: value.clone(),
                name: name.to_string(),
            },
        })
    }
}

/// Gives each round of the runs the session of its line it runs in, by the records' count, each try of it the records
/// hold, with what was filled into its session, how it missed its session's cache, of these misses, and how it went
/// against the lines of its history, as each call of a tool did, as judged: what a work's page shows of a round, and
/// nothing else needs, since working out a round's session counts the line's rounds before it.
fn in_detail(
    kept: Kept<'_>,
    runs: &mut [Run],
    misses: &[driver::cache::Miss],
    judged: &driver::judged::Judged,
) -> Result<(), Broken> {
    let of_work = kept.head.number;
    let work = kept.work;
    let records = &work.records;
    // The line of each run's cell, where the version the work is on puts it on one.
    let line_of = |run: RunId| {
        let cell = records.started.iter().find(|s| s.run == run).map(|s| &s.cell)?;
        kept.cell(cell)?.line.as_deref()
    };
    for r in runs {
        let run = RunId(r.run);
        let line = line_of(run);
        for call in &mut r.calls {
            let against = judged.calls.get(&(of_work, CallId(call.call)));
            call.time = against.and_then(|against| against.time.as_ref()).map(Against::of);
            call.size = against.and_then(|against| against.size.as_ref()).map(Against::of);
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
                .map(|ran| {
                    Ok(RoundTry {
                        attempt: ran.attempt,
                        ran: RanOn {
                            level: ran.on.level.clone(),
                            model: ran.on.model.clone(),
                            effort: ran.on.effort.clone(),
                        },
                        filled: (work.filled.iter())
                            .filter(|f| f.run == run && f.round == number && f.attempt == ran.attempt)
                            .map(|f| {
                                (f.fills.iter())
                                    .map(|fill| Fill::of(fill, records))
                                    .collect::<Result<Vec<_>, _>>()
                            })
                            .collect::<Result<_, Broken>>()?,
                    })
                })
                .collect::<Result<_, Broken>>()?;
        }
    }
    Ok(())
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
                    unmet: (unmet.iter())
                        .map(|unmet| Unmet::of(unmet, records))
                        .collect::<Result<_, Broken>>()?,
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
                    Some(Spent::of(spent, ended.saturating_sub(began))?)
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
                size: None,
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
            because: (started.because.iter())
                .map(|cause| Cause::of(cause, records))
                .collect::<Result<_, Broken>>()?,
            basis: (started.basis.iter())
                .map(|(block, version)| {
                    Ok(Basis {
                        block: block.to_string(),
                        version: version.map(|at| numbered(records, block, at)).transpose()?,
                    })
                })
                .collect::<Result<_, Broken>>()?,
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
    pub(crate) fn of(at_once: &driver::AtOnce) -> Outages {
        let gate = at_once.gate();
        let (agents, host) = (gate.holds_agents(), gate.holds_acts());
        Outages {
            agents: at_once.agents.as_ref().map(|outage| Outage::of(outage, agents)),
            host: at_once.host.as_ref().map(|outage| Outage::of(outage, host)),
            everything: at_once.everything.map(|paused| Pauser::of(paused.by)),
        }
    }
}

/// What every work's summary reads alike, worked out once for a read of many works or of one: what holds every work
/// at once as the driver gives it ([`driver::AtOnce`]), and as the pages show it and the bar reads it.
pub(crate) struct EveryWork {
    pub(crate) at_once: driver::AtOnce,
    pub(crate) outages: Outages,
    held: HeldAll,
}

impl EveryWork {
    pub(crate) fn of(driver: &Driver) -> EveryWork {
        let at_once = driver.at_once();
        EveryWork {
            outages: Outages::of(&at_once),
            held: HeldAll::of(driver, &at_once),
            at_once,
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
            may_move: outage.may_move,
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
pub(crate) mod tests {
    //! A work's levels, the words of a stalled work, who each version is named by, a call recorded as a question, and
    //! what a work is read by of the version of its flow it is on.

    use std::collections::BTreeMap;
    use std::sync::Arc;

    use decide::{Outsider, Written};
    use flow::{Has, Source, compile};

    use super::*;

    /// A question is one to answer ahead while it stands unanswered and no wait of the work names it: not once it is
    /// answered or withdrawn, and not while the work waits for it, where the waits alone count it.
    #[test]
    fn a_question_is_answered_ahead_while_it_stands_and_holds_nothing() {
        let question = |name: &str| QuestionPage {
            name: name.to_string(),
            cell: "merge".to_string(),
            version: Some(1),
            engine: false,
            waits: false,
            tree: false,
            withdrawn: None,
            ask: Some("Let it merge?".to_string()),
            options: Vec::new(),
            recommends: None,
            answer: None,
            skipped: None,
        };
        let answered = QuestionPage {
            answer: Some(AnswerPage {
                option: 1,
                at: 0,
                why: None,
            }),
            ..question("answered")
        };
        let withdrawn = QuestionPage {
            withdrawn: Some("Not needed.".to_string()),
            ask: None,
            ..question("withdrawn")
        };
        let questions = [question("ahead"), question("held"), answered, withdrawn];
        let held = Wait {
            key: "answer-held".to_string(),
            why: "Let it merge?".to_string(),
            ends: Ends::Answer {
                question: "held".to_string(),
                version: Some(1),
            },
        };
        assert_eq!(QuestionPage::ahead(&questions, &[held]), 1);
        assert_eq!(QuestionPage::ahead(&questions, &[]), 2);
    }

    /// A call recorded as a question for a person, which the engine no longer makes, is read as it was recorded.
    #[test]
    fn a_call_recorded_as_a_question_for_a_person_is_read_as_one() {
        let mut work = Work::default();
        work.calls.insert(
            CallId(47),
            CallOf::Asked(AskOf::Person {
                who: Who::Owner,
                question: "Which name?".to_string(),
            }),
        );
        let Ok(Call::Question { who, question }) = call_of(&work, CallId(47)) else {
            panic!("the call is read as a question");
        };
        assert!(matches!(who, Person::Owner));
        assert_eq!(question, "Which name?");
        assert!(call_of(&work, CallId(48)).is_err());
    }

    /// A writer the owner's request runs, and the merge, with the repository's words beside it.
    pub(crate) fn flow() -> flow::Compiled {
        let text = r#"
            name = "small"
            order = ["write", "merge"]
            [limits]
            runs = 3
            returns = 2
            calls = 3
            [lines.main]
            [blocks.request]
            people = true
            [blocks.text]
            [blocks.merged]
            shown = true
        "#;
        let write = "title = \"Write\"\nkind = \"agent\"\neffort = \"high\"\nline = \"main\"\ntask = \"Write.\"\n\
                     watches = [\"request\"]\noutputs = [\"text\"]\n[kinds]\nrequest = \"last_and_latest\"\n";
        let merge =
            "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\nwatches = [\"text\"]\noutputs = [\"merged\"]\n";
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

    /// The item of one work and the work's head, on the flow of this name, as the store keeps them, for a [`Kept`] to
    /// borrow.
    pub(crate) struct Stored {
        head: store::WorkHead,
        item: store::Item,
    }

    impl Stored {
        pub(crate) fn new(flow: &str) -> Stored {
            let first = store::First {
                title: "A work".to_string(),
                level: None,
                project: None,
                depends_on: Vec::new(),
                tags: Vec::new(),
            };
            Stored {
                head: store::WorkHead {
                    number: WorkNumber(1),
                    item: ItemNumber(1),
                    flow: flow.to_string(),
                    added: Time(0),
                },
                item: store::Item::new(ItemNumber(1), Time(0), first, Vec::new()),
            }
        }

        /// The work with these records, on this version, the flow installed being this one.
        pub(crate) fn kept<'a>(&'a self, flow: &'a flow::Compiled, on: &'a driver::On, work: &'a Work) -> Kept<'a> {
            Kept {
                head: &self.head,
                item: &self.item,
                flow,
                on,
                passed: &[],
                work,
            }
        }
    }

    /// The outline of an earlier version of the flow, on which the work ended cancelled.
    pub(crate) fn earlier(outline: flow::Outline) -> driver::On {
        driver::On::Earlier {
            version: "earlier".to_string(),
            outline: Arc::new(outline),
            end: End::Cancelled,
            reading: Default::default(),
        }
    }

    // A work's level and its cell's are what the rule that gives a round its level says (the route's test shows a line
    // that raises it): where the rule refuses the item's level, neither is said, and only an agent's cell has a level.
    // How far the cell's line raises it is the line's, as an agent's cell stands, whatever the rule says.
    #[test]
    fn a_works_level_and_its_cells_are_the_rules_and_none_where_it_refuses_or_no_agent_runs() {
        let flow = flow();
        let level = |name: &str| service::Level {
            name: name.to_string(),
            desc: format!("Work of level {name}"),
        };
        let levels = Levels::new(["1", "2", "3"].map(level), "2", Some("2")).expect("the levels are made");
        let of = |item: Option<&str>, now: &Now| levels_of(&levels, item, &flow, now);
        let at = |cell: &str| Now::Next { cell: cell.to_string() };
        let given = |level: &str| Some(level.to_string());
        assert_eq!(of(Some("1"), &at("write")), (given("1"), given("1"), Some(0)));
        assert_eq!(
            of(None, &at("write")),
            (given("2"), given("2"), Some(0)),
            "an item of no level is at the default"
        );
        assert_eq!(
            of(Some("1"), &at("merge")),
            (given("1"), None, None),
            "an engine cell's act runs on no level"
        );
        assert_eq!(
            of(Some("1"), &Now::Merged),
            (given("1"), None, None),
            "a work that ended stands at no cell"
        );
        assert_eq!(of(Some("9"), &at("write")), (None, None, Some(0)), "no level");
        assert_eq!(
            of(Some("3"), &at("write")),
            (None, None, Some(0)),
            "above the highest an item may be at"
        );
    }

    // A question of a cell that does not run says why in the words a stalled work says a cell does not run in, so that
    // the same thing reads the same wherever it is said.
    #[test]
    fn a_question_of_a_skipped_cell_says_why_in_the_words_of_a_stalled_work() {
        let flow = flow();
        let words = &flow.waits.stalled;
        let key = |words: &str| CellKey {
            test: words.to_string(),
            words: words.to_string(),
        };
        assert_eq!(
            skipped_words(words, "review", &[key("big = yes"), key("ready")]),
            "`review` does not run, because it is skipped, `big = yes` and `ready` not holding."
        );
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
            "It waits for `checked = yes`, which only `M1` can make hold, and `M1` does not run, because nothing new \
             runs it."
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
             because it is skipped, `big = yes` not holding, and `check` does not run, because the cell where the work \
             stands holds it back."
        );
    }

    /// A writer who may ask the owner and be spoken to, and the merge, which waits for main from outside.
    fn asking() -> flow::Compiled {
        let text = r#"
            name = "asking"
            order = ["write", "merge"]
            [limits]
            runs = 3
            returns = 2
            calls = 3
            [lines.main]
            [blocks.request]
            people = true
            [blocks.main]
            outside = "main"
            [blocks.text]
            [blocks.write_questions]
            outside = "answer"
            [blocks.write_words]
            outside = "words"
            [blocks.merged]
            shown = true
        "#;
        let write = "title = \"Write\"\nkind = \"agent\"\neffort = \"high\"\nline = \"main\"\ntask = \"Write.\"\n\
                     watches = [\"request\", \"write_questions\", \"write_words\"]\n\
                     outputs = [\"text\", \"write_questions\", \"write_words\"]\n[kinds]\n\
                     request = \"last_and_latest\"\nwrite_questions = \"all\"\nwrite_words = \"all\"\n";
        let merge = "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\nwatches = [\"text\", \"main\"]\n\
                     outputs = [\"merged\"]\n";
        compiled(text, &[("write", write), ("merge", merge)])
    }

    /// The flow of this definition and these cells' files, with the repository's words beside it.
    fn compiled(text: &str, cells: &[(&str, &str)]) -> flow::Compiled {
        let source = Source {
            flow: text.to_string(),
            cells: (cells.iter())
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
            sources: ["answer", "words", "main"].iter().map(|s| s.to_string()).collect(),
            acts: ["merge".to_string()].into(),
            ..Has::default()
        };
        compile(&source, &has).expect("the definition compiles")
    }

    /// A design the owner's `deep` turns on, a writer, and the merge: each of the first two may ask the owner and be
    /// spoken to.
    fn skipping() -> flow::Compiled {
        let text = r#"
            name = "skipping"
            order = ["design", "write", "merge"]
            [limits]
            runs = 3
            returns = 2
            calls = 3
            [lines.main]
            [blocks.request]
            people = true
            [blocks.deep]
            values = ["yes", "no"]
            people = true
            [blocks.main]
            outside = "main"
            [blocks.design]
            [blocks.text]
            [blocks.design_questions]
            outside = "answer"
            [blocks.design_words]
            outside = "words"
            [blocks.write_questions]
            outside = "answer"
            [blocks.write_words]
            outside = "words"
            [blocks.merged]
            shown = true
        "#;
        let asks = |title: &str, block: &str, own: &str| {
            format!(
                "title = \"{title}\"\nkind = \"agent\"\neffort = \"high\"\nline = \"main\"\ntask = \"Do it.\"\n\
                 watches = [\"request\", \"{own}_questions\", \"{own}_words\"]\n\
                 outputs = [\"{block}\", \"{own}_questions\", \"{own}_words\"]\n{when}{reads}[kinds]\n\
                 request = \"last_and_latest\"\n{own}_questions = \"all\"\n{own}_words = \"all\"\n{kind}",
                when = if own == "design" {
                    "when = { deep = \"yes\" }\n"
                } else {
                    ""
                },
                reads = if own == "write" { "reads = [\"design\"]\n" } else { "" },
                kind = if own == "write" { "design = \"latest\"\n" } else { "" }
            )
        };
        let (design, write) = (asks("Design", "design", "design"), asks("Write", "text", "write"));
        let merge = "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\nwatches = [\"text\", \"main\"]\n\
                     outputs = [\"merged\"]\n";
        compiled(text, &[("design", &design), ("write", &write), ("merge", merge)])
    }

    // A question of a cell whose condition does not hold says why its cell does not run, in the words a stalled work
    // says it in; one of a cell that runs says nothing, and the cell applying again takes what was said away.
    #[test]
    fn a_question_of_a_skipped_cell_is_drawn_with_why_and_one_of_a_cell_that_runs_with_none() {
        use decide::{Questioned, Started};

        let flow = skipping();
        let mut work = Work::default();
        let r = &mut work.records;
        let wrote = |block: &str, value: Option<&str>, at: u64| Written {
            block: block.into(),
            by: Outsider::Person,
            matters: true,
            value: value.map(str::to_string),
            empty: false,
            said_to: None,
            at: Seq(at),
        };
        r.written.push(wrote("request", None, 1));
        r.written.push(wrote("deep", Some("no"), 2));
        for (run, cell, at) in [(1, "design", 3), (2, "write", 4)] {
            r.started.push(Started {
                run: RunId(run),
                cell: cell.into(),
                at: Seq(at),
                because: Vec::new(),
                basis: BTreeMap::new(),
            });
        }
        let asked = |name: &str, cell: &str, run: u64, at: u64| Questioned {
            name: name.to_string(),
            cell: cell.into(),
            run: RunId(run),
            engine: false,
            waits: true,
            withdrawn: false,
            at: Seq(at),
        };
        r.questions.push(asked("how", "design", 1, 5));
        r.questions.push(asked("which", "write", 2, 6));
        let said = |work: &Work| -> BTreeMap<String, Option<String>> {
            let stored = Stored::new("skipping");
            let on = driver::On::installed_flow(&flow);
            let kept = stored.kept(&flow, &on, work);
            let pages = QuestionPage::all(&kept).expect("the questions are drawn");
            pages.into_iter().map(|q| (q.name, q.skipped)).collect()
        };
        let skipped = said(&work);
        assert_eq!(
            skipped["how"].as_deref(),
            Some("`design` does not run, because it is skipped, `deep = yes` not holding.")
        );
        assert_eq!(skipped["which"], None);
        // Answered, its question says nothing of why its cell does not run.
        let mut answered = work.clone();
        answered.records.answers.push(decide::Answered {
            question: "how".to_string(),
            version: Seq(5),
            option: 1,
            why: None,
            at: Seq(7),
        });
        assert_eq!(said(&answered)["how"], None);
        // The owner turns the design on: its cell applies, and its question says nothing of why it does not run.
        work.records.written.push(wrote("deep", Some("yes"), 8));
        assert_eq!(said(&work)["how"], None);
    }

    // Each version is named by who wrote it as a round is told it (decision a-version-is-told-by-the-line-that-wrote-it),
    // worked out from the records as the engine writes them: what a person writes, the owner's words to a cell and the
    // owner's answer by the owner; a question a cell's run asks by that run, as what it hands in is; the engine's own
    // question, and what the outside writes, put into a round as main is taken in or not, by the outside.
    #[test]
    fn each_version_is_named_by_who_wrote_it_as_a_round_is_told_it() {
        use decide::{Answered, Finished, PutIn, Questioned, Round, Started, Write};

        let flow = asking();
        let mut work = Work::default();
        let r = &mut work.records;
        let wrote = |block: &str, by: Outsider, at: u64| Written {
            block: block.into(),
            by,
            matters: true,
            value: None,
            empty: false,
            said_to: None,
            at: Seq(at),
        };
        let run = RunId(1);
        r.written.push(wrote("request", Outsider::Person, 1));
        r.started.push(Started {
            run,
            cell: "write".into(),
            at: Seq(2),
            because: Vec::new(),
            basis: BTreeMap::new(),
        });
        r.rounds.push(Round { run, at: Seq(3) });
        // The owner speaks to the cell while its round is under way; as main is taken in, main is put into the round.
        r.written.push(wrote("write_words", Outsider::Outside, 4));
        r.written.push(wrote("main", Outsider::Outside, 5));
        r.put_in.push(PutIn {
            run,
            block: "main".into(),
            version: Seq(5),
            at: Seq(6),
        });
        r.finished.push(Finished {
            run,
            at: Seq(7),
            writes: vec![Write {
                block: "text".into(),
                meaning_unchanged: false,
                value: None,
                empty: false,
            }],
            dealt_with: None,
            waits_for: None,
        });
        // The run's return asks a question; the owner answers it; the engine asks one of its own at the round's end.
        let asked = |name: &str, engine: bool, at: u64| Questioned {
            name: name.to_string(),
            cell: "write".into(),
            run,
            engine,
            waits: true,
            withdrawn: false,
            at: Seq(at),
        };
        r.questions.push(asked("which", false, 8));
        r.written.push(wrote("write_questions", Outsider::Outside, 9));
        r.answers.push(Answered {
            question: "which".to_string(),
            version: Seq(8),
            option: 1,
            why: None,
            at: Seq(10),
        });
        r.written.push(wrote("write_questions", Outsider::Outside, 11));
        r.questions.push(asked("protected", true, 12));
        r.written.push(wrote("write_questions", Outsider::Outside, 13));
        let versions = decide::versions(&work.records);
        let stored = Stored::new("asking");
        let on = driver::On::installed_flow(&flow);
        let kept = stored.kept(&flow, &on, &work);
        let named = |block: &str, at: u64| {
            let version = (versions[&BlockId::from(block)].iter())
                .find(|v| v.at == Seq(at))
                .expect("the version is read");
            let writer = writer_of(kept, &BlockId::from(block), version).expect("its writer is named");
            serde_json::to_value(writer).expect("the writer is given as JSON")
        };
        let owner = serde_json::json!({ "kind": "owner" });
        let outside = serde_json::json!({ "kind": "outside" });
        let the_run = serde_json::json!({ "kind": "run", "run": 1, "cell": "write" });
        assert_eq!(named("request", 1), owner);
        assert_eq!(named("write_words", 4), owner);
        assert_eq!(named("main", 5), outside);
        assert_eq!(named("text", 7), the_run);
        assert_eq!(named("write_questions", 9), the_run);
        assert_eq!(named("write_questions", 11), owner);
        assert_eq!(named("write_questions", 13), outside);
    }

    // Acceptance 13: a work on no version kept no stretches as it ended and is decided by no flow, so its bar is drawn
    // from its records alone: a round a model's where it had a try and the tools' where it had none, and what it waited
    // for between them not known (decision a-work-keeps-what-its-time-went-to).
    #[test]
    fn the_bar_of_a_work_on_no_version_is_its_rounds_from_its_records_and_its_waits_not_known() {
        use decide::{Returned, Round, Started};
        use work::Moment;
        use work::sessions::{Ran, RanOn};

        let flow = flow();
        let mut work = Work::default();
        let r = &mut work.records;
        for (run, cell, begun) in [(1, "write", 1), (2, "merge", 3)] {
            r.started.push(Started {
                run: RunId(run),
                cell: cell.into(),
                at: Seq(begun),
                because: Vec::new(),
                basis: BTreeMap::new(),
            });
            r.rounds.push(Round {
                run: RunId(run),
                at: Seq(begun),
            });
            r.returned.push(Returned {
                run: RunId(run),
                at: Seq(begun + 1),
            });
        }
        // The first round ran from 0 to 1 s, the second from 2 s to 3 s, and nothing ran between.
        work.moments = (1..=4)
            .map(|place| Moment {
                from: Seq(place),
                at: Time((place - 1) * 1000),
            })
            .collect();
        work.ran.push(Ran {
            at: Seq(1),
            run: RunId(1),
            round: 1,
            attempt: 1,
            on: RanOn {
                level: "2".to_string(),
                model: "a-model".to_string(),
                effort: None,
                directory: None,
                account: None,
            },
        });
        let stored = Stored::new(&flow.name);
        let on = driver::On::NotKept { end: End::Merged };
        let kept = stored.kept(&flow, &on, &work);
        let all = HeldAll {
            everything: Vec::new(),
            cannot_run: None,
        };
        let third = (BAR_CELLS / 3) as usize;
        let drawn = bar(kept, &all, Time(10_000));
        let expected = "m".repeat(third) + &"?".repeat(third) + &"t".repeat(third);
        assert_eq!(drawn, expected);
    }

    /// A work on an earlier version of the asking writer ([`asking`]) whose writer is on the line `writing`, the flow
    /// installed since being the plain writer ([`flow`]), which has no questions or words of the writer's and puts it
    /// on the line `main`: the writer's round, in which the owner speaks to it, and the question its return asks,
    /// written as the writer's questions.
    fn asked_on_an_earlier_version() -> (flow::Compiled, driver::On, Work) {
        use decide::{Questioned, Returned, Round, Started};
        use work::Moment;

        let mut outline = flow::Outline::of(&asking());
        let write = (outline.cells.iter_mut())
            .find(|c| c.id.as_str() == "write")
            .expect("the writer");
        write.line = Some("writing".to_string());
        let mut work = Work::default();
        let r = &mut work.records;
        let wrote = |block: &str, by: Outsider, at: u64| Written {
            block: block.into(),
            by,
            matters: true,
            value: None,
            empty: false,
            said_to: None,
            at: Seq(at),
        };
        let run = RunId(1);
        r.written.push(wrote("request", Outsider::Person, 1));
        r.started.push(Started {
            run,
            cell: "write".into(),
            at: Seq(2),
            because: Vec::new(),
            basis: BTreeMap::new(),
        });
        r.rounds.push(Round { run, at: Seq(3) });
        r.written.push(wrote("write_words", Outsider::Outside, 4));
        r.returned.push(Returned { run, at: Seq(5) });
        r.questions.push(Questioned {
            name: "which".to_string(),
            cell: "write".into(),
            run,
            engine: false,
            waits: true,
            withdrawn: false,
            at: Seq(6),
        });
        r.written.push(wrote("write_questions", Outsider::Outside, 7));
        work.given.insert((run, 1), GivenOf::Task);
        work.spent.insert((run, 1), work::Spent::default());
        work.moments.push(Moment {
            from: Seq(1),
            at: Time(0),
        });
        (flow(), earlier(outline), work)
    }

    // The three tests below read a work on an earlier version as that version has it, not as the flow installed since,
    // which has none of what each reads, so that a page reading the flow installed fails each. Milliseconds, as each
    // test of this module: what each catches is one reading of the page, which no route test reaches.

    // The question the writer's return asked is numbered as the version names the writer's questions.
    #[test]
    fn a_work_on_an_earlier_version_numbers_its_questions_as_that_version_names_them() {
        let (installed, on, work) = asked_on_an_earlier_version();
        let stored = Stored::new("asking");
        let kept = stored.kept(&installed, &on, &work);
        let numbered: Vec<_> = (QuestionPage::all(&kept).expect("the questions are read").into_iter())
            .map(|q| (q.name, q.version))
            .collect();
        assert_eq!(numbered, [("which".to_string(), Some(1))]);
    }

    // The owner's words to the writer are named as the owner's, and its questions as the run's that asked them, as the
    // version says where each comes from.
    #[test]
    fn a_work_on_an_earlier_version_names_who_wrote_each_version_as_that_version_says_where_it_comes_from() {
        let (installed, on, work) = asked_on_an_earlier_version();
        let stored = Stored::new("asking");
        let kept = stored.kept(&installed, &on, &work);
        let versions = decide::versions(&work.records);
        let named = |block: &str, at: u64| {
            let version = (versions[&BlockId::from(block)].iter())
                .find(|v| v.at == Seq(at))
                .expect("the version is read");
            let writer = writer_of(kept, &BlockId::from(block), version).expect("its writer is named");
            serde_json::to_value(writer).expect("the writer is given as JSON")
        };
        assert_eq!(named("write_words", 4), serde_json::json!({ "kind": "owner" }));
        assert_eq!(
            named("write_questions", 7),
            serde_json::json!({ "kind": "run", "run": 1, "cell": "write" })
        );
    }

    // The writer's round runs in a session of the line the version puts the writer on.
    #[test]
    fn a_work_on_an_earlier_version_puts_its_rounds_on_the_lines_that_version_puts_their_cells_on() {
        let (installed, on, work) = asked_on_an_earlier_version();
        let stored = Stored::new("asking");
        let kept = stored.kept(&installed, &on, &work);
        let mut runs = runs(kept).expect("the runs are read");
        in_detail(kept, &mut runs, &[], &driver::judged::Judged::default()).expect("the rounds are read in detail");
        let lines: Vec<_> = (runs.iter().flat_map(|run| &run.rounds))
            .map(|round| round.session.as_ref().map(|s| s.line.as_str()))
            .collect();
        assert_eq!(lines, [Some("writing")]);
    }

    // Whether a work only delivers is as the version it is on has it, and a work on none kept is taken as one of its
    // own: the flow installed here only delivers, so that a reading of it fails both a work on an earlier version
    // with a writer and one on none kept. Milliseconds.
    #[test]
    fn a_work_delivers_as_the_version_it_is_on_has_it_and_one_on_none_kept_is_its_own() {
        let installed = only_delivers();
        let work = Work::default();
        let stored = Stored::new(&installed.name);
        let of = |on: &driver::On| delivers(stored.kept(&installed, on, &work));
        assert!(of(&driver::On::installed_flow(&installed)));
        assert!(!of(&earlier(flow::Outline::of(&flow()))));
        assert!(!of(&driver::On::NotKept { end: End::Merged }));
    }

    /// The flow of a deliverable item as the repository defines it (`flow/deliver`), whose one cell is the engine's.
    fn only_delivers() -> flow::Compiled {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/deliver");
        let source = flow::read(&dir).expect("the deliver flow is read");
        let has = Has {
            sources: [driver::DEPENDENCIES.to_string()].into(),
            acts: ["deliver".to_string()].into(),
            ..Has::default()
        };
        compile(&source, &has).expect("the deliver flow compiles")
    }

    // What a work spent beside its rounds is told by kind, each call counted as a round, and by when it ended: a ping and
    // a fork that ended at different times. Microseconds.
    #[test]
    fn what_a_work_spent_beside_its_rounds_is_told_by_kind_and_by_when_each_call_ended() {
        let mut work = Work::default();
        for (from, at) in [(1, 1_000), (10, 5_000)] {
            work.moments.push(work::Moment {
                from: decide::Seq(from),
                at: Time(at),
            });
        }
        work.pinged.push(work::Pinged {
            at: decide::Seq(2),
            line: "engineer".to_string(),
            session: 0,
            // Kept before the tokens written to the cache were kept apart: its 5 are sent fresh or written, not told.
            spent: work::Spent {
                usd_micros: 20_000,
                uncached_tokens: 5,
                cache_written_tokens: None,
                cached_tokens: 100,
                output_tokens: 1,
                ..Default::default()
            },
            failed: None,
        });
        work.forked.push(work::Forked {
            at: decide::Seq(11),
            run: decide::RunId(1),
            round: 1,
            usd_micros: 300_000,
            uncached_tokens: 120,
            cache_written_tokens: Some(100),
            cached_tokens: 18_100,
            output_tokens: 30,
        });
        let each =
            |usd_micros, (input_tokens, cache_written_tokens, unsplit_tokens), cached_tokens, output_tokens| Spent {
                usd_micros,
                input_tokens,
                cache_written_tokens,
                unsplit_tokens,
                cached_tokens,
                output_tokens,
                working_ms: 0,
                rounds: 1,
            };
        let all = spent_beside_rounds(&work, &|_| true).expect("the records are sound");
        assert_eq!(
            all.pinged,
            each(20_000, (0, 0, 5), 100, 1),
            "a ping not told apart counts in neither"
        );
        assert_eq!(all.forked, each(300_000, (20, 100, 0), 18_100, 30));
        assert_eq!((all.all().usd_micros, all.all().rounds), (320_000, 2));
        assert_eq!(
            (
                all.all().input_tokens,
                all.all().cache_written_tokens,
                all.all().unsplit_tokens
            ),
            (20, 100, 5),
            "the tokens of the two are told apart as far as the records tell them"
        );
        let early = spent_beside_rounds(&work, &|at| at < 2_000).expect("the records are sound");
        assert_eq!(
            (early.pinged.rounds, early.forked.rounds),
            (1, 0),
            "only the ping had ended"
        );
        let kinds: Vec<work::BesideKind> = beside_rounds(&work, &|_| true)
            .expect("the records are sound")
            .into_iter()
            .map(|(_, kind, _)| kind)
            .collect();
        assert_eq!(kinds, [work::BesideKind::Ping, work::BesideKind::Fork]);
    }

    // The bar of a work going on is the one it keeps as it ends (decision a-work-keeps-what-its-time-went-to): drawn
    // live, each cell less than half of which ran is decided at its middle alone, and drawn from what the work kept,
    // read in the stretch that holds the middle. Seven times the writer's round fails, the owner lets the work on while
    // the agents' wait holds its round back, and pauses everything a while within that wait, each a little longer than
    // the last, so that the waits begin and end within cells. Milliseconds; the one test that the two drawings agree.
    #[test]
    fn the_bar_of_a_work_going_on_is_the_one_it_keeps_as_it_ends() {
        use decide::run::Failure;
        use engine::{Effect, Event, Gate};

        let flow = flow();
        let mut work = Work::default();
        let wrote = Event::Wrote {
            block: "request".into(),
            by: Outsider::Person,
            content: "Greet.".to_string(),
            matters: true,
            value: None,
        };
        let round = |effects: &[Effect]| {
            effects.iter().find_map(|effect| match effect {
                Effect::Round { run, .. } => Some(*run),
                _ => None,
            })
        };
        let effects = engine::handle(&flow, &mut work, Time(0), wrote).expect("the request is taken");
        let mut run = round(&effects).expect("the writer's round begins");
        let mut everything = Vec::new();
        let mut t = 0;
        for i in 0..7_u64 {
            t += 700 + 130 * i;
            let failed = Event::Failed {
                run,
                why: "it broke".to_string(),
                failure: Failure::Other,
                resets: None,
                spent: work::Spent::default(),
            };
            engine::handle(&flow, &mut work, Time(t), failed).expect("the round fails");
            t += 400 + 90 * i;
            let free = decide::Room::FREE;
            engine::handle_held(&flow, &mut work, Time(t), Event::Resumed, free, Gate::Agents).expect("it is let on");
            everything.push(driver::Span {
                from: Time(t + 150),
                until: Some(Time(t + 350 + 50 * i)),
                by: decide::Pauser::Owner,
            });
            t += 600 + 70 * i;
            let effects =
                engine::handle_held(&flow, &mut work, Time(t), Event::Look, free, Gate::Open).expect("it is looked at");
            run = round(&effects).expect("the round begins again");
        }
        let now = Time(t + 500);
        let all = HeldAll {
            everything,
            cannot_run: None,
        };
        let stored = Stored::new(&flow.name);
        let on = driver::On::installed_flow(&flow);
        let live = bar(stored.kept(&flow, &on, &work), &all, now);
        let stretches = driver::time::stretches(&stored.kept(&flow, &on, &work), &all.everything, None, now);
        let mut ended = work.clone();
        ended.kept_time.push(work::KeptTime {
            at: work.tick(),
            stretches,
        });
        assert_eq!(live, bar(stored.kept(&flow, &on, &ended), &all, now));
        for went in ['m', 'o', 'p', '.'] {
            assert!(live.contains(went), "{live}");
        }
    }

    // A new session begun because what a new one would begin with changed is given to the page with the tokens of
    // context of the session it left, and as null where none was told, never as nothing. Milliseconds; the one test of
    // how this reason reaches the page.
    #[test]
    fn a_session_begun_for_a_changed_prompt_or_form_gives_the_context_it_left_or_null() {
        let given = |context| serde_json::to_value(SessionWhy::of(&Why::Changed { context })).expect("it is JSON");
        assert_eq!(
            given(Some(86_953)),
            serde_json::json!({ "kind": "changed", "context": 86_953 })
        );
        assert_eq!(given(None), serde_json::json!({ "kind": "changed", "context": null }));
    }

    // A cell its condition passes over stands as its record does: done where it finished a run, as R2 does once it has
    // run, what it wrote still counting; skipped only where it never ran (decision conditions-and-preconditions). It
    // does not run again for a block it reads that changed, since the walk passes it.
    #[test]
    fn a_cell_passed_over_is_done_where_it_ran_and_skipped_where_it_never_did() {
        assert_eq!(settled(false, true, false), CellState::Done);
        assert_eq!(settled(false, true, true), CellState::Done);
        assert_eq!(settled(false, false, false), CellState::Skipped);
        assert_eq!(settled(true, false, false), CellState::NotYet);
        assert_eq!(settled(true, true, true), CellState::Again);
        assert_eq!(settled(true, true, false), CellState::Done);
    }
}
