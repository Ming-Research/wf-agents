// A value an agent handed back whose shape the page does not draw in a way of its own: as labelled text, by its
// shape, never as a tree of JSON. Text reads as the agent wrote it, a list as a list, an object as its fields each
// under its name as a sub-heading.

import type { JsonValue } from './api';
import { Markdown } from './markdown';
import { SubHeading } from './parts';

export type Obj = { [key in string]: JsonValue };

export const isObj = (v: JsonValue | undefined): v is Obj => typeof v === 'object' && v !== null && !Array.isArray(v);

/// A value as one text: a string as it is, nothing as empty, anything else as it is written.
export const str = (v: JsonValue | undefined): string =>
  typeof v === 'string' ? v : v === undefined || v === null ? '' : typeof v === 'object' ? JSON.stringify(v) : String(v);

/// A name as words: `to_check` is "to check".
export const words = (s: string | null | undefined) => (s ?? '').replaceAll('_', ' ');

/// A name as a label: `vs_item` is "Vs item".
export const label = (s: string) => {
  const w = words(s).replaceAll('.', ' · ');
  return w.charAt(0).toUpperCase() + w.slice(1);
};

/// Nothing worth showing: no value, empty text, an empty list or object.
export const empty = (v: JsonValue | undefined): boolean =>
  v === undefined ||
  v === null ||
  (typeof v === 'string' && v.trim() === '') ||
  (Array.isArray(v) && v.length === 0) ||
  (isObj(v) && Object.values(v).every(empty));

export function Labelled({ value }: { value: JsonValue | undefined }) {
  if (value === undefined || value === null) return null;
  if (typeof value === 'string') return <Markdown text={value} />;
  if (typeof value === 'number' || typeof value === 'boolean') return <p className="prose">{String(value)}</p>;
  if (Array.isArray(value)) {
    const items = value.filter((v) => !empty(v));
    if (items.length === 0) return null;
    return (
      <ul className="labelled-list">
        {items.map((v, i) => (
          <li key={i}>
            <Labelled value={v} />
          </li>
        ))}
      </ul>
    );
  }
  const fields = Object.entries(value).filter(([, v]) => !empty(v));
  return (
    <div className="labelled">
      {fields.map(([k, v]) => (
        <div key={k} className="labelled-field">
          <SubHeading>{label(k)}</SubHeading>
          <Labelled value={v} />
        </div>
      ))}
    </div>
  );
}
