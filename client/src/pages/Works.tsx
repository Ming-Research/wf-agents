// Every work, or the one open. The list is one table over the window's width, a group of rows for each project: the
// project's head (its name and, at the right end of the same row, the buttons that show or hide its works by state, the
// very buttons of the list of conversations and the owner's one choice for both, done and ended works hidden at first),
// then its architect, which opens the architect's page, then its works, newest first. The groups share the columns and
// their widths, which the owner drags, and every cell stays on one line. A row says how its work goes on (a round
// running, what holds it back, what it waits for), its state as it is in effect and why, and leads to its group in the
// messages view, the one place the owner acts on it. A work opened from it takes the whole window under the bar, its
// chart under a line that says which work it is, how it goes on and where, leads to its group, and to the other works
// running or waiting with what each needs of the owner. The marks of rounds that went wrong open the work with only
// those rounds.

import { useState, type PointerEvent as ReactPointerEvent } from 'react';

import type { ArchitectStanding, Status, WorkState, WorkSummary } from '../api';
import { architectMark, architectTotals } from '../architect';
import { course, liveState, Loaded, StateChip, WORK_TONE, WorkStateFilter } from '../components';
import { sortedProjects } from '../conversation';
import { bringUpWords, held } from '../course';
import { cost, duration, time } from '../format';
import { useResource, useShared, useStatus } from '../live';
import { ItemRef, PrLink, ProjectHead } from '../parts';
import { architectHref, href, Link, navigate, type WorkRoute, type WorkTab, workHref } from '../route';
import { t, tn } from '../text';
import { joinedWords, SEP } from '../voices';
import { HeadLine, State, Where, WorksBack } from './HeadLine';
import { useTalkStates } from './Messages';
import { Work } from './Work';
import { RoundsMarks } from './WorkChart';

/// The project a work's item is of, said after its title on the line over an open work's chart, where no project's head
/// says it, and only while there is more than one project, as the engine's configuration has them: with one, it says
/// nothing.
function projectOf(w: WorkSummary, status: Status | null): string | null {
  return status !== null && Object.keys(status.repos).length > 1 ? w.project : null;
}

/// The project after a work's item, a fact about it, where it is said at all.
function ProjectAfter({ project }: { project: string | null }) {
  if (project === null) return null;
  return (
    <span className="muted">
      {SEP}
      {project}
    </span>
  );
}

/// A work's row, every cell on one line, cut where its column ends and whole on hovering; its item opens from it, and
/// its group in the messages view, where the owner acts on it, from its link.
function Row({ w, tab }: { w: WorkSummary; tab: WorkTab }) {
  const status = useStatus();
  const a = { c: course(w, status) };
  const item = w.item !== null ? `#${w.item} ${w.title ?? ''}` : w.graph;
  const at = `${w.at} ${w.at_title ?? ''}`;
  const stage = w.stage !== null ? w.stage.replaceAll('_', ' ') : '';
  return (
    <tr className="clickable" onClick={() => navigate(workHref(w.id, { tab }))}>
      <td>{t('work_name', { id: w.id })}</td>
      <td title={item}>
        {w.item !== null ? <ItemRef id={w.item} title={w.title} hint={`${item}\n${t('open_item')}`} /> : w.graph}
      </td>
      <td className="state-cell" title={held(a.c, w) ?? w.ended ?? undefined}>
        <State w={w} c={a.c} />
      </td>
      <td className="control-cell" onClick={(e) => e.stopPropagation()}>
        <ConversationLink w={w} />
      </td>
      <td title={a.c.outdated ? `${at}\n${bringUpWords(w, status)}` : at}>
        <Where w={w} c={a.c} />
      </td>
      <td title={stage}>{stage}</td>
      <td className="num" title={String(w.rounds)}>
        {w.rounds}
        <RoundsMarks w={w} to={(only) => workHref(w.id, { only })} />
      </td>
      <td className="num" title={duration(w.ran_ms)}>
        {duration(w.ran_ms)}
      </td>
      <td title={time(w.last_at)}>{time(w.last_at)}</td>
      <td className="num" title={cost(w.cost_usd)}>
        {cost(w.cost_usd)}
      </td>
      <td title={w.pr !== null ? `#${w.pr}` : undefined}>{w.pr !== null ? `#${w.pr}` : ''}</td>
    </tr>
  );
}

/// A project's architect's row, the first of its group: the columns of a work's row, read over all the architect's works.
/// Its work is "Architect", its item the architect of the project, leading to its page, which the row opens too; its
/// state is how the architect stands, as its mark in the list of conversations says it; it is at the cell of the work
/// that goes on, or else the newest; its rounds, run time and cost are the sums over its works and its last activity
/// the latest of theirs; it has no stage and no pull request. Its group in the messages view, where the owner speaks to
/// it, is its link. The buttons that show or hide works by state leave it alone, as they leave the architect's row in
/// the list of conversations.
export function ArchitectRow({ a }: { a: ArchitectStanding }) {
  const status = useStatus();
  const mark = architectMark(a, status);
  const total = architectTotals(a);
  const w = total.work;
  const page = architectHref({ project: a.project });
  const name = t('conv_architect', { project: a.project });
  const at = w !== null ? `${w.at} ${w.at_title ?? ''}` : '';
  return (
    <tr className="clickable works-architect" onClick={() => navigate(page)}>
      <td>{t('works_architect')}</td>
      <td title={name}>
        <Link to={page} title={t('talk_architect_page')}>
          {name}
        </Link>
      </td>
      <td className="state-cell" title={mark.words}>
        <span className="course">
          <StateChip tone={WORK_TONE[mark.state]}>{mark.words}</StateChip>
        </span>
      </td>
      <td className="control-cell" onClick={(e) => e.stopPropagation()}>
        <Link to={href({ page: 'messages', open: `architect:${a.project}` })} title={t('go_to_conversation_hint')}>
          {t('go_to_conversation')}
        </Link>
      </td>
      <td title={at}>{w !== null ? <Where w={w} c={course(w, status)} /> : null}</td>
      <td />
      <td className="num" title={String(total.rounds)}>
        {total.rounds}
      </td>
      <td className="num" title={duration(total.ran_ms)}>
        {duration(total.ran_ms)}
      </td>
      <td title={time(total.last_at)}>{time(total.last_at)}</td>
      <td className="num" title={cost(total.cost_usd)}>
        {cost(total.cost_usd)}
      </td>
      <td />
    </tr>
  );
}

/// The columns of the table, in order, with their headers and their own widths; the item takes what the others leave.
const COLUMNS = [
  { key: 'id', head: 'col_work', width: 96, num: false },
  { key: 'item', head: 'col_item', width: null, num: false },
  { key: 'state', head: 'col_state', width: 360, num: false },
  { key: 'control', head: null, width: 110, num: false },
  { key: 'at', head: 'col_at', width: 190, num: false },
  { key: 'stage', head: 'col_stage', width: 120, num: false },
  { key: 'rounds', head: 'col_rounds', width: 90, num: true },
  { key: 'ran', head: 'col_ran', width: 90, num: true },
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

/// The works shown in a project's group: those whose state, as it is in effect, the owner's buttons show, newest first.
/// The buttons count the project's works by the same states (`projectStates`), so that no button counts what the rows
/// do not show.
export function shownWorks(
  works: readonly WorkSummary[],
  status: Status | null,
  shown: readonly WorkState[],
): WorkSummary[] {
  return works.filter((w) => shown.includes(liveState(w, status))).sort((a, b) => b.id - a.id);
}

/// The state of each work of a project, as it is in effect, which its buttons count.
export const projectStates = (works: readonly WorkSummary[], status: Status | null): WorkState[] =>
  works.map((w) => liveState(w, status));

/// The id of the heading of a project's group, which the group is labelled by.
const projectHeading = (project: string) => `works-project-${project}`;

/// A project's head row, the first of its group: its name and, at the right end of the same row, the buttons that show
/// or hide its works by state; none for a project with no work. Its cell spans the table and holds the head in a block
/// as wide as the window's visible part, which stays at the left while the table scrolls sideways, so that the buttons
/// are never scrolled out of sight.
function ProjectRow({ project, states, shown, onShown }: {
  project: string;
  states: readonly WorkState[];
  shown: readonly WorkState[];
  onShown: (s: WorkState) => void;
}) {
  return (
    <tr className="works-project">
      <td colSpan={COLUMNS.length} className="works-project-cell">
        <div className="works-project-head">
          <ProjectHead id={projectHeading(project)} name={project}>
            {states.length > 0 ? <WorkStateFilter states={states} shown={shown} onShown={onShown} /> : null}
          </ProjectHead>
        </div>
      </td>
    </tr>
  );
}

/// The works as one table: its header, then a group of rows for each project, by name as the list of conversations
/// orders its projects (`sortedProjects`): the project's head, its architect, once read, and the works its buttons
/// show. A row opens its work; what waits for the owner and the rounds that went wrong open that tab, or only those
/// rounds.
function Table({ works, architects, tab, widths, onWidths }: {
  works: WorkSummary[];
  architects: ArchitectStanding[] | undefined;
  tab: WorkTab;
  widths: Widths;
  onWidths: (w: Widths) => void;
}) {
  const status = useStatus();
  const [shown, toggle] = useTalkStates();
  const fixed = COLUMNS.filter((c) => c.width !== null).reduce((sum, c) => sum + widthOf(widths, c.key), 0);
  const projects = sortedProjects([...(architects ?? []).map((a) => a.project), ...works.map((w) => w.project ?? '')]);
  if (projects.length === 0) return <p className="muted">{t('none')}</p>;
  return (
    <div className="works-wide">
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
        {projects.map((p) => {
          const mine = works.filter((w) => (w.project ?? '') === p);
          const architect = architects?.find((a) => a.project === p);
          return (
            <tbody key={p} aria-labelledby={projectHeading(p)}>
              <ProjectRow project={p} states={projectStates(mine, status)} shown={shown} onShown={toggle} />
              {architect !== undefined ? <ArchitectRow a={architect} /> : null}
              {shownWorks(mine, status, shown).map((w) => (
                <Row key={w.id} w={w} tab={tab} />
              ))}
            </tbody>
          );
        })}
      </table>
    </div>
  );
}

/// A work's conversation in the messages view, where the owner acts on it.
function ConversationLink({ w }: { w: WorkSummary }) {
  return (
    <Link to={href({ page: 'messages', open: `work:${w.id}` })} title={t('go_to_conversation_hint')}>
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
  const project = projectOf(w, status);
  const name = w.item !== null ? joinedWords([`#${w.id}`, `#${w.item} ${w.title ?? ''}`, project]) : `#${w.id}`;
  const stage = w.stage !== null ? w.stage.replaceAll('_', ' ') : '';
  const at = joinedWords([`${w.at} ${w.at_title ?? ''}`, stage]);
  return (
    <HeadLine
      lead={<WorksBack />}
      name={
        <>
          <strong>{t('work_name', { id: w.id })}</strong>{' '}
          {w.item !== null ? (
            <>
              <ItemRef id={w.item} title={w.title} hint={`${name}\n${t('open_item')}`} />
              <ProjectAfter project={project} />
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
      whereHint={c.outdated ? `${at}\n${bringUpWords(w, status)}` : at}
      facts={[
        tn('work_rounds_n', w.rounds),
        cost(w.cost_usd),
        w.pr !== null ? <PrLink key="pr" project={w.project} n={w.pr} /> : null,
      ]}
      conversation={`work:${w.id}`}
    />
  );
}

export function Works({ work }: { work: WorkRoute | null }) {
  const works = useResource<WorkSummary[]>('works', '/api/v1/works');
  // How every project's architect stands, read once for the page and again when an architect changes.
  const architects = useShared<ArchitectStanding[]>('architect', '/api/v1/architects');
  const tab = work?.tab ?? 'rounds';
  // The columns' widths are one for every project's group.
  const [widths, setWidths] = useState<Widths>(savedWidths);
  const open = work !== null ? works.data?.find((w) => w.id === work.id) : undefined;
  return (
    <div className="works-screen">
      {/* The table stays as it was, its buttons as they were set, while a work is open in its place. */}
      <div className="works-lists" style={work !== null ? { display: 'none' } : undefined}>
        <Loaded r={works}>
          {(list) => <Table works={list} architects={architects} tab={tab} widths={widths} onWidths={setWidths} />}
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
