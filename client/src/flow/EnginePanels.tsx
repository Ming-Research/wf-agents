// The panels of what the engine does beside the cells: the actions on a line, each with what it does and the stores it
// writes, and the lines through it; and a store, with what it holds, where it belongs, and in each graph what writes it
// and which cells read it, each cell a link to its own panel in its own graph.

import { Fragment } from 'react';

import type { CompiledGraph } from '../api';
import type { Graph } from '../graph/FlowChart';
import { actionWords, type Chart } from '../graph/flowModel';
import { storeUses, writesOf } from '../graph/stores';
import { t } from '../text';
import { Section } from './CellPanel';
import { useFlow } from './context';
import { inline } from './words';

/// A cell of a graph, by its id and title, that opens its panel in that graph.
function CellIn({ g, id, extra }: { g: CompiledGraph; id: string; extra?: string | null }) {
  const f = useFlow();
  const c = g.cells.find((x) => x.id === id);
  return (
    <button type="button" className="go" onClick={() => f.open({ kind: 'cell', cell: id, graph: g.name })}>
      <strong>{id}</strong> {c?.title ?? ''}
      {extra ? <span className="go-extra"> · {extra}</span> : null}
    </button>
  );
}

/// Stores, each a link to its panel.
function StoreLinks({ names }: { names: string[] }) {
  const f = useFlow();
  const stores = f.compiled.engine.stores ?? {};
  if (names.length === 0) return <span className="muted">{t('none')}</span>;
  return (
    <span className="links">
      {names.map((n, i) => (
        <Fragment key={n}>
          {i > 0 ? ', ' : null}
          <button type="button" className="go" onClick={() => f.open({ kind: 'store', store: n })}>
            {stores[n]?.title ?? n}
          </button>
        </Fragment>
      ))}
    </span>
  );
}

/// The actions on a line: what each does and the stores it writes; then the rows of each cell that go through them, and
/// the cell they lead into.
export function ActionPanel({ chart, node }: { chart: Chart; node: string }) {
  const f = useFlow();
  const n = chart.nodes.find((x) => x.id === node);
  if (n === undefined) return <p className="fmt">{t('none')}</p>;
  const into = chart.edges.filter((e) => e.target === node && e.cell !== null);
  return (
    <>
      <Section title={t('sec_actions')}>
        <ul className="bl">
          {n.actions.map((a) => (
            <li key={a}>
              <strong>{actionWords(a)}</strong> <code>{a}</code>
              <div className="fd">{f.compiled.engine.actions[a] ?? ''}</div>
              <div className="srcs">
                <span className="ul">{t('action_writes')}</span>
                <StoreLinks names={writesOf(f.compiled.engine, [a])} />
              </div>
            </li>
          ))}
        </ul>
      </Section>
      <Section title={t('sec_action_lines')}>
        <ul className="bl">
          {into.map((e) => (
            <li key={e.id}>
              <CellIn g={f.g} id={e.cell ?? ''} extra={e.label} />
            </li>
          ))}
        </ul>
        <p className="fmt">
          {t('action_into')} <CellIn g={f.g} id={n.cell} />
        </p>
      </Section>
    </>
  );
}

/// The words under a store's title: where it belongs, and that it is computed when it is.
export function storeWho(layer: string, computed: boolean): string {
  const where = t(
    layer === 'project' ? 'store_of_project' : layer === 'item' ? 'store_of_item' : layer === 'work' ? 'store_of_work' : 'store_of_round',
  );
  return computed ? `${where} · ${t('store_computed_long')}` : where;
}

/// A store: what it holds; what the owner does that writes it; then in each graph, what writes it and the cells that
/// carry that out, and the cells that read it.
export function StorePanel({ store, graphs }: { store: string; graphs: Graph[] }) {
  const f = useFlow();
  const s = f.compiled.engine.stores?.[store];
  if (s === undefined) return <p className="fmt">{t('none')}</p>;
  const several = graphs.length > 1;
  return (
    <>
      <Section>
        <p>{s.desc}</p>
        {s.owner ? <p className="fmt">{t('store_owner', { what: s.owner })}</p> : null}
      </Section>
      {graphs.map(({ g }) => {
        const uses = storeUses(f.compiled.engine, g, store);
        if (uses.writes.length === 0 && uses.reads.length === 0) return null;
        return (
          <Fragment key={g.name}>
            <Section title={several ? t('store_written_in', { graph: g.name }) : t('store_written')}>
              {uses.writes.length === 0 ? <p className="fmt">{t(s.computed ? 'store_computed_long' : 'none')}</p> : null}
              <ul className="bl">
                {uses.writes.map((w) => (
                  <li key={`${w.step}:${w.what}`}>
                    <strong>{actionWords(w.what)}</strong>{' '}
                    <span className="fw">{inline(t(w.step ? 'store_by_step' : 'store_by_action', { name: `\`${w.what}\`` }))}</span>
                    <div className="srcs">
                      {w.at.map((x, i) => (
                        <Fragment key={`${x.cell}:${x.row ?? ''}`}>
                          {i > 0 ? ', ' : null}
                          <CellIn g={g} id={x.cell} extra={x.row} />
                        </Fragment>
                      ))}
                    </div>
                  </li>
                ))}
              </ul>
            </Section>
            <Section title={several ? t('store_read_in', { graph: g.name }) : t('store_read')}>
              {uses.reads.length === 0 ? <p className="fmt">{t('none')}</p> : null}
              <div className="srcs">
                {uses.reads.map((id, i) => (
                  <Fragment key={id}>
                    {i > 0 ? ', ' : null}
                    <CellIn g={g} id={id} />
                  </Fragment>
                ))}
              </div>
            </Section>
          </Fragment>
        );
      })}
    </>
  );
}
