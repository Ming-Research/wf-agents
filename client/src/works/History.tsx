// A work's history, drawn as the flow's matrix is: the cells of the work's flow across in the order of the walk, each
// head the matrix's own; down the side the runs, one to a row in the order they happened, the oldest at the top. At the
// left of a row, what woke its run, named as the matrix names its rows, with the owner's sign, the outside's or the
// tool's mark (`placing.ts`). Under the cells, an arrow from the middle of the column of the cell whose run wrote what
// woke it to the run's own column, drawn as the matrix draws its ways back and in the accent where it runs back
// to an earlier cell, and in the run's column its mark, how it ended or stands, alone. At the right, how long the run
// took, or has been under way while it is open, in the colour a console card gives a work's age, and when it began on
// pointing at it; then what its latest round ran on, as the records of its try kept it: the level, in the look a level
// has, the reasoning effort and the model, none of a run of the engine's own cell. A run whose rounds ran on more than
// one says them all in the step's window. Where the work was paused, let on
// again, cancelled or reopened, a row half as high lies between the runs it fell between: a dashed line in the accent
// across the cells, and at the right what was done, and when on pointing at it. The table keeps its whole width under
// the window over its right, as the flow's chart does, and the frame scrolls on past what the window covers. The chart opens scrolled to its end, the newest step, and follows new
// steps while the owner is at the end.

import { type CSSProperties, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';

import type { Cell } from '../api';
import { CellHead, LineLine, RowArrow, WIDTHS, WordsHead, useWheel } from '../Chart';
import { matrix } from '../matrix';
import { LevelMark, MARKS, type MarkKey, markName } from '../marks';
import { age } from '../console/items';
import { Arrow, Edge } from '../parts';
import { WIDTH_KEYS, useKeptWidth, useKeptWidths } from '../sizes';
import type { WorkPage } from '../works-api';
import { type HistoryRow, type Named } from './placing';
import { colourOf } from './state-marks';
import { useNow } from './marks';
import { duration, fullTime, isOpen, partsText, ranLast, runTook } from './words';

/// The marks the history draws, in the order its legend lists them (`Legend` in marks.tsx): the arrow from a write to
/// the run it woke and the way back; the signs of who wrote what woke a run, and a tool's mark; how a run
/// ended, then how an open one stands.
export const HISTORY_MARKS: MarkKey[] = [
  'wakes',
  'wayBack',
  'people',
  'outside',
  'tools',
  'handedIn',
  'running',
  'waiting',
  'atRest',
  'failed',
  'again',
  'stopped',
];

/// The width of a cell's column at first, which holds one mark and the arrows through it, and the least it is dragged
/// to.
const CELLS = { first: 36, min: 28 };

/// The least width of the column at the right, which holds how long each run took and what it ran on.
const FACTS_MIN = 360;

/// The heads of what the column at the right holds, in the order its rows give them.
const FACTS = ['Took', 'Level', 'Effort', 'Model'];

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

/// How the arrow of a run's row runs through a cell's column: out of the writer's column's middle, through, or into the
/// run's mark, along the side it comes from; and whether it is a way back, to an earlier cell.
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
  const [widths, setWidth] = useKeptWidths(WIDTH_KEYS.historyCells, CELLS.min, WIDTHS.cellMax);
  const [pointed, setPointed] = useState<number | null>(null);
  // The flow's lines, each the matrix's own row of it, under the cells' heads.
  const lines = useMemo(() => matrix(page.flow).lines, [page.flow]);
  // The flow's groups, whose names a row writes with the word `group` after them, as the matrix does.
  const groups = useMemo(() => new Set(page.flow.groups.map((g) => g.name)), [page.flow]);
  // How long an open run has been under way counts on each second while one is.
  const now = useNow(rows.some((r) => r.step.kind === 'run' && isOpen(r.step)));
  useWheel(frame);
  const leadWidth = lead ?? NAMES.first;
  const widthOf = (id: string) => widths[id] ?? CELLS.first;
  const taken = cells.reduce((sum, cell) => sum + widthOf(cell.id), leadWidth);
  // Under the window the table keeps its whole width, as the flow's chart does (styles/chart.css).
  const size = { width: 'calc(100% + var(--window-covers, 0px))', minWidth: taken + FACTS_MIN };
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
              label={
                <span className="run-facts">
                  {FACTS.map((f) => (
                    <span key={f}>{f}</span>
                  ))}
                </span>
              }
            />
          </tr>
          {lines.map((row) => (
            <LineLine
              key={row.name}
              row={row}
              chosen={false}
              onChoose={() => undefined}
              cells={cells}
              chosenCell={null}
              saying={null}
              pointed={pointed}
              onPoint={setPointed}
            />
          ))}
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
                groups={groups}
                chosen={chosen === row.index}
                pointed={pointed}
                onChoose={() => onChoose(chosen === row.index ? null : row.index)}
                onPoint={setPointed}
                now={now}
              />
            ),
          )}
        </tbody>
      </table>
      {rows.length === 0 && <p className="message muted">Nothing has happened to the work yet</p>}
    </div>
  );
}

/// Where the work was paused, let on again, cancelled or reopened, between two runs: a line across the cells, and at
/// the right what was done; when, on pointing at it.
function DividerRow({ row, span }: { row: HistoryRow; span: number }) {
  const { step } = row;
  const did = partsText(row.did);
  return (
    <tr className="divider-row" title={`${fullTime(step.time)}: ${did}`}>
      <th scope="row" className="lead">
        <span className="visually-hidden">{did}</span>
      </th>
      <td className="divider-line" colSpan={span} aria-hidden="true" />
      <td className="words">
        <span className="divider-words">{row.divider}</span>
      </td>
    </tr>
  );
}

/// A run: what woke it, the arrow and its mark under the cells, how long it took, coloured as a card colours a work's
/// age, and what it ran on.
function StepRow(props: {
  row: HistoryRow;
  cells: Cell[];
  /// The names of the flow's groups.
  groups: ReadonlySet<string>;
  chosen: boolean;
  pointed: number | null;
  onChoose: () => void;
  onPoint: (column: number | null) => void;
  /// The time now, against which an open run's time under way is counted.
  now: number;
}) {
  const { row, cells, groups, chosen, pointed, onChoose, onPoint, now } = props;
  const { step } = row;
  const did = partsText(row.did);
  const ms = step.kind === 'run' ? runTook(step, now) : null;
  const ran = step.kind === 'run' ? ranLast(step) : null;
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
              {groups.has(n.name) && <span className="row-label">group</span>}
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
            className={[
              'spot',
              pointed === c ? 'pointed' : '',
              runHere || fromHere ? 'has-mark' : '',
              runHere ? 'ran' : '',
            ].join(' ')}
            style={runHere ? ({ '--ran': colourOf(runHere.state) } as CSSProperties) : undefined}
            title={words || undefined}
            onMouseEnter={() => onPoint(c)}
          >
            {line?.left && <span className={`seg seg-left ${kind}`} aria-hidden="true" />}
            {line?.right && <span className={`seg seg-right ${kind}`} aria-hidden="true" />}
            {runHere && <span className="glyphs">{MARKS[runHere.state].draw(words)}</span>}
            {runHere && row.from !== null && (
              <RowArrow reach={RUN_REACH} from={row.from.column > runHere.column ? 'right' : 'left'} />
            )}
            {words && <span className="visually-hidden">{words}</span>}
          </td>
        );
      })}
      <td className="words" title={`Began ${fullTime(step.time)}`} onMouseEnter={() => onPoint(null)}>
        <span className="run-facts">
          <span className={ms === null ? '' : `step-time ${age(ms).tone}`}>{ms !== null && duration(ms)}</span>
          <span>{ran !== null && <LevelMark level={ran.level} raise={0} />}</span>
          <span className="muted" title={ran?.effort ? `effort ${ran.effort}` : undefined}>
            {ran?.effort}
          </span>
          <span className="mono muted" title={ran ? `model ${ran.model}` : undefined}>
            {ran?.model}
          </span>
        </span>
      </td>
    </tr>
  );
}
