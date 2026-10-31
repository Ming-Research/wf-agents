// What the engine writes into a conversation from the flow, each a bubble of one line of the agent whose doing it is
// (`LiveBubble`), posted when it began and updated in place, how it stands a mark at its head: in a work's group, the
// work's own bubble for each stage it entered, saying what happens now and, once the stage is over, how it ended, with
// what more there is folded under it; in an architect's one-to-one, a bubble of the architect's for each matter it took
// up but the owner's words (whose own bubble says where they stand), working on it, held by an ask, done with what came
// of it, or withdrawn, a matter that changed nothing leaving once done; each version of its plan; and the policy as it
// took effect. Each leads to where its detail is: a stage to its rounds, its pull request and commit, a matter to its
// cell on the architect's chart or its work's group, what it changed to each observation and item, the policy to the
// panel beside the one-to-one.

import { type ElementType, Fragment, type ReactNode } from 'react';

import type {
  ConversationEntry,
  MadeThing,
  MatterProgress,
  PlanChange,
  PolicyInForce,
  StageCard,
  WorkSummary,
} from '../api';
import {
  matterKind,
  type MatterResult,
  matterResult,
  matterSentence,
  matterSteps,
  type MatterWords,
} from '../architect';
import {
  BACK_MARK,
  course,
  heldName,
  ITEM_MARK,
  liveState,
  StateMark,
  type Tone,
  ToneMark,
  WORK_MARK,
} from '../components';
import { stageOutcome, stageTitle, stageWhy } from '../conversation';
import { lineWho, nowWords } from '../course';
import { cost, duration } from '../format';
import { useStatus } from '../live';
import { words } from '../labelled';
import { firstWords, Markdown, MarkdownLine } from '../markdown';
import { CommitLink, Fold, ItemRef, LinkButton, PrLink, RoundLink } from '../parts';
import { architectHref, href, Link, observationsHref, roundAt, sideHref } from '../route';
import { t, tn, type Key } from '../text';
import { joinedWords, SEP } from '../voices';
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
/// and updated in place (`LiveBubble`): its head the stage and which time the work is at it, its mark, when the work
/// came in, its rounds and, once over, how long it took and what it cost; the mark, while the work is at it, the work's
/// own as the list of conversations shows it (`StateMark`, the running halo while a round runs), once over how it ended
/// (`ToneMark`); its one line, while the work is at it what happens now in the owner's words, who is at work, what
/// holds it and what it waits for, live and as the head of the group says it, with no cell's title, its rounds' link
/// leading to the cell, and what the engine told of the work meanwhile under it; once over, how it ended and why in
/// words ("sent back: 6 findings in “Design tree review”"), then the pull request and the commit that merged the work,
/// or the work that took it over; then the gist of what its last round said; and what more there is folded under it
/// (`StageFolds`), the design card pinned for the owner (`pinned`) named there in one line. Its links go to the work's
/// project on GitHub (`project`).
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
  const ended =
    outcome !== null && why !== null ? t('stage_outcome_why', { outcome: outcome.words, why }) : (outcome?.words ?? why);
  const mark = s.current ? (
    <StateMark state={w !== null ? liveState(w, status) : null} />
  ) : outcome !== null ? (
    <ToneMark mark={outcome.mark} tone={outcome.tone} words={outcome.words} />
  ) : null;
  const line = s.current ? (
    <span className="live-line-now">{now}</span>
  ) : (
    <>
      {ended !== null ? <span className="live-line-why">{ended}</span> : null}
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
    <BubbleHead who={words('engineer')} about={stageTitle(s)} mark={mark} at={s.started_at}>
      <Link to={rounds}>{t('stage_rounds', { first: s.first, last: s.last, work })}</Link>
      {s.ended_at !== null ? (
        <span className="talk-when">
          {joinedWords([duration(s.ended_at - s.started_at), s.cost_usd > 0 ? cost(s.cost_usd) : null])}
        </span>
      ) : null}
    </BubbleHead>
  );
  const title = joinedWords([s.current ? now : ended, latest !== null ? firstWords(latest.text) : null]);
  return (
    <LiveBubble
      who="engineer"
      head={head}
      line={line}
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

/// Where a matter stands, by its state, in words and as the mark at the head of its bubble, in its state's tone: back in
/// the queue, the mark of going back, and held by an ask of the owner, a work's mark of asking, wait on someone; in
/// hand goes on, the architect's own mark while it has the matter (`MatterBubble`); done, a work's mark of done;
/// withdrawn, no meaning of its own, an item's mark of cancelled.
const MATTER_STATE: Record<string, { key: Key; tone: Tone; mark: ElementType }> = {
  queued: { key: 'matter_line_queued', tone: 'attention', mark: BACK_MARK },
  held: { key: 'matter_line_held', tone: 'attention', mark: WORK_MARK.asking },
  working: { key: 'matter_line_working', tone: 'accent', mark: WORK_MARK.ready },
  done: { key: 'matter_line_done', tone: 'success', mark: WORK_MARK.done },
  withdrawn: { key: 'matter_line_withdrawn', tone: 'neutral', mark: ITEM_MARK.cancelled },
};

/// What was done to a thing a matter changed, by its kind and change, in the words before it.
const THING: Record<string, Key> = {
  'item new': 'thing_item_new',
  'item cancelled': 'thing_item_cancelled',
  'item reordered': 'thing_item_reordered',
  'observation new': 'thing_observation_new',
  'observation merge': 'thing_observation_merge',
  'observation close': 'thing_observation_close',
};

/// Where an item was moved in the queue: first, last, or before, after or under another item.
const PLACE: Record<string, Key> = {
  first: 'thing_place_first',
  last: 'thing_place_last',
  before: 'thing_place_before',
  after: 'thing_place_after',
  under: 'thing_place_under',
};

const did = (x: MadeThing): string => {
  const key = THING[`${x.kind} ${x.change}`];
  return key !== undefined ? t(key) : words(x.change);
};

const place = (x: MadeThing): string | null => {
  const key = x.place !== null ? PLACE[x.place] : undefined;
  if (key === undefined) return null;
  return x.next_to !== null ? `${t(key)} #${x.next_to}` : t(key);
};

/// A thing a matter changed, in words, as its line says it, for hovering.
const thingWords = (x: MadeThing): string => [did(x), `#${x.id}`, firstWords(x.title), place(x)].filter((p) => p !== null).join(' ');

/// A matter as the owner reads it wherever it shows, in its bubble and in the architect's queue: its kind, small and
/// muted, then what it asks to be done, in one sentence, whole, wrapping with the line it is in.
export function MatterWhat({ m }: { m: MatterWords }) {
  return (
    <>
      <span className="talk-card-kind">{matterKind(m.kind, m.detail)}</span> <span>{matterSentence(m)}</span>
    </>
  );
}

/// One thing a matter changed, in one look wherever it shows, on the bubble's line or on a line of its own under it:
/// what was done to it in words, then the thing by its number and its title, a link to it (an item to its page, an
/// observation to it among the observations), its title cut where the line ends; for an item moved, then where to.
function Thing({ x }: { x: MadeThing }) {
  const to = place(x);
  return (
    <span className="live-thing">
      <span className="live-thing-did">{did(x)}</span>
      <span className="live-thing-ref">
        {x.kind === 'observation' ? (
          <Link to={observationsHref(null, x.id)} className="item-ref" title={firstWords(x.title)}>
            <span className="item-ref-n">#{x.id}</span> <MarkdownLine text={x.title} />
          </Link>
        ) : (
          <ItemRef id={x.id} title={x.title} />
        )}
      </span>
      {to !== null ? (
        <span className="live-thing-place">
          {t(PLACE[x.place ?? ''])}
          {x.next_to !== null ? (
            <>
              {' '}
              <ItemRef id={x.next_to} />
            </>
          ) : null}
        </span>
      ) : null}
    </span>
  );
}

/// What came of a matter done, on its bubble's line in words with nothing before them (`matterResult`): its parts
/// joined by a bar, the reply leading to the group it went to; or, when it changed one thing and nothing else, that
/// thing (`Thing`).
function ResultLine({ r }: { r: MatterResult }) {
  if (r.one !== null) return <Thing x={r.one} />;
  return (
    <span className="live-line-why">
      {r.parts.map((p, i) => (
        <Fragment key={p.key}>
          {i > 0 ? SEP : null}
          {p.to !== null ? <Link to={p.to}>{p.words}</Link> : p.words}
        </Fragment>
      ))}
    </span>
  );
}

/// What a matter changed, under its bubble's line and not folded, one to a line in the order its line counts them, each
/// as the line would say it (`Thing`); none when the line says the one thing it changed.
function ResultThings({ r }: { r: MatterResult }) {
  if (r.one !== null || r.things.length === 0) return null;
  return (
    <ul className="live-made">
      {r.things.map((x) => (
        <li key={`${x.kind}-${x.change}-${x.id}`} title={thingWords(x)}>
          <Thing x={x} />
        </li>
      ))}
    </ul>
  );
}

/// The head of a bubble of the architect's own, every bubble's (`BubbleHead`): who it is, what it is about, how it
/// stands, when, and where more of it is.
function ArchitectHead({ at, about = null, mark = null, children }: {
  at: number | null;
  about?: string | null;
  mark?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <BubbleHead who={words('architect')} about={about} mark={mark} at={at}>
      {children}
    </BubbleHead>
  );
}

/// A matter of the architect's, as one bubble of the architect's of one line, with no one named, posted when it took
/// the matter up and updated in place (`LiveBubble`): its head names its kind, what it is about, then where it stands as
/// a mark (back in the queue, held by the owner's ask, done or withdrawn; while in hand the architect's own, as its
/// one-to-one's row shows it: the running halo while a round of it runs, its own trouble while held); its line, in
/// words: while in hand what happens now, as a stage's line says it; once done what came of it (`matterResult`), and
/// under the line what it changed, one to a line, each by number and title and leading to it, one shape for every
/// result and every matter (`ResultLine`, `ResultThings`); once withdrawn, why; what it asks, whole, on hovering and in
/// a fold under the line, and a place to set straight with its steps folded too. A matter about a work leads to that
/// work's group, which holds it in full. A matter that changed nothing is not in the conversation once done: the queue
/// beside it lists it among those handled.
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
      ? architectHref({ project, round: roundAt(m.architect_work, m.round), cell: m.cell })
      : m.handled !== null
        ? architectHref({ project, round: roundAt(m.handled.work, m.handled.round) })
        : null;
  // The architect's own state and trouble show on the matter it has in hand.
  const c = m.state === 'working' && architect !== null ? course(architect, status) : null;
  const held = c !== null && c.held !== null ? heldName(c) : null;
  const stands = MATTER_STATE[m.state] ?? MATTER_STATE.queued;
  const kind = matterKind(m.kind, m.detail);
  const result = m.state === 'done' ? matterResult(m) : null;
  const mark =
    c !== null && architect !== null ? (
      <StateMark
        state={liveState(architect, status)}
        words={c.running !== null ? t(stands.key) : (held?.name ?? undefined)}
      />
    ) : (
      <ToneMark mark={stands.mark} tone={stands.tone} words={t(stands.key)} />
    );
  const said =
    result !== null
      ? result.one !== null
        ? thingWords(result.one)
        : joinedWords(result.parts.map((p) => p.words))
      : c !== null && architect !== null
        ? nowWords(c, architect)
        : m.state === 'withdrawn' && m.withdrawn !== null
          ? t('matter_withdrawn_why', { why: m.withdrawn })
          : t(stands.key);
  const head = (
    <ArchitectHead at={e.at} about={kind} mark={mark}>
      {group !== null ? <Link to={group}>{t('matter_in_group', { work: m.work ?? '' })}</Link> : null}
      {where !== null ? (
        <Link to={where}>
          {m.state === 'working' ? t('matter_on_chart') : `R${m.handled?.work ?? ''}-${m.handled?.round ?? ''}`}
        </Link>
      ) : null}
    </ArchitectHead>
  );
  return (
    <LiveBubble
      who="architect"
      head={head}
      line={result !== null ? <ResultLine r={result} /> : <span className="live-line-now">{said}</span>}
      gist={null}
      title={joinedWords([said, kind, matterSentence(m)])}
    >
      {result !== null ? <ResultThings r={result} /> : null}
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
      line={
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
          <Link to={architectHref({ project, round: roundAt(p.work, p.round) })}>{`R${p.work}-${p.round}`}</Link>
        </ArchitectHead>
      }
      line={<span className="live-line-now">{said}</span>}
      gist={p.changed}
      title={joinedWords([said, p.changed])}
    >
      <Fold className="talk-fold" summary={t('plan_whole')}>
        <Said text={p.plan} />
      </Fold>
    </LiveBubble>
  );
}
