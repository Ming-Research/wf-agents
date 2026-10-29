// A project's architect's page, laid out as a work's page, since an architect runs a long time and the owner comes back
// to it. Its line says, with more than one project, whose architect it is, with the switch to another's; how the
// architect stands in words (asleep since when and how many matters are in its queue; or awake: the question it is
// at, the matter it handles, a round running and for how long, or what holds it back), what stops the project's new
// works from starting, the architect's Start, Pause and Step with the session they run its next round in, and the
// switch between its wakes (when each woke, why, how many matters it handled, what it cost). Under the line, its graph
// fills the window with the wake shown placed on it as a work is (the wake going on, else the latest, else the bare
// graph), and the column docked at its right holds the architect's panel, where what is open on the chart takes the
// place of its list, with a way back to it. The panel's tabs, on one line: the owner's conversation with the
// architect, first, with how many of its messages are unread and whether something in it waits for the owner (the same
// conversation the messages view shows: what waits pinned on top, the owner's words and what the architect told the
// owner and asked, in time order, the newest at the bottom; whether the owner's words still wait for the architect,
// and why; and a box that says more); its asks of the owner, those waiting first with their forms; its queue, the
// matters in it, then those an ask holds, then, folded, those it handled with what became of each; the item changes
// its batches made; the project's observations; the policy in force and the one being set, with the asks that read
// and approve it; where things stand as it understands them; and the rounds of the wake shown. Each part names the cell
// that wrote it.

import { useEffect, useMemo, useRef, useState } from 'react';

import type {
  Architect as ArchitectRead,
  Compiled,
  CompiledGraph,
  Conversation,
  ConversationEntry,
  FlowView,
  Handled,
  ItemChange,
  Matter,
  Observation,
  OwnerAsk,
  Wake,
  WorkDetail,
  WorkSummary,
} from '../api';
import {
  architectState,
  askTab,
  observationsShown,
  outcomes,
  overWake,
  queueParts,
  shownWake,
  wordsWait,
} from '../architect';
import { course, heldName, Loaded } from '../components';
import { Control, Controls, held, runningWords, SessionChoice, useActions, useNewSession } from '../course';
import { FlowScreen } from '../flow/FlowScreen';
import { cost, duration, roundName, time } from '../format';
import { postJson } from '../http';
import { Fold } from '../item/Shapes';
import { isObj, Labelled, str, words } from '../labelled';
import { Markdown } from '../markdown';
import { reread, useResource, useStatus } from '../live';
import {
  type ArchitectRoute,
  type ArchitectTab,
  architectHref,
  forProject,
  href,
  Link,
  navigate,
  workHref,
} from '../route';
import { t, type Key } from '../text';
import { ArchitectAsk, askKindWords } from './ArchitectAsk';
import {
  ChangeView,
  type OpenRound,
  PolicyText,
  ProbesView,
  ReadingView,
  StudyView,
  UnderstandingView,
  Wrote,
} from './ArchitectViews';
import { bringIntoView } from './Decide';
import { ConversationView } from './Messages';
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

/// An architect's Step: one round of the wake going on, or, when none goes on and a matter waits, a new wake and its
/// first round, resuming its session or in a new one for its wake as the owner chose; and its Start and Pause while no
/// wake goes on. Step asks first, as a real round runs on the owner's account.
function useStep(project: string) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [newSession, setNewSession] = useNewSession('architect');
  const send = (path: string, body: object) => {
    setBusy(true);
    setError(null);
    postJson(path, body).then(
      () => {
        setBusy(false);
        setNewSession(false);
        reread('architect', 'works', 'work:*');
      },
      (e: unknown) => {
        setBusy(false);
        setError(e instanceof Error ? e.message : String(e));
      },
    );
  };
  const start = () => send(forProject('/api/v1/architect/start', project), { new_session: newSession });
  const pause = () => send(forProject('/api/v1/architect/pause', project), {});
  const step = (ask: string) => {
    if (window.confirm(ask)) send(forProject('/api/v1/architect/step', project), { new_session: newSession });
  };
  return { busy, error, step, start, pause, newSession, setNewSession };
}

/// How the wake going on goes on, on one line: a round running and for how long, or what holds it back, or what it
/// waits for, linked to the ask; then the question it is at and the matter it handles; and its Start, Pause and Step.
function Awake({ w, project, handling, onAsk }: {
  w: WorkSummary;
  project: string;
  handling: string;
  onAsk: (id: number) => void;
}) {
  const status = useStatus();
  const a = useActions(w, status, true);
  const s = useStep(project);
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
          <span className={`badge ${heldName(c).problem ? 'state-stopped' : 'state-paused'}`}>{heldName(c).name}</span>
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

const HOLDS: Record<string, Key> = { architect: 'hold_architect', set_straight: 'hold_set_straight', policy: 'hold_policy' };

/// The line above an architect's chart: with more than one project, whose architect it is and the switch to another's;
/// how it stands, what stops the project's new works, its controls, and the switch between its wakes.
function Line({ a, shown, current, onTab, onAsk, pop, setPop }: {
  a: ArchitectRead;
  shown: Wake | null;
  /// The wake going on, as freshly read as the page has it.
  current: WorkSummary | null;
  onTab: (tab: ArchitectTab) => void;
  onAsk: (id: number) => void;
  pop: 'wakes' | null;
  setPop: (p: 'wakes' | null) => void;
}) {
  const status = useStatus();
  const state = architectState(a.wakes, a.queue);
  const s = useStep(a.project);
  const handling = state.kind === 'awake' && state.handling !== null ? state.handling.what : '';
  const started = status !== null && status.architects_started.includes(a.project);
  // With no wake going on, the architect is held back while every work is paused and the owner has not started it:
  // then its Start, and its Step, which wakes it while a matter waits; else the engine wakes it by itself. Started, it
  // wakes by itself whatever the global state, until paused. Step and the session choice beside it are shown greyed
  // while they cannot be used.
  const asleep = state.kind !== 'awake' && status !== null;
  const canStart = asleep && status.paused && !started;
  const canPause = asleep && started;
  const wakeWhyNot = !canStart
    ? t('work_step_goes_on')
    : state.waiting === 0
      ? t('arch_queue_none_waiting')
      : status.stepping !== null || status.halt !== null
        ? t('work_step_later')
        : null;
  return (
    <div className="work-line arch-line">
      {a.projects.length > 1 ? (
        <select
          className="arch-project"
          value={a.project}
          title={t('arch_project_hint')}
          onChange={(e) => navigate(architectHref({ project: e.target.value }))}
        >
          {a.projects.map((p) => (
            <option key={p} value={p}>
              {t('arch_of_project', { project: p })}
            </option>
          ))}
        </select>
      ) : null}
      {state.kind === 'awake' && current !== null ? (
        <Awake w={current} project={a.project} handling={handling} onAsk={onAsk} />
      ) : (
        <span className="work-state">
          <strong>{state.kind === 'never' ? t('arch_never') : t('arch_asleep', { at: time(state.kind === 'asleep' ? state.since : null) })}</strong>
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
          {h.why !== null ? `: ${h.why}` : null}
        </span>
      ))}
      {canPause ? <span className="badge state-running">{t('arch_started')}</span> : null}
      {canStart ? <Control icon="start" hint={t('arch_start_hint')} onClick={s.start} disabled={s.busy} /> : null}
      {canPause ? <Control icon="pause" hint={t('arch_pause_hint')} onClick={s.pause} disabled={s.busy} /> : null}
      {asleep ? (
        <Control
          icon="step"
          hint={wakeWhyNot ?? t('arch_wake_hint')}
          onClick={() => s.step(t('arch_wake_confirm'))}
          disabled={s.busy || wakeWhyNot !== null}
        />
      ) : null}
      {asleep ? <SessionChoice on={s.newSession} set={s.setNewSession} disabled={s.busy || !canStart} /> : null}
      {s.error !== null ? <span className="bad why">{t('request_failed', { error: s.error })}</span> : null}
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

const WAITS: Record<string, Key> = {
  paused: 'talk_waits_paused',
  held: 'talk_waits_held',
  asleep: 'talk_waits_asleep',
  awake: 'talk_waits_awake',
};

/// The name of each tab of the architect's panel.
const TAB_NAMES: Record<ArchitectTab, Key> = {
  conversation: 'talk_title',
  asks: 'arch_tab_asks',
  queue: 'arch_tab_queue',
  changes: 'arch_tab_changes',
  observations: 'arch_tab_observations',
  policy: 'arch_tab_policy',
  understanding: 'arch_tab_understanding',
  rounds: 'rounds',
};

/// The architect's wakes over time, newest first: when each woke, why, how many matters it handled, its rounds and
/// cost; a click shows that wake, and the one shown by default follows the wake going on again.
function WakeList({ a, shown, tab, project, onClose }: {
  a: ArchitectRead;
  shown: number | null;
  tab: ArchitectTab;
  /// The project the address names, kept as it is.
  project: string | null;
  onClose: () => void;
}) {
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
              const whyAll = x.woke_for.map((m) => `${words(m.kind)}${m.detail !== null ? ` (${words(m.detail)})` : ''}: ${m.what}`);
              return (
                <tr
                  key={x.work.id}
                  className={`clickable${x.work.id === shown ? ' selected' : ''}`}
                  onClick={() => {
                    navigate(architectHref({ project, wake: x.work.id === followed ? null : x.work.id, tab }));
                    onClose();
                  }}
                >
                  <td className="nowrap">
                    #{x.work.id}
                    {!overWake(x.work) ? <span className="badge state-running">{t('arch_wake_going')}</span> : null}
                  </td>
                  <td className="nowrap">{time(x.work.started_at)}</td>
                  <td className="clip" title={whyAll.join('\n')}>
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

/// One matter in an architect's queue: its kind, and what more its kind says, as a chip coloured by who raised it;
/// what it is; whether it is urgent; its item; and who waits for it and how long; or the ask that holds it, or that the
/// owner answered, so that it goes back to the cell that asked.
function MatterRow({ m, now, onAsk }: { m: Matter; now: number; onAsk: (id: number) => void }) {
  const since = duration(Math.max(0, now - m.at));
  return (
    <li className="arch-matter" id={`matter-${m.id}`}>
      <div className="work-item-head">
        <span
          className={`badge kind src-${m.source}`}
          title={SOURCES[m.source] !== undefined ? t(SOURCES[m.source]) : undefined}
        >
          {words(m.kind)}
          {m.detail !== null ? ` · ${words(m.detail)}` : ''}
        </span>
        {m.urgent ? <span className="badge verdict-bad">{t('matter_urgent')}</span> : null}
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
        {m.asked !== null ? (
          <button type="button" className="link" onClick={() => onAsk(m.asked ?? 0)}>
            {t(m.answered ? 'matter_answered' : 'matter_held_ask', { id: m.asked, cell: m.resume_cell ?? '' })}
          </button>
        ) : null}
      </div>
    </li>
  );
}

/// A matter handled, as a block: its kind and what it was, the cell and round that closed it, and one line for each
/// kind of outcome it had, each leading to the tab that holds the rest of it.
function HandledBlock({ h, onRound, onTab }: { h: Handled; onRound: OpenRound; onTab: (tab: ArchitectTab) => void }) {
  return (
    <li className="arch-block" id={`matter-${h.matter}`}>
      <div className="arch-block-h">
        <span className="badge">
          {words(h.kind)}
          {h.detail !== null ? ` · ${words(h.detail)}` : ''}
        </span>
        <span className="arch-matter-what">{h.what}</span>
        <Wrote cell={h.cell} work={h.work} round={h.round} onRound={onRound} />
      </div>
      {outcomes(h).map((o) => (
        <p key={o.key} className="arch-outcome" title={o.text !== '' ? o.text : undefined}>
          {o.tab !== null ? (
            <button type="button" className="link" onClick={() => onTab(o.tab ?? 'queue')}>
              {o.label}
            </button>
          ) : (
            <span className="arch-block-label">{o.label}</span>
          )}{' '}
          {o.text}
        </p>
      ))}
    </li>
  );
}

/// An architect's queue: the matters in it, in the order they came; then those an ask of the owner holds; then, folded,
/// the matters it handled, newest first, with what became of each.
function Queue({ queue, handled, onAsk, onRound, onTab }: {
  queue: Matter[];
  handled: Handled[];
  onAsk: (id: number) => void;
  onRound: OpenRound;
  onTab: (tab: ArchitectTab) => void;
}) {
  const now = useNow(false);
  const { waiting, held: holding } = queueParts(queue);
  const list = (ms: Matter[]) => (
    <ul className="arch-matters">
      {ms.map((m) => (
        <MatterRow key={m.id} m={m} now={now} onAsk={onAsk} />
      ))}
    </ul>
  );
  return (
    <>
      {queue.length === 0 ? <p className="muted">{t('arch_queue_none')}</p> : null}
      {queue.length > 0 && waiting.length === 0 ? <p className="muted">{t('arch_queue_none_waiting')}</p> : null}
      {waiting.length > 0 ? list(waiting) : null}
      {holding.length > 0 ? (
        <section>
          <div className="works-label">{t('arch_queue_held', { n: holding.length })}</div>
          {list(holding)}
        </section>
      ) : null}
      {handled.length > 0 ? (
        <Fold title={t('handled_title', { n: handled.length })}>
          <ul className="arch-matters">
            {handled.map((h) => (
              <HandledBlock key={h.matter} h={h} onRound={onRound} onTab={onTab} />
            ))}
          </ul>
        </Fold>
      ) : null}
    </>
  );
}

/// The item changes an architect's batches made or dropped, newest first: each change as handed in, with why; what it
/// made, or for one dropped, what the review still found; the matter it came from and the round that handed it in.
function Changes({ changes, onRound }: { changes: ItemChange[]; onRound: OpenRound }) {
  if (changes.length === 0) return <p className="muted">{t('arch_changes_none')}</p>;
  return (
    <ul className="arch-matters">
      {changes.map((c) => (
        <li key={c.id} className="boxed">
          <div className="work-item-head">
            <span className={`badge ${c.state === 'made' ? 'verdict-good' : 'verdict-bad'}`}>{words(c.state)}</span>
            <span className="muted">{t('change_matter', { n: c.matter })}</span>
            <Wrote cell={c.cell} work={c.work} round={c.round} onRound={onRound} />
          </div>
          <ChangeView value={c.change} />
          {c.outcome !== null ? (
            <div className="field">
              <span className="decision-label">{t(c.state === 'made' ? 'change_made' : 'change_dropped')}</span>
              <Labelled value={c.outcome} />
            </div>
          ) : null}
        </li>
      ))}
    </ul>
  );
}

/// One observation: what it is; its tags; where it came from and how many times it was reported; what it refers to;
/// the item that took it in; and, closed, why; open, the owner's Close, which asks why.
function ObservationRow({ o }: { o: Observation }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const close = () => {
    const why = window.prompt(t('observation_close_why'));
    if (why === null || why.trim() === '') return;
    setBusy(true);
    setError(null);
    postJson(`/api/v1/observations/${o.id}/close`, { why: why.trim() }).then(
      () => reread('architect'),
      (e: unknown) => {
        setBusy(false);
        setError(e instanceof Error ? e.message : String(e));
      },
    );
  };
  return (
    <li className="boxed">
      <div className="work-item-head">
        <strong>{t('observation_head', { id: o.id })}</strong>
        {o.tags.map((x) => (
          <span key={x} className="badge">
            {x}
          </span>
        ))}
        <span className="muted">{t('observation_reported', { n: o.times_reported })}</span>
        {o.closed !== null ? (
          <span className="badge">{t('observation_closed', { why: words(o.closed) })}</span>
        ) : (
          <button type="button" className="link" disabled={busy} onClick={close}>
            {t('observation_close')}
          </button>
        )}
      </div>
      <Markdown text={o.what} />
      <div className="muted arch-matter-meta">
        <span>{t('observation_source', { source: o.source })}</span>
        {o.refs.length > 0 ? <span>{t('observation_refs', { refs: o.refs.join(', ') })}</span> : null}
        {o.folded_into !== null ? (
          <Link to={`/items/${o.folded_into}`}>{t('observation_folded', { item: o.folded_into })}</Link>
        ) : null}
        {o.created !== null ? <span>{t('observation_at', { at: time(o.created) })}</span> : null}
      </div>
      {o.closed_why !== null ? <p className="muted">{o.closed_why}</p> : null}
      {error !== null ? <p className="bad">{t('request_failed', { error })}</p> : null}
    </li>
  );
}

/// A project's observations: by tag, with what each tag covers, searchable, the open ones and, when asked for, those
/// closed.
function Observations({ a }: { a: ArchitectRead }) {
  const [tag, setTag] = useState('');
  const [search, setSearch] = useState('');
  const [closed, setClosed] = useState(false);
  const shown = observationsShown(a.observations, tag, search, closed);
  const covers = a.tags.find((x) => x.name === tag)?.desc ?? '';
  return (
    <>
      <div className="bar-pop-row">
        <select value={tag} onChange={(e) => setTag(e.target.value)} title={covers}>
          <option value="">{t('observation_every_tag')}</option>
          {a.tags.map((x) => (
            <option key={x.name} value={x.name} title={x.desc}>
              {x.name}
            </option>
          ))}
        </select>
        <input
          type="search"
          value={search}
          placeholder={t('observation_search')}
          onChange={(e) => setSearch(e.target.value)}
        />
        <label>
          <input type="checkbox" checked={closed} onChange={(e) => setClosed(e.target.checked)} />{' '}
          {t('observation_show_closed')}
        </label>
      </div>
      {covers !== '' ? <p className="muted">{covers}</p> : null}
      {shown.length === 0 ? (
        <p className="muted">{t('observation_none')}</p>
      ) : (
        <ul className="arch-matters">
          {shown.map((o) => (
            <ObservationRow key={o.id} o={o} />
          ))}
        </ul>
      )}
    </>
  );
}

/// The architect's asks of the owner: those waiting first, with their forms, each under the cell that put it; those
/// answered folded under them. The Asks tab holds those its matters' paths put, the Policy tab the study and the
/// approval of a new policy.
function Asks({ asks, onRound, none = true }: { asks: OwnerAsk[]; onRound: OpenRound; none?: boolean }) {
  if (asks.length === 0) return none ? <p className="muted">{t('arch_asks_none')}</p> : null;
  const waiting = asks.filter((x) => x.answer === null);
  const answered = asks.filter((x) => x.answer !== null).reverse();
  const one = (x: OwnerAsk) => (
    <div key={x.id} id={`ask-${x.id}`} className="boxed">
      <span className="work-item-head">
        <strong>{t('ask_head', { id: x.id })}</strong>
        <span className="badge">{askKindWords(x)}</span>
        <span className="muted">{x.answer === null ? t('ask_waits') : t('ask_answered')}</span>
        <Wrote cell={isObj(x.body) ? str(x.body.cell) : ''} work={x.work} round={x.round} onRound={onRound} />
      </span>
      <ArchitectAsk a={x} />
    </div>
  );
  return (
    <>
      {waiting.length === 0 ? (none ? <p className="muted">{t('arch_asks_none_waiting')}</p> : null) : waiting.map(one)}
      {answered.length > 0 ? (
        <details className="boxed work-old">
          <summary>{t('work_answered_asks', { n: answered.length })}</summary>
          {answered.map(one)}
        </details>
      ) : null}
    </>
  );
}

/// The policy, all that the cells that set a policy produced, each under the cell and round that wrote it: its asks of
/// the owner (the study to read, the policy to approve) with their forms; the one being set, while a wake sets one (the
/// latest study, the text with its probes and the readings of it); and the policy in force, with when it took effect.
function Policy({ a, asks, onRound }: { a: ArchitectRead; asks: OwnerAsk[]; onRound: OpenRound }) {
  const s = a.setting;
  const p = a.policy;
  return (
    <>
      <Asks asks={asks} onRound={onRound} none={false} />
      {s !== null ? (
        <section className="arch-part arch-setting">
          <div className="decision-label">{t('policy_being_set')}</div>
          {s.study !== null ? (
            <Fold
              open={s.draft === null}
              title={
                <>
                  {t('policy_study')} · {s.study.cell} · {roundName(s.study.work, s.study.round)}
                </>
              }
            >
              <Wrote cell={s.study.cell} work={s.study.work} round={s.study.round} onRound={onRound} />
              <StudyView value={s.study.value} />
            </Fold>
          ) : null}
          {s.draft !== null ? (
            <>
              <div className="work-item-head">
                <span className="decision-label">{t('policy_draft')}</span>
                <Wrote cell={s.draft.cell} work={s.work} round={s.draft.round} onRound={onRound} />
              </div>
              <PolicyText value={s.draft.policy} />
              <Fold title={t('policy_probes', { n: Array.isArray(s.draft.probes) ? s.draft.probes.length : 0 })}>
                <ProbesView value={s.draft.probes} />
              </Fold>
              {s.draft.readings.map((r) => (
                <ReadingView
                  key={r.round}
                  from={<Wrote cell={r.cell} work={s.work} round={r.round} onRound={onRound} />}
                  reading={r.reading}
                  verdict={r.verdict}
                />
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
              {t('policy_took_effect', { at: time(p.at) })} <Wrote cell={p.cell} work={p.work} round={p.round} onRound={onRound} />
            </span>
          ) : null}
        </div>
        {p !== null ? <PolicyText value={p.policy} /> : <p className="muted">{t('policy_none')}</p>}
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
  // An ask is in the tab of the stage that put it; a matter in the queue, among those waiting or those handled.
  const onAsk = (id: number) => {
    const ask = a.asks.find((x) => x.id === id);
    want.current = `ask-${id}`;
    go({ tab: ask !== undefined ? askTab(ask) : 'asks' });
  };
  const onMatter = (id: number) => {
    want.current = `matter-${id}`;
    go({ tab: 'queue' });
  };
  const policyAsks = a.asks.filter((x) => askTab(x) === 'policy');
  const otherAsks = a.asks.filter((x) => askTab(x) === 'asks');
  const waiting = otherAsks.filter((x) => x.answer === null).length;
  const policyWaiting = policyAsks.filter((x) => x.answer === null).length;
  // The conversation's tab says how many of its messages are unread and how many things in it wait for the owner, as
  // the list of conversations has them.
  const key = `architect:${a.project}`;
  const all = useResource<Conversation[]>('messages', '/api/v1/conversations');
  const talk = all.data?.find((c) => c.key === key) ?? null;
  const why = (entries: ConversationEntry[]) => {
    const waits = wordsWait(entries, architectState(a.wakes, a.queue), status, a.project);
    return waits !== null ? t(WAITS[waits]) : null;
  };
  const tabs: Array<{ tab: ArchitectTab; label: string; n: number | null; chip: string | null; hint: string }> = [
    {
      tab: 'conversation',
      label: t('talk_title'),
      n: talk !== null && talk.unread > 0 ? talk.unread : null,
      chip: talk !== null && talk.waiting > 0 ? t('conv_waits_chip', { n: talk.waiting }) : null,
      hint: t('conv_waits_n', { n: talk?.waiting ?? 0 }),
    },
    {
      tab: 'asks',
      label: t('arch_tab_asks'),
      n: otherAsks.length,
      chip: waiting > 0 ? t('open_asks_chip', { n: waiting }) : null,
      hint: t('open_asks_n', { n: waiting }),
    },
    { tab: 'queue', label: t('arch_tab_queue'), n: queueParts(a.queue).waiting.length, chip: null, hint: '' },
    { tab: 'changes', label: t('arch_tab_changes'), n: a.changes.length, chip: null, hint: '' },
    {
      tab: 'observations',
      label: t('arch_tab_observations'),
      n: a.observations.filter((o) => o.closed === null).length,
      chip: null,
      hint: '',
    },
    {
      tab: 'policy',
      label: t('arch_tab_policy'),
      n: null,
      chip: policyWaiting > 0 ? t('open_asks_chip', { n: policyWaiting }) : null,
      hint: t('open_asks_n', { n: policyWaiting }),
    },
    { tab: 'understanding', label: t('arch_tab_understanding'), n: null, chip: null, hint: '' },
    { tab: 'rounds', label: t('rounds'), n: d?.rounds.length ?? null, chip: null, hint: '' },
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
                <span className="chip warn" title={x.hint}>
                  {x.chip}
                </span>
              ) : null}
            </button>
          ))}
        </div>
      </div>
      <div className="drawer-body" role="tabpanel" ref={body}>
        {route.tab === 'conversation' ? <ConversationView conversation={key} note={why} /> : null}
        {route.tab === 'asks' ? <Asks asks={otherAsks} onRound={onRound} /> : null}
        {route.tab === 'queue' ? (
          <Queue queue={a.queue} handled={a.handled} onAsk={onAsk} onRound={onRound} onTab={(tab) => go({ tab })} />
        ) : null}
        {route.tab === 'changes' ? <Changes changes={a.changes} onRound={onRound} /> : null}
        {route.tab === 'observations' ? <Observations a={a} /> : null}
        {route.tab === 'policy' ? <Policy a={a} asks={policyAsks} onRound={onRound} /> : null}
        {route.tab === 'understanding' ? (
          <UnderstandingView u={a.understanding} onRound={onRound} onMatter={onMatter} />
        ) : null}
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
                stepPath={forProject('/api/v1/architect/step', a.project)}
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

/// The architect's graph filling the window, with the wake shown placed on it, and the docked column: the panel in its
/// upper half, the conversation in its lower half.
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
      list={t(TAB_NAMES[route.tab])}
      side={(open) => open ?? <Panel route={route} a={a} d={d} g={g} go={go} onRound={onRound} />}
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
  const [pop, setPop] = useState<'wakes' | null>(null);
  const go = (to: Partial<ArchitectRoute>) => navigate(href({ ...route, ...to }), true);
  // A link to an ask on this page brings it into view once the page is there.
  const ready = compiled.data !== undefined && view.data !== undefined;
  useEffect(() => {
    if (!ready) return;
    const at = window.location.hash.slice(1);
    if (at !== '') bringIntoView(at);
  }, [ready]);
  const onAsk = (ask: number) => {
    const asked = a.asks.find((x) => x.id === ask);
    go({ tab: asked !== undefined ? askTab(asked) : 'asks' });
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
      {pop === 'wakes' ? (
        <WakeList a={a} shown={id} tab={route.tab} project={route.project} onClose={() => setPop(null)} />
      ) : null}
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

/// What this browser kept of the split and the fold of the conversation under the architect's panel, before the
/// conversation became a tab of the panel; cleared when the page opens. This goes once the browsers that kept them
/// have opened the page, by the first version's review.
const KEPT_BEFORE = ['wf-agents.arch-split', 'wf-agents.arch-talk-folded'];

export function Architect({ route }: { route: ArchitectRoute }) {
  const a = useResource<ArchitectRead>('architect', forProject('/api/v1/architect', route.project));
  useEffect(() => {
    try {
      for (const key of KEPT_BEFORE) localStorage.removeItem(key);
    } catch {
      // Nothing is kept where the browser lets nothing be.
    }
  }, []);
  return (
    <div className="works-screen">
      <Loaded r={a}>{(data) => <Body route={route} a={data} />}</Loaded>
    </div>
  );
}
