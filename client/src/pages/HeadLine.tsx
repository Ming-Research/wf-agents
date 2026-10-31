// The line over the chart of a work's page (an architect's page has none), and the parts it says how a work goes on
// with, which the list of works shares: a round running as a state chip and how long it has run, ticking;
// what holds the work back, with why on hovering; what it waits for, linked to where the owner gives it; and where it
// is, with that it is on an older flow version, or was moved to the one it is on, as a tag's chip saying in full on
// hovering what that means. Only a stop of the engine's or the architect's says why on the line, since the owner looks
// at it first; the line's facts are muted words joined by a bar, its conversation a link among them.

import { ArrowLeftIcon } from '@primer/octicons-react';
import type { ReactNode } from 'react';

import type { Status, WorkSummary } from '../api';
import { type Course, heldName, StateChip, TagChip, WorkBadge } from '../components';
import { held, runningWords, takeoverWords, waitsWords } from '../course';
import { duration } from '../format';
import { useNow, useStatus } from '../live';
import { LabelLink } from '../parts';
import { href, Link, workHref } from '../route';
import { t, tn } from '../text';
import { joined, SEP } from '../voices';

/// A round running, as a state chip in the colour of what goes on, and how long it has run, ticking, from `since`.
export function Running({ words, since }: { words: string; since: number | null }) {
  const now = useNow(since !== null);
  return (
    <>
      <StateChip tone="accent">{words}</StateChip>
      {since !== null ? <span className="muted">{duration(Math.max(0, now - since))}</span> : null}
    </>
  );
}

/// What holds a work back, as a state chip, in the danger colour for a stop of the engine's, with why on hovering.
export function Held({ c, why }: { c: Course; why: string | null }) {
  const h = heldName(c);
  return (
    <StateChip tone={h.problem ? 'danger' : 'attention'} title={why ?? undefined}>
      {h.name}
    </StateChip>
  );
}

/// How a work goes on, on one line: a round of it running and how long it has run, or what holds it back, then what it
/// waits for, linked to where the owner gives it, or, what it waited for come, that it waits for a free slot, saying
/// why on hovering, and its design cards and asks waiting for the owner as small chips; then why the engine or the
/// architect stopped it, or why it ended.
export function State({ w, c }: { w: WorkSummary; c: Course }) {
  if (c.over) {
    return (
      <span className="course">
        <WorkBadge state={w.state} />
        {w.ended ? <span className="why">{w.ended}</span> : null}
      </span>
    );
  }
  const why = held(c, w);
  const parts: ReactNode[] = [];
  if (c.running !== null) {
    parts.push(<Running key="running" words={runningWords(c)} since={w.running?.started_at ?? null} />);
  } else if (why !== null) {
    parts.push(<Held key="held" c={c} why={why} />);
  }
  const waits = c.waits;
  if (waits !== null) {
    const to =
      waits.kind === 'answer' || waits.kind === 'ruling' || waits.kind === 'architect'
        ? href({ page: 'messages', open: `work:${w.id}` })
        : waits.kind === 'items' && waits.items.length > 0
          ? `/items/${waits.items[0]}`
          : workHref(w.id);
    parts.push(
      <LabelLink key="waits" to={to} variant="attention" title={waitsWords(waits)}>
        {waitsWords(waits)}
      </LabelLink>,
    );
  }
  if (c.slot && c.running === null) parts.push(<WorkBadge key="slot" state="waiting_for_slot" />);
  if (c.running === null && why === null && waits === null && !c.slot) {
    parts.push(
      c.started ? (
        <StateChip key="ready" tone="neutral">
          {t('course_ready_started')}
        </StateChip>
      ) : (
        <WorkBadge key="ready" state="ready" />
      ),
    );
  }
  // The asks and design cards waiting for the owner that what the work waits for does not name already: they are the
  // owner's to do, not what holds the work up.
  const asks = w.open_asks - (waits?.kind === 'answer' ? 1 : 0);
  const cards = w.open_design_cards - (waits?.kind === 'ruling' ? 1 : 0);
  if (asks > 0) {
    parts.push(
      <LabelLink key="asks" to={href({ page: 'messages', open: `work:${w.id}` })} variant="attention" title={t('open_asks_n', { n: asks })}>
        {t('open_asks_chip', { n: asks })}
      </LabelLink>,
    );
  }
  if (cards > 0) {
    parts.push(
      <LabelLink
        key="cards"
        to={href({ page: 'messages', open: `work:${w.id}` })}
        variant="attention"
        title={tn('open_design_cards_later_n', cards)}
      >
        {t('open_design_cards_chip', { n: cards })}
      </LabelLink>,
    );
  }
  return (
    <span className="course">
      {parts}
      {(c.held === 'engine' || c.held === 'architect') && w.paused ? <span className="why">{w.paused}</span> : null}
    </span>
  );
}

/// That a work is on an older flow version, as a tag's chip, saying on hovering what taking it over will do.
export function OldFlow({ w, status }: { w: WorkSummary; status: Status | null }) {
  return <TagChip title={takeoverWords(w, status)}>{t('course_outdated')}</TagChip>;
}

/// Where a work is: the cell's code, that the work is on an older flow version or was moved to the one it is on, and the
/// cell's title; with `stage`, its stage after them.
export function Where({ w, c, stage = false }: { w: WorkSummary; c: Course; stage?: boolean }) {
  const status = useStatus();
  const s = stage && w.stage !== null ? w.stage.replaceAll('_', ' ') : '';
  return (
    <>
      <span className="cell-id">{w.at}</span>{' '}
      {c.outdated ? (
        <>
          <OldFlow w={w} status={status} />{' '}
        </>
      ) : w.moved_from !== null ? (
        <>
          <TagChip title={t('course_moved_hint', { from: w.moved_from, version: w.flow_version })}>
            {t('course_moved', { version: w.flow_version })}
          </TagChip>{' '}
        </>
      ) : null}
      {w.at_title}
      {s !== '' ? (
        <span className="muted">
          {SEP}
          {s}
        </span>
      ) : null}
    </>
  );
}

/// The line over a work's chart: what leads away from it first (back to the list), which it is, how it goes on, where,
/// and its facts in muted words joined by a bar, last among them its conversation, where the owner acts on it, as a
/// link; each part says in full on hovering what it cuts. An architect's page has none: what it would say is in the
/// head of the architect's one-to-one and the panel beside it.
export function HeadLine({ lead, name, nameHint, state, stateHint, where, whereHint, facts, conversation }: {
  lead?: ReactNode;
  name?: ReactNode;
  nameHint?: string;
  state: ReactNode;
  stateHint?: string;
  where?: ReactNode;
  whereHint?: string;
  facts: ReactNode[];
  /// The key of its conversation in the messages view.
  conversation: string;
}) {
  return (
    <div className="work-line">
      {lead}
      {name !== undefined ? (
        <span className="work-name" title={nameHint}>
          {name}
        </span>
      ) : null}
      <span className="work-state" title={stateHint}>
        {state}
      </span>
      {where !== undefined ? (
        <span className="work-where" title={whereHint}>
          {where}
        </span>
      ) : null}
      <span className="muted">
        {joined([
          ...facts,
          <Link key="talk" to={href({ page: 'messages', open: conversation })} title={t('go_to_conversation_hint')}>
            {t('go_to_conversation')}
          </Link>,
        ])}
      </span>
    </div>
  );
}

/// The way back from a work to the list of works.
export function WorksBack() {
  return (
    <Link to="/works" className="work-back" title={t('work_back')}>
      <ArrowLeftIcon /> {t('nav_works')}
    </Link>
  );
}
