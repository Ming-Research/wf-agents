// A cell, as its panel explains it, in tabs. For an agent's cell: its inputs, first the message as the engine sends it
// every time the cell is entered, in the order sent (why the round happens, what is given, the task word for word,
// what to work from, the review table if any, what to hand in), with what the engine fills in each round shown as a
// placeholder, then the conditional inputs that message gives or points at by their numbers; what it hands back, what
// is shown on the item and what others read; where it is entered from and where each of its rows goes; its checks,
// what the engine does every time it returns a field and what it hands over. For an engine step, a tool or the owner:
// what the step does and what it reads, which never goes into any agent's context, and the same after. A work's chart
// adds its rounds at the cell; a comparison, what changed.

import { type ReactNode, useEffect, useRef } from 'react';

import type { CellDiff, CellInput, CompiledCell, CompiledInput, GraphView, InputsNow, Named, RoundSummary, Src } from '../api';
import { OutcomeBadge } from '../components';
import { roundName, time } from '../format';
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

/// The words under a cell's title: who does its work, its line, its stage, and how its line's session is kept (when
/// it starts and which cells continue it) as the flow says it.
export function who(c: CompiledCell, lines: Named[]): ReactNode {
  const parts: string[] = [];
  const session = c.kind === 'agent' && c.line != null ? (lines.find((l) => l.name === c.line)?.desc ?? null) : null;
  if (c.kind === 'agent') {
    parts.push(c.line == null ? t('who_agent_fresh') : t('who_agent', { line: c.line }));
  } else {
    parts.push(t(c.kind === 'tool' ? 'who_tool' : c.kind === 'owner' ? 'who_owner' : 'who_engine'));
  }
  if (c.gate) parts.push(t('who_gate'));
  if (c.stage) parts.push(t('who_stage', { stage: c.stage }));
  return (
    <>
      {parts.join(' · ')}
      {session !== null ? <span className="panel-session">{session}</span> : null}
    </>
  );
}

/// The tabs of a cell's panel.
export type CellTab = 'changes' | 'rounds' | 'inputs' | 'returns' | 'route' | 'checks';

/// A cell's tabs, in order: what changed, when two flows are compared; its rounds, on a work's chart; its inputs,
/// what it hands back, and where it is entered from and where its rows go; its checks, when it has any.
export function cellTabs(c: CompiledCell, has: { rounds: boolean; diff: boolean }): CellTab[] {
  const tabs: CellTab[] = [];
  if (has.diff) tabs.push('changes');
  if (has.rounds) tabs.push('rounds');
  tabs.push('inputs', 'returns', 'route');
  if ((c.checks ?? []).length + (c.always ?? []).length + (c.handoff ?? []).length > 0) tabs.push('checks');
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
  checks: 'sec_checks',
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

function Table({ c, highlight }: { c: CompiledCell; highlight: number[] }) {
  const f = useFlow();
  const known = new Set(Object.keys(f.compiled.engine.values));
  return (
    <ol className="tt">
      {c.rows.map((r, i) => (
        <li key={r.name} className={highlight.includes(i) ? 'tt-on' : undefined}>
          <RowNo row={i} />
          <div className="tb">
            <div>
              <strong>{r.name}</strong>
              {r.main ? <span className="tag tag-main">{t('row_main')}</span> : null}
              {r.common ? <span className="tag">{t('row_shared')}</span> : null}
              {r.waits ? <span className="tag">{t('row_waits')}</span> : null}
              {r.pauses ? <span className="tag">{t('row_pauses')}</span> : null}
              {r.interject ? <span className="tag">{t('row_interjection')}</span> : null}
            </div>
            <div className="tx">
              <span className="ul">{t('row_if')}</span>
              <span>{inline(conditionsWords(r.if, known))}</span>
            </div>
            <div className="tx">
              <span className="ul">{t('row_to')}</span>
              <Target c={c} row={i} />
            </div>
            {r.counts ? (
              <div className="tx">
                <span className="ul">{t('row_counts')}</span>
                <code>{r.counts}</code>
                <span className="fw">{f.g.counters.find((k) => k.name === r.counts)?.desc ?? ''}</span>
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
            {(r.do ?? []).map((d, j) => {
              const name = typeof d === 'string' ? d : d.do;
              return (
                <div key={j} className="tx">
                  <span className="ul">{t('row_action')}</span>
                  <code>{name}</code>
                  {typeof d === 'string' ? null : <span className="fw">{inline(t('row_action_at', { at: `\`${d.at}\`` }))}</span>}
                  <span className="fw">{f.compiled.engine.actions[name] ?? ''}</span>
                </div>
              );
            })}
            {r.note ? <div className="tx">{inline(r.note)}</div> : null}
          </div>
        </li>
      ))}
    </ol>
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

/// Where the cell is entered from: every row of every cell that leads here.
function Entered({ c }: { c: CompiledCell }) {
  const f = useFlow();
  const view = f.cellView(c.id);
  return (
    <Section title={t('sec_entered')}>
      {(view?.entered_from ?? []).length === 0 ? <p className="fmt">{t('entered_none')}</p> : null}
      <ul className="bl">
        {(view?.entered_from ?? []).map((r) => {
          const from = f.cell(r.cell);
          const row = from?.rows[r.row];
          if (!from || !row) return null;
          return (
            <li key={`${r.cell}:${r.row}`}>
              {r.cell === c.id ? t('entered_itself') : <CellLink id={r.cell} />} <RowNo row={r.row} />{' '}
              <strong>{row.name}</strong>
              <div className="tx">{inline(conditionsWords(row.if, new Set(Object.keys(f.compiled.engine.values))))}</div>
            </li>
          );
        })}
      </ul>
    </Section>
  );
}

export function CellPanel(props: {
  c: CompiledCell;
  /// Rows to point out, such as those of the shared circle the panel was opened from.
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
  const tabs = cellTabs(c, {
    rounds: work !== undefined && rounds !== undefined && onRound !== undefined,
    diff: diff != null,
  });
  const tab = cellTab(tabs, chosen);
  const table = useRef<HTMLDivElement>(null);
  const bar = useRef<HTMLDivElement>(null);
  // A tab shows from its top; the transition table, when rows of it are pointed out, from the table.
  useEffect(() => {
    if (rows.length > 0 && tab === 'route') table.current?.scrollIntoView({ block: 'start' });
    else bar.current?.parentElement?.scrollTo({ top: 0 });
  }, [rows, tab]);
  const checks = c.checks ?? [];
  const always = c.always ?? [];
  const handoff = c.handoff ?? [];
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
          <Section>{view ? <Returns c={c} r={view.returns} /> : null}</Section>
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
      {tab === 'route' ? (
        <>
          <Entered c={c} />
          <div ref={table} className="cell-table">
            <Section title={t('sec_table')}>
              <Table c={c} highlight={rows} />
            </Section>
          </div>
        </>
      ) : null}
      {tab === 'checks' ? (
        <>
          {checks.length > 0 ? (
            <Section title={t('sec_checks')} note={t('checks_note')}>
              <ul className="bl">
                {checks.map((k, i) => (
                  <li key={i}>
                    <code>{k.check}</code>
                    {k.at ? <span className="fw"> · {inline(t('check_at', { at: `\`${k.at}\`` }))}</span> : null}
                    <div className="fd">{f.compiled.engine.checks[k.check] ?? ''}</div>
                  </li>
                ))}
              </ul>
            </Section>
          ) : null}
          {always.length > 0 ? (
            <Section title={t('sec_always')} note={t('always_note')}>
              <ul className="bl">
                {always.map((a, i) => (
                  <li key={i}>
                    {a.view
                      ? inline(t('always_view', { at: `\`${a.at}\``, view: `\`${a.view}\`` }))
                      : inline(t('always_action', { at: `\`${a.at}\``, action: `\`${a.do ?? ''}\`` }))}
                    {a.do ? <div className="fd">{f.compiled.engine.actions[a.do] ?? ''}</div> : null}
                  </li>
                ))}
              </ul>
            </Section>
          ) : null}
          {handoff.length > 0 ? (
            <Section title={t('sec_handoff')} note={t('handoff_note')}>
              <ul className="bl">
                {handoff.map((h) => (
                  <li key={h}>
                    <code>{h}</code>
                  </li>
                ))}
              </ul>
            </Section>
          ) : null}
        </>
      ) : null}
    </>
  );
}

/// The words under a cell's title, with how its line's session is kept, as the flow's view of the graph says it.
export function cellWho(view: GraphView, c: CompiledCell): ReactNode {
  return who(c, view.lines);
}
