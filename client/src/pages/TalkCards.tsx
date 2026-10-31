// What the engine writes into a conversation from the flow: in a work's group, the work's own bubble for each stage it
// entered, posted when it came in and updated in place, one line saying what happens now and, once the stage is over,
// how it ended, with what more there is folded under it; in an architect's one-to-one, the cards in the page's own
// voice, one for each matter the architect took up but the owner's words, whose bubble says where they stand, from
// when it took it up, held by an ask, worked on at a cell, handled with what became of it, or withdrawn, and the policy
// as it took effect. Each leads to where its detail is: a stage to its rounds, its pull request and commit, a matter to
// its cell on the architect's chart or its work's group, and the observations it recorded or changed to the project's
// observations.

import { BlockedIcon, CheckIcon, CircleIcon, DotFillIcon, FileIcon, SkipIcon } from '@primer/octicons-react';
import { Timeline } from '@primer/react';
import type { ReactNode } from 'react';

import type { ConversationEntry, MatterProgress, PolicyInForce, StageCard, WorkSummary } from '../api';
import { matterKind, matterSentence, matterSteps, outcomes, type MatterWords } from '../architect';
import { course, StateChip } from '../components';
import { stageOutcome, stageTitle, stageWhy } from '../conversation';
import { lineWho, nowWords } from '../course';
import { cost, duration, time } from '../format';
import { useStatus } from '../live';
import { firstWords, Markdown } from '../markdown';
import { CommitLink, Fold, PrLink, RoundLink } from '../parts';
import { architectHref, href, Link } from '../route';
import { t, tn, type Key } from '../text';
import { joinedWords } from '../voices';
import { LiveBubble } from './Bubble';
import { DesignCardView } from './Decide';
import { PolicyText } from './ArchitectViews';
import { DesignShown } from './Item';

/// What an agent said, word for word, in the box the UI spec gives what an agent handed back.
function Said({ text }: { text: string }) {
  return (
    <div className="agent-box">
      <Markdown text={text} />
    </div>
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

/// What a stage's bubble folds under its line: what its rounds said, oldest first, each with who said it and its round;
/// at the design stage, the design (the approaches compared, the one chosen, the design and the acceptance, or while
/// none is chosen, where the engineer leans and what the choice waits for); the design cards its rounds handed in,
/// each folded to its head line, the one that waits for the owner's ruling with its buttons once opened; and, once the
/// stage is over, what the engine told of the work meanwhile.
function StageFolds({ s, w, work }: { s: StageCard; w: WorkSummary | null; work: number }) {
  const engineer = s.notes.filter((n) => n.line === 'engineer');
  const leans = engineer.length > 0 ? engineer[engineer.length - 1].text : null;
  const design = s.stage === 'design' && s.shown.some((x) => ['options', 'chosen', 'design', 'acceptance'].includes(x.path));
  return (
    <>
      {s.notes.length > 0 ? (
        <Fold className="talk-fold" summary={tn('stage_notes', s.notes.length)}>
          {s.notes.map((n) => (
            <div key={n.round} className="stage-note">
              <p className="talk-card-note">
                {n.line !== null ? `${lineWho(n.line)} ` : null}
                <RoundLink work={work} round={n.round} />
              </p>
              <Said text={n.text} />
            </div>
          ))}
        </Fold>
      ) : null}
      {design ? (
        <Fold className="talk-fold" summary={t('stage_design_fold')}>
          <div className="agent-box document">
            <DesignShown shown={s.shown} leans={leans} waits={s.current ? (w?.waits ?? null) : null} />
          </div>
        </Fold>
      ) : null}
      {s.design_cards.map((c) => (
        <DesignCardView key={c.id} c={c} round={<RoundLink work={c.work} round={c.round} />} folded />
      ))}
      {!s.current && s.events.length > 0 ? (
        <Fold className="talk-fold" summary={tn('stage_events', s.events.length)}>
          {s.events.map((x, i) => (
            <p key={i} className="talk-card-note">
              {x}
            </p>
          ))}
        </Fold>
      ) : null}
    </>
  );
}

/// The bubble of a stage a work entered, in the engineer's look as the work's own voice, posted when the work came in
/// and updated in place (`LiveBubble`): its head the stage and which time the work is at it, when the work came in,
/// its rounds and, once over, how long it took and what it cost; its one line, while the work is at it what happens
/// now, where and with who at work, live and as the head of the group says it, with what the engine told of the work
/// meanwhile under it; once over, how it ended and why: what its reviews found, the pull request and the commit that
/// merged the work, or why it ended and the work that took it over; then the gist of what its last round said; and
/// what more there is folded under it (`StageFolds`). Its links go to the work's project on GitHub (`project`).
export function StageBubble({ e, s, w, project }: {
  e: ConversationEntry;
  s: StageCard;
  w: WorkSummary | null;
  project: string | null;
}) {
  const status = useStatus();
  const work = e.work ?? 0;
  const latest = s.notes.length > 0 ? s.notes[s.notes.length - 1] : null;
  const rounds = href({ page: 'work', id: work, round: s.first, cell: null, tab: 'rounds', only: null });
  const c = s.current && w !== null ? course(w, status) : null;
  const outcome = stageOutcome(s);
  const why = stageWhy(s);
  const now = c !== null && w !== null ? joinedWords([w.at_title, nowWords(c, w)]) : '';
  const state = s.current ? (
    <span className="live-line-now">{now}</span>
  ) : (
    <>
      {outcome !== null ? <StateChip tone={outcome.tone}>{outcome.words}</StateChip> : null}
      {why !== null ? <span className="live-line-why">{why}</span> : null}
      {s.pr !== null ? <PrLink project={project} n={s.pr} /> : null}
      {s.commit !== null ? <CommitLink project={project} sha={s.commit} /> : null}
      {s.taken_over_by !== null ? (
        <Link to={href({ page: 'messages', open: `work:${s.taken_over_by}` })}>
          {t('stage_taken_over', { id: s.taken_over_by })}
        </Link>
      ) : null}
    </>
  );
  const head = (
    <>
      <span className="talk-who">{stageTitle(s)}</span>
      <span className="talk-when">{time(s.started_at)}</span>
      <Link to={rounds}>{t('stage_rounds', { first: s.first, last: s.last, work })}</Link>
      {s.ended_at !== null ? (
        <span className="talk-when">
          {joinedWords([duration(s.ended_at - s.started_at), s.cost_usd > 0 ? cost(s.cost_usd) : null])}
        </span>
      ) : null}
    </>
  );
  const said = s.current ? now : joinedWords([outcome?.words ?? null, why]);
  const title = joinedWords([said, latest !== null ? firstWords(latest.text) : null]);
  return (
    <LiveBubble
      who="engineer"
      head={head}
      live={c !== null && c.running !== null}
      state={state}
      gist={latest?.text ?? null}
      title={title}
    >
      {s.current
        ? s.events.map((x, i) => (
            <p key={i} className="live-line-event">
              {x}
            </p>
          ))
        : null}
      <StageFolds s={s} w={w} work={work} />
    </LiveBubble>
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
/// one handled says what became of it, line by line, the observations it recorded or changed leading to the project's
/// observations, and leads to the round that closed it; one worked on leads to its cell on the architect's chart.
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
        ? outcomes(m.handled, project).map((o) => (
            <p key={o.key} className="talk-card-note">
              <span className="talk-card-kind">{o.label}</span> {o.to !== null ? <Link to={o.to}>{o.text}</Link> : o.text}
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
