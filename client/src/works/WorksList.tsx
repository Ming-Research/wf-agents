// The list of works: every work of every item, one to a row, what waits for the owner first. A row says what the work
// does now, how each cell of its flow stands, what it has taken and when something last happened; it opens the work.

import { useCallback, useEffect, useState } from 'react';

import type { Items } from '../works-api';
import { orderWorks, rank } from './placing';
import { NowLine, StateLegend, Strip } from './marks';
import { useChanges, useSource } from './source';
import { clock, fullTime, spentWords } from './words';

export function WorksList() {
  const source = useSource();
  const [items, setItems] = useState<Items | { failed: string } | null>(null);
  const read = useCallback(() => {
    source.loadItems().then(setItems, (e: unknown) => setItems({ failed: e instanceof Error ? e.message : String(e) }));
  }, [source]);
  useEffect(read, [read]);
  useChanges(read);

  if (!items) return <p className="message muted">Reading the works</p>;
  if ('failed' in items) return <p className="message">The page could not read the works: {items.failed}</p>;
  return <WorksTable items={items} />;
}

export function WorksTable({ items }: { items: Items }) {
  const works = orderWorks(items.items);
  const forOwner = works.filter((w) => rank(w.work) === 0).length;
  const open = (work: number) => {
    window.location.hash = `#/works/${work}`;
  };
  return (
    <>
      <div className="view-line">
        <span className="view-name">Works</span>
        <span className="view-facts">
          {works.length} works of {items.items.length} items, {forOwner} waiting for the owner
        </span>
        <span className="view-end">
          <StateLegend />
        </span>
      </div>
      <main className="list-view">
        {works.length === 0 ? (
          <p className="message muted">No item has a work yet</p>
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
              {works.map(({ item, work }) => (
                <tr
                  key={work.number}
                  className={`work-row ${rank(work) === 0 ? 'for-owner' : ''}`}
                  data-work={work.number}
                  onClick={() => open(work.number)}
                >
                  <td className="mono muted">#{item.number}</td>
                  <td className="work-title">
                    <a href={`#/works/${work.number}`} onClick={(e) => e.stopPropagation()}>
                      {item.title}
                    </a>
                    {item.works.length > 1 && <span className="muted small"> work {work.number}</span>}
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
              ))}
            </tbody>
          </table>
        )}
      </main>
    </>
  );
}
