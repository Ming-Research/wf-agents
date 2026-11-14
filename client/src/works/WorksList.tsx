// The list of works: the work of every item, one to a row, what waits for the owner first. A row says what the work
// does now, how each cell of its flow stands, what it has taken and when something last happened; it opens the work.
// Where the installation names more than one project, a choice in the view's line shows the works of one project's
// items, with every work that waits for the owner whatever its project, and says how many it shows; the counts of the
// works and of those waiting for the owner are always of every work. The choice lasts while the list is shown: it is
// kept nowhere. The list is read again on each change of an item or a work, and whenever the stream of changes opens
// again, the projects with it; a read that fails keeps the list as it was and says so. With several hundred works, a change of one draws its row again and
// leaves the others as they are: the items are read once for the list and the bar, each that did not change kept as
// it was (source.ts), and a row is drawn again only when its item is another.

import { memo, useCallback, useMemo, useState } from 'react';

import type { Item, Items } from '../works-api';
import { CopiesLine } from './Copies';
import { NowLine, StateLegend, Strip } from './marks';
import { orderWorks, rank } from './placing';
import { useChanges, useItems, useReading, useSource } from './source';
import { clock, fullTime, spentWords } from './words';

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

export function WorksTable({ items, projects = [], failed = null }: {
  items: Items;
  /// The projects the installation names.
  projects?: string[];
  failed?: string | null;
}) {
  const [picked, pick] = useState<string | null>(null);
  // The project chosen, while the installation names it among others; none shows every work.
  const chosen = projects.length > 1 && picked !== null && projects.includes(picked) ? picked : null;
  const all = useMemo(() => orderWorks(items.items), [items]);
  const forOwner = all.filter((w) => rank(w.work) === 0).length;
  // A work that waits for the owner is shown whatever project is chosen.
  const works = useMemo(
    () => all.filter((w) => chosen === null || w.item.project === chosen || rank(w.work) === 0),
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
          {all.length} works, {forOwner} waiting for the owner
          {chosen !== null && `, ${works.length} shown`}
        </span>
        <span className="view-end">
          <CopiesLine />
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
                <th scope="col" className="label">
                  Item
                </th>
                <th scope="col" className="label">
                  Title
                </th>
                <th scope="col" className="label">
                  Now
                </th>
                <th scope="col" className="label">
                  Cells
                </th>
                <th scope="col" className="label">
                  Taken
                </th>
                <th scope="col" className="label">
                  Last
                </th>
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
      className={`work-row ${rank(work) === 0 ? 'for-owner' : ''}`}
      data-work={work.number}
      onClick={() => open(work.number)}
    >
      <td className="mono muted">#{item.number}</td>
      <td className="work-title">
        <a href={`#/works/${work.number}`} onClick={(e) => e.stopPropagation()}>
          {item.title}
        </a>
      </td>
      <td className="work-now">
        <NowLine now={work.now} paused={work.paused} />
      </td>
      <td>
        <Strip cells={work.cells} />
      </td>
      <td className="small muted nowrap">{spentWords(work.spent)}</td>
      <td className="small muted nowrap" title={work.last === null ? undefined : fullTime(work.last)}>
        {work.last === null ? 'nothing yet' : clock(work.last)}
      </td>
    </tr>
  );
});
