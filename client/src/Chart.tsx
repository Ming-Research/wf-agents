// The chart: the matrix of the blocks, one to a row, against the cells, one to a column in the order of the walk. It is
// how the owner checks the flow (Article 7), so every cell and every block is on it, and what a cell does with a block
// is a mark where they cross. A write right of a read of the same block, by a later cell, is the work going back: its
// line is drawn leftwards along the row to each read it sends the work back to, in the way back's purple, so that
// every way back shows at a glance. The blocks' names stay in place as the chart scrolls sideways, and the cells' heads
// as it scrolls down, in the chart's own frame.

import { type CSSProperties, type RefObject, useEffect, useRef, useState } from 'react';

import type { Block, Cell, Change } from './api';
import {
  type BlockRow,
  type CellColumn,
  type Mark,
  MARK_PARTS,
  type MarkPart,
  type Matrix,
  type Spot,
  hasMark,
} from './matrix';
import { Edge, STATUS_WORDS, Sign, Tag } from './parts';
import { WIDTH_KEYS, useKeptWidth, useKeptWidths } from './sizes';
import { conditionWords } from './words';

/// What the owner chose: a cell, a block, or nothing.
export type Chosen = { cell: string } | { block: string } | null;

const PART_WORDS: Record<MarkPart, string> = {
  read: 'reads',
  write: 'may write',
  condition: 'runs only by the value of',
};

/// The words a mark says, for the pointer's tip and for a reader of the screen: "plan reads opinions".
export function markWords(cell: string, block: string, spot: Pick<Spot, 'mark' | 'added' | 'removed'>): string {
  const parts = MARK_PARTS.flatMap((part) => {
    if (spot.added[part]) return [`${PART_WORDS[part]} (put in)`];
    if (spot.removed[part]) return [`${PART_WORDS[part]} (taken out)`];
    return spot.mark[part] ? [PART_WORDS[part]] : [];
  });
  return parts.length > 0 ? `${cell} ${parts.join(', ')} ${block}` : '';
}

/// How one part of a mark is drawn: as it stands, put in by the change, or taken out by it.
function partClass(part: MarkPart, spot: Spot): string | null {
  if (spot.added[part]) return 'added';
  if (spot.removed[part]) return 'removed';
  return spot.mark[part] ? 'plain' : null;
}

/// A mark: a ring where the cell reads the block, a dot where it may write it, both where both, and a diamond round
/// them where its condition reads the block's value.
export function MarkGlyph({ spot }: { spot: Spot }) {
  const read = partClass('read', spot);
  const write = partClass('write', spot);
  const condition = partClass('condition', spot);
  if (!read && !write && !condition) return null;
  return (
    <svg className="glyph" viewBox="0 0 24 24" aria-hidden="true">
      {condition && <polygon className={`part condition ${condition}`} points="12,1.5 22.5,12 12,22.5 1.5,12" />}
      {read && <circle className={`part read ${read}`} cx="12" cy="12" r="6.5" />}
      {write && <circle className={`part write ${write}`} cx="12" cy="12" r={read ? 3.25 : 6} />}
    </svg>
  );
}

/// The legend of the marks and signs, in the view's line; with what a change's colours mean when comparing.
export function Legend({ waysBack, comparing }: { waysBack: number; comparing: boolean }) {
  const spot = (mark: Partial<Mark>): Spot => ({
    mark: { read: false, write: false, condition: false, ...mark },
    added: { read: false, write: false, condition: false },
    removed: { read: false, write: false, condition: false },
    line: 'none',
    back: 'none',
    backTo: false,
  });
  return (
    <ul className="legend" aria-label="What the marks mean">
      <li>
        <MarkGlyph spot={spot({ read: true })} /> reads
      </li>
      <li>
        <MarkGlyph spot={spot({ write: true })} /> may write
      </li>
      <li>
        <MarkGlyph spot={spot({ read: true, write: true })} /> both
      </li>
      <li>
        <MarkGlyph spot={spot({ condition: true })} /> its condition reads
      </li>
      <li>
        <span className="legend-back" aria-hidden="true" /> way back ({waysBack})
      </li>
      <li>
        <Sign name="people" /> people write
      </li>
      <li>
        <Sign name="outside" /> the outside writes
      </li>
      <li>
        <Sign name="shown" /> shown
      </li>
      {comparing &&
        (['added', 'removed', 'changed'] as const).map((status) => (
          <li key={status}>
            <span className={`legend-status status-${status}`} aria-hidden="true" /> {STATUS_WORDS[status]}
          </li>
        ))}
    </ul>
  );
}

/// Who runs a cell: an agent in its line, named by the line, or the engine.
export function runBy(cell: Cell): string {
  return cell.kind === 'agent' ? (cell.line ?? '') : 'engine';
}

/// A name that may break after each underscore, and nowhere else, when its column is narrow.
export function Breakable({ name }: { name: string }) {
  const parts = name.split('_');
  return (
    <>
      {parts.map((part, i) => (
        <span key={i}>
          {part}
          {i < parts.length - 1 && (
            <>
              _<wbr />
            </>
          )}
        </span>
      ))}
    </>
  );
}

/// The widths the chart's columns take, in pixels: the names' column at first, and a cell's column at least, at most
/// when the cells share the free width evenly, and at most when dragged. Kept in step with tokens.css.
export const WIDTHS = { names: 200, namesMin: 120, namesMax: 480, cellMin: 88, cellShare: 160, cellMax: 480 };

/// How wide the table is: the whole width of its frame, the cells sharing evenly what the names' column and the cells
/// whose width was dragged leave, each up to `cellShare`, so that a few cells on a wide window do not grow absurd; and
/// at least as wide as every column at its least, the frame scrolling sideways below that.
export function tableWidth(names: number, cells: string[], dragged: Record<string, number>): { width: string; minWidth: string } {
  const set = cells.reduce((n, c) => n + (dragged[c] ?? 0), 0);
  const free = cells.filter((c) => dragged[c] === undefined).length;
  const most = names + set + free * WIDTHS.cellShare;
  const least = names + set + free * WIDTHS.cellMin;
  return { width: `min(100%, ${most}px)`, minWidth: `${least}px` };
}

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
}

export function Chart({ matrix, chosen, onChoose }: ChartProps) {
  const { blocks, cells, spots } = matrix;
  const [pointed, setPointed] = useState<number | null>(null);
  const frame = useRef<HTMLDivElement>(null);
  const [names, setNames] = useKeptWidth(WIDTH_KEYS.names, WIDTHS.namesMin, WIDTHS.namesMax);
  const [widths, setWidth] = useKeptWidths(WIDTH_KEYS.cells, WIDTHS.cellMin, WIDTHS.cellMax);
  useWheel(frame);
  const namesWidth = names ?? WIDTHS.names;
  const size = tableWidth(
    namesWidth,
    cells.map((c) => c.id),
    widths,
  );

  // What is chosen elsewhere (the list of changes, a panel) is brought into view in the frame.
  useEffect(() => {
    if (!chosen) return;
    const key = 'cell' in chosen ? `cell:${chosen.cell}` : `block:${chosen.block}`;
    const target = frame.current?.querySelector(`[data-key="${CSS.escape(key)}"]`);
    target?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }, [chosen]);

  const chosenCell = chosen && 'cell' in chosen ? chosen.cell : null;
  const chosenBlock = chosen && 'block' in chosen ? chosen.block : null;

  return (
    <div className="frame" ref={frame} onMouseLeave={() => setPointed(null)}>
      <table className="matrix" style={size as CSSProperties}>
        <colgroup>
          <col style={{ width: namesWidth }} />
          {cells.map((column) => (
            <col key={column.id} style={widths[column.id] !== undefined ? { width: widths[column.id] } : undefined} />
          ))}
        </colgroup>
        <thead>
          <tr>
            <th className="lead corner" scope="col" aria-label="Blocks down the side, cells across">
              <span className="label corner-cells">Cells →</span>
              <span className="label corner-blocks">Blocks ↓</span>
              <Edge
                side="right"
                label="Drag the width of the blocks' names"
                start={() => namesWidth}
                onDrag={setNames}
                onReset={() => setNames(null)}
              />
            </th>
            {cells.map((column, c) => (
              <CellHead
                key={column.id}
                column={column}
                chosen={chosenCell === column.id}
                pointed={pointed === c}
                onChoose={() => onChoose(chosenCell === column.id ? null : { cell: column.id })}
                onPoint={() => setPointed(c)}
                onWidth={(w) => setWidth(column.id, w)}
              />
            ))}
          </tr>
        </thead>
        <tbody>
          {blocks.map((row, r) => {
            const isChosen = chosenBlock === row.name;
            return (
              <tr
                key={row.name}
                className={`row status-${row.status} ${row.block ? '' : 'absent'} ${isChosen ? 'chosen' : ''}`}
              >
                <BlockHead row={row} chosen={isChosen} onChoose={() => onChoose(isChosen ? null : { block: row.name })} />
                {cells.map((column, c) => (
                  <SpotCell
                    key={column.id}
                    spot={spots[r][c]}
                    cell={column.id}
                    block={row.name}
                    absent={!column.cell}
                    chosen={chosenCell === column.id}
                    pointed={pointed === c}
                    onPoint={() => setPointed(c)}
                  />
                ))}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

/// The head of a cell's column: its id and title, who runs it, and, cut to two lines, when it runs and what it waits
/// for; the whole words are on the pointer's tip and in the cell's panel.
function CellHead(props: {
  column: CellColumn;
  chosen: boolean;
  pointed: boolean;
  onChoose: () => void;
  onPoint: () => void;
  /// Sets the column's width as dragged, or, with none, lets the cells share the width again.
  onWidth: (width: number | null) => void;
}) {
  const { column, chosen, pointed, onChoose, onPoint, onWidth } = props;
  const head = useRef<HTMLTableCellElement>(null);
  const cell = (column.cell ?? column.other) as Cell;
  const changed = (...kinds: Change['kind'][]) => (kinds.some((k) => column.changed.has(k)) ? 'changed-field' : '');
  const when = cell.condition ? `runs when ${conditionWords(cell.condition)}` : '';
  const waits = cell.preconditions.map((p) => p.words).join('; ');
  return (
    <th
      scope="col"
      className={`cell-head status-${column.status} ${column.cell ? '' : 'absent'} ${chosen ? 'chosen' : ''} ${
        pointed ? 'pointed' : ''
      }`}
      data-key={`cell:${column.id}`}
      onMouseEnter={onPoint}
      ref={head}
    >
      <button type="button" className="head-button" aria-pressed={chosen} onClick={onChoose}>
        <span className={`cell-id ${changed('cellMoved')}`}>{column.id}</span>
        <span className={`cell-title ${changed('title')}`}>{cell.title}</span>
        <span className={`cell-line ${cell.kind === 'agent' ? 'line-name' : ''} ${changed('kind', 'line')}`}>
          <Breakable name={runBy(cell)} />
        </span>
        {when && (
          <span className={`cell-when ${changed('condition')}`} title={when}>
            {when}
          </span>
        )}
        {waits && (
          <span
            className={`cell-waits ${changed('preconditionAdded', 'preconditionRemoved')}`}
            title={`waits until ${waits}`}
          >
            waits until {waits}
          </span>
        )}
        <Tag status={column.status} />
      </button>
      <Edge
        side="right"
        label={`Drag the width of ${column.id}'s column`}
        start={() => head.current?.getBoundingClientRect().width ?? WIDTHS.cellShare}
        onDrag={onWidth}
        onReset={() => onWidth(null)}
      />
    </th>
  );
}

/// The name of a block at the left of its row, with its signs and the values of a choice after it. What a change did
/// to it is the colour of its edge, which the legend names, so that the name keeps the room.
function BlockHead({ row, chosen, onChoose }: { row: BlockRow; chosen: boolean; onChoose: () => void }) {
  const block = (row.block ?? row.other) as Block;
  const changed = (kind: 'values' | 'people' | 'outside' | 'shown') => (row.changed.has(kind) ? 'changed-field' : '');
  const status = row.status === 'same' ? '' : `, ${STATUS_WORDS[row.status]}`;
  return (
    <th scope="row" className="lead block-head" data-key={`block:${row.name}`}>
      <button
        type="button"
        className="head-button"
        aria-pressed={chosen}
        aria-label={`Block ${row.name}${status}`}
        onClick={onChoose}
      >
        <span className="block-name" title={row.name}>
          {row.name}
        </span>
        {block.writtenBy.people && <Sign name="people" className={changed('people')} />}
        {block.writtenBy.outside && <Sign name="outside" className={changed('outside')} />}
        {block.readBy.shown && <Sign name="shown" className={changed('shown')} />}
        {block.values && (
          <span className={`block-values ${changed('values')}`} title={block.values.join(' | ')}>
            {block.values.join('|')}
          </span>
        )}
      </button>
    </th>
  );
}

function SpotCell(props: {
  spot: Spot;
  cell: string;
  block: string;
  absent: boolean;
  chosen: boolean;
  pointed: boolean;
  onPoint: () => void;
}) {
  const { spot, cell, block, absent, chosen, pointed, onPoint } = props;
  // The left half of the crossing carries the way back when it does, the row's line otherwise; so the right.
  const half = (side: 'left' | 'right') =>
    spot.back === side || spot.back === 'through'
      ? 'back'
      : spot.line === side || spot.line === 'through'
        ? 'line'
        : null;
  const left = half('left');
  const right = half('right');
  const words = markWords(cell, block, spot);
  const marked = hasMark(spot.mark) || hasMark(spot.added) || hasMark(spot.removed);
  return (
    <td
      className={`spot ${marked ? 'has-mark' : ''} ${absent ? 'absent' : ''} ${chosen ? 'chosen' : ''} ${
        pointed ? 'pointed' : ''
      }`}
      onMouseEnter={onPoint}
      title={words || undefined}
    >
      {left && <span className={`seg seg-left ${left}`} aria-hidden="true" />}
      {right && <span className={`seg seg-right ${right}`} aria-hidden="true" />}
      {spot.backTo && <span className="arrow" aria-hidden="true" />}
      <MarkGlyph spot={spot} />
      {words && <span className="visually-hidden">{words}</span>}
    </td>
  );
}
