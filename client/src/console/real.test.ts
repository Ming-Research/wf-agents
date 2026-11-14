import { describe, expect, it } from 'vitest';
import type { VersionPage, WorkPage, Writer } from '../works-api';
import { readBody } from './real';

/// A version of a block at a place, by a writer, with content where `chars` is above none.
const version = (at: number, by: Writer, chars: number) => ({ at, time: at * 1000, by, meaning: at, value: null, chars });

/// A work of a flow with a research, `R1`, that the owner speaks to, whose record of what is said is `discussion`:
/// the owner said something to the engineer's line, which reached R1, and R1 answered.
function page(): WorkPage {
  const writtenBy = (people: boolean, outside: string | null) => ({ cells: [], people, note: false, outside });
  const readBy = { cells: ['R1'], tests: [], conditions: [], shown: true };
  const flow = {
    name: 'engineer',
    blocks: [
      { name: 'discussion', values: null, default: null, what: null, field: 'reply', writtenBy: writtenBy(true, null), readBy },
      {
        name: 'r1_spoken_to',
        values: null,
        default: null,
        what: null,
        trigger: true,
        writtenBy: writtenBy(false, 'words'),
        readBy,
      },
    ],
    cells: [
      {
        id: 'R1',
        line: 'engineer',
        triggers: [{ name: 'on_spoken', what: null, block: 'r1_spoken_to', group: [], read: ['discussion'], do: null }],
      },
    ],
  };
  const blocks = [
    {
      name: 'discussion',
      versions: [version(10, { kind: 'person' }, 20), version(13, { kind: 'run', run: 4, cell: 'R1' }, 5)],
    },
    { name: 'r1_spoken_to', versions: [version(11, { kind: 'outside' }, 0)] },
  ];
  return { summary: { number: 1, flow: 'engineer' }, flow, blocks } as unknown as WorkPage;
}

const contents: Record<string, string> = {
  'discussion@10': '> Keep the old table.\n\nWhy not the old table?',
  'discussion@13': 'It has no room for the cost.',
};

const load = (_: number, block: string, at: number): Promise<VersionPage> =>
  Promise.resolve({ block, at, content: contents[`${block}@${at}`] ?? '' });

describe('the messages of a work', () => {
  it("are the record of what is said: the owner's words to the cell their line reached, and the cells' answers", async () => {
    const body = await readBody(page(), load);
    expect(body.messages).toEqual([
      { who: 'owner', at: 10_000, to: ['R1'], quote: 'Keep the old table.', text: 'Why not the old table?' },
      { who: 'R1', at: 13_000, to: ['owner'], quote: null, text: 'It has no room for the cost.' },
    ]);
  });
});
