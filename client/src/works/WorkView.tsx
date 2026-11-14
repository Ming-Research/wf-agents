// One work: the view's line with its item's title and project, what it does now, what it has taken and the owner's
// buttons; its history as a chart; and the panel docked at the chart's right with what the owner chose. What the work
// waits for, and what the owner writes to it, are in the console's item window. The work is read again each time the
// interface says it changed and whenever the stream of changes opens again, keeping what the owner has chosen and
// opened; a read that fails keeps what is shown and says so.

import { type CSSProperties, useCallback, useEffect, useMemo, useState } from 'react';

import { Attr, Edge } from '../parts';
import type { Fold } from '../Side';
import { PANEL, WIDTH_KEYS, useKeptWidth } from '../sizes';
import type { Act, RoundPage, ToldPage, WorkPage } from '../works-api';
import { ActButtons, type Send } from './Boxes';
import { type Chosen, HistoryChart } from './History';
import { NowLine } from './marks';
import { type Source, useChanges, useReading, useSource } from './source';
import { ChosenPanel, type Reader } from './StepPanel';
import { spentWords, tokensWords } from './words';

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
    version: (block, at) => cached(`v:${block}@${at}`, async () => (await source.loadVersion(work, block, at)).content),
    round: (run, number, ended) =>
      ended
        ? cached<RoundPage>(`r:${run}/${number}`, () => source.loadRound(work, run, number))
        : source.loadRound(work, run, number),
    told: (run, number, ended) =>
      ended
        ? cached<ToldPage>(`t:${run}/${number}`, () => source.loadTold(work, run, number))
        : source.loadTold(work, run, number),
    call: (call) => cached(`c:${call}`, async () => (await source.loadCall(work, call)).result),
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
  const [chosen, setChosen] = useState<Chosen>(() => {
    const step = run === undefined ? -1 : page.steps.findIndex((s) => s.kind === 'run' && s.run === run);
    return step < 0 ? null : { step };
  });
  // The panel is open at first, unless the window is too narrow to show the history beside it and nothing is chosen.
  const [folded, setFolded] = useState(() => chosen === null && window.innerWidth < 2 * PANEL.first);
  const [panelWidth, setPanelWidth] = useKeptWidth(WIDTH_KEYS.panel, PANEL.min, PANEL.max);

  const send: Send = useCallback(
    async (what: Act) => {
      try {
        await source.act(work, what);
        onActed();
        return null;
      } catch (e) {
        return e instanceof Error ? e.message : String(e);
      }
    },
    [source, work, onActed],
  );

  const choose = useCallback((c: Chosen) => {
    setChosen(c);
    if (c !== null) setFolded(false);
  }, []);
  useEffect(() => {
    const close = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setChosen(null);
    };
    window.addEventListener('keydown', close);
    return () => window.removeEventListener('keydown', close);
  }, []);

  const edge = (
    <Edge
      side="left"
      label="Drag the panel's width"
      start={() => panelWidth ?? PANEL.first}
      onDrag={setPanelWidth}
      onReset={() => setPanelWidth(null)}
    />
  );
  const fold: Fold = { folded, onFold: () => setFolded((f) => !f), edge };
  return (
    <>
      <div className="view-line">
        <a className="link" href="#/works">
          Works
        </a>
        <span className="mono muted">#{summary.item}</span>
        <span className="view-name">{page.title}</span>
        {page.project !== null && <Attr name="project" value={page.project} />}
        <span className="view-facts">
          <NowLine now={summary.now} paused={summary.paused} />
        </span>
        <span className="view-end">
          <span className="muted small" title={tokensWords(summary.spent)}>
            {spentWords(summary.spent)}
          </span>
          <ActButtons summary={summary} lines={page.flow.lines.map((l) => l.name)} send={send} />
        </span>
      </div>
      {failed !== null && (
        <p className="read-failed" role="alert">
          The work could not be read again: {failed}. What is shown is from before.
        </p>
      )}
      <main
        className={`view-body with-panel ${folded ? 'panel-folded' : ''}`}
        style={panelWidth !== null ? ({ '--panel-width': `${panelWidth}px` } as CSSProperties) : undefined}
      >
        <HistoryChart page={page} chosen={chosen} onChoose={choose} />
        <ChosenPanel page={page} reader={reader} fold={fold} onChoose={choose} chosen={chosen} />
      </main>
    </>
  );
}
