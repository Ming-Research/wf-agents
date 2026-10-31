//! What the interface hands out. Times are UTC milliseconds since the Unix epoch, as in the database; JSON the
//! database keeps whole (a round's input and return, a design card's body) is handed out as it is kept. A round is
//! given by its work and its number in the work, as the database keeps it, and named `R<work>-<n>`.

use std::collections::BTreeMap;

use flow::format::Src;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

/// What the owner sends with a work's or the architect's Start and Step: whether its next round resumes the session
/// its line goes on in (the default) or starts a new one.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
pub struct RunAsk {
    #[serde(default)]
    pub new_session: bool,
}

/// What the owner sends with the switch that keeps waiting sessions warm while every work is paused.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct KeepWarm {
    pub on: bool,
}

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

/// The engine as it runs now, and what the page needs of the installation.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Status {
    /// Whether the owner paused every work, as the engine starts: no work goes on by itself and no new work starts,
    /// but for the works the owner started since; a work's round the owner asks for runs.
    pub paused: bool,
    /// Whether the sessions of waiting works are kept warm while every work is paused: the owner's switch beside the
    /// pause, on at first and kept across restarts.
    pub keep_warm: bool,
    /// The works the owner started since every work was last paused: each goes on whatever the global state.
    pub started: Vec<i64>,
    /// The works the owner started that are still paused, until their machine has a slot free: the engine then lifts
    /// their pause.
    pub releasing: Vec<i64>,
    /// The works whose wait is over, the owner's or the architect's answer come or what their row waited for, that
    /// wait on, holding no slot, until their machine has a slot free, as the engine's last step of each found: each
    /// then goes on before any new work starts. A work's state on the page is waiting for a free slot while it is
    /// named here.
    pub waiting_for_slot: Vec<i64>,
    /// The projects whose architect the owner started since every work was last paused: its works go on whatever the
    /// global state.
    pub architects_started: Vec<String>,
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
    /// What the architect's tab shows changed: its queue, what stops new works, or a work of the architect, its rounds
    /// or its asks.
    Architect,
    /// A project's observations changed: one was recorded, reported again, taken in by an item or closed.
    Observations,
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
    /// What it waited for came, and it waits on, holding no slot, until its machine has a slot free. The works as read
    /// never stand so: the engine's status names such works (`waiting_for_slot`), and the page gives a work so named
    /// this state in place of the one it reads.
    WaitingForSlot,
    /// The owner, the architect or Pause all paused it.
    Paused,
    /// The engine stopped it for a problem, which the owner looks at before starting it again.
    Halted,
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
    /// Whether a tool it runs is under way: started, as tool_runs records it, and not ended.
    pub tool: bool,
    /// Who is at work in it, by its line: the agent whose cell it is; for a cell that runs the tools an agent asked
    /// for, the line that asked; none for the engine's own steps and the tools it runs of its own.
    pub line: Option<String>,
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
    /// The architect, which the engine keeps as its own pause, with the architect's reason: Start all does not let it
    /// go on; the architect's resume, or the owner's Continue of that work, does.
    Architect,
}

/// What a work waits for: once it comes, the work goes on by itself, unless it is paused.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Waits {
    /// The owner's answer to an ask, which the owner also gives for the architect.
    Answer { ask: i64 },
    /// The owner's ruling on a design card.
    Ruling { design_card: i64 },
    /// The reply of the architect that holds the work's question, to the matter that stands for it in that
    /// architect's queue: the project's own architect, or a related project's (`project`) that it was handed to.
    Architect {
        matter: i64,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        project: Option<String>,
    },
    /// The items its item depends on, until each is merged.
    Items { items: Vec<i64> },
    /// What the row it took waits for, when it is none of these: the cell and the row.
    Row { cell: String, row: String },
}

/// A work as a list shows it: what it works on, where it stands and what it has cost.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct WorkSummary {
    pub id: i64,
    /// The item worked on, its title and its project; for a work of an architect, no item and the architect's project.
    pub item: Option<i64>,
    pub title: Option<String>,
    pub project: Option<String>,
    /// The graph it runs, of the flow version it is pinned to.
    pub graph: String,
    pub flow_version: i64,
    /// The flow version it last moved from to the one it runs on, when it went on after the flow moved to a new version
    /// at a cell unchanged there; none for a work that never moved.
    pub moved_from: Option<i64>,
    /// The cell it is at, with the cell's title and the stage the work is at, in its flow version.
    pub at: String,
    pub at_title: Option<String>,
    pub stage: Option<String>,
    pub state: WorkState,
    /// Why it was paused, and who paused it (for a work the architect stopped, the architect's reason); why it ended
    /// before its end.
    pub paused: Option<String>,
    pub paused_by: Option<PausedBy>,
    pub ended: Option<String>,
    /// What it waits for before it goes on by itself.
    pub waits: Option<Waits>,
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
    /// What its rounds and the keep-warm pings of its sessions cost, as the command lines reported it, in US dollars.
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

/// A round as a list of rounds shows it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct RoundSummary {
    /// The work it is a round of, and its number in that work.
    pub work: i64,
    pub n: i64,
    /// Its cell; none for a round that only answered an interjection.
    pub cell: Option<String>,
    /// The line whose session it ran in; none for the engine's own steps, tool cells and the owner.
    pub line: Option<String>,
    pub outcome: RoundOutcome,
    /// The row its return took.
    pub transition: Option<String>,
    /// Why it was sent back, failed or was cut off; for a row the engine took as a session would start afresh, why.
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
    /// The session it ran in, by its row; none for the engine's own steps, tool cells and the owner.
    pub session: Option<i64>,
    /// When it started a new session: why, as the session manager recorded it: line, the line had none going on (its
    /// first in the work, or a cell on no line, whose session is its own every round); cell, a cell the line starts a
    /// new session at; limit, the line's session before was past the context limit; model or account, the model or the
    /// login account its rounds run under changed; owner, the owner asked for one.
    pub new_session: Option<String>,
}

/// A window of a project's architect's rounds, across its works, in the order they started, oldest first; and whether
/// rounds older than its first and newer than its last are there to read.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ArchitectRounds {
    pub rounds: Vec<RoundSummary>,
    pub older: bool,
    pub newer: bool,
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
    /// What each kind of these tokens cost, by the prices of the round's model as the session manager keeps them;
    /// none for a model with no prices there.
    pub costs: Option<TokenCosts>,
}

/// What each kind of a round's tokens cost, in US dollars, by its model's prices.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
pub struct TokenCosts {
    /// Input not read from the cache, reads from the cache, writes to it, and output.
    pub input: f64,
    pub read: f64,
    pub written: f64,
    pub output: f64,
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
    /// The newest design card of its work the owner ruled on that this one took the place of, directly or through cards
    /// between: the owner confirms again what changed since.
    pub ruled_before: Option<i64>,
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
    /// None while it waits.
    pub answer: Option<Value>,
    /// The images the owner sent with the answer, by their ids, in order.
    pub answer_images: Vec<i64>,
    /// When the owner was told of it, from which it waits: the engine's notice of it, or else the start of the round
    /// that put it.
    pub asked_at: i64,
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
    /// For the engine's notice of a new ask of the owner: that ask.
    pub ask: Option<i64>,
    /// For one that came of a work of an architect, told by one of its rounds or the engine's notice of one: the
    /// architect's project; none otherwise.
    pub architect: Option<String>,
}

/// The cache misses a notice tells of: how many, the tokens they wrote to the cache again, what that cost beyond
/// reading the cache, and when the latest came.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Misses {
    pub count: i64,
    pub written: i64,
    pub cost_usd: f64,
    pub last_at: Option<i64>,
}

/// Where a fault of wf-agents went: the project whose architect it was forwarded to, who forwarded it, and how the
/// matter it became stands there.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Forwarded {
    pub project: String,
    /// `owner`, forwarded by hand; `setting`, by the owner's setting that forwards each new fault as it comes.
    pub by: String,
    /// The matter it is in that architect's queue.
    pub matter: i64,
    /// `queued` while the matter is not handled; `handled`, with the architect's round that handled it, by its work
    /// and its number; `withdrawn`.
    pub state: String,
    pub handled_work: Option<i64>,
    pub handled_round: Option<i64>,
}

/// Where the faults of wf-agents can go, for wf-agents' own conversation: the projects whose architects a fault may
/// be forwarded to, every project of the configuration in its order, and the one whose architect the owner's setting
/// forwards each new fault to as it comes, none for nobody.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Forwarding {
    pub projects: Vec<String>,
    pub auto: Option<String>,
}

/// What the owner sends to forward a fault of wf-agents: the project whose architect takes it.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
pub struct ForwardFault {
    pub project: String,
}

/// What the owner sends to set where each new fault of wf-agents goes as it comes: a project, whose architect takes
/// it, or none for nobody.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
pub struct ForwardNew {
    pub project: Option<String>,
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
    /// For an ask that holds an engineer's work: `architect` to hand the question back to the project's architect with
    /// the answer as a decision for it to carry out, instead of resolving it.
    #[serde(default)]
    #[ts(optional)]
    pub to: Option<String>,
    /// The images sent with the answer, in the order the owner added them.
    #[serde(default)]
    pub images: Vec<ImageSent>,
}

/// An image the owner sends with words, as the page hands it in: a PNG, JPEG, GIF or WebP image of at most 5 MB, at
/// most 10 to a message.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
pub struct ImageSent {
    /// image/png, image/jpeg, image/gif or image/webp.
    pub media_type: String,
    /// Its bytes, as base64.
    pub data: String,
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
    /// The sessions its lines ran in, oldest first.
    pub sessions: Vec<WorkSession>,
    /// The keep-warm pings sent to its sessions while it waited, oldest first.
    pub pings: Vec<Ping>,
}

/// The session of a line, as the command line knows it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Session {
    pub line: String,
    pub cli: String,
    pub cli_session: String,
}

/// A session one of a work's lines ran in.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct WorkSession {
    pub id: i64,
    pub line: String,
    /// Why it started: line, cell, limit, or carried (a work of the architect going on in the CLI session the project's
    /// previous architect work left).
    pub started: String,
    /// When it ended, so that the line's next round started a new one; none while it goes on.
    pub ended: Option<i64>,
}

/// A keep-warm ping: a minimal message the session manager sent a session while its work waited, so that the
/// session's prompt cache was read instead of expiring. A ping means the work waited over an hour.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Ping {
    pub id: i64,
    pub session: i64,
    pub line: String,
    /// The cell the work waited at, what it waited on (pause, owner, architect, items or tool) and that in words.
    pub cell: String,
    pub on: String,
    pub what: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub model: Option<String>,
    /// What it cost, as the command line reported it; none for one that failed or runs.
    pub cost_usd: Option<f64>,
    /// Tokens read from the cache, and written to it.
    pub read: i64,
    pub written: i64,
    /// Why it failed, when it did.
    pub error: Option<String>,
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
    /// Its address, such as `R5-12·D2·design`, `rule:ground` or `format:common`.
    pub address: String,
    /// The hash of its content, which tells two versions under one address apart.
    pub hash: String,
    /// Given in the message; otherwise the session already held it, and the message only referred to it.
    pub given: bool,
}

/// An image the owner sent that an agent round's message named.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct RoundImage {
    /// The image, by its id; it is read at `/api/v1/attachments/<id>`.
    pub attachment: i64,
    /// The address of what the message gave that named it: the owner's words it came with, or what holds them.
    pub with: String,
    /// Given after the message's text; otherwise the session held it, and the message only referred to it.
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
    /// The images the owner sent that the message named, in the order named: those given after its text, and those
    /// its session held, which it only referred to.
    pub images: Vec<RoundImage>,
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
    /// The items not merged yet that it depends on by a blocking dependency, which keep it from starting; empty while a
    /// work on it goes on, or it is done or cancelled.
    pub blocked_by: Vec<i64>,
}

/// A dependency: `item` cannot finish until `depends_on` is done.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ItemLink {
    pub item: i64,
    pub depends_on: i64,
    /// What `item` needs `depends_on` to hand in.
    pub needs: String,
    /// blocking: `item` does not start until `depends_on` is merged; parallel: it may start before.
    pub kind: String,
}

/// Every item, and the dependencies between them.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Items {
    pub items: Vec<ItemSummary>,
    pub links: Vec<ItemLink>,
}

/// An item on the other end of a dependency, with what the dependency needs and its kind, blocking or parallel.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct LinkedItem {
    pub item: ItemSummary,
    pub needs: String,
    pub kind: String,
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
    /// Each time its tier changed after it was made, oldest first: when, by whom, from what to what.
    pub level_changes: Vec<LevelChange>,
}

/// A change of an item's tier, as its history keeps it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct LevelChange {
    pub at: i64,
    /// owner, architect or engine.
    pub by: String,
    pub from: String,
    pub to: String,
}

/// The settings the owner keeps on the page, as the configuration's last commit has them: the installation's and each
/// project's, those the owner changes there and those only shown, and the text of each rule a project takes in.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Settings {
    /// Each tier, in order.
    pub tiers: Vec<Tier>,
    /// The highest tier an item may be at; none, any.
    pub highest_level: Option<String>,
    /// The tier the architect runs at.
    pub architect: String,
    /// The context, in tokens, past which a line's session is replaced at its next round.
    pub context_limit: i64,
    /// Keep-warm, which is not configuration: how many minutes after an idle session's last call it is pinged; at most
    /// how many times in one wait is each tier's.
    pub keep_warm_minutes: i64,
    /// The language everything the owner reads is written in.
    pub language: String,
    /// The machines works run on, by name.
    pub hosts: Vec<HostSlots>,
    /// The CLIs agents run in, by name.
    pub clis: Vec<CliArgs>,
    /// The gh commands agents may have the engine run, each as the words it starts with.
    pub gh_allow: Vec<String>,
    /// The most rows a query hands back, and how long it may run, in milliseconds.
    pub query_max_rows: i64,
    pub query_timeout_ms: i64,
    /// Each project, by name.
    pub projects: Vec<ProjectSettings>,
    /// The text of each rule a project takes in, by the rule's name.
    pub rules: BTreeMap<String, String>,
    /// The configuration commit they are read from.
    pub commit: String,
}

/// A tier: its name, the model it runs, what work it is for, and at most how many keep-warm pings one wait of a
/// session on its model has, which is not configuration: as many as cost no more than writing the session again.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Tier {
    pub name: String,
    pub model: String,
    pub desc: String,
    pub keep_warm_most: i64,
}

/// A machine works run on: its name, and how many works may run on it at once.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct HostSlots {
    pub name: String,
    pub slots: i64,
}

/// A CLI agents run in: its name, the command that starts it, and the arguments put before the ones the engine adds.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct CliArgs {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
}

/// A project's settings: those the owner changes on the page (the limits of its counters, the tags its observations
/// may carry, and whether its agents hand in flow feedback) and those only shown (where it is, how its design and its
/// CI are set up, its constitution and the rules it takes in).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ProjectSettings {
    pub name: String,
    /// The limit of each counter of the flow the project sets one for, in the flow's order.
    pub limits: Vec<CounterLimit>,
    /// The tags its observations may carry, in order.
    pub observation_tags: Vec<Tag>,
    /// Whether its agents hand in what was wrong with their rounds' cells.
    pub flow_feedback: bool,
    /// The GitHub repository, as owner/name.
    pub repo: String,
    /// Its clone on the engine's machine.
    pub checkout: String,
    /// The branch works merge into.
    pub main: String,
    /// The paths whose change is a change of design, and those that hold tests.
    pub design_paths: Vec<String>,
    pub test_paths: Vec<String>,
    /// The command that checks the design tree; none when there is none.
    pub design_check: Option<String>,
    /// The workflow each push of a work's branch runs, those that make up the full CI, and the checks a merge needs.
    pub ci_fast: String,
    pub ci_full: Vec<String>,
    pub merge_checks: Vec<String>,
    /// Its constitution, a path in its checkout; none when it has none.
    pub constitution: Option<String>,
    /// The rules it takes in, by name.
    pub rules: Vec<String>,
}

/// The limit a project sets on a counter of the flow, with what the counter counts.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct CounterLimit {
    pub counter: String,
    pub limit: i64,
    pub desc: String,
}

/// A change to the settings the owner makes on the page: each value given takes the place of the one there.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
pub struct SettingsChange {
    /// A model for each tier changed, by the tier's name.
    pub models: BTreeMap<String, String>,
    pub highest_level: Option<String>,
    pub architect: Option<String>,
    pub context_limit: Option<i64>,
    #[serde(default)]
    pub language: Option<String>,
    /// How many works may run at once on each machine changed, by its name.
    #[serde(default)]
    pub slots: BTreeMap<String, i64>,
    /// What changed of each project, by its name.
    #[serde(default)]
    pub projects: BTreeMap<String, ProjectChange>,
}

/// A change to a project's settings the owner makes on the page: each value given takes the place of the one there.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, TS)]
pub struct ProjectChange {
    /// The limit of each counter changed, by the counter's name.
    #[serde(default)]
    pub limits: BTreeMap<String, i64>,
    /// The tags its observations may carry, all of them in order, when they changed.
    #[serde(default)]
    pub observation_tags: Option<Vec<Tag>>,
    #[serde(default)]
    pub flow_feedback: Option<bool>,
}

/// The login accounts the agents of a CLI may run under: the CLI's configuration directories in the directory of the
/// one they run with now, each signed in once by hand.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct CliAccounts {
    /// The CLI's name in the configuration, which each account's directory is named by: the CLI's name alone, or it
    /// and a dash and the account's own name.
    pub cli: String,
    /// The command that starts it.
    pub command: String,
    /// The directory the accounts' directories are in.
    pub dir: String,
    /// How a new account's directory is signed in: the variable that names the directory to the CLI, and what to type
    /// once it runs.
    pub env: String,
    pub login: String,
    /// Each account, by its directory's name.
    pub accounts: Vec<Account>,
}

/// A login account the agents may run under: a configuration directory of the CLI.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Account {
    /// The directory's name, such as `claude-work`.
    pub name: String,
    /// The directory, whole.
    pub path: String,
    /// The account it is signed in as, by its email; none when it is not signed in.
    pub email: Option<String>,
    /// Whether the agents run under it now.
    pub used: bool,
}

/// The login account the owner has the agents of a CLI run under: the CLI's name and the account's directory's name.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
pub struct AccountChoice {
    pub cli: String,
    pub name: String,
}

/// An item's new tier, as the owner sets it on the item's page.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
pub struct NewLevel {
    pub level: String,
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

/// A project's architect, as its page and the panel beside its one-to-one show it: its works, its queue, what it asked
/// the owner, the matters it handled, the item changes its batches made, the policy it works by and the draft of a new
/// one, and its notebook. Its works are works of the architect's graph for the project, on no item.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Architect {
    /// The project whose architect it is.
    pub project: String,
    /// Every project of the configuration, each of which has its architect.
    pub projects: Vec<String>,
    /// Its works, newest first.
    pub works: Vec<ArchitectWork>,
    /// The matters it has not handled: those in its queue, in the order they came; then those an ask of the owner holds.
    pub queue: Vec<Matter>,
    /// What stops the project's new works from starting, oldest first.
    pub holds: Vec<DispatchHold>,
    /// What its works asked the owner, oldest first.
    pub asks: Vec<OwnerAsk>,
    /// The matters it handled, newest first, each with what became of it.
    pub handled: Vec<Handled>,
    /// The item changes its batches made or dropped, newest first.
    pub changes: Vec<ItemChange>,
    /// The policy in force for the project; none before the first takes effect.
    pub policy: Option<PolicyInForce>,
    /// The draft of a new policy that waits for the owner's approval: the latest the architect wrote, until a new
    /// policy takes effect; none otherwise.
    pub draft: Option<PolicyDraft>,
    /// Its notebook, by section in their order (what the project is now, the plan, what is unclear, anything else):
    /// each section as it stands, with the round and the cell that wrote it, and every version of it, newest first.
    pub notebook: Vec<NotebookSection>,
    /// The project's constitution, which the architect judges by with the policy: its path in the project's checkout,
    /// as the configuration names it, and its text there now, empty when it cannot be read; none when the project names
    /// none.
    pub constitution: Option<NamedText>,
}

/// One of the owner's conversations, as their list shows it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Conversation {
    /// Which: `architect:<project>` for the owner's one-to-one with a project's architect, `wf-agents` for wf-agents
    /// itself, `work:<id>` for a work's group of the owner, its engineer and its project's architect, and
    /// `projects:<project>|<project>...` for the architects of projects the configuration connects.
    pub key: String,
    /// architect, wf_agents, work or projects.
    pub kind: String,
    /// The project it is about; none for wf-agents' and a group of projects.
    pub project: Option<String>,
    /// For a group of projects: its projects, in order.
    pub projects: Vec<String>,
    /// For a work's group: the work, its item, the item's title and the tier the item is at, and whether the work is
    /// over (done or ended), which greys the group out in place.
    pub work: Option<i64>,
    pub item: Option<i64>,
    pub title: Option<String>,
    pub level: Option<String>,
    pub over: bool,
    /// How many things in it wait for the owner: the open blockers the owner holds, design cards not ruled, faults not
    /// read.
    pub waiting: i64,
    /// How many of its entries from anyone but the owner came after the latest the owner read, what waits aside.
    pub unread: i64,
    /// When its latest entry came; none while it has none.
    pub last_at: Option<i64>,
}

/// One entry of a conversation of the owner's: what `conversation_entries` gives, or what the engine writes from the
/// flow (a work's stage card, an architect's matter card, the policy taking effect).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ConversationEntry {
    /// What it is: message, notice, ask, interjection, words, talk (the owner's words added to a talk with the
    /// architect), question, report, reply, card, ruling, between; stage, matter or policy. The owner's answer to an
    /// ask is in the ask's entry, never one of its own.
    pub source: String,
    /// Its row in the table it comes from; for a stage card, its number in the work's stage cards, from 1.
    pub id: i64,
    /// Who said it and to whom: a line, the owner, the engine or wf-agents; the owner's words in a work's group are to
    /// the engineer, the architect or both; for between, the projects.
    pub sender: String,
    pub recipient: String,
    pub text: String,
    pub at: i64,
    /// The work and the round it came with, when it came with one.
    pub work: Option<i64>,
    pub round: Option<i64>,
    /// For an ask, the ask; for a design card and a ruling on it, the card; for what is or holds an
    /// architect's matter, or is an entry of the talk about it, the matter.
    pub ask: Option<i64>,
    pub design_card: Option<i64>,
    pub matter: Option<i64>,
    /// For a message or a notice: when the owner marked it read; none while it is unread.
    pub read_at: Option<i64>,
    /// Whether it waits for the owner: an open blocker the owner holds, a design card not ruled, a fault not read.
    pub waiting: bool,
    /// Whether the owner's words in it are not taken up yet.
    pub pending: bool,
    /// Whether the owner's words in it were never taken in: the work ended first.
    pub undelivered: bool,
    /// Whether it is an engineer's question the architect holds now, which the owner may answer in the group.
    pub answerable: bool,
    /// For a notice of a fault of wf-agents, to be fixed in wf-agents whichever project it came up in, its kind:
    /// cache_miss or flow_anomaly.
    pub fault: Option<String>,
    /// For a notice of cache misses: the misses it tells of, the first and those added to it since.
    pub misses: Option<Misses>,
    /// For a notice of a fault of wf-agents: the architects it was forwarded to, in the order it went; none for
    /// another entry.
    pub forwards: Vec<Forwarded>,
    /// The images the owner sent with the owner's words it holds, by their ids, in order; each is read at
    /// `/api/v1/attachments/<id>`. Empty for anything else.
    pub images: Vec<i64>,
    /// For an ask: the ask in full.
    pub ask_detail: Option<OwnerAsk>,
    /// For a stage card: the stage the work went through.
    pub stage: Option<StageCard>,
    /// For a matter card: where the matter stands with the architect.
    pub matter_card: Option<MatterProgress>,
    /// For the owner's words the architect they were said to took up, which have no matter card: where they stand
    /// with it, shown under their bubble.
    pub words_state: Option<WordsState>,
    /// For the policy taking effect: the policy in force.
    pub policy: Option<PolicyInForce>,
    /// For a version of the architect's plan: what changed and the plan whole.
    pub plan: Option<PlanChange>,
}

/// A version of the plan in an architect's notebook, as its bubble in the one-to-one says it: what changed from the
/// version before, as the first line the new one has that the one before had not, none for the first plan; the plan
/// whole; and the round of the architect's line that wrote it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct PlanChange {
    pub changed: Option<String>,
    pub plan: String,
    pub work: i64,
    pub round: i64,
}

/// A stage a work entered (research, design, implementation, testing and fixing, merge, watching main), as its bubble
/// in the work's group says it: posted when the work came into the stage, from the round that came in to the round
/// that left, and updated in place while the work is there. A stage the work comes back to is a stage of its own. A
/// stretch of the engine's own steps that sent the work straight back, such as the merge's judge finding the reviews'
/// findings open, is none of its own: its sending back is how the stage before it ended.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct StageCard {
    /// The stage, as the flow names it.
    pub stage: String,
    /// How many times the work was at this stage before.
    pub again: i64,
    /// Its rounds, by their numbers in the work: the first and the last.
    pub first: i64,
    pub last: i64,
    /// When its first round started, and when its last ended; none while the work is at it.
    pub started_at: i64,
    pub ended_at: Option<i64>,
    /// What its rounds cost.
    pub cost_usd: f64,
    /// Whether the work is at this stage now.
    pub current: bool,
    /// How it ended; none while the work is at it: done; the verdict of the gate or the tool it left by, as the row's
    /// name (stands, does_not_stand, merged, green); no_findings, its reviews found nothing; sent_back; or ended, the
    /// work ended before its end.
    pub outcome: Option<String>,
    /// Why it was sent back or the work ended, in the flow's or the engine's words, where its reviews' findings do not
    /// say it.
    pub why: Option<String>,
    /// For a stage sent back for what its reviews found: each review that found something, and how many it found.
    pub findings: Vec<ReviewFindings>,
    /// For a stage that merged the work: its pull request and the merge commit.
    pub pr: Option<i64>,
    pub commit: Option<String>,
    /// For a work taken over by a new work on its item: that work.
    pub taken_over_by: Option<i64>,
    /// What its rounds said they did, oldest first, the latest last.
    pub notes: Vec<StageNote>,
    /// What the engine told the owner of the work while it was at the stage, such as a limit passed or main broken by
    /// its merge, in words that do not name the work; its pauses and stops are how the work stands, which its group
    /// says where the work is, and not said again here.
    pub events: Vec<String>,
    /// What the agents' rounds in it handed back that the flow shows on the item, oldest first: for the design, the
    /// approaches compared, the one chosen, the design and the acceptance.
    pub shown: Vec<Shown>,
    /// The design cards its rounds handed in, the one that waits for the owner's ruling among them.
    pub design_cards: Vec<DesignCard>,
}

/// What a round said it did, its progress note: the round, the line whose round it was, and the note.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct StageNote {
    pub round: i64,
    pub line: Option<String>,
    pub text: String,
}

/// What a review found: the review, by its cell's title, its round, and how many findings it left open.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ReviewFindings {
    pub review: String,
    pub round: i64,
    pub findings: i64,
}

/// Where a matter stands with its architect, as the engine writes it from the flow: queued, held by an ask of the
/// owner, worked on at a cell, handled with what became of it, or withdrawn with why.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct MatterProgress {
    pub matter: i64,
    pub kind: String,
    pub detail: Option<String>,
    /// What it is, in one sentence, as the engine wrote it.
    pub what: String,
    /// queued, held, working, done or withdrawn.
    pub state: String,
    /// While it is worked on: the cell the architect is at, with its title, and the round of the architect's work
    /// there.
    pub cell: Option<String>,
    pub cell_title: Option<String>,
    pub architect_work: Option<i64>,
    pub round: Option<i64>,
    /// The work it is about, whose group holds it in full, and that work's item.
    pub work: Option<i64>,
    pub item: Option<i64>,
    /// Once handled: what became of it.
    pub handled: Option<Handled>,
    /// Once withdrawn: why.
    pub withdrawn: Option<String>,
    /// The ask of the owner that holds it, or held it.
    pub asked: Option<i64>,
    /// For a place to set straight: the place, as the check of what runs found it (where, what is wrong there, the
    /// steps to set it straight); none for a matter of another kind.
    pub fix: Option<Value>,
    /// What its path changed that the owner reads by name, listed in its bubble: the observations it recorded, merged
    /// a report into or closed, and the items it made, cancelled or moved in the queue.
    pub made: Vec<MadeThing>,
}

/// Something an architect's matter changed, by its kind: an observation it recorded (new), merged a report into
/// (merge) or closed (close), or an item it made (new), cancelled or moved in the queue (reordered); its number, and
/// its title, for an observation recorded before observations had titles what it says; for an item moved, where to
/// (`place`: first, last, before, after or under) and the item it went before, after or under (`next_to`).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct MadeThing {
    pub kind: String,
    pub change: String,
    pub id: i64,
    pub title: String,
    pub place: Option<String>,
    pub next_to: Option<i64>,
}

/// Where the owner's words stand with the architect they were said to, once it took them up, as one line under their
/// bubble says it, in place of a matter card that would repeat them.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct WordsState {
    /// replying while the architect has them in hand; replied once handled; asked while a question of the architect's
    /// to the owner holds them; withdrawn. (waits and held, a talk's wait for the owner's reply, a flow version before
    /// had.)
    pub state: String,
    /// The project of the architect they were said to; while it has them in hand, its work, the work's latest round
    /// and the cell it is at, where its chart shows them handled.
    pub project: String,
    pub architect_work: Option<i64>,
    pub round: Option<i64>,
    pub cell: Option<String>,
}

/// A conversation in full: its entries in time order; what waits for the owner in it, the open blockers the owner
/// holds with their forms (the asks, which show where they were asked, and the design cards whose ruling a work waits
/// for, pinned at its top); in the one-to-one with an architect, the questions it pinned for the owner's reply, at its
/// bottom, and the one it waits on now; and how far the owner has read it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ConversationDetail {
    pub key: String,
    pub entries: Vec<ConversationEntry>,
    /// The asks that wait for the owner where they were asked; the questions pinned are not among them.
    pub asks: Vec<OwnerAsk>,
    pub design_cards: Vec<DesignCard>,
    /// In the one-to-one with an architect: the questions it pinned for the owner's reply, newest first, each an ask of
    /// the architect's that waits and parks a matter still open, as the engine reads them; none in another
    /// conversation. Each is an entry too, where it was asked.
    pub questions: Vec<OwnerAsk>,
    /// In the one-to-one with an architect: the question it waits for the owner's reply to now at its look at the queue
    /// (`Q0`'s row `waits`), as the engine works it out: the newest pinned, put less than the installation's wait for
    /// the owner's reply ago, with nothing said by the owner since; none otherwise.
    pub waits_on: Option<i64>,
    /// The time of the latest entry the owner read; none before the first.
    pub read_to: Option<i64>,
    /// For wf-agents' own conversation: where its faults can go; none for another.
    pub forwarding: Option<Forwarding>,
}

/// What the owner says in a work's group: to its engineer, to its project's architect, or to both. Saying it to the
/// architect alone, the engineer is given it as a word that needs no answer; to the engineer alone, the architect is
/// given it at its next pick. With `question`, it answers that question of the engineer's, which the architect holds:
/// the answer is the reply, and the architect is told.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
pub struct WorkSaid {
    pub text: String,
    /// engineer (the default), architect or both.
    #[serde(default)]
    #[ts(optional)]
    pub to: Option<String>,
    /// The architect's matter that holds the question the words answer.
    #[serde(default)]
    #[ts(optional)]
    pub question: Option<i64>,
    /// The images sent with the words, in the order the owner added them; words may be images alone.
    #[serde(default)]
    pub images: Vec<ImageSent>,
}

/// What the owner says in a group of projects: to one of their architects, or to every one.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
pub struct ProjectsSaid {
    /// The group, by its key.
    pub key: String,
    pub text: String,
    /// The project whose architect it is for; none for every architect of the group.
    #[serde(default)]
    #[ts(optional)]
    pub to: Option<String>,
    /// The images sent with the words, in the order the owner added them; words may be images alone.
    #[serde(default)]
    pub images: Vec<ImageSent>,
}

/// What the owner says to a project's architect in their one-to-one: the words, the images sent with them, and the
/// question of the architect's they reply to, when the box replies to one.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
pub struct ArchitectSaid {
    pub text: String,
    /// The item it is about, if any.
    #[serde(default)]
    #[ts(optional)]
    pub item: Option<i64>,
    /// The question pinned in the one-to-one that the words reply to.
    #[serde(default)]
    #[ts(optional)]
    pub reply_to: Option<i64>,
    /// The images sent with the words, in the order the owner added them; words may be images alone.
    #[serde(default)]
    pub images: Vec<ImageSent>,
}

/// How far the owner has read a conversation: the time of the latest entry the page showed.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
pub struct ReadTo {
    pub key: String,
    pub at: i64,
}

/// One work of an architect.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ArchitectWork {
    pub work: WorkSummary,
    /// The matter it handles now: the one its latest pick chose, until that matter is handled or held by an ask; none
    /// when it handles none, or is over.
    pub handling: Option<i64>,
}

/// A matter for an architect that it has not handled.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Matter {
    pub id: i64,
    /// Its kind, which decides its path: set_straight, engineer_question, report, design_card, failure, owner_words,
    /// other_architect or slots_free.
    pub kind: String,
    /// What more its kind says, such as a side report's kind; none for an engineer's question, which carries no kind.
    pub detail: Option<String>,
    /// What it is, in one sentence, as the engine wrote it.
    pub what: String,
    /// Who raised it: `work`, a round of a work; `architect`, a round of an architect's; `owner`, the owner's words;
    /// `engine`, the engine.
    pub source: String,
    /// The item it concerns, and the item's title.
    pub item: Option<i64>,
    pub title: Option<String>,
    /// The round that raised it, by its work and its number in the work.
    pub work: Option<i64>,
    pub round: Option<i64>,
    /// When it came.
    pub at: i64,
    /// The ask of the owner that holds it until answered, and the cell it goes back to with the answer.
    pub asked: Option<i64>,
    pub resume_cell: Option<String>,
    /// Whether the owner answered that ask, so that it is back in the queue, for the cell that asked.
    pub answered: bool,
    /// For a matter in the queue: who waits for it (`agent`, an engineer stopped for its question; `owner`; or
    /// `none`), and how long since it came, in milliseconds.
    pub waits: Option<String>,
    pub waited_ms: Option<i64>,
    /// Whether the report it holds was marked urgent.
    pub urgent: bool,
    /// For a place to set straight: the place, as the check of what runs found it (where, what is wrong there, the
    /// steps to set it straight); none for a matter of another kind.
    pub fix: Option<Value>,
    /// For a design card to look at: how the card stands.
    pub card: Option<MatterCard>,
}

/// How the design card a matter is about stands: its number; open, ruled or replaced; its rulings, the owner's and the
/// architect's alike; and the card that replaced it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct MatterCard {
    pub id: i64,
    pub status: String,
    pub rulings: Value,
    pub replaced_by: Option<i64>,
}

/// What stops a project's new works from starting: `architect`, its architect's own hold; `set_straight`, while its
/// architect sets it straight (`policy`, while a new policy was set, a flow version before); since when; and for the
/// architect's own hold, why, as it said it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct DispatchHold {
    pub reason: String,
    pub since: i64,
    pub why: Option<String>,
}

/// A matter an architect handled, and what became of it, from what was done on its path rather than what was said of
/// it: each kind of outcome it had.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Handled {
    pub matter: i64,
    pub kind: String,
    pub detail: Option<String>,
    pub what: String,
    /// The round that closed it: its work, its number and its cell, and when it ended.
    pub work: i64,
    pub round: i64,
    pub cell: String,
    pub at: Option<i64>,
    /// The reply taken back to the work that asked, and that work.
    pub reply: Option<String>,
    pub reply_to: Option<i64>,
    /// How many messages its path told the owner.
    pub told: i64,
    /// The item changes it collected: made, dropped, and still waiting for the review of their batch.
    pub made: i64,
    pub dropped: i64,
    pub collected: i64,
    /// Each change its path made to the observations.
    pub observations: Vec<ObservationChange>,
    /// What its path did at once: each action's kind, with the item it was about.
    pub actions: Vec<HandledAction>,
    /// The projects whose architects its path wrote to.
    pub messages: Vec<String>,
    /// The ask of the owner its path put about it.
    pub ask: Option<i64>,
    /// When nothing else came of it: why, as the cell that closed it said.
    pub why: Option<String>,
    /// Whether nothing came of it: no reply, nothing told the owner, no item change, no observation, nothing done at
    /// once, no message to another architect and no ask of the owner.
    pub nothing: bool,
}

/// A change a matter's path made to the observations: its kind (new, merge or close), and the observation a merge
/// reported once more or a close closed; none for a new one, whose number its source's matter finds.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ObservationChange {
    pub kind: String,
    pub observation: Option<i64>,
}

/// Something a matter's path did at once: its kind (pause_work, resume_work, start_work_over, hold_starts,
/// release_starts, interject), with the item it was about.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct HandledAction {
    pub kind: String,
    pub item: Option<i64>,
}

/// An item change an architect's batch made or dropped, with the matter it came from.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ItemChange {
    pub id: i64,
    /// The round that handed it in: its work and its number.
    pub work: i64,
    pub round: i64,
    /// The cell of that round.
    pub cell: String,
    pub matter: i64,
    /// The change as handed in, with its reason.
    pub change: Value,
    /// made, or dropped when the review still found it wrong at its third look.
    pub state: String,
    /// What it made, such as the item it created; for one dropped, what the review found.
    pub outcome: Option<Value>,
}

/// One of a project's observations: something worth doing that no item does yet.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Observation {
    pub id: i64,
    /// What is wrong, as a short phrase; none for one recorded before observations had titles.
    pub title: Option<String>,
    /// What it is, why it is worth doing, and what happens if it is not done.
    pub what: String,
    /// Its tags, from the project's set.
    pub tags: Vec<String>,
    /// Who found it, in what situation, and the matter it came from.
    pub source: String,
    pub times_reported: i64,
    /// What it refers to: PRs, branches, design cards, the owner's words.
    pub refs: Vec<String>,
    /// The item that took it in, which closes it once its work merges.
    pub folded_into: Option<i64>,
    /// Why it was closed (outdated, folded, owner or other), in a word and in a sentence; none while it is open.
    pub closed: Option<String>,
    pub closed_why: Option<String>,
    /// When it was recorded, and last changed.
    pub created: Option<i64>,
    pub updated: Option<i64>,
}

/// A project's observations, as the Observations view shows them: the tags they may carry, in the project's order, each
/// with what it covers, and the observations, the open ones first, then those an item took in or that were closed,
/// each newest first.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ProjectObservations {
    pub project: String,
    pub tags: Vec<Tag>,
    pub observations: Vec<Observation>,
}

/// A tag a project's observations may carry, with what it covers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Tag {
    pub name: String,
    pub desc: String,
}

/// What the owner sends to close an observation: why.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
pub struct CloseObservation {
    pub why: String,
}

/// The policy in force: its text, and the round of the architect's work in which it took effect, after the owner
/// approved it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct PolicyInForce {
    pub work: i64,
    pub round: i64,
    /// The cell of the round that wrote it.
    pub cell: String,
    pub at: Option<i64>,
    pub policy: Value,
}

/// The draft of a new policy that waits for the owner's approval: its text, and the round of the architect's work that
/// wrote it (cell `P2`).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct PolicyDraft {
    pub work: i64,
    pub round: i64,
    pub at: Option<i64>,
    pub policy: Value,
}

/// A section of a project's architect's notebook: which (`project_now`, `plan`, `unclear` or `other`), what it says
/// now, none before a round wrote it, and every version of it, newest first.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct NotebookSection {
    pub section: String,
    pub now: Option<NotebookVersion>,
    pub versions: Vec<NotebookVersion>,
}

/// A version of a section of an architect's notebook: its whole text, and the round of the architect's line that wrote
/// it, by its work and its number in the work, with its cell and when it ended.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct NotebookVersion {
    pub text: String,
    pub work: i64,
    pub round: i64,
    pub cell: String,
    pub at: Option<i64>,
}

/// How the whole system stands now, and what it did over a period of whole weeks, the week under way the last: what
/// the System view shows.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct System {
    /// When the period starts, on a Monday at midnight in the owner's time zone; none for all time.
    pub from: Option<i64>,
    /// When it was read, which ends the period.
    pub at: i64,
    pub now: SystemNow,
    /// The period's weeks, oldest first.
    pub weeks: Vec<Week>,
    pub output: SystemOutput,
    pub spend: SystemSpend,
    pub waste: SystemWaste,
    /// What went wrong in the period, the latest first.
    pub anomalies: Vec<Anomaly>,
}

/// What may need the owner now besides the works: each project's architect, and what waits for the owner.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct SystemNow {
    pub architects: Vec<ArchitectNow>,
    /// The asks of the owner not answered, the design cards not ruled on and the faults of wf-agents not read.
    pub asks: Waiting,
    pub design_cards: Waiting,
    pub faults: Waiting,
}

/// A project's architect now.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ArchitectNow {
    pub project: String,
    /// Its work not over, by its id, while one runs; none while none runs.
    pub work: Option<i64>,
    /// Whether that work is paused.
    pub paused: bool,
    /// The matters in its queue, and when the oldest of them came.
    pub queue: i64,
    pub oldest: Option<i64>,
}

/// Things that wait for the owner: how many, and when the oldest of them came.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Waiting {
    pub n: i64,
    pub oldest: Option<i64>,
}

/// One week of the period.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Week {
    /// When it starts: Monday at midnight in the owner's time zone.
    pub start: i64,
    /// The items merged, made and cancelled in it.
    pub merged: i64,
    pub made: i64,
    pub cancelled: i64,
    /// The pull requests open at its end, or now for the week under way.
    pub open_prs: i64,
    /// What the engineers' works and the architects' works spent in it, their keep-warm pings included, in US dollars.
    pub engineers_usd: f64,
    pub architects_usd: f64,
}

/// What got done in the period.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct SystemOutput {
    /// The items merged in the period, the latest first.
    pub merged: Vec<Merged>,
    /// The items made, and those cancelled, in the period, the latest first.
    pub made: Vec<ItemAt>,
    pub cancelled: Vec<ItemAt>,
    /// The pull requests open now, the oldest first.
    pub open_prs: Vec<OpenPr>,
}

/// An item merged, by the work that merged it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Merged {
    pub item: i64,
    pub title: String,
    pub work: i64,
    /// When the work's first round started, and when it merged.
    pub started_at: i64,
    pub merged_at: i64,
    /// What the work cost, its keep-warm pings included, in US dollars.
    pub cost_usd: f64,
}

/// An item made or cancelled, and when; for one cancelled, why.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ItemAt {
    pub item: i64,
    pub title: String,
    pub at: i64,
    pub why: Option<String>,
}

/// A pull request open now, by the work it is of.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct OpenPr {
    pub pr: i64,
    pub project: Option<String>,
    pub work: i64,
    pub item: Option<i64>,
    pub title: Option<String>,
    /// When a work first recorded it.
    pub since: i64,
}

/// Where the money went in the period.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct SystemSpend {
    /// What the rounds and the keep-warm pings cost, by the line of their session and the model they ran on.
    pub by_line_model: Vec<LineSpend>,
    /// What each item's works cost, the costliest first.
    pub by_item: Vec<ItemSpend>,
    /// The ten cells whose rounds cost most, the costliest first.
    pub cells: Vec<CellSpend>,
}

/// What the rounds and the keep-warm pings of one line on one model cost.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct LineSpend {
    pub line: String,
    pub model: String,
    pub rounds: i64,
    pub pings: i64,
    /// What they cost, as the command lines reported it, in US dollars.
    pub cost_usd: f64,
}

/// What an item's works cost, and how many works and rounds they took.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ItemSpend {
    pub item: i64,
    pub title: String,
    pub works: i64,
    pub rounds: i64,
    pub cost_usd: f64,
}

/// What a cell's rounds cost: the cell by its code now, with its title.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct CellSpend {
    pub graph: String,
    pub cell: String,
    pub title: Option<String>,
    pub rounds: i64,
    pub cost_usd: f64,
}

/// Where the period spent for nothing, and how well the prompt cache did.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct SystemWaste {
    /// The input tokens of the agents' rounds: read from the prompt cache, written to it, and given without it.
    pub read: i64,
    pub written: i64,
    pub input: i64,
    /// The rounds that missed their session's prompt cache as a fault of wf-agents, the tokens they wrote to it again,
    /// and what that cost beyond reading them, in US dollars; a miss the engine expects is no fault.
    pub misses: i64,
    pub missed: i64,
    pub miss_usd: f64,
    /// The rounds whose return was sent back, and those whose run failed or was cut off, by work, the costliest first.
    pub sent_back: Vec<WorkCount>,
    pub failed: Vec<WorkCount>,
    /// How long no agent's round could start, a failed run's cause stopping every work: by that cause, and in all.
    pub halts: Vec<HaltTime>,
    pub halted_ms: i64,
    /// The keep-warm pings, by work, the costliest first.
    pub pings: Vec<WorkCount>,
}

/// How many rounds or pings of one work, and what they cost.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct WorkCount {
    pub work: i64,
    pub graph: String,
    /// Its item and the item's title, and its project.
    pub item: Option<i64>,
    pub title: Option<String>,
    pub project: Option<String>,
    pub n: i64,
    pub cost_usd: f64,
}

/// How long one cause of failed runs stopped every work.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct HaltTime {
    /// The kind of failed run, as the flow names it: `usage_limit`, `offline` or `cannot_run`.
    pub kind: String,
    pub ms: i64,
    /// The failed runs that found it.
    pub runs: i64,
}

/// Something that went wrong, in a work.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Anomaly {
    pub work: i64,
    pub graph: String,
    pub item: Option<i64>,
    pub title: Option<String>,
    pub project: Option<String>,
    /// The round it came of, by its number in the work; none when the engine stopped the work, which no round records.
    pub round: Option<i64>,
    pub at: i64,
    pub what: AnomalyKind,
}

/// What went wrong, each kind of its own.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnomalyKind {
    /// A row counting a flow anomaly: the way on an engineer chose did not fit the situation.
    FlowAnomaly { row: String },
    /// A counter of the work went past the limit its project's configuration sets.
    Counter { counter: String, limit: i64 },
    /// The engine stopped the work, saying why.
    Stopped { why: String },
    /// A tool failed after the engine's retries, at the cell that ran it, and the architect was asked.
    ToolFailed { cell: String },
    /// Main went red after the work merged: how, as the watch of main tells it (`environment_changed`,
    /// `broken_by_merge` or `known_red`), and the checks red.
    MainRed { how: String, checks: Vec<String> },
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

/// The lines of a graph of the flow built into this program: a compiled flow version keeps the lines, not what they
/// are.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct GraphLines {
    pub graph: String,
    pub lines: Vec<LineView>,
}

/// A line, the agent whose cells go on in one session.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct LineView {
    pub name: String,
    /// How its session is kept (when it starts and which cells continue it).
    pub desc: String,
    /// The colour its role is drawn in wherever the page shows it: its cells on a chart, its avatar and its bubbles.
    pub colour: flow::format::Colour,
}
