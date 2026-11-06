// The console: the items as a tree in snapping columns on a canvas drawn by WebGL (`console/snap.ts`), as they stand,
// drawn again as they change, in the look chosen at the top right.

import { SegmentedControl } from '@primer/react';
import { useEffect, useRef, useState } from 'react';

import type { Items, WorkSummary } from '../api';
import { fakeShown } from '../console/fake';
import { shownTree } from '../console/items';
import { type Look, SnapScene } from '../console/snap';
import { useResource } from '../live';
import { t } from '../messages';

const LOOKS: Look[] = ['schematic', 'cards'];

export function Console() {
  const host = useRef<HTMLDivElement>(null);
  const scene = useRef<SnapScene | null>(null);
  const items = useResource<Items>('items', '/api/v1/items');
  const works = useResource<WorkSummary[]>('works', '/api/v1/works');
  const data = items.data;
  const workList = works.data;
  const [look, setLook] = useState<Look>('schematic');
  const lookNow = useRef(look);

  useEffect(() => {
    const el = host.current;
    if (el === null) return;
    const s = new SnapScene();
    let alive = true;
    void s.mount(el).then(() => {
      if (!alive) return;
      scene.current = s;
      s.look = lookNow.current;
      if (latest.current !== undefined) s.show(shownTree(latest.current, latestWorks.current));
      // For trying the drawing from the browser's console: `consoleScene.show(consoleFake(2000))` draws a made-up tree.
      Object.assign(window, { consoleScene: s, consoleFake: fakeShown });
    });
    return () => {
      alive = false;
      scene.current = null;
      s.destroy();
    };
  }, []);

  useEffect(() => {
    lookNow.current = look;
    scene.current?.setLook(look);
  }, [look]);

  // The items and works as last read, for a scene that is made after they came.
  const latest = useRef<Items | undefined>(undefined);
  const latestWorks = useRef<WorkSummary[]>([]);
  useEffect(() => {
    latest.current = data;
    latestWorks.current = workList ?? [];
    if (data !== undefined) scene.current?.show(shownTree(data, workList ?? []));
  }, [data, workList]);

  return (
    <div className="console">
      <div className="console-canvas" ref={host} />
      <div className="console-look">
        <SegmentedControl aria-label={t('console_look')} size="small" onChange={(i) => setLook(LOOKS[i])}>
          {LOOKS.map((l) => (
            <SegmentedControl.Button key={l} selected={look === l}>
              {t(l === 'schematic' ? 'console_look_schematic' : 'console_look_cards')}
            </SegmentedControl.Button>
          ))}
        </SegmentedControl>
      </div>
    </div>
  );
}
