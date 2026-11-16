import { describe, expect, it } from 'vitest';

import type { Block as FlowBlock, Flow } from '../api';
import type { Version, VersionPage, Wait, WorkPage } from '../works-api';
import { FLOW, fixtureItems } from './fixtures';
import { readBody, readRulings } from './real';

/// The words of a cell's own, which the engine's source of the owner's words writes and the cell writes too, in the
/// texts of their file.
const wordsOf = (cell: string): FlowBlock => ({
  name: `${cell}_words`,
  values: null,
  default: null,
  what: null,
  writtenBy: { cells: [cell], people: false, note: false, outside: 'words' },
  readBy: { cells: [cell], conditions: [], tests: [], shown: false },
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
});

/// The made-up flow with words of their own of the research and of the implementation, which each writes and watches.
const SPOKEN: Flow = {
  ...FLOW,
  cells: FLOW.cells.map((c) =>
    c.id === 'implement' || c.id === 'research' ? { ...c, outputs: [...c.outputs, `${c.id}_words`] } : c,
  ),
  blocks: [...FLOW.blocks, wordsOf('research'), wordsOf('implement')],
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

describe("the item's body, read from the data the flow's card names", () => {
  it('is a section for each datum the card names, in its order and by its name, with its current version, whatever the flow is called', async () => {
    const item = fixtureItems().items[0];
    const plain = (name: string): FlowBlock => ({
      name,
      values: null,
      default: null,
      what: null,
      writtenBy: { cells: ['research'], people: false, note: false, outside: null },
      readBy: { cells: [], conditions: [], tests: [], shown: true },
    });
    // The card names `how` before `why`, and a datum nothing has written yet; `background` and `plan_owner` stand in
    // the flow and are on no card.
    const flow: Flow = {
      ...FLOW,
      name: 'any',
      card: ['how', 'why', 'later'],
      blocks: [...FLOW.blocks, plain('why'), plain('how'), plain('later'), plain('background'), plain('plan_owner')],
    };
    const by = { kind: 'run', run: 1, cell: 'research' } as const;
    const contents: Record<string, string> = {
      'background@1': 'Not on the card.',
      'why@2': 'Because it was asked.',
      'how@3': 'By hand, first.',
      'how@4': 'By hand.',
      'plan_owner@5': 'Not on the card either.',
    };
    const page: WorkPage = {
      summary: { ...item.work, flow: 'any' },
      title: item.title,
      level: item.level,
      project: item.project,
      history: item.history,
      flow,
      cells: [],
      blocks: [
        { name: 'background', versions: [version(1, { kind: 'owner' }, 16)] },
        { name: 'why', versions: [version(2, by, 21)] },
        { name: 'how', versions: [version(3, by, 15), version(4, by, 8)] },
        { name: 'plan_owner', versions: [version(5, by, 23)] },
      ],
      steps: [],
    };
    const load = (_: number, block: string, at: number): Promise<VersionPage> =>
      Promise.resolve({ block, at, content: contents[`${block}@${at}`] ?? '' });
    const body = await readBody(page, load);
    expect(body.sections).toEqual([
      { title: 'how', parts: [{ kind: 'text', text: 'By hand.' }] },
      { title: 'why', parts: [{ kind: 'text', text: 'Because it was asked.' }] },
      { title: 'later', parts: [] },
    ]);
  });
});

describe("the messages of a work, read from each cell's own words", () => {
  it("are the owner's versions, each to the cell whose words they are, and the cells' answers, with the words alone", async () => {
    const item = fixtureItems().items[0];
    const contents: Record<string, string> = {
      'implement_words@5': 'The owner:\n\n> Greet them.\n\nBy the first name.',
      'implement_words@9': 'An agent:\n\nDone.',
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
        {
          name: 'implement_words',
          versions: [version(5, { kind: 'owner' }, 60), version(9, { kind: 'run', run: 3, cell: 'implement' }, 30)],
        },
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

  it("read each of the owner's versions as to the cell whose words they are, which its words do not name", async () => {
    const item = fixtureItems().items[0];
    const page: WorkPage = {
      summary: item.work,
      title: item.title,
      level: item.level,
      project: item.project,
      history: item.history,
      flow: SPOKEN,
      cells: [],
      blocks: [
        { name: 'implement_words', versions: [version(12, { kind: 'owner' }, 25)] },
        { name: 'research_words', versions: [version(14, { kind: 'owner' }, 25)] },
      ],
      steps: [],
    };
    const load = (_: number, block: string, at: number): Promise<VersionPage> =>
      Promise.resolve({ block, at, content: 'The owner:\n\nOne table.' });
    const body = await readBody(page, load);
    expect(body.messages).toEqual([
      { who: 'owner', at: 12000, to: ['implement'], quote: null, text: 'One table.' },
      { who: 'owner', at: 14000, to: ['research'], quote: null, text: 'One table.' },
    ]);
  });
});

describe('what the owner may rule of a precondition the work waits for', () => {
  it("is each block the engine's list says the owner may write, with what its latest version says, and no other", async () => {
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
    // The engine found which blocks keep the test from holding, by what it recorded of each: only `edits`, which holds
    // nothing but whitespace; `more`, a choice with no words beside it, holds its value.
    const why = 'research requires test = changes, which edits keeps from holding.';
    const waits: Wait[] = [{ key: 'write-edits', why, ends: { kind: 'write', block: 'edits', value: null } }];
    const page: WorkPage = {
      summary: {
        ...item.work,
        now: { kind: 'waits', at: 'research', on: [{ kind: 'precondition', test: 'changes', words, by: [{ kind: 'owner' }] }] },
        waits,
      },
      title: item.title,
      level: item.level,
      project: item.project,
      history: item.history,
      flow,
      cells: [],
      blocks: [
        { name: 'edits', versions: [{ ...version(4, by, 3), empty: true }] },
        { name: 'more', versions: [{ ...version(5, by, 0), value: 'yes', empty: false }] },
      ],
      steps: [],
    };
    const load = (_: number, block: string, at: number): Promise<VersionPage> =>
      Promise.resolve({ block, at, content: ' \n ' });
    expect(await readRulings(page, waits, load)).toEqual([
      { key: 'write-edits', why, block: 'edits', at: 4, values: [], now: null, holding: null, text: ' \n ' },
    ]);
  });
});
