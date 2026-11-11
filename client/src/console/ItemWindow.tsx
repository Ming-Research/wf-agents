// What the panel beside the console shows of the item selected: its number, its title, which brings the item into view
// in the tree, and the flow its work runs on; a line of where the item stands and a line of what its work does, with
// the works page's strip of the work's cells and what the work has taken; what the owner does to the work (let it on,
// pause it, cancel it); and two tabs. The first says what the work waits for, and holds the notes: a box to write for a
// cell, in a block that people write, chosen above it, with the cells that read that block named under it. The second
// holds what of the work the lines do not say, and the legend of the strip's marks. With no item selected it says how
// to select one. What the owner sends is sent once: its button cannot be pressed while it is under way, and a refusal
// is said under the item it was for, and only there.
//
// Brought over from the console of the engine on `main` (`client/src/console/ItemWindow.tsx`), its layout as it was, its
// parts the page's own (parts.tsx) where it used Primer's. What it showed that the interface has nothing for is left
// out: the item's project, goal and tier, the strip of what is new to the owner, the item's body with its choices, the
// notes written before, and the work's tier and returns sent back.

import { Fragment, type ReactNode, useState } from 'react';

import { Button, Panel, PanelPart } from '../parts';
import type { Act, Item, Maker, Summary, Waited, WorkPage } from '../works-api';
import { StateLegend, Strip } from '../works/marks';
import { type ItemStanding, standingOf, whoGives } from './items';
import { age, dollars, type Mood, moodOf } from './snap';

const ago = (at: number) => age(Date.now() - at).label;

/// The words for who gives what a work waits for.
const WHO = { owner: 'you', architect: 'the architect' } as const;

/// Who gives what a work waits for, in words: the person a call names or the engine, you for a run let on at its
/// limit, and for a precondition each who the engine says can make it hold.
function whom(w: Waited, page: WorkPage | null): string {
  switch (w.kind) {
    case 'limit':
      return WHO.owner;
    case 'precondition':
      return makersWords(w.by, page);
    case 'call':
      return w.who === null ? 'the engine' : WHO[w.who];
    case 'slot':
      return 'the engine';
  }
}

/// The words for each who can make a precondition hold, joined: you, a source outside the work, or a cell by its title.
function makersWords(by: Maker[], page: WorkPage | null): string {
  const words = by.map((m) =>
    m.kind === 'owner' ? WHO.owner : m.kind === 'outside' ? `the outside (${m.source})` : titleOf(page, m.cell),
  );
  return words.join(' or ');
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

const capital = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

/// What the work waits for, one thing at a time, in words.
export function waitedWords(w: Waited, page: WorkPage | null, at: string): string {
  switch (w.kind) {
    case 'call':
      switch (w.what.kind) {
        case 'question':
          return `${capital(whom(w, page))} to answer: ${w.what.question}`;
        case 'tool':
          return `The engine to run ${w.what.name}`;
        case 'atLimit':
          return w.what.limit.of === 'returns'
            ? `${capital(whom(w, page))} to let the run go on after ${w.what.limit.sentBack} returns sent back`
            : `${capital(whom(w, page))} to let the run go on after ${w.what.limit.made} calls`;
      }
      return '';
    case 'precondition': {
      const one = w.by.length === 1 && w.by[0].kind !== 'owner';
      return `${w.words}, which ${whom(w, page)} make${one ? 's' : ''} hold`;
    }
    case 'limit':
      return `You to let ${titleOf(page, at)} run again after ${w.runs} runs in a row for what a cell after it wrote`;
    case 'slot':
      return 'A free slot, which the engine gives in its order';
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
      const who = [...new Set(now.on.map((w) => whom(w, page)))];
      return { word: `Waiting for ${who.join(' and ')}`, facts: `at ${titleOf(page, now.at)}` };
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

export type ItemPanelProps = {
  /// The item selected, as the interface gives it.
  item: Item;
  /// Its work's page, which says the flow.
  page: PageState;
  act: (work: number, what: Act) => Promise<unknown>;
  /// The items it depends on that are not merged yet.
  waitsFor: number[];
  folded: boolean;
  onFold: () => void;
  edge?: ReactNode;
  /// Brings the item into view in the tree.
  onTitle: () => void;
};

/// The panel of the item selected. Its lines and its body are the item's own: what one item was sending, or was refused,
/// is not shown under another.
export function ItemPanel({ item, page, act, waitsFor, folded, onFold, edge, onTitle }: ItemPanelProps) {
  const work = item.work;
  const read = pageIn(page);
  const send = (what: Act) => act(work.number, what);
  return (
    <Panel
      label={`Item ${item.number}`}
      id={item.number}
      title={
        <button type="button" className="cw-title-link" title="Show it in the tree" onClick={onTitle}>
          {item.title}
        </button>
      }
      belongs={work.flow}
      lines={
        <div key={item.number}>
          <ItemLine item={item} waitsFor={waitsFor} />
          <WorkLine w={work} page={read} />
          {!ended(work) && <Acts paused={work.paused || work.now.kind === 'paused'} act={send} />}
        </div>
      }
      folded={folded}
      onFold={onFold}
      edge={edge}
    >
      <WindowBody key={item.number} work={work} page={page} act={send} />
    </Panel>
  );
}

function ItemLine({ item, waitsFor }: { item: Item; waitsFor: number[] }) {
  const standing = standingOf(item);
  return (
    <Line tone={standing}>
      <span className="cw-strong">{STANDING_WORDS[standing]}</span>
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
      {/* How far it has come and what it has taken, together at the line's right end. */}
      <span className="cw-progress">
        <Strip cells={w.cells} />
        <span className="cw-mono" title="What its rounds have taken">
          {dollars(w.spent.usdMicros)}
        </span>
      </span>
    </Line>
  );
}

/// What the owner does to a work that goes on: let it on while it is paused, or pause it; cancel it, once they confirm.
/// While one is under way, none can be pressed.
function Acts({ paused, act }: { paused: boolean; act: (what: Act) => Promise<unknown> }) {
  const { waiting, refused, send } = useSending(act);
  return (
    <div className="cw-acts">
      {paused ? (
        <Button primary title="Let the work go on" disabled={waiting} onClick={() => void send({ kind: 'resume' })}>
          Let it on
        </Button>
      ) : (
        <Button
          title="Only a note to a cell is answered; a round under way goes on"
          disabled={waiting}
          onClick={() => void send({ kind: 'pause' })}
        >
          Pause
        </Button>
      )}
      <Button
        title="Cancel the work and its item"
        disabled={waiting}
        onClick={() => {
          if (window.confirm('Cancel this work? It cannot be let on again.')) void send({ kind: 'cancel' });
        }}
      >
        Cancel
      </Button>
      {refused !== null && <span className="cw-refused">{refused}</span>}
    </div>
  );
}

/// The panel's body: two tabs, what the work waits for and the notes, and what of the work the lines do not say.
function WindowBody({ work, page, act }: { work: Summary; page: PageState; act: (what: Act) => Promise<unknown> }) {
  const [tab, setTab] = useState<'item' | 'work'>('item');
  const read = pageIn(page);
  return (
    <div className="cw-item">
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
      {tab === 'work' ? (
        <WorkFacts w={work} />
      ) : (
        <>
          {work.now.kind === 'waits' && <Waits on={work.now.on} page={read} at={work.now.at} />}
          <Notes work={work.number} page={page} act={act} />
        </>
      )}
    </div>
  );
}

function Waits({ on, page, at }: { on: Waited[]; page: WorkPage | null; at: string }) {
  return (
    <PanelPart label={`Waits at ${titleOf(page, at)}`} count={on.length}>
      <ul>
        {on.map((x, i) => (
          <li key={i} className={whoGives(x) === 'owner' ? 'cw-wait cw-wait-owner' : 'cw-wait'}>
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

/// The notes: a box to write for a cell. The owner picks a block that people write, of those that hold words rather
/// than a choice, and what they send is written into it; the line under the box names the cells that read it.
function Notes({ work, page, act }: { work: number; page: PageState; act: (what: Act) => Promise<unknown> }) {
  const read = pageIn(page);
  const blocks = (read?.flow.blocks ?? []).filter((b) => b.writtenBy.people && b.values === null);
  const [chosen, setChosen] = useState<string | null>(null);
  const block = blocks.find((b) => b.name === chosen) ?? blocks[0];
  return (
    <section className="cw-notes">
      <h3>Notes</h3>
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
            key={draftKey(work, block.name)}
            work={work}
            block={block.name}
            readers={block.readBy.cells.map((c) => titleOf(read, c))}
            act={act}
          />
        </div>
      )}
    </section>
  );
}

/// The box for one block of one work, its draft kept as it is typed.
function Compose({ work, block, readers, act }: {
  work: number;
  block: string;
  readers: string[];
  act: (what: Act) => Promise<unknown>;
}) {
  const key = draftKey(work, block);
  const [text, setText] = useState(() => readDraft(key));
  const [sent, setSent] = useState(false);
  const { waiting, refused, send } = useSending(act);
  const type = (t: string) => {
    setText(t);
    setSent(false);
    writeDraft(key, t);
  };
  const submit = () => {
    const content = text.trim();
    void send({ kind: 'write', block, content, value: null }).then((done) => {
      if (!done) return;
      writeDraft(key, '');
      setText((now) => (now.trim() === content ? '' : now));
      setSent(true);
    });
  };
  return (
    <>
      <textarea
        rows={3}
        value={text}
        aria-label={`What to write in ${block}`}
        placeholder={`Write for the cells that read ${block}`}
        onChange={(e) => type(e.target.value)}
      />
      <div className="cw-compose-foot">
        <span className="cw-muted">{readers.length > 0 ? `Goes to ${readers.join(', ')}` : 'No cell reads it'}</span>
        <Button primary disabled={waiting || text.trim() === ''} onClick={submit}>
          Send
        </Button>
      </div>
      {sent && <p className="cw-done">Written in {block}</p>}
      {refused !== null && <p className="cw-refused">{refused}</p>}
    </>
  );
}

/// What of the work the lines do not say, and what the marks of the strip in its line mean.
function WorkFacts({ w }: { w: Summary }) {
  const rows: [string, ReactNode][] = [
    ['Rounds', String(w.spent.rounds)],
    ['Cost', dollars(w.spent.usdMicros)],
    ['Started', `${ago(w.added)} ago`],
  ];
  return (
    <>
      <dl className="pairs cw-facts">
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
