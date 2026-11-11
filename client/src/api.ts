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

/// How the blocks a named test reads make it hold: when every one, at least one, or none has the value it looks for;
/// or, `noneStanding`, when none has it as written by a person, the outside or a cell that is not skipped now.
export type TestKind = 'all' | 'any' | 'none' | 'noneStanding';

/// A named test: the words shown for it, and what it holds on.
export interface Test {
  name: string;
  words: string;
  kind: TestKind;
  /// The blocks it reads, as the definition lists them.
  reads: string[];
  /// The value it looks for in each block it reads.
  is: string;
}

export type Kind = 'agent' | 'engine';

/// When a cell runs: when a block has a value, or when a named test holds.
export type Condition = { on: 'block'; block: string; value: string } | { on: 'test'; test: Test };

export interface Cell {
  id: string;
  title: string;
  kind: Kind;
  /// The line an agent's cell runs in; an engine's cell has none.
  line: string | null;
  /// What an agent's cell's agent is told to do, with its line breaks; an engine's cell has none.
  task: string | null;
  /// The act of the engine's own that carries out an engine's cell; an agent's cell has none.
  act: string | null;
  /// None is always.
  condition: Condition | null;
  preconditions: Test[];
  inputs: string[];
  outputs: string[];
  /// The blocks among its outputs that every hand-in of it includes.
  mustWrite: string[];
  limits: Limits;
}

export interface Block {
  name: string;
  /// The values of a choice; none for a block that is not one.
  values: string[] | null;
  /// `outside`: the source outside the work that writes it; none where the outside does not.
  /// `note`: what people write in it is a note, their words to the cells that read it.
  writtenBy: { cells: string[]; people: boolean; note: boolean; outside: string | null };
  /// `tests`: the named tests that read its value, by name. `shown`: the owner's page reads it.
  readBy: { cells: string[]; conditions: string[]; tests: string[]; shown: boolean };
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
  | { kind: 'noTask' }
  | { kind: 'engineHasTask' }
  | { kind: 'noAct' }
  | { kind: 'agentHasAct'; act: string }
  | { kind: 'unknownAct'; act: string }
  | { kind: 'valuesEmpty' }
  | { kind: 'noteNotByPeople'; block: string }
  | { kind: 'noteIsAChoice'; block: string }
  | { kind: 'noSuchBlock'; block: string }
  /// The cell must write the block each time, and it is not among its outputs.
  | { kind: 'notAnOutput'; block: string }
  | { kind: 'noWriter'; block: string }
  | { kind: 'noReader'; block: string }
  | { kind: 'unknownSource'; block: string; source: string }
  | { kind: 'notAChoice'; block: string }
  | { kind: 'notAValue'; block: string; is: string }
  | { kind: 'conditionOnOwnOutput'; block: string }
  | { kind: 'conditionNotWrittenBefore'; block: string }
  | { kind: 'lastHasCondition' }
  | { kind: 'undeclaredTest'; test: string }
  | { kind: 'readsEmpty'; test: string }
  | { kind: 'unusedTest'; test: string }
  | { kind: 'testOnOwnOutput'; test: string; block: string }
  | { kind: 'testNotWrittenBefore'; test: string; block: string }
  | { kind: 'testWrittenAfter'; test: string; block: string; cell: string }
  | { kind: 'standingWrittenByTwo'; test: string; block: string; cell: string }
  | {
      kind: 'flowRefused';
      reason: 'empty' | 'twice' | 'conditionOnOwnOutput' | 'lastHasCondition' | 'noLimit' | 'noSuchTest';
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
  | { kind: 'testAdded'; test: Test }
  | { kind: 'testRemoved'; test: Test }
  | { kind: 'testReworded'; test: string; before: string; after: string }
  | { kind: 'testChanged'; test: string; before: Test; after: Test }
  | { kind: 'cellAdded'; cell: string; place: number }
  | { kind: 'cellRemoved'; cell: string; place: number }
  | { kind: 'cellMoved'; cell: string; before: number; after: number }
  | { kind: 'title'; cell: string; before: string; after: string }
  | { kind: 'kind'; cell: string; before: Kind; after: Kind }
  | { kind: 'line'; cell: string; before: string | null; after: string | null }
  | { kind: 'task'; cell: string; before: string | null; after: string | null }
  | { kind: 'act'; cell: string; before: string | null; after: string | null }
  | { kind: 'condition'; cell: string; before: Condition | null; after: Condition | null }
  | { kind: 'preconditionAdded'; cell: string; test: Test }
  | { kind: 'preconditionRemoved'; cell: string; test: Test }
  | { kind: 'inputAdded'; cell: string; block: string }
  | { kind: 'inputRemoved'; cell: string; block: string }
  | { kind: 'outputAdded'; cell: string; block: string }
  | { kind: 'outputRemoved'; cell: string; block: string }
  | { kind: 'mustWriteAdded'; cell: string; block: string }
  | { kind: 'mustWriteRemoved'; cell: string; block: string }
  | { kind: 'cellLimit'; cell: string; limit: LimitName; before: number; after: number }
  | { kind: 'blockAdded'; block: string }
  | { kind: 'blockRemoved'; block: string }
  | { kind: 'values'; block: string; before: string[] | null; after: string[] | null }
  | { kind: 'people'; block: string; before: boolean; after: boolean }
  | { kind: 'note'; block: string; before: boolean; after: boolean }
  | { kind: 'outside'; block: string; before: string | null; after: string | null }
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
export async function read<T>(path: string): Promise<T> {
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
