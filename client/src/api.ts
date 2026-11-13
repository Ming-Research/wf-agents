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
  /// How many levels above its item's its rounds run at, never above the highest an item may be at.
  levelsAbove: number;
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

/// What the context of a cell's session must hold of a block the cell is given when the cell runs: its latest
/// version, the version the cell last consumed and the latest, or every version.
export type InputKind = 'latest' | 'lastAndLatest' | 'all';

/// A block whose new version runs a cell.
export interface RunsOn {
  block: string;
  /// What the cell's agent does when the block is new; none where the definition does not say.
  why: string | null;
}

/// A block a cell is given.
export interface Given {
  block: string;
  kind: InputKind;
  /// What the cell's agent does with the block; none where the definition does not say.
  why: string | null;
}

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
  /// The reasoning effort an agent's cell's rounds run at; an engine's cell has none.
  effort: string | null;
  /// The work's branch takes the latest main in as each run of the cell starts; an engine's cell's never does.
  takesMain: boolean;
  /// None is always.
  condition: Condition | null;
  preconditions: Test[];
  /// The blocks whose new version runs it, in the order it names them.
  runsOn: RunsOn[];
  /// The blocks it is given, its session kept supplied with them, in the order it names them; none for an engine's
  /// cell.
  given: Given[];
  outputs: string[];
  /// The blocks among its outputs that every hand-in of it includes.
  mustWrite: string[];
  /// The tools of the engine's that an agent's cell may ask for in its return, by name; none for an engine's cell.
  tools: string[];
  limits: Limits;
}

export interface Block {
  name: string;
  /// The values of a choice; none for a block that is not one.
  values: string[] | null;
  /// What the block is, in a sentence; none where the definition does not say.
  what: string | null;
  /// `outside`: the source outside the work that writes it; none where the outside does not.
  /// `note`: what people write in it is a note, their words to the cells that read it.
  writtenBy: { cells: string[]; people: boolean; note: boolean; outside: string | null };
  /// `runs`: the cells a new version of it runs; `given`: the cells it is given to; `conditions`: the cells whose
  /// condition reads its value; each in the order of the walk. `tests`: the named tests that read its value, by name.
  /// `shown`: the owner's page reads it.
  readBy: { runs: string[]; given: string[]; conditions: string[]; tests: string[]; shown: boolean };
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
  | { kind: 'engineHasEffort' }
  | { kind: 'noEffort' }
  | { kind: 'engineHasTools' }
  | { kind: 'unknownAct'; act: string }
  | { kind: 'unknownTool'; tool: string }
  /// A line names a set of rules the flow has no file for.
  | { kind: 'noSuchRules'; name: string }
  /// A line names among its sets of rules a part the engine places in every system prompt itself.
  | { kind: 'enginePart'; name: string }
  /// A set of the flow's rules has the name of a part the engine places in every system prompt itself.
  | { kind: 'rulesNamedAsEnginePart'; name: string }
  /// A set of the flow's rules that no line names.
  | { kind: 'unusedRules'; name: string }
  | { kind: 'valuesEmpty' }
  | { kind: 'noteNotByPeople'; block: string }
  | { kind: 'noteIsAChoice'; block: string }
  | { kind: 'noSuchBlock'; block: string }
  /// The cell must write the block each time, and it is not among its outputs.
  | { kind: 'notAnOutput'; block: string }
  /// The cell says what the work is by the block, and the block does not run it.
  | { kind: 'describedNotRunOn'; block: string }
  /// In the old form, the cell says how or why it takes the block, and it is not among its inputs.
  | { kind: 'takenNotRead'; block: string }
  /// The cell names what it takes both in the old form and in the new.
  | { kind: 'twoForms' }
  | { kind: 'engineIsGiven'; block: string }
  /// In the new form, an agent's cell that nothing runs.
  | { kind: 'runByNothing' }
  /// In the new form, the cell does not say the kind of a block it is given.
  | { kind: 'noKind'; block: string }
  /// A table that says nothing of the block it is named for.
  | { kind: 'saysNothing'; block: string }
  /// The cell says why the block runs it, and it is not among those that run it.
  | { kind: 'notRunOn'; block: string }
  /// The cell says how or why it is given the block, and it is not given it.
  | { kind: 'notGiven'; block: string }
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
      reason: 'empty' | 'twice' | 'conditionOnOwnOutput' | 'lastHasCondition' | 'noLimit' | 'noSuchTest' | 'unwritten';
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
  | { kind: 'rulesAdded'; name: string; text: string }
  | { kind: 'rulesRemoved'; name: string; text: string }
  | { kind: 'rulesChanged'; name: string; before: string; after: string }
  | { kind: 'cellAdded'; cell: string; place: number }
  | { kind: 'cellRemoved'; cell: string; place: number }
  | { kind: 'cellMoved'; cell: string; before: number; after: number }
  | { kind: 'title'; cell: string; before: string; after: string }
  | { kind: 'kind'; cell: string; before: Kind; after: Kind }
  | { kind: 'line'; cell: string; before: string | null; after: string | null }
  | { kind: 'readsOnly'; cell: string; before: boolean; after: boolean }
  /// The parts of the system prompt of the session of the cell's line, by name.
  | { kind: 'system'; cell: string; before: string[]; after: string[] }
  | { kind: 'takesMain'; cell: string; before: boolean; after: boolean }
  | { kind: 'task'; cell: string; before: string | null; after: string | null }
  | { kind: 'act'; cell: string; before: string | null; after: string | null }
  | { kind: 'condition'; cell: string; before: Condition | null; after: Condition | null }
  | { kind: 'preconditionAdded'; cell: string; test: Test }
  | { kind: 'preconditionRemoved'; cell: string; test: Test }
  | { kind: 'runsOnAdded'; cell: string; block: string }
  | { kind: 'runsOnRemoved'; cell: string; block: string }
  | { kind: 'givenAdded'; cell: string; block: string }
  | { kind: 'givenRemoved'; cell: string; block: string }
  | { kind: 'outputAdded'; cell: string; block: string }
  | { kind: 'outputRemoved'; cell: string; block: string }
  | { kind: 'mustWriteAdded'; cell: string; block: string }
  | { kind: 'mustWriteRemoved'; cell: string; block: string }
  | { kind: 'describesAdded'; cell: string; block: string }
  | { kind: 'describesRemoved'; cell: string; block: string }
  | { kind: 'toolAdded'; cell: string; tool: string }
  | { kind: 'toolRemoved'; cell: string; tool: string }
  | { kind: 'runsWhy'; cell: string; block: string; before: string | null; after: string | null }
  | { kind: 'givenKind'; cell: string; block: string; before: InputKind; after: InputKind }
  /// The blocks the cell is given in both stand in another order; both lists whole.
  | { kind: 'givenOrder'; cell: string; before: string[]; after: string[] }
  | { kind: 'givenWhy'; cell: string; block: string; before: string | null; after: string | null }
  | { kind: 'cellLimit'; cell: string; limit: LimitName; before: number; after: number }
  | { kind: 'effort'; cell: string; before: string | null; after: string | null }
  | { kind: 'levelsAbove'; cell: string; before: number; after: number }
  | { kind: 'blockAdded'; block: string }
  | { kind: 'blockRemoved'; block: string }
  | { kind: 'values'; block: string; before: string[] | null; after: string[] | null }
  | { kind: 'people'; block: string; before: boolean; after: boolean }
  | { kind: 'note'; block: string; before: boolean; after: boolean }
  | { kind: 'outside'; block: string; before: string | null; after: string | null }
  | { kind: 'shown'; block: string; before: boolean; after: boolean }
  | { kind: 'what'; block: string; before: string | null; after: string | null };

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
