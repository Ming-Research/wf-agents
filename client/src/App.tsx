// The page: the bar, and the flow view under it, the only view there is yet. The flow view is the view's line (the
// flow's name, its limits, the legend, and when comparing the switch between the flow as it is and as it was), the
// chart under it, and the panel docked at its right with what the owner chose or what changed. The owner reviews every
// change to the flow here, so when there is a definition to compare against, what changed is marked on the chart and
// listed in the panel.

import { type CSSProperties, type ReactNode, useCallback, useEffect, useMemo, useState } from 'react';

import { type Comparison, type Definition, type Flow, load } from './api';
import { Chart, type Chosen, Legend } from './Chart';
import { type Version, changeBlock, changeCell, matrix } from './matrix';
import { Bar, Edge, Panel, StatusLine, Switch } from './parts';
import { WIDTH_KEYS, useKeptWidth } from './sizes';
import { BlockPanel, CellPanel, ChangesList, Faults, type Fold } from './Side';
import { type Theme, useTheme } from './theme';
import { LIMIT_WORDS } from './words';

/// The panel's width at first, and the least and most it is dragged to.
const PANEL = { first: 360, min: 280, max: 720 };

type Loaded = { definition: Definition; comparison: Comparison } | { failed: string };

export function App() {
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  useEffect(() => {
    load().then(setLoaded, (e: unknown) => setLoaded({ failed: e instanceof Error ? e.message : String(e) }));
  }, []);
  return (
    <Shell>
      {!loaded ? (
        <p className="message muted">Reading the flow</p>
      ) : 'failed' in loaded ? (
        <p className="message">The page could not read the flow: {loaded.failed}</p>
      ) : (
        <FlowPage definition={loaded.definition} comparison={loaded.comparison} />
      )}
    </Shell>
  );
}

/// The page round a view: the bar with the views, of which only the flow exists yet, and the switch of light and dark.
export function Shell({ children }: { children: ReactNode }) {
  const [theme, setTheme] = useTheme();
  return (
    <div className="page">
      <Bar
        name="wf-agents"
        views={[{ name: 'Flow', href: '/' }]}
        current="/"
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

export function FlowPage({ definition, comparison }: { definition: Definition; comparison: Comparison }) {
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
    window.addEventListener('keydown', close);
    return () => window.removeEventListener('keydown', close);
  }, [close]);

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
