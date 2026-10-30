// The cards the engine writes into a conversation from the flow, in the page's own voice and apart from what was said
// (the UI spec's narrow cards): in a work's group, one for each stretch of the work at a stage, updated in place while
// the work is there, with its result, time and cost, the design folded in the design's card and its design card to
// rule on; in an architect's one-to-one, one for each matter the architect took up but the owner's words, whose bubble
// says where they stand, from when it took it up, held by an ask, worked on at a cell, handled with what became of it,
// or withdrawn, and the policy as it took effect; in wf-agents' own, each observation, which the owner closes there.
// Each leads to where its detail is: a stage to its rounds, a matter to its cell on the architect's chart or its
// work's group.

import {
  BlockedIcon,
  CheckIcon,
  CircleIcon,
  DotFillIcon,
  EyeIcon,
  FileIcon,
  SkipIcon,
  SyncIcon,
} from '@primer/octicons-react';
import { Button, Label, Timeline } from '@primer/react';
import { useState, type ReactNode } from 'react';

import type { ConversationEntry, MatterProgress, Observation, PolicyInForce, Shown, StageCard, WorkSummary } from '../api';
import { matterKind, matterSentence, matterSteps, outcomes, type MatterWords } from '../architect';
import { StateChip } from '../components';
import { stageFacts } from '../conversation';
import { cost, duration, time } from '../format';
import { postJson } from '../http';
import { words } from '../labelled';
import { reread } from '../live';
import { Markdown } from '../markdown';
import { Fold } from '../parts';
import { architectHref, href, Link, workHref } from '../route';
import { t, tn, type Key } from '../text';
import { joinedWords } from '../voices';
import { DesignCardView } from './Decide';
import { PolicyText } from './ArchitectViews';
import { DesignShown } from './Item';

/// The name of each stage the engineer's graph has, as the owner reads it.
const STAGES: Record<string, Key> = {
  research: 'stage_research',
  design: 'stage_design',
  implement: 'stage_implement',
  test: 'stage_test',
  merge: 'stage_merge',
  watch_main: 'stage_watch_main',
};

export const stageName = (s: string): string => (STAGES[s] !== undefined ? t(STAGES[s]) : words(s));

/// What an agent said, word for word, in the box the UI spec gives what an agent handed back.
function Said({ text }: { text: string }) {
  return (
    <div className="agent-box">
      <Markdown text={text} />
    </div>
  );
}

/// The facts of a card on one line, small and muted: when it ran, how long and what it cost.
function When({ from, to, usd }: { from: number; to: number | null; usd: number | null }) {
  return (
    <span className="talk-card-facts">
      {joinedWords([time(from), to !== null ? duration(to - from) : null, usd !== null && usd > 0 ? cost(usd) : null])}
    </span>
  );
}

/// A card of the engine's in the stream: its badge, saying where it stands, and what it holds.
function Card({ icon, tone, children, id }: {
  icon: ReactNode;
  tone: 'accent' | 'success' | 'attention' | 'done' | undefined;
  children: ReactNode;
  id?: string;
}) {
  return (
    <Timeline.Item condensed className="talk-card" id={id}>
      <Timeline.Badge variant={tone}>{icon}</Timeline.Badge>
      <Timeline.Body className="talk-card-body">{children}</Timeline.Body>
    </Timeline.Item>
  );
}

/// The design a stretch at the design stage handed back, folded: the approaches compared, the one chosen, the design
/// and the acceptance; and, while no approach is chosen, so, with where the engineer stands and what the choice waits
/// for. Among the card's own words it is in the box of what an agent handed back.
function DesignOfStage({ shown, s, w }: { shown: Shown[]; s: StageCard; w: WorkSummary | null }) {
  const has = shown.some((x) => ['options', 'chosen', 'design', 'acceptance'].includes(x.path));
  if (!has) return null;
  return (
    <details className="talk-fold">
      <summary>{t('stage_design_fold')}</summary>
      <div className="talk-fold-body agent-box document">
        <DesignShown shown={shown} leans={s.said} waits={s.current ? (w?.waits ?? null) : null} />
      </div>
    </details>
  );
}

/// A stretch of a work at one stage: the stage, how many times the work came back to it, where it is while the work
/// is there (its cell) or done, when, how long and what it cost, with its rounds; why it came back; the facts the
/// engine knows of it; what the engineer's last round in it said, folded; and in the design's card, the design folded
/// and the design cards its rounds handed in, the one that waits for the owner's ruling with its buttons.
export function StageCardView({ e, s, w }: { e: ConversationEntry; s: StageCard; w: WorkSummary | null }) {
  const work = e.work ?? 0;
  const tone = s.current ? 'accent' : 'success';
  const icon = s.current ? <DotFillIcon /> : s.again > 0 ? <SyncIcon /> : <CheckIcon />;
  const facts = stageFacts(s);
  const rounds = href({ page: 'work', id: work, round: s.first, cell: null, tab: 'rounds', only: null });
  return (
    <Card icon={icon} tone={tone} id={`stage-${work}-${e.id}`}>
      <div className="talk-card-line">
        <span className="talk-card-title">
          {stageName(s.stage)}
          {s.again > 0 ? ` ${t('stage_again', { n: s.again + 1 })}` : ''}
        </span>
        {s.current ? (
          <Label variant="accent">{t('stage_at', { cell: s.cell ?? '', title: s.cell_title ?? '' })}</Label>
        ) : (
          <Label variant="success">{t('stage_done')}</Label>
        )}
        <When from={s.started_at} to={s.ended_at} usd={s.cost_usd} />
        <Link to={rounds}>
          {t('stage_rounds', { first: s.first, last: s.last, work })}
        </Link>
      </div>
      {s.back !== null ? (
        <p className="talk-card-note">
          {t('stage_back', { cell: s.back.cell, title: s.back.title ?? '', row: words(s.back.row) })}
          {s.back.note !== null ? ` ${s.back.note}` : ''}
        </p>
      ) : null}
      {facts.length > 0 ? <p className="talk-card-note">{joinedWords(facts)}</p> : null}
      {s.said !== null ? (
        <details className="talk-fold">
          <summary>{t('stage_said', { round: `R${work}-${s.said_round ?? ''}` })}</summary>
          <div className="talk-fold-body">
            <Said text={s.said} />
          </div>
        </details>
      ) : null}
      {s.stage === 'design' ? <DesignOfStage shown={s.shown} s={s} w={w} /> : null}
      {s.design_cards.map((c) => (
        <DesignCardView key={c.id} c={c} round={<Link to={workHref(c.work, { round: c.round })}>{`R${c.work}-${c.round}`}</Link>} folded={c.status === 'replaced'} />
      ))}
    </Card>
  );
}

/// Where a matter stands, as its card's badge and Label say it.
const MATTER_STATES: Record<string, { key: Key; tone: 'accent' | 'success' | 'attention' | 'done' | undefined; icon: ReactNode }> = {
  queued: { key: 'matter_queued', tone: undefined, icon: <CircleIcon /> },
  held: { key: 'matter_held', tone: 'attention', icon: <BlockedIcon /> },
  working: { key: 'matter_working', tone: 'accent', icon: <DotFillIcon /> },
  done: { key: 'matter_done', tone: 'success', icon: <CheckIcon /> },
  withdrawn: { key: 'matter_withdrawn', tone: undefined, icon: <SkipIcon /> },
};

/// A matter as the owner reads it wherever it shows, in its card and in the architect's queue: its kind, small and
/// muted, then what it asks to be done, in one sentence, whole, wrapping with the line it is in.
export function MatterWhat({ m }: { m: MatterWords }) {
  return (
    <>
      <span className="talk-card-kind">{matterKind(m.kind, m.detail)}</span> <span>{matterSentence(m)}</span>
    </>
  );
}

/// A matter of the architect's: its kind and what it asks, whole, with where it stands (back in the queue, held by an
/// ask of the owner, worked on at a cell, handled, withdrawn) and its links after them; a place to set straight with
/// the steps to take there folded under it; a matter about a work leads to that work's group, which holds it in full;
/// one handled says what became of it, line by line, and leads to the round that closed it; one worked on leads to its
/// cell on the architect's chart.
export function MatterCardView({ m, project, current }: { m: MatterProgress; project: string; current: boolean }) {
  const s = MATTER_STATES[m.state] ?? MATTER_STATES.queued;
  const steps = matterSteps(m);
  const group = m.work !== null ? href({ page: 'messages', open: `work:${m.work}` }) : null;
  const where =
    m.state === 'working' && m.cell !== null
      ? architectHref({ project, work: m.architect_work, round: m.round, cell: m.cell })
      : m.handled !== null
        ? architectHref({ project, work: m.handled.work, round: m.handled.round })
        : null;
  const label =
    m.state === 'working'
      ? t('matter_working_at', { cell: m.cell ?? '', title: m.cell_title ?? '' })
      : t(s.key);
  return (
    <Card icon={s.icon} tone={s.tone} id={`matter-${m.matter}`}>
      <p className={current ? 'talk-card-matter talk-card-current' : 'talk-card-matter'}>
        <MatterWhat m={m} /> <StateChip tone={s.tone ?? 'neutral'}>{label}</StateChip>
        {group !== null ? (
          <>
            {' '}
            <Link to={group}>
              {t('matter_in_group', { work: m.work ?? '' })}
            </Link>
          </>
        ) : null}
        {where !== null ? (
          <>
            {' '}
            <Link to={where}>
              {m.state === 'working' ? t('matter_on_chart') : `R${m.handled?.work ?? ''}-${m.handled?.round ?? ''}`}
            </Link>
          </>
        ) : null}
      </p>
      {steps.length > 0 ? (
        <Fold summary={tn('matter_steps', steps.length)}>
          <Said text={steps.map((x, i) => `${i + 1}. ${x}`).join('\n')} />
        </Fold>
      ) : null}
      {m.state === 'withdrawn' && m.withdrawn !== null ? <p className="talk-card-note">{m.withdrawn}</p> : null}
      {m.handled !== null && group === null
        ? outcomes(m.handled).map((o) => (
            <p key={o.key} className="talk-card-note">
              <span className="talk-card-kind">{o.label}</span> {o.text}
            </p>
          ))
        : null}
    </Card>
  );
}

/// The policy in force, as it took effect, with the document folded, in the box of what an agent handed back among the
/// card's own words.
export function PolicyCard({ p }: { p: PolicyInForce }) {
  return (
    <Card icon={<FileIcon />} tone="done">
      <div className="talk-card-line">
        <span className="talk-card-title">{t('policy_took_effect_card')}</span>
        <span className="talk-card-facts">{time(p.at)}</span>
      </div>
      <details className="talk-fold">
        <summary>{t('policy_read')}</summary>
        <div className="talk-fold-body agent-box">
          <PolicyText value={p.policy} />
        </div>
      </details>
    </Card>
  );
}

/// An observation of wf-agents: what it is, as its architect recorded it; its tags and how many times it was reported;
/// closed, why; open, the owner's Close, which asks why.
export function ObservationCard({ o }: { o: Observation }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const close = () => {
    const why = window.prompt(t('observation_close_why'));
    if (why === null || why.trim() === '') return;
    setBusy(true);
    setError(null);
    postJson(`/api/v1/observations/${o.id}/close`, { why: why.trim() }).then(
      () => reread('messages', 'architect'),
      (x: unknown) => {
        setBusy(false);
        setError(x instanceof Error ? x.message : String(x));
      },
    );
  };
  return (
    <Card icon={<EyeIcon />} tone={o.closed === null ? 'attention' : undefined} id={`observation-${o.id}`}>
      <div className="talk-card-line">
        <span className="talk-card-title">{t('observation_head', { id: o.id })}</span>
        {o.tags.map((x) => (
          <Label key={x} variant="secondary">
            {x}
          </Label>
        ))}
        <span className="talk-card-facts">{tn('observation_reported', o.times_reported)}</span>
        {o.closed !== null ? (
          <StateChip tone="neutral">{t('observation_closed', { why: words(o.closed) })}</StateChip>
        ) : (
          <Button size="small" disabled={busy} onClick={close}>
            {t('observation_close')}
          </Button>
        )}
      </div>
      <Said text={o.what} />
      {o.closed_why !== null ? <p className="talk-card-note">{o.closed_why}</p> : null}
      {error !== null ? <p className="talk-card-note bad">{t('request_failed', { error })}</p> : null}
    </Card>
  );
}
