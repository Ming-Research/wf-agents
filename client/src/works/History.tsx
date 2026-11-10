// A work's history as a chart, drawn as the flow's chart is: the cells across in the order of the walk, each head with
// how the cell stands now; the steps down, one to a row in the order they happened, each with its time and why at the
// left. A run is in its cell's column, saying how it stands, how many rounds it had, how long it took, what it cost
// and what it wrote; what is not a run is a row of its own, its words at the left and, for a write, a ring where each
// cell that reads the block stands. The oldest step is at the top, as the walk goes down the flow's chart, and the
// chart opens scrolled to its end, the newest step.

import { type CSSProperties, useLayoutEffect, useMemo, useRef, useState } from 'react';

import type { Flow } from '../api';
import { Breakable, MarkGlyph, WIDTHS, tableWidth, useWheel } from '../Chart';
import type { Spot } from '../matrix';
import { Edge, Sign } from '../parts';
import { WIDTH_KEYS, useKeptWidth, useKeptWidths } from '../sizes';
import type { CellNow, Step, WorkPage } from '../works-api';
import { type HistoryRow, type RowMark, placeSteps } from './placing';
import { StateMark, cellNowWords } from './marks';
import {
  RUN_WORDS,
  STATE_WORDS,
  dayOf,
  dollars,
  duration,
  fullTime,
  isWayRound,
  minuteOf,
  runState,
  runTook,
} from './words';

/// What the owner chose on the chart: a step by its place among the steps, a cell, or nothing.
export type Chosen = { step: number } | { cell: string } | null;

/// The width of the column of times and words at first, and the least and most it is dragged to.
const STEPS = { first: 260, min: 160, max: 520 };

/// A mark of the flow's chart, for a row of its own: a ring where a cell reads what was written, a diamond where its
/// condition does.
function spotOf(mark: RowMark): Spot {
  const none = { read: false, write: false, condition: false };
  return {
    mark: { ...none, read: mark === 'read', condition: mark === 'condition' },
    added: none,
    removed: none,
    line: 'none',
    back: 'none',
    backTo: false,
  };
}

/// The words of a row at its left: why a run ran, or what was done.
export function stepWords(step: Step): string {
  switch (step.kind) {
    case 'run': {
      if (isWayRound(step)) {
        return `way round for ${step.because.flatMap((c) => (c.kind === 'changed' ? [c.block] : [])).join(', ')}`;
      }
      const changed = step.because.flatMap((c) => (c.kind === 'changed' ? [c.block] : []));
      if (changed.length > 0) return `for ${changed.join(', ')}`;
      return 'first run';
    }
    case 'wrote': {
      const who = step.by.kind === 'person' ? 'a person' : step.by.kind === 'outside' ? 'the outside' : `run ${step.by.run}`;
      return `${who} wrote ${step.block}${step.value !== null ? ` = ${step.value}` : ''}${step.changed ? '' : ', same meaning'}`;
    }
    case 'letGo':
      return `the owner let ${step.cell} go on`;
    case 'paused':
      return 'the owner paused the work';
    case 'resumed':
      return 'the owner let the work on';
    case 'cancelled':
      return 'the owner cancelled the work';
  }
}

export function HistoryChart({ page, chosen, onChoose }: {
  page: WorkPage;
  chosen: Chosen;
  onChoose: (chosen: Chosen) => void;
}) {
  const { flow, steps } = page;
  const rows = useMemo(() => placeSteps(steps, flow, page.blocks), [steps, flow, page.blocks]);
  const frame = useRef<HTMLDivElement>(null);
  const [lead, setLead] = useKeptWidth(WIDTH_KEYS.steps, STEPS.min, STEPS.max);
  const [widths, setWidth] = useKeptWidths(WIDTH_KEYS.historyCells, WIDTHS.cellMin, WIDTHS.cellMax);
  const [pointed, setPointed] = useState<number | null>(null);
  useWheel(frame);
  const leadWidth = lead ?? STEPS.first;
  const size = tableWidth(
    leadWidth,
    flow.cells.map((c) => c.id),
    widths,
  );
  const now = Date.now();

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

  const chosenCell = chosen && 'cell' in chosen ? chosen.cell : null;
  const chosenStep = chosen && 'step' in chosen ? chosen.step : null;
  const states = new Map(page.summary.cells.map((c) => [c.cell, c]));

  return (
    <div className="frame" ref={frame} onScroll={scrolled} onMouseLeave={() => setPointed(null)}>
      <table className="matrix history" style={size as CSSProperties}>
        <colgroup>
          <col style={{ width: leadWidth }} />
          {flow.cells.map((c) => (
            <col key={c.id} style={widths[c.id] !== undefined ? { width: widths[c.id] } : undefined} />
          ))}
        </colgroup>
        <thead>
          <tr>
            <th className="lead corner" scope="col" aria-label="Steps down the side, cells across">
              <span className="label corner-cells">Cells →</span>
              <span className="label corner-blocks">Steps ↓</span>
              <Edge
                side="right"
                label="Drag the width of the steps' column"
                start={() => leadWidth}
                onDrag={setLead}
                onReset={() => setLead(null)}
              />
            </th>
            {flow.cells.map((c, i) => (
              <HistoryHead
                key={c.id}
                flow={flow}
                cell={c.id}
                now={states.get(c.id)}
                chosen={chosenCell === c.id}
                pointed={pointed === i}
                onChoose={() => onChoose(chosenCell === c.id ? null : { cell: c.id })}
                onPoint={() => setPointed(i)}
                onWidth={(w) => setWidth(c.id, w)}
              />
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <StepRow
              key={row.key}
              row={row}
              flow={flow}
              now={now}
              chosen={chosenStep === row.index}
              chosenCell={chosenCell}
              pointed={pointed}
              onChoose={() => onChoose(chosenStep === row.index ? null : { step: row.index })}
              onPoint={setPointed}
            />
          ))}
        </tbody>
      </table>
      {rows.length === 0 && <p className="message muted">Nothing has happened to the work yet</p>}
    </div>
  );
}

/// The head of a cell's column: its id, its title, and how it stands for the work now.
function HistoryHead(props: {
  flow: Flow;
  cell: string;
  now: CellNow | undefined;
  chosen: boolean;
  pointed: boolean;
  onChoose: () => void;
  onPoint: () => void;
  onWidth: (width: number | null) => void;
}) {
  const { flow, cell, now, chosen, pointed, onChoose, onPoint, onWidth } = props;
  const head = useRef<HTMLTableCellElement>(null);
  const c = flow.cells.find((x) => x.id === cell);
  return (
    <th
      scope="col"
      ref={head}
      className={`cell-head ${chosen ? 'chosen' : ''} ${pointed ? 'pointed' : ''}`}
      data-key={`cell:${cell}`}
      onMouseEnter={onPoint}
    >
      <button type="button" className="head-button" aria-pressed={chosen} onClick={onChoose}>
        <span className="cell-id">
          <Breakable name={cell} />
        </span>
        <span className="cell-title">{c?.title ?? cell}</span>
        {now && (
          <span className={`cell-state state-text-${now.state}`} title={cellNowWords(now)}>
            <StateMark state={now.state} /> {STATE_WORDS[now.state]}
            {now.finished > 0 && <span className="muted"> · {now.finished}</span>}
          </span>
        )}
      </button>
      <Edge
        side="right"
        label={`Drag the width of ${cell}'s column`}
        start={() => head.current?.getBoundingClientRect().width ?? WIDTHS.cellShare}
        onDrag={onWidth}
        onReset={() => onWidth(null)}
      />
    </th>
  );
}

function StepRow(props: {
  row: HistoryRow;
  flow: Flow;
  now: number;
  chosen: boolean;
  chosenCell: string | null;
  pointed: number | null;
  onChoose: () => void;
  onPoint: (column: number) => void;
}) {
  const { row, flow, now, chosen, chosenCell, pointed, onChoose, onPoint } = props;
  const { step } = row;
  const words = stepWords(step);
  const wayRound = step.kind === 'run' && isWayRound(step);
  const sign = step.kind === 'wrote' && step.by.kind !== 'run' ? (step.by.kind === 'person' ? 'people' : 'outside') : null;
  return (
    <tr className={`row step-row step-${step.kind} ${chosen ? 'chosen' : ''}`} onClick={onChoose}>
      <th scope="row" className="lead step-lead">
        <button
          type="button"
          className="head-button"
          aria-pressed={chosen}
          onClick={(e) => {
            e.stopPropagation();
            onChoose();
          }}
        >
          <span className="step-time" title={fullTime(step.time)}>
            {row.newDay ? `${dayOf(step.time)} ` : ''}
            {minuteOf(step.time)}
          </span>
          {sign && <Sign name={sign} />}
          <span className={`step-words ${wayRound ? 'way-back' : ''}`} title={words}>
            {step.kind === 'run' && <span className="mono muted">{step.run} </span>}
            {words}
          </span>
        </button>
      </th>
      {flow.cells.map((c, i) => {
        const back =
          row.backFrom !== null && row.column !== null && i > row.column && i <= row.backFrom
            ? i === row.backFrom
              ? 'back-from'
              : 'back-through'
            : '';
        const mark = row.marks.get(i);
        return (
          <td
            key={c.id}
            className={`spot step-spot ${back} ${chosenCell === c.id ? 'chosen' : ''} ${pointed === i ? 'pointed' : ''} ${
              mark ? 'has-mark' : ''
            }`}
            onMouseEnter={() => onPoint(i)}
          >
            {row.column === i && step.kind === 'run' && <RunBox run={step} now={now} back={row.backFrom !== null} />}
            {mark && mark !== 'letGo' && (
              <>
                <MarkGlyph spot={spotOf(mark)} />
                <span className="visually-hidden">
                  {mark === 'read' ? `${c.id} reads it` : `${c.id}'s condition reads it`}
                </span>
              </>
            )}
            {mark === 'letGo' && <span className="let-go-mark">let go</span>}
            {back && <span className="seg-back" aria-hidden="true" />}
          </td>
        );
      })}
    </tr>
  );
}

/// A run in its cell's column: how it stands and how many rounds it had; how long it took and what it cost; what it
/// wrote.
function RunBox({ run, now, back }: { run: Step & { kind: 'run' }; now: number; back: boolean }) {
  const state = runState(run);
  const writes = run.end?.kind === 'finished' ? run.end.writes.map((w) => w.block) : [];
  const facts = `${duration(runTook(run, now))} · ${dollars(run.spent.usdMicros)}`;
  const title = [
    `run ${run.run} of ${run.cell}: ${RUN_WORDS[state]}, ${run.rounds.length} rounds`,
    facts,
    writes.length > 0 ? `wrote ${writes.join(', ')}` : '',
  ]
    .filter(Boolean)
    .join('\n');
  return (
    <span className={`run run-${state}`} title={title}>
      {back && <span className="run-arrow" aria-hidden="true" />}
      <span className="run-line">
        <span className="run-word">{RUN_WORDS[state]}</span>
        <span className="muted"> {run.rounds.length} round{run.rounds.length === 1 ? '' : 's'}</span>
      </span>
      <span className="run-line muted">{facts}</span>
      {writes.length > 0 && <span className="run-line mono run-writes">{writes.join(' ')}</span>}
    </span>
  );
}
