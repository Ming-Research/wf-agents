// What changed from one flow to another: in a cell, each input, rule, prompt line, form, return field, row, check,
// action done every time and handoff added, removed or changed, with what it was and what it is; in the whole, every
// cell added, removed or changed, and what changed in each graph apart from its cells.

import type { CellDiff, Change, CompiledInput, CompiledRow, FlowDiff, JsonValue, Part } from '../api';
import { type Key, srcText, t } from '../text';
import { useFlow } from './context';
import { CellLink, conditionsWords, inline, targetWords } from './words';

const PART: Record<Part, Key> = {
  title: 'part_title',
  kind: 'part_kind',
  gate: 'part_gate',
  line: 'part_line',
  stage: 'part_stage',
  step: 'part_step',
  needs: 'part_needs',
  input: 'part_input',
  rule: 'part_rule',
  prompt: 'part_prompt',
  prompt_by: 'part_prompt_by',
  forms: 'part_forms',
  field: 'part_field',
  row: 'part_row',
  row_order: 'part_row_order',
  check: 'part_check',
  always: 'part_always',
  handoff: 'part_handoff',
  shown: 'part_shown',
  inenum: 'part_inenum',
  start: 'part_start',
  end: 'part_end',
  asking: 'part_asking',
  receive: 'part_receive',
  counter: 'part_counter',
  interjection: 'part_interjection',
  rounds: 'part_rounds',
};

function lines(v: JsonValue | null): string[] {
  if (v === null) return [];
  if (Array.isArray(v)) return v.map((x) => (typeof x === 'string' ? x : JSON.stringify(x)));
  if (typeof v === 'string') return [v];
  return [JSON.stringify(v, null, 1)];
}

function rowWords(r: CompiledRow, known: Set<string>): string {
  const to = targetWords(r.to) ?? `\`${r.to}\``;
  return t('change_row', { if: conditionsWords(r.if, known), to });
}

function inputWords(x: CompiledInput): string {
  return `${x.what} (${x.from.map(srcText).join('; ')})${x.maybe ? `, ${t('input_maybe')}` : ''}`;
}

/// One side of a change in words, as far as the part allows.
function side(part: Part, v: JsonValue | null, known: Set<string>): string[] {
  if (v === null) return [];
  if (part === 'row') return [rowWords(v as unknown as CompiledRow, known)];
  if (part === 'input') return [inputWords(v as unknown as CompiledInput)];
  const o = typeof v === 'object' && !Array.isArray(v) ? (v as { [k: string]: JsonValue }) : null;
  if (o && part === 'field') {
    const kind = String(o.kind ?? '');
    const values = Array.isArray(o.values) && o.values.length > 0 ? ` (${o.values.map((x) => (Array.isArray(x) ? x[0] : x)).join(', ')})` : '';
    return [`${kind}${o.optional ? ', optional' : ''}${values}: ${String(o.desc ?? '')}`];
  }
  if (o && part === 'always') return [`\`${String(o.at)}\` → \`${String(o.do ?? o.view ?? '')}\``];
  if (o && part === 'check') return [`\`${String(o.check)}\`${o.at ? ` on \`${String(o.at)}\`` : ''}`];
  if (o && part === 'rule' && Array.isArray(o.text)) return [String(o.desc ?? ''), ...o.text.map(String)];
  if (o && part === 'counter') return [String(o.desc ?? '')];
  if (part === 'prompt' || part === 'handoff' || part === 'shown' || part === 'row_order' || part === 'asking') return lines(v);
  if (typeof v === 'string' || typeof v === 'number' || typeof v === 'boolean') return [String(v)];
  return [JSON.stringify(v, null, 1)];
}

/// One side of a change; a long one is folded, with its first line in sight.
function Side({ lines: ls, cls, mark }: { lines: string[]; cls: string; mark: string }) {
  if (ls.length === 0) return null;
  const long = ls.join('\n').length > 400;
  const body = (
    <div className={cls}>
      {ls.map((l, i) => (
        <div key={i}>
          {mark} {inline(l)}
        </div>
      ))}
    </div>
  );
  if (!long) return body;
  return (
    <details className="chg-long">
      <summary>
        {mark} {ls[0].slice(0, 120)}…
      </summary>
      {body}
    </details>
  );
}

function ChangeLine({ c }: { c: Change }) {
  const f = useFlow();
  const known = new Set(Object.keys(f.compiled.engine.values));
  const kind = c.before === null ? 'added' : c.after === null ? 'removed' : 'changed';
  const before = side(c.part, c.before, known);
  const after = side(c.part, c.after, known);
  // A prompt is shown by the lines taken out and put in.
  const shown =
    c.part === 'prompt' && kind === 'changed'
      ? { before: before.filter((l) => !after.includes(l)), after: after.filter((l) => !before.includes(l)) }
      : { before, after };
  return (
    <li className={`chg chg-${kind}`}>
      <div>
        <span className={`tag tag-${kind}`}>{t(`change_${kind}`)}</span> <strong>{t(PART[c.part])}</strong>
        {c.key !== '' ? <span> {inline(`\`${c.key}\``)}</span> : null}
      </div>
      <Side lines={shown.before} cls="chg-before" mark="−" />
      <Side lines={shown.after} cls="chg-after" mark="+" />
    </li>
  );
}

/// What changed in one cell.
export function Changes({ d }: { d: CellDiff }) {
  if (d.status !== 'changed') return <p className="fmt">{t(d.status === 'added' ? 'cell_added' : 'cell_removed')}</p>;
  return (
    <ul className="bl chg-list">
      {d.changes.map((c, i) => (
        <ChangeLine key={i} c={c} />
      ))}
    </ul>
  );
}

/// Every change from one flow to another, for the graph shown.
export function DiffPanel({ diff, graph }: { diff: FlowDiff; graph: string }) {
  const f = useFlow();
  const g = diff.graphs.find((x) => x.name === graph);
  if (!g) return <p className="fmt">{t('diff_none')}</p>;
  return (
    <>
      {g.changes.length > 0 ? (
        <section className="ds">
          <h4>{t('diff_graph')}</h4>
          <ul className="bl chg-list">
            {g.changes.map((c, i) => (
              <ChangeLine key={i} c={c} />
            ))}
          </ul>
        </section>
      ) : null}
      <section className="ds">
        <h4>{t('diff_cells')}</h4>
        {g.cells.length === 0 ? <p className="fmt">{t('diff_no_cells')}</p> : null}
        <ul className="bl">
          {g.cells.map((c) => (
            <li key={c.id}>
              <span className={`tag tag-${c.status}`}>{t(`change_${c.status}`)}</span>{' '}
              {f.cell(c.id) ? (
                <CellLink id={c.id} />
              ) : (
                <span>
                  <strong>{c.id}</strong> {c.title}
                </span>
              )}
              {c.status === 'changed' ? (
                <span className="fw">
                  {' '}
                  · {[...new Set(c.changes.map((x) => t(PART[x.part])))].join(', ')}
                </span>
              ) : null}
            </li>
          ))}
        </ul>
      </section>
    </>
  );
}
