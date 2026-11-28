// Every mark the page draws, in one table: what draws each, one part for each look, and its name, one or two words in
// the system's own terms; and the legend, which draws a list of them on one line. The flow's matrix, the flowchart, the
// strip of a work's cells, a work's history and the console each list in their legend the marks they
// draw, so that one thing has one name and one look in every view (Article 8). What the flow's files define is named by
// its key (principle definitions-shown-as-written); how a run ended or stands has one set of marks, which the strip
// draws for each cell, as its run stands, and the history for each run (`works/state-marks.ts`). Beside the table, the
// looks of the level a work's cell runs at and of a work's lines added and removed, which the page and the console's
// cards both draw from.

import type { ReactNode } from 'react';

import { ConditionGlyph, DataGlyphs, LineGlyph, ToolGlyph } from './Chart';
import { type Mark, NO_MARK } from './matrix';
import { TrackBlock, type TrackKind } from './console/track';
import { Sign, TICK } from './parts';
import { RUN_STATES, type RunState, lookStyle, pulses } from './works/state-marks';
import type { Held } from './works-api';
import { NO_LEVEL, REPLY_READ, keysLine, linesAdded, linesRemoved } from './works/words';

/// A mark the page draws: of the flow's definition, as its files' keys name it; of a line between cells; of who
/// writes a datum besides the cells; of how a run ended or stands.
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
  | 'sentBackWay'
  | 'mainPath'
  | 'fromOutside'
  | 'when'
  | 'writer'
  | 'people'
  | 'outside'
  | 'shown'
  | 'filesChanged'
  | RunState
  | TrackMark
  | ConsoleMark;

/// A colour of the time track the item window draws for a work, drawn here as a sample for the console's legend: a
/// model that ran, the tools that ran, and what the work waited for while nothing ran.
export type TrackMark = `track${Capitalize<TrackKind>}`;

/// The mark of each kind of stretch a track draws, so that the legend lists every colour the track has.
export const TRACK_MARKS: Record<TrackKind, TrackMark> = {
  model: 'trackModel',
  tool: 'trackTool',
  owner: 'trackOwner',
  paused: 'trackPaused',
  items: 'trackItems',
  turn: 'trackTurn',
  gap: 'trackGap',
  unknown: 'trackUnknown',
};

/// A mark of the console's tree, drawn on its canvas (`console/snap.ts`) and here as a sample for its legend: a card by
/// how its work stands, a shadow, the tags and rows of a card, what joins the cards, and the map of the columns.
export type ConsoleMark =
  | 'cardStopped'
  | 'cardRunning'
  | 'cardWaiting'
  | 'cardPaused'
  | 'cardAbove'
  | 'cardMerged'
  | 'cardCancelled'
  | 'cardElsewhere'
  | 'forYou'
  | 'answerAhead'
  | 'unreadReplies'
  | 'notTakenIn'
  | 'takenIn'
  | 'sentBack'
  | 'level'
  | 'cells'
  | 'lines'
  | 'age'
  | 'funnel'
  | 'folded'
  | 'litWay'
  | 'map';

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
/// cell that watches what it writes, thin in the colour of the data's lines; a way back, thick in the accent, and on a
/// work's history, where it is a send-back, in red; the main path, thick in the text's colour; an arrow from what is
/// written outside the cells, dashed.
function LineSample({ kind }: { kind: 'wakes' | 'back' | 'sent' | 'main' | 'outside' }) {
  return <span className={`legend-line ${kind}`} aria-hidden="true" />;
}

/// A node of the flowchart: the way into a cell, by its `when` and its `requires`; edged in the accent, a cell that
/// writes what the chosen cell takes.
function NodeSample({ lit = false }: { lit?: boolean }) {
  return <span className={`legend-node${lit ? ' lit' : ''}`} aria-hidden="true" />;
}

type CardLook = 'stopped' | 'running' | 'waiting' | 'paused' | 'above' | 'elsewhere';

/// A card of the console's tree, as a sample, by who moves it next, as the canvas draws it (`Tone`): filled red for
/// the owner, filled blue for the system; edged in yellow while it waits its turn; grey, plain, where it does not move
/// by itself, its edge in a light step of the most pressing tone under it; a shadow of an item shown elsewhere edged in
/// dashes.
function CardSample({ look }: { look: CardLook }) {
  return <span className={`legend-card ${look}`} aria-hidden="true" />;
}

/// An item's number on a card whose work ended, as a sample: green where it merged, a pale grey where it was cancelled,
/// its title then struck through.
function NumberSample({ look }: { look: 'merged' | 'cancelled' }) {
  return (
    <span aria-hidden="true">
      <span className={`mono legend-number ${look}`}>12</span>
      {look === 'cancelled' && <span className="legend-title cancelled"> title</span>}
    </span>
  );
}

/// How many times what a run handed in was sent back, as a work's history marks it on the run's cell: an arrow turning
/// back and the count, in red and strong, so that the runs sent back stand out; `says` is what it says on pointing at
/// it, none in a legend.
export function SentBackMark({ count, says }: { count: number; says?: string }) {
  return (
    <span className="sent-back" title={says} aria-label={says} aria-hidden={says === undefined}>
      <svg viewBox="0 0 10 10" width="10" height="10" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true">
        <path d="M8 1.5v5H2M4 4.5 2 6.5l2 2" />
      </svg>
      {count}
    </span>
  );
}

/// What joins a card to the items under it, as a sample: the grey funnel; the dot it folds into; or the lit way into a
/// running item, ending in the dot that ends it while the item is folded away.
function JoinSample({ look }: { look: 'funnel' | 'lit' }) {
  return look === 'funnel' ? (
    <svg className="legend-funnel" viewBox="0 0 16 12" width="16" height="12" aria-hidden="true">
      <path d="M0 3C8 3 8 0 16 0V12C8 12 8 9 0 9Z" />
    </svg>
  ) : (
    <svg className="legend-lit" viewBox="0 0 16 12" width="16" height="12" aria-hidden="true">
      <path d="M0 6H11" stroke="currentColor" strokeWidth="1.5" />
      <circle cx="12.5" cy="6" r="2.5" fill="currentColor" />
    </svg>
  );
}

/// How long a work has gone on, as a sample: grey under half an hour, amber to an hour, red past it.
function AgeSample() {
  return (
    <span className="mono" aria-hidden="true">
      <span style={{ color: 'var(--muted)' }}>12m</span> <span style={{ color: 'var(--orange-text)' }}>45m</span>{' '}
      <span style={{ color: 'var(--red)' }}>3h</span>
    </span>
  );
}

/// The flow's files in the directory shown differ from those the engine started with: a tag framed in grey, in the
/// text's colour, with the sign for unequal. It takes none of the colours that mean a state.
export function FilesChangedMark() {
  return (
    <span className="files-changed" aria-hidden="true">
      ≠
    </span>
  );
}

const data = (fields: Partial<Mark>) => <DataGlyphs mark={{ ...NO_MARK, ...fields }} />;

/// What draws a mark, with `words` said on pointing at it where its part says anything, and the mark's name.
type Entry = { name: string; draw: (words?: string) => ReactNode };

const ofRun = (state: RunState, name: string): Entry => ({
  name,
  draw: (words) => <StateMark state={state} title={words} />,
});

/// A sample of a colour of the time track: the track's own block in a small box, saying on pointing at it what the
/// colour is.
function TrackSample({ kind }: { kind: TrackKind }) {
  return (
    <span className="legend-track">
      <TrackBlock kind={kind} />
    </span>
  );
}

const ofTrack = (kind: TrackKind, name: string): Entry => ({ name, draw: () => <TrackSample kind={kind} /> });

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
  sentBackWay: { name: 'way back', draw: () => <LineSample kind="sent" /> },
  mainPath: { name: 'main path', draw: () => <LineSample kind="main" /> },
  fromOutside: { name: 'from outside', draw: () => <LineSample kind="outside" /> },
  when: { name: 'when', draw: () => <NodeSample /> },
  writer: { name: 'writer', draw: () => <NodeSample lit /> },
  people: { name: 'people', draw: (words) => <Sign name="people" words={words} /> },
  outside: { name: 'outside', draw: (words) => <Sign name="outside" words={words} /> },
  shown: { name: 'shown', draw: (words) => <Sign name="shown" words={words} /> },
  filesChanged: { name: 'files changed', draw: () => <FilesChangedMark /> },
  running: ofRun('running', 'running'),
  waiting: ofRun('waiting', 'waiting'),
  atRest: ofRun('atRest', 'at rest'),
  failed: ofRun('failed', 'failed'),
  stopped: ofRun('stopped', 'stopped'),
  handedIn: ofRun('handedIn', 'handed in'),
  again: ofRun('again', 'runs again'),
  notYet: ofRun('notYet', 'not yet'),
  skipped: ofRun('skipped', 'skipped'),
  notRun: ofRun('notRun', 'did not run'),
  trackModel: ofTrack('model', 'model ran'),
  trackTool: ofTrack('tool', 'tools ran'),
  trackOwner: ofTrack('owner', 'waits for you'),
  trackPaused: ofTrack('paused', 'pause'),
  trackItems: ofTrack('items', 'waits for items'),
  trackTurn: ofTrack('turn', 'waits for turn'),
  trackGap: ofTrack('gap', 'nothing ran'),
  trackUnknown: ofTrack('unknown', 'not known'),
  cardStopped: { name: 'you move it next', draw: () => <CardSample look="stopped" /> },
  cardRunning: { name: 'the system moves it', draw: () => <CardSample look="running" /> },
  cardWaiting: { name: 'waits its turn', draw: () => <CardSample look="waiting" /> },
  cardPaused: { name: 'does not move by itself', draw: () => <CardSample look="paused" /> },
  cardAbove: { name: 'edge: the most pressing under it', draw: () => <CardSample look="above" /> },
  cardMerged: { name: 'merged', draw: () => <NumberSample look="merged" /> },
  cardCancelled: { name: 'cancelled', draw: () => <NumberSample look="cancelled" /> },
  cardElsewhere: { name: 'shown elsewhere', draw: () => <CardSample look="elsewhere" /> },
  forYou: {
    name: 'for you',
    draw: () => (
      <span className="tag legend-for-you" aria-hidden="true">
        1
      </span>
    ),
  },
  answerAhead: {
    name: 'to answer ahead',
    draw: () => (
      <span className="tag legend-ahead" aria-hidden="true">
        1
      </span>
    ),
  },
  unreadReplies: {
    name: 'replies not read',
    draw: () => (
      <span className="tag legend-unread" aria-hidden="true">
        1
      </span>
    ),
  },
  notTakenIn: { name: 'not read or taken yet', draw: () => <ReceiptMark taken={false} words={REPLY_READ.unread} /> },
  takenIn: { name: 'read or taken', draw: () => <ReceiptMark taken words={REPLY_READ.read} /> },
  sentBack: { name: 'sent back', draw: () => <SentBackMark count={2} /> },
  level: { name: 'level', draw: () => <LevelMark level="2" raise={0} /> },
  cells: {
    name: 'cells',
    draw: () => (
      <span className="legend-cells" aria-hidden="true">
        <StateMark state="handedIn" />
        <StateMark state="running" />
        <StateMark state="notYet" />
      </span>
    ),
  },
  lines: { name: 'lines', draw: () => <LinesChanged added={210} removed={40} /> },
  age: { name: 'age', draw: () => <AgeSample /> },
  funnel: { name: 'items under it', draw: () => <JoinSample look="funnel" /> },
  folded: { name: 'folded', draw: () => <span className="dot" aria-hidden="true" /> },
  litWay: { name: 'lit way', draw: () => <JoinSample look="lit" /> },
  map: { name: 'map', draw: () => <span className="legend-map" aria-hidden="true" /> },
};

/// Whether the one a message is for has taken it in: a clock until it has, then the page's tick. On the owner's words it
/// says whether the cell they went to has taken them; on a cell's reply, whether the owner has read it. `words` says on
/// pointing which it is.
export function ReceiptMark({ taken, words }: { taken: boolean; words: string }) {
  return (
    <svg
      className={taken ? 'cw-taken' : 'cw-waiting'}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2.5"
      strokeLinecap="round"
      role="img"
      aria-label={words}
    >
      <title>{words}</title>
      {taken ? (
        <path d={TICK} />
      ) : (
        <>
          <circle cx="12" cy="12" r="8" />
          <path d="M12 7.5 V12 L15 14" />
        </>
      )}
    </svg>
  );
}

/// A mark's name.
export const markName = (key: MarkKey): string => MARKS[key].name;

/// The looks of the level a work's cell really runs at, its item's level raised as far as the cell's line gets it,
/// lowest first: the first muted, the second in the text's colour, the third in the accent, and each above it in the
/// accent and strong. Each colour is a token's name, as small text takes it (decision
/// a-level-looks-as-the-level-it-runs-at).
export const LEVEL_LOOKS = [
  { colour: '--muted', strong: false },
  { colour: '--text', strong: false },
  { colour: '--accent-text', strong: false },
  { colour: '--accent-text', strong: true },
] as const;

export type LevelLook = (typeof LEVEL_LOOKS)[number];

/// A level as a card and the item's window write it: `L2`, and `L2+1` where the cell really runs one level above it.
export const levelWords = (level: string, raise: number): string => (raise > 0 ? `L${level}+${raise}` : `L${level}`);

/// The look of the level a cell runs at, by the number the level's name is and how far it is raised; a level whose
/// name is no number takes the second.
export function levelLook(level: string, raise: number): LevelLook {
  const real = /^\d+$/.test(level) ? Number(level) + raise : 2;
  return LEVEL_LOOKS[Math.min(LEVEL_LOOKS.length, Math.max(1, real)) - 1];
}

/// What pointing at a work's level says, as the keys of the definitions name it: the item's `level`, the line's
/// `levels_above` where it asks for any, and, where a round was held below what its line asks, the setting that held it
/// with the level it held the cell at (`cellLevel`). Where the engine gives no level, why it gives none.
export function levelPointed(level: string | null, raise: number, held: Held | null, cellLevel: string | null): string {
  if (level === null) return NO_LEVEL.level;
  const keys: [string, string][] = [['level', level]];
  const asks = held === null ? raise : held.asks;
  if (asks > 0) keys.push(['levels_above', String(asks)]);
  if (held !== null && held.by !== null && cellLevel !== null) keys.push([held.by, cellLevel]);
  return keysLine(keys);
}

/// The level a work's cell runs at, `L2` or `L2+1`, in its look, saying on pointing at it what `levelPointed` says;
/// where the engine gives no level, a dash.
export function LevelMark({ level, raise, held = null, cellLevel = null }: {
  level: string | null;
  raise: number;
  held?: Held | null;
  cellLevel?: string | null;
}) {
  if (level === null) {
    return (
      <span className="mono muted" title={levelPointed(null, raise, held, cellLevel)}>
        {NO_LEVEL.shown}
      </span>
    );
  }
  const look = levelLook(level, raise);
  return (
    <span
      className="mono"
      style={{ color: `var(${look.colour})`, fontWeight: look.strong ? 'var(--weight-strong)' : undefined }}
      title={levelPointed(level, raise, held, cellLevel)}
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
