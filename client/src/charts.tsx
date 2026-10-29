// The marks the page shows numbers with, one look wherever they are: a tile (what it counts, the number, a line under
// it), bars one under another, a stacked bar, and columns over time, each drawn to scale. Every mark says what it is on
// hovering, through the tip of the view that holds it; labels are HTML, so that they keep their size as the marks
// shrink with the view; colours come from the page's tokens, in light and dark alike.

import { type MouseEvent, useState } from 'react';

import { Link } from './route';

/// `x` as a percentage of `total`; none of nothing.
export const pct = (x: number, total: number) => (total > 0 ? (100 * x) / total : 0);

/// `x` as a whole percentage of `total`.
export const share = (x: number, total: number) => `${Math.round(pct(x, total))}%`;

/// The tip a view shows beside the pointer for the mark under it, which says what it is in its `data-tip`: the view
/// passes `move` and `leave` to the element that holds its marks, and shows `shown`.
export function useTip() {
  const [tip, setTip] = useState<{ text: string; x: number; y: number } | null>(null);
  const move = (e: MouseEvent<HTMLElement>) => {
    const at = (e.target as Element).closest('[data-tip]');
    const text = at?.getAttribute('data-tip') ?? null;
    setTip(text !== null ? { text, x: e.clientX, y: e.clientY } : null);
  };
  const shown =
    tip !== null ? (
      <div className="us-tip" style={{ left: Math.min(tip.x + 12, window.innerWidth - 440), top: tip.y + 14 }}>
        {tip.text}
      </div>
    ) : null;
  return { move, leave: () => setTip(null), shown };
}

/// Opens the folded section of the page that holds a list, and brings it into view.
function openSection(id: string) {
  const el = document.getElementById(id);
  if (el instanceof HTMLDetailsElement) el.open = true;
  el?.scrollIntoView({ block: 'nearest' });
}

/// A number, with what it counts above it and a line under it. A number that counts a list leads to it: `to`, a view of
/// its own, or `open`, the folded section of this page that holds it. A tile of nothing is muted and leads nowhere.
export function Tile({ k, v, s, to, open, none = false, hint }: {
  k: string;
  v: string;
  s?: string;
  to?: string;
  open?: string;
  none?: boolean;
  hint?: string;
}) {
  const body = (
    <>
      <span className="us-k">{k}</span>
      <span className="us-v">{v}</span>
      {s !== undefined && s !== '' ? <span className="us-s">{s}</span> : null}
    </>
  );
  if (!none && to !== undefined) {
    return (
      <Link to={to} className="us-tile us-tile-link" title={hint}>
        {body}
      </Link>
    );
  }
  if (!none && open !== undefined) {
    const id = open;
    return (
      <button type="button" className="us-tile us-tile-link" title={hint} onClick={() => openSection(id)}>
        {body}
      </button>
    );
  }
  return (
    <div className={none ? 'us-tile us-tile-none' : 'us-tile'} title={hint}>
      {body}
    </div>
  );
}

/// One part of a stacked bar or column: its value, its colour, what it says on hovering, and its name in the legend.
export type Part = { value: number; colour: string; tip: string; label: string };

/// The legend of stacked parts.
function Legend({ parts }: { parts: Array<{ colour: string; label: string }> }) {
  return (
    <div className="us-legend">
      {parts.map((p, i) => (
        <span key={i}>
          <i style={{ background: p.colour }} />
          {p.label}
        </span>
      ))}
    </div>
  );
}

/// A stacked bar across the width, in HTML so that it keeps its height, with its legend under it.
export function Stack({ parts }: { parts: Part[] }) {
  const total = parts.reduce((a, p) => a + p.value, 0);
  return (
    <>
      <div className="us-stack">
        {parts
          .filter((p) => p.value > 0)
          .map((p, i) => (
            <span key={i} style={{ width: `${pct(p.value, total)}%`, background: p.colour }} data-tip={p.tip} />
          ))}
      </div>
      <Legend parts={parts} />
    </>
  );
}

/// Bars one under another, each its label, its bar and its value, the longest the full width.
export function Bars({ items, colour }: {
  items: Array<{ label: string; value: number; text: string; tip: string }>;
  colour: string;
}) {
  const max = Math.max(...items.map((x) => x.value), 0);
  return (
    <div className="us-bars">
      {items.map((x, i) => (
        <div key={i} className="us-bar">
          <span className="us-bar-label" title={x.label}>
            {x.label}
          </span>
          <span className="us-bar-track">
            <span
              className="us-bar-fill"
              style={{ width: `${Math.max(pct(x.value, max), 0.6)}%`, background: colour }}
              data-tip={x.tip}
            />
            <span className="us-bar-value">{x.text}</span>
          </span>
        </div>
      ))}
    </div>
  );
}

/// Columns over time, oldest at the left, each stacked from its parts, all to one scale from nothing at the base to
/// the highest column at the top, which the axis names (`top`); each part says what it is on hovering, and the legend
/// names the parts (`legend`).
export function Columns({ columns, legend, top }: {
  columns: Array<{ label: string; parts: Part[] }>;
  legend: Array<{ colour: string; label: string }>;
  top: (max: number) => string;
}) {
  const totals = columns.map((c) => c.parts.reduce((a, p) => a + p.value, 0));
  const max = Math.max(...totals, 0);
  return (
    <div className="cols">
      <div className="cols-plot">
        <span className="cols-top">{top(max)}</span>
        {columns.map((c, i) => (
          <div key={i} className="cols-col">
            <div className="cols-bar" style={{ height: `${pct(totals[i], max)}%` }}>
              {c.parts
                .filter((p) => p.value > 0)
                .map((p, j) => (
                  <span key={j} style={{ height: `${pct(p.value, totals[i])}%`, background: p.colour }} data-tip={p.tip} />
                ))}
            </div>
          </div>
        ))}
      </div>
      <div className="cols-x">
        {columns.map((c, i) => (
          <span key={i}>{c.label}</span>
        ))}
      </div>
      <Legend parts={legend} />
    </div>
  );
}
