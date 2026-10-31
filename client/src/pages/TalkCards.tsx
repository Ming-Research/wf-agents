// What the engine writes into a conversation from the flow, each a bubble of one line of the agent whose doing it is
// (`LiveBubble`), posted when it began and updated in place: in a work's group, the work's own bubble for each stage it
// entered, saying what happens now and, once the stage is over, how it ended, with what more there is folded under it;
// in an architect's one-to-one, a bubble of the architect's for each matter it took up but the owner's words (whose
// own bubble says where they stand), working on it, held by an ask, done with what it made, or withdrawn, a matter
// that changed nothing leaving once done; each version of its plan; and the policy as it took effect. Each leads to
// where its detail is: a stage to its rounds, its pull request and commit, a matter to its cell on the architect's
// chart or its work's group, what it made to each observation and item, the policy to the panel beside the one-to-one.

import type { ReactNode } from 'react';

import type {
  ConversationEntry,
  MadeThing,
  MatterProgress,
  PlanChange,
  PolicyInForce,
  StageCard,
  WorkSummary,
} from '../api';
import { matterKind, matterSentence, matterSteps, outcomes, type MatterWords } from '../architect';
import { course, heldName, heldWords, StateChip } from '../components';
import { stageOutcome, stageTitle, stageWhy } from '../conversation';
import { lineWho, nowWords } from '../course';
import { cost, duration, time } from '../format';
import { useStatus } from '../live';
import { words } from '../labelled';
import { firstWords, Markdown, MarkdownLine } from '../markdown';
import { CommitLink, Fold, ItemRef, PrLink, RoundLink } from '../parts';
import { architectHref, href, Link, observationsHref, sideHref } from '../route';
import { t, tn, type Key } from '../text';
import { joinedWords } from '../voices';
import { LiveBubble } from './Bubble';
import { DesignCardView } from './Decide';
import { DesignShown } from './Item';

/// What an agent said, word for word, in the box the UI spec gives what an agent handed back.
function Said({ text }: { text: string }) {
  return (
    <div className="agent-box">
      <Markdown text={text} />
    </div>
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

/// What the line of a matter's bubble opens with, by where the matter stands: back in the queue, held by an ask of the
/// owner, in hand, done, or withdrawn.
const MATTER_LINE: Record<string, Key> = {
  queued: 'matter_line_queued',
  held: 'matter_line_held',
  working: 'matter_line_working',
  done: 'matter_line_done',
  withdrawn: 'matter_line_withdrawn',
};

/// What a matter made, by its kind and what became of it, as the line before it says it.
const MADE: Record<string, Key> = {
  'observation new': 'made_observation',
  'item new': 'made_item_new',
  'item cancelled': 'made_item_cancelled',
  'item reordered': 'made_item_reordered',
};

/// A matter as the owner reads it wherever it shows, in its bubble and in the architect's queue: its kind, small and
/// muted, then what it asks to be done, in one sentence, whole, wrapping with the line it is in.
export function MatterWhat({ m }: { m: MatterWords }) {
  return (
    <>
      <span className="talk-card-kind">{matterKind(m.kind, m.detail)}</span> <span>{matterSentence(m)}</span>
    </>
  );
}

/// What a matter made, listed under its line and not folded, each leading to it: the observations it recorded, each
/// by its title, and the items it made, cancelled or moved in the queue, each by its number and its title.
function Made({ made }: { made: MadeThing[] }) {
  if (made.length === 0) return null;
  return (
    <ul className="live-made">
      {made.map((x) => (
        <li key={`${x.kind}-${x.id}`}>
          <span className="talk-card-kind">{MADE[`${x.kind} ${x.change}`] !== undefined ? t(MADE[`${x.kind} ${x.change}`]) : words(x.change)}</span>{' '}
          {x.kind === 'observation' ? (
            <Link to={observationsHref(null, x.id)} title={firstWords(x.title)}>
              <MarkdownLine text={x.title} />
            </Link>
          ) : (
            <ItemRef id={x.id} title={x.title} />
          )}
        </li>
      ))}
    </ul>
  );
}

/// The head of a bubble of the architect's own: who it is, when, and where more of it is.
function ArchitectHead({ at, children }: { at: number | null; children?: ReactNode }) {
  return (
    <>
      <span className="talk-who">{t('conv_architect_short')}</span>
      <span className="talk-when">{time(at)}</span>
      {children}
    </>
  );
}

/// A matter of the architect's, as one bubble of the architect's of one line, with no one named, posted when it took
/// the matter up and updated in place (`LiveBubble`): while it is in hand, "Working on:" and the matter, live while a
/// round of the architect runs, with where it is at and what holds the architect, its own trouble, when anything does
/// (`architect`, its work going on); held by the owner's ask, back in the queue or withdrawn with why, so; once done,
/// its result: the matter, then under the line what it made, each by name and leading to it (`Made`), and what else
/// came of it, the engineer it answered leading to that work's group; a place to set straight with its steps folded
/// under it. A matter about a work leads to that work's group, which holds it in full. A matter that changed nothing is
/// not in the conversation once done: the queue beside it lists it among those handled.
export function MatterBubble({ e, m, project, architect }: {
  e: ConversationEntry;
  m: MatterProgress;
  project: string;
  architect: WorkSummary | null;
}) {
  const status = useStatus();
  const steps = matterSteps(m);
  const group = m.work !== null ? href({ page: 'messages', open: `work:${m.work}` }) : null;
  const where =
    m.state === 'working' && m.cell !== null
      ? architectHref({ project, work: m.architect_work, round: m.round, cell: m.cell })
      : m.handled !== null
        ? architectHref({ project, work: m.handled.work, round: m.handled.round })
        : null;
  // The architect's own trouble shows on the matter it has in hand.
  const c = m.state === 'working' && architect !== null ? course(architect, status) : null;
  const held = c !== null && c.held !== null ? heldName(c) : null;
  const lead = t(MATTER_LINE[m.state] ?? 'matter_line_queued');
  const gist = m.state === 'working' ? m.cell_title : m.state === 'withdrawn' ? m.withdrawn : null;
  const made = new Set(m.made.map((x) => x.kind));
  const rest =
    m.handled !== null
      ? outcomes(m.handled, project).filter(
          (o) => !(o.key === 'observations' && made.has('observation')) && !(o.key === 'changes' && made.has('item')),
        )
      : [];
  const head = (
    <ArchitectHead at={e.at}>
      {group !== null ? <Link to={group}>{t('matter_in_group', { work: m.work ?? '' })}</Link> : null}
      {where !== null ? (
        <Link to={where}>
          {m.state === 'working' ? t('matter_on_chart') : `R${m.handled?.work ?? ''}-${m.handled?.round ?? ''}`}
        </Link>
      ) : null}
    </ArchitectHead>
  );
  const state = (
    <>
      <span className="live-line-now">
        {lead} <MatterWhat m={m} />
      </span>
      {held !== null && architect !== null ? (
        <StateChip tone={held.problem ? 'danger' : 'attention'} title={heldWords(c ?? course(architect, status), architect.paused)}>
          {held.name}
        </StateChip>
      ) : null}
    </>
  );
  return (
    <LiveBubble
      who="architect"
      head={head}
      live={c !== null && c.running !== null}
      state={state}
      gist={gist}
      title={joinedWords([lead, matterKind(m.kind, m.detail), matterSentence(m), held?.name ?? null, gist])}
    >
      <Made made={m.made} />
      {rest.map((o) => (
        <p key={o.key} className="talk-card-note">
          {o.key === 'reply' && m.handled?.reply_to != null ? (
            <Link to={href({ page: 'messages', open: `work:${m.handled.reply_to}` })}>{o.label}</Link>
          ) : (
            <span className="talk-card-kind">{o.label}</span>
          )}{' '}
          {o.to !== null ? <Link to={o.to}>{o.text}</Link> : o.text}
        </p>
      ))}
      {steps.length > 0 ? (
        <Fold className="talk-fold" summary={tn('matter_steps', steps.length)}>
          <Said text={steps.map((x, i) => `${i + 1}. ${x}`).join('\n')} />
        </Fold>
      ) : null}
    </LiveBubble>
  );
}

/// The policy as it took effect, as one bubble of the architect's of one line, leading to it in the panel beside the
/// one-to-one.
export function PolicyBubble({ p, project }: { p: PolicyInForce; project: string }) {
  return (
    <LiveBubble
      who="architect"
      head={<ArchitectHead at={p.at} />}
      live={false}
      state={
        <span className="live-line-now">
          {t('policy_took_effect_card')} <Link to={sideHref(project, 'policy')}>{t('policy_read_it')}</Link>
        </span>
      }
      gist={null}
      title={t('policy_took_effect_card')}
    />
  );
}

/// A version of the architect's plan, as one bubble of the architect's of one line: what changed from the version
/// before, or that it made the plan, with the plan whole in an inline card that opens under it, and the round that
/// wrote it on the architect's chart.
export function PlanBubble({ e, p, project }: { e: ConversationEntry; p: PlanChange; project: string }) {
  const said = p.changed !== null ? t('plan_changed') : t('plan_made');
  return (
    <LiveBubble
      who="architect"
      head={
        <ArchitectHead at={e.at}>
          <Link to={architectHref({ project, work: p.work, round: p.round })}>{`R${p.work}-${p.round}`}</Link>
        </ArchitectHead>
      }
      live={false}
      state={<span className="live-line-now">{said}</span>}
      gist={p.changed}
      title={joinedWords([said, p.changed])}
    >
      <Fold className="talk-fold" summary={t('plan_whole')}>
        <Said text={p.plan} />
      </Fold>
    </LiveBubble>
  );
}
