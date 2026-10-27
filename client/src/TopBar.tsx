// The bar on every view: the three views, how the engine stands (and what stops every work, when something does), and
// what the owner asks of it (its mode, and in single-step mode the next step).

import { useState } from 'react';

import type { Mode, Status } from './api';
import { time } from './format';
import { postJson } from './http';
import { useOnline, useStatus } from './live';
import { Link, type Route } from './route';
import { type T, useT } from './text';

const MODES: Mode[] = ['run', 'step', 'hold'];

function LastStep({ status, t }: { status: Status; t: T }) {
  const s = status.last_step;
  if (!s) return null;
  const what =
    s.work === null
      ? t('step_none')
      : s.started_item !== null
        ? t('step_started', { work: s.work, item: s.started_item })
        : t('step_took', { work: s.work });
  const last = s.rounds.length > 0 ? s.rounds[s.rounds.length - 1] : null;
  return (
    <span className="last-step" title={t('step_rounds', { rounds: s.rounds.join(', ') })}>
      {t('last_step')}:{' '}
      {s.work !== null && last !== null ? <Link to={`/works/${s.work}?round=${last}`}>{what}</Link> : what}
      {s.cell ? <span className="cell-id"> {s.cell}</span> : null}
    </span>
  );
}

function Engine() {
  const t = useT();
  const online = useOnline();
  const status = useStatus();
  const [error, setError] = useState<string | null>(null);
  if (!online || !status) {
    return (
      <div className="engine">
        <span className="dot dot-off" />
        <span title={t('engine_offline_hint')}>{t('engine_offline')}</span>
      </div>
    );
  }
  const ask = (path: string, body: unknown) => {
    setError(null);
    postJson(path, body).catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)));
  };
  const switchTo = (mode: Mode) => {
    if (mode === status.mode) return;
    if (mode === 'run' && !window.confirm(t('confirm_run'))) return;
    ask('/api/v1/engine/mode', { mode });
  };
  return (
    <div className="engine">
      <span className={`dot dot-${status.mode}`} />
      <div className="modes" role="group">
        {MODES.map((m) => (
          <button
            key={m}
            className={m === status.mode ? 'mode mode-on' : 'mode'}
            title={t(`mode_${m}_hint`)}
            onClick={() => switchTo(m)}
          >
            {t(`mode_${m}`)}
          </button>
        ))}
      </div>
      {status.mode === 'step' ? (
        <button className="step" disabled={status.stepping} onClick={() => ask('/api/v1/engine/step', {})}>
          {status.stepping ? t('stepping') : t('next_step')}
        </button>
      ) : null}
      <LastStep status={status} t={t} />
      {status.halt !== null ? (
        <span className="bad" title={status.halt.why}>
          {t('halt', { kind: status.halt.kind, until: time(status.halt.until) })}
        </span>
      ) : null}
      <span className="muted">{t('slots', { used: status.slots_used, total: status.slots })}</span>
      {error !== null ? <span className="bad">{t('request_failed', { error })}</span> : null}
    </div>
  );
}

export function TopBar({ route }: { route: Route }) {
  const t = useT();
  const on = (pages: Route['page'][]) => (pages.includes(route.page) ? 'nav nav-on' : 'nav');
  return (
    <nav className="topbar">
      <span className="brand">wf-agents</span>
      <Link to="/works" className={on(['works', 'work'])}>
        {t('nav_works')}
      </Link>
      <Link to="/items" className={on(['items', 'item'])}>
        {t('nav_items')}
      </Link>
      <Link to="/flow" className={on(['flow'])}>
        {t('nav_flow')}
      </Link>
      <Engine />
    </nav>
  );
}
