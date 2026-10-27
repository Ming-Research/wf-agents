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
/// then why the engine stopped it or why it ended.
function State({ w, c, now }: { w: WorkSummary; c: Course; now: number }) {
  if (c.over) {
    return (
      <>
        <WorkBadge state={w.state} />
        {w.ended ? <span className="why">{w.ended}</span> : null}
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
      {c.held === 'engine' && w.paused ? <span className="why">{w.paused}</span> : null}
    </>
  );
}

/// A work's row, every cell on one line, cut where its column ends and whole on hovering; its item opens from it, and
/// its controls act without opening it.
function Row({ w, selected, tab, now }: { w: WorkSummary; selected: boolean; tab: WorkTab; now: number }) {
  const status = useStatus();
  const a = useActions(w, status, true);
  const item = w.item !== null ? `#${w.item} ${w.title ?? ''}${w.project ? ` · ${w.project}` : ''}` : w.graph;
  const at = `${w.at} ${w.at_title ?? ''}`;
  const stage = w.stage !== null ? w.stage.replaceAll('_', ' ') : '';
  return (
    <tr className={`clickable${selected ? ' selected' : ''}`} onClick={() => navigate(workHref(w.id, { tab }))}>
      <td title={`#${w.id}`}>#{w.id}</td>
      <td title={item}>
        {w.item !== null ? (
          <Link to={`/items/${w.item}`} className="row-link" title={`${item}\n${t('open_item')}`}>
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
          <span className="bad why" title={a.error}>
            {t('request_failed', { error: a.error })}
          </span>
        ) : null}
      </td>
      <td title={a.c.outdated ? `${at}\n${takeoverWords(w, status)}` : at}>
        <span className="cell-id">{w.at}</span>
        {a.c.outdated ? (
          <span className="chip warn" title={takeoverWords(w, status)}>
            {t('course_outdated', { version: w.flow_version })}
          </span>
        ) : null}{' '}
        {w.at_title}
      </td>
      <td title={stage}>{stage}</td>
      <td className="num" title={String(w.rounds)}>
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
      <td title={time(w.started_at)}>{time(w.started_at)}</td>
      <td title={time(w.last_at)}>{time(w.last_at)}</td>
      <td className="num" title={cost(w.cost_usd)}>
        {cost(w.cost_usd)}
      </td>
      <td title={w.pr !== null ? `#${w.pr}` : undefined}>{w.pr !== null ? `#${w.pr}` : ''}</td>
    </tr>
  );
}

/// The columns of both lists, in order, with their headers and their own widths; the item takes what the others leave.
const COLUMNS = [
  { key: 'id', head: 'col_work', width: 56, num: false },
  { key: 'item', head: 'col_item', width: null, num: false },
  { key: 'state', head: 'col_state', width: 360, num: false },
  { key: 'control', head: null, width: 130, num: false },
  { key: 'at', head: 'col_at', width: 190, num: false },
  { key: 'stage', head: 'col_stage', width: 120, num: false },
  { key: 'rounds', head: 'col_rounds', width: 90, num: true },
  { key: 'started', head: 'col_started', width: 130, num: false },
  { key: 'last', head: 'col_last', width: 130, num: false },
  { key: 'cost', head: 'col_cost', width: 80, num: true },
  { key: 'pr', head: 'col_pr', width: 50, num: false },
] as const;

type Column = (typeof COLUMNS)[number]['key'];

/// The widths the owner dragged columns to, by column; a column not in it has its own width.
type Widths = Partial<Record<Column, number>>;

const WIDTHS_KEY = 'wf-agents.works-columns';

/// The narrowest a column drags to, and the narrowest the item is left before the lists scroll sideways.
const COLUMN_MIN = 36;
const ITEM_MIN = 240;

/// The widths kept in this browser, where it lets the page keep them.
function savedWidths(): Widths {
  try {
    const kept: unknown = JSON.parse(window.localStorage.getItem(WIDTHS_KEY) ?? '{}');
    const out: Widths = {};
    if (typeof kept !== 'object' || kept === null) return out;
    for (const c of COLUMNS) {
      const v = (kept as Record<string, unknown>)[c.key];
      if (typeof v === 'number' && Number.isFinite(v) && v >= COLUMN_MIN) out[c.key] = v;
    }
    return out;
  } catch {
    return {};
  }
}

function keepWidths(w: Widths) {
  try {
    window.localStorage.setItem(WIDTHS_KEY, JSON.stringify(w));
  } catch {
    // The widths are kept only where the browser lets them be.
  }
}

function widthOf(widths: Widths, key: Column): number {
  return widths[key] ?? COLUMNS.find((c) => c.key === key)?.width ?? COLUMN_MIN;
}

/// The right edge of a column's header, which drags the column's width; the edge after the item, which takes what is
/// left, drags the state's from its left. A double click gives the column its own width again.
function ColumnGrip({ column, widths, onWidths }: {
  column: Column;
  widths: Widths;
  onWidths: (w: Widths) => void;
}) {
  const target: Column = column === 'item' ? 'state' : column;
  const sign = column === 'item' ? -1 : 1;
  const press = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    const el = e.currentTarget;
    el.setPointerCapture(e.pointerId);
    const x0 = e.clientX;
    const w0 = widthOf(widths, target);
    let last = widths;
    const move = (ev: PointerEvent) => {
      const next: Widths = { ...widths };
      next[target] = Math.max(COLUMN_MIN, Math.round(w0 + sign * (ev.clientX - x0)));
      last = next;
      onWidths(next);
    };
    const up = () => {
      el.removeEventListener('pointermove', move);
      el.removeEventListener('pointerup', up);
      el.removeEventListener('pointercancel', up);
      keepWidths(last);
    };
    el.addEventListener('pointermove', move);
    el.addEventListener('pointerup', up);
    el.addEventListener('pointercancel', up);
  };
  const reset = () => {
    const next = { ...widths };
    delete next[target];
    onWidths(next);
    keepWidths(next);
  };
  return (
    <div
      className="col-grip"
      role="separator"
      aria-orientation="vertical"
      title={t('works_column_grip')}
      onPointerDown={press}
      onDoubleClick={reset}
    />
  );
}

/// The works as rows. A row opens its work with the tab the open work shows, so that going from work to work (to rule on
/// their design cards, say) keeps to what the owner is doing; what waits for the owner and the rounds that went wrong
/// open that tab, or only those rounds.
function Table({ works, selected, tab, widths, onWidths }: {
  works: WorkSummary[];
  selected: number | null;
  tab: WorkTab;
  widths: Widths;
  onWidths: (w: Widths) => void;
}) {
  const status = useStatus();
  const now = useNow(works.some((w) => status !== null && status.running === w.id));
  const fixed = COLUMNS.filter((c) => c.width !== null).reduce((sum, c) => sum + widthOf(widths, c.key), 0);
  return (
    <table className="list works-table" style={{ minWidth: fixed + ITEM_MIN }}>
      <colgroup>
        {COLUMNS.map((c) => (
          <col key={c.key} style={c.width !== null ? { width: widthOf(widths, c.key) } : undefined} />
        ))}
      </colgroup>
      <thead>
        <tr>
          {COLUMNS.map((c) => (
            <th
              key={c.key}
              className={c.num ? 'num' : undefined}
              aria-label={c.head === null ? t('col_control') : undefined}
              title={c.head !== null ? t(c.head) : undefined}
            >
              {c.head !== null ? t(c.head) : null}
              <ColumnGrip column={c.key} widths={widths} onWidths={onWidths} />
            </th>
          ))}
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
function Scroll({ name, works, selected, tab, widths, onWidths }: {
  name: string;
  works: WorkSummary[];
  selected: number | null;
  tab: WorkTab;
  widths: Widths;
  onWidths: (w: Widths) => void;
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
        <Table works={works} selected={selected} tab={tab} widths={widths} onWidths={onWidths} />
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
  // Both lists have the same columns at the same widths.
  const [widths, setWidths] = useState<Widths>(savedWidths);
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
                <Scroll name="active" works={active} selected={selected} tab={tab} widths={widths} onWidths={setWidths} />
                <details className="works-ended">
                  <summary className="works-label">
                    {t('works_ended')} ({ended.length})
                  </summary>
                  <Scroll name="ended" works={ended} selected={selected} tab={tab} widths={widths} onWidths={setWidths} />
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
