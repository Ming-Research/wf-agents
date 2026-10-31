// A work's page, under its line above the chart, which holds its facts and how it goes on: the state machine of the
// flow version it is pinned to, filling the window under the panel floating over its right side, with what the work
// did on it: the cell it is at ringed, each cell it visited with its rounds and the rounds sent back or failed, the
// arrows it took standing out, the cell a round runs or has just run at in a moving halo, and beside the cell it waits
// at why, in a few words. The page only reads the work: the owner acts on it in its group in the messages view, which
// its line and what it waits for lead to. The floating window holds on top the work's panel in tabs: its rounds; its
// design cards, the one waiting for the owner's ruling first; the talk of its helpers (the advisor, the review, the
// adversarial testing) with its engineer, one thread for each helper, which is in no group; and its usage. A round or a
// cell opened shows in the window's lower part, its cell brought into view. The chart, the panel, the rounds tab, the line and the usage are
// the architect's page's too (WorkChart.tsx, HeadLine.tsx, Usage.tsx).

import type { Compiled, CompiledGraph, DesignCard, FlowView, RoundSummary, WorkDetail } from '../api';
import { Loaded } from '../components';
import { useResource } from '../live';
import { RoundAt, RoundLink, Section } from '../parts';
import { href, navigate, type WorkRoute, type WorkTab } from '../route';
import { t } from '../text';
import { time } from '../format';
import { Bubble } from './Bubble';
import { currentCard, DesignCardView, words } from './Decide';
import { RoundTold } from './RoundView';
import { Usage } from './Usage';
import { ChartPanel, RoundsTab, WorkChart } from './WorkChart';

/// The work's design cards, in the card every card shares: the one waiting for the owner's ruling first, then the
/// others, newest first; the current one (the one waiting, else the newest not replaced) open, every other folded to
/// its one line.
function Cards({ cards, onRound }: { cards: DesignCard[]; onRound: (r: number) => void }) {
  if (cards.length === 0) return <p className="muted">{t('work_no_design_cards')}</p>;
  const current = currentCard(cards);
  const open = cards.filter((c) => c.status === 'open');
  const rest = cards.filter((c) => c.status !== 'open').reverse();
  return (
    <>
      <p className="dc-lead">{t('dc_lead')}</p>
      {[...open, ...rest].map((c) => (
        <DesignCardView
          key={c.id}
          c={c}
          round={<RoundLink work={c.work} round={c.round} onRound={(_, r) => onRound(r)} />}
          folded={c.id !== current}
        />
      ))}
    </>
  );
}

/// The helpers of a work, each the line that runs it: its talk with the engineer is a thread of its own.
export const HELPERS = ['advisor', 'review', 'adversarial'] as const;

/// A helper's thread with the side across from it, from the work's rounds in order, each round once and in order:
/// each run of the helper's rounds (those with no round of the engineer's between them); before it, the round that
/// handed it what it works on: the round that asked it, where the row it took leads to the helper's cell, else the
/// engineer's latest round that handed in its own return with a commit (`head`), the one reviewed or tested, else the
/// engineer's latest round; and after it, what that side did with what it said: the last round taken at the first cell
/// of that side's after the run, up to another agent's cell.
export function helperThread(rounds: RoundSummary[], line: string, g: CompiledGraph): RoundSummary[] {
  const cells = new Map(g.cells.map((c) => [c.id, c]));
  const agent = (r: RoundSummary) => r.line !== null && r.not_asked === null;
  const rowOf = (r: RoundSummary) =>
    r.cell !== null && r.transition !== null ? cells.get(r.cell)?.rows.find((x) => x.name === r.transition) : undefined;
  // A round that handed in its cell's own return, and that return holds the commit it made.
  const handsHead = (r: RoundSummary) => {
    const row = rowOf(r);
    const c = r.cell !== null ? cells.get(r.cell) : undefined;
    if (row === undefined || row.common || row.fallback === true || c === undefined) return false;
    const own = (c.forms ?? []).findIndex((f) => f.own === true);
    const schema = c.schema;
    const within =
      schema !== null && schema !== undefined && typeof schema === 'object' && !Array.isArray(schema) ? schema.properties : undefined;
    const form = within !== null && typeof within === 'object' && !Array.isArray(within) ? within[g.within] : undefined;
    const alts = form !== null && typeof form === 'object' && !Array.isArray(form) ? form.anyOf : undefined;
    const ownForm = Array.isArray(alts) && own >= 0 ? alts[own] : form;
    const props = ownForm !== null && typeof ownForm === 'object' && !Array.isArray(ownForm) ? ownForm.properties : undefined;
    return props !== null && typeof props === 'object' && !Array.isArray(props) && 'head' in props;
  };
  const picked = new Map<number, RoundSummary>();
  let i = 0;
  while (i < rounds.length) {
    if (rounds[i].line !== line) {
      i += 1;
      continue;
    }
    const first = rounds[i];
    let last = i;
    for (let k = i; k < rounds.length && rounds[k].line !== 'engineer'; k += 1) {
      if (rounds[k].line !== line) continue;
      // A return sent back, or a run that failed, said nothing to the other side.
      if (rounds[k].outcome === 'took' || rounds[k].outcome === 'running') picked.set(rounds[k].n, rounds[k]);
      last = k;
    }
    const before = rounds.slice(0, i).reverse();
    const asker = before.find((r) => agent(r) && r.outcome === 'took');
    const handed =
      asker !== undefined && asker.line !== line && rowOf(asker)?.to === first.cell
        ? asker
        : (before.find((r) => r.line === 'engineer' && r.outcome === 'took' && handsHead(r)) ??
          before.find((r) => r.line === 'engineer' && r.outcome === 'took'));
    if (handed !== undefined) picked.set(handed.n, handed);
    const side = handed?.line ?? 'engineer';
    const after = rounds.slice(last + 1);
    const start = after.findIndex((r) => r.line === side && agent(r));
    if (start >= 0) {
      const at = after[start].cell;
      let took: RoundSummary | undefined;
      for (const r of after.slice(start)) {
        if (agent(r) && r.cell !== at) break;
        if (r.line === side && r.outcome === 'took') took = r;
      }
      if (took !== undefined) picked.set(took.n, took);
    }
    i = last + 1;
  }
  return [...picked.values()].sort((a, b) => a.n - b.n);
}

/// One message of a helper's thread in its side's bubble, the helper's on the right: what the round said it did, its
/// whole return folded under it; its head says who, the round at its cell, opening it on the chart, and when.
function Said({ work, r, line, g, onRound }: {
  work: number;
  r: RoundSummary;
  line: string;
  g: CompiledGraph;
  onRound: (round: number) => void;
}) {
  const cell = r.cell !== null ? g.cells.find((c) => c.id === r.cell) : undefined;
  const who = r.line ?? 'engineer';
  return (
    <Bubble
      who={who}
      right={who === line}
      head={
        <>
          <span className="talk-who">{words(who)}</span>
          <RoundAt round={<RoundLink work={work} round={r.n} onRound={(_, n) => onRound(n)} />} cell={r.cell} title={cell?.title} />
          <span className="talk-when">{time(r.ended_at ?? r.started_at)}</span>
        </>
      }
    >
      <RoundTold work={work} r={r} cell={cell} within={g.within} />
    </Bubble>
  );
}

/// What a work's helpers and its engineer said to each other, read only, one thread for each helper and never mixed,
/// as a chat: what was handed to the helper, word for word (the question, or the commit and what to review or test),
/// its answer word for word, and what the engineer did with it next.
function Helpers({ d, g, onRound }: { d: WorkDetail; g: CompiledGraph; onRound: (round: number) => void }) {
  const threads = HELPERS.map((line) => ({ line, rounds: helperThread(d.rounds, line, g) })).filter(
    (x) => x.rounds.some((r) => r.line === x.line),
  );
  if (threads.length === 0) return <p className="muted">{t('helpers_none')}</p>;
  return (
    <>
      {threads.map((x) => (
        <Section key={x.line} title={t('helpers_thread', { line: words(x.line) })} n={x.rounds.filter((r) => r.line === x.line).length} open>
          <div className="talk-messages">
            {x.rounds.map((r) => (
              <Said key={r.n} work={d.work.id} r={r} line={x.line} g={g} onRound={onRound} />
            ))}
          </div>
        </Section>
      ))}
    </>
  );
}

/// The work's panel, on top of the window floating over the right of its chart: its rounds, its design cards, its
/// helpers' talk with its engineer, and its usage.
function WorkPanel({ d, g, r, go }: { d: WorkDetail; g: CompiledGraph; r: WorkRoute; go: (to: Partial<WorkRoute>) => void }) {
  const onRound = (round: number) => go({ round, cell: null });
  const openCards = d.design_cards.filter((c) => c.status === 'open').length;
  const helped = d.rounds.filter((x) => x.line !== null && (HELPERS as readonly string[]).includes(x.line)).length;
  const tab = (x: WorkTab) => href({ ...r, tab: x });
  return (
    <ChartPanel
      label={t('work_tabs')}
      current={r.tab}
      onTab={(x) => go({ tab: x })}
      tabs={[
        { tab: 'rounds', label: t('rounds'), href: tab('rounds'), n: d.rounds.length },
        {
          tab: 'cards',
          label: t('design_cards'),
          href: tab('cards'),
          n: d.design_cards.length,
          waiting: openCards,
          hint: t('open_design_cards_n', { n: openCards }),
        },
        { tab: 'helpers', label: t('helpers_tab'), href: tab('helpers'), n: helped },
        { tab: 'usage', label: t('us_tab'), href: tab('usage'), n: null },
      ]}
    >
      {r.tab === 'rounds' ? (
        <RoundsTab d={d} g={g} only={r.only} selected={r.round} onAll={() => go({ only: null })} onRound={onRound} />
      ) : null}
      {r.tab === 'cards' ? <Cards cards={d.design_cards} onRound={onRound} /> : null}
      {r.tab === 'helpers' ? <Helpers d={d} g={g} onRound={onRound} /> : null}
      {r.tab === 'usage' ? <Usage works={[d]} cells={new Map(g.cells.map((c) => [c.id, { kind: c.kind, title: c.title }]))} /> : null}
    </ChartPanel>
  );
}

export function Body({ r, d, compiled, view }: { r: WorkRoute; d: WorkDetail; compiled: Compiled; view: FlowView }) {
  const go = (to: Partial<WorkRoute>) => navigate(href({ ...r, ...to }), true);
  return (
    <WorkChart
      compiled={compiled}
      view={view}
      graph={d.work.graph}
      d={d}
      cell={r.cell}
      round={r.round}
      onCell={(c) => go({ round: null, cell: c })}
      onRound={(x) => go({ round: x, cell: null })}
      panel={(g) => <WorkPanel d={d} g={g} r={r} go={go} />}
    />
  );
}

export function Work({ r }: { r: WorkRoute }) {
  const detail = useResource<WorkDetail>(`work:${r.id}`, `/api/v1/works/${r.id}`);
  const version = detail.data?.work.flow_version ?? null;
  const compiled = useResource<Compiled>(null, version !== null ? `/api/v1/flows/${version}` : null);
  const view = useResource<FlowView>(null, version !== null ? `/api/v1/flows/${version}/view` : null);
  return (
    <div className="screen">
      <Loaded r={detail}>
        {(d) => (
          <Loaded r={compiled}>
            {(c) => <Loaded r={view}>{(v) => <Body r={r} d={d} compiled={c} view={v} />}</Loaded>}
          </Loaded>
        )}
      </Loaded>
    </div>
  );
}
