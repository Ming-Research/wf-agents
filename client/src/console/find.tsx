// The console's search, which shows only while it is used: `/` opens it over the middle of the console's top, where
// what it finds has room under it. It finds an item by its number or a part of its title, over every item, open, done
// or cancelled (`findItems`), and lists what it finds, each row the item's number, its title and how it stands. While it
// is open the tree is dimmed, but for what it finds, lit as round the item under the pointer. The arrow keys or the
// pointer move in the list, which changes nothing in the tree, and Enter or a click picks the row they stand on, which
// the console then shows. A pick, Escape, or a press anywhere else closes it. What is typed and not yet picked is kept in the browser, and shows again
// the next time it opens (principle typed-words-are-kept); a pick or Escape empties it.

import { type KeyboardEvent, useEffect, useMemo, useRef, useState } from 'react';

import { read, write } from '../kept';
import type { Items } from '../works-api';
import { DOT, STANDING_WORDS } from './ItemWindow';
import { findItems, standingOf } from './items';

/// The key under which the browser keeps what was typed in the search and not picked from.
const KEPT = 'wf-agents.console-find';

/// How many items the list shows at most.
const MOST = 8;

/// What the search says: of the field, on pointing at it and in it while it is empty; of a search that finds nothing.
export const FIND_SAYS = {
  title: 'Find an item by its number or a part of its title',
  empty: 'Find an item',
  none: 'No item has that number or title',
} as const;

export function Find({ data, onPick, onClose, onFound }: {
  data: Items | undefined;
  /// Told the number of the item picked.
  onPick: (id: number) => void;
  /// Told that the search is to close: at a pick, at Escape, and when the keys go anywhere else.
  onClose: () => void;
  /// Told the numbers of the items found as they change, from none as it opens, and that it is closed as it closes.
  onFound: (ids: number[] | null) => void;
}) {
  const [query, setQuery] = useState(() => read(KEPT) ?? '');
  // The row of the list the arrow keys stand on.
  const [at, setAt] = useState(0);
  const found = useMemo(() => (data === undefined ? [] : findItems(data, query, MOST)), [data, query]);
  // What the console is told, as the latest render gave it.
  const tell = useRef({ onFound });
  useEffect(() => {
    tell.current = { onFound };
  });
  useEffect(() => tell.current.onFound(found.map((i) => i.number)), [found]);
  useEffect(() => () => tell.current.onFound(null), []);
  const type = (q: string) => {
    setQuery(q);
    setAt(0);
    write(KEPT, q === '' ? null : q);
  };
  /// Empties the field and closes the search.
  const done = () => {
    type('');
    onClose();
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
      if (chosen !== undefined) pick(chosen.number);
    } else if (e.key === 'Escape') {
      // The console's own Escape, which lets the item go, waits for the next.
      e.preventDefault();
      e.stopPropagation();
      done();
    }
  };
  return (
    <div className="console-find" role="search">
      <input
        className="field"
        type="text"
        aria-label={FIND_SAYS.empty}
        title={FIND_SAYS.title}
        placeholder={FIND_SAYS.empty}
        value={query}
        autoFocus
        onFocus={(e) => e.currentTarget.select()}
        onBlur={onClose}
        onChange={(e) => type(e.target.value)}
        onKeyDown={key}
      />
      {query.trim() === '' ? null : (
        // A press on the list keeps the keys in the field, so that it is not closed before the row is picked.
        <div className="console-found" onMouseDown={(e) => e.preventDefault()}>
          {found.length === 0 ? (
            <p className="console-found-none">{FIND_SAYS.none}</p>
          ) : (
            <ul role="listbox" aria-label="The items found">
              {found.map((item, k) => {
                const standing = standingOf(item);
                return (
                  <li
                    key={item.number}
                    role="option"
                    aria-selected={k === at}
                    className={k === at ? 'list-row console-found-row chosen' : 'list-row console-found-row'}
                    onMouseEnter={() => setAt(k)}
                    onClick={() => pick(item.number)}
                  >
                    <span className="mono">#{item.number}</span>
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
      )}
    </div>
  );
}
