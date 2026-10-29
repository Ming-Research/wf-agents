// The panels of what the engine does beside the cells: the actions on a line, each with what it does and the stores it
// writes, and the lines through it; and a store, with what it holds, where it belongs, and in each graph what writes it
// and which cells read it, each cell a link to its own panel in its own graph.

import { Fragment } from 'react';

import type { CompiledGraph } from '../api';
import type { Graph } from '../graph/FlowChart';
import { actionWords, type Chart } from '../graph/flowModel';
import { storeUses, writesOf } from '../graph/stores';
import { LinkButton } from '../parts';
import { t } from '../text';
import { Named, Pairs, Part } from '../voices';
import { useFlow } from './context';
import { inline } from './words';

/// A cell of a graph, by its code and title, that opens its panel in that graph.
function CellIn({ g, id, extra }: { g: CompiledGraph; id: string; extra?: string | null }) {
  const f = useFlow();
  const c = g.cells.find((x) => x.id === id);
  return (
    <LinkButton onClick={() => f.open({ kind: 'cell', cell: id, graph: g.name })}>
      <span className="cell-id">{id}</span> {c?.title ?? ''}
      {extra ? ` · ${extra}` : null}
    </LinkButton>
  );
}

/// Stores, each a link to its panel.
function StoreLinks({ names }: { names: string[] }) {
  const f = useFlow();
  const stores = f.compiled.engine.stores ?? {};
  if (names.length === 0) return <span className="pp-facts">{t('none')}</span>;
  return (
    <span>
      {names.map((n, i) => (
        <Fragment key={n}>
          {i > 0 ? ', ' : null}
          <LinkButton onClick={() => f.open({ kind: 'store', store: n })}>{stores[n]?.title ?? n}</LinkButton>
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
  if (n === undefined) return <p className="pp-note">{t('none')}</p>;
  const into = chart.edges.filter((e) => e.target === node && e.cell !== null);
  return (
    <div className="pp-page">
      <Part title={t('sec_actions')}>
        {n.actions.map((a) => (
          <div key={a} className="pp-part">
            <Named title={actionWords(a)} code={a} as="h4" />
            <p className="pp-note">{f.compiled.engine.actions[a] ?? ''}</p>
            <p className="pp-note">
              {t('action_writes')} <StoreLinks names={writesOf(f.compiled.engine, [a])} />
            </p>
          </div>
        ))}
      </Part>
      <Part title={t('sec_action_lines')}>
        {into.map((e) => (
          <div key={e.id}>
            <CellIn g={f.g} id={e.cell ?? ''} extra={e.label} />
          </div>
        ))}
        <p className="pp-note">
          {t('action_into')} <CellIn g={f.g} id={n.cell} />
        </p>
      </Part>
    </div>
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
  if (s === undefined) return <p className="pp-note">{t('none')}</p>;
  const several = graphs.length > 1;
  return (
    <div className="pp-page">
      <div className="pp-part">
        <p className="pp-plain">{s.desc}</p>
        {s.owner ? <p className="pp-note">{t('store_owner', { what: s.owner })}</p> : null}
      </div>
      {graphs.map(({ g }) => {
        const uses = storeUses(f.compiled.engine, g, store);
        if (uses.writes.length === 0 && uses.reads.length === 0) return null;
        return (
          <Fragment key={g.name}>
            <Part title={several ? t('store_written_in', { graph: g.name }) : t('store_written')}>
              {uses.writes.length === 0 ? <p className="pp-note">{t(s.computed ? 'store_computed_long' : 'none')}</p> : null}
              <Pairs
                rows={uses.writes.map((w) => [
                  <span key="w">
                    {actionWords(w.what)}{' '}
                    <span className="pp-facts">{inline(t(w.step ? 'store_by_step' : 'store_by_action', { name: `\`${w.what}\`` }))}</span>
                  </span>,
                  <span key="c">
                    {w.at.map((x, i) => (
                      <Fragment key={`${x.cell}:${x.row ?? ''}`}>
                        {i > 0 ? ', ' : null}
                        <CellIn g={g} id={x.cell} extra={x.row} />
                      </Fragment>
                    ))}
                  </span>,
                ])}
              />
            </Part>
            <Part title={several ? t('store_read_in', { graph: g.name }) : t('store_read')}>
              {uses.reads.length === 0 ? <p className="pp-note">{t('none')}</p> : null}
              <span>
                {uses.reads.map((id, i) => (
                  <Fragment key={id}>
                    {i > 0 ? ', ' : null}
                    <CellIn g={g} id={id} />
                  </Fragment>
                ))}
              </span>
            </Part>
          </Fragment>
        );
      })}
    </div>
  );
}
