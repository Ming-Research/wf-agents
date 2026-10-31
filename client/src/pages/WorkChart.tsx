// What a work's page and an architect's page share, each drawn once here so that a change to one is a change to both:
// the chart of the work shown, with what the rounds listed did marked on it and the halo going round the cell a round
// runs at; the window floating over its right side, the page's panel in tabs over what is open on the chart; and the
// panel's rounds tab, one list of rounds for both pages, of one work or of the architect's works, scrolling to read more
// at its ends where more are left, with only the rounds sent back or failed when a work's page was opened on them, and
// the marks that open it so in a list.

import { ReplyIcon, XCircleIcon, XIcon } from '@primer/octicons-react';
import { CounterLabel, IconButton, Label, UnderlineNav } from '@primer/react';
import { type ReactNode, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';

import type { Compiled, CompiledGraph, FlowView, GraphView, RoundSummary, Status, WorkSummary } from '../api';
import { type Course, course, OutcomeBadge } from '../components';
import { SessionIcon, waitOf } from '../course';
import { useDockHead } from '../flow/Dock';
import { FlowScreen } from '../flow/FlowScreen';
import { cost, duration, roundName, time } from '../format';
import { chart, type Flash, haloAt, overlays, roundArrows, taken } from '../graph/flowModel';
import { useActivity, useStatus } from '../live';
import { badged, sessionMarkWords, spent, startedSession, tookWords } from '../round';
import { centredTop, endsNear, inSight } from '../rounds';
import { roleClass, useColours } from '../roles';
import { LinkButton } from '../parts';
import { Link, type RoundKey, type RoundsOnly, sameRound } from '../route';
import { t, tn } from '../text';
import { words } from '../labelled';
import { joinedWords } from '../voices';
import { RoundView } from './RoundView';

/// Who ran a round, in words and as the chart colours its cell: an agent by its line, in its role's colour; the
/// engine, also for a gate it answered itself, the tools and the owner each in the colour of their cells.
export function whoRan(r: RoundSummary, g: CompiledGraph): { words: string; kind: 'line' | 'engine' | 'tool' | 'owner' } {
  if (r.line !== null && r.not_asked === null) return { words: r.line, kind: 'line' };
  const kind = r.cell !== null ? g.cells.find((c) => c.id === r.cell)?.kind : undefined;
  if (kind === 'tool') return { words: t('round_by_tools'), kind: 'tool' };
  if (kind === 'owner') return { words: t('round_by_owner'), kind: 'owner' };
  return { words: t('round_by_engine'), kind: 'engine' };
}

/// Who ran a round as a chip filled in the tint of its cell's colour on the chart, its words in that colour, with no
/// edge of another colour.
function Who({ r, g }: { r: RoundSummary; g: CompiledGraph }) {
  const colours = useColours();
  const who = whoRan(r, g);
  const look = who.kind === 'line' ? roleClass(colours, r.line) : `kind-${who.kind}`;
  return (
    <Label variant="secondary" className={`who-chip ${look}`}>
      {who.words}
    </Label>
  );
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

/// The mark of a round that started a new session of its line, after its name: the new session's speech bubble, saying
/// why on hovering and to a screen reader.
export function SessionMark({ r }: { r: RoundSummary }) {
  const why = sessionMarkWords(r);
  return (
    <span className="session-mark" title={why} role="img" aria-label={why}>
      <SessionIcon />
    </span>
  );
}

/// A round's row in the list of rounds, by its name, as its element names it.
const rowName = (r: RoundSummary) => roundName(r.work, r.n);

/// The list of rounds of a work's page and an architect's, one part for both, newest first: each round's name, with a
/// mark when it started a new session of its line; the cell and who ran it, in its cell's colour on the chart; what it
/// came to on one line, cut and whole on hovering (a chip only for what is not the usual course: sent back, failed, cut
/// off, running; the row it took, or that it ended the work, in plain words), and for a round running what its agent
/// did last; what an agent's round cost and where its cost went (its key is in the chart's "?"); and how long it took.
/// Before the first round of each of the architect's works, a rule with when that work began, between two works'
/// rounds. Scrolled near its top or its bottom, it asks for the newer or older rounds left unread (`onMore`); rounds
/// joining it above leave the rows in sight where they are, unless it is at its top with nothing newer left, where the
/// newest come into sight. The round chosen, however it was chosen, is brought to the list's middle when it is out of
/// sight.
export function RoundsList(props: {
  rounds: RoundSummary[];
  g: CompiledGraph;
  selected: RoundKey | null;
  onRound: (r: RoundSummary) => void;
  older?: boolean;
  newer?: boolean;
  onMore?: (end: 'older' | 'newer') => void;
  loading?: 'older' | 'newer' | null;
}) {
  const { rounds, g, selected, onRound, older = false, newer = false, onMore, loading = null } = props;
  const status = useStatus();
  const scroller = useRef<HTMLDivElement>(null);
  const newest = useMemo(() => [...rounds].reverse(), [rounds]);
  const live = newest.find((r) => r.outcome === 'running');
  const activity = useActivity(live?.work ?? 0);
  // The row nearest the list's top and how far below the top it was, kept as the list scrolls, so that rows joining
  // above leave it where it was; whether the list was at its top with nothing newer left; and the round last brought
  // into sight.
  const anchor = useRef<{ row: string; offset: number; top: boolean } | null>(null);
  const shown = useRef<string | null>(null);
  const keep = () => {
    const el = scroller.current;
    if (el === null) return;
    const top = el.getBoundingClientRect().top;
    // The rows are in order down the list: the first whose bottom is below the top is found by halves.
    const rows = el.querySelectorAll<HTMLElement>('tr[data-round]');
    let low = 0;
    let high = rows.length;
    while (low < high) {
      const mid = (low + high) >> 1;
      if (rows[mid].getBoundingClientRect().bottom > top) high = mid;
      else low = mid + 1;
    }
    const row = rows.item(low) ?? undefined;
    anchor.current =
      row !== undefined
        ? { row: row.dataset.round ?? '', offset: row.getBoundingClientRect().top - top, top: el.scrollTop <= 0 && !newer }
        : null;
  };
  const ends = () => {
    const el = scroller.current;
    if (el === null || onMore === undefined || loading !== null) return;
    for (const end of endsNear(el, { older, newer })) onMore(end);
  };
  useLayoutEffect(() => {
    const el = scroller.current;
    if (el === null) return;
    const listTop = el.getBoundingClientRect().top;
    const place = (row: HTMLElement) => {
      const b = row.getBoundingClientRect();
      return { top: b.top - listTop + el.scrollTop, height: b.height };
    };
    // Rows that joined above leave the row that was nearest the top where it was.
    const a = anchor.current;
    if (a !== null && !a.top) {
      const row = el.querySelector<HTMLElement>(`tr[data-round="${a.row}"]`);
      if (row !== null) el.scrollTop += row.getBoundingClientRect().top - listTop - a.offset;
    }
    // The round chosen comes to the middle when it is out of sight, once each time it is chosen.
    const chosen = selected !== null ? roundName(selected.work, selected.n) : null;
    if (chosen !== shown.current) {
      const row = chosen !== null ? el.querySelector<HTMLElement>(`tr[data-round="${chosen}"]`) : null;
      if (row !== null) {
        const p = place(row);
        if (!inSight(p, el)) el.scrollTop = centredTop(p, el);
        shown.current = chosen;
      } else if (chosen === null) {
        shown.current = null;
      }
    }
    keep();
    ends();
  });
  if (rounds.length === 0) return <p className="muted">{t('no_rounds')}</p>;
  const rows: ReactNode[] = [];
  newest.forEach((r, i) => {
    const stale = r.outcome === 'running' && status !== null && status.running !== r.work;
    const last = r.outcome === 'running' && !stale ? activity.filter((x) => x.round === r.n && live?.work === r.work).pop() : undefined;
    const what = r.not_asked !== null ? t('round_not_asked', { need: words(r.not_asked) }) : stale ? t('round_left_open') : (last?.what ?? tookWords(r));
    const name = rowName(r);
    rows.push(
      <tr
        key={name}
        data-round={name}
        className={`clickable${sameRound(selected, r) ? ' selected' : ''}`}
        onClick={() => onRound(r)}
      >
        <td className="muted nowrap">
          {name}
          {startedSession(r) ? (
            <>
              {' '}
              <SessionMark r={r} />
            </>
          ) : null}
        </td>
        <td className="nowrap">
          <span className="cell-id">{r.cell ?? '–'}</span> <Who r={r} g={g} />
        </td>
        <td className="rounds-what" title={stale ? t('round_left_open_hint') : what}>
          {badged(r.outcome) && !stale ? (
            <>
              <OutcomeBadge outcome={r.outcome} />{' '}
            </>
          ) : null}
          <span className={r.not_asked !== null || last !== undefined || stale ? 'muted' : undefined}>{what}</span>
        </td>
        <td className="num muted nowrap">
          <SpentBar r={r} />
          {r.cost_usd !== null && r.line !== null ? cost(r.cost_usd) : ''}
        </td>
        <td className="num muted nowrap rounds-time">{r.ended_at !== null ? duration(r.ended_at - r.started_at) : ''}</td>
      </tr>,
    );
    // Where one of the architect's works began, below its first round, when a work's rounds come before it.
    if (r.n === 1 && (i + 1 < newest.length || older)) {
      rows.push(
        <tr key={`${name}-began`} className="rounds-began" aria-hidden="true">
          <td colSpan={5}>
            <span className="rounds-began-at">{time(r.started_at)}</span>
          </td>
        </tr>,
      );
    }
  });
  return (
    <div
      className="rounds-scroll"
      ref={scroller}
      onScroll={() => {
        keep();
        ends();
      }}
    >
      {loading === 'newer' ? <p className="rounds-more muted">{t('loading')}</p> : null}
      <table className="list rounds-list">
        <tbody>{rows}</tbody>
      </table>
      {loading === 'older' ? <p className="rounds-more muted">{t('loading')}</p> : null}
    </div>
  );
}

/// The rounds tab of a work's page: when the page was opened on only the rounds sent back or only those that failed,
/// that it shows only those and the way to all of them; then the list of rounds, every round of the work.
export function RoundsTab({ rounds, g, only, selected, onAll, onRound }: {
  rounds: RoundSummary[];
  g: CompiledGraph;
  only: RoundsOnly | null;
  selected: RoundKey | null;
  /// Shows every round again.
  onAll: () => void;
  onRound: (round: RoundKey) => void;
}) {
  const shown = roundsShown(rounds, only);
  return (
    <>
      {only !== null ? (
        <p className="rounds-only">
          {tn(only === 'sent_back' ? 'rounds_only_sent_back' : 'rounds_only_failed', shown.length)}{' '}
          <LinkButton onClick={onAll}>{t('rounds_show_all', { n: rounds.length })}</LinkButton>
        </p>
      ) : null}
      <RoundsList rounds={shown} g={g} selected={selected} onRound={(x) => onRound({ work: x.work, n: x.n })} />
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

/// What a work's chart marks: what the rounds placed on it did at each cell, of the work or of the architect's works
/// its list holds, and the cell the work is at, whether a round runs there and why the work waits there while none
/// does; the cell the halo goes round; how many times those rounds took each arrow; and the arrows the round chosen
/// came in by and left by.
export function workMarks(
  g: CompiledGraph,
  v: GraphView,
  w: WorkSummary,
  rounds: RoundSummary[],
  c: Course,
  status: Status | null,
  halo: string | null,
  round: RoundKey | null = null,
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
/// it (for the architect, its latest work), with the rounds listed placed on it and the cell the work is at, opening on
/// the cell of the round chosen or the cell the work is at; with no work, the bare graph named; and the window floating
/// over its right side, the page's panel on top and what is open on the chart under it, a round in the round's own
/// panel.
export function WorkChart({ compiled, view, graph, w, rounds, cell, round, onCell, onRound, panel }: {
  compiled: Compiled;
  view: FlowView;
  /// The graph drawn while no work is shown.
  graph: string;
  /// The work shown, where it is and how it goes on: a work's page's, or the architect's latest; none for the bare
  /// graph.
  w: WorkSummary | null;
  /// The rounds placed on the chart: every round of a work, or the rounds the architect's list holds, oldest first.
  rounds: RoundSummary[];
  /// The cell open, and the round open, by its work and its number in it.
  cell: string | null;
  round: RoundKey | null;
  onCell: (cell: string | null) => void;
  onRound: (round: RoundKey | null) => void;
  /// The page's panel, given the graph drawn.
  panel: (g: CompiledGraph) => ReactNode;
}) {
  const status = useStatus();
  const name = w?.graph ?? graph;
  const g = compiled.graphs.find((x) => x.name === name);
  const v = view.graphs.find((x) => x.name === name);
  const graphs = useMemo(() => (g && v ? [{ g, view: v }] : []), [g, v]);
  const c = w !== null ? course(w, status) : null;
  // The halo goes round the rounds of the work shown that come in.
  const id = w?.id ?? null;
  const own = useMemo(() => (id !== null ? rounds.filter((r) => r.work === id) : NO_ROUNDS), [rounds, id]);
  const halo = useHalo(w, own, c);
  if (!g || !v) return <p className="muted page">{t('chart_no_graph', { graph: name })}</p>;
  const roundCell = round !== null ? (rounds.find((x) => sameRound(x, round))?.cell ?? null) : null;
  return (
    <FlowScreen
      key={`${w?.flow_version ?? 'none'}:${name}`}
      compiled={compiled}
      graphs={graphs}
      legend={w !== null ? 'work' : 'flow'}
      marks={[w !== null && c !== null ? workMarks(g, v, w, rounds, c, status, halo, round, compiled) : BARE]}
      cell={cell}
      onCell={(x) => onCell(x)}
      start={roundCell ?? w?.at ?? null}
      rounds={w !== null ? rounds : undefined}
      round={round}
      onRound={onRound}
      renderRound={
        w !== null
          ? (k) => <RoundView work={k.work} n={k.n} rounds={rounds.filter((r) => r.work === k.work)} />
          : undefined
      }
      side={panel(g)}
    />
  );
}
