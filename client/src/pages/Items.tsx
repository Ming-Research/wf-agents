// The backlog: the graph of what depends on what (when anything depends on anything), then every item by project,
// what moves first at the top: those being worked on, then those queued in the order they start, with their place in
// the queue, then the rest.

import type { Items as ItemsData, ItemState, ItemSummary } from '../api';
import { ItemBadge, Loaded } from '../components';
import { ItemGraph } from '../graph/ItemGraph';
import { useResource } from '../live';
import { Link, navigate } from '../route';
import { t } from '../text';

/// The order states are listed in: what moves first, what is settled last.
const ORDER: ItemState[] = ['working', 'queued', 'new', 'done', 'stopped', 'cancelled'];

function Table({ items, place }: { items: ItemSummary[]; place: Map<number, number> }) {
  return (
    <table className="list">
      <thead>
        <tr>
          <th>{t('col_item')}</th>
          <th>{t('col_title')}</th>
          <th>{t('col_state')}</th>
          <th className="num">{t('col_queued')}</th>
          <th>{t('col_work')}</th>
          <th>{t('col_level')}</th>
          <th>{t('col_deliver')}</th>
          <th>{t('col_pr')}</th>
        </tr>
      </thead>
      <tbody>
        {items.map((i) => (
          <tr key={i.id} className="clickable" onClick={() => navigate(`/items/${i.id}`)}>
            <td>
              <Link to={`/items/${i.id}`}>#{i.id}</Link>
            </td>
            <td>{i.title}</td>
            <td>
              <ItemBadge state={i.state} />
              {i.cancelled ? <div className="why">{i.cancelled}</div> : null}
            </td>
            <td className="num">{place.get(i.id) ?? ''}</td>
            <td>
              {i.work !== null ? (
                <span>
                  #{i.work} <span className="cell-id">{i.at}</span>
                </span>
              ) : null}
            </td>
            <td>{i.level}</td>
            <td>{i.deliver}</td>
            <td>{i.pr !== null ? `#${i.pr}` : ''}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function Lists({ data }: { data: ItemsData }) {
  const projects = [...new Set(data.items.map((i) => i.project))].sort();
  // The queue is one for every project: an item's place in it counts from 1.
  const place = new Map(
    data.items
      .filter((i) => i.queued !== null)
      .sort((a, b) => (a.queued ?? 0) - (b.queued ?? 0))
      .map((i, n) => [i.id, n + 1]),
  );
  return (
    <>
      {projects.map((p) => {
        const items = data.items
          .filter((i) => i.project === p)
          .sort(
            (a, b) =>
              ORDER.indexOf(a.state) - ORDER.indexOf(b.state) || (a.queued ?? 0) - (b.queued ?? 0) || a.id - b.id,
          );
        return (
          <section key={p}>
            <div className="works-label">{p}</div>
            <Table items={items} place={place} />
          </section>
        );
      })}
    </>
  );
}

export function Items() {
  const items = useResource<ItemsData>('items', '/api/v1/items');
  return (
    <div className="page page-wide">
      <Loaded r={items}>
        {(data) =>
          data.items.length === 0 ? (
            <p className="muted">{t('no_items')}</p>
          ) : (
            <>
              {data.links.length > 0 ? (
                <>
                  <div className="works-label">
                    {t('items_graph')} <span className="muted">· {t('items_graph_hint')}</span>
                  </div>
                  <div className="dag">
                    <ItemGraph items={data} onSelect={(id) => navigate(`/items/${id}`)} />
                  </div>
                </>
              ) : null}
              <Lists data={data} />
            </>
          )
        }
      </Loaded>
    </div>
  );
}
