// A cell, as its panel explains it on one page: under its title (and where it stands when the chart does not show it
// plainly), an overview of three rows, what it is given, what it hands back and where it is entered from; then three
// sections, folded at first, those the owner opens staying open from cell to cell: what it is given (CellGiven.tsx),
// what it hands back and where each answer leads (Returns.tsx), and where it comes from. An engine's step, a tool or
// the owner answers the same three questions with what its step reads, what it comes to and where each of its rows
// leads. On a work's chart its rounds there, and in a comparison what changed, are tabs beside it.

import { Label } from '@primer/react';
import { UnderlinePanels } from '@primer/react/experimental';
import { Fragment, type ReactNode, useEffect, useRef, useState } from 'react';

import type { CellDiff, CompiledCell, CompiledGraph, RoundSummary, RowRef } from '../api';
import { OutcomeBadge } from '../components';
import { roundName, time } from '../format';
import { styleOf } from '../graph/flowDraw';
import { actionWords, fallback, letterOf, mainPath } from '../graph/flowModel';
import { words } from '../labelled';
import { LinkButton } from '../parts';
import { type Key, t } from '../text';
import { Name, Overview, Pairs, PanelSection, Part } from '../voices';
import { AgentGiven, inputName, numbered, StepGiven } from './CellGiven';
import { Changes } from './Changes';
import { useFlow } from './context';
import { AgentReturns, leavesTo, StepReturns, telling, titleOf } from './Returns';
import { CellCode, CellLink, conditionsWords, inline, Target } from './words';

export { counterFirst, fallbackWords, leavesTo, ownMoves, telling } from './Returns';
export { numbered, projectOwn } from './CellGiven';

/// Where a cell stands, as its title's parenthesis says it: its stage, as the box around it on the chart names it, or
/// the asking part; and its line, when the chart does not draw that line in a colour of its own. None for neither.
export function cellWhere(g: CompiledGraph, c: CompiledCell): string | null {
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
  if (styleOf(c) === 'line-other' && c.line != null) parts.push(t('panel_line', { line: c.line }));
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

/// The sections of a cell's page, which the owner opens and folds, kept from cell to cell.
export type CellSection = 'given' | 'back' | 'from';

/// Where a cell is entered from, by kind: other cells' own answers that lead here, and the cell's own back to itself;
/// the rows back here after a move (from the tools, a pause, answers, an ask, or to where the work stopped); the
/// moves of other cells that lead here, each with the cells that make it; and whether the engine's fallback brings the
/// cell back to itself.
export type EnteredFrom = {
  answers: RowRef[];
  back: RowRef[];
  moves: Array<{ row: string; cells: string[] }>;
  fallback: boolean;
};

export function enteredFrom(g: CompiledGraph, refs: RowRef[], id: string): EnteredFrom {
  const out: EnteredFrom = { answers: [], back: [], moves: [], fallback: false };
  for (const ref of refs) {
    const row = g.cells.find((x) => x.id === ref.cell)?.rows[ref.row];
    if (row === undefined) continue;
    if (fallback(row)) {
      if (ref.cell === id) out.fallback = true;
    } else if (row.common) {
      if (ref.cell === id) continue;
      const m = out.moves.find((x) => x.row === row.name);
      if (m === undefined) out.moves.push({ row: row.name, cells: [ref.cell] });
      else if (!m.cells.includes(ref.cell)) m.cells.push(ref.cell);
    } else if (row.to.startsWith('@') && ref.cell !== id) {
      out.back.push(ref);
    } else {
      out.answers.push(ref);
    }
  }
  return out;
}

/// Things joined as a list is: a, b and c; or, as names a line starts with, a, b, c (`commas`).
function Listed({ items, commas }: { items: ReactNode[]; commas?: boolean }) {
  return (
    <>
      {items.map((x, i) => (
        <Fragment key={i}>
          {i > 0 ? (i === items.length - 1 && !commas ? ` ${t('and_word')} ` : ', ') : null}
          {x}
        </Fragment>
      ))}
    </>
  );
}

/// Where the cell is entered from: the rows of other cells that lead here, the ways back after its own moves, the
/// moves of other cells that lead here, and the engine's fallback.
function From({ c }: { c: CompiledCell }) {
  const f = useFlow();
  const into = enteredFrom(f.g, f.cellView(c.id)?.entered_from ?? [], c.id);
  const known = new Set(Object.keys(f.compiled.engine.values));
  const none = into.answers.length + into.back.length + into.moves.length === 0 && !into.fallback;
  return (
    <>
      {none ? <p className="pp-note">{t('entered_none')}</p> : null}
      {into.answers.length > 0 ? (
        <Pairs
          rows={into.answers.map((ref) => {
            const row = f.cell(ref.cell)?.rows[ref.row];
            if (row === undefined) return null;
            const cond = telling(row);
            return [
              ref.cell === c.id ? <span key="c">{t('entered_itself')}</span> : <CellLink key="c" id={ref.cell} />,
              <span key="r">
                <Name>{row.name}</Name>
                {cond.length > 0 ? <span className="pp-facts"> {inline(t('row_when', { when: conditionsWords(cond, known) }))}</span> : null}
              </span>,
            ];
          })}
        />
      ) : null}
      {into.back.length > 0 ? (
        <Part title={t('entered_back')} facts={t('entered_back_facts')}>
          <Pairs
            rows={into.back.map((ref) => [
              <CellLink key="c" id={ref.cell} />,
              <Name key="r">{f.cell(ref.cell)?.rows[ref.row]?.name ?? ''}</Name>,
            ])}
          />
        </Part>
      ) : null}
      {into.moves.length > 0 ? (
        <Part title={t('entered_moves')} facts={t('entered_moves_facts')}>
          <Pairs
            rows={into.moves.map((m) => [
              <Name key="r">{m.row}</Name>,
              <span key="c">
                {t('entered_moves_from')} <Listed items={m.cells.map((x) => <CellCode key={x} id={x} />)} />
              </span>,
            ])}
          />
        </Part>
      ) : null}
      {into.fallback ? <p className="pp-note">{t('entered_fallback')}</p> : null}
    </>
  );
}

/// The overview of a cell: what it is given, what it hands back and where it is entered from, each in a line, what
/// the line leaves out in a second, small one.
function CellOverview({ c, onInput }: { c: CompiledCell; onInput: (i: number) => void }) {
  const f = useFlow();
  const agent = c.kind === 'agent';
  const view = f.cellView(c.id);
  const inputs = numbered(c).filter(({ no }) => no !== null || !agent);
  const given = (
    <Listed
      commas
      items={inputs.map(({ x, i }) => (
        <span key={i}>
          <LinkButton onClick={() => onInput(i)}>{inputName(x)}</LinkButton>
          {x.maybe ? <span className="pp-facts"> {t('input_if_any')}</span> : null}
        </span>
      ))}
    />
  );
  const others = (view?.inputs ?? []).filter((m) => m.of.kind !== 'input');
  const rules = others.flatMap((m) => (m.of.kind === 'rule' ? [m.of.name] : []));
  const once = [
    rules.length > 0 ? t(rules.length === 1 ? 'given_once_rule' : 'given_once_rules', { rules: listWords(rules) }) : null,
    others.some((m) => m.of.kind === 'project_rules') ? t('given_once_project_rules') : null,
    others.some((m) => m.of.kind === 'common_format' || m.of.kind === 'cell_format') ? t('given_once_formats') : null,
    others.some((m) => m.of.kind === 'workspace') ? t('given_once_worktree') : null,
  ].filter((x) => x !== null);
  const out = leavesTo(c);
  const own = view?.returns.forms.find((x) => x.field === null)?.fields ?? [];
  const forms = (c.forms ?? []).filter((x) => !x.own && x.field != null).map((x) => words(x.field ?? ''));
  const into = enteredFrom(f.g, view?.entered_from ?? [], c.id);
  const from = [...new Set(into.answers.map((r) => r.cell))];
  const back = [...new Set(into.back.map((r) => r.cell))];
  // Each place the rows lead, once, after the rows that lead there; the cell's own answer by its fields' titles.
  const answers = agent ? out.answers : c.rows.map((_, i) => i);
  const byTarget: number[][] = [];
  for (const i of answers) {
    const same = byTarget.find((g) => c.rows[g[0]].to === c.rows[i].to);
    if (same !== undefined) same.push(i);
    else byTarget.push([i]);
  }
  const lead = (rows: number[]) => (
    <span key={rows[0]}>
      {answers.length > 1 || !agent ? (
        <>
          <Listed commas items={rows.map((i) => <Name key={i}>{c.rows[i].name}</Name>)} />{' '}
        </>
      ) : null}
      <span className="pp-arrow">→</span> <Target c={c} row={rows[0]} />
      {rows.some((i) => mainPath(c.rows[i])) ? (
        <>
          {' '}
          <Label variant="secondary">{t('row_main')}</Label>
        </>
      ) : null}
    </span>
  );
  return (
    <Overview
      rows={[
        {
          label: t('ov_given'),
          value: inputs.length > 0 ? given : t('none_given'),
          sub: agent
            ? once.length > 0
              ? t('given_once', { what: listWords(once) })
              : undefined
            : c.step
              ? inline(t('given_by_step', { step: `\`${c.step}\`` }))
              : undefined,
        },
        {
          label: t('ov_back'),
          value: (
            <>
              {agent && own.length > 0
                ? `${own.map((x) => titleOf(c.titles, x.path, x.name)).join(', ')}${answers.length > 1 ? ': ' : ' '}`
                : null}
              {byTarget.length > 0 ? <Listed commas items={byTarget.map(lead)} /> : t('leaves_none')}
            </>
          ),
          sub: forms.length > 0 ? t('back_or_forms', { forms: forms.join(', ') }) : undefined,
        },
        {
          label: t('ov_from'),
          value:
            from.length > 0 ? (
              <Listed commas items={from.map((x) => (x === c.id ? <span key={x}>{t('entered_itself')}</span> : <CellLink key={x} id={x} />))} />
            ) : into.moves.length > 0 ? (
              t('from_moves_only', { moves: listWords(into.moves.map((m) => actionWords(m.row))) })
            ) : back.length > 0 ? (
              t('from_only_back')
            ) : (
              t('entered_none')
            ),
          sub:
            back.length > 0 || (into.moves.length > 0 && from.length > 0) ? (
              <>
                {back.length > 0 ? (
                  <>
                    {t('from_back')} <Listed items={back.map((x) => <CellCode key={x} id={x} />)} />
                  </>
                ) : null}
                {back.length > 0 && into.moves.length > 0 && from.length > 0 ? '; ' : null}
                {into.moves.length > 0 && from.length > 0 ? (
                  <>
                    {t('from_moves')} <Listed items={into.moves.map((m) => actionWords(m.row))} />
                  </>
                ) : null}
              </>
            ) : undefined,
        },
      ]}
    />
  );
}

/// Names as a list is written: a, b and c.
function listWords(xs: string[]): string {
  if (xs.length <= 1) return xs.join('');
  return `${xs.slice(0, -1).join(', ')} ${t('and_word')} ${xs[xs.length - 1]}`;
}

/// The cell's own page: its overview and its three sections.
function CellPage({ c, open, onOpen }: { c: CompiledCell; open: Set<CellSection>; onOpen: (s: CellSection, open: boolean) => void }) {
  const f = useFlow();
  const agent = c.kind === 'agent';
  const [inputs, setInputs] = useState<Set<number>>(new Set());
  const [focus, setFocus] = useState<number | null>(null);
  // An input picked in the overview opens in its section and comes into view.
  useEffect(() => {
    if (focus === null) return;
    document.getElementById(`cell-input-${focus}`)?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    setFocus(null);
  }, [focus]);
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
  const into = enteredFrom(f.g, view?.entered_from ?? [], c.id);
  const fromCells = new Set(into.answers.map((r) => r.cell)).size;
  const back = new Set(into.back.map((r) => r.cell)).size;
  const forms = (c.forms ?? []).length;
  return (
    <div className="pp-page">
      <CellOverview
        c={c}
        onInput={(i) => {
          onOpen('given', true);
          onOpened(i, true);
          setFocus(i);
        }}
      />
      <PanelSection
        title={t('sec_given')}
        n={agent ? own.length : c.inputs.length}
        facts={agent ? t('given_facts', { inputs: plural(own.length, 'n_inputs'), rules: plural(rules, 'n_rules') }) : t('given_step_facts')}
        open={open.has('given')}
        onOpen={(o) => onOpen('given', o)}
      >
        {agent ? <AgentGiven c={c} opened={inputs} onOpened={onOpened} /> : <StepGiven c={c} opened={inputs} onOpened={onOpened} />}
      </PanelSection>
      <PanelSection
        title={t('sec_back')}
        n={agent ? forms : c.rows.length}
        facts={agent ? t('back_facts') : t('back_step_facts')}
        open={open.has('back')}
        onOpen={(o) => onOpen('back', o)}
      >
        {agent ? <AgentReturns c={c} /> : <StepReturns c={c} />}
      </PanelSection>
      <PanelSection
        title={t('sec_from')}
        n={into.answers.length + into.back.length + into.moves.length}
        facts={[fromCells > 0 ? plural(fromCells, 'n_cells') : null, back > 0 ? t('from_facts_back', { n: back }) : null]
          .filter((x) => x !== null)
          .join(', ')}
        open={open.has('from')}
        onOpen={(o) => onOpen('from', o)}
      >
        <From c={c} />
      </PanelSection>
    </div>
  );
}

/// A count in words: "1 input", "7 inputs".
function plural(n: number, key: 'n_inputs' | 'n_rules' | 'n_cells'): string {
  return t(n === 1 ? (`${key}_one` as Key) : key, { n });
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
  open: Set<CellSection>;
  onOpen: (s: CellSection, open: boolean) => void;
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
