// The bar on every view: the views (the works, the architect, the items, the flow, and the messages to the owner with
// how many are unread), and how the engine stands: running, paused (saying which works the owner started run
// meanwhile), pausing while the last round finishes, or waiting for what stops every work, with the switch that pauses
// every work or starts them all, the switch that keeps waiting sessions warm while every work is paused, and the slots
// in use. The engine starts with every work paused.

import { useState } from 'react';

import type { OwnerMessage, Status, WorkSummary } from './api';
import { course } from './components';
import { time } from './format';
import { postJson } from './http';
import { reread, useOnline, useResource, useStatus } from './live';
import { Link, type Route } from './route';
import { t } from './text';

/// How many rounds run that a pause of every work lets finish: none while the engine runs, or for a round the owner
/// asked for, or of a work the owner started.
function finishing(s: Status): number {
  const goesOn = s.running !== null && (s.running === s.stepping || s.started.includes(s.running) || s.architect_started);
  return s.paused && s.running !== null && !goesOn ? 1 : 0;
}

/// What the engine is doing, in a few words: while every work is paused, which the owner started run.
function stateWords(s: Status): string {
  if (s.halt !== null) return t('engine_halt', { kind: s.halt.kind, until: time(s.halt.until) });
  if (!s.paused) return t('engine_running');
  const n = finishing(s);
  if (n === 1) return t('engine_pausing_one');
  if (n > 0) return t('engine_pausing', { n });
  const runs = [...s.started.map((w) => `#${w}`), ...(s.architect_started ? [t('engine_runs_architect')] : [])];
  return runs.length > 0 ? t('engine_paused_runs', { which: runs.join(', ') }) : t('engine_paused');
}

/// The switch that keeps waiting sessions warm while every work is paused, on at first: one click turns it over, and
/// the engine keeps it across restarts.
function KeepWarm({ status }: { status: Status }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const flip = () => {
    setBusy(true);
    setError(null);
    postJson('/api/v1/engine/keep-warm', { on: !status.keep_warm }).then(
      () => setBusy(false),
      (e: unknown) => {
        setBusy(false);
        setError(e instanceof Error ? e.message : String(e));
      },
    );
  };
  return (
    <>
      <button
        type="button"
        className={`keep-warm${status.keep_warm ? ' on' : ''}`}
        aria-pressed={status.keep_warm}
        onClick={flip}
        disabled={busy}
        title={status.keep_warm ? t('keep_warm_on_hint') : t('keep_warm_off_hint')}
      >
        <span className="keep-warm-box" aria-hidden="true">
          {status.keep_warm ? '✓' : ''}
        </span>
        {t('keep_warm')}
      </button>
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
      taken > 0 ? t('confirm_start_all_takeover', { n: taken, version: status.flow_version }) : t('confirm_start_all');
    if (!window.confirm(ask)) return;
    post('/api/v1/engine/start');
  };
  return (
    <>
      <button
        type="button"
        className="engine-switch"
        onClick={press}
        disabled={busy}
        title={status.paused ? t('start_all_hint') : t('pause_all_hint')}
      >
        {status.paused ? t('start_all') : t('pause_all')}
      </button>
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
        <span className="dot dot-off" />
        <span title={t('engine_offline_hint')}>{t('engine_offline')}</span>
      </div>
    );
  }
  const dot =
    status.halt !== null ? 'dot-halt' : !status.paused ? 'dot-run' : finishing(status) > 0 ? 'dot-pausing' : 'dot-paused';
  return (
    <div className="engine">
      <span className="engine-state" title={status.halt?.why ?? (status.paused ? t('engine_paused_hint') : undefined)}>
        <span className={`dot ${dot}`} />
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

/// The view of the messages to the owner, with how many are unread.
function Unread({ className }: { className: string }) {
  const r = useResource<OwnerMessage[]>('messages', '/api/v1/messages');
  const n = r.data?.length ?? 0;
  return (
    <Link to="/messages" className={className}>
      {t('nav_messages')}
      {n > 0 ? <span className="nav-count">{n}</span> : null}
    </Link>
  );
}

/// The bar: the views, and how the engine stands with the switch of every work. A view's own controls stay in the
/// view.
export function TopBar({ route }: { route: Route }) {
  const on = (pages: Route['page'][]) => (pages.includes(route.page) ? 'nav nav-on' : 'nav');
  return (
    <nav className="topbar">
      <span className="brand">wf-agents</span>
      <Link to="/works" className={on(['works', 'work'])}>
        {t('nav_works')}
      </Link>
      <Link to="/architect" className={on(['architect'])}>
        {t('nav_architect')}
      </Link>
      <Link to="/items" className={on(['items', 'item'])}>
        {t('nav_items')}
      </Link>
      <Link to="/flow" className={on(['flow'])}>
        {t('nav_flow')}
      </Link>
      <Link to="/spend" className={on(['spend'])}>
        {t('nav_spend')}
      </Link>
      <Link to="/settings" className={on(['settings'])}>
        {t('nav_settings')}
      </Link>
      <Unread className={on(['messages'])} />
      <span className="bar-gap" />
      <Engine />
    </nav>
  );
}
