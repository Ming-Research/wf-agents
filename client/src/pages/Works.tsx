// Every work, or the one open. The list shows every row: those running or waiting, then those that have ended,
// folded at first; both have the same columns at the same widths, which the owner drags, so that they read as one
// table, and every cell stays on one line. A row says how its work goes on (a round running, what holds it back, what
// it waits for), its state as it is in effect and why, and leads to its group in the messages view, the one place the
// owner acts on it. A work opened from it takes the whole window under the bar, its chart under a line that says which
// work it is, how it goes on and where, leads to its group, and to the other works running or waiting with what each
// needs of the owner. The marks of rounds that went wrong open the work with only those rounds.

import { CounterLabel, Heading, Link as PrimerLink } from '@primer/react';
import { useState, type PointerEvent as ReactPointerEvent } from 'react';

import type { Status, WorkSummary } from '../api';
import { course, Loaded } from '../components';
import { held, takeoverWords } from '../course';
import { cost, time } from '../format';
import { useResource, useStatus } from '../live';
import { HeadingLevel, ItemRef, Section } from '../parts';
import { href, Link, navigate, type WorkRoute, type WorkTab, workHref } from '../route';
import { t, tn } from '../text';
import { HeadLine, State, Where, WorksBack } from './HeadLine';
import { Work } from './Work';
import { RoundsMarks } from './WorkChart';

const ENDED = new Set(['done', 'ended']);

/// The project a work's item is of, said after its title only while there is more than one project, as the engine's
/// configuration has them: with one, it says nothing.
function projectOf(w: WorkSummary, status: Status | null): string | null {
  return status !== null && Object.keys(status.repos).length > 1 ? w.project : null;
}

/// A work's row, every cell on one line, cut where its column ends and whole on hovering; its item opens from it, and
/// its group in the messages view, where the owner acts on it, from its link.
function Row({ w, tab }: { w: WorkSummary; tab: WorkTab }) {
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
        <State w={w} c={a.c} />
      </td>
      <td className="control-cell" onClick={(e) => e.stopPropagation()}>
        <ConversationLink w={w} />
      </td>
      <td title={a.c.outdated ? `${at}\n${takeoverWords(w, status)}` : at}>
        <Where w={w} c={a.c} />
      </td>
      <td title={stage}>{stage}</td>
      <td className="num" title={String(w.rounds)}>
        {w.rounds}
        <RoundsMarks w={w} to={(only) => workHref(w.id, { only })} />
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
          <Row key={w.id} w={w} tab={tab} />
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

/// A work's conversation in the messages view, where the owner acts on it.
function ConversationLink({ w }: { w: WorkSummary }) {
  return (
    <Link to={href({ page: 'messages', open: `work:${w.id}` })} title={t('go_to_conversation_hint')} muted>
      {t('go_to_conversation')}
    </Link>
  );
}

/// The line above an open work's chart, which only reads it, as the shared head line draws it: back to the list; which
/// work it is; how it goes on; where it is and its stage; its rounds, cost and pull request; and its group in the
/// messages view, where the owner acts on it.
function WorkLine({ w }: { w: WorkSummary }) {
  const status = useStatus();
  const c = course(w, status);
  const repo = w.project !== null ? status?.repos[w.project] : undefined;
  const project = projectOf(w, status);
  const name = w.item !== null ? `#${w.id} · #${w.item} ${w.title ?? ''}${project ? ` · ${project}` : ''}` : `#${w.id}`;
  const stage = w.stage !== null ? w.stage.replaceAll('_', ' ') : '';
  const at = `${w.at} ${w.at_title ?? ''}${stage ? ` · ${stage}` : ''}`;
  return (
    <HeadLine
      lead={<WorksBack />}
      name={
        <>
          <strong>#{w.id}</strong>{' '}
          {w.item !== null ? (
            <>
              <ItemRef id={w.item} title={w.title} hint={`${name}\n${t('open_item')}`} />
              {project ? <span className="muted"> · {project}</span> : null}
            </>
          ) : (
            w.graph
          )}
        </>
      }
      nameHint={name}
      state={<State w={w} c={c} />}
      stateHint={held(c, w) ?? w.ended ?? undefined}
      where={<Where w={w} c={c} stage />}
      whereHint={c.outdated ? `${at}\n${takeoverWords(w, status)}` : at}
      facts={[
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
      ]}
      conversation={`work:${w.id}`}
    />
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
