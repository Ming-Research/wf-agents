// The System view: how the whole system stands, in parts, each for what the owner does with it. Now says whether the
// owner needs to step in: the engine, its slots, the wait every work shares, the copies of the database, the works by
// where they stand, and the rounds under way, each with what its calls have come to so far against the lines it is
// judged by, read again as the engine says it changed. Produced says what got done, Waited what the works stopped for and
// for how long, how long CI took them and how many slots they held, Spent where the money and the time went, by project
// too, and Gone wrong what was wasted or failed, each over the span the owner picks in the view's
// line, as links: the week under way, the four weeks to its end, or all time. Each part sums up first, in numbers that
// stand alone, over every project; a number that counts a list unfolds it under the numbers. Each thing is said once:
// the engine's pause and its slots in Now, and the rounds under way in their list, not among the numbers of the works.

import { Fragment, useCallback, useEffect, useMemo, useState } from 'react';

import { CHOICE_KEYS, write } from '../kept';
import { Coded, PanelPart, RoundLink, SpanLinks } from '../parts';
import { hashOf, type Period } from '../route';
import type { Copies, Engine, Item, Items, Spent } from '../works-api';
import { NowLine } from '../works/marks';
import { useChanges, useGoings, useItems, useReading, useSource } from '../works/source';
import { waitingCount } from '../works/waiting';
import {
  allTokens,
  clock,
  dollars,
  duration,
  failureWords,
  fullTime,
  lineWords,
  outageWords,
  tokens,
  tokensWords,
  waitCounted,
} from '../works/words';
import { type Figure, type Going, type RanOn, type Rest, type System, type UnderWay, useSystemSource } from './api';
import { copiesTone, copiesWords } from './copies';
import { ItemName, ProjectName, Tile, Tiles, ViewTable, WorkLink } from './parts';
import { redMainWords } from './redMain';
import {
  crowdWords,
  endedWords,
  takingWords,
  took,
  WAIT_REASONS,
  WAIT_WORDS,
  WAITED_WORDS,
  waitedBy,
  weekCells,
  works,
} from './waited';
import {
  cacheShare,
  mergeAverages,
  percent,
  STANDING_WORDS,
  STANDINGS,
  type Standing,
  standingOf,
  spentPerMerge,
  summed,
  nextWeekStart,
  weekStarts,
} from './standing';

const SPANS: { value: Period; name: string; href: string; title: string }[] = [
  { value: 'week', name: 'This week', href: hashOf({ view: 'system', period: 'week' }), title: "From this week's Monday" },
  {
    value: '4w',
    name: '4 weeks',
    href: hashOf({ view: 'system', period: '4w' }),
    title: 'From the Monday three weeks before this one',
  },
  { value: 'all', name: 'All time', href: hashOf({ view: 'system', period: 'all' }), title: 'Everything the engine recorded' },
];

/// How many rows of a long list are drawn before the rest are counted.
const ROWS = 50;

export function SystemView({ period, active = true }: { period: Period; active?: boolean }) {
  const works = useSource();
  const system = useSystemSource();
  const items = useItems();
  const loadEngine = useCallback(
    () => (works.loadEngine === undefined ? Promise.resolve(null) : works.loadEngine()),
    [works],
  );
  const engine = useReading(loadEngine);
  const loadCopies = useCallback(() => works.loadCopies(), [works]);
  const copies = useReading(loadCopies);
  // All time is counted from the week the earliest item was added in, once the items are read. The weeks are cut anew
  // from the time each read is made, so that a week begins at its Monday's midnight however long the view is open,
  // and the span is read again then.
  const earliest = useMemo(
    () =>
      period !== 'all' || items.value === null
        ? null
        : Math.min(Date.now(), ...items.value.items.map((item) => item.added)),
    [period, items.value],
  );
  const waiting = period === 'all' && earliest === null;
  const loadSpan = useCallback(
    () =>
      waiting ? new Promise<System>(() => undefined) : system.loadSystem(weekStarts(period, Date.now(), earliest)),
    [system, period, earliest, waiting],
  );
  const span = useReading(loadSpan);
  // What the calls of the rounds under way have come to, read again each time the engine says it changed while the view
  // is shown, and once more when it is shown again, having been told nothing while hidden.
  const loadGoing = useCallback(() => system.loadUnderWay(), [system]);
  const going = useReading(loadGoing);
  useGoings(going.read, active);
  // The span the view is over is the one the owner last picked, kept in the browser for an address that names none.
  useEffect(() => write(CHOICE_KEYS.systemPeriod, period), [period]);
  const readSpan = span.read;
  useEffect(() => {
    const now = Date.now();
    const timer = window.setTimeout(readSpan, nextWeekStart(now) - now);
    return () => window.clearTimeout(timer);
  }, [readSpan, span.value]);
  const readAll = useCallback(() => {
    engine.read();
    copies.read();
    span.read();
  }, [engine, copies, span]);
  useChanges((changed) => {
    if (changed.copies) copies.read();
    if (changed.items || changed.works.length > 0) {
      engine.read();
      span.read();
    }
  }, readAll);

  const byNumber = useMemo(
    () => new Map((items.value?.items ?? []).map((item) => [item.number, item])),
    [items.value],
  );
  return (
    <>
      <div className="view-line">
        <span className="view-name">System</span>
        <span className="view-end">
          <SpanLinks<Period> label="The span the numbers count over" value={period} choices={SPANS} />
        </span>
      </div>
      {[items.failed, engine.failed, span.failed, going.failed].some((f) => f !== null) && (
        <p className="read-failed" role="alert">
          {[
            items.failed && `The works could not be read: ${items.failed}.`,
            engine.failed && `The engine could not be read: ${engine.failed}.`,
            span.failed && `The span could not be read: ${span.failed}.`,
            going.failed && `What the rounds under way have come to could not be read: ${going.failed}.`,
          ]
            .filter(Boolean)
            .join(' ')}{' '}
          What is shown is from before.
        </p>
      )}
      <main className="view-page">
        {items.value === null ? (
          <p className="message muted">Reading the system</p>
        ) : (
          <NowPart
            items={items.value}
            engine={engine.value}
            copies={copies.value}
            copiesFailed={copies.failed}
            system={span.value}
            going={going.value}
          />
        )}
        {span.value === null ? (
          span.failed === null && <p className="message muted">Reading the span</p>
        ) : (
          <>
            <Produced s={span.value} items={byNumber} />
            <WaitedPart s={span.value} />
            <SpentPart s={span.value} items={byNumber} />
            <GoneWrong s={span.value} items={byNumber} />
          </>
        )}
      </main>
    </>
  );
}

/// Whether the owner needs to step in: the engine, the wait every work shares, the copies; the works by where they
/// stand, each count unfolding its works; and the rounds under way. While a stop of the engine stands, the engine says
/// when it is expected to end, and each round under way how much longer it is expected to take, as the span read last
/// gives them, and what each agent's round's calls have come to so far as the engine last said it.
function NowPart({ items, engine, copies, copiesFailed, system, going }: {
  items: Items;
  engine: Engine | null;
  copies: Copies | null;
  copiesFailed: string | null;
  system: System | null;
  going: Going[] | null;
}) {
  const [open, setOpen] = useState<Standing | null>(null);
  // The works whose round is under way are in the list of the rounds under way, not among the numbers.
  const by = STANDINGS.filter((standing) => standing !== 'running').map((standing) => ({
    standing,
    list: items.items.filter((item) => standingOf(item.work) === standing),
  }));
  const running = items.items.filter((item) => item.work.now.kind === 'round');
  // The wait every work shares, said only where it holds a work.
  const shared = items.outage ? outageWords(items.outage) : null;
  const opened = by.find((b) => b.standing === open);
  const stopping = engine?.stopping ?? null;
  const heads = ['Work', 'Item', 'Project', 'Cell', 'Since', ...(stopping === null ? [] : ['To go']), ...CALLS];
  // When the engine is expected to end: once the last of the rounds under way has.
  const last = system?.stopping?.rest ?? null;
  const ends =
    last === null
      ? ''
      : last.kind === 'expected'
        ? `, expected in about ${duration(last.ms)}`
        : ', one of them having taken longer than any round of its cell before';
  return (
    <PanelPart label="Now">
      <dl className="pairs view-pairs">
        <dt>Engine</dt>
        <dd>
          {engine === null
            ? 'not given'
            : stopping !== null
              ? `Stopping since ${clock(stopping.since)}${stopping.atOnce ? ', at once' : ''}: no round begins, and the engine ends once the rounds under way have ended${ends}`
              : engine.paused === null
                ? 'Every work goes on'
                : `Every work paused since ${clock(engine.paused.since)}, by the ${engine.paused.by}: no work begins a round but to answer a note`}
        </dd>
        <dt title="Rounds of agents under way, of how many may be at once: agents.slots">Slots</dt>
        <dd>{engine === null ? 'not given' : `${engine.taken} taken of ${engine.slots}`}</dd>
        <dt>The wait every work shares</dt>
        <dd className={shared === null ? undefined : 'failed'}>{shared === null ? 'none' : <Coded text={shared} />}</dd>
        {/* A project whose main fails a test on its head: its works wait on what no change of theirs mends. */}
        {(system?.redMain ?? []).map((red) => (
          <Fragment key={red.project}>
            <dt>Main of {red.project}</dt>
            <dd className="failed">{redMainWords(red)}</dd>
          </Fragment>
        ))}
        {/* An engine that keeps no copies, one that runs a script, has none to show. */}
        {copies?.state.kind !== 'notKept' && (
          <>
            <dt>Copies of the database</dt>
            <dd>
              {copies === null ? (
                <span className={copiesFailed ? 'failed' : 'muted'}>
                  {copiesFailed ? `could not be read: ${copiesFailed}` : 'reading'}
                </span>
              ) : (
                <CopiesFacts copies={copies} />
              )}
            </dd>
          </>
        )}
      </dl>
      <Tiles>
        {by.map(({ standing, list }) => {
          // What waits for the owner is counted by the one count, the things that wait, not the works they wait in.
          const waiting = standing === 'owner' ? waitingCount(list.map((item) => item.work)) : null;
          return (
            <Tile
              key={standing}
              value={waiting ?? list.length}
              label={waiting === null ? STANDING_WORDS[standing].name : waitCounted(waiting)}
              title={STANDING_WORDS[standing].title}
              open={open === standing}
              onOpen={list.length > 0 ? () => setOpen(open === standing ? null : standing) : undefined}
            />
          );
        })}
      </Tiles>
      {opened !== undefined && opened.list.length > 0 && (
        <ViewTable heads={['Work', 'Item', 'Now', 'Last']} label={STANDING_WORDS[opened.standing].name}>
          {opened.list.map((item) => (
            <tr key={item.work.number}>
              <td>
                <WorkLink work={item.work.number} />
              </td>
              <td className="view-cell-wide">
                <ItemName item={item.number} title={item.title} />
              </td>
              <td className="view-cell-wide">
                <NowLine work={item.work} />
              </td>
              <td className="small muted" title={item.work.last === null ? undefined : fullTime(item.work.last)}>
                {item.work.last === null ? 'nothing yet' : clock(item.work.last)}
              </td>
            </tr>
          ))}
        </ViewTable>
      )}
      <h4 className="view-sub label">Rounds under way {running.length}</h4>
      {running.length === 0 ? (
        <p className="muted">No round is under way</p>
      ) : (
        <ViewTable heads={heads} numbers={CALLS.map((_, n) => heads.length - CALLS.length + n)}>
          {running.map((item) => (
            <RunningRow
              key={item.work.number}
              item={item}
              rests={stopping === null ? null : (system?.underWay ?? [])}
              going={going?.find((g) => runsAs(item, g))}
            />
          ))}
        </ViewTable>
      )}
    </PanelPart>
  );
}

/// The columns of what a round's calls have come to so far.
const CALLS = ['Context', 'Added', 'Per call', 'Cost'];

/// Whether the figures are those of the round the item's work has under way.
function runsAs(item: Item, g: Going): boolean {
  const now = item.work.now;
  return now.kind === 'round' && g.work === item.work.number && g.run === now.run && g.round === now.round;
}

/// What the run guard stops a round on, said where a figure it is judged by is.
const GUARD =
  'The run guard stops a round only where its growth over the round and its growth for each call after the first are both past their lines.';

/// A round under way, with how much longer it is expected to take where `rests` gives what is under way, and what its
/// calls have come to so far where `going` gives it.
function RunningRow({ item, rests, going }: { item: Item; rests: UnderWay[] | null; going: Going | undefined }) {
  const now = item.work.now;
  if (now.kind !== 'round') return null;
  const rest = rests?.find(
    (u) => u.kind === 'round' && u.work === item.work.number && u.run === now.run && u.round === now.round,
  );
  return (
    <tr>
      <td>
        <RoundLink work={item.work.number} run={now.run} round={now.round} />
      </td>
      <td className="view-cell-wide">
        <ItemName item={item.number} title={item.title} />
      </td>
      <td>
        <ProjectName project={item.project} />
      </td>
      <td className="mono">{now.cell}</td>
      <td title={fullTime(now.since)}>{clock(now.since)}</td>
      {rests !== null && (
        <td title="How much longer the round is expected to take: the median of what the rounds of its cell that took longer had left at this point">
          {rest?.kind === 'round' && rest.rest ? restWords(rest.rest) : 'not known'}
        </td>
      )}
      <td className="num">
        {going?.context == null ? (
          <NotYet />
        ) : (
          <span title="The whole prompt of the round's latest call, in tokens">{tokens(going.context)}</span>
        )}
      </td>
      <td className="num">
        <FigureCell
          figure={going?.added}
          unit={tokens}
          says={`The tokens the round's context has grown by since it began. ${GUARD}`}
        />
      </td>
      <td className="num">
        <FigureCell
          figure={going?.perCall}
          unit={tokens}
          says={`The tokens its context has grown by for each call after the first, told from the second call. ${GUARD}`}
        />
      </td>
      <td className="num">
        <FigureCell
          figure={going?.cost}
          unit={dollars}
          says="What the round's calls cost so far, priced from the calls at their model's prices."
        />
      </td>
    </tr>
  );
}

/// What a round has no figure of yet: it has made no call, or it is an engine's own act, which makes none.
function NotYet() {
  return (
    <span className="muted" title="Told once the round has made a call; an engine's own act makes none">
      none yet
    </span>
  );
}

/// A figure of a round under way, with the line it is judged by beside it where the history gives one, in the colour of
/// a warning where it is past it; the pointer's tip says what it is and what gave the line.
function FigureCell({ figure, unit, says }: {
  figure: Figure | null | undefined;
  unit: (n: number) => string;
  says: string;
}) {
  if (figure === null || figure === undefined) return <NotYet />;
  const line = figure.line;
  const title =
    line === null
      ? `${says} No line yet: too few rounds to give one.`
      : `${says} Its line is ${unit(line.value)}, which ${lineWords(line)}; it is ${figure.past ? 'past' : 'within'} it.`;
  return (
    <span className={figure.past ? 'warn-text' : undefined} title={title}>
      {unit(figure.value)}
      {line !== null && <span className={figure.past ? undefined : 'muted'}> of {unit(line.value)}</span>}
    </span>
  );
}

/// How much longer something under way is expected to take, in words.
function restWords(rest: Rest): string {
  return rest.kind === 'expected' ? `about ${duration(rest.ms)}` : 'longer than any before';
}

/// How the copies stand, and when the newest was taken and kept off the machine, and the last that failed.
function CopiesFacts({ copies }: { copies: Copies }) {
  const tone = copiesTone(copies);
  const at = (time: number | null) => (time === null ? 'never' : clock(time));
  return (
    <>
      <span className={tone === 'quiet' ? undefined : tone === 'failed' ? 'failed' : 'warn-text'}>{copiesWords(copies)}</span>
      <span className="muted">
        {' '}
        · newest taken {at(copies.lastTaken)} · newest kept off this machine {at(copies.lastUploaded)}
        {copies.pending > 0 && ` · ${copies.pending} not yet kept off it`}
        {copies.lastFailure !== null &&
          ` · last failed ${clock(copies.lastFailure.at)}, at ${copies.lastFailure.step}: ${copies.lastFailure.why}`}
      </span>
    </>
  );
}

/// What the span produced: the works merged and cancelled and the items added, each count unfolding its list; how
/// long a merge took from its first round, and what it cost.
function Produced({ s, items }: { s: System; items: Map<number, Item> }) {
  const [open, setOpen] = useState<'merged' | 'cancelled' | 'added' | null>(null);
  const toggle = (what: 'merged' | 'cancelled' | 'added', n: number) =>
    n > 0 ? () => setOpen(open === what ? null : what) : undefined;
  const avg = mergeAverages(s.merged);
  const perMerge = spentPerMerge(s.spent, s.merged);
  return (
    <PanelPart label="Produced">
      <Tiles>
        <Tile value={s.merged.length} label="Works merged" open={open === 'merged'} onOpen={toggle('merged', s.merged.length)} />
        <Tile
          value={s.cancelled.length}
          label="Works cancelled"
          open={open === 'cancelled'}
          onOpen={toggle('cancelled', s.cancelled.length)}
        />
        <Tile value={s.added.length} label="Items added" open={open === 'added'} onOpen={toggle('added', s.added.length)} />
        <Tile
          value={avg?.took == null ? 'none' : duration(avg.took)}
          label="To merge, on average"
          title="From a merged work's first round to its merge"
        />
        <Tile
          value={perMerge === null ? 'none' : dollars(perMerge)}
          more={avg === null ? undefined : `the merged works' own: ${dollars(avg.usdMicros)}`}
          label="A merge's cost, on average"
          title="What the span spent in all, the rounds that ended in it and the keep-warm pings, for each work merged in it: what the works that did not merge and the rounds sent back or failed cost is in it. Beside it, what the merged works cost themselves on average, from their first round to the merge, some of which may be before the span."
        />
      </Tiles>
      {open === 'merged' && (
        <ViewTable heads={['Item', 'Work', 'Merged', 'Took', 'Under way', 'Cost']} numbers={[3, 4, 5]}>
          {s.merged.map((m) => (
            <tr key={m.work}>
              <td className="view-cell-wide">
                <ItemName item={m.item} title={items.get(m.item)?.title} />
              </td>
              <td>
                <WorkLink work={m.work} />
              </td>
              <td title={fullTime(m.at)}>{clock(m.at)}</td>
              <td className="num">{m.from === null ? 'none' : duration(m.at - m.from)}</td>
              <td className="num">{duration(m.spent.workingMs)}</td>
              <td className="num">{dollars(m.spent.usdMicros)}</td>
            </tr>
          ))}
        </ViewTable>
      )}
      {open === 'cancelled' && (
        <ViewTable heads={['Item', 'Work', 'Cancelled', 'Why']}>
          {s.cancelled.map((c) => (
            <tr key={c.work}>
              <td className="view-cell-wide">
                <ItemName item={c.item} title={items.get(c.item)?.title} />
              </td>
              <td>
                <WorkLink work={c.work} />
              </td>
              <td title={fullTime(c.at)}>{clock(c.at)}</td>
              <td className="view-cell-wide" title={c.why ?? undefined}>
                {c.why ?? <span className="muted">no reason given</span>}
              </td>
            </tr>
          ))}
        </ViewTable>
      )}
      {open === 'added' && (
        <ViewTable heads={['Item', 'Work', 'Added']}>
          {s.added.slice(0, ROWS).map((a) => (
            <tr key={a.work}>
              <td className="view-cell-wide">
                <ItemName item={a.item} title={items.get(a.item)?.title} />
              </td>
              <td>
                <WorkLink work={a.work} />
              </td>
              <td title={fullTime(a.at)}>{clock(a.at)}</td>
            </tr>
          ))}
        </ViewTable>
      )}
      <More shown={open === 'added' ? Math.min(ROWS, s.added.length) : 0} of={open === 'added' ? s.added.length : 0} />
    </PanelPart>
  );
}

/// What the works waited for over the span: for each cause how long they stopped, how often and for how long at a time;
/// how long the calls and rounds that waited for CI took, and how long its jobs waited for a runner; how many works held
/// a slot and how many ran, against how many slots there were; and, over more than one week, each week by itself. What
/// the engine does not know is said, never shown as nothing.
function WaitedPart({ s }: { s: System }) {
  const waited = s.waited;
  if (waited === undefined) return null;
  const ci = waited.ci;
  const o = waited.occupancy;
  const stopsWords = (stops: number, what: string) => `${stops} ${stops === 1 ? what : `${what}s`}`;
  return (
    <PanelPart label="Waited">
      <Tiles>
        {waitedBy(waited).map((by) => (
          <Tile
            key={by.reason}
            value={took(by.totalMs)}
            more={[stopsWords(by.stops, 'stop'), endedWords(by)].filter(Boolean).join(' · ')}
            label={WAIT_WORDS[by.reason].name}
            title={`${WAIT_WORDS[by.reason].title}. ${
              by.longestMs === null ? '' : `The longest stop that ended: ${duration(by.longestMs)}. `
            }Counted where a stop ended; a stop still going on counts its time so far.`}
          />
        ))}
      </Tiles>
      <Tiles>
        <Tile
          value={took(ci.totalMs)}
          more={[stopsWords(ci.stops, 'call'), endedWords(ci)].filter(Boolean).join(' · ')}
          label={WAITED_WORDS.ci}
          title={WAITED_WORDS.ciTitle}
        />
        <Tile
          value={ci.queuedMedianMs === null ? 'not kept' : duration(ci.queuedMedianMs)}
          more={
            ci.queuedLongestMs === null
              ? undefined
              : `${WAITED_WORDS.longest} ${duration(ci.queuedLongestMs)} · ${ci.jobs} jobs`
          }
          label={WAITED_WORDS.queued}
          title={`${WAITED_WORDS.queuedTitle}. Jobs run before the engine kept it say none.`}
        />
        <Tile
          value={works(o.takingAvg)}
          more={`${o.takingPeak} at most, of ${o.slots === null ? WAITED_WORDS.notKept : o.slots}`}
          label={WAITED_WORDS.taking}
          title={`${WAITED_WORDS.takingTitle}${o.recorded ? '' : `. ${WAITED_WORDS.notKeptTitle}`}`}
        />
        <Tile
          value={works(o.runningAvg)}
          more={`${o.runningPeak} at most`}
          label={WAITED_WORDS.running}
          title={WAITED_WORDS.runningTitle}
        />
      </Tiles>
      {s.weeks.length > 1 && (
        <>
          <h4 className="view-sub label">Week by week</h4>
          <ViewTable
            heads={[
              WAITED_WORDS.week,
              ...WAIT_REASONS.map((reason) => WAIT_WORDS[reason].name),
              WAITED_WORDS.ci,
              WAITED_WORDS.taking,
              WAITED_WORDS.running,
            ]}
            numbers={[1, 2, 3, 4, 5, 6, 7, 8]}
          >
            {s.weeks.map((w) => (
              <tr key={w.start}>
                <td title={fullTime(w.start)}>{clock(w.start).slice(0, 5)}</td>
                {weekCells(w).map((cell, i) => (
                  <td key={WAIT_REASONS[i]} className="num" title={WAIT_WORDS[WAIT_REASONS[i]].title}>
                    {cell}
                  </td>
                ))}
                <td className="num">{w.waited === undefined ? '' : took(w.waited.ciMs)}</td>
                <td className="num">{w.waited === undefined ? '' : takingWords(w.waited.occupancy)}</td>
                <td className="num">
                  {w.waited === undefined
                    ? ''
                    : crowdWords(w.waited.occupancy.runningAvg, w.waited.occupancy.runningPeak)}
                </td>
              </tr>
            ))}
          </ViewTable>
        </>
      )}
      {waited.unseparated.totalMs > 0 && (
        <p className="small muted">
          {took(waited.unseparated.totalMs)} {WAITED_WORDS.unseparated}
        </p>
      )}
      {waited.uncounted > 0 && (
        <p className="small muted">
          {waited.uncounted} {WAITED_WORDS.uncounted}
        </p>
      )}
    </PanelPart>
  );
}

/// What a round ran on, in the table by model.
function ranOnWords(on: RanOn) {
  switch (on.kind) {
    case 'model':
      return on.model;
    case 'act':
      return <span className="muted">none: the engine's own acts</span>;
    case 'notRecorded':
      return <span className="muted">not recorded: agents' rounds kept before their tries were</span>;
  }
}

/// What a list leaves out after the rows it draws.
function More({ shown, of }: { shown: number; of: number }) {
  return of > shown ? <p className="small muted">and {of - shown} more, the earliest</p> : null;
}

/// A cost's share of the span's.
const shareOf = (spent: Spent, of: Spent) => (of.usdMicros === 0 ? 'none' : percent(spent.usdMicros / of.usdMicros));

/// Where the span's money and time went: in all, week by week over more than one week, by project, by cell, by model,
/// and by work, the costliest first. The projects together spent what the span did.
function SpentPart({ s, items }: { s: System; items: Map<number, Item> }) {
  const share = cacheShare(s.spent);
  return (
    <PanelPart label="Spent">
      <Tiles>
        <Tile value={dollars(s.spent.usdMicros)} label="Cost" />
        <Tile value={s.spent.rounds} label="Rounds" />
        <Tile value={duration(s.spent.workingMs)} label="Under way" title="How long the rounds were under way" />
        <Tile value={tokens(allTokens(s.spent))} label="Tokens" more={tokensWords(s.spent)} />
        <Tile
          value={share === null ? 'none' : percent(share)}
          label="Read from the cache"
          title="The share of the tokens sent that the prompt cache gave"
        />
        {s.pinged && s.pinged.rounds > 0 && (
          <Tile
            value={dollars(s.pinged.usdMicros)}
            label="Keep-warm pings"
            title={`${s.pinged.rounds} pings, which keep the sessions of lines that wait in the prompt cache`}
          />
        )}
      </Tiles>
      {s.weeks.length > 1 && (
        <>
          <h4 className="view-sub label">Week by week</h4>
          <ViewTable
            heads={['Week of', 'Cost', 'Rounds', 'Pings', 'Merged', 'Cancelled', 'Added', 'Sent back', 'Failed']}
            numbers={[1, 2, 3, 4, 5, 6, 7, 8]}
          >
            {s.weeks.map((w) => (
              <tr key={w.start}>
                <td title={fullTime(w.start)}>{clock(w.start).slice(0, 5)}</td>
                <td className="num">{dollars(w.spent.usdMicros)}</td>
                <td className="num">{w.spent.rounds}</td>
                <td className="num" title={w.pinged ? `${w.pinged.rounds} keep-warm pings` : undefined}>
                  {dollars(w.pinged?.usdMicros ?? 0)}
                </td>
                <td className="num">{w.merged}</td>
                <td className="num">{w.cancelled}</td>
                <td className="num">{w.added}</td>
                <td className="num">{w.sentBack}</td>
                <td className="num">{w.failed}</td>
              </tr>
            ))}
          </ViewTable>
        </>
      )}
      {s.byProject.length > 0 && (
        <>
          <h4 className="view-sub label">By project</h4>
          <ViewTable heads={['Project', 'Rounds', 'Under way', 'Cost', 'Share']} numbers={[1, 2, 3, 4]}>
            {s.byProject.map((p) => (
              <tr key={JSON.stringify(p.project)}>
                <td>
                  <ProjectName project={p.project} />
                </td>
                <td className="num">{p.spent.rounds}</td>
                <td className="num">{duration(p.spent.workingMs)}</td>
                <td className="num">{dollars(p.spent.usdMicros)}</td>
                <td className="num">{shareOf(p.spent, s.spent)}</td>
              </tr>
            ))}
          </ViewTable>
        </>
      )}
      <h4 className="view-sub label">By cell</h4>
      {s.byCell.length === 0 ? (
        <p className="muted">No round ended in the span</p>
      ) : (
        <ViewTable heads={['Cell', 'Flow', 'Rounds', 'Under way', 'Cost', 'Share']} numbers={[2, 3, 4, 5]}>
          {s.byCell.map((c) => (
            <tr key={`${c.flow}/${c.cell}`}>
              <td className="mono">{c.cell}</td>
              <td className="mono muted">{c.flow}</td>
              <td className="num">{c.spent.rounds}</td>
              <td className="num">{duration(c.spent.workingMs)}</td>
              <td className="num">{dollars(c.spent.usdMicros)}</td>
              <td className="num">{shareOf(c.spent, s.spent)}</td>
            </tr>
          ))}
        </ViewTable>
      )}
      {s.byModel.length > 0 && (
        <>
          <h4 className="view-sub label">By model</h4>
          <ViewTable heads={['Model', 'Rounds', 'Tokens', 'Cost', 'Share']} numbers={[1, 2, 3, 4]}>
            {s.byModel.map((m) => (
              <tr key={m.on.kind === 'model' ? m.on.model : m.on.kind}>
                <td className={m.on.kind === 'model' ? 'mono' : undefined}>{ranOnWords(m.on)}</td>
                <td className="num">{m.spent.rounds}</td>
                <td className="num" title={tokensWords(m.spent)}>
                  {tokens(allTokens(m.spent))}
                </td>
                <td className="num">{dollars(m.spent.usdMicros)}</td>
                <td className="num">{shareOf(m.spent, s.spent)}</td>
              </tr>
            ))}
          </ViewTable>
        </>
      )}
      {s.byWork.length > 0 && (
        <>
          <h4 className="view-sub label">The costliest works</h4>
          <ViewTable heads={['Work', 'Item', 'Rounds', 'Under way', 'Cost', 'Share']} numbers={[2, 3, 4, 5]}>
            {s.byWork.slice(0, 10).map((w) => (
              <tr key={w.work}>
                <td>
                  <WorkLink work={w.work} />
                </td>
                <td className="view-cell-wide">
                  <ItemName item={w.item} title={items.get(w.item)?.title} />
                </td>
                <td className="num">{w.spent.rounds}</td>
                <td className="num">{duration(w.spent.workingMs)}</td>
                <td className="num">{dollars(w.spent.usdMicros)}</td>
                <td className="num">{shareOf(w.spent, s.spent)}</td>
              </tr>
            ))}
          </ViewTable>
          {s.byWork.length > 10 && <p className="small muted">and {s.byWork.length - 10} more</p>}
        </>
      )}
    </PanelPart>
  );
}

/// What the span wasted or saw go wrong: the rounds sent back and what they cost, each count unfolding its rounds;
/// and the rounds that failed, the latest first, each with its cause.
function GoneWrong({ s, items }: { s: System; items: Map<number, Item> }) {
  const [openSent, setOpenSent] = useState(false);
  const sentBack = summed(s.sentBack);
  const failed = summed(s.failed);
  return (
    <PanelPart label="Gone wrong">
      <Tiles>
        <Tile
          value={s.sentBack.length}
          label="Rounds sent back"
          more={s.sentBack.length > 0 ? dollars(sentBack.usdMicros) : undefined}
          title="Rounds given back what the round before handed in, which did not meet what the flow requires"
          open={openSent}
          onOpen={s.sentBack.length > 0 ? () => setOpenSent(!openSent) : undefined}
        />
        <Tile
          value={s.failed.length}
          label="Rounds failed"
          more={s.failed.length > 0 ? dollars(failed.usdMicros) : undefined}
        />
      </Tiles>
      {openSent && (
        <ViewTable heads={['Began', 'Work', 'Item', 'Cell', 'Cost']} numbers={[4]}>
          {s.sentBack.slice(0, ROWS).map((r) => (
            <tr key={`${r.work}/${r.run}/${r.round}`}>
              <td title={fullTime(r.began)}>{clock(r.began)}</td>
              <td>
                <RoundLink work={r.work} run={r.run} round={r.round} />
              </td>
              <td className="view-cell-wide">
                <ItemName item={r.item} title={items.get(r.item)?.title} />
              </td>
              <td className="mono">{r.cell}</td>
              <td className="num">{r.spent === null ? 'under way' : dollars(r.spent.usdMicros)}</td>
            </tr>
          ))}
        </ViewTable>
      )}
      {openSent && <More shown={Math.min(ROWS, s.sentBack.length)} of={s.sentBack.length} />}
      <h4 className="view-sub label">The latest rounds that failed</h4>
      {s.failed.length === 0 ? (
        <p className="muted">No round failed in the span</p>
      ) : (
        <>
          <ViewTable heads={['Ended', 'Work', 'Item', 'Cell', 'Cause', 'Why']}>
            {s.failed.slice(0, ROWS).map((f) => (
              <tr key={`${f.work}/${f.run}/${f.round}`}>
                <td title={fullTime(f.ended)}>{clock(f.ended)}</td>
                <td>
                  <RoundLink work={f.work} run={f.run} round={f.round} />
                </td>
                <td className="view-cell-wide">
                  <ItemName item={f.item} title={items.get(f.item)?.title} />
                </td>
                <td className="mono">{f.cell}</td>
                <td title={f.failure}>{failureWords(f.failure)}</td>
                <td className="view-cell-wide failed" title={f.why}>
                  {f.why}
                </td>
              </tr>
            ))}
          </ViewTable>
          <More shown={Math.min(ROWS, s.failed.length)} of={s.failed.length} />
        </>
      )}
    </PanelPart>
  );
}
