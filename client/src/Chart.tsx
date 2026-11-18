// The chart: the matrix of the data, one to a row, against the cells, one to a column in the order of the walk. It is
// how the owner checks the flow (Article 7), so every cell and every datum is on it, and what a cell does with a datum
// is a mark where they cross, named by the key of the cell's file: a ring where the cell `watches` it, a new version
// running the cell; the ring with the kind inside where it only `reads` it; a disc where it may write it (`outputs`). A
// group a cell watches by its name has a row of its own over its first block, which its watchers' rings stand on. A
// later cell's write of a datum a cell watches is the work going back: its line, the only line along a row of the data,
// is drawn in the accent leftwards along the row to each cell it sends the work back to, with an arrow whose point is
// on the edge of that cell's mark, so that every way back shows at a glance. The rows' names stay in place as the chart
// scrolls sideways, and the cells' heads as it scrolls down, in the chart's own frame.
//
// A cell's column holds only marks, so it is only as wide as they are, and its head holds the cell's id alone. The
// width this leaves, right of the marks, is a column of words, one line to a row: what the row's thing is, or, on the
// rows of the cell chosen, what that cell says of it. What a head has no room for, the cell's title, its line, when it
// runs and what it waits for, stands over that column for the cell under the pointer. A work's history, drawn as this
// chart, takes its cells' heads, the head of its last column and the arrow at a run's mark from here
// (`works/History.tsx`).
//
// Over the data stand bands of rows, each thing drawn once: the lines, one to a line the agents' cells name, with a dot
// under each cell on it, the dots joined along the row, so that a joined row reads as one session going on; the
// conditions, one to a named test or a block or datum a key of a cell's `when` or `requires` names, each key of a cell
// marked in its own row: for `when`, a tick where the cell runs when the test holds or the block is yes, a cross where
// it runs when the block is no, and the value itself where it waits for another; for `requires`, a square, which the
// work waits before until it holds, its value in its words; a datum's row there is its one row, the marks of what each
// cell's file says of it beside the condition's and its way back along it; the engine's tools, one to a tool, a small
// square under each cell that may call it, since a call is a step of the work; and what `[every_agent]` names, one to a
// name as `flow.toml` writes it, under each cell what runs it there and what its own file says of those data, the row's
// words saying what every agent's cell only reads. A band's row is tagged with its kind, and a datum's is named with
// its values and signs; the corner names only the cells.

import { type CSSProperties, type ReactNode, type RefObject, useEffect, useRef, useState } from 'react';

import type { Block, Cell, InputKind } from './api';
import {
  type BlockRow,
  type ConditionRow,
  type ConditionSpot,
  type EveryRow,
  type LineRow,
  type LineSpot,
  type Mark,
  type Matrix,
  type Spot,
  type ToolRow,
} from './matrix';
import type { MarkKey } from './marks';
import { Arrow, CROSS, Edge, Sign, TICK } from './parts';
import { WIDTH_KEYS, useKeptWidth, useKeptWidths } from './sizes';
import {
  cellSays,
  everyIs,
  everyMarkWords,
  everySays,
  groupSays,
  headFacts,
  keysWords,
  lineKeys,
  listed,
  markWords,
} from './words';

/// What the owner chose: a cell, a block, a line, a named test, one of the engine's tools, a group of blocks, what
/// `[every_agent]` names, or nothing.
export type Chosen =
  | { cell: string }
  | { block: string }
  | { line: string }
  | { test: string }
  | { tool: string }
  | { group: string }
  | { everyAgent: true }
  | null;

/// The key a chosen thing's head is found by on the chart; none for what `[every_agent]` names, a band of rows.
function keyOf(chosen: NonNullable<Chosen>): string | null {
  if ('cell' in chosen) return `cell:${chosen.cell}`;
  if ('block' in chosen) return `block:${chosen.block}`;
  if ('line' in chosen) return `line:${chosen.line}`;
  if ('test' in chosen) return `test:${chosen.test}`;
  if ('group' in chosen) return `group:${chosen.group}`;
  if ('tool' in chosen) return `tool:${chosen.tool}`;
  return null;
}

/// What a kind of what a cell is given is drawn with inside the ring of `reads`: the latest version, the last and the
/// latest, every version.
const KIND_SIGNS: Record<InputKind, string> = {
  latest: '1',
  lastAndLatest: '2',
  all: '+',
};

/// A mark of the chart, drawn in a box of 24 units at the mark's size; what it draws past its box, as an arrow, shows.
const glyph = (key: string, child: ReactNode) => (
  <svg key={key} className="glyph" viewBox="0 0 24 24" aria-hidden="true">
    {child}
  </svg>
);

/// How far each mark of a datum reaches from the middle of its box, in its units: the disc to its edge, and each ring
/// to the outer edge of its line, half the line's width past its radius.
const REACH = { write: 6, watch: 7.5, read: 9 } as const;

/// The arrow of a way back, in a box's units: as long and half as wide as the tokens `--chart-arrow-length` and
/// `--chart-arrow` say at the mark's size.
const ARROW = { length: 9, half: 6 } as const;

/// The arrow of a line along a row, pointing at the mark it runs to, its point on the mark's edge, `reach` from its
/// middle: coming from the right, as a way back does, or from the left.
function arrowAt(reach: number, from: 'right' | 'left' = 'right'): ReactNode {
  const side = from === 'right' ? 1 : -1;
  const tip = 12 + side * reach;
  const base = tip + side * ARROW.length;
  return <polygon className="part arrow" points={`${tip},12 ${base},${12 - ARROW.half} ${base},${12 + ARROW.half}`} />;
}

/// The arrow of a line along a row alone, over a mark that is none of the chart's glyphs, as a run's in a work's
/// history: its point `reach` units of a glyph's box from the mark's middle; coming from the right, a way back, in the
/// accent; from the left, an arrow a write runs, in the colour of the data's lines.
export function RowArrow({ reach, from }: { reach: number; from: 'right' | 'left' }) {
  return (
    <svg className={`glyph row-arrow ${from === 'right' ? 'back' : 'wakes'}`} viewBox="0 0 24 24" aria-hidden="true">
      {arrowAt(reach, from)}
    </svg>
  );
}

/// The marks of a datum's crossing, one for each key, in the order drawn: a disc where the cell writes the datum
/// (`outputs`); a ring where it watches it (`watches`), or the ring with the kind inside where it only reads it
/// (`reads`), as `[kinds]` writes it; the last with the arrow of a way back where one runs the cell again.
function dataParts(mark: Pick<Mark, 'write' | 'watch' | 'read' | 'kind'>, arrow: boolean): ReactNode[] {
  const parts: [keyof typeof REACH, ReactNode][] = [];
  if (mark.write) parts.push(['write', <circle className="part write" cx="12" cy="12" r="6" />]);
  if (mark.watch) parts.push(['watch', <circle className="part watch" cx="12" cy="12" r="6.5" />]);
  if (mark.read) {
    parts.push([
      'read',
      <>
        <circle className="part read" cx="12" cy="12" r="8" />
        {mark.kind && (
          <text className="part kind" x="12" y="12">
            {KIND_SIGNS[mark.kind]}
          </text>
        )}
      </>,
    ]);
  }
  return parts.map(([key, child], i) =>
    glyph(
      key,
      arrow && i === parts.length - 1 ? (
        <>
          {child}
          {arrowAt(REACH[key])}
        </>
      ) : (
        child
      ),
    ),
  );
}

/// The marks of a crossing, one for each key, side by side (`dataParts`).
export function DataGlyphs({ mark }: { mark: Pick<Mark, 'write' | 'watch' | 'read' | 'kind'> }) {
  const parts = dataParts(mark, false);
  return parts.length > 0 ? <span className="glyphs">{parts}</span> : null;
}

/// The mark of a cell on a line: a dot in the accent, as the lines' names are.
export function LineGlyph({ on }: { on: boolean }) {
  if (!on) return null;
  return (
    <svg className="glyph" viewBox="0 0 24 24" aria-hidden="true">
      <circle className="part on-line" cx="12" cy="12" r="4.5" />
    </svg>
  );
}

/// The mark of a cell that may call one of the engine's tools: a small square, in the colour of the text.
export function ToolGlyph({ on }: { on: boolean }) {
  if (!on) return null;
  return (
    <svg className="glyph" viewBox="0 0 24 24" aria-hidden="true">
      <rect className="part tool" x="7.5" y="7.5" width="9" height="9" />
    </svg>
  );
}

/// The marks of a cell on a condition: where a key of its `when` is on the row, a tick where the cell runs when the
/// test holds or the block is yes, a cross where it runs when the block is no, and the value itself where it runs on
/// another; a square where a key of its `requires` is, which the work waits before the cell until it holds.
export function ConditionGlyph({ spot }: { spot: ConditionSpot }) {
  const parts = conditionParts(spot);
  return parts.length > 0 ? <span className="glyphs">{parts}</span> : null;
}

/// The marks of a cell on a condition, in the order drawn (`ConditionGlyph`).
function conditionParts({ when, value, requires }: ConditionSpot): ReactNode[] {
  const parts: ReactNode[] = [];
  if (when) {
    if (value === undefined || value === 'yes') parts.push(glyph('when', <path className="part holds" d={TICK} />));
    else if (value === 'no') parts.push(glyph('when', <path className="part holds" d={CROSS} />));
    else {
      parts.push(
        <span key="when" className="spot-value">
          {value}
        </span>,
      );
    }
  }
  if (requires) {
    parts.push(glyph('requires', <rect className="part precondition" x="3.5" y="3.5" width="17" height="17" />));
  }
  return parts;
}

/// The marks the chart draws, in the order its legend lists them (`Legend` in marks.tsx), each named by its key; the
/// way back with how many the flow has.
export const MATRIX_MARKS: MarkKey[] = [
  'line',
  'whenYes',
  'whenNo',
  'requires',
  'tools',
  'watches',
  'reads',
  'outputs',
  'wayBack',
  'latest',
  'lastAndLatest',
  'all',
  'people',
  'outside',
  'shown',
];

/// The widths the columns of a chart take, in pixels: the names' column at first and as dragged, and a cell's column at
/// most when dragged. Kept in step with tokens.css.
export const WIDTHS = {
  names: 200,
  namesMin: 120,
  namesMax: 480,
  cellMax: 480,
};

/// The widths of the flow's chart, and of a work's history, which draws the flow's cells as it does, in pixels: a cell's
/// column until the owner drags it, which is also the least it is dragged to, wide enough for its marks and a short id;
/// and the least the column of words keeps, the frame scrolling sideways below that.
export const FLOW_WIDTHS = { cell: 56, words: 280 };

/// Scrolls the frame by a wheel or a trackpad wherever the pointer is over it, the fixed row of heads and the fixed
/// column of names too, which a browser may leave unscrolled.
export function useWheel(frame: RefObject<HTMLDivElement | null>) {
  useEffect(() => {
    const el = frame.current;
    if (!el) return;
    const wheel = (e: WheelEvent) => {
      const over = e.target instanceof Element ? e.target.closest('thead, th.lead') : null;
      if (!over || e.ctrlKey) return;
      const unit = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? el.clientHeight : 1;
      const [dx, dy] = e.shiftKey && e.deltaX === 0 ? [e.deltaY, 0] : [e.deltaX, e.deltaY];
      e.preventDefault();
      el.scrollBy({ left: dx * unit, top: dy * unit });
    };
    el.addEventListener('wheel', wheel, { passive: false });
    return () => el.removeEventListener('wheel', wheel);
  }, [frame]);
}

interface ChartProps {
  matrix: Matrix;
  chosen: Chosen;
  onChoose: (chosen: Chosen) => void;
  /// Kept as it is, scrolled where the owner left it, while another view of the flow is shown.
  hidden?: boolean;
  /// How much of the frame's right a window that floats over it covers, in pixels; none while it is away.
  covered?: number;
}

export function Chart({ matrix, chosen, onChoose, hidden = false, covered = 0 }: ChartProps) {
  const { blocks, cells, spots, lines, conditions, tools, every } = matrix;
  const [pointed, setPointed] = useState<number | null>(null);
  const frame = useRef<HTMLDivElement>(null);
  const [names, setNames] = useKeptWidth(WIDTH_KEYS.names, WIDTHS.namesMin, WIDTHS.namesMax);
  const [widths, setWidth] = useKeptWidths(WIDTH_KEYS.flowCells, FLOW_WIDTHS.cell, WIDTHS.cellMax);
  useWheel(frame);
  const namesWidth = names ?? WIDTHS.names;
  const widthOf = (id: string) => widths[id] ?? FLOW_WIDTHS.cell;
  // The table takes the frame's whole width: the names' column and the cells' at theirs, the words the rest.
  const taken = cells.reduce((sum, cell) => sum + widthOf(cell.id), namesWidth);
  // Under a window the table keeps its whole width, and the frame scrolls on past it by what the window covers
  // (`--window-covers`, styles/chart.css), so that what stands under the window can be brought out from under it.
  const size = { width: 'calc(100% + var(--window-covers, 0px))', minWidth: taken + FLOW_WIDTHS.words };

  // What is chosen, here or elsewhere (a panel), is brought into view in the frame, into what a window over it leaves
  // where one is out: again as the window comes out. A datum `[every_agent]` names is found by the row of its band
  // that stands for it, having no row of its own.
  const under = covered > 0;
  useEffect(() => {
    const key = chosen && keyOf(chosen);
    if (!key) return;
    const target =
      frame.current?.querySelector(`[data-key="${CSS.escape(key)}"]`) ??
      (chosen && 'block' in chosen ? frame.current?.querySelector(`[data-blocks~="${CSS.escape(chosen.block)}"]`) : null);
    target?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }, [chosen, under]);

  const chosenCell = chosen && 'cell' in chosen ? chosen.cell : null;
  const chosenBlock = chosen && 'block' in chosen ? chosen.block : null;
  const chosenLine = chosen && 'line' in chosen ? chosen.line : null;
  const chosenTest = chosen && 'test' in chosen ? chosen.test : null;
  const chosenGroup = chosen && 'group' in chosen ? chosen.group : null;
  const chosenTool = chosen && 'tool' in chosen ? chosen.tool : null;
  const chosenEvery = chosen !== null && 'everyAgent' in chosen;
  // The cell whose words the rows show, and the cell the head of the words is about: the one under the pointer, or
  // else the one chosen.
  const saying = cells.find((c) => c.id === chosenCell) ?? null;
  const about = (pointed !== null ? cells[pointed] : undefined) ?? saying;
  const unpoint = () => setPointed(null);
  const band = { cells, chosenCell, saying, pointed, onPoint: setPointed };

  return (
    <div
      className="frame"
      ref={frame}
      onMouseLeave={unpoint}
      hidden={hidden}
      style={covered > 0 ? ({ '--window-covers': `${covered}px` } as CSSProperties) : undefined}
    >
      <table className="matrix flow" style={size as CSSProperties}>
        <colgroup>
          <col style={{ width: namesWidth }} />
          {cells.map((cell) => (
            <col key={cell.id} style={{ width: widthOf(cell.id) }} />
          ))}
          <col />
        </colgroup>
        <thead>
          <tr>
            <th className="lead corner" scope="col" aria-label="Cells across" onMouseEnter={unpoint}>
              <span className="label corner-cells">
                Cells <Arrow to="right" />
              </span>
              <Edge
                side="right"
                label="Drag the width of the rows' names"
                start={() => namesWidth}
                onDrag={setNames}
                onReset={() => setNames(null)}
              />
            </th>
            {cells.map((cell, c) => (
              <CellHead
                key={cell.id}
                cell={cell}
                chosen={chosenCell === cell.id}
                pointed={pointed === c}
                onChoose={() => onChoose(chosenCell === cell.id ? null : { cell: cell.id })}
                onPoint={() => setPointed(c)}
                onUnpoint={unpoint}
                width={widthOf(cell.id)}
                onWidth={(w) => setWidth(cell.id, w)}
              />
            ))}
            <WordsHead about={about} saying={saying} onPoint={unpoint} />
          </tr>
        </thead>
        {lines.length > 0 && (
          <tbody className="band">
            {lines.map((row) => (
              <LineLine
                key={row.name}
                row={row}
                chosen={chosenLine === row.name}
                onChoose={() => onChoose(chosenLine === row.name ? null : { line: row.name })}
                {...band}
              />
            ))}
          </tbody>
        )}
        {conditions.length > 0 && (
          <tbody className="band">
            {conditions.map((row) => {
              const isChosen = row.test ? chosenTest === row.name : chosenBlock === row.block;
              const chosenNow: Chosen = row.test ? { test: row.name } : { block: row.name };
              return (
                <ConditionLine
                  key={`${row.test ? 'test' : 'block'}:${row.name}`}
                  row={row}
                  chosen={isChosen}
                  onChoose={() => onChoose(isChosen ? null : chosenNow)}
                  {...band}
                />
              );
            })}
          </tbody>
        )}
        {tools.length > 0 && (
          <tbody className="band">
            {tools.map((row) => (
              <ToolLine
                key={row.name}
                row={row}
                chosen={chosenTool === row.name}
                onChoose={() => onChoose(chosenTool === row.name ? null : { tool: row.name })}
                {...band}
              />
            ))}
          </tbody>
        )}
        {every.length > 0 && (
          <tbody className="band">
            {every.map((row) => (
              <EveryLine
                key={row.name}
                row={row}
                chosen={chosenEvery || (chosenBlock !== null && row.blocks.includes(chosenBlock))}
                onChoose={() => onChoose(chosenEvery ? null : { everyAgent: true })}
                {...band}
              />
            ))}
          </tbody>
        )}
        <tbody>
          {blocks.map((row, r) => {
            const isChosen = row.group ? chosenGroup === row.name : chosenBlock === row.name;
            const what = row.group ? listed(row.group) : (row.block?.what ?? '');
            // What the chosen cell says of the row's thing, by its keys; of a datum it may write, with what the datum
            // is, which is what the cell is to hand in.
            const said = !saying
              ? ''
              : row.group
                ? groupSays(saying, row.name, row.group)
                : cellSays(saying, row.name, row.block?.what ?? null);
            return (
              <tr key={`${row.group ? 'group' : 'block'}:${row.name}`} className={`row ${isChosen ? 'chosen' : ''}`}>
                <BlockHead
                  row={row}
                  chosen={isChosen}
                  onChoose={() => onChoose(isChosen ? null : row.group ? { group: row.name } : { block: row.name })}
                  onPoint={unpoint}
                />
                {cells.map((cell, c) => (
                  <SpotCell
                    key={cell.id}
                    spot={spots[r][c]}
                    words={markWords(cell, row.name, spots[r][c].mark, row.group)}
                    chosen={chosenCell === cell.id}
                    pointed={pointed === c}
                    onPoint={() => setPointed(c)}
                  />
                ))}
                <td
                  className={`words ${said ? 'said' : saying ? 'beside' : ''}`}
                  title={said || what || undefined}
                  onMouseEnter={unpoint}
                >
                  {said || what}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

/// The head of a cell's column: its id, which is all a column as wide as its marks has room for. Its title, its line,
/// when it runs and what it waits for are what its button is called, over the column of words while the pointer or the
/// keyboard is on the column, and in the cell's panel; its line, its `when` and its `requires` are drawn in the bands
/// over the blocks. Where a press on it chooses nothing, as on a work's history, it is no button, and says them on the
/// pointer's tip.
export function CellHead(props: {
  cell: Cell;
  chosen: boolean;
  pointed: boolean;
  onChoose?: () => void;
  onPoint: () => void;
  onUnpoint: () => void;
  width: number;
  /// Sets the column's width as dragged, or, with none, gives it back the width it starts at.
  onWidth: (width: number | null) => void;
}) {
  const { cell, chosen, pointed, onChoose, onPoint, onUnpoint, width, onWidth } = props;
  const facts = headFacts(cell);
  const name = [`${cell.id} ${cell.title}`, facts.by, facts.when, facts.waits].filter((f) => f !== null);
  return (
    <th
      scope="col"
      className={`cell-head ${chosen ? 'chosen' : ''} ${pointed ? 'pointed' : ''}`}
      data-key={`cell:${cell.id}`}
      onMouseEnter={onPoint}
    >
      {onChoose === undefined ? (
        <span className="head-button head-still" title={name.join(', ')}>
          <span className="cell-id">{cell.id}</span>
        </span>
      ) : (
        <button
          type="button"
          className="head-button"
          aria-pressed={chosen}
          aria-label={name.join(', ')}
          onClick={onChoose}
          onFocus={onPoint}
          onBlur={onUnpoint}
        >
          <span className="cell-id">{cell.id}</span>
        </button>
      )}
      <Edge
        side="right"
        label={`Drag the width of ${cell.id}'s column`}
        start={() => width}
        onDrag={onWidth}
        onReset={() => onWidth(null)}
      />
    </th>
  );
}

/// The head of the column of words: the cell under the pointer, or else the cell chosen, by its id, its title, who
/// runs it, when it runs and what it waits for; and under that what the column holds, which is what each row's thing is
/// until a cell is chosen, and then what that cell says of each it has to do with; or, where the column holds something
/// else, as on a work's history, what `label` says.
export function WordsHead(props: { about: Cell | null; saying: Cell | null; onPoint: () => void; label?: string }) {
  const { about: cell, saying, onPoint, label } = props;
  const facts = cell ? headFacts(cell) : null;
  const whole =
    cell && facts ? [`${cell.id} ${cell.title}`, facts.by, facts.when, facts.waits].filter((f) => f !== null) : [];
  return (
    <th scope="col" className="words-head" onMouseEnter={onPoint}>
      <span className="words-about" title={whole.join(' · ') || undefined}>
        {cell && facts && (
          <>
            <span className="cell-id">{cell.id}</span> <span className="cell-title">{cell.title}</span>
            <span className="muted">
              {' · '}
              {facts.by}
              {facts.when !== null && (
                <>
                  {' · '}
                  {facts.when}
                </>
              )}
              {facts.waits !== null && (
                <>
                  {' · '}
                  {facts.waits}
                </>
              )}
            </span>
          </>
        )}
      </span>
      <span className="label words-label">
        {label ?? (saying ? `What ${saying.id} says of each row` : 'What each row is')}
      </span>
    </th>
  );
}

/// The signs of who writes data besides the cells, as their rows' names carry them: people, and the source outside.
function WriterSigns({ blocks }: { blocks: Block[] }) {
  const sources = [...new Set(blocks.flatMap((b) => (b.writtenBy.outside !== null ? [b.writtenBy.outside] : [])))];
  return (
    <>
      {blocks.some((b) => b.writtenBy.people) && <Sign name="people" />}
      {sources.length > 0 && (
        <span title={`from ${sources.join(', ')}`}>
          <Sign name="outside" />
        </span>
      )}
    </>
  );
}

/// The name of a datum at the left of its row, with its signs, the values of a choice and the groups it is of whose
/// rows its watchers stand on; or of a group a cell watches by its name, which chooses the group.
function BlockHead(props: { row: BlockRow; chosen: boolean; onChoose: () => void; onPoint: () => void }) {
  const { row, chosen, onChoose, onPoint } = props;
  const block = row.block;
  if (!block) {
    return (
      <th scope="row" className="lead block-head" data-key={`group:${row.name}`} onMouseEnter={onPoint}>
        <button type="button" className="head-button" aria-pressed={chosen} aria-label={`Group ${row.name}`} onClick={onChoose}>
          <span className="block-name" title={row.name}>
            {row.name}
          </span>
          <span className="row-label">group</span>
        </button>
      </th>
    );
  }
  return (
    <th scope="row" className="lead block-head" data-key={`block:${row.name}`} onMouseEnter={onPoint}>
      <button type="button" className="head-button" aria-pressed={chosen} aria-label={`Block ${row.name}`} onClick={onChoose}>
        <span className="block-name" title={row.name}>
          {row.name}
        </span>
        {row.of.map((group) => (
          <span key={group} className="row-label" title={`group ${group}`}>
            {group}
          </span>
        ))}
        <WriterSigns blocks={[block]} />
        {block.readBy.shown && <Sign name="shown" />}
        {block.values && (
          <span className="block-values" title={block.values.join(' | ')}>
            {block.values.join('|')}
          </span>
        )}
      </button>
    </th>
  );
}

/// The class of a crossing of a band's row.
function bandSpot(cell: Cell, chosenCell: string | null, pointed: boolean): string {
  return `spot ${chosenCell === cell.id ? 'chosen' : ''} ${pointed ? 'pointed' : ''}`;
}

/// What every row of a band is given: the cells, the cell chosen and the one whose words the rows show, and the
/// column under the pointer.
export type BandProps = {
  cells: Cell[];
  chosenCell: string | null;
  saying: Cell | null;
  pointed: number | null;
  onPoint: (c: number | null) => void;
};

/// The head of a band's row: its name, what kind of row it is, and what more its name carries; chosen by its name, it
/// opens its panel.
function BandHead(props: {
  name: string;
  label: string;
  dataKey?: string;
  aria: string;
  nameClass?: string;
  chosen: boolean;
  onChoose: () => void;
  onPoint: (c: number | null) => void;
  children?: ReactNode;
}) {
  const { name, label, dataKey, aria, nameClass, chosen, onChoose, onPoint, children } = props;
  return (
    <th scope="row" className="lead block-head" data-key={dataKey} onMouseEnter={() => onPoint(null)}>
      <button type="button" className="head-button" aria-pressed={chosen} aria-label={aria} onClick={onChoose}>
        <span className={`block-name ${nameClass ?? ''}`} title={name}>
          {name}
        </span>
        <span className="row-label">{label}</span>
        {children}
      </button>
    </th>
  );
}

/// The column of words of a band's row: what the chosen cell says of the row where it says anything, or else what the
/// row is.
function BandWords(props: { says: string; what: string; saying: Cell | null; onPoint: (c: number | null) => void }) {
  const { says, what, saying, onPoint } = props;
  return (
    <td
      className={`words ${says ? 'said' : saying ? 'beside' : ''}`}
      title={says || what || undefined}
      onMouseEnter={() => onPoint(null)}
    >
      {says || what}
    </td>
  );
}

/// The row of a line, at the top: its name in the accent, a dot under each cell on it, the dots joined along the row,
/// and the line's keys as its file writes them; on the row of the chosen cell's line, that it is its `line`. Chosen by
/// its name, it opens the line's panel.
export function LineLine(props: BandProps & { row: LineRow; chosen: boolean; onChoose: () => void }) {
  const { row, cells, chosen, onChoose, chosenCell, saying, pointed, onPoint } = props;
  const keys = row.line ? keysWords(lineKeys(row.line)) : '';
  const says = saying?.kind === 'agent' && saying.line === row.name ? 'line' : '';
  return (
    <tr className={`row line-row ${chosen ? 'chosen' : ''}`}>
      <BandHead
        name={row.name}
        label="line"
        dataKey={`line:${row.name}`}
        aria={`Line ${row.name}`}
        nameClass="line-name"
        chosen={chosen}
        onChoose={onChoose}
        onPoint={onPoint}
      />
      {cells.map((cell, c) => {
        const spot = row.spots[c];
        const words = spot.on ? `${cell.id} line ${row.name}` : '';
        return (
          <td
            key={cell.id}
            className={bandSpot(cell, chosenCell, pointed === c)}
            title={words || undefined}
            onMouseEnter={() => onPoint(c)}
          >
            <LineSegments spot={spot} />
            <LineGlyph on={spot.on} />
            {words && <span className="visually-hidden">{words}</span>}
          </td>
        );
      })}
      <BandWords says={says} what={keys} saying={saying} onPoint={onPoint} />
    </tr>
  );
}

/// A line's row through a crossing, up to the middle, where the dot is drawn over it.
function LineSegments({ spot }: { spot: LineSpot }) {
  const runs = (side: 'left' | 'right') => spot.line === side || spot.line === 'through';
  return (
    <>
      {runs('left') && <span className="seg seg-left line" aria-hidden="true" />}
      {runs('right') && <span className="seg seg-right line" aria-hidden="true" />}
    </>
  );
}

/// The row of a condition, under the lines: a named test, or a block or datum a key of a cell's `when` or `requires`
/// names, by its name; under each cell with a key of its `when` on the row, a tick, a cross or the value
/// (`ConditionGlyph`), and a square under each with a key of its `requires` on it, the value said in its words; and
/// what the test `says`, or the block's `default`. A datum's row is its one row: its head is the datum's, with its
/// values and its signs, and under each cell the marks of what the cell's file says of the datum follow the condition's,
/// with the way back along the row; its words are what the datum is and its `default`. On the row of a condition the
/// chosen cell names, by which key it names it, and what the cell's file says of the datum. Chosen by its name, it opens
/// the test's panel, or the block's.
function ConditionLine(props: BandProps & { row: ConditionRow; chosen: boolean; onChoose: () => void }) {
  const { row, cells, chosen, onChoose, chosenCell, saying, pointed, onPoint } = props;
  const data = row.data;
  const declared = data?.row.block;
  // A key on the row as the cell's file writes it: the test, or the block with the value it waits for.
  const keyText = (value?: string) => (row.test ? `test = ${row.name}` : `${row.name} = ${value}`);
  const at = saying ? cells.findIndex((cell) => cell.id === saying.id) : -1;
  const own = at === -1 ? undefined : row.spots[at];
  const says = [
    ...(own?.when ? [`when ${keyText(own.value)}`] : []),
    ...(own?.requires ? [`requires ${keyText(own.requiresValue)}`] : []),
    ...(saying && data ? [cellSays(saying, row.name, declared?.what ?? null)] : []),
  ]
    .filter((w) => w !== '')
    .join('; ');
  const what = row.test
    ? row.test.words
    : [declared?.what ?? '', declared?.default ? `default ${declared.default}` : ''].filter((w) => w !== '').join('; ');
  return (
    <tr className={`row condition-row ${chosen ? 'chosen' : ''}`}>
      {data ? (
        <BlockHead row={data.row} chosen={chosen} onChoose={onChoose} onPoint={() => onPoint(null)} />
      ) : (
        <BandHead
          name={row.name}
          label={row.test ? 'test' : 'block'}
          dataKey={row.test ? `test:${row.name}` : undefined}
          aria={row.test ? `Test ${row.name}` : `Block ${row.name}`}
          chosen={chosen}
          onChoose={onChoose}
          onPoint={onPoint}
        />
      )}
      {cells.map((cell, c) => {
        const spot = row.spots[c];
        const marks = data?.spots[c];
        const words = [
          spot.when ? `${cell.id} when ${keyText(spot.value)}` : '',
          spot.requires ? `${cell.id} requires ${keyText(spot.requiresValue)}` : '',
          marks ? markWords(cell, row.name, marks.mark) : '',
        ]
          .filter((w) => w !== '')
          .join(', ');
        return (
          <Crossing
            key={cell.id}
            condition={spot}
            spot={marks}
            words={words}
            className={bandSpot(cell, chosenCell, pointed === c)}
            onPoint={() => onPoint(c)}
          />
        );
      })}
      <BandWords says={says} what={what} saying={saying} onPoint={onPoint} />
    </tr>
  );
}

/// The row of one of the engine's tools: its name, a small square under each cell that may call it, and `tools`, the
/// key that names it; on the row of a tool the chosen cell may call, that too. Chosen by its name, it opens the
/// tool's panel.
function ToolLine(props: BandProps & { row: ToolRow; chosen: boolean; onChoose: () => void }) {
  const { row, cells, chosen, onChoose, chosenCell, saying, pointed, onPoint } = props;
  const says = saying?.tools.includes(row.name) ? 'tools' : '';
  return (
    <tr className={`row tool-row ${chosen ? 'chosen' : ''}`}>
      <BandHead
        name={row.name}
        label="tool"
        dataKey={`tool:${row.name}`}
        aria={`Tool ${row.name}`}
        chosen={chosen}
        onChoose={onChoose}
        onPoint={onPoint}
      />
      {cells.map((cell, c) => {
        const words = row.spots[c] ? `${cell.id} tools ${row.name}` : '';
        return (
          <td
            key={cell.id}
            className={bandSpot(cell, chosenCell, pointed === c)}
            title={words || undefined}
            onMouseEnter={() => onPoint(c)}
          >
            <ToolGlyph on={row.spots[c]} />
            {words && <span className="visually-hidden">{words}</span>}
          </td>
        );
      })}
      <BandWords says={says} what="tools" saying={saying} onPoint={onPoint} />
    </tr>
  );
}

/// The row of a name `[every_agent]` writes, as `flow.toml` writes it, with the signs of who writes the data it stands
/// for: under each cell what runs it of those data and what its own file says of them, by the marks of the data's rows,
/// with the ways back along it; and in the words, what `[every_agent]` does with them and what they are, or what the
/// chosen cell's file and `[every_agent]` say of them. Chosen by its name, it opens the panel of `[every_agent]`.
function EveryLine(props: BandProps & { row: EveryRow; chosen: boolean; onChoose: () => void }) {
  const { row, cells, chosen, onChoose, chosenCell, saying, pointed, onPoint } = props;
  return (
    <tr className={`row every-row ${chosen ? 'chosen' : ''}`} data-blocks={row.blocks.join(' ')}>
      <BandHead
        name={row.name}
        label="every_agent"
        aria={`every_agent reads ${row.name}`}
        chosen={chosen}
        onChoose={onChoose}
        onPoint={onPoint}
      >
        <WriterSigns blocks={row.data} />
      </BandHead>
      {cells.map((cell, c) => (
        <SpotCell
          key={cell.id}
          spot={row.spots[c]}
          words={everyMarkWords(cell, row)}
          chosen={chosenCell === cell.id}
          pointed={pointed === c}
          onPoint={() => onPoint(c)}
        />
      ))}
      <BandWords
        says={saying ? everySays(saying, row) : ''}
        what={everyIs(row, row.data[0]?.what ?? null)}
        saying={saying}
        onPoint={onPoint}
      />
    </tr>
  );
}

/// A crossing of a row of the data, or of `[every_agent]`: the way back through it where one runs, and the cell's marks,
/// with the arrow at a cell it runs again.
function SpotCell(props: { spot: Spot; words: string; chosen: boolean; pointed: boolean; onPoint: () => void }) {
  const { spot, words, chosen, pointed, onPoint } = props;
  return (
    <Crossing
      spot={spot}
      words={words}
      className={`spot ${chosen ? 'chosen' : ''} ${pointed ? 'pointed' : ''}`}
      onPoint={onPoint}
    />
  );
}

/// A crossing of a row: the way back through it where one runs, and the cell's marks side by side, those of a
/// condition first, then those of a datum, the last with the arrow of a way back where it runs the cell again. How many
/// marks it has (`--marks`) is what the way back's line stops short of.
function Crossing(props: {
  condition?: ConditionSpot;
  spot?: Spot;
  words: string;
  className: string;
  onPoint: () => void;
}) {
  const { condition, spot, words, className, onPoint } = props;
  const runs = (side: 'left' | 'right') => spot !== undefined && (spot.back === side || spot.back === 'through');
  const parts = [...(condition ? conditionParts(condition) : []), ...(spot ? dataParts(spot.mark, spot.backTo) : [])];
  return (
    <td
      className={`${className} ${parts.length > 0 ? 'has-mark' : ''}`}
      style={parts.length > 1 ? ({ '--marks': parts.length } as CSSProperties) : undefined}
      onMouseEnter={onPoint}
      title={words || undefined}
    >
      {runs('left') && <span className="seg seg-left back" aria-hidden="true" />}
      {runs('right') && <span className="seg seg-right back" aria-hidden="true" />}
      {parts.length > 0 && <span className="glyphs">{parts}</span>}
      {words && <span className="visually-hidden">{words}</span>}
    </td>
  );
}
