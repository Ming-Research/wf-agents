// What the window beside the console shows of the item selected: its number, its title, which brings the item into
// view in the tree, and its project; a line of where the item stands and a line of what its work does, with the works
// page's strip of the work's cells and where its time went; what the owner does to the work (let it on, pause it,
// cancel it); two tabs; and under them, while something in the item is new to the owner, a strip that says what
// changed, with a link to mark the item read. The first tab holds what the work waits at, and the body (where the item
// came from, the problem it solves, the plan, with each choice that waits for the owner where it stands in the plan,
// picked by a press on an option and let go by another, and at the plan's end a button as wide as the body to confirm
// it, which counts the picks and waits until each choice has one; once confirmed, the picks stand), each passage
// changed since the owner read it marked at its left, and under the body the notes: the owner's and the replies, and a
// box to write one into a block of the work that people write, chosen above it, which a passage selected in the body
// can be quoted into and which names the cells that read it. The second holds what of the work the lines do not say,
// and the legend of the strip's marks. While the window is out, R marks the item read, as its link does, unless a box
// has the keys. What the owner sends is sent once: its button cannot be pressed while it is under way, and a refusal is
// said under the item it was for, and only there.
//
// Brought over from the console of the engine on `main` (`client/src/console/ItemWindow.tsx`), its layout as it was,
// its parts the page's own (parts.tsx) where it used Primer's. What the engine does not give yet comes from the
// console's made-up data (`mock.ts`), which says what of it is made up. What the new console added is kept in that
// layout: the work line's facts, the row of what the owner does to the work under the lines, what the work waits at
// over the body, the block a note is written in over its box, and the flow's name among the work's facts.

import { type MouseEvent, type ReactNode, type RefObject, useCallback, useEffect, useRef, useState } from 'react';

import { Button, Confirm, PanelPart } from '../parts';
import type { Act, Item, Summary, Waited, WorkPage } from '../works-api';
import { StateLegend, Strip } from '../works/marks';
import { type ItemStanding, standingOf } from './items';
import { Markdown } from './markdown';
import { bodyMock, itemMock, markRead, workTierMock } from './mock';
import { age, dollars, type Mood, moodOf } from './snap';
import { barOf, barSpans, TimeTrack } from './track';
import { type Body, changedIn, named, type Note, type Part, picks, type Who } from './window';

const ago = (at: number) => age(Date.now() - at).label;

/// The words for who gives what a work waits for.
const WHO = { owner: 'you', architect: 'the architect' } as const;
const whom = (w: Waited): string => (w.kind === 'limit' ? WHO.owner : w.who === null ? 'the engine' : WHO[w.who]);

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

const capital = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

/// What the work waits for, one thing at a time, in words.
export function waitedWords(w: Waited, page: WorkPage | null, at: string): string {
  switch (w.kind) {
    case 'call':
      switch (w.what.kind) {
        case 'question':
          return `${capital(whom(w))} to answer: ${w.what.question}`;
        case 'tool':
          return `The engine to run ${w.what.name}`;
        case 'atLimit':
          return w.what.limit.of === 'returns'
            ? `${capital(whom(w))} to let the run go on after ${w.what.limit.sentBack} returns sent back`
            : `${capital(whom(w))} to let the run go on after ${w.what.limit.made} calls`;
      }
      return '';
    case 'precondition':
      return `${w.words}, which ${whom(w)} make${w.who === 'owner' ? '' : 's'} hold`;
    case 'limit':
      return `You to let ${titleOf(page, at)} run again after ${w.runs} runs in a row for what a cell after it wrote`;
  }
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
      const who = [...new Set(now.on.map(whom))];
      return { word: `Waiting for ${who.join(' and ')}`, facts: `at ${titleOf(page, now.at)}` };
    }
    case 'paused':
      return now.by === 'engine' ? { word: MOOD_WORDS.halted, facts: now.why } : { word: MOOD_WORDS.paused, facts: null };
    case 'fault':
      return { word: MOOD_WORDS.halted, facts: now.words };
    case 'merged':
      return { word: 'Merged', facts: null };
    case 'cancelled':
      return { word: 'Cancelled', facts: null };
  }
}

/// The tone of a dot: an item's standing, or what its work does.
type Tone = ItemStanding | Mood | 'ended';

/// A status line of the panel, as the look's (`StatusLine`), but with the five tones of a work, and room at its right
/// end for the strip of the work's cells.
function Line({ tone, children }: { tone: Tone; children: ReactNode }) {
  return (
    <div className="cw-line">
      <span className={`cw-dot dot-${tone}`} aria-hidden="true" />
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

/// The words for who writes a note or is named in one.
const WRITER: Record<Who, string> = {
  owner: 'You',
  engineer: 'Engineer',
  architect: 'Architect',
  assistant: 'Assistant',
};

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

function ItemView({ item, page, act, waitsFor, goal = false, open = true, onRead, onTitle }: ItemPanelProps) {
  const work = item.work;
  const read = pageIn(page);
  const send = (what: Act) => act(work.number, what);
  const mock = itemMock(item);
  const [body, setBody] = useState<Body>(() => bodyMock(item));
  const [unseen, setUnseen] = useState(mock.unseen);
  const [tab, setTab] = useState<'item' | 'work'>('item');
  const [quote, setQuote] = useState<string | null>(null);
  const [offer, setOffer] = useState<{ x: number; y: number; text: string } | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const inner = useRef<HTMLDivElement>(null);
  const box = useRef<HTMLTextAreaElement>(null);
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
  // A press on an option picks it, and a press on the one picked lets it go.
  const choose = (at: number, option: number) =>
    setBody((was) => ({
      ...was,
      plan: was.plan.map((p, i) =>
        i === at && p.kind === 'choice' ? { ...p, chosen: p.chosen === option ? null : option } : p,
      ),
    }));
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
  // A note sent stands under the others, the owner's own.
  const sent = (note: Note) => {
    setBody((was) => ({ ...was, notes: [...was.notes, note] }));
    setQuote(null);
  };
  const count = picks(body);
  return (
    <div className="cw-item" ref={inner}>
      <div className="cw-head">
        <span className="cw-id">{item.number}</span>
        <h2 className="cw-title">
          <button type="button" className="cw-title-link" title="Show it in the tree" onClick={onTitle}>
            {item.title}
          </button>
        </h2>
        <span className="cw-project">{mock.project}</span>
      </div>
      <ItemLine item={item} waitsFor={waitsFor} goal={goal} tier={mock.tier} />
      <WorkLine w={work} page={read} />
      {!ended(work) && <Acts paused={work.paused || work.now.kind === 'paused'} act={send} />}
      <div className="cw-tabs" role="tablist">
        {(['item', 'work'] as const).map((k) => (
          <button
            key={k}
            type="button"
            role="tab"
            aria-selected={tab === k}
            className={tab === k ? 'on' : ''}
            onClick={() => setTab(k)}
          >
            {k === 'item' ? 'Item' : 'Work'}
          </button>
        ))}
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
          <button type="button" className="cw-fresh-read" aria-keyshortcuts="r" onClick={markAsRead}>
            {keyed('Mark as read', 'r')}
          </button>
        </div>
      ) : null}
      {tab === 'work' ? (
        <WorkFacts w={work} page={read} />
      ) : (
        <>
          {work.now.kind === 'waits' && <Waits on={work.now.on} page={read} at={work.now.at} />}
          <div className="cw-body" onMouseUp={selected}>
            <Section title="Background" parts={body.background} />
            <Section title="Problem" parts={body.problem} />
            <Section title="Plan" parts={body.plan} choose={body.planWaits ? choose : undefined} />
            {body.planWaits ? (
              <div className="cw-confirm">
                <button
                  type="button"
                  className="button button-primary cw-confirm-button"
                  disabled={count.picked < count.all}
                  onClick={() => {
                    setBody((was) => ({ ...was, planWaits: false }));
                    setConfirmed(true);
                  }}
                >
                  {count.all > 0 ? `Confirm (${count.picked}/${count.all})` : 'Confirm'}
                </button>
              </div>
            ) : null}
            {confirmed ? <p className="cw-done">You confirmed the plan.</p> : null}
          </div>
          {offer !== null ? (
            <button
              type="button"
              className="cw-quote-offer"
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
          <Notes
            work={work}
            page={page}
            notes={body.notes}
            quote={quote}
            unquote={() => setQuote(null)}
            box={box}
            act={send}
            sent={sent}
          />
        </>
      )}
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
    <Line tone={standing}>
      <span className="cw-strong">{STANDING_WORDS[standing]}</span>
      {goal ? <span>Goal</span> : null}
      {tier !== null ? <span>Tier {tier}</span> : null}
      <span>Added {ago(item.added)} ago</span>
      {waitsFor.length > 0 && <span>Waits for {waitsFor.join(', ')} to merge</span>}
    </Line>
  );
}

function WorkLine({ w, page }: { w: Summary; page: WorkPage | null }) {
  const { word, facts } = nowWords(w, page);
  return (
    <Line tone={ended(w) ? 'ended' : moodOf(w)}>
      {/* What it does in one phrase, then where. */}
      <span className="cw-strong">{word}</span>
      {facts !== null && <span>{facts}</span>}
      <a href={`#/works/${w.number}`} className="cw-mono" title={`Open work ${w.number}`}>
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

/// What the owner does to a work that goes on: let it on while it is paused, or pause it; cancel it, once they confirm.
/// While one is under way, none can be pressed.
function Acts({ paused, act }: { paused: boolean; act: (what: Act) => Promise<unknown> }) {
  const { waiting, refused, send } = useSending(act);
  const [asking, setAsking] = useState(false);
  return (
    <div className="cw-acts">
      {paused ? (
        <Button primary title="Let the work go on" disabled={waiting} onClick={() => void send({ kind: 'resume' })}>
          Let it on
        </Button>
      ) : (
        <Button
          title="No round of it begins; a round under way goes on"
          disabled={waiting}
          onClick={() => void send({ kind: 'pause' })}
        >
          Pause
        </Button>
      )}
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
      {refused !== null && <span className="cw-refused">{refused}</span>}
    </div>
  );
}

function Waits({ on, page, at }: { on: Waited[]; page: WorkPage | null; at: string }) {
  return (
    <PanelPart label={`Waits at ${titleOf(page, at)}`} count={on.length}>
      <ul>
        {on.map((x, i) => (
          <li key={i} className={x.kind === 'limit' || x.who === 'owner' ? 'cw-wait cw-wait-owner' : 'cw-wait'}>
            {waitedWords(x, page, at)}
          </li>
        ))}
      </ul>
    </PanelPart>
  );
}

/// Where a draft of a note is kept, so that it is not lost when the panel shows another item.
const draftKey = (work: number, block: string) => `wf-agents.console-note.${work}.${block}`;

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
  choose,
}: {
  title: string;
  parts: Part[];
  /// Picks an option of a choice, while the plan waits to be confirmed.
  choose?: (at: number, option: number) => void;
}) {
  if (parts.length === 0) return null;
  return (
    <section className="cw-section">
      <h3>{title}</h3>
      {parts.map((p, i) => (
        <div key={i} className={p.changed === true ? 'cw-part cw-changed' : 'cw-part'}>
          {p.kind === 'text' ? (
            <Markdown text={p.text} />
          ) : (
            <Choice p={p} pick={choose === undefined ? undefined : (o) => choose(i, o)} />
          )}
        </div>
      ))}
    </section>
  );
}

/// A choice of the plan: a press on an option picks it, on another changes the pick, and on the one picked lets it go,
/// until the plan is confirmed.
function Choice({ p, pick }: { p: Extract<Part, { kind: 'choice' }>; pick?: (option: number) => void }) {
  return (
    <div className={pick === undefined ? 'cw-choice cw-choice-done' : 'cw-choice'}>
      <p className="cw-choice-q">{p.question}</p>
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
            {p.recommended === i ? <span className="cw-tag">Recommended</span> : null}
          </span>
          <span className="cw-option-detail">{o.detail}</span>
        </button>
      ))}
    </div>
  );
}

/// The notes: those written before, the owner's and the replies, and a box to write one. The owner picks a block of
/// the work that people write, of those that hold words rather than a choice, and what they send is written into it;
/// the line under the box names the cells that read it, and those the note names with `@`.
function Notes({ work, page, notes, quote, unquote, box, act, sent }: {
  work: Summary;
  page: PageState;
  notes: Note[];
  quote: string | null;
  unquote: () => void;
  box: RefObject<HTMLTextAreaElement | null>;
  act: (what: Act) => Promise<unknown>;
  sent: (note: Note) => void;
}) {
  const read = pageIn(page);
  const blocks = (read?.flow.blocks ?? []).filter((b) => b.writtenBy.people && b.values === null);
  const [chosen, setChosen] = useState<string | null>(null);
  const block = blocks.find((b) => b.name === chosen) ?? blocks[0];
  return (
    <section className="cw-notes">
      <h3>Notes</h3>
      {notes.map((n, i) => (
        <div key={i} className={n.who === 'owner' ? 'cw-note cw-note-owner' : 'cw-note'}>
          <div className="cw-note-head">
            <span className="cw-strong">{WRITER[n.who]}</span>
            <span>to {n.to.map((x) => WRITER[x]).join(', ')}</span>
            <span className="cw-mono">{ago(n.at)} ago</span>
          </div>
          {n.quote !== null ? <blockquote className="cw-note-quote">{n.quote}</blockquote> : null}
          <Markdown text={n.text} />
        </div>
      ))}
      {page === null ? (
        <p className="cw-muted">Reading the work's flow</p>
      ) : 'failed' in page ? (
        <p className="cw-refused">The page could not read the work: {page.failed}</p>
      ) : block === undefined ? (
        <p className="cw-muted">People write no block of this flow</p>
      ) : (
        <div className="cw-compose">
          <label className="cw-block">
            <span className="label">Write in</span>
            <select className="cw-select" value={block.name} onChange={(e) => setChosen(e.target.value)}>
              {blocks.map((b) => (
                <option key={b.name} value={b.name}>
                  {b.name}
                </option>
              ))}
            </select>
          </label>
          <Compose
            key={draftKey(work.number, block.name)}
            work={work}
            block={block.name}
            readers={block.readBy.cells.map((c) => titleOf(read, c))}
            quote={quote}
            unquote={unquote}
            box={box}
            act={act}
            sent={sent}
          />
        </div>
      )}
    </section>
  );
}

/// The box for one block of one work, its draft kept as it is typed, with the passage quoted into it over it, and the
/// names one can write to offered when `@` is typed.
function Compose({ work, block, readers, quote, unquote, box, act, sent }: {
  work: Summary;
  block: string;
  readers: string[];
  quote: string | null;
  unquote: () => void;
  box: RefObject<HTMLTextAreaElement | null>;
  act: (what: Act) => Promise<unknown>;
  sent: (note: Note) => void;
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
      sent({ who: 'owner', at: Date.now(), to: at.length > 0 ? at : [working ? 'engineer' : 'assistant'], quote, text: words });
    });
  };
  return (
    <>
      {quote !== null ? (
        <div className="cw-quoted">
          <span>{quote}</span>
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
        placeholder="Write a note. Name others with @engineer, @architect or @assistant."
        onChange={(e) => type(e.target.value)}
      />
      {naming !== null && offered.length > 0 ? (
        <div className="cw-names">
          {offered.map((x) => (
            <button key={x} type="button" onClick={() => name(x)}>
              @{x} · {WRITER[x]}
            </button>
          ))}
        </div>
      ) : null}
      <div className="cw-compose-foot">
        <span className="cw-muted">{to.length > 0 ? `Goes to ${to.join(', ')}` : 'No cell reads it'}</span>
        <Button primary disabled={waiting || text.trim() === ''} onClick={submit}>
          Send
        </Button>
      </div>
      {written && <p className="cw-done">Written in {block}</p>}
      {refused !== null && <p className="cw-refused">{refused}</p>}
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
function WorkFacts({ w, page }: { w: Summary; page: WorkPage | null }) {
  const rows: [string, ReactNode][] = [
    ['Rounds', String(w.spent.rounds)],
    ['Cost', dollars(w.spent.usdMicros)],
    ['Started', w.from === null ? 'Not yet' : `${ago(w.from ?? w.added)} ago`],
    ['Tier', workTierMock()],
    ['Sent back', String(sentBackOf(w, page))],
    ['Flow', w.flow],
  ];
  return (
    <>
      <dl className="cw-facts">
        {rows.map(([key, value]) => (
          <div key={key}>
            <dt>{key}</dt>
            <dd>{value}</dd>
          </div>
        ))}
      </dl>
      <PanelPart label="The cells' marks">
        <StateLegend />
      </PanelPart>
    </>
  );
}
