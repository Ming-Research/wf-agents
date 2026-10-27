// Every item, by project and state, and the graph of their dependencies.

import type { Items as ItemsData, ItemState, ItemSummary } from '../api';
import { ItemBadge, Loaded } from '../components';
import { ItemGraph } from '../graph/ItemGraph';
import { useResource } from '../live';
import { Link, navigate } from '../route';
import { t } from '../text';

/// The order states are listed in: what moves first, what is settled last.
const ORDER: ItemState[] = ['working', 'queued', 'new', 'done', 'stopped', 'cancelled'];

function Table({ items }: { items: ItemSummary[] }) {
  return (
    <table className="list">
      <thead>
        <tr>
          <th>{t('col_item')}</th>
          <th>{t('col_title')}</th>
          <th>{t('col_state')}</th>
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
  return (
    <>
      {projects.map((p) => {
        const items = data.items
          .filter((i) => i.project === p)
          .sort((a, b) => ORDER.indexOf(a.state) - ORDER.indexOf(b.state) || a.id - b.id);
        return (
          <section key={p}>
            <h2>{p}</h2>
            <Table items={items} />
          </section>
        );
      })}
    </>
  );
}

export function Items() {
  const items = useResource<ItemsData>('items', '/api/v1/items');
  return (
    <div className="page">
      <h1>{t('items_title')}</h1>
      <Loaded r={items}>
        {(data) =>
          data.items.length === 0 ? (
            <p className="muted">{t('no_items')}</p>
          ) : (
            <>
              <h2>{t('items_graph')}</h2>
              <p className="muted">{t('items_graph_hint')}</p>
              <div className="dag">
                <ItemGraph items={data} onSelect={(id) => navigate(`/items/${id}`)} />
              </div>
              <Lists data={data} />
            </>
          )
        }
      </Loaded>
    </div>
  );
}
