// The console: the items as a tree in snapping columns on a canvas drawn by WebGL (`snap.ts`), as they stand, drawn
// again as they change; with `?fixtures` in the page's address, made-up items that show every state the drawing has
// (`fixtures.ts`): those that stand as the boxes over the tree's corner tick, open, done or cancelled, and what stands
// over them. A press on the funnel or the dot of an item's fold folds the items under it away or shows them again, as
// `e` does over its card, except while the owner types in a field or holds Ctrl, Alt or Meta: a choice kept in the
// browser for that item, which wins over the tree's own rule. Under the boxes, a search finds any item (`find.tsx`),
// which `/` gives the keys to as `e` folds; an item picked from it is selected as one the address names, its box ticked
// first where its kind is not. `n` goes to the next of what the cards' red and orange counts count, in the tree's
// order and in the order the window draws them, and `N` to the one before, each selected and brought to the top of the
// window as a pick is. At the tree's bottom left, a question mark shows what the marks and the keys mean
// (`help.tsx`). Beside the tree, a window shows the item selected (`ItemWindow.tsx`). Selecting an item slides the
// window out; letting it go slides the window back, unless the tab stuck to its left edge brought it out, in which case
// it stays until the tab takes it back. The tab takes it back without letting the item go, and the next item selected
// brings it out again. The window coming out over the item selected, or an item selected half under it or half off the
// screen, moves the tree only until the item is wholly within what the window leaves. Escape lets the item go while the
// tree has the keys, and slides the window in while it has them. Each time the interface says something happened, the
// items are read again, at most one read at a time and one more after it, and drawn where they stand, the item selected
// and the panel as they were; when the stream opens again after a cut, everything is. An item the address names is
// selected once the tree shows it, as from a notification of a question it waits on. The page's shell mounts it lazily
// (`lazy.tsx`).
//
// The window slides over the tree, which keeps its whole width under it (`Drawer`, in the page's parts, which the flow
// view's panel and a work's item on its page are in too).

import './console.css';

import { useEffect, useMemo, useRef, useState } from 'react';

import { Drawer } from '../parts';
import { CHOICE_KEYS, useKeptFolds, useKeptTicks } from '../kept';
import { WIDTH_KEYS } from '../sizes';
import type { Items, WorkPage } from '../works-api';
import { oneAtATime } from '../works/source';
import { goalMock } from './mock';
import { fakeShown } from './fake';
import { Find } from './find';
import { ConsoleHelp } from './help';
import { type ItemStanding, STANDINGS, type Shown, shownTree, standingOf, walkOrder } from './items';
import { ItemPanel, type PageState } from './ItemWindow';
import { SnapScene } from './snap';
import { type Heard, type Source, inOrder, pageSource } from './source';

/// Each kind of item as its box over the tree names it, and what pointing at the box says.
const SHOWS: Record<ItemStanding, { name: string; title: string }> = {
  open: { name: 'Open', title: 'The items not done or cancelled: running, paused or waiting' },
  done: { name: 'Done', title: 'The items whose work merged' },
  cancelled: { name: 'Cancelled', title: 'The items whose work was cancelled' },
};

export function ConsolePage({
  source: given,
  named,
  wait,
  active = true,
}: {
  source?: Source;
  named?: number;
  wait?: string;
  /// Whether the console is the view shown: kept while another is, it stops drawing and takes no keys.
  active?: boolean;
} = {}) {
  const source = useMemo(() => given ?? pageSource(), [given]);
  const panel = useRef<HTMLElement>(null);
  const host = useRef<HTMLDivElement>(null);
  const scene = useRef<SnapScene | null>(null);
  const [data, setData] = useState<Items | undefined>(undefined);
  const [failed, setFailed] = useState<string | null>(null);
  const [undrawn, setUndrawn] = useState<string | null>(null);
  const [selected, setSelected] = useState<Shown | null>(null);
  // The panel: whether it is out, and whether its handle brought it out, which keeps it out once the item is let go.
  const [win, setWin] = useState({ open: false, byHand: false });
  const open = win.open;
  // Which kinds of item the tree shows, open, done and cancelled each ticked or not, and what stands over them.
  const [shows, tick] = useKeptTicks(CHOICE_KEYS.consoleShows, ['open'], STANDINGS);
  const showsNow = useRef(shows);
  // Which items the owner folded and which he unfolded, as he last chose: the others fold as the tree's rule says.
  const [folds, turnFold] = useKeptFolds(CHOICE_KEYS.consoleFolds);
  const foldsNow = useRef(folds);
  const turnFoldNow = useRef(turnFold);
  useEffect(() => {
    turnFoldNow.current = turnFold;
  }, [turnFold]);

  // The items, read at first and again each time something happened, at most one read at a time.
  const heard = useRef<(h: Heard) => void>(() => undefined);
  useEffect(() => {
    let alive = true;
    const reads = oneAtATime(() =>
      source.loadItems().then(
        (items) => {
          if (!alive) return;
          setData(items);
          setFailed(null);
        },
        (e: unknown) => alive && setFailed(e instanceof Error ? e.message : String(e)),
      ),
    );
    reads.ask();
    const stop = source.changes((h) => {
      if (h === 'everything' || h.items || h.works.length > 0) reads.ask();
      heard.current(h);
    });
    return () => {
      alive = false;
      reads.stop();
      stop();
    };
  }, [source]);

  useEffect(() => {
    const el = host.current;
    if (el === null) return;
    const s = new SnapScene();
    let alive = true;
    s.mount(el).then(
      () => {
        if (!alive) return;
        scene.current = s;
        let was: number | null = null;
        s.onSelect = (item) => {
          setSelected(item);
          // The scene tells the item again each time it draws new data; only another item moves the panel.
          const id = item?.id ?? null;
          if (id === was) return;
          was = id;
          // The address says the item selected while the console is shown, so that its tab leads back to it (decision
          // a-views-choices-are-kept-in-the-browser); an address of one of its waits stands while it is selected.
          if (activeNow.current) {
            const at = id === null ? '#/console' : `#/console/items/${id}`;
            const hash = window.location.hash;
            if (hash !== at && !(id !== null && hash.startsWith(`${at}/waits/`))) window.location.replace(at);
          }
          setWin((w) => {
            if (id !== null) return w.open ? w : { open: true, byHand: false };
            return w.byHand ? w : { open: false, byHand: false };
          });
        };
        // A press on the funnel or the dot of an item's fold is the owner's choice for that item, kept, and the tree is
        // drawn again.
        s.onFold = (id, folded) => turnFoldNow.current(id, folded);
        if (latestData.current !== undefined)
          s.show(shownTree(latestData.current, showsNow.current, foldsNow.current));
        selectNamed();
        // To try the drawing from the browser's console on the dev server: `consoleScene.show(consoleFake(2000))` draws
        // a made-up tree.
        if (import.meta.env.DEV) Object.assign(window, { consoleScene: s, consoleFake: fakeShown });
      },
      // A browser that cannot draw the tree (no WebGL) says so in its place.
      (e: unknown) => alive && setUndrawn(e instanceof Error ? e.message : String(e)),
    );
    return () => {
      alive = false;
      scene.current = null;
      s.destroy();
    };
  }, []);

  // The items as last read, for a scene made after they came.
  const latestData = useRef<Items | undefined>(undefined);
  // The item the address names, until the tree shows it and it is selected.
  const toSelect = useRef<number | undefined>(named);
  // Selects the item the address names once the tree shows it; where its kind is not ticked, the box is ticked first and
  // the item is selected as the tree is drawn again, as for an item picked from the search.
  const selectNamed = () => {
    const id = toSelect.current;
    if (id === undefined || scene.current === null) return;
    const found = latestData.current?.items.find((i) => i.number === id);
    if (found === undefined) return;
    const standing = standingOf(found);
    if (!showsNow.current.has(standing)) {
      tick(standing, true);
      return;
    }
    toSelect.current = undefined;
    scene.current.select(id);
  };
  useEffect(() => {
    latestData.current = data;
    showsNow.current = shows;
    foldsNow.current = folds;
    if (data !== undefined) scene.current?.show(shownTree(data, shows, folds));
    selectNamed();
  }, [data, shows, folds]);
  useEffect(() => {
    toSelect.current = named;
    selectNamed();
  }, [named]);
  // The search's field, which `/` gives the keys to. An item picked from it is selected as one the address names: the
  // folds over it open, and where its kind is not ticked, its box is ticked first, so that the tree shows it.
  const findField = useRef<HTMLInputElement>(null);
  const pick = (id: number) => {
    if (latestData.current?.items.some((i) => i.number === id) !== true) return;
    toSelect.current = id;
    selectNamed();
  };

  // The walk of the stops by n and N, and by the button under each stop: the item the walk last went to and the place
  // among its stops, -1 for its last; and the stop the window is asked to bring to the top of what it shows. The walk
  // goes in the tree's order, each item before the items under it (`walkOrder`), and from the last stop to the first,
  // and back. How many stops an item has is what its counts say, which the window draws only once it has read the work.
  const walk = useRef<{ item: number; at: number } | null>(null);
  const [stopAsked, setStopAsked] = useState<{ item: number; at: number; seq: number } | null>(null);
  // The item selected, as the scene holds it at this moment, which the page's own state follows a render later.
  const selectedNow = () => scene.current?.selectedId() ?? null;
  const goStop = (item: number, at: number) => {
    walk.current = { item, at };
    setStopAsked((was) => ({ item, at, seq: (was?.seq ?? 0) + 1 }));
    if (item !== selectedNow()) pick(item);
  };
  const walkOn = (way: 1 | -1) => {
    const d = latestData.current;
    if (d === undefined) return;
    const order = walkOrder(shownTree(d, new Set(STANDINGS), foldsNow.current));
    const begin = way > 0 ? 0 : -1;
    const stopsOf = (id: number) => order.find((o) => o.id === id)?.stops ?? 0;
    // The next item with stops from the item `from`, `way` on in the tree's order, round from the end to the start.
    const after = (from: number) => {
      const at = order.findIndex((o) => o.id === from);
      for (let k = 1; k <= order.length; k += 1) {
        const o = order[(((at + way * k) % order.length) + order.length) % order.length];
        if (o.stops > 0) return o.id;
      }
      return null;
    };
    const w = walk.current;
    const sel = selectedNow();
    if (w !== null && w.item === sel) {
      const count = stopsOf(w.item);
      const at = (w.at < 0 ? count - 1 : w.at) + way;
      if (at >= 0 && at < count) return goStop(w.item, at);
      const next = after(w.item);
      if (next !== null) goStop(next, begin);
      return;
    }
    // A walk begins at the item selected: at its own stops where it has any, else at the next item that has.
    if (sel !== null && stopsOf(sel) > 0) return goStop(sel, begin);
    const from = sel ?? (way > 0 ? order[order.length - 1]?.id : order[0]?.id);
    const next = from === undefined ? null : after(from);
    if (next !== null) goStop(next, begin);
  };
  const walkOnNow = useRef(walkOn);
  useEffect(() => {
    walkOnNow.current = walkOn;
  });

  /// The width of the tree that the window leaves to be seen: all of it while the window is in.
  const room = (out: boolean) => {
    const tree = host.current?.clientWidth ?? 0;
    return out ? tree - (panel.current?.offsetWidth ?? 0) : tree;
  };
  const selectedId = selected?.id ?? null;
  useEffect(() => {
    if (open && selectedId !== null) scene.current?.reveal(selectedId, room(true));
  }, [open, selectedId]);

  // Whether the console is shown, for the scene's handler, made once.
  const activeNow = useRef(active);
  useEffect(() => {
    activeNow.current = active;
    scene.current?.setShown(active);
  }, [active]);

  useEffect(() => {
    if (!active) return;
    const key = (e: KeyboardEvent) => {
      // `e` folds the items under the card the pointer is over away, or shows them again, as a click on its funnel or
      // its dot does; `/` gives the keys to the search; `n` goes to the next stop and `N` to the one before. None while
      // the owner types in a field, nor with Ctrl, Alt or Meta held.
      if (e.key === 'e' || e.key === '/' || e.key === 'n' || e.key === 'N') {
        const to = e.target as HTMLElement | null;
        if (e.ctrlKey || e.metaKey || e.altKey) return;
        if (to && (to.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(to.tagName))) return;
        if (e.key === '/') {
          e.preventDefault();
          findField.current?.focus();
        } else if (e.key === 'n' || e.key === 'N') {
          e.preventDefault();
          walkOnNow.current(e.key === 'n' ? 1 : -1);
        } else if (scene.current?.foldPointedCard() === true) e.preventDefault();
        return;
      }
      // Escape answers a question the page asks, and nothing else.
      if (e.key !== 'Escape' || document.querySelector('dialog[open]') !== null) return;
      const inside = panel.current?.contains(document.activeElement) === true;
      if (open && inside) {
        (document.activeElement as HTMLElement | null)?.blur();
        setWin({ open: false, byHand: false });
      } else scene.current?.select(null);
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, [open, active]);

  // The item selected as the interface gives it, and its work's page, read when another is selected and again when
  // something happened to that work; an answer older than the one shown is dropped, and a read that failed says so.
  const item = selectedId === null ? undefined : data?.items.find((i) => i.number === selectedId);
  const work = item?.work.number;
  const [page, setPage] = useState<{ work: number; state: PageState } | null>(null);
  const takePage = useMemo(() => inOrder(), []);
  useEffect(() => {
    if (work === undefined) return;
    const read = () =>
      takePage(
        source.loadWork(work),
        (p: WorkPage) => setPage({ work, state: { page: p } }),
        (why) => setPage({ work, state: { failed: why } }),
      );
    read();
    heard.current = (h) => {
      if (h === 'everything' || h.works.includes(work)) read();
    };
    return () => {
      heard.current = () => undefined;
    };
  }, [work, source, takePage]);
  const shownPage = page !== null && page.work === work ? page.state : null;

  const fold = () => setWin((w) => (w.open ? { open: false, byHand: false } : { open: true, byHand: true }));

  return (
    <div className="console">
      {/* A press on the tree gives it the keys back from the window. */}
      <div
        className="console-canvas"
        ref={host}
        onPointerDown={() => (document.activeElement as HTMLElement | null)?.blur()}
      >
        <div className="console-over">
          <div className="console-shows" role="group" aria-label="Which items the tree shows">
            {STANDINGS.map((s) => (
              <label key={s} className="view-toggle" title={SHOWS[s].title}>
                <input type="checkbox" checked={shows.has(s)} onChange={(e) => tick(s, e.target.checked)} />
                {SHOWS[s].name}
              </label>
            ))}
          </div>
          <Find data={data} field={findField} onPick={pick} />
          {failed !== null && <p className="console-message">The page could not read the items: {failed}</p>}
          {undrawn !== null && <p className="console-message">The browser could not draw the tree: {undrawn}</p>}
        </div>
        <ConsoleHelp />
      </div>
      <Drawer
        open={open}
        onToggle={fold}
        panel={panel}
        label="The item selected"
        hide="Put the item away"
        show="Show the item selected"
        widthKey={WIDTH_KEYS.itemWindow}
      >
        {item === undefined ? (
          <p className="cw-empty">Click an item on the left to see it here.</p>
        ) : (
          <ItemPanel
            item={item}
            page={shownPage}
            act={source.act}
            loadVersion={source.loadVersion}
            wait={wait}
            goal={selected !== null && goalMock(selected.work !== null, selected.children.length)}
            open={open && active}
            onRead={() =>
              latestData.current !== undefined &&
              scene.current?.show(shownTree(latestData.current, showsNow.current, foldsNow.current))
            }
            onTitle={() => {
              scene.current?.reveal(item.number, room(open));
              scene.current?.flash(item.number);
            }}
            stop={stopAsked !== null && stopAsked.item === item.number ? stopAsked : undefined}
            onStopChosen={(at) => {
              walk.current = { item: item.number, at };
            }}
            onNextStop={(at) => {
              walk.current = { item: item.number, at };
              walkOn(1);
            }}
          />
        )}
      </Drawer>
    </div>
  );
}
