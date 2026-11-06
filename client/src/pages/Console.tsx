// The console: the items as a tree in snapping columns on a canvas drawn by WebGL (`console/snap.ts`), as they stand,
// drawn again as they change.

import { useEffect, useRef } from 'react';

import type { Items, Status, WorkSummary } from '../api';
import { fakeShown } from '../console/fake';
import { shownTree } from '../console/items';
import { SnapScene } from '../console/snap';
import { useResource, useStatus } from '../live';

export function Console() {
  const host = useRef<HTMLDivElement>(null);
  const scene = useRef<SnapScene | null>(null);
  const items = useResource<Items>('items', '/api/v1/items');
  const works = useResource<WorkSummary[]>('works', '/api/v1/works');
  const data = items.data;
  const workList = works.data;
  const status = useStatus();

  useEffect(() => {
    const el = host.current;
    if (el === null) return;
    const s = new SnapScene();
    let alive = true;
    void s.mount(el).then(() => {
      if (!alive) return;
      scene.current = s;
      if (latest.current !== undefined) s.show(shownTree(latest.current, latestWorks.current, latestStatus.current));
      // For trying the drawing from the browser's console: `consoleScene.show(consoleFake(2000))` draws a made-up tree.
      Object.assign(window, { consoleScene: s, consoleFake: fakeShown });
    });
    return () => {
      alive = false;
      scene.current = null;
      s.destroy();
    };
  }, []);

  // The items, the works and the engine's status as last read, for a scene that is made after they came.
  const latest = useRef<Items | undefined>(undefined);
  const latestWorks = useRef<WorkSummary[]>([]);
  const latestStatus = useRef<Status | null>(null);
  useEffect(() => {
    latest.current = data;
    latestWorks.current = workList ?? [];
    latestStatus.current = status;
    if (data !== undefined) scene.current?.show(shownTree(data, workList ?? [], status));
  }, [data, workList, status]);

  return (
    <div className="console">
      <div className="console-canvas" ref={host} />
    </div>
  );
}
