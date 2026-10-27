// A cell, as its panel explains it. For an agent's cell: its inputs, of two kinds, what is given every time the cell
// is entered and what is given only when the session does not hold it yet, the latter by where it comes from; what
// it hands back; its transition table; the checks on its return; what the engine does every time it returns a field;
// what it hands over; and every row that leads into it. For an engine step, a tool or the owner: what the step does,
// what it reads, which never goes into any agent's context, and the same after.

import { type ReactNode, useEffect, useRef } from 'react';

import type { CellDiff, CompiledCell, Material, Origin, RoundSummary } from '../api';
import { OutcomeBadge } from '../components';
import { circled } from '../graph/flowModel';
import { time } from '../format';
import { type Key, t } from '../text';
import { useFlow } from './context';
import { Changes } from './Changes';
import { Returns } from './Returns';
import { CellLink, conditionsWords, inline, SourceChip, Target } from './words';

export function Section({ title, note, children, id }: { title: string; note?: ReactNode; children: ReactNode; id?: string }) {
  return (
    <section className="ds" id={id}>
      <h4>{title}</h4>
      {note ? <p className="fmt">{note}</p> : null}
      {children}
    </section>
  );
}

/// The words under a cell's title: who does its work, its line and how its session runs, its stage.
export function who(c: CompiledCell, firstOfLine: boolean): string {
  const parts: string[] = [];
  if (c.kind === 'agent') {
    if (c.line === null) parts.push(t('who_agent_fresh'));
    else {
      parts.push(t('who_agent', { line: c.line }));
      parts.push(firstOfLine ? t('who_starts') : t('who_continues'));
    }
  } else {
    parts.push(t(c.kind === 'tool' ? 'who_tool' : c.kind === 'owner' ? 'who_owner' : 'who_engine'));
  }
  if (c.gate) parts.push(t('who_gate'));
  if (c.stage) parts.push(t('who_stage', { stage: c.stage }));
  return parts.join(' · ');
}

const ORIGINS: Array<[Origin, Key, Key]> = [
  ['line', 'origin_line', 'origin_line_note'],
  ['start', 'origin_start', 'origin_start_note'],
  ['outside', 'origin_outside', 'origin_outside_note'],
  ['either', 'origin_either', 'origin_either_note'],
];

/// One material: what it is, where it comes from and its address.
function MaterialLine({ c, m, rules }: { c: CompiledCell; m: Material; rules: boolean }) {
  const f = useFlow();
  const addr = m.address ? <code className="addr">{m.address}</code> : null;
  switch (m.of.kind) {
    case 'input': {
      const x = c.inputs[m.of.input];
      const src = x?.from[m.of.source];
      if (!x || src === undefined) return null;
      return (
        <li>
          {x.what}
          {x.maybe ? <span className="fw"> · {t('input_maybe')}</span> : null}
          <div className="srcs">
            <span className="ul">{t('input_source')}</span>
            <SourceChip src={src} cells={m.cells} />
            {addr}
          </div>
        </li>
      );
    }
    case 'rule': {
      const name = m.of.name;
      const rule = f.g.rules.find((r) => r.name === name);
      return (
        <li>
          <button type="button" className="go" onClick={() => f.open({ kind: 'rule', rule: name })}>
            {t('mat_rule', { name })}
          </button>
          {rule ? <span className="fw"> · {rule.desc}</span> : null}
          <div className="srcs">{addr}</div>
          {rules && rule ? (
            <details className="rule-text">
              <summary>{t('rule_full_text')}</summary>
              <ul className="bl">
                {rule.text.map((line, i) => (
                  <li key={i}>{inline(line)}</li>
                ))}
              </ul>
            </details>
          ) : null}
        </li>
      );
    }
    default: {
      const key: Record<string, Key> = {
        project_rules: 'mat_project_rules',
        settings: 'mat_settings',
        tables: 'mat_tables',
        common_format: 'mat_common_format',
        cell_format: 'mat_cell_format',
      };
      return (
        <li>
          {t(key[m.of.kind] ?? 'mat_common_format', { cell: c.id })}
          <div className="srcs">{addr}</div>
        </li>
      );
    }
  }
}

/// What an agent's round is given: every time, and once in a session by where it comes from.
function AgentInputs({ c }: { c: CompiledCell }) {
  const f = useFlow();
  const view = f.cellView(c.id);
  const materials = view?.materials ?? [];
  const itself = materials.filter((m) => m.origin === 'itself' && m.of.kind === 'input');
  const prompt = c.prompt ?? [];
  const shared = (c.forms ?? []).filter((x) => x.field !== null).map((x) => `\`${x.field}\``);
  const enums = c.prompt_by ? (c.inenums?.[c.prompt_by.by] ?? []) : [];
  return (
    <Section title={t('sec_inputs')}>
      <div className="ig">
        <div className="il">{t('inputs_always')}</div>
        <p className="fmt">{t('inputs_always_note')}</p>
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
        {itself.length > 0 ? (
          <div className="ig">
            <div className="il">{t('inputs_itself')}</div>
            <ul className="bl">
              {itself.map((m, i) => (m.of.kind === 'input' ? <li key={i}>{c.inputs[m.of.input]?.what}</li> : null))}
            </ul>
          </div>
        ) : null}
        <ul className="bl">
          <li>{t('inputs_work_from')}</li>
          <li>
            {inline(
              shared.length > 0
                ? t('inputs_hand_in', { cell: c.id, forms: shared.join(', ') })
                : t('inputs_hand_in_own', { cell: c.id }),
            )}
          </li>
        </ul>
      </div>
      <div className="ig">
        <div className="il">{t('inputs_conditional')}</div>
        <p className="fmt">{t('inputs_conditional_note')}</p>
        {ORIGINS.map(([origin, title, note]) => {
          const list = materials.filter((m) => m.origin === origin);
          if (list.length === 0) return null;
          return (
            <div key={origin} className="origin">
              <div className="origin-h">{t(title)}</div>
              <p className="fmt">{t(note)}</p>
              <ul className="bl">
                {list.map((m, i) => (
                  <MaterialLine key={i} c={c} m={m} rules />
                ))}
              </ul>
            </div>
          );
        })}
      </div>
    </Section>
  );
}

/// What a step that is no agent's round reads.
function StepInputs({ c }: { c: CompiledCell }) {
  const f = useFlow();
  const materials = f.cellView(c.id)?.materials ?? [];
  return (
    <Section title={t('sec_inputs')} note={t('inputs_step_note')}>
      {c.inputs.length === 0 ? <p className="fmt">{t('none')}</p> : null}
      <ul className="bl">
        {c.inputs.map((x, i) => (
          <li key={i}>
            {x.what}
            {x.maybe ? <span className="fw"> · {t('input_maybe')}</span> : null}
            <div className="srcs">
              <span className="ul">{t('input_source')}</span>
              {x.from.map((s, j) => {
                const m = materials.find((y) => y.of.kind === 'input' && y.of.input === i && y.of.source === j);
                return <SourceChip key={j} src={s} cells={m?.cells ?? []} />;
              })}
            </div>
          </li>
        ))}
      </ul>
    </Section>
  );
}

function Table({ c, highlight }: { c: CompiledCell; highlight: number[] }) {
  const f = useFlow();
  const known = new Set(Object.keys(f.compiled.engine.values));
  return (
    <ol className="tt">
      {c.rows.map((r, i) => (
        <li key={r.name} className={highlight.includes(i) ? 'tt-on' : undefined}>
          <span className="no">{circled(i)}</span>
          <div className="tb">
            <div>
              <strong>{r.name}</strong>
              {r.main ? <span className="tag tag-main">{t('row_main')}</span> : null}
              {r.common ? <span className="tag">{t('row_shared')}</span> : null}
              {r.waits ? <span className="tag">{t('row_waits')}</span> : null}
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

function RoundsHere({ rounds, onRound }: { rounds: RoundSummary[]; onRound: (r: RoundSummary) => void }) {
  if (rounds.length === 0) return <p className="fmt">{t('no_rounds')}</p>;
  return (
    <table className="list mini">
      <tbody>
        {[...rounds].reverse().map((r) => (
          <tr key={r.id} className="clickable" onClick={() => onRound(r)}>
            <td className="muted">#{r.id}</td>
            <td>
              <OutcomeBadge outcome={r.outcome} />
            </td>
            <td className="clip">{r.transition ?? r.why ?? ''}</td>
            <td className="muted">{time(r.started_at)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

export function CellPanel(props: {
  c: CompiledCell;
  /// Rows to point out, such as those of the shared circle the panel was opened from.
  rows?: number[];
  /// On a work's page, the work's rounds at this cell.
  rounds?: RoundSummary[];
  onRound?: (r: RoundSummary) => void;
  /// When two flows are compared, what changed in this cell.
  diff?: CellDiff | null;
}) {
  const { c, rows = [], rounds, onRound, diff } = props;
  const f = useFlow();
  const view = f.cellView(c.id);
  const agent = c.kind === 'agent';
  const table = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (rows.length > 0) table.current?.scrollIntoView({ block: 'start' });
  }, [rows]);
  const checks = c.checks ?? [];
  const always = c.always ?? [];
  const handoff = c.handoff ?? [];
  return (
    <>
      {rounds && onRound ? (
        <Section title={t('sec_rounds_here')}>
          <RoundsHere rounds={rounds} onRound={onRound} />
        </Section>
      ) : null}
      {diff ? (
        <Section title={t('sec_changes')}>
          <Changes d={diff} />
        </Section>
      ) : null}
      {!agent && c.step ? (
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
      {agent ? <AgentInputs c={c} /> : <StepInputs c={c} />}
      <Section title={t('sec_returns')}>
        {view ? <Returns c={c} r={view.returns} /> : null}
      </Section>
      <div ref={table}>
        <Section title={t('sec_table')}>
          <Table c={c} highlight={rows} />
        </Section>
      </div>
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
      <Section title={t('sec_entered')}>
        {(view?.entered_from ?? []).length === 0 ? <p className="fmt">{t('entered_none')}</p> : null}
        <ul className="bl">
          {(view?.entered_from ?? []).map((r) => {
            const from = f.cell(r.cell);
            const row = from?.rows[r.row];
            if (!from || !row) return null;
            return (
              <li key={`${r.cell}:${r.row}`}>
                {r.cell === c.id ? t('entered_itself') : <CellLink id={r.cell} />}{' '}
                <span className="no">{circled(r.row)}</span> <strong>{row.name}</strong>
                <div className="tx">
                  {inline(conditionsWords(row.if, new Set(Object.keys(f.compiled.engine.values))))}
                </div>
              </li>
            );
          })}
        </ul>
      </Section>
    </>
  );
}

/// The words under a cell's title, with whether it is the first cell of its line, which starts the line's session.
export function cellWho(cells: CompiledCell[], c: CompiledCell): string {
  const first = cells.find((x) => x.kind === 'agent' && x.line !== null && x.line === c.line);
  return who(c, first?.id === c.id);
}
