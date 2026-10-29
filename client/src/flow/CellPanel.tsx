// A cell, as its panel explains it: after its title, where it stands on the chart when the chart does not show it
// plainly; then tabs. For an agent's cell: its inputs, first the message as the engine sends it every time the cell
// is entered, in the order sent (why the round happens, what is given, the task word for word, what to work from, the
// review table if any, what to hand in), with what the engine fills in each round shown as a placeholder, then the
// conditional inputs that message gives or points at by their numbers; what it hands back, each field with the checks
// on it and what the engine does every time the return holds it, what is shown on the item and what others read;
// where it is entered from and where it leaves to, by kind: what the engine does every time with no field to set it
// off, its own answers, the moves it makes of its own that the legend does not say, and the engine's fallback apart.
// For an engine step, a tool or the owner: what the step does and what it reads, which never goes into any agent's
// context, and the same after. A work's chart adds its rounds at the cell; a comparison, what changed.

import { type ReactNode, useEffect, useRef } from 'react';

import type {
  Atom,
  CellDiff,
  CellInput,
  CompiledCell,
  CompiledGraph,
  CompiledInput,
  CompiledRow,
  InputsNow,
  RoundSummary,
  RowRef,
  Src,
} from '../api';
import { OutcomeBadge } from '../components';
import { roundName, time } from '../format';
import { styleOf } from '../graph/flowDraw';
import { actionWords, fallback, letterOf, mainPath, type Move, moves, rowActions } from '../graph/flowModel';
import { useResource } from '../live';
import { Markdown } from '../markdown';
import { type Key, t } from '../text';
import { useFlow } from './context';
import { Changes } from './Changes';
import { Returns } from './Returns';
import { CellLink, conditionsWords, inline, RowNo, SourceChip, Target } from './words';

export function Section({ title, note, children, id }: {
  title?: string;
  note?: ReactNode;
  children: ReactNode;
  id?: string;
}) {
  return (
    <section className="ds" id={id}>
      {title !== undefined ? <h4>{title}</h4> : null}
      {note ? <p className="fmt">{note}</p> : null}
      {children}
    </section>
  );
}

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

/// The tabs of a cell's panel.
export type CellTab = 'changes' | 'rounds' | 'inputs' | 'returns' | 'route';

/// A cell's tabs, in order: what changed, when two flows are compared; its rounds, on a work's chart; its inputs,
/// what it hands back with the checks on it, and where it is entered from and where it leaves to.
export function cellTabs(has: { rounds: boolean; diff: boolean }): CellTab[] {
  const tabs: CellTab[] = [];
  if (has.diff) tabs.push('changes');
  if (has.rounds) tabs.push('rounds');
  tabs.push('inputs', 'returns', 'route');
  return tabs;
}

/// The tab a cell's panel shows: the one the owner chose last, when this cell has it, else its first, so that going
/// from cell to cell keeps to what the owner reads.
export function cellTab(tabs: CellTab[], chosen: CellTab | null): CellTab {
  return chosen !== null && tabs.includes(chosen) ? chosen : tabs[0];
}

const TAB_NAMES: Record<CellTab, Key> = {
  changes: 'sec_changes',
  rounds: 'tab_rounds',
  inputs: 'sec_inputs',
  returns: 'sec_returns',
  route: 'tab_route',
};

/// What the engine fills in each round, in the placeholder's style: a description between angle quotes, or the parts
/// of an address (`‹work›`, `‹n›`, `‹id›`, `‹cell›`, `‹name›`) known only when the round runs.
function Filled({ text }: { text: string }) {
  return (
    <>
      {text.split(/(‹[^›]*›)/).map((part, i) =>
        part.startsWith('‹') ? (
          <span key={i} className="ph">
            {part}
          </span>
        ) : (
          part
        ),
      )}
    </>
  );
}

/// What a conditional input holds as far as the page can know outside a round, folded under its row as a round's
/// inputs are.
function Holds({ children }: { children: ReactNode }) {
  return (
    <details className="input-held">
      <summary className="muted">{t('input_open')}</summary>
      <div className="input-content">{children}</div>
    </details>
  );
}

/// A text the flow writes, in full.
function FlowText({ text }: { text: string }) {
  return <pre className="j-text file-text">{text}</pre>;
}

/// What one source of an input holds now: a setting, as each project's configuration has it; the project guide, as
/// each project's checkout has it, the nodes an item cites added for each item; what else the engine computes, as the
/// flow says; a table or view, as its definition describes it; nothing for a place of a return, which the cell it
/// comes from shows, or what rows recorded.
function SourceHolds({ src, now }: { src: Src; now: InputsNow | undefined }) {
  const f = useFlow();
  if (Array.isArray(src) || src === 'self' || 'recorded' in src) return null;
  if ('engine' in src && src.engine !== 'project_guide') {
    return <p>{f.compiled.engine.inputs[src.engine] ?? src.engine}</p>;
  }
  if (now === undefined) return <p className="muted">{t('loading')}</p>;
  if ('db' in src) {
    const table = now.tables.find((x) => x.name === src.db);
    return table !== undefined ? <p>{table.text}</p> : null;
  }
  if ('config' in src) {
    return (
      <>
        {now.projects.map((p) => {
          const value = p.settings.find((s) => s.name === src.config);
          return (
            <div key={p.project} className="holds-project">
              <span className="decision-label">{p.project}</span>
              {value !== undefined ? <Markdown text={value.text} /> : <p className="muted">{t('holds_unset')}</p>}
            </div>
          );
        })}
      </>
    );
  }
  return (
    <>
      {now.projects.map((p) => (
        <div key={p.project} className="holds-project">
          <span className="decision-label">{p.project}</span>
          <details>
            <summary>AGENTS.md</summary>
            <Markdown text={p.agents_md} />
          </details>
          <details>
            <summary>{t('holds_design_tree', { n: p.design_tree.length })}</summary>
            <ul className="bl">
              {p.design_tree.map((n) => (
                <li key={n.name}>
                  <code>{n.name}</code> {n.text}
                </li>
              ))}
            </ul>
          </details>
        </div>
      ))}
      <p className="muted">{t('holds_guide_cited')}</p>
    </>
  );
}

/// One input of the cell the conditional inputs list, by the number the message's list to work from gives it: what it
/// is, each of its sources with its address, and, folded, what it holds as far as the page can know.
function NumberedInput(props: { no: number; x: CompiledInput; sources: CellInput[]; now: InputsNow | undefined }) {
  const { no, x, sources, now } = props;
  const srcs = sources.flatMap((m) => {
    if (m.of.kind !== 'input') return [];
    const src = x.from[m.of.source];
    return src === undefined ? [] : [{ m, src }];
  });
  const holding = srcs.filter(({ src }) => !Array.isArray(src) && src !== 'self' && !('recorded' in src));
  return (
    <li>
      <span className="in-no">{no}.</span> {x.name ? <strong>{x.name}</strong> : null} {x.what}
      {x.maybe ? <span className="fw"> · {t('input_maybe')}</span> : null}
      <div className="srcs">
        <span className="ul">{t('input_source')}</span>
        {srcs.map(({ m, src }, i) => (
          <span key={i} className="src">
            <SourceChip src={src} cells={m.cells} />
            {m.address ? (
              <code className="addr">
                <Filled text={m.address} />
              </code>
            ) : null}
          </span>
        ))}
      </div>
      {holding.length > 0 ? (
        <Holds>
          {holding.map(({ src }, i) => (
            <SourceHolds key={i} src={src} now={now} />
          ))}
        </Holds>
      ) : null}
    </li>
  );
}

/// A conditional input that is none of the cell's own: a rule, a return format, what each numbered input is, the
/// project's rules, the worktree; each with what it holds folded under it, and a rule with a link to its own window.
function OtherInput({ c, m, now }: { c: CompiledCell; m: CellInput; now: InputsNow | undefined }) {
  const f = useFlow();
  const addr = m.address ? (
    <code className="addr">
      <Filled text={m.address} />
    </code>
  ) : null;
  if (m.of.kind === 'rule') {
    const name = m.of.name;
    const rule = f.g.rules.find((r) => r.name === name);
    return (
      <li>
        <button type="button" className="go" onClick={() => f.open({ kind: 'rule', rule: name })}>
          {t('input_rule', { name })}
        </button>
        {rule ? <span className="fw"> · {rule.desc}</span> : null}
        <div className="srcs">{addr}</div>
        {rule ? (
          <Holds>
            <ul className="bl">
              {rule.text.map((line, i) => (
                <li key={i}>{inline(line)}</li>
              ))}
            </ul>
          </Holds>
        ) : null}
      </li>
    );
  }
  const key: Record<string, Key> = {
    project_rules: 'input_project_rules',
    common_format: 'input_common_format',
    cell_format: 'input_cell_format',
    cell_inputs: 'input_cell_inputs',
    workspace: 'input_workspace',
  };
  const holds =
    m.of.kind === 'common_format' ? (
      <FlowText text={f.view.common_format} />
    ) : m.holds !== null ? (
      <FlowText text={m.holds} />
    ) : m.of.kind === 'project_rules' ? (
      now === undefined ? (
        <p className="muted">{t('loading')}</p>
      ) : (
        now.projects.map((p) => (
          <div key={p.project} className="holds-project">
            <span className="decision-label">{p.project}</span>
            {p.rules.length === 0 ? <p className="muted">{t('none')}</p> : null}
            {p.rules.map((r) => (
              <details key={r.name}>
                <summary>{t('input_rule', { name: r.name })}</summary>
                <Markdown text={r.text} />
              </details>
            ))}
          </div>
        ))
      )
    ) : m.of.kind === 'workspace' ? (
      <p>{t('holds_workspace')}</p>
    ) : null;
  return (
    <li>
      {t(key[m.of.kind] ?? 'input_common_format', { cell: c.id })}
      <div className="srcs">{addr}</div>
      {holds !== null ? <Holds>{holds}</Holds> : null}
    </li>
  );
}

/// Whether an input is the project's own context, which the message gives before anything else: the project's guide
/// and a setting of its configuration, such as its design goal.
export function projectOwn(x: CompiledInput): boolean {
  return x.from.some((s) => typeof s === 'object' && !Array.isArray(s) && ('config' in s || ('engine' in s && s.engine === 'project_guide')));
}

/// The cell's inputs, each with the number the message's list to work from gives it, or none for an input the agent
/// reads itself; the engine numbers them so in `inputs:<cell>`.
export function numbered(c: CompiledCell): Array<{ x: CompiledInput; i: number; no: number | null }> {
  let no = 0;
  return c.inputs.map((x, i) => {
    if (x.from.every((s) => s === 'self')) return { x, i, no: null };
    no += 1;
    return { x, i, no };
  });
}

/// What an agent's round is given: the message as it is sent every time the cell is entered, in the order sent, what
/// the engine fills in each round as placeholders; then the conditional inputs, those of the cell numbered as the
/// message's list to work from points at them.
function AgentInputs({ c }: { c: CompiledCell }) {
  const f = useFlow();
  const view = f.cellView(c.id);
  const inputs = view?.inputs ?? [];
  const own = numbered(c);
  // What the inputs from outside the flow hold now: the projects' settings, guides and rules, and the database.
  const now = useResource<InputsNow>('inputs-now', '/api/v1/inputs-now').data;
  const sources = (i: number) => inputs.filter((m) => m.of.kind === 'input' && m.of.input === i);
  const others = inputs.filter((m) => m.of.kind !== 'input');
  const rules = others.filter((m) => m.of.kind === 'project_rules' || m.of.kind === 'rule').map((m) => m.address ?? '');
  const worktree = others.find((m) => m.of.kind === 'workspace')?.address ?? null;
  // The table review is given the columns of the review table the rule of that name lists, each with its number.
  const table = (c.rules ?? []).some((r) => r.name === 'review_table');
  const prompt = c.prompt ?? [];
  const enums = c.prompt_by ? (c.inenums?.[c.prompt_by.by] ?? []) : [];
  return (
    <>
      <Section title={t('inputs_message')}>
        <div className="sent">
          <div className="sent-line">
            <Filled text={t('sent_title', { cell: c.id, title: c.title })} />
          </div>
          <div className="sent-line">
            <span className="ph">‹{t('sent_why')}›</span>
          </div>
          <div className="sent-h">{t('sent_given')}</div>
          <div className="sent-line">
            <span className="ph">‹{t('sent_given_what')}›</span>
          </div>
          <div className="sent-h">{t('sent_task')}</div>
          {prompt.length > 0 ? (
            <div className="prompt">
              {prompt.map((p, i) => (
                <p key={i}>{inline(p)}</p>
              ))}
            </div>
          ) : null}
          {c.prompt_by ? (
            <div className="prompt-by">
              <div className="fmt">{inline(t('prompt_by', { by: `\`${c.prompt_by.by}\`` }))}</div>
              {Object.entries(c.prompt_by.cases).map(([k, lines]) => (
                <div key={k} className="case">
                  <div className="case-h">
                    <code>{k}</code>
                    {enums.find(([v]) => v === k)?.[1] ? (
                      <span className="fw"> · {enums.find(([v]) => v === k)?.[1]}</span>
                    ) : null}
                  </div>
                  {lines.map((l, i) => (
                    <p key={i}>{inline(l)}</p>
                  ))}
                </div>
              ))}
            </div>
          ) : null}
          <div className="sent-h">{t('sent_work_from')}</div>
          {own.map(({ x, i, no }) =>
            no === null ? (
              <div key={i} className="sent-line">
                {t('sent_read_yourself', { what: x.what })}
              </div>
            ) : (
              <div key={i} className="sent-line" title={x.what}>
                {no}.{' '}
                <Filled
                  text={sources(i)
                    .map((m) => m.address ?? '')
                    .filter((a) => a !== '')
                    .join(', ')}
                />
              </div>
            ),
          )}
          {rules.length > 0 ? (
            <div className="sent-line">
              <Filled text={t('sent_rules', { rules: rules.join(', ') })} />
            </div>
          ) : null}
          {worktree !== null ? (
            <div className="sent-line">
              <Filled text={t('sent_worktree', { address: worktree })} />
            </div>
          ) : null}
          {table ? (
            <>
              <div className="sent-h">{t('sent_table')}</div>
              <div className="sent-line">
                <span className="ph">‹{t('sent_table_what')}›</span>
              </div>
            </>
          ) : null}
          <div className="sent-line sent-end">{t('sent_hand_in', { cell: c.id })}</div>
        </div>
      </Section>
      <Section title={t('inputs_conditional')}>
        <ul className="bl">
          {own.map(({ x, i, no }) =>
            no === null || !projectOwn(x) ? null : <NumberedInput key={i} no={no} x={x} sources={sources(i)} now={now} />,
          )}
          {others.map((m, i) => (
            <OtherInput key={i} c={c} m={m} now={now} />
          ))}
          {own.map(({ x, i, no }) =>
            no === null || projectOwn(x) ? null : <NumberedInput key={i} no={no} x={x} sources={sources(i)} now={now} />,
          )}
        </ul>
      </Section>
    </>
  );
}

/// What a step that is no agent's round does and reads.
function StepInputs({ c }: { c: CompiledCell }) {
  const f = useFlow();
  const inputs = f.cellView(c.id)?.inputs ?? [];
  return (
    <>
      {c.step ? (
        <Section title={t('sec_step')}>
          <p>
            <code>{c.step}</code> {f.compiled.engine.steps[c.step] ?? ''}
          </p>
          {c.needs ? (
            <p className="fmt">
              {inline(t('step_needs', { need: `\`${c.needs}\`` }))} {f.compiled.engine.needs[c.needs] ?? ''}
            </p>
          ) : null}
        </Section>
      ) : null}
      <Section title={t('sec_step_reads')} note={t('inputs_step_note')}>
        {c.inputs.length === 0 ? <p className="fmt">{t('none')}</p> : null}
        <ul className="bl">
          {c.inputs.map((x, i) => (
            <li key={i}>
              {x.what}
              {x.maybe ? <span className="fw"> · {t('input_maybe')}</span> : null}
              <div className="srcs">
                <span className="ul">{t('input_source')}</span>
                {x.from.map((s, j) => {
                  const m = inputs.find((y) => y.of.kind === 'input' && y.of.input === i && y.of.source === j);
                  return <SourceChip key={j} src={s} cells={m?.cells ?? []} />;
                })}
              </div>
            </li>
          ))}
        </ul>
      </Section>
    </>
  );
}

/// The parts of a row's condition that say something its name and kind do not: not that the return is the cell's own
/// answer, nor what a move or the engine's fallback turns on, which their names and the list they are in say, nor a
/// value that is the row's own name.
export function telling(r: CompiledRow): Atom[] {
  if (r.common) return [];
  return r.if.filter((a) => {
    if (a[0] === 'own' || a[0] === 'all') return false;
    if (a[0] === 'is' && a[2].length === 1 && a[2][0] === r.name) return false;
    return true;
  });
}

/// What the engine does for a row of its fallback, as the flow's section on rounds says it; none in a flow version
/// stored before it said it so.
export function fallbackWords(g: CompiledGraph, r: CompiledRow): string | null {
  const rounds = g.rounds;
  const a = r.if[0];
  if (rounds == null || a === undefined) return null;
  if (a[0] === 'sent_back') return a.length === 2 ? rounds.sentBack.pause : rounds.sentBack.desc;
  if (a[0] === 'failed') return rounds.failed.kinds.find((k) => k.name === a[1])?.desc ?? null;
  if (a[0] === 'engine') return rounds.engine?.find((k) => k.name === a[1])?.desc ?? null;
  return null;
}

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

/// Where a cell leaves to, by kind, each a list of its rows by their places: its own answers, those on the main path
/// first; the moves it makes of its own; and the engine's fallback.
export function leavesTo(c: CompiledCell): { answers: number[]; moves: number[]; fallback: number[] } {
  const all = c.rows.map((_, i) => i);
  const answers = all.filter((i) => !c.rows[i].common && !fallback(c.rows[i]));
  return {
    answers: [...answers.filter((i) => c.rows[i].main), ...answers.filter((i) => !c.rows[i].main)],
    moves: all.filter((i) => c.rows[i].common && !fallback(c.rows[i])),
    fallback: all.filter((i) => fallback(c.rows[i])),
  };
}

/// The moves of its own a cell's panel lists, apart from what the legend says of every cell: the rows of the moves
/// drawn as lines, and of any the legend does not name, by their places; the moves the legend gives this cell with the
/// others, by name, for one line saying so; and the moves the legend gives most cells that could make them, which this
/// one does not.
export function ownMoves(g: CompiledGraph, c: CompiledCell): { rows: number[]; legend: string[]; lacks: Move[] } {
  const all = moves(g);
  const rows: number[] = [];
  const legend: string[] = [];
  for (const i of leavesTo(c).moves) {
    const m = all.find((x) => x.row === c.rows[i].name);
    if (m === undefined || m.tier === 'lines' || !m.cells.includes(c.id)) rows.push(i);
    else if (!legend.includes(m.row)) legend.push(m.row);
  }
  const lacks = all.filter((m) => m.tier === 'marked' && m.could.includes(c.id) && !m.cells.includes(c.id));
  return { rows, legend, lacks };
}

/// The rows of a cell whose counter's description its panel gives, in the order it lists them: the first row of each
/// counter; the others name the counter alone.
export function counterFirst(c: CompiledCell, listed: number[]): Set<number> {
  const seen = new Set<string>();
  const out = new Set<number>();
  for (const i of listed) {
    const k = c.rows[i]?.counts;
    if (k == null || seen.has(k)) continue;
    seen.add(k);
    out.add(i);
  }
  return out;
}

/// The longest a condition is written out before it is folded.
const LONG = 140;

/// A row's condition, where it says something, folded when it is long.
function Condition({ r }: { r: CompiledRow }) {
  const f = useFlow();
  const atoms = telling(r);
  if (atoms.length === 0) return null;
  const words = conditionsWords(atoms, new Set(Object.keys(f.compiled.engine.values)));
  if (words.length <= LONG) return <div className="tx">{inline(words)}</div>;
  return (
    <details className="tx">
      <summary className="muted">{t('row_if')}</summary>
      {inline(words)}
    </details>
  );
}

/// One row the cell leaves by: its number, name and target, and what the engine does on it, counts and records; the
/// main path only on the cell's own answers, a move being on none; the counter's description where `counter` says.
function RowItem({ c, i, on, counter }: { c: CompiledCell; i: number; on: boolean; counter: boolean }) {
  const f = useFlow();
  const r = c.rows[i];
  const acts = rowActions(r);
  const counted = counter ? (f.g.counters.find((k) => k.name === r.counts)?.desc ?? '') : '';
  return (
    <li className={on ? 'tt-on' : undefined}>
      <RowNo row={i} />
      <div className="tb">
        <div>
          <strong>{r.name}</strong> → <Target c={c} row={i} />
          {mainPath(r) ? <span className="tag tag-main">{t('row_main')}</span> : null}
          {r.waits ? <span className="tag">{t('row_waits')}</span> : null}
          {r.pauses ? <span className="tag">{t('row_pauses')}</span> : null}
          {r.interject ? <span className="tag">{t('row_interjection')}</span> : null}
        </div>
        <Condition r={r} />
        {acts.length > 0 ? (
          <div className="tx">
            <span className="ul">{t('row_action')}</span>
            {acts.map((a, j) => (
              <span key={a} title={f.compiled.engine.actions[a] ?? ''}>
                {j > 0 ? ' · ' : null}
                {actionWords(a)}
              </span>
            ))}
          </div>
        ) : null}
        {r.counts ? (
          <div className="tx">
            <span className="ul">{t('row_counts')}</span>
            <code>{r.counts}</code>
            {counter ? <span className="fw">{counted}</span> : null}
          </div>
        ) : null}
        {Object.entries(r.sets ?? {}).length > 0 ? (
          <div className="tx">
            <span className="ul">{t('row_records')}</span>
            {Object.entries(r.sets ?? {}).map(([k, v]) => (
              <code key={k}>
                {k} = {v}
              </code>
            ))}
          </div>
        ) : null}
        {r.note ? <div className="tx">{inline(r.note)}</div> : null}
      </div>
    </li>
  );
}

/// One row of the engine's fallback: what it is for, and what the engine does; a return sent back points at the
/// checks, in what the cell hands back.
function FallbackItem({ c, i, onReturns }: { c: CompiledCell; i: number; onReturns: () => void }) {
  const f = useFlow();
  const r = c.rows[i];
  const words = fallbackWords(f.g, r);
  const back = r.if[0]?.[0] === 'sent_back';
  return (
    <li>
      <RowNo row={i} />
      <div className="tb">
        <div>
          <strong>{r.name}</strong>
        </div>
        <div className="tx">{inline(words ?? conditionsWords(r.if, new Set()))}</div>
        {back ? (
          <button type="button" className="link" onClick={onReturns}>
            {t('fallback_checks')}
          </button>
        ) : null}
      </div>
    </li>
  );
}

/// Cells by their codes alone, each a link that opens it, with its title on hovering: for a move many cells make.
function CellCodes({ ids }: { ids: string[] }) {
  const f = useFlow();
  return (
    <span className="links">
      {ids.map((id, i) => (
        <span key={id}>
          {i > 0 ? ', ' : null}
          <button type="button" className="go" title={f.cell(id)?.title} onClick={() => f.open({ kind: 'cell', cell: id })}>
            {id}
          </button>
        </span>
      ))}
    </span>
  );
}

/// Where the cell is entered from and where it leaves to, by kind.
function Route({ c, highlight, onReturns }: { c: CompiledCell; highlight: number[]; onReturns: () => void }) {
  const f = useFlow();
  const into = enteredFrom(f.g, f.cellView(c.id)?.entered_from ?? [], c.id);
  const out = leavesTo(c);
  const own = ownMoves(f.g, c);
  const counter = counterFirst(c, [...out.answers, ...own.rows]);
  const does = c.actions ?? [];
  const none = into.answers.length + into.back.length + into.moves.length === 0 && !into.fallback;
  return (
    <>
      <Section title={t('sec_entered')}>
        {none ? <p className="fmt">{t('entered_none')}</p> : null}
        {into.answers.length > 0 ? (
          <ul className="bl">
            {into.answers.map((ref) => {
              const row = f.cell(ref.cell)?.rows[ref.row];
              if (row === undefined) return null;
              return (
                <li key={`${ref.cell}:${ref.row}`}>
                  {ref.cell === c.id ? t('entered_itself') : <CellLink id={ref.cell} />} <RowNo row={ref.row} />{' '}
                  <strong>{row.name}</strong>
                  <Condition r={row} />
                </li>
              );
            })}
          </ul>
        ) : null}
        {into.back.length > 0 ? (
          <>
            <h5 className="ds-sub">{t('entered_back')}</h5>
            <ul className="bl">
              {into.back.map((ref) => (
                <li key={`${ref.cell}:${ref.row}`}>
                  <CellLink id={ref.cell} /> <strong>{f.cell(ref.cell)?.rows[ref.row]?.name ?? ''}</strong>
                </li>
              ))}
            </ul>
          </>
        ) : null}
        {into.moves.length > 0 ? (
          <>
            <h5 className="ds-sub">{t('entered_moves')}</h5>
            <ul className="bl">
              {into.moves.map((m) => (
                <li key={m.row}>
                  <strong>{actionWords(m.row)}</strong>: <CellCodes ids={m.cells} />
                </li>
              ))}
            </ul>
          </>
        ) : null}
        {into.fallback ? <p className="fmt">{t('entered_fallback')}</p> : null}
      </Section>
      <Section title={t('sec_leaves')}>
        {does.length > 0 ? (
          <div className="tx">
            <span className="ul">{t('leaves_does')}</span>
            {does.map((a, j) => (
              <span key={a} title={f.compiled.engine.actions[a] ?? ''}>
                {j > 0 ? ' · ' : null}
                {actionWords(a)}
              </span>
            ))}
          </div>
        ) : null}
        {out.answers.length + out.moves.length + out.fallback.length === 0 ? <p className="fmt">{t('leaves_none')}</p> : null}
        {out.answers.length > 0 ? (
          <ol className="tt">
            {out.answers.map((i) => (
              <RowItem key={i} c={c} i={i} on={highlight.includes(i)} counter={counter.has(i)} />
            ))}
          </ol>
        ) : null}
        {own.rows.length + own.lacks.length + own.legend.length > 0 ? (
          <>
            <h5 className="ds-sub">{t('leaves_moves')}</h5>
            {own.rows.length > 0 ? (
              <ol className="tt">
                {own.rows.map((i) => (
                  <RowItem key={i} c={c} i={i} on={highlight.includes(i)} counter={counter.has(i)} />
                ))}
              </ol>
            ) : null}
            {own.lacks.length > 0 ? (
              <ul className="bl">
                {own.lacks.map((m) => (
                  <li key={m.row} title={t(m.engine ? 'leaves_lacks_engine' : 'chart_lacks_title', { cell: c.id, move: actionWords(m.row), to: m.to })}>
                    {t('chart_lacks', { move: actionWords(m.row) })}
                  </li>
                ))}
              </ul>
            ) : null}
            {own.legend.length > 0 ? (
              <p className="fmt">{t('leaves_moves_legend', { moves: own.legend.map(actionWords).join(', ') })}</p>
            ) : null}
          </>
        ) : null}
        {out.fallback.length > 0 ? (
          <details className="fallback">
            <summary>{t('leaves_fallback', { n: out.fallback.length })}</summary>
            <ol className="tt">
              {out.fallback.map((i) => (
                <FallbackItem key={i} c={c} i={i} onReturns={onReturns} />
              ))}
            </ol>
          </details>
        ) : null}
      </Section>
    </>
  );
}

/// The work's rounds at the cell, newest first; a badge only for what is not the usual course, as in the work's list.
function RoundsHere({ work, rounds, onRound }: { work: number; rounds: RoundSummary[]; onRound: (r: RoundSummary) => void }) {
  if (rounds.length === 0) return <p className="fmt">{t('no_rounds')}</p>;
  return (
    <table className="list mini">
      <tbody>
        {[...rounds].reverse().map((r) => (
          <tr key={r.n} className="clickable" onClick={() => onRound(r)}>
            <td className="muted nowrap">{roundName(work, r.n)}</td>
            <td className="clip">
              {r.outcome !== 'took' ? (
                <>
                  <OutcomeBadge outcome={r.outcome} />{' '}
                </>
              ) : null}
              {r.transition ?? r.why ?? ''}
            </td>
            <td className="muted nowrap">{time(r.started_at)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

export function CellPanel(props: {
  c: CompiledCell;
  /// Rows to point out.
  rows?: number[];
  /// On a work's page, the work and its rounds at this cell.
  work?: number;
  rounds?: RoundSummary[];
  onRound?: (r: RoundSummary) => void;
  /// When two flows are compared, what changed in this cell.
  diff?: CellDiff | null;
  /// The tab the owner chose last, kept from cell to cell, and how a choice is kept.
  tab: CellTab | null;
  onTab: (tab: CellTab) => void;
}) {
  const { c, rows = [], work, rounds, onRound, diff, tab: chosen, onTab } = props;
  const f = useFlow();
  const view = f.cellView(c.id);
  const agent = c.kind === 'agent';
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
  const checked = (c.checks ?? []).length > 0;
  return (
    <>
      <div className="tabs cell-tabs" role="tablist" ref={bar}>
        {tabs.map((x) => (
          <button key={x} type="button" role="tab" className="tab" aria-selected={x === tab} onClick={() => onTab(x)}>
            {t(TAB_NAMES[x])}
            {x === 'rounds' ? <span className="tab-n">{rounds?.length ?? 0}</span> : null}
          </button>
        ))}
      </div>
      {tab === 'changes' && diff ? <Changes d={diff} /> : null}
      {tab === 'rounds' && work !== undefined && rounds && onRound ? (
        <RoundsHere work={work} rounds={rounds} onRound={onRound} />
      ) : null}
      {tab === 'inputs' ? agent ? <AgentInputs c={c} /> : <StepInputs c={c} /> : null}
      {tab === 'returns' ? (
        <>
          <Section note={checked ? t('checks_note') : undefined}>{view ? <Returns c={c} r={view.returns} /> : null}</Section>
          {(c.shown ?? []).length > 0 ? (
            <Section title={t('sec_shown')} note={t('shown_note')}>
              <ul className="bl">
                {(c.shown ?? []).map((p) => (
                  <li key={p}>
                    <code>{p}</code>
                  </li>
                ))}
              </ul>
            </Section>
          ) : null}
          {(c.read_by ?? []).length > 0 ? (
            <Section title={t('sec_read_by')} note={t('read_by_note')}>
              <ul className="bl">
                {(c.read_by ?? []).map((x) => (
                  <li key={`${x.at}:${x.role}`}>
                    <code>{x.at}</code> · {t('use_role', { role: x.role })}
                  </li>
                ))}
              </ul>
            </Section>
          ) : null}
        </>
      ) : null}
      {tab === 'route' ? <Route c={c} highlight={rows} onReturns={() => onTab('returns')} /> : null}
    </>
  );
}
