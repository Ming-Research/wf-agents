// The head of a conversation, as a chat app's group information: what the conversation is about and the owner's
// controls of it, which are here and nowhere else. A work's group says its work and item, the stage it is at, how it
// goes on and what it cost so far, with the work's Start, Pause, Step and Start over first in its row and the tier its
// item is at;
// the one-to-one with an architect is one row: its Start, Pause and Step first, its name leading to its page, what it
// handles now, at which cell, or that it is idle, paused or waits for the owner, and at the row's end the holds on the
// project's new works and, while the panel beside the one-to-one is closed, the button that opens it; its queue is in
// that panel; wf-agents' own says where each new fault goes as it comes, which the owner sets there. The architect is
// always there to the owner: the page never names its works.

import { BlockedIcon, SidebarExpandIcon } from '@primer/octicons-react';
import { IconButton, Label, Select } from '@primer/react';
import { Fragment, useState } from 'react';

import type { Architect, Conversation, Forwarding, Settings, Status, WorkSummary } from '../api';
import { architectNow, architectState, holdWords, matterSentence } from '../architect';
import { course, heldName, RunningMark, StateChip, WORK_TONE } from '../components';
import { forwardNewChoices } from '../conversation';
import {
  Control,
  Controls,
  runningWords,
  SessionChoice,
  StartOver,
  useActions,
  useNewSession,
  useOpenStepped,
  waitsWords,
} from '../course';
import { cost, time } from '../format';
import { postJson } from '../http';
import { reread, useResource, useStatus } from '../live';
import { architectHref, forProject, Link, navigate, workHref } from '../route';
import { t } from '../text';
import { factsAfter } from '../voices';
import { stageName } from './TalkCards';

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
      <label htmlFor={`tier-${item}`}>{t('col_tier')}</label>
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
  return <StateChip tone="neutral">{t('state_ready')}</StateChip>;
}

/// The head of a work's group, one line with nothing on it said twice: its Start, Pause, Step and Start over while it
/// goes on, the work by its number with its item's title, how it goes on, then after a bar its facts: its item, its rounds and what
/// it cost, and while it goes on its stage, what it waits for and the tier its item is at, which the owner sets here.
export function WorkHead({ c, w }: { c: Conversation; w: WorkSummary | null }) {
  const status = useStatus();
  const on = w !== null && !c.over ? w : null;
  const facts = factsAfter([
    c.item !== null ? <Link to={`/items/${c.item}`}>{t('talk_item', { id: c.item })}</Link> : null,
    c.work !== null ? <Link to={workHref(c.work)}>{t('talk_rounds')}</Link> : null,
    w !== null ? cost(w.cost_usd) : null,
    on !== null && on.stage !== null ? stageName(on.stage) : null,
    on !== null && on.waits !== null ? waitsWords(on.waits) : null,
    on !== null && c.item !== null && c.level !== null ? <Tier item={c.item} level={c.level} /> : null,
  ]);
  return (
    <header className="talk-head">
      <div className="talk-head-title">
        {on !== null ? <WorkControls w={on} status={status} /> : null}
        <h2 className="talk-head-name">
          {t('conv_work_item', { id: c.work ?? '', title: c.title ?? '' })}
        </h2>
        {w !== null ? <WorkState w={w} status={status} /> : null}
        {facts.length > 0 ? (
          <span className="talk-head-facts">
            {facts.map((x, k) => (
              <Fragment key={k}>{x}</Fragment>
            ))}
          </span>
        ) : null}
      </div>
    </header>
  );
}

/// A work's Start, Pause and Step, with the session they run its next round in, and its Start over; Step opens the round
/// it ran on the work's page once it has ended.
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
      <StartOver a={a} w={w} status={status} />
      {a.error !== null ? <span className="bad">{t('request_failed', { error: a.error })}</span> : null}
    </span>
  );
}

/// The architect's Start, Pause and Step, and the session they run its next round in. While it works, they are its
/// current round's; while it is idle, Start lets it work by itself whatever the global state, Pause holds it back, and
/// Step runs its next round while a matter waits, asking first, since a real round runs on the owner's account.
function ArchitectControls({ a, status }: { a: Architect; status: Status | null }) {
  const state = architectState(a.works, a.queue);
  const going = state.kind === 'working' ? state.work : null;
  if (going !== null) return <ArchitectWorkControls w={going} status={status} />;
  return <IdleControls a={a} status={status} waiting={state.waiting} />;
}

function ArchitectWorkControls({ w, status }: { w: WorkSummary; status: Status | null }) {
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
        hint={whyNot ?? t('arch_step_hint')}
        onClick={() => {
          if (window.confirm(t('arch_step_confirm'))) send('/api/v1/architect/step', { new_session: fresh });
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
/// take them up. Its works are never named.
function ArchitectNow({ a, status }: { a: Architect; status: Status | null }) {
  const state = architectState(a.works, a.queue);
  const now = architectNow(a, status);
  const chip = <StateChip tone={WORK_TONE[now.state]}>{now.words}</StateChip>;
  if (state.kind !== 'working') return chip;
  const at = `${state.work.at} ${state.work.at_title ?? ''}`;
  const what = state.handling !== null ? matterSentence(state.handling) : null;
  return (
    <>
      {chip}
      <span className="talk-head-now" title={what ?? undefined}>
        {now.state === 'running' ? <RunningMark /> : null}
        {what !== null ? t('talk_architect_handles', { what, at }) : t('talk_architect_at', { at })}
      </span>
    </>
  );
}

/// The head of the one-to-one with a project's architect, in one row: its Start, Pause and Step, whose architect (a link
/// to its page), what it does now, and at the row's end the holds on the project's new works and, while the panel beside
/// the one-to-one is closed, the button that opens it (`onSide`).
export function ArchitectHead({ c, a, onSide }: { c: Conversation; a: Architect | null; onSide: (() => void) | null }) {
  const project = c.project ?? '';
  const status = useStatus();
  return (
    <header className="talk-head">
      <div className="talk-head-title">
        {a !== null ? <ArchitectControls a={a} status={status} /> : null}
        <h2 className="talk-head-name">
          <Link to={architectHref({ project })} title={t('talk_architect_page')}>
            {t('conv_architect', { project })}
          </Link>
        </h2>
        {a !== null ? <ArchitectNow a={a} status={status} /> : null}
        {a !== null ? (
          <span className="talk-head-end">
            {a.holds.map((h) => (
              <span key={h.reason} className="talk-head-hold" title={t('hold_since', { at: time(h.since) })}>
                <BlockedIcon size={16} />
                {holdWords(h.reason)}
              </span>
            ))}
            {onSide !== null ? (
              <IconButton size="small" variant="invisible" icon={SidebarExpandIcon} aria-label={t('side_open')} onClick={onSide} />
            ) : null}
          </span>
        ) : null}
      </div>
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
