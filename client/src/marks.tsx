// Every mark the page draws, in one table: what draws each, one part for each look, and its name, one or two words in
// the system's own terms; and the legend, which draws a list of them on one line. The flow's matrix, the flowchart, the
// strip of a work's cells, a work's history and the observations each list in their legend the marks they draw, so
// that one thing has one name and one look in every view (Article 8). What the flow's files define is named by its key
// (principle definitions-shown-as-written); how a run ended or stands has one set of marks, which the strip draws for
// each cell, as its run stands, and the history for each run (`works/state-marks.ts`). Beside the table, the looks of
// the level a work's cell runs at and of a work's lines added and removed, which the page and the console's cards both
// draw from.

import type { ReactNode } from 'react';

import { ConditionGlyph, DataGlyphs, LineGlyph, ToolGlyph } from './Chart';
import { type Mark, NO_MARK } from './matrix';
import { Sign } from './parts';
import { RUN_STATES, type RunState, lookStyle, pulses } from './works/state-marks';
import { NO_LEVEL, keysLine, linesAdded, linesRemoved } from './works/words';

/// A mark the page draws: of the flow's definition, as its files' keys name it; of a line between cells; of who
/// writes a datum besides the cells; of how a run ended or stands; of an observation not read.
export type MarkKey =
  | 'line'
  | 'whenYes'
  | 'whenNo'
  | 'requires'
  | 'tools'
  | 'watches'
  | 'reads'
  | 'latest'
  | 'lastAndLatest'
  | 'all'
  | 'outputs'
  | 'wakes'
  | 'wayBack'
  | 'mainPath'
  | 'fromOutside'
  | 'when'
  | 'writer'
  | 'people'
  | 'outside'
  | 'shown'
  | 'unread'
  | RunState;

/// How a run ended or stands, as a square of its look, a round running and waiting for a call pulsing; `title` says
/// it on pointing at it, its name where none is given.
export function StateMark({ state, title }: { state: RunState; title?: string }) {
  return (
    <span
      className={`state-mark${pulses(state) ? ' pulses' : ''}`}
      style={lookStyle(RUN_STATES[state])}
      title={title ?? MARKS[state].name}
      aria-hidden="true"
    />
  );
}

/// A line between cells, as the flowchart draws it and a work's history along its rows: an arrow a write runs to a
/// cell that watches what it writes, thin in the colour of the data's lines; a way back, thick in the accent; the main
/// path, thick in the text's colour; an arrow from what is written outside the cells, dashed.
function LineSample({ kind }: { kind: 'wakes' | 'back' | 'main' | 'outside' }) {
  return <span className={`legend-line ${kind}`} aria-hidden="true" />;
}

/// A node of the flowchart: the way into a cell, by its `when` and its `requires`; edged in the accent, a cell that
/// writes what the chosen cell takes.
function NodeSample({ lit = false }: { lit?: boolean }) {
  return <span className={`legend-node${lit ? ' lit' : ''}`} aria-hidden="true" />;
}

/// The mark of an observation not yet read: a dot in the text's colour.
export function UnreadMark() {
  return <span className="dot dot-unread" role="img" aria-label="unread" title="unread" />;
}

const data = (fields: Partial<Mark>) => <DataGlyphs mark={{ ...NO_MARK, ...fields }} />;

/// What draws a mark, with `words` said on pointing at it where its part says anything, and the mark's name.
type Entry = { name: string; draw: (words?: string) => ReactNode };

const ofRun = (state: RunState, name: string): Entry => ({
  name,
  draw: (words) => <StateMark state={state} title={words} />,
});

/// The table: every mark, its part and its name.
export const MARKS: Record<MarkKey, Entry> = {
  line: { name: 'line', draw: () => <LineGlyph on /> },
  whenYes: { name: 'when yes', draw: () => <ConditionGlyph spot={{ when: true, requires: false }} /> },
  whenNo: { name: 'when no', draw: () => <ConditionGlyph spot={{ when: true, value: 'no', requires: false }} /> },
  requires: { name: 'requires', draw: () => <ConditionGlyph spot={{ when: false, requires: true }} /> },
  tools: { name: 'tools', draw: () => <ToolGlyph on /> },
  watches: { name: 'watches', draw: () => data({ watch: true }) },
  reads: { name: 'reads', draw: () => data({ read: true }) },
  latest: { name: 'latest', draw: () => data({ read: true, kind: 'latest' }) },
  lastAndLatest: { name: 'last_and_latest', draw: () => data({ read: true, kind: 'lastAndLatest' }) },
  all: { name: 'all', draw: () => data({ read: true, kind: 'all' }) },
  outputs: { name: 'outputs', draw: () => data({ write: true }) },
  wakes: { name: 'wakes', draw: () => <LineSample kind="wakes" /> },
  wayBack: { name: 'way back', draw: () => <LineSample kind="back" /> },
  mainPath: { name: 'main path', draw: () => <LineSample kind="main" /> },
  fromOutside: { name: 'from outside', draw: () => <LineSample kind="outside" /> },
  when: { name: 'when', draw: () => <NodeSample /> },
  writer: { name: 'writer', draw: () => <NodeSample lit /> },
  people: { name: 'people', draw: (words) => <Sign name="people" words={words} /> },
  outside: { name: 'outside', draw: (words) => <Sign name="outside" words={words} /> },
  shown: { name: 'shown', draw: (words) => <Sign name="shown" words={words} /> },
  unread: { name: 'unread', draw: () => <UnreadMark /> },
  running: ofRun('running', 'running'),
  waiting: ofRun('waiting', 'waiting'),
  atRest: ofRun('atRest', 'at rest'),
  failed: ofRun('failed', 'failed'),
  stopped: ofRun('stopped', 'stopped'),
  handedIn: ofRun('handedIn', 'handed in'),
  again: ofRun('again', 'runs again'),
  notYet: ofRun('notYet', 'not yet'),
  skipped: ofRun('skipped', 'skipped'),
};

/// A mark's name.
export const markName = (key: MarkKey): string => MARKS[key].name;

/// The looks of the level a work's cell really runs at, its item's level raised as far as the cell's line says, lowest
/// first: the first muted, the second in the text's colour, the third in the accent, and each above it in the accent
/// and strong. Each colour is a token's name, as small text takes it (decision a-level-looks-as-the-level-it-runs-at).
export const LEVEL_LOOKS = [
  { colour: '--muted', strong: false },
  { colour: '--text', strong: false },
  { colour: '--accent-text', strong: false },
  { colour: '--accent-text', strong: true },
] as const;

export type LevelLook = (typeof LEVEL_LOOKS)[number];

/// A level as a card and the item's window write it: `L2`, and `L2+1` where the cell's line raises it one above.
export const levelWords = (level: string, raise: number): string => (raise > 0 ? `L${level}+${raise}` : `L${level}`);

/// The look of a level raised so far, by the number the level's name is and the raise, past the highest an item may be
/// at too; a level whose name is no number takes the second.
export function levelLook(level: string, raise: number): LevelLook {
  const real = /^\d+$/.test(level) ? Number(level) + raise : 2;
  return LEVEL_LOOKS[Math.min(LEVEL_LOOKS.length, Math.max(1, real)) - 1];
}

/// The level a work's cell runs at, `L2` or `L2+1`, in its look, saying on pointing at it its keys, `level 2,
/// levels_above 1`; where the engine gives no level, a dash, and why on pointing at it.
export function LevelMark({ level, raise }: { level: string | null; raise: number }) {
  if (level === null) {
    return (
      <span className="mono muted" title={NO_LEVEL.level}>
        {NO_LEVEL.shown}
      </span>
    );
  }
  const look = levelLook(level, raise);
  const keys: [string, string][] = [['level', level]];
  if (raise > 0) keys.push(['levels_above', String(raise)]);
  return (
    <span
      className="mono"
      style={{ color: `var(${look.colour})`, fontWeight: look.strong ? 'var(--weight-strong)' : undefined }}
      title={keysLine(keys)}
    >
      {levelWords(level, raise)}
    </span>
  );
}

/// The colours of lines added and removed, as tokens' names: the green and the red a diff draws added and removed text
/// in (`console/diff.tsx`), as small text takes them.
export const LINES_LOOKS = { added: '--green-text', removed: '--red-text' } as const;

/// Lines added and removed, `+210 −40`, each in its colour; on pointing at them, what they count by its keys, with the
/// files where they are given.
export function LinesChanged({ added, removed, files }: { added: number; removed: number; files?: number }) {
  const keys: [string, string][] = [
    ...(files === undefined ? [] : [['files', String(files)] as [string, string]]),
    ['added', String(added)],
    ['removed', String(removed)],
  ];
  return (
    <span className="mono lines-changed" title={keysLine(keys)}>
      <span style={{ color: `var(${LINES_LOOKS.added})` }}>{linesAdded(added)}</span>{' '}
      <span style={{ color: `var(${LINES_LOOKS.removed})` }}>{linesRemoved(removed)}</span>
    </span>
  );
}

/// The legend of a view: each mark it draws, as the table draws it, with its name and, where one is given, how many
/// of it the view has, `way back (3)`; on one line, wrapping only where the view's line has no room for it.
export function Legend({ marks, counts = {}, label = 'What the marks mean' }: {
  marks: MarkKey[];
  counts?: Partial<Record<MarkKey, number>>;
  label?: string;
}) {
  return (
    <ul className="legend" aria-label={label}>
      {marks.map((key) => (
        <li key={key}>
          {MARKS[key].draw()} {MARKS[key].name}
          {counts[key] !== undefined && ` (${counts[key]})`}
        </li>
      ))}
    </ul>
  );
}
