// The page's words for what the interface gives as data: conditions, limits, faults and changes. Each is worded here
// once, and every part of the page that shows one takes its words from here.

import type { Block, Change, Condition, Fault, Kind, LimitName } from './api';

/// A condition, as the column "Runs when" and the cell's panel show it.
export function conditionWords(condition: Condition | null): string {
  if (!condition) return 'always';
  return condition.on === 'block' ? `${condition.block} is ${condition.value}` : condition.words;
}

export const LIMIT_WORDS: Record<LimitName, string> = {
  runs: 'runs in a row that are ways round',
  returns: 'returns sent back within a run',
  calls: 'calls within a run',
};

export function kindWords(kind: Kind): string {
  return kind === 'agent' ? 'an agent' : 'the engine';
}

/// Who writes a block besides cells, in words; empty when only cells do.
export function writersBesideCells(block: Block): string[] {
  return [...(block.writtenBy.people ? ['people'] : []), ...(block.writtenBy.outside ? ['the outside'] : [])];
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
    case 'valuesEmpty':
      return 'A choice has no values to choose from.';
    case 'noSuchBlock':
      return `The block ${w.block} is not declared in flow.toml.`;
    case 'noWriter':
      return `Nothing writes ${w.block}: no cell of the order, no person and nothing outside.`;
    case 'noReader':
      return `Nothing reads ${w.block}: no cell, no condition, and neither the page nor a test shows it.`;
    case 'notAChoice':
      return `A condition reads ${w.block}, which is not a choice.`;
    case 'notAValue':
      return `A condition waits for ${w.block} to be ${w.is}, which is not one of its values.`;
    case 'conditionOnOwnOutput':
      return `The condition reads ${w.block}, which the cell itself writes: the cell could never come to run.`;
    case 'conditionNotWrittenBefore':
      return `The condition reads ${w.block}, which neither a cell before this one nor people write.`;
    case 'lastHasCondition':
      return 'The last cell has a condition, but a work ends when its last cell has a record, so it always runs.';
    case 'undeclaredTest':
      return `The test ${w.test} is not declared in flow.toml.`;
    case 'unknownTest':
      return `The engine has no test ${w.test}.`;
    case 'unusedTest':
      return `No cell uses the test ${w.test}.`;
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
    case 'testReworded':
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
      return { what: 'Declared', before: '', after: c.words };
    case 'testRemoved':
      return { what: 'No longer declared', before: c.words, after: '' };
    case 'testReworded':
      return { what: 'Words', before: c.before, after: c.after };
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
    case 'condition':
      return { what: 'Runs when', before: conditionWords(c.before), after: conditionWords(c.after) };
    case 'preconditionAdded':
      return { what: 'Waits until', before: '', after: c.words };
    case 'preconditionRemoved':
      return { what: 'Waits until', before: c.words, after: '' };
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
      return { what: 'Written by the outside', before: yesNo(c.before), after: yesNo(c.after) };
    case 'shown':
      return { what: 'Shown on the page or read by a test', before: yesNo(c.before), after: yesNo(c.after) };
  }
}
