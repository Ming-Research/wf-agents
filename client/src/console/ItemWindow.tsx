// What the window beside the console shows of the item selected: its number, its title, which brings the item into view
// in the tree, and its project; a line of where the item stands, with the level the work's cell runs at, the works
// page's strip of the work's cells and where its time went at its right, and a line of what its work does, the lines it
// changed at its right end; what the owner does to the work (let it on, pause it, cancel it); four tabs, Item, Work,
// Step and Usage, the same on the console and on a work's page; and under them, while something in the item is new to
// the owner, a strip that says what changed, with a link to mark the item read. The first tab holds the body (a section
// for each datum the work's flow's card names, in its order and by its name, with each choice that waits for the owner
// where it stands in it, and each question put to the owner that stands, once and only there: at the first pointer to
// it, `{{name}}`, in a passage of the body or in a message an agent wrote, a later pointer to it being a link to it,
// and after the last section where no text points to it; drawn as they are: what it asks, as it was written, its
// options with what each does and what follows, and the one it recommends; one the engine's one list of what waits on
// the owner names on the red tint, framed in red, the look of every stop, an answered one settled, its options words
// with the one taken marked and nothing to press. Each is picked by a press on an option and let go by another, and at
// the end of the last section a button as wide as the body confirms them, which counts the picks, waits until each
// choice has one and sends each question's answer to the engine; once confirmed, the picks stand, settled; and last,
// each block the owner may write that keeps the work from going on, drawn as such a question is, its values the
// options, with a button that writes the one picked), each passage changed since the owner read it marked at its left,
// and under the body the messages: the owner's and the replies, what the owner said from the window standing under them
// until a read of the work brings it, and a box to write one to a line of the work's flow, picked above it among those
// on which the engine has a cell to give the words to, which a passage selected in the body can be quoted into and
// which names that cell. Under the work's state line, each other stop of the work, in the same
// look: one sentence of why and the buttons that end it, from the engine's list, and a fault or a stall, which no one
// waits on, said as the engine gives it. The second tab holds what of the work the lines do not say, and what the work
// changed, file by file, each opening its diff; the third, Step, everything about the
// row of the work's history the page chose, else the run the work stands at; the fourth, Usage, what the work used.
// Each page keeps its own choice of tab. While the window is out, R marks the item read, as its link does, unless a box
// has the keys. What the owner sends is sent once: its button cannot be pressed while it is under way, and a refusal is
// said under the item it was for, and only there.
//
// Brought over from the console of the engine on `old-main` (`client/src/console/ItemWindow.tsx`), its layout as it
// was, its parts the page's own (parts.tsx) where it used Primer's. What the engine does not give yet comes from the
// console's made-up data (`mock.ts`), which says what of it is made up, when the page shows the mocks; otherwise the
// body and the messages are read from the work's blocks, and the rest is not drawn (`real.ts`). What the new console
// added is kept in that layout: the work line's facts, the row of what the owner does to the work under the lines, the
// block a message is written in over its box, and the flow's name among the work's facts.
//
// A work's page shows the same window for the work's item, with the row of the work's history chosen and the title
// leading to the item in the console, which the page gives (`row`, `titleHref`), and its own choice of tab (`tab`).

import './console.css';

import {
  Fragment,
  type MouseEvent,
  type ReactNode,
  type RefObject,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';

import { CHOICE_KEYS, useKept } from '../kept';
import { LevelMark, LinesChanged } from '../marks';
import { mocksShown } from '../mock-data';
import { Button, Coded, Confirm, Fold, ListRow, Switch, TICK, Tabs } from '../parts';
import {
  type Act,
  type ChangesPage,
  type DiffPage,
  type EngineAct,
  type FileChange,
  type Item,
  loadVersion as loadVersionOf,
  type Question,
  type Said,
  type SpokenTo,
  type Summary,
  type Waited,
  type WorkPage,
} from '../works-api';
import { CallsTook, type Got, Loaded, Parts, Strip, useWanted } from '../works/marks';
import { type HistoryRow, placeSteps, sessionsBegun, standingRun } from '../works/placing';
import { type Reader, readerOf, useSource } from '../works/source';
import { StepTab } from '../works/StepPanel';
import { UsageTab } from '../works/Usage';
import { type Wait, type WaitAct, actOf, drawnInBody, waitsOf } from '../works/waiting';
import {
  CANCEL_DOES,
  CANCEL_WHY,
  ENDS_WORDS,
  NO_ENGINE,
  OWN_PAUSE,
  SHOW_QUESTION,
  TAKEN,
  type Tone as WorkTone,
  WRITE_WORDS,
  count,
  stateOf,
  stateParts,
  stoppedAt,
  waitedWords,
} from '../works/words';
import { Diff } from './diff';
import { type ItemStanding, age, dollars, linesOf, standingOf } from './items';
import { Markdown, type Pointers } from './markdown';
import { markRead } from './mock';
import {
  askedChoice,
  firstBody,
  itemOf,
  type LoadVersion,
  readBody,
  readRulings,
  type Ruling,
  wordsOf,
  workTierOf,
} from './real';
import { barOf, barSpans, TimeTrack } from './track';
import {
  type Body,
  changedIn,
  type Choice,
  heldAt,
  type Message,
  type MessageCell,
  messagesAbout,
  type Part,
  picks,
  type Pointing,
  quoteOf,
  type Who,
} from './window';

const ago = (at: number) => age(Date.now() - at).label;

const STANDING_WORDS: Record<ItemStanding, string> = { open: 'Open', done: 'Done', cancelled: 'Cancelled' };

/// Whether a work has ended: merged or cancelled.
const ended = (s: Summary) => s.now.kind === 'merged' || s.now.kind === 'cancelled';

/// The work's page as the panel has it: read, not read, or still being read.
export type PageState = { page: WorkPage } | { failed: string } | null;

const pageIn = (p: PageState): WorkPage | null => (p !== null && 'page' in p ? p.page : null);

/// The window's tabs, in the order its switch shows them, on the console and on a work's page alike.
export const WINDOW_TABS = ['item', 'work', 'step', 'usage'] as const;
export type WindowTab = (typeof WINDOW_TABS)[number];

const TAB_NAMES: Record<WindowTab, string> = { item: 'Item', work: 'Work', step: 'Step', usage: 'Usage' };

/// What a tab that draws the work's page says until it is read, or where it could not be.
function Unread({ page }: { page: PageState }) {
  return page !== null && 'failed' in page ? (
    <p className="failed small">The page could not read the work: {page.failed}</p>
  ) : (
    <p className="muted">Reading the work</p>
  );
}

/// A cell's title in the work's flow, or its id where the flow does not name it.
const titleOf = (page: WorkPage | null, cell: string) => page?.flow.cells.find((c) => c.id === cell)?.title ?? cell;

/// Whether what a work waits for is a question a cell asked the owner as a call, which is drawn in the plan as a choice
/// rather than said on the status line.
const asked = (w: Waited): w is Extract<Waited, { kind: 'call' }> & { what: { kind: 'question' } } =>
  w.kind === 'call' && w.what.kind === 'question';

/// What a question's card says of the engine's wait for its answer, where the engine's list names one: that it holds
/// the work.
function holding(s: Summary, key: string): Pick<Choice, 'holds'> {
  return { holds: waitsOf(s).some((w) => w.key === key) };
}

/// The questions a cell asked the owner as a call the work waits at, as choices at the plan's end, each holding the work
/// where the engine's list names it.
function askedOf(s: Summary): Choice[] {
  const now = s.now;
  if (now.kind !== 'waits') return [];
  return now.on.flatMap((w) =>
    asked(w) && w.who === 'owner'
      ? [{ ...askedChoice(w.what.question, w.call, now.at), ...holding(s, callKey(w.call)) }]
      : [],
  );
}

/// The key the engine's list gives the wait for a question put as a call, and for one of the work's own by its name.
const callKey = (call: number) => `call-${call}`;
const answerKey = (question: string) => `answer-${question}`;

/// The questions put to the owner that stand, as the work's page gives them, each as a choice: what it asks, its
/// options, the one it recommends, and the owner's answer to its latest version, which confirms it. One the engine's
/// list names holds the work, at its cell or at the merge.
function questionsStanding(questions: Question[] | undefined, s: Summary): Choice[] {
  return (questions ?? []).flatMap((q): Choice[] => {
    // A question no version of its cell's questions names cannot be answered by one.
    if (q.withdrawn !== null || q.ask === null || q.version === null) return [];
    const answered = q.answer !== null;
    const held = answered ? { holds: false } : holding(s, answerKey(q.name));
    return [
      {
        kind: 'choice',
        question: q.ask,
        options: q.options.map((o) => ({ label: o.does, detail: o.follows })),
        recommended: q.recommends === null ? null : q.recommends - 1,
        chosen: q.answer === null ? null : q.answer.option - 1,
        confirmed: answered,
        asks: { cell: q.cell, question: q.name, version: q.version },
        ...held,
      },
    ];
  });
}

/// Where a text that may hold a question stands, as `heldAt` names it: a passage of a section, by the section's title
/// and its place there, or a message of the discussion, by its place.
const partKey = (section: string, at: number) => `part ${at} of ${section}`;
const messageKey = (at: number) => `message ${at}`;

/// What the owner's pick of a question is kept by: the call that waits for it, or the version of the question.
const pickKey = (c: Choice): string =>
  c.asks === undefined ? '' : 'call' in c.asks ? `call ${c.asks.call}` : `${c.asks.question}@${c.asks.version}`;

/// The key of the wait in the engine's list that a question's choice answers, which its card carries.
const waitKeyOf = (c: Choice): string | undefined =>
  c.asks === undefined ? undefined : 'call' in c.asks ? callKey(c.asks.call) : answerKey(c.asks.question);

/// A cell as the page shows one: its id in the monospace, muted, and its title semibold, which the work's flow gives.
function CellName({ cell, page }: { cell: string; page: WorkPage | null }) {
  const title = page?.flow.cells.find((c) => c.id === cell)?.title;
  return (
    <span>
      <span className="cell-id">{cell}</span>
      {title !== undefined && (
        <>
          {' '}
          <span className="cell-title">{title}</span>
        </>
      )}
    </span>
  );
}

/// The tone of a dot: an item's standing, or what its work does.
type Tone = ItemStanding | WorkTone;

/// The dot of each tone, as the look's status line colours it: red for a stop, a work that waits for the owner or one
/// the engine halted, as its card is; the yellow of a call for one that waits for the calls of tools; grey for one that
/// waits for anything else.
const DOT: Record<Tone, string> = {
  open: 'dot dot-running',
  running: 'dot dot-running',
  calling: 'dot dot-calling',
  done: 'dot dot-added',
  owner: 'dot dot-removed',
  failed: 'dot dot-removed',
  quiet: 'dot',
  cancelled: 'dot',
};

/// A status line of the panel, the look's (`StatusLine`), but with the tones of a work, and room at its right end for
/// the strip of the work's cells.
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
  // What the engine answered, where it took the act; none where it was not sent or was refused.
  const send = (what: Act): Promise<{ answer: unknown } | null> => {
    if (waiting) return Promise.resolve(null);
    setWaiting(true);
    setRefused(null);
    return act(what).then(
      (answer) => {
        setWaiting(false);
        return { answer };
      },
      (e: unknown) => {
        setWaiting(false);
        setRefused(refusal(e));
        return null;
      },
    );
  };
  return { waiting, refused, send };
}

/// Why what the owner did was refused, in words.
const refusal = (e: unknown): string => (e instanceof Error ? e.message : String(e));

/// The words for who writes a message or is named in one that is no cell of a flow.
const WRITER: Record<string, string> = {
  owner: 'You',
  architect: 'Architect',
  assistant: 'Assistant',
};

/// Who writes a message or is named in one, in words: a cell, such as one that asked a question, by its title in the
/// work's flow.
const writerOf = (x: Who | MessageCell, page: WorkPage | null): string =>
  Object.hasOwn(WRITER, x) ? WRITER[x] : titleOf(page, x);

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
  /// Whether it is a goal (mock).
  goal?: boolean;
  /// Whether the window is out, which lets R mark the item read.
  open?: boolean;
  /// Tells that the owner marked the item read.
  onRead?: () => void;
  /// Brings the item into view in the tree, where the window stands beside it.
  onTitle?: () => void;
  /// Where the title leads instead, where the window stands on another page than the tree's.
  titleHref?: string;
  /// The row of the work's history the owner chose, which Step shows, where the page draws the history and one is
  /// chosen.
  row?: HistoryRow;
  /// The tab shown, where the page chooses it, and what is told of the owner's choice of another; else the window keeps
  /// the console's choice itself.
  tab?: WindowTab;
  onTab?: (tab: WindowTab) => void;
  /// The card of what the work waits for that the window is to show, by its key (`Wait.key`), where the address names
  /// one.
  wait?: string;
  /// What the owner does to the engine as a whole, where the page has an engine.
  actEngine?: (what: EngineAct) => Promise<unknown>;
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
  goal = false,
  open = true,
  onRead,
  onTitle,
  titleHref,
  row,
  tab: chosenTab,
  onTab,
  wait,
  actEngine,
}: ItemPanelProps) {
  const work = item.work;
  const read = pageIn(page);
  const send = (what: Act) => act(work.number, what);
  // What Step and Usage read of the work: the rows of its history and the sessions each run began, and its contents,
  // each read once.
  const source = useSource();
  const reader = useMemo(() => readerOf(source, work.number), [source, work.number]);
  const rows = useMemo(() => (read === null ? [] : placeSteps(read)), [read]);
  const sessions = useMemo(() => sessionsBegun(read?.steps ?? []), [read]);
  const standing = read === null ? null : standingRun(read);
  const stepRow = row ?? rows.find((r) => r.step.kind === 'run' && r.step.run === standing);
  const mock = itemOf(item);
  const [body, setBody] = useState<Body>(() => firstBody(item));
  // Without the mocks, the body is read from the work's blocks each time its page is read: whether it was read yet,
  // and why it could not be.
  const [bodyRead, setBodyRead] = useState(mocksShown());
  const [bodyFailed, setBodyFailed] = useState<string | null>(null);
  // Without the mocks, the names of the questions of the work's own in the page the body was last read from, or could
  // not be: the questions the window draws, a question asked since waiting for the texts that may point to it, so
  // that no card stands after the body and then moves into a text that points to it.
  const [readWith, setReadWith] = useState<ReadonlySet<string>>(new Set());
  // Without the mocks, what the owner may rule of the preconditions the work waits for, read with the body.
  const [rulings, setRulings] = useState<Ruling[]>([]);
  // What the owner said from this window, which stands under the messages the body was read with until a read begun
  // after it was said brings it: the engine wrote the words as it took them, so a later read holds them, while the
  // body of a read already under way cannot, and what the owner said must not go out of view.
  const [said, setSaid] = useState<Message[]>([]);
  useEffect(() => {
    if (mocksShown() || read === null) return;
    let alive = true;
    const begun = Date.now();
    const failed = (e: unknown) => alive && setBodyFailed(e instanceof Error ? e.message : String(e));
    const names = new Set((read.questions ?? []).map((q) => q.name));
    readBody(read, loadVersion).then(
      (b) => {
        if (!alive) return;
        setBody(b);
        setSaid((was) => was.filter((m) => m.at >= begun));
        setBodyRead(true);
        setBodyFailed(null);
        setReadWith(names);
      },
      (e: unknown) => {
        failed(e);
        if (alive) setReadWith(names);
      },
    );
    readRulings(read, waitsOf(read.summary), loadVersion).then((r) => alive && setRulings(r), failed);
    return () => {
      alive = false;
    };
  }, [read, loadVersion]);
  const [unseen, setUnseen] = useState(mock.unseen);
  const [ownTab, setOwnTab] = useKept(CHOICE_KEYS.consoleTab, 'item', WINDOW_TABS);
  const tab = chosenTab ?? ownTab;
  const setTab = (to: WindowTab) => {
    if (chosenTab === undefined) setOwnTab(to);
    onTab?.(to);
  };
  const [quote, setQuote] = useState<string | null>(null);
  const [offer, setOffer] = useState<{ x: number; y: number; text: string } | null>(null);
  // What the owner's last Confirm did, said under it.
  const [confirmed, setConfirmed] = useState<string | null>(null);
  // Whom the message being written goes to, where the owner chose; and how many times a question was taken to the
  // discussion, to bring the box into view once it is drawn for the cell chosen.
  const [messageTo, setMessageTo] = useState<string | null>(null);
  const [discussed, setDiscussed] = useState(0);
  const inner = useRef<HTMLDivElement>(null);
  const box = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    if (discussed === 0) return;
    box.current?.scrollIntoView({ block: 'center', behavior: 'smooth' });
    box.current?.focus({ preventScroll: true });
  }, [discussed]);
  // The card the address names is brought into view once it is drawn, once.
  const shownWait = useRef<string | undefined>(undefined);
  useEffect(() => {
    if (wait === undefined || shownWait.current === wait) return;
    const card = [...(inner.current?.querySelectorAll<HTMLElement>('[data-wait]') ?? [])].find((c) => c.dataset.wait === wait);
    if (!card) return;
    shownWait.current = wait;
    card.scrollIntoView({ block: 'center' });
  });
  // A question taken to the discussion: quoted into the box, which goes to the line of the cell that asked it, or for a
  // question of the plan to the line the work stands at. Nothing else changes: the work is not paused.
  const discuss = (c: Choice) => {
    setQuote(quoteOf(c));
    setMessageTo(read?.flow.cells.find((x) => x.id === c.asks?.cell)?.line ?? null);
    setDiscussed((n) => n + 1);
  };
  const jump = (at: number) =>
    document.getElementById(messageId(work.number, at))?.scrollIntoView({ block: 'center', behavior: 'smooth' });
  // The messages the window draws, in one list: those the body was read with, and under them what the owner said that
  // it does not hold yet.
  const messages = said.length === 0 ? body.messages : [...body.messages, ...said];
  const about = (c: Choice) => messagesAbout(messages, c);
  const markAsRead = useCallback(() => {
    const clear = (ps: Part[]) => ps.map((p) => ({ ...p, changed: false }));
    markRead(item.number);
    setUnseen(false);
    setBody((was) => ({ ...was, sections: was.sections.map((s) => ({ ...s, parts: clear(s.parts) })) }));
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
  // The sections the flow's card names: as read, or, until the body is read, each with nothing in it yet.
  const sections = bodyRead ? body.sections : (read?.flow.card ?? []).map((title) => ({ title, parts: [] }));
  // What a section with nothing in it says.
  const nothingYet = bodyRead ? 'Nothing is written yet' : 'Reading';
  // The questions put to the owner: those of the work's own that stand, once the texts that may point to them are
  // read, or could not be, and those a cell asked as a call the work waits at; each not yet answered with the option
  // picked, or the words written to one with no options, by what its answer is sent to.
  const [answers, setAnswers] = useState<Record<string, number | null>>({});
  const [written, setWritten] = useState<Record<string, string>>({});
  const placed = (c: Choice) =>
    mocksShown() || (c.asks !== undefined && 'question' in c.asks && readWith.has(c.asks.question));
  const questions = [...questionsStanding(read?.questions, work).filter(placed), ...askedOf(work)].map((c) =>
    c.confirmed === true ? c : { ...c, chosen: answers[pickKey(c)] ?? null, answer: written[pickKey(c)] ?? '' },
  );
  // Those still to answer, which Confirm sends.
  const toAnswer = questions.filter((q) => q.confirmed !== true);
  // Whether a question has its answer: an option picked, or for one with no options, words written.
  const answered = (q: Choice) => (q.options.length === 0 ? (q.answer ?? '').trim() !== '' : q.chosen !== null);
  // The questions of the work's own by their names, which a pointer names; and where each a text points to is drawn:
  // at the first pointer to it in the passages of the card's sections and then in the messages an agent wrote, in the
  // order the window draws them.
  const named = new Map(
    questions.flatMap((q): [string, Choice][] =>
      q.asks !== undefined && 'question' in q.asks ? [[q.asks.question, q]] : [],
    ),
  );
  const held = heldAt(
    [
      ...sections.flatMap((s) =>
        s.parts.flatMap((p, i): Pointing[] => (p.kind === 'text' ? [{ key: partKey(s.title, i), text: p.text }] : [])),
      ),
      ...messages.flatMap((m, i): Pointing[] => (m.who === 'owner' ? [] : [{ key: messageKey(i), text: m.text }])),
    ],
    new Set(named.keys()),
  );
  // A press on an option of a question picks it, and a press on the one picked lets it go; words written to one with no
  // options are kept as they are typed.
  const pickQuestion = (q: Choice, option: number) => {
    const key = pickKey(q);
    setAnswers((was) => ({ ...was, [key]: was[key] === option ? null : option }));
  };
  const writeQuestion = (q: Choice, text: string) => setWritten((was) => ({ ...was, [pickKey(q)]: text }));
  // The cell a question belongs to, and the line that cell runs on, as the work's flow names it; none of a choice of
  // the plan, which belongs to no cell.
  const askedBy = (q: Choice) => {
    const asks = q.asks;
    if (asks === undefined) return undefined;
    const cell = read?.flow.cells.find((c) => c.id === asks.cell);
    return { cell: asks.cell, line: cell?.line ?? null };
  };
  // A question's card, wherever it stands.
  const card = (q: Choice) => (
    <ChoiceBox
      p={q}
      by={askedBy(q)}
      pick={q.confirmed === true ? undefined : (o) => pickQuestion(q, o)}
      write={q.confirmed === true ? undefined : (t) => writeQuestion(q, t)}
      discuss={() => discuss(q)}
      messages={about(q)}
      jump={jump}
      links={pointersOf(null)}
    />
  );
  // What the text of this key draws at its pointers: the card of each question it holds, and a link to any other
  // question that stands; a text of no key holds none.
  const pointersOf = (key: string | null): Pointers => ({
    hold: (name) => {
      const q = named.get(name);
      return key !== null && q !== undefined && held.get(name) === key ? card(q) : null;
    },
    link: (name) => (named.has(name) ? <QuestionLink name={name} show={() => showQuestion(name)} /> : null),
  });
  // Brings a question's card into view, by the key of the wait its answer ends, which it carries.
  const showQuestion = (name: string) =>
    [...(inner.current?.querySelectorAll<HTMLElement>('[data-wait]') ?? [])]
      .find((c) => c.dataset.wait === answerKey(name))
      ?.scrollIntoView({ block: 'center', behavior: 'smooth' });
  // The last section as drawn: its own parts, a choice of them open while its choices wait to be confirmed, then the
  // questions no text holds; with no section, those questions alone.
  const last = sections.at(-1);
  const own = last?.parts ?? [];
  const after = questions.filter(
    (q) => !(q.asks !== undefined && 'question' in q.asks && held.has(q.asks.question)),
  );
  const asked: Part[] = [
    ...(body.choicesWait ? own : own.map((p) => (p.kind === 'choice' ? { ...p, confirmed: true } : p))),
    ...after,
  ];
  // A press on an option of a part of the last section: of a question after its own parts, or of its own choice, by
  // its place.
  const choose = (at: number, option: number) => {
    const q = after[at - own.length];
    if (q !== undefined) {
      pickQuestion(q, option);
      return;
    }
    const pick = (p: Part, i: number): Part =>
      i === at && p.kind === 'choice' ? { ...p, chosen: p.chosen === option ? null : option } : p;
    setBody((was) => ({
      ...was,
      sections: was.sections.map((s, n) => (n === was.sections.length - 1 ? { ...s, parts: s.parts.map(pick) } : s)),
    }));
  };
  // Words written to a question after the last section's own parts, with no options.
  const write = (at: number, text: string) => {
    const q = after[at - own.length];
    if (q !== undefined) writeQuestion(q, text);
  };
  // One Confirm for the last section's choices and the questions: each question's answer is sent, one after another,
  // to what it answers: the option picked, counted from one, to the version of the work's question; the label of its
  // option or the words written to it, to the call that waits for it. Once all are taken, the section's picks stand.
  const answering = useSending(send);
  const ownPicks = body.choicesWait ? picks(body) : { picked: 0, all: 0 };
  const count = {
    picked: ownPicks.picked + toAnswer.filter(answered).length,
    all: ownPicks.all + toAnswer.length,
  };
  const confirm = async () => {
    for (const q of toAnswer) {
      if (q.asks === undefined || !answered(q)) continue;
      const taken = await answering.send(
        'call' in q.asks
          ? {
              kind: 'result',
              call: q.asks.call,
              result: q.chosen === null ? (q.answer ?? '').trim() : q.options[q.chosen].label,
            }
          : {
              kind: 'answer',
              question: q.asks.question,
              version: q.asks.version,
              option: (q.chosen ?? 0) + 1,
            },
      );
      if (!taken) return;
    }
    if (body.choicesWait) setBody((was) => ({ ...was, choicesWait: false }));
    setConfirmed(body.choicesWait ? 'You confirmed the plan.' : 'Your answer was sent.');
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
  // A message sent stands under the others, the owner's own, until a read of the work brings it.
  const sent = (message: Message) => {
    setSaid((was) => [...was, message]);
    setQuote(null);
  };
  return (
    <div className="cw-item" ref={inner}>
      <header className="drawer-head">
        <span className="drawer-id">{item.number}</span>
        <h2 className="drawer-title">
          {titleHref !== undefined ? (
            <a className="cw-title-link" href={titleHref} title="Open it in the console">
              {item.title}
            </a>
          ) : (
            <button type="button" className="cw-title-link" title="Show it in the tree" onClick={onTitle}>
              {item.title}
            </button>
          )}
        </h2>
        {item.project !== null && <span className="drawer-belongs mono">{item.project}</span>}
      </header>
      <ItemLine
        item={item}
        goal={goal}
        tier={mock.tier}
        level={<LevelMark level={work.level ?? null} raise={work.raise ?? 0} />}
      >
        <Progress w={work} page={read} />
      </ItemLine>
      <WorkLine w={work} page={read} />
      {!ended(work) && <Acts paused={work.paused === 'owner'} act={send} />}
      <StopCards work={work} act={send} actEngine={actEngine} />
      <div className="drawer-body">
        <div className="cw-tabs">
          <Tabs
            label="What the window shows"
            choices={WINDOW_TABS.map((value) => ({ value, name: TAB_NAMES[value] }))}
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
        {tab === 'step' ? (
          read === null ? (
            <Unread page={page} />
          ) : (
            <StepTab page={read} row={stepRow} sessions={sessions} reader={reader} shown={open} />
          )
        ) : tab === 'usage' ? (
          read === null ? <Unread page={page} /> : <UsageTab page={read} />
        ) : tab === 'work' ? (
          <WorkFacts w={work} page={read} verdicts={mock.verdicts} reader={reader} />
        ) : (
          <>
            <div className="cw-body" onMouseUp={selected}>
              {bodyFailed !== null && <p className="failed small">The page could not read the item: {bodyFailed}</p>}
              {sections.slice(0, -1).map((s) => (
                <Section
                  key={s.title}
                  title={s.title}
                  parts={s.parts}
                  empty={nothingYet}
                  pointers={(i) => pointersOf(partKey(s.title, i))}
                  links={pointersOf(null)}
                  askedBy={askedBy}
                />
              ))}
              <Section
                title={last?.title}
                parts={asked}
                keep={rulings.length > 0 || toAnswer.length > 0}
                empty={last === undefined ? undefined : nothingYet}
                choose={body.choicesWait || toAnswer.length > 0 ? choose : undefined}
                write={toAnswer.length > 0 ? write : undefined}
                discuss={discuss}
                about={about}
                jump={jump}
                pointers={(i) => pointersOf(last === undefined ? null : partKey(last.title, i))}
                links={pointersOf(null)}
                askedBy={askedBy}
                after={
                  <>
                    {body.choicesWait || toAnswer.length > 0 ? (
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
                      <RulingBox key={r.key} ruling={r} act={send} links={pointersOf(null)} />
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
              messages={messages}
              pointers={(i) => pointersOf(messages[i]?.who === 'owner' ? null : messageKey(i))}
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

/// A pointer to a question drawn elsewhere in the window: the question's name, as the text wrote it, a link that
/// brings its card into view.
function QuestionLink({ name, show }: { name: string; show: () => void }) {
  return (
    <button type="button" className="link mono cw-pointer" title={SHOW_QUESTION} onClick={show}>
      {name}
    </button>
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

/// The item's line: where it stands, and when it was added; at its right, what follows it (its work's progress). The
/// items it waits for are said once, on its work's line.
function ItemLine({
  item,
  goal,
  tier,
  level,
  children,
}: {
  item: Item;
  goal: boolean;
  tier: string | null;
  /// The level its work's cell runs at, as the card writes it under its number, after the item's standing.
  level: ReactNode;
  children?: ReactNode;
}) {
  const standing = standingOf(item);
  return (
    <Line tone={standing} word={STANDING_WORDS[standing]}>
      {level}
      {goal ? <span className="muted">Goal</span> : null}
      {tier !== null ? <span className="muted">Tier {tier}</span> : null}
      <span className="muted">Added {ago(item.added)} ago</span>
      {children}
    </Line>
  );
}

/// How far the work has come and where its time went, together at the right end of the item's line, which its own
/// line, holding the cell's title, has no room left for: the cell it stands at, by its id, before the strip.
function Progress({ w, page }: { w: Summary; page: WorkPage | null }) {
  const { cell } = stateOf(w);
  return (
    <span className="cw-progress">
      <span className="cw-at">
        {cell !== null && <span className="mono muted">{cell}</span>}
        <Strip cells={w.cells} stopped={stoppedAt(w)} />
      </span>
      <span className="cw-track">
        <TimeTrack spans={barSpans(w.bar ?? (page === null ? '' : barOf(page)))} />
      </span>
    </span>
  );
}

/// The work's state line, one line, the same part for every state: its dot and the word of its state, the calls of
/// tools it waits for with a link to where each runs, and how long it has waited for them; the cell it stands at where
/// the engine's state names one, as a cell is shown everywhere; where it waits for the items its item depends on,
/// which, as the engine gives them, those cancelled said so; the work; its pull request, where it has one, a link to it
/// on the host of its repository; and at its right end, where the work changed lines, those it added and removed, as
/// its card says them.
function WorkLine({ w, page }: { w: Summary; page: WorkPage | null }) {
  const { tone, cell } = stateOf(w);
  const now = w.now;
  const items = now.kind === 'waits' ? now.on.filter((x) => x.kind === 'items') : [];
  const lines = linesOf(w);
  return (
    <Line tone={tone} word={<Parts parts={stateParts(w)} />}>
      {tone === 'calling' && <CallsTook work={w} />}
      {cell !== null && <CellName cell={cell} page={page} />}
      {items.map((x, i) => (
        <span key={i} className="muted">
          {waitedWords(x)}
        </span>
      ))}
      <a href={`#/works/${w.number}`} className="link mono" title={`Open work ${w.number}`}>
        W{w.number}
      </a>
      {w.pull != null && (
        <a
          href={w.pull.url}
          className="link mono"
          target="_blank"
          rel="noreferrer"
          title={`Open pull request ${w.pull.number} on the host of the work's repository`}
        >
          PR #{w.pull.number}
        </a>
      )}
      {lines !== null && (
        <span className="cw-lines">
          <LinesChanged added={lines.added} removed={lines.removed} files={lines.files} />
        </span>
      )}
    </Line>
  );
}

/// What the owner does to a work that goes on: start it or pause it, with one switch, named the owner's own pause, whose
/// pressed choice says whether that pause holds it, the engine's being let on from its card alone; cancel it, once they
/// confirm and say why. While one is under way, none takes effect.
function Acts({ paused, act }: { paused: boolean; act: (what: Act) => Promise<unknown> }) {
  const { waiting, refused, send } = useSending(act);
  const [asking, setAsking] = useState(false);
  const [why, setWhy] = useState('');
  return (
    <div className="acts cw-acts">
      {/* Going on and paused exclude each other, so they are one switch that shows which the work is in: pressing the
          other choice starts or pauses it. */}
      <Switch<'on' | 'paused'>
        label={OWN_PAUSE.label}
        choices={[
          { value: 'on', ...OWN_PAUSE.on },
          { value: 'paused', ...OWN_PAUSE.paused },
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
          ready={why.trim() !== ''}
          onAnswer={(yes) => {
            setAsking(false);
            if (yes) void send({ kind: 'cancel', why: why.trim() });
          }}
        >
          <p>{CANCEL_DOES}.</p>
          <textarea
            rows={2}
            aria-label="Why it is cancelled"
            placeholder={CANCEL_WHY}
            value={why}
            onChange={(e) => setWhy(e.target.value)}
          />
        </Confirm>
      )}
      {refused !== null && <span className="failed small">{refused}</span>}
    </div>
  );
}

/// The work's stops that are not drawn in its body, each a card under the state line in the look of every stop: what
/// the engine's list says it waits on the owner for, one sentence of why and the button that ends it, each saying on
/// pointing at it what it does; and a fault or a stall, which no one waits on, with what is wrong.
function StopCards({ work, act, actEngine }: {
  work: Summary;
  act: (what: Act) => Promise<unknown>;
  actEngine?: (what: EngineAct) => Promise<unknown>;
}) {
  const [waiting, setWaiting] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  const send = (w: WaitAct) => {
    if (waiting) return;
    setWaiting(true);
    setRefused(null);
    const sent = w.on === 'work' ? act(w.act) : (actEngine?.(w.act) ?? Promise.reject(new Error(NO_ENGINE)));
    sent.then(
      () => setWaiting(false),
      (e: unknown) => {
        setWaiting(false);
        setRefused(refusal(e));
      },
    );
  };
  const now = work.now;
  const halted = now.kind === 'fault' ? now.words : now.kind === 'stalled' ? now.why : null;
  const waits = waitsOf(work).filter((w) => !drawnInBody(w));
  if (waits.length === 0 && halted === null) return null;
  return (
    <div className="cw-waits">
      {halted !== null && <StopCard why={halted} />}
      {waits.map((w) => (
        <StopCard key={w.key} wait={w.key} why={w.why}>
          <WaitButton wait={w} waiting={waiting} send={send} />
        </StopCard>
      ))}
      {refused !== null && <p className="failed small">{refused}</p>}
    </div>
  );
}

/// A stop of a work, the one look of every stop: on the red tint, framed in red; its reason as the engine gives it, its
/// marked names and values in the monospace and the rest plain text, then what stands under it and the buttons that
/// end it, inside the card.
function StopCard({ wait, why, children }: { wait?: string; why: string; children?: ReactNode }) {
  return (
    <div className="cw-part">
      <div className="cw-choice cw-choice-holds" data-wait={wait}>
        <div className="cw-choice-q">
          <p>
            <Coded text={why} />
          </p>
        </div>
        {children}
      </div>
    </div>
  );
}

/// The button that ends a wait, inside its card, saying on pointing at it what it does; for what no control of the
/// page does, what the owner does, in the engine's words.
function WaitButton({ wait, waiting, send }: { wait: Wait; waiting: boolean; send: (w: WaitAct) => void }) {
  const ends = wait.ends;
  if (ends.kind === 'outside') return <p className="cw-stop-do">{ends.what}</p>;
  const todo = actOf(ends);
  if (todo === null || !(ends.kind in ENDS_WORDS)) return null;
  const words = ENDS_WORDS[ends.kind as keyof typeof ENDS_WORDS];
  return (
    <div className="cw-stop-acts">
      <Button primary title={words.title} disabled={waiting} onClick={() => send(todo)}>
        {words.label}
      </Button>
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
  pointers,
  links,
  askedBy,
}: {
  /// Its name, over it; with none, its parts are drawn with no fold.
  title?: string;
  parts: Part[];
  /// Draws the section with no parts, for what stands under them.
  keep?: boolean;
  /// Picks an option of a choice, while its choices wait to be confirmed.
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
  /// What a passage of it, by its place, draws at its pointers to the questions.
  pointers?: (at: number) => Pointers;
  /// What a choice's words draw at theirs: links alone.
  links?: Pointers;
  /// The cell a choice belongs to, and the line it is on; none where it belongs to no cell.
  askedBy?: (c: Choice) => { cell: string; line: string | null } | undefined;
}) {
  if (parts.length === 0 && !keep && empty === undefined) return null;
  const drawn = (
    <>
      {parts.length === 0 && empty !== undefined ? <p className="muted">{empty}</p> : null}
      {parts.map((p, i) => (
        <div key={i} className={p.changed === true ? 'cw-part cw-changed' : 'cw-part'}>
          {p.kind === 'text' ? (
            <Markdown text={p.text} pointers={pointers?.(i)} />
          ) : (
            <ChoiceBox
              p={p}
              by={askedBy?.(p)}
              links={links}
              pick={choose === undefined || p.confirmed === true ? undefined : (o) => choose(i, o)}
              write={write === undefined || p.confirmed === true ? undefined : (t) => write(i, t)}
              discuss={discuss === undefined ? undefined : () => discuss(p)}
              messages={about?.(p) ?? []}
              jump={jump}
            />
          )}
        </div>
      ))}
      {parts.length > 0 || keep ? after : null}
    </>
  );
  return title === undefined ? drawn : <Fold title={title}>{drawn}</Fold>;
}

/// The options of a card that offers them, drawn as choices and told apart by shape from what stands above them: under
/// a hairline, each a grey frame with a radio mark at its left, the one picked framed in the accent with its mark
/// filled; the one recommended tagged.
function Options({ options, chosen, recommended = null, pick, code = false }: {
  options: { label: string; detail: string }[];
  chosen: number | null;
  recommended?: number | null;
  pick?: (option: number) => void;
  /// Whether each label is a key and its value, set in the monospace as the files write it.
  code?: boolean;
}) {
  return (
    <div className="cw-options" role="radiogroup">
      {options.map((o, i) => (
        <button
          key={o.label}
          type="button"
          role="radio"
          disabled={pick === undefined}
          aria-checked={chosen === i}
          aria-pressed={chosen === i}
          className={chosen === i ? 'cw-option on' : 'cw-option'}
          onClick={() => pick?.(i)}
        >
          <span className="cw-radio" aria-hidden="true" />
          <span className="cw-option-text">
            <span className="cw-option-label">
              {code ? <code>{o.label}</code> : o.label}
              {recommended === i ? <span className="tag cw-tag">Recommended</span> : null}
            </span>
            {o.detail !== '' && <span className="cw-option-detail">{o.detail}</span>}
          </span>
        </button>
      ))}
    </div>
  );
}

/// The options of a question answered, which is settled: under the same hairline, each its words alone, with nothing
/// that can be pressed; the one taken in the text's colour, marked at its left by a drawn tick that says on pointing at
/// it that it was taken, the others muted; the one recommended tagged.
function Settled({ options, chosen, recommended }: {
  options: { label: string; detail: string }[];
  chosen: number | null;
  recommended: number | null;
}) {
  return (
    <ul className="cw-options">
      {options.map((o, i) => (
        <li key={o.label} className={chosen === i ? 'cw-settled cw-settled-taken' : 'cw-settled'}>
          {chosen === i ? <TickSign /> : <span className="cw-tick" aria-hidden="true" />}
          <span className="cw-option-text">
            <span className="cw-option-label">
              {o.label}
              {recommended === i ? <span className="tag cw-tag">Recommended</span> : null}
            </span>
            {o.detail !== '' && <span className="cw-option-detail">{o.detail}</span>}
          </span>
        </li>
      ))}
    </ul>
  );
}

/// Whether the cell the owner's words went to has taken them: a clock until a run of it begins with them, then the
/// page's tick; what it means said on pointing at it.
function TakenMark({ taken, to }: { taken: boolean; to: string }) {
  const words = taken ? `Taken by ${to}` : `Not taken yet: ${to} takes it as it next runs`;
  return (
    <svg
      className={taken ? 'cw-taken' : 'cw-waiting'}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2.5"
      strokeLinecap="round"
      role="img"
      aria-label={words}
    >
      <title>{words}</title>
      {taken ? (
        <path d={TICK} />
      ) : (
        <>
          <circle cx="12" cy="12" r="8" />
          <path d="M12 7.5 V12 L15 14" />
        </>
      )}
    </svg>
  );
}

/// The tick that marks the option taken, the page's tick (`TICK`), as wide as an option's radio mark.
function TickSign() {
  return (
    <svg
      className="cw-tick"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="3"
      role="img"
      aria-label={TAKEN}
    >
      <title>{TAKEN}</title>
      <path d={TICK} />
    </svg>
  );
}

/// A choice of the plan: a press on an option picks it, on another changes the pick, and on the one picked lets it go,
/// until it is confirmed; once confirmed, it is settled, and its options are words with the one taken marked.
/// Discuss takes it to the discussion, confirmed or not; under its options, how many messages are about it, which
/// brings the first into view. A question with no options is answered in words, in a box in their place. A question
/// the engine's list names holds the work, at its cell or at the merge: on the red tint, framed in red, the look of
/// every stop.
function ChoiceBox({ p, by, pick, write, discuss, messages, jump, links }: {
  p: Choice;
  /// The cell it belongs to and the line that cell is on; none of a choice of the plan, which belongs to no cell.
  by?: { cell: string; line: string | null };
  pick?: (option: number) => void;
  write?: (text: string) => void;
  discuss?: () => void;
  messages: number[];
  jump?: (at: number) => void;
  /// What its words draw at a pointer to another question: a link.
  links?: Pointers;
}) {
  return (
    <fieldset
      className={`cw-choice${pick === undefined ? ' cw-choice-done' : ''}${p.holds === true ? ' cw-choice-holds' : ''}`}
      data-wait={waitKeyOf(p)}
    >
      <Asked by={by} />
      <div className="cw-choice-q">
        <Markdown text={p.question} pointers={links} />
      </div>
      {p.options.length === 0 && p.asks !== undefined && p.confirmed !== true ? (
        <textarea
          rows={3}
          value={p.answer ?? ''}
          aria-label="Your answer"
          placeholder="Write your answer"
          disabled={write === undefined}
          onChange={(e) => write?.(e.target.value)}
        />
      ) : null}
      {p.options.length > 0 &&
        (p.confirmed === true ? (
          <Settled options={p.options} chosen={p.chosen} recommended={p.recommended} />
        ) : (
          <Options options={p.options} chosen={p.chosen} recommended={p.recommended} pick={pick} />
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
    </fieldset>
  );
}

/// Whose question it is, on the frame of its card: the cell it belongs to, by its id, and the line that cell runs on
/// after it. A card that belongs to no cell draws none, and its frame is unbroken.
function Asked({ by }: { by?: { cell: string; line: string | null } }) {
  if (by === undefined) return null;
  return (
    <legend className="cw-choice-by">
      <span className="mono">{by.cell}</span>
      {by.line !== null && <span className="muted"> {by.line}</span>}
    </legend>
  );
}

/// A block the owner may write that keeps a precondition the work waits for from holding, as the engine's list names
/// it, drawn as a question that holds the work is: why, in the engine's words, and what the block's latest version
/// says; its values but the one it has now as the options, the one that makes the precondition hold picked first; and
/// inside the card the button that writes the one picked. To have the cell judge again, the owner writes to it in the
/// discussion.
function RulingBox({ ruling, act, links }: {
  ruling: Ruling;
  act: (what: Act) => Promise<unknown>;
  /// What its words draw at a pointer to a question: a link.
  links: Pointers;
}) {
  const { waiting, refused, send } = useSending(act);
  const options = ruling.values.filter((v) => v !== ruling.now);
  const first = ruling.holding === null ? -1 : options.indexOf(ruling.holding);
  const [chosen, setChosen] = useState<number | null>(first < 0 ? null : first);
  const value = chosen === null ? null : options[chosen];
  return (
    <>
      <div className="cw-part">
        <div className="cw-choice cw-choice-holds" data-wait={ruling.key}>
          <div className="cw-choice-q">
            <p>
              <Coded text={ruling.why} />
            </p>
            {ruling.text.trim() !== '' && <Markdown text={ruling.text} pointers={links} />}
          </div>
          {options.length > 0 && (
            <Options
              options={options.map((v) => ({ label: `${ruling.block} = ${v}`, detail: '' }))}
              chosen={chosen}
              pick={(o) => setChosen((was) => (was === o ? null : o))}
              code
            />
          )}
          {options.length > 0 && (
            <div className="cw-stop-acts">
              <Button
                primary
                title={value === null ? WRITE_WORDS.none : WRITE_WORDS.writes(ruling.block, value)}
                disabled={waiting || value === null}
                onClick={() => {
                  if (value !== null) void send({ kind: 'write', block: ruling.block, content: '', value });
                }}
              >
                {WRITE_WORDS.label}
              </Button>
            </div>
          )}
        </div>
      </div>
      {refused !== null && <p className="failed small">{refused}</p>}
    </>
  );
}

/// The discussion: the messages written before, the owner's and the replies, and a box to write one. The owner picks
/// the line it goes to, of the lines one of whose cells has words of its own, and what they send is said to that line;
/// the engine chooses the cell it reaches (`speaking`). Each button names its line and, where the engine has a cell to
/// give the words to on it, that cell by its id; a line on which it has none is grey, cannot be picked, and says why on
/// pointing at it, in the engine's words. Of the lines that take the words, the one picked at first is the line the
/// work stands at, else the one it reached last; where none takes them, none is picked, no box is drawn, and why is
/// said once, under the lines. The line under the box names the cell the words reach by its title.
function Discussion({ work, page, messages, pointers, messageTo, setMessageTo, quote, unquote, box, act, sent }: {
  work: Summary;
  page: PageState;
  messages: Message[];
  /// What a message, by its place, draws at its pointers to the questions.
  pointers: (at: number) => Pointers;
  /// The line the owner chose the message goes to; none where they did not.
  messageTo: string | null;
  setMessageTo: (line: string) => void;
  quote: string | null;
  unquote: () => void;
  box: RefObject<HTMLTextAreaElement | null>;
  act: (what: Act) => Promise<unknown>;
  sent: (message: Message) => void;
}) {
  const read = pageIn(page);
  const flow = read?.flow;
  const lineOf = (cell: string | null) => flow?.cells.find((c) => c.id === cell)?.line ?? null;
  // The lines one of whose cells has words of its own, each with what the engine says of words said to it: the cell
  // they reach, or why no cell can take them.
  const targets = (flow?.lines ?? [])
    .filter((l) => flow !== undefined && flow.cells.some((c) => c.line === l.name && wordsOf(flow, c.id) !== undefined))
    .map((l) => {
      const to: SpokenTo = { kind: 'line', line: l.name };
      return { line: l.name, to, speaking: read?.speaking?.find((s) => sameTo(s.to, to)) };
    });
  // Why the words said to a line reach no cell, where they reach none.
  const refusedOn = (t: (typeof targets)[number]): string | null =>
    t.speaking === undefined
      ? 'the engine does not say which cell the words would reach'
      : t.speaking.cell === null
        ? (t.speaking.refused ?? 'the engine does not say why')
        : null;
  const taking = targets.filter((t) => refusedOn(t) === null);
  // The lines the work reached, the one it reached last first: the lines of the cells of its runs, latest first.
  const reached = (read?.steps ?? [])
    .flatMap((s) => (s.kind === 'run' ? [lineOf(s.cell)] : []))
    .reverse();
  // The one the owner chose, else the line the work stands at, else the one it reached last, of those taking words.
  const target =
    taking.find((t) => t.line === messageTo) ??
    taking.find((t) => t.line === lineOf(stateOf(work).cell)) ??
    reached.map((line) => taking.find((t) => t.line === line)).find((t) => t !== undefined) ??
    taking[0];
  const reaches = target?.speaking?.cell ?? null;
  // Why no line takes the words, where none does: each reason the engine gives, once.
  const none = [...new Set(targets.map((t) => refusedOn(t) ?? ''))].filter((why) => why !== '').join('; ');
  return (
    <Fold title="Discussion" count={messages.length}>
      {messages.map((n, i) => (
        <div key={i} id={messageId(work.number, i)} className={n.who === 'owner' ? 'cw-message cw-message-owner' : 'cw-message'}>
          <div className="status-line">
            <span className="status-word">{writerOf(n.who, read)}</span>
            <span className="muted">to {n.to.map((x) => writerOf(x, read)).join(', ')}</span>
            <span className="mono muted">{ago(n.at)} ago</span>
            {n.taken !== undefined && <TakenMark taken={n.taken} to={n.to.map((x) => writerOf(x, read)).join(', ')} />}
          </div>
          {n.quote !== null ? <blockquote className="cw-quote">{n.quote}</blockquote> : null}
          <Markdown text={n.text} pointers={pointers(i)} />
        </div>
      ))}
      {page === null ? (
        <p className="muted">Reading the work's flow</p>
      ) : 'failed' in page ? (
        <p className="failed small">The page could not read the work: {page.failed}</p>
      ) : targets.length === 0 ? (
        <p className="muted">No cell of this flow takes the owner's words</p>
      ) : (
        <div className="write-box cw-compose">
          <div className="cw-to switch" role="radiogroup" aria-label="Whom the message goes to">
            {targets.map((t) => {
              const refused = refusedOn(t);
              const chosen = t.line === target?.line;
              return (
                <button
                  key={t.line}
                  type="button"
                  role="radio"
                  className="switch-choice"
                  aria-checked={chosen}
                  aria-pressed={chosen}
                  disabled={refused !== null}
                  title={refused === null ? undefined : `No cell takes the words: ${refused}`}
                  onClick={() => setMessageTo(t.line)}
                >
                  {t.speaking?.cell == null ? t.line : `${t.line} · ${t.speaking.cell}`}
                </button>
              );
            })}
          </div>
          {target !== undefined && reaches !== null ? (
            <Compose
              key={draftKey(work.number, `${target.line}_spoken_to`)}
              work={work}
              draft={`${target.line}_spoken_to`}
              line={target.line}
              to={target.to}
              reaches={reaches}
              title={(cell) => titleOf(read, cell)}
              quote={quote}
              unquote={unquote}
              box={box}
              act={act}
              sent={sent}
            />
          ) : (
            <p className="muted small">No cell takes the words: {none}</p>
          )}
        </div>
      )}
    </Fold>
  );
}

/// Whether two say the words to the same line or to the same cell.
const sameTo = (a: SpokenTo, b: SpokenTo): boolean =>
  a.kind === 'line' ? b.kind === 'line' && a.line === b.line : b.kind === 'cell' && a.cell === b.cell;

/// The box for what the owner says to one cell of one work, its draft kept as it is typed under the name `draft`, with
/// the passage quoted into it over it, and the names one can write to offered when `@` is typed.
function Compose({ work, draft, line, to, reaches, title, quote, unquote, box, act, sent }: {
  work: Summary;
  draft: string;
  /// The line the message goes to.
  line: string;
  /// What the message is said to.
  to: SpokenTo;
  /// The cell it reaches, as the engine chooses it.
  reaches: MessageCell;
  title: (cell: string) => string;
  quote: string | null;
  unquote: () => void;
  box: RefObject<HTMLTextAreaElement | null>;
  act: (what: Act) => Promise<unknown>;
  sent: (message: Message) => void;
}) {
  const key = draftKey(work.number, draft);
  const [text, setText] = useState(() => readDraft(key));
  const { waiting, refused, send } = useSending(act);
  const working = !ended(work);
  const type = (t: string) => {
    setText(t);
    writeDraft(key, t);
  };
  // Typing `@` offers the names one can write to: the line the message goes to, only while a work goes on, then the
  // architect and the assistant.
  const naming = /(^|\s)@(\w*)$/.exec(text);
  const others: Who[] = ['architect', 'assistant'];
  const offered = [...(working ? [line] : []), ...others].filter((x) =>
    x.startsWith((naming?.[2] ?? '').toLowerCase()),
  );
  const name = (x: string) => {
    type(text.replace(/@(\w*)$/, `@${x} `));
    box.current?.focus();
  };
  const submit = () => {
    const words = text.trim();
    const content = quote === null ? words : `${quote.replace(/^/gm, '> ')}\n\n${words}`;
    void send({ kind: 'say', to, words: content }).then((done) => {
      if (done === null) return;
      const reached = (done.answer as Said | null)?.reached ?? null;
      writeDraft(key, '');
      setText((now) => (now.trim() === words ? '' : now));
      // The cell the engine gave the words to, as it answered the act, has not taken them yet.
      sent({ who: 'owner', at: Date.now(), to: reached === null ? [] : [reached], quote, text: words, taken: false });
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
        aria-label={`What to say to ${title(reaches)}`}
        placeholder="Write a message"
        onChange={(e) => type(e.target.value)}
      />
      {naming !== null && offered.length > 0 ? (
        <div className="cw-names">
          {offered.map((x) => (
            <button key={x} type="button" className="button" onClick={() => name(x)}>
              {Object.hasOwn(WRITER, x) ? `@${x} · ${WRITER[x]}` : `@${x}`}
            </button>
          ))}
        </div>
      ) : null}
      <div className="write-line">
        {/* Only the cell the engine chooses: a name written with `@` is the owner's words, and reaches no one of itself. */}
        <span className="muted small write-readers">Goes to {title(reaches)}</span>
        <Button primary disabled={waiting || text.trim() === ''} onClick={submit}>
          Send
        </Button>
      </div>
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

/// What the work changed, file by file, as git in its project's repository says: each file's path, its lines added and
/// removed or that it is binary, and that it is not committed where only the work's worktree holds its change; a press
/// on a file's path opens its diff, drawn as a diff in a passage is. Read while the tab shows it, and again each time
/// the work's count of what it changed changes, what was read standing meanwhile; nothing where the work has nothing
/// to tell, as before its branch is made.
function Changes({ w, reader }: { w: Summary; reader: Reader }) {
  const changed = w.changed ?? null;
  const of = changed === null ? null : `${changed.files} ${changed.added} ${changed.removed}`;
  const [read, setRead] = useState<{ of: string; got: Got<ChangesPage> } | null>(null);
  useEffect(() => {
    if (of === null) return;
    let alive = true;
    reader.changes().then(
      (value) => alive && setRead({ of, got: { value } }),
      (e: unknown) => alive && setRead({ of, got: { failed: refusal(e) } }),
    );
    return () => {
      alive = false;
    };
  }, [reader, of]);
  if (changed === null) return null;
  return (
    <Fold title="Files changed" count={changed.files}>
      <Loaded
        got={read?.got ?? null}
        show={(page) =>
          page.files.length === 0 ? (
            <p className="muted">None</p>
          ) : (
            <ul>
              {page.files.map((f) => (
                <FileRow key={f.path} file={f} reader={reader} of={read?.of} />
              ))}
            </ul>
          )
        }
      />
    </Fold>
  );
}

/// A file the work changed: its path, which opens its diff at a press and lets it go at another, the diff read when it
/// is first opened and again once the work's count of what it changed changes (`of`); its lines added and removed, or
/// that it is binary; and that it is not committed, where only the worktree holds its change.
function FileRow({ file, reader, of }: { file: FileChange; reader: Reader; of: string | undefined }) {
  const [open, setOpen] = useState(false);
  const got = useWanted(open, () => reader.diff(file.path), of);
  return (
    <ListRow>
      <button
        type="button"
        className="mono cw-file"
        aria-expanded={open}
        title={open ? 'Hide its diff' : 'Show its diff'}
        onClick={() => setOpen(!open)}
      >
        {file.path}
      </button>
      {file.binary ? <span className="muted">binary</span> : <LinesChanged added={file.added} removed={file.removed} />}
      {!file.committed && <span className="muted">not committed</span>}
      {open && (
        <div className="opener">
          <Loaded got={got} show={(d) => <FileDiff page={d} />} />
        </div>
      )}
    </ListRow>
  );
}

/// The diff of a file, drawn as a diff in a passage is, and where it was cut, how long it is whole.
function FileDiff({ page }: { page: DiffPage }) {
  return (
    <>
      <Diff text={page.diff} path={page.path} />
      {page.cut !== null && <p className="muted small">Cut short of its whole {count(page.cut)} bytes</p>}
    </>
  );
}

/// What of the work the lines do not say, and what the work changed.
function WorkFacts({ w, page, verdicts, reader }: {
  w: Summary;
  page: WorkPage | null;
  verdicts: [string, string][];
  reader: Reader;
}) {
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
      <Changes w={w} reader={reader} />
    </>
  );
}
