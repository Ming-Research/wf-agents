import { describe, expect, it } from 'vitest';

import type { Block as FlowBlock, Flow } from '../api';
import { outlineOf } from '../testing';
import type { Step, Version, VersionPage, Wait, WorkPage } from '../works-api';
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

/// A version written at this place, the one of this number among its block's versions, which its content is read by.
const version = (at: number, number: number, by: Version['by'], chars: number, saidTo: string | null = null): Version => ({
  at,
  number,
  time: at * 1000,
  by,
  meaning: number,
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
      'background/1': 'Not on the card.',
      'why/1': 'Because it was asked.',
      'how/1': 'By hand, first.',
      'how/2': 'By hand.',
      'plan_owner/1': 'Not on the card either.',
    };
    const page: WorkPage = {
      summary: { ...item.work, flow: 'any' },
      title: item.title,
      level: item.level,
      project: item.project,
      history: item.history,
      outline: outlineOf(flow),
      flow,
      cells: [],
      blocks: [
        { name: 'background', versions: [version(1, 1, { kind: 'owner' }, 16)] },
        { name: 'why', versions: [version(2, 1, by, 21)] },
        { name: 'how', versions: [version(3, 1, by, 15), version(4, 2, by, 8)] },
        { name: 'plan_owner', versions: [version(5, 1, by, 23)] },
      ],
      steps: [],
    };
    const load = (_: number, block: string, number: number): Promise<VersionPage> =>
      Promise.resolve({ block, number, content: contents[`${block}/${number}`] ?? '' });
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
      'implement_words/1': 'The owner:\n\n> Greet them.\n\nBy the first name.',
      'implement_words/2': 'An agent:\n\nDone.',
    };
    const page: WorkPage = {
      summary: item.work,
      title: item.title,
      level: item.level,
      project: item.project,
      history: item.history,
      outline: outlineOf(SPOKEN),
      flow: SPOKEN,
      cells: [],
      blocks: [
        {
          name: 'implement_words',
          versions: [version(5, 1, { kind: 'owner' }, 60), version(9, 2, { kind: 'run', run: 3, cell: 'implement' }, 30)],
        },
      ],
      steps: [],
    };
    const load = (_: number, block: string, number: number): Promise<VersionPage> =>
      Promise.resolve({ block, number, content: contents[`${block}/${number}`] ?? '' });
    const body = await readBody(page, load);
    expect(body.messages).toEqual([
      { who: 'owner', at: 5000, to: ['implement'], quote: 'Greet them.', text: 'By the first name.', taken: false },
      { who: 'implement', at: 9000, to: ['owner'], quote: null, text: 'Done.' },
    ]);
    // A work that ended is given no flow: its card and its talk are read from its outline alone, as a work going on's.
    const ended = await readBody({ ...page, flow: null }, load);
    expect(ended).toEqual(body);
  });

  it("say of the owner's words whether a run of their cell has begun with them, or with a later version", async () => {
    const item = fixtureItems().items[0];
    const ran = (run: number, version: number): Step =>
      ({
        kind: 'run',
        run,
        cell: 'implement',
        at: run,
        time: run * 1000,
        because: [],
        basis: [{ block: 'implement_words', version }],
        rounds: [],
        calls: [],
        end: null,
        spent: {
          usdMicros: 0,
          inputTokens: 0,
          cacheWrittenTokens: 0,
          unsplitTokens: 0,
          cachedTokens: 0,
          outputTokens: 0,
          workingMs: 0,
          rounds: 0,
        },
      }) as Step;
    const page: WorkPage = {
      summary: { ...item.work, number: item.work.number + 2 },
      title: item.title,
      level: item.level,
      project: item.project,
      history: item.history,
      outline: outlineOf(SPOKEN),
      flow: SPOKEN,
      cells: [],
      blocks: [
        {
          name: 'implement_words',
          versions: [version(5, 1, { kind: 'owner' }, 25), version(8, 2, { kind: 'owner' }, 25)],
        },
      ],
      // A run began with the first version, and none yet with the second.
      steps: [ran(6, 1)],
    };
    const load = (_: number, block: string, number: number): Promise<VersionPage> =>
      Promise.resolve({ block, number, content: `The owner:\n\nSaid ${number}.` });
    const body = await readBody(page, load);
    expect(body.messages.map((m) => [m.text, m.taken])).toEqual([
      ['Said 1.', true],
      ['Said 2.', false],
    ]);
  });

  it("read each of the owner's versions as to the cell whose words they are, which its words do not name", async () => {
    const item = fixtureItems().items[0];
    const page: WorkPage = {
      // Another work than the one before: the content of a version, once read, is kept by its work, block and number.
      summary: { ...item.work, number: item.work.number + 1 },
      title: item.title,
      level: item.level,
      project: item.project,
      history: item.history,
      outline: outlineOf(SPOKEN),
      flow: SPOKEN,
      cells: [],
      blocks: [
        { name: 'implement_words', versions: [version(12, 1, { kind: 'owner' }, 25)] },
        { name: 'research_words', versions: [version(14, 1, { kind: 'owner' }, 25)] },
      ],
      steps: [],
    };
    const load = (_: number, block: string, number: number): Promise<VersionPage> =>
      Promise.resolve({ block, number, content: 'The owner:\n\nOne table.' });
    const body = await readBody(page, load);
    expect(body.messages).toEqual([
      { who: 'owner', at: 12000, to: ['implement'], quote: null, text: 'One table.', taken: false },
      { who: 'owner', at: 14000, to: ['research'], quote: null, text: 'One table.', taken: false },
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
      outline: outlineOf(flow),
      flow,
      cells: [],
      blocks: [
        { name: 'edits', versions: [{ ...version(4, 1, by, 3), empty: true }] },
        { name: 'more', versions: [{ ...version(5, 1, by, 0), value: 'yes', empty: false }] },
      ],
      steps: [],
    };
    const load = (_: number, block: string, number: number): Promise<VersionPage> =>
      Promise.resolve({ block, number, content: ' \n ' });
    expect(await readRulings(page, waits, load)).toEqual([
      { key: 'write-edits', why, block: 'edits', at: 4, values: [], now: null, holding: null, text: ' \n ' },
    ]);
  });
});
