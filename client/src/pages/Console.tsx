// The console: the items as a tree in snapping columns on a canvas drawn by WebGL (`console/snap.ts`), as they stand,
// drawn again as they change; at `/console/demo`, made-up items that show every state the drawing has. Beside the tree,
// a window slid out by the tab stuck to its left edge shows the item selected (`console/ItemWindow.tsx`), so far with
// made-up contents. Escape lets the item go while the tree has the keys, and slides the window in while it has them.

import { useEffect, useRef, useState } from 'react';

import type { Conversation, Items, Status, WorkSummary } from '../api';
import { Drawer } from '../console/Drawer';
import { demoShown, fakeShown } from '../console/fake';
import { type Shown, shownTree } from '../console/items';
import { ItemWindow } from '../console/ItemWindow';
import { SnapScene } from '../console/snap';
import { windowMock } from '../console/window-mock';
import { useResource, useStatus } from '../live';

export function Console({ demo }: { demo: boolean }) {
  const host = useRef<HTMLDivElement>(null);
  const scene = useRef<SnapScene | null>(null);
  const items = useResource<Items>('items', '/api/v1/items');
  const works = useResource<WorkSummary[]>('works', '/api/v1/works');
  // The owner's conversations, as the bar reads them, for each work's messages unread.
  const conversations = useResource<Conversation[]>('messages', '/api/v1/conversations').data;
  const data = items.data;
  const workList = works.data;
  const status = useStatus();
  const [selected, setSelected] = useState<Shown | null>(null);
  const [open, setOpen] = useState(false);
  const panel = useRef<HTMLElement>(null);

  useEffect(() => {
    const el = host.current;
    if (el === null) return;
    const s = new SnapScene();
    let alive = true;
    void s.mount(el).then(() => {
      if (!alive) return;
      scene.current = s;
      s.onSelect = setSelected;
      if (demo) s.show(demoShown());
      else if (latest.current !== undefined)
        s.show(shownTree(latest.current, latestWorks.current, latestStatus.current, latestConversations.current));
      // For trying the drawing from the browser's console: `consoleScene.show(consoleFake(2000))` draws a made-up tree.
      Object.assign(window, { consoleScene: s, consoleFake: fakeShown });
    });
    return () => {
      alive = false;
      scene.current = null;
      s.destroy();
    };
  }, [demo]);

  // The items, the works, the engine's status and the conversations as last read, for a scene made after they came.
  const latest = useRef<Items | undefined>(undefined);
  const latestWorks = useRef<WorkSummary[]>([]);
  const latestStatus = useRef<Status | null>(null);
  const latestConversations = useRef<Conversation[]>([]);
  useEffect(() => {
    latest.current = data;
    latestWorks.current = workList ?? [];
    latestStatus.current = status;
    latestConversations.current = conversations ?? [];
    if (data !== undefined && !demo) scene.current?.show(shownTree(data, workList ?? [], status, conversations ?? []));
  }, [data, workList, status, conversations, demo]);

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      const inside = panel.current?.contains(document.activeElement) === true;
      if (open && inside) {
        (document.activeElement as HTMLElement | null)?.blur();
        setOpen(false);
      } else scene.current?.select(null);
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, [open]);

  return (
    <div className="console">
      {/* A press on the tree gives it the keys back from the window. */}
      <div
        className="console-canvas"
        ref={host}
        onPointerDown={() => (document.activeElement as HTMLElement | null)?.blur()}
      />
      <Drawer open={open} onToggle={() => setOpen(!open)} panel={panel}>
        <ItemWindow item={selected === null ? null : windowMock(selected)} demo={demo} />
      </Drawer>
    </div>
  );
}
