// The bar on every view: the views (the messages to the owner with how many are unread), and how the engine stands:
// running, holding, or waiting for what stops every work, with the slots in use. The bar holds no control: the
// engine's state opens a panel where the owner switches between running and holding.

import { useEffect, useRef, useState } from 'react';

import type { Mode, OwnerMessage, Status } from './api';
import { time } from './format';
import { postJson } from './http';
import { useOnline, useResource, useStatus } from './live';
import { Link, type Route } from './route';
import { t } from './text';

const MODES: Mode[] = ['run', 'hold'];

/// What the engine is doing, in a few words.
function stateWords(s: Status): string {
  if (s.halt !== null) return t('engine_halt', { kind: s.halt.kind, until: time(s.halt.until) });
  return t(s.mode === 'run' ? 'engine_running' : 'engine_holding');
}

/// The panel the engine's state opens: running or holding, and what each means.
function EnginePanel({ status, onClose }: { status: Status; onClose: () => void }) {
  const [error, setError] = useState<string | null>(null);
  const switchTo = (mode: Mode) => {
    if (mode === status.mode) return;
    if (mode === 'run' && !window.confirm(t('confirm_run'))) return;
    setError(null);
    postJson('/api/v1/engine/mode', { mode }).then(onClose, (e: unknown) =>
      setError(e instanceof Error ? e.message : String(e)),
    );
  };
  return (
    <div className="engine-panel" role="dialog">
      {MODES.map((m) => (
        <button
          key={m}
          type="button"
          className={m === status.mode ? 'engine-mode engine-mode-on' : 'engine-mode'}
          onClick={() => switchTo(m)}
        >
          <strong>{t(`mode_${m}`)}</strong>
          <span className="muted">{t(`mode_${m}_hint`)}</span>
        </button>
      ))}
      {status.halt !== null ? <p className="bad">{status.halt.why}</p> : null}
      {error !== null ? <p className="bad">{t('request_failed', { error })}</p> : null}
    </div>
  );
}

function Engine() {
  const online = useOnline();
  const status = useStatus();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('pointerdown', away);
    return () => document.removeEventListener('pointerdown', away);
  }, [open]);
  if (!online || !status) {
    return (
      <div className="engine">
        <span className="dot dot-off" />
        <span title={t('engine_offline_hint')}>{t('engine_offline')}</span>
      </div>
    );
  }
  const dot = status.halt !== null ? 'dot-halt' : `dot-${status.mode}`;
  return (
    <div className="engine" ref={ref}>
      <button type="button" className="engine-state" onClick={() => setOpen((o) => !o)} aria-expanded={open}>
        <span className={`dot ${dot}`} />
        <span className={status.halt !== null ? 'bad' : undefined}>{stateWords(status)}</span>
      </button>
      <span className="muted">{t('slots', { used: status.slots_used, total: status.slots })}</span>
      {open ? <EnginePanel status={status} onClose={() => setOpen(false)} /> : null}
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

/// The bar; `slot` is given the part of it a view fills with what it needs at hand.
export function TopBar({ route, slot }: { route: Route; slot: (el: HTMLElement | null) => void }) {
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
      <Unread className={on(['messages'])} />
      <div className="bar-slot" ref={slot} />
      <Engine />
    </nav>
  );
}
