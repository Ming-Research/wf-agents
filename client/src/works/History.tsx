// A work's history, drawn as the flow's matrix is: the cells of the work's flow across in the order of the walk, each
// head the flow chart's own; the steps down, one to a row in the order they happened, the oldest at the top, each with
// its time at its left, the day with it where the day changes. Under the cells a row holds marks and never words: a
// run's mark in its cell's column, how it ended or stands; the owner's sign under each cell an act of the owner's ran or
// let go on; the outside's sign under each cell a write of the outside's ran. A run that is a way round has the way back
// along its row, in the accent, from the furthest cell after it whose write it ran for to its mark, with the arrow the
// flow's chart draws. At the right, one line of what the step did, cut where it is too long and whole on the pointer's
// tip. Which steps are rows, and what each says, are worked out in `placing.ts`. The table is as wide as what the window
// over its right leaves, so that the lines stay in view. The chart opens scrolled to its end, the newest step, and
// follows new steps while the owner is at the end.

import { type CSSProperties, useEffect, useLayoutEffect, useRef, useState } from 'react';

import type { Cell } from '../api';
import { BackArrow, CellHead, FLOW_WIDTHS, WIDTHS, WordsHead, useWheel } from '../Chart';
import { MARKS, type MarkKey, markName } from '../marks';
import { Arrow, Edge } from '../parts';
import { WIDTH_KEYS, useKeptWidth, useKeptWidths } from '../sizes';
import type { WorkPage } from '../works-api';
import { Parts } from './marks';
import { type HistoryRow, type RowMark } from './placing';
import { dayOf, fullTime, minuteOf, partsText } from './words';

/// The marks the history draws, in the order its legend lists them (`Legend` in marks.tsx): how a run ended, then how
/// an open one stands; the owner's sign and the outside's; and the way back.
export const HISTORY_MARKS: MarkKey[] = [
  'handedIn',
  'nothingNew',
  'abandoned',
  'running',
  'waiting',
  'atRest',
  'failed',
  'stopped',
  'people',
  'outside',
  'wayBack',
];

/// The width of the column of times at first, and the least and most it is dragged to.
const TIMES = { first: 96, min: 56, max: 200 };

/// How far a run's mark reaches from its middle, in the units of a glyph's box: half its side (`--space-8`) where the
/// box is `--chart-mark` wide in 24 units.
const RUN_REACH = 6;

/// Where a way back runs through a cell's column: into it from its right to its mark, at the run; through it; or out of
/// its middle to its left, at the furthest cell whose write it ran for.
function backAt(back: HistoryRow['back'], column: number): 'right' | 'through' | 'left' | null {
  if (back === null) return null;
  const furthest = back.from[back.from.length - 1];
  if (column === back.to) return 'right';
  if (column === furthest) return 'left';
  return column > back.to && column < furthest ? 'through' : null;
}

/// What a row's mark under a cell says, on the pointer's tip and to a reader of the screen.
function markWords(mark: RowMark, cell: string): string {
  if (mark.kind === 'run') return `${cell} run ${mark.run}: ${markName(mark.mark)}`;
  return `${cell}: ${markName(mark.kind === 'owner' ? 'people' : 'outside')}`;
}

/// A row's mark under a cell: a run's, its words there for a reader of the screen, or the owner's or the outside's sign,
/// which carries its words.
function RowMarkSign({ mark, words }: { mark: RowMark; words: string }) {
  if (mark.kind === 'run')
    return (
      <>
        {MARKS[mark.mark].draw(words)}
        <span className="visually-hidden">{words}</span>
      </>
    );
  return MARKS[mark.kind === 'owner' ? 'people' : 'outside'].draw(words);
}

export function HistoryChart({ page, rows, chosen, onChoose, covered }: {
  page: WorkPage;
  rows: HistoryRow[];
  /// The place among the steps of the step chosen; none where nothing is.
  chosen: number | null;
  onChoose: (step: number | null) => void;
  /// How much of the frame's right the window over it covers, in pixels; none while it is away.
  covered: number;
}) {
  const cells = page.flow.cells;
  const frame = useRef<HTMLDivElement>(null);
  const [lead, setLead] = useKeptWidth(WIDTH_KEYS.times, TIMES.min, TIMES.max);
  const [widths, setWidth] = useKeptWidths(WIDTH_KEYS.flowCells, FLOW_WIDTHS.cell, WIDTHS.cellMax);
  const [pointed, setPointed] = useState<number | null>(null);
  useWheel(frame);
  const leadWidth = lead ?? TIMES.first;
  const widthOf = (id: string) => widths[id] ?? FLOW_WIDTHS.cell;
  const taken = cells.reduce((sum, cell) => sum + widthOf(cell.id), leadWidth);
  const unpoint = () => setPointed(null);

  // The chart opens at its end, the newest step; it follows new steps while the owner is at the end, and stays where
  // the owner scrolled to otherwise.
  const atEnd = useRef(true);
  useLayoutEffect(() => {
    const el = frame.current;
    if (el && atEnd.current) el.scrollTop = el.scrollHeight;
  }, [rows.length]);
  const scrolled = () => {
    const el = frame.current;
    if (el) atEnd.current = el.scrollTop + el.clientHeight >= el.scrollHeight - 4;
  };
  // The step chosen, here, from the address or in the window, is brought into view.
  useEffect(() => {
    if (chosen === null) return;
    frame.current?.querySelector(`[data-key="step:${chosen}"]`)?.scrollIntoView({ block: 'nearest' });
  }, [chosen]);

  return (
    <div
      className="frame"
      ref={frame}
      onScroll={scrolled}
      onMouseLeave={unpoint}
      style={covered > 0 ? ({ '--window-covers': `${covered}px` } as CSSProperties) : undefined}
    >
      <table className="matrix flow history" style={{ width: '100%', minWidth: taken + FLOW_WIDTHS.words }}>
        <colgroup>
          <col style={{ width: leadWidth }} />
          {cells.map((cell) => (
            <col key={cell.id} style={{ width: widthOf(cell.id) }} />
          ))}
          <col />
        </colgroup>
        <thead>
          <tr>
            <th className="lead corner" scope="col" aria-label="Steps down the side, cells across" onMouseEnter={unpoint}>
              <span className="label corner-cells">
                Cells <Arrow to="right" />
              </span>
              <Edge
                side="right"
                label="Drag the width of the times' column"
                start={() => leadWidth}
                onDrag={setLead}
                onReset={() => setLead(null)}
              />
            </th>
            {cells.map((cell, c) => (
              <CellHead
                key={cell.id}
                cell={cell}
                chosen={false}
                pointed={pointed === c}
                onPoint={() => setPointed(c)}
                onUnpoint={unpoint}
                width={widthOf(cell.id)}
                onWidth={(w) => setWidth(cell.id, w)}
              />
            ))}
            <WordsHead
              about={pointed !== null ? (cells[pointed] ?? null) : null}
              saying={null}
              onPoint={unpoint}
              label="What each step did"
            />
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <StepRow
              key={row.key}
              row={row}
              cells={cells}
              chosen={chosen === row.index}
              pointed={pointed}
              onChoose={() => onChoose(chosen === row.index ? null : row.index)}
              onPoint={setPointed}
            />
          ))}
        </tbody>
      </table>
      {rows.length === 0 && <p className="message muted">Nothing has happened to the work yet</p>}
    </div>
  );
}

/// A step: its time, a mark under each cell it has one for, and the line of what it did.
function StepRow(props: {
  row: HistoryRow;
  cells: Cell[];
  chosen: boolean;
  pointed: number | null;
  onChoose: () => void;
  onPoint: (column: number | null) => void;
}) {
  const { row, cells, chosen, pointed, onChoose, onPoint } = props;
  const { step } = row;
  const did = partsText(row.did);
  const time = `${row.newDay ? `${dayOf(step.time)} ` : ''}${minuteOf(step.time)}`;
  return (
    <tr className={`row step-row ${chosen ? 'chosen' : ''}`} onClick={onChoose}>
      <th scope="row" className="lead step-lead" data-key={`step:${row.index}`} onMouseEnter={() => onPoint(null)}>
        <button
          type="button"
          className="head-button"
          aria-pressed={chosen}
          aria-label={`${fullTime(step.time)}: ${did}`}
          onClick={(e) => {
            e.stopPropagation();
            onChoose();
          }}
        >
          <span className="step-time" title={fullTime(step.time)}>
            {time}
          </span>
        </button>
      </th>
      {cells.map((cell, c) => {
        const mark = row.marks.get(c);
        const words = mark === undefined ? '' : markWords(mark, cell.id);
        const back = backAt(row.back, c);
        return (
          <td
            key={cell.id}
            className={`spot ${pointed === c ? 'pointed' : ''} ${mark ? 'has-mark' : ''}`}
            title={words || undefined}
            onMouseEnter={() => onPoint(c)}
          >
            {(back === 'left' || back === 'through') && <span className="seg seg-left back" aria-hidden="true" />}
            {(back === 'right' || back === 'through') && <span className="seg seg-right back" aria-hidden="true" />}
            {mark && (
              <span className="glyphs">
                <RowMarkSign mark={mark} words={words} />
              </span>
            )}
            {back === 'right' && <BackArrow reach={RUN_REACH} />}
          </td>
        );
      })}
      <td className="words" title={did || undefined} onMouseEnter={() => onPoint(null)}>
        <Parts parts={row.did} />
      </td>
    </tr>
  );
}
