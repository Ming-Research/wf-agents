// A work's page, under its line above the chart, which holds its facts and the owner's controls: the state machine of
// the flow version it is pinned to, filling the window but for the column docked at its right, with what the work did
// on it: the cell it is at ringed, each cell it visited with its rounds and the rounds sent back or failed, the arrows
// it took standing out, the cell a round runs at in a moving halo and the cell a work held back stands at marked. The
// column holds on top the work's panel in tabs: its rounds, under how it goes on, the Step that runs one being the
// icon in the line above with what it will do on hovering; its design cards, the one waiting for the owner's ruling
// first with its controls; its conversation, the same the messages view shows: its asks waiting for the owner's answer
// pinned with their forms, then what its agents, the owner, the architect and the engine said about it. A round or a
// cell opened shows in the column's lower part, its cell brought into view.

import { useEffect, useMemo, useRef, useState } from 'react';

import type {
  Compiled,
  CompiledGraph,
  DesignCard,
  FlowView,
  GraphView,
  RoundSummary,
  WorkDetail,
  WorkSummary,
} from '../api';
import { type Course, course, heldName, Loaded, OutcomeBadge } from '../components';
import { held, runningWords, takeoverWords, waitsWords } from '../course';
import { FlowScreen } from '../flow/FlowScreen';
import { cost, duration, roundName } from '../format';
import { overlays, roundArrows, taken, chart } from '../graph/flowModel';
import { spent } from '../round';
import { useActivity, useResource, useStatus } from '../live';
import { ItemRef, RoundLink } from '../parts';
import { href, navigate, type RoundsOnly, type WorkRoute, type WorkTab } from '../route';
import { t } from '../text';
import { bringIntoView, currentCard, DesignCardView, words } from './Decide';
import { ConversationView } from './Messages';
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
                  <span className="badge" title={t('round_left_open_hint')}>
                    {t('round_left_open')}
                  </span>
                ) : r.outcome !== 'took' ? (
                  <OutcomeBadge outcome={r.outcome} />
                ) : null}{' '}
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
function Going({ d, onCard, onAsk }: {
  d: WorkDetail;
  /// Open the design card, or the ask, the work waits for, in its tab of the window.
  onCard: (id: number) => void;
  onAsk: (id: number) => void;
}) {
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
  return (
    <section className={`next going${c.held === 'engine' ? ' paused' : ''}`}>
      <div className="next-head">
        <strong>{head}</strong>
        {runs && since !== null ? <span className="muted">{duration(now - since)}</span> : null}
      </div>
      {(c.held === 'engine' || c.held === 'architect') && w.paused !== null ? <p className="prose">{w.paused}</p> : null}
      {c.held === 'you' && why !== t('course_paused') ? <p className="prose">{why}</p> : null}
      {c.held === 'all' && !runs ? <p className="muted">{t('course_note_all')}</p> : null}
      {c.releasing ? <p className="muted">{t('course_releasing_why')}</p> : null}
      {waits !== null && (runs || why !== null) ? <p className="prose">{waitsWords(waits)}</p> : null}
      {waits?.kind === 'ruling' ? (
        <div>
          <button type="button" className="decide-button" onClick={() => onCard(waits.design_card)}>
            {t('next_rule_card', { id: waits.design_card })}
          </button>
        </div>
      ) : null}
      {waits?.kind === 'answer' ? (
        <div>
          <button type="button" className="decide-button" onClick={() => onAsk(waits.ask)}>
            {t('next_answer_ask', { id: waits.ask })}
          </button>
        </div>
      ) : null}
      {waits?.kind === 'items' ? (
        <p>
          {waits.items.map((i) => (
            <ItemRef key={i} id={i} />
          ))}
        </p>
      ) : null}
      {!runs && waits === null && w.answered !== null ? (
        <p className="muted">{t('course_answered', { id: w.answered })}</p>
      ) : null}
      {c.outdated ? <p className="warn">{takeoverWords(w, status)}</p> : null}
      {runs ? (
        <ol className="next-activity">
          {activity.map((x, i) => (
            <li key={i}>{x.what}</li>
          ))}
        </ol>
      ) : null}
    </section>
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

/// The work's panel, on top of the column docked at the right of its chart: its tabs, the rounds (under how the work
/// goes on and its next round), the design cards and the messages.
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
  const onCard = (id: number) => show('cards', `design-card-${id}`);
  const onAsk = (id: number) => show('messages', `ask-${id}`);
  const openCards = d.design_cards.filter((c) => c.status === 'open').length;
  const openAsks = d.asks.filter((a) => a.answer === null).length;
  const tabs: Array<{ tab: WorkTab; label: string; n: number | null; chip: string | null; hint: string }> = [
    { tab: 'rounds', label: t('rounds'), n: d.rounds.length, chip: null, hint: '' },
    {
      tab: 'cards',
      label: t('design_cards'),
      n: d.design_cards.length,
      chip: openCards > 0 ? t('open_design_cards_chip', { n: openCards }) : null,
      hint: t('open_design_cards_n', { n: openCards }),
    },
    {
      tab: 'messages',
      label: t('work_messages_tab'),
      n: d.messages.length + d.asks.length,
      chip: openAsks > 0 ? t('open_asks_chip', { n: openAsks }) : null,
      hint: t('open_asks_n', { n: openAsks }),
    },
    { tab: 'usage', label: t('us_tab'), n: null, chip: null, hint: '' },
  ];
  const shown = roundsShown(d.rounds, r.only);
  return (
    <div className="drawer">
      <div className="drawer-head">
        <div className="tabs" role="tablist">
          {tabs.map((x) => (
            <button
              key={x.tab}
              type="button"
              role="tab"
              className="tab"
              aria-selected={x.tab === r.tab}
              onClick={() => show(x.tab, null)}
            >
              {x.label}
              {x.n !== null ? <span className="tab-n">{x.n}</span> : null}
              {x.chip !== null ? (
                <span className="chip warn" title={x.hint}>
                  {x.chip}
                </span>
              ) : null}
            </button>
          ))}
        </div>
      </div>
      <div className="drawer-body" role="tabpanel" ref={body}>
        {r.tab === 'rounds' ? (
          <>
            <Going d={d} onCard={onCard} onAsk={onAsk} />
            {r.only !== null ? (
              <div className="rounds-only">
                <span>
                  {t(r.only === 'sent_back' ? 'rounds_only_sent_back' : 'rounds_only_failed', { n: shown.length })}
                </span>
                <button type="button" className="link" onClick={() => go({ only: null })}>
                  {t('rounds_show_all', { n: d.rounds.length })}
                </button>
              </div>
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
        {r.tab === 'messages' ? <ConversationView conversation={`work:${d.work.id}`} /> : null}
        {r.tab === 'usage' ? <Usage d={d} g={g} /> : null}
      </div>
    </div>
  );
}

/// What a work's chart marks: what it did at each cell (the cell it is at, whether a round runs there and why it is
/// held back), how many times it took each arrow, and the arrows the round chosen came in by and left by.
export function workMarks(
  g: CompiledGraph,
  v: GraphView,
  w: WorkSummary,
  rounds: RoundSummary[],
  c: Course,
  round: number | null = null,
) {
  const model = chart(g, v);
  return {
    // A tool cell runs while its tool is under way, as well as while the engine takes its step.
    overlays: overlays(rounds, w.at, { running: c.running !== null || w.running?.tool === true, held: held(c, w) }),
    taken: taken(model, g, rounds),
    lit: round !== null ? roundArrows(model, g, rounds, round) : undefined,
    added: new Set<string>(),
    changed: new Set<string>(),
  };
}

function Body({ r, d, compiled, view }: { r: WorkRoute; d: WorkDetail; compiled: Compiled; view: FlowView }) {
  const status = useStatus();
  const g = compiled.graphs.find((x) => x.name === d.work.graph);
  const v = view.graphs.find((x) => x.name === d.work.graph);
  const graphs = useMemo(() => (g && v ? [{ g, view: v }] : []), [g, v]);
  if (!g || !v) return <p className="muted page">{t('none')}</p>;
  const roundCell = r.round !== null ? (d.rounds.find((x) => x.n === r.round)?.cell ?? null) : null;
  const go = (to: Partial<WorkRoute>) => navigate(href({ ...r, ...to }), true);
  const c = course(d.work, status);
  return (
    <FlowScreen
      compiled={compiled}
      graphs={graphs}
      legend="work"
      marks={[workMarks(g, v, d.work, d.rounds, c, r.round)]}
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
