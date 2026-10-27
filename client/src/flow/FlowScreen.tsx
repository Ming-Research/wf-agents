// A flow's chart filling the window under the bar, and a column docked at its right, whose width drags at its left
// edge: on a work's page the work's panel on top, at the column's full height, and while a cell, a rule, the
// interjection, a comparison or a round is open, its panel in the lower part, split from the top by a divider that
// drags; on the flow page that panel alone, while something is open. The flow page and a work's page both show a flow
// this way.

import { type ReactNode, useEffect, useMemo, useRef, useState } from 'react';

import type { Compiled, CompiledGraph, FlowDiff, GraphView, RoundSummary } from '../api';
import { FlowChart, type FlowChartHandle, type Pick } from '../graph/FlowChart';
import { roundName } from '../format';
import type { Marks } from '../graph/flowDraw';
import { t } from '../text';
import { CellPanel, type CellTab, cellWho } from './CellPanel';
import { DiffPanel } from './Changes';
import { flowOf, FlowContext, type Subject } from './context';
import { Grip, Panel, savedSize, useTrail } from './Panel';
import { CellLinks, inline } from './words';

const same = (a: Subject, b: Subject) => JSON.stringify(a) === JSON.stringify(b);

export type FlowScreenProps = {
  compiled: Compiled;
  g: CompiledGraph;
  view: GraphView;
  legend: 'flow' | 'work' | 'compare';
  marks: Omit<Marks, 'selected'>;
  /// The cell the address names: its panel is open.
  cell: string | null;
  /// Called when the panel opens a cell, or closes; the page keeps it in its address.
  onCell: (cell: string | null) => void;
  /// The cell the chart opens on.
  start: string | null;
  /// When two flows are compared: what changed, which the panel opens on.
  diff?: FlowDiff | null;
  /// On a work's page: the work, its rounds, a round's panel, and the round the address names, each round by its
  /// number in the work.
  work?: number;
  rounds?: RoundSummary[];
  round?: number | null;
  onRound?: (round: number | null) => void;
  renderRound?: (round: number) => ReactNode;
  /// On a work's page: the work's panel, on top of the column.
  side?: ReactNode;
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
  const { compiled, g, view, legend, marks, cell, onCell, start, diff, work, rounds, round, onRound, renderRound, side } =
    props;
  const [width, setWidth] = useState<number | null>(() => savedSize(DOCK_KEY, DOCK_MIN));
  const [split, setSplit] = useState<number | null>(() => savedSize(SPLIT_KEY, SPLIT_MIN));
  const nameOf = (n: number) => roundName(work ?? 0, n);
  const chartRef = useRef<FlowChartHandle>(null);
  const panelRef = useRef<HTMLElement>(null);
  const trail = useTrail<Subject>(same);
  const [picked, setPicked] = useState<string | null>(null);
  // The tab of a cell's panel the owner chose last: going to another cell keeps it.
  const [cellTab, setCellTab] = useState<CellTab | null>(null);
  const flow = useMemo(
    () =>
      flowOf(compiled, g, view, (s) => {
        trail.open(s);
        if (s.kind === 'cell') {
          setPicked(`c:${s.cell}`);
          window.setTimeout(() => chartRef.current?.focus(s.cell), 0);
        }
      }),
    // `trail.open` keeps no state of its own between renders.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [compiled, g, view],
  );
  // The address and the panel follow each other: a cell or round named in the address is opened, and opening one
  // writes it there.
  const now = trail.now;
  useEffect(() => {
    if (round != null) {
      if (now?.kind !== 'round' || now.round !== round) trail.open({ kind: 'round', round });
    } else if (cell !== null) {
      if (now?.kind !== 'cell' || now.cell !== cell) {
        trail.open({ kind: 'cell', cell });
        setPicked(`c:${cell}`);
      }
    }
    // Only when the address changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cell, round]);
  useEffect(() => {
    if (now === null) {
      if (cell !== null) onCell(null);
      if (round != null) onRound?.(null);
    } else if (now.kind === 'cell' && now.cell !== cell) {
      onCell(now.cell);
    } else if (now.kind === 'round' && now.round !== round) {
      onRound?.(now.round);
    }
    // Only when the panel changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [now]);
  // A comparison opens on what changed.
  useEffect(() => {
    if (diff && cell === null && round == null) trail.open({ kind: 'changes' });
    // Only when the comparison changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [diff]);
  // Brings a cell into view once the panel that opens with it is drawn, as the chart narrows to make room for it.
  const into = (cell: string, keep: boolean) => window.setTimeout(() => chartRef.current?.focus(cell, keep), 0);
  const onPick = (p: Pick) => {
    switch (p.kind) {
      case 'cell':
        setPicked(`c:${p.cell}`);
        trail.open({ kind: 'cell', cell: p.cell });
        into(p.cell, true);
        break;
      case 'rule':
        setPicked(`r:${p.rule}:${p.cell}`);
        trail.open({ kind: 'rule', rule: p.rule }, { kind: 'cell', cell: p.cell });
        break;
      case 'interjection':
        setPicked(`j:${p.cell}`);
        trail.open({ kind: 'interjection' }, { kind: 'cell', cell: p.cell });
        break;
      case 'shared':
        setPicked(`c:${p.cell}`);
        setCellTab('route');
        trail.open({ kind: 'cell', cell: p.cell, rows: g.cells.find((c) => c.id === p.cell)?.rows.flatMap((r, i) => (r.common ? [i] : [])) });
        break;
      case 'out':
        setPicked(`c:${p.to}`);
        trail.open({ kind: 'cell', cell: p.to }, { kind: 'cell', cell: p.cell });
        into(p.to, false);
        break;
      case 'in':
        setPicked(`c:${p.from}`);
        trail.open({ kind: 'cell', cell: p.from }, { kind: 'cell', cell: p.cell });
        into(p.from, false);
        break;
    }
  };
  const titleOf = (s: Subject): string => {
    switch (s.kind) {
      case 'cell': {
        const c = g.cells.find((x) => x.id === s.cell);
        return c ? `${c.id} ${c.title}` : s.cell;
      }
      case 'rule':
        return `rule:${s.rule}`;
      case 'interjection':
        return g.interject?.name ?? '';
      case 'changes':
        return t('panel_changes');
      case 'round':
        return nameOf(s.round);
    }
  };
  const panel = (() => {
    if (now === null) return null;
    switch (now.kind) {
      case 'cell': {
        const c = g.cells.find((x) => x.id === now.cell);
        if (!c) return null;
        return {
          title: (
            <>
              <strong>{c.id}</strong> {c.title}
            </>
          ),
          who: cellWho(view, c),
          body: (
            <CellPanel
              c={c}
              rows={now.rows}
              work={work}
              rounds={rounds?.filter((r) => r.cell === c.id)}
              onRound={onRound ? (r) => trail.open({ kind: 'round', round: r.n }) : undefined}
              diff={diff?.graphs.find((x) => x.name === g.name)?.cells.find((x) => x.id === c.id) ?? null}
              tab={cellTab}
              onTab={setCellTab}
            />
          ),
        };
      }
      case 'rule':
        return {
          title: `rule:${now.rule}`,
          who: g.rules.find((r) => r.name === now.rule)?.desc,
          body: <RulePanel name={now.rule} />,
        };
      case 'interjection':
        return { title: g.interject?.name ?? '', who: g.interject?.desc, body: <InterjectionPanel /> };
      case 'changes':
        return { title: t('panel_changes'), who: null, body: diff ? <DiffPanel diff={diff} graph={g.name} /> : null };
      case 'round':
        return { title: nameOf(now.round), who: null, body: renderRound?.(now.round) ?? null };
    }
  })();
  // A round's panel brings its cell into view.
  useEffect(() => {
    if (now?.kind !== 'round') return;
    const at = rounds?.find((r) => r.n === now.round)?.cell;
    if (at) {
      setPicked(`c:${at}`);
      chartRef.current?.focus(at);
    }
  }, [now, rounds]);
  const open = panel !== null && now !== null;
  return (
    <FlowContext.Provider value={flow}>
      <div className="flow-screen">
        <div className="flow-chart-area">
          <FlowChart
            ref={chartRef}
            g={g}
            view={view}
            marks={{ ...marks, selected: now === null ? null : picked }}
            start={start}
            onPick={onPick}
            legend={legend}
          />
        </div>
        {side !== undefined || open ? (
          <aside className="dock" style={width !== null ? { width } : undefined}>
            <Grip edge="left" min={DOCK_MIN} storage={DOCK_KEY} label={t('dock_grip')} onSize={setWidth} />
            {side !== undefined ? (
              <div
                className={open ? 'dock-top dock-top-split' : 'dock-top'}
                style={open ? { height: split !== null ? split : '50%' } : undefined}
              >
                {side}
                {open ? (
                  <Grip edge="bottom" min={SPLIT_MIN} storage={SPLIT_KEY} label={t('dock_split')} onSize={setSplit} />
                ) : null}
              </div>
            ) : null}
            {panel !== null && now !== null ? (
              <Panel
                ref={panelRef}
                title={panel.title}
                who={panel.who}
                back={trail.last !== null ? titleOf(trail.last) : null}
                onBack={() => {
                  const prev = trail.last;
                  trail.back();
                  if (prev?.kind === 'cell') {
                    setPicked(`c:${prev.cell}`);
                    chartRef.current?.focus(prev.cell, true);
                  }
                }}
                onClose={trail.close}
                shown={JSON.stringify(now)}
              >
                {panel.body}
              </Panel>
            ) : null}
          </aside>
        ) : null}
      </div>
    </FlowContext.Provider>
  );
}
