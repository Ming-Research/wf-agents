// What agents and the engine told the owner: messages that need no answer, newest first. The engine's own notices (a
// work stopped by a new flow version, a design card posted, every work halted until a limit resets) are marked as the
// engine's. The owner marks one read, or all of them; read ones are shown on asking.

import { useState } from 'react';

import type { OwnerMessage } from '../api';
import { time } from '../format';
import { postJson } from '../http';
import { reread, useResource } from '../live';
import { Link } from '../route';
import { t } from '../text';

/// Marks messages read: an agent's message or an engine's notice, or every one unread.
async function markRead(body: { ids: number[] } | { notices: number[] } | { all: true }): Promise<void> {
  await postJson('/api/v1/messages/read', body);
  reread('messages');
}

function Message({ m, onError }: { m: OwnerMessage; onError: (e: string) => void }) {
  const who = m.engine
    ? t('message_engine')
    : [m.line, m.cell !== null ? t('message_cell', { cell: m.cell }) : null].filter((x) => x !== null).join(' · ');
  const work = m.work;
  return (
    <li className={m.read_at === null ? 'msg msg-unread' : 'msg'}>
      <div className="msg-head">
        {work !== null ? (
          <Link to={m.round !== null ? `/works/${work}?round=${m.round}` : `/works/${work}`}>
            {t('work_title', { id: work })}
          </Link>
        ) : null}
        {m.item !== null ? (
          <Link to={`/items/${m.item}`} className="muted">
            #{m.item} {m.title ?? ''}
          </Link>
        ) : null}
        <span className="muted">{who}</span>
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
        {[...list].reverse().map((m) => (
          <Message key={`${m.engine ? 'e' : 'a'}${m.id}`} m={m} onError={setError} />
        ))}
      </ul>
    </div>
  );
}
