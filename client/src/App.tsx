// The page: the bar with its tabs, and under it the view the address's hash names (route.ts): the console, the works
// (works/), the flow, the observations, the system or the settings. The flow view is the view's line (the flow's name,
// its limits, its tools, the switch between the matrix and the flowchart, the legend of the one shown), the matrix or
// the flowchart under it (graph/), and over its right the console's window (`Drawer`) with what the owner chose, the
// same from either, coming and going as the console's does.

import { Fragment, type ReactNode, Suspense, useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { type Definition, type Flow, type Given, load } from './api';
import { Chart, type Chosen, Legend } from './Chart';
import { LazyConsolePage } from './console/lazy';
import { type BarItems, Engine, useBarItems, waitsForOwner } from './Engine';
import { LazyFlowChart } from './graph/lazy';
import { GraphLegend } from './graph/legend';
import { chartOf } from './graph/model';
import { matrix } from './matrix';
import { usesFixtures } from './mock-data';
import { useNotifications } from './notify';
import { ObservationsView } from './observations/ObservationsView';
import { Bar, Drawer, Panel, Switch } from './parts';
import { type Route, hashOf, periodOf, tabOf, useRoute } from './route';
import { type Serves, opensOnFlow, useServes } from './serves';
import { WIDTH_KEYS } from './sizes';
import { BlockPanel, CellPanel, Faults, GroupPanel, LinePanel, TestPanel, ToolPanel } from './Side';
import { SettingsView } from './system/SettingsView';
import { SystemView } from './system/SystemView';
import { type Theme, useTheme } from './theme';
import { WorkView } from './works/WorkView';
import { WorksList } from './works/WorksList';

export function App() {
  const named = useRoute();
  const serves = useServes();
  // A server that gives the flow alone opens on the flow where the address names no view: the flow is shown at once,
  // and the address is brought to say so.
  const onFlow = opensOnFlow(serves, window.location.hash);
  useEffect(() => {
    if (onFlow) window.location.replace('#/flow');
  }, [onFlow]);
  const route: Route = onFlow ? { view: 'flow' } : named;
  return (
    <Shell route={route} serves={serves}>
      <Views route={route} serves={serves} />
    </Shell>
  );
}

type Loaded = { definition: Definition; given?: Given } | { failed: string };

/// The view a route names. The flow, once shown, stays as the owner left it (what is chosen, the panel) while another
/// tab is shown, and is not read again on coming back.
export function Views({
  route,
  loadFlow = load,
  serves = 'all',
}: {
  route: Route;
  loadFlow?: () => Promise<Loaded>;
  /// What the server gives (serves.ts); nothing while it is being asked.
  serves?: Serves | null;
}) {
  const [flowShown, setFlowShown] = useState(route.view === 'flow');
  if (route.view === 'flow' && !flowShown) setFlowShown(true);
  return (
    <>
      {route.view !== 'flow' && serves !== 'all' && (
        <div className="view">
          {serves === null ? <p className="message muted">Reading</p> : <FlowAlone route={route} />}
        </div>
      )}
      {route.view !== 'flow' && serves === 'all' && (
        <div className="view">
          {route.view === 'console' && (
            // The console's drawing library is large: it is read only once the console is first shown.
            <Suspense fallback={<p className="message muted">Reading the console</p>}>
              <LazyConsolePage named={'item' in route ? route.item : undefined} />
            </Suspense>
          )}
          {route.view === 'works' && <WorksList />}
          {route.view === 'work' && (
            <WorkView key={`${route.work}/${route.run ?? ''}`} work={route.work} run={route.run} />
          )}
          {route.view === 'observations' && <ObservationsView />}
          {route.view === 'system' && <SystemView period={periodOf(route)} />}
          {route.view === 'settings' && <SettingsView />}
        </div>
      )}
      {flowShown && (
        <div className="view" hidden={route.view !== 'flow'}>
          <FlowView active={route.view === 'flow'} loadFlow={loadFlow} />
        </div>
      )}
    </>
  );
}

/// Said in place of the console and the works on a server that gives the flow alone.
function FlowAlone({ route }: { route: Route }) {
  return (
    <p className="message">
      This server gives the flow alone: it has no items and no works to show. <a href="#/flow">Open the flow</a>, or{' '}
      <a href={`?fixtures${hashOf(route)}`}>look at this view with sample data</a>.
    </p>
  );
}

/// The flow view: the flow read from the interface, then drawn.
function FlowView({ active, loadFlow }: { active: boolean; loadFlow: () => Promise<Loaded> }) {
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  useEffect(() => {
    loadFlow().then(setLoaded, (e: unknown) => setLoaded({ failed: e instanceof Error ? e.message : String(e) }));
  }, [loadFlow]);
  if (!loaded) return <p className="message muted">Reading the flow</p>;
  if ('failed' in loaded) return <p className="message">The page could not read the flow: {loaded.failed}</p>;
  return (
    <FlowPage definition={loaded.definition} given={loaded.given} active={active} />
  );
}

/// The views, as the bar's tabs: the console first, which the page is centred on.
export const TABS = [
  { name: 'Console', href: '#/console' },
  { name: 'Works', href: '#/works' },
  { name: 'Flow', href: '#/flow' },
  { name: 'Observations', href: '#/observations' },
  { name: 'System', href: '#/system' },
  { name: 'Settings', href: '#/settings' },
];

/// The page round a view: the bar with the views as tabs, the one shown marked and the console saying how many items
/// wait for the owner; at its right the engine's part and the switch of light and dark. The engine's part, and the
/// items read for the count, are there only where the server gives everything: `wf-agents flow page` gives the flow
/// alone, and has no engine to show.
export function Shell({ route, serves = 'all', children }: {
  route: Route;
  /// What the server gives (serves.ts); nothing while it is being asked.
  serves?: Serves | null;
  children: ReactNode;
}) {
  const [theme, setTheme] = useTheme();
  const [bar, setBar] = useState<BarItems>({ items: null, failed: null });
  // The questions that hold a work, told of by the browser as they come, on the engine's own page only.
  useNotifications(bar.items, serves === 'all' && !usesFixtures(window.location.search));
  const waiting = bar.items?.items.filter((i) => waitsForOwner(i.work.now)).length ?? 0;
  const views = TABS.map((t) =>
    t.href === '#/console' ? { ...t, count: waiting, countTitle: `${waiting} waiting for you` } : t,
  );
  return (
    <div className="page">
      <Bar
        name="wf-agents"
        views={views}
        current={tabOf(route)}
        end={
          <>
            {serves === 'all' && <BarEngine onRead={setBar} />}
            <Switch<Theme>
            label="Light or dark"
            value={theme}
            onChange={setTheme}
            choices={[
              { value: 'auto', name: 'Auto', title: "The system's light or dark" },
              { value: 'light', name: 'Light' },
              { value: 'dark', name: 'Dark' },
            ]}
            />
          </>
        }
      />
      {children}
    </div>
  );
}

/// The engine's part of the bar, with the items it reads, which it tells the bar for the console's count.
function BarEngine({ onRead }: { onRead: (bar: BarItems) => void }) {
  const { items, failed } = useBarItems();
  useEffect(() => onRead({ items, failed }), [items, failed, onRead]);
  return <Engine items={items} failed={failed} />;
}

/// The limits of the flow as a whole, by their keys as `flow.toml` writes them.
function limitsWords(flow: Flow): string {
  const { runs, returns, calls } = flow.limits;
  return `limits runs ${runs}, returns ${returns}, calls ${calls}`;
}

/// How the flow view draws the flow: the matrix of its blocks against its cells, or the flowchart of how a work moves
/// through its cells.
export type ChartView = 'matrix' | 'flowchart';

export function FlowPage({ definition, given, active = true }: {
  definition: Definition;
  /// What an agent is sent as each cell of the definition shown is entered; none where the page was not given it.
  given?: Given;
  /// Whether the flow is the view shown: a flow kept while another tab is shown takes no keys.
  active?: boolean;
}) {
  const [chosen, setChosen] = useState<Chosen>(null);
  // The matrix first; the flowchart, once shown, is kept as the owner left it while the matrix is shown.
  const [chartView, setChartView] = useState<ChartView>('matrix');
  const [graphShown, setGraphShown] = useState(false);
  // Whether the flowchart draws every arrow a write runs; else only the chosen cell's, as what it is given.
  const [dataArrows, setDataArrows] = useState(false);
  if (chartView === 'flowchart' && !graphShown) setGraphShown(true);
  // The window with what is chosen, the console's: whether it is out, and whether its tab brought it out, which keeps
  // it out once what is chosen is let go.
  const [win, setWin] = useState({ open: false, byHand: false });
  const panel = useRef<HTMLElement>(null);
  // How wide the window is, for the room the chart leaves under it.
  const [windowWidth, setWindowWidth] = useState(0);

  const flow = definition.state === 'compiled' ? definition.flow : undefined;
  const drawn = useMemo(() => (flow ? matrix(flow) : undefined), [flow]);
  const graphWaysBack = useMemo(() => (flow ? chartOf(flow).waysBack : 0), [flow]);

  // Choosing slides the window out; letting go slides it back, unless its tab brought it out, in which case it stays
  // until the tab takes it back. The tab takes it back without letting go, and the next choice brings it out again.
  const choose = useCallback((c: Chosen) => {
    setChosen(c);
    setWin((w) => {
      if (c !== null) return w.open ? w : { open: true, byHand: false };
      return w.byHand ? w : { open: false, byHand: false };
    });
  }, []);
  const fold = useCallback(
    () => setWin((w) => (w.open ? { open: false, byHand: false } : { open: true, byHand: true })),
    [],
  );
  // Escape slides the window in while it has the keys, and else lets go of what is chosen; a question the page asks
  // answers it alone.
  useEffect(() => {
    if (!active) return;
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
  }, [active, win.open, choose]);
  const compiled = flow !== undefined;
  useEffect(() => {
    const el = panel.current;
    if (el === null) return;
    setWindowWidth(el.offsetWidth);
    if (typeof ResizeObserver === 'undefined') return;
    const watch = new ResizeObserver(() => setWindowWidth(el.offsetWidth));
    watch.observe(el);
    return () => watch.disconnect();
  }, [compiled]);
  // What the window covers of the chart's right while it is out: what is chosen is brought into the rest.
  const covered = win.open ? windowWidth : 0;

  if (!flow || !drawn) {
    return (
      <>
        <div className="view-line">
          <span className="view-name">The flow does not compile</span>
        </div>
        <main className="faults-view">
          {definition.state === 'faults' && (
            <>
              <p className="muted">Every fault the compiler found, each where it is; the chart shows once it compiles</p>
              <Faults faults={definition.faults} />
            </>
          )}
          {definition.state === 'unreadable' && <p>The flow's directory cannot be read: {definition.reason}</p>}
        </main>
      </>
    );
  }

  const cell = chosen && 'cell' in chosen ? drawn.cells.find((c) => c.id === chosen.cell) : undefined;
  // A block chosen by its name: its datum's row, or else the row of a trigger that stands on it, which a block standing
  // only for a trigger has alone. A trigger's row is named for the trigger, which may be another block's name; a
  // group's trigger stands on no one block.
  const block =
    chosen && 'block' in chosen
      ? (drawn.blocks.find((b) => b.trigger === undefined && b.on === chosen.block) ??
          drawn.blocks.find((b) => b.block !== undefined && b.on === chosen.block))?.block
      : undefined;
  const lineRow = chosen && 'line' in chosen ? drawn.lines.find((l) => l.name === chosen.line) : undefined;
  const testRow =
    chosen && 'test' in chosen ? drawn.conditions.find((c) => c.test && c.name === chosen.test) : undefined;
  const tool = chosen && 'tool' in chosen ? chosen.tool : undefined;
  const group = chosen && 'group' in chosen ? chosen.group : undefined;

  let shown = (
    <Panel label="Nothing chosen">
      <p className="muted">Choose a cell's head or a row's name to see everything about it</p>
    </Panel>
  );
  if (cell) {
    shown = (
      <CellPanel
        cell={cell}
        matrix={drawn}
        flow={flow}
        sent={given?.state === 'compiled' ? given.cells[cell.id] : undefined}
        tools={given?.state === 'compiled' ? given.tools : undefined}
        act={given?.state === 'compiled' ? given.acts[cell.id] : undefined}
        unknown={given?.state === 'compiled' ? given.unknown : undefined}
        onChoose={choose}
      />
    );
  } else if (block) {
    shown = <BlockPanel block={block} matrix={drawn} flow={flow} onChoose={choose} />;
  } else if (lineRow) {
    shown = <LinePanel row={lineRow} flow={flow} onChoose={choose} />;
  } else if (testRow) {
    shown = <TestPanel row={testRow} flow={flow} onChoose={choose} />;
  } else if (tool !== undefined) {
    shown = (
      <ToolPanel
        name={tool}
        words={given?.state === 'compiled' ? given.toolFiles[tool] : undefined}
        unread={given?.state === 'unread' ? given.why : undefined}
      />
    );
  } else if (group !== undefined) {
    shown = <GroupPanel name={group} flow={flow} onChoose={choose} />;
  }

  return (
    <>
      <div className="view-line">
        <span className="view-name">{flow.name}</span>
        <span className="view-facts" title="flow.toml">
          {limitsWords(flow)}
          {flow.tools.length > 0 && (
            <>
              {' · tools '}
              {flow.tools.map((t, i) => (
                <Fragment key={t}>
                  {i > 0 && ', '}
                  <button
                    type="button"
                    className="link mono"
                    aria-pressed={tool === t}
                    onClick={() => choose(tool === t ? null : { tool: t })}
                  >
                    {t}
                  </button>
                </Fragment>
              ))}
            </>
          )}
        </span>
        <span className="view-end">
          <Switch<ChartView>
            label="Matrix or flowchart"
            value={chartView}
            onChange={setChartView}
            choices={[
              { value: 'matrix', name: 'Matrix' },
              { value: 'flowchart', name: 'Flowchart' },
            ]}
          />
          {chartView === 'matrix' ? (
            <Legend waysBack={drawn.waysBack} />
          ) : (
            <>
              <label
                className="view-toggle"
                title="Every arrow a write runs; else only those into and out of the cell chosen"
              >
                <input type="checkbox" checked={dataArrows} onChange={(e) => setDataArrows(e.target.checked)} />
                Data arrows
              </label>
              <GraphLegend waysBack={graphWaysBack} />
            </>
          )}
        </span>
      </div>
      {given?.state === 'unread' && (
        <p className="read-failed" role="alert">
          What an agent is sent could not be read, and the flow is shown without it: {given.why}
        </p>
      )}
      <main className="view-body with-window">
        <Chart
          matrix={drawn}
          chosen={chosen}
          onChoose={choose}
          covered={chartView === 'matrix' ? covered : 0}
          hidden={chartView !== 'matrix'}
        />
        {graphShown && (
          <div className="graph-place" hidden={chartView !== 'flowchart'}>
            <Suspense fallback={<p className="message muted">Reading the flowchart</p>}>
              <LazyFlowChart
                flow={flow}
                dataArrows={dataArrows}
                chosen={chosen}
                onChoose={choose}
                covered={chartView === 'flowchart' ? covered : 0}
              />
            </Suspense>
          </div>
        )}
        <Drawer
          open={win.open}
          onToggle={fold}
          panel={panel}
          label="What is chosen"
          hide="Put the panel away"
          show="Show what is chosen"
          widthKey={WIDTH_KEYS.flowWindow}
        >
          {shown}
        </Drawer>
      </main>
    </>
  );
}
