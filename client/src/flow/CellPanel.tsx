// A cell, as its panel explains it on one page: under its title (and where it stands when the chart does not show it
// plainly), the four sections every cell's panel and every round's has (Sides.tsx): In, what it is given
// (CellGiven.tsx); Out, the pattern of what it hands back, as JSON (Returns.tsx); From, the rows that lead into it; and
// To, its own rows, each with what of its return it is worked out from and where it leads (Rows.tsx). An engine's
// step, a tool or the owner answers the same with what its step reads and what it comes to. On a work's chart its
// rounds there, and in a comparison what changed, are tabs beside it.

import { UnderlinePanels } from '@primer/react/experimental';
import { useEffect, useRef, useState } from 'react';

import type { CellDiff, CompiledCell, CompiledGraph, RoundSummary } from '../api';
import { OutcomeBadge } from '../components';
import { roundName, time } from '../format';
import { letterOf } from '../graph/flowModel';
import { LinkButton } from '../parts';
import type { Colours } from '../roles';
import { type Key, t, tn } from '../text';
import { Sheet } from '../voices';
import { AgentGiven, numbered, StepGiven } from './CellGiven';
import { Changes } from './Changes';
import { useFlow } from './context';
import { OwnerButtons, ReturnChecks, ReturnPattern } from './Returns';
import { FromTable, intoRows, ToTable } from './Rows';
import { type Side, Sides } from './Sides';

export { numbered, projectOwn } from './CellGiven';

/// Where a cell stands, as its title's parenthesis says it: its stage, as the box around it on the chart names it, or
/// the asking part; and an agent's cell's line, when the line has no colour of its own in `colours`, which the chart
/// draws its cells in. None for neither.
export function cellWhere(g: CompiledGraph, c: CompiledCell, colours: Colours): string | null {
  const parts: string[] = [];
  if (g.asking.includes(c.id)) {
    const letters = new Set(g.asking.map(letterOf));
    const letter = letters.size === 1 ? [...letters][0] : '';
    parts.push(t(letter !== '' ? 'chart_asking_part_letter' : 'chart_asking_part', { letter }));
  } else if (c.stage) {
    const letters = new Set(g.cells.filter((x) => x.stage === c.stage).map((x) => letterOf(x.id)));
    const letter = letters.size === 1 ? [...letters][0] : '';
    parts.push(t(letter !== '' ? 'chart_stage_letter' : 'chart_stage', { stage: c.stage, letter }));
  }
  if (c.kind === 'agent' && c.line != null && !colours.has(c.line)) parts.push(t('panel_line', { line: c.line }));
  return parts.length > 0 ? parts.join('; ') : null;
}

/// The tabs of a cell's panel: what changed, when two flows are compared; its rounds, on a work's chart; and the cell.
export type CellTab = 'changes' | 'rounds' | 'cell';

/// A cell's tabs, in order; only the cell's own page when there is nothing beside it.
export function cellTabs(has: { rounds: boolean; diff: boolean }): CellTab[] {
  const tabs: CellTab[] = [];
  if (has.diff) tabs.push('changes');
  if (has.rounds) tabs.push('rounds');
  tabs.push('cell');
  return tabs;
}

/// The tab a cell's panel shows: the one the owner chose last, when this cell has it, else its first, so that going
/// from cell to cell keeps to what the owner reads.
export function cellTab(tabs: CellTab[], chosen: CellTab | null): CellTab {
  return chosen !== null && tabs.includes(chosen) ? chosen : tabs[0];
}

const TAB_NAMES: Record<CellTab, Key> = { changes: 'sec_changes', rounds: 'tab_rounds', cell: 'tab_cell' };

/// The cell's own page: the four sections, In, Out, From and To.
function CellPage({ c, open, onOpen }: { c: CompiledCell; open: ReadonlySet<Side>; onOpen: (s: Side, open: boolean) => void }) {
  const f = useFlow();
  const agent = c.kind === 'agent';
  const [inputs, setInputs] = useState<Set<number>>(new Set());
  useEffect(() => {
    setInputs(new Set());
  }, [c.id]);
  const onOpened = (i: number, o: boolean) =>
    setInputs((s) => {
      const n = new Set(s);
      if (o) n.add(i);
      else n.delete(i);
      return n;
    });
  const view = f.cellView(c.id);
  const own = numbered(c).filter(({ no }) => no !== null);
  const rules = (view?.inputs ?? []).filter((m) => m.of.kind === 'rule').length;
  const into = intoRows(f, c);
  const sources = new Set(into.flatMap((x) => x.cells)).size;
  const forms = view?.returns.within != null ? view.returns.forms.length : 0;
  return (
    <div className="pp-page">
      <Sides
        open={open}
        onOpen={onOpen}
        parts={{
          in: {
            n: agent ? own.length : c.inputs.length,
            facts: agent ? t('given_facts', { inputs: tn('n_inputs', own.length), rules: tn('n_rules', rules) }) : t('given_step_facts'),
            body: agent ? <AgentGiven c={c} opened={inputs} onOpened={onOpened} /> : <StepGiven c={c} opened={inputs} onOpened={onOpened} />,
          },
          out: {
            facts: forms > 1 ? tn('out_forms', forms) : undefined,
            body: (
              <>
                <OwnerButtons c={c} />
                {agent ? (
                  <Sheet mono>
                    <ReturnPattern c={c} />
                  </Sheet>
                ) : (
                  <ReturnPattern c={c} />
                )}
                <ReturnChecks c={c} />
              </>
            ),
          },
          from: { n: into.length, facts: sources > 0 ? tn('n_cells', sources) : undefined, body: <FromTable c={c} /> },
          to: { n: c.rows.length, body: <ToTable c={c} /> },
        }}
      />
    </div>
  );
}

/// The work's rounds at the cell, newest first; a badge only for what is not the usual course, as in the work's list.
function RoundsHere({ work, rounds, onRound }: { work: number; rounds: RoundSummary[]; onRound: (r: RoundSummary) => void }) {
  if (rounds.length === 0) return <p className="pp-note">{t('no_rounds')}</p>;
  return (
    <div className="pp-rounds">
      {[...rounds].reverse().map((r) => (
        <div key={r.n} className="pp-round" role="button" tabIndex={0} onClick={() => onRound(r)} onKeyDown={(e) => (e.key === 'Enter' ? onRound(r) : null)}>
          <span>
            <LinkButton onClick={() => onRound(r)}>{roundName(work, r.n)}</LinkButton>
          </span>
          <span>
            {r.outcome !== 'took' ? (
              <>
                <OutcomeBadge outcome={r.outcome} />{' '}
              </>
            ) : null}
            {r.transition ?? r.why ?? ''}
          </span>
          <span className="pp-facts">{time(r.started_at)}</span>
        </div>
      ))}
    </div>
  );
}

export function CellPanel(props: {
  c: CompiledCell;
  /// On a work's page, the work and its rounds at this cell.
  work?: number;
  rounds?: RoundSummary[];
  onRound?: (r: RoundSummary) => void;
  /// When two flows are compared, what changed in this cell.
  diff?: CellDiff | null;
  /// The tab the owner chose last, kept from cell to cell, and how a choice is kept.
  tab: CellTab | null;
  onTab: (tab: CellTab) => void;
  /// The sections the owner opened, kept from cell to cell.
  open: ReadonlySet<Side>;
  onOpen: (s: Side, open: boolean) => void;
}) {
  const { c, work, rounds, onRound, diff, tab: chosen, onTab, open, onOpen } = props;
  const tabs = cellTabs({
    rounds: work !== undefined && rounds !== undefined && onRound !== undefined,
    diff: diff != null,
  });
  const tab = cellTab(tabs, chosen);
  const bar = useRef<HTMLDivElement>(null);
  // A tab shows from its top.
  useEffect(() => {
    bar.current?.parentElement?.scrollTo({ top: 0 });
  }, [c.id, tab]);
  const page = <CellPage c={c} open={open} onOpen={onOpen} />;
  if (tabs.length === 1) return page;
  return (
    <div className="cell-panel" ref={bar}>
      <UnderlinePanels aria-label={t('cell_tabs')} className="cell-tabs" value={tab} onChange={({ value }) => onTab(value as CellTab)}>
        {tabs.map((x) => (
          <UnderlinePanels.Tab key={x} value={x} counter={x === 'rounds' ? (rounds?.length ?? 0) : undefined}>
            {t(TAB_NAMES[x])}
          </UnderlinePanels.Tab>
        ))}
        {tabs.map((x) => (
          <UnderlinePanels.Panel key={x} value={x} className="cell-tab">
            {x !== tab ? null : x === 'changes' && diff ? (
              <Changes d={diff} />
            ) : x === 'rounds' && work !== undefined && rounds && onRound ? (
              <RoundsHere work={work} rounds={rounds} onRound={onRound} />
            ) : x === 'cell' ? (
              page
            ) : null}
          </UnderlinePanels.Panel>
        ))}
      </UnderlinePanels>
    </div>
  );
}
