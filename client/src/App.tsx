// The page: the bar with its tabs, and under it the view the address's hash names (route.ts): the console, the works
// (works/), the flow, the observations, the system or the settings. The flow view is the view's line (the flow's name, its limits, the legend), the chart under it,
// and the panel docked at its right with what the owner chose.

import { type CSSProperties, type ReactNode, Suspense, useCallback, useEffect, useMemo, useState } from 'react';

import { type Definition, type Flow, type Given, load } from './api';
import { Chart, type Chosen, Legend } from './Chart';
import { LazyConsolePage } from './console/lazy';
import { type BarItems, Engine, useBarItems, waitsForOwner } from './Engine';
import { matrix } from './matrix';
import { Bar, Edge, Panel, Switch } from './parts';
import { type Route, hashOf, tabOf, useRoute } from './route';
import { type Serves, opensOnFlow, useServes } from './serves';
import { PANEL, WIDTH_KEYS, useKeptWidth } from './sizes';
import { BlockPanel, CellPanel, Faults, type Fold, LinePanel, TestPanel } from './Side';
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
              <LazyConsolePage />
            </Suspense>
          )}
          {route.view === 'works' && <WorksList />}
          {route.view === 'work' && <WorkView key={route.work} work={route.work} />}
          {(route.view === 'observations' || route.view === 'system' || route.view === 'settings') && (
            <p className="message muted">Nothing is shown here yet.</p>
          )}
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

export function FlowPage({ definition, given, active = true }: {
  definition: Definition;
  /// What an agent is sent as each cell of the definition shown is entered; none where the page was not given it.
  given?: Given;
  /// Whether the flow is the view shown: a flow kept while another tab is shown takes no keys.
  active?: boolean;
}) {
  const [chosen, setChosen] = useState<Chosen>(null);
  // The panel is folded away at first: its room stays the chart's until something is chosen.
  const [folded, setFolded] = useState(true);
  const [panelWidth, setPanelWidth] = useKeptWidth(WIDTH_KEYS.panel, PANEL.min, PANEL.max);

  const flow = definition.state === 'compiled' ? definition.flow : undefined;
  const drawn = useMemo(() => (flow ? matrix(flow) : undefined), [flow]);

  // Choosing brings the panel back if it was folded away; Escape lets go of what is chosen.
  const choose = useCallback((c: Chosen) => {
    setChosen(c);
    if (c !== null) setFolded(false);
  }, []);
  const close = useCallback((e: KeyboardEvent) => {
    if (e.key === 'Escape') setChosen(null);
  }, []);
  useEffect(() => {
    if (!active) return;
    window.addEventListener('keydown', close);
    return () => window.removeEventListener('keydown', close);
  }, [close, active]);

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
  // A block chosen by its name: a datum's row, or else the row of a trigger that stands on it, which a block standing
  // only for a trigger has alone.
  const row =
    chosen && 'block' in chosen
      ? (drawn.blocks.find((b) => b.name === chosen.block) ?? drawn.blocks.find((b) => b.on === chosen.block))
      : undefined;
  const lineRow = chosen && 'line' in chosen ? drawn.lines.find((l) => l.name === chosen.line) : undefined;
  const testRow =
    chosen && 'test' in chosen ? drawn.conditions.find((c) => c.test && c.name === chosen.test) : undefined;
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

  let panel = (
    <Panel label="Nothing chosen" {...fold}>
      <p className="muted">Choose a cell's head or a row's name to see everything about it</p>
    </Panel>
  );
  if (cell) {
    panel = (
      <CellPanel
        cell={cell}
        matrix={drawn}
        flow={flow}
        sent={given?.state === 'compiled' ? given.cells[cell.id] : undefined}
        tools={given?.state === 'compiled' ? given.tools : undefined}
        act={given?.state === 'compiled' ? given.acts[cell.id] : undefined}
        unknown={given?.state === 'compiled' ? given.unknown : undefined}
        onChoose={choose}
        fold={fold}
      />
    );
  } else if (row) {
    panel = (
      <BlockPanel
        row={row}
        matrix={drawn}
        flow={flow}
        onChoose={choose}
        fold={fold}
      />
    );
  } else if (lineRow) {
    panel = <LinePanel row={lineRow} flow={flow} onChoose={choose} fold={fold} />;
  } else if (testRow) {
    panel = <TestPanel row={testRow} flow={flow} onChoose={choose} fold={fold} />;
  }

  return (
    <>
      <div className="view-line">
        <span className="view-name">{flow.name}</span>
        <span className="view-facts" title="flow.toml">
          {limitsWords(flow)}
        </span>
        <span className="view-end">
          <Legend waysBack={drawn.waysBack} />
        </span>
      </div>
      <main
        className={`view-body with-panel ${folded ? 'panel-folded' : ''}`}
        style={panelWidth !== null ? ({ '--panel-width': `${panelWidth}px` } as CSSProperties) : undefined}
      >
        <Chart matrix={drawn} chosen={chosen} onChoose={choose} />
        {panel}
      </main>
    </>
  );
}
