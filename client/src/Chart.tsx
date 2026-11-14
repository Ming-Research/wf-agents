// The chart: the matrix of the blocks, one to a row, against the cells, one to a column in the order of the walk. It is
// how the owner checks the flow (Article 7), so every cell and every block is on it, and what a cell does with a block
// is a mark where they cross. What runs a cell is a trigger, one to a row over the data's rows: a write of the block a
// trigger stands on, by a cell right of one the trigger runs, is the work going back, and its line is drawn leftwards
// along the trigger's row to each cell it sends the work back to, in the way back's purple, so that every way back
// shows at a glance. The blocks' names stay in place as the chart scrolls sideways, and the cells' heads as it scrolls
// down, in the chart's own frame.
//
// A cell's column holds only marks, so it is only as wide as they are, and its head holds the cell's id alone. The
// width this leaves, right of the marks, is a column of words, one line to a row: what the row's thing is, or, on the
// rows of the cell chosen, what that cell says of it. What a head has no room for, the cell's title, its line, when it
// runs and what it waits for, stands over that column for the cell under the pointer.
//
// Over the blocks stand bands of rows, each thing drawn once: the lines, one to a line the agents' cells name, with a
// dot under each cell on it, the dots joined along the row, so that a joined row reads as one session going on; and
// the conditions, one to a named test or a block a cell's condition names: under a cell whose `when` it is, a tick
// where the cell runs when the test holds or the block is yes, a cross where it runs when the block is no, and the
// value itself where it waits for another; and a square under each cell that `requires` the test, which the work waits
// before until it holds. The engine's tools are the flow's, alike for every agent's cell, and are named once, in the
// view's line.

import { type CSSProperties, type ReactNode, type RefObject, useEffect, useRef, useState } from 'react';

import type { Block, Cell, InputKind } from './api';
import {
  type BlockRow,
  type ConditionRow,
  type ConditionSpot,
  type LineRow,
  type LineSpot,
  type Mark,
  MARK_PARTS,
  type MarkPart,
  type Matrix,
  NO_MARK,
  type Spot,
  hasMark,
} from './matrix';
import { Edge, Sign } from './parts';
import { WIDTH_KEYS, useKeptWidth, useKeptWidths } from './sizes';
import { cellSays, headFacts, inputKindWords, keysWords, lineKeys } from './words';

/// What the owner chose: a cell, a block, a line, a named test, one of the engine's tools, or nothing.
export type Chosen =
  { cell: string } | { block: string } | { line: string } | { test: string } | { tool: string } | null;

/// The key a chosen thing's head is found by on the chart; none for a tool, which has no row.
function keyOf(chosen: NonNullable<Chosen>): string | null {
  if ('cell' in chosen) return `cell:${chosen.cell}`;
  if ('block' in chosen) return `block:${chosen.block}`;
  if ('line' in chosen) return `line:${chosen.line}`;
  if ('test' in chosen) return `test:${chosen.test}`;
  return null;
}

const PART_WORDS: Record<MarkPart, string> = {
  runs: 'triggered_by',
  given: 'context',
  write: 'outputs',
  condition: 'when',
  precondition: 'requires',
};

/// The words a mark says, for the pointer's tip and for a reader of the screen, by the cell's keys: on a datum's row
/// the keys that name the datum, then the datum (`plan context latest, outputs plan`); on a trigger's row each key with
/// what it names, the trigger or the block it stands on (`judge triggered_by on_plan`, `plan outputs plan`).
export function markWords(cell: string, block: string, spot: Pick<Spot, 'mark'>, trigger?: string): string {
  if (trigger !== undefined) {
    const named = [
      ...(spot.mark.runs ? [`${PART_WORDS.runs} ${trigger}`] : []),
      ...(spot.mark.write ? [`${PART_WORDS.write} ${block}`] : []),
    ];
    return named.length > 0 ? `${cell} ${named.join(', ')}` : '';
  }
  const parts = MARK_PARTS.flatMap((part) => {
    if (!spot.mark[part]) return [];
    return part === 'given' && spot.mark.kind
      ? [`${PART_WORDS[part]} ${inputKindWords(spot.mark.kind)}`]
      : [PART_WORDS[part]];
  });
  return parts.length > 0 ? `${cell} ${parts.join(', ')} ${block}` : '';
}

/// A mark: a ring where the block runs the cell and the cell is given it, the ring's left half alone where the block
/// only runs the cell, its right half alone where the cell is only given the block; a dot where the cell may write
/// it, inside the ring or a half of it where both; a diamond round them where its condition reads the block's value,
/// and a square round them where a precondition does.
export function MarkGlyph({ spot }: { spot: Spot }) {
  const { runs, given, write, condition, precondition } = spot.mark;
  if (!hasMark(spot.mark)) return null;
  return (
    <svg className="glyph" viewBox="0 0 24 24" aria-hidden="true">
      {condition && <polygon className="part condition" points="12,1.5 22.5,12 12,22.5 1.5,12" />}
      {precondition && <rect className="part precondition" x="3.5" y="3.5" width="17" height="17" />}
      {runs && <path className="part runs" d="M12 5.5 A6.5 6.5 0 0 0 12 18.5" />}
      {given && <path className="part given" d="M12 5.5 A6.5 6.5 0 0 1 12 18.5" />}
      {write && <circle className="part write" cx="12" cy="12" r={runs || given ? 3.25 : 6} />}
    </svg>
  );
}

/// What a kind of context is drawn with inside its ring: the latest version, the last and the latest, every version.
const KIND_SIGNS: Record<InputKind, string> = {
  latest: '1',
  lastAndLatest: '2',
  all: '+',
};

/// A mark on a trigger's row: a ring where the trigger runs the cell, a disc where the cell writes the block the
/// trigger stands on, a ring with a dot where both.
export function TriggerGlyph({ spot }: { spot: Spot }) {
  const { runs, write } = spot.mark;
  if (!runs && !write) return null;
  return (
    <svg className="glyph" viewBox="0 0 24 24" aria-hidden="true">
      {runs && <circle className="part runs" cx="12" cy="12" r="6.5" />}
      {write && <circle className="part write" cx="12" cy="12" r={runs ? 3.25 : 6} />}
    </svg>
  );
}

/// The marks on a datum's row, one for each thing, side by side: a disc where the cell writes it; a ring with its
/// kind inside where the cell is given it.
export function DataGlyphs({ spot }: { spot: Spot }) {
  const m = spot.mark;
  if (!m.write && !m.given) return null;
  const box = (key: string, child: ReactNode) => (
    <svg key={key} className="glyph" viewBox="0 0 24 24" aria-hidden="true">
      {child}
    </svg>
  );
  return (
    <span className="glyphs">
      {m.write && box('write', <circle className="part write" cx="12" cy="12" r="6" />)}
      {m.given &&
        box(
          'given',
          <>
            <circle className="part given" cx="12" cy="12" r="8" />
            {m.kind && (
              <text className="part kind" x="12" y="12">
                {KIND_SIGNS[m.kind]}
              </text>
            )}
          </>,
        )}
    </span>
  );
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

/// The marks of a cell on a condition: where it is the cell's `when`, a tick where the cell runs when the test holds or
/// the block is yes, a cross where it runs when the block is no, and the value itself where it runs on another; a
/// square where the work waits before the cell until the test holds.
export function ConditionGlyph({ spot }: { spot: ConditionSpot }) {
  const { when, value, requires } = spot;
  if (!when && !requires) return null;
  const box = (key: string, child: ReactNode) => (
    <svg key={key} className="glyph" viewBox="0 0 24 24" aria-hidden="true">
      {child}
    </svg>
  );
  const whenMark = () => {
    if (value === undefined || value === 'yes') {
      return box('when', <path className="part holds" d="M5.5 12.5 L10 17 L18.5 7" />);
    }
    if (value === 'no') return box('when', <path className="part holds" d="M7 7 L17 17 M17 7 L7 17" />);
    return (
      <span key="when" className="spot-value">
        {value}
      </span>
    );
  };
  return (
    <span className="glyphs">
      {when && whenMark()}
      {requires && box('requires', <rect className="part precondition" x="3.5" y="3.5" width="17" height="17" />)}
    </span>
  );
}

/// The legend of the marks and signs, in the view's line.
export function Legend({ waysBack }: { waysBack: number }) {
  const spot = (mark: Partial<Mark>): Spot => ({
    mark: { ...NO_MARK, ...mark },
    line: 'none',
    back: 'none',
    backTo: false,
  });
  const condition = (when: boolean, requires: boolean, value?: string): ConditionSpot => ({ when, value, requires });
  return (
    <ul className="legend" aria-label="What the marks mean">
      <li>
        <LineGlyph on /> line
      </li>
      <li>
        <ConditionGlyph spot={condition(true, false)} /> when yes, when test
      </li>
      <li>
        <ConditionGlyph spot={condition(true, false, 'no')} /> when no
      </li>
      <li>
        <ConditionGlyph spot={condition(false, true)} /> requires
      </li>
      <li>
        <TriggerGlyph spot={spot({ runs: true })} /> triggered_by
      </li>
      <li>
        <TriggerGlyph spot={spot({ write: true })} /> outputs
      </li>
      <li>
        <TriggerGlyph spot={spot({ runs: true, write: true })} /> triggered_by, outputs
      </li>
      <li>
        <span className="legend-back" aria-hidden="true" /> way back ({waysBack})
      </li>
      <li>
        <DataGlyphs spot={spot({ given: true, kind: 'latest' })} /> context latest
      </li>
      <li>
        <DataGlyphs spot={spot({ given: true, kind: 'lastAndLatest' })} /> context last_and_latest
      </li>
      <li>
        <DataGlyphs spot={spot({ given: true, kind: 'all' })} /> context all
      </li>
      <li>
        <Sign name="people" /> people
      </li>
      <li>
        <Sign name="outside" /> outside
      </li>
      <li>
        <Sign name="shown" /> shown
      </li>
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
export const WIDTHS = {
  names: 200,
  namesMin: 120,
  namesMax: 480,
  cellMin: 88,
  cellShare: 160,
  cellMax: 480,
};

/// The widths of the flow's chart, in pixels: a cell's column until the owner drags it, which is also the least it is
/// dragged to, wide enough for its marks and a short id; and the least the column of words keeps, the frame scrolling
/// sideways below that.
export const FLOW_WIDTHS = { cell: 56, words: 280 };

/// How wide the table is: the whole width of its frame, the cells sharing evenly what the names' column and the cells
/// whose width was dragged leave, each up to `cellShare`, so that a few cells on a wide window do not grow absurd; and
/// at least as wide as every column at its least, the frame scrolling sideways below that.
export function tableWidth(
  names: number,
  cells: string[],
  dragged: Record<string, number>,
): { width: string; minWidth: string } {
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
  const { blocks, cells, spots, lines, conditions } = matrix;
  const isTrigger = (row: BlockRow) => row.trigger !== undefined;
  const [pointed, setPointed] = useState<number | null>(null);
  const frame = useRef<HTMLDivElement>(null);
  const [names, setNames] = useKeptWidth(WIDTH_KEYS.names, WIDTHS.namesMin, WIDTHS.namesMax);
  const [widths, setWidth] = useKeptWidths(WIDTH_KEYS.flowCells, FLOW_WIDTHS.cell, WIDTHS.cellMax);
  useWheel(frame);
  const namesWidth = names ?? WIDTHS.names;
  const widthOf = (id: string) => widths[id] ?? FLOW_WIDTHS.cell;
  // The table takes the frame's whole width: the names' column and the cells' at theirs, the words the rest.
  const taken = cells.reduce((sum, cell) => sum + widthOf(cell.id), namesWidth);
  const size = { width: '100%', minWidth: taken + FLOW_WIDTHS.words };

  // What is chosen elsewhere (a panel) is brought into view in the frame. A block is found by its datum's row, or else
  // by the row of a trigger that stands on it, which a block standing only for a trigger has alone.
  useEffect(() => {
    const key = chosen && keyOf(chosen);
    if (!key) return;
    const target =
      frame.current?.querySelector(`[data-key="${CSS.escape(key)}"]`) ??
      (chosen && 'block' in chosen ? frame.current?.querySelector(`[data-on="${CSS.escape(chosen.block)}"]`) : null);
    target?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }, [chosen]);

  const chosenCell = chosen && 'cell' in chosen ? chosen.cell : null;
  const chosenBlock = chosen && 'block' in chosen ? chosen.block : null;
  const chosenLine = chosen && 'line' in chosen ? chosen.line : null;
  const chosenTest = chosen && 'test' in chosen ? chosen.test : null;
  // The cell whose words the rows show, and the cell the head of the words is about: the one under the pointer, or
  // else the one chosen.
  const saying = cells.find((c) => c.id === chosenCell) ?? null;
  const about = (pointed !== null ? cells[pointed] : undefined) ?? saying;
  const unpoint = () => setPointed(null);

  return (
    <div className="frame" ref={frame} onMouseLeave={unpoint}>
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
            <th
              className="lead corner"
              scope="col"
              aria-label="Blocks down the side, cells across"
              onMouseEnter={unpoint}
            >
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
                cells={cells}
                chosen={chosenLine === row.name}
                onChoose={() => onChoose(chosenLine === row.name ? null : { line: row.name })}
                chosenCell={chosenCell}
                saying={saying}
                pointed={pointed}
                onPoint={setPointed}
              />
            ))}
          </tbody>
        )}
        {conditions.length > 0 && (
          <tbody className="band">
            {conditions.map((row) => {
              const isChosen = row.test ? chosenTest === row.name : chosenBlock === row.block;
              const chosenNow: Chosen = row.test ? { test: row.name } : { block: row.name };
              const declared = row.test ? undefined : blocks.find((b) => !b.trigger && b.name === row.name)?.block;
              return (
                <ConditionLine
                  key={`${row.test ? 'test' : 'block'}:${row.name}`}
                  row={row}
                  declared={declared}
                  cells={cells}
                  chosen={isChosen}
                  onChoose={() => onChoose(isChosen ? null : chosenNow)}
                  chosenCell={chosenCell}
                  saying={saying}
                  pointed={pointed}
                  onPoint={setPointed}
                />
              );
            })}
          </tbody>
        )}
        {[true, false].map((trigger) => (
          <tbody key={trigger ? 'triggers' : 'blocks'} className={trigger ? 'band triggers' : undefined}>
            {blocks.map((row, r) => {
              if (isTrigger(row) !== trigger) return null;
              const isChosen = chosenBlock === row.on;
              const what = trigger ? (row.what ?? '') : (row.block.what ?? '');
              // What the chosen cell says of the row's thing: of a trigger, what it does when the trigger fires; of a
              // datum, its keys, and of one it may write, with what the datum is, which is what the cell is to hand in.
              const says = !saying
                ? ''
                : trigger
                  ? ((saying.triggers ?? []).find((t) => t.name === row.trigger && t.block === row.on)?.do ?? '')
                  : cellSays(saying, row.on);
              const said = !trigger && says && what && saying?.outputs.includes(row.on) ? `${says}: ${what}` : says;
              return (
                <tr key={trigger ? `${row.trigger}:${row.on}` : row.on} className={`row ${isChosen ? 'chosen' : ''}`}>
                  <BlockHead
                    row={row}
                    label={trigger ? row.name : null}
                    chosen={isChosen}
                    onChoose={() => onChoose(isChosen ? null : { block: row.on })}
                    onPoint={unpoint}
                  />
                  {cells.map((cell, c) => (
                    <SpotCell
                      key={cell.id}
                      spot={spots[r][c]}
                      trigger={row.trigger}
                      cell={cell.id}
                      block={row.on}
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
        ))}
      </table>
    </div>
  );
}

/// The head of a cell's column: its id, which is all a column as wide as its marks has room for. Its title, its line,
/// when it runs and what it waits for are what its button is called, over the column of words while the pointer or the
/// keyboard is on the column, and in the cell's panel; its line, its `when` and its `requires` are drawn in the bands
/// over the blocks.
function CellHead(props: {
  cell: Cell;
  chosen: boolean;
  pointed: boolean;
  onChoose: () => void;
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
/// until a cell is chosen, and then what that cell says of each it has to do with.
function WordsHead(props: { about: Cell | null; saying: Cell | null; onPoint: () => void }) {
  const { about: cell, saying, onPoint } = props;
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
        {saying ? `What ${saying.id} says of each row` : 'What each row is'}
      </span>
    </th>
  );
}

/// The name of a block at the left of its row, with its signs, the source outside the work that writes it and the
/// values of a choice after it; or of a trigger, which is found by the block it stands on, and chooses that block.
function BlockHead(props: {
  row: BlockRow;
  /// Of a row that stands only for a trigger, the trigger's name.
  label: string | null;
  chosen: boolean;
  onChoose: () => void;
  onPoint: () => void;
}) {
  const { row, label, chosen, onChoose, onPoint } = props;
  const block = row.block;
  return (
    <th
      scope="row"
      className="lead block-head"
      data-key={row.trigger !== undefined ? `trigger:${row.trigger}:${row.on}` : `block:${row.on}`}
      data-on={row.trigger !== undefined ? row.on : undefined}
      onMouseEnter={onPoint}
    >
      <button
        type="button"
        className="head-button"
        aria-pressed={chosen}
        aria-label={row.trigger !== undefined ? `Trigger ${row.trigger}, block ${row.on}` : `Block ${row.on}`}
        onClick={onChoose}
      >
        <span className="block-name" title={row.name}>
          {label ?? row.name}
        </span>
        {label !== null && <span className="row-label">trigger</span>}
        {block.writtenBy.people && <Sign name="people" />}
        {block.writtenBy.outside !== null && (
          <span title={`from ${block.writtenBy.outside}`}>
            <Sign name="outside" />
          </span>
        )}
        {label === null && block.readBy.shown && <Sign name="shown" />}
        {label === null && block.values && (
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

/// The row of a line, at the top: its name in the accent, a dot under each cell on it, the dots joined along the row,
/// and the line's keys as its file writes them; on the row of the chosen cell's line, that it is its `line`. Chosen by
/// its name, it opens the line's panel.
function LineLine(props: {
  row: LineRow;
  cells: Cell[];
  chosen: boolean;
  onChoose: () => void;
  chosenCell: string | null;
  saying: Cell | null;
  pointed: number | null;
  onPoint: (c: number | null) => void;
}) {
  const { row, cells, chosen, onChoose, chosenCell, saying, pointed, onPoint } = props;
  const keys = row.line ? keysWords(lineKeys(row.line)) : '';
  const says = saying?.kind === 'agent' && saying.line === row.name ? 'line' : '';
  return (
    <tr className={`row line-row ${chosen ? 'chosen' : ''}`}>
      <th scope="row" className="lead block-head" data-key={`line:${row.name}`} onMouseEnter={() => onPoint(null)}>
        <button
          type="button"
          className="head-button"
          aria-pressed={chosen}
          aria-label={`Line ${row.name}`}
          onClick={onChoose}
        >
          <span className="block-name line-name" title={row.name}>
            {row.name}
          </span>
          <span className="row-label">line</span>
        </button>
      </th>
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
      <td
        className={`words ${says ? 'said' : saying ? 'beside' : ''}`}
        title={says || keys || undefined}
        onMouseEnter={() => onPoint(null)}
      >
        {says || keys}
      </td>
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

/// The row of a condition, under the lines: a named test, or a block a cell's `when` reads, by its name; under each
/// cell whose `when` it is, a tick, a cross or the value (`ConditionGlyph`), and a square under each that `requires`
/// it; and what the test `says`, or the block's `default`. On the row of a condition the chosen cell names, by which
/// key it names it. Chosen by its name, it opens the test's panel, or the block's.
function ConditionLine(props: {
  row: ConditionRow;
  /// Of a block's row, the block as the flow declares it.
  declared?: Block;
  cells: Cell[];
  chosen: boolean;
  onChoose: () => void;
  chosenCell: string | null;
  saying: Cell | null;
  pointed: number | null;
  onPoint: (c: number | null) => void;
}) {
  const { row, declared, cells, chosen, onChoose, chosenCell, saying, pointed, onPoint } = props;
  // How a cell's `when` names the row's condition: by the test, or by the block with its value.
  const whenWords = (value?: string) => (row.test ? 'when' : `when ${row.name} = ${value}`);
  const at = saying ? cells.findIndex((cell) => cell.id === saying.id) : -1;
  const own = at === -1 ? undefined : row.spots[at];
  const says = own
    ? [...(own.when ? [whenWords(own.value)] : []), ...(own.requires ? ['requires'] : [])].join('; ')
    : '';
  const what = row.test ? row.test.words : declared?.default ? `default ${declared.default}` : '';
  return (
    <tr className={`row condition-row ${chosen ? 'chosen' : ''}`}>
      <th
        scope="row"
        className="lead block-head"
        data-key={row.test ? `test:${row.name}` : undefined}
        onMouseEnter={() => onPoint(null)}
      >
        <button
          type="button"
          className="head-button"
          aria-pressed={chosen}
          aria-label={row.test ? `Test ${row.name}` : `Block ${row.name}`}
          onClick={onChoose}
        >
          <span className="block-name" title={row.name}>
            {row.name}
          </span>
          <span className="row-label">{row.test ? 'test' : 'block'}</span>
        </button>
      </th>
      {cells.map((cell, c) => {
        const spot = row.spots[c];
        const words = [
          spot.when ? `${cell.id} ${whenWords(spot.value)}${row.test ? ` ${row.name}` : ''}` : '',
          spot.requires ? `${cell.id} requires ${row.name}` : '',
        ]
          .filter((w) => w !== '')
          .join(', ');
        return (
          <td
            key={cell.id}
            className={bandSpot(cell, chosenCell, pointed === c)}
            title={words || undefined}
            onMouseEnter={() => onPoint(c)}
          >
            <ConditionGlyph spot={spot} />
            {words && <span className="visually-hidden">{words}</span>}
          </td>
        );
      })}
      <td
        className={`words ${says ? 'said' : saying ? 'beside' : ''}`}
        title={says || what || undefined}
        onMouseEnter={() => onPoint(null)}
      >
        {says || what}
      </td>
    </tr>
  );
}

function SpotCell(props: {
  spot: Spot;
  /// Of a crossing on a trigger's row, the trigger's name.
  trigger?: string;
  cell: string;
  block: string;
  chosen: boolean;
  pointed: boolean;
  onPoint: () => void;
}) {
  const { spot, trigger, cell, block, chosen, pointed, onPoint } = props;
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
  const words = markWords(cell, block, spot, trigger);
  return (
    <td
      className={`spot ${hasMark(spot.mark) ? 'has-mark' : ''} ${chosen ? 'chosen' : ''} ${pointed ? 'pointed' : ''}`}
      onMouseEnter={onPoint}
      title={words || undefined}
    >
      {half('left')}
      {half('right')}
      {spot.backTo && <span className={`arrow ${arrowLane}`} aria-hidden="true" />}
      {trigger !== undefined ? <TriggerGlyph spot={spot} /> : <DataGlyphs spot={spot} />}
      {words && <span className="visually-hidden">{words}</span>}
    </td>
  );
}
