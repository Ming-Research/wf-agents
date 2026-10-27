// The types of the owner's interface, generated from the Rust types in crates/api and crates/flow. Do not edit:
// change the Rust types and run `WF_AGENTS_GENERATE=1 cargo test -p api --test typescript` to write this file again.

export type JsonValue = number | string | boolean | Array<JsonValue> | { [key in string]: JsonValue } | null;

/**
 * How the engine dispatches: what it starts, and when.
 */
export type Mode = "run" | "step" | "hold";

/**
 * One step the engine took in step mode, and what it came to.
 */
export type StepTaken = { 
/**
 * When the step ended.
 */
at: number, 
/**
 * The work it took a step of; none when no work had a step to take.
 */
work: number | null, 
/**
 * The item the step started that work on, when the step started it.
 */
started_item: number | null, 
/**
 * The rounds the step started or ended, and the cell of the last of them.
 */
rounds: Array<number>, cell: string | null, };

/**
 * The engine as it runs now, and what the page needs of the installation.
 */
export type Status = { mode: Mode, 
/**
 * In step mode: whether the step the owner asked for is under way.
 */
stepping: boolean, 
/**
 * The last step taken in step mode since the engine started.
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
 * The language everything the owner reads is written in, as the configuration's `owner.language` names it.
 */
language: string, 
/**
 * When the engine started.
 */
started_at: number, 
/**
 * The version of wf-agents that runs.
 */
version: string, };

/**
 * What changed, as the event stream gives it. An event names what to read again and never carries that state,
 * except the engine's status, which lives only in the engine. The stream gives each event an id; a client that
 * reconnects with the last id it saw is given what it missed.
 */
export type Event = { "kind": "ready" } | { "kind": "resync" } | { "kind": "status", status: Status, } | { "kind": "work", work: number, } | { "kind": "round", work: number, round: number, } | { "kind": "card", card: number, work: number | null, } | { "kind": "item", item: number | null, } | { "kind": "flow", version: number, };

/**
 * Where a work stands.
 */
export type WorkState = "running" | "ready" | "asking" | "waiting" | "paused" | "done" | "ended";

/**
 * The round of a work that is running.
 */
export type RunningRound = { id: number, cell: string | null, started_at: number, };

/**
 * A work as a list shows it: what it works on, where it stands and what it has cost.
 */
export type WorkSummary = { id: number, 
/**
 * The item worked on, its title and its project; none for a work on no item.
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
 * Why it was paused, or ended before its end.
 */
paused: string | null, ended: string | null, branch: string | null, pr: number | null, 
/**
 * When its first round started, and when its last round started or ended.
 */
started_at: number | null, last_at: number | null, rounds: number, 
/**
 * Rounds whose return was sent back, and rounds whose run failed or was cut off.
 */
sent_back: number, failed: number, running: RunningRound | null, 
/**
 * What its rounds cost, as the command lines reported it, in US dollars.
 */
cost_usd: number, 
/**
 * Its cards still waiting for a ruling.
 */
open_cards: number, };

/**
 * How a round ended.
 */
export type RoundOutcome = "running" | "took" | "sent_back" | "failed" | "interrupted" | "ended";

/**
 * A round as a work's timeline shows it.
 */
export type RoundSummary = { id: number, 
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
progress: string | null, };

/**
 * A card, as the round that handed it in wrote it, with its rulings.
 */
export type Card = { id: number, round: number, work: number, item: number | null, 
/**
 * design or ask.
 */
kind: string, 
/**
 * open, ruled or replaced.
 */
status: string, body: JsonValue, rulings: JsonValue, replaced_by: number | null, };

/**
 * What the owner or the architect said to a work.
 */
export type Interjection = { id: number, work: number, line: string, speaker: string, text: string, 
/**
 * The round it was handed in; none while it waits.
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
recorded: JsonValue, rounds: Array<RoundSummary>, cards: Array<Card>, interjections: Array<Interjection>, };

/**
 * The session of a line, as the command line knows it.
 */
export type Session = { line: string, cli: string, cli_session: string, };

/**
 * One input a round was given: what it is, its value, and where its cell reads it from.
 */
export type RoundInput = { what: string, value: JsonValue, 
/**
 * Whether the agent's session already held it, so that it was not given again.
 */
held: boolean | null, 
/**
 * The sources the cell's definition names for it; empty when the cell no longer names this input.
 */
from: Array<Src>, };

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
 * The prompt exactly as the agent was given it; none for a round no agent ran.
 */
prompt: string | null, inputs: Array<RoundInput>, 
/**
 * What the engine gave, as recorded: the prompt and inputs above, and what the engine knew besides, such as
 * the head and the interjections handed in.
 */
input: JsonValue, output: JsonValue | null, 
/**
 * Why the return was sent back or the run failed.
 */
rejected: JsonValue | null, 
/**
 * Tokens and cost as the command line reported them.
 */
usage: JsonValue | null, tool_runs: Array<ToolRun>, cards: Array<Card>, 
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
 * Its latest work, the cell that work is at, and its pull request.
 */
work: number | null, at: string | null, pr: number | null, };

/**
 * A dependency: `item` cannot finish until `depends_on` is done.
 */
export type ItemLink = { item: number, depends_on: number, 
/**
 * What `item` needs `depends_on` to hand in.
 */
needs: string, };

/**
 * Every item, and the dependencies between them.
 */
export type Items = { items: Array<ItemSummary>, links: Array<ItemLink>, };

/**
 * An item on the other end of a dependency, with what the dependency needs.
 */
export type LinkedItem = { item: ItemSummary, needs: string, };

/**
 * An item in full: as its creator wrote it, its locked acceptance, its dependencies both ways, its works and cards.
 */
export type ItemDetail = { item: ItemSummary, problem: string, why: string, approach: string | null, 
/**
 * The locked acceptance, and the round that locked it; none until a first design passes its checks.
 */
acceptance: JsonValue | null, acceptance_round: number | null, 
/**
 * The items it depends on, and the items that depend on it.
 */
depends_on: Array<LinkedItem>, depended_on_by: Array<LinkedItem>, works: Array<WorkSummary>, cards: Array<Card>, };

/**
 * The owner's ask to switch the engine's mode.
 */
export type SetMode = { mode: Mode, };

/**
 * Why a request failed.
 */
export type ApiError = { error: string, };

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
asking: Array<string>, receive: string | null, counters: Array<Counter>, rules: Array<Rule>, interject: CompiledInterject | null, cells: Array<CompiledCell>, };

export type Counter = { name: string, desc: string, 
/**
 * The counter is the work's total time; exactly one is.
 */
time?: boolean, };

export type Rule = { name: string, desc: string, text: Array<string>, };

export type CompiledInterject = { name: string, desc: string, text: Array<string>, from: Array<Named>, };

export type Named = { name: string, desc: string, };

export type CompiledCell = { id: string, 
/**
 * engine, agent, tool or owner.
 */
kind: string, 
/**
 * Written as a gate.
 */
gate: boolean, title: string, step: string | null, needs: string | null, line: string | null, stage: string | null, tools?: Array<string>, inputs: Array<CompiledInput>, 
/**
 * For an agent's cell: how it gets the project's rules, which every agent is given.
 */
project_rules: Held | null, rules?: Array<RuleUse>, prompt?: Array<string>, prompt_by: PromptBy | null, forms?: Array<Form>, 
/**
 * What the cell's return must be, as JSON Schema; for an agent, its round's prompt gives it.
 */
schema: JsonValue | null, rows: Array<CompiledRow>, always?: Array<Always>, checks?: Array<Check>, handoff?: Array<string>, inenums?: { [key in string]: Array<[string, string]> }, };

export type CompiledInput = { id: string | null, what: string, from: Array<Src>, maybe: boolean, 
/**
 * For an agent's cell; none for one it reads itself.
 */
held: Held | null, };

/**
 * Where an input comes from.
 */
export type Src = [string, string] | "self" | { config: string } | { db: string, by?: string } | { engine: string } | { recorded: string };

/**
 * How an agent's cell gets an input or a rule: its line's session already holds it on every way to the cell, or
 * the engine gives it.
 */
export type Held = "session" | "given";

export type RuleUse = { name: string, held: Held, };

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

export type CompiledRow = { name: string, if: Array<Atom>, to: string, sets?: { [key in string]: string }, do?: Array<Do>, counts: string | null, waits: boolean, interject: boolean, main: boolean, 
/**
 * A row every agent's cell has.
 */
common: boolean, note: string | null, };

/**
 * A condition of a row, written as a list: `["all"]`; `["own"]` (the return is the cell's own answer);
 * `["or", [conditions]]`; `["has", path]` (the place is there); `["is", path, values]` (its value is one of
 * them); `["any", path, values]` and `["none", path, values]` (some value, or no value, in a list is one of them);
 * `["inp", name, values]` (a value the engine knows or rows recorded is one of them).
 */
export type Atom = ["all"] | ["own"] | ["or", Array<Atom>] | ["has", string] | ["is" | "any" | "none" | "inp", string, Array<string>];

/**
 * An action on a row: a name, or a name done for a place of the output.
 */
export type Do = string | DoAt;

export type DoAt = { at: string, do: string, };

/**
 * What the engine does every time a cell returns a place: shows it in a view, or carries out an action.
 */
export type Always = { at: string, view: string | null, do: string | null, };

/**
 * A check the engine makes on a return, on a place of it or on the whole.
 */
export type Check = { at: string | null, check: string, };
