// A project's architect's page, laid out as a work's page and drawn with the same parts (the chart, the panel, its
// list of rounds and the usage), read only and with no line over its chart: what the architect does now, the holds on
// the project's new works, its matters left, its notebook and its policy are in the head of its one-to-one and the
// panel beside it in the messages view, where the owner speaks to it, answers it and starts, pauses and steps it; and
// another project's architect is reached from that project's one-to-one, whose head links to its page. Its graph fills
// the window under the bar, drawn on the flow version its latest work runs on, with the rounds its list holds placed on
// it and the cell it is at, and the window floating over the chart's right side holds the architect's panel, with what
// is open on the chart split under it, as on a work's page. The panel's tabs, on one line: its rounds, across all its
// works as one list, opening on the newest or around the round the address names; the item changes its batches made;
// and its usage over a span of time the owner picks. Each part names the cell that wrote it, and every tab draws its
// blocks, sections, items and rounds with the page's shared parts. The project's observations, which belong to the
// project rather than to a round of the architect, are a view of their own.

import { Flash, Label } from '@primer/react';
import { useEffect, useMemo } from 'react';

import type {
  Architect as ArchitectRead,
  Compiled,
  CompiledGraph,
  FlowView,
  ItemChange,
  RoundSummary,
  WorkDetail,
  WorkSummary,
} from '../api';
import { overWork } from '../architect';
import { Loaded } from '../components';
import { Labelled, words } from '../labelled';
import { useResource, useResources, useStatus } from '../live';
import { type ArchitectRoute, type ArchitectTab, forProject, href, navigate, type Span, SPANS } from '../route';
import { useRoundsWindow } from '../rounds';
import { t } from '../text';
import { By, CellTitles, type OpenRound, PeriodSwitch, SubHeading } from '../parts';
import { ChangeBlock } from './ArchitectViews';
import { bringIntoView } from './Decide';
import { Usage } from './Usage';
import { ChartPanel, RoundsList, WorkChart } from './WorkChart';

/// The graph the architect's works run, as the flow names it.
const GRAPH = 'architect';

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

/// The architect's rounds, as the panel's rounds tab lists them: one list across all its works.
type Rounds = ReturnType<typeof useRoundsWindow>;

/// The architect's panel, on top of the window floating over the right of its chart, in tabs, each saying how many of its
/// things there are: its rounds across all its works, its item changes and its usage.
function Panel({ route, a, rounds, g, compiled, go, onRound }: {
  route: ArchitectRoute;
  a: ArchitectRead;
  rounds: Rounds;
  g: CompiledGraph;
  compiled: Compiled;
  go: (to: Partial<ArchitectRoute>) => void;
  onRound: OpenRound;
}) {
  const tab = (x: ArchitectTab) => href({ ...route, tab: x });
  const total = a.works.reduce((n, x) => n + x.work.rounds, 0);
  const w = rounds.w;
  return (
    <ChartPanel
      label={t('arch_tabs')}
      current={route.tab}
      onTab={(x) => go({ tab: x })}
      tabs={[
        { tab: 'rounds', label: t('rounds'), href: tab('rounds'), n: total },
        { tab: 'changes', label: t('arch_tab_changes'), href: tab('changes'), n: a.changes.length },
        { tab: 'usage', label: t('us_tab'), href: tab('usage'), n: null },
      ]}
    >
      {route.tab === 'changes' ? <Changes changes={a.changes} onRound={onRound} /> : null}
      {route.tab === 'usage' ? <ArchitectUsage route={route} a={a} compiled={compiled} /> : null}
      {route.tab === 'rounds' ? (
        <>
          {rounds.error !== null ? <Flash variant="danger">{t('error', { error: rounds.error })}</Flash> : null}
          {a.works.length === 0 ? (
            <p className="muted">{t('arch_never_worked')}</p>
          ) : w === null ? (
            rounds.error === null ? <p className="muted">{t('loading')}</p> : null
          ) : (
            <RoundsList
              rounds={w.rounds}
              g={g}
              selected={route.round}
              onRound={(x) => go({ round: { work: x.work, n: x.n }, cell: null })}
              older={w.older}
              newer={w.newer}
              onMore={rounds.error === null ? rounds.more : undefined}
              loading={rounds.loading}
            />
          )}
        </>
      ) : null}
    </ChartPanel>
  );
}

/// No rounds, while none is read yet.
const NO_ROUNDS: RoundSummary[] = [];

/// The architect's graph filling the window, with the rounds its list holds placed on it and the cell its latest work is
/// at, and the window floating over its right side: the panel on top, what is open on the chart under it.
export function Chart({ route, a, latest, rounds, compiled, view, go }: {
  route: ArchitectRoute;
  a: ArchitectRead;
  /// Its latest work, the one going on, else the one that ended last; none before its first.
  latest: WorkSummary | null;
  rounds: Rounds;
  compiled: Compiled;
  view: FlowView;
  go: (to: Partial<ArchitectRoute>) => void;
}) {
  // A round of any work of the architect opens on the chart, its list brought to it.
  const onRound: OpenRound = (work, n) => go({ round: { work, n }, cell: null });
  return (
    <WorkChart
      compiled={compiled}
      view={view}
      graph={GRAPH}
      w={latest}
      rounds={rounds.w?.rounds ?? NO_ROUNDS}
      cell={route.cell}
      round={route.round}
      onCell={(cell) => go({ cell, round: null })}
      onRound={(x) => go({ round: x, cell: null })}
      panel={(g) => <Panel route={route} a={a} rounds={rounds} g={g} compiled={compiled} go={go} onRound={onRound} />}
    />
  );
}

function Body({ route, a }: { route: ArchitectRoute; a: ArchitectRead }) {
  const status = useStatus();
  // Its latest work: the one going on, else the one that ended last (its works are newest first).
  const latest = (a.works.find((x) => !overWork(x.work)) ?? a.works[0])?.work ?? null;
  const rounds = useRoundsWindow(a.project, route.round);
  const version = latest?.flow_version ?? status?.flow_version ?? null;
  const compiled = useResource<Compiled>(null, version !== null ? `/api/v1/flows/${version}` : null);
  const view = useResource<FlowView>(null, version !== null ? `/api/v1/flows/${version}/view` : null);
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
        <div className="screen">
          <Loaded r={compiled}>
            {(c) => (
              <Loaded r={view}>
                {(v) => <Chart route={route} a={a} latest={latest} rounds={rounds} compiled={c} view={v} go={go} />}
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
