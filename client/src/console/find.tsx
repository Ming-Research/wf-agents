// The console's search: a field over the tree's corner, under the boxes of which items the tree shows, that finds an
// item by its number, its path in the tree or a part of its title, over every item, open, done or cancelled
// (`findItems`); and under it a short list of what it finds, each row the item's path, its title and how it stands.
// The arrow keys move in the list, Enter or a click picks the row they stand on, which the console then shows, and
// Escape empties the field and gives the keys back. What is typed and not yet picked is kept in the browser (principle
// typed-words-are-kept).

import { type KeyboardEvent, type RefObject, useMemo, useState } from 'react';

import { read, write } from '../kept';
import type { Items } from '../works-api';
import { DOT, STANDING_WORDS } from './ItemWindow';
import { findItems, pathWords, standingOf } from './items';

/// The key under which the browser keeps what was typed in the search and not picked from.
const KEPT = 'wf-agents.console-find';

/// How many items the list shows at most.
const MOST = 8;

/// What the search says: of the field, on pointing at it and in it while it is empty; of a search that finds nothing.
export const FIND_SAYS = {
  title: 'Find an item by its number, its path in the tree, or a part of its title (/)',
  empty: 'Find an item',
  none: 'No item has that number, path or title',
} as const;

export function Find({ data, field, onPick }: {
  data: Items | undefined;
  /// The field, which `/` gives the keys to.
  field: RefObject<HTMLInputElement | null>;
  /// Told the number of the item picked.
  onPick: (id: number) => void;
}) {
  const [query, setQuery] = useState(() => read(KEPT) ?? '');
  // The row of the list the arrow keys stand on.
  const [at, setAt] = useState(0);
  const found = useMemo(() => (data === undefined ? [] : findItems(data, query, MOST)), [data, query]);
  const type = (q: string) => {
    setQuery(q);
    setAt(0);
    write(KEPT, q === '' ? null : q);
  };
  /// Empties the field and gives the keys back.
  const done = () => {
    type('');
    field.current?.blur();
  };
  const pick = (id: number) => {
    done();
    onPick(id);
  };
  const key = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      if (found.length > 0) setAt((a) => (a + (e.key === 'ArrowDown' ? 1 : found.length - 1)) % found.length);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const chosen = found[Math.min(at, found.length - 1)];
      if (chosen !== undefined) pick(chosen.item.number);
    } else if (e.key === 'Escape') {
      // The console's own Escape, which lets the item go, waits for the next.
      e.preventDefault();
      e.stopPropagation();
      done();
    }
  };
  return (
    <div className="console-find">
      <input
        ref={field}
        className="field"
        type="text"
        aria-label={FIND_SAYS.empty}
        title={FIND_SAYS.title}
        placeholder={FIND_SAYS.empty}
        value={query}
        onChange={(e) => type(e.target.value)}
        onKeyDown={key}
      />
      {query.trim() === '' ? null : found.length === 0 ? (
        <p className="console-found-none">{FIND_SAYS.none}</p>
      ) : (
        <ul className="console-found" role="listbox" aria-label="The items found">
          {found.map(({ item, path }, k) => {
            const standing = standingOf(item);
            return (
              <li
                key={item.number}
                role="option"
                aria-selected={k === at}
                className={k === at ? 'list-row console-found-row chosen' : 'list-row console-found-row'}
                onClick={() => pick(item.number)}
              >
                <span className="mono">{pathWords(path)}</span>
                <span className="console-found-title">{item.title}</span>
                <span className="console-found-standing">
                  <span className={DOT[standing]} aria-hidden="true" />
                  {STANDING_WORDS[standing]}
                </span>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
