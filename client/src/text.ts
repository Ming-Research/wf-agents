// The page's texts, and how parts of the flow are written out in words.

import type { Atom, Do, Src } from './api';

export { t, tn } from './messages';
export type { Key } from './messages';

/// Where an input is read from, as one line.
export function srcText(s: Src): string {
  if (Array.isArray(s)) return `${s[0]} → ${s[1]}`;
  if (s === 'self') return 'self';
  if ('config' in s) return `config ${s.config}`;
  if ('db' in s) return `db ${s.db}${s.by ? ` by ${s.by}` : ''}`;
  if ('engine' in s) return `engine ${s.engine}`;
  return `recorded ${s.recorded}`;
}

/// A row's condition, as one line.
export function atomText(a: Atom): string {
  const [op, x, y] = a as unknown as [string, unknown, unknown];
  if (op === 'all') return '*';
  if (op === 'own') return 'own';
  if (op === 'or') return `(${(x as Atom[]).map(atomText).join(' or ')})`;
  if (op === 'has') return `has ${String(x)}`;
  return `${String(x)} ${op} ${(y as string[]).join(' | ')}`;
}

/// A row's action, as one line.
export function doText(d: Do): string {
  return typeof d === 'string' ? d : `${d.do} @ ${d.at}`;
}
