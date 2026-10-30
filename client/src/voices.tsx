// The three voices a panel's texts are in, each in one look wherever the page shows it (ui-spec.md): the page's own
// words, plain; what is sent to an agent, a sheet, filled, with a thick edge at its left; what an agent handed back, a
// box with a thin border. Inside a sheet or a box the page adds only names, numbers and controls: a name small and
// muted, what each round fills in as a dashed placeholder, and the control that opens a text over 1,000 characters,
// which shows its first three lines until then. And the parts a panel about one thing is laid out with: its overview
// of label and value pairs, its sections folded at first, the parts inside a section, and rows that open, each headed
// by a name in words with its code small and muted after it.

import { ChevronDownIcon, ChevronRightIcon, ChevronUpIcon } from '@primer/octicons-react';
import { CounterLabel, Details, Heading } from '@primer/react';
import { createElement, type ReactNode, useState } from 'react';

import { LinkButton } from './parts';
import { chars, FOLD_AT } from './round';
import { t, tn } from './text';

export { chars } from './round';

/// A number as the page writes it, with its thousands apart.
export const counted = (n: number) => n.toLocaleString('en-US');

/// A count of characters in words, its thousands apart: "1 character", "10,489 characters".
export const charCount = (n: number): string => tn('chars', n, { n: counted(n) });

/// What is sent to an agent: a sheet. `mono` for a message as sent, a format, data as given and what a tool printed.
export function Sheet({ mono, id, children }: { mono?: boolean; id?: string; children: ReactNode }) {
  return (
    <div className={mono ? 'sheet sheet-mono' : 'sheet'} id={id}>
      {children}
    </div>
  );
}

/// What an agent handed back: a box. `mono` for its code and data; `list` for rows of one box, each parted by a line.
export function Said({ mono, list, children }: { mono?: boolean; list?: boolean; children: ReactNode }) {
  return <div className={['said', mono ? 'said-mono' : '', list ? 'said-list' : ''].filter((c) => c !== '').join(' ')}>{children}</div>;
}

/// A text inside a sheet or a box: whole when it is 1,000 characters or fewer; else its first three lines and, under
/// them, what opens it, its size and `more` (such as its share of the message).
export function Long({ text, more, children }: { text: string; more?: string; children?: (text: string) => ReactNode }) {
  const [open, setOpen] = useState(false);
  const n = chars(text);
  const shown = children !== undefined ? children(text) : text;
  if (n <= FOLD_AT) return <div className="long">{shown}</div>;
  return (
    <>
      <div className={open ? 'long' : 'long long-folded'}>{shown}</div>
      <LinkButton className="long-more" onClick={() => setOpen(!open)}>
        {open ? <ChevronUpIcon /> : <ChevronDownIcon />}
        {open ? t('long_less') : t('long_more', { chars: charCount(n), more: more ?? '' })}
      </LinkButton>
    </>
  );
}

/// What each round fills in, named where it goes, inside a sheet.
export function Hole({ children }: { children: ReactNode }) {
  return <span className="hole">{children}</span>;
}

/// A text with what each round fills in (`‹work›`, `‹n›`, `‹id›`, `‹cell›`, `‹name›`) drawn as placeholders.
export function Filled({ text }: { text: string }) {
  return (
    <>
      {text.split(/(‹[^›]*›)/).map((part, i) =>
        part.startsWith('‹') && part.endsWith('›') ? <Hole key={i}>{part.slice(1, -1)}</Hole> : part,
      )}
    </>
  );
}

/// A name the page gives a value inside a sheet or a box: small and muted.
export function InnerName({ children }: { children: ReactNode }) {
  return <span className="inner-name">{children}</span>;
}

/// A code the flow names a thing by, after its name in words: monospace, small and muted.
export function Code({ children }: { children: ReactNode }) {
  return <span className="pp-code">{children}</span>;
}

/// A code inside a sentence of the page: monospace, in the colour of the text round it.
export function Name({ children }: { children: ReactNode }) {
  return <span className="cell-id">{children}</span>;
}

/// Facts about a thing: its type, flags and where it comes from, as small muted words.
export function Facts({ children }: { children: ReactNode }) {
  return <span className="pp-facts">{children}</span>;
}

/// Facts joined by " · ", the empty ones left out.
export function joined(xs: Array<ReactNode | null | undefined | false>): ReactNode[] {
  const out: ReactNode[] = [];
  for (const x of xs) {
    if (x === null || x === undefined || x === false || x === '') continue;
    if (out.length > 0) out.push(' · ');
    out.push(x);
  }
  return out;
}

/// The overview a panel about one thing opens with: a row of label and value for each question it answers, the
/// value's second line, where it has one, small and muted.
export function Overview({ rows }: { rows: Array<{ label: string; value: ReactNode; sub?: ReactNode } | null> }) {
  return (
    <dl className="overview">
      {rows.map((r) =>
        r === null ? null : (
          <div key={r.label} className="overview-row">
            <dt>{r.label}</dt>
            <dd>
              <span>{r.value}</span>
              {r.sub !== undefined && r.sub !== null ? <span className="overview-sub">{r.sub}</span> : null}
            </dd>
          </div>
        ),
      )}
    </dl>
  );
}

/// What a click on the line that folds a part does: the page, not the browser, opens and folds it, so that what is
/// open is always what the page draws.
function toggle(open: boolean, set: (open: boolean) => void) {
  return (e: { preventDefault: () => void }) => {
    e.preventDefault();
    set(!open);
  };
}

/// A section of a panel: its heading in the section's step, its count and what it holds in small muted words on the
/// line that folds it, a rule under that line; open as the owner left it (`open`, `onOpen`).
export function PanelSection({ title, n, facts, open, onOpen, children }: {
  title: string;
  n?: number;
  facts?: ReactNode;
  open: boolean;
  onOpen: (open: boolean) => void;
  children: ReactNode;
}) {
  return (
    <Details className="pp-section" open={open}>
      <summary onClick={toggle(open, onOpen)}>
        <ChevronRightIcon className="fold-closed" />
        <ChevronDownIcon className="fold-open" />
        <Heading as="h2" variant="small" className="pp-section-title">
          {title}
        </Heading>
        {n !== undefined ? <CounterLabel>{n}</CounterLabel> : null}
        {facts !== undefined ? <Facts>{facts}</Facts> : null}
      </summary>
      {open ? <div className="pp-section-body">{children}</div> : null}
    </Details>
  );
}

/// A part inside a section: its name as a sub-heading (`as`, h3 by default) with its code and its facts after it,
/// over what it holds.
export function Part({ title, code, n, facts, as = 'h3', id, children }: {
  title: ReactNode;
  code?: ReactNode;
  n?: number;
  facts?: ReactNode;
  as?: 'h3' | 'h4' | 'h5' | 'h6';
  id?: string;
  children?: ReactNode;
}) {
  return (
    <div className="pp-part" id={id}>
      <Named title={title} code={code} n={n} facts={facts} as={as} />
      {children}
    </div>
  );
}

/// A thing the flow names, where its name heads what the owner reads: its name in words as a sub-heading, its code
/// small and muted, its count, its facts.
export function Named({ title, code, n, facts, as = 'h3' }: {
  title: ReactNode;
  code?: ReactNode;
  n?: number;
  facts?: ReactNode;
  as?: 'h3' | 'h4' | 'h5' | 'h6';
}) {
  return (
    <div className="pp-heading">
      {createElement(as, { className: 'pp-sub' }, title)}
      {code !== undefined && code !== null ? <Code>{code}</Code> : null}
      {n !== undefined ? <CounterLabel>{n}</CounterLabel> : null}
      {facts !== undefined && facts !== null ? <Facts>{facts}</Facts> : null}
    </div>
  );
}

/// A part inside a section that folds, its name a sub-heading on the line that opens it; folded unless `open`.
export function FoldPart({ title, n, facts, open = false, children }: {
  title: ReactNode;
  n?: number;
  facts?: ReactNode;
  open?: boolean;
  children: ReactNode;
}) {
  const [shown, setShown] = useState(open);
  return (
    <details className="pp-fold" open={shown}>
      <summary onClick={toggle(shown, setShown)}>
        <ChevronRightIcon className="fold-closed" />
        <ChevronDownIcon className="fold-open" />
        <h3 className="pp-sub">{title}</h3>
        {n !== undefined ? <CounterLabel>{n}</CounterLabel> : null}
        {facts !== undefined ? <Facts>{facts}</Facts> : null}
      </summary>
      {shown ? <div className="pp-fold-body">{children}</div> : null}
    </details>
  );
}

/// Rows one under another, each a thing that opens.
export function Rows({ children }: { children: ReactNode }) {
  return <div className="pp-rows">{children}</div>;
}

/// A row: its chevron, its number if it has one, its name, its code and its facts, wrapping under the name; what it
/// holds under it, set in by the chevron's width, drawn only once opened. A row that holds nothing has no chevron.
export function Row({ no, name, code, facts, id, open, onOpen, children }: {
  no?: number;
  name: ReactNode;
  code?: ReactNode;
  facts?: ReactNode;
  id?: string;
  /// Open from outside, as when a link elsewhere on the panel leads to it.
  open?: boolean;
  onOpen?: (open: boolean) => void;
  children?: ReactNode;
}) {
  const [own, setOwn] = useState(false);
  const shown = open ?? own;
  const text = (
    <span className="pp-row-text">
      {no !== undefined ? <span className="pp-no">{no}</span> : null}
      <span className="pp-row-name">{name}</span>
      {code !== undefined && code !== null ? <Code>{code}</Code> : null}
      {facts !== undefined && facts !== null && facts !== false ? <Facts>{facts}</Facts> : null}
    </span>
  );
  if (children === undefined || children === null || children === false) {
    return (
      <div className="pp-row-line" id={id}>
        <span />
        {text}
      </div>
    );
  }
  return (
    <details className="pp-row" id={id} open={shown}>
      <summary
        onClick={toggle(shown, (now) => {
          setOwn(now);
          onOpen?.(now);
        })}
      >
        <ChevronRightIcon className="fold-closed" />
        <ChevronDownIcon className="fold-open" />
        {text}
      </summary>
      {shown ? <div className="pp-row-body">{children}</div> : null}
    </details>
  );
}

/// Words the page says over a sheet or a box: what the text is, where it came from; small and muted.
export function Over({ children }: { children: ReactNode }) {
  return <span className="pp-over">{children}</span>;
}

/// Label and value pairs in two columns, for a list the page lays out by its names (where a row leads, the checks).
export function Pairs({ rows }: { rows: Array<[ReactNode, ReactNode] | null> }) {
  return (
    <div className="pp-pairs">
      {rows.map((r, i) =>
        r === null ? null : (
          <div key={i} className="pp-pair">
            <span>{r[0]}</span>
            <span>{r[1]}</span>
          </div>
        ),
      )}
    </div>
  );
}
