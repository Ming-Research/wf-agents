// The architect's page, laid out as a work's page, since the architect runs a long time and the owner comes back to it.
// Its line says how the architect stands in words (asleep since when, what falls due next and how many matters wait;
// or awake: the question it is at, the matter it handles, a round running and for how long, or what holds it back),
// what stops new works from starting, the architect's Pause, Continue and Step, a way to say something to it, and the
// switch between its wakes (when each woke, why, how many matters it handled, what it cost). Under the line, its graph
// fills the window with the wake shown placed on it as a work is (the wake going on, else the latest, else the bare
// graph), and the column docked at its right holds the architect's panel on top and what is open on the chart below
// it. The panel's tabs: its asks of the owner, those waiting first with their forms; its queue, the matters waiting for
// it, then those held and those not due yet; its decisions, newest first; the policy in force, the one being set and the
// plan; where things stand as it understands them; and the rounds of the wake shown.

import { useEffect, useMemo, useRef, useState } from 'react';

import type {
  Architect as ArchitectRead,
  Compiled,
  CompiledGraph,
  FlowView,
  Matter,
  OwnerAsk,
  Wake,
  WorkDetail,
  WorkSummary,
} from '../api';
import { architectState, overWake, queueParts, shownWake } from '../architect';
import { course, Loaded } from '../components';
import { Controls, held, runningWords, useActions } from '../course';
import { FlowScreen } from '../flow/FlowScreen';
import { cost, duration, roundName, time } from '../format';
import { postJson } from '../http';
import { Fold } from '../item/Shapes';
import { words } from '../labelled';
import { reread, useResource, useStatus } from '../live';
import { type ArchitectRoute, type ArchitectTab, architectHref, href, Link, navigate, workHref } from '../route';
import { t, type Key } from '../text';
import { ArchitectAsk, askKindWords } from './ArchitectAsk';
import {
  LogEntryView,
  type OpenRound,
  PlanView,
  PolicyText,
  ProbesView,
  ReadingView,
  RoundLink,
  StudyView,
  UnderstandingView,
} from './ArchitectViews';
import { bringIntoView } from './Decide';
import { RoundView } from './RoundView';
import { Next, Timeline, workMarks } from './Work';

/// The graph the architect's wakes run, as the flow names it.
const GRAPH = 'architect';

/// What the bare graph marks: nothing.
const BARE = { overlays: null, taken: null, added: new Set<string>(), changed: new Set<string>() };

/// The time now, ticking every second while `ticking`.
function useNow(ticking: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!ticking) return;
    const tick = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(tick);
  }, [ticking]);
  return now;
}

/// The architect's Step: one round of the wake going on, or, when none goes on and a matter waits, a new wake and its
/// first round. It asks first, as a real round runs on the owner's account.
function useStep() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const step = (ask: string) => {
    if (!window.confirm(ask)) return;
    setBusy(true);
    setError(null);
    postJson('/api/v1/architect/step', {}).then(
      () => {
        setBusy(false);
        reread('architect', 'works', 'work:*');
      },
      (e: unknown) => {
        setBusy(false);
        setError(e instanceof Error ? e.message : String(e));
      },
    );
  };
  return { busy, error, step };
}

/// How the wake going on goes on, on one line: a round running and for how long, or what holds it back, or what it
/// waits for, linked to the ask; then the question it is at and the matter it handles; and its Pause, Continue and Step.
function Awake({ w, handling, onAsk }: { w: WorkSummary; handling: string; onAsk: (id: number) => void }) {
  const status = useStatus();
  const a = useActions(w, status, true);
  const s = useStep();
  const c = a.c;
  const waits = c.waits;
  const now = useNow(c.running !== null);
  const why = held(c, w);
  const stage = w.stage !== null ? words(w.stage) : '';
  const cell = `${w.at} ${w.at_title ?? ''}`;
  return (
    <>
      <span className="work-state" title={why ?? undefined}>
        <strong>{t('arch_awake', { id: w.id })}</strong>{' '}
        {c.running !== null ? (
          <>
            <span className="badge state-running">{runningWords(c)}</span>
            {w.running ? <span className="muted"> {duration(Math.max(0, now - w.running.started_at))}</span> : null}
          </>
        ) : why !== null ? (
          <span className={`badge ${c.held === 'engine' ? 'state-stopped' : 'state-paused'}`}>
            {c.held === 'engine' ? t('course_stopped') : t('course_paused')}
          </span>
        ) : null}
      </span>
      {waits?.kind === 'answer' ? (
        <button type="button" className="chip waits chip-button" onClick={() => onAsk(waits.ask)}>
          {t('arch_waits_answer', { id: waits.ask })}
        </button>
      ) : null}
      <span className="work-where" title={`${cell}${stage ? ` · ${stage}` : ''}`}>
        <span className="cell-id">{w.at}</span> {w.at_title}
        {stage ? <span className="muted"> · {stage}</span> : null}
      </span>
      {handling !== '' ? (
        <span className="work-name" title={handling}>
          {t('arch_handling', { what: handling })}
        </span>
      ) : null}
      <Controls
        a={a}
        w={w}
        status={status}
        onStep={() => s.step(t('arch_step_confirm', { id: w.id, cell }))}
      />
      {a.error !== null || s.error !== null ? (
        <span className="bad why" title={a.error ?? s.error ?? ''}>
          {t('request_failed', { error: a.error ?? s.error ?? '' })}
        </span>
      ) : null}
    </>
  );
}

const HOLDS: Record<string, Key> = { architect: 'hold_architect', tidying: 'hold_tidying', policy: 'hold_policy' };

/// The line above the architect's chart: how it stands, what stops new works, its controls, what to say to it, and
/// the switch between its wakes.
function Line({ a, shown, current, onTab, onAsk, pop, setPop }: {
  a: ArchitectRead;
  shown: Wake | null;
  /// The wake going on, as freshly read as the page has it.
  current: WorkSummary | null;
  onTab: (tab: ArchitectTab) => void;
  onAsk: (id: number) => void;
  pop: 'say' | 'wakes' | null;
  setPop: (p: 'say' | 'wakes' | null) => void;
}) {
  const status = useStatus();
  const state = architectState(a.wakes, a.queue);
  const s = useStep();
  const handling = (() => {
    const h = state.kind === 'awake' ? state.handling : null;
    return h !== null && typeof h === 'object' && !Array.isArray(h) && typeof h.matter === 'string' ? h.matter : '';
  })();
  // With no wake going on, the owner's Step wakes the architect while every work is paused and a matter waits: else
  // the engine wakes it by itself.
  const canWake = state.kind !== 'awake' && state.waiting > 0 && status !== null && status.paused;
  return (
    <div className="work-line arch-line">
      {state.kind === 'awake' && current !== null ? (
        <Awake w={current} handling={handling} onAsk={onAsk} />
      ) : (
        <span className="work-state">
          <strong>{state.kind === 'never' ? t('arch_never') : t('arch_asleep', { at: time(state.kind === 'asleep' ? state.since : null) })}</strong>
          {state.kind === 'asleep' && state.next !== null ? (
            <span className="muted" title={state.next.what}>
              {' '}
              · {t('arch_next_due', { at: time(state.next.due) })}
            </span>
          ) : null}
        </span>
      )}
      {state.waiting > 0 ? (
        <button type="button" className="link" onClick={() => onTab('queue')}>
          {t('arch_matters_wait', { n: state.waiting })}
        </button>
      ) : null}
      {a.holds.map((h) => (
        <span key={h.reason} className="chip warn" title={t('hold_since', { at: time(h.since) })}>
          {HOLDS[h.reason] !== undefined ? t(HOLDS[h.reason]) : t('hold_other', { reason: words(h.reason) })}
        </span>
      ))}
      {canWake ? (
        <button
          type="button"
          className="control"
          disabled={s.busy || status?.stepping !== null || status?.halt !== null}
          title={t('arch_wake_hint')}
          onClick={() => s.step(t('arch_wake_confirm'))}
        >
          {t('work_step')}
        </button>
      ) : null}
      {s.error !== null ? <span className="bad why">{t('request_failed', { error: s.error })}</span> : null}
      <button
        type="button"
        className="control"
        aria-expanded={pop === 'say'}
        title={t('arch_say_hint')}
        onClick={() => setPop(pop === 'say' ? null : 'say')}
      >
        {t('arch_say')}
      </button>
      {shown !== null ? (
        <button
          type="button"
          className="bar-btn arch-wake-switch"
          aria-expanded={pop === 'wakes'}
          title={t('arch_wakes_hint')}
          onClick={() => setPop(pop === 'wakes' ? null : 'wakes')}
        >
          {t('arch_wake_shown', { id: shown.work.id, at: time(shown.work.started_at), n: a.wakes.length })} ▾
        </button>
      ) : null}
    </div>
  );
}

/// What the owner says to the architect: it goes to the architect's queue as the owner's words.
function SayBox({ onClose }: { onClose: () => void }) {
  const [text, setText] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const send = () => {
    setBusy(true);
    setError(null);
    postJson('/api/v1/architect/say', { text: text.trim() }).then(
      () => {
        reread('architect');
        onClose();
      },
      (e: unknown) => {
        setBusy(false);
        setError(e instanceof Error ? e.message : String(e));
      },
    );
  };
  return (
    <div className="bar-pop arch-pop arch-say" role="dialog" aria-label={t('arch_say')}>
      <div className="bar-pop-h">{t('arch_say')}</div>
      <p className="muted">{t('arch_say_note')}</p>
      <textarea value={text} rows={4} onChange={(e) => setText(e.target.value)} disabled={busy} />
      <div className="bar-pop-row">
        <button type="button" className="decide-button" onClick={send} disabled={busy || text.trim() === ''}>
          {t('arch_say_send')}
        </button>
        <button type="button" className="tog-btn" onClick={onClose}>
          {t('close')}
        </button>
      </div>
      {error !== null ? <p className="bad">{t('request_failed', { error })}</p> : null}
    </div>
  );
}

/// The architect's wakes over time, newest first: when each woke, why, how many matters it handled, its rounds and
/// cost; a click shows that wake, and the one shown by default follows the wake going on again.
function WakeList({ a, shown, tab, onClose }: { a: ArchitectRead; shown: number | null; tab: ArchitectTab; onClose: () => void }) {
  const followed = shownWake(a.wakes, null)?.work.id ?? null;
  return (
    <div className="bar-pop arch-pop arch-wakes" role="dialog" aria-label={t('arch_wakes', { n: a.wakes.length })}>
      <div className="bar-pop-h">{t('arch_wakes', { n: a.wakes.length })}</div>
      <div className="arch-wakes-list">
        <table className="list mini">
          <thead>
            <tr>
              <th>{t('arch_col_wake')}</th>
              <th>{t('arch_col_woke')}</th>
              <th>{t('arch_col_why')}</th>
              <th className="num">{t('arch_col_handled')}</th>
              <th className="num">{t('col_rounds')}</th>
              <th className="num">{t('col_cost')}</th>
            </tr>
          </thead>
          <tbody>
            {a.wakes.map((x) => {
              const why = [...new Set(x.woke_for.map((m) => words(m.kind)))].join(', ');
              return (
                <tr
                  key={x.work.id}
                  className={`clickable${x.work.id === shown ? ' selected' : ''}`}
                  onClick={() => {
                    navigate(architectHref({ wake: x.work.id === followed ? null : x.work.id, tab }));
                    onClose();
                  }}
                >
                  <td className="nowrap">
                    #{x.work.id}
                    {!overWake(x.work) ? <span className="badge state-running">{t('arch_wake_going')}</span> : null}
                  </td>
                  <td className="nowrap">{time(x.work.started_at)}</td>
                  <td className="clip" title={x.woke_for.map((m) => `${words(m.kind)}: ${m.what}`).join('\n')}>
                    {why !== '' ? why : '–'}
                  </td>
                  <td className="num">{x.handled}</td>
                  <td className="num">{x.work.rounds}</td>
                  <td className="num">{cost(x.work.cost_usd)}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      <div className="bar-pop-row">
        <button type="button" className="tog-btn" onClick={onClose}>
          {t('close')}
        </button>
      </div>
    </div>
  );
}

const SOURCES: Record<string, Key> = {
  work: 'source_work',
  architect: 'source_architect',
  owner: 'source_owner',
  engine: 'source_engine',
};

/// One matter in the architect's queue: its kind as a chip coloured by who raised it, what it is, its item, and who
/// waits for it and how long; or what holds it, or when it falls due.
function MatterRow({ m, now, onAsk }: { m: Matter; now: number; onAsk: (id: number) => void }) {
  const since = duration(Math.max(0, now - (m.due ?? m.at)));
  return (
    <li className="arch-matter">
      <div className="work-item-head">
        <span
          className={`badge kind src-${m.source}`}
          title={SOURCES[m.source] !== undefined ? t(SOURCES[m.source]) : undefined}
        >
          {words(m.kind)}
        </span>
        <span className="arch-matter-what">{m.what}</span>
      </div>
      <div className="muted arch-matter-meta">
        {m.item !== null ? (
          <Link to={`/items/${m.item}`}>
            #{m.item} {m.title ?? ''}
          </Link>
        ) : null}
        {m.waits === 'agent' && m.work !== null ? (
          <span>
            <Link to={workHref(m.work)}>{t('work_title', { id: m.work })}</Link> {t('matter_waits_agent', { for: since })}
          </span>
        ) : null}
        {m.waits === 'owner' ? <span>{t('matter_waits_owner', { for: since })}</span> : null}
        {m.waits === 'none' ? <span>{t('matter_waits_none', { for: since })}</span> : null}
        {m.held !== null && m.held_by !== null ? (
          <button type="button" className="link" onClick={() => onAsk(m.held_by ?? 0)}>
            {t(m.held === 'approval' ? 'matter_held_approval' : 'matter_held_ask', { id: m.held_by })}
          </button>
        ) : null}
        {m.waits === null && m.held === null && m.due !== null ? <span>{t('matter_due', { at: time(m.due) })}</span> : null}
        {m.waits === null && m.held === null && m.due_item !== null ? (
          <span>{t('matter_due_item', { item: m.due_item })}</span>
        ) : null}
      </div>
    </li>
  );
}

/// The architect's queue: the matters waiting for it, in the order they came; then those held for the owner; then
/// those not due yet.
function Queue({ queue, onAsk }: { queue: Matter[]; onAsk: (id: number) => void }) {
  const now = useNow(false);
  if (queue.length === 0) return <p className="muted">{t('arch_queue_none')}</p>;
  const { waiting, held: holding, later } = queueParts(queue);
  const list = (ms: Matter[]) => (
    <ul className="arch-matters">
      {ms.map((m) => (
        <MatterRow key={m.id} m={m} now={now} onAsk={onAsk} />
      ))}
    </ul>
  );
  return (
    <>
      {waiting.length > 0 ? list(waiting) : <p className="muted">{t('arch_queue_none_waiting')}</p>}
      {holding.length > 0 ? (
        <section>
          <div className="works-label">{t('arch_queue_held', { n: holding.length })}</div>
          {list(holding)}
        </section>
      ) : null}
      {later.length > 0 ? (
        <section>
          <div className="works-label">{t('arch_queue_later', { n: later.length })}</div>
          {list(later)}
        </section>
      ) : null}
    </>
  );
}

/// The architect's asks of the owner: those waiting first, with their forms; those answered folded under them.
function Asks({ asks, onRound }: { asks: OwnerAsk[]; onRound: OpenRound }) {
  if (asks.length === 0) return <p className="muted">{t('arch_asks_none')}</p>;
  const waiting = asks.filter((x) => x.answer === null);
  const answered = asks.filter((x) => x.answer !== null).reverse();
  const one = (x: OwnerAsk) => (
    <div key={x.id} id={`ask-${x.id}`} className="boxed">
      <span className="work-item-head">
        <strong>{t('ask_head', { id: x.id })}</strong>
        <span className="badge">{askKindWords(x)}</span>
        <span className="muted">{x.answer === null ? t('ask_waits') : t('ask_answered')}</span>
        <RoundLink work={x.work} round={x.round} onRound={onRound} />
      </span>
      <ArchitectAsk a={x} />
    </div>
  );
  return (
    <>
      {waiting.length === 0 ? <p className="muted">{t('arch_asks_none_waiting')}</p> : waiting.map(one)}
      {answered.length > 0 ? (
        <details className="boxed work-old">
          <summary>{t('work_answered_asks', { n: answered.length })}</summary>
          {answered.map(one)}
        </details>
      ) : null}
    </>
  );
}

/// The policy: the one being set, while a wake sets one (the latest study, the text with its probes and the readings
/// of it); the policy in force, with when it took effect; and the plan that carries it out.
function Policy({ a, onRound }: { a: ArchitectRead; onRound: OpenRound }) {
  const s = a.setting;
  const p = a.policy;
  return (
    <>
      {s !== null ? (
        <section className="arch-part arch-setting">
          <div className="decision-label">{t('policy_being_set')}</div>
          {s.study !== null ? (
            <Fold
              open={s.draft === null}
              title={
                <>
                  {t('policy_study')} · {roundName(s.study.work, s.study.round)}
                </>
              }
            >
              <StudyView value={s.study.value} />
            </Fold>
          ) : null}
          {s.draft !== null ? (
            <>
              <div className="work-item-head">
                <span className="decision-label">{t('policy_draft')}</span>
                <RoundLink work={s.work} round={s.draft.round} onRound={onRound} />
              </div>
              <PolicyText value={s.draft.policy} />
              <Fold title={t('policy_probes', { n: Array.isArray(s.draft.probes) ? s.draft.probes.length : 0 })}>
                <ProbesView value={s.draft.probes} />
              </Fold>
              {s.draft.readings.map((r) => (
                <ReadingView key={r.round} round={roundName(s.work, r.round)} reading={r.reading} verdict={r.verdict} />
              ))}
            </>
          ) : null}
        </section>
      ) : null}
      <section className="arch-part">
        <div className="work-item-head">
          <span className="decision-label">{t('policy_in_force')}</span>
          {p !== null ? (
            <span className="muted">
              {t('policy_took_effect', { at: time(p.at) })} <RoundLink work={p.work} round={p.round} onRound={onRound} />
            </span>
          ) : null}
        </div>
        {p !== null ? <PolicyText value={p.policy} /> : <p className="muted">{t('policy_none')}</p>}
      </section>
      <section className="arch-part">
        <div className="decision-label">{t('plan_title')}</div>
        <PlanView segments={a.plan.segments} gates={a.plan.gates} budgets={a.plan.budgets} />
      </section>
    </>
  );
}

/// The architect's panel, on top of the column docked at the right of its chart, in tabs, each saying how many of its
/// things there are and, for the asks, how many wait for the owner.
function Panel({ route, a, d, g, go, onRound }: {
  route: ArchitectRoute;
  a: ArchitectRead;
  /// The wake shown, in full; none before the first wake.
  d: WorkDetail | null;
  g: CompiledGraph;
  go: (to: Partial<ArchitectRoute>) => void;
  onRound: OpenRound;
}) {
  const status = useStatus();
  // What to bring into view once its tab is shown: the ask the owner was sent to; else a tab shows from its top.
  const want = useRef<string | null>(null);
  const body = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (want.current === null) body.current?.scrollTo({ top: 0 });
  }, [route.tab]);
  useEffect(() => {
    if (want.current !== null && bringIntoView(want.current)) want.current = null;
  });
  const onAsk = (id: number) => {
    want.current = `ask-${id}`;
    go({ tab: 'asks' });
  };
  const waiting = a.asks.filter((x) => x.answer === null).length;
  const live = new Set(a.wakes.filter((w) => !overWake(w.work)).map((w) => w.work.id));
  const tabs: Array<{ tab: ArchitectTab; label: string; n: number | null; chip: string | null }> = [
    { tab: 'asks', label: t('arch_tab_asks'), n: a.asks.length, chip: waiting > 0 ? t('open_asks_chip', { n: waiting }) : null },
    { tab: 'queue', label: t('arch_tab_queue'), n: queueParts(a.queue).waiting.length, chip: null },
    { tab: 'decisions', label: t('arch_tab_decisions'), n: a.log.length, chip: null },
    { tab: 'policy', label: t('arch_tab_policy'), n: null, chip: null },
    { tab: 'understanding', label: t('arch_tab_understanding'), n: null, chip: null },
    { tab: 'rounds', label: t('rounds'), n: d?.rounds.length ?? null, chip: null },
  ];
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
              aria-selected={x.tab === route.tab}
              onClick={() => {
                want.current = null;
                go({ tab: x.tab });
              }}
            >
              {x.label}
              {x.n !== null ? <span className="tab-n">{x.n}</span> : null}
              {x.chip !== null ? (
                <span className="chip warn" title={t('open_asks_n', { n: waiting })}>
                  {x.chip}
                </span>
              ) : null}
            </button>
          ))}
        </div>
      </div>
      <div className="drawer-body" role="tabpanel" ref={body}>
        {route.tab === 'asks' ? <Asks asks={a.asks} onRound={onRound} /> : null}
        {route.tab === 'queue' ? <Queue queue={a.queue} onAsk={onAsk} /> : null}
        {route.tab === 'decisions' ? (
          a.log.length === 0 ? (
            <p className="muted">{t('arch_log_none')}</p>
          ) : (
            a.log.map((e, i) => (
              <LogEntryView
                key={`${e.kind}-${e.work}-${e.round}`}
                e={e}
                open={i === 0}
                live={live.has(e.work)}
                onRound={onRound}
              />
            ))
          )
        ) : null}
        {route.tab === 'policy' ? <Policy a={a} onRound={onRound} /> : null}
        {route.tab === 'understanding' ? <UnderstandingView u={a.understanding} onRound={onRound} /> : null}
        {route.tab === 'rounds' ? (
          d === null ? (
            <p className="muted">{a.wakes.length === 0 ? t('arch_never_woke') : t('loading')}</p>
          ) : (
            <>
              <Next
                d={d}
                onRound={(r) => go({ wake: d.work.id, round: r, cell: null })}
                onCard={() => undefined}
                onAsk={onAsk}
                stepPath="/api/v1/architect/step"
              />
              <Timeline
                work={d.work.id}
                rounds={d.rounds}
                g={g}
                stale={status !== null && status.running !== d.work.id}
                selected={route.wake === d.work.id ? route.round : null}
                onRound={(x) => go({ wake: d.work.id, round: x.n, cell: null })}
              />
            </>
          )
        ) : null}
      </div>
    </div>
  );
}

/// The architect's graph filling the window, with the wake shown placed on it, and the docked column.
function Chart({ route, a, wake, d, compiled, view, go }: {
  route: ArchitectRoute;
  a: ArchitectRead;
  wake: Wake | null;
  d: WorkDetail | null;
  compiled: Compiled;
  view: FlowView;
  go: (to: Partial<ArchitectRoute>) => void;
}) {
  const status = useStatus();
  const g = compiled.graphs.find((x) => x.name === GRAPH);
  const v = view.graphs.find((x) => x.name === GRAPH);
  const graphs = useMemo(() => (g && v ? [{ g, view: v }] : []), [g, v]);
  if (!g || !v) return <p className="muted page">{t('arch_no_graph')}</p>;
  const id = wake?.work.id ?? null;
  const roundCell = route.round !== null && d !== null ? (d.rounds.find((x) => x.n === route.round)?.cell ?? null) : null;
  // A round of any wake opens on the chart of that wake.
  const onRound: OpenRound = (work, round) => go({ wake: work, round, cell: null });
  return (
    <FlowScreen
      key={id ?? 'none'}
      compiled={compiled}
      graphs={graphs}
      legend={d !== null ? 'work' : 'flow'}
      marks={[d !== null ? workMarks(g, v, d.work, d.rounds, course(d.work, status)) : BARE]}
      cell={route.cell}
      onCell={(cell) => go({ cell, round: null })}
      start={roundCell ?? d?.work.at ?? null}
      work={id ?? undefined}
      rounds={d?.rounds}
      round={route.wake === id ? route.round : null}
      onRound={(x) => go(x !== null ? { wake: id, round: x, cell: null } : { round: null })}
      renderRound={id !== null && d !== null ? (x) => <RoundView work={id} n={x} rounds={d.rounds} /> : undefined}
      side={<Panel route={route} a={a} d={d} g={g} go={go} onRound={onRound} />}
    />
  );
}

function Body({ route, a }: { route: ArchitectRoute; a: ArchitectRead }) {
  const status = useStatus();
  const wake = shownWake(a.wakes, route.wake);
  const id = wake?.work.id ?? null;
  const detail = useResource<WorkDetail>(id !== null ? `work:${id}` : null, id !== null ? `/api/v1/works/${id}` : null);
  const version = wake?.work.flow_version ?? status?.flow_version ?? null;
  const compiled = useResource<Compiled>(null, version !== null ? `/api/v1/flows/${version}` : null);
  const view = useResource<FlowView>(null, version !== null ? `/api/v1/flows/${version}/view` : null);
  const d = detail.data !== undefined && detail.data.work.id === id ? detail.data : null;
  const going = a.wakes.find((w) => !overWake(w.work)) ?? null;
  const current = going !== null && d !== null && d.work.id === going.work.id ? d.work : (going?.work ?? null);
  const [pop, setPop] = useState<'say' | 'wakes' | null>(null);
  const go = (to: Partial<ArchitectRoute>) => navigate(href({ ...route, ...to }), true);
  // A link to an ask on this page brings it into view once the page is there.
  const ready = compiled.data !== undefined && view.data !== undefined;
  useEffect(() => {
    if (!ready) return;
    const at = window.location.hash.slice(1);
    if (at !== '') bringIntoView(at);
  }, [ready]);
  const onAsk = (ask: number) => {
    go({ tab: 'asks' });
    window.setTimeout(() => bringIntoView(`ask-${ask}`), 0);
  };
  return (
    <div className="arch-screen">
      <Line
        a={a}
        shown={wake}
        current={current}
        onTab={(tab) => go({ tab })}
        onAsk={onAsk}
        pop={pop}
        setPop={setPop}
      />
      {pop === 'say' ? <SayBox onClose={() => setPop(null)} /> : null}
      {pop === 'wakes' ? <WakeList a={a} shown={id} tab={route.tab} onClose={() => setPop(null)} /> : null}
      <div className="screen">
        <Loaded r={compiled}>
          {(c) => (
            <Loaded r={view}>
              {(v) => <Chart route={route} a={a} wake={wake} d={d} compiled={c} view={v} go={go} />}
            </Loaded>
          )}
        </Loaded>
      </div>
    </div>
  );
}

export function Architect({ route }: { route: ArchitectRoute }) {
  const a = useResource<ArchitectRead>('architect', '/api/v1/architect');
  return (
    <div className="works-screen">
      <Loaded r={a}>{(data) => <Body route={route} a={data} />}</Loaded>
    </div>
  );
}
