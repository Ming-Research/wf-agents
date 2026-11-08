// Small parts every view uses.

import {
  AlertIcon,
  CheckCircleIcon,
  CircleIcon,
  ClockIcon,
  HourglassIcon,
  type Icon,
  IssueOpenedIcon,
  NoEntryIcon,
  PauseIcon,
  QuestionIcon,
  SkipIcon,
  StopIcon,
  UndoIcon,
  XIcon,
} from '@primer/octicons-react';
import { Button, Flash, Label, type LabelColorOptions } from '@primer/react';
import { type ElementType, type ReactNode, useSyncExternalStore } from 'react';

import type { ItemState, RoundOutcome, Status, Waits, WorkState, WorkSummary } from './api';
import { Halo } from './graph/flowDraw';
import type { Resource } from './live';
import { t, tn } from './text';

/// Label and value pairs.
export function Facts({ rows }: { rows: Array<[ReactNode, ReactNode] | null | false> }) {
  return (
    <dl className="facts">
      {rows.map((r, i) =>
        r ? (
          <div key={i} className="fact">
            <dt>{r[0]}</dt>
            <dd>{r[1]}</dd>
          </div>
        ) : null,
      )}
    </dl>
  );
}

/// A work's state as it is now, worked out from what the work is doing (`course`), so that the icon, the words, the
/// halo and the head say the same: a work the engine's status names as stopping, a cancel or a start over asked, is
/// stopping whatever it reads, since it goes no further than the step it is in; a work is running while a stage of it
/// works, where a round of it runs or a tool of it is under way, or its next step is due, or the round its last step
/// left open is to be closed by the next, and the engine takes it; a round left open that the page cannot tell the
/// engine takes up (the engine is out of reach, halted or holds the work back) is not running, and the work is ready,
/// its next round closing that round first, as is a work whose next step is due; a work held back only because every
/// work is paused is paused, as the words say; and a work the engine's status names as waiting for a free slot, what it
/// waited for come or its pause asked to be lifted, waits for that slot, though as read it is ready, still waiting or
/// paused, as does a paused work the owner started, as its words say. A state the work's row gives otherwise stands as
/// read.
export function liveState(w: WorkSummary, status: Status | null): WorkState {
  const c = course(w, status);
  if (c.stopping) return 'stopping';
  if (c.over) return w.state;
  if (c.working !== null) return 'running';
  const state = w.state === 'running' ? 'ready' : w.state;
  // The owner's Start of a paused work waits for a slot: its words say "started", not paused.
  if (c.releasing && state === 'paused') return 'waiting_for_slot';
  const waits = state === 'ready' || state === 'waiting' || state === 'asking';
  if (waits && c.held === 'all') return 'paused';
  if (c.slot && (waits || state === 'paused' || state === 'halted')) return 'waiting_for_slot';
  return state;
}

/// Whether the engine's status names a work as waiting for a free slot: what it waited for came, or its pause is asked
/// to be lifted, and it goes on once its machine has a slot free, before any new work starts.
function forSlot(w: WorkSummary, status: Status | null): boolean {
  return status !== null && status.waiting_for_slot.includes(w.id);
}

/// Whether the engine's status names a work as stopping: it ends once the step it is in has ended.
function isStopping(w: WorkSummary, status: Status | null): boolean {
  return status !== null && status.stopping.includes(w.id);
}

/// How a work goes on, in the owner's terms. Two things are kept apart: what holds it back though it could go on (the
/// owner's pause, every work paused, or the engine's or the architect's stop), which the work's Start lifts, and Start
/// all lifts for every work paused with them all; and what it waits for, after which it goes on by itself. Both can
/// hold at once. The later of the owner's actions wins: a work the owner started goes on while every work is paused,
/// until every work is paused again; a work the owner paused stays paused through Start all.
export type Course = {
  /// Done or ended: it goes on no more.
  over: boolean;
  /// A cancel or a start over was asked: it ends once the step it is in has ended, as the engine's status says.
  stopping: boolean;
  /// A round of it runs now: after it the work goes on by itself, or stops because it is held back, or the round is
  /// the one the owner asked for. The engine's step of a work that waits, which only looks whether what it waits for
  /// came and writes no round, is not a round running.
  running: 'goes_on' | 'pausing' | 'step' | null;
  /// The cell a stage works at now, the one thing the page marks as working (the halo on the chart, the ring of the cell,
  /// the running mark, the icon of the state): where a round of it runs or a tool of it is under way, or where it goes
  /// on and its next step is due. None while it waits, is held back, waits for a free slot, or the engine halts or is
  /// out of reach, and once it is over: nothing then works, and the page marks none.
  working: string | null;
  held: 'you' | 'all' | 'engine' | 'architect' | null;
  /// What it waits for, after which it goes on by itself; none once it came, though the work waits on for a slot.
  waits: Waits | null;
  /// What it waited for came, and it waits, holding no slot, until its machine has a slot free, as the engine's status
  /// says: it then goes on before any new work starts.
  slot: boolean;
  /// Its flow version is older than the one the engine runs: when it goes on, it is brought to that one.
  outdated: boolean;
  /// The owner started it since every work was last paused (a work of the architect: the architect): it goes on
  /// whatever the global state.
  started: boolean;
  /// The owner started it while it was paused: its pause is lifted once its machine has a slot free.
  releasing: boolean;
  /// The owner asked for a round of it that has not ended yet.
  asked: boolean;
};

export function course(w: WorkSummary, status: Status | null): Course {
  if (w.state === 'done' || w.state === 'ended') {
    return {
      over: true,
      stopping: false,
      running: null,
      working: null,
      held: null,
      waits: null,
      slot: false,
      outdated: false,
      started: false,
      releasing: false,
      asked: false,
    };
  }
  const started =
    status !== null &&
    (status.started.includes(w.id) ||
      (w.graph === 'architect' && w.project !== null && status.architects_started.includes(w.project)));
  const releasing = status !== null && status.releasing.includes(w.id);
  const slot = forSlot(w, status);
  const held: Course['held'] =
    w.paused_by === 'engine'
      ? 'engine'
      : w.paused_by === 'architect'
        ? 'architect'
        : w.paused_by === 'owner'
          ? 'you'
          : status?.paused && !started
            ? 'all'
            : null;
  const asked = status !== null && status.stepping.includes(w.id);
  // The engine steps a work that waits only to look whether what it waits for came, and that step writes no round. The
  // work's row says it waits (`waiting`, with the retry wait the interface reads from the flow's kinds of failed run)
  // or has asked and waits for the answer (`asking`, whose round stays open meanwhile), as the engine judges it.
  const stands = w.state === 'waiting' || w.state === 'asking';
  let running: Course['running'] = null;
  if (status !== null && status.running.includes(w.id) && !stands) {
    running = asked ? 'step' : held !== null ? 'pausing' : 'goes_on';
  }
  // A tool of the round that is under way works on after the engine's step of it ended. Nothing works while the engine is
  // out of reach: the page cannot tell what it does. A work that goes on, nothing holding it back or making it wait, works
  // between two steps of the engine: its next step is due (`ready`), or the works as read still show the round its last
  // step ended (`running`, as the works are read at most once in a while and the status is live) and the next step takes
  // it. Both would otherwise make the stage's mark go out and come back at each step. While the engine stops it takes no
  // next step: only the steps in flight (`running`) and the tools under way work, and a work that was to go on waits for
  // the engine to start again.
  const tool = status !== null && w.running?.tool === true;
  const goesOn =
    status !== null &&
    !status.engine_stopping &&
    status.halt === null &&
    held === null &&
    !slot &&
    w.waits === null &&
    (w.state === 'ready' || w.state === 'running');
  const working = running !== null || tool ? (w.running?.cell ?? w.at) : goesOn ? w.at : null;
  // The retry wait the interface reads from the rounds ends only when the next round opens, which the engine never
  // opens for a work held back: it is no wait of a held work, which waits for nothing and may be given a Step.
  const retryHeld = held !== null && w.waits?.kind === 'retry';
  return {
    over: false,
    stopping: isStopping(w, status),
    running,
    working,
    held,
    waits: slot || retryHeld ? null : w.waits,
    slot,
    outdated: status !== null && w.flow_version !== status.flow_version,
    started,
    releasing,
    asked,
  };
}

/// What the owner can do with a work now; a work over or stopping, or any while the engine's state is not known, has
/// nothing. Otherwise it has exactly one of Start and Pause: Start while it is held back (paused by the owner, every work
/// paused, or stopped by the engine or the architect), after which it goes on whatever the global state, its own pause
/// lifted; Pause while it goes on, or its pause is asked to be lifted, by the owner's Start, the command line or the
/// architect, and it waits for a slot, after which it stays paused whatever the global state. Step, one round of it, is
/// for a work held back that no round of it runs and that waits for nothing (`step`), and can run only while no round of
/// it asked for before is under way and nothing stops every work (`canStep`): the rounds asked of other works run beside
/// it.
export function controls(c: Course, status: Status | null) {
  if (c.over || c.stopping || status === null) return { pause: false, start: false, step: false, canStep: false };
  const lifting = c.releasing || (c.slot && c.held !== 'all');
  const back = c.held !== null && !lifting;
  return {
    pause: !back,
    start: back,
    step: back && c.running === null && c.waits === null,
    canStep: !c.asked && status.halt === null,
  };
}

/// Why the engine pauses a work the owner paused without saying why, as the engine writes it.
export const PAUSED_BY_OWNER = 'the owner paused it';

/// What holds a work back, in a few words, and whether the engine halted it for a problem the owner looks at: the
/// owner's Start waiting for a free slot, the owner's pause, the engine's halt, the architect's pause, or Pause all.
export function heldName(c: Course): { name: string; problem: boolean } {
  if (c.releasing) return { name: t('course_releasing'), problem: false };
  if (c.held === 'engine') return { name: t('course_halted'), problem: true };
  if (c.held === 'architect') return { name: t('course_paused_architect'), problem: false };
  if (c.held === 'all') return { name: t('course_all'), problem: false };
  return { name: t('course_paused'), problem: false };
}

/// Why a work is held back, in words: the owner's Start waiting for a free slot, halted by the engine, paused by the
/// architect, by Pause all or by the owner.
export function heldWords(c: Course, why: string | null): string {
  if (c.releasing) return t('course_releasing_why');
  if (c.held === 'engine') return t('course_halted_why', { why: why ?? '' });
  if (c.held === 'architect') return t('course_paused_architect_why', { why: why ?? '' });
  if (c.held === 'all') return t('course_all_why');
  return why !== null && why !== PAUSED_BY_OWNER ? t('course_paused_why', { why }) : t('course_paused');
}

/// The colour of a state's chip, as the design system gives each state its meaning
/// (design/wf-agents/frontend/design-system.md, "状态的颜色就是其含义的颜色"): going on (accent), done
/// (success), waiting for someone (attention), gone wrong (danger), or none of these (neutral, grey); `done` is
/// Primer's purple, for a state Primer's own parts mark so.
export type Tone = 'accent' | 'success' | 'attention' | 'danger' | 'done' | 'neutral';

/// The Label variant a tone is drawn in: a state of no meaning of its own takes Primer's primary variant, which the page
/// fills grey (parts.css), so that it is a coloured chip as every state is, and not an uncoloured one, which is a tag's.
const VARIANT: Record<Tone, LabelColorOptions> = {
  accent: 'accent',
  success: 'success',
  attention: 'attention',
  danger: 'danger',
  done: 'done',
  neutral: 'primary',
};

/// The Label variant of a state's tone, for a part that draws the Label itself, such as a chip that is a link.
export const toneVariant = (tone: Tone): LabelColorOptions => VARIANT[tone];

/// A state, wherever the page shows one (a work's, an item's, a round's): a chip filled in the state's colour, the same
/// colour for the same state on every view. Every Label is drawn as a chip by parts.css.
export function StateChip({ tone, title, children }: { tone: Tone; title?: string; children: ReactNode }) {
  return (
    <Label variant={VARIANT[tone]} title={title}>
      {children}
    </Label>
  );
}

/// A tag or a kind (an observation's tag, a decision's change, a row's mark): an uncoloured chip, outlined, of a state's
/// chip's shape and height.
export function TagChip({ title, children }: { title?: string; children: ReactNode }) {
  return (
    <Label variant="secondary" title={title}>
      {children}
    </Label>
  );
}

/// A work's state's tone, which an architect takes for the state it stands in too.
export const WORK_TONE: Record<WorkState, Tone> = {
  running: 'accent',
  ready: 'neutral',
  asking: 'attention',
  waiting: 'attention',
  waiting_for_slot: 'attention',
  paused: 'attention',
  halted: 'danger',
  stopping: 'attention',
  done: 'success',
  ended: 'neutral',
};

/// The Octicon of each state of a work but running, whose mark is the running halo: the one map for every place that
/// shows a work's state as an icon (design/wf-agents/frontend/design-system.md, "工作的状态只算一次").
/// Ready is a ring at rest, the running halo's ring still; asking a question; waiting a clock; waiting for a free slot
/// an hourglass; paused two bars; halted by the engine for a problem GitHub's mark of an alert, red; stopping a
/// no-entry sign; done a tick in a ring; ended GitHub's mark of a run stopped before its end, grey as GitHub draws a
/// run cancelled.
export const WORK_MARK: Record<Exclude<WorkState, 'running'>, Icon> = {
  ready: CircleIcon,
  asking: QuestionIcon,
  waiting: ClockIcon,
  waiting_for_slot: HourglassIcon,
  paused: PauseIcon,
  halted: AlertIcon,
  stopping: NoEntryIcon,
  done: CheckCircleIcon,
  ended: StopIcon,
};

/// The Octicon of each state of an item, the one map for every place that shows an item's state as an icon, none of
/// them a work's mark of another meaning: open GitHub's mark of an issue open; done the tick in a ring of a work done,
/// since an item is done once a work of it reached its end; cancelled GitHub's mark of an issue closed as not planned.
/// What its work does has the work's own marks.
export const ITEM_MARK: Record<ItemState, Icon> = {
  open: IssueOpenedIcon,
  done: CheckCircleIcon,
  cancelled: SkipIcon,
};

/// The Octicon of a thing that ran and went back to where it was before (a stage sent back, a matter back in the
/// architect's queue), GitHub's mark of undoing, which no state of a work or of an item has.
export const BACK_MARK: Icon = UndoIcon;

/// The mark of a stage working: the halo the chart draws round the cell a stage works at, at an icon's size round
/// nothing, its arcs as wide as they must be to read beside an Octicon's strokes; the one mark of a state that moves,
/// so that what works looks the same on the chart and wherever else it shows.
export function RunningMark() {
  return (
    <svg className="running-mark" width={16} height={16} viewBox="0 0 16 16" aria-hidden="true">
      <Halo box={{ x: 5.5, y: 5.5, width: 5, height: 5 }} round small />
    </svg>
  );
}

/// A work's state's mark wherever it shows as an icon (a row's mark, a button of the list's filter): the running halo
/// for a stage working, else its Octicon.
export const workStateMark = (state: WorkState): ElementType => (state === 'running' ? RunningMark : WORK_MARK[state]);

/// A state as a mark, where a row has room for an icon and not for a chip (the list of conversations): a stage working
/// as the running halo, every other state as its Octicon in the colour of its chip, saying on hovering what its chip
/// says (a work's state by default). Nothing known yet keeps the mark's room empty, so that the rows stay in line.
export function StateMark({ state, words }: { state: WorkState | null; words?: string }) {
  if (state === null) return <span className="state-mark" aria-hidden="true" />;
  return <ToneMark mark={workStateMark(state)} tone={WORK_TONE[state]} words={words ?? t(`state_${state}`)} />;
}

/// Any state as a mark, drawn as a work's is (`StateMark`): its Octicon, or the running halo, in its tone's colour,
/// saying its words on hovering and as its name; for a state that is no work's, such as how a stage ended or where an
/// architect's matter stands, at the head of its bubble.
export function ToneMark({ mark: Mark, tone, words }: { mark: ElementType; tone: Tone; words: string }) {
  return (
    <span className={`state-mark state-mark-${tone}`} title={words} role="img" aria-label={words}>
      <Mark />
    </span>
  );
}

/// An item's state's tone, which the items' tree gives the button that shows or hides the items in that state too.
export const ITEM_TONE: Record<ItemState, Tone> = {
  open: 'accent',
  done: 'success',
  cancelled: 'danger',
};

/// The order a work's states are listed in, as the interface declares them, the states of a work over last.
export const WORK_STATES: WorkState[] = [
  'running',
  'ready',
  'asking',
  'waiting',
  'waiting_for_slot',
  'paused',
  'halted',
  'stopping',
  'done',
  'ended',
];

/// What this browser keeps of a view under `key`: null for nothing, undefined where the browser keeps nothing at all.
function keptText(key: string): string | null | undefined {
  try {
    return window.localStorage.getItem(`wf-agents.${key}`);
  } catch {
    return undefined;
  }
}

function keep(key: string, value: unknown) {
  try {
    window.localStorage.setItem(`wf-agents.${key}`, JSON.stringify(value));
  } catch {
    // Kept only where the browser lets it be.
  }
}

/// The states a `StateFilter` shows, by the key they are kept under, with the text kept they were read from, so that
/// every part that reads one key gets the very same states until they change: the list of conversations and the bar,
/// which counts only what the list shows. A list whose values all start shown and may grow later, as tags do, keeps
/// those it hides instead, so that a value new to it shows.
const shownByKey = new Map<string, { text: string | null | undefined; states: string[] }>();

/// The parts that read what a `StateFilter` shows, told when the owner changes it.
const shownReaders = new Set<() => void>();

const readShown = (told: () => void) => {
  shownReaders.add(told);
  return () => {
    shownReaders.delete(told);
  };
};

/// The states kept under `key`, `first` while none are kept or what is kept is not states of `all`.
function shownStates<S extends string>(key: string, all: readonly S[], first: readonly S[]): S[] {
  const text = keptText(key);
  const was = shownByKey.get(key);
  if (was !== undefined && (text === undefined || text === was.text)) return was.states as S[];
  let states = [...first];
  try {
    const v: unknown = text != null ? JSON.parse(text) : null;
    if (Array.isArray(v) && v.every((s) => typeof s === 'string' && (all as readonly string[]).includes(s))) {
      states = v as S[];
    }
  } catch {
    // What is kept is not states: the first ones.
  }
  shownByKey.set(key, { text, states });
  return states;
}

/// The states whose rows a list shows, or that the owner picked, as the owner set them with its `StateFilter`, one
/// choice for every project in the list, kept in this browser under `key`; `first` until the owner sets them. With the
/// way to turn one on or off, and to set them all at once (the observations' clear of every pick). Every part that
/// reads one key reads the same states and changes with them.
export function useShownStates<S extends string>(
  key: string,
  all: readonly S[],
  first: readonly S[],
): [S[], (s: S) => void, (states: readonly S[]) => void] {
  const read = () => shownStates(key, all, first);
  const shown = useSyncExternalStore(readShown, read, read);
  const set = (states: readonly S[]) => {
    const next = [...states];
    keep(key, next);
    shownByKey.set(key, { text: keptText(key), states: next });
    for (const told of shownReaders) told();
  };
  const toggle = (s: S) => {
    const now = read();
    set(now.includes(s) ? now.filter((x) => x !== s) : [...now, s]);
  };
  return [shown, toggle, set];
}

/// The buttons in a project's head (`ProjectHead`), at the right end of its row, in a list (the items' tree, the works in
/// the list of conversations and in the list of works, the observations) that show or hide its rows in each state, or
/// pick its rows by tag and by state: one for each state
/// among them, in the list's order of states (`order`), with how many are in it (`states`, the state of each row, a
/// row with several tags once for each); each the state's mark (`mark`, a work's state's or an item's) and its count
/// with no words, so that the row of buttons fits on one line, the state, its count and what a click does said on
/// hovering and as the button's name (`title`); a tag, which has no mark, its name (`words`) and its count; each a fill
/// with no edge, in its state's colour as the state's chip while it is on and in its tint while it is off, a tag in
/// the neutral one, which stands for no meaning of its own, from the one set of a state's colours that the items' cards
/// take too (items.css). By default a button is on while its rows show, and a click shows or hides them, a state with
/// no row having no button. With `pick` (the observations') a button is on while it is picked (`shown`): the list
/// shows every row while nothing is picked and else only those the picks match, and each button in `order` is drawn,
/// its count what it would show under the other picks, none among them, so that a pick keeps its button to be taken
/// back by.
export function StateFilter<S extends string>(props: {
  order: readonly S[];
  states: readonly S[];
  shown: readonly S[];
  tone: Record<S, Tone> | Tone;
  mark?: (s: S) => ElementType;
  words?: (s: S) => string;
  title: (s: S, n: number, shown: boolean) => string;
  onShown: (s: S) => void;
  pick?: boolean;
}) {
  return (
    <span className="tree-states">
      {props.order.map((s) => {
        const n = props.states.filter((x) => x === s).length;
        if (n === 0 && props.pick !== true) return null;
        const on = props.shown.includes(s);
        const said = props.title(s, n, on);
        const tone = typeof props.tone === 'string' ? props.tone : props.tone[s];
        return (
          <Button
            key={s}
            size="small"
            variant="invisible"
            className={`tree-state tree-tone-${tone}`}
            leadingVisual={props.mark?.(s)}
            count={n}
            aria-pressed={on}
            aria-label={said}
            title={said}
            onClick={() => props.onShown(s)}
          >
            {props.words?.(s)}
          </Button>
        );
      })}
    </span>
  );
}

/// The buttons over a project's works that show or hide them by state, one part for the list of conversations and the
/// list of works: `StateFilter` in a work's states, their order, colours and marks, saying on hovering how many of the
/// project's works are in a state and what a click does. `states` is the state of each of the project's works as it is
/// in effect (`liveState`), `shown` the states the owner chose to show, one choice for both lists (`useTalkStates`).
export function WorkStateFilter({ states, shown, onShown }: {
  states: readonly WorkState[];
  shown: readonly WorkState[];
  onShown: (s: WorkState) => void;
}) {
  return (
    <StateFilter
      order={WORK_STATES}
      states={states}
      shown={shown}
      tone={WORK_TONE}
      mark={workStateMark}
      title={(s, n, on) => tn(on ? 'talks_hide' : 'talks_show', n, { state: t(`state_${s}`) })}
      onShown={onShown}
    />
  );
}

/// The command that takes back every pick of a list's `StateFilter`s in their `pick` mode, shown only while something
/// is picked: a small invisible `Button` with an `XIcon`, as GitHub clears a list's filters, saying on hovering what it
/// does.
export function ClearPicks({ title, onClear }: { title: string; onClear: () => void }) {
  return (
    <Button size="small" variant="invisible" className="filter-clear" leadingVisual={XIcon} title={title} onClick={onClear}>
      {t('filter_clear')}
    </Button>
  );
}

const OUTCOME_TONE: Record<RoundOutcome, Tone> = {
  running: 'accent',
  took: 'success',
  sent_back: 'attention',
  failed: 'danger',
  interrupted: 'danger',
  ended: 'neutral',
};

/// A work's state as its chip; waiting for a free slot says on hovering why the work does not move, what it waited for
/// come.
export function WorkBadge({ state }: { state: WorkState }) {
  const why =
    state === 'waiting_for_slot'
      ? t('course_slot_why')
      : state === 'stopping'
        ? t('course_stopping_why')
        : undefined;
  return (
    <StateChip tone={WORK_TONE[state]} title={why}>
      {t(`state_${state}`)}
    </StateChip>
  );
}

export function ItemBadge({ state }: { state: ItemState }) {
  return <StateChip tone={ITEM_TONE[state]}>{t(`item_${state}`)}</StateChip>;
}

/// The kind of a dependency in a word, blocking or parallel, with what it means; nothing for none.
export function DependencyKind({ kind }: { kind: string | null }) {
  if (kind === null) return null;
  const blocking = kind === 'blocking';
  return (
    <span className="muted" title={t(blocking ? 'dependency_blocking_title' : 'dependency_parallel_title')}>
      {t(blocking ? 'dependency_blocking' : 'dependency_parallel')}
    </span>
  );
}

export function OutcomeBadge({ outcome }: { outcome: RoundOutcome }) {
  return <StateChip tone={OUTCOME_TONE[outcome]}>{t(`outcome_${outcome}`)}</StateChip>;
}

/// What a read gives: its view when it came, or that it is still coming, or why it failed.
export function Loaded<T>({ r, children }: { r: Resource<T>; children: (data: T) => ReactNode }) {
  if (r.data !== undefined) return <>{children(r.data)}</>;
  if (r.error !== null) return <Flash variant="danger">{t('error', { error: r.error })}</Flash>;
  return <div className="muted">{t('loading')}</div>;
}
