// The numbers of items in the text an agent or the owner wrote, drawn as links to the item: `#46`, and the numbers
// after the word `item`, `items` or `条目`. The titles come from the items the bar reads, once for the whole page
// (`ItemTitlesProvider`, in the `Shell`); a number that names no item the page read stays as it was written, and so
// does a number a letter, a digit or a path joins to what is round it, and one after PR, pull request or issue.

import { type ReactNode, createContext, useContext, useMemo } from 'react';

import { hashOf } from './route';
import type { Items } from './works-api';

/// The title of each item the page has read, by its number.
const ItemTitlesContext = createContext<ReadonlyMap<number, string>>(new Map());

/// Gives every part under it the titles of the items read, so that a number in a text can say which item it names.
export function ItemTitlesProvider({ items, children }: { items: Items | null; children: ReactNode }) {
  const titles = useMemo(() => new Map((items?.items ?? []).map((i) => [i.number, i.title])), [items]);
  return <ItemTitlesContext.Provider value={titles}>{children}</ItemTitlesContext.Provider>;
}

/// The titles of the items the page has read, by their numbers.
export const useItemTitles = () => useContext(ItemTitlesContext);

/// What may not stand just before or after a number for it to be one number of its own: an ASCII letter, a digit or an
/// underscore join it to a word, a slash or a hyphen or a dot to a path, a version or a date. Han characters and the
/// punctuation of Chinese text never join, as Chinese is written with no blanks.
const NUMBER = String.raw`[1-9]\d*(?![A-Za-z0-9_/]|[-.]\d)`;
/// What joins the numbers of a list after an item word.
const AND = String.raw`\s*(?:,\s*and|,|、|and|和|与)\s*`;
const LIST = `${NUMBER}(?:${AND}${NUMBER})*`;
/// An item number as the texts write it: `#46` on its own, or one or a list of them after the word `item`, `items` or
/// `条目`. A word is not joined to a letter before it (`\b`). Each branch ends with its numbers, so that a match's tail
/// is where they stand.
const WRITTEN = new RegExp(String.raw`(#)(${NUMBER})|\bitems?\s*(${LIST})|条目\s*(${LIST})`, 'gi');
/// A character that joins a `#` to what stands before it.
const JOINS_BEFORE = /[A-Za-z0-9_/.-]/;
/// What names another kind of numbered thing, before a number written with `#`.
const NOT_AN_ITEM = /(?:^|[^A-Za-z0-9_])(?:PR|pull request|issue)\s*$/i;

/// Where in a text a number of an item stands, as it is written: from where the `#` or the digits begin to where they
/// end, and the item's number.
type Written = { from: number; to: number; n: number };

/// Each item number a text writes, in order, of an item whose title is in `titles`.
function writtenIn(text: string, titles: ReadonlyMap<number, string>): Written[] {
  const found: Written[] = [];
  WRITTEN.lastIndex = 0;
  for (let m = WRITTEN.exec(text); m !== null; m = WRITTEN.exec(text)) {
    const hashed = m[1] === '#';
    if (hashed && (JOINS_BEFORE.test(text[m.index - 1] ?? '') || NOT_AN_ITEM.test(text.slice(0, m.index)))) continue;
    const numbers = (hashed ? m[2] : (m[3] ?? m[4])) as string;
    const start = m.index + m[0].length - numbers.length;
    for (const one of numbers.matchAll(/[1-9]\d*/g)) {
      const n = Number(one[0]);
      if (!titles.has(n)) continue;
      const from = start + (one.index ?? 0);
      found.push({ from: hashed ? from - 1 : from, to: from + one[0].length, n });
    }
  }
  return found;
}

/// The text with each item number it writes, of an item whose title is in `titles`, drawn as a link to the item in the
/// console, whose `title` says the item's; the rest as written. The link's address is built from the number alone.
export function linkItemNumbers(text: string, titles: ReadonlyMap<number, string>): ReactNode[] {
  if (titles.size === 0) return [text];
  const out: ReactNode[] = [];
  let at = 0;
  for (const { from, to, n } of writtenIn(text, titles)) {
    if (from > at) out.push(text.slice(at, from));
    out.push(
      <a key={from} className="link mono" href={hashOf({ view: 'console', item: n })} title={`#${n} ${titles.get(n)}`}>
        {text.slice(from, to)}
      </a>,
    );
    at = to;
  }
  if (at < text.length) out.push(text.slice(at));
  return out;
}
