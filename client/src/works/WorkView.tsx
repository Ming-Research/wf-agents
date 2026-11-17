// One work: the view's line, with the link back to the works, the item's number and its title, and the button that has
// a line begin a new session, which the item's window does not hold; under it, the legend of the history's marks; the
// work's history as a chart; and over its right the console's window (`Drawer`) with the console's item window for the
// work's item (`ItemPanel`), whose switch holds, after the console's Item and Work, the row of the history chosen (Step)
// and what the work used (Usage). What the window shows (the item's state, the work's cost and time, the owner's
// buttons) is not said again in the view's line. Choosing a row slides the window out on Step; letting the row go
// slides it back, unless its tab brought it out, in which case it stays until the tab takes it back: as the console's
// and the flow's window. The window is out as the page opens, as if its tab had brought it out, on Item, or on Step
// where the address names a run, whose row is then chosen. Escape slides the window in while it has the keys, and else
// lets the row go. The work, and the items its item is read from, are read again each time the interface says they
// changed and whenever the stream of changes opens again, keeping what the owner chose and opened; a read that fails
// keeps what is shown and says so.

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { ItemPanel } from '../console/ItemWindow';
import { Legend } from '../marks';
import { Drawer } from '../parts';
import { hashOf } from '../route';
import { WIDTH_KEYS } from '../sizes';
import type { Act, RoundPage, ToldPage, WorkPage } from '../works-api';
import { NewSession, type Send } from './Boxes';
import { HISTORY_MARKS, HistoryChart } from './History';
import { placeSteps, sessionsBegun } from './placing';
import { type Source, useChanges, useItems, useReading, useSource } from './source';
import { type Reader, StepTab } from './StepPanel';
import { UsageTab } from './Usage';
import { hasEnded } from './words';

export function WorkView({ work, run }: {
  work: number;
  /// The run the address names, chosen as the work is first shown, as by a link to it.
  run?: number;
}) {
  const source = useSource();
  const load = useCallback(() => source.loadWork(work), [source, work]);
  const { value: page, failed, read } = useReading(load);
  useChanges((c) => {
    if (c.works.includes(work)) read();
  }, read);
  if (!page) {
    return failed ? (
      <p className="message">The page could not read work {work}: {failed}</p>
    ) : (
      <p className="message muted">Reading work {work}</p>
    );
  }
  return <WorkScreen page={page} failed={failed} onActed={read} run={run} />;
}

/// The reads of a work's contents, each made once: a version, an ended round, what it was told and returned, and a
/// call's result do not change once there. A round under way is read again each time, since it has more to come.
export function readerOf(source: Source, work: number): Reader {
  const once = new Map<string, Promise<unknown>>();
  const cached = <T,>(key: string, load: () => Promise<T>): Promise<T> => {
    let p = once.get(key) as Promise<T> | undefined;
    if (!p) {
      p = load();
      once.set(key, p);
      p.catch(() => once.delete(key));
    }
    return p;
  };
  return {
    version: (block, number) =>
      cached(`v:${block}/${number}`, async () => (await source.loadVersion(work, block, number)).content),
    round: (run, number, ended) =>
      ended
        ? cached<RoundPage>(`r:${run}/${number}`, () => source.loadRound(work, run, number))
        : source.loadRound(work, run, number),
    told: (run, number, ended) =>
      ended
        ? cached<ToldPage>(`t:${run}/${number}`, () => source.loadTold(work, run, number))
        : source.loadTold(work, run, number),
    call: (call) => cached(`c:${call}`, async () => (await source.loadCall(work, call)).result),
    system: (line, session) =>
      cached(`s:${line}/${session}`, async () => (await source.loadSystem(work, line, session)).text),
    doing: async (run, number) => (await source.loadDoing(work, run, number)).doing,
  };
}

export function WorkScreen({ page, failed = null, onActed, run }: {
  page: WorkPage;
  /// Why the last read of the work failed, where it did: what is shown is from before.
  failed?: string | null;
  onActed: () => void;
  /// The run chosen as the work is first shown; none chooses nothing.
  run?: number;
}) {
  const source = useSource();
  const { summary } = page;
  const work = summary.number;
  const reader = useMemo(() => readerOf(source, work), [source, work]);
  const rows = useMemo(() => placeSteps(page), [page]);
  const sessions = useMemo(() => sessionsBegun(page.steps), [page.steps]);
  const items = useItems();
  const item = items.value?.items.find((i) => i.work.number === work);
  // The row chosen, by its step's place among the steps; and the choice of the window's switch.
  const [chosen, setChosen] = useState<number | null>(() => {
    const step = run === undefined ? -1 : page.steps.findIndex((s) => s.kind === 'run' && s.run === run);
    return step < 0 ? null : step;
  });
  const [tab, setTab] = useState<string>(() => (chosen === null ? 'item' : 'step'));
  // The window: whether it is out, and whether its tab brought it out, which keeps it out once the row is let go.
  const [win, setWin] = useState({ open: true, byHand: true });
  const panel = useRef<HTMLElement>(null);
  // How wide the window is, for the room the chart leaves under it.
  const [windowWidth, setWindowWidth] = useState(0);

  const choose = useCallback((step: number | null) => {
    setChosen(step);
    if (step !== null) setTab('step');
    setWin((w) => {
      if (step !== null) return w.open ? w : { open: true, byHand: false };
      return w.byHand ? w : { open: false, byHand: false };
    });
  }, []);
  const fold = useCallback(
    () => setWin((w) => (w.open ? { open: false, byHand: false } : { open: true, byHand: true })),
    [],
  );
  // Escape slides the window in while it has the keys, and else lets the row go; a question the page asks answers it
  // alone.
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'Escape' || document.querySelector('dialog[open]') !== null) return;
      const inside = panel.current?.contains(document.activeElement) === true;
      if (win.open && inside) {
        (document.activeElement as HTMLElement | null)?.blur();
        setWin({ open: false, byHand: false });
      } else choose(null);
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, [win.open, choose]);
  useEffect(() => {
    const el = panel.current;
    if (el === null) return;
    setWindowWidth(el.offsetWidth);
    if (typeof ResizeObserver === 'undefined') return;
    const watch = new ResizeObserver(() => setWindowWidth(el.offsetWidth));
    watch.observe(el);
    return () => watch.disconnect();
  }, []);

  // What the owner does in the window, the work read again once it is taken.
  const act = useCallback(
    async (to: number, what: Act) => {
      const done = await source.act(to, what);
      onActed();
      return done;
    },
    [source, onActed],
  );
  const send: Send = useCallback(
    async (what: Act) => {
      try {
        await act(work, what);
        return null;
      } catch (e) {
        return e instanceof Error ? e.message : String(e);
      }
    },
    [act, work],
  );

  const row = chosen === null ? undefined : rows.find((r) => r.index === chosen);
  const more = [
    {
      value: 'step',
      name: 'Step',
      content: (
        <StepTab
          page={page}
          row={row}
          sessions={sessions}
          reader={reader}
          shown={win.open && tab === 'step'}
        />
      ),
    },
    { value: 'usage', name: 'Usage', content: <UsageTab page={page} /> },
  ];
  return (
    <>
      <div className="view-line view-lines">
        <div className="view-row">
          <a className="link" href="#/works">
            Works
          </a>
          <span className="mono muted">#{summary.item}</span>
          <span className="view-name">{page.title}</span>
          {!hasEnded(summary) && (
            <span className="view-end">
              <NewSession lines={page.flow.lines.map((l) => l.name)} send={send} />
            </span>
          )}
        </div>
        <div className="view-row">
          <Legend marks={HISTORY_MARKS} />
        </div>
      </div>
      {failed !== null && (
        <p className="read-failed" role="alert">
          The work could not be read again: {failed}. What is shown is from before.
        </p>
      )}
      <main className="view-body with-window">
        <HistoryChart page={page} rows={rows} chosen={chosen} onChoose={choose} covered={win.open ? windowWidth : 0} />
        <Drawer
          open={win.open}
          onToggle={fold}
          panel={panel}
          label="The work's item"
          hide="Put the item away"
          show="Show the work's item"
          widthKey={WIDTH_KEYS.itemWindow}
        >
          {item === undefined ? (
            <p className="cw-empty">
              {items.failed !== null
                ? `The page could not read the work's item: ${items.failed}`
                : items.value === null
                  ? 'Reading the item'
                  : `No item the page read has W${work} for its work`}
            </p>
          ) : (
            <ItemPanel
              item={item}
              page={{ page }}
              act={act}
              loadVersion={source.loadVersion}
              actEngine={source.actOnEngine}
              open={win.open}
              titleHref={hashOf({ view: 'console', item: item.number })}
              more={more}
              tab={tab}
              onTab={setTab}
            />
          )}
        </Drawer>
      </main>
    </>
  );
}
