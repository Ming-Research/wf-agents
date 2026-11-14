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

/// The cells that share one session, and what its file writes of it.
export interface Line {
  name: string;
  /// In the order of the walk.
  cells: string[];
  /// How many levels above its item's its rounds run at, never above the highest an item may be at.
  levelsAbove: number;
  /// The file it is written in, from the flow's directory.
  file: string;
  /// Its agents only read.
  readsOnly: boolean;
  /// The parts of its system prompt, as written; none where it names only the rules it takes.
  system: string[];
  /// The sets of rules for its agents, as written.
  rules: string[];
}

/// How the blocks a named test reads make it hold: when every one, at least one, or none has the value it looks for;
/// or, `noneStanding`, when none has it as written by a person, the outside or a cell that is not skipped now.
export type TestKind = 'all' | 'any' | 'none' | 'noneStanding';

/// A named test: the words shown for it, and what it holds on.
export interface Test {
  name: string;
  /// Its `says`.
  words: string;
  /// The file it is written in, from the flow's directory.
  file: string;
  kind: TestKind;
  /// The blocks it reads, as the definition lists them.
  reads: string[];
  /// The value it looks for in each block it reads.
  is: string;
}

export type Kind = 'agent' | 'engine';

/// A trigger of a cell, as the definition writes it.
export interface TriggerView {
  name: string;
  /// What it is, as its file writes it.
  what: string | null;
  /// The block it stands on; of a group's trigger, the group's name.
  block: string;
  /// Of a group's trigger, the group's blocks, a new version of any of which fires it; empty otherwise.
  group: string[];
  read: string[];
  do: string | null;
}

/// A group of the flow's blocks, as `flow.toml` names it under `[groups]`, whose trigger fires for a new version of
/// any of them.
export interface Group {
  name: string;
  blocks: string[];
}

/// What the context of a cell's session must hold of a block the cell is given when the cell runs: its latest
/// version, the version the cell last consumed and the latest, or every version.
export type InputKind = 'latest' | 'lastAndLatest' | 'all';

/// One of the blocks a cell names: whether a new version of it runs the cell, with why, and whether the cell is given
/// it, its session kept supplied with its content, with how and why; it does one or both. Each why and kind is none
/// where the definition does not say.
export interface Input {
  runs: { why: string | null } | null;
  given: { kind: InputKind | null; why: string | null } | null;
}

/// One key of a cell's `when` or `requires`: a block or a datum with the value it must have (`deep_needed = "yes"`),
/// or a named test that holds (`test = "settled"`).
export type Condition = { on: 'block'; block: string; value: string } | { on: 'test'; test: Test };

export interface Cell {
  id: string;
  title: string;
  kind: Kind;
  /// The line an agent's cell runs in; an engine's cell has none.
  line: string | null;
  /// What an agent's cell's agent is told to do, with its line breaks; an engine's cell has none.
  task: string | null;
  /// The file the task is written in, from the flow's directory; none where it is written in place.
  taskFile: string | null;
  /// The act of the engine's own that carries out an engine's cell; an agent's cell has none.
  act: string | null;
  /// The reasoning effort an agent's cell's rounds run at; an engine's cell has none.
  effort: string | null;
  /// The work's branch takes the latest main in as each run of the cell starts.
  takesMain: boolean;
  /// When it runs: every key of its `when` holds, in the order written; none is always.
  when: Condition[];
  /// What the work waits for before it: every key of its `requires`, in the order written.
  requires: Condition[];
  /// The blocks it names: those that run it, in the order it names them, then those it is only given.
  inputs: string[];
  /// The blocks whose new version runs it, in the order it names them.
  runsOn: string[];
  /// The blocks it is given, in the order it names them; none for an engine's cell.
  given: string[];
  /// Of each block it names, by the block's name, whether it runs the cell and whether the cell is given it.
  input: Record<string, Input>;
  outputs: string[];
  /// The blocks among its outputs that every hand-in of it includes.
  mustWrite: string[];
  /// The tools of the engine's that an agent's cell may ask for in its return, by name, in the order written: the
  /// flow's, where it names any, else the cell's own; none for an engine's cell.
  tools: string[];
  /// What runs it, each a trigger by its name, with the block it stands on, the data its agent reads for it and what
  /// the cell does when it fires, as the definition writes them.
  triggers: TriggerView[];
  limits: Limits;
}

export interface Block {
  name: string;
  /// The values of a choice; none for a block that is not one.
  values: string[] | null;
  /// Of a choice, the value it counts as before anyone has written it; none where it has none.
  default: string | null;
  /// What the block is, in a sentence; none where the definition does not say.
  what: string | null;
  /// The field of a return a cell writes it through, where it is not written as a block.
  field?: string | null;
  /// It has no content and stands only for a trigger.
  trigger?: boolean;
  /// Of a block a source of the engine's writes, what it writes, each text by its name as its file writes them.
  writes?: [string, SentPiece[]][];
  /// `outside`: the source outside the work that writes it; none where the outside does not.
  /// `note`: what people write in it is a note, their words to the cells that read it.
  writtenBy: {
    cells: string[];
    people: boolean;
    note: boolean;
    outside: string | null;
  };
  /// `cells`: the cells that name it, that it runs or that are given it. `tests`: the named tests that read its value,
  /// by name. `shown`: the owner's page reads it.
  readBy: {
    cells: string[];
    conditions: string[];
    tests: string[];
    shown: boolean;
  };
}

export interface Flow {
  name: string;
  limits: Limits;
  lines: Line[];
  tests: Test[];
  /// The engine's tools every agent's cell of the flow may ask for, in the order `flow.toml` writes them.
  tools: string[];
  /// The groups of the flow's blocks, by name, each with its blocks as `flow.toml` names them.
  groups: Group[];
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
  /// The cell's task is written in a file the flow's directory does not have.
  | { kind: 'noTaskFile'; file: string }
  /// The cell's task is written in a file of no words.
  | { kind: 'taskFileEmpty'; file: string }
  /// The cell's task is written in a file whose path leaves the flow's directory, or is not plain.
  | { kind: 'taskFileOutside'; file: string }
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
  /// A group of blocks has the name of a block or of a datum of the flow.
  | { kind: 'groupNameTaken'; group: string }
  /// A group names no block.
  | { kind: 'groupEmpty'; group: string }
  /// The block says what it counts as before anyone writes it, and it is not a choice.
  | { kind: 'defaultNotAChoice'; block: string }
  /// The block counts before anyone writes it as a value that is not one of its values.
  | { kind: 'defaultNotAValue'; block: string; default: string }
  /// A block is named `test`, which a cell's `when` reads as the name of a named test.
  | { kind: 'blockNamedTest' }
  /// A cell's `when` or `requires` has no keys.
  | { kind: 'emptyCondition' }
  | { kind: 'noteNotByPeople'; block: string }
  | { kind: 'noteIsAChoice'; block: string }
  | { kind: 'noSuchBlock'; block: string }
  /// The cell must write the block each time, and it is not among its outputs.
  | { kind: 'notAnOutput'; block: string }
  /// The cell says what the work is by the block, and the block does not run it.
  | { kind: 'describedNotRunOn'; block: string }
  /// In the old form, the cell says how or why it takes the block, and it is not among its inputs.
  | { kind: 'takenNotRead'; block: string }
  /// The cell names what it takes in two forms of three.
  | { kind: 'twoForms' }
  /// The flow names among the engine's own data one that no file beside the flows defines.
  | { kind: 'noSuchData'; name: string }
  /// The thing is defined in this file and again in another.
  | { kind: 'definedTwice'; name: string; also: string }
  /// The cell names a trigger that nothing beside the flows defines.
  | { kind: 'noSuchTrigger'; trigger: string }
  /// The cell names a trigger a rule makes of a datum the flow does not have.
  | { kind: 'noSuchDatum'; trigger: string; datum: string }
  /// The block stands only for a trigger: no cell is given it, writes it or describes the work by it.
  | { kind: 'triggerOnly'; block: string }
  /// The cell names `triggered_by` and `context` in a flow not laid out one thing to a file.
  | { kind: 'notLaidOut' }
  /// A value of a file beside the flows lacks the name that stands for each cell or each datum.
  | { kind: 'missingPlaceholder'; placeholder: string }
  /// The block is written through a field of a return the engine does not have.
  | { kind: 'unknownReturnField'; field: string }
  /// The trigger says what happened by a place the engine does not fill.
  | { kind: 'unknownDetail'; detail: string }
  /// A trigger of an agent's cell has its agent read a datum that the cell's context does not hold.
  | { kind: 'readNotInContext'; trigger: string; block: string }
  /// The cell says what it does for a trigger that is not among its triggers.
  | { kind: 'notATrigger'; trigger: string }
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
  { state: 'compiled'; flow: Flow } | { state: 'faults'; faults: Fault[] } | { state: 'unreadable'; reason: string };

/// What a part of what an agent is sent is, for the page to know what it may do with it: the program's own part; a
/// part of the system prompt; the form of the return; a part of a message that is no content of a block, nor the task,
/// nor the blocks the cell writes; the content of a block the cell is given; the blocks the cell writes; and its task.
export type SentWhat = 'program' | 'system' | 'form' | 'message' | 'input' | 'outputs' | 'task';

/// A piece of the text of a part: sent as written, or a place filled in a run, by its name.
export type SentPiece = { piece: 'text'; text: string } | { piece: 'slot'; name: string };

/// How a part reaches the agent: the program's own, which the engine does not send; in the system prompt of the
/// session, taken once as the session begins; given with every call of the program; or in a message, as the cell is
/// entered.
export type SentCarried = 'program' | 'system' | 'call' | 'message';

/// One part of what an agent is sent as the engine enters its cell, as the files say it: its key there, its attributes,
/// the file its words come from, and its text.
export interface SentPart {
  what: SentWhat;
  carried: SentCarried;
  /// Its key in the files.
  name: string;
  /// Of a part written anew each time, when it is sent at all: its `only`.
  only: string | null;
  /// The file its words come from.
  source: string;
  /// The block, of the content of a block the cell is given.
  block: string | null;
  /// The kind the cell gives that block in, as its file writes it.
  kind: string | null;
  /// The text sent, where it is written already.
  pieces: SentPiece[];
  /// What is sent in its place where the session holds it already: its `held`.
  held: SentPiece[];
  /// Of the content of a block, what is sent in place of its text where the session lacks what the text needs.
  whole: SentPiece[];
  /// Of a part of a list, each item it may hold, by its key, in the order sent: of the triggers, the line of each
  /// trigger of the cell, by the trigger's name, sent after `pieces` where the trigger ran the cell; and then `after`.
  items: [string, SentPiece[]][];
  after: SentPiece[];
}

/// A name a text sent to an agent gives between backticks that nothing defines, and the file it is in.
export interface Unknown {
  name: string;
  source: string;
}

/// For each agent's cell of a definition that compiles, by its id, the parts it is sent in the order sent: the system
/// prompt and the message as the engine makes them, every form of each part.
export type Given =
  | {
      state: 'compiled';
      cells: Record<string, SentPart[]>;
      /// The flow's tools, so that the page tells a tool in the form from a field.
      tools: string[];
      /// Each of the engine's tools as its file in `agents/tools/` writes it, by the tool's name.
      toolFiles: Record<string, ActWords>;
      /// What each engine's cell's act writes, by the cell's id.
      acts: Record<string, ActWords>;
      unknown: Unknown[];
    }
  | { state: 'notCompiled' }
  | { state: 'unread'; why: string };

/// What an act of the engine writes, as its file says it: each key, dotted where it is in a table, with its text.
export interface ActWords {
  source: string;
  words: [string, SentPiece[]][];
}

/// Reads a path of the interface.
export async function read<T>(path: string): Promise<T> {
  const response = await fetch(path, {
    headers: { accept: 'application/json' },
  });
  if (!response.ok) {
    throw new Error(`${path} answered ${response.status}: ${await response.text()}`);
  }
  return (await response.json()) as T;
}

/// The definition shown, and what an agent is sent as each of its cells is entered, read together. What an agent is
/// sent that could not be read is said with why, the route's own reason or the read's, and the flow is shown without
/// it.
export async function load(): Promise<{
  definition: Definition;
  given: Given;
}> {
  const [definition, given] = await Promise.all([
    read<Definition>('/api/v1/flow'),
    read<Given>('/api/v1/flow/given').catch(
      (e: unknown): Given => ({ state: 'unread', why: e instanceof Error ? e.message : String(e) }),
    ),
  ]);
  return { definition, given };
}
