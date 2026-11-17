// The list of works: the record of every work, ended or not, one to a row, for the owner to look back at; what needs
// the owner now is the console's. A row says the item's number and title, the work's state and the cell it stands at,
// how each cell of its flow stands, the level its rounds run at and the level of its cell, as the engine gives them,
// what it cost, how long its rounds took and when something last happened; it opens the work. The list is ordered by
// when something last happened, the latest first, until the owner presses the head of another column that orders it,
// a second press turning the order; nothing of a work's state orders it. The order lasts while the list is shown.
// Where the installation names more than one project, a choice in the view's line shows the works of one project's
// items, with every work that waits for the owner whatever its project, and says how many it shows; the count of the
// works, and the one count of what waits for the owner, the bar's, are always of every work. The choice lasts while the list is shown: it is
// kept nowhere. The list is read again on each change of an item or a work, and whenever the stream of changes opens
// again, the projects with it; a read that fails keeps the list as it was and says so. With several hundred works, a change of one draws its row again and
// leaves the others as they are: the items are read once for the list and the bar, each that did not change kept as
// it was (source.ts), and a row is drawn again only when its item is another.

import { memo, useCallback, useMemo, useState } from 'react';

import { Arrow } from '../parts';
import type { Item, Items } from '../works-api';
import { NowLine, StateLegend, Strip } from './marks';
import { LAST_FIRST, type Order, type OrderBy, orderWorks } from './placing';
import { useChanges, useItems, useReading, useSource } from './source';
import { waitingCount, waitsOnOwner } from './waiting';
import { LIST_HEADS, NO_LEVEL, clock, dollars, duration, fullTime, stoppedAt, waitCount } from './words';

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

/// A column of the list: its head's name; what orders the list by it, where it does, or else what its head says on
/// pointing at it; and the class of its head and cells, by which the narrower windows leave it out.
interface Column {
  name: string;
  by?: OrderBy;
  title?: string;
  className: string;
}

/// The list's columns, in order.
const COLUMNS: Column[] = [
  { name: '#', by: 'item', className: 'col-item' },
  { name: 'Title', className: 'col-title' },
  { name: 'State', className: 'col-state' },
  { name: 'Cells', className: 'col-cells' },
  { name: 'Level', title: LIST_HEADS.level, className: 'col-level' },
  { name: 'Cell level', title: LIST_HEADS.cellLevel, className: 'col-level' },
  { name: 'Cost', by: 'cost', className: 'col-taken' },
  { name: 'Time', by: 'time', className: 'col-taken' },
  { name: 'Last', by: 'last', className: 'col-last' },
];

/// A column's head; one that orders the list is a button, which orders it by the column, the largest first, or, where
/// it orders the list so already, the other way, and carries the arrow of the order while it orders the list.
function Head({ column, order, onOrder }: { column: Column; order: Order; onOrder: (order: Order) => void }) {
  const { name, by, title, className } = column;
  if (by === undefined) {
    return (
      <th scope="col" className={`label ${className}`} title={title}>
        {name}
      </th>
    );
  }
  const orders = order.by === by;
  const next: Order = { by, way: orders && order.way === 'down' ? 'up' : 'down' };
  return (
    <th
      scope="col"
      className={`label ${className}`}
      aria-sort={orders ? (order.way === 'down' ? 'descending' : 'ascending') : undefined}
    >
      <button type="button" className="order-head" title={LIST_HEADS[by][next.way]} onClick={() => onOrder(next)}>
        {name}
        {orders && <Arrow to={order.way} />}
      </button>
    </th>
  );
}

/// A level as the engine gives it, or a muted dash where it gives none.
function Level({ level }: { level: string | null | undefined }) {
  return level === null || level === undefined ? <span className="muted">{NO_LEVEL}</span> : <>{level}</>;
}

export function WorksTable({ items, projects = [], failed = null }: {
  items: Items;
  /// The projects the installation names.
  projects?: string[];
  failed?: string | null;
}) {
  const [picked, pick] = useState<string | null>(null);
  const [order, setOrder] = useState<Order>(LAST_FIRST);
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
          <table className="works">
            <thead>
              <tr>
                {COLUMNS.map((column) => (
                  <Head key={column.name} column={column} order={order} onOrder={setOrder} />
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
        <Level level={work.level} />
      </td>
      <td className="mono col-level">
        <Level level={work.cellLevel} />
      </td>
      <td className="small muted col-taken">{dollars(work.spent.usdMicros)}</td>
      <td className="small muted col-taken">{duration(work.spent.workingMs)}</td>
      <td className="small muted col-last" title={work.last === null ? undefined : fullTime(work.last)}>
        {work.last === null ? 'nothing yet' : clock(work.last)}
      </td>
    </tr>
  );
});
