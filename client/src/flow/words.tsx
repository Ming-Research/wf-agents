// The flow written out in words: a row's condition and target, where an input comes from, what reads a field, each
// cell by its id and title, a link that opens it. Names the flow gives (fields, values, views, actions) are set in
// code type; what they are is said in words.

import { Fragment, type ReactNode } from 'react';

import type { Atom, CompiledCell, Src, Use } from '../api';
import { circled } from '../graph/flowModel';
import { type Key, t } from '../text';
import { useFlow } from './context';

/// A text with names between backticks set in code type.
export function inline(text: string): ReactNode {
  const parts = text.split('`');
  return parts.map((p, i) => (i % 2 === 1 ? <code key={i}>{p}</code> : <Fragment key={i}>{p}</Fragment>));
}

/// Values, as `a`, `b` or `c`.
export function valuesWords(vs: string[]): string {
  const q = vs.map((v) => `\`${v}\``);
  if (q.length <= 1) return q.join('');
  return t('words_or', { list: q.slice(0, -1).join(', '), last: q[q.length - 1] });
}

/// A row's condition in words. `known` names the values the engine itself knows; any other value a condition reads
/// by name is one rows recorded.
export function conditionWords(a: Atom, known: Set<string>): string {
  switch (a[0]) {
    case 'all':
      return t('cond_all');
    case 'own':
      return t('cond_own');
    case 'or':
      return a[1].map((x) => conditionWords(x, known)).join(t('cond_or'));
    case 'has':
      return t('cond_has', { path: `\`${a[1]}\`` });
    case 'is':
      return t('cond_is', { path: `\`${a[1]}\``, values: valuesWords(a[2]) });
    case 'any':
      return t('cond_any', { path: `\`${a[1]}\``, values: valuesWords(a[2]) });
    case 'none':
      return t('cond_none', { path: `\`${a[1]}\``, values: valuesWords(a[2]) });
    case 'inp':
      return t(known.has(a[1]) ? 'cond_engine' : 'cond_recorded', { name: `\`${a[1]}\``, values: valuesWords(a[2]) });
  }
}

/// A row's conditions, all of which hold for it to be taken.
export function conditionsWords(atoms: Atom[], known: Set<string>): string {
  return atoms.map((a) => conditionWords(a, known)).join(t('cond_and'));
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

/// A cell, by its id and title, that opens its panel.
export function CellLink({ id, extra }: { id: string; extra?: string }) {
  const f = useFlow();
  const c = f.cell(id);
  return (
    <button type="button" className="go" onClick={() => f.open({ kind: 'cell', cell: id })}>
      <strong>{id}</strong> {c?.title ?? ''}
      {extra ? <span className="go-extra"> · {extra}</span> : null}
    </button>
  );
}

/// Cells, each a link, joined.
export function CellLinks({ ids }: { ids: string[] }) {
  return (
    <span className="links">
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
          <CellLinks ids={cells} />
        </>
      ) : null}
    </span>
  );
}

/// The rows of a table or view an input reads: those of the work, item or project at hand.
const BY: Record<string, Key> = { work: 'src_by_work', item: 'src_by_item', project: 'src_by_project' };

/// Where one source of an input reads from, as a chip; a place of a cell opens that cell.
export function SourceChip({ src, cells }: { src: Src; cells: string[] }) {
  const f = useFlow();
  if (Array.isArray(src)) {
    const [at, path] = src;
    if (!at.startsWith('@')) {
      return (
        <span className="src">
          <CellLink id={at} extra={path} />
        </span>
      );
    }
    const who = at === '@from' ? t('src_from') : at === '@caller' ? t('src_caller') : t('src_stopped');
    return (
      <span className="src">
        {who} · <code>{path}</code>
        {cells.length > 0 ? (
          <>
            {' ('}
            <CellLinks ids={cells} />
            {')'}
          </>
        ) : null}
      </span>
    );
  }
  if (src === 'self') return <span className="src plain">{t('src_self')}</span>;
  if ('config' in src) return <span className="src plain">{inline(t('src_config', { name: `\`${src.config}\`` }))}</span>;
  if ('db' in src) {
    const key = src.by ? BY[src.by] : undefined;
    const by = key ? t(key) : src.by ? t('src_by', { column: src.by }) : null;
    return (
      <span className="src plain">
        {inline(t('src_db', { name: `\`${src.db}\`` }))}
        {by ? `, ${by}` : ''}
      </span>
    );
  }
  if ('engine' in src) {
    const desc = f.compiled.engine.inputs[src.engine];
    return (
      <span className="src plain" title={desc}>
        {inline(t('src_engine', { name: `\`${src.engine}\`` }))}
      </span>
    );
  }
  return <span className="src plain">{inline(t('src_recorded', { name: `\`${src.recorded}\`` }))}</span>;
}

/// What reads a field, as a chip; `here` is the cell whose field it is.
export function UseChip({ u, here }: { u: Use; here: string }) {
  const f = useFlow();
  const of = (cell: string, what: ReactNode) =>
    cell === here ? (
      what
    ) : (
      <>
        <CellLink id={cell} />
        {': '}
        {what}
      </>
    );
  switch (u.kind) {
    case 'input': {
      const what = f.cell(u.cell)?.inputs[u.input]?.what ?? '';
      return (
        <span className="use" title={what}>
          {t('use_input')} <CellLink id={u.cell} />
        </span>
      );
    }
    case 'condition':
      return <span className="use">{of(u.cell, t('use_condition', { no: circled(u.row) }))}</span>;
    case 'action':
      return (
        <span className="use" title={f.compiled.engine.actions[u.action]}>
          {of(u.cell, inline(t('use_action', { no: circled(u.row), action: `\`${u.action}\`` })))}
        </span>
      );
    case 'always':
      return (
        <span className="use" title={u.action ? f.compiled.engine.actions[u.action] : undefined}>
          {of(
            u.cell,
            inline(
              u.view
                ? t('use_view', { view: `\`${u.view}\`` })
                : t('use_always_action', { action: `\`${u.action ?? ''}\`` }),
            ),
          )}
        </span>
      );
    case 'check':
      return (
        <span className="use" title={f.compiled.engine.checks[u.check]}>
          {of(u.cell, inline(t('use_check', { check: `\`${u.check}\`` })))}
        </span>
      );
    case 'handoff':
      return <span className="use">{of(u.cell, t('use_handoff'))}</span>;
    case 'shown':
      return <span className="use">{of(u.cell, t('use_shown'))}</span>;
  }
}

/// The uses of a field, after a label.
export function Uses({ uses, here, label }: { uses: Use[]; here: string; label?: string }) {
  if (uses.length === 0) return null;
  return (
    <div className="uses">
      <span className="ul">{label ?? t('used_by')}</span>
      {uses.map((u, i) => (
        <UseChip key={i} u={u} here={here} />
      ))}
    </div>
  );
}
