// Every work, and below the list the one picked: those running or waiting at the top, in an area four rows high
// that scrolls and whose height can be dragged; those that have ended folded under them, opening to a list that
// scrolls the same way. A work picked from either opens its chart and rounds below, and the list stays. Both lists
// have the same columns at the same widths, so that they read as one table. A row says how its work goes on (a round
// running, what holds it back, what it waits for) and holds the owner's Pause, Continue and Step of it. What waits
// for the owner opens that tab of the work, and the marks of rounds that went wrong open the work with only those
// rounds.

import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent, type ReactNode } from 'react';

import type { WorkSummary } from '../api';
import { type Course, WorkBadge, Loaded } from '../components';
import { Controls, held, runningWords, takeoverWords, useActions, waitsWords } from '../course';
import { cost, duration, time } from '../format';
import { useResource, useStatus } from '../live';
import { Link, navigate, type WorkRoute, type WorkTab, workHref } from '../route';
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

/// How a work goes on, on one line: a round of it running and how long it has run, or what holds it back, then what it
/// waits for, linked to where the owner gives it, and its design cards and asks waiting for the owner as small chips;
/// under it, why the engine stopped it or why it ended, cut to the line.
function State({ w, c, now }: { w: WorkSummary; c: Course; now: number }) {
  if (c.over) {
    return (
      <>
        <WorkBadge state={w.state} />
        {w.ended ? <div className="why">{w.ended}</div> : null}
      </>
    );
  }
  const why = held(c, w);
  const parts: ReactNode[] = [];
  if (c.running !== null) {
    parts.push(
      <span key="running" className="badge state-running">
        {runningWords(c)}
      </span>,
    );
    if (w.running) {
      parts.push(
        <span key="for" className="muted">
          {' '}
          {duration(Math.max(0, now - w.running.started_at))}
        </span>,
      );
    }
  } else if (why !== null) {
    parts.push(
      <span key="held" className={`badge ${c.held === 'engine' ? 'state-stopped' : 'state-paused'}`} title={why}>
        {c.held === 'engine' ? t('course_stopped') : t('course_paused')}
      </span>,
    );
  }
  const waits = c.waits;
  if (waits !== null) {
    const to =
      waits.kind === 'answer'
        ? workHref(w.id, { tab: 'messages' })
        : waits.kind === 'ruling'
          ? workHref(w.id, { tab: 'cards' })
          : waits.kind === 'items' && waits.items.length > 0
            ? `/items/${waits.items[0]}`
            : workHref(w.id);
    parts.push(
      <Link key="waits" to={to} className="chip waits" title={waitsWords(waits)}>
        {waitsWords(waits)}
      </Link>,
    );
  }
  if (c.running === null && why === null && waits === null) {
    parts.push(<WorkBadge key="ready" state="ready" />);
  }
  if (c.running === null && waits === null && w.answered !== null) {
    parts.push(
      <span key="answered" className="chip" title={t('course_answered_hint')}>
        {t('course_answered', { id: w.answered })}
      </span>,
    );
  }
  // The asks and design cards waiting for the owner that what the work waits for does not name already: they are the
  // owner's to do, not what holds the work up.
  const asks = w.open_asks - (waits?.kind === 'answer' ? 1 : 0);
  const cards = w.open_design_cards - (waits?.kind === 'ruling' ? 1 : 0);
  if (asks > 0) {
    parts.push(
      <Link key="asks" to={workHref(w.id, { tab: 'messages' })} className="chip warn" title={t('open_asks_n', { n: asks })}>
        {t('open_asks_chip', { n: asks })}
      </Link>,
    );
  }
  if (cards > 0) {
    parts.push(
      <Link
        key="cards"
        to={workHref(w.id, { tab: 'cards' })}
        className="chip warn"
        title={t('open_design_cards_later_n', { n: cards })}
      >
        {t('open_design_cards_chip', { n: cards })}
      </Link>,
    );
  }
  return (
    <>
      {parts}
      {c.held === 'engine' && w.paused ? <div className="why">{w.paused}</div> : null}
    </>
  );
}

/// A work's row; its item opens from it, and its controls act without opening it.
function Row({ w, selected, tab, now }: { w: WorkSummary; selected: boolean; tab: WorkTab; now: number }) {
  const status = useStatus();
  const a = useActions(w, status, true);
  return (
    <tr className={`clickable${selected ? ' selected' : ''}`} onClick={() => navigate(workHref(w.id, { tab }))}>
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
      <td className="state-cell" title={held(a.c, w) ?? w.ended ?? undefined}>
        <State w={w} c={a.c} now={now} />
      </td>
      <td className="control-cell" onClick={(e) => e.stopPropagation()}>
        <Controls a={a} w={w} status={status} onStep={() => a.step()} />
        {a.error !== null ? (
          <div className="bad why" title={a.error}>
            {t('request_failed', { error: a.error })}
          </div>
        ) : null}
      </td>
      <td>
        <span className="cell-id">{w.at}</span> {w.at_title}
        {a.c.outdated ? (
          <span className="chip warn" title={takeoverWords(w, status)}>
            {t('course_outdated', { version: w.flow_version })}
          </span>
        ) : null}
      </td>
      <td>{w.stage !== null ? w.stage.replaceAll('_', ' ') : ''}</td>
      <td className="num">
        {w.rounds}
        {w.sent_back > 0 ? (
          <>
            {' '}
            <Link
              to={workHref(w.id, { only: 'sent_back' })}
              className="warn rounds-mark"
              title={t('rounds_sent_back_title', { n: w.sent_back })}
            >
              ↩{w.sent_back}
            </Link>
          </>
        ) : null}
        {w.failed > 0 ? (
          <>
            {' '}
            <Link
              to={workHref(w.id, { only: 'failed' })}
              className="bad rounds-mark"
              title={t('rounds_failed_title', { n: w.failed })}
            >
              ✕{w.failed}
            </Link>
          </>
        ) : null}
      </td>
      <td className="nowrap">{time(w.started_at)}</td>
      <td className="nowrap">{time(w.last_at)}</td>
      <td className="num">{cost(w.cost_usd)}</td>
      <td>{w.pr !== null ? `#${w.pr}` : ''}</td>
    </tr>
  );
}

/// The works as rows. A row opens its work with the tab the open work shows, so that going from work to work (to rule on
/// their design cards, say) keeps to what the owner is doing; what waits for the owner and the rounds that went wrong
/// open that tab, or only those rounds.
function Table({ works, selected, tab }: { works: WorkSummary[]; selected: number | null; tab: WorkTab }) {
  const status = useStatus();
  const now = useNow(works.some((w) => status !== null && status.running === w.id));
  return (
    <table className="list works-table">
      <colgroup>
        <col className="w-id" />
        <col />
        <col className="w-state" />
        <col className="w-control" />
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
          <th aria-label={t('col_control')} />
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
          <Row key={w.id} w={w} selected={w.id === selected} tab={tab} now={now} />
        ))}
      </tbody>
    </table>
  );
}

/// A list in an area of its own that scrolls, with a grip under it that drags its height.
function Scroll({ name, works, selected, tab }: {
  name: string;
  works: WorkSummary[];
  selected: number | null;
  tab: WorkTab;
}) {
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
        <Table works={works} selected={selected} tab={tab} />
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

export function Works({ work }: { work: WorkRoute | null }) {
  const works = useResource<WorkSummary[]>('works', '/api/v1/works');
  const selected = work?.id ?? null;
  const tab = work?.tab ?? 'rounds';
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
                <Scroll name="active" works={active} selected={selected} tab={tab} />
                <details className="works-ended">
                  <summary className="works-label">
                    {t('works_ended')} ({ended.length})
                  </summary>
                  <Scroll name="ended" works={ended} selected={selected} tab={tab} />
                </details>
              </>
            );
          }}
        </Loaded>
      </div>
      {work !== null ? (
        <Work r={work} />
      ) : (
        <p className="muted works-pick">{t('works_pick')}</p>
      )}
    </div>
  );
}
