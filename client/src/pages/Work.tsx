// A work's page, under its line above the chart, which holds its facts and the owner's controls: the state machine of
// the flow version it is pinned to, filling the window but for the column docked at its right, with what the work did
// on it: the cell it is at ringed, each cell it visited with its rounds and the rounds sent back or failed, the arrows
// it took standing out, the cell a round runs or has just run at in a moving halo, and beside the cell it waits at why,
// in a few words. The page only reads the work: the owner acts on it in its group in the messages view, which its line
// and what it waits for lead to. The column holds on top the work's panel in tabs: its rounds, under how it goes on;
// its design cards, the one waiting for the owner's ruling first; what its helpers (the advisor, the review, the
// adversarial testing) and its engineer said to each other, one thread for each helper, which is in no group; and its
// usage. A round or a cell opened shows in the column's lower part, its cell brought into view.

import { CounterLabel, Flash as PrimerFlash, Label, UnderlineNav } from '@primer/react';
import { useEffect, useMemo, useRef, useState } from 'react';

import type {
  Compiled,
  CompiledGraph,
  DesignCard,
  FlowView,
  GraphView,
  RoundSummary,
  Status,
  WorkDetail,
  WorkSummary,
} from '../api';
import { type Course, course, heldName, Loaded, OutcomeBadge } from '../components';
import { held, runningWords, takeoverWords, waitOf, waitsWords } from '../course';
import { FlowScreen } from '../flow/FlowScreen';
import { cost, duration, roundName } from '../format';
import { chart, type Flash, haloAt, overlays, roundArrows, taken } from '../graph/flowModel';
import { spent } from '../round';
import { useActivity, useResource, useStatus } from '../live';
import { By, ItemRef, LinkButton, RoundLink, Section } from '../parts';
import { href, Link, navigate, type RoundsOnly, type WorkRoute, type WorkTab } from '../route';
import { t, tn } from '../text';
import { Markdown } from '../markdown';
import { bringIntoView, currentCard, DesignCardView, words } from './Decide';
import { RoundView } from './RoundView';
import { Usage } from './Usage';

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

/// Where an agent's round spent its cost, beside its cost in its row: a small bar of the shares read again from the
/// cache, newly cached and output, and its steps; the whole of it on hovering. None for a round no agent ran.
function SpentBar({ r }: { r: RoundSummary }) {
  if (r.tokens === null || r.line === null || r.not_asked !== null) return null;
  const s = spent(r.tokens, r.model);
  const sum = s.shares !== null ? s.shares.read + s.shares.written + s.shares.output : 0;
  const width = (x: number) => `${sum > 0 ? (100 * x) / sum : 0}%`;
  return (
    <span className="spent" title={s.parts.map((p) => p.whole).join(' · ')}>
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
/// course: sent back, failed, cut off, running), whole and wrapped rather than cut, what an agent's round cost, where
/// its cost went (its key is in the chart's "?") and how long it took. The round chosen, however it was chosen, is
/// scrolled into view.
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
          const what = r.not_asked !== null ? t('round_not_asked', { need: words(r.not_asked) }) : took;
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
                    <Label variant="secondary" title={t('round_left_open_hint')}>
                      {t('round_left_open')}
                    </Label>{' '}
                  </>
                ) : r.outcome !== 'took' ? (
                  <>
                    <OutcomeBadge outcome={r.outcome} />{' '}
                  </>
                ) : null}
                <span className={r.not_asked !== null ? 'muted' : undefined}>{what}</span>
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

/// How the work goes on, at the top of its rounds: whether a round of it runs (how long, and what the agent is doing),
/// what holds it back and why in full, and what it waits for, with where the owner gives it; on an older flow version,
/// that it is taken over when it goes on. The owner's controls of it are in its line above the chart.
function Going({ d }: { d: WorkDetail }) {
  const status = useStatus();
  const w = d.work;
  const c = course(w, status);
  const activity = useActivity(w.id);
  const [now, setNow] = useState(() => Date.now());
  const runs = c.running !== null;
  useEffect(() => {
    if (!runs) return;
    const tick = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(tick);
  }, [runs]);
  if (c.over) return null;
  const why = held(c, w);
  const since = w.running?.started_at ?? null;
  const head = runs
    ? runningWords(c)
    : why !== null
      ? heldName(c).name
      : c.waits !== null
        ? waitsWords(c.waits)
        : c.started
          ? t('course_ready_started')
          : t('state_ready');
  const waits = c.waits;
  // A Flash in the colour of how the work goes on: a round running is news, a stop of the engine's is a problem, and
  // a work held back or waiting waits for someone.
  const variant = runs ? 'default' : c.held === 'engine' ? 'danger' : why !== null || waits !== null ? 'warning' : 'default';
  return (
    <PrimerFlash variant={variant} className="going">
      <div className="going-head">
        <strong>{head}</strong>
        {runs && since !== null ? <span className="muted">{duration(now - since)}</span> : null}
      </div>
      {(c.held === 'engine' || c.held === 'architect') && w.paused !== null ? <p className="prose">{w.paused}</p> : null}
      {c.held === 'you' && why !== t('course_paused') ? <p className="prose">{why}</p> : null}
      {c.held === 'all' && !runs ? <p>{t('course_note_all')}</p> : null}
      {c.releasing ? <p>{t('course_releasing_why')}</p> : null}
      {waits !== null && (runs || why !== null) ? <p className="prose">{waitsWords(waits)}</p> : null}
      {waits?.kind === 'ruling' || waits?.kind === 'answer' || waits?.kind === 'architect' ? (
        <p>
          <Link to={href({ page: 'messages', open: `work:${w.id}` })}>{t('go_to_conversation')}</Link>
        </p>
      ) : null}
      {waits?.kind === 'items' ? (
        <p className="going-items">
          {waits.items.map((i) => (
            <ItemRef key={i} id={i} />
          ))}
        </p>
      ) : null}
      {!runs && waits === null && w.answered !== null ? <p>{t('course_answered', { id: w.answered })}</p> : null}
      {c.outdated ? <p>{takeoverWords(w, status)}</p> : null}
      {runs && activity.length > 0 ? (
        <ol className="going-activity">
          {activity.map((x, i) => (
            <li key={i}>{x.what}</li>
          ))}
        </ol>
      ) : null}
    </PrimerFlash>
  );
}

/// The work's design cards, in the card every card shares: the one waiting for the owner's ruling first, then the
/// others, newest first; the current one (the one waiting, else the newest not replaced) open, every other folded to
/// its one line.
function Cards({ cards, onRound }: { cards: DesignCard[]; onRound: (r: number) => void }) {
  if (cards.length === 0) return <p className="muted">{t('work_no_design_cards')}</p>;
  const current = currentCard(cards);
  const open = cards.filter((c) => c.status === 'open');
  const rest = cards.filter((c) => c.status !== 'open').reverse();
  return (
    <>
      <p className="dc-lead">{t('dc_lead')}</p>
      {[...open, ...rest].map((c) => (
        <DesignCardView
          key={c.id}
          c={c}
          round={<RoundLink work={c.work} round={c.round} onRound={(_, r) => onRound(r)} />}
          folded={c.id !== current}
        />
      ))}
    </>
  );
}

/// The rounds of the engineer next to a helper's: the one that asked it, before it, and the one that took up what it
/// said, after it, the engine's and the tools' rounds between them aside.
function nextTo(rounds: RoundSummary[], i: number, step: number): RoundSummary | null {
  for (let k = i + step; k >= 0 && k < rounds.length; k += step) {
    const r = rounds[k];
    if (r.line === 'engineer') return r;
    if (r.line !== null) return null;
  }
  return null;
}

/// The helpers of a work, each the line that runs it: its talk with the engineer is a thread of its own.
const HELPERS = ['advisor', 'review', 'adversarial'] as const;

/// What a work's helpers and its engineer said to each other, read only, one thread for each helper and never mixed:
/// each helper's rounds with the engineer's round that asked it before them and the one that took up what it said
/// after them, each saying who, its round and what it said it did.
function Helpers({ d, onRound }: { d: WorkDetail; onRound: (round: number) => void }) {
  const threads = HELPERS.map((line) => {
    const picked = new Map<number, RoundSummary>();
    d.rounds.forEach((r, i) => {
      if (r.line !== line) return;
      for (const x of [nextTo(d.rounds, i, -1), r, nextTo(d.rounds, i, 1)]) if (x !== null) picked.set(x.n, x);
    });
    return { line, rounds: [...picked.values()].sort((a, b) => a.n - b.n) };
  }).filter((x) => x.rounds.length > 0);
  if (threads.length === 0) return <p className="muted">{t('helpers_none')}</p>;
  return (
    <>
      {threads.map((x) => (
        <Section key={x.line} title={t('helpers_thread', { line: words(x.line) })} n={x.rounds.length} open>
          {x.rounds.map((r) => (
            <div key={r.n} className="helper-said">
              <By who={words(r.line ?? '')} cell={r.cell} at={r.ended_at ?? r.started_at} work={d.work.id} round={r.n} onRound={(_, n) => onRound(n)} />
              {r.progress !== null && r.progress !== '' ? (
                <div className="agent-box">
                  <Markdown text={r.progress} />
                </div>
              ) : null}
            </div>
          ))}
        </Section>
      ))}
    </>
  );
}

/// The work's panel, on top of the column docked at the right of its chart: its tabs, the rounds (under how the work
/// goes on), the design cards, its helpers' talk with its engineer, and its usage.
function WorkPanel({ d, g, r, go }: {
  d: WorkDetail;
  g: CompiledGraph;
  r: WorkRoute;
  go: (to: Partial<WorkRoute>) => void;
}) {
  const status = useStatus();
  // What to bring into view once its tab is shown: the design card or the ask the owner was sent to; else a tab shows
  // from its top.
  const want = useRef<string | null>(null);
  const body = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (want.current === null) body.current?.scrollTo({ top: 0 });
  }, [r.tab]);
  useEffect(() => {
    if (want.current !== null && bringIntoView(want.current)) want.current = null;
  });
  const show = (tab: WorkTab, at: string | null) => {
    want.current = at;
    go({ tab });
  };
  const onRound = (round: number) => go({ round, cell: null });
  const openCards = d.design_cards.filter((c) => c.status === 'open').length;
  const helped = d.rounds.filter((x) => x.line !== null && (HELPERS as readonly string[]).includes(x.line)).length;
  // Each tab's count, and what in it waits for the owner as a count of its own.
  const tabs: Array<{ tab: WorkTab; label: string; n: number | null; waiting: number; hint: string }> = [
    { tab: 'rounds', label: t('rounds'), n: d.rounds.length, waiting: 0, hint: '' },
    {
      tab: 'cards',
      label: t('design_cards'),
      n: d.design_cards.length,
      waiting: openCards,
      hint: t('open_design_cards_n', { n: openCards }),
    },
    { tab: 'helpers', label: t('helpers_tab'), n: helped, waiting: 0, hint: '' },
    { tab: 'usage', label: t('us_tab'), n: null, waiting: 0, hint: '' },
  ];
  const shown = roundsShown(d.rounds, r.only);
  return (
    <div className="drawer">
      <UnderlineNav aria-label={t('work_tabs')} className="drawer-tabs">
        {tabs.map((x) => (
          <UnderlineNav.Item
            key={x.tab}
            href={href({ ...r, tab: x.tab, round: r.round, cell: r.cell })}
            aria-current={x.tab === r.tab ? 'page' : undefined}
            onSelect={(e) => {
              // A click with a modifier key opens the tab as the browser would.
              if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
              e.preventDefault();
              show(x.tab, null);
            }}
          >
            {x.label}
            {x.n !== null ? <CounterLabel className="drawer-count">{x.n}</CounterLabel> : null}
            {x.waiting > 0 ? (
              <CounterLabel variant="primary" className="drawer-count" title={x.hint}>
                {x.waiting}
              </CounterLabel>
            ) : null}
          </UnderlineNav.Item>
        ))}
      </UnderlineNav>
      <div className="drawer-body" ref={body}>
        {r.tab === 'rounds' ? (
          <>
            <Going d={d} />
            {r.only !== null ? (
              <p className="rounds-only">
                {tn(r.only === 'sent_back' ? 'rounds_only_sent_back' : 'rounds_only_failed', shown.length)}{' '}
                <LinkButton onClick={() => go({ only: null })}>{t('rounds_show_all', { n: d.rounds.length })}</LinkButton>
              </p>
            ) : null}
            <Timeline
              work={d.work.id}
              rounds={shown}
              g={g}
              stale={status !== null && status.running !== d.work.id}
              selected={r.round}
              onRound={(x) => onRound(x.n)}
            />
          </>
        ) : null}
        {r.tab === 'cards' ? <Cards cards={d.design_cards} onRound={onRound} /> : null}
        {r.tab === 'helpers' ? <Helpers d={d} onRound={onRound} /> : null}
        {r.tab === 'usage' ? <Usage d={d} g={g} /> : null}
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
/// round chosen came in by and left by; `flow`, the work's flow, gives the arrows to the circles beside the actions
/// that start a task of another graph, which the work takes with the rows through those actions.
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
  const model = chart(g, v, flow?.engine ?? null, flow?.graphs ?? []);
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

export function Body({ r, d, compiled, view }: { r: WorkRoute; d: WorkDetail; compiled: Compiled; view: FlowView }) {
  const status = useStatus();
  const g = compiled.graphs.find((x) => x.name === d.work.graph);
  const v = view.graphs.find((x) => x.name === d.work.graph);
  const graphs = useMemo(() => (g && v ? [{ g, view: v }] : []), [g, v]);
  const c = course(d.work, status);
  const halo = useHalo(d.work, d.rounds, c);
  if (!g || !v) return <p className="muted page">{t('none')}</p>;
  const roundCell = r.round !== null ? (d.rounds.find((x) => x.n === r.round)?.cell ?? null) : null;
  const go = (to: Partial<WorkRoute>) => navigate(href({ ...r, ...to }), true);
  return (
    <FlowScreen
      compiled={compiled}
      graphs={graphs}
      legend="work"
      marks={[workMarks(g, v, d.work, d.rounds, c, status, halo, r.round, compiled)]}
      cell={r.cell}
      onCell={(c) => go({ round: null, cell: c })}
      start={roundCell ?? d.work.at}
      work={d.work.id}
      rounds={d.rounds}
      round={r.round}
      onRound={(x) => go({ round: x, cell: null })}
      renderRound={(x) => <RoundView work={d.work.id} n={x} rounds={d.rounds} />}
      side={<WorkPanel d={d} g={g} r={r} go={go} />}
    />
  );
}

export function Work({ r }: { r: WorkRoute }) {
  const detail = useResource<WorkDetail>(`work:${r.id}`, `/api/v1/works/${r.id}`);
  const version = detail.data?.work.flow_version ?? null;
  const compiled = useResource<Compiled>(null, version !== null ? `/api/v1/flows/${version}` : null);
  const view = useResource<FlowView>(null, version !== null ? `/api/v1/flows/${version}/view` : null);
  return (
    <div className="screen">
      <Loaded r={detail}>
        {(d) => (
          <Loaded r={compiled}>
            {(c) => <Loaded r={view}>{(v) => <Body r={r} d={d} compiled={c} view={v} />}</Loaded>}
          </Loaded>
        )}
      </Loaded>
    </div>
  );
}
