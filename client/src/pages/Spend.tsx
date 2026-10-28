// What the agents spent across every work: by line and model, and by day with the advisor's part of each model apart,
// so that the owner sees how much of the top tier the advisors use, and over time.

import type { Spend as SpendRow } from '../api';
import { cost } from '../format';
import { useResource } from '../live';
import { spendTable } from '../spend';
import { t } from '../text';
import { modelName } from './RoundView';

export function Spend() {
  const r = useResource<SpendRow[]>('works', '/api/v1/spend');
  if (r.error !== null) return <p className="bad">{r.error}</p>;
  if (r.data === undefined) return null;
  const s = spendTable(r.data);
  if (s.models.length === 0) return <p className="muted page">{t('spend_none')}</p>;
  const head = (first: string) => (
    <thead>
      <tr>
        <th>{first}</th>
        {s.models.map((m) => (
          <th key={m}>{modelName(m)}</th>
        ))}
        <th>{t('spend_col_total')}</th>
      </tr>
    </thead>
  );
  return (
    <div className="page us">
      <p className="muted">{t('spend_note')}</p>
      <section className="us-sec">
        <h4>{t('spend_by_line')}</h4>
        <table className="us-table">
          {head(t('spend_col_line'))}
          <tbody>
            {s.lines.map((l) => (
              <tr key={l.key}>
                <td>
                  <b>{l.key}</b>
                </td>
                {s.models.map((m) => (
                  <td key={m} className="num">
                    {cost(l.byModel[m])}
                  </td>
                ))}
                <td className="num">
                  <b>{cost(l.total)}</b>
                </td>
              </tr>
            ))}
            <tr>
              <td>{t('spend_col_total')}</td>
              {s.models.map((m) => (
                <td key={m} className="num">
                  <b>{cost(s.byModel[m])}</b>
                </td>
              ))}
              <td className="num">
                <b>{cost(s.total)}</b>
              </td>
            </tr>
          </tbody>
        </table>
      </section>
      <section className="us-sec">
        <h4>{t('spend_by_day')}</h4>
        <table className="us-table">
          {head(t('spend_col_day'))}
          <tbody>
            {s.days.map((d) => (
              <tr key={d.key}>
                <td>{d.key}</td>
                {s.models.map((m) => (
                  <td key={m} className="num">
                    {cost(d.byModel[m])}
                    {d.advisor[m] !== undefined ? (
                      <div className="us-sub">{t('spend_advisor', { cost: cost(d.advisor[m]) })}</div>
                    ) : null}
                  </td>
                ))}
                <td className="num">
                  <b>{cost(d.total)}</b>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>
    </div>
  );
}
