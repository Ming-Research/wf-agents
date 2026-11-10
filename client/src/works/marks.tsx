// The marks of the works page: how a cell stands for a work, as a small square in the colour of its state, and the
// strip of one such mark per cell of a work's flow; the line of what a work does now, a dot in its tone and its words;
// and a part that opens a content only when the owner asks for it.

import { type ReactNode, useState } from 'react';

import type { CellNow, CellState, Now } from '../works-api';
import { STATE_WORDS, nowTone, nowWords } from './words';

/// A cell's state as a square: filled green when done, filled blue while running, a blue ring at rest, an orange ring
/// when it runs again, a hairline ring before its first run, a dash when skipped.
export function StateMark({ state, title }: { state: CellState; title?: string }) {
  return <span className={`state-mark state-${state}`} title={title ?? STATE_WORDS[state]} aria-hidden="true" />;
}

/// What a cell's mark says on the pointer's tip: `plan: done, 2 runs finished`.
export function cellNowWords(c: CellNow): string {
  return `${c.cell}: ${STATE_WORDS[c.state]}, ${c.finished} run${c.finished === 1 ? '' : 's'} finished`;
}

/// One mark per cell of a work's flow, in the order of the walk.
export function Strip({ cells }: { cells: CellNow[] }) {
  return (
    <span className="strip" role="img" aria-label={cells.map(cellNowWords).join('; ')}>
      {cells.map((c) => (
        <StateMark key={c.cell} state={c.state} title={cellNowWords(c)} />
      ))}
    </span>
  );
}

/// The legend of the states' marks.
export function StateLegend() {
  const states: CellState[] = ['done', 'running', 'resting', 'again', 'notYet', 'skipped'];
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

/// Whether the owner's pause is to be said beside what a work does now: only where that says something else, a round
/// still under way or something it waits for, since a work that has ended or says it is paused needs no more.
export function pauseDiffers(now: Now, paused: boolean): boolean {
  return paused && now.kind !== 'paused' && now.kind !== 'merged' && now.kind !== 'cancelled';
}

/// What a work does now: a dot in its tone, and its words.
export function NowLine({ now, paused = false }: { now: Now; paused?: boolean }) {
  const words = nowWords(now);
  return (
    <span className={`now now-${nowTone(now)}`} title={words}>
      <span className="now-dot" aria-hidden="true" />
      <span className="now-words">{words}</span>
      {pauseDiffers(now, paused) && <span className="now-paused">paused</span>}
    </span>
  );
}

/// A content the owner opens: read when first opened, then shown as it is, preformatted.
export function Opener({ label, load }: { label: ReactNode; load: () => Promise<string> }) {
  const [open, setOpen] = useState(false);
  const [content, setContent] = useState<{ text: string } | { failed: string } | null>(null);
  const toggle = () => {
    if (!open && content === null) {
      load().then(
        (text) => setContent({ text }),
        (e: unknown) => setContent({ failed: e instanceof Error ? e.message : String(e) }),
      );
    }
    setOpen(!open);
  };
  return (
    <div className="opener">
      <button type="button" className="link" aria-expanded={open} onClick={toggle}>
        {open ? 'Hide' : 'Show'} {label}
      </button>
      {open &&
        (content === null ? (
          <p className="muted small">Reading</p>
        ) : 'failed' in content ? (
          <p className="small failed">It could not be read: {content.failed}</p>
        ) : (
          <pre className="content">{content.text === '' ? <span className="muted">(empty)</span> : content.text}</pre>
        ))}
    </div>
  );
}
