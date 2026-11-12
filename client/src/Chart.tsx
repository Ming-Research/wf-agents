// The chart: the matrix of the blocks, one to a row, against the cells, one to a column in the order of the walk. It is
// how the owner checks the flow (Article 7), so every cell and every block is on it, and what a cell does with a block
// is a mark where they cross. A write right of a read of the same block, by a later cell, is the work going back: its
// line is drawn leftwards along the row to each read it sends the work back to, in the way back's purple, so that
// every way back shows at a glance. The blocks' names stay in place as the chart scrolls sideways, and the cells' heads
// as it scrolls down, in the chart's own frame.
//
// A cell's column holds only marks, so it is only as wide as they are, and its head holds the cell's id and the first
// word of its line. The width this leaves, right of the marks, is a column of words, one line to a row: what the block
// is, or, on the rows of the cell chosen, what that cell says of the block. What a head has no room for, the cell's
// title and when it runs, stands over that column for the cell under the pointer.

import { type CSSProperties, type RefObject, useEffect, useRef, useState } from 'react';

import type { Block, Cell, Change } from './api';
import {
  type BlockRow,
  type CellColumn,
  type Mark,
  MARK_PARTS,
  type MarkPart,
  type Matrix,
  NO_MARK,
  type Spot,
  hasMark,
} from './matrix';
import { Edge, STATUS_WORDS, Sign } from './parts';
import { WIDTH_KEYS, useKeptWidth, useKeptWidths } from './sizes';
import { cellSays, headFacts, runBy } from './words';

/// What the owner chose: a cell, a block, or nothing.
export type Chosen = { cell: string } | { block: string } | null;

const PART_WORDS: Record<MarkPart, string> = {
  read: 'reads',
  write: 'may write',
  condition: 'runs only by the value of',
  precondition: 'waits on the value of',
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

/// A mark: a ring where the cell reads the block, a dot where it may write it, both where both, a diamond round them
/// where its condition reads the block's value, and a square round them where a precondition does.
export function MarkGlyph({ spot }: { spot: Spot }) {
  const read = partClass('read', spot);
  const write = partClass('write', spot);
  const condition = partClass('condition', spot);
  const precondition = partClass('precondition', spot);
  if (!read && !write && !condition && !precondition) return null;
  return (
    <svg className="glyph" viewBox="0 0 24 24" aria-hidden="true">
      {condition && <polygon className={`part condition ${condition}`} points="12,1.5 22.5,12 12,22.5 1.5,12" />}
      {precondition && (
        <rect className={`part precondition ${precondition}`} x="3.5" y="3.5" width="17" height="17" />
      )}
      {read && <circle className={`part read ${read}`} cx="12" cy="12" r="6.5" />}
      {write && <circle className={`part write ${write}`} cx="12" cy="12" r={read ? 3.25 : 6} />}
    </svg>
  );
}

/// The legend of the marks and signs, in the view's line; with what a change's colours mean when comparing.
export function Legend({ waysBack, comparing }: { waysBack: number; comparing: boolean }) {
  const spot = (mark: Partial<Mark>): Spot => ({
    mark: { ...NO_MARK, ...mark },
    added: NO_MARK,
    removed: NO_MARK,
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
        <MarkGlyph spot={spot({ precondition: true })} /> its precondition reads
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

/// The widths the columns of a chart take, in pixels: the names' column at first and as dragged; and, in the history
/// of a work, whose cells' columns hold words, a cell's column at least, at most when the cells share the free width
/// evenly, and at most when dragged. Kept in step with tokens.css.
export const WIDTHS = { names: 200, namesMin: 120, namesMax: 480, cellMin: 88, cellShare: 160, cellMax: 480 };

/// The widths of the flow's chart, in pixels: a cell's column until the owner drags it, which is also the least it is
/// dragged to, wide enough for its marks, a short id and the first word of its line; and the least the column of words
/// keeps, the frame scrolling sideways below that.
export const FLOW_WIDTHS = { cell: 56, words: 280 };

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
  const [widths, setWidth] = useKeptWidths(WIDTH_KEYS.flowCells, FLOW_WIDTHS.cell, WIDTHS.cellMax);
  useWheel(frame);
  const namesWidth = names ?? WIDTHS.names;
  const widthOf = (id: string) => widths[id] ?? FLOW_WIDTHS.cell;
  // The table takes the frame's whole width: the names' column and the cells' at theirs, the words the rest.
  const taken = cells.reduce((sum, column) => sum + widthOf(column.id), namesWidth);
  const size = { width: '100%', minWidth: taken + FLOW_WIDTHS.words };

  // What is chosen elsewhere (the list of changes, a panel) is brought into view in the frame.
  useEffect(() => {
    if (!chosen) return;
    const key = 'cell' in chosen ? `cell:${chosen.cell}` : `block:${chosen.block}`;
    const target = frame.current?.querySelector(`[data-key="${CSS.escape(key)}"]`);
    target?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }, [chosen]);

  const chosenCell = chosen && 'cell' in chosen ? chosen.cell : null;
  const chosenBlock = chosen && 'block' in chosen ? chosen.block : null;
  const cellOf = (column: CellColumn | undefined) => (column ? ((column.cell ?? column.other) as Cell) : null);
  // The cell whose words the rows show, and the cell the head of the words is about: the one under the pointer, or
  // else the one chosen.
  const sayingColumn = cells.find((c) => c.id === chosenCell);
  const saying = cellOf(sayingColumn);
  const about = (pointed !== null ? cells[pointed] : undefined) ?? sayingColumn;
  const unpoint = () => setPointed(null);

  return (
    <div className="frame" ref={frame} onMouseLeave={unpoint}>
      <table className="matrix flow" style={size as CSSProperties}>
        <colgroup>
          <col style={{ width: namesWidth }} />
          {cells.map((column) => (
            <col key={column.id} style={{ width: widthOf(column.id) }} />
          ))}
          <col />
        </colgroup>
        <thead>
          <tr>
            <th className="lead corner" scope="col" aria-label="Blocks down the side, cells across" onMouseEnter={unpoint}>
              <span className="label corner-blocks">Blocks ↓</span>
              <span className="label corner-cells">Cells →</span>
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
                onUnpoint={unpoint}
                width={widthOf(column.id)}
                onWidth={(w) => setWidth(column.id, w)}
              />
            ))}
            <WordsHead about={about} saying={saying} onPoint={unpoint} />
          </tr>
        </thead>
        <tbody>
          {blocks.map((row, r) => {
            const isChosen = chosenBlock === row.name;
            const what = (row.block ?? row.other)?.what ?? '';
            // What the chosen cell says of the block; of one it may write, with what the block is, which is what the
            // cell is to hand in.
            const says = saying ? cellSays(saying, row.name) : '';
            const said = says && what && saying?.outputs.includes(row.name) ? `${says}: ${what}` : says;
            return (
              <tr
                key={row.name}
                className={`row status-${row.status} ${row.block ? '' : 'absent'} ${isChosen ? 'chosen' : ''}`}
              >
                <BlockHead
                  row={row}
                  chosen={isChosen}
                  onChoose={() => onChoose(isChosen ? null : { block: row.name })}
                  onPoint={unpoint}
                />
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
                <td
                  className={`words ${said ? 'said' : saying ? 'beside' : ''} ${row.changed.has('what') ? 'changed-field' : ''}`}
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

/// The head of a cell's column: its id and the first word of its line, which is all a column as wide as its marks has
/// room for. Its title, its line in full, when it runs and what it waits for are over the column of words while the
/// pointer or the keyboard is on the column, and in the cell's panel; what a change did to the cell is the colour of
/// the head's edge, which the legend names, and what it changed of those is marked where they are said.
function CellHead(props: {
  column: CellColumn;
  chosen: boolean;
  pointed: boolean;
  onChoose: () => void;
  onPoint: () => void;
  onUnpoint: () => void;
  width: number;
  /// Sets the column's width as dragged, or, with none, gives it back the width it starts at.
  onWidth: (width: number | null) => void;
}) {
  const { column, chosen, pointed, onChoose, onPoint, onUnpoint, width, onWidth } = props;
  const cell = (column.cell ?? column.other) as Cell;
  const changed = (...kinds: Change['kind'][]) => (kinds.some((k) => column.changed.has(k)) ? 'changed-field' : '');
  const facts = headFacts(cell);
  const status = column.status === 'same' ? [] : [STATUS_WORDS[column.status]];
  const name = [`${cell.id} ${cell.title}`, facts.by, facts.when, facts.waits, ...status].filter((f) => f !== null);
  return (
    <th
      scope="col"
      className={`cell-head status-${column.status} ${column.cell ? '' : 'absent'} ${chosen ? 'chosen' : ''} ${
        pointed ? 'pointed' : ''
      }`}
      data-key={`cell:${column.id}`}
      onMouseEnter={onPoint}
    >
      <button
        type="button"
        className="head-button"
        aria-pressed={chosen}
        aria-label={name.join(', ')}
        onClick={onChoose}
        onFocus={onPoint}
        onBlur={onUnpoint}
      >
        <span className={`cell-id ${changed('cellMoved')}`}>{column.id}</span>
        <span className={`cell-line ${cell.kind === 'agent' ? 'line-name' : ''} ${changed('kind', 'line', 'readsOnly')}`}>
          {runBy(cell).split('_')[0]}
        </span>
      </button>
      <Edge
        side="right"
        label={`Drag the width of ${column.id}'s column`}
        start={() => width}
        onDrag={onWidth}
        onReset={() => onWidth(null)}
      />
    </th>
  );
}

/// The head of the column of words: the cell under the pointer, or else the cell chosen, by its id, its title, who
/// runs it, when it runs and what it waits for, each marked where a change changed it; and under that what the column
/// holds, which is what each block is until a cell is chosen, and then what that cell says of each block it has to do
/// with.
function WordsHead(props: { about: CellColumn | undefined; saying: Cell | null; onPoint: () => void }) {
  const { about, saying, onPoint } = props;
  const cell = about ? ((about.cell ?? about.other) as Cell) : null;
  const facts = cell ? headFacts(cell) : null;
  const changed = (...kinds: Change['kind'][]) => (kinds.some((k) => about?.changed.has(k)) ? 'changed-field' : '');
  const whole = cell && facts ? [`${cell.id} ${cell.title}`, facts.by, facts.when, facts.waits].filter((f) => f !== null) : [];
  return (
    <th scope="col" className="words-head" onMouseEnter={onPoint}>
      <span
        className={`words-about ${about && !about.cell ? 'absent' : ''} ${about?.status === 'removed' ? 'taken-out' : ''}`}
        title={whole.join(' · ') || undefined}
      >
        {cell && facts && (
          <>
            <span className="cell-id">{cell.id}</span> <span className={`cell-title ${changed('title')}`}>{cell.title}</span>
            <span className="muted">
              {' · '}
              <span className={changed('kind', 'line', 'readsOnly')}>{facts.by}</span>
              {facts.when !== null && (
                <>
                  {' · '}
                  <span className={changed('condition')}>{facts.when}</span>
                </>
              )}
              {facts.waits !== null && (
                <>
                  {' · '}
                  <span className={changed('preconditionAdded', 'preconditionRemoved')}>{facts.waits}</span>
                </>
              )}
            </span>
          </>
        )}
      </span>
      <span className="label words-label">
        {saying ? `What ${saying.id} says of each block` : 'What each block is'}
      </span>
    </th>
  );
}

/// The name of a block at the left of its row, with its signs, the source outside the work that writes it and the
/// values of a choice after it. What a change did to it is the colour of its edge, which the legend names, so that the
/// name keeps the room.
function BlockHead(props: { row: BlockRow; chosen: boolean; onChoose: () => void; onPoint: () => void }) {
  const { row, chosen, onChoose, onPoint } = props;
  const block = (row.block ?? row.other) as Block;
  const changed = (kind: 'values' | 'people' | 'outside' | 'shown') => (row.changed.has(kind) ? 'changed-field' : '');
  const status = row.status === 'same' ? '' : `, ${STATUS_WORDS[row.status]}`;
  return (
    <th scope="row" className="lead block-head" data-key={`block:${row.name}`} onMouseEnter={onPoint}>
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
        {block.writtenBy.outside !== null && (
          <>
            <Sign name="outside" className={changed('outside')} />
            <span className={`block-source ${changed('outside')}`} title={`written by ${block.writtenBy.outside}`}>
              {block.writtenBy.outside}
            </span>
          </>
        )}
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
  // Each half of the crossing carries the row's line, the way back, or both. Where both run, each keeps to its lane,
  // the line above the middle and the way back below it, so that neither covers the other; the mark stays in the
  // middle.
  const runs = (which: 'line' | 'back', side: 'left' | 'right') => spot[which] === side || spot[which] === 'through';
  const half = (side: 'left' | 'right') => {
    const lane = runs('line', side) && runs('back', side) ? 'lane' : '';
    return (
      <>
        {runs('line', side) && <span className={`seg seg-${side} line ${lane}`} aria-hidden="true" />}
        {runs('back', side) && <span className={`seg seg-${side} back ${lane}`} aria-hidden="true" />}
      </>
    );
  };
  // The arrow of a way back is at the right of the read it points at, in the way back's lane.
  const arrowLane = runs('line', 'right') && runs('back', 'right') ? 'lane' : '';
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
      {half('left')}
      {half('right')}
      {spot.backTo && <span className={`arrow ${arrowLane}`} aria-hidden="true" />}
      <MarkGlyph spot={spot} />
      {words && <span className="visually-hidden">{words}</span>}
    </td>
  );
}
