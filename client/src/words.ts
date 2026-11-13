// The page's words for what the interface gives as data: conditions, named tests, limits, faults and changes. Each is
// worded here once, and every part of the page that shows one takes its words from here.

import type { Block, Cell, Change, Condition, Fault, Flow, InputKind, Kind, LimitName, Test } from './api';

/// What a named test holds on, from the blocks it reads: "ci is not red", "any of needs_design, changes_tree is yes",
/// "none of verdict, deep_verdict is no, not counting what a cell skipped now wrote".
export function readsWords(test: Test): string {
  const standing = test.kind === 'noneStanding' ? ', not counting what a cell skipped now wrote' : '';
  if (test.reads.length === 1) {
    const not = test.kind === 'none' || test.kind === 'noneStanding' ? 'not ' : '';
    return `${test.reads[0]} is ${not}${test.is}${standing}`;
  }
  const list = test.reads.join(', ');
  const words = {
    all: `all of ${list} are`,
    any: `any of ${list} is`,
    none: `none of ${list} is`,
    noneStanding: `none of ${list} is`,
  };
  return `${words[test.kind]} ${test.is}${standing}`;
}

/// A named test, wherever a cell's condition or precondition is said: its words, and what it reads.
export function testWords(test: Test): string {
  return `${test.words} (${readsWords(test)})`;
}

/// A condition, as the column "Runs when" and the cell's panel show it.
export function conditionWords(condition: Condition | null): string {
  if (!condition) return 'always';
  return condition.on === 'block' ? `${condition.block} is ${condition.value}` : testWords(condition.test);
}

export const LIMIT_WORDS: Record<LimitName, string> = {
  runs: 'runs in a row that are ways round',
  returns: 'returns sent back within a run',
  calls: 'calls within a run',
};

export function kindWords(kind: Kind): string {
  return kind === 'agent' ? 'an agent' : 'the engine';
}

/// Who writes a block besides cells, in words, the outside by the source that writes it; empty when only cells do.
export function writersBesideCells(block: Block): string[] {
  const { people, note, outside } = block.writtenBy;
  const byPeople = note ? 'people, as a note to the cells that read it' : 'people';
  return [...(people ? [byPeople] : []), ...(outside !== null ? [`the outside (${outside})`] : [])];
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
      return `${w.name} is named a second time in the same list.`;
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
    case 'valuesEmpty':
      return 'A choice has no values to choose from.';
    case 'noteNotByPeople':
      return `${w.block} is a note, but people do not write it.`;
    case 'noteIsAChoice':
      return `${w.block} is a note, but it is a choice, and a note is words to a cell.`;
    case 'noSuchBlock':
      return `The block ${w.block} is not declared in flow.toml.`;
    case 'notAnOutput':
      return `The cell must write ${w.block} every time, but ${w.block} is not among the blocks it may write.`;
    case 'describedNotRunOn':
      return `The cell says what the work is by ${w.block}, but ${w.block} does not run it.`;
    case 'takenNotRead':
      return `The cell says how it takes ${w.block}, but ${w.block} is not among the blocks it reads.`;
    case 'twoForms':
      return 'Name them one way: either inputs, or runs_on and given.';
    case 'engineIsGiven':
      return `An engine's cell says how it is given ${w.block}, but it has no session.`;
    case 'runByNothing':
      return "Nothing runs this agent's cell, so it would never be entered.";
    case 'noKind':
      return `The cell is given ${w.block}, but does not say its kind.`;
    case 'saysNothing':
      return `The table for ${w.block} says nothing.`;
    case 'notRunOn':
      return `The cell says why ${w.block} runs it, but ${w.block} is not among what runs it.`;
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
        noSuchTest: "A cell names a test the flow does not have.",
        unwritten: 'A test a cell uses reads a block that nothing writes.',
      }[w.reason];
      return `${why}${at}`;
    }
  }
}

/// Where a change is: the flow, a test, a cell or a block.
export type Place = { on: 'flow' } | { on: 'test'; test: string } | { on: 'cell'; cell: string } | { on: 'block'; block: string };

export function placeOf(change: Change): Place {
  switch (change.kind) {
    case 'name':
    case 'flowLimit':
    case 'rulesAdded':
    case 'rulesRemoved':
    case 'rulesChanged':
      return { on: 'flow' };
    case 'testAdded':
    case 'testRemoved':
      return { on: 'test', test: change.test.name };
    case 'testReworded':
    case 'testChanged':
      return { on: 'test', test: change.test };
    case 'blockAdded':
    case 'blockRemoved':
    case 'values':
    case 'people':
    case 'note':
    case 'outside':
    case 'shown':
    case 'what':
      return { on: 'block', block: change.block };
    default:
      return { on: 'cell', cell: change.cell };
  }
}

/// A change in words: what changed, and how, from before to after. `before` is empty for what was only put in, and
/// `after` for what was only taken out; both are empty when the words of `what` say it all.
export interface ChangeWords {
  what: string;
  before: string;
  after: string;
}

const yesNo = (b: boolean) => (b ? 'yes' : 'no');
const values = (v: string[] | null) => (v ? v.join(', ') : 'not a choice');
const names = (v: string[]) => (v.length > 0 ? v.join(', ') : 'none');
const place = (p: number) => `place ${p + 1}`;

export function changeWords(change: Change): ChangeWords {
  const c = change;
  switch (c.kind) {
    case 'name':
      return { what: 'Name', before: c.before, after: c.after };
    case 'flowLimit':
      return { what: `Limit of ${LIMIT_WORDS[c.limit]}`, before: `${c.before}`, after: `${c.after}` };
    case 'testAdded':
      return { what: 'Declared', before: '', after: testWords(c.test) };
    case 'testRemoved':
      return { what: 'No longer declared', before: testWords(c.test), after: '' };
    case 'testReworded':
      return { what: 'Words', before: c.before, after: c.after };
    case 'testChanged':
      return { what: 'Holds when', before: readsWords(c.before), after: readsWords(c.after) };
    case 'rulesAdded':
      return { what: `Rules ${c.name}`, before: '', after: c.text };
    case 'rulesRemoved':
      return { what: `Rules ${c.name}`, before: c.text, after: '' };
    case 'rulesChanged':
      return { what: `Rules ${c.name}`, before: c.before, after: c.after };
    case 'cellAdded':
      return { what: 'Put in', before: '', after: place(c.place) };
    case 'cellRemoved':
      return { what: 'Taken out', before: place(c.place), after: '' };
    case 'cellMoved':
      return { what: 'Moved', before: place(c.before), after: place(c.after) };
    case 'title':
      return { what: 'Title', before: c.before, after: c.after };
    case 'kind':
      return { what: 'Run by', before: kindWords(c.before), after: kindWords(c.after) };
    case 'line':
      return { what: 'Line', before: c.before ?? 'none', after: c.after ?? 'none' };
    case 'readsOnly':
      return { what: 'Only reads', before: yesNo(c.before), after: yesNo(c.after) };
    case 'system':
      return { what: 'Rules of its line', before: names(c.before), after: names(c.after) };
    case 'takesMain':
      return { what: 'Takes main in as it starts', before: yesNo(c.before), after: yesNo(c.after) };
    case 'task':
      return { what: 'Task', before: c.before ?? 'none', after: c.after ?? 'none' };
    case 'act':
      return { what: 'Act', before: c.before ?? 'none', after: c.after ?? 'none' };
    case 'condition':
      return { what: 'Runs when', before: conditionWords(c.before), after: conditionWords(c.after) };
    case 'preconditionAdded':
      return { what: 'Waits until', before: '', after: testWords(c.test) };
    case 'preconditionRemoved':
      return { what: 'Waits until', before: testWords(c.test), after: '' };
    case 'runsOnAdded':
      return { what: 'Runs it', before: '', after: c.block };
    case 'runsOnRemoved':
      return { what: 'Runs it', before: c.block, after: '' };
    case 'givenAdded':
      return { what: 'Is given', before: '', after: c.block };
    case 'givenRemoved':
      return { what: 'Is given', before: c.block, after: '' };
    case 'outputAdded':
      return { what: 'May write', before: '', after: c.block };
    case 'outputRemoved':
      return { what: 'May write', before: c.block, after: '' };
    case 'mustWriteAdded':
      return { what: 'Writes every time', before: '', after: c.block };
    case 'mustWriteRemoved':
      return { what: 'Writes every time', before: c.block, after: '' };
    case 'describesAdded':
      return { what: 'Says what the work is by', before: '', after: c.block };
    case 'describesRemoved':
      return { what: 'Says what the work is by', before: c.block, after: '' };
    case 'toolAdded':
      return { what: 'May ask the engine for', before: '', after: c.tool };
    case 'toolRemoved':
      return { what: 'May ask the engine for', before: c.tool, after: '' };
    case 'runsWhy':
      return { what: `${c.block} runs it because`, before: c.before ?? 'not said', after: c.after ?? 'not said' };
    case 'givenKind':
      return { what: `Is given ${c.block} as`, before: inputKindWords(c.before), after: inputKindWords(c.after) };
    case 'givenOrder':
      return { what: 'Is given, in the order', before: names(c.before), after: names(c.after) };
    case 'givenWhy':
      return { what: `Is given ${c.block} because`, before: c.before ?? 'not said', after: c.after ?? 'not said' };
    case 'cellLimit':
      return { what: `Limit of ${LIMIT_WORDS[c.limit]}`, before: `${c.before}`, after: `${c.after}` };
    case 'effort':
      return { what: 'Effort', before: c.before ?? 'none', after: c.after ?? 'none' };
    case 'levelsAbove':
      return { what: "Levels above the item's", before: `${c.before}`, after: `${c.after}` };
    case 'blockAdded':
      return { what: 'Declared', before: '', after: '' };
    case 'blockRemoved':
      return { what: 'No longer declared', before: '', after: '' };
    case 'values':
      return { what: 'Values', before: values(c.before), after: values(c.after) };
    case 'people':
      return { what: 'Written by people', before: yesNo(c.before), after: yesNo(c.after) };
    case 'note':
      return { what: 'A note', before: yesNo(c.before), after: yesNo(c.after) };
    case 'outside':
      return { what: 'Written by the outside', before: c.before ?? 'no', after: c.after ?? 'no' };
    case 'shown':
      return { what: 'Shown on the page', before: yesNo(c.before), after: yesNo(c.after) };
    case 'what':
      return { what: 'What it is', before: c.before ?? 'not said', after: c.after ?? 'not said' };
  }
}

/// The words for what the context must hold of an input.
export function inputKindWords(kind: InputKind | null): string {
  switch (kind) {
    case 'latest':
      return 'the latest';
    case 'lastAndLatest':
      return 'the last and the latest';
    case 'all':
      return 'all of it';
    case null:
      return 'not said';
  }
}

/// When a block is there for a cell to read, where that is not plain from who writes it: a cell before the reader
/// that runs only under a condition, or that need not write the block each time. Empty where there is nothing to add.
export function thereWhen(flow: Flow, reader: string, block: Block): string {
  const at = flow.cells.findIndex((c) => c.id === reader);
  const parts: string[] = [];
  for (const id of block.writtenBy.cells) {
    const i = flow.cells.findIndex((c) => c.id === id);
    if (i < 0 || i >= at) continue;
    const writer = flow.cells[i];
    if (writer.condition !== null) parts.push(`only when ${id} runs (${conditionWords(writer.condition)})`);
    else if (!writer.mustWrite.includes(block.name)) parts.push(`if ${id} writes it`);
  }
  return parts.join('; ');
}

/// What a cell says of a block, on one line, for the chart's column of words: of a block it reads, how it takes it and
/// why; of one it may write, whether each time, and whether it brings up to date what it read; of one whose value says
/// whether it runs or what it waits for, that. Empty where the cell has nothing to do with the block.
export function cellSays(cell: Cell, block: string): string {
  const parts: string[] = [];
  const reads = cell.inputs.includes(block);
  if (reads) {
    const taken = cell.input[block];
    const how = taken?.kind ? `takes ${inputKindWords(taken.kind)}` : 'reads';
    parts.push(taken?.why ? `${how}: ${taken.why}` : how);
  }
  if (cell.outputs.includes(block)) {
    const write = cell.mustWrite.includes(block) ? 'writes every time' : 'may write';
    parts.push(reads ? `${write}, ${BROUGHT_UP_TO_DATE}` : write);
  }
  const condition = cell.condition;
  if (condition !== null && (condition.on === 'block' ? condition.block === block : condition.test.reads.includes(block))) {
    parts.push(`runs when ${conditionWords(condition)}`);
  }
  for (const test of cell.preconditions) {
    if (test.reads.includes(block)) parts.push(`waits until ${testWords(test)}`);
  }
  return parts.join('; ');
}

/// What is said of a block a cell both reads and may write: what it hands in is the block it read, brought up to
/// date.
export const BROUGHT_UP_TO_DATE = 'brought up to date';

/// Who runs a cell: an agent in its line, named by the line, or the engine.
export function runBy(cell: Cell): string {
  return cell.kind === 'agent' ? (cell.line ?? '') : 'engine';
}

/// What a cell's head on the chart has no room to say besides its title, each apart so that a change of one can be
/// marked: who runs it, when it runs, and what it waits for; none of the last two where the cell has none.
export function headFacts(cell: Cell): { by: string; when: string | null; waits: string | null } {
  return {
    by: cell.kind === 'agent' ? `${runBy(cell)} line` : runBy(cell),
    when: cell.condition ? `runs when ${conditionWords(cell.condition)}` : null,
    waits: cell.preconditions.length > 0 ? `waits until ${cell.preconditions.map(testWords).join('; ')}` : null,
  };
}
