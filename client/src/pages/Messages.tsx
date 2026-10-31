// The owner's conversations, the one place on the page the owner acts: their list on the left, the one open on the
// right. The list's order never changes for what waits: wf-agents' own first; then each project's, by name, headed by
// the buttons that show or hide its works in each state, the done and the ended hidden at first: the one-to-one with
// its architect, then a group for each work, newest first, which greys out in place once the work ends; then the
// groups of projects the configuration connects. Each architect's and each work's is marked by how that architect or
// that work stands, a round running in motion; what waits for the owner and what is unread are counts on each, red and
// grey.
// A work's group holds the owner, its engineer and its project's architect: everything said in it reaches all three,
// each agent's message beside its avatar in its colour and headed by whom it is for, the owner's on the right; the
// engine's notices are grey lines, and its stage cards, one for each stretch of the work at a stage, updated in place,
// the design's with the design folded and the design card to rule on. The one-to-one holds the owner's words, what the
// architect tells and asks, a card for each of its matters and the policy as it took effect, under its queue; the
// questions the architect pinned for the owner's reply are at its bottom, over the box, the newest open and the others
// folded, and the box replies to the one open while the tick over it is on. wf-agents' own holds its faults, each of
// which the owner forwards to any project's architect, once to each, and says where it went and how it stands there;
// its head sets where each new fault goes as it comes, nobody at first. An ask of the owner from a work shows once,
// where it was asked, as the asking agent's bubble, in full while it waits, and is answered in that bubble: the buttons
// of its options, the one picked marked, then a small box for the owner's words, free unless the ask is in words alone
// or no option is picked, and Answer; a question the architect holds has the small box in its bubble too. What is written there answers that bubble; once answered, the buttons and the box give way to the
// owner's answer, in the same bubble and named as the owner's answer to it, which shows nowhere else. An architect's
// question has no box: replied to, it is back where it was asked, with the reply in its bubble.
// While an ask that waits is out of view, a slim line over the stream says so and brings it into view. A design card a
// work waits on to be ruled is pinned at the top. The one box at the bottom says something, in a work's group to the
// engineer, the architect or both, and in the one-to-one to the architect, whose words it takes first and answers in
// one round, or replies to the question ticked. Opening a conversation reads it up to its latest entry.

import { ArrowDownIcon, ArrowUpIcon, BellIcon, CheckIcon, ChevronRightIcon, CpuIcon, StackIcon } from '@primer/octicons-react';
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
  Timeline,
} from '@primer/react';
import {
  type Dispatch,
  type MouseEvent,
  type ReactNode,
  type RefObject,
  type SetStateAction,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';

import type {
  Architect,
  Conversation,
  ConversationDetail,
  ConversationEntry,
  Forwarded,
  Forwarding,
  Misses,
  OwnerAsk,
  PolicyDraft,
  Status,
  WordsState,
  WorkState,
  WorkSummary,
} from '../api';
import { AnswersHere, answerWay, askAnswer, enterSends, handsBack, needsWords, pickAfter } from '../answer';
import { architectMark, architectState } from '../architect';
import {
  course,
  heldWords,
  liveState,
  StateFilter,
  StateMark,
  useShownStates,
  WORK_STATES,
  WORK_TONE,
  workStateMark,
} from '../components';
import {
  architectAsk,
  asksApproval,
  awayOf,
  closedWhy,
  conversationTitle,
  DRAFT_APPROVED,
  type ForwardChoice,
  forwardMenu,
  forwardParts,
  isQuestion,
  latestAt,
  listParts,
  member,
  mention,
  mentions,
  openQuestion,
  ownerReplies,
  party,
  pendingWords,
  pinnedQuestions,
  replyTicked,
  rowsShown,
  sayTo,
  sayToTicked,
  shownByState,
  streamOf,
  type Ticked,
  toArchitect,
  voiceOf,
  wordsStateText,
} from '../conversation';
import { cost, time } from '../format';
import { postJson } from '../http';
import { imagesSent, type PendingImage, saysSomething } from '../images';
import { isObj, label, str, words } from '../labelled';
import { reread, useResource, useStatus } from '../live';
import { Markdown } from '../markdown';
import { SayBox, Thumbs } from '../parts';
import { architectHref, forProject, href, Link, navigate, type SideTab, sideHref, workHref } from '../route';
import { t, tn } from '../text';
import { joinedWords, SEP } from '../voices';
import { ArchitectAskBlock } from './ArchitectAsk';
import { ArchitectSide, useSideOpen } from './ArchitectSide';
import { Bubble } from './Bubble';
import { AskView, DesignCardView } from './Decide';
import { ArchitectHead, PlainHead, WfAgentsHead, WorkHead } from './TalkHead';
import { MatterBubble, PlanBubble, PolicyBubble, StageBubble } from './TalkCards';

/// Marks a fault of wf-agents read, where it shows.
async function markFault(id: number): Promise<void> {
  await postJson('/api/v1/messages/read', { ids: [], notices: [id] });
  reread('messages');
}

/// How the work of a work's group stands now, for its mark; nothing while the works are not read yet.
export function workMark(work: number | null, works: WorkSummary[] | undefined, status: Status | null): WorkState | null {
  const w = works?.find((x) => x.id === work);
  return w !== undefined ? liveState(w, status) : null;
}

/// The mark of an architect's one-to-one: how the architect stands, from what its head reads.
function ArchitectMark({ project, status }: { project: string; status: Status | null }) {
  const r = useResource<Architect>('architect', forProject('/api/v1/architect', project));
  const m = r.data !== undefined && r.data.project === project ? architectMark(r.data, status) : null;
  return <StateMark state={m?.state ?? null} words={m?.words} />;
}

/// The mark beside a conversation in the list: for an architect's one-to-one and a work's group, how that architect or
/// that work stands, a round running in motion, since the list's headings and the row's name say what kind of row it
/// is; for wf-agents' own its processor, for a group of connected projects their stack.
function RowMark({ c, works, status }: { c: Conversation; works: WorkSummary[] | undefined; status: Status | null }) {
  if (c.kind === 'architect') return <ArchitectMark project={c.project ?? ''} status={status} />;
  if (c.kind === 'work') return <StateMark state={workMark(c.work, works, status)} />;
  return c.kind === 'projects' ? <StackIcon /> : <CpuIcon />;
}

/// The states of a work whose groups the list hides until the owner sets otherwise: those of a work over.
export const HIDDEN: WorkState[] = ['done', 'ended'];

/// The states of a work whose groups the list shows until the owner sets otherwise.
export const SHOWN: WorkState[] = WORK_STATES.filter((s) => !HIDDEN.includes(s));

/// The states whose works' groups the list shows, as the owner set them with its buttons and kept in this browser, and
/// the way to show or hide one: one choice for the list and the bar, which counts what the list shows. The browser
/// keeps the states hidden, so that a state the page comes to have shows until the owner hides it.
export function useTalkStates(): [WorkState[], (s: WorkState) => void] {
  const [hidden, toggle] = useShownStates('talks-hidden-states', WORK_STATES, HIDDEN);
  return [WORK_STATES.filter((s) => !hidden.includes(s)), toggle];
}

/// The conversations whose counts of what waits for the owner and what is unread the bar sums: those the list's
/// buttons of a work's states show (`shown`), whichever is open, so that the bar counts nothing the list hides.
export function counted(
  list: Conversation[],
  works: WorkSummary[] | undefined,
  status: Status | null,
  shown: readonly WorkState[],
): Conversation[] {
  return list.filter((c) => shownByState(c, (x) => workMark(x.work, works, status), shown));
}

/// The list of conversations in its fixed order, each with its mark, what waits for the owner in it (red) and what is
/// unread (grey); a work's group over greyed in place. Each project's group is headed by its name and the buttons that
/// show or hide its works in each state, as the items' tree's are (`StateFilter`), the owner's choice one for every
/// project and kept in this browser; the conversation open stays whatever its state, without its counts while its
/// state is hidden, since what the buttons hide counts nowhere, as the bar does not count it.
export function List({ list, open, works }: { list: Conversation[]; open: string | null; works: WorkSummary[] | undefined }) {
  const status = useStatus();
  const [shown, toggle] = useTalkStates();
  const stateOf = (c: Conversation) => workMark(c.work, works, status);
  return (
    <NavList aria-label={t('nav_messages')} className="talks-list">
      {listParts(list).map((p) => {
        // The states of the project's works, once read.
        const states = p.conversations.flatMap((c) => {
          const s = c.kind === 'work' ? stateOf(c) : null;
          return s !== null ? [s] : [];
        });
        return (
          <NavList.Group
            key={p.project !== null ? `project:${p.project}` : p.part}
            title={p.part === 'project' ? (p.project ?? undefined) : p.part === 'projects' ? t('talks_projects') : undefined}
          >
            {states.length > 0 ? (
              <li className="talks-states">
                <StateFilter
                  order={WORK_STATES}
                  states={states}
                  shown={shown}
                  tone={WORK_TONE}
                  mark={workStateMark}
                  title={(s, n, on) => tn(on ? 'talks_hide' : 'talks_show', n, { state: t(`state_${s}`) })}
                  onShown={toggle}
                />
              </li>
            ) : null}
            {(p.part === 'project' ? rowsShown(p.conversations, stateOf, shown, open) : p.conversations).map((c) => (
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
                  <RowMark c={c} works={works} status={status} />
                </NavList.LeadingVisual>
                <span className="talks-title" title={conversationTitle(c)}>
                  {conversationTitle(c)}
                </span>
                {(c.waiting > 0 || c.unread > 0) && shownByState(c, stateOf, shown) ? (
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
        );
      })}
    </NavList>
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

/// What a message of the owner's or an agent's says: its words as written, in the page's markdown, and the images the
/// owner sent with them as thumbnails under them; a ruling, which card; an ask, in full while it waits, so that it is
/// read where it is answered, and once answered its words with the ask in full folded under them.
function Words({ e }: { e: ConversationEntry }) {
  if (e.source === 'ruling') return <p>{t('conv_ruling', { id: e.design_card ?? e.id })}</p>;
  if (e.ask_detail !== null && e.ask_detail.answer === null) return <AskIn a={e.ask_detail} />;
  return (
    <>
      {e.text !== '' ? <Markdown text={e.text} /> : null}
      <Thumbs ids={e.images} />
      {e.ask_detail !== null ? (
        <details className="talk-fold">
          <summary>{t('conv_ask_full', { id: e.ask_detail.id })}</summary>
          <div className="talk-fold-body">
            <AskIn a={e.ask_detail} />
          </div>
        </details>
      ) : null}
    </>
  );
}

/// An ask, in the look its kind has: an architect's (a question, a choice) or an engineer's.
function AskIn({ a }: { a: OwnerAsk }) {
  const architect = isObj(a.body) && a.body.from === 'architect';
  return architect ? (
    <ArchitectAskBlock a={a} />
  ) : (
    <AskView a={a} round={<Link to={workHref(a.work, { round: a.round })}>{`R${a.work}-${a.round}`}</Link>} />
  );
}

/// The small box in a bubble where the owner's words, and the images sent with them, answer that bubble, beside
/// Answer: Enter sends and Shift with Enter starts a new line; what failed is said under it.
function AnswerBox({ placeholder, ready, busy, text, setText, images, setImages, send, error, children = null }: {
  placeholder: string;
  ready: boolean;
  busy: boolean;
  text: string;
  setText: (text: string) => void;
  images: PendingImage[];
  setImages: Dispatch<SetStateAction<PendingImage[]>>;
  send: () => void;
  error: string | null;
  children?: ReactNode;
}) {
  return (
    <>
      <div className="talk-answer-box">
        <SayBox
          small
          text={text}
          set={setText}
          images={images}
          setImages={setImages}
          disabled={busy}
          placeholder={placeholder}
          label={placeholder}
          title={t('send_keys_hint')}
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

/// What answering in a bubble needs: the words and the images that go with them, whether they are being sent, what
/// failed, and how to send them, the images with the words.
function useAnswer(): {
  text: string;
  setText: (text: string) => void;
  images: PendingImage[];
  setImages: Dispatch<SetStateAction<PendingImage[]>>;
  busy: boolean;
  error: string | null;
  post: (url: string, body: object) => void;
} {
  const [text, setText] = useState('');
  const [images, setImages] = useState<PendingImage[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const post = (url: string, body: object) => {
    setBusy(true);
    setError(null);
    postJson(url, { ...body, images: imagesSent(images) }).then(
      () => {
        setBusy(false);
        setText('');
        setImages([]);
        reread('messages', 'architect', 'works', 'work:*');
      },
      (e: unknown) => {
        setBusy(false);
        setError(e instanceof Error ? e.message : String(e));
      },
    );
  };
  return { text, setText, images, setImages, busy, error, post };
}

/// How an ask that waits is answered, in its bubble: the buttons of what it offers (the options of its choice), a click
/// picking one and a second clearing it, the one picked marked; then the small box of the owner's words and Answer. The
/// words are needed for an ask in words alone and for an option ask with none picked, else they are free; an
/// engineer's ask the owner holds can be handed back to the architect with them.
function AskAnswer({ a }: { a: OwnerAsk }) {
  const way = answerWay(a);
  const [pick, setPick] = useState('');
  const [back, setBack] = useState(false);
  const { text, setText, images, setImages, busy, error, post } = useAnswer();
  const answer = askAnswer(pick, text, handsBack(a) && back, images.length > 0);
  const button = (value: string) => (
    <Button
      key={value}
      size="small"
      leadingVisual={pick === value ? CheckIcon : undefined}
      aria-pressed={pick === value}
      onClick={() => setPick(pickAfter(pick, value))}
    >
      {value}
    </Button>
  );
  return (
    <div className="talk-answer" role="group" aria-label={t('conv_answer_ask', { id: a.id })}>
      {way.by === 'option' ? <div className="talk-answer-buttons">{way.options.map((o) => button(o))}</div> : null}
      <AnswerBox
        placeholder={t(needsWords(pick) ? 'conv_answer_words' : 'conv_answer_more')}
        ready={answer !== null}
        busy={busy}
        text={text}
        setText={setText}
        images={images}
        setImages={setImages}
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
  const { text, setText, images, setImages, busy, error, post } = useAnswer();
  const ready = saysSomething(text, images);
  return (
    <div className="talk-answer">
      <AnswerBox
        placeholder={t('conv_answer_words')}
        ready={ready}
        busy={busy}
        text={text}
        setText={setText}
        images={images}
        setImages={setImages}
        send={() => {
          if (ready) post(`/api/v1/works/${work}/say`, { text: text.trim(), question: matter });
        }}
        error={error}
      />
    </div>
  );
}

/// The owner's answer inside the bubble it answers, named as the owner's answer to it so that it is not read as other
/// words of the owner's: what it answers and when, then the choice made, the words and the images sent with them, in
/// the look of what is sent to an agent.
function AnswerIn({ head, at, choice = '', text, images }: {
  head: string;
  at: number | null;
  choice?: string;
  text: string;
  images: number[];
}) {
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
          <Thumbs ids={images} />
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
      images={a.answer_images}
    />
  );
}

/// One message of the owner's or an agent's: who it is from, to whom (in a group, at its head), when, its round, and
/// what it says; the owner's on the right in the look of what is sent to an agent, an agent's beside its avatar in the
/// look of what an agent handed back, in its colour; what the architect's policy setting says, as the architect's. The
/// owner's words not taken up yet, and those never taken in, say so on themselves; once the architect took them up,
/// where they stand with it is a line under them. Each answer to a bubble is in that bubble: an ask of a work's holds
/// what answers it while it waits, then the owner's answer; an architect's question nothing while it waits, since the
/// box at the bottom replies to it, then the owner's reply; a question the architect holds, the box that answers it,
/// then the owner's answer (`reply`). The question whether the draft of a new policy is approved leads to the draft, in
/// the panel beside the one-to-one (`draft`).
function Message({ e, to, reply, draft = null }: {
  e: ConversationEntry;
  to: string | null;
  reply: ConversationEntry | null;
  draft?: string | null;
}) {
  const own = e.sender === 'owner';
  // The architect of another project, in a group of projects, speaks as an architect, in the architect's colour.
  const from = e.source === 'between' ? 'architect' : member(e.sender);
  const round = e.work !== null && e.round !== null && e.source !== 'between';
  const ask = e.source === 'ask' ? e.ask_detail : null;
  const question = e.answerable && e.work !== null && e.matter !== null ? { work: e.work, matter: e.matter } : null;
  const asks = ask !== null && ask.answer === null && !architectAsk(ask);
  const answers = asks || question !== null;
  return (
    <Bubble
      who={own ? 'owner' : from}
      wide={answers}
      ask={ask !== null ? ask.id : undefined}
      head={
        <>
          {own ? null : <span className="talk-who">{party(e, e.sender)}</span>}
          {to !== null ? <span className="talk-mention">{to}</span> : null}
          <span className="talk-when">{time(e.at)}</span>
          {round ? (
            <Link to={workHref(e.work ?? 0, { round: e.round ?? 0 })}>
              {`R${e.work}-${e.round}`}
            </Link>
          ) : null}
          {draft !== null ? <Link to={draft}>{t('conv_draft')}</Link> : null}
          {e.pending && own ? <span className="talk-state">{t('conv_pending')}</span> : null}
          {e.undelivered ? <span className="talk-state talk-state-bad">{t('conv_undelivered')}</span> : null}
        </>
      }
      after={
        <>
          {asks && ask !== null ? <AskAnswer a={ask} /> : null}
          {ask !== null && ask.answer !== null ? <AskAnswered a={ask} /> : null}
          {question !== null ? <QuestionAnswer work={question.work} matter={question.matter} /> : null}
          {reply !== null ? (
            <AnswerIn head={t('conv_your_reply')} at={reply.at} text={reply.text} images={reply.images} />
          ) : null}
        </>
      }
      under={own && e.words_state !== null ? <WordsLine w={e.words_state} /> : undefined}
    >
      <Words e={e} />
    </Bubble>
  );
}

/// A line of the page's own, small and grey, for what the engine says, and what a helper of the work told the owner:
/// an item of the run's Timeline, as a card is, its CpuIcon its badge, so that every badge of the run is in one column.
function SystemLine({ e }: { e: ConversationEntry }) {
  const helper = e.sender !== 'engine';
  return (
    <Timeline.Item condensed className="talk-card">
      <Timeline.Badge>
        <CpuIcon />
      </Timeline.Badge>
      <Timeline.Body className="talk-card-body">
        <div className="talk-line">
          <span className="talk-line-text">
            {helper ? `${t('conv_helper_said', { line: words(e.sender) })} ` : null}
            {e.source === 'card' ? t('conv_card', { id: e.design_card ?? e.id }) : e.text}
          </span>
          <span className="talk-when">{time(e.at)}</span>
        </div>
      </Timeline.Body>
    </Timeline.Item>
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

/// Where the owner's words stand with the architect they were said to, once it took them up, in place of a matter card
/// that would repeat them: one line of the page's own under them, leading to the cell on the architect's chart where it
/// handles them while it does.
function WordsLine({ w }: { w: WordsState }) {
  const chart =
    w.architect_work !== null && w.cell !== null
      ? architectHref({ project: w.project, work: w.architect_work, round: w.round, cell: w.cell })
      : null;
  return (
    <p className="talk-line-more">
      {wordsStateText(w)}
      {chart !== null ? (
        <>
          {SEP}
          <Link to={chart}>
            {t('matter_on_chart')}
          </Link>
        </>
      ) : null}
    </p>
  );
}

/// One place a fault went, and how it stands there: in the queue, or handled in a round, which leads to that round on
/// the architect's chart.
function ForwardLine({ f }: { f: Forwarded }) {
  const p = forwardParts(f);
  return (
    <p className="talk-line-more">
      {joinedWords([p.lead, p.state])}
      {p.round !== null ? (
        <>
          {' '}
          <Link to={architectHref({ project: f.project, work: f.handled_work, round: f.handled_round })}>
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

/// One entry of the stream, in its voice; a question with the owner's answer to it (`replies`, by its matter); the
/// question whether the draft of a new policy that waits (`draft`) is approved leading to the draft; the bubble of the
/// matter the architect has in hand with how its work goes on (`architect`).
function Entry({ e, c, w, architect, replies, forwarding, draft }: {
  e: ConversationEntry;
  c: Conversation;
  w: WorkSummary | null;
  architect: WorkSummary | null;
  replies: ReadonlyMap<number, ConversationEntry>;
  forwarding: Forwarding | null;
  draft: PolicyDraft | null;
}) {
  const voice = voiceOf(e);
  if (voice === 'live') {
    const project = c.project ?? '';
    if (e.stage !== null) return <StageBubble e={e} s={e.stage} w={w} project={c.project} />;
    if (e.matter_card !== null) return <MatterBubble e={e} m={e.matter_card} project={project} architect={architect} />;
    if (e.policy !== null) return <PolicyBubble p={e.policy} project={project} />;
    if (e.plan !== null) return <PlanBubble e={e} p={e.plan} project={project} />;
    return null;
  }
  if (voice === 'system') return <SystemLine e={e} />;
  if (voice === 'fault') return <Fault e={e} forwarding={forwarding} />;
  if (voice === 'none') return null;
  const reply = e.matter !== null && isQuestion(e) ? (replies.get(e.matter) ?? null) : null;
  return (
    <Message
      e={e}
      to={mentions(e, c.kind) ? mention(e, c.kind) : null}
      reply={reply}
      draft={c.project !== null && asksApproval(e, draft) ? sideHref(c.project, 'policy') : null}
    />
  );
}

/// What is pinned for the owner, in one look wherever it is: a band in the tint of what waits for the owner, headed by
/// a bell and what it is in that colour, with no line between it and the stream; at the top of a work's group, the
/// design card its work waits on, and at the bottom of the one-to-one with an architect, over the box, the questions
/// the architect pinned.
function Pins({ at, children }: { at: 'top' | 'bottom'; children: ReactNode }) {
  return (
    <section className={`talk-pins talk-pins-${at}`} aria-label={t('conv_waits_for_you')}>
      <h3 className="talk-pins-title">
        <BellIcon />
        {t('conv_waits_for_you')}
      </h3>
      {children}
    </section>
  );
}

/// What waits for the owner pinned at the top of the conversation: the design card whose ruling a work waits for
/// before its merge, with the verdicts to send, folded to its head line until the owner opens it, as the owner asked,
/// since open it took most of the conversation. An ask of a work's is not pinned: it waits where it was asked.
function Pinned({ d }: { d: ConversationDetail }) {
  if (d.design_cards.length === 0) return null;
  return (
    <Pins at="top">
      {d.design_cards.map((c) => (
        <DesignCardView key={c.id} c={c} round={<Link to={workHref(c.work, { round: c.round })}>{`R${c.work}-${c.round}`}</Link>} folded />
      ))}
    </Pins>
  );
}

/// The architect's own trouble, pinned at the top of its one-to-one while the owner must act on it: the engine halted
/// the work it goes on in, and why; the head's Start lets it go on once that is fixed.
function Trouble({ a }: { a: Architect | null }) {
  const status = useStatus();
  if (a === null) return null;
  const s = architectState(a.works, a.queue);
  if (s.kind !== 'working') return null;
  const c = course(s.work, status);
  if (c.held !== 'engine') return null;
  return (
    <Pins at="top">
      <p className="talk-pinned-trouble">{t('arch_trouble', { why: heldWords(c, s.work.paused) })}</p>
    </Pins>
  );
}

/// The owner's approval of the draft of a new policy, from the question that asks for it: the reply the flow takes as
/// the approval, exactly "approved", to that question; other words go by the box at the bottom, ticked to reply to it.
function Approve({ project, question }: { project: string; question: number }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  return (
    <div className="talk-answer-buttons">
      <Button
        size="small"
        variant="primary"
        title={t('draft_approve_hint')}
        disabled={busy}
        onClick={() => {
          setBusy(true);
          setError(null);
          postJson(forProject('/api/v1/architect/say', project), toArchitect(DRAFT_APPROVED, question)).then(
            () => {
              setBusy(false);
              reread('messages', 'architect');
            },
            (x: unknown) => {
              setBusy(false);
              setError(x instanceof Error ? x.message : String(x));
            },
          );
        }}
      >
        {t('draft_approve')}
      </Button>
      {error !== null ? <span className="bad">{t('request_failed', { error })}</span> : null}
    </div>
  );
}

/// A question pinned at the bottom folded to one line: its fold marker, who put it, when, and its words cut to the
/// line, whole on hovering; a click opens it.
function FoldedQuestion({ e, onOpen }: { e: ConversationEntry; onOpen: () => void }) {
  return (
    <button type="button" className="talk-pinned-fold" title={e.text} aria-expanded={false} onClick={onOpen}>
      <ChevronRightIcon />
      <span className="talk-who">{party(e, e.sender)}</span>
      <span className="talk-when">{time(e.at)}</span>
      <span className="talk-pinned-text">{e.text}</span>
    </button>
  );
}

/// The questions an architect pinned for the owner's reply, at the bottom of the one-to-one over the box, in the order
/// they were put: the one open (the newest, unless the owner opened another) as the architect's message, the others
/// folded to a line each; opening one folds the one that was open and makes it the one the box replies to. They have no
/// buttons: the box replies.
export function Questions({ pinned, open, onOpen, project, draft }: {
  pinned: ConversationEntry[];
  open: number | null;
  onOpen: (question: number) => void;
  project: string;
  draft: PolicyDraft | null;
}) {
  if (pinned.length === 0) return null;
  return (
    <Pins at="bottom">
      {[...pinned].reverse().map((e) =>
        e.id === open ? (
          <div key={e.id}>
            <Message
              e={e}
              to={asksApproval(e, draft) ? mention(e, 'architect') : null}
              reply={null}
              draft={asksApproval(e, draft) ? sideHref(project, 'policy') : null}
            />
            {asksApproval(e, draft) ? <Approve project={project} question={e.id} /> : null}
          </div>
        ) : (
          <FoldedQuestion key={e.id} e={e} onOpen={() => onOpen(e.id)} />
        ),
      )}
    </Pins>
  );
}

/// The question the box at the bottom of the one-to-one replies to: the one open at the bottom, its words, and whether
/// the tick over the box is on, which the owner sets.
export type Reply = { question: number; words: string; on: boolean; set: (on: boolean) => void };

/// The tick over the box: "Reply to:" and the question open, on one line, whole on hovering.
function ReplyTick({ reply }: { reply: Reply }) {
  return (
    <FormControl layout="horizontal" className="talk-reply-tick">
      <Checkbox checked={reply.on} onChange={(e) => reply.set(e.target.checked)} />
      <FormControl.Label>
        <span className="talk-reply-words" title={reply.words}>
          {t('conv_reply_to', { question: reply.words })}
        </span>
      </FormControl.Label>
    </FormControl>
  );
}

/// The one box at the bottom of a conversation: it says something, with the images added to it or images alone, in a
/// work's group to the engineer, the architect or both; in a group of projects to one architect or every one; in the
/// one-to-one to the architect, as the owner's words it takes first, or, with the tick over it on, as the reply to the
/// question open at the bottom (`reply`). It never answers a bubble: an ask of a work's, and a question the architect
/// holds, are answered in their bubbles. Closed, it says why: a work at its end no longer changes its way on.
export function Composer({ c, w, reply = null }: { c: Conversation; w: WorkSummary | null; reply?: Reply | null }) {
  const choices = sayTo(c);
  const [text, setText] = useState('');
  const [images, setImages] = useState<PendingImage[]>([]);
  const [ticked, setTicked] = useState<string[]>(choices.slice(0, 1));
  const sendTo = sayToTicked(c, ticked);
  const to = sendTo.length === 1 ? sendTo[0] : '';
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const closed = closedWhy(c, w);
  if (choices.length === 0) return null;
  const ready = closed === null && saysSomething(text, images) && sendTo.length > 0;
  const done = () => {
    setBusy(false);
    setText('');
    setImages([]);
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
    const sentImages = imagesSent(images);
    let sent: Promise<void>;
    if (c.kind === 'work') {
      sent = postJson(`/api/v1/works/${c.work ?? 0}/say`, { text: words, to, images: sentImages });
    } else if (c.kind === 'projects') {
      sent = Promise.all(
        sendTo.map((x) =>
          postJson(
            '/api/v1/projects/say',
            x === 'all' ? { key: c.key, text: words, images: sentImages } : { key: c.key, text: words, to: x, images: sentImages },
          ),
        ),
      ).then(() => undefined);
    } else {
      sent = postJson(forProject('/api/v1/architect/say', c.project), {
        ...toArchitect(words, reply?.on === true ? reply.question : null),
        images: sentImages,
      });
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
      {closed === null && reply !== null ? <ReplyTick reply={reply} /> : null}
      <div className="talk-say-row">
        <SayBox
          text={text}
          set={setText}
          images={images}
          setImages={setImages}
          disabled={busy || closed !== null}
          placeholder={
            closed !== null
              ? closed
              : c.kind === 'work'
                ? t(to === 'engineer' ? 'conv_say_engineer' : to === 'architect' ? 'conv_say_architect_work' : sendTo.length === 0 ? 'conv_say_nobody' : 'conv_say_both')
                : t(reply?.on === true ? 'conv_say_reply' : 'conv_say_architect')
          }
          title={t('send_keys_hint')}
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

/// The stream of a conversation, oldest first, scrolled to its newest: the bubbles of who said what, the members'
/// messages and the live bubbles of what a work's stages and an architect's matters came to; in a conversation that is
/// no work's group or one-to-one, the engine's lines in the page's own voice; an ask where it was asked; under it, while
/// the owner's words added to the queue wait to be taken up in the one-to-one, that they do; over it, while an ask that
/// waits is out of view, the line that says so. In the one-to-one with an architect (`architect`), the question
/// whether its draft of a new policy that waits (`draft`) is approved leads to the draft, and the bubble of the matter
/// it has in hand says what holds it.
export function Stream({ d, c, w, draft = null, architect = null }: {
  d: ConversationDetail;
  c: Conversation;
  w: WorkSummary | null;
  draft?: PolicyDraft | null;
  architect?: Architect | null;
}) {
  const replies = useMemo(() => ownerReplies(d.entries), [d]);
  const stream = streamOf(d);
  const going = architect !== null ? architectState(architect.works, architect.queue) : null;
  const aw = going !== null && going.kind === 'working' ? going.work : null;
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
    const v = voiceOf(e);
    const cards = v === 'system';
    const last = runs[runs.length - 1];
    if (last !== undefined && last.cards === cards) last.entries.push(e);
    else runs.push({ cards, entries: [e] });
  }
  // Words added to the queue or to a talk wait their turn; those interjected say on themselves that they are not taken
  // up yet.
  const pending = c.kind === 'architect' && pendingWords(d.entries).some((e) => e.source === 'words' || e.source === 'talk');
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
              architect={aw}
              replies={replies}
              forwarding={d.forwarding}
              draft={draft}
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
/// the questions an architect pinned over the box, and the one box at the bottom, with the tick that replies to the
/// question open; the one-to-one with an architect has the panel of its notebook, policy and queue beside it, while it
/// is open, on the tab the address names (`side`), else on the notebook. Reading it marks it read up to its latest
/// entry.
function Open({ c, works, side }: { c: Conversation; works: WorkSummary[] | undefined; side: SideTab | null }) {
  const r = useResource<ConversationDetail>('messages', `/api/v1/conversation?key=${encodeURIComponent(c.key)}`);
  const d = r.data !== undefined && r.data.key === c.key ? r.data : null;
  const w = c.work !== null ? ((works ?? []).find((x) => x.id === c.work) ?? null) : null;
  const project = c.kind === 'architect' ? (c.project ?? '') : null;
  const ar = useResource<Architect>(
    project !== null ? 'architect' : null,
    project !== null ? forProject('/api/v1/architect', project) : null,
  );
  const a = ar.data !== undefined && ar.data.project === project ? ar.data : null;
  const [sideOpen, setSideOpen] = useSideOpen(side !== null);
  // The question the owner opened at the bottom, and how the owner set the tick for it.
  const [opened, setOpened] = useState<number | null>(null);
  const [ticked, setTicked] = useState<Ticked | null>(null);
  const pinned = d !== null ? pinnedQuestions(d) : [];
  const open = openQuestion(pinned, opened);
  const reply: Reply | null =
    open !== null
      ? {
          question: open.id,
          words: open.text,
          on: replyTicked(open, d?.waits_on ?? null, ticked),
          set: (on) => setTicked({ question: open.id, on }),
        }
      : null;
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
      <ArchitectHead c={c} a={a} onSide={sideOpen ? null : () => setSideOpen(true)} />
    ) : c.kind === 'wf_agents' ? (
      <WfAgentsHead title={conversationTitle(c)} forwarding={d?.forwarding ?? null} />
    ) : (
      <PlainHead title={conversationTitle(c)} />
    );
  const talk = (
    <AnswersHere.Provider value={true}>
      <div className="talk-open">
        {head}
        {d === null ? (
          <p className="talk-none">{r.error ?? t('loading')}</p>
        ) : (
          <>
            <Pinned d={d} />
            <Trouble a={a} />
            <Stream d={d} c={c} w={w} draft={a?.draft ?? null} architect={a} />
            <Questions
              pinned={pinned}
              open={open?.id ?? null}
              onOpen={setOpened}
              project={project ?? ''}
              draft={a?.draft ?? null}
            />
          </>
        )}
        <Composer key={c.key} c={c} w={w} reply={reply} />
      </div>
    </AnswersHere.Provider>
  );
  if (a === null || !sideOpen) return talk;
  return (
    <PageLayout containerWidth="full" padding="none" rowGap="none" columnGap="none" className="talk-split">
      <PageLayout.Content padding="none" width="full" className="talk-split-content">
        {talk}
      </PageLayout.Content>
      <PageLayout.Pane
        position="end"
        resizable
        widthStorageKey="wf-agents.architect-side-width"
        width={{ min: '280px', default: '440px', max: '800px' }}
        divider="line"
        padding="none"
        className="talk-side"
        aria-label={t('side_tabs')}
      >
        <div className="talk-side-body">
          <ArchitectSide
            a={a}
            tab={side ?? 'notebook'}
            onClose={() => {
              setSideOpen(false);
              if (side !== null) navigate(href({ page: 'messages', open: c.key }), true);
            }}
          />
        </div>
      </PageLayout.Pane>
    </PageLayout>
  );
}

/// The owner's conversations: the list on the left, in its fixed order, its width dragged and kept in the browser; the
/// conversation open on the right, when the address names none the first the interface lists, the one-to-one with the
/// architect of the configuration's first project. The works are read once for
/// both: the list's marks and the work of the group open. The address may name the tab of the panel beside the one-to-one
/// with an architect (`side`).
export function Messages({ open, side }: { open: string | null; side: SideTab | null }) {
  const r = useResource<Conversation[]>('messages', '/api/v1/conversations');
  const works = useResource<WorkSummary[]>('works', '/api/v1/works').data;
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
        <List list={list} open={shown?.key ?? null} works={works} />
      </PageLayout.Pane>
      <PageLayout.Content padding="none" width="full" className="talks-content">
        {shown !== null ? <Open key={shown.key} c={shown} works={works} side={shown.kind === 'architect' ? side : null} /> : null}
      </PageLayout.Content>
    </PageLayout>
  );
}
