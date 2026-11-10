// The items and the works as the pages read them, as `crates/web/src/works.rs` gives them: the types here follow
// those, field for field. Every time is in milliseconds since the start of 1970. A place (`at`) is the place of an
// entry in the order of what happened to a work; a version of a block is named by the place of the entry that wrote
// it.
//
// The routes:
// - `GET  /api/v1/items`: every item with its work in brief (`Items`).
// - `POST /api/v1/items`: adds an item with its work (`NewItem`), and gives their numbers (`Added`).
// - `GET  /api/v1/works/{work}`: one work, as it is now and step by step from its start (`WorkPage`).
// - `GET  /api/v1/works/{work}/versions/{block}/{at}`: the content of a version of a block (`VersionPage`).
// - `GET  /api/v1/works/{work}/runs/{run}/rounds/{number}`: one round in full (`RoundPage`).
// - `GET  /api/v1/works/{work}/calls/{call}`: a call with its result (`CallPage`).
// - `POST /api/v1/works/{work}/acts`: what the owner does to a work (`Act`); answers with no content, or with why it
//   was refused (`Refusal`) and a status of 4xx.
// - `GET  /api/v1/changes`: a stream of server-sent events, one each time something happened (`Changed`), on which a
//   page reads again what it shows.

import { read } from './api';
import type { Flow } from './api';

export type Person = 'owner' | 'architect';

/// The limit a run reached.
export type Limit = { of: 'returns'; sentBack: number } | { of: 'calls'; made: number };

/// What a call is: a tool the engine runs, a question for a person, or a run at a limit that the owner lets go on.
export type Call =
  | { kind: 'tool'; name: string; input: string }
  | { kind: 'question'; who: Person; question: string }
  | { kind: 'atLimit'; limit: Limit };

/// What a work waits for.
export type Waited =
  /// The result of a call a run rests at: from this person, or from the engine where there is none.
  | { kind: 'call'; call: number; who: Person | null; what: Call }
  /// A precondition of the cell does not hold: the named test, the words shown for it, and who can make it hold.
  | { kind: 'precondition'; test: string; words: string; who: Person }
  /// The cell's last runs, this many in a row, were each for what a cell after it wrote.
  | { kind: 'limit'; runs: number };

/// What a work does now.
export type Now =
  | { kind: 'fault'; words: string }
  | { kind: 'merged' }
  | { kind: 'cancelled' }
  /// A round of this run of this cell is under way, since this time.
  | { kind: 'round'; cell: string; run: number; round: number; since: number }
  /// It is paused, and what it would do next is begin a round.
  | { kind: 'paused' }
  /// It waits at this cell, for each of these.
  | { kind: 'waits'; at: string; on: Waited[] }
  /// It is about to do something at this cell; a work is never left so between two things that happen to it.
  | { kind: 'next'; cell: string };

/// How a cell stands for a work: a round of its run is under way; its run is at rest; its condition is not met;
/// it never finished a run; it has a record and runs again when the walk reaches it; its record knows everything.
export type CellState = 'running' | 'resting' | 'skipped' | 'notYet' | 'again' | 'done';

export interface CellNow {
  cell: string;
  state: CellState;
  /// How many of its runs finished.
  finished: number;
  /// Its latest finished run.
  record: number | null;
  /// Its run that has neither finished nor been abandoned.
  open: number | null;
}

/// What rounds took.
export interface Spent {
  /// Millionths of a dollar.
  usdMicros: number;
  inputTokens: number;
  cachedTokens: number;
  outputTokens: number;
  /// How long the rounds were under way.
  workingMs: number;
  /// How many rounds ended.
  rounds: number;
}

/// A work in brief.
export interface Summary {
  number: number;
  item: number;
  flow: string;
  added: number;
  /// No round of it begins; a round under way goes on.
  paused: boolean;
  now: Now;
  /// In the order of the walk.
  cells: CellNow[];
  spent: Spent;
  /// When something last happened to it.
  last: number | null;
}

export interface Item {
  number: number;
  title: string;
  added: number;
  dependsOn: number[];
  /// The one work done for it, which was added with it.
  work: Summary;
}

export interface Items {
  items: Item[];
}

/// Why a cell runs: it has no record, or a block has another meaning than the one its record knows (`was`), which a
/// cell after it gave the block where `behind`.
export type Cause = { kind: 'noRecord' } | { kind: 'changed'; block: string; was: number | null; behind: boolean };

/// A block a cell reads, as the cell's record knows it and as it is now.
export interface Input {
  block: string;
  current: number | null;
  known: number | null;
  changed: boolean;
}

export interface CellDetail {
  cell: string;
  /// Why the cell would run were the walk to reach it; empty where it would not.
  needs: Cause[];
  inputs: Input[];
}

export type Writer = { kind: 'person' } | { kind: 'outside' } | { kind: 'run'; run: number; cell: string };

/// A version of a block: one write of it.
export interface Version {
  at: number;
  time: number;
  by: Writer;
  /// The version at which the meaning it has last changed: itself, or an earlier one.
  meaning: number;
  value: string | null;
  /// How long its content is, in characters.
  chars: number;
}

export interface Block {
  name: string;
  versions: Version[];
}

export type Unmet =
  | { kind: 'asksNothing' }
  | { kind: 'notAnOutput'; block: string }
  | { kind: 'writtenTwice'; block: string }
  | { kind: 'noValue'; block: string }
  | { kind: 'notAValue'; block: string; value: string }
  | { kind: 'notAChoice'; block: string };

/// What a round is given: the cell's task, the results of the calls the round before asked for, or why what the
/// round before handed in was sent back.
export type Given = { kind: 'task' } | { kind: 'results'; calls: number[] } | { kind: 'sentBack'; unmet: Unmet[] };

export type Ask = { kind: 'tool'; name: string; input: string } | { kind: 'question'; who: Person; question: string };

/// A block as a round hands it in, without its content.
export interface Handed {
  block: string;
  /// Whether the run says the change matters to the cells that read the block.
  matters: boolean;
  value: string | null;
  chars: number;
}

export type Back = { kind: 'asked'; asks: Ask[] } | { kind: 'handedIn'; blocks: Handed[] };

export interface Round {
  /// The first being one.
  number: number;
  began: number;
  /// None while it is under way.
  ended: number | null;
  given: Given;
  /// None while it is under way, and for a round that ended after its run was abandoned.
  back: Back | null;
  spent: Spent | null;
}

export interface CallRow {
  call: number;
  what: Call;
  made: number;
  done: number | null;
  /// How long its result is, in characters.
  chars: number | null;
}

export type RunEnd =
  /// It finished and wrote these blocks: each is the version of its block named by `at`.
  | { kind: 'finished'; at: number; time: number; writes: Handed[] }
  | { kind: 'abandoned'; at: number; time: number };

/// One run of a cell.
export interface Run {
  run: number;
  cell: string;
  at: number;
  time: number;
  because: Cause[];
  /// The version of each block it reads or may write that it started from.
  basis: { block: string; version: number | null }[];
  rounds: Round[];
  calls: CallRow[];
  /// None while it is open.
  end: RunEnd | null;
  spent: Spent;
}

/// One thing that happened to a work. A run stands where it started.
export type Step =
  | ({ kind: 'run' } & Run)
  | {
      kind: 'wrote';
      at: number;
      time: number;
      block: string;
      by: Writer;
      /// Whether the write changed the block's meaning.
      changed: boolean;
      value: string | null;
      chars: number;
    }
  | { kind: 'letGo'; at: number; time: number; cell: string }
  | { kind: 'paused'; at: number; time: number }
  | { kind: 'resumed'; at: number; time: number }
  | { kind: 'cancelled'; at: number; time: number };

/// One work: as it is now, and step by step from its start.
export interface WorkPage {
  summary: Summary;
  /// The title of its item.
  title: string;
  /// The flow it runs on.
  flow: Flow;
  /// In the order of the walk.
  cells: CellDetail[];
  /// Every block that has a version, by name.
  blocks: Block[];
  /// In the order it happened.
  steps: Step[];
}

export interface VersionPage {
  block: string;
  at: number;
  content: string;
}

export interface CallPage {
  call: number;
  what: Call;
  result: string | null;
}

/// One round in full: the results it was given, and the content of each block it handed in.
export interface RoundPage {
  run: number;
  number: number;
  results: CallPage[];
  handed: { block: string; content: string }[];
}

/// What the owner does to a work.
export type Act =
  /// Writes a block people write: what the item is about, an opinion for a cell, a choice (with its value).
  | { kind: 'write'; block: string; content: string; value: string | null }
  | { kind: 'letGo'; cell: string }
  | { kind: 'pause' }
  | { kind: 'resume' }
  | { kind: 'cancel' }
  /// Gives a call that waits for a person its result.
  | { kind: 'answer'; call: number; result: string };

export interface NewItem {
  title: string;
  dependsOn: number[];
  /// The flow its work runs on. The work is added paused, and begins no round until it is let on.
  flow: string;
  /// What is written for it as it is added, each a block people write.
  writes: { block: string; content: string; value: string | null }[];
}

export interface Added {
  item: number;
  work: number;
}

/// Why something asked of the interface was refused, in words.
export interface Refusal {
  error: string;
}

/// Something happened: these works grew, and the items changed where `items`.
export interface Changed {
  works: number[];
  items: boolean;
}

/// Sends something to the interface; gives why where it is refused.
async function send<T>(path: string, body: unknown): Promise<T | null> {
  const response = await fetch(path, {
    method: 'POST',
    headers: { 'content-type': 'application/json', accept: 'application/json' },
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    const refusal = (await response.json().catch(() => null)) as Refusal | null;
    throw new Error(refusal?.error ?? `${path} answered ${response.status}`);
  }
  return response.status === 204 ? null : ((await response.json()) as T);
}

export const loadItems = () => read<Items>('/api/v1/items');
export const loadWork = (work: number) => read<WorkPage>(`/api/v1/works/${work}`);
export const loadVersion = (work: number, block: string, at: number) =>
  read<VersionPage>(`/api/v1/works/${work}/versions/${encodeURIComponent(block)}/${at}`);
export const loadRound = (work: number, run: number, round: number) =>
  read<RoundPage>(`/api/v1/works/${work}/runs/${run}/rounds/${round}`);
export const loadCall = (work: number, call: number) => read<CallPage>(`/api/v1/works/${work}/calls/${call}`);
export const act = (work: number, what: Act) => send<null>(`/api/v1/works/${work}/acts`, what);
export const addItem = (item: NewItem) => send<Added>('/api/v1/items', item);
