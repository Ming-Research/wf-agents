// The flow page: one screen, the chart of the flow and beside it what the owner chose or what changed. The owner
// reviews every change to the flow here, so when there is a definition to compare against, what changed is marked on
// the chart and listed beside it, and the owner can switch between the flow as it is and as it was.

import { SegmentedControl } from '@primer/react';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { type Comparison, type Definition, type Flow, load } from './api';
import { Chart, type Chosen, Legend } from './Chart';
import { type Version, changeBlock, changeCell, matrix } from './matrix';
import { BlockPanel, CellPanel, ChangesList, Faults } from './Side';
import { LIMIT_WORDS } from './words';

type Loaded = { definition: Definition; comparison: Comparison } | { failed: string };

export function App() {
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  useEffect(() => {
    load().then(setLoaded, (e: unknown) => setLoaded({ failed: e instanceof Error ? e.message : String(e) }));
  }, []);
  if (!loaded) return <p className="message quiet">Reading the flow…</p>;
  if ('failed' in loaded) return <p className="message">The page could not read the flow: {loaded.failed}</p>;
  return <FlowPage definition={loaded.definition} comparison={loaded.comparison} />;
}

/// The limits of the flow as a whole, in one line of the bar.
function FlowLimits({ flow }: { flow: Flow }) {
  return (
    <span className="flow-limits">
      Each cell, unless it says otherwise:{' '}
      {(['runs', 'returns', 'calls'] as const).map((limit, i) => (
        <span key={limit}>
          {i > 0 && ' · '}
          <strong>{flow.limits[limit]}</strong> {LIMIT_WORDS[limit]}
        </span>
      ))}
    </span>
  );
}

export function FlowPage({ definition, comparison }: { definition: Definition; comparison: Comparison }) {
  const [version, setVersion] = useState<Version>('after');
  const [chosen, setChosen] = useState<Chosen>(null);

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

  const close = useCallback((e: KeyboardEvent) => {
    if (e.key === 'Escape') setChosen(null);
  }, []);
  useEffect(() => {
    window.addEventListener('keydown', close);
    return () => window.removeEventListener('keydown', close);
  }, [close]);

  if (!after || !drawn) {
    return (
      <div className="page">
        <header className="bar">
          <h1>The flow does not compile</h1>
        </header>
        <main className="alone">
          {definition.state === 'faults' && (
            <>
              <p>Every fault the compiler found, each where it is; the chart shows once the definition compiles.</p>
              <Faults faults={definition.faults} />
            </>
          )}
          {definition.state === 'unreadable' && <p>The flow's directory cannot be read: {definition.reason}</p>}
        </main>
      </div>
    );
  }

  const shownFlow = shownVersion === 'before' && before ? before : after;
  const column = chosen && 'cell' in chosen ? drawn.cells.find((c) => c.id === chosen.cell) : undefined;
  const row = chosen && 'block' in chosen ? drawn.blocks.find((b) => b.name === chosen.block) : undefined;
  const flowOf = (inShown: boolean) => (inShown ? shownFlow : shownFlow === after ? (before ?? after) : after);

  let side = null;
  if (column) {
    side = (
      <CellPanel
        column={column}
        matrix={drawn}
        flow={flowOf(column.cell !== undefined)}
        changes={(changes ?? []).filter((c) => changeCell(c) === column.id)}
        onChoose={setChosen}
      />
    );
  } else if (row) {
    side = (
      <BlockPanel
        row={row}
        matrix={drawn}
        flow={flowOf(row.block !== undefined)}
        changes={(changes ?? []).filter((c) => changeBlock(c) === row.name)}
        onChoose={setChosen}
      />
    );
  } else if (comparing) {
    side = <ChangesList changes={changes} after={after} before={before} onChoose={setChosen} />;
  } else if (against && against.state !== 'compiled') {
    side = (
      <section className="side-section" aria-label="What changed">
        <h2 className="side-title">Nothing to compare</h2>
        {against.state === 'faults' ? (
          <>
            <p>The definition compared against does not compile:</p>
            <Faults faults={against.faults} />
          </>
        ) : (
          <p>The definition compared against cannot be read: {against.reason}</p>
        )}
      </section>
    );
  }

  return (
    <div className="page">
      <header className="bar">
        <h1>{shownFlow.name}</h1>
        {comparing && (
          <SegmentedControl
            aria-label="Version of the flow"
            size="small"
            onChange={(i) => setVersion(i === 0 ? 'after' : 'before')}
          >
            <SegmentedControl.Button selected={shownVersion === 'after'}>As it is</SegmentedControl.Button>
            <SegmentedControl.Button selected={shownVersion === 'before'}>As it was</SegmentedControl.Button>
          </SegmentedControl>
        )}
        <FlowLimits flow={shownFlow} />
        <Legend waysBack={drawn.waysBack} comparing={comparing} />
      </header>
      <main className={side ? 'with-side' : ''}>
        <Chart matrix={drawn} chosen={chosen} onChoose={setChosen} />
        {side && <aside className="side">{side}</aside>}
      </main>
    </div>
  );
}
