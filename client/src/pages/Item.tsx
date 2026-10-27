// An item in full, what matters most first: what it is, as its creator wrote it; what its works found and decided,
// as it stands now; what done means (its locked acceptance); what it depends on and what depends on it, when there is
// any; its works; what an import recorded; its design cards and asks of the owner.

import type { ItemDetail, JsonValue, LinkedItem, Shown as ShownValue } from '../api';
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

/// Where a value shown on the item came from: the work, the round and the cell, opening the round.
function From({ x }: { x: ShownValue }) {
  return (
    <Link to={`/works/${x.work}?round=${x.round}`} className="shown-from">
      {t('shown_from', { work: x.work, round: x.round, cell: x.cell })}
    </Link>
  );
}

/// A field's path in words: `design_tree` is "Design tree".
const pathWords = (p: string) => {
  const w = p.replaceAll('_', ' ').replaceAll('.', ' · ');
  return w.charAt(0).toUpperCase() + w.slice(1);
};

/// How long a value shown on the item may be and still be open at first; a longer one opens on asking, so that the
/// page first reads as a list of what was found and decided.
const SHORT = 400;

/// What the item's works found and decided, as the flow shows it on the item: where it stands now (the latest
/// progress); each thing found or decided as it stands now, with the round that wrote it and the earlier versions folded
/// under it; and every progress note, newest first. The rounds come in order, so the last of each is the newest.
function Shown({ d }: { d: ItemDetail }) {
  const values = d.shown.filter((x) => !empty(x.value));
  if (values.length === 0) return <p className="muted">{t('none')}</p>;
  const progress = values.filter((x) => x.path === 'progress');
  const paths = [...new Set(values.filter((x) => x.path !== 'progress').map((x) => x.path))];
  const latest = progress.length > 0 ? progress[progress.length - 1] : undefined;
  return (
    <>
      {latest !== undefined ? (
        <div className="shown-now">
          <strong>{t('shown_now')}</strong> <From x={latest} />
          <p className="prose">{String(latest.value)}</p>
        </div>
      ) : null}
      {paths.map((p) => {
        const all = values.filter((x) => x.path === p);
        const now = all[all.length - 1];
        const before = all.slice(0, -1).reverse();
        return (
          <details key={p} className="shown-path" open={JSON.stringify(now.value).length <= SHORT}>
            <summary>
              <strong>{pathWords(p)}</strong> <From x={now} />
            </summary>
            <Json value={now.value} open={3} />
            {before.length > 0 ? (
              <details className="shown-before">
                <summary className="muted">{t('shown_before', { n: before.length })}</summary>
                {before.map((x) => (
                  <div key={x.round} className="shown-old">
                    <From x={x} />
                    <Json value={x.value} />
                  </div>
                ))}
              </details>
            ) : null}
          </details>
        );
      })}
      {progress.length > 1 ? (
        <details className="shown-path">
          <summary>
            <strong>{t('shown_progress', { n: progress.length })}</strong>
          </summary>
          <ol className="shown-log">
            {[...progress].reverse().map((x) => (
              <li key={x.round}>
                <From x={x} /> <span className="prose">{String(x.value)}</span>
              </li>
            ))}
          </ol>
        </details>
      ) : null}
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
          {i.pr !== null ? <span>PR #{i.pr}</span> : null}
        </div>
        {i.cancelled ? <div className="why">{i.cancelled}</div> : null}
      </header>
      <Section title={t('problem')}>
        <p className="prose">{d.problem}</p>
      </Section>
      <Section title={t('why')}>
        <p className="prose">{d.why}</p>
      </Section>
      {d.approach !== null ? (
        <Section title={t('approach')}>
          <p className="prose">{d.approach}</p>
        </Section>
      ) : null}
      <Section title={t('item_shown')}>
        <Shown d={d} />
      </Section>
      <Section title={t('acceptance')}>
        {d.acceptance !== null ? <Acceptance value={d.acceptance} /> : <p className="muted">{t('not_locked')}</p>}
      </Section>
      {d.depends_on.length > 0 || d.depended_on_by.length > 0 ? (
        <div className="two">
          {d.depends_on.length > 0 ? (
            <Section title={t('depends_on')}>
              <Linked list={d.depends_on} />
            </Section>
          ) : null}
          {d.depended_on_by.length > 0 ? (
            <Section title={t('depended_on_by')}>
              <Linked list={d.depended_on_by} />
            </Section>
          ) : null}
        </div>
      ) : null}
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
