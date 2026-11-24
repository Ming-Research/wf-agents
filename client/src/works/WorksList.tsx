// The list of works: the record of every work, ended or not, one to a row, for the owner to look back at; what needs
// the owner now is the console's. The works stand in a group for each project, in order of the projects' names
// (projects.ts), each work in its own project's group, one that waits for the owner as well. A group's head stands
// outside its table: the project's name, which folds the group away and opens it again, with how many works it holds and
// what they cost (parts.tsx, `ProjectGroup`); which groups are folded is kept in the browser. A row says the item's
// number and title, the work's state and the cell it stands at, how each cell of its flow stands, the level its rounds
// run at and the level of its cell, as the engine gives them, what it cost, how long its rounds took and when something
// last happened; it opens the work. Each group is ordered by when something last happened, the latest first, until the
// owner presses the head of another column in any group's table, each but the strip's having an order, a second press
// turning it; nothing of a work's state orders it but its own column. The order is kept in the browser. The groups'
// tables have one set of columns: the owner drags the width of each by its head's right edge, kept in the browser, and
// one he has not dragged is as wide as the widest of what it holds in any group (`useOneSetOfColumns`). The count of
// the works, and the one count of what waits for the owner, the bar's, are of every work. The list is read again on
// each change of an item or a work, and whenever the stream of changes opens again, the projects with it; a read that
// fails keeps the list as it was and says so. With several hundred works, a change of one draws its row again and
// leaves the others as they are: the items are read once for the list and the bar, each that did not change kept as
// it was (source.ts), and a row is drawn again only when its item is another.

import { type CSSProperties, type RefObject, memo, useLayoutEffect, useMemo, useRef } from 'react';

import { CHOICE_KEYS, useKept, useKeptFoldedGroups } from '../kept';
import { Legend } from '../marks';
import { Arrow, Edge, ProjectGroup } from '../parts';
import { groupByProject } from '../projects';
import { WIDTH_KEYS, useKeptWidths } from '../sizes';
import type { Item, Items } from '../works-api';
import { NowLine, STRIP_MARKS, Strip } from './marks';
import { FIRST_WAY, LAST_FIRST, type ListedWork, type Order, type OrderBy, orderWorks, wayOf } from './placing';
import { useItems, useProjects } from './source';
import { waitingCount, waitsOnOwner } from './waiting';
import {
  NO_LEVEL,
  ORDER_WORDS,
  clock,
  dollars,
  duration,
  fullTime,
  itemName,
  stoppedAt,
  waitCount,
} from './words';

/// An order as it is kept: its column, and `turned` after it where it runs the other way.
type OrderKey = `${OrderBy}` | `${OrderBy} turned`;
const orderKey = (o: Order): OrderKey => (o.turned ? `${o.by} turned` : o.by);
const ORDER_KEYS = (Object.keys(FIRST_WAY) as OrderBy[]).flatMap((by): OrderKey[] => [by, `${by} turned`]);
const orderOfKey = (k: OrderKey): Order => {
  const [by, turned] = k.split(' ');
  return { by: by as OrderBy, turned: turned === 'turned' };
};

export function WorksList() {
  const { value: items, failed } = useItems();
  const projects = useProjects();

  if (!items) {
    return failed ? (
      <p className="message">The page could not read the works: {failed}</p>
    ) : (
      <p className="message muted">Reading the works</p>
    );
  }
  // Where the projects cannot be read, or are not yet, the groups are those of the projects the items name.
  return <WorksTable items={items} projects={projects} failed={failed} />;
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

/// The list's columns, in order. Each is as wide as what it holds in every group until the owner drags it, the title
/// as wide as the others leave; each keeps at least the width of its head.
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

/// The columns fitted to what they hold in every group: all but the title's, which takes what the others leave.
const FITTED = COLUMNS.filter((c) => c.key !== 'title');

/// The name under which a column's fitted width is set on the list, which its cells take where the owner has not
/// dragged it (styles/works.css).
const fitName = (c: Column) => `--fit-${c.className}`;

/// Gives each column one width in every group's table, so that the tables, drawn apart under their projects' heads,
/// read as one list: a column the owner has not dragged is as wide as the widest of what it holds in any group. Worked
/// out after the list is drawn and before it is shown: each table first takes its columns as wide as its own cells,
/// and the widest of each column is then set on the list for every table. Worked out again whenever what is drawn
/// changes (`drawn`), and whenever the list's width changes, which shows or hides columns (styles/works.css), since a
/// hidden column has no width to take.
function useOneSetOfColumns(list: RefObject<HTMLElement | null>, drawn: unknown) {
  useLayoutEffect(() => {
    const el = list.current;
    if (el === null) return undefined;
    const fit = () => {
      for (const c of FITTED) el.style.removeProperty(fitName(c));
      const widest = new Map<string, number>();
      for (const th of el.querySelectorAll<HTMLElement>('table.works > thead th[data-column]')) {
        const key = th.dataset.column ?? '';
        widest.set(key, Math.max(widest.get(key) ?? 0, th.getBoundingClientRect().width));
      }
      for (const c of FITTED) {
        const width = widest.get(c.key) ?? 0;
        if (width > 0) el.style.setProperty(fitName(c), `${Math.ceil(width)}px`);
      }
    };
    fit();
    if (typeof ResizeObserver === 'undefined') return undefined;
    let width = el.clientWidth;
    const watch = new ResizeObserver(() => {
      if (el.clientWidth === width) return;
      width = el.clientWidth;
      fit();
    });
    watch.observe(el);
    return () => watch.disconnect();
  }, [list, drawn]);
}

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
  const { key, name, by, least, className } = column;
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
      <th ref={head} scope="col" className={`label ${className}`} data-column={key}>
        {name}
        {edge}
      </th>
    );
  }
  const orders = order.by === by;
  const next: Order = { by, turned: orders && !order.turned };
  return (
    <th
      ref={head}
      scope="col"
      className={`label ${className}`}
      data-column={key}
      aria-sort={orders ? wayOf(order) : undefined}
    >
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

/// How many works, in words.
const worksCounted = (n: number) => `${n} ${n === 1 ? 'work' : 'works'}`;

/// No project named, the same list at every drawing, so that the groups are not worked out again for nothing.
const NO_PROJECTS: string[] = [];

export function WorksTable({ items, projects = NO_PROJECTS, failed = null }: {
  items: Items;
  /// The projects the installation names, in order of their names.
  projects?: string[];
  failed?: string | null;
}) {
  // The order the owner last picked, kept in this browser as its column and whether it was turned.
  const [keptOrder, keepOrder] = useKept(CHOICE_KEYS.worksOrder, orderKey(LAST_FIRST), ORDER_KEYS);
  const order = orderOfKey(keptOrder);
  const setOrder = (o: Order) => keepOrder(orderKey(o));
  const [widths, setWidth] = useKeptWidths(WIDTH_KEYS.worksColumns, WIDTHS.least, WIDTHS.most);
  const [folded, turn] = useKeptFoldedGroups(CHOICE_KEYS.worksFolded);
  // The widths the owner dragged, each at least its column's least, set on every group's table for every row's cells;
  // while the title's is set, a table is as wide as its columns, and else as the window, the title taking what is left.
  const style: Record<string, string> = Object.fromEntries(
    COLUMNS.flatMap((c) => {
      const w = widths[c.key];
      return w === undefined ? [] : [[`--${c.className}`, `${Math.max(c.least, w)}px`]];
    }),
  );
  if (widths.title !== undefined) style.width = 'auto';
  const all = useMemo(() => orderWorks(items.items, order), [items, order]);
  const groups = useMemo(() => groupByProject(projects, all, (w) => w.item.project), [projects, all]);
  const waiting = waitingCount(all.map((w) => w.work));
  // The columns are fitted again when the groups or what of them is open change, and when the owner drags a column or
  // gives one back its own width; not at each step of a drag, which changes no other column's width.
  const dragged = Object.keys(widths).sort().join(' ');
  const drawn = useMemo(() => [groups, folded, dragged], [groups, folded, dragged]);
  const list = useRef<HTMLElement>(null);
  useOneSetOfColumns(list, drawn);
  const heads = (
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
  );
  return (
    <>
      <div className="view-line">
        <span className="view-name">Works</span>
        <span className="view-facts">
          {all.length} works, {waitCount(waiting)}
        </span>
        <span className="view-end works-legend">
          <Legend marks={STRIP_MARKS} />
        </span>
      </div>
      {failed !== null && (
        <p className="read-failed" role="alert">
          The works could not be read again: {failed}. What is shown is from before.
        </p>
      )}
      <main ref={list} className="list-view">
        {groups.length === 0 ? (
          <p className="message muted">No item has a work yet</p>
        ) : (
          groups.map(({ project, rows }) => (
            <ProjectGroup
              key={JSON.stringify(project)}
              project={project}
              what="works"
              count={rows.length}
              countTitle={worksCounted(rows.length)}
              note={dollars(spentOf(rows))}
              noteTitle="What its works cost"
              open={!folded.has(project)}
              onToggle={() => turn(project)}
            >
              {rows.length === 0 ? (
                <p className="message muted">No item of project {project} has a work yet</p>
              ) : (
                <table className="works" style={style as CSSProperties}>
                  {heads}
                  <tbody>
                    {rows.map(({ item }) => (
                      <WorkRow key={item.work.number} item={item} />
                    ))}
                  </tbody>
                </table>
              )}
            </ProjectGroup>
          ))
        )}
      </main>
    </>
  );
}

/// What the works cost, in millionths of a dollar, as their column of costs counts it.
const spentOf = (works: ListedWork[]) => works.reduce((sum, w) => sum + w.work.spent.usdMicros, 0);

/// The row of an item's work, drawn again only when the item is another object, which it is only where it changed.
export const WorkRow = memo(function WorkRow({ item }: { item: Item }) {
  const work = item.work;
  return (
    <tr
      className={`work-row ${waitsOnOwner(work) ? 'for-owner' : ''}`}
      data-work={work.number}
      onClick={() => open(work.number)}
    >
      <td className="mono muted col-item">{itemName(item.number)}</td>
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
