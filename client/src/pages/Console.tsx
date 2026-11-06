// The console: the items as a tree in snapping columns on a canvas drawn by WebGL (`console/snap.ts`), as they stand,
// drawn again as they change; at `/console/demo`, made-up items that show every state the drawing has.

import { useEffect, useRef } from 'react';

import type { Conversation, Items, Status, WorkSummary } from '../api';
import { demoShown, fakeShown } from '../console/fake';
import { shownTree } from '../console/items';
import { SnapScene } from '../console/snap';
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

  useEffect(() => {
    const el = host.current;
    if (el === null) return;
    const s = new SnapScene();
    let alive = true;
    void s.mount(el).then(() => {
      if (!alive) return;
      scene.current = s;
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

  return (
    <div className="console">
      <div className="console-canvas" ref={host} />
    </div>
  );
}
