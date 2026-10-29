// The types of the owner's interface, generated from the Rust types in crates/api and crates/flow. Do not edit:
// change the Rust types and run `WF_AGENTS_GENERATE=1 cargo test -p api --test typescript` to write this file again.

export type JsonValue = number | string | boolean | Array<JsonValue> | { [key in string]: JsonValue } | null;

/**
 * What the owner sends with a work's or the architect's Start and Step: whether its next round resumes the session
 * its line goes on in (the default) or starts a new one.
 */
export type RunAsk = { new_session: boolean, };

/**
 * What the owner sends with the switch that keeps waiting sessions warm while every work is paused.
 */
export type KeepWarm = { on: boolean, };

/**
 * The round of a work the owner asked for, and what it came to.
 */
export type StepTaken = { 
/**
 * When the round ended.
 */
at: number, 
/**
 * The work the owner asked a round of, and the work the round was of: the new work that took the asked work's
 * item over, when the asked work was on an older flow version.
 */
asked: number, work: number, 
/**
 * The rounds of the work it started or ended, by their numbers in the work, and the cell of the last of them.
 */
rounds: Array<number>, cell: string | null, };

/**
 * The engine as it runs now, and what the page needs of the installation.
 */
export type Status = { 
/**
 * Whether the owner paused every work, as the engine starts: no work goes on by itself and no new work starts,
 * but for the works the owner started since; a work's round the owner asks for runs.
 */
paused: boolean, 
/**
 * Whether the sessions of waiting works are kept warm while every work is paused: the owner's switch beside the
 * pause, on at first and kept across restarts.
 */
keep_warm: boolean, 
/**
 * The works the owner started since every work was last paused: each goes on whatever the global state.
 */
started: Array<number>, 
/**
 * The works the owner started that are still paused, until their machine has a slot free: the engine then lifts
 * their pause.
 */
releasing: Array<number>, 
/**
 * The projects whose architect the owner started since every work was last paused: its wakes go on whatever the
 * global state.
 */
architects_started: Array<string>, 
/**
 * The work the engine is taking a step of now: its round runs. While every work is paused it is the last round
 * before the pause, unless the owner asked for it.
 */
running: number | null, 
/**
 * The work whose round the owner asked for, until that round has ended.
 */
stepping: number | null, 
/**
 * The last round the owner asked for since the engine started.
 */
last_step: StepTaken | null, 
/**
 * The slots of every machine together, and how many of them works hold.
 */
slots: number, slots_used: number, 
/**
 * The flow version new works start on.
 */
flow_version: number, 
/**
 * The configuration commit the engine runs with.
 */
config_commit: string, 
/**
 * Each project's GitHub repository, as owner/name, by the project's name: the page links a work's pull request.
 */
repos: { [key in string]: string }, 
/**
 * When the engine started.
 */
started_at: number, 
/**
 * The version of wf-agents that runs.
 */
version: string, 
/**
 * While a failed run's cause stops every work alike, such as a usage limit or no network: no new round starts
 * until it is over.
 */
halt: Halt | null, };

/**
 * Why the engine starts no new round for any work, and until when.
 */
export type Halt = { 
/**
 * The kind of failed run, as the flow names it: `usage_limit`, `offline` or `cannot_run`.
 */
kind: string, 
/**
 * Until when: when the usage limit resets, or when the engine next lets a round find out whether the cause has
 * passed.
 */
until: number, 
/**
 * What the command line reported.
 */
why: string, };

/**
 * What changed, as the event stream gives it. An event names what to read again and never carries that state,
 * except the engine's status, which lives only in the engine. The stream gives each event an id; a client that
 * reconnects with the last id it saw is given what it missed.
 */
export type Event = { "kind": "ready" } | { "kind": "resync" } | { "kind": "status", status: Status, } | { "kind": "work", work: number, } | { "kind": "round", work: number, round: number, } | { "kind": "design_card", design_card: number, work: number | null, } | { "kind": "ask", ask: number, work: number | null, } | { "kind": "owner_message", message: number, work: number | null, } | { "kind": "item", item: number | null, } | { "kind": "flow", version: number, } | { "kind": "architect" } | { "kind": "activity", work: number, round: number, what: string, at: number, };

/**
 * Where a work stands.
 */
export type WorkState = "running" | "ready" | "asking" | "waiting" | "paused" | "done" | "ended";

/**
 * The round of a work that is running.
 */
export type RunningRound = { 
/**
 * Its number in the work.
 */
n: number, cell: string | null, started_at: number, 
/**
 * Whether a tool it runs is under way: started, as tool_runs records it, and not ended.
 */
tool: boolean, };

/**
 * Who paused a work.
 */
export type PausedBy = "owner" | "engine" | "architect";

/**
 * What a work waits for: once it comes, the work goes on by itself, unless it is paused.
 */
export type Waits = { "kind": "answer", ask: number, } | { "kind": "ruling", design_card: number, } | { "kind": "items", items: Array<number>, } | { "kind": "row", cell: string, row: string, };

/**
 * A work as a list shows it: what it works on, where it stands and what it has cost.
 */
export type WorkSummary = { id: number, 
/**
 * The item worked on, its title and its project; for a wake of an architect, no item and the architect's project.
 */
item: number | null, title: string | null, project: string | null, 
/**
 * The graph it runs, of the flow version it is pinned to.
 */
graph: string, flow_version: number, 
/**
 * The cell it is at, with the cell's title and the stage the work is at, in its flow version.
 */
at: string, at_title: string | null, stage: string | null, state: WorkState, 
/**
 * Why it was paused, and who paused it (for a work the architect stopped, the architect's reason); why it ended
 * before its end.
 */
paused: string | null, paused_by: PausedBy | null, ended: string | null, 
/**
 * What it waits for before it goes on by itself.
 */
waits: Waits | null, 
/**
 * The ask of the owner, put at the cell it is at, whose answer came: its next step takes the answer in.
 */
answered: number | null, branch: string | null, pr: number | null, 
/**
 * When its first round started, and when its last round started or ended.
 */
started_at: number | null, last_at: number | null, rounds: number, 
/**
 * Rounds whose return was sent back, and rounds whose run failed or was cut off.
 */
sent_back: number, failed: number, running: RunningRound | null, 
/**
 * What its rounds and the keep-warm pings of its sessions cost, as the command lines reported it, in US dollars.
 */
cost_usd: number, 
/**
 * Its asks of the owner still waiting for an answer.
 */
open_asks: number, 
/**
 * Its design cards still waiting for a ruling.
 */
open_design_cards: number, };

/**
 * How a round ended.
 */
export type RoundOutcome = "running" | "took" | "sent_back" | "failed" | "interrupted" | "ended";

/**
 * A round as a work's timeline shows it.
 */
export type RoundSummary = { 
/**
 * Its number in its work.
 */
n: number, 
/**
 * Its cell; none for a round that only answered an interjection.
 */
cell: string | null, 
/**
 * The line whose session it ran in; none for the engine's own steps, tool cells and the owner.
 */
line: string | null, outcome: RoundOutcome, 
/**
 * The row its return took.
 */
transition: string | null, 
/**
 * Why it was sent back, failed or was cut off.
 */
why: string | null, started_at: number, ended_at: number | null, model: string | null, cost_usd: number | null, compactions: number, tool_runs: number, 
/**
 * What the round said it did.
 */
progress: string | null, 
/**
 * For a gate the engine answered itself, since the project does not have what it needs: that need.
 */
not_asked: string | null, 
/**
 * For an agent's round, where it spent its tokens, from its usage record.
 */
tokens: RoundTokens | null, 
/**
 * The session it ran in, by its row; none for the engine's own steps, tool cells and the owner.
 */
session: number | null, 
/**
 * When it started a new session: why, as the session manager recorded it: line, the line's first session in the
 * work (or a cell on no line, whose session is its own); cell, a cell the line starts a new session at; limit, the
 * line's session before was past the context limit.
 */
new_session: string | null, };

/**
 * Where an agent's round spent its tokens, as its usage record gives them, and how large its context was, which
 * the page divides the round's cost by.
 */
export type RoundTokens = { 
/**
 * The command line's steps in the round.
 */
steps: number, 
/**
 * Tokens of input not read from the cache.
 */
input: number, 
/**
 * Tokens read again from the cache.
 */
read: number, 
/**
 * Tokens newly written to the cache, kept five minutes and kept an hour.
 */
written_5m: number, written_1h: number, 
/**
 * Tokens of output, and of them thinking.
 */
output: number, thinking: number, 
/**
 * The context at the round's end: its last call's prompt; none when the record does not give the last call.
 */
context_end: number | null, 
/**
 * The size of the context window, as the command line reported it for the models of the session.
 */
window: number | null, 
/**
 * The context at the round's start: the end of the latest round before it in the same session, since the usage
 * record keeps only the round's last call; none for a round that started its session.
 */
context_before: number | null, 
/**
 * The level the round ran at, as its call record says it; none for a round recorded before it said.
 */
level: string | null, 
/**
 * The reasoning effort its command line gave it.
 */
effort: string | null, 
/**
 * The models besides the round's own whose tokens grew during the round, by the command line's usage per model,
 * which counts the whole session: the subagents that ran on other models.
 */
others: Array<string>, };

/**
 * A design card, as the round that handed it in wrote it, with its rulings.
 */
export type DesignCard = { id: number, 
/**
 * The round that handed it in: its number in the work.
 */
round: number, work: number, item: number | null, 
/**
 * open, ruled or replaced.
 */
status: string, body: JsonValue, rulings: JsonValue, replaced_by: number | null, 
/**
 * For a design card a newer one replaced: that one, as the engine knows it.
 */
replaced: Replacement | null, 
/**
 * The cell of the round that handed it in, the cell's title in the work's flow version, and when that round
 * ended.
 */
cell: string | null, title: string | null, at: number | null, 
/**
 * The design card this one replaced, if any, and what this one changed from it and why, in a sentence or two of
 * its agent's.
 */
replaces: number | null, changed: string | null, };

/**
 * The design card that replaced another: the round that handed it in, at which cell and when.
 */
export type Replacement = { design_card: number, 
/**
 * The round that handed it in: its number in the work.
 */
round: number, work: number, 
/**
 * The cell of that round, and the cell's title in the work's flow version.
 */
cell: string | null, title: string | null, 
/**
 * When that round ended.
 */
at: number | null, 
/**
 * What the newer design card changed and why, in a sentence or two of its agent's; none for one handed in before
 * the flow asked for it.
 */
changed: string | null, };

/**
 * An ask of the owner: what is asked, and the owner's answer.
 */
export type OwnerAsk = { id: number, 
/**
 * The round that put it to the owner: its number in the work.
 */
round: number, work: number, item: number | null, body: JsonValue, 
/**
 * The kind of each point it asks about, in order, with the kind's group.
 */
kinds: Array<AskKind>, 
/**
 * None while it waits.
 */
answer: JsonValue | null, };

/**
 * The kind of a point an ask is about, with the group of kinds it is in, so that the owner sees at a glance what
 * sort of question it is.
 */
export type AskKind = { kind: string, 
/**
 * The group the flow built into this program puts the kind in; none for a kind it does not have, such as one
 * an earlier flow version had.
 */
group: string | null, };

/**
 * What an agent told the owner in a return, or the engine of its own accord, needing no answer.
 */
export type OwnerMessage = { id: number, 
/**
 * Whether the engine wrote it: then `id` is the engine's notice's, and it has no round.
 */
engine: boolean, 
/**
 * The round whose return holds it, by its number in the work, its cell and line, and when it ended; for the
 * engine's notice, when it was written.
 */
round: number | null, cell: string | null, line: string | null, at: number | null, work: number | null, item: number | null, title: string | null, text: string, 
/**
 * When the owner marked it read; none while it is unread.
 */
read_at: number | null, 
/**
 * For the engine's notice of a new ask of the owner: that ask, and the kinds of the points it asks about.
 */
ask: number | null, ask_kinds: Array<AskKind>, 
/**
 * For one that came of a wake of an architect, told by one of its rounds or the engine's notice of one: the
 * architect's project; none otherwise.
 */
architect: string | null, };

/**
 * The cache misses a notice tells of: how many, the tokens they wrote to the cache again, what that cost beyond
 * reading the cache, and when the latest came.
 */
export type Misses = { count: number, written: number, cost_usd: number, last_at: number | null, };

/**
 * One of the owner's conversations, by its subject, as their list shows it.
 */
export type Conversation = { 
/**
 * Which: `work:<id>` for an engineer's work, `architect:<project>` for a project's architect,
 * `architects:<project>|<project>` for two projects' architects, `wf-agents` for wf-agents itself.
 */
key: string, 
/**
 * The project it is about; none for one between two projects' architects, and for wf-agents'.
 */
project: string | null, 
/**
 * For a work's: the work, its item and the item's title.
 */
work: number | null, item: number | null, title: string | null, 
/**
 * How many things in it wait for the owner: asks not answered, design cards not ruled, faults not read.
 */
waiting: number, 
/**
 * How many of the messages and notices in it the owner has not read, faults aside.
 */
unread: number, 
/**
 * When its latest entry came.
 */
last_at: number, };

/**
 * One entry of a conversation of the owner's, as `conversation_entries` gives it.
 */
export type ConversationEntry = { 
/**
 * What it is: message, notice, ask, answer, interjection, words, question, reply, advice_ask, advice, card,
 * ruling or between.
 */
source: string, 
/**
 * Its row in the table it comes from; for advice_ask and advice, the round's number in its work.
 */
id: number, 
/**
 * Who said it and to whom: a line, the owner, the engine or wf-agents; for between, the projects.
 */
sender: string, recipient: string, text: string, at: number, 
/**
 * The work and the round it came with, when it came with one.
 */
work: number | null, round: number | null, 
/**
 * For an ask and its answer, the ask; for a design card and a ruling on it, the card.
 */
ask: number | null, design_card: number | null, 
/**
 * For a message or a notice: when the owner marked it read; none while it is unread.
 */
read_at: number | null, 
/**
 * Whether it waits for the owner: an ask not answered, a design card not ruled, a fault not read.
 */
waiting: boolean, 
/**
 * Whether the owner's words in it are not taken up yet.
 */
pending: boolean, 
/**
 * Whether it passes between agents, not to or from the owner.
 */
agents: boolean, 
/**
 * For a notice of a fault of wf-agents, to be fixed in wf-agents whichever project it came up in, its kind:
 * cache_miss or flow_anomaly.
 */
fault: string | null, 
/**
 * For a notice of cache misses: the misses it tells of, the first and those added to it since.
 */
misses: Misses | null, };

/**
 * A conversation in full: its entries in time order, and what waits for the owner in it with its form: the asks not
 * answered and the design cards not ruled.
 */
export type ConversationDetail = { key: string, entries: Array<ConversationEntry>, asks: Array<OwnerAsk>, design_cards: Array<DesignCard>, };

/**
 * What the owner says in a conversation about a work: to its engineer, or another of its lines, as an interjection;
 * or to the architect of its project, as the owner's words naming the work.
 */
export type WorkSaid = { text: string, 
/**
 * The line it is for, `engineer` unless named; `architect` for the project's architect.
 */
to?: string, };

/**
 * Which messages to the owner the owner marks read: those named (agents' messages by `ids`, the engine's notices by
 * `notices`), or every one unread.
 */
export type MarkRead = { ids: Array<number>, notices: Array<number>, all: boolean, };

/**
 * The owner's answer to an ask of the owner, as the page and the command line hand it in.
 */
export type Answer = { text: string, 
/**
 * For what the architect is asked: why it was decided this way.
 */
why?: string, 
/**
 * For what the architect is asked: the rest of the architect's reply (children, rest, waits).
 */
reply?: JsonValue, 
/**
 * For an ask of the architect's that offers options, the option chosen: for its study or its policy, one of those
 * the ask lists.
 */
choice?: string, };

/**
 * The owner's ruling on a design card, as the page and the command line hand it in: a verdict on each of its
 * decisions, all sent at once.
 */
export type Ruling = { 
/**
 * One for each decision of the design card.
 */
decisions: Array<DecisionRuling>, };

/**
 * The ruling on one decision of a design card.
 */
export type DecisionRuling = { 
/**
 * The decision's number on the design card, from 1.
 */
decision: number, 
/**
 * confirm or disagree.
 */
verdict: string, 
/**
 * Why, for a disagreement; it may be left out when another approach is chosen.
 */
text?: string, 
/**
 * For a disagreement: another of the decision's approaches, chosen instead of the one recommended, by its name.
 */
option?: string, };

/**
 * What an answer or a ruling did, in words.
 */
export type Done = { done: string, };

/**
 * How many messages a mark made read.
 */
export type Marked = { marked: number, };

/**
 * What the owner or the architect said to a work.
 */
export type Interjection = { id: number, work: number, line: string, speaker: string, text: string, 
/**
 * The round of the work it was handed in, by its number in the work; none while it waits.
 */
delivered_round: number | null, };

/**
 * A work in full: where it stands as the engine keeps it, and every round it went through.
 */
export type WorkDetail = { work: WorkSummary, machine: string | null, worktree: string | null, pushed_head: string | null, 
/**
 * The cell the work left to ask, and the cells waiting for an answer, innermost last.
 */
resume_at: string | null, waiting: JsonValue, 
/**
 * The values its rows recorded.
 */
recorded: JsonValue, rounds: Array<RoundSummary>, design_cards: Array<DesignCard>, asks: Array<OwnerAsk>, 
/**
 * What its agents told the owner, and what the engine told the owner of it, oldest first.
 */
messages: Array<OwnerMessage>, interjections: Array<Interjection>, 
/**
 * The sessions its lines ran in, oldest first.
 */
sessions: Array<WorkSession>, 
/**
 * The keep-warm pings sent to its sessions while it waited, oldest first.
 */
pings: Array<Ping>, };

/**
 * The session of a line, as the command line knows it.
 */
export type Session = { line: string, cli: string, cli_session: string, };

/**
 * A session one of a work's lines ran in.
 */
export type WorkSession = { id: number, line: string, 
/**
 * Why it started: line, cell, limit, or wake (a wake of the architect going on in the CLI session the last wake
 * left).
 */
started: string, 
/**
 * When it ended, so that the line's next round started a new one; none while it goes on.
 */
ended: number | null, };

/**
 * A keep-warm ping: a minimal message the session manager sent a session while its work waited, so that the
 * session's prompt cache was read instead of expiring. A ping means the work waited over an hour.
 */
export type Ping = { id: number, session: number, line: string, 
/**
 * The cell the work waited at, what it waited on (pause, owner, architect, items or tool) and that in words.
 */
cell: string, on: string, what: string, started_at: number, ended_at: number | null, model: string | null, 
/**
 * What it cost, as the command line reported it; none for one that failed or runs.
 */
cost_usd: number | null, 
/**
 * Tokens read from the cache, and written to it.
 */
read: number, written: number, 
/**
 * Why it failed, when it did.
 */
error: string | null, };

/**
 * One input a round was given, as a step that is not an agent's round records it, or as an agent's round recorded it
 * before its message was recorded with its conditional inputs: what it is, its value, and where its cell reads it
 * from.
 */
export type RoundInput = { what: string, value: JsonValue, 
/**
 * The sources the cell's definition names for it; empty when the cell no longer names this input.
 */
from: Array<Src>, };

/**
 * One conditional input of an agent round's message.
 */
export type RoundConditional = { 
/**
 * Its address, such as `R5-12·D2·design`, `rule:ground` or `format:common`.
 */
address: string, 
/**
 * The hash of its content, which tells two versions under one address apart.
 */
hash: string, 
/**
 * Given in the message; otherwise the session already held it, and the message only referred to it.
 */
given: boolean, };

/**
 * A file a tool run kept, or that its result points to, under the data directory.
 */
export type KeptFile = { 
/**
 * The path as the database holds it.
 */
path: string, 
/**
 * Where the interface serves it: `logs/<name>` or `tool-output/<name>`, under `/api/v1/files/`.
 */
file: string, };

/**
 * One attempt of a tool the engine ran.
 */
export type ToolRun = { id: number, tool: string, args: JsonValue, attempt: number, 
/**
 * The result, a refusal and its reason included; none while it runs.
 */
result: JsonValue | null, files: Array<KeptFile>, queued_at: number, started_at: number | null, ended_at: number | null, };

/**
 * A round in full: everything the engine gave, the return, what the engine made of it, and what it cost.
 */
export type RoundDetail = { round: RoundSummary, work: number, 
/**
 * The work's flow version and graph, in which the round's cell is found.
 */
flow_version: number, graph: string, session: Session | null, config_commit: string, 
/**
 * The message exactly as the agent was sent it (for a round recorded before messages were, its prompt); none
 * for a round no agent ran.
 */
message: string | null, 
/**
 * Each conditional input the message gave or referred to; empty for a round no agent ran, or one recorded
 * before.
 */
conditional: Array<RoundConditional>, 
/**
 * How the command line was called: the whole command line, the session started or resumed, the engine's and
 * the command line's versions; none for a round no agent ran, or one recorded before.
 */
call: JsonValue | null, 
/**
 * The level an agent's round ran at: as its call record says, or for a round recorded before it said, the one
 * level the configuration at the round's commit maps its model and effort to; none when no one level does.
 */
level: string | null, 
/**
 * The effort an agent's round ran with, as its command line gave it.
 */
effort: string | null, 
/**
 * For a round that locked a new acceptance: the acceptance locked before, which it replaced; none for one that
 * locked the first.
 */
acceptance_before: JsonValue | null, 
/**
 * The limits of the flow's counters for the work's project, as the configuration the round ran with sets them,
 * by counter.
 */
limits: { [key in string]: number }, 
/**
 * The inputs of a step that is not an agent's round, or of an agent's round recorded before its conditional
 * inputs were.
 */
inputs: Array<RoundInput>, 
/**
 * What the engine gave, as recorded: the message, conditional inputs, call or inputs above, and what the engine knew
 * besides, such as the head and the interjections handed in.
 */
input: JsonValue, output: JsonValue | null, 
/**
 * Why the return was sent back or the run failed.
 */
rejected: JsonValue | null, 
/**
 * Tokens and cost as the command line reported them.
 */
usage: JsonValue | null, tool_runs: Array<ToolRun>, design_cards: Array<DesignCard>, asks: Array<OwnerAsk>, 
/**
 * What was said to the work and handed in with this round.
 */
interjections: Array<Interjection>, };

/**
 * A stored flow version.
 */
export type FlowVersion = { id: number, digest: string, 
/**
 * How many works are pinned to it.
 */
works: number, };

/**
 * Where an item stands.
 */
export type ItemState = "new" | "queued" | "working" | "done" | "stopped" | "cancelled";

/**
 * An item as a list shows it.
 */
export type ItemSummary = { id: number, project: string, title: string, 
/**
 * change or report.
 */
deliver: string, level: string, cancelled: string | null, state: ItemState, 
/**
 * Its place in the dispatch queue, lower first; none when it is not queued.
 */
queued: number | null, 
/**
 * Its latest work and the cell that work is at.
 */
work: number | null, at: string | null, 
/**
 * The stage its latest work is at, and the stages of that work's graph in their order, among which it stands;
 * none and empty before a work starts.
 */
stage: string | null, stages: Array<string>, 
/**
 * Its latest work's asks of the owner still waiting for an answer, and its design cards still waiting for a
 * ruling.
 */
open_asks: number, open_design_cards: number, 
/**
 * Its pull request: that of its latest work that has one, or the one it was imported with.
 */
pr: number | null, 
/**
 * The items not merged yet that it depends on by a blocking dependency, which keep it from starting; empty while a
 * work on it goes on, or it is done or cancelled.
 */
blocked_by: Array<number>, };

/**
 * A dependency: `item` cannot finish until `depends_on` is done.
 */
export type ItemLink = { item: number, depends_on: number, 
/**
 * What `item` needs `depends_on` to hand in.
 */
needs: string, 
/**
 * blocking: `item` does not start until `depends_on` is merged; parallel: it may start before.
 */
kind: string, };

/**
 * Every item, and the dependencies between them.
 */
export type Items = { items: Array<ItemSummary>, links: Array<ItemLink>, };

/**
 * An item on the other end of a dependency, with what the dependency needs and its kind, blocking or parallel.
 */
export type LinkedItem = { item: ItemSummary, needs: string, kind: string, };

/**
 * An item in full: as its creator wrote it, its locked acceptance, its dependencies both ways, its works, design cards
 * and asks of the owner.
 */
export type ItemDetail = { item: ItemSummary, problem: string, why: string, approach: string | null, 
/**
 * The locked acceptance, and the round that locked it, by its work and its number in the work; none until a first
 * design passes its checks.
 */
acceptance: JsonValue | null, acceptance_work: number | null, acceptance_round: number | null, 
/**
 * The items it depends on, and the items that depend on it.
 */
depends_on: Array<LinkedItem>, depended_on_by: Array<LinkedItem>, 
/**
 * Its works, the earlier ones too: a work stopped when the flow moved to a new version, and the work after it
 * that took the item over.
 */
works: Array<WorkSummary>, 
/**
 * What an import recorded of work on the item outside this flow: where it came from, its branch and its pull
 * request; none for an item made here.
 */
imported_from: string | null, imported_branch: string | null, imported_pr: number | null, design_cards: Array<DesignCard>, asks: Array<OwnerAsk>, 
/**
 * What the rounds of its works handed back that their flow versions show on the item, oldest first.
 */
shown: Array<Shown>, 
/**
 * Each time its tier changed after it was made, oldest first: when, by whom, from what to what.
 */
level_changes: Array<LevelChange>, };

/**
 * A value a round handed back that the flow shows on the item the work is on: the owner reads it there.
 */
export type Shown = { work: number, 
/**
 * The round that handed it back: its number in the work.
 */
round: number, cell: string, 
/**
 * The path of the return it is at, as the flow writes it.
 */
path: string, value: JsonValue, };

/**
 * A change of an item's tier, as its history keeps it.
 */
export type LevelChange = { at: number, 
/**
 * owner, architect or engine.
 */
by: string, from: string, to: string, };

/**
 * The settings the owner keeps on the page, as the configuration's last commit has them: each tier's model, the
 * highest tier an item may be at (its advisor runs one tier above), the architect's tier, and the session manager's
 * settings.
 */
export type Settings = { 
/**
 * Each tier, in order.
 */
tiers: Array<Tier>, 
/**
 * The highest tier an item may be at; none, any.
 */
highest_level: string | null, 
/**
 * The tier the architect runs at.
 */
architect: string, 
/**
 * The context, in tokens, past which a line's session is replaced at its next round.
 */
context_limit: number, 
/**
 * Keep-warm, which is not configuration: how many minutes after a waiting session's last call it is pinged, and
 * at most how many times in one wait.
 */
keep_warm_minutes: number, keep_warm_most: number, 
/**
 * The configuration commit they are read from.
 */
commit: string, };

/**
 * A tier: its name, the model it runs, and what work it is for.
 */
export type Tier = { name: string, model: string, desc: string, };

/**
 * A change to the settings the owner makes on the page: each value given takes the place of the one there.
 */
export type SettingsChange = { 
/**
 * A model for each tier changed, by the tier's name.
 */
models: { [key in string]: string }, highest_level: string | null, architect: string | null, context_limit: number | null, };

/**
 * An item's new tier, as the owner sets it on the item's page.
 */
export type NewLevel = { level: string, };

/**
 * A project's architect, as its tab shows it: its wakes, its queue, what it asked the owner, its log, the item changes
 * its batches made, the policy it works by, where things stand as it last wrote it, and the project's observations. A
 * wake is a work of the architect's graph for the project, on no item.
 */
export type Architect = { 
/**
 * The project whose architect it is.
 */
project: string, 
/**
 * Every project of the configuration, each of which has its architect.
 */
projects: Array<string>, 
/**
 * Its wakes, newest first.
 */
wakes: Array<Wake>, 
/**
 * The matters it has not handled: those in its queue, in the order they came; then those an ask of the owner holds.
 */
queue: Array<Matter>, 
/**
 * What stops the project's new works from starting, oldest first.
 */
holds: Array<DispatchHold>, 
/**
 * What its wakes asked the owner, oldest first.
 */
asks: Array<OwnerAsk>, 
/**
 * The matters it handled, newest first, each with what became of it.
 */
handled: Array<Handled>, 
/**
 * The item changes its batches made or dropped, newest first.
 */
changes: Array<ItemChange>, 
/**
 * The policy in force for the project; none before the first takes effect.
 */
policy: PolicyInForce | null, 
/**
 * The policy a wake is setting now; none while no wake sets one.
 */
setting: PolicySetting | null, 
/**
 * Its latest check of whether the policy in force still holds (cell `P1`); none before the first.
 */
check: Said | null, understanding: Understanding, next: NextPlan, 
/**
 * The project's observations: the open ones first, then those closed, each newest first.
 */
observations: Array<Observation>, 
/**
 * The project's tags for observations, each with what it covers.
 */
tags: Array<Tag>, };

/**
 * One wake of an architect.
 */
export type Wake = { work: WorkSummary, 
/**
 * What woke it: the matters its queue held when it woke, as its first round was given them.
 */
woke_for: Array<Queued>, 
/**
 * How many matters it handled.
 */
handled: number, 
/**
 * The matter it handles now: the one its latest pick chose, until that matter is handled or held by an ask; none
 * when it handles none, or is over.
 */
handling: number | null, };

/**
 * A matter as an architect's queue gave it to a wake.
 */
export type Queued = { matter: number, kind: string, detail: string | null, what: string, item: number | null, };

/**
 * A matter for an architect that it has not handled.
 */
export type Matter = { id: number, 
/**
 * Its kind, which decides its path: set_straight, engineer_question, report, design_card, failure, owner_words,
 * other_architect or slots_free.
 */
kind: string, 
/**
 * What more its kind says, such as an engineer's question's own kind.
 */
detail: string | null, 
/**
 * What it is, in one sentence, as the engine wrote it.
 */
what: string, 
/**
 * Who raised it: `work`, a round of a work; `architect`, a round of an architect's; `owner`, the owner's words;
 * `engine`, the engine.
 */
source: string, 
/**
 * The item it concerns, and the item's title.
 */
item: number | null, title: string | null, 
/**
 * The round that raised it, by its work and its number in the work.
 */
work: number | null, round: number | null, 
/**
 * When it came.
 */
at: number, 
/**
 * The ask of the owner that holds it until answered, and the cell it goes back to with the answer.
 */
asked: number | null, resume_cell: string | null, 
/**
 * Whether the owner answered that ask, so that it is back in the queue, for the cell that asked.
 */
answered: boolean, 
/**
 * For a matter in the queue: who waits for it (`agent`, an engineer stopped for its question; `owner`; or
 * `none`), and how long since it came, in milliseconds.
 */
waits: string | null, waited_ms: number | null, 
/**
 * Whether the report it holds was marked urgent.
 */
urgent: boolean, 
/**
 * For a place to set straight: the place, as the check of what runs found it (where, what is wrong there, the
 * steps to set it straight); none for a matter of another kind.
 */
fix: JsonValue | null, 
/**
 * For a design card to look at: how the card stands.
 */
card: MatterCard | null, };

/**
 * How the design card a matter is about stands: its number; open, ruled or replaced; its rulings, the owner's and the
 * architect's alike; and the card that replaced it.
 */
export type MatterCard = { id: number, status: string, rulings: JsonValue, replaced_by: number | null, };

/**
 * What stops a project's new works from starting: `architect`, its architect's own hold; `set_straight`, while its
 * architect sets it straight; `policy`, while a new policy is set; since when; and for the architect's own hold, why,
 * as it said it.
 */
export type DispatchHold = { reason: string, since: number, why: string | null, };

/**
 * What a round of an architect handed back: its wake, its number in the wake, its cell, when it ended, and its answer.
 */
export type Said = { work: number, round: number, cell: string, at: number | null, value: JsonValue, };

/**
 * A matter an architect handled, and what became of it, from what was done on its path rather than what was said of
 * it: each kind of outcome it had.
 */
export type Handled = { matter: number, kind: string, detail: string | null, what: string, 
/**
 * The round that closed it: its wake, its number and its cell, and when it ended.
 */
work: number, round: number, cell: string, at: number | null, 
/**
 * The reply taken back to the work that asked, and that work.
 */
reply: string | null, reply_to: number | null, 
/**
 * How many messages its path told the owner.
 */
told: number, 
/**
 * The item changes it collected: made, dropped, and still waiting for the review of their batch.
 */
made: number, dropped: number, collected: number, 
/**
 * The kind of each change its path made to the observations: new, merge or close.
 */
observations: Array<string>, 
/**
 * What its path did at once: each action's kind, with the item it was about.
 */
actions: Array<HandledAction>, 
/**
 * The projects whose architects its path wrote to.
 */
messages: Array<string>, 
/**
 * The ask of the owner its path put about it.
 */
ask: number | null, 
/**
 * When nothing else came of it: why, as the cell that closed it said.
 */
why: string | null, };

/**
 * Something a matter's path did at once: its kind (pause_work, resume_work, hold_starts, release_starts,
 * interject), with the item it was about.
 */
export type HandledAction = { kind: string, item: number | null, };

/**
 * An item change an architect's batch made or dropped, with the matter it came from.
 */
export type ItemChange = { id: number, 
/**
 * The round that handed it in: its wake and its number.
 */
work: number, round: number, 
/**
 * The cell of that round.
 */
cell: string, matter: number, 
/**
 * The change as handed in, with its reason.
 */
change: JsonValue, 
/**
 * made, or dropped when the review still found it wrong at its third look.
 */
state: string, 
/**
 * What it made, such as the item it created; for one dropped, what the review found.
 */
outcome: JsonValue | null, };

/**
 * One of a project's observations: something worth doing that no item does yet.
 */
export type Observation = { id: number, 
/**
 * What it is, why it is worth doing, and what happens if it is not done.
 */
what: string, 
/**
 * Its tags, from the project's set.
 */
tags: Array<string>, 
/**
 * Who found it, in what situation, and the matter it came from.
 */
source: string, times_reported: number, 
/**
 * What it refers to: PRs, branches, design cards, the owner's words.
 */
refs: Array<string>, 
/**
 * The item that took it in, which closes it once its work merges.
 */
folded_into: number | null, 
/**
 * Why it was closed (outdated, folded, owner or other), in a word and in a sentence; none while it is open.
 */
closed: string | null, closed_why: string | null, 
/**
 * When it was recorded, and last changed.
 */
created: number | null, updated: number | null, };

/**
 * A tag a project's observations may carry, with what it covers.
 */
export type Tag = { name: string, desc: string, };

/**
 * What the owner sends to close an observation: why.
 */
export type CloseObservation = { why: string, };

/**
 * The policy in force: its text, and the round of the wake in which it took effect, after the owner approved it.
 */
export type PolicyInForce = { work: number, round: number, 
/**
 * The cell of the round that wrote it.
 */
cell: string, at: number | null, policy: JsonValue, };

/**
 * A new policy as a wake sets it: the latest study of it, and the latest text with its probes and every reading of it
 * by a new architect since, with the architect's verdict on each.
 */
export type PolicySetting = { work: number, study: Said | null, draft: Draft | null, };

/**
 * A version of the policy being set, by the round that wrote it.
 */
export type Draft = { round: number, 
/**
 * The cell of that round.
 */
cell: string, policy: JsonValue, probes: JsonValue, readings: Array<Reading>, };

/**
 * A new architect's reading of the policy being set, by its round, with the verdict on it; none until judged.
 */
export type Reading = { round: number, 
/**
 * The cell of that round.
 */
cell: string, reading: JsonValue, verdict: JsonValue, };

/**
 * What the project is now, as the architect last wrote it (cell `U1`): the state of each line of work, what changed
 * and what it could not settle.
 */
export type Understanding = { state: Said | null, 
/**
 * Every version of the understanding, newest first.
 */
versions: Array<StateVersion>, };

/**
 * What to do next under the policy, as the architect last planned it (cell `N1`): what was missing, what mattered
 * most and whether what runs fits; none before the first plan.
 */
export type NextPlan = { plan: Said | null, 
/**
 * The matters the plan's rows became, in their order: each thing missing, then each place to set straight.
 */
matters: Array<number>, };

/**
 * One version of the understanding: what an understanding round answered, by the wake and its round.
 */
export type StateVersion = { work: number, round: number, cell: string, at: number | null, state: JsonValue, };

/**
 * What the agents spent on one day, on one model, on one line, across every work: their rounds and the keep-warm pings
 * of their sessions, by the model each ran on and the line of its session.
 */
export type Spend = { 
/**
 * The day, in UTC, as YYYY-MM-DD.
 */
day: string, model: string, line: string, rounds: number, pings: number, 
/**
 * What they cost, as the command lines reported it, in US dollars.
 */
cost_usd: number, };

/**
 * Why a request failed.
 */
export type ApiError = { error: string, };

/**
 * What the flow page shows of a compiled flow besides the flow itself, graph by graph.
 */
export type FlowView = { graphs: Array<GraphView>, };

export type GraphView = { name: string, 
/**
 * How each line's session is kept (when it starts and which cells continue it), as the flow built into this
 * program says: a compiled flow version keeps the lines, not what they are.
 */
lines: Array<Named>, 
/**
 * Each cell, in the order the graph has them.
 */
cells: Array<CellView>, 
/**
 * The forms every agent's return shares, as `format:common` gives them, with no project's settings written beside
 * their names and no kind of question left out.
 */
common_format: string, };

/**
 * One cell, as the flow page explains it.
 */
export type CellView = { id: string, 
/**
 * For each row of its transition table, in order, the cells the row can lead to: its target, or the cells a
 * target the engine keeps track of (`@self`, `@caller`, `@asker`, `@stopped`) can be.
 */
targets: Array<Array<string>>, 
/**
 * Every row of every cell that can lead here, in the order of the cells and of their rows.
 */
entered_from: Array<RowRef>, 
/**
 * What a round of the cell is given: for an agent's cell, every input, always or conditional, each with its
 * source and address; for any other cell, the sources of its inputs.
 */
inputs: Array<CellInput>, 
/**
 * What the cell hands back, with whatever reads each field.
 */
returns: Returns, };

/**
 * A row of a cell, by its place in the cell's transition table, from 0.
 */
export type RowRef = { cell: string, row: number, };

/**
 * One input a round of a cell is given.
 */
export type CellInput = { 
/**
 * Given every time the cell is entered: what the agent reads itself. Every other input is conditional: given
 * when the session does not hold it, and referred to by its address when it does.
 */
always: boolean, of: CellInputOf, 
/**
 * The address it is given under, as the engine addresses it, with `‹work›`, `‹n›`, `‹id›`, `‹cell›` or `‹name›`
 * standing for what is known only when the round runs; none for what the agent reads itself.
 */
address: string | null, 
/**
 * For a place of the cell the work came from, the cell that called or the cell the work stopped at: the cells
 * it can be.
 */
cells: Array<string>, 
/**
 * What it holds, for a text the flow itself writes: the cell's own return format and what each of its inputs
 * is, as the engine gives them, with no project's settings beside their names.
 */
holds: string | null, };

/**
 * What an input is.
 */
export type CellInputOf = { "kind": "input", input: number, source: number, } | { "kind": "project_rules" } | { "kind": "rule", name: string, } | { "kind": "common_format" } | { "kind": "cell_format" } | { "kind": "cell_inputs" } | { "kind": "workspace" };

/**
 * What the inputs that come from outside the flow hold now, as far as the page can know outside a round: each
 * project's settings, guide and rules, as its configuration and its checkout have them, and what each table and view
 * of the database holds.
 */
export type InputsNow = { projects: Array<ProjectNow>, 
/**
 * Each table and view agents can read, with what its definition says it holds.
 */
tables: Array<NamedText>, };

/**
 * One project's part of what the inputs from outside the flow hold now.
 */
export type ProjectNow = { project: string, 
/**
 * Each setting the flow may read, by its name, with its value as text: a text as it is, anything else as JSON.
 */
settings: Array<NamedText>, 
/**
 * The project's AGENTS.md as its checkout has it now.
 */
agents_md: string, 
/**
 * The index of its design tree as its checkout has it now: each node's path with its first line. The full text
 * of the nodes an item cites is added for each item.
 */
design_tree: Array<NamedText>, 
/**
 * The project's own rules, each by its name with its text.
 */
rules: Array<NamedText>, };

/**
 * A name with a text.
 */
export type NamedText = { name: string, text: string, };

/**
 * What a cell hands back, as a tree of fields.
 */
export type Returns = { 
/**
 * For an agent's cell: the field its return holds exactly one form in.
 */
within: string | null, 
/**
 * For an agent's cell: the forms it may hand in, its own answer first.
 */
forms: Array<ReturnForm>, 
/**
 * The fields every return holds: for an agent's cell those beside the form, for any other cell all of them.
 */
every: Array<Field>, 
/**
 * What reads the return as a whole.
 */
used_by: Array<Use>, };

/**
 * One form an agent's return may hold.
 */
export type ReturnForm = { 
/**
 * The form's field, such as `ask` or `tools`; none for the cell's own answer.
 */
field: string | null, 
/**
 * The form's fields: the own answer's fields, or the one field the form is.
 */
fields: Array<Field>, 
/**
 * What reads that a return is this form: the rows whose condition is the cell's own answer.
 */
used_by: Array<Use>, };

/**
 * The kind of value a field holds.
 */
export type FieldKind = "text" | "integer" | "number" | "boolean" | "enum" | "object" | "list" | "union" | "any";

/**
 * A field of a return, with whatever reads it.
 */
export type Field = { name: string, 
/**
 * Its path as the flow writes it, such as `end.test_changes[added].catches`.
 */
path: string, kind: FieldKind, 
/**
 * For a list, the kind of its items.
 */
item: FieldKind | null, desc: string | null, optional: boolean, 
/**
 * For a list, the fewest items it holds.
 */
min: number | null, 
/**
 * For one of a fixed set of values: each value, with what it means when the flow says.
 */
values: Array<[string, string | null]>, used_by: Array<Use>, 
/**
 * The fields of an object, or of the objects a list holds.
 */
fields: Array<Field>, 
/**
 * For one of several shapes: the field whose value says which, and what reads that field.
 */
disc: string | null, disc_used_by: Array<Use>, 
/**
 * For one of several shapes: the fields every shape has, and each shape.
 */
common: Array<Field>, branches: Array<Branch>, };

/**
 * One of the shapes a field may take.
 */
export type Branch = { 
/**
 * The value of the field that says which shape it is.
 */
value: string, 
/**
 * Its path as the flow writes it, such as `result[green]`.
 */
path: string, desc: string | null, used_by: Array<Use>, fields: Array<Field>, };

/**
 * Something that reads a field of a return.
 */
export type Use = { "kind": "input", cell: string, input: number, } | { "kind": "condition", cell: string, row: number, } | { "kind": "action", cell: string, row: number, action: string, } | { "kind": "always", cell: string, view: string | null, action: string | null, } | { "kind": "check", cell: string, check: string, } | { "kind": "handoff", cell: string, } | { "kind": "shown", cell: string, } | { "kind": "role", cell: string, role: string, };

/**
 * What changed from one compiled flow to another, graph by graph.
 */
export type FlowDiff = { graphs: Array<GraphDiff>, };

export type GraphDiff = { name: string, 
/**
 * Whether the graph was added or removed whole, or changed.
 */
status: ChangeKind, 
/**
 * What changed in the graph apart from its cells: its start and end, its rules, the interjection, the counters.
 */
changes: Array<Change>, 
/**
 * Each cell added, removed or changed, in the order of the newer flow, the removed ones last.
 */
cells: Array<CellDiff>, };

/**
 * Whether a thing was added, removed or changed.
 */
export type ChangeKind = "added" | "removed" | "changed";

export type CellDiff = { id: string, 
/**
 * Its title in the newer flow, or in the older one for a cell removed.
 */
title: string, status: ChangeKind, changes: Array<Change>, };

/**
 * The part of a cell or a graph a change is in.
 */
export type Part = "title" | "kind" | "gate" | "line" | "stage" | "step" | "needs" | "input" | "rule" | "prompt" | "prompt_by" | "forms" | "field" | "row" | "row_order" | "check" | "always" | "handoff" | "shown" | "read_by" | "inenum" | "start" | "end" | "asking" | "receive" | "counter" | "interjection" | "rounds";

/**
 * One change: the part, which one of it, and what it was and is, as the compiled flow writes it.
 */
export type Change = { part: Part, 
/**
 * Which one of the part: an input's text, a rule's or a row's name, a field's path, a check's name; empty for
 * a part a cell has one of.
 */
key: string, 
/**
 * What it was; none for what was added.
 */
before: JsonValue | null, 
/**
 * What it is; none for what was removed.
 */
after: JsonValue | null, };

/**
 * A project's checkout, where the flow can be read at a git ref.
 */
export type FlowSource = { project: string, checkout: string, 
/**
 * Its branches, the most recently committed first.
 */
refs: Array<string>, };

/**
 * The flow as `flow/flow.json` is at a git ref of a project's checkout, compiled by the flow's compiler.
 */
export type FlowAt = { project: string, 
/**
 * The ref as it was asked for, and the commit it names.
 */
rev: string, commit: string, digest: string, 
/**
 * The stored flow version with the same digest, when there is one.
 */
version: number | null, compiled: Compiled, view: FlowView, };

export type Compiled = { format: string, engine: EngineNames, tools: Array<ToolUse>, graphs: Array<CompiledGraph>, 
/**
 * The configuration settings the flow reads, for `config check`.
 */
settings: Array<string>, };

/**
 * The engine's part of the flow, by name. Each map takes a name to what it does.
 */
export type EngineNames = { 
/**
 * What a cell that is not an agent does when the work reaches it: a cell's `step`.
 */
steps: { [key in string]: string }, 
/**
 * What the engine does on a row, or every time a cell returns a field: a row's `do`, an `always` with `do`.
 */
actions: { [key in string]: string }, 
/**
 * What the engine checks on a return before it takes a row; a return that fails goes back to the agent.
 */
checks: { [key in string]: string }, 
/**
 * What the engine computes for a cell's input: an input source `{"engine": name}`.
 */
inputs: { [key in string]: string }, 
/**
 * The values the engine itself knows about a work, which rows switch on with `inp` and cells name in
 * `inenums`; a value rows record with `sets` is not one of these.
 */
values: { [key in string]: string }, 
/**
 * What a gate needs to apply to a project. A gate whose need the project does not meet is passed by the
 * engine on its own row, without a round.
 */
needs: { [key in string]: string }, };

export type ToolUse = { name: string, runs: string, };

export type CompiledGraph = { name: string, start: string, end: string, 
/**
 * The field an agent's return holds its one form in.
 */
within: string, 
/**
 * The cells of the asking part, and the one that receives answers to the cells of its line.
 */
asking: Array<string>, receive: string | null, counters: Array<Counter>, rules: Array<Rule>, interject: CompiledInterject | null, 
/**
 * The kinds of question that only a project meeting a need can meet: an agent working on a project that does
 * not meet it is not told of the kind.
 */
ask_needs?: Array<AskNeed>, 
/**
 * How every agent's return is written, which the forms every return shares say once.
 */
write?: Array<string>, 
/**
 * Fields of agents' own answers that several cells hand in alike, which the forms every return shares give once
 * and each cell's own format refers to.
 */
once?: Array<string>, 
/**
 * What the engine does with an agent's round that does not end in a return the cell takes; none in a flow
 * version stored before the flow said it in this form, which runs by the section of the flow built into the
 * engine.
 */
rounds?: Rounds | null, cells: Array<CompiledCell>, };

export type Counter = { name: string, desc: string, 
/**
 * The counter is the work's total time; exactly one is.
 */
time?: boolean, };

export type Rule = { name: string, desc: string, text: Array<string>, 
/**
 * The lines every agent's cell of which is given the rule, besides the cells that name it; none: only those.
 */
lines?: Array<string>, 
/**
 * What a cell given any of must be given the rule with, by an engine input's name, a table's or a view's, or a
 * cell's return place as `cell·place`: the compiler refuses an agent's cell given one of them without the rule.
 */
givenWith?: Array<string>, };

export type CompiledInterject = { name: string, desc: string, text: Array<string>, from: Array<Named>, };

/**
 * A kind of question only a project meeting a need can meet.
 */
export type AskNeed = { kind: string, 
/**
 * One of `engine.needs`.
 */
need: string, 
/**
 * The kind with what it means, as a return format's description of the field that names a kind says it: what
 * is taken out of that description when the kind is left out.
 */
text: string, };

export type Named = { name: string, desc: string, };

/**
 * What the engine does with an agent's round that does not end in a return the cell takes: a return sent back, or
 * a run of the agent's command line that ended without one. Every agent's cell has this alike.
 */
export type Rounds = { desc: string, sentBack: SentBack, failed: Failed, 
/**
 * What every round records.
 */
record: Array<string>, 
/**
 * What an agent's command line starts with, apart from what the flow gives it.
 */
environment: Array<string>, };

export type SentBack = { desc: string, 
/**
 * How many returns of one cell in a row may be sent back before the engine pauses the work.
 */
pauseAfter: number, pause: string, };

export type Failed = { desc: string, kinds: Array<Failure>, };

/**
 * One kind of run that ended without a return; every agent's cell has a shared row of its name for it.
 */
export type Failure = { name: string, 
/**
 * What the command line reports when this happens.
 */
when: string, handle: Handle, desc: string, };

export type Handle = "wait" | "retry" | "pause";

export type CompiledCell = { id: string, 
/**
 * engine, agent, tool or owner.
 */
kind: string, 
/**
 * Written as a gate.
 */
gate: boolean, title: string, step: string | null, needs: string | null, line: string | null, 
/**
 * The cell starts a new session of its line at its first round in a work, besides the line's first round.
 */
new_session?: boolean, 
/**
 * The reasoning effort an agent's cell runs at, whatever the model; none in a flow version stored before cells
 * declared one.
 */
effort?: string | null, stage: string | null, tools?: Array<string>, inputs: Array<CompiledInput>, 
/**
 * The flow's rules an agent's cell is given, besides the project's rules, which every agent is given.
 */
rules?: Array<RuleUse>, prompt?: Array<string>, prompt_by: PromptBy | null, forms?: Array<Form>, 
/**
 * What the cell's return must be, as JSON Schema; for an agent, its round's prompt gives it.
 */
schema: JsonValue | null, rows: Array<CompiledRow>, always?: Array<Always>, checks?: Array<Check>, handoff?: Array<string>, 
/**
 * Paths of the return the owner's page shows on the item the work is on.
 */
shown?: Array<string>, 
/**
 * Places of the return a role reads outside this flow, through the database.
 */
read_by?: Array<ReadBy>, inenums?: { [key in string]: Array<[string, string]> }, };

export type CompiledInput = { id: string | null, 
/**
 * For an input every agent's cell has: what it is called. A cell's list of what its inputs are names it only;
 * the forms every return shares say what it is.
 */
name?: string | null, 
/**
 * For a table the cell answers row by row: the prefix the engine numbers its rows with, across all its sources.
 */
numbered?: string | null, what: string, from: Array<Src>, maybe: boolean, };

/**
 * Where an input comes from.
 */
export type Src = [string, string] | "self" | { config: string } | { db: string, by?: string } | { engine: string } | { recorded: string };

/**
 * A rule of the flow an agent's cell is given, by its name.
 */
export type RuleUse = { name: string, 
/**
 * Given to every agent's cell of the cell's line, not named by the cell itself.
 */
shared: boolean, };

export type PromptBy = { by: string, cases: { [key in string]: Array<string> }, };

/**
 * One form of an agent's return: the cell's own answer, or one of the common fields.
 */
export type Form = { own?: boolean, field: string | null, say: string, 
/**
 * The lines whose cells outside the asking part have this form; with none, every agent's cell has it.
 */
lines: Array<string> | null, 
/**
 * Fields of `every` a return of this form does not hold. It leaves them as an earlier return wrote them.
 */
without?: Array<string>, };

export type CompiledRow = { name: string, if: Array<Atom>, to: string, sets?: { [key in string]: string }, do?: Array<Do>, counts: string | null, waits: boolean, 
/**
 * The row pauses the work and tells the owner; once the owner lets it go on, the row goes where it says.
 */
pauses: boolean, interject: boolean, main: boolean, 
/**
 * The row goes on without answering the cells that wait for an answer, which get it later another way.
 */
drops?: boolean, 
/**
 * A row every agent's cell has.
 */
common: boolean, note: string | null, };

/**
 * A condition of a row, written as a list: `["all"]`; `["own"]` (the return is the cell's own answer);
 * `["or", [conditions]]`; `["has", path]` (the place is there); `["is", path, values]` (its value is one of
 * them); `["any", path, values]` and `["none", path, values]` (some value, or no value, in a list is one of them);
 * `["inp", name, values]` (a value the engine knows or rows recorded is one of them). Two hold for no return, and
 * only the shared rows the rounds section gives every agent's cell have them: `["sent_back"]` (the return is sent
 * back), with a number (that many of the cell's returns in a row are), and `["failed", kind]` (the run ended
 * without a return, of that kind).
 */
export type Atom = ["all"] | ["own"] | ["or", Array<Atom>] | ["has", string] | ["is" | "any" | "none" | "inp", string, Array<string>] | ["sent_back"] | ["sent_back", number] | ["failed", string];

/**
 * An action on a row: a name, or a name done for a place of the output.
 */
export type Do = string | DoAt;

export type DoAt = { at: string, do: string, };

/**
 * What the engine does every time a cell's return holds a place: the place that sets it off, and the action. The
 * flow names only actions here; a flow version stored before may name a view instead, which did nothing.
 */
export type Always = { 
/**
 * The place of the return whose presence sets the action off.
 */
at: string, view: string | null, do: string | null, };

/**
 * A check the engine makes on a return, on a place of it or on the whole.
 */
export type Check = { at: string | null, check: string, };

/**
 * A place of a return a role reads outside this flow, through the database, such as the architect reading side
 * reports: what the role does with it is not a cell of this flow.
 */
export type ReadBy = { at: string, 
/**
 * The role that reads it, one of the roles an agent may ask.
 */
role: string, };
