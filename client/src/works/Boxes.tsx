// What the owner does to a work from its page: answer a question it waits for, let a run or a cell go on at its limit,
// pause it, let it on, cancel it, and write a block that people write. Each sends an `Act`; where the interface refuses
// one, its reason is shown where the act was.

import { useState } from 'react';

import type { Flow } from '../api';
import { Button } from '../parts';
import type { Act, Summary, Waited } from '../works-api';
import { peopleBlocks } from './placing';
import { readersWords } from './StepPanel';
import { hasEnded, waitedWords, waitsForOwner } from './words';

/// Sends an act, and says whether it was taken; a refusal's reason is shown by the part that sent it.
export type Send = (what: Act) => Promise<string | null>;

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
      {now.on.map((w, i) => (
        <Wait key={i} at={now.at} waited={w} send={send} />
      ))}
    </section>
  );
}

function Wait({ at, waited, send }: { at: string; waited: Waited; send: Send }) {
  const [answer, setAnswer] = useState('');
  const [refused, setRefused] = useState<string | null>(null);
  const go = async (what: Act, clear = false) => {
    const why = await send(what);
    setRefused(why);
    if (why === null && clear) setAnswer('');
  };
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
            if (answer.trim() !== '') void go({ kind: 'answer', call: waited.call, result: answer }, true);
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
          <button type="submit" className="button button-primary" disabled={answer.trim() === ''}>
            Answer
          </button>
        </form>
      )}
      {waited.kind === 'call' && waited.what.kind === 'atLimit' && (
        <Button primary onClick={() => void go({ kind: 'answer', call: waited.call, result: 'go on' })}>
          Let the run go on
        </Button>
      )}
      {waited.kind === 'limit' && (
        <Button primary onClick={() => void go({ kind: 'letGo', cell: at })}>
          Let {at} go on
        </Button>
      )}
      {refused !== null && <span className="failed small">Refused: {refused}</span>}
    </div>
  );
}

/// Pause, let on and cancel; cancel asks first, since it cannot be taken back.
export function ActButtons({ summary, send }: { summary: Summary; send: Send }) {
  const [asking, setAsking] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  if (hasEnded(summary)) return null;
  const go = async (what: Act) => {
    setRefused(await send(what));
    setAsking(false);
  };
  return (
    <span className="acts">
      {summary.paused ? (
        <Button onClick={() => void go({ kind: 'resume' })} title="Let rounds of the work begin again">
          Let on
        </Button>
      ) : (
        <Button onClick={() => void go({ kind: 'pause' })} title="Let no round of the work begin; a round under way goes on">
          Pause
        </Button>
      )}
      {asking ? (
        <span className="confirm" role="group" aria-label="Cancel the work?">
          <span className="small">Cancel the work? It cannot be taken back</span>
          <Button onClick={() => void go({ kind: 'cancel' })}>Yes, cancel it</Button>
          <Button onClick={() => setAsking(false)}>No</Button>
        </span>
      ) : (
        <Button onClick={() => setAsking(true)}>Cancel</Button>
      )}
      {refused !== null && <span className="failed small">Refused: {refused}</span>}
    </span>
  );
}

/// Which cells read a block, and when they run again for a write of it: a choice changes its meaning only when its
/// value does, and what a person writes otherwise always changes it.
export function runsAgainWords(flow: Flow, block: string): string {
  const b = flow.blocks.find((x) => x.name === block);
  if (!b || b.readBy.cells.length + b.readBy.conditions.length === 0) return 'No cell reads it';
  return `${readersWords(flow, block)}: ${b.values ? 'each runs again if the value changes' : 'each runs again for it'}`;
}

/// What the owner is writing: the block, the value of a choice, and the words. Kept by the work's page, so that
/// choosing another step or reading the work again leaves it as typed.
export interface Draft {
  /// None while the box follows the block of the cell chosen or the cell the work is at.
  block: string | null;
  value: string | null;
  content: string;
}

export const EMPTY_DRAFT: Draft = { block: null, value: null, content: '' };

/// The box to write a block people write, by default the opinions of the cell chosen or the cell the work is at; under
/// it, which cells read the block, since those are the cells that run again for it.
export function WriteBox({ flow, summary, fallback, draft, onDraft, send }: {
  flow: Flow;
  summary: Summary;
  /// The block the box follows while the owner has not picked one.
  fallback: string | null;
  draft: Draft;
  onDraft: (draft: Draft) => void;
  send: Send;
}) {
  const [refused, setRefused] = useState<string | null>(null);
  const blocks = peopleBlocks(flow);
  const block = draft.block ?? fallback;
  if (blocks.length === 0 || block === null) return <p className="muted small">No block of the flow is written by people</p>;
  const values = flow.blocks.find((b) => b.name === block)?.values ?? null;
  const value = values ? (draft.value !== null && values.includes(draft.value) ? draft.value : values[0]) : null;
  const ended = hasEnded(summary);
  const ready = !ended && (values !== null || draft.content.trim() !== '');
  const write = async () => {
    const why = await send({ kind: 'write', block, content: draft.content, value });
    setRefused(why);
    if (why === null) onDraft({ ...EMPTY_DRAFT, block: draft.block });
  };
  return (
    <form
      className="write-box"
      aria-label="Write a block"
      onSubmit={(e) => {
        e.preventDefault();
        if (ready) void write();
      }}
    >
      <div className="write-line">
        <label className="label" htmlFor="write-block">
          Write
        </label>
        <select
          id="write-block"
          className="select mono"
          value={block}
          onChange={(e) => onDraft({ ...draft, block: e.target.value, value: null })}
        >
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
            onChange={(e) => onDraft({ ...draft, block, value: e.target.value })}
          >
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
        disabled={ended}
        placeholder={ended ? 'The work has ended' : values ? 'Why, for the cells that read it (may be left empty)' : 'What to say to the cells that read it'}
        onChange={(e) => onDraft({ ...draft, block: draft.block, content: e.target.value })}
      />
      <div className="write-line">
        <span className="muted small write-readers">{runsAgainWords(flow, block)}</span>
        <button type="submit" className="button button-primary" disabled={!ready}>
          Write
        </button>
      </div>
      {refused !== null && <p className="failed small">Refused: {refused}</p>}
    </form>
  );
}
