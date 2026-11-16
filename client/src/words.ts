// How the page writes what the interface gives as data: conditions, named tests and limits by their keys and values as
// the flow's files write them (principle definitions-shown-as-written), and faults in a sentence each.

import type { Block, Cell, Condition, Fault, Flow, InputKind, Line, Test } from './api';
import { type EveryRow, kindOf, namesItself } from './matrix';

/// A name or a value as the files write it: a key of the interface in the files' own spelling.
export function snake(name: string): string {
  return name.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`);
}

/// A list as the files write it.
export function listed(names: string[]): string {
  return `[${names.join(', ')}]`;
}

/// A named test by its attributes, as its file writes them: a test of content has no `is`.
export function readsWords(test: Test): string {
  const is = test.is === null ? '' : `, is ${test.is}`;
  return `kind ${snake(test.kind)}, reads ${listed(test.reads)}${is}`;
}

/// A line's keys as its file writes them, each with its value, in the order the files write them. A key it leaves out
/// has no value of its own (`false`, `0`, none), and is not said.
export function lineKeys(line: Line): [string, string][] {
  const keys: [string, string][] = [];
  if (line.levelsAbove !== 0) keys.push(['levels_above', `${line.levelsAbove}`]);
  if (line.readsOnly) keys.push(['reads_only', 'true']);
  if (line.system.length > 0) keys.push(['system', listed(line.system)]);
  if (line.rules.length > 0) keys.push(['rules', listed(line.rules)]);
  return keys;
}

/// Keys and their values on one line, as a row of the chart says a thing: `levels_above 1, reads_only true`.
export function keysWords(keys: [string, string][]): string {
  return keys.map(([key, value]) => `${key} ${value}`).join(', ');
}

/// A named test, wherever a key of a cell's `when` or `requires` names it: by its name.
export function testWords(test: Test): string {
  return test.name;
}

/// One key of a cell's `when` or `requires` as its file writes it: a block with the value it waits for
/// (`deep_needed = no`), or the named test (`test = settled`).
export function keyWords(key: Condition): string {
  return key.on === 'block' ? `${key.block} = ${key.value}` : `test = ${testWords(key.test)}`;
}

/// A cell's `when` or `requires` as its file writes it, every key of it: `deep_needed = no, test = settled`; nothing
/// where it has none.
export function conditionWords(keys: Condition[]): string {
  return keys.map(keyWords).join(', ');
}

/// Who writes a block besides cells, by the block's own keys: `people`, `note`, `outside` with its source, and
/// `changed_by` with whoever changes what that source writes.
export function writersBesideCells(block: Block): string[] {
  const { people, note, outside, changedBy } = block.writtenBy;
  return [
    ...(people ? ['people'] : []),
    ...(note ? ['note'] : []),
    ...(outside !== null ? [`outside ${outside}`] : []),
    ...(changedBy !== undefined ? [`changed_by ${changedBy}`] : []),
  ];
}

/// What is wrong, in a sentence.
export function faultWords(fault: Fault): string {
  const w = fault.what;
  switch (w.kind) {
    case 'unreadable':
      return `The file cannot be read: ${w.message}`;
    case 'orderEmpty':
      return 'The order names no cell.';
    case 'noCellFile':
      return `The order names ${w.cell}, which has no file.`;
    case 'notInOrder':
      return `The cell ${w.cell} has a file but is not named in the order.`;
    case 'twice':
      // At the field `tools` itself, a cell names tools of its own where the flow already gives them to every agent cell.
      return fault.field === 'tools'
        ? `The cell names ${w.name} of its own, and the flow already gives its agent cells theirs.`
        : `${w.name} is named a second time in the same list.`;
    case 'limitOutOfRange':
      return `The limit ${w.value} is not a whole number from 1 to the largest count the engine keeps.`;
    case 'noLine':
      return "An agent's cell names no line to run in.";
    case 'engineHasLine':
      return `An engine's cell runs in no session, but names the line ${w.line}.`;
    case 'noSuchLine':
      return `The line ${w.line} is not declared in flow.toml.`;
    case 'noTask':
      return "An agent's cell has no task, or one of no words: its agent would be told nothing to do.";
    case 'noTaskFile':
      return `The cell's task is written in ${w.file}, which is not in the flow's directory.`;
    case 'taskFileEmpty':
      return `The cell's task is written in ${w.file}, which has no words: its agent would be told nothing to do.`;
    case 'taskFileOutside':
      return `The cell's task is written in ${w.file}, a path that leaves the flow's directory or is not plain.`;
    case 'engineHasTask':
      return "An engine's cell has a task, which no agent reads.";
    case 'noAct':
      return "An engine's cell names no act to carry it out.";
    case 'agentHasAct':
      return `An agent carries the cell out, but the cell names the act "${w.act}".`;
    case 'engineHasEffort':
      return "An engine's cell names a reasoning effort, which no agent runs at.";
    case 'noEffort':
      return "An agent's cell names no reasoning effort for its rounds to run at.";
    case 'engineHasTools':
      return "An engine's cell names tools, which no agent asks for.";
    case 'unknownAct':
      return `The engine has no act named "${w.act}".`;
    case 'unknownTool':
      return `The engine has no tool named "${w.tool}".`;
    case 'noSuchRules':
      return `The line names the rules ${w.name}, but the flow has no rules/${w.name}.md.`;
    case 'enginePart':
      return `The engine places ${w.name} in every system prompt itself: a line names only its own sets of rules.`;
    case 'rulesNamedAsEnginePart':
      return `A set of rules is named ${w.name}, which is a part the engine places in every system prompt itself.`;
    case 'unusedRules':
      return `No line names the rules ${w.name}, so no agent would be given them.`;
    case 'readsOnlyAndTestsOnly':
      return "The line says its agents change only the project's tests, and that they only read, which changes no file.";
    case 'valuesEmpty':
      return 'A choice has no values to choose from.';
    case 'groupNameTaken':
      return `The group ${w.group} has the name of a block or a datum of the flow.`;
    case 'groupEmpty':
      return `The group ${w.group} names no block.`;
    case 'defaultNotAChoice':
      return `${w.block} says what it counts as before it is written, but it is not a choice.`;
    case 'defaultNotAValue':
      return `${w.block} counts as ${w.default} before it is written, which is not one of its values.`;
    case 'blockNamedTest':
      return "A block is named test, which a cell's when takes for a named test.";
    case 'emptyCondition':
      return "A cell's when or requires has no keys: it says nothing that must hold.";
    case 'changedByWithoutSource':
      return `${w.block} says who changes what its source writes, but no source outside the work writes it.`;
    case 'noteNotByPeople':
      return `${w.block} is a note, but people do not write it.`;
    case 'noteIsAChoice':
      return `${w.block} is a note, but it is a choice, and a note is words to a cell.`;
    case 'noSuchBlock':
      return `The block ${w.block} is not declared in flow.toml.`;
    case 'notAnOutput':
      return `The cell must write ${w.block} every time, but ${w.block} is not among the blocks it may write.`;
    case 'noSuchData':
      return `The flow names ${w.name} among the engine's own data, but no file of ../data/ defines ${w.name}.`;
    case 'definedTwice':
      return `${w.name} is defined here and again in ${w.also}.`;
    case 'missingPlaceholder':
      return `This must hold ${w.placeholder}.`;
    case 'asksWithNoAnswer':
      return `The cell writes ${w.block}, the work's questions, but no answer runs it: the owner's answer would run no cell.`;
    case 'asksAndCannotBeSpokenTo':
      return `The cell writes ${w.block}, the work's questions, but the owner's words do not run it: words about its question would reach no cell.`;
    case 'notItsOwn':
      return `The cell writes ${w.block}, a block of the owner's answers or words that is not its own.`;
    case 'spokenToAndAsksNothing':
      return `The cell writes ${w.block}, its words with the owner, but asks the owner nothing.`;
    case 'verdictOfDialogues':
      return `The cell hands in ${w.verdict}, its verdict, and judges in ${w.dialogues.join(', ')}, so its points could go in more than one.`;
    case 'verdictWithoutDialogue':
      return `The cell hands in ${w.verdict}, its verdict, and judges in no dialogue its points could go in.`;
    case 'lineDataOfAnother':
      return `${w.block} is the line ${w.line}'s own, and the cell is not on that line.`;
    case 'cannotBeAskedIn':
      return `The engine cannot ask in ${w.cell}: it must be an agent's cell, on a line that may change the work, with words of its own.`;
    case 'notWatched':
      return `There is a do for ${w.block}, but ${w.block} is not watched.`;
    case 'neverNew':
      return `The cell watches ${w.watched}, but it runs only where ${w.needs}, and each writer of ${w.block} runs only where it cannot: ${w.writers
        .map(([cell, when]) => `${cell} (${when})`)
        .join('; ')}; ${w.watched} can never be new to it.`;
    case 'engineIsGiven':
      return `An engine's cell says how it is given ${w.block}, but it has no session.`;
    case 'runByNothing':
      return "Nothing runs this agent's cell, so it would never be entered.";
    case 'noKind':
      return `The cell is given ${w.block}, but does not say its kind.`;
    case 'notGiven':
      return `The cell says how it is given ${w.block}, but it is not given ${w.block}.`;
    case 'noWriter':
      return `Nothing writes ${w.block}: no cell of the order, no person and nothing outside.`;
    case 'noReader':
      return `Nothing reads ${w.block}: no cell, no condition, no test, and the page does not show it.`;
    case 'unknownSource':
      return `${w.block} is written by the source ${w.source}, which the engine does not have.`;
    case 'notAChoice':
      return `A condition or a test reads ${w.block}, which is not a choice.`;
    case 'notAValue':
      return `A condition or a test waits for ${w.block} to be ${w.is}, which is not one of its values.`;
    case 'conditionOnOwnOutput':
      return `The condition reads ${w.block}, which the cell itself writes and another writes too: the cell would decide by what it writes itself, beside another, whether it runs.`;
    case 'conditionNotWrittenBefore':
      return `The condition reads ${w.block}, which neither a cell before this one, people nor the outside write.`;
    case 'lastHasCondition':
      return 'The last cell has a condition, but a work ends when its last cell has a record, so it always runs.';
    case 'undeclaredTest':
      return `The test ${w.test} is not declared in flow.toml.`;
    case 'readsEmpty':
      return `The test ${w.test} reads no block.`;
    case 'testNamesNoValue':
      return `The test ${w.test} names no value to look for: it has no is.`;
    case 'contentTestNamesValue':
      return `The test ${w.test} looks for content, and names a value to look for, which it does not read.`;
    case 'unusedTest':
      return `No cell uses the test ${w.test}.`;
    case 'testOnOwnOutput':
      return `The test ${w.test} reads ${w.block}, which the cell itself writes: the cell would decide by what it writes itself whether the work waits before it, or, beside another writer, whether it runs.`;
    case 'testNotWrittenBefore':
      return `The test ${w.test} reads ${w.block}, which neither a cell before this one, people nor the outside write.`;
    case 'testWrittenAfter':
      return `The test ${w.test} reads ${w.block}, which ${w.cell}, a cell after this one, writes.`;
    case 'standingWrittenByTwo':
      return `The test ${w.test}, of what stands, reads ${w.block}, which ${w.cell} writes as well as a cell before it: a skipped cell's write would hide the other's word.`;
    case 'flowRefused': {
      const at = w.cell ? ` (${w.cell})` : '';
      const why = {
        empty: 'The flow has no cell.',
        twice: 'Two cells have the same name.',
        conditionOnOwnOutput: "A cell's condition reads a block the cell itself writes, and another writes it too.",
        lastHasCondition: 'The last cell has a condition.',
        noLimit: "A cell's limit is zero.",
        noSuchTest: 'A cell names a test the flow does not have.',
        unwritten: 'A test a cell uses reads a block that nothing writes.',
      }[w.reason];
      return `${why}${at}`;
    }
  }
}

/// A kind a cell gives a block in, as its file writes it; nothing where it says none.
export function inputKindWords(kind: InputKind | null): string {
  return kind === null ? '' : kind.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`);
}

/// A kind as the files write it, after what it is the kind of: ` latest`; nothing where there is none.
function kindAfter(kind: InputKind | null): string {
  return kind === null ? '' : ` ${inputKindWords(kind)}`;
}

/// Names a cell takes, each with the kind it is given it in, as a crossing or a row says them: the names, then after a
/// comma their kind where they share one (`a_questions, all`), else each name with its own (`x latest, y all`).
function namesWithKinds(cell: Cell, names: string[]): string {
  const kinds = [...new Set(names.map((n) => kindOf(cell, n)))];
  if (kinds.length === 1) return `${names.join(', ')}${kinds[0] === null ? '' : `,${kindAfter(kinds[0])}`}`;
  return names.map((n) => `${n}${kindAfter(kindOf(cell, n))}`).join(', ');
}

/// What a crossing of a datum's row says, for the pointer's tip and for a reader of the screen, by the cell's keys:
/// `judge watches plan, last_and_latest`, `T1 reads background, latest`, `R1 watches plan_review, all; outputs
/// plan_review`. Of a group's row, the group it watches and the blocks of it it writes.
export function markWords(
  cell: Cell,
  name: string,
  mark: { watch: boolean; read: boolean; write: boolean },
  group?: string[],
): string {
  const parts = [
    ...(mark.watch ? [`watches ${group ? name : namesWithKinds(cell, [name])}`] : []),
    ...(mark.read ? [`reads ${namesWithKinds(cell, [name])}`] : []),
    ...(mark.write ? [`outputs ${group ? group.filter((b) => cell.outputs.includes(b)).join(', ') : name}`] : []),
  ];
  return parts.length > 0 ? `${cell.id} ${parts.join('; ')}` : '';
}

/// What a crossing of a row of `[every_agent]` says, by the cell's keys: what runs the cell of the data the row stands
/// for, by `[every_agent]` or by its own file, what its own file reads of them, each with its kind, and what it writes.
export function everyMarkWords(cell: Cell, row: EveryRow): string {
  const of = (names: string[]) => [...new Set(names.filter((n) => row.blocks.includes(n)))];
  const watched = (list: string) => of(cell.watches.filter((w) => w.list === list && namesItself(w)).map((w) => w.name));
  const alike = watched('every_agent');
  const own = watched('watches');
  const read = of(cell.reads.flatMap((r) => r.blocks)).filter((b) => !alike.includes(b) && !own.includes(b));
  const written = of(cell.outputs);
  const parts = [
    ...(alike.length > 0 ? [`every_agent watches ${namesWithKinds(cell, alike)}`] : []),
    ...(own.length > 0 ? [`watches ${namesWithKinds(cell, own)}`] : []),
    ...(read.length > 0 ? [`reads ${namesWithKinds(cell, read)}`] : []),
    ...(written.length > 0 ? [`outputs ${written.join(', ')}`] : []),
  ];
  return parts.length > 0 ? `${cell.id} ${parts.join('; ')}` : '';
}

/// Of each cell before the reader that writes a block, what decides whether it is there: the writer's `when`, or that
/// it need not write it (`must_write` false). Empty where nothing does.
export function thereWhen(flow: Flow, reader: string, block: Block): string {
  const at = flow.cells.findIndex((c) => c.id === reader);
  const parts: string[] = [];
  for (const id of block.writtenBy.cells) {
    const i = flow.cells.findIndex((c) => c.id === id);
    if (i < 0 || i >= at) continue;
    const writer = flow.cells[i];
    if (writer.when.length > 0) parts.push(`${id} when ${conditionWords(writer.when)}`);
    else if (!writer.mustWrite.includes(block.name)) parts.push(`${id} must_write false`);
  }
  return parts.join('; ');
}

/// What a cell's file says of a datum's row, on one line, for the chart's column of words, each by its key: how it
/// takes the datum (`watches` by the datum's name or through a group, `reads`, each with its kind), `outputs` or
/// `must_write`; then, after a colon, what the cell does when the datum is new, on the row of the name its file watches
/// it by, or else what the datum is, where the cell writes it. Empty where the cell has nothing to do with the datum. Its
/// `when` and its `requires` are said on the rows of the conditions, once; what `[every_agent]` names has a band of its
/// own (`everySays`).
export function cellSays(cell: Cell, block: string, what: string | null = null): string {
  const parts: string[] = [];
  const kind = kindAfter(kindOf(cell, block));
  const itself = cell.watches.find((w) => w.list === 'watches' && w.name === block && namesItself(w));
  const group = cell.watches.find((w) => w.list === 'watches' && !namesItself(w) && w.blocks.includes(block));
  const read = cell.reads.find((r) => r.blocks.includes(block));
  if (itself) parts.push(`watches${kind}`);
  else if (group) parts.push(`watches ${group.name},${kind}`);
  else if (read) parts.push(namesItself(read) ? `reads${kind}` : `reads ${read.name},${kind}`);
  const writes = cell.outputs.includes(block);
  if (writes) parts.push(cell.mustWrite.includes(block) ? 'must_write' : 'outputs');
  const said = parts.join('; ');
  if (itself?.do) return `${said}: ${itself.do}`;
  return said && writes && what ? `${said}: ${what}` : said;
}

/// What a cell's file says of a group it watches by its name, for the chart's column of words: `watches` where its
/// file names the group, `outputs` and the blocks of it the cell writes; then, after a colon, what the cell does when
/// the group is new.
export function groupSays(cell: Cell, group: string, blocks: string[]): string {
  const watched = cell.watches.find((w) => w.list === 'watches' && w.name === group);
  const written = blocks.filter((b) => cell.outputs.includes(b));
  const said = [...(watched ? ['watches'] : []), ...(written.length > 0 ? [`outputs ${written.join(', ')}`] : [])].join(
    '; ',
  );
  return watched?.do ? `${said}: ${watched.do}` : said;
}

/// What a cell's file and `[every_agent]` say of a row of `[every_agent]`, for the chart's column of words: what its
/// own file watches and reads of the data the row stands for, each with its kind, and what it writes of them; then
/// what it takes of them by `[every_agent]`, by the row's list and kind. What `[every_agent]` says the cell does is said
/// once, in its panel.
export function everySays(cell: Cell, row: EveryRow): string {
  const of = (names: string[]) => [...new Set(names.filter((n) => row.blocks.includes(n)))];
  const watched = of(cell.watches.filter((w) => w.list === 'watches' && namesItself(w)).map((w) => w.name));
  const read = of(cell.reads.flatMap((r) => r.blocks)).filter((b) => !watched.includes(b));
  const written = of(cell.outputs);
  const alike = row.blocks.some((b) => cell.given.some((g) => g.block === b && g.list === 'every_agent'));
  return [
    ...(watched.length > 0 ? [`watches ${namesWithKinds(cell, watched)}`] : []),
    ...(read.length > 0 ? [`reads ${namesWithKinds(cell, read)}`] : []),
    ...(written.length > 0 ? [`outputs ${written.join(', ')}`] : []),
    ...(alike ? [`every_agent ${row.list}${kindAfter(row.kind)}`] : []),
  ].join('; ');
}

/// What a row of `[every_agent]` is, for the chart's column of words: its list and its kind as `flow.toml` writes
/// them, and what its data are.
export function everyIs(row: EveryRow, what: string | null): string {
  const said = `every_agent ${row.list}${kindAfter(row.kind)}`;
  return what ? `${said}: ${what}` : said;
}

/// Who runs a cell: its line, or the engine.
export function runBy(cell: Cell): string {
  return cell.kind === 'agent' ? (cell.line ?? '') : 'engine';
}

/// What a cell's head on the chart has no room to say besides its title, each by its key: its line or `engine`, its
/// `when`, its `requires`; none of the last two where the cell has none.
export function headFacts(cell: Cell): {
  by: string;
  when: string | null;
  waits: string | null;
} {
  return {
    by: runBy(cell),
    when: cell.when.length > 0 ? `when ${conditionWords(cell.when)}` : null,
    waits: cell.requires.length > 0 ? `requires ${conditionWords(cell.requires)}` : null,
  };
}

/// How long a text is, in characters, in words: `2,950 characters`.
export function charactersWords(n: number): string {
  return `${n.toLocaleString('en-US')} character${n === 1 ? '' : 's'}`;
}
