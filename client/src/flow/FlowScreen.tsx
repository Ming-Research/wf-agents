// A flow's chart filling the window under the bar, its graphs side by side, and a window floating over its right side
// (Dock.tsx), whose width drags at its left edge, half the chart's width at first on a work's page and the architect's,
// and which never moves the chart. On those two pages the page's panel is on top, at the window's full height until a
// cell, a rule, the interjection, a comparison or a round is opened, whose panel is then in the lower part, split from
// the top by a divider that drags, at the share the owner last set on either page. On the flow page, which has no
// panel of its own, what is open is alone in the window, which shows from the first thing opened. What is open has no
// close: it stays until something else is opened or the way back is taken. The window starts minimised; a click on
// the chart that opens something expands it. The lines of an open panel's From and To light their rows on the chart
// (lights.ts). Every page that shows a flow shows it here, in this one way, so that what is opened on one shows as on
// the others.

import { type ReactNode, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';

import type { Compiled, FlowDiff, RoundSummary } from '../api';
import { FlowChart, type FlowChartHandle, type Graph, type Pick } from '../graph/FlowChart';
import { roundName } from '../format';
import type { Marks } from '../graph/flowDraw';
import { actionWords, chart } from '../graph/flowModel';
import { RoundAt } from '../parts';
import { t } from '../text';
import { CellPanel, type CellTab, cellWhere } from './CellPanel';
import { DiffPanel } from './Changes';
import { ActionPanel, StorePanel, storeWho } from './EnginePanels';
import { type Flow, flowOf, FlowContext, type Subject, subjectKey } from './context';
import { coverOf, Dock, DockContext, type Fold, FoldButton, foldIn, foldOnClick, useMinimised } from './Dock';
import { LightsContext, useLights } from './lights';
import { Grip, Panel, savedSize, useTrail } from './Panel';
import { OPEN_FIRST, type Side, toggled } from './Sides';
import { CellLinks, inline, sheetText } from './words';
import { charCount, chars, joinedWords, Name, Part, Sheet } from '../voices';

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

/// Whether a click on the chart expands the window floating over it, when it is minimised: a click that opens what it
/// is on does, so that it is seen; a click on the chart's background, which opens nothing, does not.
export function pickExpands(p: Pick): boolean {
  return p.kind !== 'nothing';
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
  /// Called when the panel opens a cell, with its graph; the page keeps it in its address.
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
  /// On a work's page and the architect's: the page's panel, on top of the window, with what is open on the chart
  /// split under it.
  side?: ReactNode;
};

/// The height of the page's panel over what is open, one for every page, so that a work's page and the architect's
/// split alike.
const SPLIT_KEY = 'wf-agents.dock-split';
const SPLIT_MIN = 120;

/// A rule of the flow: its text, as the cells given it are sent it, and the cells it is given to.
function RulePanel({ name }: { name: string }) {
  return (
    <FlowContext.Consumer>
      {(f) => {
        const rule = f?.g.rules.find((r) => r.name === name);
        const at = f?.g.cells.filter((c) => (c.rules ?? []).some((r) => r.name === name)).map((c) => c.id) ?? [];
        const text = (rule?.text ?? []).join('\n');
        return (
          <div className="pp-page">
            <Part title={t('sec_rule_text')} code={`rule:${name}`} facts={charCount(chars(text))}>
              <Sheet>
                {(rule?.text ?? []).map((l, i) => (
                  <p key={i}>{sheetText(l)}</p>
                ))}
              </Sheet>
            </Part>
            <Part title={t('sec_rule_at')}>
              <p className="pp-note">{inline(t('rule_at_note', { address: `\`rule:${name}\`` }))}</p>
              <CellLinks ids={at} />
            </Part>
          </div>
        );
      }}
    </FlowContext.Consumer>
  );
}

/// The interjection: who may interject, and what the agents are told of it, as they are sent it; and the steps that
/// take one.
function InterjectionPanel() {
  return (
    <FlowContext.Consumer>
      {(f) => {
        const ij = f?.g.interject;
        if (!f || !ij) return null;
        const steps = f.g.cells.filter((c) => c.kind !== 'agent' && c.rows.some((r) => r.interject)).map((c) => c.id);
        return (
          <div className="pp-page">
            <Part title={t('sec_ij_from')}>
              <Sheet>
                {ij.from.map((x) => (
                  <p key={x.name}>
                    <Name>{x.name}</Name>: {sheetText(x.desc)}
                  </p>
                ))}
              </Sheet>
            </Part>
            <Part title={t('sec_ij_text')}>
              <Sheet>
                {ij.text.map((l, i) => (
                  <p key={i}>{sheetText(l)}</p>
                ))}
              </Sheet>
            </Part>
            <Part title={t('sec_ij_at')}>
              <p className="pp-note">{inline(t('ij_agents'))}</p>
              <CellLinks ids={steps} />
            </Part>
          </div>
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
  } = props;
  const [split, setSplit] = useState<number | null>(() => savedSize(SPLIT_KEY, SPLIT_MIN));
  const nameOf = (n: number) => roundName(work ?? 0, n);
  const chartRef = useRef<FlowChartHandle>(null);
  const panelRef = useRef<HTMLElement>(null);
  const areaRef = useRef<HTMLDivElement>(null);
  const dockRef = useRef<HTMLElement>(null);
  const { minimised, minimise, expand } = useMinimised(dockRef);
  // What the window covers of the chart, read as they are drawn each time the chart centres on something.
  const cover = useCallback(() => {
    const dock = dockRef.current;
    const area = areaRef.current?.getBoundingClientRect() ?? null;
    return coverOf(area, dock !== null && !dock.classList.contains('dock-min') ? dock.getBoundingClientRect() : null);
  }, []);
  // A round the address names is open from the page's first drawing.
  const trail = useTrail<Subject>(same, round != null ? { kind: 'round', round } : null);
  // The node whose panel is open, in its graph, or the store.
  const [picked, setPicked] = useState<{ graph: string; node: string } | { store: string } | null>(null);
  // The tab of a cell's panel the owner chose last: going to another cell keeps it.
  const [cellTab, setCellTab] = useState<CellTab | null>(null);
  // The sections of a cell's page the owner opened: going to another cell keeps them.
  const [cellOpen, setCellOpen] = useState<ReadonlySet<Side>>(OPEN_FIRST);
  const onCellOpen = (x: Side, o: boolean) => setCellOpen((was) => toggled(was, x, o));
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
  // Brings a cell into view once the window that opens with it is drawn, in the part of the chart the window leaves.
  const into = (c: string, keep: boolean, graph: string) =>
    window.setTimeout(() => chartRef.current?.focus(c, keep, graph), 0);
  const onPick = (p: Pick) => {
    if (pickExpands(p)) expand();
    // A click on nothing leaves what is open as it is, since it has no close.
    if (p.kind === 'nothing') return;
    // A store is every graph's.
    if (p.kind === 'store') {
      setPicked({ store: p.store });
      open({ kind: 'store', store: p.store });
      // A store beside a cell leads to its box among the stores, as a circle leads to the cell it names.
      if (p.go === true) window.setTimeout(() => chartRef.current?.focusStore(p.store), 0);
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
        return joinedWords(
          charts
            .get(graphFor(s.graph)?.g.name ?? '')
            ?.nodes.find((n) => n.id === s.node)
            ?.actions.map(actionWords) ?? [],
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
              <span className="cell-id">{c.id}</span> {c.title}
              {where !== null ? <span className="panel-where"> ({where})</span> : null}
            </>
          ),
          who: null,
          body: (
            <CellPanel
              c={c}
              work={work}
              rounds={rounds?.filter((r) => r.cell === c.id)}
              onRound={onRound ? (r) => open({ kind: 'round', round: r.n }) : undefined}
              diff={diff?.graphs.find((d) => d.name === x.g.name)?.cells.find((d) => d.id === c.id) ?? null}
              tab={cellTab}
              onTab={setCellTab}
              open={cellOpen}
              onOpen={onCellOpen}
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
                  {several ? <h3 className="sub-heading diff-graph">{t('chart_graph', { graph: x.g.name })}</h3> : null}
                  <DiffPanel diff={diff} graph={x.g.name} />
                </FlowContext.Provider>
              ))
            : null,
        };
      case 'round': {
        // The round at its cell, which opens on the chart.
        const at = rounds?.find((r) => r.n === now.round)?.cell ?? null;
        const c = at !== null ? first?.g.cells.find((x) => x.id === at) : undefined;
        return {
          flow: flowFor(undefined),
          title: (
            <RoundAt
              round={nameOf(now.round)}
              cell={c?.id}
              title={c?.title}
              onCell={c !== undefined ? () => open({ kind: 'cell', cell: c.id }) : undefined}
              hint={t('rp_open_cell')}
            />
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
  // The rows the lines of the panel's From and To light on the chart, let go when it shows something else.
  const { lights, lit } = useLights(now !== null ? subjectKey(now) : null);
  const docked = side !== undefined || shown;
  // The window's top row, which it folds to, holds the button that minimises and expands it, and a click on the row
  // does the same.
  const head = foldIn(side !== undefined, shown, minimised);
  const toggle = minimised ? expand : minimise;
  const fold: Fold = { button: <FoldButton minimised={minimised} onFold={toggle} />, onRow: foldOnClick(toggle) };
  // The chart reads again what the window covers as it opens, minimises, expands or is dragged, and does not move.
  useLayoutEffect(() => {
    chartRef.current?.covered();
  }, [docked, minimised]);
  useEffect(() => {
    const el = dockRef.current;
    if (el === null) return;
    const resized = new ResizeObserver(() => chartRef.current?.covered());
    resized.observe(el);
    return () => resized.disconnect();
  }, [docked]);
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
        shown={subjectKey(now)}
        minimised={minimised}
        fold={head === 'open' ? fold : null}
      >
        <FlowContext.Provider value={panel.flow}>
          <LightsContext.Provider value={lights}>{panel.body}</LightsContext.Provider>
        </FlowContext.Provider>
      </Panel>
    ) : null;
  const chartMarks = useMemo(
    () =>
      graphs.map((x, i) => ({
        ...(marks[i] ?? { overlays: null, taken: null, added: new Set<string>(), changed: new Set<string>() }),
        selected: now !== null && picked !== null && 'graph' in picked && picked.graph === x.g.name ? picked.node : null,
        rows: lit.filter((l) => l.graph === x.g.name).flatMap((l) => l.rows),
      })),
    [graphs, marks, now, picked, lit],
  );
  const openStore = now !== null && picked !== null && 'store' in picked ? picked.store : null;
  return (
    <FlowContext.Provider value={flowFor(undefined)}>
      <div className="flow-screen">
        <div className="flow-chart-area" ref={areaRef}>
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
            cover={cover}
          />
        </div>
        {docked ? (
          <Dock ref={dockRef} half={side !== undefined} minimised={minimised}>
            {side !== undefined ? (
              <div
                className={shown ? 'dock-top dock-top-split' : 'dock-top'}
                style={shown ? { height: split !== null ? split : '50%' } : undefined}
              >
                <DockContext.Provider value={{ fold: head === 'tabs' ? fold : null, expand }}>{side}</DockContext.Provider>
                {shown ? (
                  <Grip edge="bottom" min={SPLIT_MIN} storage={SPLIT_KEY} label={t('dock_split')} onSize={setSplit} />
                ) : null}
              </div>
            ) : null}
            {opened}
          </Dock>
        ) : null}
      </div>
    </FlowContext.Provider>
  );
}
