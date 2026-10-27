// Any stored flow version's state machine, and a cell's definition when one is clicked.

import { useState } from 'react';

import type { Compiled, FlowVersion } from '../api';
import { Loaded } from '../components';
import { FlowChart } from '../graph/FlowChart';
import { useResource, useStatus } from '../live';
import { href, navigate } from '../route';
import { useT } from '../text';
import { CellView } from './CellView';

function Version({ version, cell }: { version: number; cell: string | null }) {
  const t = useT();
  const flow = useResource<Compiled>(null, `/api/v1/flows/${version}`);
  const [graph, setGraph] = useState<string | null>(null);
  return (
    <Loaded r={flow}>
      {(c) => {
        const g = c.graphs.find((x) => x.name === graph) ?? c.graphs[0];
        if (!g) return <p className="muted">{t('none')}</p>;
        const def = cell !== null ? (g.cells.find((x) => x.id === cell) ?? null) : null;
        return (
          <>
            {c.graphs.length > 1 ? (
              <div className="tabs">
                {c.graphs.map((x) => (
                  <button
                    key={x.name}
                    className={x.name === g.name ? 'tab tab-on' : 'tab'}
                    onClick={() => setGraph(x.name)}
                  >
                    {x.name}
                  </button>
                ))}
              </div>
            ) : null}
            <div className="split">
              <div className="split-chart">
                <FlowChart
                  graph={g}
                  selected={cell}
                  onSelect={(id) => navigate(href({ page: 'flow', version, cell: id }), true)}
                />
              </div>
              <div className="split-side">
                {def ? <CellView cell={def} /> : <p className="muted">{t('pick_cell')}</p>}
              </div>
            </div>
          </>
        );
      }}
    </Loaded>
  );
}

export function Flow({ version, cell }: { version: number | null; cell: string | null }) {
  const t = useT();
  const status = useStatus();
  const versions = useResource<FlowVersion[]>('flows', '/api/v1/flows');
  return (
    <div className="page page-wide">
      <Loaded r={versions}>
        {(all) => {
          const chosen = version ?? status?.flow_version ?? all[0]?.id ?? null;
          return (
            <>
              <header className="work-head">
                <h1>{t('flow_title')}</h1>
                <div className="head-facts">
                  <select
                    value={chosen ?? ''}
                    onChange={(e) => navigate(href({ page: 'flow', version: Number(e.target.value), cell: null }))}
                  >
                    {all.map((v) => (
                      <option key={v.id} value={v.id}>
                        {t('flow_version', { id: v.id })} · {v.digest.slice(0, 12)} · {t('flow_works', { n: v.works })}
                        {status?.flow_version === v.id ? ` · ${t('flow_current')}` : ''}
                      </option>
                    ))}
                  </select>
                </div>
              </header>
              {chosen !== null ? <Version key={chosen} version={chosen} cell={cell} /> : null}
            </>
          );
        }}
      </Loaded>
    </div>
  );
}
