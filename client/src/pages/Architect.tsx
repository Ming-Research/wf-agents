// A project's architect's page, laid out as a work's page, read only: the owner speaks to the architect, answers it
// and starts, pauses and steps it in its one-to-one in the messages view, which the page's line leads to. Its line
// says, with more than one project, whose architect it is, with the switch to another's; what the architect does now
// in words (the matter it handles and at which cell, or that it is idle, paused or held back), with no word of its
// wakes, which to the owner are only how it is run; what stops the project's new works from starting; and the switch
// between the stretches it worked in (when each began, what it was for, how many matters it handled, what it cost),
// whose rounds the chart shows. Under the line, its graph fills the window with the stretch shown placed on it as a
// work is, and the column docked at its right holds the architect's panel, where what is open on the chart takes the
// place of its list, with a way back to it. The panel's tabs, on one line: its rounds; the item changes its batches
// made; the project's observations, each leading to the conversation it is closed in; what to do next; and the policy
// in force, as one document. Each part names the cell that wrote it, and every tab draws its blocks, sections, items
// and rounds with the page's shared parts.

import { BlockedIcon, SearchIcon, TriangleDownIcon } from '@primer/octicons-react';
import {
  Button,
  Checkbox,
  CounterLabel,
  FormControl,
  Heading,
  Label,
  Select,
  TextInput,
  UnderlineNav,
} from '@primer/react';
import { useEffect, useMemo, useRef, useState } from 'react';

import type {
  Architect as ArchitectRead,
  Compiled,
  CompiledGraph,
  FlowView,
  ItemChange,
  Observation,
  RoundSummary,
  Wake,
  WorkDetail,
  WorkSummary,
} from '../api';
import { architectState, holdWords, matterKind, matterSentence, observationsShown, overWake, shownWake } from '../architect';
import { course, heldName, Loaded } from '../components';
import { runningWords } from '../course';
import { FlowScreen } from '../flow/FlowScreen';
import { cost, duration, time } from '../format';
import { Labelled, words } from '../labelled';
import { Markdown } from '../markdown';
import { useResource, useStatus } from '../live';
import { type ArchitectRoute, type ArchitectTab, architectHref, forProject, href, Link, navigate } from '../route';
import { t, type Key } from '../text';
import { Block, By, CellTitles, ItemRef, type OpenRound, SubHeading } from '../parts';
import { ChangeBlock, NextView, PolicyText } from './ArchitectViews';
import { bringIntoView } from './Decide';
import { RoundView } from './RoundView';
import { Timeline, useHalo, workMarks } from './Work';

/// The graph the architect's wakes run, as the flow names it.
const GRAPH = 'architect';

/// What the bare graph marks: nothing.
const BARE = { overlays: null, taken: null, added: new Set<string>(), changed: new Set<string>() };

/// No rounds, for the halo while no wake is shown.
const NO_ROUNDS: RoundSummary[] = [];

/// The time now, ticking every second while `ticking`.
function useNow(ticking: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!ticking) return;
    const tick = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(tick);
  }, [ticking]);
  return now;
}

/// What the architect does now, on its line, in words, with no word of its wakes: the matter it handles and the cell
/// it is at, a round running and for how long, or what holds it back; else idle, or held back with matters waiting.
function Now({ a, current }: { a: ArchitectRead; current: WorkSummary | null }) {
  const status = useStatus();
  const state = architectState(a.wakes, a.queue);
  const now = useNow(current?.running !== null && current?.running !== undefined);
  if (state.kind === 'awake' && current !== null) {
    const c = course(current, status);
    const at = `${current.at} ${current.at_title ?? ''}`;
    const what = state.handling !== null ? matterSentence(state.handling) : null;
    return (
      <>
        {c.running !== null ? (
          <>
            <Label variant="accent">{c.running === 'step' ? runningWords(c) : t('talk_architect_working')}</Label>
            {current.running ? <span className="muted">{duration(Math.max(0, now - current.running.started_at))}</span> : null}
          </>
        ) : c.held !== null ? (
          <Label variant={heldName(c).problem ? 'danger' : 'attention'}>{heldName(c).name}</Label>
        ) : (
          <Label variant="secondary">{t('talk_architect_between')}</Label>
        )}
        <span className="work-where" title={what ?? at}>
          {what !== null ? t('talk_architect_handles', { what, at }) : t('talk_architect_at', { at })}
        </span>
      </>
    );
  }
  const started = status !== null && status.architects_started.includes(a.project);
  const held = state.waiting > 0 && status !== null && status.paused && !started;
  return (
    <Label variant={held ? 'attention' : 'secondary'}>
      {held ? t('talk_architect_held') : state.waiting > 0 ? t('talk_architect_takes_up') : t('talk_architect_idle')}
    </Label>
  );
}

/// The line above an architect's chart: with more than one project, whose architect it is and the switch to another's;
/// what it does now, what stops the project's new works, its one-to-one in the messages view, where the owner acts on
/// it, and the switch between the stretches it worked in.
function Line({ a, shown, current, pop, setPop }: {
  a: ArchitectRead;
  shown: Wake | null;
  /// The stretch going on, as freshly read as the page has it.
  current: WorkSummary | null;
  pop: 'wakes' | null;
  setPop: (p: 'wakes' | null) => void;
}) {
  const state = architectState(a.wakes, a.queue);
  return (
    <div className="work-line arch-line">
      {a.projects.length > 1 ? (
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
      ) : null}
      <span className="work-state course">
        <Now a={a} current={state.kind === 'awake' ? current : null} />
      </span>
      {state.waiting > 0 ? <span className="muted">{t('talk_queue_left', { n: state.waiting })}</span> : null}
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
      <Link to={href({ page: 'messages', open: `architect:${a.project}` })} title={t('go_to_conversation_hint')}>
        {t('go_to_conversation')}
      </Link>
      {shown !== null ? (
        <Button
          size="small"
          className="arch-wake-switch"
          trailingAction={TriangleDownIcon}
          aria-expanded={pop === 'wakes'}
          title={t('arch_wakes_hint')}
          onClick={() => setPop(pop === 'wakes' ? null : 'wakes')}
        >
          {t('arch_wake_shown', { at: time(shown.work.started_at) })}
        </Button>
      ) : null}
    </div>
  );
}

/// The name of each tab of the architect's panel.
const TAB_NAMES: Record<ArchitectTab, Key> = {
  changes: 'arch_tab_changes',
  observations: 'arch_tab_observations',
  policy: 'arch_tab_policy',
  next: 'arch_tab_next',
  rounds: 'rounds',
};

/// The stretches the architect worked in, newest first: when each began, what it was for, how many matters it
/// handled, its rounds and cost; a click shows that stretch's rounds on the chart, and the one shown by default follows
/// the one going on again.
function WakeList({ a, shown, tab, project, onClose }: {
  a: ArchitectRead;
  shown: number | null;
  tab: ArchitectTab;
  /// The project the address names, kept as it is.
  project: string | null;
  onClose: () => void;
}) {
  const followed = shownWake(a.wakes, null)?.work.id ?? null;
  return (
    <div className="arch-wakes" role="dialog" aria-labelledby="arch-wakes-title">
      <Heading as="h2" variant="small" id="arch-wakes-title">
        {t('arch_wakes', { n: a.wakes.length })}
      </Heading>
      <div className="arch-wakes-list">
        <table className="list">
          <thead>
            <tr>
              <th>{t('arch_col_woke')}</th>
              <th>{t('arch_col_why')}</th>
              <th className="num">{t('arch_col_handled')}</th>
              <th className="num">{t('col_rounds')}</th>
              <th className="num">{t('col_cost')}</th>
            </tr>
          </thead>
          <tbody>
            {a.wakes.map((x) => {
              const why = [...new Set(x.woke_for.map((m) => matterKind(m.kind, null)))].join(', ');
              const whyAll = x.woke_for.map((m) => `${matterKind(m.kind, m.detail)}: ${m.what}`);
              return (
                <tr
                  key={x.work.id}
                  className={`clickable${x.work.id === shown ? ' selected' : ''}`}
                  onClick={() => {
                    navigate(architectHref({ project, wake: x.work.id === followed ? null : x.work.id, tab }));
                    onClose();
                  }}
                >
                  <td className="nowrap">
                    {time(x.work.started_at)}{' '}
                    {!overWake(x.work) ? <Label variant="accent">{t('arch_wake_going')}</Label> : null}
                  </td>
                  <td className="clip" title={whyAll.join('\n')}>
                    {why !== '' ? why : '–'}
                  </td>
                  <td className="num">{x.handled}</td>
                  <td className="num">{x.work.rounds}</td>
                  <td className="num">{cost(x.work.cost_usd)}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      <div className="arch-wakes-actions">
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

/// One observation, as a block, read only: what it is; its tags; how many times it was reported; closed, why, and open,
/// the conversation where the owner closes it, wf-agents' own for wf-agents' and the architect's one-to-one for any
/// other project's; then where it came from, what it refers to, the item that took it in and when.
function ObservationRow({ o, conversation }: { o: Observation; conversation: string }) {
  const head = (
    <>
      <strong>{t('observation_head', { id: o.id })}</strong>
      {o.tags.map((x) => (
        <Label key={x} variant="secondary">
          {x}
        </Label>
      ))}
      <span className="muted">{t('observation_reported', { n: o.times_reported })}</span>
      {o.closed !== null ? (
        <Label variant="secondary">{t('observation_closed', { why: words(o.closed) })}</Label>
      ) : (
        <Link to={`${href({ page: 'messages', open: conversation })}#observation-${o.id}`} muted>
          {t('observation_close_there')}
        </Link>
      )}
    </>
  );
  return (
    <Block head={head}>
      <Markdown text={o.what} />
      <div className="block-meta">
        <span>{t('observation_source', { source: o.source })}</span>
        {o.refs.length > 0 ? <span>{t('observation_refs', { refs: o.refs.join(', ') })}</span> : null}
        {o.folded_into !== null ? (
          <span>
            {t('observation_folded')} <ItemRef id={o.folded_into} />
          </span>
        ) : null}
        {o.created !== null ? <span>{t('observation_at', { at: time(o.created) })}</span> : null}
      </div>
      {o.closed_why !== null ? <p className="muted">{o.closed_why}</p> : null}
    </Block>
  );
}

/// A project's observations: by tag, with what each tag covers, searchable, the open ones and, when asked for, those
/// closed.
function Observations({ a }: { a: ArchitectRead }) {
  const conversation = a.own ? 'wf-agents' : `architect:${a.project}`;
  const [tag, setTag] = useState('');
  const [search, setSearch] = useState('');
  const [closed, setClosed] = useState(false);
  const shown = observationsShown(a.observations, tag, search, closed);
  const covers = a.tags.find((x) => x.name === tag)?.desc ?? '';
  return (
    <>
      <div className="arch-filters">
        <Select
          size="small"
          value={tag}
          onChange={(e) => setTag(e.target.value)}
          title={covers}
          aria-label={t('observation_tag')}
        >
          <Select.Option value="">{t('observation_every_tag')}</Select.Option>
          {a.tags.map((x) => (
            <Select.Option key={x.name} value={x.name}>
              {x.name}
            </Select.Option>
          ))}
        </Select>
        <TextInput
          size="small"
          type="search"
          leadingVisual={SearchIcon}
          value={search}
          placeholder={t('observation_search')}
          aria-label={t('observation_search')}
          onChange={(e) => setSearch(e.target.value)}
        />
        <FormControl layout="horizontal">
          <Checkbox checked={closed} onChange={(e) => setClosed(e.target.checked)} />
          <FormControl.Label>{t('observation_show_closed')}</FormControl.Label>
        </FormControl>
      </div>
      {covers !== '' ? <p className="muted">{covers}</p> : null}
      {shown.length === 0 ? (
        <p className="muted">{t('observation_none')}</p>
      ) : (
        shown.map((o) => <ObservationRow key={o.id} o={o} conversation={conversation} />)
      )}
    </>
  );
}

/// The policy in force, as one document, with when it took effect and the round that wrote it; or that none has taken
/// effect yet. Setting a new one, its study and drafts, and the owner's readings of them are in the one-to-one with the
/// architect, where the owner answers them; the owner reads the policy here to recall it.
function Policy({ a, onRound }: { a: ArchitectRead; onRound: OpenRound }) {
  const p = a.policy;
  if (p === null) return <p className="muted">{t('policy_none')}</p>;
  return (
    <div className="policy-tab">
      <p className="muted">
        <By cell={p.cell} work={p.work} round={p.round} onRound={onRound}>
          {t('policy_took_effect', { at: time(p.at) })}
        </By>
      </p>
      <PolicyText value={p.policy} />
    </div>
  );
}

/// The architect's panel, on top of the column docked at the right of its chart, in tabs, each saying how many of its
/// things there are and, for the asks, how many wait for the owner.
function Panel({ route, a, d, g, go, onRound }: {
  route: ArchitectRoute;
  a: ArchitectRead;
  /// The wake shown, in full; none before the first wake.
  d: WorkDetail | null;
  g: CompiledGraph;
  go: (to: Partial<ArchitectRoute>) => void;
  onRound: OpenRound;
}) {
  const status = useStatus();
  // What to bring into view once its tab is shown: the ask the owner was sent to; else a tab shows from its top.
  const want = useRef<string | null>(null);
  const body = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (want.current === null) body.current?.scrollTo({ top: 0 });
  }, [route.tab]);
  useEffect(() => {
    if (want.current !== null && bringIntoView(want.current)) want.current = null;
  });
  // A matter of the plan is in the architect's one-to-one, as a card.
  const onMatter = (id: number) => navigate(`${href({ page: 'messages', open: `architect:${a.project}` })}#matter-${id}`);
  // Each tab's count.
  const tabs: Array<{ tab: ArchitectTab; label: string; n: number | null; waiting: number; hint: string }> = [
    { tab: 'rounds', label: t('rounds'), n: d?.rounds.length ?? null, waiting: 0, hint: '' },
    { tab: 'changes', label: t('arch_tab_changes'), n: a.changes.length, waiting: 0, hint: '' },
    {
      tab: 'observations',
      label: t('arch_tab_observations'),
      n: a.observations.filter((o) => o.closed === null).length,
      waiting: 0,
      hint: '',
    },
    { tab: 'next', label: t('arch_tab_next'), n: null, waiting: 0, hint: '' },
    { tab: 'policy', label: t('arch_tab_policy'), n: null, waiting: 0, hint: '' },
  ];
  return (
    <div className="drawer">
      <UnderlineNav aria-label={t('arch_tabs')} className="drawer-tabs">
        {tabs.map((x) => (
          <UnderlineNav.Item
            key={x.tab}
            href={href({ ...route, tab: x.tab })}
            aria-current={x.tab === route.tab ? 'page' : undefined}
            onSelect={(e) => {
              // A click with a modifier key opens the tab as the browser would.
              if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
              e.preventDefault();
              want.current = null;
              go({ tab: x.tab });
            }}
          >
            {x.label}
            {x.n !== null ? <CounterLabel className="drawer-count">{x.n}</CounterLabel> : null}
            {x.waiting > 0 ? (
              <CounterLabel variant="primary" className="drawer-count" title={x.hint}>
                {x.waiting}
              </CounterLabel>
            ) : null}
          </UnderlineNav.Item>
        ))}
      </UnderlineNav>
      <div className="drawer-body" ref={body}>
        {route.tab === 'changes' ? <Changes changes={a.changes} onRound={onRound} /> : null}
        {route.tab === 'observations' ? <Observations a={a} /> : null}
        {route.tab === 'policy' ? <Policy a={a} onRound={onRound} /> : null}
        {route.tab === 'next' ? <NextView next={a.next} onRound={onRound} onMatter={onMatter} /> : null}
        {route.tab === 'rounds' ? (
          d === null ? (
            <p className="muted">{a.wakes.length === 0 ? t('arch_never_woke') : t('loading')}</p>
          ) : (
            <Timeline
              work={d.work.id}
              rounds={d.rounds}
              g={g}
              stale={status !== null && status.running !== d.work.id}
              selected={route.wake === d.work.id ? route.round : null}
              onRound={(x) => go({ wake: d.work.id, round: x.n, cell: null })}
            />
          )
        ) : null}
      </div>
    </div>
  );
}

/// The architect's graph filling the window, with the wake shown placed on it, and the docked column: the panel in its
/// upper half, the conversation in its lower half. A circle to or from another graph goes to that cell on the flow
/// page, which draws every graph.
function Chart({ route, a, wake, d, compiled, version, view, go }: {
  route: ArchitectRoute;
  a: ArchitectRead;
  wake: Wake | null;
  d: WorkDetail | null;
  compiled: Compiled;
  /// The stored flow version the chart draws.
  version: number | null;
  view: FlowView;
  go: (to: Partial<ArchitectRoute>) => void;
}) {
  const status = useStatus();
  // The chart draws the graph of the work shown: a wake's, or a policy task's, whose cells (P2 to P12) the architect
  // graph no longer holds, so that a round chosen of either is found on it.
  const name = d?.work.graph ?? GRAPH;
  const g = compiled.graphs.find((x) => x.name === name);
  const v = view.graphs.find((x) => x.name === name);
  const graphs = useMemo(() => (g && v ? [{ g, view: v }] : []), [g, v]);
  const c = d !== null ? course(d.work, status) : null;
  const halo = useHalo(d?.work ?? null, d?.rounds ?? NO_ROUNDS, c);
  if (!g || !v) return <p className="muted page">{t('arch_no_graph')}</p>;
  const id = wake?.work.id ?? null;
  const roundCell = route.round !== null && d !== null ? (d.rounds.find((x) => x.n === route.round)?.cell ?? null) : null;
  // A round of any wake opens on the chart of that wake.
  const onRound: OpenRound = (work, round) => go({ wake: work, round, cell: null });
  return (
    <FlowScreen
      key={id ?? 'none'}
      compiled={compiled}
      graphs={graphs}
      legend={d !== null ? 'work' : 'flow'}
      marks={[
        d !== null && c !== null
          ? workMarks(g, v, d.work, d.rounds, c, status, halo, route.wake === id ? route.round : null, compiled)
          : BARE,
      ]}
      cell={route.cell}
      onCell={(cell) => go({ cell, round: null })}
      start={roundCell ?? d?.work.at ?? null}
      work={id ?? undefined}
      rounds={d?.rounds}
      round={route.wake === id ? route.round : null}
      onRound={(x) => go(x !== null ? { wake: id, round: x, cell: null } : { round: null })}
      renderRound={id !== null && d !== null ? (x) => <RoundView work={id} n={x} rounds={d.rounds} /> : undefined}
      list={t(TAB_NAMES[route.tab])}
      side={(open) => open ?? <Panel route={route} a={a} d={d} g={g} go={go} onRound={onRound} />}
      onGraph={(graph, cell) =>
        navigate(href({ page: 'flow', version, project: null, ref: null, from: null, graph, cell }))
      }
    />
  );
}

function Body({ route, a }: { route: ArchitectRoute; a: ArchitectRead }) {
  const status = useStatus();
  const wake = shownWake(a.wakes, route.wake);
  const id = wake?.work.id ?? null;
  const detail = useResource<WorkDetail>(id !== null ? `work:${id}` : null, id !== null ? `/api/v1/works/${id}` : null);
  const version = wake?.work.flow_version ?? status?.flow_version ?? null;
  const compiled = useResource<Compiled>(null, version !== null ? `/api/v1/flows/${version}` : null);
  const view = useResource<FlowView>(null, version !== null ? `/api/v1/flows/${version}/view` : null);
  const d = detail.data !== undefined && detail.data.work.id === id ? detail.data : null;
  const going = a.wakes.find((w) => !overWake(w.work)) ?? null;
  const current = going !== null && d !== null && d.work.id === going.work.id ? d.work : (going?.work ?? null);
  const [pop, setPop] = useState<'wakes' | null>(null);
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
        <Line a={a} shown={wake} current={current} pop={pop} setPop={setPop} />
        {pop === 'wakes' ? (
          <WakeList a={a} shown={id} tab={route.tab} project={route.project} onClose={() => setPop(null)} />
        ) : null}
        <div className="screen">
          <Loaded r={compiled}>
            {(c) => (
              <Loaded r={view}>
                {(v) => (
                  <Chart route={route} a={a} wake={wake} d={d} compiled={c} version={version} view={v} go={go} />
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
