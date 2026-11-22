// The marks of the works page: the strip of a work's cells, one mark per cell of its flow, each standing as its run
// does, and the marks its legend lists; the line of what a work does now, a dot in its tone and its words, and how long
// a work has waited for the calls of tools, kept current; a line of pieces of words; and a part that opens a content,
// or anything read, only when the owner asks for it, with the read it makes once it is wanted.

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

/// The time now, kept current each second while `on`: how long a work has waited for the calls of tools, or a run has
/// been under way, is said where it is shown, and would otherwise stand still until something else changed.
export function useNow(on: boolean): number {
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
/// says each thing that holds it (decision the-owners-pause-stands-beside-the-state). The engine's pause is said by
/// the state itself, as what waits on the owner, and a work that has ended is held by no pause of the owner's.
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

/// What was read: its value, or why it could not be read.
export type Got<T> = { value: T } | { failed: string };

/// What `load` reads once it is `wanted`: none until then and while it is read, then what it read or why it could
/// not. What was read is kept while `of` stays the same, and read again, when next wanted, once it changes: what was
/// read of a round under way is not what it is once the round has ended.
export function useWanted<T>(wanted: boolean, load: () => Promise<T>, of: unknown = null): Got<T> | null {
  const [content, setContent] = useState<{ of: unknown; got: Got<T> } | null>(null);
  const latest = useRef(load);
  const asked = useRef<{ of: unknown } | null>(null);
  useEffect(() => {
    latest.current = load;
  });
  const got = content !== null && Object.is(content.of, of) ? content.got : null;
  const asking = wanted && got === null;
  useEffect(() => {
    if (!asking || (asked.current !== null && Object.is(asked.current.of, of))) return;
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
  }, [asking, of]);
  return got;
}

/// What was read, as `show` draws it; until it is, that it is being read; or why it could not be.
export function Loaded<T>({ got, show }: { got: Got<T> | null; show: (value: T) => ReactNode }) {
  if (got === null) return <p className="muted small">Reading</p>;
  if ('failed' in got) return <p className="small failed">It could not be read: {got.failed}</p>;
  return <>{show(got.value)}</>;
}

/// What the owner opens: read when first opened (`useWanted`), then shown as `show` draws it.
export function Unfold<T>({ label, load, show, of = null }: {
  label: ReactNode;
  load: () => Promise<T>;
  show: (value: T) => ReactNode;
  of?: unknown;
}) {
  const [open, setOpen] = useState(false);
  const got = useWanted(open, load, of);
  return (
    <div className="opener">
      <button type="button" className="link" aria-expanded={open} onClick={() => setOpen(!open)}>
        {open ? 'Hide' : 'Show'} {label}
      </button>
      {open && <Loaded got={got} show={show} />}
    </div>
  );
}

/// A content the owner opens: read when first opened, then shown as it is, preformatted.
export function Opener({ label, load }: { label: ReactNode; load: () => Promise<string> }) {
  return <Unfold label={label} load={load} show={(text) => <Content text={text} />} />;
}
