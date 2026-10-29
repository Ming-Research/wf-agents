// A flow's chart filling the window under the bar, its graphs side by side, and a column docked at its right, whose
// width drags at its left edge, sharing the width evenly with the chart at first on a work's page and the architect's.
// On a work's page the work's panel is on top, at the column's full height, and while a cell, a rule, the
// interjection, a comparison or a round is open, its panel is in the lower part, split from the top by a divider that
// drags; on the architect's page what is open takes the place of the panel's list, with a way back to it; on the flow
// page that panel is alone, while something is open. The flow page, a work's page and the architect's page all show a
// flow this way.

import { type ReactNode, useEffect, useMemo, useRef, useState } from 'react';

import type { Compiled, FlowDiff, RoundSummary } from '../api';
import { FlowChart, type FlowChartHandle, type Graph, type Pick } from '../graph/FlowChart';
import { roundName } from '../format';
import type { Marks } from '../graph/flowDraw';
import { actionWords, chart } from '../graph/flowModel';
import { t } from '../text';
import { CellPanel, type CellTab, cellWhere } from './CellPanel';
import { DiffPanel } from './Changes';
import { ActionPanel, StorePanel, storeWho } from './EnginePanels';
import { type Flow, flowOf, FlowContext, type Subject, subjectKey } from './context';
import { Grip, Panel, savedSize, useTrail } from './Panel';
import { CellLinks, inline } from './words';

const same = (a: Subject, b: Subject) => subjectKey(a) === subjectKey(b);

/// The graph a cell is of: the first that has it; else the first graph.
export function graphOfCell(graphs: Graph[], cell: string): Graph | undefined {
  return graphs.find((x) => x.g.cells.some((c) => c.id === cell)) ?? graphs[0];
}

/// A subject with its graph named: a cell's the graph that has it, a rule's, the interjection's and the actions on a
/// line's the first graph, when the subject names none.
export function withGraph(s: Subject, graphs: Graph[]): Subject {
  switch (s.kind) {
    case 'cell':
      return s.graph !== undefined ? s : { ...s, graph: graphOfCell(graphs, s.cell)?.g.name };
    case 'rule':
    case 'interjection':
    case 'action':
      return s.graph !== undefined ? s : { ...s, graph: graphs[0]?.g.name };
    default:
      return s;
  }
}

export type FlowScreenProps = {
  compiled: Compiled;
  /// The graphs drawn side by side, left to right; the same object while they are the same, as each new list is laid
  /// out again.
  graphs: Graph[];
  legend: 'flow' | 'work' | 'compare';
  /// What is marked on each graph, in the same order.
  marks: Array<Omit<Marks, 'selected'>>;
  /// The cell the address names: its panel is open.
  cell: string | null;
  /// The graph of that cell, where two graphs drawn may each have a cell of its name; with none, the first that has it.
  cellGraph?: string | null;
  /// Called when the panel opens a cell, with its graph, or closes; the page keeps it in its address.
  onCell: (cell: string | null, graph?: string) => void;
  /// The cell the chart opens on, and its graph.
  start: string | null;
  startGraph?: string | null;
  /// When two flows are compared: what changed, which the panel opens on.
  diff?: FlowDiff | null;
  /// On a work's page: the work, its rounds, a round's panel, and the round the address names, each round by its
  /// number in the work. A work runs on the first graph.
  work?: number;
  rounds?: RoundSummary[];
  round?: number | null;
  onRound?: (round: number | null) => void;
  renderRound?: (round: number) => ReactNode;
  /// On a work's page: the work's panel, on top of the column. On the architect's page, given what is open on the chart
  /// (none while nothing is), the whole column, which puts what is open in place of its list.
  side?: ReactNode | ((open: ReactNode | null) => ReactNode);
  /// When what is open takes the place of a list: the list's name, which the way back to it says.
  list?: string;
};

const DOCK_KEY = 'wf-agents.dock-width';
const DOCK_MIN = 320;
const SPLIT_KEY = 'wf-agents.dock-split';
const SPLIT_MIN = 120;

function RulePanel({ name }: { name: string }) {
  return (
    <FlowContext.Consumer>
      {(f) => {
        const rule = f?.g.rules.find((r) => r.name === name);
        const at = f?.g.cells.filter((c) => (c.rules ?? []).some((r) => r.name === name)).map((c) => c.id) ?? [];
        return (
          <>
            <section className="ds">
              <h4>{t('sec_rule_text')}</h4>
              <ul className="bl">
                {(rule?.text ?? []).map((l, i) => (
                  <li key={i}>{inline(l)}</li>
                ))}
              </ul>
            </section>
            <section className="ds">
              <h4>{t('sec_rule_at')}</h4>
              <p className="fmt">{inline(t('rule_at_note', { address: `\`rule:${name}\`` }))}</p>
              <CellLinks ids={at} />
            </section>
          </>
        );
      }}
    </FlowContext.Consumer>
  );
}

function InterjectionPanel() {
  return (
    <FlowContext.Consumer>
      {(f) => {
        const ij = f?.g.interject;
        if (!f || !ij) return null;
        const steps = f.g.cells.filter((c) => c.kind !== 'agent' && c.rows.some((r) => r.interject)).map((c) => c.id);
        return (
          <>
            <section className="ds">
              <h4>{t('sec_ij_from')}</h4>
              <ul className="bl">
                {ij.from.map((x) => (
                  <li key={x.name}>
                    <strong>{x.name}</strong>: {inline(x.desc)}
                  </li>
                ))}
              </ul>
            </section>
            <section className="ds">
              <h4>{t('sec_ij_text')}</h4>
              <ul className="bl">
                {ij.text.map((l, i) => (
                  <li key={i}>{inline(l)}</li>
                ))}
              </ul>
            </section>
            <section className="ds">
              <h4>{t('sec_ij_at')}</h4>
              <p className="fmt">{inline(t('ij_agents'))}</p>
              <CellLinks ids={steps} />
            </section>
          </>
        );
      }}
    </FlowContext.Consumer>
  );
}

export function FlowScreen(props: FlowScreenProps) {
  const {
    compiled,
    graphs,
    legend,
    marks,
    cell,
    cellGraph,
    onCell,
    start,
    startGraph,
    diff,
    work,
    rounds,
    round,
    onRound,
    renderRound,
    side,
    list,
  } = props;
  const [width, setWidth] = useState<number | null>(() => savedSize(DOCK_KEY, DOCK_MIN));
  const [split, setSplit] = useState<number | null>(() => savedSize(SPLIT_KEY, SPLIT_MIN));
  const nameOf = (n: number) => roundName(work ?? 0, n);
  const chartRef = useRef<FlowChartHandle>(null);
  const panelRef = useRef<HTMLElement>(null);
  const trail = useTrail<Subject>(same);
  // The node whose panel is open, in its graph, or the store.
  const [picked, setPicked] = useState<{ graph: string; node: string } | { store: string } | null>(null);
  // The tab of a cell's panel the owner chose last: going to another cell keeps it.
  const [cellTab, setCellTab] = useState<CellTab | null>(null);
  const open = (s: Subject, from?: Subject) =>
    trail.open(withGraph(s, graphs), from !== undefined ? withGraph(from, graphs) : undefined);
  const pickCell = (graph: string | undefined, c: string) =>
    setPicked({ graph: graph ?? graphOfCell(graphs, c)?.g.name ?? '', node: `c:${c}` });
  // Each graph's lookups for the panels opened in it; a panel that opens a cell brings it into view.
  const flows = useMemo(
    () =>
      new Map<string, Flow>(
        graphs.map(({ g, view }) => [
          g.name,
          flowOf(compiled, g, view, (s) => {
            trail.open(withGraph(s, graphs));
            if (s.kind === 'cell') {
              const graph = s.graph ?? g.name;
              setPicked({ graph, node: `c:${s.cell}` });
              window.setTimeout(() => chartRef.current?.focus(s.cell, false, graph), 0);
            } else if (s.kind === 'store') {
              setPicked({ store: s.store });
            }
          }),
        ]),
      ),
    // `trail.open` keeps no state of its own between renders.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [compiled, graphs],
  );
  // Each graph's chart, for the panel of the actions on a line, which names the rows through them.
  const charts = useMemo(
    () => new Map(graphs.map(({ g, view }) => [g.name, chart(g, view, compiled.engine)])),
    [graphs, compiled],
  );
  const first = graphs[0];
  const flowFor = (graph: string | undefined): Flow | null =>
    flows.get(graph ?? '') ?? (first ? (flows.get(first.g.name) ?? null) : null);
  const graphFor = (graph: string | undefined): Graph | undefined => graphs.find((x) => x.g.name === graph) ?? first;
  // The address and the panel follow each other: a cell or round named in the address is opened, and opening one
  // writes it there.
  const now = trail.now;
  useEffect(() => {
    if (round != null) {
      if (now?.kind !== 'round' || now.round !== round) open({ kind: 'round', round });
    } else if (cell !== null) {
      const graph = cellGraph ?? undefined;
      if (now?.kind !== 'cell' || now.cell !== cell || (graph !== undefined && now.graph !== graph)) {
        open(graph !== undefined ? { kind: 'cell', cell, graph } : { kind: 'cell', cell });
        pickCell(graph, cell);
      }
    }
    // Only when the address changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cell, cellGraph, round]);
  useEffect(() => {
    if (now === null) {
      if (cell !== null) onCell(null);
      if (round != null) onRound?.(null);
    } else if (now.kind === 'cell' && (now.cell !== cell || (cellGraph != null && now.graph !== cellGraph))) {
      onCell(now.cell, now.graph);
    } else if (now.kind === 'round' && now.round !== round) {
      onRound?.(now.round);
    }
    // Only when the panel changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [now]);
  // A comparison opens on what changed.
  useEffect(() => {
    if (diff && cell === null && round == null) open({ kind: 'changes' });
    // Only when the comparison changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [diff]);
  // Brings a cell into view once the panel that opens with it is drawn, as the chart narrows to make room for it.
  const into = (c: string, keep: boolean, graph: string) =>
    window.setTimeout(() => chartRef.current?.focus(c, keep, graph), 0);
  const onPick = (p: Pick) => {
    // A store is every graph's.
    if (p.kind === 'store') {
      setPicked({ store: p.store });
      open({ kind: 'store', store: p.store });
      return;
    }
    const graph = p.graph;
    switch (p.kind) {
      case 'cell':
        pickCell(graph, p.cell);
        open({ kind: 'cell', graph, cell: p.cell });
        into(p.cell, true, graph);
        break;
      case 'rule':
        setPicked({ graph, node: `r:${p.rule}:${p.cell}` });
        open({ kind: 'rule', graph, rule: p.rule }, { kind: 'cell', graph, cell: p.cell });
        break;
      case 'interjection':
        setPicked({ graph, node: `j:${p.cell}` });
        open({ kind: 'interjection', graph }, { kind: 'cell', graph, cell: p.cell });
        break;
      case 'action':
        setPicked({ graph, node: p.node });
        open({ kind: 'action', graph, node: p.node });
        break;
      case 'out':
        pickCell(graph, p.to);
        open({ kind: 'cell', graph, cell: p.to }, { kind: 'cell', graph, cell: p.cell });
        into(p.to, false, graph);
        break;
      case 'in':
        pickCell(graph, p.from);
        open({ kind: 'cell', graph, cell: p.from }, { kind: 'cell', graph, cell: p.cell });
        into(p.from, false, graph);
        break;
    }
  };
  const titleOf = (s: Subject): string => {
    switch (s.kind) {
      case 'cell': {
        const c = graphFor(s.graph)?.g.cells.find((x) => x.id === s.cell);
        return c ? `${c.id} ${c.title}` : s.cell;
      }
      case 'rule':
        return `rule:${s.rule}`;
      case 'interjection':
        return graphFor(s.graph)?.g.interject?.name ?? '';
      case 'action':
        return (
          charts
            .get(graphFor(s.graph)?.g.name ?? '')
            ?.nodes.find((n) => n.id === s.node)
            ?.actions.map(actionWords)
            .join(' · ') ?? ''
        );
      case 'store':
        return compiled.engine.stores?.[s.store]?.title ?? s.store;
      case 'changes':
        return t('panel_changes');
      case 'round':
        return nameOf(s.round);
    }
  };
  const several = graphs.length > 1;
  const panel = (() => {
    if (now === null) return null;
    switch (now.kind) {
      case 'cell': {
        const x = graphFor(now.graph);
        const c = x?.g.cells.find((y) => y.id === now.cell);
        if (!x || !c) return null;
        const where = cellWhere(x.g, c);
        return {
          flow: flowFor(x.g.name),
          title: (
            <>
              <strong>{c.id}</strong> {c.title}
              {where !== null ? <span className="panel-where"> ({where})</span> : null}
            </>
          ),
          who: null,
          body: (
            <CellPanel
              c={c}
              rows={now.rows}
              work={work}
              rounds={rounds?.filter((r) => r.cell === c.id)}
              onRound={onRound ? (r) => open({ kind: 'round', round: r.n }) : undefined}
              diff={diff?.graphs.find((d) => d.name === x.g.name)?.cells.find((d) => d.id === c.id) ?? null}
              tab={cellTab}
              onTab={setCellTab}
            />
          ),
        };
      }
      case 'rule': {
        const x = graphFor(now.graph);
        return {
          flow: flowFor(x?.g.name),
          title: `rule:${now.rule}`,
          who: x?.g.rules.find((r) => r.name === now.rule)?.desc,
          body: <RulePanel name={now.rule} />,
        };
      }
      case 'interjection': {
        const x = graphFor(now.graph);
        return {
          flow: flowFor(x?.g.name),
          title: x?.g.interject?.name ?? '',
          who: x?.g.interject?.desc,
          body: <InterjectionPanel />,
        };
      }
      case 'action': {
        const x = graphFor(now.graph);
        const model = charts.get(x?.g.name ?? '');
        return {
          flow: flowFor(x?.g.name),
          title: titleOf(now),
          who: t('panel_actions_who'),
          body: model !== undefined ? <ActionPanel chart={model} node={now.node} /> : null,
        };
      }
      case 'store': {
        const s = compiled.engine.stores?.[now.store];
        return {
          flow: flowFor(undefined),
          title: titleOf(now),
          who: s !== undefined ? storeWho(s.layer, s.computed === true) : null,
          body: <StorePanel store={now.store} graphs={graphs} />,
        };
      }
      case 'changes':
        return {
          flow: flowFor(undefined),
          title: t('panel_changes'),
          who: null,
          body: diff
            ? graphs.map((x) => (
                <FlowContext.Provider key={x.g.name} value={flowFor(x.g.name)}>
                  {several ? <h3 className="diff-graph">{t('chart_graph', { graph: x.g.name })}</h3> : null}
                  <DiffPanel diff={diff} graph={x.g.name} />
                </FlowContext.Provider>
              ))
            : null,
        };
      case 'round': {
        // The round and its cell, which opens on the chart.
        const at = rounds?.find((r) => r.n === now.round)?.cell ?? null;
        const c = at !== null ? first?.g.cells.find((x) => x.id === at) : undefined;
        return {
          flow: flowFor(undefined),
          title: (
            <>
              <strong>{nameOf(now.round)}</strong>
              {c !== undefined ? (
                <>
                  {' · '}
                  <button
                    type="button"
                    className="link panel-cell"
                    title={t('rp_open_cell')}
                    onClick={() => open({ kind: 'cell', cell: c.id })}
                  >
                    <strong>{c.id}</strong> {c.title}
                  </button>
                </>
              ) : null}
            </>
          ),
          who: null,
          body: renderRound?.(now.round) ?? null,
        };
      }
    }
  })();
  // A round's panel brings its cell into view.
  useEffect(() => {
    if (now?.kind !== 'round' || !first) return;
    const at = rounds?.find((r) => r.n === now.round)?.cell;
    if (at) {
      setPicked({ graph: first.g.name, node: `c:${at}` });
      chartRef.current?.focus(at, false, first.g.name);
    }
  }, [now, rounds, first]);
  const shown = panel !== null && now !== null;
  const opened =
    panel !== null && now !== null ? (
      <Panel
        ref={panelRef}
        title={panel.title}
        who={panel.who}
        back={trail.last !== null ? titleOf(trail.last) : null}
        onBack={() => {
          const prev = trail.last;
          trail.back();
          if (prev?.kind === 'cell') {
            pickCell(prev.graph, prev.cell);
            chartRef.current?.focus(prev.cell, true, prev.graph);
          }
        }}
        onClose={trail.close}
        list={list}
        shown={subjectKey(now)}
      >
        <FlowContext.Provider value={panel.flow}>{panel.body}</FlowContext.Provider>
      </Panel>
    ) : null;
  const chartMarks = useMemo(
    () =>
      graphs.map((x, i) => ({
        ...(marks[i] ?? { overlays: null, taken: null, added: new Set<string>(), changed: new Set<string>() }),
        selected: now !== null && picked !== null && 'graph' in picked && picked.graph === x.g.name ? picked.node : null,
      })),
    [graphs, marks, now, picked],
  );
  const openStore = now !== null && picked !== null && 'store' in picked ? picked.store : null;
  return (
    <FlowContext.Provider value={flowFor(undefined)}>
      <div className="flow-screen">
        <div className="flow-chart-area">
          <FlowChart
            ref={chartRef}
            graphs={graphs}
            marks={chartMarks}
            engine={compiled.engine}
            store={openStore}
            start={start}
            startGraph={startGraph}
            onPick={onPick}
            legend={legend}
          />
        </div>
        {side !== undefined || shown ? (
          <aside className={side !== undefined ? 'dock dock-half' : 'dock'} style={width !== null ? { width } : undefined}>
            <Grip edge="left" min={DOCK_MIN} storage={DOCK_KEY} label={t('dock_grip')} onSize={setWidth} />
            {typeof side === 'function' ? (
              <div className="dock-top">{side(opened)}</div>
            ) : (
              <>
                {side !== undefined ? (
                  <div
                    className={shown ? 'dock-top dock-top-split' : 'dock-top'}
                    style={shown ? { height: split !== null ? split : '50%' } : undefined}
                  >
                    {side}
                    {shown ? (
                      <Grip
                        edge="bottom"
                        min={SPLIT_MIN}
                        storage={SPLIT_KEY}
                        label={t('dock_split')}
                        onSize={setSplit}
                      />
                    ) : null}
                  </div>
                ) : null}
                {opened}
              </>
            )}
          </aside>
        ) : null}
      </div>
    </FlowContext.Provider>
  );
}
