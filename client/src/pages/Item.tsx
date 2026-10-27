// An item in full: as its creator wrote it, its locked acceptance, its dependencies both ways, its works, design cards
// and asks of the owner.

import type { ItemDetail, JsonValue, LinkedItem } from '../api';
import { Facts, ItemBadge, Json, Loaded, Section, WorkBadge } from '../components';
import { cost, time } from '../format';
import { useResource } from '../live';
import { Link } from '../route';
import { t } from '../text';

type Row = { [key in string]: JsonValue };

const isRow = (v: JsonValue): v is Row => typeof v === 'object' && v !== null && !Array.isArray(v);

/// The acceptance as a table of its rows (id, how it is checked, the condition), or as it is kept when it is not.
function Acceptance({ value }: { value: JsonValue }) {
  if (!Array.isArray(value) || !value.every((r) => isRow(r) && typeof r.condition === 'string')) {
    return <Json value={value} />;
  }
  return (
    <table className="list defs">
      <tbody>
        {value.filter(isRow).map((r, i) => (
          <tr key={i}>
            <td>
              <code>{String(r.id ?? i + 1)}</code>
            </td>
            <td className="muted">{String(r.by ?? '')}</td>
            <td>{String(r.condition)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function Linked({ list }: { list: LinkedItem[] }) {
  if (list.length === 0) return <p className="muted">{t('none')}</p>;
  return (
    <ul className="linked">
      {list.map((l) => (
        <li key={l.item.id}>
          <Link to={`/items/${l.item.id}`}>
            #{l.item.id} {l.item.title}
          </Link>{' '}
          <ItemBadge state={l.item.state} />
          <div className="muted">{l.needs}</div>
        </li>
      ))}
    </ul>
  );
}

const empty = (v: JsonValue) =>
  v === null || (typeof v === 'string' && v.trim() === '') || (Array.isArray(v) && v.length === 0);

/// What the rounds of the item's works handed back that the flow shows on the item, work by work, round by round.
function Shown({ d }: { d: ItemDetail }) {
  const values = d.shown.filter((x) => !empty(x.value));
  if (values.length === 0) return <p className="muted">{t('none')}</p>;
  const works = [...new Set(values.map((x) => x.work))];
  return (
    <>
      {works.map((w) => (
        <div key={w} className="shown-work">
          <h3>
            <Link to={`/works/${w}`}>{t('work_title', { id: w })}</Link>
          </h3>
          {values
            .filter((x) => x.work === w)
            .map((x, i) => (
              <div key={i} className="shown-value">
                <div className="muted">
                  <Link to={`/works/${w}?round=${x.round}`}>{t('round_title', { id: x.round })}</Link> ·{' '}
                  {t('shown_at', { cell: x.cell })} · <code>{x.path}</code>
                </div>
                <Json value={x.value} />
              </div>
            ))}
        </div>
      ))}
    </>
  );
}

function Detail({ d }: { d: ItemDetail }) {
  const i = d.item;
  return (
    <>
      <header className="work-head">
        <h1>
          {t('item_title', { id: i.id })} <span className="head-item">{i.title}</span>
          <ItemBadge state={i.state} />
        </h1>
        <div className="head-facts">
          <span>{i.project}</span>
          <span>
            {t('col_level')} {i.level}
          </span>
          <span>
            {t('col_deliver')} {i.deliver}
          </span>
          {i.queued !== null ? (
            <span>
              {t('col_queued')} {i.queued}
            </span>
          ) : null}
          {i.pr !== null ? <span>PR #{i.pr}</span> : null}
        </div>
        {i.cancelled ? <div className="why">{i.cancelled}</div> : null}
      </header>
      <Section title={t('problem')}>
        <pre className="j-text">{d.problem}</pre>
      </Section>
      <Section title={t('why')}>
        <pre className="j-text">{d.why}</pre>
      </Section>
      {d.approach !== null ? (
        <Section title={t('approach')}>
          <pre className="j-text">{d.approach}</pre>
        </Section>
      ) : null}
      <Section title={t('acceptance')}>
        {d.acceptance !== null ? <Acceptance value={d.acceptance} /> : <p className="muted">{t('not_locked')}</p>}
      </Section>
      <div className="two">
        <Section title={t('depends_on')}>
          <Linked list={d.depends_on} />
        </Section>
        <Section title={t('depended_on_by')}>
          <Linked list={d.depended_on_by} />
        </Section>
      </div>
      <Section title={t('works')}>
        {d.works.length === 0 ? (
          <p className="muted">{t('none')}</p>
        ) : (
          <table className="list">
            <tbody>
              {d.works.map((w) => (
                <tr key={w.id}>
                  <td>
                    <Link to={`/works/${w.id}`}>#{w.id}</Link>
                  </td>
                  <td>
                    <WorkBadge state={w.state} />
                  </td>
                  <td>
                    <span className="cell-id">{w.at}</span> {w.at_title}
                  </td>
                  <td>{time(w.started_at)}</td>
                  <td className="num">{w.rounds}</td>
                  <td className="num">{cost(w.cost_usd)}</td>
                  <td>{w.branch ?? ''}</td>
                  <td>{w.pr !== null ? `PR #${w.pr}` : ''}</td>
                  <td className="muted">{w.ended ?? ''}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Section>
      {d.imported_from !== null || d.imported_branch !== null ? (
        <Section title={t('item_imported')}>
          <p className="muted">{t('item_imported_note')}</p>
          <Facts
            rows={[
              [t('item_imported_from'), d.imported_from ?? ''],
              [t('branch'), d.imported_branch ?? ''],
              ['PR', d.imported_pr !== null ? `#${d.imported_pr}` : ''],
            ]}
          />
        </Section>
      ) : null}
      <Section title={t('item_shown')}>
        <p className="muted">{t('item_shown_note')}</p>
        <Shown d={d} />
      </Section>
      {d.design_cards.length > 0 ? (
        <Section title={t('design_cards')}>
          {d.design_cards.map((c) => (
            <div key={c.id} className="boxed">
              <Facts
                rows={[
                  ['#', c.id],
                  [t('col_state'), c.status],
                  [
                    t('round_title', { id: c.round }),
                    <Link key="r" to={`/works/${c.work}?round=${c.round}`}>
                      {t('work_title', { id: c.work })}
                    </Link>,
                  ],
                ]}
              />
              <Json value={c.body} />
              <Json value={c.rulings} />
            </div>
          ))}
        </Section>
      ) : null}
      {d.asks.length > 0 ? (
        <Section title={t('asks')}>
          {d.asks.map((a) => (
            <div key={a.id} className="boxed">
              <Facts
                rows={[
                  ['#', a.id],
                  [t('col_state'), a.answer === null ? t('ask_waits') : t('ask_answered')],
                  [
                    t('round_title', { id: a.round }),
                    <Link key="r" to={`/works/${a.work}?round=${a.round}`}>
                      {t('work_title', { id: a.work })}
                    </Link>,
                  ],
                ]}
              />
              <Json value={a.body} />
              {a.answer !== null ? <Json value={a.answer} /> : null}
            </div>
          ))}
        </Section>
      ) : null}
    </>
  );
}

export function Item({ id }: { id: number }) {
  const item = useResource<ItemDetail>(`item:${id}`, `/api/v1/items/${id}`);
  return (
    <div className="page">
      <Loaded r={item}>{(d) => <Detail d={d} />}</Loaded>
    </div>
  );
}
