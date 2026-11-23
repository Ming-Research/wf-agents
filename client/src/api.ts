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
  /// How many levels above its item's its rounds run at, never above the highest one round may run at.
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
/// `noneStanding`, when none has it as written by a person, the outside or a cell that is not skipped now; or,
/// `anyContent`, when at least one holds anything.
export type TestKind = 'all' | 'any' | 'none' | 'noneStanding' | 'anyContent';

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
  /// The value it looks for in each block it reads; none of a test of content.
  is: string | null;
}

export type Kind = 'agent' | 'engine';

/// The list of a definition that gives a cell a block, as the files write it: what every agent's cell reads
/// (`[every_agent]` of `flow.toml`), or the cell's own `reads` or `watches`.
export type List = 'every_agent' | 'reads' | 'watches';

/// A datum or group a cell watches: its name as the definition writes it, the blocks it stands for (of a datum, the
/// datum alone), and what the cell does when it is new.
export interface Watch {
  name: string;
  blocks: string[];
  do: string | null;
}

/// A datum or group a cell's file only reads, or that `[every_agent]` names: its name as written, and the blocks it
/// stands for.
export interface Read {
  name: string;
  blocks: string[];
}

/// A block a cell is given: the kind its session holds of it, and the list that gives it.
export interface GivenBlock {
  block: string;
  kind: InputKind;
  list: List;
}

/// What every agent's cell reads beside what its own file names, as `flow.toml` writes it (`[every_agent]`): each name
/// with the data it stands for, and the kind of each, by its name.
export interface EveryAgent {
  reads: Read[];
  kinds: Record<string, InputKind>;
}

/// A group of the flow's blocks, as `flow.toml` names it under `[groups]`: a cell that watches it is run by a new
/// version of any of them.
export interface Group {
  name: string;
  blocks: string[];
}

/// What the context of a cell's session must hold of a block the cell is given when the cell runs: its latest
/// version, the version the cell last worked from and the latest, or every version.
export type InputKind = 'latest' | 'lastAndLatest' | 'all';

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
  /// What runs it: each datum or group its own file's `watches` names, in order.
  watches: Watch[];
  /// What its own file's `reads` names, in order.
  reads: Read[];
  /// Each block it is given, in the order it is given them; none for an engine's cell.
  given: GivenBlock[];
  outputs: string[];
  /// The blocks among its outputs that every hand-in of it includes.
  mustWrite: string[];
  /// The tools of the engine's that an agent's cell may ask for in its return, by name, in the order written: the
  /// flow's, where it names any, else the cell's own; none for an engine's cell.
  tools: string[];
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
  /// Of a block a source of the engine's writes, what it writes, each text by its name as its file writes them.
  writes?: [string, SentPiece[]][];
  /// `outside`: the source outside the work that writes it; none where the outside does not.
  /// `note`: what people write in it is a note, their words to the cells that read it.
  writtenBy: {
    cells: string[];
    people: boolean;
    note: boolean;
    outside: string | null;
    /// Who changes what that source writes of it by their acts, as its file writes it (`owner`); absent where it says
    /// nobody.
    changedBy?: string;
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
  /// What the item's card shows the owner of the work, in the order `flow.toml` names them; none where it names none.
  card: string[];
  /// What every agent's cell reads beside what its own file names.
  everyAgent: EveryAgent;
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
  /// The cell takes the block by two names among what it watches and reads, `first` and then `also`: by itself and
  /// through a group, or through two groups.
  | { kind: 'takenTwice'; block: string; first: string; also: string }
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
  /// A line says its agents change only the project's tests, and that they only read.
  | { kind: 'readsOnlyAndTestsOnly' }
  | { kind: 'valuesEmpty' }
  /// A group of blocks has the name of a block or of a datum of the flow.
  | { kind: 'groupNameTaken'; group: string }
  /// A group names no block.
  | { kind: 'groupEmpty'; group: string }
  /// The block says what it counts as before anyone writes it, and it is not a choice.
  | { kind: 'defaultNotAChoice'; block: string }
  /// The block counts before anyone writes it as a value that is not one of its values.
  | { kind: 'defaultNotAValue'; block: string; default: string }
  /// The block says what a return with a value holds nothing else but, and the value is not one of its values.
  | { kind: 'nothingButNotAValue'; block: string; value: string }
  /// The block says a return with a value may hold another block, which no cell that writes the block may write.
  | { kind: 'nothingButNotAnOutput'; block: string; value: string; output: string }
  /// A block is named `test`, which a cell's `when` reads as the name of a named test.
  | { kind: 'blockNamedTest' }
  /// A cell's `when` or `requires` has no keys.
  | { kind: 'emptyCondition' }
  | { kind: 'noteNotByPeople'; block: string }
  | { kind: 'changedByWithoutSource'; block: string }
  | { kind: 'noteIsAChoice'; block: string }
  | { kind: 'noSuchBlock'; block: string }
  /// The cell must write the block each time, and it is not among its outputs.
  | { kind: 'notAnOutput'; block: string }
  /// The flow names among the engine's own data one that no file beside the flows defines.
  | { kind: 'noSuchData'; name: string }
  /// The thing is defined in this file and again in another.
  | { kind: 'definedTwice'; name: string; also: string }
  /// A value of a file beside the flows lacks the name that stands for each cell or each datum.
  | { kind: 'missingPlaceholder'; placeholder: string }
  /// The cell writes the work's questions, and no answer of the owner's runs it.
  | { kind: 'asksWithNoAnswer'; block: string }
  /// The cell writes the work's questions, and the owner's words do not run it.
  | { kind: 'asksAndCannotBeSpokenTo'; block: string }
  /// The cell writes a block of the owner's answers or words that is not its own.
  | { kind: 'notItsOwn'; block: string }
  /// The cell writes its own words, which the owner speaks to it in, and asks the owner nothing.
  | { kind: 'spokenToAndAsksNothing'; block: string }
  /// The cell hands in a verdict and judges in more than one dialogue, which the verdict's points could go in.
  | { kind: 'verdictOfDialogues'; verdict: string; dialogues: string[] }
  /// The agent's cell hands in a verdict and judges in no dialogue its points could go in.
  | { kind: 'verdictWithoutDialogue'; verdict: string }
  /// The block names entries of the design tree, and no datum of the flow is the design tree.
  | { kind: 'entriesWithoutTree'; block: string }
  /// The cell is given a block that names entries of the design tree in another kind than its latest.
  | { kind: 'entriesNotLatest'; block: string }
  /// The flow names a cell for the engine to ask in that cannot change the work or cannot be spoken to.
  | { kind: 'cannotBeAskedIn'; cell: string }
  | { kind: 'engineIsGiven'; block: string }
  /// A `do` is said for the block, and the cell does not watch it.
  | { kind: 'notWatched'; block: string }
  /// The cell watches a datum or group whose every block's every writer runs only where the cell cannot: `needs` is the
  /// cell's own `when` and `requires`, and each writer a cell with its own, as written.
  | { kind: 'neverNew'; watched: string; block: string; needs: string; writers: [string, string][] }
  /// In the new form, an agent's cell that nothing runs.
  | { kind: 'runByNothing' }
  /// In the new form, the cell does not say the kind of a block it is given.
  | { kind: 'noKind'; block: string }
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
  | { kind: 'testNamesNoValue'; test: string }
  | { kind: 'contentTestNamesValue'; test: string }
  | { kind: 'unusedTest'; test: string }
  | { kind: 'testOnOwnOutput'; test: string; block: string }
  | { kind: 'testNotWrittenBefore'; test: string; block: string }
  | { kind: 'testWrittenAfter'; test: string; block: string; cell: string }
  | { kind: 'standingWrittenByTwo'; test: string; block: string; cell: string }
  | { kind: 'notACellId'; id: string }
  | { kind: 'idGivenTwice'; id: string }
  | { kind: 'idToItself'; id: string }
  | { kind: 'movedAndTakenOut'; id: string }
  | { kind: 'idStillHeld'; id: string }
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

/// The version of the flow the engine runs, by the digest of its files, and whether the files of the directory shown
/// make it: none where they cannot be read. The answer has it only where an engine runs beside the page. It says
/// nothing of the files the page reads beside the flow for what an agent is sent.
export type Installed = { version: string; same: boolean | null };

export type Definition = (
  | { state: 'compiled'; flow: Flow }
  | { state: 'faults'; faults: Fault[] }
  | { state: 'unreadable'; reason: string }
) & { installed?: Installed };

/// What a part of what an agent is sent is, for the page to know what it may do with it: the program's own part; a
/// part of the system prompt; a part of the form of the return; a part of a message that is no content of a block, nor the task,
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
  /// Its key in the files: of a part of the form, `form` for the engine's fields, `outputs` for the line's blocks and
  /// `tools` for the flow's tools.
  name: string;
  /// Of a part written anew each time, when it is sent at all: its `only`.
  only: string | null;
  /// The file its words come from; of a part of the form, each file and directory its definition comes from, between
  /// commas.
  source: string;
  /// The block, of the content of a block the cell is given.
  block: string | null;
  /// The kind the cell gives that block in, as its file writes it.
  kind: string | null;
  /// The text sent, where it is written already.
  pieces: SentPiece[];
  /// What is sent in its place where the session holds it already: its `held`.
  held: SentPiece[];
  /// Of the content of a block, what is sent in its place where the session holds it and the cell never worked from it:
  /// `held_new`.
  heldNew: SentPiece[];
  /// Of the content of a block, what is sent in place of its text where the session lacks what the text needs.
  whole: SentPiece[];
  /// Of a part of a list, each item it may hold, by its key, in the order sent: of the part `new`, the line of each
  /// datum or group the cell watches, by its name, sent after `pieces` where it is new to the cell; and then `after`.
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
