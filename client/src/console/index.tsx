// The console: the items as a tree in snapping columns on a canvas drawn by WebGL (`snap.ts`), as they stand, drawn again
// as they change; with `?fixtures` in the page's address, made-up items that show every state the drawing has
// (`fixtures.ts`). Beside the tree, the panel shows the item selected (`ItemWindow.tsx`). Selecting an item brings the
// panel out; letting it go folds the panel away, unless its handle brought it out, in which case it stays until the
// handle folds it. The handle folds it without letting the item go, and the next item selected brings it out again.
// The panel coming out over the item selected, or an item selected half under it or half off the screen, moves the tree
// only until the item is wholly within what the panel leaves. Escape lets the item go while the tree has the keys, and
// folds the panel while it has them. Each time the interface says something happened, the items are read again and
// drawn where they stand, the item selected and the panel as they were.
//
// Brought over from the console of the engine on `main` (`client/src/pages/Console.tsx`). The window there slid over the
// tree; here it is the page's docked panel (look.md), which the tree gives its width to.

import './console.css';

import { type CSSProperties, useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { Edge, Panel } from '../parts';
import { useKeptWidth } from '../sizes';
import type { Changed, Items, WorkPage } from '../works-api';
import { fakeShown } from './fake';
import { type Shown, latest, shownTree, standingOf } from './items';
import { WindowBody, WindowLines, windowHead } from './ItemWindow';
import { SnapScene } from './snap';
import { type Source, pageSource } from './source';

/// The panel's width at first, as the console's window was, and the least and most it is dragged to, as the page's
/// panel is (look.md).
const PANEL = { first: 560, min: 280, max: 720 };
const WIDTH_KEY = 'wf-agents.console-panel-width';

export function ConsolePage({ source: given }: { source?: Source } = {}) {
  const source = useMemo(() => given ?? pageSource(), [given]);
  const frame = useRef<HTMLDivElement>(null);
  const host = useRef<HTMLDivElement>(null);
  const scene = useRef<SnapScene | null>(null);
  const [data, setData] = useState<Items | undefined>(undefined);
  const [failed, setFailed] = useState<string | null>(null);
  const [selected, setSelected] = useState<Shown | null>(null);
  // The panel: whether it is out, and whether its handle brought it out, which keeps it out once the item is let go.
  const [win, setWin] = useState({ open: false, byHand: false });
  const open = win.open;
  const [width, setWidth] = useKeptWidth(WIDTH_KEY, PANEL.min, PANEL.max);

  // The items, read at first and again each time something happened.
  const changed = useRef<(c: Changed) => void>(() => undefined);
  useEffect(() => {
    let alive = true;
    const read = () =>
      source.loadItems().then(
        (items) => {
          if (!alive) return;
          setData(items);
          setFailed(null);
        },
        (e: unknown) => alive && setFailed(e instanceof Error ? e.message : String(e)),
      );
    void read();
    const stop = source.changes((c) => {
      if (c.items || c.works.length > 0) void read();
      changed.current(c);
    });
    return () => {
      alive = false;
      stop();
    };
  }, [source]);

  useEffect(() => {
    const el = host.current;
    if (el === null) return;
    const s = new SnapScene();
    let alive = true;
    void s.mount(el).then(() => {
      if (!alive) return;
      scene.current = s;
      let was: number | null = null;
      s.onSelect = (item) => {
        setSelected(item);
        // The scene tells the item again each time it draws new data; only another item moves the panel.
        const id = item?.id ?? null;
        if (id === was) return;
        was = id;
        setWin((w) => {
          if (id !== null) return w.open ? w : { open: true, byHand: false };
          return w.byHand ? w : { open: false, byHand: false };
        });
      };
      if (latestData.current !== undefined) s.show(shownTree(latestData.current));
      // For trying the drawing from the browser's console: `consoleScene.show(consoleFake(2000))` draws a made-up tree.
      Object.assign(window, { consoleScene: s, consoleFake: fakeShown });
    });
    return () => {
      alive = false;
      scene.current = null;
      s.destroy();
    };
  }, []);

  // The items as last read, for a scene made after they came.
  const latestData = useRef<Items | undefined>(undefined);
  useEffect(() => {
    latestData.current = data;
    if (data !== undefined) scene.current?.show(shownTree(data));
  }, [data]);

  /// The width of the tree that the panel leaves to be seen: all of the canvas, which the docked panel narrows.
  const room = () => host.current?.clientWidth ?? 0;
  const selectedId = selected?.id ?? null;
  useEffect(() => {
    if (open && selectedId !== null) scene.current?.reveal(selectedId, room());
  }, [open, selectedId]);

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      const panel = frame.current?.querySelector('.drawer');
      const inside = panel?.contains(document.activeElement) === true;
      if (open && inside) {
        (document.activeElement as HTMLElement | null)?.blur();
        setWin({ open: false, byHand: false });
      } else scene.current?.select(null);
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, [open]);

  // The item selected as the interface gives it, and its latest work's page, read when another is selected and again
  // when something happened to that work.
  const item = selectedId === null ? undefined : data?.items.find((i) => i.number === selectedId);
  const work = item === undefined ? undefined : latest(item)?.number;
  const [page, setPage] = useState<WorkPage | null>(null);
  const reading = useRef<number | undefined>(undefined);
  const readPage = useCallback(
    (n: number) =>
      source.loadWork(n).then(
        (p) => {
          if (reading.current === n) setPage(p);
        },
        () => undefined,
      ),
    [source],
  );
  useEffect(() => {
    reading.current = work;
    if (work === undefined) return;
    void readPage(work);
    changed.current = (c) => {
      if (c.works.includes(work)) void readPage(work);
    };
    return () => {
      changed.current = () => undefined;
    };
  }, [work, readPage]);
  const shownPage = page !== null && page.summary.number === work ? page : null;

  const fold = () => setWin((w) => (w.open ? { open: false, byHand: false } : { open: true, byHand: true }));
  const edge = (
    <Edge
      side="left"
      label="Drag the panel's width"
      start={() => width ?? PANEL.first}
      onDrag={setWidth}
      onReset={() => setWidth(null)}
    />
  );
  const byNumber = new Map((data?.items ?? []).map((i) => [i.number, i]));
  const waitsFor =
    item === undefined ? [] : item.dependsOn.filter((d) => byNumber.has(d) && standingOf(byNumber.get(d)) !== 'done');

  return (
    <div
      ref={frame}
      className={open ? 'console' : 'console console-folded'}
      style={{ '--panel-width': `${width ?? PANEL.first}px` } as CSSProperties}
    >
      {/* A press on the tree gives it the keys back from the panel. */}
      <div
        className="console-canvas"
        ref={host}
        onPointerDown={() => (document.activeElement as HTMLElement | null)?.blur()}
      >
        {failed !== null && <p className="console-message">The page could not read the items: {failed}</p>}
      </div>
      {item === undefined ? (
        <Panel label="Nothing selected" folded={!open} onFold={fold} edge={edge}>
          <p className="cw-empty">Click an item on the left to see it here</p>
        </Panel>
      ) : (
        <Panel
          label={`Item ${item.number}`}
          {...windowHead(item, () => scene.current?.reveal(item.number, room()))}
          lines={<WindowLines item={item} page={shownPage} act={source.act} waitsFor={waitsFor} />}
          folded={!open}
          onFold={fold}
          edge={edge}
        >
          <WindowBody key={item.number} item={item} page={shownPage} act={source.act} />
        </Panel>
      )}
    </div>
  );
}
