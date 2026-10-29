// The parts the page shows like things with, each with one look wherever the page shows it: a round, by its name; who
// wrote something (the cell, or who to whom), when, and its round; what an agent wrote, as a block, its head and who
// wrote it over its text; a section under its title and how many it holds, folded or open; an item, by its number and
// title; and the form that answers or says something, its words and the button that sends them.

import { createContext, useContext, type ReactNode } from 'react';

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
    <button
      type="button"
      className="link round-link"
      onClick={(e) => {
        e.stopPropagation();
        onRound(work, round);
      }}
    >
      {roundName(work, round)}
    </button>
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

/// A short labelled paragraph of a block.
export function BlockField({ label, children }: { label: ReactNode; children: ReactNode }) {
  return (
    <p className="block-field">
      <span className="block-label">{label}</span> {children}
    </p>
  );
}

/// A section: its title, how many it holds (`n`) and who wrote it, over what it holds, folded unless `open`; none when
/// it holds nothing.
export function Section({ title, n, open = false, by, id, children }: {
  title: ReactNode;
  n?: number;
  open?: boolean;
  by?: ReactNode;
  id?: string;
  children: ReactNode;
}) {
  if (n === 0) return null;
  return (
    <details className="section" open={open} id={id}>
      <summary>
        <span className="section-title">{title}</span>
        {n !== undefined ? <span className="section-n">{t('section_n', { n })}</span> : null}
        {by}
      </summary>
      <div className="section-body">{children}</div>
    </details>
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
        <textarea
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
        <button type="button" className="decide-button" onClick={onSend} disabled={!can} title={ready ? undefined : why}>
          {label}
        </button>
      </div>
      {error !== null ? <p className="bad">{t('request_failed', { error })}</p> : null}
    </div>
  );
}
