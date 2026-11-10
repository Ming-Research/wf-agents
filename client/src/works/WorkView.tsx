// One work: the view's line with its item's title, what it does now, what it has taken and the owner's buttons; what
// it waits for, in full, with what the owner can do about it; its history as a chart; and the panel docked at the
// chart's right with what the owner chose, and at the panel's foot the box to write a block. The work is read again
// each time the interface says it changed and whenever the stream of changes opens again, keeping what the owner has
// chosen, opened and typed; a read that fails keeps what is shown and says so.

import { type CSSProperties, useCallback, useEffect, useMemo, useState } from 'react';

import { Edge } from '../parts';
import type { Fold } from '../Side';
import { PANEL, WIDTH_KEYS, useKeptWidth } from '../sizes';
import type { Act, RoundPage, ToldPage, WorkPage } from '../works-api';
import { ActButtons, type Draft, EMPTY_DRAFT, type Send, WaitsBand, WriteBox } from './Boxes';
import { type Chosen, HistoryChart } from './History';
import { NowLine } from './marks';
import { cellAt, defaultBlock } from './placing';
import { type Source, useChanges, useReading, useSource } from './source';
import { ChosenPanel, type Reader } from './StepPanel';
import { spentWords, tokensWords } from './words';

export function WorkView({ work }: { work: number }) {
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
  return <WorkScreen page={page} failed={failed} onActed={read} />;
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

export function WorkScreen({ page, failed = null, onActed }: {
  page: WorkPage;
  /// Why the last read of the work failed, where it did: what is shown is from before.
  failed?: string | null;
  onActed: () => void;
}) {
  const source = useSource();
  const { summary } = page;
  const work = summary.number;
  const reader = useMemo(() => readerOf(source, work), [source, work]);
  const [chosen, setChosen] = useState<Chosen>(null);
  // The panel is open at first, unless the window is too narrow to show the history beside it.
  const [folded, setFolded] = useState(() => window.innerWidth < 2 * PANEL.first);
  const [draft, setDraft] = useState<Draft>(EMPTY_DRAFT);
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

  // The box follows the cell chosen, the cell of the step chosen, or the cell the work is at.
  const step = chosen && 'step' in chosen ? page.steps[chosen.step] : undefined;
  const followed =
    chosen && 'cell' in chosen
      ? chosen.cell
      : step?.kind === 'run'
        ? step.cell
        : step?.kind === 'letGo'
          ? step.cell
          : cellAt(summary);
  const fallback = defaultBlock(page.flow, followed);

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
  const foot = (
    <WriteBox flow={page.flow} summary={summary} fallback={fallback} draft={draft} onDraft={setDraft} send={send} />
  );

  return (
    <>
      <div className="view-line">
        <a className="link" href="#/works">
          Works
        </a>
        <span className="mono muted">#{summary.item}</span>
        <span className="view-name">{page.title}</span>
        <span className="view-facts">
          <NowLine now={summary.now} paused={summary.paused} />
        </span>
        <span className="view-end">
          <span className="muted small" title={tokensWords(summary.spent)}>
            {spentWords(summary.spent)}
          </span>
          <ActButtons summary={summary} send={send} />
        </span>
      </div>
      {failed !== null && (
        <p className="read-failed" role="alert">
          The work could not be read again: {failed}. What is shown is from before.
        </p>
      )}
      <WaitsBand summary={summary} send={send} />
      <main
        className={`view-body with-panel ${folded ? 'panel-folded' : ''}`}
        style={panelWidth !== null ? ({ '--panel-width': `${panelWidth}px` } as CSSProperties) : undefined}
      >
        <HistoryChart page={page} chosen={chosen} onChoose={choose} />
        <ChosenPanel page={page} reader={reader} fold={fold} foot={foot} onChoose={choose} chosen={chosen} />
      </main>
    </>
  );
}
