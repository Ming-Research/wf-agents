// What a round's panel reads from a round, apart from how it draws it: the message as sent, cut at its headings, a
// long part folded with its size and its share of the message; a value given or handed back, laid out by the JSON
// Schema of where it belongs (each key with what the schema says it is, the value of an enum or a const with what it
// means, the alternative of a `oneOf` or `anyOf` the value is, and in a list of objects each key's note once, at the
// list's head) and by its JSON type wherever the schema says nothing of it; and where an agent's round spent its tokens
// and its cost.

import type { JsonValue, RoundTokens } from './api';
import { isObj, type Obj } from './labelled';
import { t } from './text';

/// A part of a message as sent: a heading line, or the lines between two headings, exactly as they are.
export type Part = { heading: string } | { text: string };

/// A message cut at its headings, every character kept: the parts joined by line breaks are the message.
export function messageParts(message: string): Part[] {
  const parts: Part[] = [];
  let lines: string[] | null = null;
  for (const line of message.split('\n')) {
    if (/^#{1,6} /.test(line)) {
      if (lines !== null) parts.push({ text: lines.join('\n') });
      lines = null;
      parts.push({ heading: line });
    } else {
      (lines ??= []).push(line);
    }
  }
  if (lines !== null) parts.push({ text: lines.join('\n') });
  return parts;
}

/// A part longer than this many characters is folded.
export const FOLD_AT = 1000;

/// How many characters a text has, as a reader counts them.
export function chars(text: string): number {
  return Array.from(text).length;
}

/// A part's share of the whole message, in whole percent.
export function share(n: number, total: number): string {
  const p = total > 0 ? Math.round((100 * n) / total) : 0;
  return p === 0 && n > 0 ? '<1%' : `${p}%`;
}

/// The label of a folded part, in the page's words: its size and its share of the message to open it, its size to
/// fold it again.
export function foldLabel(n: number, total: number, open: boolean): string {
  const size = n.toLocaleString('en-US');
  return open ? t('fold_hide', { n: size }) : t('fold_show', { n: size, share: share(n, total) });
}

/// The first few lines of a folded part, as they are, shown before it is opened: the first three after any empty
/// lines it starts with, no more than 300 characters of them.
export function preview(text: string): string {
  const lines = text.split('\n');
  const first = lines.findIndex((l) => l.trim() !== '');
  const head = lines.slice(Math.max(first, 0), Math.max(first, 0) + 3).join('\n');
  return Array.from(head).slice(0, 300).join('');
}

/// A table of more rows than this is folded, its first `ROWS_PREVIEW` rows showing.
export const ROWS_FOLD_AT = 10;
export const ROWS_PREVIEW = 3;

/// The label of a folded table, in the page's words: how many rows it has, to open it or to fold it again.
export function rowsLabel(n: number, open: boolean): string {
  const rows = n.toLocaleString('en-US');
  return open ? t('fold_rows_hide', { n: rows }) : t('fold_rows_show', { n: rows });
}

/// A value laid out by its schema: a text, a number, a boolean or null as JSON writes it, an empty list or object,
/// an object by its keys in its own order, a list by its items in order; a list of objects the schema lists no fields
/// of, each holding only texts, numbers, booleans and nulls (the rows a query found), as a table, a column for each
/// key any of them has, in the order they first come, and a row for each, a key a row lacks left empty. A list of two
/// or more objects has a head that gives each key's note once (`head`), its items then holding only the notes the
/// items do not share.
export type Shown =
  | { kind: 'text'; text: string }
  | { kind: 'plain'; text: string }
  | { kind: 'empty'; text: string }
  | { kind: 'fields'; fields: ShownField[] }
  | { kind: 'list'; items: ShownItem[]; head?: HeadKey[] }
  | { kind: 'table'; columns: string[]; rows: Array<Array<string | null>> };

/// A key of an object with its value, and what the schema says of it: the note beside the key, and the whole of what
/// the schema says, for hovering, when the note is shorter; for a list that is not empty, how many items it holds.
export type ShownField = { key: string; note: string | null; full: string | null; count: string | null; value: Shown };

/// A key of the objects of a list as the list's head gives it, once for them all: its note and the whole of it, none
/// when the objects' notes for it differ (as the meanings of an enum's different values), and the keys under it, of
/// an object or of the objects of a list it holds.
export type HeadKey = { key: string; note: string | null; full: string | null; under: HeadKey[] };

/// An item of a list, with what its value means when the schema says.
export type ShownItem = { note: string | null; value: Shown };

export type ShapeOptions = {
  /// The key of an agent's return that holds exactly one of its forms.
  within?: string;
  /// What the form a return holds is, by its alternative in the schema of `within`.
  form?: (alternative: number) => string | null;
};

const props = (s: Obj): Obj => (isObj(s.properties) ? s.properties : {});

/// The alternatives of a schema that is one of several, if it is.
function alternatives(s: Obj): Obj[] | null {
  const alts = Array.isArray(s.oneOf) ? s.oneOf : Array.isArray(s.anyOf) ? s.anyOf : null;
  return alts === null ? null : alts.filter(isObj);
}

/// Whether a value is of the JSON type a schema names; a schema that names none takes any.
function typeFits(s: Obj, v: JsonValue): boolean {
  switch (s.type) {
    case 'string':
      return typeof v === 'string';
    case 'integer':
      return typeof v === 'number' && Number.isInteger(v);
    case 'number':
      return typeof v === 'number';
    case 'boolean':
      return typeof v === 'boolean';
    case 'array':
      return Array.isArray(v);
    case 'object':
      return isObj(v);
    case 'null':
      return v === null;
  }
  return true;
}

/// Whether a value fits a schema: its type, its fixed value or one of its values, the fields an object must hold and
/// may hold.
function fits(s: Obj, v: JsonValue): boolean {
  if ('const' in s) return s.const === v;
  if (Array.isArray(s.enum)) return s.enum.includes(v);
  if (alternatives(s) !== null) return branch(s, v) !== null;
  if (!typeFits(s, v)) return false;
  if (!isObj(v)) return true;
  const p = props(s);
  const required = Array.isArray(s.required) ? s.required : [];
  return required.every((k) => typeof k === 'string' && k in v) && (s.additionalProperties !== false || Object.keys(v).every((k) => k in p));
}

/// The alternative of a `oneOf` or `anyOf` a value is, with its place among them: the one whose fixed value of the
/// field every alternative fixes is the value's, else the first the value fits; none when it is none of them.
export function branch(s: Obj, v: JsonValue): { schema: Obj; at: number } | null {
  const alts = alternatives(s);
  if (alts === null) return fits(s, v) ? { schema: s, at: -1 } : null;
  const first = alts[0];
  const fixed = (a: Obj, k: string) => {
    const f = props(a)[k];
    return isObj(f) && 'const' in f ? f : null;
  };
  const disc = first !== undefined ? Object.keys(props(first)).find((k) => alts.every((a) => fixed(a, k) !== null)) : undefined;
  if (disc !== undefined) {
    if (!isObj(v)) return null;
    const at = alts.findIndex((a) => fixed(a, disc)?.const === v[disc]);
    return at >= 0 ? { schema: alts[at], at } : null;
  }
  const at = alts.findIndex((a) => fits(a, v));
  return at >= 0 ? { schema: alts[at], at } : null;
}

/// The schema a value is read by: the alternative it is of one of several, the schema itself when the value is of
/// the type it names, none when the schema does not describe the value.
function resolve(s: Obj | null, v: JsonValue): { schema: Obj; at: number } | null {
  if (s === null) return null;
  if (alternatives(s) !== null) return branch(s, v);
  return typeFits(s, v) ? { schema: s, at: -1 } : null;
}

/// A description with the meaning of each value folded into it, as the compiler writes it (`<desc>. Values: <value>:
/// <means>; …`), taken apart: the description before, and each value's meaning; a description written otherwise is
/// kept whole, and no value's meaning is known.
export function valueMeanings(desc: string | null, values: string[]): { desc: string | null; means: Map<string, string> } {
  const none = { desc, means: new Map<string, string>() };
  const first = values[0];
  if (desc === null || first === undefined) return none;
  const at = desc.lastIndexOf(`Values: ${first}: `);
  if (at < 0) return none;
  const before = desc.slice(0, at).trimEnd().replace(/\.$/, '').trimEnd();
  let rest = desc.slice(at + 'Values: '.length);
  const means = new Map<string, string>();
  for (const [i, v] of values.entries()) {
    if (!rest.startsWith(`${v}: `)) return none;
    const after = rest.slice(v.length + 2);
    const next = values[i + 1];
    const end = next === undefined ? after.length : after.indexOf(`; ${next}: `);
    if (end < 0) return none;
    means.set(v, after.slice(0, end));
    rest = after.slice(end + 2);
  }
  return { desc: before === '' ? null : before, means };
}

/// What a fixed value means: a const's description, or the meaning an enum's description gives the value.
function meaning(s: Obj | null, v: JsonValue): string | null {
  if (s === null || typeof v !== 'string') return null;
  const desc = typeof s.description === 'string' ? s.description : null;
  if ('const' in s) return s.const === v ? desc : null;
  if (!Array.isArray(s.enum) || !s.enum.includes(v)) return null;
  const values = s.enum.filter((x): x is string => typeof x === 'string');
  return valueMeanings(desc, values).means.get(v) ?? null;
}

/// What a schema says a key is: its description, or that of a list's items, an enum's without its values.
function described(s: Obj | null): string | null {
  if (s === null) return null;
  const own = typeof s.description === 'string' ? s.description : null;
  const items = isObj(s.items) && typeof s.items.description === 'string' ? s.items.description : null;
  const desc = own ?? items;
  if (desc !== null && Array.isArray(s.enum)) {
    return valueMeanings(desc, s.enum.filter((x): x is string => typeof x === 'string')).desc;
  }
  return desc;
}

/// A long description, for a note, as its first sentence or clause; the whole is read on hovering.
export function brief(text: string): string {
  if (text.length <= 120) return text;
  const m = /[.;:] /g;
  for (let hit = m.exec(text); hit !== null; hit = m.exec(text)) {
    if (hit.index >= 20) return text.slice(0, hit.index);
  }
  return text;
}

function note(text: string | null): { note: string | null; full: string | null } {
  if (text === null || text.trim() === '') return { note: null, full: null };
  const short = brief(text);
  return { note: short, full: short === text ? null : text };
}

/// A value, laid out by the schema of where it belongs; with no schema, or where the value is not what the schema
/// describes, by its JSON type alone. A list of two or more objects gives each key's note once, at its head.
export function shape(value: JsonValue, schema: JsonValue | null | undefined, o: ShapeOptions = {}): Shown {
  return headed(shapeAt(value, isObj(schema) ? schema : null, o, 0));
}

/// A value with each list of two or more objects headed by its keys' notes, given once there: an item keeps a note
/// only where the items' notes for that key differ. The objects of lists inside such a list share its head, outermost
/// first, so that a note is given once however deep it is.
function headed(v: Shown): Shown {
  if (v.kind === 'fields') return { kind: 'fields', fields: v.fields.map((f) => ({ ...f, value: headed(f.value) })) };
  if (v.kind !== 'list') return v;
  const objects = v.items.flatMap((x) => (x.value.kind === 'fields' ? [x.value.fields] : []));
  if (v.items.length < 2 || objects.length < v.items.length) {
    return { kind: 'list', items: v.items.map((x) => ({ ...x, value: headed(x.value) })) };
  }
  const head = headOf(objects);
  return { kind: 'list', head, items: v.items.map((x) => ({ ...x, value: bare(x.value, head) })) };
}

/// The objects a value holds for a head: itself when it is one, the items of a list that are objects.
function objectsIn(v: Shown): ShownField[][] {
  if (v.kind === 'fields') return [v.fields];
  if (v.kind === 'list') return v.items.flatMap((x) => (x.value.kind === 'fields' ? [x.value.fields] : []));
  return [];
}

/// The head of objects: each key in the order it first comes, with its note when every object that has it gives the
/// same, and the keys under it; a key with no note and nothing under it is left out, as the head has nothing to say
/// of it.
function headOf(objects: ShownField[][]): HeadKey[] {
  const keys = [...new Set(objects.flatMap((fs) => fs.map((f) => f.key)))];
  return keys.flatMap((key): HeadKey[] => {
    const fs = objects.flatMap((o) => o.filter((f) => f.key === key));
    const same = fs.every((f) => f.note === fs[0].note);
    const under = headOf(fs.flatMap((f) => objectsIn(f.value)));
    const note = same ? fs[0].note : null;
    if (note === null && under.length === 0) return [];
    return [{ key, note, full: note !== null ? (fs.find((f) => f.full !== null)?.full ?? null) : null, under }];
  });
}

/// A value of an object under a head, with the notes the head gives taken off, deep as the head goes.
function bare(v: Shown, head: HeadKey[]): Shown {
  if (head.length === 0) return v;
  if (v.kind === 'list') return { kind: 'list', items: v.items.map((x) => ({ ...x, value: bare(x.value, head) })) };
  if (v.kind !== 'fields') return v;
  return {
    kind: 'fields',
    fields: v.fields.map((f) => {
      const h = head.find((x) => x.key === f.key);
      if (h === undefined) return f;
      const given = h.note !== null && h.note === f.note;
      return { ...f, note: given ? null : f.note, full: given ? null : f.full, value: bare(f.value, h.under) };
    }),
  };
}

function shapeAt(v: JsonValue, s: Obj | null, o: ShapeOptions, depth: number): Shown {
  if (typeof v === 'string') return { kind: 'text', text: v };
  if (v === null || typeof v !== 'object') return { kind: 'plain', text: JSON.stringify(v) };
  if (Array.isArray(v)) {
    if (v.length === 0) return { kind: 'empty', text: '[]' };
    const items = s !== null && isObj(s.items) ? s.items : null;
    const flat = (x: JsonValue) => isObj(x) && Object.values(x).every((y) => y === null || typeof y !== 'object');
    if ((items === null || (!isObj(items.properties) && alternatives(items) === null)) && v.every(flat)) {
      const columns = [...new Set(v.flatMap((x) => (isObj(x) ? Object.keys(x) : [])))];
      const cellText = (y: JsonValue | undefined) => (y === undefined ? null : typeof y === 'string' ? y : JSON.stringify(y));
      return { kind: 'table', columns, rows: v.map((x) => columns.map((c) => (isObj(x) ? cellText(x[c]) : null))) };
    }
    return {
      kind: 'list',
      items: v.map((x) => {
        const r = resolve(items, x)?.schema ?? null;
        const means = meaning(r, x);
        return { note: means !== null ? t('shape_means', { meaning: brief(means) }) : null, value: shapeAt(x, r, o, depth + 1) };
      }),
    };
  }
  const keys = Object.keys(v);
  if (keys.length === 0) return { kind: 'empty', text: '{}' };
  const p = s !== null ? props(s) : {};
  return {
    kind: 'fields',
    fields: keys.map((k) => {
      const own = p[k];
      const ks = isObj(own) ? own : null;
      const x = v[k];
      const r = resolve(ks, x);
      const means = meaning(r?.schema ?? null, x);
      const form = depth === 0 && k === o.within && r !== null && r.at >= 0 ? (o.form?.(r.at) ?? null) : null;
      let said = note(form ?? (means !== null ? t('shape_means', { meaning: means }) : described(ks)));
      if (form !== null || means !== null) said = { note: said.note, full: described(ks) ?? said.full };
      const count =
        Array.isArray(x) && x.length > 0 ? t(x.length === 1 ? 'shape_one_item' : 'shape_items', { n: x.length }) : null;
      return { key: k, note: said.note, full: said.full, count, value: shapeAt(x, r?.schema ?? null, o, depth + 1) };
    }),
  };
}

/// The schema of the place a path names in a return's schema (`design`, `results`, `end.test_changes[added]`): each
/// name looked for among an object's fields, then among those of each alternative of the object (the forms of an
/// agent's return, a branch); a list read by its items; a name with a value in brackets keeps the alternative fixed
/// to that value. None when the schema has no such place.
export function placeSchema(schema: JsonValue | null | undefined, path: string): Obj | null {
  let s: Obj | null = isObj(schema) ? schema : null;
  for (const seg of path.split('.')) {
    if (s === null) return null;
    const hit = /^([^[]+)(?:\[(.+)\])?$/.exec(seg);
    if (hit === null) return null;
    const [, name, fixed] = hit;
    s = field(s, name);
    if (s !== null && fixed !== undefined) {
      const inner: Obj = isObj(s.items) ? s.items : s;
      const alts = alternatives(inner) ?? [];
      s = alts.find((a) => Object.values(props(a)).some((f) => isObj(f) && f.const === fixed)) ?? s;
    }
  }
  return s;
}

/// The schema of a field of an object's schema: its own, or that of the first alternative of it that has the field;
/// through a list, of its items.
function field(s: Obj, name: string): Obj | null {
  const inner: Obj = isObj(s.items) ? s.items : s;
  const own = props(inner)[name];
  if (isObj(own)) return own;
  for (const a of alternatives(inner) ?? []) {
    const f = field(a, name);
    if (f !== null) return f;
  }
  for (const f of Object.values(props(inner))) {
    if (isObj(f) && alternatives(f) !== null) {
      const g = field(f, name);
      if (g !== null) return g;
    }
  }
  return null;
}

/// What a million tokens cost, in US dollars: input, output, a read from the cache, and a write to it kept five
/// minutes or an hour.
type Prices = { input: number; output: number; read: number; write5m: number; write1h: number };

/// The prices of the models the agents run on, by model: input and output as Anthropic lists them (October 2026), a
/// write to the cache at 1.25 times the input for five minutes and 2 times for an hour; the read from the cache as
/// Claude Code charges it, which with the rest gives exactly the cost each round records (`cost_usd`), checked on
/// every round of the owner's works: Sonnet 5.5's read is $0.10 where Anthropic's list says $0.20. Haiku 5.5 costs
/// more for a call whose prompt is over 100K tokens.
const PRICES: { [model in string]: { prices: Prices; above?: { tokens: number; prices: Prices } } } = {
  'claude-opus-5-5': { prices: { input: 4, output: 20, read: 0.2, write5m: 5, write1h: 8 } },
  'claude-sonnet-5-5': { prices: { input: 2, output: 10, read: 0.1, write5m: 2.5, write1h: 4 } },
  'claude-haiku-5-5': {
    prices: { input: 0.1, output: 0.5, read: 0.01, write5m: 0.125, write1h: 0.2 },
    above: { tokens: 100_000, prices: { input: 0.5, output: 2.5, read: 0.05, write5m: 0.625, write1h: 1 } },
  },
};

/// A number of tokens, short: 74, 71K, 12.51M.
export function tokens(n: number): string {
  if (n < 1000) return String(n);
  if (n < 999_500) return `${Math.round(n / 1000)}K`;
  return `${(n / 1_000_000).toFixed(2)}M`;
}

/// A part of what a round spent, as the page says it: its words in the short line, its tokens on hovering, and its
/// words in the whole line.
export type SpentPart = {
  text: string;
  title: string | null;
  whole: string;
  /// Which of the shares the bar and the key colour it is: read again from the cache, newly cached or output.
  share?: 'read' | 'new' | 'output';
};

/// Where an agent's round spent its tokens and its cost: its steps; its context at its start (where the round before
/// it in its session ended) and at its end; and the share of the round's cost each kind of token took, read again from
/// the cache, newly cached, the output with its thinking, and the input when it comes to a percent. The shares are by
/// the model's prices, so they add up to the round's cost; for a model with no prices here, the tokens alone.
export type Spent = {
  steps: number;
  parts: SpentPart[];
  /// The share of the cost that was read again, newly cached and output, from 0 to 1; none without prices.
  shares: { read: number; written: number; output: number } | null;
};

type SpentKey = 'spent_read' | 'spent_new' | 'spent_output' | 'spent_input';

/// What each kind of token of a round cost, by the model's prices, in millionths of a dollar: the new input, the
/// reads from the cache, the writes to it and the output; none for a model with no prices here.
export type CostParts = { input: number; read: number; written: number; output: number };

export function costParts(x: RoundTokens, model: string | null): CostParts | null {
  const priced = model !== null ? PRICES[model] : undefined;
  // A model priced higher above a size of prompt is priced by the last call's here: the record sums the calls.
  const above = priced?.above !== undefined && x.context_end !== null && x.context_end > priced.above.tokens;
  const p = above ? priced?.above?.prices : priced?.prices;
  if (p === undefined) return null;
  return {
    input: x.input * p.input,
    read: x.read * p.read,
    written: x.written_5m * p.write5m + x.written_1h * p.write1h,
    output: x.output * p.output,
  };
}

export function spent(x: RoundTokens, model: string | null): Spent {
  const costs = costParts(x, model);
  const total = costs !== null ? costs.input + costs.read + costs.written + costs.output : 0;
  const parts: SpentPart[] = [];
  const steps = t(x.steps === 1 ? 'spent_step' : 'spent_steps', { n: x.steps });
  parts.push({ text: steps, title: null, whole: steps });
  if (x.context_end !== null) {
    const text =
      x.context_before !== null
        ? t('spent_context', { before: tokens(x.context_before), end: tokens(x.context_end) })
        : t('spent_context_end', { end: tokens(x.context_end) });
    parts.push({ text, title: t(x.context_before !== null ? 'spent_context_hint' : 'spent_context_end_hint'), whole: text });
  }
  const part = (key: SpentKey, n: number, c: number | null, title: string) => {
    if (n === 0) return;
    const pct = c !== null && total > 0 ? share(c, total) : null;
    const which = key === 'spent_read' ? 'read' : key === 'spent_new' ? 'new' : key === 'spent_output' ? 'output' : undefined;
    parts.push({ text: t(key, { share: pct ?? tokens(n) }), title, whole: pct !== null ? `${title}, ${pct}` : title, share: which });
  };
  part('spent_read', x.read, costs?.read ?? null, t('spent_read_hint', { n: tokens(x.read) }));
  const kept = [
    x.written_1h > 0 ? t('spent_kept_1h', { n: tokens(x.written_1h) }) : null,
    x.written_5m > 0 ? t('spent_kept_5m', { n: tokens(x.written_5m) }) : null,
  ].filter((k) => k !== null);
  part('spent_new', x.written_5m + x.written_1h, costs?.written ?? null, t('spent_new_hint', { n: kept.join(', ') }));
  part(
    'spent_output',
    x.output,
    costs?.output ?? null,
    x.thinking > 0
      ? t('spent_output_thinking_hint', { n: tokens(x.output), thinking: tokens(x.thinking) })
      : t('spent_output_hint', { n: tokens(x.output) }),
  );
  if (costs !== null && total > 0 && Math.round((100 * costs.input) / total) >= 1) {
    part('spent_input', x.input, costs.input, t('spent_input_hint', { n: tokens(x.input) }));
  }
  const shares =
    costs !== null && total > 0
      ? { read: costs.read / total, written: costs.written / total, output: costs.output / total }
      : null;
  return { steps: x.steps, parts, shares };
}
