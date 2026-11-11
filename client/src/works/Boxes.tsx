// What the owner does to a work from its page: answer a question it waits for, let a run or a cell go on at its limit,
// pause it, let it on, cancel it, and write a block that people write. Each sends an `Act`, and its control is disabled
// until the answer comes, so that nothing is sent twice; where the interface refuses one, its reason is shown where
// the act was.

import { useRef, useState } from 'react';

import type { Flow } from '../api';
import { Button } from '../parts';
import type { Act, Summary, Waited } from '../works-api';
import { peopleBlocks } from './placing';
import { readersWords } from './StepPanel';
import { hasEnded, waitedWords, waitsForOwner } from './words';

/// Sends an act, and gives why it was refused, or none where it was taken.
export type Send = (what: Act) => Promise<string | null>;

/// An act on its way: whether one is, and why the last was refused. `go` sends nothing while one is on its way, and
/// says whether what it sent was taken.
export function useAct(send: Send) {
  const [pending, setPending] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  const sending = useRef(false);
  const go = async (what: Act): Promise<boolean> => {
    if (sending.current) return false;
    sending.current = true;
    setPending(true);
    const why = await send(what);
    sending.current = false;
    setPending(false);
    setRefused(why);
    return why === null;
  };
  return { pending, refused, go };
}

function Refused({ why }: { why: string | null }) {
  return why === null ? null : (
    <span className="failed small" role="alert">
      Refused: {why}
    </span>
  );
}

/// What a waiting thing is, so that what the owner typed for it stays with it as the list changes: a call by its number,
/// a precondition by its test, a limit or a slot by its cell.
export function waitedKey(at: string, waited: Waited): string {
  switch (waited.kind) {
    case 'call':
      return `call:${waited.call}`;
    case 'precondition':
      return `test:${waited.test}`;
    case 'limit':
      return `limit:${at}`;
    case 'slot':
      return `slot:${at}`;
    case 'agents':
      return 'agents';
  }
}

/// What the work waits for, each in full with what the owner can do about it: a box to answer a question, a button
/// to let a run or a cell go on, the words of a precondition.
export function WaitsBand({ summary, send }: { summary: Summary; send: Send }) {
  const now = summary.now;
  if (now.kind === 'fault') {
    return (
      <section className="waits waits-failed" aria-label="What is wrong">
        <div className="wait">
          <span className="wait-word">Fault</span>
          <span>{now.words}</span>
        </div>
      </section>
    );
  }
  if (now.kind !== 'waits') return null;
  return (
    <section className="waits" aria-label="What the work waits for">
      {now.on.map((w) => (
        <Wait key={waitedKey(now.at, w)} at={now.at} waited={w} send={send} />
      ))}
    </section>
  );
}

function Wait({ at, waited, send }: { at: string; waited: Waited; send: Send }) {
  const [answer, setAnswer] = useState('');
  const { pending, refused, go } = useAct(send);
  const forOwner = waitsForOwner(waited);
  return (
    <div className={`wait ${forOwner ? 'wait-owner' : ''}`}>
      <span className="wait-word">
        Waits at <span className="mono">{at}</span> for
      </span>
      <span>{waitedWords(waited)}</span>
      {waited.kind === 'call' && waited.what.kind === 'question' && (
        <form
          className="answer"
          onSubmit={(e) => {
            e.preventDefault();
            if (answer.trim() === '') return;
            void go({ kind: 'answer', call: waited.call, result: answer }).then((taken) => {
              if (taken) setAnswer('');
            });
          }}
        >
          <p className="question">{waited.what.question}</p>
          <textarea
            aria-label="Answer"
            rows={2}
            value={answer}
            placeholder="The answer, as the run will be given it"
            onChange={(e) => setAnswer(e.target.value)}
          />
          <Button primary submit disabled={pending || answer.trim() === ''}>
            Answer
          </Button>
        </form>
      )}
      {waited.kind === 'call' && waited.what.kind === 'atLimit' && (
        <Button primary disabled={pending} onClick={() => void go({ kind: 'answer', call: waited.call, result: 'go on' })}>
          Let the run go on
        </Button>
      )}
      {waited.kind === 'limit' && (
        <Button primary disabled={pending} onClick={() => void go({ kind: 'letGo', cell: at })}>
          Let {at} go on
        </Button>
      )}
      <Refused why={refused} />
    </div>
  );
}

/// Pause, let on, a new session for a line, and cancel; cancel asks first, since it cannot be taken back, and a new
/// session asks for which line.
export function ActButtons({ summary, lines, send }: { summary: Summary; lines: string[]; send: Send }) {
  const [asking, setAsking] = useState(false);
  const [choosing, setChoosing] = useState(false);
  const { pending, refused, go } = useAct(send);
  if (hasEnded(summary)) return null;
  const act = (what: Act) => {
    void go(what).then(() => {
      setAsking(false);
      setChoosing(false);
    });
  };
  return (
    <span className="acts">
      {summary.paused ? (
        <Button disabled={pending} onClick={() => act({ kind: 'resume' })} title="Let rounds of the work begin again">
          Let on
        </Button>
      ) : (
        <Button
          disabled={pending}
          onClick={() => act({ kind: 'pause' })}
          title="Let no round of the work begin; a round under way goes on"
        >
          Pause
        </Button>
      )}
      {lines.length > 0 &&
        (choosing ? (
          <span className="confirm" role="group" aria-label="A new session for which line?">
            <span className="small">A new session at the next round of</span>
            {lines.map((line) => (
              <Button key={line} disabled={pending} onClick={() => act({ kind: 'newSession', line })}>
                {line}
              </Button>
            ))}
            <Button disabled={pending} onClick={() => setChoosing(false)}>
              No
            </Button>
          </span>
        ) : (
          <Button
            disabled={pending}
            onClick={() => setChoosing(true)}
            title="Have a line of the work begin a new session at its next round"
          >
            New session
          </Button>
        ))}
      {asking ? (
        <span className="confirm" role="group" aria-label="Cancel the work?">
          <span className="small">Cancel the work? It cannot be taken back</span>
          <Button disabled={pending} onClick={() => act({ kind: 'cancel' })}>
            Yes, cancel it
          </Button>
          <Button disabled={pending} onClick={() => setAsking(false)}>
            No
          </Button>
        </span>
      ) : (
        <Button disabled={pending} onClick={() => setAsking(true)}>
          Cancel
        </Button>
      )}
      <Refused why={refused} />
    </span>
  );
}

/// What the owner is writing, kept by the work's page, so that choosing another step or reading the work again leaves
/// it as typed. While `held` is false the box follows the block of the cell chosen or of the cell the work is at; once
/// the owner types or picks anything it holds the block it shows, and Escape or another step no longer moves it.
export interface Draft {
  held: boolean;
  block: string | null;
  /// None until the owner picks one: a choice is never set for the owner.
  value: string | null;
  content: string;
}

export const EMPTY_DRAFT: Draft = { held: false, block: null, value: null, content: '' };

/// Which cells read a block, and when they run again for a write of it: a choice changes its meaning only when its
/// value does, and what a person writes otherwise always changes it.
export function runsAgainWords(flow: Flow, block: string): string {
  const b = flow.blocks.find((x) => x.name === block);
  if (!b || b.readBy.cells.length + b.readBy.conditions.length === 0) return 'No cell reads it';
  return `${readersWords(flow, block)}: ${b.values ? 'each runs again if the value changes' : 'each runs again for it'}`;
}

/// The box to write a block people write: by default the opinions of the cell chosen or the cell the work is at, or no
/// block where that cell reads none people write; under it, which cells read the block, since those run again for it.
export function WriteBox({ flow, summary, fallback, draft, onDraft, send }: {
  flow: Flow;
  summary: Summary;
  /// The block the box follows while the owner has neither typed nor picked anything.
  fallback: string | null;
  draft: Draft;
  onDraft: (draft: Draft) => void;
  send: Send;
}) {
  const { pending, refused, go } = useAct(send);
  const blocks = peopleBlocks(flow);
  if (blocks.length === 0) return <p className="muted small">No block of the flow is written by people</p>;
  const block = draft.held ? draft.block : fallback;
  const values = (block !== null && flow.blocks.find((b) => b.name === block)?.values) || null;
  const value = values && draft.value !== null && values.includes(draft.value) ? draft.value : null;
  const ended = hasEnded(summary);
  const ready = !ended && !pending && block !== null && (values ? value !== null : draft.content.trim() !== '');
  // Whatever the owner does holds the box where it is.
  const hold = (change: Partial<Draft>) => onDraft({ ...draft, held: true, block, value, ...change });
  return (
    <form
      className="write-box"
      aria-label="Write a block"
      onSubmit={(e) => {
        e.preventDefault();
        if (!ready || block === null) return;
        void go({ kind: 'write', block, content: draft.content, value }).then((taken) => {
          if (taken) onDraft(EMPTY_DRAFT);
        });
      }}
    >
      <div className="write-line">
        <label className="label" htmlFor="write-block">
          Write
        </label>
        <select
          id="write-block"
          className="select mono"
          value={block ?? ''}
          onChange={(e) => hold({ block: e.target.value === '' ? null : e.target.value, value: null })}
        >
          <option value="">Pick a block</option>
          {blocks.map((b) => (
            <option key={b} value={b}>
              {b}
            </option>
          ))}
        </select>
        {values && (
          <select
            aria-label="Value"
            className="select mono"
            value={value ?? ''}
            onChange={(e) => hold({ value: e.target.value === '' ? null : e.target.value })}
          >
            <option value="">Pick a value</option>
            {values.map((v) => (
              <option key={v} value={v}>
                {v}
              </option>
            ))}
          </select>
        )}
      </div>
      <textarea
        aria-label="What to write"
        rows={3}
        value={draft.content}
        disabled={ended || pending}
        placeholder={
          ended
            ? 'The work has ended'
            : values
              ? 'Why, for the cells that read it (may be left empty)'
              : 'What to say to the cells that read it'
        }
        onChange={(e) => hold({ content: e.target.value })}
      />
      <div className="write-line">
        <span className="muted small write-readers">
          {block === null ? 'Pick the block to write' : runsAgainWords(flow, block)}
        </span>
        <Button primary submit disabled={!ready}>
          Write
        </Button>
      </div>
      {refused !== null && (
        <p className="failed small" role="alert">
          Refused: {refused}
        </p>
      )}
    </form>
  );
}
