// The interface the page reads, as `crates/web/src/view.rs` serves it: the types here follow those, field for field,
// and the routes' tests in `crates/web/tests/` hold the JSON they give.

/// The limits of what repeats.
export interface Limits {
  /// How many runs of a cell in a row may be ways round before the work waits for the owner.
  runs: number;
  /// How often a return may be sent back within one run.
  returns: number;
  /// How many calls one run may make.
  calls: number;
}

export type LimitName = keyof Limits;

/// The cells that share one session.
export interface Line {
  name: string;
  cells: string[];
}

/// A named test, with the words shown for it.
export interface Test {
  name: string;
  words: string;
}

export type Kind = 'agent' | 'engine';

/// When a cell runs: when a block has a value, or when a named test holds.
export type Condition = { on: 'block'; block: string; value: string } | { on: 'test'; test: string; words: string };

export interface Cell {
  id: string;
  title: string;
  kind: Kind;
  /// The line an agent's cell runs in; an engine's cell has none.
  line: string | null;
  /// None is always.
  condition: Condition | null;
  preconditions: Test[];
  inputs: string[];
  outputs: string[];
  limits: Limits;
}

export interface Block {
  name: string;
  /// The values of a choice; none for a block that is not one.
  values: string[] | null;
  writtenBy: { cells: string[]; people: boolean; outside: boolean };
  /// `shown`: the owner's page or a named test reads it.
  readBy: { cells: string[]; conditions: string[]; shown: boolean };
}

export interface Flow {
  name: string;
  limits: Limits;
  lines: Line[];
  tests: Test[];
  /// In the order of the walk.
  cells: Cell[];
  /// By name.
  blocks: Block[];
}

/// What is wrong with a definition, as data.
export type What =
  | { kind: 'unreadable'; message: string }
  | { kind: 'orderEmpty' }
  | { kind: 'noCellFile'; cell: string }
  | { kind: 'notInOrder'; cell: string }
  | { kind: 'twice'; name: string }
  | { kind: 'limitOutOfRange'; value: number }
  | { kind: 'noLine' }
  | { kind: 'engineHasLine'; line: string }
  | { kind: 'noSuchLine'; line: string }
  | { kind: 'valuesEmpty' }
  | { kind: 'noSuchBlock'; block: string }
  | { kind: 'noWriter'; block: string }
  | { kind: 'noReader'; block: string }
  | { kind: 'notAChoice'; block: string }
  | { kind: 'notAValue'; block: string; is: string }
  | { kind: 'conditionOnOwnOutput'; block: string }
  | { kind: 'conditionNotWrittenBefore'; block: string }
  | { kind: 'lastHasCondition' }
  | { kind: 'undeclaredTest'; test: string }
  | { kind: 'unknownTest'; test: string }
  | { kind: 'unusedTest'; test: string }
  | {
      kind: 'flowRefused';
      reason: 'empty' | 'twice' | 'conditionOnOwnOutput' | 'lastHasCondition' | 'noLimit';
      cell: string | null;
    };

export interface Fault {
  file: string;
  /// Empty when the whole file is at fault.
  field: string;
  what: What;
}

export type Definition =
  | { state: 'compiled'; flow: Flow }
  | { state: 'faults'; faults: Fault[] }
  | { state: 'unreadable'; reason: string };

/// One change from the definition compared against to the one shown. Places are counted from 0.
export type Change =
  | { kind: 'name'; before: string; after: string }
  | { kind: 'flowLimit'; limit: LimitName; before: number; after: number }
  | { kind: 'testAdded'; test: string; words: string }
  | { kind: 'testRemoved'; test: string; words: string }
  | { kind: 'testReworded'; test: string; before: string; after: string }
  | { kind: 'cellAdded'; cell: string; place: number }
  | { kind: 'cellRemoved'; cell: string; place: number }
  | { kind: 'cellMoved'; cell: string; before: number; after: number }
  | { kind: 'title'; cell: string; before: string; after: string }
  | { kind: 'kind'; cell: string; before: Kind; after: Kind }
  | { kind: 'line'; cell: string; before: string | null; after: string | null }
  | { kind: 'condition'; cell: string; before: Condition | null; after: Condition | null }
  | { kind: 'preconditionAdded'; cell: string; test: string; words: string }
  | { kind: 'preconditionRemoved'; cell: string; test: string; words: string }
  | { kind: 'inputAdded'; cell: string; block: string }
  | { kind: 'inputRemoved'; cell: string; block: string }
  | { kind: 'outputAdded'; cell: string; block: string }
  | { kind: 'outputRemoved'; cell: string; block: string }
  | { kind: 'cellLimit'; cell: string; limit: LimitName; before: number; after: number }
  | { kind: 'blockAdded'; block: string }
  | { kind: 'blockRemoved'; block: string }
  | { kind: 'values'; block: string; before: string[] | null; after: string[] | null }
  | { kind: 'people'; block: string; before: boolean; after: boolean }
  | { kind: 'outside'; block: string; before: boolean; after: boolean }
  | { kind: 'shown'; block: string; before: boolean; after: boolean };

export type Comparison =
  | { state: 'nothingToCompare' }
  | {
      state: 'compared';
      against: Definition;
      /// None when either definition does not compile.
      changes: Change[] | null;
    };

/// Reads a path of the interface.
async function read<T>(path: string): Promise<T> {
  const response = await fetch(path, { headers: { accept: 'application/json' } });
  if (!response.ok) {
    throw new Error(`${path} answered ${response.status}: ${await response.text()}`);
  }
  return (await response.json()) as T;
}

/// The definition shown and what changed from the one compared against, read together.
export async function load(): Promise<{ definition: Definition; comparison: Comparison }> {
  const [definition, comparison] = await Promise.all([
    read<Definition>('/api/v1/flow'),
    read<Comparison>('/api/v1/flow/changes'),
  ]);
  return { definition, comparison };
}
