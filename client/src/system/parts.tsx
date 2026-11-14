// The parts the system's and the settings' views share: a number that stands alone, a table of the view's, and the
// links to a work and to an item.

import type { ReactNode } from 'react';

/// A number that stands alone, with what it counts under it, and, small after the number, what more it says. Where it
/// counts a list, pressing it unfolds the list under the numbers, and folds it again.
export function Tile({ value, label, more, title, open, onOpen }: {
  value: ReactNode;
  label: string;
  more?: ReactNode;
  title?: string;
  /// Whether the list it counts is unfolded; none where it counts no list.
  open?: boolean;
  onOpen?: () => void;
}) {
  const inner = (
    <>
      <span className="tile-value">
        {value}
        {more !== undefined && <span className="tile-more small muted"> {more}</span>}
      </span>
      <span className="label">{label}</span>
    </>
  );
  if (onOpen === undefined) {
    return (
      <span className="tile" title={title}>
        {inner}
      </span>
    );
  }
  return (
    <button
      type="button"
      className="tile tile-opens"
      aria-expanded={open}
      title={title ?? (open ? 'Fold the list away' : 'Show the list it counts')}
      onClick={onOpen}
    >
      {inner}
    </button>
  );
}

/// The numbers of a part, side by side.
export function Tiles({ children }: { children: ReactNode }) {
  return <div className="tiles">{children}</div>;
}

/// A table of a view: its heads in labels, a column of numbers at the right of its cells; every cell on one line, cut
/// at its edge and whole on the pointer's tip, which each cell that may be cut gives.
export function ViewTable({ heads, numbers = [], children, label }: {
  heads: string[];
  /// The columns, by their place, that hold numbers.
  numbers?: number[];
  label?: string;
  children: ReactNode;
}) {
  return (
    <table className="table view-table" aria-label={label}>
      <thead>
        <tr>
          {heads.map((h, i) => (
            <th key={h} scope="col" className={numbers.includes(i) ? 'label num' : 'label'}>
              {h}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>{children}</tbody>
    </table>
  );
}

/// A work by its number, which opens it.
export function WorkLink({ work }: { work: number }) {
  return <a href={`#/works/${work}`} className="mono">{`W${work}`}</a>;
}

/// An item by its number and its title; its number alone where its title is not known.
export function ItemName({ item, title }: { item: number; title?: string }) {
  return (
    <span title={title}>
      <span className="mono muted">#{item}</span>
      {title !== undefined && <> {title}</>}
    </span>
  );
}
