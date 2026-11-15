import { describe, expect, it } from 'vitest';

import type { Block as FlowBlock, Flow } from '../api';
import type { Version, VersionPage, WorkPage } from '../works-api';
import { FLOW, fixtureItems } from './fixtures';
import { readBody, readRulings } from './real';

/// A block of the flow written from outside by the engine's source of the owner's words.
const byWords = (name: string, more: Partial<FlowBlock>): FlowBlock => ({
  name,
  values: null,
  default: null,
  what: null,
  writtenBy: { cells: [], people: false, note: false, outside: 'words' },
  readBy: { cells: ['implement'], conditions: [], tests: [], shown: false },
  ...more,
});

/// The made-up flow with the record of what is said, which the implementation is given and writes in its return, and
/// the block of the owner's words to the implementation, which runs it.
const SPOKEN: Flow = {
  ...FLOW,
  cells: FLOW.cells.map((c) =>
    c.id === 'implement'
      ? {
          ...c,
          triggers: [
            { name: 'on_spoken', what: null, block: 'implement_spoken_to', group: [], read: ['discussion'], do: null },
          ],
        }
      : c,
  ),
  // The trigger's block of the owner's words, which the same source writes, comes first: the record is the one no
  // trigger stands on.
  blocks: [
    ...FLOW.blocks,
    byWords('implement_spoken_to', { trigger: true }),
    byWords('discussion', {
      writes: [
        [
          'owner',
          [
            { piece: 'text', text: 'The owner:\n\n' },
            { piece: 'slot', name: 'words' },
          ],
        ],
        [
          'reply',
          [
            { piece: 'text', text: 'An agent:\n\n' },
            { piece: 'slot', name: 'reply' },
          ],
        ],
      ],
    }),
  ],
};

const version = (at: number, by: Version['by'], chars: number, saidTo: string | null = null): Version => ({
  at,
  time: at * 1000,
  by,
  meaning: at,
  value: null,
  saidTo,
  chars,
  empty: chars === 0,
});

describe('the messages of a work, read from the record of what is said', () => {
  it("are the owner's entries, each to the cell its version is said to, and the cells' replies, with the words alone", async () => {
    const item = fixtureItems().items[0];
    const contents: Record<string, string> = {
      'discussion@5': 'The owner:\n\n> Greet them.\n\nBy the first name.',
      'discussion@9': 'An agent:\n\nDone.',
    };
    const page: WorkPage = {
      summary: item.work,
      title: item.title,
      level: item.level,
      project: item.project,
      history: item.history,
      flow: SPOKEN,
      cells: [],
      blocks: [
        { name: 'discussion', versions: [version(5, { kind: 'outside' }, 60, 'implement'), version(9, { kind: 'run', run: 3, cell: 'implement' }, 30)] },
        { name: 'implement_spoken_to', versions: [version(6, { kind: 'outside' }, 0)] },
      ],
      steps: [],
    };
    const load = (_: number, block: string, at: number): Promise<VersionPage> =>
      Promise.resolve({ block, at, content: contents[`${block}@${at}`] ?? '' });
    const body = await readBody(page, load);
    expect(body.messages).toEqual([
      { who: 'owner', at: 5000, to: ['implement'], quote: 'Greet them.', text: 'By the first name.' },
      { who: 'implement', at: 9000, to: ['owner'], quote: null, text: 'Done.' },
    ]);
  });

  it("read each of the owner's entries as to the cell its version is said to, which its words do not name", async () => {
    const item = fixtureItems().items[0];
    const page: WorkPage = {
      summary: item.work,
      title: item.title,
      level: item.level,
      project: item.project,
      history: item.history,
      flow: SPOKEN,
      cells: [],
      blocks: [{ name: 'discussion', versions: [version(12, { kind: 'outside' }, 25, 'implement'), version(14, { kind: 'outside' }, 25, 'review')] }],
      steps: [],
    };
    const load = (_: number, block: string, at: number): Promise<VersionPage> =>
      Promise.resolve({ block, at, content: 'The owner:\n\nOne table.' });
    const body = await readBody(page, load);
    expect(body.messages).toEqual([
      { who: 'owner', at: 12000, to: ['implement'], quote: null, text: 'One table.' },
      { who: 'owner', at: 14000, to: ['review'], quote: null, text: 'One table.' },
    ]);
  });
});

describe('what the owner may rule of a precondition the work waits for', () => {
  it('keeps a test of content from holding by what the engine recorded as holding nothing, never by a length', async () => {
    const item = fixtureItems().items[0];
    const people = (name: string, values: string[] | null): FlowBlock => ({
      name,
      values,
      default: null,
      what: null,
      writtenBy: { cells: ['research'], people: true, note: false, outside: null },
      readBy: { cells: [], conditions: [], tests: ['changes'], shown: false },
    });
    const words = 'The plan changes the design tree';
    const flow: Flow = {
      ...FLOW,
      tests: [{ name: 'changes', words, file: 'tests/changes.toml', kind: 'anyContent', reads: ['edits', 'more'], is: null }],
      blocks: [...FLOW.blocks, people('edits', null), people('more', ['yes', 'no'])],
    };
    const by = { kind: 'run', run: 1, cell: 'research' } as const;
    const page: WorkPage = {
      summary: {
        ...item.work,
        now: { kind: 'waits', at: 'research', on: [{ kind: 'precondition', test: 'changes', words, by: [{ kind: 'owner' }] }] },
      },
      title: item.title,
      level: item.level,
      project: item.project,
      history: item.history,
      flow,
      cells: [],
      blocks: [
        // Only whitespace, which holds nothing.
        { name: 'edits', versions: [{ ...version(4, by, 3), empty: true }] },
        // A choice with no words beside it, which holds its value.
        { name: 'more', versions: [{ ...version(5, by, 0), value: 'yes', empty: false }] },
      ],
      steps: [],
    };
    const load = (_: number, block: string, at: number): Promise<VersionPage> =>
      Promise.resolve({ block, at, content: ' \n ' });
    expect(await readRulings(page, load)).toEqual([{ block: 'edits', at: 4, by, text: ' \n ', value: null }]);
  });
});
