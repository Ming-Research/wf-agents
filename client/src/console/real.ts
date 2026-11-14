// What the console shows of an item from the engine's own data, in the shapes of the made-up data (`mock.ts`), so
// that the drawing code only chooses where it takes them from: the mocks when the page shows them (`mocksShown`), and
// otherwise what the engine gives, with nothing where it gives nothing.
//
// From the engine, with the mocks off:
// - The body, from the work's blocks, the current version of each: Background is `background`; Plan is `plan_owner`,
//   and under it `questions`; there is no Problem. For an item of the `deliver` flow, Plan is `delivered`.
// - The messages: each version of the record of what is said, the block the cells write through the return's field
//   `reply` and the owner writes too, in the order it was written: one the owner wrote is the owner's, to the cell of
//   the block of the owner's words the engine wrote with it in the same act, at the place after it; one a cell wrote
//   is that cell's answer, to the owner.
// - A question a cell asked the owner: its sentence alone, answered in words, since the engine gives no options.
// - A ruling: for each precondition the work waits for, each block its test reads that people write whose current
//   version keeps the test from holding, with that version's writer and content (`readRulings`).
// Nothing, with the mocks off: an item's unseen mark, unread messages, open questions, tier and verdicts; a work's tier.

import { mocksShown } from '../mock-data';
import type { Test } from '../api';
import type { Item, Version, VersionPage, WorkPage, Writer } from '../works-api';
import { askedMock, bodyMock, type ItemMock, itemMock, workTierMock } from './mock';
import type { Body, Choice, Message, Part } from './window';

/// Reads the content of a version of a block.
export type LoadVersion = (work: number, block: string, at: number) => Promise<VersionPage>;

/// What a card and the window show of an item that the engine does not give: made up with the mocks, none without.
export function itemOf(item: Item): ItemMock {
  if (mocksShown()) return itemMock(item);
  return { unseen: false, unread: 0, openQuestions: 0, tier: null, verdicts: [] };
}

/// A question a cell asked the owner, as a choice at the plan's end: with made-up options with the mocks, and with none
/// without, to be answered in words.
export function askedChoice(question: string, call: number, cell: string): Choice {
  if (mocksShown()) return askedMock(question, call, cell);
  return { kind: 'choice', question, options: [], recommended: null, chosen: null, asks: { call, cell } };
}

/// The work's tier: made up with the mocks, none without.
export const workTierOf = (): string | null => (mocksShown() ? workTierMock() : null);

/// A body with nothing in it.
const EMPTY: Body = { background: [], problem: [], plan: [], planWaits: false, messages: [] };

/// The body the window shows before the work is read: the made-up one with the mocks, an empty one without, which
/// `readBody` fills.
export const firstBody = (item: Item): Body => (mocksShown() ? bodyMock(item) : EMPTY);

/// The contents of versions read, by work, block and place: a version never changes.
const read = new Map<string, Promise<string>>();

function contentOf(load: LoadVersion, work: number, block: string, at: number): Promise<string> {
  const key = `${work}:${block}@${at}`;
  let kept = read.get(key);
  if (kept === undefined) {
    kept = load(work, block, at).then((p) => p.content);
    // A read that failed is tried again the next time.
    void kept.catch(() => read.delete(key));
    read.set(key, kept);
  }
  return kept;
}

/// The versions of a block, in the order they were written.
const allVersionsOf = (page: WorkPage, block: string): Version[] =>
  [...(page.blocks.find((b) => b.name === block)?.versions ?? [])].sort((a, b) => a.at - b.at);

/// The versions of a block that have content, in the order they were written.
const versionsOf = (page: WorkPage, block: string): Version[] => allVersionsOf(page, block).filter((v) => v.chars > 0);

/// The source outside the work that writes the block of the owner's words to a cell as the owner speaks to its line.
const WORDS = 'words';

/// A message as the owner's box writes it: the passage it quotes, each line after `> `, a blank line, and the words.
function quoted(content: string): { quote: string | null; text: string } {
  const found = /^((?:>[^\n]*(?:\n|$))+)\n*([\s\S]*)$/.exec(content);
  if (found === null || found[2].trim() === '') return { quote: null, text: content };
  const quote = found[1]
    .split('\n')
    .filter((l) => l !== '')
    .map((l) => l.replace(/^> ?/, ''))
    .join('\n');
  return { quote, text: found[2].trim() };
}

/// The item's body and messages, from its work's blocks as the page read them.
export async function readBody(page: WorkPage, load: LoadVersion): Promise<Body> {
  const work = page.summary.number;
  // The current version of a block, as a passage; none where it has none or it is empty.
  const current = async (block: string): Promise<Part[]> => {
    const last = versionsOf(page, block).at(-1);
    if (last === undefined) return [];
    const text = await contentOf(load, work, block, last.at);
    return text.trim() === '' ? [] : [{ kind: 'text', text }];
  };
  const deliver = page.summary.flow === 'deliver' || page.flow.name === 'deliver';
  const [background, plan, questions] = await Promise.all([
    current('background'),
    current(deliver ? 'delivered' : 'plan_owner'),
    deliver ? Promise.resolve([]) : current('questions'),
  ]);
  // The record of what is said: written through the return's field `reply`, and by the owner.
  const record = page.flow.blocks.find((b) => b.field === 'reply' && b.writtenBy.people);
  // Of each place, the cell whose block of the owner's words the engine wrote there.
  const spokenAt = new Map<number, string>();
  for (const c of page.flow.cells) {
    for (const t of c.triggers) {
      const block = page.flow.blocks.find((b) => b.name === t.block);
      if (block?.writtenBy.outside !== WORDS) continue;
      for (const v of allVersionsOf(page, t.block)) spokenAt.set(v.at, c.id);
    }
  }
  const said =
    record === undefined
      ? []
      : await Promise.all(
          versionsOf(page, record.name).map(async (v): Promise<Message & { place: number }> => {
            const content = await contentOf(load, work, record.name, v.at);
            if (v.by.kind === 'run') return { who: v.by.cell, at: v.time, to: ['owner'], quote: null, text: content, place: v.at };
            const { quote, text } = quoted(content);
            const cell = spokenAt.get(v.at + 1);
            return { who: 'owner', at: v.time, to: cell === undefined ? [] : [cell], quote, text, place: v.at };
          }),
        );
  const messages = said
    .filter((m) => m.text.trim() !== '')
    .sort((a, b) => a.place - b.place)
    .map(({ who, at, to, quote, text }) => ({ who, at, to, quote, text }));
  return { background, problem: [], plan: [...plan, ...questions], planWaits: false, messages };
}

/// A block that keeps a precondition the work waits for from holding, which people may write and so the owner may
/// rule: its current version, who wrote it and what it says, and the value that makes the test hold, none where the
/// block's values do not make that plain.
export interface Ruling {
  block: string;
  at: number;
  by: Writer;
  text: string;
  value: string | null;
}

/// Whether a version of a block keeps a test that reads it from holding: for a test that holds when every one or at
/// least one block has its value, when the version has another; for one that holds when none has it, when the version
/// has it, and for `noneStanding` only as written by a person, the outside or a cell that is not skipped now.
function keeps(test: Test, v: Version, skipped: Set<string>): boolean {
  switch (test.kind) {
    case 'all':
    case 'any':
      return v.value !== test.is;
    case 'none':
      return v.value === test.is;
    case 'noneStanding':
      return v.value === test.is && (v.by.kind !== 'run' || !skipped.has(v.by.cell));
  }
}

/// The value of a block that makes a test that reads it hold: the test's own where it looks for one, and where it
/// looks for none, the block's only other value; none where that is not plain.
function holding(test: Test, values: string[] | null): string | null {
  if (test.kind === 'all' || test.kind === 'any') return values?.includes(test.is) === true ? test.is : null;
  const others = (values ?? []).filter((v) => v !== test.is);
  return others.length === 1 ? others[0] : null;
}

/// What the owner may rule of the preconditions the work waits for, from the flow's tests and the work's blocks: each
/// block an unmet test reads, of those people write, whose current version keeps the test from holding.
export async function readRulings(page: WorkPage, load: LoadVersion): Promise<Ruling[]> {
  const now = page.summary.now;
  if (now.kind !== 'waits') return [];
  const work = page.summary.number;
  const skipped = new Set(page.summary.cells.filter((c) => c.state === 'skipped').map((c) => c.cell));
  const found = new Map<string, { v: Version; value: string | null }>();
  for (const w of now.on) {
    if (w.kind !== 'precondition') continue;
    const test = page.flow.tests.find((t) => t.name === w.test);
    if (test === undefined) continue;
    for (const name of test.reads) {
      const block = page.flow.blocks.find((b) => b.name === name);
      if (block === undefined || !block.writtenBy.people || found.has(name)) continue;
      const v = (page.blocks.find((b) => b.name === name)?.versions ?? []).reduce<Version | undefined>(
        (last, x) => (last === undefined || x.at > last.at ? x : last),
        undefined,
      );
      if (v === undefined || !keeps(test, v, skipped)) continue;
      found.set(name, { v, value: holding(test, block.values) });
    }
  }
  return Promise.all(
    [...found].map(async ([block, { v, value }]) => ({
      block,
      at: v.at,
      by: v.by,
      text: v.chars > 0 ? await contentOf(load, work, block, v.at) : '',
      value,
    })),
  );
}
