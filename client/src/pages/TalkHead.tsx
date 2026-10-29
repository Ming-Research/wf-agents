// The head of a conversation, as a chat app's group information: what the conversation is about and the owner's
// controls of it, which are here and nowhere else. A work's group says its work and item, the stage it is at, how it
// goes on and what it cost so far, with the work's Start, Pause and Step and the tier its item is at; the one-to-one
// with an architect says what the architect handles now, at which cell, or that it is idle, paused or waits for the
// owner, with its Start, Pause and Step, and under it its queue: how many matters are left, the oldest wait and how
// many it handled today, opening to the list with the matter in hand marked; wf-agents' own says where each new fault
// goes as it comes, which the owner sets there. The architect is always there to the owner: the page never names its
// wakes.

import { BlockedIcon } from '@primer/octicons-react';
import { Label, Select } from '@primer/react';
import { useState } from 'react';

import type { Architect, Conversation, Forwarding, Settings, Status, WorkSummary } from '../api';
import { architectState, holdWords, matterKind, matterSentence, queueParts } from '../architect';
import { course, heldName } from '../components';
import { forwardNewChoices } from '../conversation';
import { Control, Controls, runningWords, SessionChoice, useActions, useNewSession, useOpenStepped, waitsWords } from '../course';
import { cost, duration, time } from '../format';
import { postJson } from '../http';
import { reread, useResource, useStatus } from '../live';
import { architectHref, forProject, Link, navigate, workHref } from '../route';
import { t } from '../text';
import { stageName } from './TalkCards';
import { Halo } from '../graph/flowDraw';

/// The tiers an item may be put at: those up to the highest an item may be at, in order.
export function tiersAllowed(s: Settings): string[] {
  const names = s.tiers.map((x) => x.name);
  const top = s.highest_level === null ? -1 : names.indexOf(s.highest_level);
  return top < 0 ? names : names.slice(0, top + 1);
}

/// The tier a work's item is at, which the owner changes here: a tier above the highest an item may be at is not
/// offered. Its works run at the new tier from their next round.
function Tier({ item, level }: { item: number; level: string }) {
  const s = useResource<Settings>('settings', '/api/v1/settings');
  const [error, setError] = useState<string | null>(null);
  const allowed = s.data !== undefined ? tiersAllowed(s.data) : [];
  const options = allowed.includes(level) ? allowed : [...allowed, level];
  return (
    <span className="talk-head-tier">
      <label htmlFor={`tier-${item}`}>{t('col_level')}</label>
      <Select
        id={`tier-${item}`}
        size="small"
        value={level}
        onChange={(e) => {
          setError(null);
          postJson(`/api/v1/items/${item}/level`, { level: e.target.value }).then(
            () => reread('messages', `item:${item}`, 'items'),
            (x: unknown) => setError(x instanceof Error ? x.message : String(x)),
          );
        }}
      >
        {options.map((n) => (
          <Select.Option key={n} value={n} disabled={!allowed.includes(n)}>
            {n}
          </Select.Option>
        ))}
      </Select>
      {error !== null ? <span className="bad">{error}</span> : null}
    </span>
  );
}

/// How a work goes on, in a few words, as its Label: a round running, what holds it back, what it waits for, or over.
function WorkState({ w, status }: { w: WorkSummary; status: Status | null }) {
  const c = course(w, status);
  if (c.over) return <Label variant={w.state === 'done' ? 'success' : 'danger'}>{t(`state_${w.state}`)}</Label>;
  if (c.running !== null) {
    return (
      <span className="talk-head-running">
        <RunningMark />
        <Label variant="accent">{runningWords(c)}</Label>
      </span>
    );
  }
  if (c.held !== null) {
    const h = heldName(c);
    return <Label variant={h.problem ? 'danger' : 'attention'}>{h.name}</Label>;
  }
  if (c.waits !== null) return <Label variant="attention" title={waitsWords(c.waits)}>{t('talk_waits')}</Label>;
  return <Label variant="secondary">{t('state_ready')}</Label>;
}

/// The head of a work's group: the work, its item and title, its stage, how it goes on and what it cost, what it
/// waits for, the tier its item is at, and its Start, Pause and Step; its rounds a link away.
export function WorkHead({ c, w }: { c: Conversation; w: WorkSummary | null }) {
  const status = useStatus();
  return (
    <header className="talk-head">
      <div className="talk-head-title">
        <h2 className="talk-head-name">
          {t('conv_work_item', { id: c.work ?? '', title: c.title ?? '' })}
        </h2>
        {w !== null ? <WorkState w={w} status={status} /> : null}
      </div>
      <div className="talk-head-facts">
        {c.item !== null ? <Link to={`/items/${c.item}`}>{t('talk_item', { id: c.item })}</Link> : null}
        {w !== null && w.stage !== null ? <span>{stageName(w.stage)}</span> : null}
        {w !== null ? <span>{cost(w.cost_usd)}</span> : null}
        {w !== null && w.waits !== null ? <span>{waitsWords(w.waits)}</span> : null}
        {c.work !== null ? <Link to={workHref(c.work)}>{t('talk_rounds')}</Link> : null}
        {c.item !== null && c.level !== null && !c.over ? <Tier item={c.item} level={c.level} /> : null}
        {w !== null && !c.over ? <WorkControls w={w} status={status} /> : null}
      </div>
    </header>
  );
}

/// A work's Start, Pause and Step, with the session they run its next round in; Step opens the round it ran on the
/// work's page once it has ended.
function WorkControls({ w, status }: { w: WorkSummary; status: Status | null }) {
  const a = useActions(w, status, true);
  const stepped = useOpenStepped(status, w.id, (work, round) => navigate(workHref(work, { round })));
  return (
    <span className="talk-head-controls">
      <Controls
        a={a}
        w={w}
        status={status}
        onStep={() => {
          const at = Date.now();
          a.step(() => stepped(at));
        }}
      />
      {a.error !== null ? <span className="bad">{t('request_failed', { error: a.error })}</span> : null}
    </span>
  );
}

/// The architect's Start, Pause and Step, and the session they run its next round in. While it works, they are its
/// current round's; while it is idle, Start lets it work by itself whatever the global state, Pause holds it back, and
/// Step runs its next round while a matter waits, asking first, since a real round runs on the owner's account.
function ArchitectControls({ a, status }: { a: Architect; status: Status | null }) {
  const state = architectState(a.wakes, a.queue);
  const going = state.kind === 'awake' ? state.wake : null;
  if (going !== null) return <WakeControls w={going} status={status} />;
  return <IdleControls a={a} status={status} waiting={state.waiting} />;
}

function WakeControls({ w, status }: { w: WorkSummary; status: Status | null }) {
  const a = useActions(w, status, true);
  return (
    <span className="talk-head-controls">
      <Controls a={a} w={w} status={status} onStep={() => a.step()} />
      {a.error !== null ? <span className="bad">{t('request_failed', { error: a.error })}</span> : null}
    </span>
  );
}

function IdleControls({ a, status, waiting }: { a: Architect; status: Status | null; waiting: number }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [fresh, setFresh] = useNewSession('architect');
  const send = (path: string, body: object) => {
    setBusy(true);
    setError(null);
    postJson(forProject(path, a.project), body).then(
      () => {
        setBusy(false);
        setFresh(false);
        reread('architect', 'messages', 'works');
      },
      (e: unknown) => {
        setBusy(false);
        setError(e instanceof Error ? e.message : String(e));
      },
    );
  };
  const started = status !== null && status.architects_started.includes(a.project);
  const canStart = status !== null && status.paused && !started;
  const canPause = started;
  const whyNot = !canStart
    ? t('work_step_goes_on')
    : waiting === 0
      ? t('arch_queue_none_waiting')
      : status.stepping !== null || status.halt !== null
        ? t('work_step_later')
        : null;
  return (
    <span className="talk-head-controls">
      {canStart ? (
        <Control icon="start" hint={t('arch_start_hint')} onClick={() => send('/api/v1/architect/start', { new_session: fresh })} disabled={busy} />
      ) : null}
      {canPause ? <Control icon="pause" hint={t('arch_pause_hint')} onClick={() => send('/api/v1/architect/pause', {})} disabled={busy} /> : null}
      <Control
        icon="step"
        hint={whyNot ?? t('arch_wake_hint')}
        onClick={() => {
          if (window.confirm(t('arch_wake_confirm'))) send('/api/v1/architect/step', { new_session: fresh });
        }}
        disabled={busy || whyNot !== null}
      />
      <SessionChoice on={fresh} set={setFresh} disabled={busy || !canStart} />
      {error !== null ? <span className="bad">{t('request_failed', { error })}</span> : null}
    </span>
  );
}

/// What the architect does now, in a few words and as a Label: it handles a matter at a cell; it works at a cell with
/// no matter in hand; it is paused or held back; or it is idle, with nothing in its queue or matters waiting for it to
/// take them up. Its wakes are never named.
function ArchitectNow({ a, status }: { a: Architect; status: Status | null }) {
  const state = architectState(a.wakes, a.queue);
  if (state.kind === 'awake') {
    const c = course(state.wake, status);
    const at = `${state.wake.at} ${state.wake.at_title ?? ''}`;
    const what = state.handling !== null ? matterSentence(state.handling) : null;
    return (
      <>
        {c.running !== null ? (
          <Label variant="accent">{c.running === 'step' ? runningWords(c) : t('talk_architect_working')}</Label>
        ) : c.held !== null ? (
          <Label variant="attention">{heldName(c).name}</Label>
        ) : (
          <Label variant="secondary">{t('talk_architect_between')}</Label>
        )}
        <span className="talk-head-now" title={what ?? undefined}>
          {c.running !== null ? <RunningMark /> : null}
          {what !== null ? t('talk_architect_handles', { what, at }) : t('talk_architect_at', { at })}
        </span>
      </>
    );
  }
  const started = status !== null && status.architects_started.includes(a.project);
  if (state.waiting > 0 && status !== null && status.paused && !started) {
    return <Label variant="attention">{t('talk_architect_held')}</Label>;
  }
  return <Label variant="secondary">{state.waiting > 0 ? t('talk_architect_takes_up') : t('talk_architect_idle')}</Label>;
}

/// The mark beside the cell a round runs at, in a conversation's head: the chart's halo drawn small round a dot, the one
/// animation the page has, so that what runs looks the same on the chart and here.
export function RunningMark() {
  return (
    <svg className="running-mark" width={16} height={16} viewBox="0 0 16 16" aria-hidden="true">
      <Halo box={{ x: 4, y: 4, width: 8, height: 8 }} round small />
    </svg>
  );
}

/// The start of today in the browser's time zone.
const today = (): number => {
  const d = new Date();
  d.setHours(0, 0, 0, 0);
  return d.getTime();
};

/// The architect's queue, pinned under the head: how many matters are left, how long the oldest has waited, and how
/// many it handled today; opened, the matters left in the order they came, the one in hand marked, and those an ask of
/// the owner holds.
function QueueBar({ a }: { a: Architect }) {
  const { waiting, held } = queueParts(a.queue);
  const state = architectState(a.wakes, a.queue);
  const handling = state.kind === 'awake' ? (state.handling?.id ?? null) : null;
  const oldest = waiting.reduce<number | null>((m, x) => (m === null || x.at < m ? x.at : m), null);
  const from = today();
  const done = a.handled.filter((h) => h.at !== null && h.at >= from).length;
  const [now] = useState(() => Date.now());
  return (
    <details className="talk-queue">
      <summary>
        <span className="talk-queue-n">{t('talk_queue_left', { n: waiting.length })}</span>
        {oldest !== null ? <span>{t('talk_queue_oldest', { for: duration(now - oldest) })}</span> : null}
        <span>{t('talk_queue_today', { n: done })}</span>
        {held.length > 0 ? <span>{t('talk_queue_held', { n: held.length })}</span> : null}
      </summary>
      <ol className="talk-queue-list">
        {[...waiting, ...held].map((m) => (
          <li key={m.id} className={m.id === handling ? 'talk-queue-current' : undefined}>
            <span className="talk-card-kind">{matterKind(m.kind, m.detail)}</span> <span>{matterSentence(m)}</span>{' '}
            <span className="talk-card-facts">{time(m.at)}</span>
            {m.id === handling ? (
              <>
                {' '}
                <Label variant="accent">{t('matter_working')}</Label>
              </>
            ) : null}
            {held.includes(m) ? (
              <>
                {' '}
                <Label variant="attention">{t('matter_held')}</Label>
              </>
            ) : null}
            {m.work !== null && m.source === 'work' ? (
              <>
                {' '}
                <Link to={`/messages?c=${encodeURIComponent(`work:${m.work}`)}`} muted>
                  {t('matter_in_group', { work: m.work })}
                </Link>
              </>
            ) : null}
          </li>
        ))}
      </ol>
    </details>
  );
}

/// The head of the one-to-one with a project's architect: whose architect, what it does now, its Start, Pause and
/// Step, the holds on the project's new works, its page a link away; and its queue pinned under it.
export function ArchitectHead({ c }: { c: Conversation }) {
  const project = c.project ?? '';
  const status = useStatus();
  const r = useResource<Architect>('architect', forProject('/api/v1/architect', project));
  const a = r.data !== undefined && r.data.project === project ? r.data : null;
  return (
    <header className="talk-head">
      <div className="talk-head-title">
        <h2 className="talk-head-name">{t('conv_architect', { project })}</h2>
        {a !== null ? <ArchitectNow a={a} status={status} /> : null}
        {a !== null && a.holds.length > 0 ? (
          <span className="talk-head-holds">
            {a.holds.map((h) => (
              <span key={h.reason} className="talk-head-hold" title={t('hold_since', { at: time(h.since) })}>
                <BlockedIcon size={16} />
                {holdWords(h.reason)}
              </span>
            ))}
          </span>
        ) : null}
      </div>
      <div className="talk-head-facts">
        <Link to={architectHref({ project })}>{t('talk_architect_page')}</Link>
        {a !== null ? <ArchitectControls a={a} status={status} /> : null}
      </div>
      {a !== null ? <QueueBar a={a} /> : null}
    </header>
  );
}

/// The head of wf-agents' own conversation: its name, and where each new fault goes as it comes, which the owner sets
/// here: nobody, as it is at first, or a project's architect, which takes each as a matter of its queue.
export function WfAgentsHead({ title, forwarding }: { title: string; forwarding: Forwarding | null }) {
  const [error, setError] = useState<string | null>(null);
  return (
    <header className="talk-head">
      <div className="talk-head-title">
        <h2 className="talk-head-name">{title}</h2>
      </div>
      {forwarding !== null ? (
        <div className="talk-head-facts">
          <span className="talk-head-choice">
            <label htmlFor="forward-new">{t('fault_forward_new')}</label>
            <Select
              id="forward-new"
              size="small"
              value={forwarding.auto ?? ''}
              onChange={(e) => {
                setError(null);
                const project = e.target.value === '' ? null : e.target.value;
                postJson('/api/v1/faults/forward-new', { project }).then(
                  () => reread('messages'),
                  (x: unknown) => setError(x instanceof Error ? x.message : String(x)),
                );
              }}
            >
              {forwardNewChoices(forwarding).map((c) => (
                <Select.Option key={c.value} value={c.value}>
                  {c.label}
                </Select.Option>
              ))}
            </Select>
            {error !== null ? <span className="bad">{t('request_failed', { error })}</span> : null}
          </span>
        </div>
      ) : null}
    </header>
  );
}

/// The head of a conversation of no work and no architect's: its name.
export function PlainHead({ title }: { title: string }) {
  return (
    <header className="talk-head">
      <div className="talk-head-title">
        <h2 className="talk-head-name">{title}</h2>
      </div>
    </header>
  );
}
