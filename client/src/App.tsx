// The page: the bar with its tabs, and under it the view the address's hash names (route.ts): the console, the works
// (works/), or the flow. The flow view is the view's line (the flow's name, its limits, the legend, and when comparing
// the switch between the flow as it is and as it was), the chart under it, and the panel docked at its right with what
// the owner chose or what changed. The owner reviews every change to the flow here, so when there is a definition to
// compare against, what changed is marked on the chart and listed in the panel.

import { type CSSProperties, type ReactNode, Suspense, useCallback, useEffect, useMemo, useState } from 'react';

import { type Comparison, type Definition, type Flow, load } from './api';
import { Chart, type Chosen, Legend } from './Chart';
import { LazyConsolePage } from './console/lazy';
import { type Version, changeBlock, changeCell, matrix } from './matrix';
import { Bar, Edge, Panel, StatusLine, Switch } from './parts';
import { type Route, hashOf, tabOf, useRoute } from './route';
import { type Serves, useServes } from './serves';
import { PANEL, WIDTH_KEYS, useKeptWidth } from './sizes';
import { BlockPanel, CellPanel, ChangesList, Faults, type Fold } from './Side';
import { type Theme, useTheme } from './theme';
import { WorkView } from './works/WorkView';
import { WorksList } from './works/WorksList';
import { LIMIT_WORDS } from './words';

export function App() {
  const route = useRoute();
  const serves = useServes();
  // A server that gives the flow alone opens on the flow.
  useEffect(() => {
    if (serves === 'flow' && window.location.hash === '') window.location.replace('#/flow');
  }, [serves]);
  return (
    <Shell route={route}>
      <Views route={route} serves={serves} />
    </Shell>
  );
}

type Loaded = { definition: Definition; comparison: Comparison } | { failed: string };

/// The view a route names. The flow, once shown, stays as the owner left it (what is chosen, the version, the panel)
/// while another tab is shown, and is not read again on coming back.
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
  return <FlowPage definition={loaded.definition} comparison={loaded.comparison} active={active} />;
}

/// The views, as the bar's tabs.
export const TABS = [
  { name: 'Console', href: '#/console' },
  { name: 'Works', href: '#/works' },
  { name: 'Flow', href: '#/flow' },
];

/// The page round a view: the bar with the views as tabs, the one shown marked, and the switch of light and dark.
export function Shell({ route, children }: { route: Route; children: ReactNode }) {
  const [theme, setTheme] = useTheme();
  return (
    <div className="page">
      <Bar
        name="wf-agents"
        views={TABS}
        current={tabOf(route)}
        end={
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
        }
      />
      {children}
    </div>
  );
}

/// The limits of the flow as a whole, in one short muted sentence; the words of each in full on the pointer's tip.
function limitsWords(flow: Flow): string {
  const { runs, returns, calls } = flow.limits;
  return `Limits of a cell: ${runs} ways round in a row, ${returns} returns sent back, ${calls} calls`;
}

function limitsHint(flow: Flow): string {
  const each = (['runs', 'returns', 'calls'] as const).map((l) => `${flow.limits[l]} ${LIMIT_WORDS[l]}`).join(', ');
  return `Each cell, unless it says otherwise: ${each}`;
}

export function FlowPage({ definition, comparison, active = true }: {
  definition: Definition;
  comparison: Comparison;
  /// Whether the flow is the view shown: a flow kept while another tab is shown takes no keys.
  active?: boolean;
}) {
  const [version, setVersion] = useState<Version>('after');
  const [chosen, setChosen] = useState<Chosen>(null);
  // The panel is folded away at first, unless there is a list of changes to read in it.
  const [folded, setFolded] = useState(() => comparison.state !== 'compared' || comparison.changes === null);
  const [panelWidth, setPanelWidth] = useKeptWidth(WIDTH_KEYS.panel, PANEL.min, PANEL.max);

  const after = definition.state === 'compiled' ? definition.flow : undefined;
  const against = comparison.state === 'compared' ? comparison.against : undefined;
  const before = against?.state === 'compiled' ? against.flow : undefined;
  const changes = comparison.state === 'compared' ? (comparison.changes ?? undefined) : undefined;
  const comparing = after !== undefined && before !== undefined && changes !== undefined;

  const shownVersion: Version = comparing ? version : 'after';
  const drawn = useMemo(
    () => (after ? matrix(comparing ? { after, before, changes } : { after }, shownVersion) : undefined),
    [after, before, changes, comparing, shownVersion],
  );

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

  if (!after || !drawn) {
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

  const shownFlow = shownVersion === 'before' && before ? before : after;
  const column = chosen && 'cell' in chosen ? drawn.cells.find((c) => c.id === chosen.cell) : undefined;
  const row = chosen && 'block' in chosen ? drawn.blocks.find((b) => b.name === chosen.block) : undefined;
  const flowOf = (inShown: boolean) => (inShown ? shownFlow : shownFlow === after ? (before ?? after) : after);
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
      <p className="muted">Choose a cell's head or a block's name to see everything about it</p>
    </Panel>
  );
  if (column) {
    panel = (
      <CellPanel
        column={column}
        matrix={drawn}
        flow={flowOf(column.cell !== undefined)}
        changes={(changes ?? []).filter((c) => changeCell(c) === column.id)}
        onChoose={choose}
        fold={fold}
      />
    );
  } else if (row) {
    panel = (
      <BlockPanel
        row={row}
        matrix={drawn}
        flow={flowOf(row.block !== undefined)}
        changes={(changes ?? []).filter((c) => changeBlock(c) === row.name)}
        onChoose={choose}
        fold={fold}
      />
    );
  } else if (comparing) {
    panel = <ChangesList changes={changes} after={after} before={before} onChoose={choose} fold={fold} />;
  } else if (against && against.state !== 'compiled') {
    panel = (
      <Panel
        label="Nothing to compare"
        title="Nothing to compare"
        lines={
          <StatusLine tone="removed" word={against.state === 'faults' ? 'Does not compile' : 'Cannot be read'}>
            the definition compared against
          </StatusLine>
        }
        {...fold}
      >
        {against.state === 'faults' ? <Faults faults={against.faults} /> : <p>{against.reason}</p>}
      </Panel>
    );
  }

  return (
    <>
      <div className="view-line">
        <span className="view-name">{shownFlow.name}</span>
        {comparing && (
          <Switch<Version>
            label="Version of the flow"
            value={shownVersion}
            onChange={setVersion}
            choices={[
              { value: 'after', name: 'As it is' },
              { value: 'before', name: 'As it was' },
            ]}
          />
        )}
        <span className="view-facts" title={limitsHint(shownFlow)}>
          {limitsWords(shownFlow)}
        </span>
        <span className="view-end">
          <Legend waysBack={drawn.waysBack} comparing={comparing} />
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
