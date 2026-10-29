// What a round's panel reads from a round, apart from how it draws it: the message as sent, cut at its headings, a
// long part folded with its size and its share of the message; a value given or handed back, laid out by the JSON
// Schema of where it belongs, each field under the title the flow gives it, found by the place's path, a value the
// schema fixes (a kind) apart, and by its JSON type wherever the schema says nothing of it; and where an agent's round
// spent its tokens and its cost.

import type { JsonValue, RoundTokens } from './api';
import { isObj, type Obj, words } from './labelled';
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

/// A text longer than this many characters is folded to its first three lines.
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

/// What the control that opens a folded part of a message adds after its size: the part's share of the message, and
/// for the task, that it is the cell's, the same every round.
export function shareWords(n: number, total: number, task: boolean): string {
  return t(task ? 'long_share_task' : 'long_share', { share: share(n, total) });
}

/// A field's title, as the flow gives it by the field's path (`understanding.grounds[verified].claim`), or that of
/// the same path with its branches taken out; for a flow version stored before fields had titles, its key in words.
export function titleOf(titles: Record<string, string> | undefined, path: string, key: string): string {
  const own = titles?.[path];
  if (own !== undefined) return own;
  const bare = (p: string) => p.replace(/\[[^\]]*\]/g, '');
  const want = bare(path);
  for (const [p, title] of Object.entries(titles ?? {})) if (bare(p) === want) return title;
  const w = words(key);
  return w.charAt(0).toUpperCase() + w.slice(1);
}

/// A value laid out by its schema: a text; a number, a boolean or null as JSON writes it; a value the schema fixes
/// (a const, or one of an enum's values), which is a kind; an empty list or object; an object by its fields in its
/// own order; a list by its items in order.
export type Shown =
  | { kind: 'text'; text: string }
  | { kind: 'plain'; text: string }
  | { kind: 'fixed'; text: string }
  | { kind: 'empty'; text: string }
  | { kind: 'fields'; fields: ShownField[] }
  | { kind: 'list'; items: Shown[] };

/// A field of an object with its value: its key and the title the flow gives the place, none where the schema does
/// not describe the key, which is then shown by its key alone.
export type ShownField = { key: string; title: string | null; value: Shown };

export type ShapeOptions = {
  /// The key of an agent's return that holds exactly one of its forms: the form's fields are at the places' root.
  within?: string;
  /// The titles of the places of the return, by their paths.
  titles?: Record<string, string>;
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

/// The alternative of a `oneOf` or `anyOf` a value is, with its place among them and, where a field every alternative
/// fixes tells them apart, the value it fixes: the one whose fixed value is the value's, else the first the value
/// fits; none when it is none of them.
export function branch(s: Obj, v: JsonValue): { schema: Obj; at: number; value?: string } | null {
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
    return at >= 0 ? { schema: alts[at], at, value: String(v[disc]) } : null;
  }
  const at = alts.findIndex((a) => fits(a, v));
  return at >= 0 ? { schema: alts[at], at } : null;
}

/// The schema a value is read by: the alternative it is of one of several, the schema itself when the value is of
/// the type it names, none when the schema does not describe the value.
function resolve(s: Obj | null, v: JsonValue): { schema: Obj; at: number; value?: string } | null {
  if (s === null) return null;
  if (alternatives(s) !== null) return branch(s, v);
  return typeFits(s, v) || 'const' in s || Array.isArray(s.enum) ? { schema: s, at: -1 } : null;
}

/// A long description, for a note, as its first sentence or clause.
export function brief(text: string): string {
  if (text.length <= 120) return text;
  const m = /[.;:] /g;
  for (let hit = m.exec(text); hit !== null; hit = m.exec(text)) {
    if (hit.index >= 20) return text.slice(0, hit.index);
  }
  return text;
}

/// A value, laid out by the schema of where it belongs; with no schema, or where the value is not what the schema
/// describes, by its JSON type alone.
export function shape(value: JsonValue, schema: JsonValue | null | undefined, o: ShapeOptions = {}): Shown {
  return shapeAt(value, isObj(schema) ? schema : null, o, '', 0);
}

/// The path of a place under a branch: the branch's fixed value in brackets.
const under = (path: string, r: { value?: string } | null) => (r?.value !== undefined ? `${path}[${r.value}]` : path);

function shapeAt(v: JsonValue, s: Obj | null, o: ShapeOptions, path: string, depth: number): Shown {
  if (typeof v === 'string') {
    const fixed = s !== null && ('const' in s || Array.isArray(s.enum));
    return { kind: fixed ? 'fixed' : 'text', text: v };
  }
  if (v === null || typeof v !== 'object') return { kind: 'plain', text: JSON.stringify(v) };
  if (Array.isArray(v)) {
    if (v.length === 0) return { kind: 'empty', text: '[]' };
    const items = s !== null && isObj(s.items) ? s.items : null;
    return {
      kind: 'list',
      items: v.map((x) => {
        const r = resolve(items, x);
        return shapeAt(x, r?.schema ?? null, o, under(path, r), depth + 1);
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
      // The forms of an agent's return are at the places' root: their fields are named as the cell's own.
      const form = depth === 0 && k === o.within;
      const at = form ? '' : path === '' ? k : `${path}.${k}`;
      const title = ks === null || form ? null : titleOf(o.titles, at, k);
      return { key: k, title, value: shapeAt(x, r?.schema ?? null, o, form ? '' : under(at, r), depth + 1) };
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
  /// A fact of the round, its steps or its context, rather than a share of its cost.
  fact?: boolean;
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
  parts.push({ text: steps, title: null, whole: steps, fact: true });
  if (x.context_end !== null) {
    const text =
      x.context_before !== null
        ? t('spent_context', { before: tokens(x.context_before), end: tokens(x.context_end) })
        : t('spent_context_end', { end: tokens(x.context_end) });
    parts.push({ text, title: t(x.context_before !== null ? 'spent_context_hint' : 'spent_context_end_hint'), whole: text, fact: true });
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
