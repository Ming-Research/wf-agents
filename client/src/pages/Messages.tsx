// What agents and the engine told the owner: messages that need no answer, newest first. What an agent told shows once,
// as the agent's with its cell. The engine's own notices (a work stopped by a new flow version, a design card posted,
// every work halted until a limit resets) are marked as the engine's; what a wake of the architect told is named as the
// architect's and leads to its page; a notice of a new ask shows the kinds it asks about as chips coloured by their
// group. A report of a fault of wf-agents (a cache miss, a flow anomaly) has a look of its own and stays on top until
// read; one of cache misses says how many it holds, the tokens they wrote again and what they cost. The owner marks one
// read, or all of them; read ones are shown on asking.

import { useState } from 'react';

import type { OwnerMessage } from '../api';
import { cost, time } from '../format';
import { postJson } from '../http';
import { reread, useResource } from '../live';
import { words } from '../labelled';
import { architectHref, Link, workHref } from '../route';
import { t, type Key } from '../text';
import { KindChip } from './Decide';

/// The kinds of a fault of wf-agents, in words.
const FAULTS: Record<string, Key> = { cache_miss: 'fault_cache_miss', flow_anomaly: 'fault_flow_anomaly' };

/// Whether a message is a report of a fault of wf-agents the owner has not read: it stays on top.
export function pinned(m: OwnerMessage): boolean {
  return m.fault !== null && m.read_at === null;
}

/// The messages, oldest first as the interface gives them, in the order the view shows them: the reports of a fault of
/// wf-agents still unread, then every other one, each part newest first.
export function messageOrder(list: OwnerMessage[]): OwnerMessage[] {
  const newest = [...list].reverse();
  return [...newest.filter(pinned), ...newest.filter((m) => !pinned(m))];
}

/// Marks messages read: an agent's message or an engine's notice, or every one unread.
export async function markRead(body: { ids: number[] } | { notices: number[] } | { all: true }): Promise<void> {
  await postJson('/api/v1/messages/read', body);
  reread('messages');
}

function Message({ m, onError }: { m: OwnerMessage; onError: (e: string) => void }) {
  const who = m.engine
    ? t('message_engine')
    : [m.line, m.cell !== null ? t('message_cell', { cell: m.cell }) : null].filter((x) => x !== null).join(' · ');
  const work = m.work;
  const kind = m.fault !== null ? (FAULTS[m.fault] !== undefined ? t(FAULTS[m.fault]) : words(m.fault)) : null;
  const look = ['msg', m.read_at === null ? 'msg-unread' : null, kind !== null ? 'msg-fault' : null];
  return (
    <li className={look.filter((x) => x !== null).join(' ')}>
      <div className="msg-head">
        {kind !== null ? (
          <span className="badge verdict-bad" title={t('message_fault_hint')}>
            {t('message_fault', { kind })}
          </span>
        ) : null}
        {work !== null && m.architect !== null ? (
          <Link
            to={architectHref(
              m.round !== null
                ? { project: m.architect, wake: work, round: m.round, tab: 'rounds' }
                : { project: m.architect, wake: work },
            )}
          >
            {t('message_architect', { id: work, project: m.architect })}
          </Link>
        ) : null}
        {work !== null && m.architect === null ? (
          <Link to={workHref(work, { round: m.round })}>
            {t('work_title', { id: work })}
          </Link>
        ) : null}
        {m.item !== null ? (
          <Link to={`/items/${m.item}`} className="muted">
            #{m.item} {m.title ?? ''}
          </Link>
        ) : null}
        {kind === null ? <span className="muted">{who}</span> : null}
        {m.at !== null ? <span className="muted">{time(m.at)}</span> : null}
        {m.read_at === null ? (
          <button
            type="button"
            className="link"
            onClick={() => {
              markRead(m.engine ? { notices: [m.id] } : { ids: [m.id] }).catch((e: unknown) =>
                onError(e instanceof Error ? e.message : String(e)),
              );
            }}
          >
            {t('message_mark_read')}
          </button>
        ) : (
          <span className="muted">{t('message_read_at', { at: time(m.read_at) })}</span>
        )}
      </div>
      <p className="msg-text">{m.text}</p>
      {m.misses !== null && m.misses.count > 1 ? (
        <p className="muted">
          {t('message_misses', {
            n: m.misses.count,
            written: m.misses.written.toLocaleString('en-US'),
            cost: cost(m.misses.cost_usd),
            at: time(m.misses.last_at),
          })}
        </p>
      ) : null}
      {m.ask !== null && (m.item !== null || m.architect !== null) ? (
        <div className="msg-ask">
          {m.ask_kinds.map((k, i) => (
            <KindChip key={i} k={k} />
          ))}
          <Link
            to={
              m.architect !== null
                ? `${architectHref({ project: m.architect })}#ask-${m.ask}`
                : `/items/${m.item ?? ''}#ask-${m.ask}`
            }
          >
            {t('message_answer_ask', { id: m.ask })}
          </Link>
        </div>
      ) : null}
    </li>
  );
}

export function Messages() {
  const [all, setAll] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const r = useResource<OwnerMessage[]>('messages', all ? '/api/v1/messages?all=true' : '/api/v1/messages');
  const list = r.data ?? [];
  const unread = list.filter((m) => m.read_at === null).length;
  return (
    <div className="page">
      <p className="muted">{t('messages_note')}</p>
      <div className="msg-tools">
        <label>
          <input type="checkbox" checked={all} onChange={(e) => setAll(e.target.checked)} /> {t('messages_all')}
        </label>
        {unread > 0 ? (
          <button
            type="button"
            onClick={() => {
              markRead({ all: true }).catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)));
            }}
          >
            {t('messages_mark_all_read')}
          </button>
        ) : null}
        {error !== null ? <span className="bad">{t('request_failed', { error })}</span> : null}
        {r.error !== null ? <span className="bad">{r.error}</span> : null}
      </div>
      {list.length === 0 && r.data !== undefined ? (
        <p className="muted">{t(all ? 'messages_none' : 'messages_none_unread')}</p>
      ) : null}
      <ul className="msgs">
        {messageOrder(list).map((m) => (
          <Message key={`${m.engine ? 'e' : 'a'}${m.id}`} m={m} onError={setError} />
        ))}
      </ul>
    </div>
  );
}
