// The page's parts, its own small design system (look.md): the bar, the view's line, a button, a segmented switch, a
// tag, a label, a list row, the docked panel, and the few signs the chart's heads use. Each is drawn by its class in
// styles/parts.css from the tokens in styles/tokens.css.

import type { PointerEvent as Press, ReactNode } from 'react';

/// The bar: the page's name, the views as text tabs (the one shown marked), and at its right what belongs to the page
/// as a whole.
export function Bar({ name, views, current, end }: {
  name: string;
  views: { name: string; href: string }[];
  current: string;
  end?: ReactNode;
}) {
  return (
    <header className="bar">
      <span className="bar-name">{name}</span>
      <nav className="bar-tabs" aria-label="Views">
        {views.map((v) => (
          <a key={v.href} className="bar-tab" href={v.href} aria-current={v.href === current ? 'page' : undefined}>
            {v.name}
          </a>
        ))}
      </nav>
      {end !== undefined && <div className="bar-end">{end}</div>}
    </header>
  );
}

/// A button: words in a hairline box; `primary` for the one action a view exists for.
export function Button({ primary, title, onClick, children }: {
  primary?: boolean;
  title?: string;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button type="button" className={primary ? 'button button-primary' : 'button'} title={title} onClick={onClick}>
      {children}
    </button>
  );
}

/// A segmented switch between two or three choices, each a button pressed while chosen.
export function Switch<T extends string>({ label, choices, value, onChange }: {
  label: string;
  choices: { value: T; name: string; title?: string }[];
  value: T;
  onChange: (value: T) => void;
}) {
  return (
    <span className="switch" role="group" aria-label={label}>
      {choices.map((c) => (
        <button
          key={c.value}
          type="button"
          className="switch-choice"
          aria-pressed={c.value === value}
          title={c.title}
          onClick={() => onChange(c.value)}
        >
          {c.name}
        </button>
      ))}
    </span>
  );
}

/// An edge the owner drags to change the width of what it is the edge of: a hairline that shows under the pointer and
/// the cursor of a resize. `onDrag` is told the width at the start of the drag and how far the pointer has moved since
/// (to the right for an edge at the right, to the left for one at the left); a double click resets the width.
export function Edge({ side, label, start, onDrag, onReset }: {
  side: 'left' | 'right';
  label: string;
  /// The width when a drag starts.
  start: () => number;
  onDrag: (width: number) => void;
  onReset: () => void;
}) {
  const press = (e: Press<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    const el = e.currentTarget;
    const from = e.clientX;
    const was = start();
    el.setPointerCapture(e.pointerId);
    const move = (m: PointerEvent) => onDrag(was + (side === 'right' ? m.clientX - from : from - m.clientX));
    const up = () => {
      el.removeEventListener('pointermove', move);
      el.removeEventListener('pointerup', up);
      el.removeEventListener('pointercancel', up);
    };
    el.addEventListener('pointermove', move);
    el.addEventListener('pointerup', up);
    el.addEventListener('pointercancel', up);
  };
  return (
    <div
      className={`edge edge-${side}`}
      role="separator"
      aria-orientation="vertical"
      aria-label={label}
      title={`${label}; a double click sets it back`}
      onPointerDown={press}
      onDoubleClick={(e) => {
        e.stopPropagation();
        onReset();
      }}
      onClick={(e) => e.stopPropagation()}
    />
  );
}

/// What a change did to a thing.
export type Status = 'same' | 'added' | 'removed' | 'changed';

export const STATUS_WORDS: Record<Exclude<Status, 'same'>, string> = {
  added: 'put in',
  removed: 'taken out',
  changed: 'changed',
};

/// A tag: a few words of a state, in its colour on its tint; nothing for a thing the change left as it was.
export function Tag({ status }: { status: Status }) {
  if (status === 'same') return null;
  return <span className={`tag tag-${status}`}>{STATUS_WORDS[status]}</span>;
}

/// A label: what a part holds or what a value is, small, upper case and muted.
export function Label({ children }: { children: ReactNode }) {
  return <span className="label">{children}</span>;
}

/// A list row: one thing, at least a row high.
export function ListRow({ children }: { children: ReactNode }) {
  return <li className="list-row">{children}</li>;
}

/// A word that opens something on the page.
export function LinkButton({ onClick, children }: { onClick: () => void; children: ReactNode }) {
  return (
    <button type="button" className="link" onClick={onClick}>
      {children}
    </button>
  );
}

/// The panel docked at the right of a view, laid out as the console's drawer: a handle on its left edge that folds it
/// away and brings it back; a head with the thing's id and title on one line and what it belongs to at the right; one
/// or more status lines under it; then its sections; and room at its bottom for a box that answers (`foot`).
export function Panel({ label, id, title, belongs, lines, folded, onFold, foot, edge, children }: {
  /// What the panel is about, for a reader of the screen.
  label: string;
  id?: ReactNode;
  /// None for a panel that has nothing to show yet, which has no head.
  title?: ReactNode;
  belongs?: ReactNode;
  lines?: ReactNode;
  folded: boolean;
  onFold: () => void;
  foot?: ReactNode;
  /// The edge that drags the panel's width.
  edge?: ReactNode;
  children: ReactNode;
}) {
  return (
    <aside className={folded ? 'drawer drawer-folded' : 'drawer'} aria-label={label}>
      {!folded && edge}
      <button
        type="button"
        className="drawer-tab"
        aria-expanded={!folded}
        title={folded ? 'Show the panel' : 'Fold the panel away (Esc closes it)'}
        aria-label={folded ? 'Show the panel' : 'Fold the panel away'}
        onClick={onFold}
      >
        {folded ? '‹' : '›'}
      </button>
      <div className="drawer-inner" hidden={folded}>
        {title !== undefined && (
          <header className="drawer-head">
            {id !== undefined && <span className="drawer-id">{id}</span>}
            <h2 className="drawer-title">{title}</h2>
            {belongs !== undefined && <span className="drawer-belongs">{belongs}</span>}
          </header>
        )}
        {lines}
        <div className="drawer-body">{children}</div>
        {foot !== undefined && <div className="drawer-foot">{foot}</div>}
      </div>
    </aside>
  );
}

/// The colours a status line's dot takes: none; green for put in or done; red for taken out or failed; orange for
/// changed or waiting for the owner; blue for running; the way back's purple.
export type Tone = 'quiet' | 'added' | 'removed' | 'changed' | 'running' | 'back';

/// A status line of a panel: a dot in the colour of what it says, a word in semibold, and the facts after it.
export function StatusLine({ tone = 'quiet', word, children }: {
  tone?: Tone;
  word: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div className="status-line">
      <span className={`dot dot-${tone}`} aria-hidden="true" />
      <span className="status-word">{word}</span>
      {children !== undefined && <span className="muted">{children}</span>}
    </div>
  );
}

/// A section of a panel: a small semibold label, how many it holds, and what it holds. `last` ends a group of
/// sections with a hairline.
export function PanelPart({ label, count, last, children }: {
  label: ReactNode;
  count?: number;
  last?: boolean;
  children: ReactNode;
}) {
  return (
    <section className={last ? 'panel-part panel-part-last' : 'panel-part'}>
      <h3 className="panel-part-label">
        {label}
        {count !== undefined && <span className="muted mono"> {count}</span>}
      </h3>
      {children}
    </section>
  );
}

/// The signs a block's name carries: people write it, the outside writes it, it is shown. Drawn by the page, 12px, in
/// the colour of the text round them.
export type SignName = 'people' | 'outside' | 'shown';

export const SIGN_WORDS: Record<SignName, string> = {
  people: 'people write it',
  outside: 'the outside writes it',
  shown: 'shown on the page or read by a test',
};

export function Sign({ name, className }: { name: SignName; className?: string }) {
  const shape = {
    people: (
      <>
        <circle cx="6" cy="4" r="2.25" />
        <path d="M1.75 11c0-2.5 1.9-4 4.25-4s4.25 1.5 4.25 4" />
      </>
    ),
    outside: (
      <>
        <circle cx="6" cy="6" r="4.75" />
        <path d="M1.25 6h9.5M6 1.25c-2.4 2.6-2.4 6.9 0 9.5M6 1.25c2.4 2.6 2.4 6.9 0 9.5" />
      </>
    ),
    shown: (
      <>
        <path d="M1 6c1.4-2.3 3-3.5 5-3.5s3.6 1.2 5 3.5c-1.4 2.3-3 3.5-5 3.5S2.4 8.3 1 6z" />
        <circle cx="6" cy="6" r="1.5" />
      </>
    ),
  }[name];
  return (
    <svg
      className={className ? `sign ${className}` : 'sign'}
      viewBox="0 0 12 12"
      width="12"
      height="12"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.25"
      role="img"
      aria-label={SIGN_WORDS[name]}
    >
      <title>{SIGN_WORDS[name]}</title>
      {shape}
    </svg>
  );
}
