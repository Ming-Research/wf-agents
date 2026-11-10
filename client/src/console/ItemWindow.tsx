// What the panel beside the console shows of the item selected: its number, its title, which brings the item into view
// in the tree, and the flow its work runs on; a line of where the item stands and a line of what its work does, with a
// mark for each cell of the work's flow and what the work has taken; what the owner does to the work (let it on, pause
// it, cancel it); and two tabs. The first says what the work waits for, and holds the notes: a box to write for a cell,
// in a block that people write, chosen above it, with the cells that read that block named under it. The second holds
// what of the work the lines do not say. With no item selected it says how to select one.
//
// Brought over from the console of the engine on `main` (`client/src/console/ItemWindow.tsx`), its layout as it was, its
// parts the page's own (parts.tsx) where it used Primer's. What it showed that the interface has nothing for is left
// out: the item's project, goal and tier, the strip of what is new to the owner, the item's body with its choices, the
// notes written before, and the work's tier and returns sent back.

import { type ReactNode, useState } from 'react';

import { Button, PanelPart } from '../parts';
import type { Act, Item, Summary, Waited, WorkPage } from '../works-api';
import { type ItemStanding, hereOf, latest, standingOf } from './items';
import { age, dollars, type Mood, moodOf } from './snap';

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

/// A cell's title in the work's flow, or its id where the flow does not name it.
const titleOf = (page: WorkPage | null, cell: string) => page?.flow.cells.find((c) => c.id === cell)?.title ?? cell;

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

const capital = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

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
      return { word: MOOD_WORDS.paused, facts: null };
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

function Line({ tone, children }: { tone: Tone; children: ReactNode }) {
  return (
    <div className="cw-line">
      <span className={`cw-dot dot-${tone}`} aria-hidden="true" />
      {children}
    </div>
  );
}

export type WindowProps = {
  /// The item selected, as the interface gives it; none while it is read.
  item: Item;
  /// Its latest work's page, which says the flow; none while it is read.
  page: WorkPage | null;
  act: (work: number, what: Act) => Promise<unknown>;
  /// The items it depends on that are not merged yet.
  waitsFor: number[];
};

/// The panel's head: the item's number, its title, which brings it into view in the tree, and the flow its work runs on.
export function windowHead(item: Item, onTitle: () => void) {
  return {
    id: item.number,
    title: (
      <button type="button" className="cw-title-link" title="Show it in the tree" onClick={onTitle}>
        {item.title}
      </button>
    ),
    belongs: latest(item)?.flow,
  };
}

/// The panel's lines under its head: where the item stands, what its work does, and what the owner does to the work.
export function WindowLines({ item, page, act, waitsFor }: WindowProps) {
  const work = latest(item);
  return (
    <>
      <ItemLine item={item} waitsFor={waitsFor} />
      {work === undefined ? <div className="cw-line cw-muted">No work yet</div> : <WorkLine w={work} page={page} />}
      {work !== undefined && !ended(work) && <Acts w={work} act={act} />}
    </>
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
        <CellMarks w={w} page={page} />
        <span className="cw-mono" title="What its rounds have taken">
          {dollars(w.spent.usdMicros)}
        </span>
      </span>
    </Line>
  );
}

/// How a cell's mark looks: done, where the work is, where it waits, or not done.
type Look = 'done' | 'now' | 'wait' | 'ahead';

const LOOK_WORDS: Record<Look, string> = { done: 'done', now: 'under way', wait: 'waits here', ahead: 'not done' };

/// How far a work has come, as one row of narrow cells, one for each cell of its flow, each the first letter of its
/// id: those whose record knows everything filled in the colour of done, the one it is at standing out, in the colour
/// of waiting while it waits, the others plain. Each says its cell on hovering.
export function CellMarks({ w, page }: { w: Summary; page: WorkPage | null }) {
  const here = ended(w) ? null : hereOf(w);
  const running = w.now.kind === 'round' || w.now.kind === 'next';
  return (
    <span className="stage-bar">
      {w.cells.map((c) => {
        const look: Look = c.cell === here ? (running ? 'now' : 'wait') : c.state === 'done' ? 'done' : 'ahead';
        const title = `${titleOf(page, c.cell)}: ${c.state === 'skipped' && look === 'ahead' ? 'skipped' : LOOK_WORDS[look]}`;
        return (
          <span
            key={c.cell}
            className={`stage-cell stage-${look}`}
            title={title}
            aria-label={title}
            aria-current={look === 'now' || look === 'wait' ? 'step' : undefined}
          >
            {c.cell.charAt(0).toUpperCase()}
          </span>
        );
      })}
    </span>
  );
}

/// What the owner does to a work that goes on: let it on while it is paused, or pause it; cancel it, once they confirm.
function Acts({ w, act }: { w: Summary; act: WindowProps['act'] }) {
  const [refused, setRefused] = useState<string | null>(null);
  const send = (what: Act) => {
    setRefused(null);
    act(w.number, what).catch((e: unknown) => setRefused(e instanceof Error ? e.message : String(e)));
  };
  const paused = w.paused || w.now.kind === 'paused';
  return (
    <div className="cw-acts">
      {paused ? (
        <Button primary title="Let the work go on" onClick={() => send({ kind: 'resume' })}>
          Let it on
        </Button>
      ) : (
        <Button title="No round of it begins; a round under way goes on" onClick={() => send({ kind: 'pause' })}>
          Pause
        </Button>
      )}
      <Button
        title="Cancel the work and its item"
        onClick={() => {
          if (window.confirm(`Cancel work ${w.number}? It cannot be let on again.`)) send({ kind: 'cancel' });
        }}
      >
        Cancel
      </Button>
      {refused !== null && <span className="cw-refused">{refused}</span>}
    </div>
  );
}

/// The panel's body: two tabs, what the work waits for and the notes, and what of the work the lines do not say.
export function WindowBody({ item, page, act }: Omit<WindowProps, 'waitsFor'>) {
  const [tab, setTab] = useState<'item' | 'work'>('item');
  const work = latest(item);
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
          {work !== undefined && work.now.kind === 'waits' && <Waits w={work} page={page} at={work.now.at} />}
          {work !== undefined && <Notes w={work} page={page} act={act} />}
        </>
      )}
    </div>
  );
}

function Waits({ w, page, at }: { w: Summary; page: WorkPage | null; at: string }) {
  const on = w.now.kind === 'waits' ? w.now.on : [];
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

/// The notes: a box to write for a cell. The owner picks a block that people write, of those that hold words rather
/// than a choice, and what they send is written into it; the line under the box names the cells that read it.
function Notes({ w, page, act }: { w: Summary; page: WorkPage | null; act: WindowProps['act'] }) {
  const blocks = (page?.flow.blocks ?? []).filter((b) => b.writtenBy.people && b.values === null);
  const [chosen, setChosen] = useState<string | null>(null);
  const block = blocks.find((b) => b.name === chosen) ?? blocks[0];
  return (
    <section className="cw-notes">
      <h3>Notes</h3>
      {page === null ? (
        <p className="cw-muted">Reading the work's flow</p>
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
            key={draftKey(w.number, block.name)}
            work={w.number}
            block={block.name}
            readers={block.readBy.cells.map((c) => titleOf(page, c))}
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
  act: WindowProps['act'];
}) {
  const key = draftKey(work, block);
  const [text, setText] = useState(() => readDraft(key));
  const [sent, setSent] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  const type = (t: string) => {
    setText(t);
    setSent(false);
    writeDraft(key, t);
  };
  const send = () => {
    const content = text.trim();
    setRefused(null);
    act(work, { kind: 'write', block, content, value: null }).then(
      () => {
        writeDraft(key, '');
        setText((now) => (now.trim() === content ? '' : now));
        setSent(true);
      },
      (e: unknown) => setRefused(e instanceof Error ? e.message : String(e)),
    );
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
        <button type="button" className="button button-primary" disabled={text.trim() === ''} onClick={send}>
          Send
        </button>
      </div>
      {sent && <p className="cw-done">Written in {block}</p>}
      {refused !== null && <p className="cw-refused">{refused}</p>}
    </>
  );
}

function WorkFacts({ w }: { w: Summary | undefined }) {
  if (w === undefined) return <p className="cw-muted">No work yet</p>;
  const rows: [string, ReactNode][] = [
    ['Rounds', String(w.spent.rounds)],
    ['Cost', dollars(w.spent.usdMicros)],
    ['Started', `${ago(w.added)} ago`],
  ];
  return (
    <dl className="cw-facts">
      {rows.map(([key, value]) => (
        <div key={key}>
          <dt>{key}</dt>
          <dd>{value}</dd>
        </div>
      ))}
    </dl>
  );
}
