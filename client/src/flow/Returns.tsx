// What a cell hands back, as its panel's second section shows it. For an agent's cell: its own answer, each row it
// takes with where it leads, the main path and what it records; the fields of its form as a tree of titles, each
// opening to what the agent is told it is and what reads it, and the note every field shown on the item is told,
// given once; then the forms every agent cell may hand in, a line each with where it leads; the fields every return
// holds, folded; the checks on the return; and the engine's fallback, folded. For an engine's step, a tool or the
// owner: where each of its rows leads, what it hands back and what the engine does every time.

import { Label } from '@primer/react';
import { Fragment, type ReactNode } from 'react';

import type { Atom, CompiledCell, CompiledGraph, CompiledRow, Field, FieldKind, Use } from '../api';
import { actionWords, fallback, mainPath, type Move, moves, rowActions, rowField } from '../graph/flowModel';
import { words } from '../labelled';
import { titleOf } from '../round';
import { type Key, t } from '../text';
import { FoldPart, joined, Name, Over, Pairs, Part, Row, Rows, Sheet } from '../voices';
import { useFlow } from './context';
import { CellCode, conditionsWords, inline, sheetText, Target } from './words';

const KIND: Record<FieldKind, Key> = {
  text: 'kind_text',
  integer: 'kind_integer',
  number: 'kind_number',
  boolean: 'kind_boolean',
  enum: 'kind_enum',
  object: 'kind_object',
  list: 'kind_list',
  union: 'kind_union',
  any: 'kind_any',
};

export { titleOf } from '../round';

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
/// others, by name; and the moves the legend gives most cells that could make them, which this one does not.
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

/// What the engine does every time `here`'s return holds a field, by the field's uses.
export function fieldActions(uses: Use[], here: string): string[] {
  return uses.flatMap((u) => (u.kind === 'always' && u.cell === here && u.action != null ? [u.action] : []));
}

/// The note every field shown on the item is told after its own description, which the flow adds to each: the
/// sentence the descriptions of the fields shown on the items of the graph's cells all end with; none when fewer
/// than two say it.
export function shownNote(g: CompiledGraph, descOf: (cell: string, path: string) => string | null): string | null {
  const descs: string[] = [];
  for (const c of g.cells) {
    if (c.kind !== 'agent') continue;
    for (const p of c.shown ?? []) {
      const d = descOf(c.id, p);
      if (d !== null) descs.push(d);
    }
  }
  if (descs.length < 2) return null;
  let common = descs[0];
  for (const d of descs.slice(1)) {
    while (common !== '' && !d.endsWith(common)) common = common.slice(1);
  }
  // From the first sentence that starts in it.
  const at = common.search(/(?<=^|[.!?] )[A-Z]/);
  const note = at >= 0 ? common.slice(at).trim() : '';
  return note.length >= 20 ? note : null;
}

/// A field's description without the note every field shown on the item is told.
function own(desc: string | null, note: string | null): string | null {
  if (desc === null || note === null || !desc.endsWith(note)) return desc;
  return desc.slice(0, desc.length - note.length).trim().replace(/\.$/, '') || null;
}

/// What a field's facts say of it: its type, whether it may be left out and its fewest items, whether the item shows
/// it, what reads it, and what the engine does every time the return holds it.
function FieldFacts({ f, here, shown }: { f: Field; here: string; shown: boolean }) {
  const engine = useFlow().compiled.engine;
  const kind = f.kind === 'list' && f.item !== null && f.item !== 'object' && f.item !== 'union' ? t('kind_list_of', { item: t(KIND[f.item]) }) : t(KIND[f.kind]);
  const uses = [...f.used_by, ...f.disc_used_by];
  const cells = [...new Set(uses.flatMap((u) => (u.kind === 'input' || (u.kind === 'condition' && u.cell !== here) ? [u.cell] : [])))];
  const roles = [...new Set(uses.flatMap((u) => (u.kind === 'role' ? [u.role] : [])))];
  const checks = [...new Set(uses.flatMap((u) => (u.kind === 'check' ? [u.check] : [])))];
  const actions = fieldActions(uses, here);
  const handoff = uses.some((u) => u.kind === 'handoff');
  return (
    <>
      {joined([
        kind,
        f.optional ? t('field_optional') : null,
        f.min !== null && f.min > 0 ? t('field_min', { n: f.min }) : null,
        shown || uses.some((u) => u.kind === 'shown') ? t('field_shown') : null,
        cells.length > 0 ? (
          <span key="cells">
            {t('field_read_by')}{' '}
            {cells.map((c, i) => (
              <Fragment key={c}>
                {i > 0 ? ', ' : null}
                <CellCode id={c} />
              </Fragment>
            ))}
          </span>
        ) : null,
        ...roles.map((r) => t('field_read_by_role', { role: r })),
        handoff ? t('use_handoff') : null,
        ...actions.map((a) => <span key={a}>{inline(engine.actions[a] ?? actionWords(a))}</span>),
        ...checks.map((k) => (
          <span key={k} title={engine.checks[k]}>
            {t('field_checked', { check: k })}
          </span>
        )),
      ])}
    </>
  );
}

/// Fields as a tree of titles, each opening to what the agent is told it is (or, for a step no agent runs, what the
/// flow says it is), its values, and what it holds.
function FieldTree({ fs, c, agent, note }: { fs: Field[]; c: CompiledCell; agent: boolean; note: string | null }) {
  return (
    <Rows>
      {fs.map((f) => (
        <FieldRow key={f.path} f={f} c={c} agent={agent} note={note} />
      ))}
    </Rows>
  );
}

/// What a field is, as said to its agent: in the sheet of what is sent; a step's own in the page's words.
function Told({ text, agent }: { text: string | null; agent: boolean }) {
  if (text === null || text === '') return null;
  if (!agent) return <p className="pp-note">{inline(text)}</p>;
  return (
    <>
      <Over>{t('told_field')}</Over>
      <Sheet>{sheetText(text)}</Sheet>
    </>
  );
}

function FieldRow({ f, c, agent, note }: { f: Field; c: CompiledCell; agent: boolean; note: string | null }) {
  const shown = (c.shown ?? []).includes(f.path);
  const desc = shown ? own(f.desc, note) : f.desc;
  const values = f.values.length > 0 && (
    <Sheet>
      {f.values.map(([v, means]) => (
        <p key={v}>
          <Name>{v}</Name>
          {means ? <>: {sheetText(means)}</> : null}
        </p>
      ))}
    </Sheet>
  );
  return (
    <Row name={titleOf(c.titles, f.path, f.name)} code={f.name} facts={<FieldFacts f={f} here={c.id} shown={shown} />}>
      <Told text={desc} agent={agent} />
      {values}
      {f.fields.length > 0 ? <FieldTree fs={f.fields} c={c} agent={agent} note={note} /> : null}
      {f.common.length > 0 ? <FieldTree fs={f.common} c={c} agent={agent} note={note} /> : null}
      {f.branches.length > 0 ? (
        <Rows>
          {f.branches.map((b) => (
            <Row
              key={b.path}
              name={
                <>
                  <Name>{b.value}</Name>
                  {f.disc ? <span className="pp-facts"> {t('branch_of', { disc: f.disc })}</span> : null}
                </>
              }
              facts={joined(b.fields.map((x) => titleOf(c.titles, x.path, x.name)))}
            >
              <Told text={b.desc} agent={agent} />
              {b.fields.length > 0 ? <FieldTree fs={b.fields} c={c} agent={agent} note={note} /> : <p className="pp-note">{t('shape_empty')}</p>}
            </Row>
          ))}
        </Rows>
      ) : null}
    </Row>
  );
}

/// Where a row leads and what it records, counts, waits for and carries out, and its condition where it says more
/// than its name.
function RowLeads({ c, i, counter }: { c: CompiledCell; i: number; counter: boolean }) {
  const f = useFlow();
  const r = c.rows[i];
  const known = new Set(Object.keys(f.compiled.engine.values));
  const cond = telling(r);
  const sets = Object.entries(r.sets ?? {});
  const counted = counter ? (f.g.counters.find((k) => k.name === r.counts)?.desc ?? '') : '';
  return (
    <span>
      <span className="pp-arrow">→</span> <Target c={c} row={i} />
      {mainPath(r) ? (
        <>
          {' '}
          <Label variant="secondary">{t('row_main')}</Label>
        </>
      ) : null}{' '}
      <span className="pp-facts">
        {joined([
          cond.length > 0 ? inline(t('row_when', { when: conditionsWords(cond, known) })) : null,
          sets.length > 0 ? inline(t('row_records_words', { what: sets.map(([k, v]) => `\`${k} = ${v}\``).join(', ') })) : null,
          r.counts ? inline(t('row_counts_words', { counter: `\`${r.counts}\`` }) + (counted !== '' ? `: ${counted}` : '')) : null,
          r.waits ? t('row_waits') : null,
          r.pauses ? t('row_pauses') : null,
          r.interject ? t('row_interjection') : null,
          ...rowActions(r).map((a) => (
            <span key={a} title={f.compiled.engine.actions[a] ?? ''}>
              {actionWords(a)}
            </span>
          )),
          r.note ? inline(r.note) : null,
        ])}
      </span>
    </span>
  );
}

/// The rows of the cell's own answer, or of a step, each with where it leads.
function Leads({ c, rows }: { c: CompiledCell; rows: number[] }) {
  const counter = counterFirst(c, rows);
  return <Pairs rows={rows.map((i) => [<Name key="n">{c.rows[i].name}</Name>, <RowLeads key="l" c={c} i={i} counter={counter.has(i)} />])} />;
}

/// The cells a set of rows leads to, each by its code, joined with "or".
function Targets({ c, rows }: { c: CompiledCell; rows: number[] }) {
  const f = useFlow();
  const cells = [...new Set(rows.flatMap((i) => f.cellView(c.id)?.targets[i] ?? [c.rows[i].to]))].filter((x) => !x.startsWith('@'));
  return (
    <span>
      <span className="pp-arrow">→</span>{' '}
      {cells.length === 1 ? (
        <CellTitled id={cells[0]} />
      ) : (
        cells.map((x, i) => (
          <Fragment key={x}>
            {i > 0 ? (i === cells.length - 1 ? ` ${t('or_word')} ` : ', ') : null}
            <CellCode id={x} />
          </Fragment>
        ))
      )}
    </span>
  );
}

function CellTitled({ id }: { id: string }) {
  const f = useFlow();
  return (
    <>
      <CellCode id={id} /> {f.cell(id)?.title ?? ''}
    </>
  );
}

/// The forms every agent cell may hand in besides its own answer, a line each with where its rows lead.
function OtherForms({ c }: { c: CompiledCell }) {
  const f = useFlow();
  const forms = (c.forms ?? []).filter((x) => !x.own && x.field != null);
  const moves = leavesTo(c).moves;
  const rows: Array<[ReactNode, ReactNode]> = forms.map((form) => {
    const field = form.field ?? '';
    const its = moves.filter((i) => rowField(c.rows[i]) === field);
    const w = words(field);
    return [
      <span key="n">
        {w.charAt(0).toUpperCase() + w.slice(1)}
        {field === 'tools' && (c.tools ?? []).length > 0 ? <span className="pp-facts"> {(c.tools ?? []).join(', ')}</span> : null}
      </span>,
      its.length > 0 ? <Targets key="t" c={c} rows={its} /> : <span key="t" />,
    ];
  });
  // A move that turns on no form of the return, such as one the engine takes for the cell's line.
  const formless = moves.filter((i) => {
    const field = rowField(c.rows[i]);
    return field === null || !forms.some((x) => x.field === field);
  });
  for (const i of formless) rows.push([<Name key="n">{c.rows[i].name}</Name>, <Targets key="t" c={c} rows={[i]} />]);
  // The moves the legend gives most cells that could make them, which this one does not.
  const lacks = ownMoves(f.g, c).lacks;
  if (rows.length === 0 && lacks.length === 0) return null;
  return (
    <Part title={t('back_forms')} facts={t('back_forms_facts')}>
      {rows.length > 0 ? <Pairs rows={rows} /> : null}
      {lacks.map((m) => (
        <p key={m.row} className="pp-note">
          {t(m.engine ? 'leaves_lacks_engine' : 'chart_lacks_title', { cell: c.id, move: actionWords(m.row), to: m.to })}
        </p>
      ))}
    </Part>
  );
}

/// What an agent's cell hands back.
export function AgentReturns({ c }: { c: CompiledCell }) {
  const f = useFlow();
  const r = f.cellView(c.id)?.returns;
  const out = leavesTo(c);
  const ownFields = r?.forms.find((x) => x.field === null)?.fields ?? [];
  const descOf = (cell: string, path: string) => {
    const find = (fs: Field[]): Field | undefined => {
      for (const x of fs) {
        if (x.path === path) return x;
        const hit = find([...x.fields, ...x.common, ...x.branches.flatMap((b) => b.fields)]);
        if (hit !== undefined) return hit;
      }
      return undefined;
    };
    const v = f.cellView(cell)?.returns;
    return find([...(v?.forms.flatMap((x) => x.fields) ?? []), ...(v?.every ?? [])])?.desc ?? null;
  };
  const note = shownNote(f.g, descOf);
  const anyShown = (c.shown ?? []).length > 0 && note !== null;
  const checks = c.checks ?? [];
  return (
    <>
      <Part title={t('back_own')}>
        {out.answers.length > 0 ? <Leads c={c} rows={out.answers} /> : null}
        {ownFields.length > 0 ? <FieldTree fs={ownFields} c={c} agent note={note} /> : null}
        {anyShown ? (
          <>
            <Over>{t('back_shown_note')}</Over>
            <Sheet>{sheetText(note)}</Sheet>
          </>
        ) : null}
      </Part>
      <OtherForms c={c} />
      {r !== undefined && r.every.length > 0 ? (
        <FoldPart title={t('back_every')} n={r.every.length} facts={t('back_every_facts')}>
          <FieldTree fs={r.every} c={c} agent note={note} />
        </FoldPart>
      ) : null}
      {checks.length > 0 ? (
        <Part title={t('back_checks')}>
          <Pairs
            rows={checks.map((k) => [
              <Name key="n">{k.check}</Name>,
              <span key="w">
                {inline(f.compiled.engine.checks[k.check] ?? '')}
                {k.at ? <span className="pp-facts"> {inline(t('back_check_at', { at: `\`${k.at}\`` }))}</span> : null}
              </span>,
            ])}
          />
          <p className="pp-note">{t('checks_note')}</p>
        </Part>
      ) : null}
      {out.fallback.length > 0 ? (
        <FoldPart title={t('back_fallback')} n={out.fallback.length} facts={t('back_fallback_facts')}>
          <Pairs
            rows={out.fallback.map((i) => [
              <Name key="n">{c.rows[i].name}</Name>,
              <span key="w">{inline(fallbackWords(f.g, c.rows[i]) ?? conditionsWords(c.rows[i].if, new Set()))}</span>,
            ])}
          />
        </FoldPart>
      ) : null}
    </>
  );
}

/// What a step that is no agent's round comes to: where each of its rows leads, what it hands back, and what the
/// engine does every time.
export function StepReturns({ c }: { c: CompiledCell }) {
  const f = useFlow();
  const r = f.cellView(c.id)?.returns;
  const rows = c.rows.map((_, i) => i);
  const does = c.actions ?? [];
  return (
    <>
      <Part title={t('back_rows')}>{rows.length > 0 ? <Leads c={c} rows={rows} /> : <p className="pp-note">{t('leaves_none')}</p>}</Part>
      {r !== undefined && r.every.length > 0 ? (
        <Part title={t('back_fields')}>
          <FieldTree fs={r.every} c={c} agent={c.kind === 'tool'} note={null} />
        </Part>
      ) : null}
      {does.length > 0 ? (
        <Part title={t('leaves_does')}>
          <Pairs rows={does.map((a) => [<Name key="n">{actionWords(a)}</Name>, <span key="w">{inline(f.compiled.engine.actions[a] ?? '')}</span>])} />
        </Part>
      ) : null}
    </>
  );
}
