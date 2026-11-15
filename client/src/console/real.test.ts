import { describe, expect, it } from 'vitest';

import type { Block as FlowBlock, Flow } from '../api';
import type { Version, VersionPage, WorkPage } from '../works-api';
import { FLOW, fixtureItems } from './fixtures';
import { readBody } from './real';

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

/// The made-up flow with the record of what is said, which the implementation is given and writes through `reply`, and
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
  blocks: [
    ...FLOW.blocks,
    byWords('discussion', {
      field: 'reply',
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
    byWords('implement_spoken_to', { trigger: true }),
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
