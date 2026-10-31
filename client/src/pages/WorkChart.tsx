// What a work's page and an architect's page share, each drawn once here so that a change to one is a change to both:
// the chart of the work shown, with what it did marked on it and the halo going round the cell a round runs at; the
// window floating over its right side, the page's panel in tabs over what is open on the chart; and the panel's rounds
// tab, with only the rounds sent back or failed when the page was opened on them, and the marks that open it so in a
// list.

import { ReplyIcon, XCircleIcon, XIcon } from '@primer/octicons-react';
import { CounterLabel, IconButton, UnderlineNav } from '@primer/react';
import { type ReactNode, useEffect, useMemo, useRef, useState } from 'react';

import type { Compiled, CompiledGraph, FlowView, GraphView, RoundSummary, Status, WorkDetail, WorkSummary } from '../api';
import { type Course, course, OutcomeBadge, StateChip } from '../components';
import { waitOf } from '../course';
import { useDockHead } from '../flow/Dock';
import { FlowScreen } from '../flow/FlowScreen';
import { cost, duration, roundName } from '../format';
import { chart, type Flash, haloAt, overlays, roundArrows, taken } from '../graph/flowModel';
import { useActivity, useStatus } from '../live';
import { spent } from '../round';
import { LinkButton } from '../parts';
import { Link, type RoundsOnly } from '../route';
import { t, tn } from '../text';
import { words } from '../labelled';
import { joinedWords } from '../voices';
import { RoundView } from './RoundView';

/// Who did a round's work: an agent, by its line; the engine, also for a gate it answered itself; the tools; the
/// owner.
function who(r: RoundSummary, g: CompiledGraph): string {
  if (r.line !== null && r.not_asked === null) return r.line;
  const kind = r.cell !== null ? g.cells.find((c) => c.id === r.cell)?.kind : undefined;
  return kind === 'tool' ? t('round_by_tools') : kind === 'owner' ? t('round_by_owner') : t('round_by_engine');
}

/// The rounds the list shows: all of them, or only those whose return was sent back, or only those whose run failed or
/// was cut off, as the list of works counts them.
export function roundsShown(rounds: RoundSummary[], only: RoundsOnly | null): RoundSummary[] {
  if (only === 'sent_back') return rounds.filter((r) => r.outcome === 'sent_back');
  if (only === 'failed') return rounds.filter((r) => r.outcome === 'failed' || r.outcome === 'interrupted');
  return rounds;
}

/// The marks after a work's count of rounds in a list of works: how many returns were sent back and how many runs
/// failed or were cut off, each saying so on hovering and opening the work with only those rounds listed.
export function RoundsMarks({ w, to }: { w: WorkSummary; to: (only: RoundsOnly) => string }) {
  return (
    <>
      {w.sent_back > 0 ? (
        <>
          {' '}
          <Link to={to('sent_back')} className="rounds-mark rounds-mark-back" title={t('rounds_sent_back_title', { n: w.sent_back })}>
            <ReplyIcon aria-label={t('outcome_sent_back')} />
            {w.sent_back}
          </Link>
        </>
      ) : null}
      {w.failed > 0 ? (
        <>
          {' '}
          <Link to={to('failed')} className="rounds-mark rounds-mark-failed" title={t('rounds_failed_title', { n: w.failed })}>
            <XCircleIcon aria-label={t('outcome_failed')} />
            {w.failed}
          </Link>
        </>
      ) : null}
    </>
  );
}

/// Where an agent's round spent its cost, beside its cost in its row: a small bar of the shares read again from the
/// cache, newly cached and output, and its steps; the whole of it on hovering. None for a round no agent ran.
function SpentBar({ r }: { r: RoundSummary }) {
  if (r.tokens === null || r.line === null || r.not_asked !== null) return null;
  const s = spent(r.tokens);
  const sum = s.shares !== null ? s.shares.read + s.shares.written + s.shares.output : 0;
  const width = (x: number) => `${sum > 0 ? (100 * x) / sum : 0}%`;
  return (
    <span className="spent" title={joinedWords(s.parts.map((p) => p.whole))}>
      {s.shares !== null ? (
        <span className="spent-bar">
          <span className="spent-read" style={{ width: width(s.shares.read) }} />
          <span className="spent-new" style={{ width: width(s.shares.written) }} />
          <span className="spent-output" style={{ width: width(s.shares.output) }} />
        </span>
      ) : null}
      <span className="spent-steps">{s.steps}</span>
    </span>
  );
}

/// The rounds, newest first: the cell and who did its work, the row it took (a badge only for what is not the usual
/// course: sent back, failed, cut off, running), whole and wrapped rather than cut, and for a round running, what its
/// agent did last; what an agent's round cost, where its cost went (its key is in the chart's "?") and how long it
/// took. The round chosen, however it was chosen, is scrolled into view.
export function Timeline(props: {
  work: number;
  rounds: RoundSummary[];
  g: CompiledGraph;
  /// Whether a round still open is one left over, not one running: the engine holds and runs nothing of this work.
  stale: boolean;
  selected: number | null;
  onRound: (r: RoundSummary) => void;
}) {
  const body = useRef<HTMLTableSectionElement>(null);
  const { selected } = props;
  const activity = useActivity(props.work);
  useEffect(() => {
    if (selected !== null) body.current?.querySelector('tr.selected')?.scrollIntoView({ block: 'nearest' });
  }, [selected]);
  if (props.rounds.length === 0) return <p className="muted">{t('no_rounds')}</p>;
  const newest = [...props.rounds].reverse();
  return (
    <table className="list timeline">
      <tbody ref={body}>
        {newest.map((r) => {
          // A row the engine took as a session would start afresh says why after its name.
          const took = r.transition !== null && r.why !== null ? `${r.transition}: ${r.why}` : (r.transition ?? r.why ?? '');
          const running = r.outcome === 'running' && !props.stale;
          const last = running ? activity.filter((x) => x.round === r.n).pop() : undefined;
          const what = r.not_asked !== null ? t('round_not_asked', { need: words(r.not_asked) }) : (last?.what ?? took);
          return (
            <tr
              key={r.n}
              className={`clickable${r.n === props.selected ? ' selected' : ''}`}
              onClick={() => props.onRound(r)}
            >
              <td className="muted nowrap">{roundName(props.work, r.n)}</td>
              <td className="nowrap">
                <span className="cell-id">{r.cell ?? '–'}</span> <span className="muted">{who(r, props.g)}</span>
              </td>
              <td className="timeline-what">
                {r.outcome === 'running' && props.stale ? (
                  <>
                    <StateChip tone="neutral" title={t('round_left_open_hint')}>
                      {t('round_left_open')}
                    </StateChip>{' '}
                  </>
                ) : r.outcome !== 'took' ? (
                  <>
                    <OutcomeBadge outcome={r.outcome} />{' '}
                  </>
                ) : null}
                <span className={r.not_asked !== null || last !== undefined ? 'muted' : undefined}>{what}</span>
              </td>
              <td className="num muted nowrap">
                <SpentBar r={r} />
                {r.cost_usd !== null && r.line !== null ? cost(r.cost_usd) : ''}
              </td>
              <td className="num muted nowrap">{r.ended_at !== null ? duration(r.ended_at - r.started_at) : ''}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

/// The rounds tab of a page's panel, a work's or an architect's: when the page was opened on only the rounds sent back
/// or only those that failed, that it shows only those and the way to all of them; then the rounds.
export function RoundsTab({ d, g, only, selected, onAll, onRound }: {
  d: WorkDetail;
  g: CompiledGraph;
  only: RoundsOnly | null;
  selected: number | null;
  /// Shows every round again.
  onAll: () => void;
  onRound: (round: number) => void;
}) {
  const status = useStatus();
  const shown = roundsShown(d.rounds, only);
  return (
    <>
      {only !== null ? (
        <p className="rounds-only">
          {tn(only === 'sent_back' ? 'rounds_only_sent_back' : 'rounds_only_failed', shown.length)}{' '}
          <LinkButton onClick={onAll}>{t('rounds_show_all', { n: d.rounds.length })}</LinkButton>
        </p>
      ) : null}
      <Timeline
        work={d.work.id}
        rounds={shown}
        g={g}
        stale={status !== null && status.running !== d.work.id}
        selected={selected}
        onRound={(x) => onRound(x.n)}
      />
    </>
  );
}

/// One tab of a page's panel: what it shows, its address, how many things it holds, and how many of them wait for the
/// owner, with what those are on hovering.
export type PanelTab<T extends string> = {
  tab: T;
  label: string;
  href: string;
  n: number | null;
  waiting?: number;
  hint?: string;
};

/// A page's panel, on top of the window floating over the right of its chart, or beside the one-to-one with an
/// architect: its tabs, each with its count and what in it waits for the owner as a count of its own, and at the row's
/// right end, while the row is the window's top row, the button that minimises and expands the window, a click on the
/// row beside the tabs doing the same, or, for a panel that closes (`onClose`), its close; and what the tab shown
/// holds, shown from its top when the tab changes. A tab picked while the window is minimised expands it to that tab.
/// A click with a modifier key opens a tab as the browser would.
export function ChartPanel<T extends string>({ label, tabs, current, onTab, onClose, children }: {
  label: string;
  tabs: Array<PanelTab<T>>;
  current: T;
  onTab: (tab: T) => void;
  onClose?: () => void;
  children: ReactNode;
}) {
  const body = useRef<HTMLDivElement>(null);
  const dock = useDockHead();
  useEffect(() => {
    body.current?.scrollTo({ top: 0 });
  }, [current]);
  return (
    <div className="drawer">
      <div className={dock?.fold ? 'drawer-head drawer-head-folds' : 'drawer-head'} onClick={dock?.fold?.onRow}>
        <UnderlineNav aria-label={label} className="drawer-tabs">
          {tabs.map((x) => (
            <UnderlineNav.Item
              key={x.tab}
              href={x.href}
              aria-current={x.tab === current ? 'page' : undefined}
              onSelect={(e) => {
                if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
                e.preventDefault();
                onTab(x.tab);
                dock?.expand();
              }}
            >
              {x.label}
              {x.n !== null ? <CounterLabel className="drawer-count">{x.n}</CounterLabel> : null}
              {(x.waiting ?? 0) > 0 ? (
                <CounterLabel variant="primary" className="drawer-count" title={x.hint}>
                  {x.waiting}
                </CounterLabel>
              ) : null}
            </UnderlineNav.Item>
          ))}
        </UnderlineNav>
        {dock?.fold ? <div className="drawer-fold">{dock.fold.button}</div> : null}
        {onClose !== undefined ? (
          <div className="drawer-fold">
            <IconButton size="small" variant="invisible" icon={XIcon} aria-label={t('close')} onClick={onClose} />
          </div>
        ) : null}
      </div>
      <div className="drawer-body" ref={body}>
        {children}
      </div>
    </div>
  );
}

/// Whether a round of a work runs now: the engine takes its step, or, at a tool cell, its tool is under way.
function runsNow(w: WorkSummary, c: Course): boolean {
  return c.running !== null || w.running?.tool === true;
}

/// What a work's chart marks: what it did at each cell (the cell it is at, whether a round runs there and why the work
/// waits there while none does), the cell the halo goes round, how many times it took each arrow, and the arrows the
/// round chosen came in by and left by.
export function workMarks(
  g: CompiledGraph,
  v: GraphView,
  w: WorkSummary,
  rounds: RoundSummary[],
  c: Course,
  status: Status | null,
  halo: string | null,
  round: number | null = null,
  flow: Compiled | null = null,
) {
  const model = chart(g, v, flow?.engine ?? null);
  return {
    overlays: overlays(rounds, w.at, { running: runsNow(w, c), waits: waitOf(c, w, status) }),
    halo,
    taken: taken(model, g, rounds),
    lit: round !== null ? roundArrows(model, g, rounds, round) : undefined,
    added: new Set<string>(),
    changed: new Set<string>(),
  };
}

/// The rounds kept for the halo to go round, the newest: more than ever come in between two reads.
const FLASHES_KEPT = 32;

/// The cell the halo goes round on a work's chart (`haloAt`): each round new since the page last read the work's
/// rounds counts as just run, so that a round of a tool or an engine step, over in milliseconds, is seen; the rounds
/// there at the first read do not.
export function useHalo(w: WorkSummary | null, rounds: RoundSummary[], c: Course | null): string | null {
  const [flashes, setFlashes] = useState<Flash[]>([]);
  const known = useRef<{ work: number; rounds: Set<number> } | null>(null);
  const [, setTick] = useState(0);
  const id = w?.id ?? null;
  useEffect(() => {
    if (id === null) return;
    const k = known.current;
    if (k === null || k.work !== id) {
      known.current = { work: id, rounds: new Set(rounds.map((r) => r.n)) };
      setFlashes([]);
      return;
    }
    const seen = Date.now();
    const fresh = rounds.filter((r) => !k.rounds.has(r.n)).sort((a, b) => a.n - b.n);
    for (const r of fresh) k.rounds.add(r.n);
    const more = fresh.flatMap((r) => (r.cell !== null ? [{ n: r.n, cell: r.cell, seen }] : []));
    if (more.length > 0) setFlashes((f) => [...f, ...more].slice(-FLASHES_KEPT));
  }, [id, rounds]);
  const newest = rounds.reduce<number | null>((m, r) => (m === null || r.n > m ? r.n : m), null);
  const { cell, next } = haloAt(flashes, Date.now(), {
    at: w?.at ?? null,
    runs: w !== null && c !== null && runsNow(w, c),
    newest,
  });
  // Moves on when the halo is due to.
  useEffect(() => {
    if (next === null) return;
    const timer = setTimeout(() => setTick((x) => x + 1), Math.max(0, next - Date.now()) + 20);
    return () => clearTimeout(timer);
  }, [next]);
  return cell;
}

/// What the bare graph marks: nothing.
const BARE = { overlays: null, taken: null, added: new Set<string>(), changed: new Set<string>() };

/// No rounds, for the halo while no work is shown.
const NO_ROUNDS: RoundSummary[] = [];

/// The chart of a work's page or an architect's: the graph of the work shown filling the window, as its flow version has
/// it, with what the work did placed on it, opening on the cell of the round chosen or the cell the work is at; with no
/// work, the bare graph named; and the window floating over its right side, the page's panel on top and what is open on
/// the chart under it, a round of the work in the round's own panel.
export function WorkChart({ compiled, view, graph, d, cell, round, onCell, onRound, panel }: {
  compiled: Compiled;
  view: FlowView;
  /// The graph drawn while no work is shown.
  graph: string;
  /// The work shown, in full; none for the bare graph.
  d: WorkDetail | null;
  /// The cell open, and the round of the work shown that is open, by its number in the work.
  cell: string | null;
  round: number | null;
  onCell: (cell: string | null) => void;
  onRound: (round: number | null) => void;
  /// The page's panel, given the graph drawn.
  panel: (g: CompiledGraph) => ReactNode;
}) {
  const status = useStatus();
  const name = d?.work.graph ?? graph;
  const g = compiled.graphs.find((x) => x.name === name);
  const v = view.graphs.find((x) => x.name === name);
  const graphs = useMemo(() => (g && v ? [{ g, view: v }] : []), [g, v]);
  const c = d !== null ? course(d.work, status) : null;
  const halo = useHalo(d?.work ?? null, d?.rounds ?? NO_ROUNDS, c);
  if (!g || !v) return <p className="muted page">{t('chart_no_graph', { graph: name })}</p>;
  const id = d?.work.id ?? null;
  const roundCell = round !== null && d !== null ? (d.rounds.find((x) => x.n === round)?.cell ?? null) : null;
  return (
    <FlowScreen
      key={id ?? 'none'}
      compiled={compiled}
      graphs={graphs}
      legend={d !== null ? 'work' : 'flow'}
      marks={[d !== null && c !== null ? workMarks(g, v, d.work, d.rounds, c, status, halo, round, compiled) : BARE]}
      cell={cell}
      onCell={(x) => onCell(x)}
      start={roundCell ?? d?.work.at ?? null}
      work={id ?? undefined}
      rounds={d?.rounds}
      round={round}
      onRound={onRound}
      renderRound={d !== null ? (x) => <RoundView work={d.work.id} n={x} rounds={d.rounds} /> : undefined}
      side={panel(g)}
    />
  );
}
