// The marks of the works page: how a cell stands for a work, as a small square in the colour of its state, and the
// strip of one such mark per cell of a work's flow; how a run in a work's history ended or stands, in the same marks,
// and the legend of the history's marks and signs; the line of what a work does now, a dot in its tone and its words,
// and how long a work has waited for the calls of tools, kept current; a line of pieces of words; and a part that
// opens a content, or anything read, only when the owner asks for it.

import { type ReactNode, useEffect, useRef, useState } from 'react';

import { Sign } from '../parts';
import type { CellNow, CellState, Summary } from '../works-api';
import { RUN_MARKS, type RunMark, lookStyle, markStyle, pulsing, runPulsing, stripStyle } from './state-marks';
import {
  type Part,
  RUN_MARK_WORDS,
  SIGNED_WORDS,
  STATE,
  STATE_WORDS,
  STOPPED_WORDS,
  UNDER_WAY_WORDS,
  callsTook,
  callsWaited,
  nowTone,
  nowWords,
  stateOf,
  stateWords,
} from './words';

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

/// A cell's state as a square: filled when it ran or runs, hollow when still to run, a dash when skipped; filled in red
/// where the work is `stopped` at it. Running and a call under way pulse. Its look is `state-marks.ts`'s, which the
/// console's cards draw too.
export function StateMark({ state, title, stopped = false }: { state: CellState; title?: string; stopped?: boolean }) {
  return (
    <span
      className={`state-mark state-${state}${stopped ? ' stopped' : ''}${pulsing(state) ? ' pulses' : ''}`}
      style={markStyle(state, stopped)}
      title={title ?? STATE_WORDS[state]}
      aria-hidden="true"
    />
  );
}

/// What a cell's mark says on the pointer's tip: `plan: done, 2 runs finished`.
export function cellNowWords(c: CellNow): string {
  return `${c.cell}: ${STATE_WORDS[c.state]}, ${c.finished} run${c.finished === 1 ? '' : 's'} finished`;
}

/// One mark per cell of a work's flow, in the order of the walk, the cell the work is `stopped` at filled in red.
export function Strip({ cells, stopped = null }: { cells: CellNow[]; stopped?: string | null }) {
  return (
    <span className="strip" style={stripStyle()} role="img" aria-label={cells.map(cellNowWords).join('; ')}>
      {cells.map((c) => (
        <StateMark key={c.cell} state={c.state} title={cellNowWords(c)} stopped={c.cell === stopped} />
      ))}
    </span>
  );
}

/// The legend of the marks: one entry for each look, the still deep mark standing for each state drawn as it is (a run
/// at rest between two rounds, or whose round failed), and last the cell a work is stopped at.
export function StateLegend() {
  const states: CellState[] = ['done', 'running', 'calling', 'resting', 'again', 'notYet', 'skipped'];
  return (
    <ul className="legend" aria-label="What the marks of a cell's state mean">
      {states.map((s) => (
        <li key={s}>
          <StateMark state={s} /> {s === 'resting' ? UNDER_WAY_WORDS : STATE_WORDS[s]}
        </li>
      ))}
      <li>
        <StateMark state="resting" stopped /> {STOPPED_WORDS}
      </li>
    </ul>
  );
}

/// How a run in a work's history ended or stands, as a mark of the strip's look (`RUN_MARKS`); a round running and a call
/// under way pulse.
export function RunMarkSign({ mark, title }: { mark: RunMark; title?: string }) {
  return (
    <span
      className={`state-mark${runPulsing(mark) ? ' pulses' : ''}`}
      style={lookStyle(RUN_MARKS[mark])}
      title={title ?? RUN_MARK_WORDS[mark]}
      aria-hidden="true"
    />
  );
}

/// The order a work's history's legend gives the runs' marks in: how a run ended, then how an open one stands.
const RUN_MARK_ORDER: RunMark[] = ['handedIn', 'nothingNew', 'abandoned', 'running', 'calling', 'sentBack', 'underWay', 'stopped'];

/// The legend of a work's history: each mark of a run and what it says, the owner's sign and the outside's, and the way
/// back, as the flow's chart's legend draws it.
export function HistoryLegend() {
  return (
    <ul className="legend" aria-label="What the marks of the history mean">
      {RUN_MARK_ORDER.map((m) => (
        <li key={m}>
          <RunMarkSign mark={m} /> {RUN_MARK_WORDS[m]}
        </li>
      ))}
      <li>
        <Sign name="people" words={SIGNED_WORDS.owner} /> {SIGNED_WORDS.owner}
      </li>
      <li>
        <Sign name="outside" words={SIGNED_WORDS.outside} /> {SIGNED_WORDS.outside}
      </li>
      <li>
        <span className="legend-back" aria-hidden="true" /> way back
      </li>
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
