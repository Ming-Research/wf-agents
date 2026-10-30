// Every work, or the one open. The list shows every row: those running or waiting, then those that have ended,
// folded at first; both have the same columns at the same widths, which the owner drags, so that they read as one
// table, and every cell stays on one line. A row says how its work goes on (a round running, what holds it back, what
// it waits for), its state as it is in effect and why, and leads to its group in the messages view, the one place the
// owner acts on it. A work opened from it takes the whole window under the bar, its chart under a line that says which
// work it is, how it goes on and where, leads to its group, and to the other works running or waiting with what each
// needs of the owner. The marks of rounds that went wrong open the work with only those rounds.

import { ArrowLeftIcon, ReplyIcon, XCircleIcon } from '@primer/octicons-react';
import { CounterLabel, Heading, Label, Link as PrimerLink } from '@primer/react';
import { useState, type PointerEvent as ReactPointerEvent, type ReactNode } from 'react';

import type { Status, WorkSummary } from '../api';
import { type Course, course, heldName, StateChip, TagChip, WorkBadge, Loaded } from '../components';
import { held, runningWords, takeoverWords, waitsWords } from '../course';
import { cost, duration, time } from '../format';
import { useNow, useResource, useStatus } from '../live';
import { HeadingLevel, ItemRef, LabelLink, Section } from '../parts';
import { href, Link, navigate, type WorkRoute, type WorkTab, workHref } from '../route';
import { t, tn } from '../text';
import { joined } from '../voices';
import { Work } from './Work';

const ENDED = new Set(['done', 'ended']);

/// How a work goes on, on one line: a round of it running and how long it has run, or what holds it back, then what it
/// waits for, linked to where the owner gives it, and its design cards and asks waiting for the owner as small chips;
/// then why the engine stopped it or why it ended.
function State({ w, c, now }: { w: WorkSummary; c: Course; now: number }) {
  if (c.over) {
    return (
      <span className="course">
        <WorkBadge state={w.state} />
        {w.ended ? <span className="why">{w.ended}</span> : null}
      </span>
    );
  }
  const why = held(c, w);
  const parts: ReactNode[] = [];
  if (c.running !== null) {
    parts.push(
      <Label key="running" variant="accent">
        {runningWords(c)}
      </Label>,
    );
    if (w.running) {
      parts.push(
        <span key="for" className="muted">
          {duration(Math.max(0, now - w.running.started_at))}
        </span>,
      );
    }
  } else if (why !== null) {
    const h = heldName(c);
    parts.push(
      <Label key="held" variant={h.problem ? 'danger' : 'attention'} title={why}>
        {h.name}
      </Label>,
    );
  }
  const waits = c.waits;
  if (waits !== null) {
    const to =
      waits.kind === 'answer' || waits.kind === 'ruling' || waits.kind === 'architect'
        ? href({ page: 'messages', open: `work:${w.id}` })
        : waits.kind === 'items' && waits.items.length > 0
              ? `/items/${waits.items[0]}`
              : workHref(w.id);
    parts.push(
      <LabelLink key="waits" to={to} variant="attention" title={waitsWords(waits)}>
        {waitsWords(waits)}
      </LabelLink>,
    );
  }
  if (c.running === null && why === null && waits === null) {
    parts.push(
      c.started ? (
        <StateChip key="ready" tone="neutral">
          {t('course_ready_started')}
        </StateChip>
      ) : (
        <WorkBadge key="ready" state="ready" />
      ),
    );
  }
  // The asks and design cards waiting for the owner that what the work waits for does not name already: they are the
  // owner's to do, not what holds the work up.
  const asks = w.open_asks - (waits?.kind === 'answer' ? 1 : 0);
  const cards = w.open_design_cards - (waits?.kind === 'ruling' ? 1 : 0);
  if (asks > 0) {
    parts.push(
      <LabelLink key="asks" to={href({ page: 'messages', open: `work:${w.id}` })} variant="attention" title={t('open_asks_n', { n: asks })}>
        {t('open_asks_chip', { n: asks })}
      </LabelLink>,
    );
  }
  if (cards > 0) {
    parts.push(
      <LabelLink
        key="cards"
        to={href({ page: 'messages', open: `work:${w.id}` })}
        variant="attention"
        title={tn('open_design_cards_later_n', cards)}
      >
        {t('open_design_cards_chip', { n: cards })}
      </LabelLink>,
    );
  }
  return (
    <span className="course">
      {parts}
      {(c.held === 'engine' || c.held === 'architect') && w.paused ? <span className="why">{w.paused}</span> : null}
    </span>
  );
}

/// The project a work's item is of, said after its title only while there is more than one project, as the engine's
/// configuration has them: with one, it says nothing.
function projectOf(w: WorkSummary, status: Status | null): string | null {
  return status !== null && Object.keys(status.repos).length > 1 ? w.project : null;
}

/// A work's row, every cell on one line, cut where its column ends and whole on hovering; its item opens from it, and
/// its group in the messages view, where the owner acts on it, from its link.
function Row({ w, tab, now }: { w: WorkSummary; tab: WorkTab; now: number }) {
  const status = useStatus();
  const a = { c: course(w, status) };
  const project = projectOf(w, status);
  const item = w.item !== null ? `#${w.item} ${w.title ?? ''}${project ? ` · ${project}` : ''}` : w.graph;
  const at = `${w.at} ${w.at_title ?? ''}`;
  const stage = w.stage !== null ? w.stage.replaceAll('_', ' ') : '';
  return (
    <tr className="clickable" onClick={() => navigate(workHref(w.id, { tab }))}>
      <td title={`#${w.id}`}>#{w.id}</td>
      <td title={item}>
        {w.item !== null ? (
          <>
            <ItemRef id={w.item} title={w.title} hint={`${item}\n${t('open_item')}`} />
            {project ? <span className="muted"> · {project}</span> : null}
          </>
        ) : (
          w.graph
        )}
      </td>
      <td className="state-cell" title={held(a.c, w) ?? w.ended ?? undefined}>
        <State w={w} c={a.c} now={now} />
      </td>
      <td className="control-cell" onClick={(e) => e.stopPropagation()}>
        <Link to={href({ page: 'messages', open: `work:${w.id}` })} title={t('go_to_conversation_hint')} muted>
          {t('go_to_conversation')}
        </Link>
      </td>
      <td title={a.c.outdated ? `${at}\n${takeoverWords(w, status)}` : at}>
        <span className="cell-id">{w.at}</span>{' '}
        {a.c.outdated ? (
          <>
            <Label variant="attention" title={takeoverWords(w, status)}>
              {t('course_outdated', { version: w.flow_version })}
            </Label>{' '}
          </>
        ) : null}
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
              className="rounds-mark rounds-mark-back"
              title={t('rounds_sent_back_title', { n: w.sent_back })}
            >
              <ReplyIcon aria-label={t('outcome_sent_back')} />
              {w.sent_back}
            </Link>
          </>
        ) : null}
        {w.failed > 0 ? (
          <>
            {' '}
            <Link
              to={workHref(w.id, { only: 'failed' })}
              className="rounds-mark rounds-mark-failed"
              title={t('rounds_failed_title', { n: w.failed })}
            >
              <XCircleIcon aria-label={t('outcome_failed')} />
              {w.failed}
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
  { key: 'control', head: null, width: 110, num: false },
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

/// The works as rows. A row opens its work; what waits for the owner and the rounds that went wrong open that tab, or
/// only those rounds.
function Table({ works, tab, widths, onWidths }: {
  works: WorkSummary[];
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
          <Row key={w.id} w={w} tab={tab} now={now} />
        ))}
      </tbody>
    </table>
  );
}

/// A list with every row, or a word that it has none.
function List({ works, tab, widths, onWidths }: {
  works: WorkSummary[];
  tab: WorkTab;
  widths: Widths;
  onWidths: (w: Widths) => void;
}) {
  if (works.length === 0) return <p className="muted">{t('none')}</p>;
  return (
    <div className="works-wide">
      <Table works={works} tab={tab} widths={widths} onWidths={onWidths} />
    </div>
  );
}

/// The line above an open work's chart, which only reads it: back to the list; which work it is; how it goes on; where
/// it is, its rounds, cost and pull request; and its group in the messages view, where the owner acts on it.
function WorkLine({ w }: { w: WorkSummary }) {
  const status = useStatus();
  const a = { c: course(w, status) };
  const now = useNow(status !== null && status.running === w.id);
  const repo = w.project !== null ? status?.repos[w.project] : undefined;
  const project = projectOf(w, status);
  const name = w.item !== null ? `#${w.id} · #${w.item} ${w.title ?? ''}${project ? ` · ${project}` : ''}` : `#${w.id}`;
  const stage = w.stage !== null ? w.stage.replaceAll('_', ' ') : '';
  const at = `${w.at} ${w.at_title ?? ''}${stage ? ` · ${stage}` : ''}`;
  return (
    <div className="work-line">
      <WorksBack />
      <span className="work-name" title={name}>
        <strong>#{w.id}</strong>{' '}
        {w.item !== null ? (
          <>
            <ItemRef id={w.item} title={w.title} hint={`${name}\n${t('open_item')}`} />
            {project ? <span className="muted"> · {project}</span> : null}
          </>
        ) : (
          w.graph
        )}
      </span>
      <span className="work-state" title={held(a.c, w) ?? w.ended ?? undefined}>
        <State w={w} c={a.c} now={now} />
      </span>
      <span className="work-where" title={a.c.outdated ? `${at}\n${takeoverWords(w, status)}` : at}>
        <span className="cell-id">{w.at}</span>{' '}
        {a.c.outdated ? (
          <>
            <Label variant="attention" title={takeoverWords(w, status)}>
              {t('course_outdated', { version: w.flow_version })}
            </Label>{' '}
          </>
        ) : w.moved_from !== null ? (
          <>
            <TagChip title={t('course_moved_hint', { from: w.moved_from, version: w.flow_version })}>
              {t('course_moved', { version: w.flow_version })}
            </TagChip>{' '}
          </>
        ) : null}
        {w.at_title}
        {stage ? <span className="muted"> · {stage}</span> : null}
      </span>
      <span className="muted">
        {joined([
          tn('work_rounds_n', w.rounds),
          cost(w.cost_usd),
          w.pr === null ? null : repo !== undefined ? (
            <PrimerLink
              key="pr"
              href={`https://github.com/${repo}/pull/${w.pr}`}
              target="_blank"
              rel="noreferrer"
              muted
              title={t('work_pr_hint')}
            >
              {t('work_pr', { n: w.pr })}
            </PrimerLink>
          ) : (
            t('work_pr', { n: w.pr })
          ),
          <Link key="talk" to={href({ page: 'messages', open: `work:${w.id}` })} title={t('go_to_conversation_hint')} muted>
            {t('go_to_conversation')}
          </Link>,
        ])}
      </span>
    </div>
  );
}

/// The way back from a work to the list of works.
function WorksBack() {
  return (
    <Link to="/works" className="work-back" title={t('work_back')}>
      <ArrowLeftIcon /> {t('nav_works')}
    </Link>
  );
}

export function Works({ work }: { work: WorkRoute | null }) {
  const works = useResource<WorkSummary[]>('works', '/api/v1/works');
  const tab = work?.tab ?? 'rounds';
  // Both lists have the same columns at the same widths.
  const [widths, setWidths] = useState<Widths>(savedWidths);
  const all = works.data ?? [];
  const active = all.filter((w) => !ENDED.has(w.state)).sort((a, b) => b.id - a.id);
  const open = work !== null ? all.find((w) => w.id === work.id) : undefined;
  return (
    <div className="works-screen">
      {/* The list stays as it was, its ended works folded or not, while a work is open in its place. */}
      <div className="works-lists" style={work !== null ? { display: 'none' } : undefined}>
        <Loaded r={works}>
          {(list) => {
            const ended = list
              .filter((w) => ENDED.has(w.state))
              .sort((a, b) => (b.last_at ?? 0) - (a.last_at ?? 0));
            return (
              <HeadingLevel.Provider value={2}>
                <section className="works-active">
                  <div className="works-head">
                    <Heading as="h2" variant="small">
                      {t('works_active')}
                    </Heading>
                    <CounterLabel>{active.length}</CounterLabel>
                  </div>
                  <List works={active} tab={tab} widths={widths} onWidths={setWidths} />
                </section>
                <Section title={t('works_ended')} n={ended.length}>
                  <List works={ended} tab={tab} widths={widths} onWidths={setWidths} />
                </Section>
              </HeadingLevel.Provider>
            );
          }}
        </Loaded>
      </div>
      {work !== null ? (
        <>
          {open !== undefined ? (
            <WorkLine w={open} />
          ) : (
            <div className="work-line">
              <WorksBack />
            </div>
          )}
          <Work r={work} />
        </>
      ) : null}
    </div>
  );
}
