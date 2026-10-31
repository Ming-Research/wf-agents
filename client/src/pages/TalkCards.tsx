// What the engine writes into a conversation from the flow, each a bubble of one line of the agent whose doing it is
// (`LiveBubble`), posted when it began and updated in place: in a work's group, the work's own bubble for each stage it
// entered, saying what happens now and, once the stage is over, how it ended, with what more there is folded under it;
// in an architect's one-to-one, a bubble of the architect's for each matter it took up but the owner's words (whose
// own bubble says where they stand), working on it, held by an ask, done with what it made, or withdrawn, a matter
// that changed nothing leaving once done; each version of its plan; and the policy as it took effect. Each leads to
// where its detail is: a stage to its rounds, its pull request and commit, a matter to its cell on the architect's
// chart or its work's group, what it made to each observation and item, the policy to the panel beside the one-to-one.

import { Fragment, type ReactNode } from 'react';

import type {
  ConversationEntry,
  MadeThing,
  MatterProgress,
  PlanChange,
  PolicyInForce,
  StageCard,
  WorkSummary,
} from '../api';
import { matterKind, matterSentence, matterSteps, type Outcome, outcomes, type MatterWords } from '../architect';
import { course, heldName, heldWords, StateChip, type Tone } from '../components';
import { stageOutcome, stageTitle, stageWhy } from '../conversation';
import { lineWho, nowWords } from '../course';
import { cost, duration } from '../format';
import { useStatus } from '../live';
import { words } from '../labelled';
import { firstWords, Markdown, MarkdownLine } from '../markdown';
import { CommitLink, Fold, ItemRef, LinkButton, PrLink, RoundLink } from '../parts';
import { architectHref, href, Link, observationsHref, sideHref } from '../route';
import { t, tn, type Key } from '../text';
import { joinedWords } from '../voices';
import { BubbleHead, LiveBubble } from './Bubble';
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

/// A design card pinned for the owner, named where else it would show: one line that says it waits above, a click
/// bringing the pinned card into view, instead of the card drawn a second time.
function PinnedAbove({ id }: { id: number }) {
  return (
    <p className="talk-card-note">
      <LinkButton
        onClick={() => {
          const el = document.getElementById(`design-card-${id}`);
          el?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
          el?.querySelector<HTMLElement>('summary')?.focus({ preventScroll: true });
        }}
      >
        {t('stage_card_pinned', { id })}
      </LinkButton>
    </p>
  );
}

/// What a stage's bubble folds under its line: what its rounds said, oldest first, each with who said it and its round;
/// at the design stage, the design (the approaches compared, the one chosen, the design and the acceptance, or while
/// none is chosen, where the engineer leans and what the choice waits for); the design cards its rounds handed in,
/// each folded to its head line, but one pinned for the owner's ruling (`pinned`), which the band at the top holds and
/// this names in one line that leads there, so that it shows once; and, once the stage is over, what the engine told
/// of the work meanwhile.
function StageFolds({ s, w, work, pinned }: {
  s: StageCard;
  w: WorkSummary | null;
  work: number;
  pinned: readonly number[];
}) {
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
      {s.design_cards.map((c) =>
        pinned.includes(c.id) ? (
          <PinnedAbove key={c.id} id={c.id} />
        ) : (
          <DesignCardView key={c.id} c={c} round={<RoundLink work={c.work} round={c.round} />} folded />
        ),
      )}
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
/// now in the owner's words, who is at work, what holds it and what it waits for, live and as the head of the group
/// says it, with no cell's title, its rounds' link leading to the cell, and what the engine told of the work meanwhile
/// under it; once over, how it ended and why: what its reviews found, the pull request and the commit that merged the
/// work, or why it ended and the work that took it over; then the gist of what its last round said; and what more there
/// is folded under it (`StageFolds`), the design card pinned for the owner (`pinned`) named there in one line. Its
/// links go to the work's project on GitHub (`project`).
export function StageBubble({ e, s, w, project, pinned }: {
  e: ConversationEntry;
  s: StageCard;
  w: WorkSummary | null;
  project: string | null;
  pinned: readonly number[];
}) {
  const status = useStatus();
  const work = e.work ?? 0;
  const latest = s.notes.length > 0 ? s.notes[s.notes.length - 1] : null;
  const rounds = href({ page: 'work', id: work, round: s.first, cell: null, tab: 'rounds', only: null });
  const c = s.current && w !== null ? course(w, status) : null;
  const outcome = stageOutcome(s);
  const why = stageWhy(s);
  const now = c !== null && w !== null ? nowWords(c, w) : '';
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
    <BubbleHead who={words('engineer')} about={stageTitle(s)} at={s.started_at}>
      <Link to={rounds}>{t('stage_rounds', { first: s.first, last: s.last, work })}</Link>
      {s.ended_at !== null ? (
        <span className="talk-when">
          {joinedWords([duration(s.ended_at - s.started_at), s.cost_usd > 0 ? cost(s.cost_usd) : null])}
        </span>
      ) : null}
    </BubbleHead>
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
      <StageFolds s={s} w={w} work={work} pinned={pinned} />
    </LiveBubble>
  );
}

/// What the line of a matter's bubble opens with, by where the matter stands, as a state's chip in its tone: back in the
/// queue and held by an ask of the owner wait on someone; in hand goes on; done; withdrawn, no meaning of its own.
const MATTER_STATE: Record<string, { key: Key; tone: Tone }> = {
  queued: { key: 'matter_line_queued', tone: 'attention' },
  held: { key: 'matter_line_held', tone: 'attention' },
  working: { key: 'matter_line_working', tone: 'accent' },
  done: { key: 'matter_line_done', tone: 'success' },
  withdrawn: { key: 'matter_line_withdrawn', tone: 'neutral' },
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

/// The head of a bubble of the architect's own, every bubble's (`BubbleHead`): who it is, what it is about, when, and
/// where more of it is.
function ArchitectHead({ at, about = null, children }: { at: number | null; about?: string | null; children?: ReactNode }) {
  return (
    <BubbleHead who={words('architect')} about={about} at={at}>
      {children}
    </BubbleHead>
  );
}

/// One thing that came of a matter done, in words: the reply by the work it went to, else its name and what it says,
/// a name with nothing after it without its colon.
function cameWords(o: Outcome, replyTo: number | null): string {
  if (o.key === 'reply') return t('matter_replied', { work: replyTo ?? '' });
  return o.text !== '' ? `${o.label} ${o.text}` : o.label.replace(/:$/, '');
}

/// What came of a matter done, on its bubble's line after its state, in a few words, each leading where it is: the
/// reply taken back to the work that asked, to that work's group, where the reply is; what its path told and asked the
/// owner; the item changes and the observations it collected or changed, the latter to the project's observations;
/// what it did at once; whom it wrote to; or why nothing was to be done.
function cameOf(o: Outcome, replyTo: number | null): ReactNode {
  const said = cameWords(o, replyTo);
  const to = o.key === 'reply' ? (replyTo !== null ? href({ page: 'messages', open: `work:${replyTo}` }) : null) : o.to;
  return to !== null ? <Link to={to}>{said}</Link> : said;
}

/// A matter of the architect's, as one bubble of the architect's of one line, with no one named, posted when it took
/// the matter up and updated in place (`LiveBubble`): its head names its kind, what it is about; its line opens with
/// where it stands as a state's chip, in hand, held by the owner's ask, back in the queue, done or withdrawn, live while
/// a round of the architect runs, with what holds the architect, its own trouble, when anything does (`architect`, its
/// work going on); once withdrawn, why; once done, what came of it in a few words, each leading to it (`cameOf`), and
/// under the line what it made, each by name and leading to it (`Made`); what it asks, whole, on hovering and in a fold
/// under the line, and a place to set straight with its steps folded too. A matter about a work leads to that work's
/// group, which holds it in full. A matter that changed nothing is not in the conversation once done: the queue beside
/// it lists it among those handled.
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
  const stands = MATTER_STATE[m.state] ?? MATTER_STATE.queued;
  const kind = matterKind(m.kind, m.detail);
  const made = new Set(m.made.map((x) => x.kind));
  const came =
    m.state === 'done' && m.handled !== null
      ? outcomes(m.handled, project).filter(
          (o) => !(o.key === 'observations' && made.has('observation')) && !(o.key === 'changes' && made.has('item')),
        )
      : [];
  const replyTo = m.handled?.reply_to ?? null;
  const why = m.state === 'withdrawn' ? m.withdrawn : null;
  const head = (
    <ArchitectHead at={e.at} about={kind}>
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
      <StateChip tone={stands.tone}>{t(stands.key)}</StateChip>
      {held !== null && architect !== null ? (
        <StateChip tone={held.problem ? 'danger' : 'attention'} title={heldWords(c ?? course(architect, status), architect.paused)}>
          {held.name}
        </StateChip>
      ) : null}
      {came.length > 0 ? (
        <span className="live-line-why">
          {came.map((o, i) => (
            <Fragment key={o.key}>
              {i > 0 ? ' | ' : null}
              {cameOf(o, replyTo)}
            </Fragment>
          ))}
        </span>
      ) : null}
    </>
  );
  const saidCame = came.map((o) => cameWords(o, replyTo));
  return (
    <LiveBubble
      who="architect"
      head={head}
      live={c !== null && c.running !== null}
      state={state}
      gist={why}
      title={joinedWords([t(stands.key), held?.name ?? null, ...saidCame, why, kind, matterSentence(m)])}
    >
      <Made made={m.made} />
      <Fold className="talk-fold" summary={t('matter_whole')}>
        <p className="talk-fold-text">{matterSentence(m)}</p>
      </Fold>
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
