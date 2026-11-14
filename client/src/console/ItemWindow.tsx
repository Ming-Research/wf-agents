// What the window beside the console shows of the item selected: its number, its title, which brings the item into
// view in the tree, and its project; a line of where the item stands and a line of what its work does, with the works
// page's strip of the work's cells and where its time went; what the owner does to the work (let it on, pause it,
// cancel it); two tabs; and under them, while something in the item is new to the owner, a strip that says what
// changed, with a link to mark the item read. The first tab holds the body (where the item came from, the problem it
// solves, the plan, with each choice that waits for the owner where it stands in the plan, and after them each question
// a cell asked the owner that the work waits at, drawn as they are with a line in red that it stops the work, each
// picked by a press on an option and let go by another, and at the plan's end a button as wide as the body to confirm
// them, which counts the picks, waits until each choice has one and sends each question's answer to the engine; once
// confirmed, the picks stand; and last, each ruling of a cell that keeps the work from going on and that the owner may
// overrule, drawn as such a question is, with a button as wide to count it as settled), each passage
// changed since the owner read it marked at its left, and under the body the messages: the owner's and the replies, and a
// box to write one into a block of the work that people write, chosen above it, which a passage selected in the body
// can be quoted into and which names the cells that read it. The second holds what of the work the lines do not say,
// and the legend of the strip's marks. While the window is out, R marks the item read, as its link does, unless a box
// has the keys. What the owner sends is sent once: its button cannot be pressed while it is under way, and a refusal is
// said under the item it was for, and only there.
//
// Brought over from the console of the engine on `old-main` (`client/src/console/ItemWindow.tsx`), its layout as it
// was, its parts the page's own (parts.tsx) where it used Primer's. What the engine does not give yet comes from the
// console's made-up data (`mock.ts`), which says what of it is made up, when the page shows the mocks; otherwise the
// body and the messages are read from the work's blocks, and the rest is not drawn (`real.ts`). What the new console
// added is kept in that layout: the work line's facts, the row of what the owner does to the work under the lines, the
// block a message is written in over its box, and the flow's name among the work's facts.

import {
  Fragment,
  type MouseEvent,
  type ReactNode,
  type RefObject,
  useCallback,
  useEffect,
  useRef,
  useState,
} from 'react';

import { Attr, Button, Confirm, PanelPart, Switch } from '../parts';
import { mocksShown } from '../mock-data';
import {
  type Act,
  type Item,
  loadVersion as loadVersionOf,
  type Summary,
  type Waited,
  type WorkPage,
} from '../works-api';
import { StateLegend, Strip } from '../works/marks';
import { callWords, outageWords } from '../works/words';
import { type ItemStanding, standingOf, whoGives } from './items';
import { Markdown } from './markdown';
import { markRead } from './mock';
import {
  askedChoice,
  firstBody,
  itemOf,
  type LoadVersion,
  readBody,
  readRulings,
  type Ruling,
  workTierOf,
} from './real';
import { age, dollars, type Mood, moodOf } from './snap';
import { barOf, barSpans, TimeTrack } from './track';
import {
  type Body,
  changedIn,
  type Choice,
  named,
  type Message,
  type MessageCell,
  messagesAbout,
  type Part,
  picks,
  quoteOf,
  type Who,
} from './window';

const ago = (at: number) => age(Date.now() - at).label;

/// The words for who gives what a work waits for.
const WHO = { owner: 'you', architect: 'the architect' } as const;

/// Who gives what a work waits for, in words: the person a call names or the engine, you for a run let on at its
/// limit, and for a precondition the person who can make it hold, or else the cells that can, by their titles; none
/// for a precondition only sources outside the work can make hold, such as the items it depends on being merged; the
/// engine for a free slot and for the wait every work shares.
function whom(w: Waited, page: WorkPage | null): string | null {
  switch (w.kind) {
    case 'limit':
    case 'calls':
      return WHO.owner;
    case 'call':
      return w.who === null ? 'the engine' : WHO[w.who];
    case 'precondition': {
      const person = whoGives(w);
      if (person !== null) return WHO[person];
      const cells = w.by.flatMap((m) => (m.kind === 'cell' ? [titleOf(page, m.cell)] : []));
      return cells.length > 0 ? cells.join(' or ') : null;
    }
    case 'slot':
    case 'agents':
      return 'the engine';
    case 'answer':
      return WHO.owner;
  }
}

/// What a work's state is to the panel, as its card colours it.
const MOOD_WORDS: Record<Mood, string> = {
  running: 'Running',
  owner: 'Waiting for you',
  paused: 'Paused',
  halted: 'Halted by the engine',
  waiting: 'Waiting',
};

const STANDING_WORDS: Record<ItemStanding, string> = { open: 'Open', done: 'Done', cancelled: 'Cancelled' };

/// Whether a work has ended: merged or cancelled.
const ended = (s: Summary) => s.now.kind === 'merged' || s.now.kind === 'cancelled';

/// The work's page as the panel has it: read, not read, or still being read.
export type PageState = { page: WorkPage } | { failed: string } | null;

const pageIn = (p: PageState): WorkPage | null => (p !== null && 'page' in p ? p.page : null);

/// A cell's title in the work's flow, or its id where the flow does not name it.
const titleOf = (page: WorkPage | null, cell: string) => page?.flow.cells.find((c) => c.id === cell)?.title ?? cell;

/// Whether what a work waits for is a question a cell asked the owner, which is drawn in the plan as a choice rather
/// than said on the status line.
const asked = (w: Waited): w is Extract<Waited, { kind: 'call' }> & { what: { kind: 'question' } } =>
  w.kind === 'call' && w.what.kind === 'question';

/// The questions a cell asked the owner that the work waits at, as choices at the plan's end.
function askedOf(now: Summary['now']): Choice[] {
  if (now.kind !== 'waits') return [];
  return now.on.flatMap((w) => (asked(w) && w.who === 'owner' ? [askedChoice(w.what.question, w.call, now.at)] : []));
}

/// What the work waits for at a cell, in a few words for the status line.
function lineWords(w: Waited, cell: string): string {
  switch (w.kind) {
    case 'call':
      switch (w.what.kind) {
        case 'question':
          return `at ${cell}, for an answer`;
        case 'tool':
          return `at ${cell}, for ${w.what.name} to run`;
        case 'atLimit':
          return w.what.limit.of === 'returns'
            ? `${cell} reached its limit: ${w.what.limit.sentBack} returns sent back`
            : `${cell} reached its limit: ${w.what.limit.made} calls made`;
        case 'wait':
          return `at ${cell}, for ${callWords(w.what)}`;
      }
      return '';
    case 'precondition':
      return `until ${w.words.charAt(0).toLowerCase()}${w.words.slice(1)}`;
    case 'limit':
      return `${cell} reached its limit: ${w.runs} runs in a row for what a cell after it wrote`;
    case 'calls':
      return `${cell} reached its limit: ${w.runs} runs in a row that only asked for calls`;
    case 'slot':
      return `at ${cell}, for a free slot, which the engine gives in its order`;
    case 'agents':
      return `at ${cell}, for the wait every work shares: ${outageWords(w.outage)}`;
    case 'answer':
      return `at ${cell}, for your answer to ${w.question}`;
  }
}

/// The act that lets a work go on past a limit it waits at, as the works page sends it: the run's call answered, or
/// the cell let go; none where it waits at no limit.
function goOnOf(now: Summary['now']): Act | null {
  if (now.kind !== 'waits') return null;
  for (const w of now.on) {
    if (w.kind === 'call' && w.what.kind === 'atLimit') return { kind: 'result', call: w.call, result: 'go on' };
    if (w.kind === 'limit' || w.kind === 'calls') return { kind: 'letGo', cell: now.at };
  }
  return null;
}

/// What the work does now, as a word and the facts after it.
export function nowWords(s: Summary, page: WorkPage | null): { word: string; facts: string | null } {
  const now = s.now;
  switch (now.kind) {
    case 'round':
      return {
        word: MOOD_WORDS.running,
        facts: `${titleOf(page, now.cell)}, round ${now.round}, for ${ago(now.since)}${s.paused ? ', paused after it' : ''}`,
      };
    case 'next':
      return { word: MOOD_WORDS.running, facts: `next ${titleOf(page, now.cell)}` };
    case 'waits': {
      const cell = titleOf(page, now.at);
      const who = [...new Set(now.on.flatMap((w) => whom(w, page) ?? []))];
      const said = now.on.flatMap((w) => (asked(w) ? [] : [lineWords(w, cell)]));
      const facts = said.length > 0 ? said.join('; ') : `at ${cell}`;
      // A work that waits only for what a source outside it gives, such as the items it depends on being merged,
      // waits for that.
      if (who.length === 0) return { word: MOOD_WORDS.waiting, facts };
      return { word: `Waiting for ${who.join(' and ')}`, facts };
    }
    case 'paused':
      return now.by === 'engine' ? { word: MOOD_WORDS.halted, facts: now.why } : { word: MOOD_WORDS.paused, facts: null };
    case 'everything':
      return { word: MOOD_WORDS.paused, facts: 'with every work' };
    case 'fault':
      return { word: MOOD_WORDS.halted, facts: now.words };
    case 'merged':
      return { word: 'Merged', facts: null };
    case 'cancelled':
      return { word: 'Cancelled', facts: null };
    case 'idle':
      return { word: MOOD_WORDS.waiting, facts: 'nothing new runs a cell' };
  }
}

/// The tone of a dot: an item's standing, or what its work does.
type Tone = ItemStanding | Mood | 'ended';

/// The dot of each tone, as the look's status line colours it: red for a work stopped for the owner or by the engine,
/// as its card is; grey for one stopped for anything else.
const DOT: Record<Tone, string> = {
  open: 'dot dot-running',
  running: 'dot dot-running',
  done: 'dot dot-added',
  owner: 'dot dot-removed',
  halted: 'dot dot-removed',
  waiting: 'dot',
  paused: 'dot',
  cancelled: 'dot',
  ended: 'dot',
};

/// A status line of the panel, the look's (`StatusLine`), but with the five tones of a work, and room at its right
/// end for the strip of the work's cells.
function Line({ tone, word, children }: { tone: Tone; word: ReactNode; children?: ReactNode }) {
  return (
    <div className="status-line">
      <span className={DOT[tone]} aria-hidden="true" />
      <span className="status-word">{word}</span>
      {children}
    </div>
  );
}

/// Sends what the owner does, once at a time: while one is under way, nothing else is sent; a refusal is kept to say.
function useSending(act: (what: Act) => Promise<unknown>) {
  const [waiting, setWaiting] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  const send = (what: Act): Promise<boolean> => {
    if (waiting) return Promise.resolve(false);
    setWaiting(true);
    setRefused(null);
    return act(what).then(
      () => {
        setWaiting(false);
        return true;
      },
      (e: unknown) => {
        setWaiting(false);
        setRefused(e instanceof Error ? e.message : String(e));
        return false;
      },
    );
  };
  return { waiting, refused, send };
}

/// The words for who writes a message or is named in one: a cell by the name the owner calls it.
const WRITER: Record<string, string> = {
  owner: 'You',
  engineer: 'Engineer',
  architect: 'Architect',
  assistant: 'Assistant',
  R1: 'Research',
  D1: 'Deep research',
  I1: 'Implementation',
};

/// Who writes a message or is named in one, in words: any other cell, such as one that asked a question, by its title
/// in the work's flow.
const writerOf = (x: Who | MessageCell, page: WorkPage | null): string =>
  Object.hasOwn(WRITER, x) ? WRITER[x] : titleOf(page, x);

/// Whether a work went through a cell: it finished a round of it, or has one open.
const went = (w: Summary, cell: string) => w.cells.some((c) => c.cell === cell && (c.finished > 0 || c.open !== null));

/// Where a message of a work stands in the window, to jump to it.
const messageId = (work: number, at: number) => `cw-message-${work}-${at}`;

/// A button's words with the letter of its key underlined: the first that begins a word.
function keyed(words: string, key: string): ReactNode {
  const at = [...words].findIndex((c, i) => c.toLowerCase() === key && (i === 0 || words[i - 1] === ' '));
  if (at < 0) return words;
  return (
    <>
      {words.slice(0, at)}
      <u>{words[at]}</u>
      {words.slice(at + 1)}
    </>
  );
}

export type ItemPanelProps = {
  /// The item selected, as the interface gives it.
  item: Item;
  /// Its work's page, which says the flow.
  page: PageState;
  act: (work: number, what: Act) => Promise<unknown>;
  /// Reads the content of a version of a block of its work, for its body and messages.
  loadVersion?: LoadVersion;
  /// The items it depends on that are not merged yet.
  waitsFor: number[];
  /// Whether it is a goal (mock).
  goal?: boolean;
  /// Whether the window is out, which lets R mark the item read.
  open?: boolean;
  /// Tells that the owner marked the item read.
  onRead?: () => void;
  /// Brings the item into view in the tree.
  onTitle: () => void;
};

/// The window's item. Its lines and its body are the item's own: what one item was sending, or was refused, is not
/// shown under another.
export function ItemPanel(props: ItemPanelProps) {
  return <ItemView key={props.item.number} {...props} />;
}

function ItemView({
  item,
  page,
  act,
  loadVersion = loadVersionOf,
  waitsFor,
  goal = false,
  open = true,
  onRead,
  onTitle,
}: ItemPanelProps) {
  const work = item.work;
  const read = pageIn(page);
  const send = (what: Act) => act(work.number, what);
  const mock = itemOf(item);
  const [body, setBody] = useState<Body>(() => firstBody(item));
  // Without the mocks, the body is read from the work's blocks each time its page is read: whether it was read yet,
  // and why it could not be.
  const [bodyRead, setBodyRead] = useState(mocksShown());
  const [bodyFailed, setBodyFailed] = useState<string | null>(null);
  // Without the mocks, what the owner may rule of the preconditions the work waits for, read with the body.
  const [rulings, setRulings] = useState<Ruling[]>([]);
  useEffect(() => {
    if (mocksShown() || read === null) return;
    let alive = true;
    const failed = (e: unknown) => alive && setBodyFailed(e instanceof Error ? e.message : String(e));
    readBody(read, loadVersion).then((b) => {
      if (!alive) return;
      setBody(b);
      setBodyRead(true);
      setBodyFailed(null);
    }, failed);
    readRulings(read, loadVersion).then((r) => alive && setRulings(r), failed);
    return () => {
      alive = false;
    };
  }, [read, loadVersion]);
  const [unseen, setUnseen] = useState(mock.unseen);
  const [tab, setTab] = useState<'item' | 'work'>('item');
  const [quote, setQuote] = useState<string | null>(null);
  const [offer, setOffer] = useState<{ x: number; y: number; text: string } | null>(null);
  // What the owner's last Confirm did, said under it.
  const [confirmed, setConfirmed] = useState<string | null>(null);
  // Whom the message being written goes to, where the owner chose; and how many times a question was taken to the discussion,
  // to bring the box into view once it is drawn for the cell chosen.
  const [messageTo, setMessageTo] = useState<MessageCell | null>(null);
  const [discussed, setDiscussed] = useState(0);
  const inner = useRef<HTMLDivElement>(null);
  const box = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    if (discussed === 0) return;
    box.current?.scrollIntoView({ block: 'center', behavior: 'smooth' });
    box.current?.focus({ preventScroll: true });
  }, [discussed]);
  // A question taken to the discussion: quoted into the box, which goes to the cell that asked it, or for a question of
  // the plan to the cell that wrote the plan. Nothing else changes: the work is not paused.
  const discuss = (c: Choice) => {
    setQuote(quoteOf(c));
    setMessageTo(c.asks?.cell ?? (went(work, 'D1') ? 'D1' : 'R1'));
    setDiscussed((n) => n + 1);
  };
  const jump = (at: number) =>
    document.getElementById(messageId(work.number, at))?.scrollIntoView({ block: 'center', behavior: 'smooth' });
  const about = (c: Choice) => messagesAbout(body.messages, c);
  const markAsRead = useCallback(() => {
    const clear = (ps: Part[]) => ps.map((p) => ({ ...p, changed: false }));
    markRead(item.number);
    setUnseen(false);
    setBody((was) => ({
      ...was,
      background: clear(was.background),
      problem: clear(was.problem),
      plan: clear(was.plan),
    }));
    onRead?.();
  }, [item.number, onRead]);
  const changes = changedIn(body);
  const fresh = unseen || changes > 0;
  useEffect(() => {
    if (!open || !fresh) return;
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'r' || e.ctrlKey || e.metaKey || e.altKey) return;
      const to = e.target as HTMLElement | null;
      if (to && (to.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(to.tagName))) return;
      e.preventDefault();
      markAsRead();
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, [open, fresh, markAsRead]);
  // The questions a cell asked the owner that the work waits at, after the plan's own, each with the option picked.
  const [answers, setAnswers] = useState<Record<number, number | null>>({});
  // What the owner wrote to a question with no options, by its call.
  const [written, setWritten] = useState<Record<number, string>>({});
  const questions = askedOf(work.now).map((c) => ({
    ...c,
    chosen: answers[c.asks?.call ?? -1] ?? null,
    answer: written[c.asks?.call ?? -1] ?? '',
  }));
  // Whether a question has its answer: an option picked, or for one with no options, words written.
  const answered = (q: Choice) => (q.options.length === 0 ? (q.answer ?? '').trim() !== '' : q.chosen !== null);
  // The plan as drawn: its own parts, a choice of them open while the plan waits to be confirmed, then the questions.
  const plan: Part[] = [
    ...(body.planWaits ? body.plan : body.plan.map((p) => (p.kind === 'choice' ? { ...p, confirmed: true } : p))),
    ...questions,
  ];
  // A press on an option picks it, and a press on the one picked lets it go.
  const choose = (at: number, option: number) => {
    const call = questions[at - body.plan.length]?.asks?.call;
    if (call !== undefined) {
      setAnswers((was) => ({ ...was, [call]: was[call] === option ? null : option }));
      return;
    }
    setBody((was) => ({
      ...was,
      plan: was.plan.map((p, i) =>
        i === at && p.kind === 'choice' ? { ...p, chosen: p.chosen === option ? null : option } : p,
      ),
    }));
  };
  // Words written to a question with no options.
  const write = (at: number, text: string) => {
    const call = questions[at - body.plan.length]?.asks?.call;
    if (call !== undefined) setWritten((was) => ({ ...was, [call]: text }));
  };
  // One Confirm for the plan's choices and the questions: each question's answer, the label of its option or the
  // words written to it, is sent to the call that waits for it, one after another; once all are taken, the plan's
  // picks stand.
  const answering = useSending(send);
  const ownPicks = body.planWaits ? picks(body) : { picked: 0, all: 0 };
  const count = {
    picked: ownPicks.picked + questions.filter(answered).length,
    all: ownPicks.all + questions.length,
  };
  const confirm = async () => {
    for (const q of questions) {
      if (q.asks === undefined || !answered(q)) continue;
      const result = q.chosen === null ? (q.answer ?? '').trim() : q.options[q.chosen].label;
      const taken = await answering.send({ kind: 'result', call: q.asks.call, result });
      if (!taken) return;
    }
    if (body.planWaits) setBody((was) => ({ ...was, planWaits: false }));
    setConfirmed(body.planWaits ? 'You confirmed the plan.' : 'Your answer was sent.');
  };
  // A passage selected in the body offers to be quoted, by a button just above it.
  const selected = () => {
    const sel = window.getSelection();
    const text = sel?.toString().trim() ?? '';
    const host = inner.current;
    if (sel === null || text === '' || host === null || sel.rangeCount === 0) {
      setOffer(null);
      return;
    }
    const at = sel.getRangeAt(0).getBoundingClientRect();
    const frame = host.getBoundingClientRect();
    setOffer({ x: at.left - frame.left + at.width / 2, y: at.top - frame.top, text });
  };
  // A message sent stands under the others, the owner's own.
  const sent = (message: Message) => {
    setBody((was) => ({ ...was, messages: [...was.messages, message] }));
    setQuote(null);
  };
  return (
    <div className="cw-item" ref={inner}>
      <header className="drawer-head">
        <span className="drawer-id">{item.number}</span>
        <h2 className="drawer-title">
          <button type="button" className="cw-title-link" title="Show it in the tree" onClick={onTitle}>
            {item.title}
          </button>
        </h2>
        {item.project !== null && (
          <span className="drawer-belongs">
            <Attr name="project" value={item.project} />
          </span>
        )}
      </header>
      <ItemLine item={item} waitsFor={waitsFor} goal={goal} tier={mock.tier} />
      <WorkLine w={work} page={read} />
      {!ended(work) && (
        <Acts paused={work.paused || work.now.kind === 'paused'} goOn={goOnOf(work.now)} act={send} />
      )}
      <div className="drawer-body">
        <div className="cw-tabs">
          <Switch
            label="What the window shows"
            choices={[
              { value: 'item', name: 'Item' },
              { value: 'work', name: 'Work' },
            ]}
            value={tab}
            onChange={setTab}
          />
        </div>
        {fresh ? (
          <div className="cw-fresh">
            <span>
              {changes === 0
                ? 'New to you'
                : changes === 1
                  ? '1 passage changed since you last read it'
                  : `${changes} passages changed since you last read it`}
            </span>
            <button type="button" className="link" aria-keyshortcuts="r" onClick={markAsRead}>
              {keyed('Mark as read', 'r')}
            </button>
          </div>
        ) : null}
        {tab === 'work' ? (
          <WorkFacts w={work} page={read} verdicts={mock.verdicts} />
        ) : (
          <>
            <div className="cw-body" onMouseUp={selected}>
              {bodyFailed !== null && <p className="failed small">The page could not read the item: {bodyFailed}</p>}
              <Section
                title="Background"
                parts={body.background}
                empty={bodyRead ? 'Nothing is written yet' : 'Reading'}
              />
              <Section title="Problem" parts={body.problem} />
              <Section
                title="Plan"
                parts={plan}
                keep={rulings.length > 0}
                choose={body.planWaits || questions.length > 0 ? choose : undefined}
                write={questions.length > 0 ? write : undefined}
                discuss={discuss}
                about={about}
                jump={jump}
                after={
                  <>
                    {body.planWaits || questions.length > 0 ? (
                      <button
                        type="button"
                        className="button button-primary cw-confirm"
                        disabled={answering.waiting || count.picked < count.all}
                        onClick={() => void confirm()}
                      >
                        {count.all > 0 ? `Confirm (${count.picked}/${count.all})` : 'Confirm'}
                      </button>
                    ) : null}
                    {answering.refused !== null && <p className="failed small">{answering.refused}</p>}
                    {confirmed !== null ? <p className="cw-done">{confirmed}</p> : null}
                    {rulings.map((r) => (
                      <RulingBox key={`${r.block}@${r.at}`} ruling={r} page={read} act={send} />
                    ))}
                  </>
                }
              />
            </div>
            {offer !== null ? (
              <button
                type="button"
                className="button cw-quote-offer"
                style={{ left: offer.x, top: offer.y }}
                onMouseDown={(e: MouseEvent) => e.preventDefault()}
                onClick={() => {
                  setQuote(offer.text);
                  setOffer(null);
                  window.getSelection()?.removeAllRanges();
                  box.current?.focus();
                }}
              >
                <QuoteSign />
                Quote
              </button>
            ) : null}
            <Discussion
              work={work}
              page={page}
              messages={body.messages}
              messageTo={messageTo}
              setMessageTo={setMessageTo}
              quote={quote}
              unquote={() => setQuote(null)}
              box={box}
              act={send}
              sent={sent}
            />
          </>
        )}
      </div>
    </div>
  );
}

/// The sign of a quote, 12 pixels square, as Primer's octicon.
function QuoteSign() {
  return (
    <svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
      <path d="M1.75 2.5h10.5a.75.75 0 0 1 0 1.5H1.75a.75.75 0 0 1 0-1.5Zm4 5h8.5a.75.75 0 0 1 0 1.5h-8.5a.75.75 0 0 1 0-1.5Zm0 5h8.5a.75.75 0 0 1 0 1.5h-8.5a.75.75 0 0 1 0-1.5ZM2.5 7.75v6a.75.75 0 0 1-1.5 0v-6a.75.75 0 0 1 1.5 0Z" />
    </svg>
  );
}

/// The sign of a cross, 12 pixels square, as Primer's octicon.
function CrossSign() {
  return (
    <svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
      <path d="M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.749.749 0 0 1 1.275.326.749.749 0 0 1-.215.734L9.06 8l3.22 3.22a.749.749 0 0 1-.326 1.275.749.749 0 0 1-.734-.215L8 9.06l-3.22 3.22a.751.751 0 0 1-1.042-.018.751.751 0 0 1-.018-1.042L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06Z" />
    </svg>
  );
}

function ItemLine({ item, waitsFor, goal, tier }: {
  item: Item;
  waitsFor: number[];
  goal: boolean;
  tier: string | null;
}) {
  const standing = standingOf(item);
  return (
    <Line tone={standing} word={STANDING_WORDS[standing]}>
      {goal ? <span className="muted">Goal</span> : null}
      {tier !== null ? <span className="muted">Tier {tier}</span> : null}
      <span className="muted">Added {ago(item.added)} ago</span>
      {waitsFor.length > 0 && <span className="muted">Waits for {waitsFor.join(', ')} to merge</span>}
    </Line>
  );
}

function WorkLine({ w, page }: { w: Summary; page: WorkPage | null }) {
  const { word, facts } = nowWords(w, page);
  return (
    <Line tone={ended(w) ? 'ended' : moodOf(w)} word={word}>
      {/* What it does in one phrase, then where. */}
      {facts !== null && <span className="muted">{facts}</span>}
      <a href={`#/works/${w.number}`} className="link mono" title={`Open work ${w.number}`}>
        W{w.number}
      </a>
      {/* How far it has come and where its time went, together at the line's right end. */}
      <span className="cw-progress">
        <Strip cells={w.cells} />
        <span className="cw-track">
          <TimeTrack spans={barSpans(w.bar ?? (page === null ? '' : barOf(page)))} />
        </span>
      </span>
    </Line>
  );
}

/// What the owner does to a work that goes on: let it go on past a limit it waits at; start it or pause it, with one
/// switch whose pressed choice says which it is in; cancel it, once they confirm. While one is under way, none takes
/// effect.
function Acts({ paused, goOn, act }: { paused: boolean; goOn: Act | null; act: (what: Act) => Promise<unknown> }) {
  const { waiting, refused, send } = useSending(act);
  const [asking, setAsking] = useState(false);
  return (
    <div className="acts cw-acts">
      {goOn !== null && (
        <Button primary title="Let the work go on past its limit" disabled={waiting} onClick={() => void send(goOn)}>
          Let it go on
        </Button>
      )}
      {/* Going on and paused exclude each other, so they are one switch that shows which the work is in: pressing the
          other choice starts or pauses it. */}
      <Switch<'on' | 'paused'>
        label="Whether the work goes on"
        choices={[
          { value: 'on', name: 'Start', title: 'Let the work go on' },
          {
            value: 'paused',
            name: 'Pause',
            title: 'Nothing of the work goes on by itself; what you write to it is still answered',
          },
        ]}
        value={paused ? 'paused' : 'on'}
        onChange={(to) => {
          if (waiting || (to === 'paused') === paused) return;
          void send({ kind: to === 'paused' ? 'pause' : 'resume' });
        }}
      />
      <Button
        title="Cancel the work and its item"
        disabled={waiting}
        onClick={() => setAsking(true)}
      >
        Cancel
      </Button>
      {asking && (
        <Confirm
          title="Cancel this work?"
          verb="Cancel the work"
          onAnswer={(yes) => {
            setAsking(false);
            if (yes) void send({ kind: 'cancel' });
          }}
        >
          It cannot be let on again.
        </Confirm>
      )}
      {refused !== null && <span className="failed small">{refused}</span>}
    </div>
  );
}

/// Where a draft of a message is kept, so that it is not lost when the panel shows another item.
const draftKey = (work: number, block: string) => `wf-agents.console-message.${work}.${block}`;

function readDraft(key: string): string {
  try {
    return localStorage.getItem(key) ?? '';
  } catch {
    return '';
  }
}

function writeDraft(key: string, text: string) {
  try {
    if (text === '') localStorage.removeItem(key);
    else localStorage.setItem(key, text);
  } catch {
    // Kept for this page only.
  }
}

/// Which sections of the item's text the owner folded away, kept in this browser, so that one folded stays folded
/// from item to item.
const FOLDED = 'console-window-folded';
function foldedNow(): string[] {
  try {
    const kept = JSON.parse(localStorage.getItem(FOLDED) ?? '[]') as unknown;
    return Array.isArray(kept) ? kept.filter((k): k is string => typeof k === 'string') : [];
  } catch {
    return [];
  }
}

/// A section of the item's window: a whole row in the accent's colour that names it and folds it away at a press,
/// and under it what it holds.
function Fold({ title, count, children }: { title: string; count?: number; children: ReactNode }) {
  const [folded, setFolded] = useState(() => foldedNow().includes(title));
  const turn = () => {
    const now = !folded;
    setFolded(now);
    try {
      const kept = foldedNow().filter((k) => k !== title);
      localStorage.setItem(FOLDED, JSON.stringify(now ? [...kept, title] : kept));
    } catch {
      // Kept only where the browser lets it be.
    }
  };
  return (
    <section className="cw-fold">
      <button type="button" className="cw-fold-head" aria-expanded={!folded} onClick={turn}>
        <span className="cw-fold-sign" aria-hidden="true">
          {folded ? '▸' : '▾'}
        </span>
        <span>{title}</span>
        {count !== undefined && <span className="cw-fold-count mono">{count}</span>}
      </button>
      {!folded && <div className="cw-fold-body">{children}</div>}
    </section>
  );
}

function Section({
  title,
  parts,
  keep = false,
  choose,
  write,
  discuss,
  about,
  jump,
  after,
  empty,
}: {
  title: string;
  parts: Part[];
  /// Draws the section with no parts, for what stands under them.
  keep?: boolean;
  /// Picks an option of a choice, while the plan waits to be confirmed.
  choose?: (at: number, option: number) => void;
  /// Writes the answer to a question with no options.
  write?: (at: number, text: string) => void;
  /// Takes a choice to the discussion.
  discuss?: (c: Choice) => void;
  /// The messages about a choice, by where they stand.
  about?: (c: Choice) => number[];
  /// Brings a message into view.
  jump?: (at: number) => void;
  /// What stands under its parts, inside the section, and is folded away with it.
  after?: ReactNode;
  /// What the section says, muted, where it has no parts; where this is not given, it is not drawn.
  empty?: string;
}) {
  if (parts.length === 0 && !keep) {
    if (empty === undefined) return null;
    return (
      <Fold title={title}>
        <p className="muted">{empty}</p>
      </Fold>
    );
  }
  return (
    <Fold title={title}>
      {parts.map((p, i) => (
        <div key={i} className={p.changed === true ? 'cw-part cw-changed' : 'cw-part'}>
          {p.kind === 'text' ? (
            <Markdown text={p.text} />
          ) : (
            <ChoiceBox
              p={p}
              pick={choose === undefined || p.confirmed === true ? undefined : (o) => choose(i, o)}
              write={write === undefined || p.confirmed === true ? undefined : (t) => write(i, t)}
              discuss={discuss === undefined ? undefined : () => discuss(p)}
              messages={about?.(p) ?? []}
              jump={jump}
            />
          )}
        </div>
      ))}
      {after}
    </Fold>
  );
}

/// A choice of the plan: a press on an option picks it, on another changes the pick, and on the one picked lets it go,
/// until it is confirmed. Discuss takes it to the discussion, confirmed or not; under its options, how many messages are about
/// it, which brings the first into view. A question with no options is answered in words, in a box in their place.
function ChoiceBox({ p, pick, write, discuss, messages, jump }: {
  p: Choice;
  pick?: (option: number) => void;
  write?: (text: string) => void;
  discuss?: () => void;
  messages: number[];
  jump?: (at: number) => void;
}) {
  return (
    <div className={pick === undefined ? 'cw-choice cw-choice-done' : 'cw-choice'}>
      <p className="cw-choice-q">{p.question}</p>
      {p.asks !== undefined && <p className="cw-choice-stops">The work waits for this answer</p>}
      {p.options.length === 0 && p.asks !== undefined ? (
        <textarea
          rows={3}
          value={p.answer ?? ''}
          aria-label="Your answer"
          placeholder="Write your answer"
          disabled={write === undefined}
          onChange={(e) => write?.(e.target.value)}
        />
      ) : null}
      {p.options.map((o, i) => (
        <button
          key={o.label}
          type="button"
          disabled={pick === undefined}
          aria-pressed={p.chosen === i}
          className={p.chosen === i ? 'cw-option on' : 'cw-option'}
          onClick={() => pick?.(i)}
        >
          <span className="cw-option-label">
            {o.label}
            {p.recommended === i ? <span className="tag cw-tag">Recommended</span> : null}
          </span>
          <span className="cw-option-detail">{o.detail}</span>
        </button>
      ))}
      {messages.length > 0 || discuss !== undefined ? (
        <div className="cw-choice-foot">
          {messages.length > 0 ? (
            <button type="button" className="link muted small" onClick={() => jump?.(messages[0])}>
              {messages.length === 1 ? '1 message about it' : `${messages.length} messages about it`}
            </button>
          ) : (
            <span />
          )}
          {discuss !== undefined ? (
            <Button
              title={
                p.asks !== undefined
                  ? 'Quote it in a message to the cell that asked it'
                  : 'Quote it in a message to the cell that wrote the plan; the work goes on'
              }
              onClick={discuss}
            >
              Discuss
            </Button>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}

/// A block that keeps a precondition the work waits for from holding, drawn as a question that stops the work is: who
/// wrote its current version, a cell by the name the owner calls it, and what it says; under it, as wide as the body,
/// the button that writes the block with the value that makes the test hold, where its values make that plain. To have
/// the cell judge again, the owner writes to it in the discussion.
function RulingBox({ ruling, page, act }: {
  ruling: Ruling;
  page: WorkPage | null;
  act: (what: Act) => Promise<unknown>;
}) {
  const { waiting, refused, send } = useSending(act);
  const by = ruling.by;
  const who =
    by.kind === 'run'
      ? writerOf(by.cell, page)
      : by.kind === 'person'
        ? WRITER.owner
        : (page?.flow.blocks.find((b) => b.name === ruling.block)?.writtenBy.outside ?? 'The outside');
  const value = ruling.value;
  return (
    <>
      <div className="cw-part">
        <div className="cw-choice">
          <p className="cw-choice-q">{who}</p>
          <p className="cw-choice-stops">The work waits for this ruling</p>
          {ruling.text.trim() !== '' && <Markdown text={ruling.text} />}
        </div>
      </div>
      {value !== null && (
        <button
          type="button"
          className="button button-primary cw-confirm"
          disabled={waiting}
          onClick={() => void send({ kind: 'write', block: ruling.block, content: '', value })}
        >
          Count it as settled
        </button>
      )}
      {refused !== null && <p className="failed small">{refused}</p>}
    </>
  );
}

/// The discussion: the messages written before, the owner's and the replies, and a box to write one. The owner picks a block of
/// the work that people write, of those that hold words rather than a choice, and what they send is written into it;
/// the line under the box names the cells that read it, and those the message names with `@`.
/// Whom the owner writes a message to, of the engineer's flow: the cells that plan and build, by the names the owner
/// calls them.
const MESSAGE_TO = [
  { cell: 'R1', name: 'Research' },
  { cell: 'D1', name: 'Deep research' },
  { cell: 'I1', name: 'Implementation' },
] as const;

function Discussion({ work, page, messages, messageTo, setMessageTo, quote, unquote, box, act, sent }: {
  work: Summary;
  page: PageState;
  messages: Message[];
  /// Whom the owner chose the message goes to; none where they did not.
  messageTo: MessageCell | null;
  setMessageTo: (cell: MessageCell) => void;
  quote: string | null;
  unquote: () => void;
  box: RefObject<HTMLTextAreaElement | null>;
  act: (what: Act) => Promise<unknown>;
  sent: (message: Message) => void;
}) {
  const read = pageIn(page);
  const blocks = (read?.flow.blocks ?? []).filter((b) => b.writtenBy.people && b.values === null);
  // Whom a message goes to: the research, the deep research or the implementation, and a cell that asked the owner a
  // question the work waits at, each by the block of the owner's words its cell reads. The deep research cannot be
  // chosen where the work did not go through it. Once the work has come to the implementation, a message goes there
  // unless the owner picks another; before that, to the research.
  const askers = [...new Set(askedOf(work.now).flatMap((c) => (c.asks === undefined ? [] : [c.asks.cell])))];
  const cells: { cell: MessageCell; name: string }[] = [
    ...MESSAGE_TO,
    ...askers.filter((c) => !MESSAGE_TO.some((t) => t.cell === c)).map((c) => ({ cell: c, name: titleOf(read, c) })),
  ];
  const targets = cells.map((t) => ({
    ...t,
    // The block that is the owner's words to this cell alone: one people write that this cell reads and no other cell
    // does. A block several cells read, such as the background, is no message to one of them and is never written
    // from here.
    block: blocks.find((b) => b.readBy.cells.length === 1 && b.readBy.cells[0] === t.cell),
    off: t.cell === 'D1' && !went(work, 'D1'),
  }));
  const usable = targets.filter((t) => t.block !== undefined && !t.off);
  const target = usable.find((t) => t.cell === (messageTo ?? (went(work, 'I1') ? 'I1' : 'R1'))) ?? usable[0];
  const block = target?.block;
  return (
    <Fold title="Discussion" count={messages.length}>
      {messages.map((n, i) => (
        <div key={i} id={messageId(work.number, i)} className={n.who === 'owner' ? 'cw-message cw-message-owner' : 'cw-message'}>
          <div className="status-line">
            <span className="status-word">{writerOf(n.who, read)}</span>
            <span className="muted">to {n.to.map((x) => writerOf(x, read)).join(', ')}</span>
            <span className="mono muted">{ago(n.at)} ago</span>
          </div>
          {n.quote !== null ? <blockquote className="cw-quote">{n.quote}</blockquote> : null}
          <Markdown text={n.text} />
        </div>
      ))}
      {page === null ? (
        <p className="muted">Reading the work's flow</p>
      ) : 'failed' in page ? (
        <p className="failed small">The page could not read the work: {page.failed}</p>
      ) : block === undefined ? (
        <p className="muted">People write no block of this flow</p>
      ) : (
        <div className="write-box cw-compose">
          <div className="cw-to switch" role="radiogroup" aria-label="Whom the message goes to">
            {targets.map((t) => (
              <button
                key={t.cell}
                type="button"
                role="radio"
                className="switch-choice"
                aria-checked={t.cell === target?.cell}
                aria-pressed={t.cell === target?.cell}
                disabled={t.block === undefined || t.off}
                title={t.off ? 'The work did not go through the deep research' : undefined}
                onClick={() => setMessageTo(t.cell)}
              >
                {t.name}
              </button>
            ))}
          </div>
          <Compose
            key={draftKey(work.number, block.name)}
            work={work}
            block={block.name}
            to={target?.cell ?? 'R1'}
            readers={block.readBy.cells.map((c) => titleOf(read, c))}
            quote={quote}
            unquote={unquote}
            box={box}
            act={act}
            sent={sent}
          />
        </div>
      )}
    </Fold>
  );
}

/// The box for one block of one work, its draft kept as it is typed, with the passage quoted into it over it, and the
/// names one can write to offered when `@` is typed.
function Compose({ work, block, to: cell, readers, quote, unquote, box, act, sent }: {
  work: Summary;
  block: string;
  /// The cell the message goes to.
  to: MessageCell;
  readers: string[];
  quote: string | null;
  unquote: () => void;
  box: RefObject<HTMLTextAreaElement | null>;
  act: (what: Act) => Promise<unknown>;
  sent: (message: Message) => void;
}) {
  const key = draftKey(work.number, block);
  const [text, setText] = useState(() => readDraft(key));
  const [written, setWritten] = useState(false);
  const { waiting, refused, send } = useSending(act);
  const working = !ended(work);
  const type = (t: string) => {
    setText(t);
    setWritten(false);
    writeDraft(key, t);
  };
  // Typing `@` offers the names one can write to, the engineer only while a work goes on.
  const naming = /(^|\s)@(\w*)$/.exec(text);
  const offered: Who[] = (['engineer', 'architect', 'assistant'] as Who[]).filter(
    (x) => (x !== 'engineer' || working) && x.startsWith((naming?.[2] ?? '').toLowerCase()),
  );
  const name = (x: Who) => {
    type(text.replace(/@(\w*)$/, `@${x} `));
    box.current?.focus();
  };
  const to = [...readers, ...named(text, working).map((x) => WRITER[x])];
  const submit = () => {
    const words = text.trim();
    const content = quote === null ? words : `${quote.replace(/^/gm, '> ')}\n\n${words}`;
    void send({ kind: 'write', block, content, value: null }).then((done) => {
      if (!done) return;
      writeDraft(key, '');
      setText((now) => (now.trim() === words ? '' : now));
      setWritten(true);
      const at = named(words, working);
      sent({ who: 'owner', at: Date.now(), to: [cell, ...at], quote, text: words });
    });
  };
  return (
    <>
      {quote !== null ? (
        <div className="cw-quoted">
          <span className="cw-quote">{quote}</span>
          <button type="button" className="cw-unquote" aria-label="Remove the quote" title="Remove the quote" onClick={unquote}>
            <CrossSign />
          </button>
        </div>
      ) : null}
      <textarea
        ref={box}
        rows={3}
        value={text}
        aria-label={`What to write in ${block}`}
        placeholder="Write a message"
        onChange={(e) => type(e.target.value)}
      />
      {naming !== null && offered.length > 0 ? (
        <div className="cw-names">
          {offered.map((x) => (
            <button key={x} type="button" className="button" onClick={() => name(x)}>
              @{x} · {WRITER[x]}
            </button>
          ))}
        </div>
      ) : null}
      <div className="write-line">
        <span className="muted small write-readers">{to.length > 0 ? `Goes to ${to.join(', ')}` : 'No cell reads it'}</span>
        <Button primary disabled={waiting || text.trim() === ''} onClick={submit}>
          Send
        </Button>
      </div>
      {written && <p className="cw-done">Written in {block}</p>}
      {refused !== null && <p className="failed small">{refused}</p>}
    </>
  );
}

/// How many of a work's rounds were given back what the round before handed in, from its page where the summary does
/// not say.
function sentBackOf(w: Summary, page: WorkPage | null): number {
  if (w.sentBack !== undefined) return w.sentBack;
  return (page?.steps ?? []).reduce(
    (n, s) => n + (s.kind === 'run' ? s.rounds.filter((r) => r.given.kind === 'sentBack').length : 0),
    0,
  );
}

/// What of the work the lines do not say, and what the marks of the strip in its line mean.
function WorkFacts({ w, page, verdicts }: { w: Summary; page: WorkPage | null; verdicts: [string, string][] }) {
  const tier = workTierOf();
  const rows: [string, ReactNode][] = [
    ['Rounds', String(w.spent.rounds)],
    ['Cost', dollars(w.spent.usdMicros)],
    ['Started', w.from === null ? 'Not yet' : `${ago(w.from ?? w.added)} ago`],
    ...(tier === null ? [] : [['Tier', tier] as [string, ReactNode]]),
    ['Sent back', String(sentBackOf(w, page))],
    ['Flow', w.flow],
    ...verdicts,
  ];
  return (
    <>
      <dl className="pairs panel-part panel-part-last">
        {rows.map(([key, value]) => (
          <Fragment key={key}>
            <dt>{key}</dt>
            <dd>{value}</dd>
          </Fragment>
        ))}
      </dl>
      <PanelPart label="The cells' marks">
        <StateLegend />
      </PanelPart>
    </>
  );
}
