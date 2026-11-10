// The chart: the matrix of the cells, in the order of the walk, against the blocks they read and write. It is how the
// owner checks the flow (Article 7), so every cell and every block is on it, and what a cell does with a block is a
// mark where they cross. A write below a read of the same block, by a later cell, is the work going back: its rail is
// drawn up the column to each read it sends the work back to, so that every way back shows at a glance. The cells'
// own columns stay in place as the blocks scroll under them in the chart's own frame.

import { CopilotIcon, EyeIcon, GearIcon, GlobeIcon, PersonIcon } from '@primer/octicons-react';
import { Label } from '@primer/react';
import { useEffect, useRef, useState } from 'react';

import type { Cell } from './api';
import { type Column, type Mark, MARK_PARTS, hasMark, type MarkPart, type Matrix, type Row, type Spot, type Status } from './matrix';
import { conditionWords } from './words';

/// What the owner chose: a cell, a block, or nothing.
export type Chosen = { cell: string } | { block: string } | null;

/// The words of a status, the same on a row, a column's head and the panels.
export const STATUS_WORDS: Record<Exclude<Status, 'same'>, { words: string; variant: 'success' | 'danger' | 'attention' }> =
  {
    added: { words: 'put in', variant: 'success' },
    removed: { words: 'taken out', variant: 'danger' },
    changed: { words: 'changed', variant: 'attention' },
  };

export function StatusLabel({ status }: { status: Status }) {
  if (status === 'same') return null;
  const { words, variant } = STATUS_WORDS[status];
  return (
    <Label variant={variant} size="small" className="status">
      {words}
    </Label>
  );
}

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
    <svg className="glyph" viewBox="0 0 24 24" width="24" height="24" aria-hidden="true">
      {condition && <polygon className={`part condition ${condition}`} points="12,1.5 22.5,12 12,22.5 1.5,12" />}
      {read && <circle className={`part read ${read}`} cx="12" cy="12" r="6" />}
      {write && <circle className={`part write ${write}`} cx="12" cy="12" r={read ? 3 : 5.5} />}
    </svg>
  );
}

/// The legend of the marks, which the bar of the page carries in one line.
export function Legend({ waysBack }: { waysBack: number }) {
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
        <PersonIcon size={12} /> people write
      </li>
      <li>
        <GlobeIcon size={12} /> the outside writes
      </li>
      <li>
        <EyeIcon size={12} /> shown
      </li>
    </ul>
  );
}

/// Who runs a cell: an agent in its line, or the engine.
export function RunBy({ cell }: { cell: Cell }) {
  return cell.kind === 'agent' ? (
    <span className="run-by">
      <CopilotIcon size={12} aria-label="agent" /> {cell.line}
    </span>
  ) : (
    <span className="run-by">
      <GearIcon size={12} aria-label="engine" /> engine
    </span>
  );
}

interface ChartProps {
  matrix: Matrix;
  chosen: Chosen;
  onChoose: (chosen: Chosen) => void;
}

export function Chart({ matrix, chosen, onChoose }: ChartProps) {
  const { rows, columns, spots } = matrix;
  const [pointed, setPointed] = useState<number | null>(null);
  const frame = useRef<HTMLDivElement>(null);
  const cellOf = (row: Row) => row.cell ?? row.other;
  const hasCondition = rows.some((r) => cellOf(r)?.condition);
  const hasPreconditions = rows.some((r) => (cellOf(r)?.preconditions.length ?? 0) > 0);

  // What is chosen elsewhere (the list of changes, a panel) is brought into view in the frame.
  useEffect(() => {
    if (!chosen) return;
    const key = 'cell' in chosen ? `cell:${chosen.cell}` : `block:${chosen.block}`;
    const target = frame.current?.querySelector(`[data-key="${CSS.escape(key)}"]`);
    target?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }, [chosen]);

  const chosenCell = chosen && 'cell' in chosen ? chosen.cell : null;
  const chosenBlock = chosen && 'block' in chosen ? chosen.block : null;
  const lead = ['lead-cell', ...(hasCondition ? ['lead-condition'] : []), ...(hasPreconditions ? ['lead-pre'] : [])];
  const leadClass = (i: number) => `lead ${lead[i]} ${i === lead.length - 1 ? 'lead-last' : ''}`;

  return (
    <div className="frame" ref={frame} onMouseLeave={() => setPointed(null)}>
      <table className={`matrix leads-${lead.length}`}>
        <thead>
          <tr>
            <th className={leadClass(0)} scope="col">
              Cell
            </th>
            {hasCondition && (
              <th className={leadClass(1)} scope="col">
                Runs when
              </th>
            )}
            {hasPreconditions && (
              <th className={leadClass(lead.length - 1)} scope="col">
                Waits until
              </th>
            )}
            {columns.map((column, c) => (
              <BlockHead
                key={column.name}
                column={column}
                chosen={chosenBlock === column.name}
                pointed={pointed === c}
                onChoose={() => onChoose(chosenBlock === column.name ? null : { block: column.name })}
                onPoint={() => setPointed(c)}
              />
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, r) => {
            const cell = cellOf(row);
            if (!cell) return null;
            const isChosen = chosenCell === row.id;
            const absent = !row.cell;
            return (
              <tr
                key={row.id}
                className={`row status-${row.status} ${absent ? 'absent' : ''} ${isChosen ? 'chosen' : ''}`}
              >
                <th className={leadClass(0)} scope="row" data-key={`cell:${row.id}`}>
                  <button
                    type="button"
                    className="cell-button"
                    aria-pressed={isChosen}
                    onClick={() => onChoose(isChosen ? null : { cell: row.id })}
                  >
                    <span className="cell-line">
                      <span className={`cell-id ${row.changed.has('cellMoved') ? 'changed-field' : ''}`}>
                        {row.id}
                      </span>
                      <span className={`cell-title ${row.changed.has('title') ? 'changed-field' : ''}`}>
                        {cell.title}
                      </span>
                    </span>
                    <span
                      className={`cell-sub ${row.changed.has('kind') || row.changed.has('line') ? 'changed-field' : ''}`}
                    >
                      <RunBy cell={cell} />
                      <StatusLabel status={row.status} />
                    </span>
                  </button>
                </th>
                {hasCondition && (
                  <td className={`${leadClass(1)} ${row.changed.has('condition') ? 'changed-field' : ''}`}>
                    {cell.condition && conditionWords(cell.condition)}
                  </td>
                )}
                {hasPreconditions && (
                  <td
                    className={`${leadClass(lead.length - 1)} ${
                      row.changed.has('preconditionAdded') || row.changed.has('preconditionRemoved')
                        ? 'changed-field'
                        : ''
                    }`}
                  >
                    {cell.preconditions.map((p) => (
                      <div key={p.name}>{p.words}</div>
                    ))}
                  </td>
                )}
                {columns.map((column, c) => (
                  <SpotCell
                    key={column.name}
                    spot={spots[r][c]}
                    cell={row.id}
                    column={column}
                    chosen={chosenBlock === column.name}
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

function BlockHead(props: {
  column: Column;
  chosen: boolean;
  pointed: boolean;
  onChoose: () => void;
  onPoint: () => void;
}) {
  const { column, chosen, pointed, onChoose, onPoint } = props;
  const block = column.block ?? column.other;
  if (!block) return null;
  const changed = (kind: 'values' | 'people' | 'outside' | 'shown') => (column.changed.has(kind) ? 'changed-field' : '');
  return (
    <th
      scope="col"
      className={`block-head status-${column.status} ${column.block ? '' : 'absent'} ${chosen ? 'chosen' : ''} ${
        pointed ? 'pointed' : ''
      }`}
      data-key={`block:${column.name}`}
      onMouseEnter={onPoint}
    >
      <button
        type="button"
        className="head-button"
        aria-pressed={chosen}
        aria-label={`Block ${column.name}${column.status === 'same' ? '' : `, ${STATUS_WORDS[column.status].words}`}`}
        title={column.status === 'same' ? column.name : `${column.name}: ${STATUS_WORDS[column.status].words}`}
        onClick={onChoose}
      >
        <span className="who">
          {block.writtenBy.people && <PersonIcon size={12} aria-label="people write it" className={changed('people')} />}
          {block.writtenBy.outside && (
            <GlobeIcon size={12} aria-label="the outside writes it" className={changed('outside')} />
          )}
          {block.readBy.shown && <EyeIcon size={12} aria-label="shown" className={changed('shown')} />}
        </span>
        <span className="vertical">
          <span className="block-name">{column.name}</span>
          {block.values && <span className={`block-values ${changed('values')}`}>{block.values.join(' · ')}</span>}
        </span>
      </button>
    </th>
  );
}

function SpotCell(props: {
  spot: Spot;
  cell: string;
  column: Column;
  chosen: boolean;
  pointed: boolean;
  onPoint: () => void;
}) {
  const { spot, cell, column, chosen, pointed, onPoint } = props;
  // The upper half of the crossing carries the way back when it does, the column's line otherwise; so the lower.
  const upper = spot.back === 'up' || spot.back === 'through' ? 'back' : spot.line === 'up' || spot.line === 'through' ? 'line' : null;
  const lower =
    spot.back === 'down' || spot.back === 'through' ? 'back' : spot.line === 'down' || spot.line === 'through' ? 'line' : null;
  const words = markWords(cell, column.name, spot);
  const marked = hasMark(spot.mark) || hasMark(spot.added) || hasMark(spot.removed);
  return (
    <td
      className={`spot ${marked ? 'has-mark' : ''} ${column.block ? '' : 'absent'} ${chosen ? 'chosen' : ''} ${
        pointed ? 'pointed' : ''
      }`}
      onMouseEnter={onPoint}
      title={words || undefined}
    >
      {upper && <span className={`seg seg-up ${upper}`} aria-hidden="true" />}
      {lower && <span className={`seg seg-down ${lower}`} aria-hidden="true" />}
      {spot.backTo && <span className="arrow" aria-hidden="true" />}
      <MarkGlyph spot={spot} />
      {words && <span className="visually-hidden">{words}</span>}
    </td>
  );
}
