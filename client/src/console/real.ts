// What the console shows of an item from the engine's own data, in the shapes of the made-up data (`mock.ts`), so
// that the drawing code only chooses where it takes them from: the mocks when the page shows them (`mocksShown`), and
// otherwise what the engine gives, with nothing where it gives nothing.
//
// From the engine, with the mocks off:
// - The body, from the work's blocks: a section for each datum the flow's card names, in its order and by its name,
//   with its current version. The questions put to the owner are not read from each cell's questions: the panel draws
//   each from the work's page, once, where a text points to it or else at the end of the last section.
// - The messages: of each cell's own words (a block among its outputs the engine's source of the owner's words
//   writes), each version the engine wrote is the owner's, to that cell, and each the cell's run wrote is its answer,
//   to the owner; each with the words alone, read back out of the version by the block's `owner` or `reply` text; of a
//   flow before cells had words of their own, each version of a block people write as a note to one cell (one people
//   write, with no fixed values, that one cell reads) is the owner's, to that cell. In the order of their times.
// - A ruling: for each block the engine's list of what waits on the owner says the owner may write, its latest version's
//   content (`readRulings`).
// Nothing, with the mocks off: an item's open questions, tier and verdicts; a work's tier. What the owner has not read of
// an item is the engine's (`Summary.fresh`, `Summary.repliesUnread`).

import { mocksShown } from '../mock-data';
import type { SentPiece } from '../api';
import type { Item, Version, VersionPage, Wait, WorkPage } from '../works-api';
import { bodyMock, type ItemMock, itemMock, workTierMock } from './mock';
import { type Body, type Message, type Part, type ReadVersion, passagesOf } from './window';

/// Reads the content of a version of a block, by its number among the block's versions.
export type LoadVersion = (work: number, block: string, number: number) => Promise<VersionPage>;

/// What a card and the window show of an item: with the mocks, made up; without, what the engine says the owner has not
/// read, and none of what it does not give.
export function itemOf(item: Item): ItemMock {
  if (mocksShown()) return itemMock(item);
  return {
    fresh: item.work.fresh ?? 'none',
    repliesUnread: item.work.repliesUnread ?? 0,
    tier: null,
    verdicts: [],
  };
}

/// The work's tier: made up with the mocks, none without.
export const workTierOf = (): string | null => (mocksShown() ? workTierMock() : null);

/// A body with nothing in it.
const EMPTY: Body = { sections: [], choicesWait: false, messages: [] };

/// The body the window shows before the work is read: the made-up one with the mocks, an empty one without, which
/// `readBody` fills.
export const firstBody = (item: Item): Body => (mocksShown() ? bodyMock(item) : EMPTY);

/// The contents of versions read, by work, block and number: a version never changes.
const read = new Map<string, Promise<string>>();

function contentOf(load: LoadVersion, work: number, block: string, number: number): Promise<string> {
  const key = `${work}:${block}/${number}`;
  let kept = read.get(key);
  if (kept === undefined) {
    kept = load(work, block, number).then((p) => p.content);
    // A read that failed is tried again the next time.
    void kept.catch(() => read.delete(key));
    read.set(key, kept);
  }
  return kept;
}

/// The versions of a block that have content, in the order they were written.
const versionsOf = (page: WorkPage, block: string): Version[] =>
  (page.blocks.find((b) => b.name === block)?.versions ?? []).filter((v) => v.chars > 0).sort((a, b) => a.at - b.at);

/// What fills each place of an entry of the record, by the place's name, as the engine wrote the entry from one of the
/// record's texts, `owner` or `reply`: the words between the text's own, the last place taking the rest; none where the
/// entry does not read so.
function placesOf(entry: string, text: SentPiece[] | undefined): Map<string, string> | null {
  if (text === undefined) return null;
  const last = text.map((p) => p.piece).lastIndexOf('slot');
  const escaped = (t: string) => t.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const pattern = text
    .map((p, i) => (p.piece === 'text' ? escaped(p.text) : i === last ? '([\\s\\S]*)' : '([\\s\\S]*?)'))
    .join('');
  const found = new RegExp(`^${pattern}$`).exec(entry);
  if (found === null) return null;
  const names = text.flatMap((p) => (p.piece === 'slot' ? [p.name] : []));
  return new Map(names.map((name, i): [string, string] => [name, found[i + 1]]));
}

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
  // The current version of a block, as passages: those it has that the last version the owner read has no equal of
  // marked changed; none where it has none or it is empty.
  const current = async (block: string): Promise<Part[]> => {
    const versions = versionsOf(page, block);
    const last = versions.at(-1);
    if (last === undefined) return [];
    const text = await contentOf(load, work, block, last.number);
    if (text.trim() === '') return [];
    const seen = versions.filter((v) => v.read === true).at(-1);
    return passagesOf(text, seen === undefined || seen === last ? null : await contentOf(load, work, block, seen.number));
  };
  // A section for each datum the card of the version the work runs on names, in its order and by its name.
  const sections = await Promise.all(page.outline.card.map(async (title) => ({ title, parts: await current(title) })));
  // What reading the item marks read: the latest version with something in it that a run wrote of each datum the card
  // names, where it is not read, as the engine counts what is new.
  const toRead = page.outline.card.flatMap((block): ReadVersion[] => {
    const latest = versionsOf(page, block)
      .filter((v) => v.by.kind === 'run' && !v.empty)
      .at(-1);
    return latest === undefined || latest.read === true ? [] : [{ block, version: latest.number }];
  });
  // Each cell's own words, as that version names them: the owner's versions, to the cell, and the cell's answers, to
  // the owner.
  const words = page.outline.cells.flatMap((c) => {
    const block = c.words === null ? undefined : { name: c.words.block, writes: c.words.writes };
    return block === undefined ? [] : versionsOf(page, block.name).map((v) => ({ v, block, cell: c.id }));
  });
  // Whether a run of the cell has begun with this version of the block it is said in, or a later one.
  const takenBy = (cell: string, block: string, version: number): boolean =>
    page.steps.some(
      (s) => s.kind === 'run' && s.cell === cell && (s.basis.find((b) => b.block === block)?.version ?? 0) >= version,
    );
  const said = await Promise.all([
    ...words.map(async ({ v, block, cell }): Promise<Message & { place: number }> => {
      const entry = await contentOf(load, work, block.name, v.number);
      const text = (name: string) => block.writes?.find(([n]) => n === name)?.[1];
      if (v.by.kind === 'run') {
        const answer = placesOf(entry, text('reply'))?.get('reply') ?? entry;
        const reply = v.reply === true ? { reply: { block: block.name, version: v.number, read: v.read === true } } : {};
        return { who: cell, at: v.time, to: ['owner'], quote: null, text: answer, place: v.at, ...reply };
      }
      const { quote, text: said } = quoted(placesOf(entry, text('owner'))?.get('words') ?? entry);
      const taken = takenBy(cell, block.name, v.number);
      return { who: 'owner', at: v.time, to: [cell], quote, text: said, taken, place: v.at };
    }),
  ]);
  // A cell's words are part of the talk where they are a reply to the owner, as the engine's one definition of a reply
  // says (`Version.reply`): what a cell writes with nobody having spoken to it has no words of the owner's to stand
  // under, and is left out, as is an answer that says nothing.
  const messages = said
    .filter((m) => (m.who === 'owner' ? m.text.trim() !== '' : m.reply !== undefined))
    .sort((a, b) => a.at - b.at || a.place - b.place)
    .map(({ who, at, to, quote, text, taken, reply }) => ({
      who,
      at,
      to,
      quote,
      text,
      ...(taken === undefined ? {} : { taken }),
      ...(reply === undefined ? {} : { reply }),
    }));
  return { sections, choicesWait: false, messages, toRead };
}

/// A block the owner may write that keeps a precondition the work waits for from holding, as the engine's list of what
/// waits on the owner names it: its key in that list and why, the block, its values and the one it has now, the value
/// that makes the precondition hold where the engine says one, and what its latest version says.
export interface Ruling {
  key: string;
  why: string;
  block: string;
  /// The place of its latest version; none where it has none.
  at: number | null;
  values: string[];
  now: string | null;
  holding: string | null;
  text: string;
}

/// What the owner may rule of the preconditions the work waits for, as the engine's list names them: each block it says
/// the owner may write, with what its latest version says.
export async function readRulings(page: WorkPage, waits: Wait[], load: LoadVersion): Promise<Ruling[]> {
  const work = page.summary.number;
  return Promise.all(
    waits.flatMap((w) => {
      const ends = w.ends;
      if (ends.kind !== 'write') return [];
      const v = (page.blocks.find((b) => b.name === ends.block)?.versions ?? []).reduce<Version | undefined>(
        (last, x) => (last === undefined || x.at > last.at ? x : last),
        undefined,
      );
      const values = page.flow?.blocks.find((b) => b.name === ends.block)?.values ?? [];
      const read = async (): Promise<Ruling> => ({
        key: w.key,
        why: w.why,
        block: ends.block,
        at: v?.at ?? null,
        values,
        now: v?.value ?? null,
        holding: ends.value,
        text: v !== undefined && v.chars > 0 ? await contentOf(load, work, ends.block, v.number) : '',
      });
      return [read()];
    }),
  );
}
