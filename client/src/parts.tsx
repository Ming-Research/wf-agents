// The parts the page shows like things with, each with one look wherever the page shows it, as the UI spec maps them
// to Primer: a round, by its name; who wrote something (the cell, or who to whom), when, and its round; what an agent
// wrote, as a block, its head and who wrote it over its text; a section under its heading and how many it holds,
// folded or open, and a sub-heading inside it; an item, by its number and title; and the form that answers or says
// something, its words and the button that sends them.

import { ChevronDownIcon, ChevronRightIcon } from '@primer/octicons-react';
import { Button, CounterLabel, Details, Flash, Heading, Link as PrimerLink, Textarea } from '@primer/react';
import { createContext, createElement, useContext, type ReactNode } from 'react';

import { roundName, time } from './format';
import { Link, workHref } from './route';
import { t } from './text';

/// What opens a round of a work where the page shows it in place.
export type OpenRound = (work: number, round: number) => void;

/// A round, by its name: a click opens it where the page shows it (`onRound`), else on its work's page.
export function RoundLink({ work, round, onRound }: { work: number; round: number; onRound?: OpenRound }) {
  if (onRound === undefined) {
    return (
      <Link to={workHref(work, { round })} className="round-link">
        {roundName(work, round)}
      </Link>
    );
  }
  return (
    <PrimerLink
      as="button"
      type="button"
      className="round-link"
      onClick={(e: { stopPropagation: () => void }) => {
        e.stopPropagation();
        onRound(work, round);
      }}
    >
      {roundName(work, round)}
    </PrimerLink>
  );
}

/// The titles of the cells of the graph a page shows, by their codes, which [`By`] gives beside a code.
export const CellTitles = createContext<ReadonlyMap<string, string>>(new Map());

/// Who wrote something: what it was first, when there is something to say (`children`); who to whom in a
/// conversation, else the cell as the flow names it, by its code and title; when; and the round, which opens as a
/// round link does.
export function By({ who, cell, at, work, round, onRound, children }: {
  who?: ReactNode;
  cell?: string | null;
  at?: number | null;
  work?: number | null;
  round?: number | null;
  onRound?: OpenRound;
  children?: ReactNode;
}) {
  const titles = useContext(CellTitles);
  const title = cell !== undefined && cell !== null ? titles.get(cell) : undefined;
  return (
    <span className="by">
      {children}
      {who !== undefined ? <span className="by-who">{who}</span> : null}
      {cell !== undefined && cell !== null && cell !== '' ? <span className="cell-id">{cell}</span> : null}
      {title !== undefined ? <span>{title}</span> : null}
      {at !== undefined && at !== null ? <span>{time(at)}</span> : null}
      {work !== undefined && work !== null && round !== undefined && round !== null ? (
        <RoundLink work={work} round={round} onRound={onRound} />
      ) : null}
    </span>
  );
}

/// What an agent wrote, or a thing of the same kind, as a block: its head (what it is, as chips and a title) and who
/// wrote it, over its text. `tone` sets it apart: new, changed or gone since before.
export function Block({ id, head, by, tone, children }: {
  id?: string;
  head?: ReactNode;
  by?: ReactNode;
  tone?: string;
  children?: ReactNode;
}) {
  return (
    <div className={tone !== undefined ? `block ${tone}` : 'block'} id={id}>
      {head !== undefined || by !== undefined ? (
        <div className="block-h">
          {head}
          {by}
        </div>
      ) : null}
      {children !== undefined && children !== null && children !== false ? (
        <div className="block-body">{children}</div>
      ) : null}
    </div>
  );
}

/// A short labelled paragraph of a block: its name run in before its text.
export function BlockField({ label, children }: { label: ReactNode; children: ReactNode }) {
  return (
    <p className="block-field">
      <span className="block-label">{label}</span> {children}
    </p>
  );
}

/// The level the next heading takes where it stands: a page converted to the UI spec gives its sections 2, and each
/// section gives what it holds one more. Elsewhere a section's heading is a sub-heading.
export const HeadingLevel = createContext(3);

/// A sub-heading: the name of a part inside a section, at the level it stands at, in the sub-heading's step.
export function SubHeading({ children, className }: { children: ReactNode; className?: string }) {
  const level = Math.min(useContext(HeadingLevel), 6);
  return createElement(`h${level}`, { className: className !== undefined ? `sub-heading ${className}` : 'sub-heading' }, children);
}

/// A section: its heading, how many it holds (`n`) and who wrote it on the line that folds it, over what it holds,
/// folded unless `open`; none when it holds nothing. A section at level 2 is a page's own, its heading in the section
/// step with a rule under it; one inside another is a sub-heading.
export function Section({ title, n, open = false, by, id, children }: {
  title: ReactNode;
  n?: number;
  open?: boolean;
  by?: ReactNode;
  id?: string;
  children: ReactNode;
}) {
  const level = Math.min(useContext(HeadingLevel), 6);
  if (n === 0) return null;
  return (
    <Details className="section" data-level={level} open={open} id={id}>
      <summary>
        <ChevronRightIcon className="fold-closed" />
        <ChevronDownIcon className="fold-open" />
        {level === 2 ? (
          <Heading as="h2" variant="small" className="section-title">
            {title}
          </Heading>
        ) : (
          createElement(`h${level}`, { className: 'section-title sub-heading' }, title)
        )}
        {n !== undefined ? <CounterLabel>{n}</CounterLabel> : null}
        {by}
      </summary>
      <div className="section-body">
        <HeadingLevel.Provider value={level + 1}>{children}</HeadingLevel.Provider>
      </div>
    </Details>
  );
}

/// An item, by its number and its title, leading to its page; `hint` says more on hovering.
export function ItemRef({ id, title, hint }: { id: number; title?: string | null; hint?: string }) {
  return (
    <Link to={`/items/${id}`} className="item-ref" title={hint}>
      <span className="item-ref-n">#{id}</span>
      {title !== undefined && title !== null && title !== '' ? ` ${title}` : null}
    </Link>
  );
}

/// The words of a form, when it takes words.
export type Words = { text: string; set: (text: string) => void; placeholder: string; rows?: number };

/// The form that answers or says something: what goes above its words (a choice, whom to); its words, which Ctrl or
/// Cmd with Enter sends too; the button that sends, greyed with why while it cannot; and what failed.
export function SendForm({ above, words, label, ready, busy, why, onSend, error }: {
  above?: ReactNode;
  words?: Words;
  label: string;
  ready: boolean;
  busy: boolean;
  /// Why it cannot be sent yet, said on hovering the button.
  why?: string;
  onSend: () => void;
  error: string | null;
}) {
  const can = ready && !busy;
  return (
    <div className="send-form">
      {above}
      {words !== undefined ? (
        <Textarea
          block
          resize="vertical"
          value={words.text}
          rows={words.rows ?? 3}
          placeholder={words.placeholder}
          title={t('send_keys_hint')}
          onChange={(e) => words.set(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && can) {
              e.preventDefault();
              onSend();
            }
          }}
          disabled={busy}
        />
      ) : null}
      <div className="send-form-row">
        <Button variant="primary" onClick={onSend} disabled={!can} title={ready ? undefined : why}>
          {label}
        </Button>
      </div>
      {error !== null ? <Flash variant="danger">{t('request_failed', { error })}</Flash> : null}
    </div>
  );
}
