// The owner's conversations: their list on the left, the one open on the right. A conversation is about one subject:
// an engineer's work (what its agents tell and ask the owner, the owner's answers and interjections, its asks of the
// architect and the advisor with their replies, the engine's notices about it), a project's architect (the owner's
// words, what it tells and asks the owner, the design cards of the project's works for the owner to rule on), two
// projects' architects (their messages), or wf-agents itself (its faults, and what the engine tells about no work).
// The list puts first, marked, the conversations where something waits for the owner (an ask not answered, a design
// card not ruled, a fault not read), then the rest by their latest entry, in groups by project when there are several,
// each with how many things wait and how many are unread. In a conversation, what waits for the owner is pinned above
// the stream, where a flood cannot bury it, and answered there; once answered it takes its place in the stream at its
// time. The stream runs oldest first, the newest at the bottom and scrolled to; each entry says who it is from and to,
// and what passes between agents is folded until the owner opens it. Opening a conversation marks its messages and
// notices read; a fault is marked read where it is pinned. The box at the bottom says something: in an architect's
// conversation to that architect; in a work's to its engineer, or to the architect about the work. The same view shows
// a conversation wherever the page shows one: here, under the architect's panel and in a work's panel.

import { Fragment, useEffect, useRef, useState } from 'react';

import type { Conversation, ConversationDetail, ConversationEntry, Misses } from '../api';
import {
  conversationGroups,
  conversationTitle,
  party,
  pendingWords,
  pinnedOf,
  runsOf,
  toMarkRead,
} from '../conversation';
import { cost, time } from '../format';
import { postJson } from '../http';
import { reread, useResource } from '../live';
import { Markdown } from '../markdown';
import { architectHref, href, Link, workHref } from '../route';
import { t } from '../text';
import { ArchitectAsk } from './ArchitectAsk';
import { AskView, DesignCardView } from './Decide';

/// Marks messages read: agents' messages and the engine's notices, or every one unread.
export async function markRead(body: { ids: number[]; notices: number[] } | { all: true }): Promise<void> {
  await postJson('/api/v1/messages/read', body);
  reread('messages');
}

/// The cache misses a notice holds, when more than its first.
function MissesLine({ m }: { m: Misses }) {
  if (m.count <= 1) return null;
  return (
    <p className="muted">
      {t('message_misses', {
        n: m.count,
        written: m.written.toLocaleString('en-US'),
        cost: cost(m.cost_usd),
        at: time(m.last_at),
      })}
    </p>
  );
}

/// What an entry says: an agent's or the owner's words as they wrote them; for a design card and a ruling on it, which
/// card, leading to it on its work's page; for a notice of cache misses, how many it holds.
function Said({ e }: { e: ConversationEntry }) {
  if (e.source === 'card' || e.source === 'ruling') {
    return (
      <p className="talk-text">
        {t(e.source === 'card' ? 'conv_card' : 'conv_ruling', { id: e.design_card ?? e.id, work: e.work ?? '' })}{' '}
        {e.work !== null ? (
          <Link to={workHref(e.work, { tab: 'cards' })}>{t('conv_see_card')}</Link>
        ) : null}
      </p>
    );
  }
  return (
    <>
      {e.text !== '' ? <Markdown text={e.text} /> : null}
      {e.source === 'ask' && e.ask !== null ? <p className="muted talk-text">{t('conv_ask', { id: e.ask })}</p> : null}
      {e.misses !== null ? <MissesLine m={e.misses} /> : null}
    </>
  );
}

/// One entry of the stream: who it is from and to, when, the round it came with, whether the owner's words in it wait
/// to be taken up, and what it says; the owner's own on the right.
function Entry({ e }: { e: ConversationEntry }) {
  const own = e.sender === 'owner';
  const look = ['talk', own ? 'talk-owner' : 'talk-other', e.agents ? 'talk-agents' : null, e.fault !== null ? 'talk-fault' : null];
  return (
    <div className={look.filter((x) => x !== null).join(' ')}>
      <div className="talk-head muted">
        <span className="talk-who">
          {party(e, e.sender)} → {party(e, e.recipient)}
        </span>
        <span>{time(e.at)}</span>
        {e.work !== null && e.round !== null && e.source !== 'between' ? (
          <Link to={workHref(e.work, { round: e.round })}>
            R{e.work}-{e.round}
          </Link>
        ) : null}
        {e.pending && own ? <span className="chip waits">{t('conv_pending')}</span> : null}
      </div>
      <Said e={e} />
    </div>
  );
}

/// A fault of wf-agents not read yet, pinned: what the engine said, how many misses it holds, and Mark read.
function PinnedFault({ e }: { e: ConversationEntry }) {
  const [error, setError] = useState<string | null>(null);
  return (
    <div className="talk talk-fault talk-pinned">
      <div className="talk-head muted">
        <span className="badge verdict-bad" title={t('message_fault_hint')}>
          {t('message_fault', { kind: t(e.fault === 'flow_anomaly' ? 'fault_flow_anomaly' : 'fault_cache_miss') })}
        </span>
        <span>{time(e.at)}</span>
        <button
          type="button"
          className="link"
          onClick={() => {
            markRead({ ids: [], notices: [e.id] }).catch((x: unknown) => setError(x instanceof Error ? x.message : String(x)));
          }}
        >
          {t('message_mark_read')}
        </button>
      </div>
      <Said e={e} />
      {error !== null ? <p className="bad">{t('request_failed', { error })}</p> : null}
    </div>
  );
}

/// The stream, oldest first: what is to or from the owner one by one, and each run of what passes between agents
/// folded to one line until the owner opens them.
function Stream({ stream }: { stream: ConversationEntry[] }) {
  const [agents, setAgents] = useState(false);
  return (
    <>
      {runsOf(stream).map((run) =>
        run.agents && !agents ? (
          <button
            key={`${run.entries[0].source}-${run.entries[0].id}-${run.entries[0].at}`}
            type="button"
            className="link talk-folded"
            onClick={() => setAgents(true)}
          >
            {t('conv_between_agents', { n: run.entries.length })}
          </button>
        ) : (
          <Fragment key={`${run.entries[0].source}-${run.entries[0].id}-${run.entries[0].at}`}>
            {run.entries.map((e) => (
              <Entry key={`${e.source}-${e.id}-${e.at}`} e={e} />
            ))}
          </Fragment>
        ),
      )}
      {agents && stream.some((e) => e.agents) ? (
        <button type="button" className="link talk-folded" onClick={() => setAgents(false)}>
          {t('conv_fold_agents')}
        </button>
      ) : null}
    </>
  );
}

/// The box at the bottom of a conversation that says something: in an architect's, to that architect; in a work's, to
/// its engineer, or to the architect about the work. None in a conversation the owner says nothing in.
function SayBox({ conversation }: { conversation: string }) {
  const [text, setText] = useState('');
  const [to, setTo] = useState<'engineer' | 'architect'>('engineer');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const at = conversation.indexOf(':');
  const kind = conversation.slice(0, at);
  const subject = conversation.slice(at + 1);
  if (at < 0 || (kind !== 'architect' && kind !== 'work')) return null;
  const ready = !busy && text.trim() !== '';
  const send = () => {
    setBusy(true);
    setError(null);
    const sent =
      kind === 'architect'
        ? postJson(`/api/v1/architect/say?project=${encodeURIComponent(subject)}`, { text: text.trim() })
        : postJson(`/api/v1/works/${subject}/say`, { text: text.trim(), to });
    sent.then(
      () => {
        setBusy(false);
        setText('');
        reread('messages', 'architect');
      },
      (e: unknown) => {
        setBusy(false);
        setError(e instanceof Error ? e.message : String(e));
      },
    );
  };
  return (
    <div className="talk-say">
      {kind === 'work' ? (
        <select value={to} onChange={(e) => setTo(e.target.value === 'architect' ? 'architect' : 'engineer')}>
          <option value="engineer">{t('conv_to_engineer')}</option>
          <option value="architect">{t('conv_to_architect')}</option>
        </select>
      ) : null}
      <textarea
        value={text}
        rows={2}
        placeholder={t(kind === 'architect' ? 'conv_say_architect' : to === 'engineer' ? 'conv_say_engineer' : 'conv_say_architect_work')}
        title={t('conv_send_hint')}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && ready) {
            e.preventDefault();
            send();
          }
        }}
        disabled={busy}
      />
      <button type="button" className="decide-button" onClick={send} disabled={!ready}>
        {t('conv_send')}
      </button>
      {error !== null ? <p className="bad talk-error">{t('request_failed', { error })}</p> : null}
    </div>
  );
}

/// One conversation: what waits for the owner pinned on top, with its form; the stream, newest at the bottom and
/// scrolled to; under it, while the owner's words wait to be taken up, why, when the place showing it can say (`note`);
/// and the box that says more. Opening it marks its messages and notices read.
export function ConversationView({ conversation, note }: {
  conversation: string;
  note?: (entries: ConversationEntry[]) => string | null;
}) {
  const r = useResource<ConversationDetail>(
    'messages',
    `/api/v1/conversation?key=${encodeURIComponent(conversation)}`,
  );
  const d = r.data !== undefined && r.data.key === conversation ? r.data : null;
  const list = useRef<HTMLDivElement>(null);
  const shown = d?.entries.length ?? 0;
  useEffect(() => {
    const el = list.current;
    if (el !== null) el.scrollTop = el.scrollHeight;
  }, [conversation, shown]);
  const marks = d !== null ? toMarkRead(d) : { ids: [], notices: [] };
  const unread = [...marks.ids.map((x) => `m${x}`), ...marks.notices.map((x) => `n${x}`)].join(',');
  useEffect(() => {
    if (unread === '') return;
    const ids = unread.split(',').filter((x) => x.startsWith('m')).map((x) => Number(x.slice(1)));
    const notices = unread.split(',').filter((x) => x.startsWith('n')).map((x) => Number(x.slice(1)));
    markRead({ ids, notices }).catch(() => {
      // Read again the next time the conversation is: nothing is lost.
    });
  }, [unread]);
  if (d === null) return <p className="muted talk-view">{r.error ?? t('loading')}</p>;
  const { faults, stream } = pinnedOf(d);
  const pinned = faults.length + d.asks.length + d.design_cards.length;
  const why = note !== undefined && pendingWords(d.entries).length > 0 ? note(d.entries) : null;
  return (
    <div className="talk-view">
      {pinned > 0 ? (
        <div className="talk-pins" aria-label={t('conv_waits_for_you')}>
          <div className="talk-pins-h">{t('conv_waits_for_you')}</div>
          {faults.map((e) => (
            <PinnedFault key={e.id} e={e} />
          ))}
          {d.asks.map((a) =>
            conversation.startsWith('architect:') ? (
              <div key={a.id} id={`ask-${a.id}`} className="boxed">
                <ArchitectAsk a={a} />
              </div>
            ) : (
              <AskView key={a.id} a={a} />
            ),
          )}
          {d.design_cards.map((c) => (
            <DesignCardView
              key={c.id}
              c={c}
              round={
                <Link to={workHref(c.work, { round: c.round })}>
                  R{c.work}-{c.round}
                </Link>
              }
            />
          ))}
        </div>
      ) : null}
      <div className="talk-stream" ref={list}>
        {stream.length === 0 ? <p className="muted">{t('conv_none')}</p> : <Stream stream={stream} />}
        {why !== null ? <p className="muted talk-note">{why}</p> : null}
      </div>
      <SayBox key={conversation} conversation={conversation} />
    </div>
  );
}

/// Where a conversation's subject is shown in full: the work's page, the architect's page.
function SubjectLink({ c }: { c: Conversation }) {
  if (c.work !== null) return <Link to={workHref(c.work)}>{t('conv_open_work')}</Link>;
  if (c.key.startsWith('architect:')) {
    return <Link to={architectHref({ project: c.key.slice('architect:'.length) })}>{t('conv_open_architect')}</Link>;
  }
  return null;
}

/// The owner's conversations: the list on the left, in groups by project when there are several, those where something
/// waits for the owner first and marked; the conversation open on the right, the first in the list when none is named.
export function Messages({ open }: { open: string | null }) {
  const r = useResource<Conversation[]>('messages', '/api/v1/conversations');
  const groups = conversationGroups(r.data ?? []);
  const all = groups.flatMap((g) => g.conversations);
  const shown = all.find((c) => c.key === open) ?? all[0] ?? null;
  return (
    <div className="talks">
      <nav className="talks-list" aria-label={t('nav_messages')}>
        {r.error !== null ? <p className="bad">{r.error}</p> : null}
        {r.data !== undefined && all.length === 0 ? <p className="muted">{t('conv_list_none')}</p> : null}
        {groups.map((g) => (
          <section key={g.project ?? ''} className="talks-group">
            {groups.length > 1 ? (
              <div className="talks-group-h">
                <span>{g.project ?? t('conv_across_projects')}</span>
                {g.waiting > 0 ? <span className="nav-count nav-fault">{g.waiting}</span> : null}
                {g.unread > 0 ? <span className="nav-count">{g.unread}</span> : null}
              </div>
            ) : null}
            {g.conversations.map((c) => (
              <Link
                key={c.key}
                to={href({ page: 'messages', open: c.key })}
                className={`talks-item${c.key === shown?.key ? ' selected' : ''}${c.waiting > 0 ? ' talks-waits' : ''}`}
              >
                <span className="talks-title">{conversationTitle(c)}</span>
                <span className="talks-meta">
                  {c.waiting > 0 ? (
                    <span className="chip waits" title={t('conv_waits_n', { n: c.waiting })}>
                      {t('conv_waits_chip', { n: c.waiting })}
                    </span>
                  ) : null}
                  {c.unread > 0 ? <span className="nav-count">{c.unread}</span> : null}
                  <span className="muted">{time(c.last_at)}</span>
                </span>
              </Link>
            ))}
          </section>
        ))}
      </nav>
      <section className="talks-open">
        {shown !== null ? (
          <>
            <div className="talks-open-h">
              <strong>{conversationTitle(shown)}</strong>
              <SubjectLink c={shown} />
            </div>
            <ConversationView key={shown.key} conversation={shown.key} />
          </>
        ) : null}
      </section>
    </div>
  );
}
