// Every work, and below the list the one picked: those running or waiting at the top, in an area four rows high
// that scrolls and whose height can be dragged; those that have ended folded under them, opening to a list that
// scrolls the same way. A work picked from either opens its chart and rounds below, and the list stays. Both lists
// have the same columns at the same widths, so that they read as one table.

import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react';

import type { WorkSummary } from '../api';
import { Loaded, WorkBadge } from '../components';
import { cost, duration, time } from '../format';
import { useResource } from '../live';
import { Link, navigate } from '../route';
import { t } from '../text';
import { Work } from './Work';

const ENDED = new Set(['done', 'ended']);

/// The height of a list's area: a header and four rows.
const FOUR_ROWS = 4 * 33 + 30;

/// A list's height as the owner dragged it, kept in this browser where it lets the page keep it.
function savedHeight(key: string): number {
  try {
    const v = Number(window.localStorage.getItem(`wf-agents.works-height.${key}`));
    return Number.isFinite(v) && v >= 60 ? v : FOUR_ROWS;
  } catch {
    return FOUR_ROWS;
  }
}

/// The time now, ticking every second while `ticking`, so that a running work's time goes on.
function useNow(ticking: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!ticking) return;
    const tick = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(tick);
  }, [ticking]);
  return now;
}

function Table({ works, selected }: { works: WorkSummary[]; selected: number | null }) {
  const now = useNow(works.some((w) => w.state === 'running'));
  return (
    <table className="list works-table">
      <colgroup>
        <col className="w-id" />
        <col />
        <col className="w-state" />
        <col className="w-at" />
        <col className="w-stage" />
        <col className="w-rounds" />
        <col className="w-time" />
        <col className="w-time" />
        <col className="w-cost" />
        <col className="w-pr" />
      </colgroup>
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
          <tr
            key={w.id}
            className={`clickable${w.id === selected ? ' selected' : ''}`}
            onClick={() => navigate(`/works/${w.id}`)}
          >
            <td>#{w.id}</td>
            <td>
              {w.item !== null ? (
                <Link to={`/items/${w.item}`} className="row-link" title={t('open_item')}>
                  <span className="muted">#{w.item} </span>
                  {w.title}
                  {w.project ? <span className="muted"> · {w.project}</span> : null}
                </Link>
              ) : (
                w.graph
              )}
            </td>
            <td className="state-cell" title={w.paused ?? w.ended ?? undefined}>
              <WorkBadge state={w.state} />
              {w.state === 'running' && w.running ? (
                <span className="muted">
                  {' '}
                  {t('running_for', { time: duration(Math.max(0, now - w.running.started_at)) })}
                </span>
              ) : null}
              {w.open_asks > 0 ? (
                <span className="chip warn" title={t('open_asks_n', { n: w.open_asks })}>
                  {t('open_asks_chip', { n: w.open_asks })}
                </span>
              ) : null}
              {w.open_design_cards > 0 ? (
                <span className="chip warn" title={t('open_design_cards_n', { n: w.open_design_cards })}>
                  {t('open_design_cards_chip', { n: w.open_design_cards })}
                </span>
              ) : null}
              {w.paused ? <div className="why">{w.paused}</div> : null}
              {w.ended ? <div className="why">{w.ended}</div> : null}
            </td>
            <td>
              <span className="cell-id">{w.at}</span> {w.at_title}
            </td>
            <td>{w.stage !== null ? w.stage.replaceAll('_', ' ') : ''}</td>
            <td className="num">
              {w.rounds}
              {w.sent_back > 0 ? <span className="warn"> ↩{w.sent_back}</span> : null}
              {w.failed > 0 ? <span className="bad"> ✕{w.failed}</span> : null}
            </td>
            <td className="nowrap">{time(w.started_at)}</td>
            <td className="nowrap">{time(w.last_at)}</td>
            <td className="num">{cost(w.cost_usd)}</td>
            <td>{w.pr !== null ? `#${w.pr}` : ''}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/// A list in an area of its own that scrolls, with a grip under it that drags its height.
function Scroll({ name, works, selected }: { name: string; works: WorkSummary[]; selected: number | null }) {
  const [height, setHeight] = useState(() => savedHeight(name));
  const from = useRef<{ y: number; h: number } | null>(null);
  const grip = (e: ReactPointerEvent<HTMLDivElement>) => {
    e.preventDefault();
    from.current = { y: e.clientY, h: height };
    e.currentTarget.setPointerCapture(e.pointerId);
  };
  const move = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (from.current === null) return;
    setHeight(Math.max(60, from.current.h + e.clientY - from.current.y));
  };
  const up = () => {
    if (from.current === null) return;
    from.current = null;
    try {
      window.localStorage.setItem(`wf-agents.works-height.${name}`, String(height));
    } catch {
      // The height is kept only where the browser lets it be.
    }
  };
  if (works.length === 0) return <p className="muted works-none">{t('none')}</p>;
  return (
    <>
      <div className="works-scroll" style={{ height }}>
        <Table works={works} selected={selected} />
      </div>
      <div
        className="works-grip"
        role="separator"
        aria-orientation="horizontal"
        title={t('works_drag')}
        onPointerDown={grip}
        onPointerMove={move}
        onPointerUp={up}
        onPointerCancel={up}
      />
    </>
  );
}

export function Works({ selected, round, cell }: { selected: number | null; round: number | null; cell: string | null }) {
  const works = useResource<WorkSummary[]>('works', '/api/v1/works');
  return (
    <div className="works-screen">
      <div className="works-lists">
        <Loaded r={works}>
          {(all) => {
            const active = all.filter((w) => !ENDED.has(w.state)).sort((a, b) => b.id - a.id);
            const ended = all
              .filter((w) => ENDED.has(w.state))
              .sort((a, b) => (b.last_at ?? 0) - (a.last_at ?? 0));
            return (
              <>
                <div className="works-label">
                  {t('works_active')} ({active.length})
                </div>
                <Scroll name="active" works={active} selected={selected} />
                <details className="works-ended">
                  <summary className="works-label">
                    {t('works_ended')} ({ended.length})
                  </summary>
                  <Scroll name="ended" works={ended} selected={selected} />
                </details>
              </>
            );
          }}
        </Loaded>
      </div>
      {selected !== null ? (
        <Work id={selected} round={round} cell={cell} />
      ) : (
        <p className="muted works-pick">{t('works_pick')}</p>
      )}
    </div>
  );
}
