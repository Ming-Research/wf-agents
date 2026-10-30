// The owner's conversations, the one place on the page the owner acts: their list on the left, the one open on the
// right. The list's order never changes for what waits: the one-to-one with each project's architect and wf-agents'
// own first, then a group for each work, newest first, which greys out in place once the work ends, then the groups of
// projects the configuration connects; what waits for the owner and what is unread are marks on each, red and grey.
// A work's group holds the owner, its engineer and its project's architect: everything said in it reaches all three,
// each agent's message beside its avatar in its colour and headed by whom it is for, the owner's on the right; the
// engine's notices are grey lines, and its stage cards, one for each stretch of the work at a stage, updated in place,
// the design's with the design folded and the design card to rule on. The one-to-one holds the owner's words, what the
// architect tells and asks, a card for each of its matters and the policy as it took effect, under its queue.
// wf-agents' own holds its faults, each of which the owner forwards to any project's architect, once to each, and says
// where it went and how it stands there; its head sets where each new fault goes as it comes, nobody at first. An ask
// of the owner shows once, where it was asked, as the asking agent's bubble, in full while it waits, and is answered in
// that bubble: the buttons of its options or of the choices its cell offers, the one picked marked, then a small box
// for the owner's words, free unless the ask or the choice asks for them, and Answer; a question the architect holds
// has the small box in its bubble too. What is written there answers that bubble; once answered, the buttons and the
// box give way to the owner's answer, in the same bubble and named as the owner's answer to it, which shows nowhere
// else. While an ask that waits is out of view, a slim line over the stream says so and brings it into view. A design
// card a work waits on to be ruled is pinned at the top. The one box at the bottom only says something, in a work's
// group to the engineer, the architect or both, and in the one-to-one, while something of the architect runs or waits
// for the owner, either interjects into it, which the architect answers there as it goes on, or adds to its queue.
// Opening a conversation reads it up to its latest entry.

import {
  ArrowDownIcon,
  ArrowUpIcon,
  CheckIcon,
  CommentDiscussionIcon,
  CpuIcon,
  ProjectRoadmapIcon,
  StackIcon,
  ToolsIcon,
} from '@primer/octicons-react';
import {
  ActionList,
  ActionMenu,
  Button,
  Checkbox,
  CounterLabel,
  Flash,
  FormControl,
  NavList,
  PageLayout,
  Textarea,
  Timeline,
} from '@primer/react';
import { useEffect, useMemo, useRef, useState, type MouseEvent, type ReactNode, type RefObject } from 'react';

import type {
  Conversation,
  ConversationDetail,
  ConversationEntry,
  Forwarded,
  Forwarding,
  JsonValue,
  Misses,
  OwnerAsk,
  WorkSummary,
} from '../api';
import { AnswersHere, answerWay, askAnswer, enterSends, handsBack, needsWords, pickAfter } from '../answer';
import {
  awayOf,
  closedWhy,
  conversationTitle,
  type ForwardChoice,
  forwardMenu,
  forwardParts,
  heldCards,
  isQuestion,
  latestAt,
  listParts,
  member,
  mention,
  mentions,
  ownerReplies,
  party,
  pendingWords,
  sayTo,
  sayToTicked,
  streamOf,
  voiceOf,
} from '../conversation';
import { cost, time } from '../format';
import { postJson } from '../http';
import { isObj, label, str, words } from '../labelled';
import { reread, useResource } from '../live';
import { Markdown } from '../markdown';
import { architectHref, forProject, href, Link, navigate, workHref } from '../route';
import { t, tn } from '../text';
import { ArchitectAskBlock } from './ArchitectAsk';
import { AskView, DesignCardView } from './Decide';
import { ArchitectHead, PlainHead, WfAgentsHead, WorkHead } from './TalkHead';
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
                      <CounterLabel variant="primary" title={tn('conv_waits_n', c.waiting)}>
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
      {tn('message_misses', m.count, {
        written: tn('n_tokens', m.written, { n: m.written.toLocaleString('en-US') }),
        cost: cost(m.cost_usd),
        at: time(m.last_at),
      })}
    </p>
  );
}

/// What a message of the owner's or an agent's says: its words as written, in the page's markdown; a ruling, which
/// card; an ask, in full while it waits, so that it is read where it is answered, and once answered its words with the
/// ask in full folded under them.
function Words({ e }: { e: ConversationEntry }) {
  if (e.source === 'ruling') return <p>{t('conv_ruling', { id: e.design_card ?? e.id })}</p>;
  if (e.ask_detail !== null && e.ask_detail.answer === null) return <AskIn a={e.ask_detail} revisions={e.revisions} />;
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

/// The small box in a bubble where the owner's words answer that bubble, beside Answer: Enter sends and Shift with
/// Enter starts a new line; what failed is said under it.
function AnswerBox({ placeholder, ready, busy, text, setText, send, error, children = null }: {
  placeholder: string;
  ready: boolean;
  busy: boolean;
  text: string;
  setText: (text: string) => void;
  send: () => void;
  error: string | null;
  children?: ReactNode;
}) {
  return (
    <>
      <div className="talk-answer-box">
        <Textarea
          block
          resize="vertical"
          rows={1}
          value={text}
          disabled={busy}
          placeholder={placeholder}
          aria-label={placeholder}
          title={t('send_keys_hint')}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (enterSends(e)) {
              e.preventDefault();
              if (ready && !busy) send();
            }
          }}
        />
        <Button size="small" onClick={send} disabled={!ready || busy}>
          {t('answer_send')}
        </Button>
      </div>
      {children}
      {error !== null ? <Flash variant="danger">{t('request_failed', { error })}</Flash> : null}
    </>
  );
}

/// What answering in a bubble needs: the words, whether they are being sent, what failed, and how to send them.
function useAnswer(): {
  text: string;
  setText: (text: string) => void;
  busy: boolean;
  error: string | null;
  post: (url: string, body: object) => void;
} {
  const [text, setText] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const post = (url: string, body: object) => {
    setBusy(true);
    setError(null);
    postJson(url, body).then(
      () => {
        setBusy(false);
        setText('');
        reread('messages', 'architect', 'works', 'work:*');
      },
      (e: unknown) => {
        setBusy(false);
        setError(e instanceof Error ? e.message : String(e));
      },
    );
  };
  return { text, setText, busy, error, post };
}

/// How an ask that waits is answered, in its bubble: the buttons of what it offers (the choices its cell offers, each
/// saying on hovering what it means and, once picked, under the buttons; or the options of its choice), a click picking
/// one and a second clearing it, the one picked marked; then the small box of the owner's words and Answer. The words
/// are needed for an ask in words alone and for a choice that asks for them, else they are free; an engineer's ask the
/// owner holds can be handed back to the architect with them.
function AskAnswer({ a }: { a: OwnerAsk }) {
  const way = answerWay(a);
  const [pick, setPick] = useState('');
  const [back, setBack] = useState(false);
  const { text, setText, busy, error, post } = useAnswer();
  const answer = askAnswer(a, pick, text, handsBack(a) && back);
  const meaning = way.by === 'ruling' ? (way.choices.find(([v]) => v === pick)?.[1] ?? '') : '';
  const button = (value: string, name: string, title?: string) => (
    <Button
      key={value}
      size="small"
      leadingVisual={pick === value ? CheckIcon : undefined}
      aria-pressed={pick === value}
      title={title}
      onClick={() => setPick(pickAfter(pick, value))}
    >
      {name}
    </Button>
  );
  return (
    <div className="talk-answer" role="group" aria-label={t('conv_answer_ask', { id: a.id })}>
      {way.by !== 'words' ? (
        <div className="talk-answer-buttons">
          {way.by === 'ruling' ? way.choices.map(([v, m]) => button(v, label(v), m)) : way.options.map((o) => button(o, o))}
        </div>
      ) : null}
      {meaning !== '' ? <p className="talk-said-note">{meaning}</p> : null}
      <AnswerBox
        placeholder={t(needsWords(way, pick) ? 'conv_answer_words' : 'conv_answer_more')}
        ready={answer !== null}
        busy={busy}
        text={text}
        setText={setText}
        send={() => {
          if (answer !== null) post(`/api/v1/asks/${a.id}/answer`, answer);
        }}
        error={error}
      >
        {handsBack(a) ? (
          <FormControl>
            <Checkbox checked={back} onChange={(e) => setBack(e.target.checked)} />
            <FormControl.Label>{t('answer_to_architect')}</FormControl.Label>
            <FormControl.Caption>{t('answer_to_architect_hint')}</FormControl.Caption>
          </FormControl>
        ) : null}
      </AnswerBox>
    </div>
  );
}

/// How a question of an engineer's that the architect holds is answered in the group, in its bubble: the small box of
/// the owner's words and Answer; the answer is the reply, and the architect is told.
function QuestionAnswer({ work, matter }: { work: number; matter: number }) {
  const { text, setText, busy, error, post } = useAnswer();
  return (
    <div className="talk-answer">
      <AnswerBox
        placeholder={t('conv_answer_words')}
        ready={text.trim() !== ''}
        busy={busy}
        text={text}
        setText={setText}
        send={() => {
          if (text.trim() !== '') post(`/api/v1/works/${work}/say`, { text: text.trim(), question: matter });
        }}
        error={error}
      />
    </div>
  );
}

/// The owner's answer inside the bubble it answers, named as the owner's answer to it so that it is not read as other
/// words of the owner's: what it answers and when, then the choice made and the words, in the look of what is sent to
/// an agent.
function AnswerIn({ head, at, choice = '', text }: { head: string; at: number | null; choice?: string; text: string }) {
  return (
    <div className="talk-answer">
      <div className="talk-bubble-head">
        <span className="talk-who">{head}</span>
        {at !== null ? <span className="talk-when">{time(at)}</span> : null}
      </div>
      <div className="talk-bubble talk-bubble-owner">
        <div className="talk-bubble-body">
          {choice !== '' ? (
            <p>
              <strong>{label(choice)}</strong>
            </p>
          ) : null}
          {text !== '' ? <Markdown text={text} /> : null}
        </div>
      </div>
    </div>
  );
}

/// What an ask once answered holds of its answer: the owner's, as the owner's answer to that ask; or, closed by the
/// engine, why no answer was needed, in the page's own voice.
function AskAnswered({ a }: { a: OwnerAsk }) {
  const ans = isObj(a.answer) ? a.answer : {};
  if (str(ans.by) === 'engine') {
    return (
      <div className="talk-answer">
        <p className="talk-said-note">{str(ans.text)}</p>
      </div>
    );
  }
  return (
    <AnswerIn
      head={t('conv_your_answer', { id: a.id })}
      at={typeof ans.at === 'number' ? ans.at : null}
      choice={str(ans.choice)}
      text={str(ans.text)}
    />
  );
}

/// One message of the owner's or an agent's: who it is from, to whom (in a group, at its head), when, its round, and
/// what it says; the owner's on the right in the look of what is sent to an agent, an agent's beside its avatar in the
/// look of what an agent handed back, in its colour; what the architect's policy setting says, as the architect's. The
/// owner's words not taken up yet, and those never taken in, say so on themselves. Each answer to a bubble is in that
/// bubble: an ask holds what answers it while it waits, then the owner's answer; a question the architect holds, the
/// box that answers it, then the owner's answer (`reply`).
function Message({ e, to, reply }: { e: ConversationEntry; to: string | null; reply: ConversationEntry | null }) {
  const own = e.sender === 'owner';
  const from = e.source === 'between' ? `architect:${e.sender}` : member(e.sender);
  const round = e.work !== null && e.round !== null && e.source !== 'between';
  const ask = e.source === 'ask' ? e.ask_detail : null;
  const question = e.answerable && e.work !== null && e.matter !== null ? { work: e.work, matter: e.matter } : null;
  const answers = (ask !== null && ask.answer === null) || question !== null;
  const rowClass = own ? 'talk-row talk-row-owner' : answers ? 'talk-row talk-row-answers' : 'talk-row';
  return (
    <div className={rowClass} data-ask={ask !== null ? ask.id : undefined}>
      {own ? null : <Avatar who={from} />}
      <div className={own ? 'talk-bubble talk-bubble-owner' : `talk-bubble talk-bubble-${from.startsWith('architect') ? 'architect' : 'engineer'}`}>
        <div className="talk-bubble-head">
          {own ? null : <span className="talk-who">{party(e, e.sender)}</span>}
          {to !== null ? <span className="talk-mention">{to}</span> : null}
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
        {ask !== null ? ask.answer === null ? <AskAnswer a={ask} /> : <AskAnswered a={ask} /> : null}
        {question !== null ? <QuestionAnswer work={question.work} matter={question.matter} /> : null}
        {reply !== null ? <AnswerIn head={t('conv_your_reply')} at={reply.at} text={reply.text} /> : null}
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

/// A fault's Forward to…: a menu of every project's architect. Picking one forwards the fault there, as a matter of its
/// queue, and marks the fault read; one it went to already is marked and cannot be picked again.
function ForwardMenu({ notice, choices, failed }: { notice: number; choices: ForwardChoice[]; failed: (x: unknown) => void }) {
  const [busy, setBusy] = useState(false);
  const forward = (project: string) => {
    setBusy(true);
    postJson(`/api/v1/faults/${notice}/forward`, { project }).then(
      () => {
        setBusy(false);
        reread('messages', 'architect');
      },
      (x: unknown) => {
        setBusy(false);
        failed(x);
      },
    );
  };
  return (
    <ActionMenu>
      <ActionMenu.Button size="small" disabled={busy}>
        {t('fault_forward')}
      </ActionMenu.Button>
      <ActionMenu.Overlay width="auto">
        <ActionList aria-label={t('fault_forward')}>
          {choices.map((c) => (
            <ActionList.Item key={c.project} disabled={c.sent} onSelect={() => forward(c.project)}>
              {c.label}
              {c.sent ? <ActionList.TrailingVisual>{t('fault_forward_sent')}</ActionList.TrailingVisual> : null}
            </ActionList.Item>
          ))}
        </ActionList>
      </ActionMenu.Overlay>
    </ActionMenu>
  );
}

/// One place a fault went, and how it stands there: in the queue, or handled in a round, which leads to that round on
/// the architect's chart.
function ForwardLine({ f }: { f: Forwarded }) {
  const p = forwardParts(f);
  return (
    <p className="talk-line-more">
      {`${p.lead} · ${p.state}`}
      {p.round !== null ? (
        <>
          {' '}
          <Link to={architectHref({ project: f.project, work: f.handled_work, round: f.handled_round })} muted>
            {p.round}
          </Link>
        </>
      ) : null}
    </p>
  );
}

/// A fault of wf-agents: what the engine said, how many misses it holds, where it was forwarded and how it stands
/// there; unread, Mark read; and Forward to…, to a project's architect.
function Fault({ e, forwarding }: { e: ConversationEntry; forwarding: Forwarding | null }) {
  const [error, setError] = useState<string | null>(null);
  const failed = (x: unknown) => setError(x instanceof Error ? x.message : String(x));
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
              markFault(e.id).catch(failed);
            }}
          >
            {t('message_mark_read')}
          </Button>
        ) : null}
        {forwarding !== null && forwarding.projects.length > 0 ? (
          <ForwardMenu notice={e.id} choices={forwardMenu(forwarding.projects, e.forwards)} failed={failed} />
        ) : null}
      </div>
      <Markdown text={e.text} />
      {e.misses !== null ? <MissesLine m={e.misses} /> : null}
      {e.forwards.map((f) => (
        <ForwardLine key={f.matter} f={f} />
      ))}
      {error !== null ? <p className="bad">{t('request_failed', { error })}</p> : null}
    </Flash>
  );
}

/// One entry of the stream, in its voice; a question with the owner's answer to it (`replies`, by its matter).
function Entry({ e, c, w, held, current, replies, forwarding }: {
  e: ConversationEntry;
  c: Conversation;
  w: WorkSummary | null;
  held: ReadonlySet<number>;
  current: number | null;
  replies: ReadonlyMap<number, ConversationEntry>;
  forwarding: Forwarding | null;
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
  if (voice === 'fault') return <Fault e={e} forwarding={forwarding} />;
  if (voice === 'none') return null;
  const reply = e.matter !== null && isQuestion(e) ? (replies.get(e.matter) ?? null) : null;
  return <Message e={e} to={mentions(e, c.kind) ? mention(e, c.kind) : null} reply={reply} />;
}

/// What waits for the owner pinned at the top of the conversation: the design card whose ruling a work waits for
/// before its merge, with the verdicts to send. An ask is not pinned: it waits where it was asked.
function Pinned({ d }: { d: ConversationDetail }) {
  if (d.design_cards.length === 0) return null;
  return (
    <section className="talk-pins" aria-label={t('conv_waits_for_you')}>
      <h3 className="talk-pins-title">{t('conv_waits_for_you')}</h3>
      {d.design_cards.map((c) => (
        <DesignCardView key={c.id} c={c} round={<Link to={workHref(c.work, { round: c.round })}>{`R${c.work}-${c.round}`}</Link>} />
      ))}
    </section>
  );
}

/// The one box at the bottom of a conversation: it says something, in a work's group to the engineer, the architect or
/// both; in a group of projects to one architect or every one; in the one-to-one to the architect, as the owner's words
/// in its queue, which it takes up in its turn. It never answers a bubble: an ask, and a question the architect holds,
/// are answered in their bubbles. Closed, it says why: a work at its end no longer changes its way on.
function Composer({ c, w }: { c: Conversation; w: WorkSummary | null }) {
  const choices = sayTo(c);
  const [text, setText] = useState('');
  const [ticked, setTicked] = useState<string[]>(choices.slice(0, 1));
  const sendTo = sayToTicked(c, ticked);
  const to = sendTo.length === 1 ? sendTo[0] : '';
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const closed = closedWhy(c, w);
  if (choices.length === 0) return null;
  const ready = closed === null && text.trim() !== '' && sendTo.length > 0;
  const done = () => {
    setBusy(false);
    setText('');
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
    if (c.kind === 'work') {
      sent = postJson(`/api/v1/works/${c.work ?? 0}/say`, { text: words, to });
    } else if (c.kind === 'projects') {
      sent = Promise.all(
        sendTo.map((x) => postJson('/api/v1/projects/say', x === 'all' ? { key: c.key, text: words } : { key: c.key, text: words, to: x })),
      ).then(() => undefined);
    } else {
      sent = postJson(forProject('/api/v1/architect/say', c.project), { text: words });
    }
    sent.then(done, failed);
  };
  const toWords = (x: string) => (c.kind === 'projects' ? t('conv_to_architect_of', { project: x }) : `@${words(x)}`);
  const tick = (x: string, on: boolean) => setTicked((now) => (on ? [...now, x] : now.filter((y) => y !== x)));
  return (
    <div className="talk-say">
      {closed === null && choices.length > 1 ? (
        <div className="talk-say-to" role="group" aria-label={t('conv_to')}>
          <span className="talk-say-to-label">{t('conv_to')}</span>
          {choices.map((x) => (
            <FormControl key={x} layout="horizontal">
              <Checkbox checked={ticked.includes(x)} onChange={(e) => tick(x, e.target.checked)} />
              <FormControl.Label>{toWords(x)}</FormControl.Label>
            </FormControl>
          ))}
        </div>
      ) : null}
      <div className="talk-say-row">
        <Textarea
          block
          resize="vertical"
          rows={2}
          value={text}
          disabled={busy || closed !== null}
          placeholder={
            closed !== null
              ? closed
              : c.kind === 'work'
                ? t(to === 'engineer' ? 'conv_say_engineer' : to === 'architect' ? 'conv_say_architect_work' : sendTo.length === 0 ? 'conv_say_nobody' : 'conv_say_both')
                : t('conv_say_architect')
          }
          title={t('send_keys_hint')}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (enterSends(e)) {
              e.preventDefault();
              send();
            }
          }}
        />
        <Button variant="primary" onClick={send} disabled={!ready || busy}>
          {t('conv_send')}
        </Button>
      </div>
      {error !== null ? <Flash variant="danger">{t('request_failed', { error })}</Flash> : null}
    </div>
  );
}

/// The slim line over the stream while an ask that waits for the owner is out of view, so that none is missed however
/// much is said after it: each such ask, with which way it is, and a click brings it into view.
function AwayLine({ away, list }: { away: Array<[number, 'above' | 'below']>; list: RefObject<HTMLDivElement | null> }) {
  if (away.length === 0) return null;
  return (
    <div className="talk-away">
      {away.map(([id, where]) => (
        <Button
          key={id}
          size="small"
          variant="invisible"
          leadingVisual={where === 'above' ? ArrowUpIcon : ArrowDownIcon}
          onClick={() => list.current?.querySelector(`[data-ask="${id}"]`)?.scrollIntoView({ block: 'start', behavior: 'smooth' })}
        >
          {t('conv_ask_waits', { id })}
        </Button>
      ))}
    </div>
  );
}

/// The stream of a conversation, oldest first, scrolled to its newest: the engine's cards and lines in the page's own
/// voice, the members' messages beside them, an ask where it was asked; under it, while the owner's words added to the
/// queue wait to be taken up in the one-to-one, that they do; over it, while an ask that waits is out of view, the line
/// that says so.
export function Stream({ d, c, w }: { d: ConversationDetail; c: Conversation; w: WorkSummary | null }) {
  const held = useMemo(() => heldCards(d), [d]);
  const replies = useMemo(() => ownerReplies(d.entries), [d]);
  const stream = streamOf(d);
  const current = d.entries.find((e) => e.matter_card?.state === 'working')?.matter_card?.matter ?? null;
  const list = useRef<HTMLDivElement>(null);
  const shown = stream.length;
  useEffect(() => {
    const el = list.current;
    if (el !== null) el.scrollTop = el.scrollHeight;
  }, [c.key, shown]);
  // Which of the asks that wait for the owner are out of view, and which way: watched as the stream scrolls and grows.
  const waiting = d.asks.map((a) => a.id).join(',');
  const [away, setAway] = useState<Array<[number, 'above' | 'below']>>([]);
  useEffect(() => {
    const root = list.current;
    const ids = waiting === '' ? [] : waiting.split(',').map(Number);
    if (root === null || ids.length === 0 || typeof IntersectionObserver === 'undefined') {
      setAway([]);
      return;
    }
    const where = new Map<number, 'above' | 'below' | null>();
    const seen = new IntersectionObserver(
      (changes) => {
        for (const x of changes) {
          const view = x.rootBounds ?? root.getBoundingClientRect();
          where.set(Number((x.target as HTMLElement).dataset.ask), awayOf(x.boundingClientRect, view));
        }
        setAway(ids.flatMap((id): Array<[number, 'above' | 'below']> => {
          const w = where.get(id);
          return w === undefined || w === null ? [] : [[id, w]];
        }));
      },
      { root },
    );
    for (const el of root.querySelectorAll<HTMLElement>('[data-ask]')) {
      if (ids.includes(Number(el.dataset.ask))) seen.observe(el);
    }
    return () => seen.disconnect();
  }, [waiting, shown, c.key]);
  // Cards run in a Timeline of their own between the messages: each run of cards and lines is one Timeline.
  const runs: Array<{ cards: boolean; entries: ConversationEntry[] }> = [];
  for (const e of stream) {
    const v = voiceOf(e, held);
    const cards = v === 'card' || v === 'system';
    const last = runs[runs.length - 1];
    if (last !== undefined && last.cards === cards) last.entries.push(e);
    else runs.push({ cards, entries: [e] });
  }
  // Words added to the queue wait their turn; those interjected say on themselves that they are not taken up yet.
  const pending = c.kind === 'architect' && pendingWords(d.entries).some((e) => e.source === 'words');
  return (
    <>
      <AwayLine away={away} list={list} />
      <div className="talk-stream" ref={list}>
        {stream.length === 0 ? <p className="talk-none">{t('conv_none')}</p> : null}
        {runs.map((run) => {
          const key = `${run.entries[0].source}-${run.entries[0].id}-${run.entries[0].at}`;
          const items: ReactNode = run.entries.map((e) => (
            <Entry
              key={`${e.source}-${e.id}-${e.at}`}
              e={e}
              c={c}
              w={w}
              held={held}
              current={current}
              replies={replies}
              forwarding={d.forwarding}
            />
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
    </>
  );
}

/// One conversation: its head with the owner's controls, the design card a work waits on pinned under it, the stream,
/// and the one box at the bottom; reading it marks it read up to its latest entry.
function Open({ c }: { c: Conversation }) {
  const r = useResource<ConversationDetail>('messages', `/api/v1/conversation?key=${encodeURIComponent(c.key)}`);
  const works = useResource<WorkSummary[]>('works', '/api/v1/works');
  const d = r.data !== undefined && r.data.key === c.key ? r.data : null;
  const w = c.work !== null ? ((works.data ?? []).find((x) => x.id === c.work) ?? null) : null;
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
    c.kind === 'work' ? (
      <WorkHead c={c} w={w} />
    ) : c.kind === 'architect' ? (
      <ArchitectHead c={c} />
    ) : c.kind === 'wf_agents' ? (
      <WfAgentsHead title={conversationTitle(c)} forwarding={d?.forwarding ?? null} />
    ) : (
      <PlainHead title={conversationTitle(c)} />
    );
  return (
    <AnswersHere.Provider value={true}>
      <div className="talk-open">
        {head}
        {d === null ? (
          <p className="talk-none">{r.error ?? t('loading')}</p>
        ) : (
          <>
            <Pinned d={d} />
            <Stream d={d} c={c} w={w} />
          </>
        )}
        <Composer key={c.key} c={c} w={w} />
      </div>
    </AnswersHere.Provider>
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
