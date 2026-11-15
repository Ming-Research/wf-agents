// The console: the items as a tree in snapping columns on a canvas drawn by WebGL (`snap.ts`), as they stand, drawn again
// as they change; with `?fixtures` in the page's address, made-up items that show every state the drawing has
// (`fixtures.ts`). Beside the tree, a window shows the item selected (`ItemWindow.tsx`). Selecting an item slides the
// window out; letting it go slides the window back, unless the tab stuck to its left edge brought it out, in which case
// it stays until the tab takes it back. The tab takes it back without letting the item go, and the next item selected
// brings it out again. The window coming out over the item selected, or an item selected half under it or half off the
// screen, moves the tree only until the item is wholly within what the window leaves. Escape lets the item go while the
// tree has the keys, and slides the window in while it has them. Each time the interface says something happened, the items are read again and
// drawn where they stand, the item selected and the panel as they were; when the stream opens again after a cut,
// everything is. An answer older than the one shown is dropped. An item the address names is selected once the tree
// shows it, as from a notification of a question it waits on. The page's shell mounts it lazily (`lazy.tsx`).
//
// Brought over from the console of the engine on `old-main` (`client/src/pages/Console.tsx`): the window slides over
// the tree, which keeps its whole width under it (`Drawer`, in the page's parts, which the flow view's panel is too).

import './console.css';

import { useEffect, useMemo, useRef, useState } from 'react';

import { Drawer } from '../parts';
import type { Items, WorkPage } from '../works-api';
import { goalMock } from './mock';
import { fakeShown } from './fake';
import { type Shown, shownTree, standingOf } from './items';
import { ItemPanel, type PageState } from './ItemWindow';
import { SnapScene } from './snap';
import { type Heard, type Source, inOrder, pageSource } from './source';

export function ConsolePage({ source: given, named }: { source?: Source; named?: number } = {}) {
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

  // The items, read at first and again each time something happened, an answer older than the one shown dropped.
  const heard = useRef<(h: Heard) => void>(() => undefined);
  useEffect(() => {
    let alive = true;
    const take = inOrder();
    const read = () =>
      take(
        source.loadItems(),
        (items) => {
          if (!alive) return;
          setData(items);
          setFailed(null);
        },
        (why) => alive && setFailed(why),
      );
    read();
    const stop = source.changes((h) => {
      if (h === 'everything' || h.items || h.works.length > 0) read();
      heard.current(h);
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
          setWin((w) => {
            if (id !== null) return w.open ? w : { open: true, byHand: false };
            return w.byHand ? w : { open: false, byHand: false };
          });
        };
        if (latestData.current !== undefined) s.show(shownTree(latestData.current));
        selectNamed();
        // To try the drawing from the browser's console: `consoleScene.show(consoleFake(2000))` draws a made-up tree.
        Object.assign(window, { consoleScene: s, consoleFake: fakeShown });
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
  const selectNamed = () => {
    const id = toSelect.current;
    if (id === undefined || scene.current === null) return;
    if (!latestData.current?.items.some((i) => i.number === id)) return;
    toSelect.current = undefined;
    scene.current.select(id);
  };
  useEffect(() => {
    latestData.current = data;
    if (data !== undefined) scene.current?.show(shownTree(data));
    selectNamed();
  }, [data]);
  useEffect(() => {
    toSelect.current = named;
    selectNamed();
  }, [named]);

  /// The width of the tree that the window leaves to be seen: all of it while the window is in.
  const room = (out: boolean) => {
    const tree = host.current?.clientWidth ?? 0;
    return out ? tree - (panel.current?.offsetWidth ?? 0) : tree;
  };
  const selectedId = selected?.id ?? null;
  useEffect(() => {
    if (open && selectedId !== null) scene.current?.reveal(selectedId, room(true));
  }, [open, selectedId]);

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
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
  }, [open]);

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
  const byNumber = new Map((data?.items ?? []).map((i) => [i.number, i]));
  const waitsFor =
    item === undefined ? [] : item.dependsOn.filter((d) => byNumber.has(d) && standingOf(byNumber.get(d)) !== 'done');

  return (
    <div className="console">
      {/* A press on the tree gives it the keys back from the window. */}
      <div
        className="console-canvas"
        ref={host}
        onPointerDown={() => (document.activeElement as HTMLElement | null)?.blur()}
      >
        {failed !== null && <p className="console-message">The page could not read the items: {failed}</p>}
        {undrawn !== null && <p className="console-message">The browser could not draw the tree: {undrawn}</p>}
      </div>
      <Drawer
        open={open}
        onToggle={fold}
        panel={panel}
        label="The item selected"
        hide="Put the item away"
        show="Show the item selected"
        widthKey="console-window-width"
      >
        {item === undefined ? (
          <p className="cw-empty">Click an item on the left to see it here.</p>
        ) : (
          <ItemPanel
            item={item}
            page={shownPage}
            act={source.act}
            loadVersion={source.loadVersion}
            waitsFor={waitsFor}
            goal={selected !== null && goalMock(selected.work !== null, selected.children.length)}
            open={open}
            onRead={() => latestData.current !== undefined && scene.current?.show(shownTree(latestData.current))}
            onTitle={() => scene.current?.reveal(item.number, room(open))}
          />
        )}
      </Drawer>
    </div>
  );
}
