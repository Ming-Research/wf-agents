// The page's words for what the interface gives as data: conditions, named tests, limits, faults and changes. Each is
// worded here once, and every part of the page that shows one takes its words from here.

import type { Block, Change, Condition, Fault, Kind, LimitName, Test } from './api';

/// What a named test holds on, from the blocks it reads: "ci is not red", "any of needs_design, changes_tree is yes".
export function readsWords(test: Test): string {
  if (test.reads.length === 1) {
    return `${test.reads[0]} is ${test.kind === 'none' ? 'not ' : ''}${test.is}`;
  }
  const list = test.reads.join(', ');
  const words = { all: `all of ${list} are`, any: `any of ${list} is`, none: `none of ${list} is` };
  return `${words[test.kind]} ${test.is}`;
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
  const { people, outside } = block.writtenBy;
  return [...(people ? ['people'] : []), ...(outside !== null ? [`the outside (${outside})`] : [])];
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
    case 'unknownAct':
      return `The engine has no act named "${w.act}".`;
    case 'valuesEmpty':
      return 'A choice has no values to choose from.';
    case 'noSuchBlock':
      return `The block ${w.block} is not declared in flow.toml.`;
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
      return `The condition reads ${w.block}, which the cell itself writes: the cell would decide by what it writes itself whether it runs.`;
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
      return `The test ${w.test} reads ${w.block}, which the cell itself writes: the cell would decide by what it writes itself whether it runs, or whether the work waits before it.`;
    case 'testNotWrittenBefore':
      return `The test ${w.test} reads ${w.block}, which neither a cell before this one, people nor the outside write.`;
    case 'flowRefused': {
      const at = w.cell ? ` (${w.cell})` : '';
      const why = {
        empty: 'The flow has no cell.',
        twice: 'Two cells have the same name.',
        conditionOnOwnOutput: "A cell's condition reads a block the cell itself writes.",
        lastHasCondition: 'The last cell has a condition.',
        noLimit: "A cell's limit is zero.",
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
    case 'outside':
    case 'shown':
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
    case 'inputAdded':
      return { what: 'Reads', before: '', after: c.block };
    case 'inputRemoved':
      return { what: 'Reads', before: c.block, after: '' };
    case 'outputAdded':
      return { what: 'May write', before: '', after: c.block };
    case 'outputRemoved':
      return { what: 'May write', before: c.block, after: '' };
    case 'cellLimit':
      return { what: `Limit of ${LIMIT_WORDS[c.limit]}`, before: `${c.before}`, after: `${c.after}` };
    case 'blockAdded':
      return { what: 'Declared', before: '', after: '' };
    case 'blockRemoved':
      return { what: 'No longer declared', before: '', after: '' };
    case 'values':
      return { what: 'Values', before: values(c.before), after: values(c.after) };
    case 'people':
      return { what: 'Written by people', before: yesNo(c.before), after: yesNo(c.after) };
    case 'outside':
      return { what: 'Written by the outside', before: c.before ?? 'no', after: c.after ?? 'no' };
    case 'shown':
      return { what: 'Shown on the page', before: yesNo(c.before), after: yesNo(c.after) };
  }
}
