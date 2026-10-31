// What comes out of a cell, in the one JSON look both panels draw it in (ui-spec.md). On a round, the return as it was
// handed back, pretty-printed: its keys, its texts and its other values each in their syntax colour, a text's line
// breaks kept and a text over 1,000 characters folded to its first lines, each object and list folding at its opening
// line, and a key saying on hovering what the flow says of its place. On a cell, the pattern of what it hands back, in
// the same look, what each round fills in as a placeholder: for an agent's cell the forms its return may hold, one at
// a time, in the field that holds one, beside the fields every return holds; a field of fixed values those values; a
// key saying on hovering what the agent is told it is and what reads it. Under an agent cell's pattern, the checks the
// engine makes on a return before it takes a row.

import { ChevronDownIcon, ChevronRightIcon } from '@primer/octicons-react';
import { type ReactNode, useState } from 'react';

import type { CompiledCell, EngineNames, Field, FieldKind, JsonValue, Use } from '../api';
import { actionWords } from '../graph/flowModel';
import { isObj, type Obj } from '../labelled';
import { chars, FOLD_AT, placeSchema, props, resolve, titleOf, under } from '../round';
import { type Key, t, tn } from '../text';
import { Hole, InnerName, joinedWords, Long, Name, Pairs, Part } from '../voices';
import { useFlow } from './context';
import { inline } from './words';

export { titleOf } from '../round';

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

/// How deep a cell's pattern is open at first: its forms, their fields and the shapes a field may take, and one level
/// inside them.
const PATTERN_OPEN = 5;

/// One line of the JSON look: the control that folds or opens it in the gutter, where it opens an object or a list.
function Line({ fold, children }: { fold?: [boolean, (open: boolean) => void]; children: ReactNode }) {
  return (
    <div className="json-line">
      {fold !== undefined ? (
        <button
          type="button"
          className="json-fold"
          aria-expanded={fold[0]}
          aria-label={t(fold[0] ? 'json_fold' : 'json_unfold')}
          onClick={() => fold[1](!fold[0])}
        >
          {fold[0] ? <ChevronDownIcon size={12} /> : <ChevronRightIcon size={12} />}
        </button>
      ) : null}
      {children}
    </div>
  );
}

/// A key and its colon, saying on hovering what the flow says of its place.
function KeyName({ k, tip }: { k: string; tip?: string }) {
  return (
    <>
      <span className={tip !== undefined ? 'json-key json-tip' : 'json-key'} title={tip}>
        &quot;{k}&quot;
      </span>
      {': '}
    </>
  );
}

/// A text as the return holds it, its line breaks kept; over 1,000 characters, its first three lines and what opens it.
function Text({ s, tip }: { s: string; tip?: string }) {
  const shown = (x: string) => (
    <span className="json-string" title={tip}>
      &quot;{x}&quot;
    </span>
  );
  return chars(s) > FOLD_AT ? <Long text={s}>{shown}</Long> : shown(s);
}

/// An object or a list: open, its opening line, its members one level in and its closing line; folded, one line, with
/// how many it holds where that is known. `note` follows the opening, `tail` the closing.
function Nest(p: {
  head: ReactNode;
  list: boolean;
  n: number | null;
  open: boolean;
  note?: ReactNode;
  tail: string;
  children: () => ReactNode;
}) {
  const { head, list, n, note, tail, children } = p;
  const [open, setOpen] = useState(p.open);
  const [l, r] = list ? ['[', ']'] : ['{', '}'];
  if (!open) {
    return (
      <Line fold={[false, setOpen]}>
        {head}
        {`${l}…${r}${tail}`}
        {note}
        {n !== null ? (
          <>
            {' '}
            <InnerName>{tn(list ? 'json_items' : 'json_fields', n)}</InnerName>
          </>
        ) : null}
      </Line>
    );
  }
  return (
    <>
      <Line fold={[true, setOpen]}>
        {head}
        {l}
        {note}
      </Line>
      <div className="json-in">{children()}</div>
      <Line>{`${r}${tail}`}</Line>
    </>
  );
}

/// What the flow says of a place of a return, for its key's tip: its title and its description.
function placeTip(s: Obj | null, titles: Record<string, string> | undefined, path: string): string | undefined {
  const title = path !== '' ? titleOf(titles, path, '') : '';
  const desc = s !== null && typeof s.description === 'string' ? s.description : '';
  const tip = [title, desc].filter((x) => x !== '').join('\n');
  return tip !== '' ? tip : undefined;
}

type Reading = { titles?: Record<string, string>; within?: string };

/// A value of a return at a place, read by the schema of the place (`s`), whose path the titles go by (`path`).
function ValueAt({ v, s, path, depth, o, head, tail }: {
  v: JsonValue;
  s: Obj | null;
  path: string;
  depth: number;
  o: Reading;
  head: ReactNode;
  tail: string;
}) {
  if (typeof v === 'string') {
    return (
      <Line>
        {head}
        <Text s={v} />
        {tail}
      </Line>
    );
  }
  if (v === null || typeof v !== 'object') {
    return (
      <Line>
        {head}
        <span className="json-constant">{JSON.stringify(v)}</span>
        {tail}
      </Line>
    );
  }
  const r = resolve(s, v);
  const at = under(path, r);
  const schema = r?.schema ?? null;
  if (Array.isArray(v)) {
    if (v.length === 0) {
      return (
        <Line>
          {head}
          {`[]${tail}`}
        </Line>
      );
    }
    const items = schema !== null && isObj(schema.items) ? schema.items : null;
    return (
      <Nest head={head} list n={v.length} open tail={tail}>
        {() => v.map((x, i) => <ValueAt key={i} v={x} s={items} path={at} depth={depth + 1} o={o} head={null} tail={i < v.length - 1 ? ',' : ''} />)}
      </Nest>
    );
  }
  const keys = Object.keys(v);
  if (keys.length === 0) {
    return (
      <Line>
        {head}
        {`{}${tail}`}
      </Line>
    );
  }
  const p = schema !== null ? props(schema) : {};
  return (
    <Nest head={head} list={false} n={keys.length} open tail={tail}>
      {() =>
        keys.map((k, i) => {
          const own = p[k];
          const ks = isObj(own) ? own : null;
          // The forms of an agent's return are at the places' root: their fields are named as the cell's own.
          const form = depth === 0 && k === o.within;
          const kp = form ? '' : at === '' ? k : `${at}.${k}`;
          return (
            <ValueAt
              key={k}
              v={v[k]}
              s={ks}
              path={kp}
              depth={depth + 1}
              o={o}
              head={<KeyName k={k} tip={placeTip(ks, o.titles, kp)} />}
              tail={i < keys.length - 1 ? ',' : ''}
            />
          );
        })
      }
    </Nest>
  );
}

/// A return as JSON, in the look above: every object and list open at first, and a key's tip read from the schema of
/// the cell that handed it back, by the titles of its places and the field that holds an agent's form.
export function Json({ value, schema, titles, within }: {
  value: JsonValue;
  schema?: JsonValue | null;
  titles?: Record<string, string>;
  within?: string;
}) {
  return (
    <div className="json">
      <ValueAt v={value} s={isObj(schema) ? schema : null} path="" depth={0} o={{ titles, within }} head={null} tail="" />
    </div>
  );
}

/// What the engine does every time `here`'s return holds a field, by the field's uses.
export function fieldActions(uses: Use[], here: string): string[] {
  return uses.flatMap((u) => (u.kind === 'always' && u.cell === here && u.action != null ? [u.action] : []));
}

/// What a field of a cell's return is, for its key's tip: its title, what the agent is told it is, and in a line what
/// the page knows of it besides: its fewest items, whether the item shows it, what reads it, what the engine does every
/// time the return holds it, and what checks it.
function fieldTip(f: Field, c: CompiledCell, engine: EngineNames): string | undefined {
  const uses = [...f.used_by, ...f.disc_used_by];
  const once = <T,>(xs: T[]) => [...new Set(xs)];
  const cells = once(uses.flatMap((u) => (u.kind === 'input' || (u.kind === 'condition' && u.cell !== c.id) ? [u.cell] : [])));
  const roles = once(uses.flatMap((u) => (u.kind === 'role' ? [u.role] : [])));
  const checks = once(uses.flatMap((u) => (u.kind === 'check' ? [u.check] : [])));
  const facts = [
    f.min !== null && f.min > 0 ? t('field_min', { n: f.min }) : null,
    (c.shown ?? []).includes(f.path) || uses.some((u) => u.kind === 'shown') ? t('field_shown') : null,
    cells.length > 0 ? `${t('field_read_by')} ${cells.join(', ')}` : null,
    ...roles.map((r) => t('field_read_by_role', { role: r })),
    uses.some((u) => u.kind === 'handoff') ? t('use_handoff') : null,
    ...fieldActions(uses, c.id).map((a) => (engine.actions[a] ?? actionWords(a)).replaceAll('`', '')),
    ...checks.map((k) => t('field_checked', { check: k })),
  ].filter((x) => x !== null);
  const tip = [titleOf(c.titles, f.path, ''), f.desc ?? '', joinedWords(facts)].filter((x) => x !== '').join('\n');
  return tip !== '' ? tip : undefined;
}

/// The values a field of fixed values may take, each saying on hovering what it means; else what each round fills in
/// for a value of its kind, as a placeholder.
function Fill({ f, kind }: { f: Field; kind: FieldKind }) {
  if (f.values.length === 0) return <Hole>{t(KIND[kind])}</Hole>;
  return (
    <>
      {f.values.map(([v, means], i) => (
        <span key={v}>
          {i > 0 ? <span className="json-or"> | </span> : null}
          <Text s={v} tip={means ?? undefined} />
        </span>
      ))}
    </>
  );
}

/// The fields of an object of a pattern, one under another, the key that tells its shape first where it is one of
/// several.
function Members({ head, fields, lead, c, depth, note, tail }: {
  head: ReactNode;
  fields: Field[];
  lead?: ReactNode;
  c: CompiledCell;
  depth: number;
  note?: ReactNode;
  tail: string;
}) {
  if (fields.length === 0 && lead === undefined) {
    return (
      <Line>
        {head}
        {`{}${tail}`}
        {note}
      </Line>
    );
  }
  return (
    <Nest head={head} list={false} n={fields.length + (lead !== undefined ? 1 : 0)} open={depth < PATTERN_OPEN} note={note} tail={tail}>
      {() => (
        <>
          {lead}
          {fields.map((x, i) => (
            <FieldAt key={x.path} f={x} c={c} depth={depth + 1} tail={i < fields.length - 1 ? ',' : ''} />
          ))}
        </>
      )}
    </Nest>
  );
}

/// The shapes a value of several shapes may take, one under another, after "one of".
function Shapes({ head, f, c, depth, note, tail }: { head: ReactNode; f: Field; c: CompiledCell; depth: number; note?: ReactNode; tail: string }) {
  const [open, setOpen] = useState(depth < PATTERN_OPEN);
  const n = f.branches.length;
  if (!open) {
    return (
      <Line fold={[false, setOpen]}>
        {head}
        <InnerName>{tn('json_one_of_n', n)}</InnerName>
        {tail}
        {note}
      </Line>
    );
  }
  return (
    <>
      <Line fold={[true, setOpen]}>
        {head}
        <InnerName>{t('json_one_of')}</InnerName>
        {note}
      </Line>
      <div className="json-in">
        {f.branches.map((b, i) => (
          <Members
            key={b.path}
            head={null}
            fields={[...f.common, ...b.fields]}
            lead={
              f.disc !== null ? (
                <Line>
                  <KeyName k={f.disc} />
                  <Text s={b.value} tip={b.desc ?? undefined} />
                  {f.common.length + b.fields.length > 0 ? ',' : ''}
                </Line>
              ) : undefined
            }
            c={c}
            depth={depth + 1}
            tail={i === n - 1 ? tail : ''}
          />
        ))}
      </div>
    </>
  );
}

/// A field of a cell's pattern: its key, saying on hovering what it is, and its value by its kind; whether it may be
/// left out after it.
function FieldAt({ f, c, depth, tail }: { f: Field; c: CompiledCell; depth: number; tail: string }) {
  const engine = useFlow().compiled.engine;
  const head = <KeyName k={f.name} tip={fieldTip(f, c, engine)} />;
  const note = f.optional ? (
    <>
      {' '}
      <InnerName>{t('field_optional')}</InnerName>
    </>
  ) : null;
  if (f.kind === 'object') return <Members head={head} fields={f.fields} c={c} depth={depth} note={note} tail={tail} />;
  if (f.kind === 'union') return <Shapes head={head} f={f} c={c} depth={depth} note={note} tail={tail} />;
  if (f.kind === 'list' && (f.item === 'object' || f.item === 'union')) {
    return (
      <Nest head={head} list n={null} open={depth < PATTERN_OPEN} note={note} tail={tail}>
        {() =>
          f.item === 'object' ? (
            <Members head={null} fields={f.fields} c={c} depth={depth + 1} tail="" />
          ) : (
            <Shapes head={null} f={f} c={c} depth={depth + 1} tail="" />
          )
        }
      </Nest>
    );
  }
  return (
    <Line>
      {head}
      {f.kind === 'list' ? (
        <>
          [<Fill f={f} kind={f.item ?? 'any'} />]
        </>
      ) : (
        <Fill f={f} kind={f.kind} />
      )}
      {tail}
      {note}
    </Line>
  );
}

/// What a cell hands back, as a pattern in the JSON look: an agent's return, one of its forms in the field that holds
/// one, beside the fields every return holds; any other cell's, its fields.
export function ReturnPattern({ c }: { c: CompiledCell }) {
  const f = useFlow();
  const r = f.cellView(c.id)?.returns;
  const every = r?.every ?? [];
  const forms = r?.within != null ? r.forms : [];
  if (every.length === 0 && forms.length === 0) return <p className="pp-note">{t('out_nothing')}</p>;
  const within = r?.within ?? '';
  return (
    <div className="json">
      <Line>{'{'}</Line>
      <div className="json-in">
        {forms.length > 0 ? (
          <>
            <Line>
              <KeyName k={within} tip={placeTip(placeSchema(c.schema, within), undefined, '')} />
              <InnerName>{t('json_one_of')}</InnerName>
            </Line>
            <div className="json-in">
              {forms.map((form, i) => (
                <Members
                  key={form.field ?? ''}
                  head={null}
                  fields={form.fields}
                  c={c}
                  depth={2}
                  tail={i === forms.length - 1 && every.length > 0 ? ',' : ''}
                />
              ))}
            </div>
          </>
        ) : null}
        {every.map((x, i) => (
          <FieldAt key={x.path} f={x} c={c} depth={1} tail={i < every.length - 1 ? ',' : ''} />
        ))}
      </div>
      <Line>{'}'}</Line>
    </div>
  );
}

/// The checks the engine makes on a cell's return before it takes a row, each with what it checks and where.
export function ReturnChecks({ c }: { c: CompiledCell }) {
  const f = useFlow();
  const checks = c.checks ?? [];
  if (checks.length === 0) return null;
  return (
    <Part title={t('out_checks')}>
      <Pairs
        rows={checks.map((k) => [
          <Name key="n">{k.check}</Name>,
          <span key="w">
            {inline(f.compiled.engine.checks[k.check] ?? '')}
            {k.at ? <span className="pp-facts"> {inline(t('out_check_at', { at: `\`${k.at}\`` }))}</span> : null}
          </span>,
        ])}
      />
      <p className="pp-note">{t('checks_note')}</p>
    </Part>
  );
}

/// The buttons the owner rules by at an owner's cell, each its value, its title and the line under it, as the ask says
/// them: what the owner can hand back there.
export function OwnerButtons({ c }: { c: CompiledCell }) {
  const buttons = c.choices ?? [];
  if (buttons.length === 0) return null;
  return (
    <Part title={t('back_buttons')} facts={t('back_buttons_facts')}>
      <Pairs
        rows={buttons.map((b) => [
          <Name key="n">{b.value}</Name>,
          <span key="w">
            {b.title}
            <span className="pp-facts"> {b.desc}</span>
          </span>,
        ])}
      />
    </Part>
  );
}
