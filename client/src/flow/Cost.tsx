// What installing the flow shown would do with each work that is neither ended nor done, as the engine judges it and
// `wf-agents flow cost` tells it for the same file: the first section of a comparison's panel, for a flow read at a git
// ref. The works are in runs by what happens to them, in the order the engine gives; those that cost something are
// open, those that carry on as they are are folded with their count.

import { Flash } from '@primer/react';

import type { CostEffect, FlowCost, PausedBy, WorkCost } from '../api';
import { roundName } from '../format';
import type { Resource } from '../live';
import { Fold, ItemRef } from '../parts';
import { Link, workHref } from '../route';
import { type Key, t, tn } from '../text';
import { SEP } from '../voices';
import { inline } from './words';

type Kind = CostEffect['kind'];

/// The title of a cell of a graph of the flow shown, when it has one.
export type CellTitle = (graph: string, cell: string) => string | undefined;

/// What a run of works is headed by: how many there are and what happens to them.
const HEADING: Record<Kind, (n: number) => string> = {
  unreadable: (n) => tn('cost_unreadable', n),
  goes_back: (n) => tn('cost_goes_back', n),
  stuck: (n) => tn('cost_stuck', n),
  ends: (n) => tn('cost_ends', n),
  moves: (n) => tn('cost_moves', n),
  stays: (n) => tn('cost_stays', n),
};

/// The works that go on as they are: folded, since there is nothing for the owner to read in them.
const CARRIES_ON: ReadonlySet<Kind> = new Set<Kind>(['moves', 'stays']);

const PAUSED: Record<PausedBy, Key> = {
  owner: 'cost_paused_owner',
  engine: 'cost_paused_engine',
  architect: 'cost_paused_architect',
};

export type CostRun = { kind: Kind; works: WorkCost[] };

/// The works in runs of one kind, in the order given: the engine lists them by what happens to them, then by id.
export function costRuns(works: WorkCost[]): CostRun[] {
  const runs: CostRun[] = [];
  for (const w of works) {
    const last = runs[runs.length - 1];
    if (last !== undefined && last.kind === w.effect.kind) last.works.push(w);
    else runs.push({ kind: w.effect.kind, works: [w] });
  }
  return runs;
}

/// A cell as a sentence names it: its code, which `inline` sets in monospace, and its title.
function cellWords(titleOf: CellTitle, graph: string, id: string): string {
  const title = titleOf(graph, id);
  return `\`${id}\`${title !== undefined && title !== '' ? ` ${title}` : ''}`;
}

/// What happens to a work, in words after what it is; nothing for a work that carries on as it is. What the engine
/// wrote (why, what the owner can do) is set after the sentence, never inside what `inline` splits at backticks.
function Effect({ w, titleOf }: { w: WorkCost; titleOf: CellTitle }) {
  const e = w.effect;
  const cell = (id: string) => cellWords(titleOf, w.stands?.graph ?? '', id);
  switch (e.kind) {
    case 'unreadable':
      return <>{e.why}</>;
    case 'goes_back': {
      const round = e.from === null ? null : roundName(w.work, e.from);
      const again =
        round === null
          ? t('cost_again_nothing')
          : e.redone === 1
            ? t('cost_again_single', { round })
            : t('cost_again_several', { round, rest: e.redone - 1 });
      return (
        <>
          {inline(t('cost_back', { through: cell(e.through), to: cell(e.to), again }))}: {e.why}
        </>
      );
    }
    case 'stuck':
      return (
        <>
          {inline(t('cost_stuck_at', { at: cell(e.at) }))}: {e.pause}
        </>
      );
    case 'ends':
      return (
        <>
          {e.why}
          {e.taken_over ? ` ${t('cost_taken_over')}` : null}
        </>
      );
    case 'moves':
    case 'stays':
      return null;
  }
}

/// One work: its name, what it is, where it stands, and what installing the flow does with it.
function WorkRow({ w, titleOf }: { w: WorkCost; titleOf: CellTitle }) {
  const s = w.stands;
  const what =
    s === null ? null : s.item !== null ? (
      <ItemRef id={s.item} title={s.title} />
    ) : (
      <span>{t('cost_graph_work', { graph: s.graph, project: s.project ?? '' })}</span>
    );
  return (
    <li className="chg">
      <div className="chg-head">
        <Link to={workHref(w.work)}>{t('work_name', { id: w.work })}</Link>
        {what}
        {s !== null ? (
          <span className="muted">
            {inline(t('cost_work_on', { version: s.version, at: cellWords(titleOf, s.graph, s.at) }))}
            {s.paused !== null ? `${SEP}${t(PAUSED[s.paused])}` : null}
          </span>
        ) : null}
      </div>
      {CARRIES_ON.has(w.effect.kind) ? null : (
        <div className="cost-effect">
          <Effect w={w} titleOf={titleOf} />
        </div>
      )}
    </li>
  );
}

/// The works of a cost, a fold for each run of one kind.
function Runs({ cost, titleOf }: { cost: FlowCost; titleOf: CellTitle }) {
  const runs = costRuns(cost.works);
  return (
    <>
      <p className="muted">{t(cost.stored ? 'cost_version_stored' : 'cost_version_new', { version: cost.version })}</p>
      {runs.length === 0 ? <p className="muted">{t('cost_none')}</p> : null}
      {runs.map((run) => (
        <Fold key={run.kind} summary={HEADING[run.kind](run.works.length)} open={!CARRIES_ON.has(run.kind)}>
          <ul className="chg-list">
            {run.works.map((w) => (
              <WorkRow key={w.work} w={w} titleOf={titleOf} />
            ))}
          </ul>
        </Fold>
      ))}
    </>
  );
}

/// The section: what installing the flow shown does with the works going on, or why that cannot be judged here: a flow
/// that does not compile against this program's database definition, or does not fit the configuration, is refused.
export function CostSection({ cost, titleOf }: { cost: Resource<FlowCost>; titleOf: CellTitle }) {
  return (
    <section className="diff-part">
      <h3 className="sub-heading">{t('cost_title')}</h3>
      {cost.data !== undefined ? (
        <Runs cost={cost.data} titleOf={titleOf} />
      ) : cost.error !== null ? (
        <>
          <Flash variant="warning">{t('cost_cannot', { error: cost.error })}</Flash>
          <p className="muted">{inline(t('cost_cannot_hint'))}</p>
        </>
      ) : (
        <p className="muted">{t('loading')}</p>
      )}
    </section>
  );
}
