// A work's history, drawn as the flow's matrix is: the cells of the work's flow across in the order of the walk, each
// head the matrix's own; down the side the runs, one to a row in the order they happened, the oldest at the top. At the
// left of a row, what woke its run, named as the matrix names its rows, with the owner's sign, the outside's or the
// tool's mark (`placing.ts`). Under the cells, the dot of `outputs` under the cell whose run wrote what woke it, an
// arrow from it to the run's own column, drawn as the matrix draws its ways back and in the accent where it runs back
// to an earlier cell, and in the run's column its mark, how it ended or stands, alone. At the right, its time, the day
// with it where the day changes. Where the work was paused, let on again, cancelled or reopened, a row half as high
// lies between the runs it fell between: a dashed line in the accent across the cells, and at the right what was done
// and when. The table keeps its whole width under the window over its right, as the flow's chart does, and the
// frame scrolls on past what the window covers. The chart opens scrolled to its end, the newest step, and follows new
// steps while the owner is at the end.

import { type CSSProperties, useEffect, useLayoutEffect, useRef, useState } from 'react';

import type { Cell } from '../api';
import { CellHead, FLOW_WIDTHS, RowArrow, WIDTHS, WordsHead, useWheel } from '../Chart';
import { MARKS, type MarkKey, markName } from '../marks';
import { Arrow, Edge } from '../parts';
import { WIDTH_KEYS, useKeptWidth, useKeptWidths } from '../sizes';
import type { WorkPage } from '../works-api';
import { type HistoryRow, type Named } from './placing';
import { dayOf, fullTime, minuteOf, partsText } from './words';

/// The marks the history draws, in the order its legend lists them (`Legend` in marks.tsx): the chain, a write's dot,
/// the arrow to the run it woke and the way back; the signs of who wrote what woke a run, and a tool's mark; how a run
/// ended, then how an open one stands.
export const HISTORY_MARKS: MarkKey[] = [
  'outputs',
  'wakes',
  'wayBack',
  'people',
  'outside',
  'tools',
  'handedIn',
  'nothingNew',
  'abandoned',
  'running',
  'waiting',
  'atRest',
  'failed',
  'stopped',
];

/// The width of the column of names at first, and the least and most it is dragged to.
const NAMES = { first: 240, min: WIDTHS.namesMin, max: WIDTHS.namesMax };

/// How far a run's mark reaches from its middle, in the units of a glyph's box: half its side (`--space-8`) where the
/// box is `--chart-mark` wide in 24 units.
const RUN_REACH = 6;

/// What a name's mark says on pointing at it: who wrote it or acted.
const WHO: Record<NonNullable<Named['mark']>, string | undefined> = {
  people: 'the owner',
  outside: 'the outside',
  tools: undefined,
};

/// How the arrow of a run's row runs through a cell's column: out of the dot's middle, through, or into the run's
/// mark, along the side it comes from; and whether it is a way back, to an earlier cell.
function lineAt(row: HistoryRow, column: number): { left: boolean; right: boolean; back: boolean } | null {
  if (row.run === null || row.from === null) return null;
  const [low, high] = [Math.min(row.from.column, row.run.column), Math.max(row.from.column, row.run.column)];
  if (column < low || column > high) return null;
  return { left: column > low, right: column < high, back: row.from.column > row.run.column };
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
  const [lead, setLead] = useKeptWidth(WIDTH_KEYS.historyNames, NAMES.min, NAMES.max);
  const [widths, setWidth] = useKeptWidths(WIDTH_KEYS.flowCells, FLOW_WIDTHS.cell, WIDTHS.cellMax);
  const [pointed, setPointed] = useState<number | null>(null);
  useWheel(frame);
  const leadWidth = lead ?? NAMES.first;
  const widthOf = (id: string) => widths[id] ?? FLOW_WIDTHS.cell;
  const taken = cells.reduce((sum, cell) => sum + widthOf(cell.id), leadWidth);
  // Under the window the table keeps its whole width, as the flow's chart does (styles/chart.css).
  const size = { width: 'calc(100% + var(--window-covers, 0px))', minWidth: taken + FLOW_WIDTHS.words };
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
      <table className="matrix flow history" style={size as CSSProperties}>
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
                label="Drag the width of the names' column"
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
              label="When"
            />
          </tr>
        </thead>
        <tbody>
          {rows.map((row) =>
            row.divider !== null ? (
              <DividerRow key={row.key} row={row} span={cells.length} />
            ) : (
              <StepRow
                key={row.key}
                row={row}
                cells={cells}
                chosen={chosen === row.index}
                pointed={pointed}
                onChoose={() => onChoose(chosen === row.index ? null : row.index)}
                onPoint={setPointed}
              />
            ),
          )}
        </tbody>
      </table>
      {rows.length === 0 && <p className="message muted">Nothing has happened to the work yet</p>}
    </div>
  );
}

/// A step: what woke it or the act, the dot, the arrow and the run's mark under the cells, and its time.
/// Where the work was paused, let on again, cancelled or reopened, between two runs: a line across the cells, and at the
/// right what was done and when.
function DividerRow({ row, span }: { row: HistoryRow; span: number }) {
  const { step } = row;
  const time = `${row.newDay ? `${dayOf(step.time)} ` : ''}${minuteOf(step.time)}`;
  const did = partsText(row.did);
  return (
    <tr className="divider-row" title={`${fullTime(step.time)}: ${did}`}>
      <th scope="row" className="lead">
        <span className="visually-hidden">{did}</span>
      </th>
      <td className="divider-line" colSpan={span} aria-hidden="true" />
      <td className="words">
        <span className="divider-words">
          {row.divider} {time}
        </span>
      </td>
    </tr>
  );
}

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
      <th
        scope="row"
        className="lead step-lead"
        data-key={`step:${row.index}`}
        title={did || undefined}
        onMouseEnter={() => onPoint(null)}
      >
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
          {row.names.map((n) => (
            <span key={`${n.name} ${n.mark}`} className="step-name">
              <span className="block-name">{n.name}</span>
              {n.mark !== null && MARKS[n.mark].draw(WHO[n.mark])}
            </span>
          ))}
        </button>
      </th>
      {cells.map((cell, c) => {
        const line = lineAt(row, c);
        const runHere = row.run !== null && row.run.column === c ? row.run : null;
        const fromHere = row.from !== null && row.from.column === c ? row.from : null;
        const words = runHere ? `${runHere.words}: ${markName(runHere.state)}` : (fromHere?.words ?? '');
        const kind = line?.back ? 'back' : 'wakes';
        return (
          <td
            key={cell.id}
            className={`spot ${pointed === c ? 'pointed' : ''} ${runHere || fromHere ? 'has-mark' : ''}`}
            title={words || undefined}
            onMouseEnter={() => onPoint(c)}
          >
            {line?.left && <span className={`seg seg-left ${kind}`} aria-hidden="true" />}
            {line?.right && <span className={`seg seg-right ${kind}`} aria-hidden="true" />}
            {fromHere && MARKS.outputs.draw()}
            {runHere && <span className="glyphs">{MARKS[runHere.state].draw(words)}</span>}
            {runHere && row.from !== null && (
              <RowArrow reach={RUN_REACH} from={row.from.column > runHere.column ? 'right' : 'left'} />
            )}
            {words && <span className="visually-hidden">{words}</span>}
          </td>
        );
      })}
      <td className="words" title={fullTime(step.time)} onMouseEnter={() => onPoint(null)}>
        <span className="step-time">{time}</span>
      </td>
    </tr>
  );
}
