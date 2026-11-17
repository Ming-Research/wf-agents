// The marks of the works page: the strip of a work's cells, one mark per cell of its flow, each standing as its run
// does, and the marks its legend lists; the line of what a work does now, a dot in its tone and its words, and how long
// a work has waited for the calls of tools, kept current; a line of pieces of words; and a part that opens a content,
// or anything read, only when the owner asks for it.

import { type ReactNode, useEffect, useRef, useState } from 'react';

import { type MarkKey, StateMark, markName } from '../marks';
import type { CellNow, Summary } from '../works-api';
import { STATE_OF_CELL, stripStyle } from './state-marks';
import { type Part, STATE, callsTook, callsWaited, nowTone, nowWords, stateOf, stateWords } from './words';

/// A line of pieces, each name or value in monospace, and a link out of the page opened beside it.
export function Parts({ parts }: { parts: Part[] }) {
  return (
    <>
      {parts.map((p, i) =>
        typeof p === 'string' ? (
          p
        ) : 'name' in p ? (
          <span key={i} className="mono">
            {p.name}
          </span>
        ) : (
          <a key={i} className="link" href={p.href} target="_blank" rel="noreferrer">
            {p.link}
          </a>
        ),
      )}
    </>
  );
}

/// The time now, kept current each second while `on`: how long a work has waited for the calls of tools is said where
/// the work is shown, and would otherwise stand still until something else changed.
function useNow(on: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!on) return;
    setNow(Date.now());
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [on]);
  return now;
}

/// How long a work has waited for the calls of tools it waits for, kept current; nothing where it waits for none, or
/// the engine does not say when they were made.
export function CallsTook({ work }: { work: Pick<Summary, 'now'> }) {
  const calls = callsWaited(work);
  const now = useNow(calls.length > 0);
  const took = callsTook(calls, now);
  return took === null ? null : <span className="muted">{took}</span>;
}

/// What a cell's mark says on the pointer's tip: `plan: handed in, 2 runs finished`.
export function cellNowWords(c: CellNow): string {
  return `${c.cell}: ${markName(STATE_OF_CELL[c.state])}, ${c.finished} run${c.finished === 1 ? '' : 's'} finished`;
}

/// One mark per cell of a work's flow, in the order of the walk, each as its run stands, the cell the work is `stopped`
/// at as stopped.
export function Strip({ cells, stopped = null }: { cells: CellNow[]; stopped?: string | null }) {
  return (
    <span className="strip" style={stripStyle()} role="img" aria-label={cells.map(cellNowWords).join('; ')}>
      {cells.map((c) => (
        <StateMark
          key={c.cell}
          state={c.cell === stopped ? 'stopped' : STATE_OF_CELL[c.state]}
          title={cellNowWords(c)}
        />
      ))}
    </span>
  );
}

/// The marks the strip draws, in the order its legend lists them (`Legend` in marks.tsx).
export const STRIP_MARKS: MarkKey[] = [
  'handedIn',
  'running',
  'waiting',
  'atRest',
  'failed',
  'again',
  'notYet',
  'skipped',
  'stopped',
];

/// Whether the owner's pause of a work is to be said beside its state: wherever it holds the work and the state says
/// something else, a round still under way or whatever it waits for, what waits on the owner among it, so that the head
/// says each thing that holds it (principle the-head-says-what-holds-it). The engine's pause is said by the state
/// itself, as what waits on the owner, and a work that has ended is held by no pause of the owner's.
export function pauseDiffers(work: Pick<Summary, 'now' | 'cells' | 'waits' | 'paused'>): boolean {
  const { word } = stateOf(work);
  const ended = work.now.kind === 'merged' || work.now.kind === 'cancelled';
  return work.paused === 'owner' && !ended && word !== STATE.paused;
}

/// What a work does now: a dot in its tone, its state's words, the state's word first, from the one catalogue, with how
/// long it has waited for the calls of tools it waits for, kept current, and why, but where it stands in a column of
/// states, which holds the state alone (principle a-column-holds-what-is-alike); and its pause where the state says
/// something else.
export function NowLine({ work, why = true }: {
  work: Pick<Summary, 'now' | 'cells' | 'waits' | 'paused'>;
  why?: boolean;
}) {
  const now = useNow(callsWaited(work).length > 0);
  const words = why ? nowWords(work, now) : stateWords(work, now);
  return (
    <span className={`now now-${nowTone(work)}`} title={words}>
      <span className="now-dot" aria-hidden="true" />
      <span className="now-words">{words}</span>
      {pauseDiffers(work) && <span className="now-paused">{STATE.paused}</span>}
    </span>
  );
}

/// A content as it is, preformatted.
export function Content({ text }: { text: string }) {
  return <pre className="content">{text === '' ? <span className="muted">(empty)</span> : text}</pre>;
}

/// What the owner opens: read when first opened, then shown as `show` draws it. What was read is kept while `of`
/// stays the same, and read again, when next shown, once it changes: what was read of a round under way is not what
/// it is once the round has ended.
export function Unfold<T>({ label, load, show, of = null }: {
  label: ReactNode;
  load: () => Promise<T>;
  show: (value: T) => ReactNode;
  of?: unknown;
}) {
  const [open, setOpen] = useState(false);
  const [content, setContent] = useState<{ of: unknown; got: { value: T } | { failed: string } } | null>(null);
  const latest = useRef(load);
  const asked = useRef<{ of: unknown } | null>(null);
  useEffect(() => {
    latest.current = load;
  });
  const got = content !== null && Object.is(content.of, of) ? content.got : null;
  const wanted = open && got === null;
  useEffect(() => {
    if (!wanted || (asked.current !== null && Object.is(asked.current.of, of))) return;
    const ask = { of };
    asked.current = ask;
    latest.current().then(
      (value) => {
        if (asked.current === ask) setContent({ of, got: { value } });
      },
      (e: unknown) => {
        if (asked.current === ask) setContent({ of, got: { failed: e instanceof Error ? e.message : String(e) } });
      },
    );
  }, [wanted, of]);
  return (
    <div className="opener">
      <button type="button" className="link" aria-expanded={open} onClick={() => setOpen(!open)}>
        {open ? 'Hide' : 'Show'} {label}
      </button>
      {open &&
        (got === null ? (
          <p className="muted small">Reading</p>
        ) : 'failed' in got ? (
          <p className="small failed">It could not be read: {got.failed}</p>
        ) : (
          show(got.value)
        ))}
    </div>
  );
}

/// A content the owner opens: read when first opened, then shown as it is, preformatted.
export function Opener({ label, load }: { label: ReactNode; load: () => Promise<string> }) {
  return <Unfold label={label} load={load} show={(text) => <Content text={text} />} />;
}
