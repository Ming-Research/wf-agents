// The types of the owner's interface, generated from the Rust types in crates/api and crates/flow. Do not edit:
// change the Rust types and run `WF_AGENTS_GENERATE=1 cargo test -p api --test typescript` to write this file again.

export type JsonValue = number | string | boolean | Array<JsonValue> | { [key in string]: JsonValue } | null;

export type Mode = "run" | "step" | "hold";

export type StepTaken = { at: number, work: number | null, started_item: number | null, rounds: Array<number>, cell: string | null, };

export type Status = { mode: Mode, stepping: boolean, last_step: StepTaken | null, slots: number, slots_used: number, flow_version: number, config_commit: string, language: string, started_at: number, version: string, };

export type Event = { "kind": "ready" } | { "kind": "resync" } | { "kind": "status", status: Status, } | { "kind": "work", work: number, } | { "kind": "round", work: number, round: number, } | { "kind": "card", card: number, work: number | null, } | { "kind": "item", item: number | null, } | { "kind": "flow", version: number, };

export type WorkState = "running" | "ready" | "asking" | "waiting" | "paused" | "done" | "ended";

export type RunningRound = { id: number, cell: string | null, started_at: number, };

export type WorkSummary = { id: number, item: number | null, title: string | null, project: string | null, graph: string, flow_version: number, at: string, at_title: string | null, stage: string | null, state: WorkState, paused: string | null, ended: string | null, branch: string | null, pr: number | null, started_at: number | null, last_at: number | null, rounds: number, sent_back: number, failed: number, running: RunningRound | null, cost_usd: number, open_cards: number, };

export type RoundOutcome = "running" | "took" | "sent_back" | "failed" | "interrupted" | "ended";

export type RoundSummary = { id: number, cell: string | null, line: string | null, outcome: RoundOutcome, transition: string | null, why: string | null, started_at: number, ended_at: number | null, model: string | null, cost_usd: number | null, compactions: number, tool_runs: number, progress: string | null, };

export type Card = { id: number, round: number, work: number, item: number | null, kind: string, status: string, body: JsonValue, rulings: JsonValue, replaced_by: number | null, };

export type Interjection = { id: number, work: number, line: string, speaker: string, text: string, delivered_round: number | null, };

export type WorkDetail = { work: WorkSummary, machine: string | null, worktree: string | null, pushed_head: string | null, resume_at: string | null, waiting: JsonValue, recorded: JsonValue, rounds: Array<RoundSummary>, cards: Array<Card>, interjections: Array<Interjection>, };

export type Session = { line: string, cli: string, cli_session: string, };

export type RoundInput = { what: string, value: JsonValue, held: boolean | null, from: Array<Src>, };

export type KeptFile = { path: string, file: string, };

export type ToolRun = { id: number, tool: string, args: JsonValue, attempt: number, result: JsonValue | null, files: Array<KeptFile>, queued_at: number, started_at: number | null, ended_at: number | null, };

export type RoundDetail = { round: RoundSummary, work: number, flow_version: number, graph: string, session: Session | null, config_commit: string, prompt: string | null, inputs: Array<RoundInput>, input: JsonValue, output: JsonValue | null, rejected: JsonValue | null, usage: JsonValue | null, tool_runs: Array<ToolRun>, cards: Array<Card>, interjections: Array<Interjection>, };

export type FlowVersion = { id: number, digest: string, works: number, };

export type ItemState = "new" | "queued" | "working" | "done" | "stopped" | "cancelled";

export type ItemSummary = { id: number, project: string, title: string, deliver: string, level: string, cancelled: string | null, state: ItemState, queued: number | null, work: number | null, at: string | null, pr: number | null, };

export type ItemLink = { item: number, depends_on: number, needs: string, };

export type Items = { items: Array<ItemSummary>, links: Array<ItemLink>, };

export type LinkedItem = { item: ItemSummary, needs: string, };

export type ItemDetail = { item: ItemSummary, problem: string, why: string, approach: string | null, acceptance: JsonValue | null, acceptance_round: number | null, depends_on: Array<LinkedItem>, depended_on_by: Array<LinkedItem>, works: Array<WorkSummary>, cards: Array<Card>, };

export type SetMode = { mode: Mode, };

export type ApiError = { error: string, };

export type Compiled = { format: string, engine: EngineNames, tools: Array<ToolUse>, graphs: Array<CompiledGraph>, settings: Array<string>, };

export type EngineNames = { steps: { [key in string]: string }, actions: { [key in string]: string }, checks: { [key in string]: string }, inputs: { [key in string]: string }, values: { [key in string]: string }, needs: { [key in string]: string }, };

export type ToolUse = { name: string, runs: string, };

export type CompiledGraph = { name: string, start: string, end: string, within: string, asking: Array<string>, receive: string | null, counters: Array<Counter>, rules: Array<Rule>, interject: CompiledInterject | null, cells: Array<CompiledCell>, };

export type Counter = { name: string, desc: string, time?: boolean, };

export type Rule = { name: string, desc: string, text: Array<string>, };

export type CompiledInterject = { name: string, desc: string, text: Array<string>, from: Array<Named>, };

export type Named = { name: string, desc: string, };

export type CompiledCell = { id: string, kind: string, gate: boolean, title: string, step: string | null, needs: string | null, line: string | null, stage: string | null, tools?: Array<string>, inputs: Array<CompiledInput>, project_rules: Held | null, rules?: Array<RuleUse>, prompt?: Array<string>, prompt_by: PromptBy | null, forms?: Array<Form>, schema: JsonValue | null, rows: Array<CompiledRow>, always?: Array<Always>, checks?: Array<Check>, handoff?: Array<string>, inenums?: { [key in string]: Array<[string, string]> }, };

export type CompiledInput = { id: string | null, what: string, from: Array<Src>, maybe: boolean, held: Held | null, };

export type Src = [string, string] | "self" | { config: string } | { db: string, by?: string } | { engine: string } | { recorded: string };

export type Held = "session" | "given";

export type RuleUse = { name: string, held: Held, };

export type PromptBy = { by: string, cases: { [key in string]: Array<string> }, };

export type Form = { own?: boolean, field: string | null, say: string, lines: Array<string> | null, without?: Array<string>, };

export type CompiledRow = { name: string, if: Array<Atom>, to: string, sets?: { [key in string]: string }, do?: Array<Do>, counts: string | null, waits: boolean, interject: boolean, main: boolean, common: boolean, note: string | null, };

export type Atom = ["all"] | ["own"] | ["or", Array<Atom>] | ["has", string] | ["is" | "any" | "none" | "inp", string, Array<string>];

export type Do = string | DoAt;

export type DoAt = { at: string, do: string, };

export type Always = { at: string, view: string | null, do: string | null, };

export type Check = { at: string | null, check: string, };
