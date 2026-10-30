// The bar on every view: the views (the works, the architect, the items, the flow, and the owner's conversations with
// how many things in them wait for the owner, red, and how many entries are unread, grey, the browser's notifications
// telling of what newly waits), and how the engine stands: running,
// paused (saying which works the owner started run meanwhile), pausing while the last round finishes, or waiting for
// what stops every work, with the switch that pauses every work or starts them all, the switch that keeps waiting
// sessions warm while every work is paused, and the slots in use. The engine starts with every work paused. Drawn as
// the UI spec's bar: the views an UnderlineNav, the engine's state a dot and words, its switch a Button and keep-warm
// a ToggleSwitch.

import { DotFillIcon } from '@primer/octicons-react';
import { Button, CounterLabel, ToggleSwitch, UnderlineNav } from '@primer/react';
import { useEffect, useState, type KeyboardEvent, type MouseEvent } from 'react';

import type { Conversation, Status, WorkSummary } from './api';
import { course } from './components';
import { time } from './format';
import { postJson } from './http';
import { reread, useOnline, useResource, useStatus } from './live';
import { useNotifications } from './notify';
import { navigate, type Route } from './route';
import { t, tn, type Key } from './text';

/// How many rounds run that a pause of every work lets finish: none while the engine runs, or for a round the owner
/// asked for, or of a work the owner started.
function finishing(s: Status): number {
  const goesOn =
    s.running !== null && (s.running === s.stepping || s.started.includes(s.running) || s.architects_started.length > 0);
  return s.paused && s.running !== null && !goesOn ? 1 : 0;
}

/// What the engine is doing, in a few words: while every work is paused, which the owner started run.
function stateWords(s: Status): string {
  if (s.halt !== null) return t('engine_halt', { kind: s.halt.kind, until: time(s.halt.until) });
  if (!s.paused) return t('engine_running');
  const n = finishing(s);
  if (n > 0) return tn('engine_pausing', n);
  const runs = [...s.started.map((w) => `#${w}`), ...s.architects_started.map((p) => t('engine_runs_architect', { project: p }))];
  return runs.length > 0 ? t('engine_paused_runs', { which: runs.join(', ') }) : t('engine_paused');
}

/// The switch that keeps waiting sessions warm while every work is paused, on at first: one click turns it over, and
/// the engine keeps it across restarts.
function KeepWarm({ status }: { status: Status }) {
  // The switch shows the click at once and waits for nothing: the request is quick, and a loading state swapped the
  // switch's label for a spinner, which made the bar jump. Until the engine's state says the same, what was clicked
  // stands; a request that fails puts the engine's state back and says why.
  const [want, setWant] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  const on = want ?? status.keep_warm;
  useEffect(() => {
    if (want !== null && want === status.keep_warm) setWant(null);
  }, [want, status.keep_warm]);
  const flip = () => {
    const next = !on;
    setWant(next);
    setError(null);
    postJson('/api/v1/engine/keep-warm', { on: next }).catch((e: unknown) => {
      setWant(null);
      setError(e instanceof Error ? e.message : String(e));
    });
  };
  return (
    <>
      <span className="keep-warm" title={on ? t('keep_warm_on_hint') : t('keep_warm_off_hint')}>
        <span id="keep-warm-label">{t('keep_warm')}</span>
        <ToggleSwitch
          aria-labelledby="keep-warm-label"
          checked={on}
          onClick={flip}
          buttonLabelOn={t('switch_on')}
          buttonLabelOff={t('switch_off')}
          statusLabelPosition="end"
        />
      </span>
      {error !== null ? <span className="bad">{t('request_failed', { error })}</span> : null}
    </>
  );
}

/// The switch of every work: Pause all while works go on, Start all while they are paused. Starting asks first, saying
/// what goes on and which works on an older flow version are taken over.
function Switch({ status }: { status: Status }) {
  const works = useResource<WorkSummary[]>('works', '/api/v1/works');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const post = (path: string) => {
    setBusy(true);
    setError(null);
    postJson(path, {}).then(
      () => {
        setBusy(false);
        reread('works');
      },
      (e: unknown) => {
        setBusy(false);
        setError(e instanceof Error ? e.message : String(e));
      },
    );
  };
  const press = () => {
    if (!status.paused) {
      post('/api/v1/engine/pause');
      return;
    }
    // The works that go on once all start: those paused by themselves stay paused.
    const taken = (works.data ?? []).filter((w) => course(w, status).outdated && w.paused_by === null).length;
    const ask =
      taken > 0 ? tn('confirm_start_all_takeover', taken, { version: status.flow_version }) : t('confirm_start_all');
    if (!window.confirm(ask)) return;
    post('/api/v1/engine/start');
  };
  return (
    <>
      <Button
        variant={status.paused ? 'primary' : 'default'}
        onClick={press}
        disabled={busy}
        title={status.paused ? t('start_all_hint') : t('pause_all_hint')}
      >
        {status.paused ? t('start_all') : t('pause_all')}
      </Button>
      {error !== null ? <span className="bad">{t('request_failed', { error })}</span> : null}
    </>
  );
}

function Engine() {
  const online = useOnline();
  const status = useStatus();
  if (!online || !status) {
    return (
      <div className="engine">
        <span className="engine-state" title={t('engine_offline_hint')}>
          <DotFillIcon className="engine-dot engine-dot-off" />
          {t('engine_offline')}
        </span>
      </div>
    );
  }
  const dot =
    status.halt !== null ? 'off' : !status.paused ? 'run' : finishing(status) > 0 ? 'pausing' : 'paused';
  return (
    <div className="engine">
      <span className="engine-state" title={status.halt?.why ?? (status.paused ? t('engine_paused_hint') : undefined)}>
        <DotFillIcon className={`engine-dot engine-dot-${dot}`} />
        <span className={status.halt !== null ? 'bad' : undefined}>{stateWords(status)}</span>
      </span>
      <Switch status={status} />
      <KeepWarm status={status} />
      <span className="muted" title={t('slots_hint')}>
        {t('slots', { used: status.slots_used, total: status.slots })}
      </span>
    </div>
  );
}

/// A view of the bar: its place, the pages that are it, and its name.
type View = { to: string; pages: Route['page'][]; name: Key };

const VIEWS: View[] = [
  { to: '/works', pages: ['works', 'work'], name: 'nav_works' },
  { to: '/architect', pages: ['architect'], name: 'nav_architect' },
  { to: '/items', pages: ['items', 'item'], name: 'nav_items' },
  { to: '/flow', pages: ['flow'], name: 'nav_flow' },
  { to: '/system', pages: ['system'], name: 'nav_system' },
  { to: '/settings', pages: ['settings'], name: 'nav_settings' },
  { to: '/messages', pages: ['messages'], name: 'nav_messages' },
];

/// Moves to a view within the page; a click with a modifier key opens it as the browser would.
const go = (to: string) => (e: MouseEvent<HTMLAnchorElement> | KeyboardEvent<HTMLAnchorElement>) => {
  if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey || ('button' in e && e.button !== 0)) return;
  e.preventDefault();
  navigate(to);
};

/// The bar: the views, the owner's conversations with how many things in them wait for the owner (the open blockers
/// the owner holds, design cards not ruled, faults of wf-agents not read) in a red count and how many entries are
/// unread in a grey one, the browser told of what newly waits, and how the engine stands with the switch of every
/// work.
export function TopBar({ route }: { route: Route }) {
  const conversations = useResource<Conversation[]>('messages', '/api/v1/conversations');
  useNotifications(conversations.data);
  const list = conversations.data ?? [];
  const waiting = list.reduce((n, c) => n + c.waiting, 0);
  const unread = list.reduce((n, c) => n + c.unread, 0);
  return (
    <header className="topbar">
      <span className="brand">wf-agents</span>
      <UnderlineNav aria-label={t('nav_views')} className="topbar-nav">
        {VIEWS.map((v) => (
          <UnderlineNav.Item
            key={v.to}
            href={v.to}
            onSelect={go(v.to)}
            aria-current={v.pages.includes(route.page) ? 'page' : undefined}
          >
            {t(v.name)}
            {v.to === '/messages' && waiting > 0 ? (
              <CounterLabel variant="primary" className="topbar-count" title={tn('nav_waiting', waiting)}>
                {waiting}
              </CounterLabel>
            ) : null}
            {v.to === '/messages' && unread > 0 ? (
              <CounterLabel className="topbar-count" title={t('nav_unread', { n: unread })}>
                {unread}
              </CounterLabel>
            ) : null}
          </UnderlineNav.Item>
        ))}
      </UnderlineNav>
      <Engine />
    </header>
  );
}
