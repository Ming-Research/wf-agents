// What the window beside the console shows of the item selected: its number, its title, which brings the item into view
// in the tree, and its project; a line of where the
// item stands and a line of what its work does, with the work's stages and where its time went as its row in the list
// of works has them; and two tabs, beside them a button to mark the item read while something in it is new to the
// owner. The first holds the body (where the item came from, the problem it solves, the plan, with each choice that
// waits for the owner where it stands in the plan and, once the plan has no choice open, a button to confirm it), each
// passage changed since the owner read it marked at its left, and under the body the notes: the owner's and the
// replies, and a box to write one, which a passage selected in the body can be quoted into and which names whom it goes
// to. The second holds what of the work the lines do not say. With no item selected it says how to select one.

import { QuoteIcon, XIcon } from '@primer/octicons-react';
import { Button, IconButton } from '@primer/react';
import { type MouseEvent, type ReactNode, type RefObject, useRef, useState } from 'react';

import type { WorkState } from '../api';
import { useDraft } from '../drafts';
import { Markdown } from '../markdown';
import { TimeTrack, barSpans } from '../charts';
import { StageCells } from '../pages/StageBar';
import { DraftArea } from '../parts';
import { Link, workHref } from '../route';
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

export function ItemWindow({ item, demo, onTitle }: { item: WindowItem | null; demo: boolean; onTitle: () => void }) {
  if (item === null) return <p className="cw-empty">{t('console_window_empty')}</p>;
  return <ItemView key={item.id} start={item} demo={demo} onTitle={onTitle} />;
}

function ItemView({ start, demo, onTitle }: { start: WindowItem; demo: boolean; onTitle: () => void }) {
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
        <h2 className="cw-title">
          <button type="button" className="cw-title-link" title={t('console_show_in_tree')} onClick={onTitle}>
            {w.title}
          </button>
        </h2>
        <span className="cw-project">{w.item.project}</span>
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
        {w.item.unseen || changedIn(w) ? (
          <Button size="small" className="cw-read" onClick={read}>
            {t('console_read')}
          </Button>
        ) : null}
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
              <QuoteIcon size={12} />
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
      <span className={`cw-dot dot-${i.standing}`} />
      <span className="cw-strong">{t(state)}</span>
      {i.goal ? <span>{t('console_goal')}</span> : null}
      {i.level !== null ? <span>{t('console_tier', { n: i.level })}</span> : null}
      <span>{t('console_added', { when: ago(i.madeAt) })}</span>
      {i.waitsFor.length > 0 ? <span>{t('console_waits_for', { items: i.waitsFor.join(', ') })}</span> : null}
    </div>
  );
}

function WorkLine({ w }: { w: WindowItem }) {
  const k = w.work;
  if (k === null) return <div className="cw-line cw-muted">{t('console_no_work')}</div>;
  const m = mood(k.state);
  return (
    <div className="cw-line">
      <span className={`cw-dot dot-${m.tone}`} />
      {/* What it waits for says what it does, in one phrase; one that waits for nothing it names says how it stands. */}
      <span className="cw-strong">{k.waits !== null ? t('console_waiting_for', { what: k.waits }) : t(m.words)}</span>
      <Link to={workHref(k.id)} className="cw-mono" title={t('console_work_open', { n: k.id })}>
        {t('console_work_name', { n: k.id })}
      </Link>
      {/* How far it has come and where its time went, together at the line's right end. */}
      <span className="cw-progress">
        <StageCells
          stages={k.stages.map((stage) => ({ stage, letter: stage.charAt(0).toUpperCase() }))}
          at={k.stage}
          state={k.state}
          waiting={m.tone !== 'running'}
        />
        <span className="cw-track">
          <TimeTrack spans={barSpans(k.bar)} />
        </span>
      </span>
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
  const rows: [Key, ReactNode][] = [
    ['console_fact_rounds', String(k.rounds)],
    ['console_fact_cost', `$${k.costUsd.toFixed(2)}`],
    ['console_fact_started', t('console_ago', { when: age(Date.now() - k.from).label })],
    ['console_fact_tier', k.tier],
    ['console_fact_sent_back', String(k.sentBack)],
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
