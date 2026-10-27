// Every work: those running or waiting first, then those that have ended.

import type { WorkSummary } from '../api';
import { Loaded, WorkBadge } from '../components';
import { cost, duration, time } from '../format';
import { useResource } from '../live';
import { Link, navigate } from '../route';
import { useT } from '../text';

const ENDED = new Set(['done', 'ended']);

function Table({ works }: { works: WorkSummary[] }) {
  const t = useT();
  const now = Date.now();
  return (
    <table className="list">
      <thead>
        <tr>
          <th>{t('col_work')}</th>
          <th>{t('col_item')}</th>
          <th>{t('col_state')}</th>
          <th>{t('col_at')}</th>
          <th>{t('col_stage')}</th>
          <th className="num">{t('col_rounds')}</th>
          <th>{t('col_started')}</th>
          <th>{t('col_last')}</th>
          <th className="num">{t('col_cost')}</th>
          <th>{t('col_pr')}</th>
        </tr>
      </thead>
      <tbody>
        {works.map((w) => (
          <tr key={w.id} className="clickable" onClick={() => navigate(`/works/${w.id}`)}>
            <td>
              <Link to={`/works/${w.id}`}>#{w.id}</Link>
            </td>
            <td>
              {w.item !== null ? (
                <span>
                  <span className="muted">#{w.item} </span>
                  {w.title}
                  {w.project ? <span className="muted"> · {w.project}</span> : null}
                </span>
              ) : (
                w.graph
              )}
            </td>
            <td>
              <WorkBadge state={w.state} />
              {w.state === 'running' && w.running ? (
                <span className="muted"> {t('running_for', { time: duration(now - w.running.started_at) })}</span>
              ) : null}
              {w.paused ? <div className="why">{t('paused_why', { why: w.paused })}</div> : null}
              {w.ended ? <div className="why">{t('ended_why', { why: w.ended })}</div> : null}
            </td>
            <td>
              <span className="cell-id">{w.at}</span> {w.at_title}
            </td>
            <td>{w.stage}</td>
            <td className="num">
              {w.rounds}
              {w.sent_back > 0 ? <span className="warn"> ↩{w.sent_back}</span> : null}
              {w.failed > 0 ? <span className="bad"> ✕{w.failed}</span> : null}
            </td>
            <td>{time(w.started_at)}</td>
            <td>{time(w.last_at)}</td>
            <td className="num">{cost(w.cost_usd)}</td>
            <td>{w.pr !== null ? `#${w.pr}` : ''}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

export function Works() {
  const t = useT();
  const works = useResource<WorkSummary[]>('works', '/api/v1/works');
  return (
    <div className="page">
      <h1>{t('works_title')}</h1>
      <Loaded r={works}>
        {(all) => {
          if (all.length === 0) return <p className="muted">{t('no_works')}</p>;
          const active = all.filter((w) => !ENDED.has(w.state)).sort((a, b) => b.id - a.id);
          const ended = all
            .filter((w) => ENDED.has(w.state))
            .sort((a, b) => (b.last_at ?? 0) - (a.last_at ?? 0));
          return (
            <>
              <h2>{t('works_active')}</h2>
              {active.length > 0 ? <Table works={active} /> : <p className="muted">{t('none')}</p>}
              <h2>{t('works_ended')}</h2>
              {ended.length > 0 ? <Table works={ended} /> : <p className="muted">{t('none')}</p>}
            </>
          );
        }}
      </Loaded>
    </div>
  );
}
