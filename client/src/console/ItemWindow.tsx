// What the window beside the console shows of the item selected: its number and title, with a button to mark it read
// while something in it is new to the owner; a line of where the item stands and a line of what its work does; and two
// tabs. The first holds the body (where the item came from, the problem it solves, the plan, with each choice that
// waits for the owner where it stands in the plan and, once the plan has no choice open, a button to confirm it), each
// passage changed since the owner read it marked at its left, and under the body the notes: the owner's and the
// replies, and a box to write one, which a passage selected in the body can be quoted into and which names whom it goes
// to. The second holds the work's facts. With no item selected it says how to select one.

import { ChevronRightIcon, XIcon } from '@primer/octicons-react';
import { Button, IconButton } from '@primer/react';
import { type MouseEvent, type ReactNode, type RefObject, useRef, useState } from 'react';

import type { WorkState } from '../api';
import { useDraft } from '../drafts';
import { Markdown } from '../markdown';
import { DraftArea } from '../parts';
import { type Key, t } from '../text';
import { age } from './snap';
import { changedIn, type Note, openChoices, type Part, recipients, type WindowItem, type Who } from './window';

const WHO: Record<Who, Key> = {
  owner: 'console_who_owner',
  engineer: 'console_who_engineer',
  architect: 'console_who_architect',
  assistant: 'console_who_assistant',
};

const ago = (at: number) => age(Date.now() - at).label;

/// What a work's state is to the window, as its card colours it.
function mood(state: WorkState): { words: Key; tone: string } {
  if (state === 'running' || state === 'ready') return { words: 'console_work_running', tone: 'running' };
  if (state === 'asking') return { words: 'console_work_owner', tone: 'owner' };
  if (state === 'paused') return { words: 'console_work_paused', tone: 'paused' };
  if (state === 'halted') return { words: 'console_work_halted', tone: 'halted' };
  return { words: 'console_work_waiting', tone: 'waiting' };
}

export function ItemWindow({ item, demo, onHide }: { item: WindowItem | null; demo: boolean; onHide: () => void }) {
  if (item === null) return <p className="cw-empty">{t('console_window_empty')}</p>;
  return <ItemView key={item.id} start={item} demo={demo} onHide={onHide} />;
}

function ItemView({ start, demo, onHide }: { start: WindowItem; demo: boolean; onHide: () => void }) {
  const [w, setW] = useState(start);
  const [tab, setTab] = useState<'item' | 'work'>('item');
  const [quote, setQuote] = useState<string | null>(null);
  const [offer, setOffer] = useState<{ x: number; y: number; text: string } | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const inner = useRef<HTMLDivElement>(null);
  const box = useRef<HTMLTextAreaElement>(null);
  const read = () => {
    const clear = (ps: Part[]) => ps.map((p) => ({ ...p, changed: false }));
    setW({
      ...w,
      item: { ...w.item, unseen: false },
      background: clear(w.background),
      problem: clear(w.problem),
      plan: clear(w.plan),
    });
  };
  const choose = (at: number, option: number) =>
    setW({ ...w, plan: w.plan.map((p, i) => (i === at && p.kind === 'choice' ? { ...p, chosen: option } : p)) });
  // A passage selected in the body offers to be quoted, by a button just above it.
  const selected = () => {
    const sel = window.getSelection();
    const text = sel?.toString().trim() ?? '';
    const host = inner.current;
    if (sel === null || text === '' || host === null || sel.rangeCount === 0) {
      setOffer(null);
      return;
    }
    const at = sel.getRangeAt(0).getBoundingClientRect();
    const frame = host.getBoundingClientRect();
    setOffer({ x: at.left - frame.left + at.width / 2, y: at.top - frame.top + host.scrollTop, text });
  };
  const send = (text: string) => {
    const to = recipients(text, w.work !== null);
    const note: Note = { who: 'owner', at: Date.now(), to, quote, text };
    setW((was) => ({ ...was, notes: [...was.notes, note] }));
    setQuote(null);
    // The demo answers as the first named would; a real note waits for its reply.
    if (demo) {
      window.setTimeout(() => {
        const reply: Note = {
          who: to[0],
          at: Date.now(),
          to: ['owner'],
          quote: null,
          text: 'Seen. I will take this into the plan.',
        };
        setW((was) => ({ ...was, notes: [...was.notes, reply] }));
      }, 1200);
    }
  };
  return (
    <div className="cw-item" ref={inner}>
      <div className="cw-head">
        <span className="cw-id">{w.id}</span>
        <h2 className="cw-title">{w.title}</h2>
        {w.item.unseen || changedIn(w) ? (
          <Button size="small" onClick={read}>
            {t('console_read')}
          </Button>
        ) : null}
        <IconButton
          size="small"
          variant="invisible"
          icon={ChevronRightIcon}
          aria-label={t('console_window_hide')}
          onClick={onHide}
        />
      </div>
      <ItemLine w={w} />
      <WorkLine w={w} />
      <div className="cw-tabs" role="tablist">
        {(['item', 'work'] as const).map((k) => (
          <button
            key={k}
            type="button"
            role="tab"
            aria-selected={tab === k}
            className={tab === k ? 'on' : ''}
            onClick={() => setTab(k)}
          >
            {t(k === 'item' ? 'console_tab_item' : 'console_tab_work')}
          </button>
        ))}
      </div>
      {tab === 'work' ? (
        <WorkFacts w={w} />
      ) : (
        <>
          <div className="cw-body" onMouseUp={selected}>
            <Section title="console_background" parts={w.background} />
            <Section title="console_problem" parts={w.problem} />
            <Section title="console_plan" parts={w.plan} choose={choose} />
            {w.planWaits && !confirmed && openChoices(w) === 0 ? (
              <div className="cw-confirm">
                <p>{t('console_plan_waits')}</p>
                <Button variant="primary" onClick={() => setConfirmed(true)}>
                  {t('console_confirm_plan')}
                </Button>
              </div>
            ) : null}
            {confirmed ? <p className="cw-done">{t('console_plan_confirmed')}</p> : null}
          </div>
          {offer !== null ? (
            <button
              type="button"
              className="cw-quote-offer"
              style={{ left: offer.x, top: offer.y }}
              onMouseDown={(e: MouseEvent) => e.preventDefault()}
              onClick={() => {
                setQuote(offer.text);
                setOffer(null);
                window.getSelection()?.removeAllRanges();
                box.current?.focus();
              }}
            >
              {t('console_quote')}
            </button>
          ) : null}
          <Notes w={w} quote={quote} unquote={() => setQuote(null)} box={box} send={send} />
        </>
      )}
    </div>
  );
}

function ItemLine({ w }: { w: WindowItem }) {
  const i = w.item;
  const state: Key =
    i.standing === 'done'
      ? 'console_item_done'
      : i.standing === 'cancelled'
        ? 'console_item_cancelled'
        : 'console_item_open';
  return (
    <div className="cw-line">
      <span className={`cw-dot cw-${i.standing}`} />
      <span className="cw-strong">{t(state)}</span>
      {i.goal ? <span>{t('console_goal')}</span> : null}
      <span>{i.project}</span>
      {i.level !== null ? <span>{t('console_tier', { n: i.level })}</span> : null}
      <span>{t('console_made', { who: t(WHO[i.madeBy]).toLowerCase(), when: ago(i.madeAt) })}</span>
      {i.waitsFor.length > 0 ? <span>{t('console_waits_for', { items: i.waitsFor.join(', ') })}</span> : null}
      {i.neededBy > 0 ? <span>{t('console_needed_by', { n: i.neededBy })}</span> : null}
    </div>
  );
}

function WorkLine({ w }: { w: WindowItem }) {
  const k = w.work;
  if (k === null) return <div className="cw-line cw-muted">{t('console_no_work')}</div>;
  const m = mood(k.state);
  const at = k.stage === null ? -1 : k.stages.indexOf(k.stage);
  return (
    <div className="cw-line">
      <span className={`cw-dot cw-${m.tone}`} />
      <span className="cw-strong">{t(m.words)}</span>
      {k.waits !== null ? <span>{t('console_waits', { what: k.waits })}</span> : null}
      <span>{t('console_work', { n: k.id })}</span>
      {at >= 0 ? <span>{t('console_stage', { stage: k.stages[at], at: at + 1, of: k.stages.length })}</span> : null}
      <span className="cw-mono">{age(Date.now() - k.from).label}</span>
      {k.sentBack > 0 ? <span>{t('console_sent_back', { n: k.sentBack })}</span> : null}
    </div>
  );
}

function Section({
  title,
  parts,
  choose,
}: {
  title: Key;
  parts: Part[];
  choose?: (at: number, option: number) => void;
}) {
  if (parts.length === 0) return null;
  return (
    <section className="cw-section">
      <h3>{t(title)}</h3>
      {parts.map((p, i) => (
        <div key={i} className={p.changed === true ? 'cw-part cw-changed' : 'cw-part'}>
          {p.kind === 'text' ? <Markdown text={p.text} /> : <Choice p={p} pick={(o) => choose?.(i, o)} />}
        </div>
      ))}
    </section>
  );
}

function Choice({ p, pick }: { p: Extract<Part, { kind: 'choice' }>; pick: (option: number) => void }) {
  const [mark, setMark] = useState<number | null>(p.recommended);
  const done = p.chosen !== null;
  return (
    <div className={done ? 'cw-choice cw-choice-done' : 'cw-choice'}>
      <p className="cw-choice-q">{p.question}</p>
      {p.options.map((o, i) => (
        <button
          key={o.label}
          type="button"
          disabled={done}
          className={(done ? p.chosen === i : mark === i) ? 'cw-option on' : 'cw-option'}
          onClick={() => setMark(i)}
        >
          <span className="cw-option-label">
            {o.label}
            {p.recommended === i ? <span className="cw-tag">{t('console_recommended')}</span> : null}
            {done && p.chosen === i ? <span className="cw-tag">{t('console_chosen')}</span> : null}
          </span>
          <span className="cw-option-detail">{o.detail}</span>
        </button>
      ))}
      {done ? null : (
        <Button size="small" variant="primary" disabled={mark === null} onClick={() => mark !== null && pick(mark)}>
          {t('console_choose')}
        </Button>
      )}
    </div>
  );
}

function Notes({
  w,
  quote,
  unquote,
  box,
  send,
}: {
  w: WindowItem;
  quote: string | null;
  unquote: () => void;
  box: RefObject<HTMLTextAreaElement | null>;
  send: (text: string) => void;
}) {
  // The words are kept as a draft of the item's, so that they are not lost when the window shows another item.
  const draft = useDraft(`item-note:${w.id}`);
  const text = draft.text;
  const setText = draft.setText;
  const to = recipients(text, w.work !== null);
  const names = to.map((x) => t(WHO[x])).join(', ');
  // Typing `@` offers the names one can write to, the engineer only while a work goes on.
  const naming = /(^|\s)@(\w*)$/.exec(text);
  const offered: Who[] = (['engineer', 'architect', 'assistant'] as Who[]).filter(
    (x) => (x !== 'engineer' || w.work !== null) && x.startsWith((naming?.[2] ?? '').toLowerCase()),
  );
  const name = (x: Who) => {
    setText(text.replace(/@(\w*)$/, `@${x} `));
    box.current?.focus();
  };
  return (
    <section className="cw-notes">
      <h3>{t('console_notes')}</h3>
      {w.notes.map((n, i) => (
        <div key={i} className={n.who === 'owner' ? 'cw-note cw-note-owner' : 'cw-note'}>
          <div className="cw-note-head">
            <span className="cw-strong">{t(WHO[n.who])}</span>
            <span>{t('console_to', { who: n.to.map((x) => t(WHO[x])).join(', ') })}</span>
            <span className="cw-mono">{t('console_ago', { when: ago(n.at) })}</span>
          </div>
          {n.quote !== null ? <blockquote className="cw-note-quote">{n.quote}</blockquote> : null}
          <Markdown text={n.text} />
        </div>
      ))}
      <div className="cw-compose">
        {quote !== null ? (
          <div className="cw-quoted">
            <span>{quote}</span>
            <IconButton
              size="small"
              variant="invisible"
              icon={XIcon}
              aria-label={t('console_unquote')}
              onClick={unquote}
            />
          </div>
        ) : null}
        <DraftArea ref={box} draft={draft} rows={3} block placeholder={t('console_note_placeholder')} />
        {naming !== null && offered.length > 0 ? (
          <div className="cw-names">
            {offered.map((x) => (
              <button key={x} type="button" onClick={() => name(x)}>
                @{x} · {t(WHO[x])}
              </button>
            ))}
          </div>
        ) : null}
        <div className="cw-compose-foot">
          <span className="cw-muted">{t('console_goes_to', { who: names })}</span>
          <Button
            size="small"
            variant="primary"
            disabled={text.trim() === ''}
            onClick={() => {
              send(text.trim());
              setText('');
            }}
          >
            {t('console_send')}
          </Button>
        </div>
      </div>
    </section>
  );
}

function WorkFacts({ w }: { w: WindowItem }) {
  const k = w.work;
  if (k === null) return <p className="cw-muted">{t('console_no_work')}</p>;
  const at = k.stage === null ? -1 : k.stages.indexOf(k.stage);
  const rows: [Key, ReactNode][] = [
    [
      'console_fact_state',
      `${t(mood(k.state).words)}${k.waits !== null ? ` ${t('console_waits', { what: k.waits })}` : ''}`,
    ],
    [
      'console_fact_stages',
      <span className="cw-stages">
        {k.stages.map((s, i) => (
          <span key={s} className={i < at ? 'gone' : i === at ? 'here' : ''}>
            {s}
          </span>
        ))}
      </span>,
    ],
    ['console_fact_rounds', String(k.rounds)],
    ['console_fact_cost', `$${k.costUsd.toFixed(2)}`],
    ['console_fact_started', t('console_ago', { when: age(Date.now() - k.from).label })],
    ['console_fact_tier', k.tier],
    [
      'console_fact_time',
      <span className="cw-bar">
        {[...k.bar].map((c, i) => (
          <i key={i} className={c === 'm' ? 'm' : c === 't' ? 't' : ''} />
        ))}
      </span>,
    ],
  ];
  return (
    <dl className="cw-facts">
      {rows.map(([key, value]) => (
        <div key={key}>
          <dt>{t(key)}</dt>
          <dd>{value}</dd>
        </div>
      ))}
    </dl>
  );
}
