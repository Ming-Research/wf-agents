// The marks of the works page: how a cell stands for a work, as a small square in the colour of its state, and the
// strip of one such mark per cell of a work's flow; the line of what a work does now, a dot in its tone and its words;
// and a part that opens a content, or anything read, only when the owner asks for it.

import { type ReactNode, useEffect, useRef, useState } from 'react';

import type { CellNow, CellState, Summary } from '../works-api';
import { markStyle, stripStyle } from './state-marks';
import { STATE, STATE_WORDS, nowTone, nowWords, stateOf } from './words';

/// A cell's state as a square: filled when done, filled in a deep step while running, a ring of that step at rest, a
/// red ring when its last round failed, a ring of done's colour when it runs again, a hairline ring before its first
/// run, a dash when skipped. Its look is `state-marks.ts`'s, which the console's cards draw too.
export function StateMark({ state, title }: { state: CellState; title?: string }) {
  return (
    <span
      className={`state-mark state-${state}`}
      style={markStyle(state)}
      title={title ?? STATE_WORDS[state]}
      aria-hidden="true"
    />
  );
}

/// What a cell's mark says on the pointer's tip: `plan: done, 2 runs finished`.
export function cellNowWords(c: CellNow): string {
  return `${c.cell}: ${STATE_WORDS[c.state]}, ${c.finished} run${c.finished === 1 ? '' : 's'} finished`;
}

/// One mark per cell of a work's flow, in the order of the walk.
export function Strip({ cells }: { cells: CellNow[] }) {
  return (
    <span className="strip" style={stripStyle()} role="img" aria-label={cells.map(cellNowWords).join('; ')}>
      {cells.map((c) => (
        <StateMark key={c.cell} state={c.state} title={cellNowWords(c)} />
      ))}
    </span>
  );
}

/// The legend of the states' marks.
export function StateLegend() {
  const states: CellState[] = ['done', 'running', 'calling', 'resting', 'failed', 'again', 'notYet', 'skipped'];
  return (
    <ul className="legend" aria-label="What the marks of a cell's state mean">
      {states.map((s) => (
        <li key={s}>
          <StateMark state={s} /> {STATE_WORDS[s]}
        </li>
      ))}
    </ul>
  );
}

/// Whether the owner's pause of a work is to be said beside its state: wherever it holds the work and the state says
/// something else, a round still under way or whatever it waits for, what waits on the owner among it, so that the head
/// says each thing that holds it (principle the-head-says-what-holds-it). The engine's pause is said by the state
/// itself, as what waits on the owner, and a work that has ended is held by no pause of the owner's.
export function pauseDiffers(work: Pick<Summary, 'now' | 'cells' | 'waits' | 'paused'>): boolean {
  const { word } = stateOf(work);
  const ended = work.now.kind === 'merged' || work.now.kind === 'cancelled';
  return work.paused === 'owner' && !ended && word !== STATE.paused;
}

/// What a work does now: a dot in its tone, its state's words, the state's word first, from the one catalogue; and its
/// pause where the state says something else.
export function NowLine({ work }: { work: Pick<Summary, 'now' | 'cells' | 'waits' | 'paused'> }) {
  const words = nowWords(work);
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
