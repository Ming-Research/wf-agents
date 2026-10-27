// What a cell hands back, as the owner's approved page showed it: for an agent's cell, the field that holds exactly
// one form, each form it may hand in as a block of its own with "or" between them, then the fields every return
// holds; each field with its type, whether it may be left out, what it is, its values and what reads it, and the
// fields it holds folded away behind a toggle.

import { createContext, Fragment, useContext, useState } from 'react';

import type { Branch, CompiledCell, Field, FieldKind, Returns as ReturnsType } from '../api';
import { type Key, t } from '../text';
import { inline, Uses } from './words';

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

/// Which fields are unfolded, by path, and the cell whose return it is.
type Folding = { open: Set<string>; toggle: (path: string) => void; cell: string };
const FoldingContext = createContext<Folding>({ open: new Set(), toggle: () => {}, cell: '' });

function hasKids(f: Field): boolean {
  return f.fields.length > 0 || f.branches.length > 0 || f.common.length > 0;
}

/// Every path that can be unfolded.
function foldable(fs: Field[], out: string[] = []): string[] {
  for (const f of fs) {
    if (hasKids(f)) out.push(f.path);
    foldable(f.fields, out);
    foldable(f.common, out);
    for (const b of f.branches) {
      if (b.fields.length > 0) out.push(b.path);
      foldable(b.fields, out);
    }
  }
  return out;
}

function Toggle({ path }: { path: string }) {
  const { open, toggle } = useContext(FoldingContext);
  const on = open.has(path);
  return (
    <button
      type="button"
      className="tog"
      aria-expanded={on}
      aria-label={on ? t('fold') : t('unfold')}
      onClick={() => toggle(path)}
    >
      {on ? '[−]' : '[+]'}
    </button>
  );
}

function typeText(f: Field): string {
  const kind = t(KIND[f.kind]);
  if (f.kind === 'list' && f.item !== null) return t('kind_list_of', { item: t(KIND[f.item]) });
  return kind;
}

function OneField({ f }: { f: Field }) {
  const { open, cell } = useContext(FoldingContext);
  const kids = hasKids(f);
  return (
    <div className="fld">
      <div className="fh">
        {kids ? <Toggle path={f.path} /> : null}
        <code>{f.name}</code>
        <span className="ft">{typeText(f)}</span>
        {f.optional ? <span className="fw">{t('field_optional')}</span> : null}
        {f.min !== null && f.min > 0 ? <span className="fw">{t('field_min', { n: f.min })}</span> : null}
      </div>
      {f.desc ? <div className="fd">{inline(f.desc)}</div> : null}
      {f.values.length > 0 ? (
        <div className="vals">
          {f.values.map(([v, means]) => (
            <div key={v}>
              <code>{v}</code>
              {means ? inline(means) : null}
            </div>
          ))}
        </div>
      ) : null}
      <Uses uses={f.used_by} here={cell} />
      {f.disc ? (
        <Uses uses={f.disc_used_by} here={cell} label={t('used_by_disc', { disc: f.disc })} />
      ) : null}
      {kids && open.has(f.path) ? (
        <div className="kids">
          {f.branches.length > 0 ? <Shapes f={f} /> : null}
          {f.fields.length > 0 ? <Fields fs={f.fields} /> : null}
        </div>
      ) : null}
    </div>
  );
}

function Fields({ fs }: { fs: Field[] }) {
  return (
    <div className="sub-f">
      {fs.map((f) => (
        <OneField key={f.path} f={f} />
      ))}
    </div>
  );
}

function Shape({ f, b }: { f: Field; b: Branch }) {
  const { open, cell } = useContext(FoldingContext);
  return (
    <div className="br">
      <div className="bh">
        {b.fields.length > 0 ? <Toggle path={b.path} /> : null}
        <code>
          {f.disc ?? ''}: "{b.value}"
        </code>
        {b.desc ? <span>{inline(b.desc)}</span> : null}
      </div>
      <Uses uses={b.used_by} here={cell} />
      {b.fields.length === 0 ? <p className="fmt">{t('shape_empty')}</p> : null}
      {b.fields.length > 0 && open.has(b.path) ? <Fields fs={b.fields} /> : null}
    </div>
  );
}

/// One of several shapes: the fields every shape has, once, then the shapes, of which one is given.
function Shapes({ f }: { f: Field }) {
  return (
    <>
      {f.common.length > 0 ? (
        <div className="ig">
          <div className="il">{t('shapes_common')}</div>
          <Fields fs={f.common} />
        </div>
      ) : null}
      <div className="brs pick">
        <div className="pick-h">
          <code>{f.disc ?? f.name}</code>
          <span className="ft">{t('one_of')}</span>
        </div>
        {f.branches.map((b, i) => (
          <Fragment key={b.value}>
            {i > 0 ? <div className="or">{t('or')}</div> : null}
            <Shape f={f} b={b} />
          </Fragment>
        ))}
      </div>
    </>
  );
}

/// The title of a form an agent's return may hold.
function formTitle(c: CompiledCell, field: string | null): string {
  if (field === null) return t('form_own');
  if (field === 'tools' && (c.tools ?? []).length > 0) {
    return t('form_tools', { field: `\`${field}\``, tools: (c.tools ?? []).map((x) => `\`${x}\``).join(', ') });
  }
  return t('form_other', { field: `\`${field}\`` });
}

export function Returns({ c, r }: { c: CompiledCell; r: ReturnsType }) {
  const [open, setOpen] = useState<Set<string>>(new Set());
  const all = [...r.forms.flatMap((f) => foldable(f.fields)), ...foldable(r.every)];
  const toggle = (path: string) =>
    setOpen((o) => {
      const n = new Set(o);
      if (n.has(path)) n.delete(path);
      else n.add(path);
      return n;
    });
  if (r.forms.length === 0 && r.every.length === 0) return <p className="fmt">{t('returns_none')}</p>;
  return (
    <FoldingContext.Provider value={{ open, toggle, cell: c.id }}>
      {all.length > 0 ? (
        <div className="tog-all">
          <button type="button" className="tog-btn" onClick={() => setOpen(new Set())}>
            {t('collapse_all')}
          </button>
          <button type="button" className="tog-btn" onClick={() => setOpen(new Set(all))}>
            {t('expand_all')}
          </button>
        </div>
      ) : null}
      <Uses uses={r.used_by} here={c.id} label={t('used_by_whole')} />
      {r.forms.length > 0 ? (
        <div className="pick">
          <div className="pick-h">
            <code>{r.within ?? ''}</code>
            <span className="ft">{t('one_of')}</span>
          </div>
          {r.forms.map((f, i) => (
            <Fragment key={f.field ?? ''}>
              {i > 0 ? <div className="or">{t('or')}</div> : null}
              <div className="ig">
                <div className="il">{inline(formTitle(c, f.field))}</div>
                <Uses uses={f.used_by} here={c.id} label={t('used_by_form')} />
                {f.fields.length > 0 ? <Fields fs={f.fields} /> : <p className="fmt">{t('form_empty')}</p>}
              </div>
            </Fragment>
          ))}
        </div>
      ) : null}
      {r.every.length > 0 ? (
        <div className="ig">
          {r.forms.length > 0 ? <div className="il">{t('every_return')}</div> : null}
          <Fields fs={r.every} />
        </div>
      ) : null}
    </FoldingContext.Provider>
  );
}
