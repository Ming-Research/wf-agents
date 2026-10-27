//! What the interface hands out. Times are UTC milliseconds since the Unix epoch, as in the database; JSON the
//! database keeps whole (a round's input and return, a design card's body) is handed out as it is kept. A round is
//! given by its work and its number in the work, as the database keeps it, and named `R<work>-<n>`.

use std::collections::BTreeMap;

use flow::format::Src;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

/// The round of a work the owner asked for, and what it came to.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct StepTaken {
    /// When the round ended.
    pub at: i64,
    /// The work the owner asked a round of, and the work the round was of: the new work that took the asked work's
    /// item over, when the asked work was on an older flow version.
    pub asked: i64,
    pub work: i64,
    /// The rounds of the work it started or ended, by their numbers in the work, and the cell of the last of them.
    pub rounds: Vec<i64>,
    pub cell: Option<String>,
}

/// What a work's next round would be, as the engine would take it now, with nothing run or recorded.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct NextRound {
    pub work: i64,
    pub cell: String,
    pub title: String,
    /// agent, engine, tool or owner: an agent's round, a step of the engine's own, a run of tools, or an ask of the
    /// owner.
    pub kind: String,
    /// For an agent's round: the line it runs on, and the message the agent would be sent.
    pub line: Option<String>,
    pub message: Option<String>,
    /// For a step of the engine's own: which.
    pub step: Option<String>,
    /// For an agent's cell whose need the project does not answer: why no agent is asked; the engine answers the cell
    /// itself, and there is no message.
    pub not_asked: Option<String>,
    /// For a tool cell: what each tool the last round asked for will do, one line each.
    pub tools: Vec<String>,
    /// For a cell whose judgment now takes a row that waits: what it waits for; no round is offered until it comes.
    pub waits: Option<String>,
    /// The design card it waits for the owner's ruling on, when it does.
    pub design_card: Option<i64>,
    /// The ask of the owner it waits for an answer to, when it does.
    pub ask: Option<i64>,
}

/// The engine as it runs now, and what the page needs of the installation.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Status {
    /// Whether the owner paused every work, as the engine starts: no work goes on by itself and no new work starts,
    /// though a work's round the owner asks for runs.
    pub paused: bool,
    /// The work the engine is taking a step of now: its round runs. While every work is paused it is the last round
    /// before the pause, unless the owner asked for it.
    pub running: Option<i64>,
    /// The work whose round the owner asked for, until that round has ended.
    pub stepping: Option<i64>,
    /// The last round the owner asked for since the engine started.
    pub last_step: Option<StepTaken>,
    /// The slots of every machine together, and how many of them works hold.
    pub slots: i64,
    pub slots_used: i64,
    /// The flow version new works start on.
    pub flow_version: i64,
    /// The configuration commit the engine runs with.
    pub config_commit: String,
    /// Each project's GitHub repository, as owner/name, by the project's name: the page links a work's pull request.
    pub repos: BTreeMap<String, String>,
    /// When the engine started.
    pub started_at: i64,
    /// The version of wf-agents that runs.
    pub version: String,
    /// While a failed run's cause stops every work alike, such as a usage limit or no network: no new round starts
    /// until it is over.
    pub halt: Option<Halt>,
}

/// Why the engine starts no new round for any work, and until when.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Halt {
    /// The kind of failed run, as the flow names it: `usage_limit`, `offline` or `cannot_run`.
    pub kind: String,
    /// Until when: when the usage limit resets, or when the engine next lets a round find out whether the cause has
    /// passed.
    pub until: i64,
    /// What the command line reported.
    pub why: String,
}

/// What changed, as the event stream gives it. An event names what to read again and never carries that state,
/// except the engine's status, which lives only in the engine. The stream gives each event an id; a client that
/// reconnects with the last id it saw is given what it missed.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    /// The stream is open and starts here: what was read before may already be out of date.
    Ready,
    /// The events since the id the client gave are no longer kept, or the engine started again: read everything
    /// again.
    Resync,
    /// The engine's status changed.
    Status { status: Box<Status> },
    /// A work started, moved, was paused, resumed or ended, or was said something to.
    Work { work: i64 },
    /// A round started or ended, or ran a tool: its work, and its number in the work.
    Round { work: i64, round: i64 },
    /// A design card was handed in, ruled or replaced.
    DesignCard { design_card: i64, work: Option<i64> },
    /// An ask of the owner was put or answered.
    Ask { ask: i64, work: Option<i64> },
    /// An agent told the owner something, or the owner marked it read.
    OwnerMessage { message: i64, work: Option<i64> },
    /// An item, its dependencies or its place in the queue changed; no item when which one is no longer known.
    Item { item: Option<i64> },
    /// A flow version was stored.
    Flow { version: i64 },
    /// What the architect's tab shows changed: its queue, what stops new works, the plan, or a wake, its rounds or
    /// its asks.
    Architect,
    /// What the agent of a running round, by its work and its number in the work, is doing now, as its command line
    /// reports it: a tool it uses, or what it says.
    Activity {
        work: i64,
        round: i64,
        what: String,
        at: i64,
    },
}

/// Where a work stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum WorkState {
    /// A round of it is running.
    Running,
    /// Its next step is due; the engine takes it on its next pass.
    Ready,
    /// It asked the owner or the architect, and waits for the ask to be answered.
    Asking,
    /// It took a row that waits: for other works, or for its turn.
    Waiting,
    /// The owner, the architect or the engine paused it.
    Paused,
    /// It reached the end of its graph.
    Done,
    /// It was stopped before its end, for example because its item was cancelled.
    Ended,
}

/// The round of a work that is running.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct RunningRound {
    /// Its number in the work.
    pub n: i64,
    pub cell: Option<String>,
    pub started_at: i64,
}

/// Who paused a work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PausedBy {
    /// The owner, on the page or the command line: Start all lets it go on.
    Owner,
    /// The engine, for a problem the owner looks at before letting it go on: a step that could not be taken, or
    /// returns sent back too often.
    Engine,
}

/// What a work waits for: once it comes, the work goes on by itself, unless it is paused.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Waits {
    /// The owner's answer to an ask, which the owner also gives for the architect.
    Answer { ask: i64 },
    /// The owner's ruling on a design card.
    Ruling { design_card: i64 },
    /// The items its item depends on, until each is merged.
    Items { items: Vec<i64> },
    /// What the row it took waits for, when it is none of these: the cell and the row, which its next round explains.
    Row { cell: String, row: String },
}

/// A work as a list shows it: what it works on, where it stands and what it has cost.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct WorkSummary {
    pub id: i64,
    /// The item worked on, its title and its project; none for a work on no item.
    pub item: Option<i64>,
    pub title: Option<String>,
    pub project: Option<String>,
    /// The graph it runs, of the flow version it is pinned to.
    pub graph: String,
    pub flow_version: i64,
    /// The cell it is at, with the cell's title and the stage the work is at, in its flow version.
    pub at: String,
    pub at_title: Option<String>,
    pub stage: Option<String>,
    pub state: WorkState,
    /// Why it was paused, and who paused it; why it ended before its end.
    pub paused: Option<String>,
    pub paused_by: Option<PausedBy>,
    pub ended: Option<String>,
    /// What it waits for before it goes on by itself.
    pub waits: Option<Waits>,
    /// The ask of the owner, put at the cell it is at, whose answer came: its next step takes the answer in.
    pub answered: Option<i64>,
    pub branch: Option<String>,
    pub pr: Option<i64>,
    /// When its first round started, and when its last round started or ended.
    pub started_at: Option<i64>,
    pub last_at: Option<i64>,
    pub rounds: i64,
    /// Rounds whose return was sent back, and rounds whose run failed or was cut off.
    pub sent_back: i64,
    pub failed: i64,
    pub running: Option<RunningRound>,
    /// What its rounds cost, as the command lines reported it, in US dollars.
    pub cost_usd: f64,
    /// Its asks of the owner still waiting for an answer.
    pub open_asks: i64,
    /// Its design cards still waiting for a ruling.
    pub open_design_cards: i64,
}

/// How a round ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RoundOutcome {
    /// It has not ended.
    Running,
    /// Its return took a row of its cell.
    Took,
    /// Its return was sent back: it did not fit the cell's schema or failed a check.
    SentBack,
    /// Its run failed before it handed in a return.
    Failed,
    /// The engine stopped while it ran; its cell runs again.
    Interrupted,
    /// It ended without a row: the end of the graph.
    Ended,
}

/// A round as a work's timeline shows it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct RoundSummary {
    /// Its number in its work.
    pub n: i64,
    /// Its cell; none for a round that only answered an interjection.
    pub cell: Option<String>,
    /// The line whose session it ran in; none for the engine's own steps, tool cells and the owner.
    pub line: Option<String>,
    pub outcome: RoundOutcome,
    /// The row its return took.
    pub transition: Option<String>,
    /// Why it was sent back, failed or was cut off.
    pub why: Option<String>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub model: Option<String>,
    pub cost_usd: Option<f64>,
    pub compactions: i64,
    pub tool_runs: i64,
    /// What the round said it did.
    pub progress: Option<String>,
    /// For a gate the engine answered itself, since the project does not have what it needs: that need.
    pub not_asked: Option<String>,
    /// For an agent's round, where it spent its tokens, from its usage record.
    pub tokens: Option<RoundTokens>,
}

/// Where an agent's round spent its tokens, as its usage record gives them, and how large its context was, which
/// the page divides the round's cost by.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct RoundTokens {
    /// The command line's steps in the round.
    pub steps: i64,
    /// Tokens of input not read from the cache.
    pub input: i64,
    /// Tokens read again from the cache.
    pub read: i64,
    /// Tokens newly written to the cache, kept five minutes and kept an hour.
    pub written_5m: i64,
    pub written_1h: i64,
    /// Tokens of output, and of them thinking.
    pub output: i64,
    pub thinking: i64,
    /// The context at the round's end: its last call's prompt; none when the record does not give the last call.
    pub context_end: Option<i64>,
    /// The size of the context window, as the command line reported it for the models of the session.
    pub window: Option<i64>,
    /// The context at the round's start: the end of the latest round before it in the same session, since the usage
    /// record keeps only the round's last call; none for a round that started its session.
    pub context_before: Option<i64>,
    /// The level the round ran at, as its call record says it; none for a round recorded before it said.
    pub level: Option<String>,
    /// The reasoning effort its command line gave it.
    pub effort: Option<String>,
    /// The models besides the round's own whose tokens grew during the round, by the command line's usage per model,
    /// which counts the whole session: the subagents that ran on other models.
    pub others: Vec<String>,
}

/// A design card, as the round that handed it in wrote it, with its rulings.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct DesignCard {
    pub id: i64,
    /// The round that handed it in: its number in the work.
    pub round: i64,
    pub work: i64,
    pub item: Option<i64>,
    /// open, ruled or replaced.
    pub status: String,
    pub body: Value,
    pub rulings: Value,
    pub replaced_by: Option<i64>,
    /// For a design card a newer one replaced: that one, as the engine knows it.
    pub replaced: Option<Replacement>,
    /// The cell of the round that handed it in, the cell's title in the work's flow version, and when that round
    /// ended.
    pub cell: Option<String>,
    pub title: Option<String>,
    pub at: Option<i64>,
    /// The design card this one replaced, if any, and what this one changed from it and why, in a sentence or two of
    /// its agent's.
    pub replaces: Option<i64>,
    pub changed: Option<String>,
}

/// The design card that replaced another: the round that handed it in, at which cell and when.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Replacement {
    pub design_card: i64,
    /// The round that handed it in: its number in the work.
    pub round: i64,
    pub work: i64,
    /// The cell of that round, and the cell's title in the work's flow version.
    pub cell: Option<String>,
    pub title: Option<String>,
    /// When that round ended.
    pub at: Option<i64>,
    /// What the newer design card changed and why, in a sentence or two of its agent's; none for one handed in before
    /// the flow asked for it.
    pub changed: Option<String>,
}

/// An ask of the owner: what is asked, and the owner's answer.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct OwnerAsk {
    pub id: i64,
    /// The round that put it to the owner: its number in the work.
    pub round: i64,
    pub work: i64,
    pub item: Option<i64>,
    pub body: Value,
    /// The kind of each point it asks about, in order, with the kind's group.
    pub kinds: Vec<AskKind>,
    /// None while it waits.
    pub answer: Option<Value>,
}

/// The kind of a point an ask is about, with the group of kinds it is in, so that the owner sees at a glance what
/// sort of question it is.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct AskKind {
    pub kind: String,
    /// The group the flow built into this program puts the kind in; none for a kind it does not have, such as one
    /// an earlier flow version had.
    pub group: Option<String>,
}

/// What an agent told the owner in a return, or the engine of its own accord, needing no answer.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct OwnerMessage {
    pub id: i64,
    /// Whether the engine wrote it: then `id` is the engine's notice's, and it has no round.
    pub engine: bool,
    /// The round whose return holds it, by its number in the work, its cell and line, and when it ended; for the
    /// engine's notice, when it was written.
    pub round: Option<i64>,
    pub cell: Option<String>,
    pub line: Option<String>,
    pub at: Option<i64>,
    pub work: Option<i64>,
    pub item: Option<i64>,
    pub title: Option<String>,
    pub text: String,
    /// When the owner marked it read; none while it is unread.
    pub read_at: Option<i64>,
    /// For the engine's notice of a new ask of the owner: that ask, and the kinds of the points it asks about.
    pub ask: Option<i64>,
    pub ask_kinds: Vec<AskKind>,
    /// Whether it came of a wake of the architect: told by one of its rounds, or the engine's notice of one.
    pub architect: bool,
}

/// Which messages to the owner the owner marks read: those named (agents' messages by `ids`, the engine's notices by
/// `notices`), or every one unread.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
pub struct MarkRead {
    #[serde(default)]
    pub ids: Vec<i64>,
    #[serde(default)]
    pub notices: Vec<i64>,
    #[serde(default)]
    pub all: bool,
}

/// The owner's answer to an ask of the owner, as the page and the command line hand it in.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
pub struct Answer {
    pub text: String,
    /// For what the architect is asked: why it was decided this way.
    #[serde(default)]
    #[ts(optional)]
    pub why: Option<String>,
    /// For what the architect is asked: the rest of the architect's reply (children, rest, waits).
    #[serde(default)]
    #[ts(optional)]
    pub reply: Option<Value>,
    /// For an ask of the architect's that offers options, the option chosen: for its study or its policy, one of those
    /// the ask lists; for its changes, approve_all, approve_some or approve_none.
    #[serde(default)]
    #[ts(optional)]
    pub choice: Option<String>,
    /// For the architect's changes approved in part, the number of each change approved.
    #[serde(default)]
    #[ts(optional)]
    pub approved: Option<Vec<i64>>,
}

/// The owner's ruling on a design card, as the page and the command line hand it in: a verdict on each of its
/// decisions, all sent at once.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
pub struct Ruling {
    /// One for each decision of the design card.
    pub decisions: Vec<DecisionRuling>,
}

/// The ruling on one decision of a design card.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
pub struct DecisionRuling {
    /// The decision's number on the design card, from 1.
    pub decision: i64,
    /// confirm or disagree.
    pub verdict: String,
    /// Why, for a disagreement; it may be left out when another approach is chosen.
    #[serde(default)]
    #[ts(optional)]
    pub text: Option<String>,
    /// For a disagreement: another of the decision's approaches, chosen instead of the one recommended, by its name.
    #[serde(default)]
    #[ts(optional)]
    pub option: Option<String>,
}

/// What an answer or a ruling did, in words.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Done {
    pub done: String,
}

/// How many messages a mark made read.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Marked {
    pub marked: i64,
}

/// What the owner or the architect said to a work.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Interjection {
    pub id: i64,
    pub work: i64,
    pub line: String,
    pub speaker: String,
    pub text: String,
    /// The round of the work it was handed in, by its number in the work; none while it waits.
    pub delivered_round: Option<i64>,
}

/// A work in full: where it stands as the engine keeps it, and every round it went through.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct WorkDetail {
    pub work: WorkSummary,
    pub machine: Option<String>,
    pub worktree: Option<String>,
    pub pushed_head: Option<String>,
    /// The cell the work left to ask, and the cells waiting for an answer, innermost last.
    pub resume_at: Option<String>,
    pub waiting: Value,
    /// The values its rows recorded.
    pub recorded: Value,
    pub rounds: Vec<RoundSummary>,
    pub design_cards: Vec<DesignCard>,
    pub asks: Vec<OwnerAsk>,
    /// What its agents told the owner, and what the engine told the owner of it, oldest first.
    pub messages: Vec<OwnerMessage>,
    pub interjections: Vec<Interjection>,
}

/// The session of a line, as the command line knows it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Session {
    pub line: String,
    pub cli: String,
    pub cli_session: String,
}

/// One input a round was given, as a step that is not an agent's round records it, or as an agent's round recorded it
/// before its message was recorded with its conditional inputs: what it is, its value, and where its cell reads it
/// from.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct RoundInput {
    pub what: String,
    pub value: Value,
    /// The sources the cell's definition names for it; empty when the cell no longer names this input.
    pub from: Vec<Src>,
}

/// One conditional input of an agent round's message.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct RoundConditional {
    /// Its address, such as `R5-12·2a·design`, `rule:ground` or `format:common`.
    pub address: String,
    /// The hash of its content, which tells two versions under one address apart.
    pub hash: String,
    /// Given in the message; otherwise the session already held it, and the message only referred to it.
    pub given: bool,
}

/// A file a tool run kept, or that its result points to, under the data directory.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct KeptFile {
    /// The path as the database holds it.
    pub path: String,
    /// Where the interface serves it: `logs/<name>` or `tool-output/<name>`, under `/api/v1/files/`.
    pub file: String,
}

/// One attempt of a tool the engine ran.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ToolRun {
    pub id: i64,
    pub tool: String,
    pub args: Value,
    pub attempt: i64,
    /// The result, a refusal and its reason included; none while it runs.
    pub result: Option<Value>,
    pub files: Vec<KeptFile>,
    pub queued_at: i64,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
}

/// A round in full: everything the engine gave, the return, what the engine made of it, and what it cost.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct RoundDetail {
    pub round: RoundSummary,
    pub work: i64,
    /// The work's flow version and graph, in which the round's cell is found.
    pub flow_version: i64,
    pub graph: String,
    pub session: Option<Session>,
    pub config_commit: String,
    /// The message exactly as the agent was sent it (for a round recorded before messages were, its prompt); none
    /// for a round no agent ran.
    pub message: Option<String>,
    /// Each conditional input the message gave or referred to; empty for a round no agent ran, or one recorded
    /// before.
    pub conditional: Vec<RoundConditional>,
    /// How the command line was called: the whole command line, the session started or resumed, the engine's and
    /// the command line's versions; none for a round no agent ran, or one recorded before.
    pub call: Option<Value>,
    /// The level an agent's round ran at: as its call record says, or for a round recorded before it said, the one
    /// level the configuration at the round's commit maps its model and effort to; none when no one level does.
    pub level: Option<String>,
    /// The effort an agent's round ran with, as its command line gave it.
    pub effort: Option<String>,
    /// For a round that locked a new acceptance: the acceptance locked before, which it replaced; none for one that
    /// locked the first.
    pub acceptance_before: Option<Value>,
    /// The limits of the flow's counters for the work's project, as the configuration the round ran with sets them,
    /// by counter.
    pub limits: BTreeMap<String, u64>,
    /// The inputs of a step that is not an agent's round, or of an agent's round recorded before its conditional
    /// inputs were.
    pub inputs: Vec<RoundInput>,
    /// What the engine gave, as recorded: the message, conditional inputs, call or inputs above, and what the engine knew
    /// besides, such as the head and the interjections handed in.
    pub input: Value,
    pub output: Option<Value>,
    /// Why the return was sent back or the run failed.
    pub rejected: Option<Value>,
    /// Tokens and cost as the command line reported them.
    pub usage: Option<Value>,
    pub tool_runs: Vec<ToolRun>,
    pub design_cards: Vec<DesignCard>,
    pub asks: Vec<OwnerAsk>,
    /// What was said to the work and handed in with this round.
    pub interjections: Vec<Interjection>,
}

/// A stored flow version.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct FlowVersion {
    pub id: i64,
    pub digest: String,
    /// How many works are pinned to it.
    pub works: i64,
}

/// Where an item stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ItemState {
    /// Never worked on, and not in the queue.
    New,
    /// In the dispatch queue, waiting for a work to start.
    Queued,
    /// Its latest work runs, waits or is paused.
    Working,
    /// Its latest work reached its end.
    Done,
    /// Its latest work was stopped before its end.
    Stopped,
    Cancelled,
}

/// An item as a list shows it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ItemSummary {
    pub id: i64,
    pub project: String,
    pub title: String,
    /// change or report.
    pub deliver: String,
    pub level: String,
    pub cancelled: Option<String>,
    pub state: ItemState,
    /// Its place in the dispatch queue, lower first; none when it is not queued.
    pub queued: Option<f64>,
    /// Its latest work and the cell that work is at.
    pub work: Option<i64>,
    pub at: Option<String>,
    /// The stage its latest work is at, and the stages of that work's graph in their order, among which it stands;
    /// none and empty before a work starts.
    pub stage: Option<String>,
    pub stages: Vec<String>,
    /// Its latest work's asks of the owner still waiting for an answer, and its design cards still waiting for a
    /// ruling.
    pub open_asks: i64,
    pub open_design_cards: i64,
    /// Its pull request: that of its latest work that has one, or the one it was imported with.
    pub pr: Option<i64>,
}

/// A dependency: `item` cannot finish until `depends_on` is done.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ItemLink {
    pub item: i64,
    pub depends_on: i64,
    /// What `item` needs `depends_on` to hand in.
    pub needs: String,
}

/// Every item, and the dependencies between them.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Items {
    pub items: Vec<ItemSummary>,
    pub links: Vec<ItemLink>,
}

/// An item on the other end of a dependency, with what the dependency needs.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct LinkedItem {
    pub item: ItemSummary,
    pub needs: String,
}

/// An item in full: as its creator wrote it, its locked acceptance, its dependencies both ways, its works, design cards
/// and asks of the owner.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ItemDetail {
    pub item: ItemSummary,
    pub problem: String,
    pub why: String,
    pub approach: Option<String>,
    /// The locked acceptance, and the round that locked it, by its work and its number in the work; none until a first
    /// design passes its checks.
    pub acceptance: Option<Value>,
    pub acceptance_work: Option<i64>,
    pub acceptance_round: Option<i64>,
    /// The items it depends on, and the items that depend on it.
    pub depends_on: Vec<LinkedItem>,
    pub depended_on_by: Vec<LinkedItem>,
    /// Its works, the earlier ones too: a work stopped when the flow moved to a new version, and the work after it
    /// that took the item over.
    pub works: Vec<WorkSummary>,
    /// What an import recorded of work on the item outside this flow: where it came from, its branch and its pull
    /// request; none for an item made here.
    pub imported_from: Option<String>,
    pub imported_branch: Option<String>,
    pub imported_pr: Option<i64>,
    pub design_cards: Vec<DesignCard>,
    pub asks: Vec<OwnerAsk>,
    /// What the rounds of its works handed back that their flow versions show on the item, oldest first.
    pub shown: Vec<Shown>,
}

/// A value a round handed back that the flow shows on the item the work is on: the owner reads it there.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Shown {
    pub work: i64,
    /// The round that handed it back: its number in the work.
    pub round: i64,
    pub cell: String,
    /// The path of the return it is at, as the flow writes it.
    pub path: String,
    pub value: Value,
}

/// The architect, as its tab shows it: its wakes, its queue, what it asked the owner, its log, the policy and the plan
/// it works by, and where things stand as it last wrote it. A wake is a work of the architect's graph, on no item.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Architect {
    /// Its wakes, newest first.
    pub wakes: Vec<Wake>,
    /// The matters it has not handled: those due and not held, in the order they came; then those held; then those
    /// not due yet, the soonest first.
    pub queue: Vec<Matter>,
    /// What stops new works from starting, oldest first.
    pub holds: Vec<DispatchHold>,
    /// What its wakes asked the owner, oldest first.
    pub asks: Vec<OwnerAsk>,
    /// Its log over its latest fifty wakes, newest first: each answer to whether what runs fits, and each matter it
    /// picked, with what it made of it.
    pub log: Vec<LogEntry>,
    /// The policy in force; none before the first takes effect.
    pub policy: Option<PolicyInForce>,
    /// The policy a wake is setting now; none while no wake sets one.
    pub setting: Option<PolicySetting>,
    pub understanding: Understanding,
    pub plan: Plan,
}

/// One wake of the architect.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Wake {
    pub work: WorkSummary,
    /// What woke it: the matters its queue held when it woke, as its first round was given them.
    pub woke_for: Vec<Queued>,
    /// How many matters it handled.
    pub handled: i64,
    /// The matter it handles now, as its last pick chose it, until it records that matter; none when it handles none,
    /// or is over.
    pub handling: Option<Value>,
}

/// A matter as the architect's queue gave it to a wake.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Queued {
    pub matter: i64,
    pub kind: String,
    pub what: String,
    pub item: Option<i64>,
}

/// A matter for the architect that it has not handled.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Matter {
    pub id: i64,
    /// Its kind, one of the flow's kinds of matter for the architect.
    pub kind: String,
    /// What it is, in one sentence, as the engine wrote it.
    pub what: String,
    /// Who raised it: `work`, a round of a work; `architect`, a round of its own; `owner`, the owner's words or
    /// answer; `engine`, the engine.
    pub source: String,
    /// The item it concerns, and the item's title.
    pub item: Option<i64>,
    pub title: Option<String>,
    /// The round that raised it, by its work and its number in the work.
    pub work: Option<i64>,
    pub round: Option<i64>,
    /// When it came, and when it is due: none for a matter due at once.
    pub at: i64,
    pub due: Option<i64>,
    /// For a matter due once an item's work ends: that item.
    pub due_item: Option<i64>,
    /// Why it waits though picked (`owner_ask`, `approval`), and the ask of the owner that holds it.
    pub held: Option<String>,
    pub held_by: Option<i64>,
    /// For a matter due and not held: who waits for it (`agent`, an engineer stopped for its question; `owner`; or
    /// `none`), and how long since it was due, in milliseconds.
    pub waits: Option<String>,
    pub waited_ms: Option<i64>,
    /// For the owner's answer to an ask that held a matter: that matter.
    pub answers: Option<i64>,
}

/// What stops new works from starting: `architect`, its own hold; `tidying`, while it sets the whole straight;
/// `policy`, while a new policy is set; and since when.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct DispatchHold {
    pub reason: String,
    pub since: i64,
}

/// What a round of the architect handed back: its wake, its number in the wake, when it ended, and its answer.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Said {
    pub work: i64,
    pub round: i64,
    pub at: Option<i64>,
    pub value: Value,
}

/// One entry of the architect's log, by the wake and the round it starts at.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LogEntry {
    /// Its answer to whether what runs and what starts next fit.
    Fit {
        work: i64,
        round: i64,
        at: Option<i64>,
        fit: Value,
    },
    /// A matter it picked: the pick (the ranked list and the one chosen), the crux, each judgment and each second look
    /// at it in order (a second look that does not stand sends it back to be judged again), and the round that
    /// recorded it; none for one it did not record, such as one that needed a new policy.
    Matter {
        work: i64,
        round: i64,
        at: Option<i64>,
        pick: Value,
        crux: Option<Said>,
        judgments: Vec<Said>,
        looks: Vec<Said>,
        recorded: Option<i64>,
    },
}

/// The policy in force: its text, and the round of the wake in which it took effect, after the owner approved it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct PolicyInForce {
    pub work: i64,
    pub round: i64,
    pub at: Option<i64>,
    pub policy: Value,
}

/// A new policy as a wake sets it: the latest study of it, and the latest text with its probes and every reading of it
/// by a new architect since, with the architect's verdict on each.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct PolicySetting {
    pub work: i64,
    pub study: Option<Said>,
    pub draft: Option<Draft>,
}

/// A version of the policy being set, by the round that wrote it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Draft {
    pub round: i64,
    pub policy: Value,
    pub probes: Value,
    pub readings: Vec<Reading>,
}

/// A new architect's reading of the policy being set, by its round, with the verdict on it; none until judged.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Reading {
    pub round: i64,
    pub reading: Value,
    pub verdict: Value,
}

/// Where things stand, as the architect last wrote it: the state of things at its latest first question, what its
/// wrap-ups learned since, oldest first, and its latest check against the constitution with the mismatches with its
/// earlier judgments.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Understanding {
    pub state: Option<Said>,
    pub learned: Vec<Value>,
    pub against: Option<Said>,
}

/// The plan: its segments in the order works start from them, its gates and its budgets.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Plan {
    pub segments: Vec<PlanSegment>,
    pub gates: Vec<PlanGate>,
    pub budgets: Vec<PlanBudget>,
}

/// A segment of the plan, with the items that hang on it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct PlanSegment {
    pub key: String,
    pub project: String,
    pub title: String,
    /// The stage of the policy it belongs to, by the stage's number, or maintenance.
    pub stage: String,
    pub delivers: String,
    pub done_when: String,
    /// The segments that must be done first, by key.
    pub after: Value,
    pub assumptions: Value,
    pub indicators: Value,
    pub size: String,
    /// Until when or what it is deferred.
    pub deferred: Option<String>,
    /// The items that hang on it, each with its title, whether it is cancelled or held, and the cell its latest work
    /// is at.
    pub items: Value,
}

/// A gate of the plan: which segments may touch what it governs.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct PlanGate {
    pub name: String,
    pub basis: String,
    pub what: String,
    pub segments: Value,
}

/// A budget of the plan: what is measured, the whole line and how it is shared out among the segments.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct PlanBudget {
    pub name: String,
    pub basis: String,
    pub measure: String,
    pub line: String,
    pub split: Value,
}

/// Why a request failed.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ApiError {
    pub error: String,
}

/// What the flow page shows of a compiled flow besides the flow itself, graph by graph.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct FlowView {
    pub graphs: Vec<GraphView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct GraphView {
    pub name: String,
    /// How each line's session is kept (when it starts and which cells continue it), as the flow built into this
    /// program says: a compiled flow version keeps the lines, not what they are.
    pub lines: Vec<flow::format::Named>,
    /// Each cell, in the order the graph has them.
    pub cells: Vec<CellView>,
    /// The forms every agent's return shares, as `format:common` gives them, with no project's settings written beside
    /// their names and no kind of question left out.
    pub common_format: String,
}

/// One cell, as the flow page explains it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct CellView {
    pub id: String,
    /// For each row of its transition table, in order, the cells the row can lead to: its target, or the cells a
    /// target the engine keeps track of (`@self`, `@caller`, `@asker`, `@stopped`) can be.
    pub targets: Vec<Vec<String>>,
    /// Every row of every cell that can lead here, in the order of the cells and of their rows.
    pub entered_from: Vec<RowRef>,
    /// What a round of the cell is given: for an agent's cell, every input, always or conditional, each with its
    /// source and address; for any other cell, the sources of its inputs.
    pub inputs: Vec<CellInput>,
    /// What the cell hands back, with whatever reads each field.
    pub returns: Returns,
}

/// A row of a cell, by its place in the cell's transition table, from 0.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct RowRef {
    pub cell: String,
    pub row: usize,
}

/// One input a round of a cell is given.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct CellInput {
    /// Given every time the cell is entered: what the agent reads itself. Every other input is conditional: given
    /// when the session does not hold it, and referred to by its address when it does.
    pub always: bool,
    pub of: CellInputOf,
    /// The address it is given under, as the engine addresses it, with `‹work›`, `‹n›`, `‹id›`, `‹cell›` or `‹name›`
    /// standing for what is known only when the round runs; none for what the agent reads itself.
    pub address: Option<String>,
    /// For a place of the cell the work came from, the cell that called or the cell the work stopped at: the cells
    /// it can be.
    pub cells: Vec<String>,
    /// What it holds, for a text the flow itself writes: the cell's own return format and what each of its inputs
    /// is, as the engine gives them, with no project's settings beside their names.
    pub holds: Option<String>,
}

/// What the inputs that come from outside the flow hold now, as far as the page can know outside a round: each
/// project's settings, guide and rules, as its configuration and its checkout have them, and what each table and view
/// of the database holds.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct InputsNow {
    pub projects: Vec<ProjectNow>,
    /// Each table and view agents can read, with what its definition says it holds.
    pub tables: Vec<NamedText>,
}

/// One project's part of what the inputs from outside the flow hold now.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ProjectNow {
    pub project: String,
    /// Each setting the flow may read, by its name, with its value as text: a text as it is, anything else as JSON.
    pub settings: Vec<NamedText>,
    /// The project's AGENTS.md as its checkout has it now.
    pub agents_md: String,
    /// The index of its design tree as its checkout has it now: each node's path with its first line. The full text
    /// of the nodes an item cites is added for each item.
    pub design_tree: Vec<NamedText>,
    /// The project's own rules, each by its name with its text.
    pub rules: Vec<NamedText>,
}

/// A name with a text.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct NamedText {
    pub name: String,
    pub text: String,
}

/// What an input is.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CellInputOf {
    /// One source of one of the cell's inputs, by their places in the cell's inputs and in the input's sources.
    Input { input: usize, source: usize },
    /// The project's rules, which every agent is given, as the project's configuration lists them, with the value of
    /// each setting they name beside it.
    ProjectRules,
    /// A rule of the flow the cell is given.
    Rule { name: String },
    /// The forms every agent's return shares.
    CommonFormat,
    /// The cell's own answer, and which of the shared forms the cell has.
    CellFormat,
    /// What each of the cell's inputs is, by the number a round's list of inputs to work from gives it.
    CellInputs,
    /// The work's worktree and branch, and the project's branch the flow calls main.
    Workspace,
}

/// What a cell hands back, as a tree of fields.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Returns {
    /// For an agent's cell: the field its return holds exactly one form in.
    pub within: Option<String>,
    /// For an agent's cell: the forms it may hand in, its own answer first.
    pub forms: Vec<ReturnForm>,
    /// The fields every return holds: for an agent's cell those beside the form, for any other cell all of them.
    pub every: Vec<Field>,
    /// What reads the return as a whole.
    pub used_by: Vec<Use>,
}

/// One form an agent's return may hold.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ReturnForm {
    /// The form's field, such as `ask` or `tools`; none for the cell's own answer.
    pub field: Option<String>,
    /// The form's fields: the own answer's fields, or the one field the form is.
    pub fields: Vec<Field>,
    /// What reads that a return is this form: the rows whose condition is the cell's own answer.
    pub used_by: Vec<Use>,
}

/// The kind of value a field holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Text,
    Integer,
    Number,
    Boolean,
    /// One of a fixed set of values.
    Enum,
    Object,
    List,
    /// One of several shapes, told apart by the value of one field.
    Union,
    /// Anything.
    Any,
}

/// A field of a return, with whatever reads it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Field {
    pub name: String,
    /// Its path as the flow writes it, such as `end.test_changes[added].catches`.
    pub path: String,
    pub kind: FieldKind,
    /// For a list, the kind of its items.
    pub item: Option<FieldKind>,
    pub desc: Option<String>,
    pub optional: bool,
    /// For a list, the fewest items it holds.
    pub min: Option<u64>,
    /// For one of a fixed set of values: each value, with what it means when the flow says.
    pub values: Vec<(String, Option<String>)>,
    pub used_by: Vec<Use>,
    /// The fields of an object, or of the objects a list holds.
    pub fields: Vec<Field>,
    /// For one of several shapes: the field whose value says which, and what reads that field.
    pub disc: Option<String>,
    pub disc_used_by: Vec<Use>,
    /// For one of several shapes: the fields every shape has, and each shape.
    pub common: Vec<Field>,
    pub branches: Vec<Branch>,
}

/// One of the shapes a field may take.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Branch {
    /// The value of the field that says which shape it is.
    pub value: String,
    /// Its path as the flow writes it, such as `result[green]`.
    pub path: String,
    pub desc: Option<String>,
    pub used_by: Vec<Use>,
    pub fields: Vec<Field>,
}

/// Something that reads a field of a return.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Use {
    /// An input of a cell, by its place in the cell's inputs.
    Input { cell: String, input: usize },
    /// The condition of a row.
    Condition { cell: String, row: usize },
    /// An action a row carries out on it.
    Action { cell: String, row: usize, action: String },
    /// What the engine does every time the cell returns it: shows it in a view, or carries out an action.
    Always {
        cell: String,
        view: Option<String>,
        action: Option<String>,
    },
    /// A check the engine makes on it before it takes a row.
    Check { cell: String, check: String },
    /// Handed to whoever takes over.
    Handoff { cell: String },
    /// Shown on the item the work is on: the owner reads it there.
    Shown { cell: String },
    /// Read outside this flow by a role, through the database.
    Role { cell: String, role: String },
}

/// What changed from one compiled flow to another, graph by graph.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct FlowDiff {
    pub graphs: Vec<GraphDiff>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct GraphDiff {
    pub name: String,
    /// Whether the graph was added or removed whole, or changed.
    pub status: ChangeKind,
    /// What changed in the graph apart from its cells: its start and end, its rules, the interjection, the counters.
    pub changes: Vec<Change>,
    /// Each cell added, removed or changed, in the order of the newer flow, the removed ones last.
    pub cells: Vec<CellDiff>,
}

/// Whether a thing was added, removed or changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Removed,
    Changed,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct CellDiff {
    pub id: String,
    /// Its title in the newer flow, or in the older one for a cell removed.
    pub title: String,
    pub status: ChangeKind,
    pub changes: Vec<Change>,
}

/// The part of a cell or a graph a change is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Part {
    Title,
    Kind,
    Gate,
    Line,
    Stage,
    Step,
    Needs,
    Input,
    Rule,
    Prompt,
    PromptBy,
    Forms,
    Field,
    Row,
    RowOrder,
    Check,
    Always,
    Handoff,
    Shown,
    ReadBy,
    Inenum,
    Start,
    End,
    Asking,
    Receive,
    Counter,
    Interjection,
    Rounds,
}

/// One change: the part, which one of it, and what it was and is, as the compiled flow writes it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Change {
    pub part: Part,
    /// Which one of the part: an input's text, a rule's or a row's name, a field's path, a check's name; empty for
    /// a part a cell has one of.
    pub key: String,
    /// What it was; none for what was added.
    pub before: Option<Value>,
    /// What it is; none for what was removed.
    pub after: Option<Value>,
}

/// A project's checkout, where the flow can be read at a git ref.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct FlowSource {
    pub project: String,
    pub checkout: String,
    /// Its branches, the most recently committed first.
    pub refs: Vec<String>,
}

/// The flow as `flow/flow.json` is at a git ref of a project's checkout, compiled by the flow's compiler.
#[derive(Debug, Clone, Serialize, TS)]
pub struct FlowAt {
    pub project: String,
    /// The ref as it was asked for, and the commit it names.
    pub rev: String,
    pub commit: String,
    pub digest: String,
    /// The stored flow version with the same digest, when there is one.
    pub version: Option<i64>,
    pub compiled: flow::Compiled,
    pub view: FlowView,
}
