// The owner's conversations, the one place on the page the owner acts: their list on the left, the one open on the
// right. The list's order never changes for what waits: the one-to-one with each project's architect and wf-agents'
// own first, then a group for each work, newest first, which greys out in place once the work ends, then the groups of
// projects the configuration connects; what waits for the owner and what is unread are marks on each, red and grey.
// A work's group holds the owner, its engineer and its project's architect: everything said in it reaches all three,
// each agent's message beside its avatar in its colour and headed by whom it is for, the owner's on the right; the
// engine's notices are grey lines, and its stage cards, one for each stretch of the work at a stage, updated in place,
// the design's with the design folded and the design card to rule on. The one-to-one holds the owner's words, what the
// architect tells and asks, a card for each of its matters and the policy as it took effect, under its queue. What
// waits for the owner (the open blockers the owner holds) is pinned at the top with its options, and the one box at the
// bottom answers the one picked, as a reply does in a chat; otherwise it says something, in a work's group to the
// engineer, the architect or both. Opening a conversation reads it up to its latest entry.

import {
  CommentDiscussionIcon,
  CpuIcon,
  ProjectRoadmapIcon,
  ReplyIcon,
  StackIcon,
  ToolsIcon,
  XIcon,
} from '@primer/octicons-react';
import {
  Button,
  Checkbox,
  CounterLabel,
  Flash,
  FormControl,
  IconButton,
  NavList,
  PageLayout,
  Select,
  Textarea,
  Timeline,
} from '@primer/react';
import { useEffect, useMemo, useRef, useState, type MouseEvent, type ReactNode } from 'react';

import type { Conversation, ConversationDetail, ConversationEntry, JsonValue, Misses, OwnerAsk, WorkSummary } from '../api';
import { type Answering, AnsweringContext, type Target } from '../answer';
import { choicesOf, rulingAnswer } from '../architect';
import {
  closedWhy,
  conversationTitle,
  heldCards,
  latestAt,
  listParts,
  mention,
  party,
  pendingWords,
  sayTo,
  streamOf,
  voiceOf,
} from '../conversation';
import { cost, time } from '../format';
import { postJson } from '../http';
import { isObj, str, words } from '../labelled';
import { reread, useResource } from '../live';
import { Markdown } from '../markdown';
import { forProject, href, Link, navigate, workHref } from '../route';
import { t } from '../text';
import { ArchitectAskBlock } from './ArchitectAsk';
import { AskView, DesignCardView } from './Decide';
import { ArchitectHead, PlainHead, WorkHead } from './TalkHead';
import { MatterCardView, ObservationCard, PolicyCard, StageCardView } from './TalkCards';

/// Marks a fault of wf-agents read, where it shows.
async function markFault(id: number): Promise<void> {
  await postJson('/api/v1/messages/read', { ids: [], notices: [id] });
  reread('messages');
}

/// The mark beside a conversation in the list, by its kind: an architect's plan, wf-agents' processor, a group's
/// discussion, the stack of connected projects.
function KindIcon({ kind }: { kind: string }) {
  if (kind === 'architect') return <ProjectRoadmapIcon />;
  if (kind === 'wf_agents') return <CpuIcon />;
  if (kind === 'projects') return <StackIcon />;
  return <CommentDiscussionIcon />;
}

/// The list of conversations in its fixed order, each with what waits for the owner in it (red) and what is unread
/// (grey); a work's group over greyed in place.
function List({ list, open }: { list: Conversation[]; open: string | null }) {
  return (
    <NavList aria-label={t('nav_messages')} className="talks-list">
      {listParts(list).map((p) => (
        <NavList.Group key={p.part} title={p.part === 'works' ? t('talks_works') : p.part === 'projects' ? t('talks_projects') : undefined}>
          {p.conversations.map((c) => (
            <NavList.Item
              key={c.key}
              href={href({ page: 'messages', open: c.key })}
              aria-current={c.key === open ? 'page' : undefined}
              className={c.over ? 'talks-over' : undefined}
              onClick={(e: MouseEvent) => {
                if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
                e.preventDefault();
                navigate(href({ page: 'messages', open: c.key }));
              }}
            >
              <NavList.LeadingVisual>
                <KindIcon kind={c.kind} />
              </NavList.LeadingVisual>
              <span className="talks-title" title={conversationTitle(c)}>
                {conversationTitle(c)}
              </span>
              {c.waiting > 0 || c.unread > 0 ? (
                <NavList.TrailingVisual>
                  <span className="talks-marks">
                    {c.waiting > 0 ? (
                      <CounterLabel variant="primary" title={t('conv_waits_n', { n: c.waiting })}>
                        {c.waiting}
                      </CounterLabel>
                    ) : null}
                    {c.unread > 0 ? <CounterLabel title={t('nav_unread', { n: c.unread })}>{c.unread}</CounterLabel> : null}
                  </span>
                </NavList.TrailingVisual>
              ) : null}
            </NavList.Item>
          ))}
        </NavList.Group>
      ))}
    </NavList>
  );
}

/// An agent's avatar: its line's mark in its colour, the engineer's tools and the architect's plan.
function Avatar({ who }: { who: string }) {
  const architect = who === 'architect' || who.startsWith('architect');
  return (
    <span className={architect ? 'talk-avatar talk-avatar-architect' : 'talk-avatar talk-avatar-engineer'} aria-hidden="true">
      {architect ? <ProjectRoadmapIcon /> : <ToolsIcon />}
    </span>
  );
}

/// The cache misses a notice of a fault holds, when more than its first.
function MissesLine({ m }: { m: Misses }) {
  if (m.count <= 1) return null;
  return (
    <p className="talk-line-more">
      {t('message_misses', {
        n: m.count,
        written: m.written.toLocaleString('en-US'),
        cost: cost(m.cost_usd),
        at: time(m.last_at),
      })}
    </p>
  );
}

/// What a message of the owner's or an agent's says: its words as written, in the page's markdown; a ruling, which
/// card; an ask once answered, folded under its question in full.
function Words({ e }: { e: ConversationEntry }) {
  if (e.source === 'ruling') return <p>{t('conv_ruling', { id: e.design_card ?? e.id })}</p>;
  return (
    <>
      {e.text !== '' ? <Markdown text={e.text} /> : null}
      {e.ask_detail !== null ? (
        <details className="talk-fold">
          <summary>{t('conv_ask_full', { id: e.ask_detail.id })}</summary>
          <div className="talk-fold-body">
            <AskIn a={e.ask_detail} revisions={e.revisions} />
          </div>
        </details>
      ) : null}
    </>
  );
}

/// An ask, in the look its kind has: an architect's (a question, a choice, a study, a policy to approve) or an
/// engineer's.
function AskIn({ a, revisions = null }: { a: OwnerAsk; revisions?: JsonValue }) {
  const architect = isObj(a.body) && a.body.from === 'architect';
  return architect ? (
    <ArchitectAskBlock a={a} revisions={revisions} />
  ) : (
    <AskView a={a} round={<Link to={workHref(a.work, { round: a.round })}>{`R${a.work}-${a.round}`}</Link>} />
  );
}

/// One message of the owner's or an agent's: who it is from, to whom (in a group, at its head), when, its round, and
/// what it says; the owner's on the right in the look of what is sent to an agent, an agent's beside its avatar in the
/// look of what an agent handed back, in its colour. The owner's words not taken up yet, and those never taken in,
/// say so on themselves; a question the architect holds can be answered here.
function Message({ e, group, answering }: { e: ConversationEntry; group: boolean; answering: Answering | null }) {
  const own = e.sender === 'owner';
  const from = e.source === 'between' ? `architect:${e.sender}` : e.sender;
  const round = e.work !== null && e.round !== null && e.source !== 'between';
  return (
    <div className={own ? 'talk-row talk-row-owner' : 'talk-row'}>
      {own ? null : <Avatar who={from} />}
      <div className={own ? 'talk-bubble talk-bubble-owner' : `talk-bubble talk-bubble-${from.startsWith('architect') ? 'architect' : 'engineer'}`}>
        <div className="talk-bubble-head">
          {own ? null : <span className="talk-who">{party(e, e.sender)}</span>}
          {group || e.source === 'between' ? <span className="talk-mention">{mention(e)}</span> : null}
          <span className="talk-when">{time(e.at)}</span>
          {round ? (
            <Link to={workHref(e.work ?? 0, { round: e.round ?? 0 })} muted>
              {`R${e.work}-${e.round}`}
            </Link>
          ) : null}
          {e.pending && own ? <span className="talk-state">{t('conv_pending')}</span> : null}
          {e.undelivered ? <span className="talk-state talk-state-bad">{t('conv_undelivered')}</span> : null}
        </div>
        <div className="talk-bubble-body">
          <Words e={e} />
        </div>
        {e.answerable && answering !== null && e.work !== null && e.matter !== null ? (
          <div className="talk-bubble-actions">
            <Button
              size="small"
              leadingVisual={ReplyIcon}
              onClick={() => answering.set({ kind: 'question', work: e.work ?? 0, matter: e.matter ?? 0, text: e.text })}
            >
              {t('conv_answer_question')}
            </Button>
          </div>
        ) : null}
      </div>
    </div>
  );
}

/// A line of the page's own, small and grey, for what the engine says, and what a helper of the work told the owner.
function SystemLine({ e }: { e: ConversationEntry }) {
  const helper = e.sender !== 'engine';
  return (
    <div className="talk-line">
      <CpuIcon className="talk-line-icon" />
      <span className="talk-line-text">
        {helper ? `${t('conv_helper_said', { line: words(e.sender) })} ` : null}
        {e.source === 'card' ? t('conv_card', { id: e.design_card ?? e.id }) : e.text}
      </span>
      <span className="talk-when">{time(e.at)}</span>
    </div>
  );
}

/// A fault of wf-agents: what the engine said, how many misses it holds, and, unread, Mark read.
function Fault({ e }: { e: ConversationEntry }) {
  const [error, setError] = useState<string | null>(null);
  return (
    <Flash variant={e.read_at === null ? 'warning' : 'default'} className="talk-fault">
      <div className="talk-bubble-head">
        <strong>
          {t('message_fault', { kind: t(e.fault === 'flow_anomaly' ? 'fault_flow_anomaly' : 'fault_cache_miss') })}
        </strong>
        <span className="talk-when">{time(e.at)}</span>
        {e.read_at === null ? (
          <Button
            size="small"
            onClick={() => {
              markFault(e.id).catch((x: unknown) => setError(x instanceof Error ? x.message : String(x)));
            }}
          >
            {t('message_mark_read')}
          </Button>
        ) : null}
      </div>
      <Markdown text={e.text} />
      {e.misses !== null ? <MissesLine m={e.misses} /> : null}
      {error !== null ? <p className="bad">{t('request_failed', { error })}</p> : null}
    </Flash>
  );
}

/// One entry of the stream, in its voice.
function Entry({ e, c, w, held, current, answering }: {
  e: ConversationEntry;
  c: Conversation;
  w: WorkSummary | null;
  held: ReadonlySet<number>;
  current: number | null;
  answering: Answering | null;
}) {
  const voice = voiceOf(e, held);
  if (voice === 'card') {
    if (e.stage !== null) return <StageCardView e={e} s={e.stage} w={w} />;
    if (e.matter_card !== null) {
      return <MatterCardView m={e.matter_card} project={c.project ?? ''} current={e.matter_card.matter === current} />;
    }
    if (e.policy !== null) return <PolicyCard p={e.policy} />;
    if (e.observation !== null) return <ObservationCard o={e.observation} />;
    return null;
  }
  if (voice === 'system') return <SystemLine e={e} />;
  if (voice === 'fault') return <Fault e={e} />;
  if (voice === 'none') return null;
  return <Message e={e} group={c.kind === 'work' || c.kind === 'projects'} answering={answering} />;
}

/// What waits for the owner, pinned at the top of the conversation with its options: the asks that stand for the open
/// blockers the owner holds, and the design card whose ruling a work waits for before its merge. An ask is answered
/// with the box at the bottom, the option or the ruling picked on it.
function Pinned({ d }: { d: ConversationDetail }) {
  if (d.asks.length === 0 && d.design_cards.length === 0) return null;
  return (
    <section className="talk-pins" aria-label={t('conv_waits_for_you')}>
      <h3 className="talk-pins-title">{t('conv_waits_for_you')}</h3>
      {d.asks.map((a) => (
        <AskIn key={a.id} a={a} revisions={d.entries.find((e) => e.source === 'ask' && e.id === a.id)?.revisions ?? null} />
      ))}
      {d.design_cards.map((c) => (
        <DesignCardView key={c.id} c={c} round={<Link to={workHref(c.work, { round: c.round })}>{`R${c.work}-${c.round}`}</Link>} />
      ))}
    </section>
  );
}

/// What the box sends to answer an ask: an architect's choice (a study, a policy to approve) with its ruling picked and
/// words; an engineer's ask with the option picked and words, or handed back to the architect with the words as the
/// decision it carries out. None while it cannot be sent.
function askAnswer(target: Extract<Target, { kind: 'ask' }>, text: string, back: boolean): object | null {
  const body = isObj(target.ask.body) ? target.ask.body : {};
  const kind = str(body.kind);
  if (body.from === 'architect' && (kind === 'study' || kind === 'policy_approval')) {
    return rulingAnswer(choicesOf(target.ask.body), target.ruling === '' ? null : target.ruling, text);
  }
  const words = text.trim();
  const answer = target.option !== '' ? `${t('answer_chosen', { option: target.option })}${words !== '' ? ` ${words}` : ''}` : words;
  if (answer === '') return null;
  return back ? { text: answer, to: 'architect' } : { text: answer };
}

/// The one box at the bottom of a conversation: it says something (in a work's group to the engineer, the architect or
/// both; in a group of projects to one architect or every one; in the one-to-one to the architect), or, switched like a
/// reply in a chat, answers what is picked: an ask pinned above, or a question the architect holds. Closed, it says why:
/// a work at its end no longer changes its way on.
function Composer({ c, w, answering }: { c: Conversation; w: WorkSummary | null; answering: Answering }) {
  const choices = sayTo(c);
  const [text, setText] = useState('');
  const [to, setTo] = useState(choices[0] ?? '');
  const [back, setBack] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const target = answering.target;
  const closed = target === null ? closedWhy(c, w) : null;
  if (choices.length === 0 && target === null) return null;
  const handsBack = target !== null && target.kind === 'ask' && isObj(target.ask.body) && target.ask.body.to === 'owner';
  const answer = target !== null && target.kind === 'ask' ? askAnswer(target, text, handsBack && back) : null;
  const ready = closed === null && (target !== null && target.kind === 'ask' ? answer !== null : text.trim() !== '');
  const done = () => {
    setBusy(false);
    setText('');
    setBack(false);
    answering.set(null);
    reread('messages', 'architect', 'works', 'work:*');
  };
  const failed = (e: unknown) => {
    setBusy(false);
    setError(e instanceof Error ? e.message : String(e));
  };
  const send = () => {
    if (!ready) return;
    setBusy(true);
    setError(null);
    const words = text.trim();
    let sent: Promise<void>;
    if (target !== null && target.kind === 'ask') {
      sent = postJson(`/api/v1/asks/${target.ask.id}/answer`, answer);
    } else if (target !== null) {
      sent = postJson(`/api/v1/works/${target.work}/say`, { text: words, question: target.matter });
    } else if (c.kind === 'work') {
      sent = postJson(`/api/v1/works/${c.work ?? 0}/say`, { text: words, to });
    } else if (c.kind === 'projects') {
      sent = postJson('/api/v1/projects/say', to === 'all' ? { key: c.key, text: words } : { key: c.key, text: words, to });
    } else {
      sent = postJson(forProject('/api/v1/architect/say', c.project), { text: words });
    }
    sent.then(done, failed);
  };
  const toWords = (x: string) =>
    x === 'all'
      ? t('conv_to_every_architect')
      : x === 'both'
        ? t('conv_to_both')
        : c.kind === 'projects'
          ? t('conv_to_architect_of', { project: x })
          : `@${words(x)}`;
  return (
    <div className="talk-say">
      {target !== null ? (
        <div className="talk-say-target">
          <ReplyIcon />
          <span>
            {target.kind === 'ask'
              ? t('conv_answering_ask', { id: target.ask.id })
              : t('conv_answering_question', { work: target.work })}
            {target.kind === 'ask' && target.option !== '' ? ` · ${t('answer_picked', { option: target.option })}` : null}
            {target.kind === 'ask' && target.ruling !== '' ? ` · ${words(target.ruling)}` : null}
          </span>
          <IconButton
            size="small"
            variant="invisible"
            icon={XIcon}
            aria-label={t('conv_answer_cancel')}
            onClick={() => answering.set(null)}
          />
        </div>
      ) : null}
      {closed !== null ? <p className="talk-say-closed">{closed}</p> : null}
      <div className="talk-say-row">
        {target === null && choices.length > 1 ? (
          <Select size="small" value={to} aria-label={t('conv_to')} onChange={(e) => setTo(e.target.value)} disabled={closed !== null}>
            {choices.map((x) => (
              <Select.Option key={x} value={x}>
                {toWords(x)}
              </Select.Option>
            ))}
          </Select>
        ) : null}
        <Textarea
          block
          resize="vertical"
          rows={2}
          value={text}
          disabled={busy || closed !== null}
          placeholder={
            target !== null
              ? t('conv_answer_words')
              : c.kind === 'work'
                ? t(to === 'engineer' ? 'conv_say_engineer' : to === 'architect' ? 'conv_say_architect_work' : 'conv_say_both')
                : t('conv_say_architect')
          }
          title={t('send_keys_hint')}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
              e.preventDefault();
              send();
            }
          }}
        />
        <Button variant="primary" onClick={send} disabled={!ready || busy}>
          {target !== null ? t('answer_send') : t('conv_send')}
        </Button>
      </div>
      {handsBack ? (
        <FormControl>
          <Checkbox checked={back} onChange={(e) => setBack(e.target.checked)} />
          <FormControl.Label>{t('answer_to_architect')}</FormControl.Label>
          <FormControl.Caption>{t('answer_to_architect_hint')}</FormControl.Caption>
        </FormControl>
      ) : null}
      {error !== null ? <Flash variant="danger">{t('request_failed', { error })}</Flash> : null}
    </div>
  );
}

/// The stream of a conversation, oldest first, scrolled to its newest: the engine's cards and lines in the page's own
/// voice, the members' messages beside them; under it, while the owner's words wait to be taken up in the one-to-one,
/// that they do.
function Stream({ d, c, w, answering }: { d: ConversationDetail; c: Conversation; w: WorkSummary | null; answering: Answering }) {
  const held = useMemo(() => heldCards(d), [d]);
  const stream = streamOf(d);
  const current = d.entries.find((e) => e.matter_card?.state === 'working')?.matter_card?.matter ?? null;
  const list = useRef<HTMLDivElement>(null);
  const shown = stream.length;
  useEffect(() => {
    const el = list.current;
    if (el !== null) el.scrollTop = el.scrollHeight;
  }, [c.key, shown]);
  // Cards run in a Timeline of their own between the messages: each run of cards and lines is one Timeline.
  const runs: Array<{ cards: boolean; entries: ConversationEntry[] }> = [];
  for (const e of stream) {
    const v = voiceOf(e, held);
    const cards = v === 'card' || v === 'system';
    const last = runs[runs.length - 1];
    if (last !== undefined && last.cards === cards) last.entries.push(e);
    else runs.push({ cards, entries: [e] });
  }
  const pending = c.kind === 'architect' && pendingWords(d.entries).length > 0;
  return (
    <div className="talk-stream" ref={list}>
      {stream.length === 0 ? <p className="talk-none">{t('conv_none')}</p> : null}
      {runs.map((run) => {
        const key = `${run.entries[0].source}-${run.entries[0].id}-${run.entries[0].at}`;
        const items: ReactNode = run.entries.map((e) => (
          <Entry key={`${e.source}-${e.id}-${e.at}`} e={e} c={c} w={w} held={held} current={current} answering={answering} />
        ));
        return run.cards ? (
          <Timeline key={key} clipSidebar className="talk-cards">
            {items}
          </Timeline>
        ) : (
          <div key={key} className="talk-messages">
            {items}
          </div>
        );
      })}
      {pending ? <p className="talk-none">{t('conv_words_wait')}</p> : null}
    </div>
  );
}

/// One conversation: its head with the owner's controls, what waits for the owner pinned under it, the stream, and the
/// one box at the bottom; reading it marks it read up to its latest entry.
function Open({ c }: { c: Conversation }) {
  const r = useResource<ConversationDetail>('messages', `/api/v1/conversation?key=${encodeURIComponent(c.key)}`);
  const works = useResource<WorkSummary[]>('works', '/api/v1/works');
  const d = r.data !== undefined && r.data.key === c.key ? r.data : null;
  const w = c.work !== null ? ((works.data ?? []).find((x) => x.id === c.work) ?? null) : null;
  const [target, setTarget] = useState<Target | null>(null);
  const answering = useMemo<Answering>(() => ({ target, set: setTarget }), [target]);
  const latest = d !== null ? latestAt(d) : null;
  const readTo = d?.read_to ?? null;
  useEffect(() => {
    if (latest === null || (readTo !== null && readTo >= latest)) return;
    postJson('/api/v1/conversations/read', { key: c.key, at: latest }).then(
      () => reread('messages'),
      () => {
        // Read again the next time the conversation is: nothing is lost.
      },
    );
  }, [c.key, latest, readTo]);
  const head =
    c.kind === 'work' ? <WorkHead c={c} w={w} /> : c.kind === 'architect' ? <ArchitectHead c={c} /> : <PlainHead title={conversationTitle(c)} />;
  return (
    <AnsweringContext.Provider value={answering}>
      <div className="talk-open">
        {head}
        {d === null ? (
          <p className="talk-none">{r.error ?? t('loading')}</p>
        ) : (
          <>
            <Pinned d={d} />
            <Stream d={d} c={c} w={w} answering={answering} />
          </>
        )}
        <Composer key={c.key} c={c} w={w} answering={answering} />
      </div>
    </AnsweringContext.Provider>
  );
}

/// The owner's conversations: the list on the left, in its fixed order, its width dragged and kept in the browser; the
/// conversation open on the right, the first in the list when the address names none.
export function Messages({ open }: { open: string | null }) {
  const r = useResource<Conversation[]>('messages', '/api/v1/conversations');
  const list = r.data ?? [];
  const shown = list.find((c) => c.key === open) ?? list[0] ?? null;
  return (
    <PageLayout containerWidth="full" padding="none" rowGap="none" columnGap="none" className="talks">
      <PageLayout.Pane
        position="start"
        resizable
        widthStorageKey="wf-agents.talks-pane"
        width={{ min: '220px', default: '320px', max: '640px' }}
        divider="line"
        padding="none"
        className="talks-pane"
        aria-label={t('nav_messages')}
      >
        {r.error !== null ? <Flash variant="danger">{r.error}</Flash> : null}
        {r.data !== undefined && list.length === 0 ? <p className="talk-none">{t('conv_list_none')}</p> : null}
        <List list={list} open={shown?.key ?? null} />
      </PageLayout.Pane>
      <PageLayout.Content padding="none" width="full" className="talks-content">
        {shown !== null ? <Open key={shown.key} c={shown} /> : null}
      </PageLayout.Content>
    </PageLayout>
  );
}
