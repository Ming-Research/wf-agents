// One work: the view's line, with the way back where the owner came from, the item's number and its title, and at its
// right the legend of the history's marks; the work's history as a chart; and over its right the console's window
// (`Drawer`) with the console's item window for the work's item (`ItemPanel`), its tabs the console's, whose Step
// shows the row of the history chosen.
// What the window shows (the item's state, the work's cost and time, the owner's buttons) is not said again in the
// view's line. Choosing a row slides the window out on Step; letting the row go slides it back, unless its tab brought
// it out, in which case it stays until the tab takes it back: as the console's and the flow's window. The window is out
// as the page opens, as if its tab had brought it out, on Item, or on Step where the address names a run, whose row is
// then chosen, and the round it names, where it does, opened and lit in the Step. The address says what is chosen: choosing
// a row replaces it with the row's, and the page follows it as it changes, so that a link to a round chooses it however
// the owner chose before. Escape slides the window in while it has the keys, and else lets the row go. The work, and the items its
// item is read from, are read again each time the interface says they changed and whenever the stream of changes opens
// again, keeping what the owner chose and opened; a read that fails keeps what is shown and says so.

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { ItemPanel, WINDOW_TABS, type WindowTab } from '../console/ItemWindow';
import { Legend } from '../marks';
import { Drawer } from '../parts';
import { CHOICE_KEYS, keptChoice, useKept, write } from '../kept';
import { goBack, hashOf, replaceHash } from '../route';
import { WIDTH_KEYS } from '../sizes';
import type { Act, WorkPage } from '../works-api';
import { ChosenRound } from './chosen-round';
import { HISTORY_MARKS, HistoryChart } from './History';
import { placeSteps } from './placing';
import { useChanges, useItems, useReading, useSource } from './source';

export function WorkView({ work, run, round, active = true }: {
  work: number;
  /// The run the address names, chosen as the address names it, as by a link to it.
  run?: number;
  /// The round of that run the address names, opened and lit in the Step.
  round?: number;
  /// Whether the work's page is the view shown: kept while another is, it takes no keys.
  active?: boolean;
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
  return <WorkScreen page={page} failed={failed} onActed={read} run={run} round={round} active={active} />;
}

/// The run a step of the work's history is, none for a step that is no run.
const runOf = (page: WorkPage, step: number | null): number | undefined => {
  const s = step === null ? undefined : page.steps[step];
  return s?.kind === 'run' ? s.run : undefined;
};

/// The place among the steps of the run `run`, or none.
const stepOfRun = (page: WorkPage, run: number | undefined): number | null => {
  const at = run === undefined ? -1 : page.steps.findIndex((s) => s.kind === 'run' && s.run === run);
  return at < 0 ? null : at;
};

export function WorkScreen({ page, failed = null, onActed, run, round, active = true }: {
  page: WorkPage;
  /// Whether the work's page is the view shown: kept while another is, it takes no keys.
  active?: boolean;
  /// Why the last read of the work failed, where it did: what is shown is from before.
  failed?: string | null;
  onActed: () => void;
  /// The run the address names, chosen as the work is first shown and each time the address names another; none
  /// chooses nothing.
  run?: number;
  /// The round of that run the address names; none names none.
  round?: number;
}) {
  const source = useSource();
  const { summary } = page;
  const work = summary.number;
  const rows = useMemo(() => placeSteps(page), [page]);
  const items = useItems();
  const item = items.value?.items.find((i) => i.work.number === work);
  // The row chosen, by its step's place among the steps; and the choice of the window's switch.
  const [chosen, setChosen] = useState<number | null>(() => stepOfRun(page, run));
  // The window's tab and whether the window is out, as the owner last left them, kept in this browser; a run the address
  // names opens on its Step.
  const [keptTab, keepTab] = useKept(CHOICE_KEYS.workTab, 'item', WINDOW_TABS);
  const [tab, setTabNow] = useState<WindowTab>(() => (chosen === null ? keptTab : 'step'));
  const setTab = useCallback(
    (to: WindowTab) => {
      setTabNow(to);
      keepTab(to);
    },
    [keepTab],
  );
  // The window: whether it is out, and whether its tab brought it out, which keeps it out once the row is let go; a row
  // the address names brings it out.
  const [win, setWin] = useState(() =>
    keptChoice(CHOICE_KEYS.workWindow, 'out', ['out', 'in']) === 'out'
      ? { open: true, byHand: true }
      : { open: chosen !== null, byHand: false },
  );
  const putWindow = useCallback((open: boolean) => {
    write(CHOICE_KEYS.workWindow, open ? 'out' : 'in');
    setWin(open ? { open: true, byHand: true } : { open: false, byHand: false });
  }, []);
  const panel = useRef<HTMLElement>(null);
  // How wide the window is, for the room the chart leaves under it.
  const [windowWidth, setWindowWidth] = useState(0);

  // Chooses the row at `step`, or lets the row go, as the address says or the owner does.
  const select = useCallback(
    (step: number | null) => {
      setChosen(step);
      if (step !== null) setTab('step');
      setWin((w) => {
        if (step !== null) return w.open ? w : { open: true, byHand: false };
        return w.byHand ? w : { open: false, byHand: false };
      });
    },
    [setTab],
  );
  // What the owner chooses is written to the address, replacing it, so that it says what is chosen: the Works tab
  // leads back to it, and a link to another row, or to a round, is a change of address the page follows.
  const choose = useCallback(
    (step: number | null) => {
      select(step);
      if (active) replaceHash(hashOf({ view: 'work', work, run: runOf(page, step) }));
    },
    [select, active, work, page],
  );
  // The address, followed as it changes: the run it names is chosen, none being let go, and the window comes out on its
  // Step. A row the owner chose is written to the address first, so that following it chooses that row again.
  const latest = useRef({ page, select });
  useEffect(() => {
    latest.current = { page, select };
  });
  const followed = useRef({ run, round });
  useEffect(() => {
    if (followed.current.run === run && followed.current.round === round) return;
    followed.current = { run, round };
    const now = latest.current;
    now.select(stepOfRun(now.page, run));
  }, [run, round]);
  // The round the address names, while the run it names is the one chosen.
  const chosenRun = runOf(page, chosen);
  const named = useMemo(
    () =>
      round !== undefined && run !== undefined && chosenRun === run
        ? { run, round, scrolled: { current: false } }
        : null,
    [run, round, chosenRun],
  );
  const fold = useCallback(() => putWindow(!win.open), [putWindow, win.open]);
  // Escape slides the window in while it has the keys, and else lets the row go; a question the page asks answers it
  // alone.
  useEffect(() => {
    if (!active) return;
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'Escape' || document.querySelector('dialog[open]') !== null) return;
      const inside = panel.current?.contains(document.activeElement) === true;
      if (win.open && inside) {
        (document.activeElement as HTMLElement | null)?.blur();
        putWindow(false);
      } else choose(null);
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, [win.open, choose, putWindow, active]);
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
  const row = chosen === null ? undefined : rows.find((r) => r.index === chosen);
  return (
    <>
      <div className="view-line">
        <span className="view-title">
          <a
            className="link"
            href="#/works"
            onClick={(e) => {
              e.preventDefault();
              goBack('#/works');
            }}
          >
            ← Back
          </a>
          <span className="mono muted">#{summary.item}</span>
          <span className="view-name">{page.title}</span>
        </span>
        <span className="view-end">
          <Legend marks={HISTORY_MARKS} />
        </span>
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
            <ChosenRound.Provider value={named}>
              <ItemPanel
                item={item}
                page={{ page }}
                act={act}
                loadVersion={source.loadVersion}
                open={win.open && active}
                titleHref={hashOf({ view: 'console', item: item.number })}
                row={row}
                tab={tab}
                onTab={setTab}
              />
            </ChosenRound.Provider>
          )}
        </Drawer>
      </main>
    </>
  );
}
