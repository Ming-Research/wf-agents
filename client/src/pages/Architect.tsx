// A project's architect's page, laid out as a work's page and drawn with the same parts (the line over the chart, the
// chart, the panel, its rounds tab and the usage), read only: the owner speaks to the architect, answers it and starts,
// pauses and steps it in its one-to-one in the messages view, which the page's line leads to. Its line says, with more
// than one project, whose architect it is, with the switch to another's; what the architect does now in words (the
// matter it handles and at which cell, or that it is idle, paused or held back), with no word of its works, which to
// the owner are only how it is run; how many matters are left, its notebook and its policy, each leading to its tab of
// the panel beside the one-to-one, where they are; what stops the project's new works from starting; and the switch
// between the stretches it worked in (when each began, how many matters it handled, its rounds with those sent back or
// failed, what it cost), whose rounds the chart shows. Under the line, its graph fills the window
// with the stretch shown placed on it as a work is, and the window floating over the chart's right side holds the
// architect's panel, with what is open on the chart split under it, as on a work's page. The panel's tabs, on one line:
// its rounds; the item changes its batches made; and its usage over a span of time the owner picks. Each part names the
// cell that wrote it, and every tab draws its blocks, sections, items and rounds with the page's shared parts. The
// project's observations, which belong to the project rather than to a round of the architect, are a view of their
// own.

import { BlockedIcon, TriangleDownIcon } from '@primer/octicons-react';
import { Button, Heading, Label, Select } from '@primer/react';
import { useEffect, useMemo, useState } from 'react';

import type {
  Architect as ArchitectRead,
  ArchitectWork,
  Compiled,
  CompiledGraph,
  FlowView,
  ItemChange,
  WorkDetail,
  WorkSummary,
} from '../api';
import { architectState, holdWords, matterSentence, overWork, shownWork } from '../architect';
import { course, Loaded, StateChip } from '../components';
import { held, runningWords } from '../course';
import { cost, time } from '../format';
import { Labelled, words } from '../labelled';
import { useResource, useResources, useStatus } from '../live';
import {
  type ArchitectRoute,
  type ArchitectTab,
  architectHref,
  forProject,
  href,
  Link,
  navigate,
  sideHref,
  type Span,
  SPANS,
} from '../route';
import { t, tn } from '../text';
import { By, CellTitles, type OpenRound, PeriodSwitch, SubHeading } from '../parts';
import { ChangeBlock } from './ArchitectViews';
import { bringIntoView } from './Decide';
import { Held, HeadLine, OldFlow, Running } from './HeadLine';
import { Usage } from './Usage';
import { ChartPanel, RoundsMarks, RoundsTab, WorkChart } from './WorkChart';

/// The graph the architect's works run, as the flow names it.
const GRAPH = 'architect';

/// What the architect does now, on its line, with no word of its works: a round running and for how long, what holds it
/// back, or that it is between rounds; else idle, or held back with matters waiting.
function Now({ a, current }: { a: ArchitectRead; current: WorkSummary | null }) {
  const status = useStatus();
  const state = architectState(a.works, a.queue);
  if (state.kind === 'working' && current !== null) {
    const c = course(current, status);
    if (c.running !== null) {
      return (
        <Running
          words={c.running === 'step' ? runningWords(c) : t('talk_architect_working')}
          since={current.running?.started_at ?? null}
        />
      );
    }
    if (c.held !== null) return <Held c={c} why={held(c, current)} />;
    return <StateChip tone="neutral">{t('talk_architect_between')}</StateChip>;
  }
  const started = status !== null && status.architects_started.includes(a.project);
  const holds = state.waiting > 0 && status !== null && status.paused && !started;
  return (
    <StateChip tone={holds ? 'attention' : 'neutral'}>
      {holds ? t('talk_architect_held') : state.waiting > 0 ? t('talk_architect_takes_up') : t('talk_architect_idle')}
    </StateChip>
  );
}

/// Where the architect is while it works: the matter it handles and the cell it is at, and that its stretch is on an
/// older flow version.
function At({ a, current }: { a: ArchitectRead; current: WorkSummary }) {
  const status = useStatus();
  const c = course(current, status);
  const state = architectState(a.works, a.queue);
  const at = `${current.at} ${current.at_title ?? ''}`;
  const what = state.kind === 'working' && state.handling !== null ? matterSentence(state.handling) : null;
  return (
    <>
      {c.outdated ? (
        <>
          <OldFlow w={current} status={status} />{' '}
        </>
      ) : null}
      {what !== null ? t('talk_architect_handles', { what, at }) : t('talk_architect_at', { at })}
    </>
  );
}

/// The line above an architect's chart, as the shared head line draws it: with more than one project, whose architect
/// it is and the switch to another's; what it does now and where; the matters left, its notebook and its policy, each a
/// link to its tab of the panel beside the one-to-one; what stops the project's new works; its one-to-one in the
/// messages view, where the owner acts on it; and the switch between the stretches it worked in.
export function Line({ a, shown, current, pop, setPop }: {
  a: ArchitectRead;
  shown: ArchitectWork | null;
  /// The stretch going on, as freshly read as the page has it.
  current: WorkSummary | null;
  pop: 'works' | null;
  setPop: (p: 'works' | null) => void;
}) {
  const status = useStatus();
  const state = architectState(a.works, a.queue);
  const working = state.kind === 'working' ? current : null;
  const c = working !== null ? course(working, status) : null;
  const what = state.kind === 'working' && state.handling !== null ? matterSentence(state.handling) : null;
  return (
    <HeadLine
      lead={
        a.projects.length > 1 ? (
          <Select
            size="small"
            className="arch-project"
            value={a.project}
            title={t('arch_project_hint')}
            aria-label={t('arch_project_hint')}
            onChange={(e) => navigate(architectHref({ project: e.target.value }))}
          >
            {a.projects.map((p) => (
              <Select.Option key={p} value={p}>
                {t('arch_of_project', { project: p })}
              </Select.Option>
            ))}
          </Select>
        ) : undefined
      }
      state={
        <span className="course">
          <Now a={a} current={working} />
        </span>
      }
      stateHint={working !== null && c !== null ? (held(c, working) ?? undefined) : undefined}
      where={working !== null ? <At a={a} current={working} /> : undefined}
      whereHint={working !== null ? (what ?? `${working.at} ${working.at_title ?? ''}`) : undefined}
      facts={[
        <Link key="queue" to={sideHref(a.project, 'queue')}>
          {tn('talk_queue_left', state.waiting)}
        </Link>,
        <Link key="notebook" to={sideHref(a.project, 'notebook')}>
          {t('side_tab_notebook')}
        </Link>,
        <Link key="policy" to={sideHref(a.project, 'policy')}>
          {t('arch_tab_policy')}
        </Link>,
      ]}
      conversation={`architect:${a.project}`}
      end={
        <>
          {a.holds.length > 0 ? (
            <span className="talk-head-end">
              {a.holds.map((h) => (
                <span key={h.reason} className="talk-head-hold" title={t('hold_since', { at: time(h.since) })}>
                  <BlockedIcon size={16} />
                  {holdWords(h.reason)}
                  {h.why !== null ? `: ${h.why}` : null}
                </span>
              ))}
            </span>
          ) : null}
          {shown !== null ? (
            <Button
              size="small"
              className="arch-work-switch"
              trailingAction={TriangleDownIcon}
              aria-expanded={pop === 'works'}
              title={t('arch_works_hint')}
              onClick={() => setPop(pop === 'works' ? null : 'works')}
            >
              {t('arch_work_shown', { at: time(shown.work.started_at) })}
            </Button>
          ) : null}
        </>
      }
    />
  );
}

/// The stretches the architect worked in, newest first: when each began, how many matters it handled, its rounds and
/// cost; a click shows that stretch's rounds on the chart, and the one shown by default follows the one going on again.
function WorkList({ a, shown, tab, project, onClose }: {
  a: ArchitectRead;
  shown: number | null;
  tab: ArchitectTab;
  /// The project the address names, kept as it is.
  project: string | null;
  onClose: () => void;
}) {
  const followed = shownWork(a.works, null)?.work.id ?? null;
  return (
    <div className="arch-works" role="dialog" aria-labelledby="arch-works-title">
      <Heading as="h2" variant="small" id="arch-works-title">
        {t('arch_works', { n: a.works.length })}
      </Heading>
      <div className="arch-works-list">
        <table className="list">
          <thead>
            <tr>
              <th>{t('arch_col_from')}</th>
              <th className="num">{t('arch_col_handled')}</th>
              <th className="num">{t('col_rounds')}</th>
              <th className="num">{t('col_cost')}</th>
            </tr>
          </thead>
          <tbody>
            {a.works.map((x) => (
              <tr
                key={x.work.id}
                className={`clickable${x.work.id === shown ? ' selected' : ''}`}
                onClick={() => {
                  navigate(architectHref({ project, work: x.work.id === followed ? null : x.work.id, tab }));
                  onClose();
                }}
              >
                <td className="nowrap">
                  {time(x.work.started_at)}{' '}
                  {!overWork(x.work) ? <Label variant="accent">{t('arch_work_going')}</Label> : null}
                </td>
                <td className="num">{x.handled}</td>
                <td className="num">
                  {x.work.rounds}
                  <RoundsMarks
                    w={x.work}
                    to={(only) => architectHref({ project, work: x.work.id === followed ? null : x.work.id, only })}
                  />
                </td>
                <td className="num">{cost(x.work.cost_usd)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className="arch-works-actions">
        <Button size="small" onClick={onClose}>
          {t('close')}
        </Button>
      </div>
    </div>
  );
}

/// The item changes an architect's batches made or dropped, newest first: each change as handed in, with why; what it
/// made, or for one dropped, what the review still found; the matter it came from and the round that handed it in.
function Changes({ changes, onRound }: { changes: ItemChange[]; onRound: OpenRound }) {
  if (changes.length === 0) return <p className="muted">{t('arch_changes_none')}</p>;
  return (
    <>
      {changes.map((c) => (
        <ChangeBlock
          key={c.id}
          value={c.change}
          before={
            <>
              <Label variant={c.state === 'made' ? 'success' : 'danger'}>{words(c.state)}</Label>
              <span className="muted">{t('change_matter', { n: c.matter })}</span>
            </>
          }
          by={<By cell={c.cell} work={c.work} round={c.round} onRound={onRound} />}
        >
          {c.outcome !== null ? (
            <div className="field">
              <SubHeading>{t(c.state === 'made' ? 'change_made' : 'change_dropped')}</SubHeading>
              <Labelled value={c.outcome} />
            </div>
          ) : null}
        </ChangeBlock>
      ))}
    </>
  );
}

/// How long back from now each span of the architect's usage counts, in milliseconds; all time has no start.
const SPAN_MS: Record<Span, number | null> = { '1d': 86_400_000, '7d': 7 * 86_400_000, '30d': 30 * 86_400_000, all: null };

/// The architect's usage over the span of time the owner picks, the last 7 days at first: the rounds of its works that
/// started in that span, counted as one, each work read in full as a work's usage reads it.
function ArchitectUsage({ route, a, compiled }: { route: ArchitectRoute; a: ArchitectRead; compiled: Compiled }) {
  const span = SPAN_MS[route.span];
  const since = span !== null ? Date.now() - span : null;
  const works = a.works.filter((x) => since === null || !overWork(x.work) || (x.work.last_at ?? x.work.started_at ?? 0) >= since);
  const details = useResources<WorkDetail>(
    works.map((x) => ({ key: `work:${x.work.id}`, path: `/api/v1/works/${x.work.id}` })),
  );
  // Its works ran the architect's graph and, a flow version before, the policy setting's: the cells of both.
  const cells = new Map(
    compiled.graphs
      .filter((g) => g.name === 'architect' || g.name === 'policy')
      .flatMap((g) => g.cells.map((c) => [c.id, { kind: c.kind, title: c.title }] as const)),
  );
  return (
    <>
      <PeriodSwitch
        periods={SPANS.map((x) => ({
          key: x,
          label: x === 'all' ? t('sys_period_all') : t(`arch_span_${x}`),
          to: href({ ...route, span: x }),
          on: x === route.span,
        }))}
      />
      <Loaded r={details}>{(list) => <Usage works={list} cells={cells} since={since} />}</Loaded>
    </>
  );
}

/// The architect's panel, on top of the window floating over the right of its chart, in tabs, each saying how many of its
/// things there are.
function Panel({ route, a, d, g, compiled, go, onRound }: {
  route: ArchitectRoute;
  a: ArchitectRead;
  /// The work shown, in full; none before the architect's first work.
  d: WorkDetail | null;
  g: CompiledGraph;
  compiled: Compiled;
  go: (to: Partial<ArchitectRoute>) => void;
  onRound: OpenRound;
}) {
  const tab = (x: ArchitectTab) => href({ ...route, tab: x });
  return (
    <ChartPanel
      label={t('arch_tabs')}
      current={route.tab}
      onTab={(x) => go({ tab: x })}
      tabs={[
        { tab: 'rounds', label: t('rounds'), href: tab('rounds'), n: d?.rounds.length ?? null },
        { tab: 'changes', label: t('arch_tab_changes'), href: tab('changes'), n: a.changes.length },
        { tab: 'usage', label: t('us_tab'), href: tab('usage'), n: null },
      ]}
    >
      {route.tab === 'changes' ? <Changes changes={a.changes} onRound={onRound} /> : null}
      {route.tab === 'usage' ? <ArchitectUsage route={route} a={a} compiled={compiled} /> : null}
      {route.tab === 'rounds' ? (
        d === null ? (
          <p className="muted">{a.works.length === 0 ? t('arch_never_worked') : t('loading')}</p>
        ) : (
          <RoundsTab
            d={d}
            g={g}
            only={route.only}
            selected={route.work === d.work.id ? route.round : null}
            onAll={() => go({ only: null })}
            onRound={(x) => go({ work: d.work.id, round: x, cell: null })}
          />
        )
      ) : null}
    </ChartPanel>
  );
}

/// The architect's graph filling the window, with the work shown placed on it, and the window floating over its right
/// side: the panel on top, what is open on the chart under it.
export function Chart({ route, a, shown, d, compiled, view, go }: {
  route: ArchitectRoute;
  a: ArchitectRead;
  shown: ArchitectWork | null;
  d: WorkDetail | null;
  compiled: Compiled;
  view: FlowView;
  go: (to: Partial<ArchitectRoute>) => void;
}) {
  const id = shown?.work.id ?? null;
  // A round of any work of the architect opens on the chart of that work.
  const onRound: OpenRound = (work, round) => go({ work, round, cell: null });
  // The chart draws the graph of the work shown, as its flow version has it: an architect work's, or the policy
  // setting's a flow version before ran as a work of its own, so that a round chosen of either is found on it.
  return (
    <WorkChart
      compiled={compiled}
      view={view}
      graph={GRAPH}
      d={d}
      cell={route.cell}
      round={route.work === id ? route.round : null}
      onCell={(cell) => go({ cell, round: null })}
      onRound={(x) => go(x !== null ? { work: id, round: x, cell: null } : { round: null })}
      panel={(g) => <Panel route={route} a={a} d={d} g={g} compiled={compiled} go={go} onRound={onRound} />}
    />
  );
}

function Body({ route, a }: { route: ArchitectRoute; a: ArchitectRead }) {
  const status = useStatus();
  const shown = shownWork(a.works, route.work);
  const id = shown?.work.id ?? null;
  const detail = useResource<WorkDetail>(id !== null ? `work:${id}` : null, id !== null ? `/api/v1/works/${id}` : null);
  const version = shown?.work.flow_version ?? status?.flow_version ?? null;
  const compiled = useResource<Compiled>(null, version !== null ? `/api/v1/flows/${version}` : null);
  const view = useResource<FlowView>(null, version !== null ? `/api/v1/flows/${version}/view` : null);
  const d = detail.data !== undefined && detail.data.work.id === id ? detail.data : null;
  const going = a.works.find((w) => !overWork(w.work)) ?? null;
  const current = going !== null && d !== null && d.work.id === going.work.id ? d.work : (going?.work ?? null);
  // The list of the stretches stays open on the address it was opened at: whatever is picked from it closes it.
  const [popAt, setPopAt] = useState<string | null>(null);
  const pop = popAt === href(route) ? 'works' : null;
  const setPop = (p: 'works' | null) => setPopAt(p === 'works' ? href(route) : null);
  const go = (to: Partial<ArchitectRoute>) => navigate(href({ ...route, ...to }), true);
  // A link to an ask on this page brings it into view once the page is there.
  const ready = compiled.data !== undefined && view.data !== undefined;
  useEffect(() => {
    if (!ready) return;
    const at = window.location.hash.slice(1);
    if (at !== '') bringIntoView(at);
  }, [ready]);
  const titles = useMemo(
    () => new Map((compiled.data?.graphs.find((g) => g.name === 'architect')?.cells ?? []).map((c) => [c.id, c.title])),
    [compiled.data],
  );
  return (
    <CellTitles.Provider value={titles}>
      <div className="arch-screen">
        <Line a={a} shown={shown} current={current} pop={pop} setPop={setPop} />
        {pop === 'works' ? (
          <WorkList a={a} shown={id} tab={route.tab} project={route.project} onClose={() => setPop(null)} />
        ) : null}
        <div className="screen">
          <Loaded r={compiled}>
            {(c) => (
              <Loaded r={view}>
                {(v) => (
                  <Chart route={route} a={a} shown={shown} d={d} compiled={c} view={v} go={go} />
                )}
              </Loaded>
            )}
          </Loaded>
        </div>
      </div>
    </CellTitles.Provider>
  );
}

/// What this browser kept of the split and the fold of the conversation under the architect's panel, before the
/// conversation became a tab of the panel; cleared when the page opens. This goes once the browsers that kept them
/// have opened the page, by the first version's review.
const KEPT_BEFORE = ['wf-agents.arch-split', 'wf-agents.arch-talk-folded'];

export function Architect({ route }: { route: ArchitectRoute }) {
  const a = useResource<ArchitectRead>('architect', forProject('/api/v1/architect', route.project));
  useEffect(() => {
    try {
      for (const key of KEPT_BEFORE) localStorage.removeItem(key);
    } catch {
      // Nothing is kept where the browser lets nothing be.
    }
  }, []);
  return (
    <div className="works-screen">
      <Loaded r={a}>{(data) => <Body route={route} a={data} />}</Loaded>
    </div>
  );
}
