// The flow written out in words: a row's condition and target, each cell by its code and title or by its code alone,
// a link that opens it. Names the flow gives (fields, values, views, actions) are set as codes, in the colour of the
// text round them; what they are is said in words.

import { Fragment, type ReactNode } from 'react';

import type { Atom, CompiledCell } from '../api';
import { LinkButton } from '../parts';
import { placeSchema } from '../round';
import { t, tn } from '../text';
import { useFlow } from './context';

/// The page's words with names between backticks set as codes: monospace, in the colour of the text round them.
export function inline(text: string): ReactNode {
  const parts = text.split('`');
  return parts.map((p, i) =>
    i % 2 === 1 ? (
      <span key={i} className="cell-id">
        {p}
      </span>
    ) : (
      <Fragment key={i}>{p}</Fragment>
    ),
  );
}

/// A text sent to an agent, as markdown reads it: what is between backticks in monospace, the backticks themselves
/// being markdown's.
export function sheetText(text: string): ReactNode {
  return <span>{inline(text)}</span>;
}

/// Values, as `a`, `b` or `c`.
export function valuesWords(vs: string[]): string {
  const q = vs.map((v) => `\`${v}\``);
  if (q.length <= 1) return q.join('');
  return t('words_or', { list: q.slice(0, -1).join(', '), last: q[q.length - 1] });
}

/// How a row's condition names a place of its cell's return, as the return holds it: an agent's form and the fields of
/// its own answer sit in the field that holds one form (`outcome`), which the engine reads them out of, so the row's
/// `take.way` is the return's `outcome.take.way`; a field every return holds, a step's field and what the cell read by
/// its id (`@…`) as the row names them.
export function placeIn(c: CompiledCell, within: string): (path: string) => string {
  const inner = c.kind === 'agent' ? placeSchema(c.schema, within) : null;
  return (p) => {
    if (inner === null || p.startsWith('@')) return p;
    return placeSchema(inner, p.split('.')[0]) !== null ? `${within}.${p}` : p;
  };
}

/// A row's condition in words. `known` names the values the engine itself knows; any other value a condition reads
/// by name is one rows recorded. `at` writes a place of the return as the return holds it ([`placeIn`]).
export function conditionWords(a: Atom, known: Set<string>, at: (path: string) => string = (p) => p): string {
  switch (a[0]) {
    case 'all':
      return t('cond_all');
    case 'own':
      return t('cond_own');
    case 'or':
      return a[1].map((x) => conditionWords(x, known, at)).join(t('cond_or'));
    case 'has':
      return t('cond_has', { path: `\`${at(a[1])}\`` });
    case 'is':
      return t('cond_is', { path: `\`${at(a[1])}\``, values: valuesWords(a[2]) });
    case 'any':
      return t('cond_any', { path: `\`${at(a[1])}\``, values: valuesWords(a[2]) });
    case 'none':
      return t('cond_none', { path: `\`${at(a[1])}\``, values: valuesWords(a[2]) });
    case 'inp':
      return t(known.has(a[1]) ? 'cond_engine' : 'cond_recorded', { name: `\`${a[1]}\``, values: valuesWords(a[2]) });
    case 'sent_back':
      return a.length === 2 ? tn('cond_sent_back_n', a[1]) : t('cond_sent_back');
    case 'failed':
      return t('cond_failed', { kind: `\`${a[1]}\`` });
    case 'engine':
      return t('cond_engine_case', { kind: `\`${a[1]}\`` });
  }
}

/// A row's conditions, all of which hold for it to be taken.
export function conditionsWords(atoms: Atom[], known: Set<string>, at?: (path: string) => string): string {
  return atoms.map((a) => conditionWords(a, known, at)).join(t('cond_and'));
}

/// The words for a target the engine keeps track of, or none for a cell.
export function targetWords(to: string): string | null {
  switch (to) {
    case '@self':
      return t('to_self');
    case '@caller':
      return t('to_caller');
    case '@asker':
      return t('to_asker');
    case '@stopped':
      return t('to_stopped');
    default:
      return null;
  }
}

/// A cell, by its code and title, that opens its panel.
export function CellLink({ id, extra }: { id: string; extra?: string }) {
  const f = useFlow();
  const c = f.cell(id);
  return (
    <LinkButton onClick={() => f.open({ kind: 'cell', cell: id })}>
      <span className="cell-id">{id}</span> {c?.title ?? ''}
      {extra ? ` · ${extra}` : null}
    </LinkButton>
  );
}

/// A cell by its code alone, that opens its panel, its title on hovering: for cells named many at once.
export function CellCode({ id }: { id: string }) {
  const f = useFlow();
  return (
    <LinkButton title={f.cell(id)?.title} onClick={() => f.open({ kind: 'cell', cell: id })}>
      <span className="cell-id">{id}</span>
    </LinkButton>
  );
}

/// Cells, each a link, joined.
export function CellLinks({ ids }: { ids: string[] }) {
  return (
    <span>
      {ids.map((id, i) => (
        <Fragment key={id}>
          {i > 0 ? ', ' : null}
          <CellLink id={id} />
        </Fragment>
      ))}
    </span>
  );
}

/// Where a row leads: a cell, or a target the engine keeps track of with the cells it can be.
export function Target({ c, row }: { c: CompiledCell; row: number }) {
  const f = useFlow();
  const to = c.rows[row].to;
  const words = targetWords(to);
  if (words === null) return <CellLink id={to} />;
  const cells = (f.cellView(c.id)?.targets[row] ?? []).filter((x) => to !== '@self' || x !== c.id);
  return (
    <span>
      {words}
      {cells.length > 0 ? (
        <>
          {': '}
          {cells.map((x, i) => (
            <Fragment key={x}>
              {i > 0 ? ', ' : null}
              <CellCode id={x} />
            </Fragment>
          ))}
        </>
      ) : null}
    </span>
  );
}
