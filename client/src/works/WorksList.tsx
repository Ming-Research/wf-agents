// The list of works: the record of every work, ended or not, one to a row, for the owner to look back at; what needs
// the owner now is the console's. A row says the item's number and title, the work's state and the cell it stands at,
// how each cell of its flow stands, the level its rounds run at and the level of its cell, as the engine gives them,
// what it cost, how long its rounds took and when something last happened; it opens the work. The list is ordered by
// when something last happened, the latest first, until the owner presses the head of another column, each but the
// strip's having an order, a second press turning it; nothing of a work's state orders it but its own column. The
// order lasts while the list is shown. The owner drags the width of each column by its head's right edge, kept in the
// browser.
// Where the installation names more than one project, a choice in the view's line shows the works of one project's
// items, with every work that waits for the owner whatever its project, and says how many it shows; the count of the
// works, and the one count of what waits for the owner, the bar's, are always of every work. The choice lasts while the list is shown: it is
// kept nowhere. The list is read again on each change of an item or a work, and whenever the stream of changes opens
// again, the projects with it; a read that fails keeps the list as it was and says so. With several hundred works, a change of one draws its row again and
// leaves the others as they are: the items are read once for the list and the bar, each that did not change kept as
// it was (source.ts), and a row is drawn again only when its item is another.

import { type CSSProperties, memo, useCallback, useMemo, useRef, useState } from 'react';

import { Arrow, Edge } from '../parts';
import { WIDTH_KEYS, useKeptWidths } from '../sizes';
import type { Item, Items } from '../works-api';
import { NowLine, StateLegend, Strip } from './marks';
import { LAST_FIRST, type Order, type OrderBy, orderWorks, wayOf } from './placing';
import { useChanges, useItems, useReading, useSource } from './source';
import { waitingCount, waitsOnOwner } from './waiting';
import { NO_LEVEL, ORDER_WORDS, clock, dollars, duration, fullTime, stoppedAt, waitCount } from './words';

/// No change the stream tells of is a change of the projects.
const noChange = () => undefined;

export function WorksList() {
  const { value: items, failed } = useItems();
  const source = useSource();
  const loadNames = useCallback(() => source.loadProjects(), [source]);
  const names = useReading(loadNames);
  useChanges(noChange, names.read);

  if (!items) {
    return failed ? (
      <p className="message">The page could not read the works: {failed}</p>
    ) : (
      <p className="message muted">Reading the works</p>
    );
  }
  // Where the projects cannot be read, or are not yet, there is no choice of one, and every work is shown.
  return <WorksTable items={items} projects={names.value?.names ?? []} failed={failed} />;
}

const open = (work: number) => {
  window.location.hash = `#/works/${work}`;
};

/// A column of the list: its key, by which its width is kept; its head's name; whether it orders the list; the least
/// width it is dragged to; and the class of its head and cells, by which its width is set and the narrower windows
/// leave it out.
interface Column {
  key: string;
  name: string;
  by?: OrderBy;
  least: number;
  className: string;
}

/// The list's columns, in order. Each is as wide as what it holds until the owner drags it, the title as wide as the
/// others leave; each keeps at least the width of its head.
const COLUMNS: Column[] = [
  { key: 'item', name: '#', by: 'item', least: 56, className: 'col-item' },
  { key: 'title', name: 'Title', by: 'title', least: 120, className: 'col-title' },
  { key: 'state', name: 'State', by: 'state', least: 104, className: 'col-state' },
  { key: 'cells', name: 'Cells', least: 72, className: 'col-cells' },
  { key: 'level', name: 'Level', by: 'level', least: 80, className: 'col-level' },
  { key: 'cellLevel', name: 'Cell level', by: 'cellLevel', least: 120, className: 'col-cell-level' },
  { key: 'cost', name: 'Cost', by: 'cost', least: 80, className: 'col-cost' },
  { key: 'time', name: 'Time', by: 'time', least: 80, className: 'col-time' },
  { key: 'last', name: 'Last', by: 'last', least: 80, className: 'col-last' },
];

/// The least and the most a column of the list is dragged to; each keeps its own least besides.
const WIDTHS = { least: 56, most: 1600 };

/// A column's head, with the edge that drags its width. One that orders the list is a button, which orders it by the
/// column the way its first press gives, or, where it orders the list so already, the other way, and carries the
/// arrow of the order while it orders the list; one that does not is its name alone.
function Head({ column, order, onOrder, onWidth }: {
  column: Column;
  order: Order;
  onOrder: (order: Order) => void;
  onWidth: (width: number | null) => void;
}) {
  const head = useRef<HTMLTableCellElement>(null);
  const { name, by, least, className } = column;
  const edge = (
    <Edge
      side="right"
      label={`Drag the width of the column ${name}`}
      start={() => head.current?.getBoundingClientRect().width ?? least}
      onDrag={onWidth}
      onReset={() => onWidth(null)}
    />
  );
  if (by === undefined) {
    return (
      <th ref={head} scope="col" className={`label ${className}`}>
        {name}
        {edge}
      </th>
    );
  }
  const orders = order.by === by;
  const next: Order = { by, turned: orders && !order.turned };
  return (
    <th ref={head} scope="col" className={`label ${className}`} aria-sort={orders ? wayOf(order) : undefined}>
      <button type="button" className="order-head" title={ORDER_WORDS[by][wayOf(next)]} onClick={() => onOrder(next)}>
        {name}
        {orders && <Arrow to={wayOf(order) === 'ascending' ? 'up' : 'down'} />}
      </button>
      {edge}
    </th>
  );
}

/// A level as the engine gives it, or a muted dash where it gives none, saying why on pointing at it.
function Level({ level, none }: { level: string | null | undefined; none: string }) {
  return level === null || level === undefined ? (
    <span className="muted" title={none}>
      {NO_LEVEL.shown}
    </span>
  ) : (
    <>{level}</>
  );
}

export function WorksTable({ items, projects = [], failed = null }: {
  items: Items;
  /// The projects the installation names.
  projects?: string[];
  failed?: string | null;
}) {
  const [picked, pick] = useState<string | null>(null);
  const [order, setOrder] = useState<Order>(LAST_FIRST);
  const [widths, setWidth] = useKeptWidths(WIDTH_KEYS.worksColumns, WIDTHS.least, WIDTHS.most);
  // The widths the owner dragged, each at least its column's least, set on the table for every row's cells; while the
  // title's is set, the table is as wide as its columns, and else as the window, the title taking what is left.
  const style: Record<string, string> = Object.fromEntries(
    COLUMNS.flatMap((c) => {
      const w = widths[c.key];
      return w === undefined ? [] : [[`--${c.className}`, `${Math.max(c.least, w)}px`]];
    }),
  );
  if (widths.title !== undefined) style.width = 'auto';
  // The project chosen, while the installation names it among others; none shows every work.
  const chosen = projects.length > 1 && picked !== null && projects.includes(picked) ? picked : null;
  const all = useMemo(() => orderWorks(items.items, order), [items, order]);
  const waiting = waitingCount(all.map((w) => w.work));
  // A work that waits for the owner is shown whatever project is chosen.
  const works = useMemo(
    () => all.filter((w) => chosen === null || w.item.project === chosen || waitsOnOwner(w.work)),
    [all, chosen],
  );
  return (
    <>
      <div className="view-line">
        <span className="view-name">Works</span>
        {projects.length > 1 && (
          <label title="Show the works of one project's items, and every work waiting for the owner">
            <span className="muted mono">project</span>{' '}
            <select
              className="select mono"
              value={chosen ?? ''}
              onChange={(e) => pick(e.target.value === '' ? null : e.target.value)}
            >
              <option value="">all</option>
              {projects.map((name) => (
                <option key={name} value={name}>
                  {name}
                </option>
              ))}
            </select>
          </label>
        )}
        <span className="view-facts">
          {all.length} works, {waitCount(waiting)}
          {chosen !== null && `, ${works.length} shown`}
        </span>
        <span className="view-end works-legend">
          <StateLegend />
        </span>
      </div>
      {failed !== null && (
        <p className="read-failed" role="alert">
          The works could not be read again: {failed}. What is shown is from before.
        </p>
      )}
      <main className="list-view">
        {works.length === 0 ? (
          <p className="message muted">
            {chosen === null ? 'No item has a work yet' : `No item of project ${chosen} has a work yet`}
          </p>
        ) : (
          <table className="works" style={style as CSSProperties}>
            <thead>
              <tr>
                {COLUMNS.map((column) => (
                  <Head
                    key={column.key}
                    column={column}
                    order={order}
                    onOrder={setOrder}
                    onWidth={(w) => setWidth(column.key, w === null ? null : Math.max(column.least, w))}
                  />
                ))}
              </tr>
            </thead>
            <tbody>
              {works.map(({ item }) => (
                <WorkRow key={item.work.number} item={item} />
              ))}
            </tbody>
          </table>
        )}
      </main>
    </>
  );
}

/// The row of an item's work, drawn again only when the item is another object, which it is only where it changed.
export const WorkRow = memo(function WorkRow({ item }: { item: Item }) {
  const work = item.work;
  return (
    <tr
      className={`work-row ${waitsOnOwner(work) ? 'for-owner' : ''}`}
      data-work={work.number}
      onClick={() => open(work.number)}
    >
      <td className="mono muted col-item">#{item.number}</td>
      <td className="work-title col-title">
        <a href={`#/works/${work.number}`} title={item.title} onClick={(e) => e.stopPropagation()}>
          {item.title}
        </a>
      </td>
      <td className="work-state col-state">
        <NowLine work={work} why={false} />
      </td>
      <td className="col-cells">
        <Strip cells={work.cells} stopped={stoppedAt(work)} />
      </td>
      <td className="mono col-level">
        <Level level={work.level} none={NO_LEVEL.level} />
      </td>
      <td className="mono col-cell-level">
        <Level level={work.cellLevel} none={NO_LEVEL.cellLevel} />
      </td>
      <td className="small muted col-cost">{dollars(work.spent.usdMicros)}</td>
      <td className="small muted col-time">{duration(work.spent.workingMs)}</td>
      <td className="small muted col-last" title={work.last === null ? undefined : fullTime(work.last)}>
        {work.last === null ? 'nothing yet' : clock(work.last)}
      </td>
    </tr>
  );
});
